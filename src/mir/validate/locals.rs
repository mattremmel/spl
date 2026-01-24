//! Local reference validation for MIR.
//!
//! Validates that all `Local` references in the MIR body are in bounds.

use crate::mir::Body;
use crate::mir::operand::{Operand, Rvalue};
use crate::mir::statement::StatementKind;
use crate::mir::terminator::TerminatorKind;
use crate::mir::types::{Local, Place, PlaceElem};

/// Validate that all local references in a MIR body are in bounds.
///
/// # Panics
///
/// Panics if any `Local` reference is out of bounds.
pub fn validate_locals(body: &Body) {
    let num_locals = body.num_locals();

    for (block_idx, block) in body.basic_blocks.iter().enumerate() {
        // Validate statements
        for (stmt_idx, stmt) in block.statements.iter().enumerate() {
            let ctx = || format!("BasicBlock({}).statements[{}]", block_idx, stmt_idx);
            validate_statement_locals(&stmt.kind, num_locals, &ctx);
        }

        // Validate terminator
        if let Some(term) = &block.terminator {
            let ctx = || format!("BasicBlock({}).terminator", block_idx);
            validate_terminator_locals(&term.kind, num_locals, &ctx);
        }
    }
}

fn validate_local(local: Local, num_locals: usize, context: &dyn Fn() -> String) {
    if local.index() as usize >= num_locals {
        panic!(
            "Local validation failed: Local({}) out of bounds (only {} locals exist) at {}",
            local.index(),
            num_locals,
            context()
        );
    }
}

fn validate_place_locals(place: &Place, num_locals: usize, context: &dyn Fn() -> String) {
    validate_local(place.local, num_locals, context);

    for elem in &place.projection {
        if let PlaceElem::Index(local) = elem {
            validate_local(*local, num_locals, context);
        }
    }
}

fn validate_operand_locals(operand: &Operand, num_locals: usize, context: &dyn Fn() -> String) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            validate_place_locals(place, num_locals, context);
        }
        Operand::Constant(_) => {}
    }
}

fn validate_rvalue_locals(rvalue: &Rvalue, num_locals: usize, context: &dyn Fn() -> String) {
    match rvalue {
        Rvalue::Use(operand) => {
            validate_operand_locals(operand, num_locals, context);
        }
        Rvalue::Ref(_, place, _)
        | Rvalue::AddressOf(_, place, _)
        | Rvalue::Len(place)
        | Rvalue::Discriminant(place) => {
            validate_place_locals(place, num_locals, context);
        }
        Rvalue::BinaryOp(_, lhs, rhs) => {
            validate_operand_locals(lhs, num_locals, context);
            validate_operand_locals(rhs, num_locals, context);
        }
        Rvalue::UnaryOp(_, operand) | Rvalue::Cast(_, operand, _) | Rvalue::Repeat(operand, _) => {
            validate_operand_locals(operand, num_locals, context);
        }
        Rvalue::Aggregate(_, operands) => {
            for operand in operands {
                validate_operand_locals(operand, num_locals, context);
            }
        }
    }
}

fn validate_statement_locals(
    kind: &StatementKind,
    num_locals: usize,
    context: &dyn Fn() -> String,
) {
    match kind {
        StatementKind::Assign(place, rvalue) => {
            validate_place_locals(place, num_locals, context);
            validate_rvalue_locals(rvalue, num_locals, context);
        }
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            validate_local(*local, num_locals, context);
        }
        StatementKind::Nop => {}
    }
}

