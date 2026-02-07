//! `SimplifyCfg` optimization pass.
//!
//! Simplifies the control flow graph by removing trivial goto chains.
//! For example: `bb0: goto -> bb1; bb1: return` becomes `bb0: return`.

use std::collections::HashMap;

use tracing::trace;

use crate::mir::Body;
use crate::mir::terminator::{BasicBlock, Terminator, TerminatorKind};
use crate::sema::types::TypeInterner;

use super::{OptimizationPass, PassResult};

/// `SimplifyCfg` optimization pass.
///
/// Removes trivial goto chains where a block only contains a goto to another block
/// that has a single predecessor.
pub struct SimplifyCfg;

impl OptimizationPass for SimplifyCfg {
    fn name(&self) -> &'static str {
        "SimplifyCfg"
    }

    fn run(&self, body: &mut Body, _types: &TypeInterner) -> PassResult {
        let mut changed = false;
        let mut redirect_count = 0u32;

        // Build predecessor counts
        let pred_counts = compute_predecessor_counts(body);

        // Find trivial gotos that can be redirected
        for bb_idx in 0..body.basic_blocks.len() {
            let bb = BasicBlock(bb_idx as u32);
            let block = &body.basic_blocks[bb_idx];

            // Skip if block has statements (not trivial)
            if !block.statements.is_empty() {
                continue;
            }

            // Check if terminator is a goto
            if let Some(Terminator {
                kind: TerminatorKind::Goto(target),
                span,
            }) = &block.terminator
            {
                let target = *target;
                let span = span.clone();

                // Skip if target has multiple predecessors
                let target_pred_count = pred_counts.get(&target).copied().unwrap_or(0);
                if target_pred_count != 1 {
                    continue;
                }

                // Skip self-loops
                if target == bb {
                    continue;
                }

                // Copy target's terminator to this block
                let target_term = body.basic_blocks[target.index() as usize]
                    .terminator
                    .clone();

                if let Some(target_term) = target_term {
                    trace!(
                        from = bb_idx,
                        to = target.index(),
                        "redirecting trivial goto"
                    );
                    // Replace this block's terminator with target's terminator
                    // but keep the original span for debugging
                    body.basic_blocks[bb_idx].terminator = Some(Terminator {
                        kind: target_term.kind,
                        span,
                    });
                    changed = true;
                    redirect_count += 1;
                }
            }
        }

        if redirect_count > 0 {
            trace!(redirect_count, "SimplifyCfg summary");
        }

        PassResult { changed }
    }
}

