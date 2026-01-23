//! MIR optimization passes.
//!
//! This module provides an optimization pass interface for MIR transformations.
//! It follows the same context-based orchestration pattern as `ValidationContext`.
//!
//! # Architecture
//!
//! - `OptimizationPass` trait for individual passes
//! - `OptimizationContext` for pipeline orchestration
//! - `PassResult { changed: bool }` for fixpoint iteration
//!
//! # Example
//!
//! ```ignore
//! let mut ctx = OptimizationContext::new(&types);
//! ctx.add_pass(SimplifyCfg);
//! ctx.add_pass(ConstantFolding);
//! ctx.run_to_fixpoint(&mut body, 10);
//! ```

mod constant;
mod simplify;

use crate::sema::types::TypeInterner;

use super::Body;

pub use constant::ConstantFolding;
pub use simplify::SimplifyCfg;

/// Result of running an optimization pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassResult {
    /// Whether the pass made any changes to the MIR.
    pub changed: bool,
}

impl PassResult {
    /// Create a result indicating changes were made.
    pub fn changed() -> Self {
        PassResult { changed: true }
    }

    /// Create a result indicating no changes were made.
    pub fn unchanged() -> Self {
        PassResult { changed: false }
    }
}

/// An optimization pass that transforms MIR.
pub trait OptimizationPass {
    /// Returns the name of this pass (for debugging/logging).
    fn name(&self) -> &'static str;

    /// Run the pass on a MIR body.
    ///
    /// Returns whether any changes were made.
    fn run(&self, body: &mut Body, types: &TypeInterner) -> PassResult;
}

/// Context for orchestrating optimization passes.
pub struct OptimizationContext<'a> {
    types: &'a TypeInterner,
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl<'a> OptimizationContext<'a> {
    /// Create a new optimization context.
    pub fn new(types: &'a TypeInterner) -> Self {
        OptimizationContext {
            types,
            passes: Vec::new(),
        }
    }

    /// Add an optimization pass to the pipeline.
    pub fn add_pass(&mut self, pass: impl OptimizationPass + 'static) {
        self.passes.push(Box::new(pass));
    }

    /// Run all passes once and return whether any changes were made.
    pub fn run_once(&self, body: &mut Body) -> bool {
        let mut changed = false;
        for pass in &self.passes {
            let result = pass.run(body, self.types);
            changed |= result.changed;
        }
        changed
    }

    /// Run passes to fixpoint or until max iterations reached.
    ///
    /// Returns the number of iterations performed.
    pub fn run_to_fixpoint(&self, body: &mut Body, max_iterations: usize) -> usize {
        for i in 0..max_iterations {
            if !self.run_once(body) {
                return i + 1;
            }
        }
        max_iterations
    }
}