fn validate_terminator_locals(
    kind: &TerminatorKind,
    num_locals: usize,
    context: &dyn Fn() -> String,
) {
    match kind {
        TerminatorKind::Return
        | TerminatorKind::Unreachable
        | TerminatorKind::Resume
        | TerminatorKind::Goto(_) => {}
        TerminatorKind::SwitchInt { discr, .. } => {
            validate_operand_locals(discr, num_locals, context);
        }
        TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } => {
            validate_operand_locals(func, num_locals, context);
            for arg in args {
                validate_operand_locals(arg, num_locals, context);
            }
            validate_place_locals(destination, num_locals, context);
        }
        TerminatorKind::Drop { place, .. } => {
            validate_place_locals(place, num_locals, context);
        }
        TerminatorKind::Assert { cond, .. } => {
            validate_operand_locals(cond, num_locals, context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::MirTestBuilder;
    use super::*;
    use crate::mir::operand::{BorrowKind, Constant, Rvalue};
    use crate::mir::statement::Statement;
    use crate::mir::terminator::{SwitchTargets, Terminator, TerminatorKind};
    use crate::mir::types::FieldIdx;
    use crate::sema::symbol::DefId;
    use crate::sema::types::TypeId;

    // Dummy type ID for structural tests (not validation)
    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn local_valid_reference() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let local1 = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(local1),
                Rvalue::Use(Operand::const_int(42, DUMMY_TY)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_return_place() {
        let mut builder = MirTestBuilder::new();

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_int(42, DUMMY_TY)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_copy_and_move() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let local1 = builder.add_local(i32_ty, true);
        let local2 = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        // Copy
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(local2),
                Rvalue::Use(Operand::Copy(Place::from_local(local1))),
                0..0,
            ),
        );
        // Move
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::Move(Place::from_local(local2))),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_storage_live_dead() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let local = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(bb, Statement::storage_live(local, 0..0));
        builder.add_statement(bb, Statement::storage_dead(local, 0..0));
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_in_terminator() {
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let cond_local = builder.add_local(bool_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(cond_local),
                    targets: SwitchTargets::new_bool(bb1, bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_in_index_projection() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let usize_ty = builder
            .types
            .primitive(crate::sema::types::PrimitiveKind::Usize);
        let array_local = builder.add_local(i32_ty, true); // Simplified: treating as array
        let index_local = builder.add_local(usize_ty, false);

        let bb = builder.add_block();
        // array[index]
        let place = Place::from_local(array_local).project(PlaceElem::Index(index_local));
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::Copy(place)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_in_ref() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let local = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Ref(BorrowKind::Shared, Place::from_local(local), DUMMY_TY),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_in_binary_op() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let local1 = builder.add_local(i32_ty, true);
        let local2 = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    crate::mir::operand::BinOp::Add,
                    Operand::copy_local(local1),
                    Operand::copy_local(local2),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_in_call() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let result_local = builder.add_local(i32_ty, true);
        let arg_local = builder.add_local(i32_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Call {
                    func: Operand::Constant(Constant::FnDef(DefId(0))),
                    args: vec![Operand::copy_local(arg_local)],
                    destination: Place::from_local(result_local),
                    target: Some(bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_in_aggregate() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let local1 = builder.add_local(i32_ty, true);
        let local2 = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Aggregate(
                    crate::mir::operand::AggregateKind::Tuple,
                    vec![Operand::copy_local(local1), Operand::copy_local(local2)],
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_field_projection() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let struct_local = builder.add_local(i32_ty, true); // Simplified: treating as struct

        let bb = builder.add_block();
        // struct_local.0
        let place = Place::from_local(struct_local).project(PlaceElem::Field(FieldIdx(0)));
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::Copy(place)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(99) out of bounds")]
    fn local_invalid_out_of_bounds() {
        let mut builder = MirTestBuilder::new();

        let bb = builder.add_block();
        // Reference non-existent local 99
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local(99)),
                Rvalue::Use(Operand::const_int(42, DUMMY_TY)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(5) out of bounds")]
    fn local_invalid_in_place() {
        let mut builder = MirTestBuilder::new();

        let bb = builder.add_block();
        // Read from non-existent local 5
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::copy_local(Local(5))),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(3) out of bounds")]
    fn local_invalid_in_index_projection() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let array_local = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        // array[Local(3)] - but Local(3) doesn't exist
        let place = Place::from_local(array_local).project(PlaceElem::Index(Local(3)));
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::Copy(place)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(10) out of bounds")]
    fn local_invalid_in_terminator_switch() {
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(Local(10)), // Invalid
                    targets: SwitchTargets::new_bool(bb1, bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(5) out of bounds")]
    fn local_invalid_in_call_arg() {
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Call {
                    func: Operand::Constant(Constant::FnDef(DefId(0))),
                    args: vec![Operand::copy_local(Local(5))], // Invalid
                    destination: Place::from_local(Local::RETURN_PLACE),
                    target: Some(bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(5) out of bounds")]
    fn local_invalid_in_call_destination() {
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Call {
                    func: Operand::Constant(Constant::FnDef(DefId(0))),
                    args: vec![],
                    destination: Place::from_local(Local(5)), // Invalid
                    target: Some(bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(5) out of bounds")]
    fn local_invalid_in_storage_live() {
        let mut builder = MirTestBuilder::new();

        let bb = builder.add_block();
        builder.add_statement(bb, Statement::storage_live(Local(5), 0..0)); // Invalid
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(5) out of bounds")]
    fn local_invalid_in_drop() {
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Drop {
                    place: Place::from_local(Local(5)), // Invalid
                    target: bb1,
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    #[should_panic(expected = "Local(5) out of bounds")]
    fn local_invalid_in_assert() {
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Assert {
                    cond: Operand::copy_local(Local(5)), // Invalid
                    expected: true,
                    target: bb1,
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_empty_body() {
        let builder = MirTestBuilder::new();
        let (body, _) = builder.build();
        validate_locals(&body);
    }

    #[test]
    fn local_valid_nop_statement() {
        let mut builder = MirTestBuilder::new();

        let bb = builder.add_block();
        builder.add_statement(bb, Statement::nop(0..0));
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_locals(&body);
    }
}
