//! Rvalue lowering from MIR to Cranelift IR.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{InstBuilder, Value};

use crate::codegen::error::CodegenError;
use crate::mir::operand::{BinOp, CastKind, Operand, Rvalue, UnOp};
use crate::mir::types::Local;
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

            Rvalue::Ref(_, _) => Err(CodegenError::Internal(
                "references not yet supported".to_string(),
            )),

            Rvalue::AddressOf(_, _) => Err(CodegenError::Internal(
                "address_of not yet supported".to_string(),
            )),

            Rvalue::Len(_) => Err(CodegenError::Internal("len not yet supported".to_string())),

            Rvalue::Aggregate(_, _) => Err(CodegenError::Internal(
                "aggregates not yet supported".to_string(),
            )),

            Rvalue::Discriminant(_) => Err(CodegenError::Internal(
                "discriminant not yet supported".to_string(),
            )),

            Rvalue::Repeat(_, _) => Err(CodegenError::Internal(
                "repeat not yet supported".to_string(),
            )),
        }
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
                crate::mir::operand::Constant::Int(_) => Ok(types::I64),
                crate::mir::operand::Constant::Bool(_) => Ok(types::I8),
                crate::mir::operand::Constant::Float(_) => Ok(types::F64),
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
