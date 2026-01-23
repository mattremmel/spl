//! Statement lowering from MIR to Cranelift IR.

use crate::codegen::error::CodegenError;
use crate::mir::statement::{Statement, StatementKind};

use super::FunctionLowerer;

impl<'a> FunctionLowerer<'a> {
    /// Lower a MIR statement to Cranelift IR.
    pub(super) fn lower_statement(&mut self, stmt: &Statement) -> Result<(), CodegenError> {
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                if !place.is_local() {
                    return Err(CodegenError::Internal(
                        "place projections not yet supported".to_string(),
                    ));
                }

                let dest = place.local;
                if let Some(val) = self.lower_rvalue(rvalue, dest)? {
                    self.def_var(dest, val);
                }
                // If rvalue returns None (ZST), no assignment needed

                Ok(())
            }

            StatementKind::StorageLive(_local) => {
                // No-op for now - could be used for stack slot optimization
                Ok(())
            }

            StatementKind::StorageDead(_local) => {
                // No-op for now - could be used for stack slot optimization
                Ok(())
            }

            StatementKind::Nop => {
                // No-op
                Ok(())
            }
        }
    }
}
