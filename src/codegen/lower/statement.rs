//! Statement lowering from MIR to Cranelift IR.

use cranelift_codegen::ir::{InstBuilder, MemFlags};

use crate::codegen::LocalStorage;
use crate::codegen::error::CodegenError;
use crate::mir::statement::{Statement, StatementKind};

use super::FunctionLowerer;

impl<'a> FunctionLowerer<'a> {
    /// Lower a MIR statement to Cranelift IR.
    pub(super) fn lower_statement(&mut self, stmt: &Statement) -> Result<(), CodegenError> {
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                if place.is_local() {
                    // Simple assignment to a local
                    let dest = place.local;

                    // Check if destination is a stack slot or variable
                    match self.local_storage(dest) {
                        Some(LocalStorage::Variable(_)) => {
                            // SSA variable - use def_var
                            if let Some(val) = self.lower_rvalue(rvalue, dest)? {
                                self.def_var(dest, val);
                            }
                        }
                        Some(LocalStorage::StackSlot(_)) => {
                            // Stack slot - store to memory
                            if let Some(val) = self.lower_rvalue(rvalue, dest)? {
                                let addr = self.local_stack_addr(dest).ok_or_else(|| {
                                    CodegenError::Internal("expected stack slot".to_string())
                                })?;
                                self.store_to_addr(addr, val);
                            }
                            // If rvalue returns None, it's either a ZST or an aggregate
                            // Aggregates are handled specially in lower_rvalue
                        }
                        Some(LocalStorage::Zst) => {
                            // ZST - evaluate rvalue for side effects but don't store
                            let _ = self.lower_rvalue(rvalue, dest)?;
                        }
                        None => {
                            return Err(CodegenError::Internal(format!(
                                "local {:?} not found in local_map",
                                dest
                            )));
                        }
                    }
                } else {
                    // Assignment through projections (e.g., tuple.0 = value)
                    let (addr, result_ty) = self.compute_place_address(place)?;

                    // Get the cranelift type for the destination
                    if let Some(clif_ty) = self.type_mapper.map_type(result_ty, self.types) {
                        // Lower the rvalue - we need to get a value that matches the type
                        if let Some(val) = self.lower_rvalue_for_type(rvalue, clif_ty)? {
                            let flags = MemFlags::trusted();
                            self.builder.ins().store(flags, val, addr, 0);
                        }
                    } else if self.type_mapper.is_zst(result_ty, self.types) {
                        // ZST destination - no store needed
                    } else {
                        // Compound type destination - need to handle aggregates
                        // For now, error out - aggregates should be handled by lower_rvalue
                        return Err(CodegenError::Internal(
                            "compound type assignment through projection not yet fully supported"
                                .to_string(),
                        ));
                    }
                }

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
