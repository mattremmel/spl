//! Operand lowering from MIR to Cranelift IR.

use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{InstBuilder, Value};

use crate::codegen::error::CodegenError;
use crate::mir::operand::{Constant, Operand};
use crate::mir::types::Place;

use super::FunctionLowerer;

impl<'a> FunctionLowerer<'a> {
    /// Lower a MIR operand to a Cranelift value.
    pub(super) fn lower_operand(
        &mut self,
        operand: &Operand,
    ) -> Result<Option<Value>, CodegenError> {
        match operand {
            Operand::Copy(place) => self.lower_place_read(place),
            Operand::Move(place) => self.lower_place_read(place),
            Operand::Constant(constant) => self.lower_constant(constant),
        }
    }

    /// Read a value from a MIR place.
    fn lower_place_read(&mut self, place: &Place) -> Result<Option<Value>, CodegenError> {
        if !place.is_local() {
            return Err(CodegenError::Internal(
                "place projections not yet supported".to_string(),
            ));
        }

        Ok(self.use_var(place.local))
    }

    /// Lower a MIR constant to a Cranelift value.
    pub(super) fn lower_constant(
        &mut self,
        constant: &Constant,
    ) -> Result<Option<Value>, CodegenError> {
        match constant {
            Constant::Int(value) => {
                // Default to i64 for constants; will be cast as needed
                let val = self.builder.ins().iconst(types::I64, *value as i64);
                Ok(Some(val))
            }
            Constant::Bool(value) => {
                let val = self.builder.ins().iconst(types::I8, *value as i64);
                Ok(Some(val))
            }
            Constant::Float(value) => {
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
            Constant::FnDef(_) => Err(CodegenError::Internal(
                "function references not yet supported".to_string(),
            )),
            Constant::Zeroed(_) => Err(CodegenError::Internal(
                "zeroed constants not yet supported".to_string(),
            )),
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
                        "cannot coerce {:?} to {:?}",
                        actual_ty, expected_ty
                    )))
                }
            }
            None => Ok(None),
        }
    }
}
