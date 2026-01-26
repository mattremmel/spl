//! Rvalue lowering from MIR to Cranelift IR.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{InstBuilder, MemFlags, Value};

use crate::codegen::LocalStorage;
use crate::codegen::error::CodegenError;
use crate::mir::operand::{AggregateKind, BinOp, CastKind, Constant, Operand, Rvalue, UnOp};
use crate::mir::types::{Local, Place};
use crate::sema::types::TypeId;

use super::FunctionLowerer;

impl<'a> FunctionLowerer<'a> {
    /// Lower a MIR rvalue to a Cranelift value.
    pub(super) fn lower_rvalue(
        &mut self,
        rvalue: &Rvalue,
        dest: Local,
    ) -> Result<Option<Value>, CodegenError> {
        match rvalue {
            Rvalue::Use(operand) => {
                // Special handling for string constants (compound type)
                if let Operand::Constant(Constant::String(s)) = operand {
                    // String is a compound type - write directly to destination
                    let dest_addr = self.local_stack_addr(dest).ok_or_else(|| {
                        CodegenError::Internal(
                            "string constant requires stack slot destination".to_string(),
                        )
                    })?;
                    let dest_ty = self.local_spl_type(dest);
                    self.lower_string_constant_to(s, dest_addr, dest_ty)?;
                    return Ok(None); // Compound type, no single value returned
                }

                // Get the destination type
                if let Some(dest_ty) = self.local_type(dest) {
                    self.lower_operand_as(operand, dest_ty)
                } else {
                    // ZST destination
                    Ok(None)
                }
            }

            Rvalue::BinaryOp(op, lhs, rhs) => self.lower_binary_op(*op, lhs, rhs, dest),

            Rvalue::UnaryOp(op, operand) => self.lower_unary_op(*op, operand, dest),

            Rvalue::Cast(kind, operand, target_ty) => self.lower_cast(*kind, operand, *target_ty),

            Rvalue::Ref(_, place, _) | Rvalue::AddressOf(_, place, _) => {
                // Both Ref and AddressOf produce a pointer to the place
                self.lower_address_of(place)
            }

            Rvalue::Len(place) => {
                // Get the type of the place (should be an array)
                let place_ty = self.local_spl_type(place.local);
                let ty_data = self.types.get(place_ty);
                match ty_data {
                    crate::sema::types::Type::Array(_, count) => {
                        // Return the static array length as a pointer-sized constant
                        let ptr_ty = self.type_mapper.pointer_type();
                        Ok(Some(self.builder.ins().iconst(ptr_ty, *count as i64)))
                    }
                    _ => Err(CodegenError::Internal("len on non-array type".to_string())),
                }
            }

            Rvalue::Aggregate(kind, operands) => {
                // Aggregates write directly to the destination
                self.lower_aggregate(dest, kind, operands)
            }

            Rvalue::Discriminant(place) => {
                // Discriminant reads the tag field (field 0) from an enum
                // Enums are represented as structs where the first field is the discriminant

                // Get the address of the place
                let (addr, _place_ty) = self.compute_place_address(place)?;

                // Discriminant is always at offset 0 (first field)
                // The discriminant type is isize (pointer-sized integer)
                let disc_ty = self.type_mapper.pointer_type();

                // Load the discriminant value from offset 0
                let flags = MemFlags::trusted();
                let disc_value = self.builder.ins().load(disc_ty, flags, addr, 0);

                Ok(Some(disc_value))
            }

            Rvalue::Repeat(operand, count) => self.lower_repeat(dest, operand, *count),
        }
    }