/// Optimize a MIR body with the default optimization pipeline.
pub fn optimize_mir(body: &mut Body, types: &TypeInterner) {
    let mut ctx = OptimizationContext::new(types);
    ctx.add_pass(SimplifyCfg);
    ctx.add_pass(ConstantFolding);
    ctx.run_to_fixpoint(body, 10);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::statement::{Statement, StatementKind};
    use crate::mir::terminator::{BasicBlock, Terminator};
    use crate::mir::validate::test_helpers::MirTestBuilder;

    // =========================================================================
    // Phase 1: Core Infrastructure Tests
    // =========================================================================

    #[test]
    fn pass_result_changed_true() {
        let result = PassResult::changed();
        assert!(result.changed);
    }

    #[test]
    fn pass_result_changed_false() {
        let result = PassResult::unchanged();
        assert!(!result.changed);
    }

    /// A no-op pass that never changes anything (test helper).
    struct NoopPass;

    impl OptimizationPass for NoopPass {
        fn name(&self) -> &'static str {
            "Noop"
        }

        fn run(&self, _body: &mut Body, _types: &TypeInterner) -> PassResult {
            PassResult::unchanged()
        }
    }

    #[test]
    fn noop_pass_returns_unchanged() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        let pass = NoopPass;
        let result = pass.run(&mut body, &types);
        assert!(!result.changed);
    }

    #[test]
    fn optimization_context_new() {
        let types = TypeInterner::new();
        let ctx = OptimizationContext::new(&types);
        assert!(ctx.passes.is_empty());
    }

    #[test]
    fn context_run_once_no_changes() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        let mut ctx = OptimizationContext::new(&types);
        ctx.add_pass(NoopPass);

        let changed = ctx.run_once(&mut body);
        assert!(!changed);
    }

    // =========================================================================
    // Phase 2: Mutation Tracking Tests
    // =========================================================================

    /// A pass that removes Nop statements (test helper).
    struct RemoveNopPass;

    impl OptimizationPass for RemoveNopPass {
        fn name(&self) -> &'static str {
            "RemoveNop"
        }

        fn run(&self, body: &mut Body, _types: &TypeInterner) -> PassResult {
            let mut changed = false;
            for block in &mut body.basic_blocks {
                let original_len = block.statements.len();
                block
                    .statements
                    .retain(|stmt| !matches!(stmt.kind, StatementKind::Nop));
                if block.statements.len() != original_len {
                    changed = true;
                }
            }
            PassResult { changed }
        }
    }

    #[test]
    fn remove_nop_pass_finds_nop() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.add_statement(bb, Statement::nop(0..0));
        builder.add_statement(bb, Statement::nop(0..0));
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        assert_eq!(body.block(bb).statements.len(), 2);

        let pass = RemoveNopPass;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);
        assert_eq!(body.block(bb).statements.len(), 0);
    }

    #[test]
    fn remove_nop_pass_no_nop() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        let pass = RemoveNopPass;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
    }

    #[test]
    fn context_run_once_with_changes() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.add_statement(bb, Statement::nop(0..0));
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        let mut ctx = OptimizationContext::new(&types);
        ctx.add_pass(RemoveNopPass);

        let changed = ctx.run_once(&mut body);
        assert!(changed);
        assert_eq!(body.block(bb).statements.len(), 0);
    }

    // =========================================================================
    // Phase 3: Fixpoint Iteration Tests
    // =========================================================================

    #[test]
    fn fixpoint_no_changes_returns_one() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        let mut ctx = OptimizationContext::new(&types);
        ctx.add_pass(NoopPass);

        let iterations = ctx.run_to_fixpoint(&mut body, 10);
        assert_eq!(iterations, 1);
    }

    /// A pass that adds a Nop if none exist, then removes one (for testing fixpoint).
    struct CountingPass {
        max_nops: usize,
    }

    impl OptimizationPass for CountingPass {
        fn name(&self) -> &'static str {
            "Counting"
        }

        fn run(&self, body: &mut Body, _types: &TypeInterner) -> PassResult {
            // Count current nops
            let nop_count: usize = body
                .basic_blocks
                .iter()
                .map(|b| {
                    b.statements
                        .iter()
                        .filter(|s| matches!(s.kind, StatementKind::Nop))
                        .count()
                })
                .sum();

            if nop_count < self.max_nops {
                // Add a nop to first block
                if !body.basic_blocks.is_empty() {
                    body.basic_blocks[0].statements.push(Statement::nop(0..0));
                    return PassResult::changed();
                }
            }
            PassResult::unchanged()
        }
    }

    #[test]
    fn fixpoint_converges_after_multiple() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        let mut ctx = OptimizationContext::new(&types);
        ctx.add_pass(CountingPass { max_nops: 3 });

        // Should add 3 nops then stop
        let iterations = ctx.run_to_fixpoint(&mut body, 10);
        assert_eq!(iterations, 4); // 3 iterations adding nops + 1 final check
        assert_eq!(body.block(BasicBlock(0)).statements.len(), 3);
    }

    #[test]
    fn fixpoint_respects_max_iterations() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        // A pass that always changes
        struct AlwaysChangesPass;
        impl OptimizationPass for AlwaysChangesPass {
            fn name(&self) -> &'static str {
                "AlwaysChanges"
            }
            fn run(&self, _body: &mut Body, _types: &TypeInterner) -> PassResult {
                PassResult::changed()
            }
        }

        let mut ctx = OptimizationContext::new(&types);
        ctx.add_pass(AlwaysChangesPass);

        let iterations = ctx.run_to_fixpoint(&mut body, 5);
        assert_eq!(iterations, 5);
    }

    // =========================================================================
    // Phase 6: Pipeline Composition Tests
    // =========================================================================

    #[test]
    fn pipeline_multiple_passes() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.add_statement(bb, Statement::nop(0..0));
        builder.set_terminator(bb, Terminator::return_(0..0));
        let (mut body, types) = builder.build();

        let mut ctx = OptimizationContext::new(&types);
        ctx.add_pass(NoopPass);
        ctx.add_pass(RemoveNopPass);

        let changed = ctx.run_once(&mut body);
        assert!(changed);
        assert_eq!(body.block(bb).statements.len(), 0);
    }

    #[test]
    fn context_add_pass() {
        let types = TypeInterner::new();
        let mut ctx = OptimizationContext::new(&types);

        ctx.add_pass(NoopPass);
        assert_eq!(ctx.passes.len(), 1);

        ctx.add_pass(RemoveNopPass);
        assert_eq!(ctx.passes.len(), 2);
    }

    #[test]
    fn pass_names() {
        assert_eq!(NoopPass.name(), "Noop");
        assert_eq!(RemoveNopPass.name(), "RemoveNop");
    }

    // =========================================================================
    // Phase 7: Integration with Validation Tests
    // =========================================================================

    #[test]
    fn optimized_mir_validates() {
        use crate::mir::operand::{Operand, Rvalue};
        use crate::mir::statement::Statement;
        use crate::mir::types::{Local, Place};
        use crate::mir::validate::validate_mir;

        // Build MIR with foldable constant and trivial goto
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        // bb0: _0 = Add(const 2, const 3); goto bb1
        builder.add_statement(
            bb0,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    crate::mir::operand::BinOp::Add,
                    Operand::const_int(2),
                    Operand::const_int(3),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));

        // bb1: goto bb2
        builder.set_terminator(bb1, Terminator::goto(bb2, 0..0));

        // bb2: return
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        // Validate before optimization
        validate_mir(&body, &types);

        // Optimize
        optimize_mir(&mut body, &types);

        // Validate after optimization - MIR should still be valid
        validate_mir(&body, &types);

        // Verify that constant folding happened
        match &body.block(bb0).statements[0].kind {
            crate::mir::statement::StatementKind::Assign(
                _,
                Rvalue::Use(Operand::Constant(crate::mir::operand::Constant::Int(5))),
            ) => {}
            other => panic!("expected folded constant 5, got {:?}", other),
        }
    }

    #[test]
    fn optimize_mir_default_pipeline() {
        use crate::mir::operand::{Operand, Rvalue};
        use crate::mir::statement::Statement;
        use crate::mir::types::{Local, Place};

        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        // bb0: _0 = Mul(const 6, const 7); goto bb1
        builder.add_statement(
            bb0,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    crate::mir::operand::BinOp::Mul,
                    Operand::const_int(6),
                    Operand::const_int(7),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));

        // bb1: return
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        // Run default optimization pipeline
        optimize_mir(&mut body, &types);

        // Verify constant was folded to 42
        match &body.block(bb0).statements[0].kind {
            crate::mir::statement::StatementKind::Assign(
                _,
                Rvalue::Use(Operand::Constant(crate::mir::operand::Constant::Int(42))),
            ) => {}
            other => panic!("expected folded constant 42, got {:?}", other),
        }
    }

    // =========================================================================
    // Additional Integration Tests
    // =========================================================================

    #[test]
    fn optimize_mir_complex_cfg_validates() {
        use crate::mir::operand::{BinOp, Operand, Rvalue};
        use crate::mir::statement::Statement;
        use crate::mir::terminator::{SwitchTargets, TerminatorKind};
        use crate::mir::types::{Local, Place};
        use crate::mir::validate::validate_mir;

        // Build a complex CFG with branches and constants
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(i32_ty);

        let cond = builder.add_local(bool_ty, false);
        let temp = builder.add_local(i32_ty, true);

        let bb_entry = builder.add_block();
        let bb_then = builder.add_block();
        let bb_else = builder.add_block();
        let bb_join = builder.add_block();
        let bb_exit = builder.add_block();

        // entry: _1 = 1 + 1; switch on cond
        builder.add_statement(
            bb_entry,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(BinOp::Add, Operand::const_int(1), Operand::const_int(1)),
                0..0,
            ),
        );
        builder.set_terminator(
            bb_entry,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(cond),
                    targets: SwitchTargets::new_bool(bb_then, bb_else),
                },
                0..0,
            ),
        );

        // then: _0 = 2 * 3; goto join
        builder.add_statement(
            bb_then,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(BinOp::Mul, Operand::const_int(2), Operand::const_int(3)),
                0..0,
            ),
        );
        builder.set_terminator(bb_then, Terminator::goto(bb_join, 0..0));

        // else: _0 = 4 + 5; goto join
        builder.add_statement(
            bb_else,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(BinOp::Add, Operand::const_int(4), Operand::const_int(5)),
                0..0,
            ),
        );
        builder.set_terminator(bb_else, Terminator::goto(bb_join, 0..0));

        // join: goto exit (trivial)
        builder.set_terminator(bb_join, Terminator::goto(bb_exit, 0..0));

        // exit: return
        builder.set_terminator(bb_exit, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        // Validate before
        validate_mir(&body, &types);

        // Optimize
        optimize_mir(&mut body, &types);

        // Validate after - critical check
        validate_mir(&body, &types);

        // Check constants were folded
        match &body.block(bb_entry).statements[0].kind {
            crate::mir::statement::StatementKind::Assign(
                _,
                Rvalue::Use(Operand::Constant(crate::mir::operand::Constant::Int(2))),
            ) => {}
            other => panic!("expected 2, got {:?}", other),
        }

        match &body.block(bb_then).statements[0].kind {
            crate::mir::statement::StatementKind::Assign(
                _,
                Rvalue::Use(Operand::Constant(crate::mir::operand::Constant::Int(6))),
            ) => {}
            other => panic!("expected 6, got {:?}", other),
        }

        match &body.block(bb_else).statements[0].kind {
            crate::mir::statement::StatementKind::Assign(
                _,
                Rvalue::Use(Operand::Constant(crate::mir::operand::Constant::Int(9))),
            ) => {}
            other => panic!("expected 9, got {:?}", other),
        }
    }

    #[test]
    fn optimize_mir_loop_cfg_validates() {
        use crate::mir::operand::{BinOp, Operand, Rvalue};
        use crate::mir::statement::Statement;
        use crate::mir::terminator::{SwitchTargets, TerminatorKind};
        use crate::mir::types::{Local, Place};
        use crate::mir::validate::validate_mir;

        // Build a loop CFG
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(i32_ty);

        let cond = builder.add_local(bool_ty, false);
        let counter = builder.add_local(i32_ty, true);

        let bb_entry = builder.add_block();
        let bb_header = builder.add_block();
        let bb_body = builder.add_block();
        let bb_exit = builder.add_block();

        // entry: _1 = 0 + 0; goto header
        builder.add_statement(
            bb_entry,
            Statement::assign(
                Place::from_local(counter),
                Rvalue::BinaryOp(BinOp::Add, Operand::const_int(0), Operand::const_int(0)),
                0..0,
            ),
        );
        builder.set_terminator(bb_entry, Terminator::goto(bb_header, 0..0));

        // header: switch cond
        builder.set_terminator(
            bb_header,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(cond),
                    targets: SwitchTargets::new_bool(bb_body, bb_exit),
                },
                0..0,
            ),
        );

        // body: _1 = _1 + 1 (can't fold); goto header
        builder.add_statement(
            bb_body,
            Statement::assign(
                Place::from_local(counter),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::copy_local(counter),
                    Operand::const_int(1),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb_body, Terminator::goto(bb_header, 0..0));

        // exit: _0 = _1; return
        builder.add_statement(
            bb_exit,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::copy_local(counter)),
                0..0,
            ),
        );
        builder.set_terminator(bb_exit, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        // Validate before
        validate_mir(&body, &types);

        // Optimize
        optimize_mir(&mut body, &types);

        // Validate after - should still be valid loop
        validate_mir(&body, &types);

        // Check the foldable constant was folded
        match &body.block(bb_entry).statements[0].kind {
            crate::mir::statement::StatementKind::Assign(
                _,
                Rvalue::Use(Operand::Constant(crate::mir::operand::Constant::Int(0))),
            ) => {}
            other => panic!("expected 0, got {:?}", other),
        }

        // Non-foldable should remain unchanged
        match &body.block(bb_body).statements[0].kind {
            crate::mir::statement::StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _)) => {
            }
            other => panic!("expected unfoldable add, got {:?}", other),
        }
    }

    #[test]
    fn optimize_mir_empty_function() {
        use crate::mir::validate::validate_mir;

        // fn foo() {}
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        validate_mir(&body, &types);
        optimize_mir(&mut body, &types);
        validate_mir(&body, &types);
    }

    #[test]
    fn optimize_mir_unreachable_block() {
        use crate::mir::validate::validate_mir;

        let mut builder = MirTestBuilder::new();
        let bb0 = builder.add_block();
        let bb1 = builder.add_block(); // unreachable

        builder.set_terminator(bb0, Terminator::return_(0..0));
        builder.set_terminator(bb1, Terminator::unreachable(0..0));

        let (mut body, types) = builder.build();

        validate_mir(&body, &types);
        optimize_mir(&mut body, &types);
        validate_mir(&body, &types);
    }

    #[test]
    fn optimize_mir_no_passes_empty_context() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        // Empty context should do nothing
        let ctx = OptimizationContext::new(&types);
        let changed = ctx.run_once(&mut body);
        assert!(!changed);
    }

    #[test]
    fn optimize_mir_preserves_terminators() {
        use crate::mir::terminator::TerminatorKind;
        use crate::mir::validate::validate_mir;

        // Ensure various terminator types are preserved
        let mut builder = MirTestBuilder::new();
        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        validate_mir(&body, &types);

        // After optimization bb0 should have return (simplified from goto->return)
        optimize_mir(&mut body, &types);

        validate_mir(&body, &types);

        // bb0 should now return directly
        assert!(matches!(
            body.block(bb0).terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }
}
