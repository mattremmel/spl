//! Terminator lowering from MIR to Cranelift IR.

use cranelift_codegen::ir::{InstBuilder, TrapCode};
use cranelift_module::Module;

use crate::codegen::error::{CodegenError, TRAP_ASSERT_FAILED, TRAP_RESUME, TRAP_UNREACHABLE};
use crate::mir::operand::{Constant, Operand};
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

            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
            } => {
                // Extract DefId from the function operand
                let def_id = match func {
                    Operand::Constant(Constant::FnDef(def_id)) => *def_id,
                    _ => {
                        return Err(CodegenError::Internal(
                            "call func must be a FnDef constant".to_string(),
                        ));
                    }
                };

                // Look up the function in the registry
                let registry = self.func_registry.ok_or_else(|| {
                    CodegenError::Internal(
                        "function registry required for call instructions".to_string(),
                    )
                })?;

                let func_info = registry.get(def_id).ok_or_else(|| {
                    CodegenError::Internal(format!("function {:?} not found in registry", def_id))
                })?;

                // Get the module to import the function reference
                let module = self.module.as_mut().ok_or_else(|| {
                    CodegenError::Internal("module required for call instructions".to_string())
                })?;

                // Import the function into the current function being built
                let func_ref = module.declare_func_in_func(func_info.func_id, self.builder.func);

                // Lower arguments, coercing to the expected parameter types
                let mut arg_values = Vec::with_capacity(args.len());
                let mut param_idx = 0;
                for arg in args {
                    // Get the expected parameter type from the callee's signature
                    let expected_ty = if param_idx < func_info.signature.params.len() {
                        Some(func_info.signature.params[param_idx].value_type)
                    } else {
                        None
                    };

                    if let Some(expected) = expected_ty {
                        if let Some(val) = self.lower_operand_as(arg, expected)? {
                            arg_values.push(val);
                            param_idx += 1;
                        }
                        // Skip ZST arguments (no param_idx increment for ZST)
                    } else {
                        // No expected type (ZST or error), just try to lower
                        if let Some(val) = self.lower_operand(arg)? {
                            arg_values.push(val);
                            param_idx += 1;
                        }
                    }
                }

                // Emit the call instruction
                let call_inst = self.builder.ins().call(func_ref, &arg_values);

                // Store the return value in the destination (if not ZST)
                let results = self.builder.inst_results(call_inst);
                if !results.is_empty() {
                    let return_val = results[0];
                    self.def_var(destination.local, return_val);
                }

                // Jump to the target block (if Some, i.e., non-diverging call)
                if let Some(target_bb) = target {
                    let target_block = self.get_block(*target_bb);
                    self.builder.ins().jump(target_block, &[]);
                }

                Ok(())
            }

            TerminatorKind::Drop { target, .. } => {
                // For now, drops are no-ops (we don't have destructors yet)
                let target_block = self.get_block(*target);
                self.builder.ins().jump(target_block, &[]);
                Ok(())
            }

            TerminatorKind::Assert {
                cond,
                expected,
                target,
            } => {
                let cond_val = self
                    .lower_operand(cond)?
                    .ok_or_else(|| CodegenError::Internal("ZST assert condition".to_string()))?;

                let fail_block = self.builder.create_block();
                let target_block = self.get_block(*target);

                // Branch based on expected value
                if *expected {
                    // expected=true: succeed if cond!=0, fail if cond==0
                    self.builder
                        .ins()
                        .brif(cond_val, target_block, &[], fail_block, &[]);
                } else {
                    // expected=false: succeed if cond==0, fail if cond!=0
                    self.builder
                        .ins()
                        .brif(cond_val, fail_block, &[], target_block, &[]);
                }

                // Fail block: trap with ASSERT_FAILED code
                self.builder.switch_to_block(fail_block);
                self.builder.seal_block(fail_block);
                self.builder
                    .ins()
                    .trap(TrapCode::user(TRAP_ASSERT_FAILED).unwrap());

                Ok(())
            }

            TerminatorKind::Unreachable => {
                self.builder
                    .ins()
                    .trap(TrapCode::user(TRAP_UNREACHABLE).unwrap());
                Ok(())
            }

            TerminatorKind::Resume => {
                // Resume unwinding - for now just trap
                self.builder
                    .ins()
                    .trap(TrapCode::user(TRAP_RESUME).unwrap());
                Ok(())
            }
        }
    }
}