    /// Lower an rvalue and ensure it produces a value of the expected Cranelift type.
    pub(super) fn lower_rvalue_for_type(
        &mut self,
        rvalue: &Rvalue,
        expected_ty: types::Type,
    ) -> Result<Option<Value>, CodegenError> {
        // For most rvalues, we can use a dummy local to get the type
        // This is a simplified version - for now just handle common cases
        match rvalue {
            Rvalue::Use(operand) => self.lower_operand_as(operand, expected_ty),

            Rvalue::Ref(_, place, _) | Rvalue::AddressOf(_, place, _) => {
                self.lower_address_of(place)
            }

            Rvalue::BinaryOp(op, lhs, rhs) => {
                // For binary ops, infer the operand type from lhs
                let operand_ty = self.infer_operand_type(lhs)?;

                let lhs_val = self.lower_operand_as(lhs, operand_ty)?.ok_or_else(|| {
                    CodegenError::Internal("ZST operand in binary op".to_string())
                })?;
                let rhs_val = self.lower_operand_as(rhs, operand_ty)?.ok_or_else(|| {
                    CodegenError::Internal("ZST operand in binary op".to_string())
                })?;

                let is_float = self.is_float_type(operand_ty);
                let result = self.compute_binary_op(*op, lhs_val, rhs_val, is_float)?;

                // Coerce result to expected type if needed
                let result_ty = self.builder.func.dfg.value_type(result);
                if result_ty == expected_ty {
                    Ok(Some(result))
                } else if result_ty.is_int() && expected_ty.is_int() {
                    if result_ty.bits() < expected_ty.bits() {
                        Ok(Some(self.builder.ins().uextend(expected_ty, result)))
                    } else {
                        Ok(Some(self.builder.ins().ireduce(expected_ty, result)))
                    }
                } else {
                    Ok(Some(result))
                }
            }

            Rvalue::UnaryOp(op, operand) => {
                let val = self
                    .lower_operand_as(operand, expected_ty)?
                    .ok_or_else(|| CodegenError::Internal("ZST operand in unary op".to_string()))?;

                let result = match op {
                    UnOp::Neg => {
                        if self.is_float_type(expected_ty) {
                            self.builder.ins().fneg(val)
                        } else {
                            self.builder.ins().ineg(val)
                        }
                    }
                    UnOp::Not => {
                        if expected_ty == types::I8 {
                            let one = self.builder.ins().iconst(types::I8, 1);
                            self.builder.ins().bxor(val, one)
                        } else {
                            self.builder.ins().bnot(val)
                        }
                    }
                };

                Ok(Some(result))
            }

            Rvalue::Cast(kind, operand, target_ty) => self.lower_cast(*kind, operand, *target_ty),

            _ => Err(CodegenError::Internal(
                "rvalue type not supported in lower_rvalue_for_type".to_string(),
            )),
        }
    }

    /// Lower a reference or address-of operation.
    fn lower_address_of(&mut self, place: &Place) -> Result<Option<Value>, CodegenError> {
        if place.is_local() {
            // Get the address of a simple local
            match self.local_storage(place.local) {
                Some(LocalStorage::StackSlot(slot)) => {
                    let ptr_ty = self.type_mapper.pointer_type();
                    Ok(Some(self.builder.ins().stack_addr(ptr_ty, slot, 0)))
                }
                Some(LocalStorage::Variable(_)) => {
                    // Can't take address of SSA variable directly
                    // This would require spilling to stack
                    Err(CodegenError::Internal(
                        "cannot take address of SSA variable".to_string(),
                    ))
                }
                Some(LocalStorage::Zst) => {
                    // ZST has a valid but arbitrary address
                    let ptr_ty = self.type_mapper.pointer_type();
                    Ok(Some(self.builder.ins().iconst(ptr_ty, 1))) // Non-null but arbitrary
                }
                None => Err(CodegenError::Internal(format!(
                    "local {:?} not found in local_map",
                    place.local
                ))),
            }
        } else {
            // Place with projections - compute the address
            let (addr, _ty) = self.compute_place_address(place)?;
            Ok(Some(addr))
        }
    }

