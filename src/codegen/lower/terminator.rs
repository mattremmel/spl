//! Terminator lowering from MIR to Cranelift IR.

use cranelift_codegen::ir::InstBuilder;

use crate::codegen::error::CodegenError;
use crate::mir::terminator::{Terminator, TerminatorKind};
use crate::mir::types::Local;

use super::FunctionLowerer;

impl<'a> FunctionLowerer<'a> {
    /// Lower a MIR terminator to Cranelift IR.
    pub(super) fn lower_terminator(&mut self, term: &Terminator) -> Result<(), CodegenError> {
        match &term.kind {
            TerminatorKind::Return => {
                // Get return value if not ZST
                if let Some(val) = self.use_var(Local::RETURN_PLACE) {
                    self.builder.ins().return_(&[val]);
                } else {
                    // Unit return
                    self.builder.ins().return_(&[]);
                }
                Ok(())
            }

            TerminatorKind::Goto(target) => {
                let target_block = self.get_block(*target);
                self.builder.ins().jump(target_block, &[]);
                Ok(())
            }

            TerminatorKind::SwitchInt { discr, targets } => {
                // Get discriminant value
                let discr_val = self
                    .lower_operand(discr)?
                    .ok_or_else(|| CodegenError::Internal("ZST discriminant".to_string()))?;

                // For boolean switch (single target + otherwise), use brif
                let target_pairs: Vec<_> = targets.iter().collect();

                if target_pairs.len() == 1 && target_pairs[0].0 == 0 {
                    // Boolean switch: 0 -> false_target, otherwise -> true_target
                    let false_block = self.get_block(target_pairs[0].1);
                    let true_block = self.get_block(targets.otherwise());

                    // brif takes a condition; non-zero = true
                    self.builder
                        .ins()
                        .brif(discr_val, true_block, &[], false_block, &[]);
                } else if target_pairs.is_empty() {
                    // No specific targets, just jump to otherwise
                    let otherwise_block = self.get_block(targets.otherwise());
                    self.builder.ins().jump(otherwise_block, &[]);
                } else {
                    // Multi-way switch - use a series of comparisons
                    // This is a simple implementation; could use jump tables for large switches
                    let otherwise_block = self.get_block(targets.otherwise());

                    // Get the discriminant type
                    let discr_ty = self.builder.func.dfg.value_type(discr_val);

                    for (value, target) in target_pairs {
                        let target_block = self.get_block(target);
                        let const_val = self.builder.ins().iconst(discr_ty, value as i64);
                        let cmp = self.builder.ins().icmp(
                            cranelift_codegen::ir::condcodes::IntCC::Equal,
                            discr_val,
                            const_val,
                        );

                        // Create a continuation block for the next comparison
                        let next_block = self.builder.create_block();

                        self.builder
                            .ins()
                            .brif(cmp, target_block, &[], next_block, &[]);

                        // Seal the next block (we're its only predecessor)
                        self.builder.seal_block(next_block);
                        self.builder.switch_to_block(next_block);
                    }

                    // Fall through to otherwise
                    self.builder.ins().jump(otherwise_block, &[]);
                }

                Ok(())
            }

            TerminatorKind::Call { .. } => Err(CodegenError::Internal(
                "function calls not yet supported".to_string(),
            )),

            TerminatorKind::Drop { target, .. } => {
                // For now, drops are no-ops (we don't have destructors yet)
                let target_block = self.get_block(*target);
                self.builder.ins().jump(target_block, &[]);
                Ok(())
            }

            TerminatorKind::Assert {
                cond: _,
                expected: _,
                target,
            } => {
                // For now, asserts are no-ops (we trust the MIR is valid)
                // In a full implementation, we'd trap on assertion failure
                let target_block = self.get_block(*target);
                self.builder.ins().jump(target_block, &[]);
                Ok(())
            }

            TerminatorKind::Unreachable => {
                self.builder
                    .ins()
                    .trap(cranelift_codegen::ir::TrapCode::user(0).unwrap());
                Ok(())
            }

            TerminatorKind::Resume => {
                // Resume unwinding - for now just trap
                self.builder
                    .ins()
                    .trap(cranelift_codegen::ir::TrapCode::user(1).unwrap());
                Ok(())
            }
        }
    }
}
