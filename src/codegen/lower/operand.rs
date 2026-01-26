//! Operand lowering from MIR to Cranelift IR.

use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{InstBuilder, MemFlags, Value};

use crate::codegen::LocalStorage;
use crate::codegen::error::CodegenError;
use crate::mir::operand::{Constant, Operand};
use crate::mir::types::{Local, Place, PlaceElem};
use crate::sema::types::TypeId;

use super::FunctionLowerer;

impl<'a> FunctionLowerer<'a> {
    /// Lower a MIR operand to a Cranelift value.
    pub(super) fn lower_operand(
        &mut self,
        operand: &Operand,
    ) -> Result<Option<Value>, CodegenError> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self.lower_place_read(place),
            Operand::Constant(constant) => self.lower_constant(constant),
        }
    }

    /// Read a value from a MIR place.
    pub(super) fn lower_place_read(
        &mut self,
        place: &Place,
    ) -> Result<Option<Value>, CodegenError> {
        // Simple case: just a local with no projections
        if place.is_local() {
            // Check if it's a stack slot or variable
            match self.local_storage(place.local) {
                Some(LocalStorage::Variable(_)) => return Ok(self.use_var(place.local)),
                Some(LocalStorage::StackSlot(_)) => {
                    // Load the entire value from stack (only works for scalar-sized types)
                    if let Some(clif_ty) = self.local_type(place.local) {
                        let addr = self.local_stack_addr(place.local).ok_or_else(|| {
                            CodegenError::Internal("expected stack slot".to_string())
                        })?;
                        return Ok(Some(self.load_from_addr(addr, clif_ty)));
                    }
                    // Compound type with no Cranelift type - return None (ZST-like for reads)
                    return Ok(None);
                }
                Some(LocalStorage::Zst) => return Ok(None),
                None => {
                    return Err(CodegenError::Internal(format!(
                        "local {:?} not found in local_map",
                        place.local  // Debug formatting required
                    )));
                }
            }
        }

        // Complex case: projections (field, deref, index, etc.)
        let (addr, result_ty) = self.compute_place_address(place)?;

        // Load the value from the computed address
        if let Some(clif_ty) = self.type_mapper.map_type(result_ty, self.types) {
            let flags = MemFlags::trusted();
            Ok(Some(self.builder.ins().load(clif_ty, flags, addr, 0)))
        } else if self.type_mapper.is_zst(result_ty, self.types) {
            Ok(None)
        } else {
            // Compound type - can't load directly into a value
            // This case happens when reading a compound type through a projection
            // The caller should handle this by copying memory
            Err(CodegenError::Internal(
                "cannot load compound type from place projection".to_string(),
            ))
        }
    }

    /// Compute the address of a place with projections.
    ///
    /// Returns the address as a Cranelift Value and the SPL type at that address.
    pub(super) fn compute_place_address(
        &mut self,
        place: &Place,
    ) -> Result<(Value, TypeId), CodegenError> {
        let mut current_ty = self.local_spl_type(place.local);
        let mut projections = place.projection.iter().peekable();

        // Special case: if the base local is an SSA variable and the first projection
        // is Deref, we use the variable's value directly as the pointer (no load needed).
        // This handles patterns like *ptr where ptr is a pointer passed as an argument.
        let mut addr = if matches!(projections.peek(), Some(PlaceElem::Deref))
            && matches!(
                self.local_storage(place.local),
                Some(LocalStorage::Variable(_))
            ) {
            // Get the pointer value directly from the SSA variable
            let ptr = self
                .use_var(place.local)
                .ok_or_else(|| CodegenError::Internal("SSA variable not found".to_string()))?;

            // Consume the Deref projection
            projections.next();

            // Update current_ty to the pointee type
            current_ty = self
                .layout
                .pointee_type(current_ty)
                .ok_or_else(|| CodegenError::Internal("deref on non-pointer type".to_string()))?;

            ptr
        } else {
            self.get_local_address(place.local)?
        };

        // Apply remaining projections
        for proj in projections {
            match proj {
                PlaceElem::Deref => {
                    // Load the pointer value, then use it as the new address
                    let ptr_ty = self.type_mapper.pointer_type();
                    let flags = MemFlags::trusted();
                    addr = self.builder.ins().load(ptr_ty, flags, addr, 0);

                    // Update current_ty to the pointee type
                    current_ty = self.layout.pointee_type(current_ty).ok_or_else(|| {
                        CodegenError::Internal("deref on non-pointer type".to_string())
                    })?;
                }

                PlaceElem::Field(field_idx) => {
                    // Add field offset to the address
                    let offset = self
                        .layout
                        .field_offset(current_ty, field_idx.index() as usize);
                    if offset != 0 {
                        addr = self.builder.ins().iadd_imm(addr, offset as i64);
                    }

                    // Update current_ty to the field type
                    current_ty = self
                        .layout
                        .field_type(current_ty, field_idx.index() as usize)
                        .ok_or_else(|| {
                            let idx = field_idx.index();
                            CodegenError::Internal(format!("field {idx} not found in type"))
                        })?;
                }

                PlaceElem::Index(index_local) => {
                    // Get the index value
                    let index_val = self.use_var(*index_local).ok_or_else(|| {
                        CodegenError::Internal("index local not found".to_string())
                    })?;

                    // Get the element stride
                    let stride = self.layout.element_stride(current_ty);

                    // Compute offset = index * stride
                    let ptr_ty = self.type_mapper.pointer_type();

                    // Extend index to pointer size if needed
                    let index_val_ext = {
                        let val_ty = self.builder.func.dfg.value_type(index_val);
                        if val_ty.bits() < ptr_ty.bits() {
                            self.builder.ins().uextend(ptr_ty, index_val)
                        } else if val_ty.bits() > ptr_ty.bits() {
                            self.builder.ins().ireduce(ptr_ty, index_val)
                        } else {
                            index_val
                        }
                    };

                    let stride_val = self.builder.ins().iconst(ptr_ty, stride as i64);
                    let offset = self.builder.ins().imul(index_val_ext, stride_val);
                    addr = self.builder.ins().iadd(addr, offset);

                    // Update current_ty to the element type
                    current_ty = self.layout.element_type(current_ty).ok_or_else(|| {
                        CodegenError::Internal("index on non-array type".to_string())
                    })?;
                }

                PlaceElem::ConstantIndex { offset, from_end } => {
                    if *from_end {
                        return Err(CodegenError::Internal(
                            "constant index from_end not yet supported".to_string(),
                        ));
                    }

                    // Get the element stride
                    let stride = self.layout.element_stride(current_ty);
                    let byte_offset = (*offset as u32) * stride;

                    if byte_offset != 0 {
                        addr = self.builder.ins().iadd_imm(addr, byte_offset as i64);
                    }

                    // Update current_ty to the element type
                    current_ty = self.layout.element_type(current_ty).ok_or_else(|| {
                        CodegenError::Internal("constant index on non-array type".to_string())
                    })?;
                }

                PlaceElem::Subslice { .. } => {
                    return Err(CodegenError::Internal(
                        "subslice projections not yet supported".to_string(),
                    ));
                }

                PlaceElem::Downcast(_) => {
                    // For now, downcast doesn't change the address (enum discriminant handling)
                    // The variant data starts at the same address as the enum
                    // In the future, we may need to handle enum layouts properly
                }
            }
        }

        Ok((addr, current_ty))
    }

    /// Get the address of a local (either stack slot address or address of the variable).
    fn get_local_address(&mut self, local: Local) -> Result<Value, CodegenError> {
        match self.local_storage(local) {
            Some(LocalStorage::StackSlot(slot)) => {
                let ptr_ty = self.type_mapper.pointer_type();
                Ok(self.builder.ins().stack_addr(ptr_ty, slot, 0))
            }
            Some(LocalStorage::Variable(_var)) => {
                // For variables, we need to spill to stack to get an address
                // This is a limitation - we need to allocate a temporary stack slot
                // For now, error out - the caller should handle this case
                Err(CodegenError::Internal(format!(
                    "cannot take address of SSA variable {local:?}"
                )))
            }
            Some(LocalStorage::Zst) => {
                // ZST has no address, but we can return a dummy pointer
                let ptr_ty = self.type_mapper.pointer_type();
                Ok(self.builder.ins().iconst(ptr_ty, 0))
            }
            None => Err(CodegenError::Internal(format!(
                "local {local:?} not found in local_map"
            ))),
        }
    }

    /// Lower a MIR constant to a Cranelift value.
    pub(super) fn lower_constant(
        &mut self,
        constant: &Constant,
    ) -> Result<Option<Value>, CodegenError> {
        match constant {
            Constant::Int(value, _ty) => {
                // Default to i64 for constants; will be cast as needed
                let val = self.builder.ins().iconst(types::I64, *value as i64);
                Ok(Some(val))
            }
            Constant::Bool(value) => {
                let val = self.builder.ins().iconst(types::I8, *value as i64);
                Ok(Some(val))
            }
            Constant::Float(value, _ty) => {
                let val = self.builder.ins().f64const(*value);
                Ok(Some(val))
            }
            Constant::Char(value) => {
                let val = self.builder.ins().iconst(types::I32, *value as i64);
                Ok(Some(val))
            }
            Constant::Unit => {
                // Unit has no runtime representation
                Ok(None)
            }
            Constant::String(_) => Err(CodegenError::Internal(
                "string constants not yet supported".to_string(),
            )),
            Constant::FnDef(def_id) => {
                // Look up the function in the registry
                let registry = self.func_registry.ok_or_else(|| {
                    CodegenError::Internal(
                        "function registry required for FnDef constants".to_string(),
                    )
                })?;

                let func_info = registry.get(*def_id).ok_or_else(|| {
                    CodegenError::Internal(format!("function {def_id:?} not found in registry"))
                })?;

                // Get the module to import the function reference
                let module = self.module.as_mut().ok_or_else(|| {
                    CodegenError::Internal("module required for FnDef constants".to_string())
                })?;

                // Import the function into the current function being built
                let func_ref = module.declare_func_in_func(func_info.func_id, self.builder.func);

                // Get the function's address as a pointer value
                let ptr_type = self.type_mapper.pointer_type();
                let addr = self.builder.ins().func_addr(ptr_type, func_ref);

                Ok(Some(addr))
            }
            Constant::Zeroed(ty) => {
                if let Some(clif_ty) = self.type_mapper.map_type(*ty, self.types) {
                    // Scalar type: emit zero constant
                    let val = if clif_ty.is_float() {
                        if clif_ty == types::F32 {
                            self.builder.ins().f32const(0.0)
                        } else {
                            self.builder.ins().f64const(0.0)
                        }
                    } else {
                        self.builder.ins().iconst(clif_ty, 0)
                    };
                    Ok(Some(val))
                } else if self.type_mapper.is_zst(*ty, self.types) {
                    // ZST - no value needed
                    Ok(None)
                } else {
                    // Compound types need special handling (memset to 0)
                    Err(CodegenError::Internal(
                        "zeroed compound types not yet supported".to_string(),
                    ))
                }
            }
        }
    }

    /// Lower an operand and ensure it has a specific Cranelift type.
    ///
    /// This handles type coercion when the operand type doesn't match
    /// the expected type (e.g., i64 constant used where i32 is needed).
    pub(super) fn lower_operand_as(
        &mut self,
        operand: &Operand,
        expected_ty: types::Type,
    ) -> Result<Option<Value>, CodegenError> {
        let val = self.lower_operand(operand)?;

        match val {
            Some(v) => {
                let actual_ty = self.builder.func.dfg.value_type(v);
                if actual_ty == expected_ty {
                    Ok(Some(v))
                } else if actual_ty.is_int() && expected_ty.is_int() {
                    // Integer type coercion
                    if actual_ty.bits() > expected_ty.bits() {
                        // Truncate
                        Ok(Some(self.builder.ins().ireduce(expected_ty, v)))
                    } else {
                        // Extend (sign extend for signed semantics)
                        Ok(Some(self.builder.ins().sextend(expected_ty, v)))
                    }
                } else {
                    Err(CodegenError::Internal(format!(
                        "cannot coerce {actual_ty:?} to {expected_ty:?}"
                    )))
                }
            }
            None => Ok(None),
        }
    }
}