/// Compute the number of predecessors for each basic block.
fn compute_predecessor_counts(body: &Body) -> HashMap<BasicBlock, usize> {
    let mut counts = HashMap::new();

    for block in &body.basic_blocks {
        for successor in block.successors() {
            *counts.entry(successor).or_insert(0) += 1;
        }
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::operand::{Operand, Rvalue};
    use crate::mir::statement::Statement;
    use crate::mir::terminator::{SwitchTargets, Terminator};
    use crate::mir::types::{Local, Place};
    use crate::mir::validate::test_helpers::MirTestBuilder;

    // =========================================================================
    // Phase 4: SimplifyCfg Pass Tests
    // =========================================================================

    #[test]
    fn simplify_cfg_removes_trivial_goto() {
        // bb0: goto -> bb1; bb1: return  =>  bb0: return
        let mut builder = MirTestBuilder::new();
        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);
        // bb0 should now have a return terminator
        assert!(matches!(
            body.block(bb0).terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }

    #[test]
    fn simplify_cfg_preserves_block_with_statements() {
        // bb0: _0 = 42; goto -> bb1; bb1: return
        // Should NOT simplify because bb0 has statements
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        let const_42 = builder.const_i32(42);
        builder.add_statement(
            bb0,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(const_42),
                0..0,
            ),
        );
        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
        // bb0 should still goto bb1
        assert!(matches!(
            body.block(bb0).terminator.as_ref().unwrap().kind,
            TerminatorKind::Goto(_)
        ));
    }

    #[test]
    fn simplify_cfg_preserves_multiple_predecessor_target() {
        // bb0: goto -> bb2
        // bb1: goto -> bb2
        // bb2: return
        // Should NOT simplify because bb2 has multiple predecessors
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb2, 0..0));
        builder.set_terminator(bb1, Terminator::goto(bb2, 0..0));
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
    }

    #[test]
    fn simplify_cfg_chain() {
        // bb0: goto -> bb1; bb1: goto -> bb2; bb2: return
        // After one pass: bb0: goto -> bb2; bb1: return; bb2: return
        // After two passes: bb0: return (if bb2 has only one pred now)
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::goto(bb2, 0..0));
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;

        // First pass - simplifies bb1 -> bb2 chain
        let result1 = pass.run(&mut body, &types);
        assert!(result1.changed);

        // Second pass - may simplify bb0 -> bb1 chain
        let result2 = pass.run(&mut body, &types);
        // After simplification, bb0 should reach return
        assert!(result2.changed);

        // Third pass - should be stable
        let result3 = pass.run(&mut body, &types);
        assert!(!result3.changed);
    }

    #[test]
    fn simplify_cfg_preserves_switch() {
        // bb0: switch -> bb1 or bb2; bb1: return; bb2: return
        // Should NOT simplify switch terminators
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let _cond = builder.add_local(bool_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(Local(1)),
                    targets: SwitchTargets::new_bool(bb1, bb2),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
    }

    #[test]
    fn simplify_cfg_self_loop_preserved() {
        // bb0: goto -> bb0 (self-loop)
        // Should NOT be simplified
        let mut builder = MirTestBuilder::new();
        let bb0 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb0, 0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
    }

    #[test]
    fn simplify_cfg_empty_body() {
        let builder = MirTestBuilder::new();
        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
    }

    // =========================================================================
    // Additional Coverage: Edge Cases and Validation
    // =========================================================================

    #[test]
    fn simplify_cfg_validates_after_optimization() {
        use crate::mir::validate::validate_mir;

        // Build a chain: bb0 -> bb1 -> bb2 (return)
        let mut builder = MirTestBuilder::new();
        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::goto(bb2, 0..0));
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        // Validate before
        validate_mir(&body, &types);

        // Optimize
        let pass = SimplifyCfg;
        pass.run(&mut body, &types);
        pass.run(&mut body, &types);

        // Validate after - MIR should still be valid
        validate_mir(&body, &types);
    }

    #[test]
    fn simplify_cfg_with_call_terminator() {
        use crate::mir::operand::Constant;
        use crate::sema::symbol::DefId;

        // bb0: goto -> bb1; bb1: call fn -> bb2
        // Should simplify: bb0 gets bb1's call terminator
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(
            bb1,
            Terminator::new(
                TerminatorKind::Call {
                    func: Operand::Constant(Constant::FnDef(DefId::new(1))),
                    args: vec![],
                    destination: Place::from_local(Local::RETURN_PLACE),
                    target: Some(bb2),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);
        // bb0 should now have a Call terminator
        assert!(matches!(
            body.block(bb0).terminator.as_ref().unwrap().kind,
            TerminatorKind::Call { .. }
        ));
    }

    #[test]
    fn simplify_cfg_with_drop_terminator() {
        // bb0: goto -> bb1; bb1: drop _1 -> bb2
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let local = builder.add_local(i32_ty, true);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(
            bb1,
            Terminator::new(
                TerminatorKind::Drop {
                    place: Place::from_local(local),
                    target: bb2,
                },
                0..0,
            ),
        );
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);
        // bb0 should now have a Drop terminator
        assert!(matches!(
            body.block(bb0).terminator.as_ref().unwrap().kind,
            TerminatorKind::Drop { .. }
        ));
    }

    #[test]
    fn simplify_cfg_with_assert_terminator() {
        // bb0: goto -> bb1; bb1: assert cond -> bb2
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let cond = builder.add_local(bool_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(
            bb1,
            Terminator::new(
                TerminatorKind::Assert {
                    cond: Operand::copy_local(cond),
                    expected: true,
                    target: bb2,
                },
                0..0,
            ),
        );
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);
        // bb0 should now have an Assert terminator
        assert!(matches!(
            body.block(bb0).terminator.as_ref().unwrap().kind,
            TerminatorKind::Assert { .. }
        ));
    }

    #[test]
    fn simplify_cfg_with_unreachable_terminator() {
        // bb0: goto -> bb1; bb1: unreachable
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::unreachable(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);
        // bb0 should now have an Unreachable terminator
        assert!(matches!(
            body.block(bb0).terminator.as_ref().unwrap().kind,
            TerminatorKind::Unreachable
        ));
    }

    #[test]
    fn simplify_cfg_diamond_pattern() {
        // Diamond: bb0 -> switch -> bb1, bb2 -> bb3 (join) -> return
        // bb1 and bb2 each have bb3 as successor, so bb3 has 2 predecessors
        // No simplification should occur
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let cond = builder.add_local(bool_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();
        let bb3 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(cond),
                    targets: SwitchTargets::new_bool(bb1, bb2),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::goto(bb3, 0..0));
        builder.set_terminator(bb2, Terminator::goto(bb3, 0..0));
        builder.set_terminator(bb3, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        // No simplification - bb3 has multiple predecessors
        assert!(!result.changed);
    }

    #[test]
    fn simplify_cfg_loop_with_trivial_latch() {
        // bb0 (entry) -> bb1 (header)
        // bb1 -> switch -> bb2 (body), bb3 (exit)
        // bb2 -> bb4 (latch: trivial goto)
        // bb4 -> bb1 (back edge)
        // bb3 -> return
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let cond = builder.add_local(bool_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();
        let bb3 = builder.add_block();
        let bb4 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(
            bb1,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(cond),
                    targets: SwitchTargets::new_bool(bb2, bb3),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb2, Terminator::goto(bb4, 0..0));
        builder.set_terminator(bb3, Terminator::return_(0..0));
        builder.set_terminator(bb4, Terminator::goto(bb1, 0..0));

        let (mut body, types) = builder.build();

        // bb1 has 2 preds (bb0, bb4), so bb2->bb4 shouldn't be simplified
        // bb4 has 1 pred (bb2), and bb4->bb1 where bb1 has 2 preds
        // So bb2->bb4 can be simplified since bb4 has single pred
        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        // bb2 should get bb4's terminator (goto bb1)
        assert!(result.changed);

        // Validate after optimization
        use crate::mir::validate::validate_mir;
        validate_mir(&body, &types);
    }

    #[test]
    fn simplify_cfg_entry_block_optimization() {
        // Entry block (bb0) is trivial: goto -> bb1
        // This should be simplified
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block(); // Entry
        let bb1 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = SimplifyCfg;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);
        // Entry block should now return directly
        assert!(matches!(
            body.block(bb0).terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));

        // Validate
        use crate::mir::validate::validate_mir;
        validate_mir(&body, &types);
    }
}