    /// Lower an aggregate construction.
    fn lower_aggregate(
        &mut self,
        dest: Local,
        kind: &AggregateKind,
        operands: &[Operand],
    ) -> Result<Option<Value>, CodegenError> {
        // Get the destination address
        let dest_addr = match self.local_storage(dest) {
            Some(LocalStorage::StackSlot(slot)) => {
                let ptr_ty = self.type_mapper.pointer_type();
                self.builder.ins().stack_addr(ptr_ty, slot, 0)
            }
            Some(LocalStorage::Variable(_)) => {
                // If destination is a variable, we can't construct an aggregate into it
                return Err(CodegenError::Internal(
                    "cannot construct aggregate into SSA variable".to_string(),
                ));
            }
            Some(LocalStorage::Zst) => {
                // ZST aggregate - nothing to store
                return Ok(None);
            }
            None => {
                return Err(CodegenError::Internal(format!(
                    "local {dest:?} not found in local_map"
                )));
            }
        };

        let dest_ty = self.local_spl_type(dest);

        match kind {
            AggregateKind::Tuple | AggregateKind::Adt(_) => {
                // Store each field at its offset
                for (i, operand) in operands.iter().enumerate() {
                    let field_offset = self.layout.field_offset(dest_ty, i);
                    let field_ty = self.layout.field_type(dest_ty, i);

                    if let Some(field_ty) = field_ty
                        && let Some(clif_ty) = self.type_mapper.map_type(field_ty, self.types)
                        && let Some(val) = self.lower_operand_as(operand, clif_ty)?
                    {
                        let field_addr = if field_offset == 0 {
                            dest_addr
                        } else {
                            self.builder.ins().iadd_imm(dest_addr, field_offset as i64)
                        };
                        let flags = MemFlags::trusted();
                        self.builder.ins().store(flags, val, field_addr, 0);
                    }
                    // If field is ZST, skip it
                }
            }

            AggregateKind::Array => {
                // Store each element at its stride offset
                let stride = self.layout.element_stride(dest_ty);
                let elem_ty = self.layout.element_type(dest_ty);

                if let Some(elem_ty) = elem_ty
                    && let Some(clif_ty) = self.type_mapper.map_type(elem_ty, self.types)
                {
                    for (i, operand) in operands.iter().enumerate() {
                        if let Some(val) = self.lower_operand_as(operand, clif_ty)? {
                            let offset = (i as u32) * stride;
                            let elem_addr = if offset == 0 {
                                dest_addr
                            } else {
                                self.builder.ins().iadd_imm(dest_addr, offset as i64)
                            };
                            let flags = MemFlags::trusted();
                            self.builder.ins().store(flags, val, elem_addr, 0);
                        }
                    }
                }
            }
        }

        // Aggregates don't return a direct Value
        Ok(None)
    }

    /// Lower a repeat expression [value; count].
    fn lower_repeat(
        &mut self,
        dest: Local,
        operand: &Operand,
        count: u64,
    ) -> Result<Option<Value>, CodegenError> {
        if count == 0 {
            return Ok(None);
        }

        // Get the destination address
        let dest_addr = match self.local_storage(dest) {
            Some(LocalStorage::StackSlot(slot)) => {
                let ptr_ty = self.type_mapper.pointer_type();
                self.builder.ins().stack_addr(ptr_ty, slot, 0)
            }
            Some(LocalStorage::Zst) => return Ok(None),
            _ => {
                return Err(CodegenError::Internal(
                    "cannot construct repeat into SSA variable".to_string(),
                ));
            }
        };

        let dest_ty = self.local_spl_type(dest);
        let stride = self.layout.element_stride(dest_ty);
        let elem_ty = self.layout.element_type(dest_ty);

        if let Some(elem_ty) = elem_ty
            && let Some(clif_ty) = self.type_mapper.map_type(elem_ty, self.types)
            && let Some(val) = self.lower_operand_as(operand, clif_ty)?
        {
            // Store the value at each position
            for i in 0..count {
                let offset = (i as u32) * stride;
                let elem_addr = if offset == 0 {
                    dest_addr
                } else {
                    self.builder.ins().iadd_imm(dest_addr, offset as i64)
                };
                let flags = MemFlags::trusted();
                self.builder.ins().store(flags, val, elem_addr, 0);
            }
        }

        Ok(None)
    }

