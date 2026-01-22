//! MIR validation.
//!
//! This module validates MIR invariants. Validation failures indicate compiler bugs,
//! not user errors, since MIR is generated from validated HIR.

mod cfg;
mod locals;
mod rvalues;
mod terminators;
mod types;

#[cfg(test)]
mod test_helpers;

use crate::sema::types::TypeInterner;

use super::Body;

pub use cfg::validate_cfg;
pub use locals::validate_locals;
pub use rvalues::validate_rvalues;
pub use terminators::validate_terminators;
pub use types::validate_types;

/// Context for MIR validation.
pub struct ValidationContext<'a> {
    body: &'a Body,
    types: &'a TypeInterner,
}

impl<'a> ValidationContext<'a> {
    /// Create a new validation context.
    pub fn new(body: &'a Body, types: &'a TypeInterner) -> Self {
        ValidationContext { body, types }
    }

    /// Run all validation passes.
    pub fn validate(&self) {
        validate_cfg(self.body);
        validate_locals(self.body);
        validate_types(self.body, self.types);
        validate_rvalues(self.body, self.types);
        validate_terminators(self.body, self.types);
    }
}

/// Validate a MIR body against all invariants.
///
/// # Panics
///
/// Panics with a descriptive message if any validation check fails.
/// Validation failures indicate compiler bugs.
pub fn validate_mir(body: &Body, types: &TypeInterner) {
    ValidationContext::new(body, types).validate();
}

#[cfg(test)]
mod integration_tests {
    use super::test_helpers::MirTestBuilder;
    use super::*;
    use crate::mir::operand::{Operand, Rvalue};
    use crate::mir::statement::Statement;
    use crate::mir::terminator::{SwitchTargets, Terminator, TerminatorKind};
    use crate::mir::types::{Local, Place};

    #[test]
    fn integration_valid_simple_function() {
        // fn foo() -> i32 { 42 }
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);

        let bb0 = builder.add_block();

        // _0 = 42
        builder.add_statement(
            bb0,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_int(42)),
                0..0,
            ),
        );
        builder.set_terminator(bb0, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_mir(&body, &types);
    }

    #[test]
    fn integration_valid_if_else() {
        // fn foo(cond: bool) -> i32 { if cond { 1 } else { 2 } }
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(i32_ty);

        let _cond_local = builder.add_local(bool_ty, false);

        let bb_entry = builder.add_block();
        let bb_then = builder.add_block();
        let bb_else = builder.add_block();
        let bb_join = builder.add_block();

        // entry: switch on cond
        builder.set_terminator(
            bb_entry,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(Local(1)),
                    targets: SwitchTargets::new_bool(bb_then, bb_else),
                },
                0..0,
            ),
        );

        // then: _0 = 1; goto join
        builder.add_statement(
            bb_then,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_int(1)),
                0..0,
            ),
        );
        builder.set_terminator(bb_then, Terminator::goto(bb_join, 0..0));

        // else: _0 = 2; goto join
        builder.add_statement(
            bb_else,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_int(2)),
                0..0,
            ),
        );
        builder.set_terminator(bb_else, Terminator::goto(bb_join, 0..0));

        // join: return
        builder.set_terminator(bb_join, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_mir(&body, &types);
    }

    #[test]
    fn integration_valid_loop() {
        // while (true) {}
        let mut builder = MirTestBuilder::new();

        let bb_entry = builder.add_block();
        let bb_header = builder.add_block();
        let bb_body = builder.add_block();
        let bb_exit = builder.add_block();

        // entry: goto header
        builder.set_terminator(bb_entry, Terminator::goto(bb_header, 0..0));

        // header: switch true -> body, false -> exit
        builder.set_terminator(
            bb_header,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::const_bool(true),
                    targets: SwitchTargets::new_bool(bb_body, bb_exit),
                },
                0..0,
            ),
        );

        // body: goto header (back edge)
        builder.set_terminator(bb_body, Terminator::goto(bb_header, 0..0));

        // exit: return
        builder.set_terminator(bb_exit, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_mir(&body, &types);
    }
}
