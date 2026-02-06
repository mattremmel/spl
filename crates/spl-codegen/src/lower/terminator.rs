//! Terminator lowering from MIR to Cranelift IR.

use cranelift_codegen::ir::{InstBuilder, MemFlags, TrapCode, Value};
use cranelift_module::Module;
use tracing::{debug, trace};

use crate::error::{CodegenError, TRAP_ASSERT_FAILED, TRAP_RESUME, TRAP_UNREACHABLE};
use spl_mir::operand::{Constant, Operand};
use spl_mir::terminator::{Terminator, TerminatorKind};
use spl_mir::types::Local;

use super::FunctionLowerer;

impl<'a> FunctionLowerer<'a> {
    /// Lower a MIR terminator to Cranelift IR.
    pub(super) fn lower_terminator(&mut self, term: &Terminator) -> Result<(), CodegenError> {
        let term_kind = match &term.kind {
            TerminatorKind::Return => "Return",
            TerminatorKind::Goto(_) => "Goto",
            TerminatorKind::SwitchInt { .. } => "SwitchInt",
            TerminatorKind::Call { .. } => "Call",
            TerminatorKind::Drop { .. } => "Drop",
            TerminatorKind::Assert { .. } => "Assert",
            TerminatorKind::Unreachable => "Unreachable",
            TerminatorKind::Resume => "Resume",
        };
        trace!(kind = term_kind, "lowering terminator");
        match &term.kind {
            TerminatorKind::Return => {
                // Get return value if not ZST
                if let Some(val) = self.use_var(Local::RETURN_PLACE) {
                    trace!(has_value = true, "emitting return");
                    self.builder.ins().return_(&[val]);
                } else {
                    // Unit return
                    trace!(has_value = false, "emitting return");
                    self.builder.ins().return_(&[]);
                }
                Ok(())
            }

            TerminatorKind::Goto(target) => {
                trace!(target_block = target.index(), "emitting goto");
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
                trace!(
                    target_count = target_pairs.len(),
                    otherwise_block = targets.otherwise().index(),
                    "emitting switch"
                );

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

                debug!(callee = ?def_id, arg_count = args.len(), dest = ?destination.local, "emitting function call");

                // Look up the function in the registry
                let registry = self.func_registry.ok_or_else(|| {
                    CodegenError::Internal(
                        "function registry required for call instructions".to_string(),
                    )
                })?;

                let func_info = registry.get(def_id).ok_or_else(|| {
                    CodegenError::Internal(format!("function {def_id:?} not found in registry"))
                })?;

                // Get the module to import the function reference
                let module = self.module.as_mut().ok_or_else(|| {
                    CodegenError::Internal("module required for call instructions".to_string())
                })?;

                // Import the function into the current function being built
                let func_ref = module.declare_func_in_func(func_info.func_id, self.builder.func);

                // Lower arguments, coercing to the expected parameter types
                // Fat pointer arguments are passed as multiple values
                let mut arg_values = Vec::with_capacity(args.len() * 2);
                let mut param_idx = 0;
                for arg in args {
                    // Check if this argument is a fat pointer (string constant or fat-pointer typed place)
                    // Note: We must check the RESULT type after projections, not just the base local
                    let arg_ty = match arg {
                        Operand::Constant(Constant::String(_)) => {
                            // String constants are always StrRef type
                            Some(self.types.str_ref())
                        }
                        Operand::Copy(place) | Operand::Move(place) => {
                            // Get the result type after applying all projections
                            Some(self.compute_place_result_type(place))
                        }
                        Operand::Constant(_) => None,
                    };

                    let is_fat_pointer =
                        arg_ty.is_some_and(|ty| self.type_mapper.is_fat_pointer(ty, self.types));

                    if is_fat_pointer {
                        // Fat pointer is passed as two values (ptr, len)
                        let (ptr, len) = self.lower_fat_pointer_operand(arg)?;
                        arg_values.push(ptr);
                        arg_values.push(len);
                        param_idx += 2; // Fat pointer takes 2 parameter slots
                    } else {
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
                }

                // Emit the call instruction
                let call_inst = self.builder.ins().call(func_ref, &arg_values);

                // Store the return value in the destination
                // Copy results to avoid borrow conflict with later mutable operations
                let results: Vec<_> = self.builder.inst_results(call_inst).to_vec();
                let dest_ty = self.body.local_decl(destination.local).ty;

                if self.type_mapper.is_fat_pointer(dest_ty, self.types) {
                    // Fat pointer return: store both values to stack slot
                    let addr = self.local_stack_addr(destination.local).ok_or_else(|| {
                        CodegenError::Internal(
                            "fat pointer return requires stack slot destination".to_string(),
                        )
                    })?;

                    // Get field offsets from layout
                    let field_0_offset = self.layout.field_offset(dest_ty, 0) as i32;
                    let field_1_offset = self.layout.field_offset(dest_ty, 1) as i32;

                    let flags = MemFlags::trusted();
                    self.builder
                        .ins()
                        .store(flags, results[0], addr, field_0_offset);
                    self.builder
                        .ins()
                        .store(flags, results[1], addr, field_1_offset);
                } else if !results.is_empty() {
                    // Scalar return: define the variable
                    self.def_var(destination.local, results[0]);
                }

                // Jump to the target block (if Some, i.e., non-diverging call)
                if let Some(target_bb) = target {
                    let target_block = self.get_block(*target_bb);
                    self.builder.ins().jump(target_block, &[]);
                }

                Ok(())
            }

            TerminatorKind::Drop { target, .. } => {
                trace!(target_block = target.index(), "emitting drop (no-op)");
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
                trace!(expected = expected, target_block = target.index(), "emitting assert");
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
                trace!("emitting unreachable trap");
                self.builder
                    .ins()
                    .trap(TrapCode::user(TRAP_UNREACHABLE).unwrap());
                Ok(())
            }

            TerminatorKind::Resume => {
                trace!("emitting resume trap");
                // Resume unwinding - for now just trap
                self.builder
                    .ins()
                    .trap(TrapCode::user(TRAP_RESUME).unwrap());
                Ok(())
            }
        }
    }

    /// Lower a fat pointer operand to its component values.
    ///
    /// Fat pointers (`StrRef`, slices) must be passed as separate values
    /// at the ABI level. Currently supports 2-field fat pointers.
    fn lower_fat_pointer_operand(
        &mut self,
        operand: &Operand,
    ) -> Result<(Value, Value), CodegenError> {
        let ptr_ty = self.type_mapper.pointer_type();

        match operand {
            Operand::Constant(Constant::String(s)) => {
                // Create the string data and get its address
                let gv = self.declare_string_data(s)?;
                let str_ptr = self.builder.ins().global_value(ptr_ty, gv);
                let str_len = self.builder.ins().iconst(ptr_ty, s.len() as i64);
                Ok((str_ptr, str_len))
            }
            Operand::Copy(place) | Operand::Move(place) => {
                // Get the type after projections to use for layout
                let place_ty = self.compute_place_result_type(place);

                // Load both fields from the fat pointer in memory
                let (addr, _) = self.compute_place_address(place)?;

                // Get field offsets from layout
                let field_0_offset = self.layout.field_offset(place_ty, 0) as i32;
                let field_1_offset = self.layout.field_offset(place_ty, 1) as i32;

                // Load field 0 (ptr) and field 1 (len)
                let flags = MemFlags::trusted();
                let field_0 = self.builder.ins().load(ptr_ty, flags, addr, field_0_offset);
                let field_1 = self.builder.ins().load(ptr_ty, flags, addr, field_1_offset);

                Ok((field_0, field_1))
            }
            Operand::Constant(_) => Err(CodegenError::Internal(
                "unexpected operand for fat pointer".to_string(),
            )),
        }
    }

    /// Compute the result type of a place after applying all projections.
    fn compute_place_result_type(&self, place: &spl_mir::types::Place) -> spl_sema::types::TypeId {
        use spl_mir::types::PlaceElem;

        let mut current_ty = self.body.local_decl(place.local).ty;

        for proj in &place.projection {
            current_ty = match proj {
                PlaceElem::Field(idx) => {
                    // Get the field type
                    self.layout
                        .field_type(current_ty, idx.index() as usize)
                        .unwrap_or(current_ty)
                }
                PlaceElem::Deref => {
                    // Get the pointee type
                    self.layout.pointee_type(current_ty).unwrap_or(current_ty)
                }
                PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                    // Get the element type
                    self.layout.element_type(current_ty).unwrap_or(current_ty)
                }
                // Subslice preserves slice type, Downcast preserves enum variant type
                PlaceElem::Subslice { .. } | PlaceElem::Downcast(_) => current_ty,
            };
        }

        current_ty
    }
}