    /// Compute a binary operation result.
    fn compute_binary_op(
        &mut self,
        op: BinOp,
        lhs_val: Value,
        rhs_val: Value,
        is_float: bool,
    ) -> Result<Value, CodegenError> {
        let result = match op {
            BinOp::Add => {
                if is_float {
                    self.builder.ins().fadd(lhs_val, rhs_val)
                } else {
                    self.builder.ins().iadd(lhs_val, rhs_val)
                }
            }
            BinOp::Sub => {
                if is_float {
                    self.builder.ins().fsub(lhs_val, rhs_val)
                } else {
                    self.builder.ins().isub(lhs_val, rhs_val)
                }
            }
            BinOp::Mul => {
                if is_float {
                    self.builder.ins().fmul(lhs_val, rhs_val)
                } else {
                    self.builder.ins().imul(lhs_val, rhs_val)
                }
            }
            BinOp::Div => {
                if is_float {
                    self.builder.ins().fdiv(lhs_val, rhs_val)
                } else {
                    self.builder.ins().sdiv(lhs_val, rhs_val)
                }
            }
            BinOp::Rem => {
                if is_float {
                    return Err(CodegenError::Internal(
                        "float remainder not supported".to_string(),
                    ));
                }
                self.builder.ins().srem(lhs_val, rhs_val)
            }
            BinOp::BitAnd => self.builder.ins().band(lhs_val, rhs_val),
            BinOp::BitOr => self.builder.ins().bor(lhs_val, rhs_val),
            BinOp::BitXor => self.builder.ins().bxor(lhs_val, rhs_val),
            BinOp::Shl => self.builder.ins().ishl(lhs_val, rhs_val),
            BinOp::Shr => self.builder.ins().sshr(lhs_val, rhs_val),
            BinOp::Eq => {
                if is_float {
                    self.builder.ins().fcmp(FloatCC::Equal, lhs_val, rhs_val)
                } else {
                    self.builder.ins().icmp(IntCC::Equal, lhs_val, rhs_val)
                }
            }
            BinOp::Ne => {
                if is_float {
                    self.builder.ins().fcmp(FloatCC::NotEqual, lhs_val, rhs_val)
                } else {
                    self.builder.ins().icmp(IntCC::NotEqual, lhs_val, rhs_val)
                }
            }
            BinOp::Lt => {
                if is_float {
                    self.builder.ins().fcmp(FloatCC::LessThan, lhs_val, rhs_val)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedLessThan, lhs_val, rhs_val)
                }
            }
            BinOp::Le => {
                if is_float {
                    self.builder
                        .ins()
                        .fcmp(FloatCC::LessThanOrEqual, lhs_val, rhs_val)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedLessThanOrEqual, lhs_val, rhs_val)
                }
            }
            BinOp::Gt => {
                if is_float {
                    self.builder
                        .ins()
                        .fcmp(FloatCC::GreaterThan, lhs_val, rhs_val)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThan, lhs_val, rhs_val)
                }
            }
            BinOp::Ge => {
                if is_float {
                    self.builder
                        .ins()
                        .fcmp(FloatCC::GreaterThanOrEqual, lhs_val, rhs_val)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, lhs_val, rhs_val)
                }
            }
        };
        Ok(result)
    }

    /// Lower a binary operation.
    fn lower_binary_op(
        &mut self,
        op: BinOp,
        lhs: &Operand,
        rhs: &Operand,
        dest: Local,
    ) -> Result<Option<Value>, CodegenError> {
        // Get the operand type from the destination for most ops,
        // but comparisons return bool while operating on the operand types
        let operand_ty = self.infer_operand_type(lhs)?;

        let lhs_val = self
            .lower_operand_as(lhs, operand_ty)?
            .ok_or_else(|| CodegenError::Internal("ZST operand in binary op".to_string()))?;
        let rhs_val = self
            .lower_operand_as(rhs, operand_ty)?
            .ok_or_else(|| CodegenError::Internal("ZST operand in binary op".to_string()))?;

        let is_float = self.is_float_type(operand_ty);

        let result = match op {
            // Arithmetic operations
            BinOp::Add => {
                if is_float {
                    self.builder.ins().fadd(lhs_val, rhs_val)
                } else {
                    self.builder.ins().iadd(lhs_val, rhs_val)
                }
            }
            BinOp::Sub => {
                if is_float {
                    self.builder.ins().fsub(lhs_val, rhs_val)
                } else {
                    self.builder.ins().isub(lhs_val, rhs_val)
                }
            }
            BinOp::Mul => {
                if is_float {
                    self.builder.ins().fmul(lhs_val, rhs_val)
                } else {
                    self.builder.ins().imul(lhs_val, rhs_val)
                }
            }
            BinOp::Div => {
                if is_float {
                    self.builder.ins().fdiv(lhs_val, rhs_val)
                } else {
                    self.builder.ins().sdiv(lhs_val, rhs_val)
                }
            }
            BinOp::Rem => {
                if is_float {
                    return Err(CodegenError::Internal(
                        "float remainder not supported".to_string(),
                    ));
                }
                self.builder.ins().srem(lhs_val, rhs_val)
            }

            // Bitwise operations
            BinOp::BitAnd => self.builder.ins().band(lhs_val, rhs_val),
            BinOp::BitOr => self.builder.ins().bor(lhs_val, rhs_val),
            BinOp::BitXor => self.builder.ins().bxor(lhs_val, rhs_val),
            BinOp::Shl => self.builder.ins().ishl(lhs_val, rhs_val),
            BinOp::Shr => self.builder.ins().sshr(lhs_val, rhs_val),

            // Comparison operations
            BinOp::Eq => {
                if is_float {
                    self.builder.ins().fcmp(FloatCC::Equal, lhs_val, rhs_val)
                } else {
                    self.builder.ins().icmp(IntCC::Equal, lhs_val, rhs_val)
                }
            }
            BinOp::Ne => {
                if is_float {
                    self.builder.ins().fcmp(FloatCC::NotEqual, lhs_val, rhs_val)
                } else {
                    self.builder.ins().icmp(IntCC::NotEqual, lhs_val, rhs_val)
                }
            }
            BinOp::Lt => {
                if is_float {
                    self.builder.ins().fcmp(FloatCC::LessThan, lhs_val, rhs_val)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedLessThan, lhs_val, rhs_val)
                }
            }
            BinOp::Le => {
                if is_float {
                    self.builder
                        .ins()
                        .fcmp(FloatCC::LessThanOrEqual, lhs_val, rhs_val)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedLessThanOrEqual, lhs_val, rhs_val)
                }
            }
            BinOp::Gt => {
                if is_float {
                    self.builder
                        .ins()
                        .fcmp(FloatCC::GreaterThan, lhs_val, rhs_val)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThan, lhs_val, rhs_val)
                }
            }
            BinOp::Ge => {
                if is_float {
                    self.builder
                        .ins()
                        .fcmp(FloatCC::GreaterThanOrEqual, lhs_val, rhs_val)
                } else {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, lhs_val, rhs_val)
                }
            }
        };

        // For comparisons, the result is i8 (bool), may need to extend for the destination
        let dest_ty = self.local_type(dest);
        if let Some(ty) = dest_ty {
            let result_ty = self.builder.func.dfg.value_type(result);
            if result_ty != ty && result_ty.is_int() && ty.is_int() {
                if result_ty.bits() < ty.bits() {
                    return Ok(Some(self.builder.ins().uextend(ty, result)));
                } else if result_ty.bits() > ty.bits() {
                    return Ok(Some(self.builder.ins().ireduce(ty, result)));
                }
            }
        }

        Ok(Some(result))
    }

    /// Lower a unary operation.
    fn lower_unary_op(
        &mut self,
        op: UnOp,
        operand: &Operand,
        dest: Local,
    ) -> Result<Option<Value>, CodegenError> {
        let dest_ty = self
            .local_type(dest)
            .ok_or_else(|| CodegenError::Internal("ZST destination for unary op".to_string()))?;

        let val = self
            .lower_operand_as(operand, dest_ty)?
            .ok_or_else(|| CodegenError::Internal("ZST operand in unary op".to_string()))?;

        let result = match op {
            UnOp::Neg => {
                if self.is_float_type(dest_ty) {
                    self.builder.ins().fneg(val)
                } else {
                    self.builder.ins().ineg(val)
                }
            }
            UnOp::Not => {
                // For bool (i8), we flip the bit: 1 -> 0, 0 -> 1
                // For integers, we do bitwise NOT (bnot)
                if dest_ty == types::I8 {
                    // Boolean NOT: XOR with 1
                    let one = self.builder.ins().iconst(types::I8, 1);
                    self.builder.ins().bxor(val, one)
                } else {
                    // Integer bitwise NOT
                    self.builder.ins().bnot(val)
                }
            }
        };

        Ok(Some(result))
    }

    /// Lower a cast operation.
    fn lower_cast(
        &mut self,
        kind: CastKind,
        operand: &Operand,
        target_ty: TypeId,
    ) -> Result<Option<Value>, CodegenError> {
        let target_clif_ty = self
            .type_mapper
            .map_type(target_ty, self.types)
            .ok_or_else(|| CodegenError::Internal("ZST target in cast".to_string()))?;

        // Get the source value
        let source_ty = self.infer_operand_type(operand)?;
        let val = self
            .lower_operand_as(operand, source_ty)?
            .ok_or_else(|| CodegenError::Internal("ZST source in cast".to_string()))?;

        let result = match kind {
            CastKind::IntToInt => {
                let source_bits = source_ty.bits();
                let target_bits = target_clif_ty.bits();

                if source_bits < target_bits {
                    // Widening: sign extend
                    self.builder.ins().sextend(target_clif_ty, val)
                } else if source_bits > target_bits {
                    // Narrowing: truncate
                    self.builder.ins().ireduce(target_clif_ty, val)
                } else {
                    // Same size: no-op
                    val
                }
            }
            CastKind::IntToFloat => self.builder.ins().fcvt_from_sint(target_clif_ty, val),
            CastKind::FloatToInt => self.builder.ins().fcvt_to_sint_sat(target_clif_ty, val),
            CastKind::FloatToFloat => {
                let source_bits = source_ty.bits();
                let target_bits = target_clif_ty.bits();

                if source_bits < target_bits {
                    self.builder.ins().fpromote(target_clif_ty, val)
                } else if source_bits > target_bits {
                    self.builder.ins().fdemote(target_clif_ty, val)
                } else {
                    val
                }
            }
            CastKind::PtrToPtr => {
                // Pointers are just integers at the CLIF level
                val
            }
            CastKind::Unsize => {
                return Err(CodegenError::Internal(
                    "unsize casts not yet supported".to_string(),
                ));
            }
        };

        Ok(Some(result))
    }

    /// Infer the Cranelift type for an operand.
    fn infer_operand_type(&self, operand: &Operand) -> Result<types::Type, CodegenError> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self
                .local_type(place.local)
                .ok_or_else(|| CodegenError::Internal("ZST local".to_string())),
            Operand::Constant(constant) => match constant {
                crate::mir::operand::Constant::Int(..) => Ok(types::I64),
                crate::mir::operand::Constant::Bool(_) => Ok(types::I8),
                crate::mir::operand::Constant::Float(..) => Ok(types::F64),
                crate::mir::operand::Constant::Char(_) => Ok(types::I32),
                crate::mir::operand::Constant::Unit => {
                    Err(CodegenError::Internal("unit constant".to_string()))
                }
                crate::mir::operand::Constant::String(_) => {
                    Err(CodegenError::Internal("string constant".to_string()))
                }
                crate::mir::operand::Constant::FnDef(_) => {
                    Err(CodegenError::Internal("fn def constant".to_string()))
                }
                crate::mir::operand::Constant::Zeroed(ty) => self
                    .type_mapper
                    .map_type(*ty, self.types)
                    .ok_or_else(|| CodegenError::Internal("ZST zeroed".to_string())),
            },
        }
    }
}
