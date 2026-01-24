//! Terminator-specific validation for MIR.
//!
//! Validates terminator-specific invariants:
//! - SwitchInt discriminant must be integer type
//! - Assert condition must be bool

use crate::mir::Body;
use crate::mir::operand::Operand;
use crate::mir::terminator::TerminatorKind;
use crate::mir::types::Place;
use crate::sema::types::{PrimitiveKind, Type, TypeId, TypeInterner};

/// Validate terminator-specific invariants in a MIR body.
///
/// # Panics
///
/// Panics if:
/// - SwitchInt discriminant is not an integer type
/// - Assert condition is not bool
pub fn validate_terminators(body: &Body, types: &TypeInterner) {
    for (block_idx, block) in body.basic_blocks.iter().enumerate() {
        if let Some(term) = &block.terminator {
            let ctx = format!("BasicBlock({}).terminator", block_idx);
            validate_terminator(&term.kind, body, types, &ctx);
        }
    }
}

fn validate_terminator(kind: &TerminatorKind, body: &Body, types: &TypeInterner, context: &str) {
    match kind {
        TerminatorKind::SwitchInt { discr, .. } => {
            validate_switch_int_discr(discr, body, types, context);
        }
        TerminatorKind::Assert { cond, .. } => {
            validate_assert_cond(cond, body, types, context);
        }
        TerminatorKind::Call { destination, .. } => {
            // Could validate that destination type matches function return type
            // For now, just ensure destination is valid (already done by locals validation)
            let _ = destination;
        }
        TerminatorKind::Return
        | TerminatorKind::Goto(_)
        | TerminatorKind::Drop { .. }
        | TerminatorKind::Unreachable
        | TerminatorKind::Resume => {
            // No additional validation needed
        }
    }
}

fn validate_switch_int_discr(discr: &Operand, body: &Body, types: &TypeInterner, context: &str) {
    let discr_ty = get_operand_type(discr, body, types);

    // Allow error type
    if discr_ty == types.error() {
        return;
    }

    let discr_type = types.get(discr_ty);

    // SwitchInt discriminant must be an integer type (including bool, which is treated as integer)
    if !is_switchable_type(discr_type) {
        panic!(
            "Terminator validation failed: SwitchInt discriminant must be integer type at {}: \
            got {:?}",
            context, discr_type
        );
    }
}

fn validate_assert_cond(cond: &Operand, body: &Body, types: &TypeInterner, context: &str) {
    let cond_ty = get_operand_type(cond, body, types);

    // Allow error type
    if cond_ty == types.error() {
        return;
    }

    let cond_type = types.get(cond_ty);

    // Assert condition must be bool
    if !matches!(cond_type, Type::Primitive(PrimitiveKind::Bool)) {
        panic!(
            "Terminator validation failed: Assert condition must be bool at {}: \
            got {:?}",
            context, cond_type
        );
    }
}

fn is_switchable_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Primitive(
            PrimitiveKind::I8
                | PrimitiveKind::I16
                | PrimitiveKind::I32
                | PrimitiveKind::I64
                | PrimitiveKind::I128
                | PrimitiveKind::Isize
                | PrimitiveKind::U8
                | PrimitiveKind::U16
                | PrimitiveKind::U32
                | PrimitiveKind::U64
                | PrimitiveKind::U128
                | PrimitiveKind::Usize
                | PrimitiveKind::Bool
                | PrimitiveKind::Char
        )
    )
}

fn get_operand_type(operand: &Operand, body: &Body, types: &TypeInterner) -> TypeId {
    use crate::mir::operand::Constant;

    match operand {
        Operand::Copy(place) | Operand::Move(place) => get_place_type(place, body, types),
        Operand::Constant(constant) => match constant {
            Constant::Int(_, ty) => *ty,
            Constant::Float(_, ty) => *ty,
            Constant::Bool(_) => types.bool(),
            Constant::Char(_) => types.char(),
            Constant::String(_) => types.str_ref(),
            Constant::Unit => types.unit(),
            Constant::FnDef(_) => types.error(),
            Constant::Zeroed(ty) => *ty,
        },
    }
}

fn get_place_type(place: &Place, body: &Body, types: &TypeInterner) -> TypeId {
    let mut ty = body.local_decl(place.local).ty;

    for elem in &place.projection {
        ty = project_type(ty, elem, types);
    }

    ty
}

fn project_type(ty: TypeId, elem: &crate::mir::types::PlaceElem, types: &TypeInterner) -> TypeId {
    use crate::mir::types::PlaceElem;

    match elem {
        PlaceElem::Deref => match types.get(ty) {
            Type::Ref(_, inner) => *inner,
            _ => types.error(),
        },
        PlaceElem::Field(field_idx) => match types.get(ty) {
            Type::Tuple(fields) => {
                let idx = field_idx.index() as usize;
                if idx < fields.len() {
                    fields[idx]
                } else {
                    types.error()
                }
            }
            _ => types.error(),
        },
        PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => match types.get(ty) {
            Type::Array(elem, _) | Type::Slice(elem) => *elem,
            _ => types.error(),
        },
        PlaceElem::Subslice { .. } | PlaceElem::Downcast(_) => ty,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::MirTestBuilder;
    use super::*;
    use crate::mir::operand::Operand;
    use crate::mir::terminator::{SwitchTargets, Terminator, TerminatorKind};

    #[test]
    fn term_valid_switch_on_bool() {
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

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    fn term_valid_switch_on_int() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let discr_local = builder.add_local(i32_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(discr_local),
                    targets: SwitchTargets::new(
                        vec![(0, bb1), (1, bb2)],
                        bb1, // otherwise
                    ),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    fn term_valid_switch_on_char() {
        let mut builder = MirTestBuilder::new();
        let char_ty = builder.types.char();
        let discr_local = builder.add_local(char_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(discr_local),
                    targets: SwitchTargets::new(vec![], bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    fn term_valid_switch_on_all_int_types() {
        for prim in [
            PrimitiveKind::I8,
            PrimitiveKind::I16,
            PrimitiveKind::I32,
            PrimitiveKind::I64,
            PrimitiveKind::I128,
            PrimitiveKind::Isize,
            PrimitiveKind::U8,
            PrimitiveKind::U16,
            PrimitiveKind::U32,
            PrimitiveKind::U64,
            PrimitiveKind::U128,
            PrimitiveKind::Usize,
        ] {
            let mut builder = MirTestBuilder::new();
            let ty = builder.types.primitive(prim);
            let discr_local = builder.add_local(ty, false);

            let bb0 = builder.add_block();
            let bb1 = builder.add_block();

            builder.set_terminator(
                bb0,
                Terminator::new(
                    TerminatorKind::SwitchInt {
                        discr: Operand::copy_local(discr_local),
                        targets: SwitchTargets::new(vec![], bb1),
                    },
                    0..0,
                ),
            );
            builder.set_terminator(bb1, Terminator::return_(0..0));

            let (body, types) = builder.build();
            validate_terminators(&body, &types);
        }
    }

    #[test]
    fn term_valid_assert_bool() {
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let cond_local = builder.add_local(bool_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Assert {
                    cond: Operand::copy_local(cond_local),
                    expected: true,
                    target: bb1,
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    fn term_valid_assert_const_bool() {
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Assert {
                    cond: Operand::const_bool(true),
                    expected: true,
                    target: bb1,
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    #[should_panic(expected = "SwitchInt discriminant must be integer")]
    fn term_invalid_switch_on_float() {
        let mut builder = MirTestBuilder::new();
        let f64_ty = builder.types.f64();
        let discr_local = builder.add_local(f64_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(discr_local),
                    targets: SwitchTargets::new(vec![], bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    #[should_panic(expected = "SwitchInt discriminant must be integer")]
    fn term_invalid_switch_on_unit() {
        let mut builder = MirTestBuilder::new();
        let unit_ty = builder.types.unit();
        let discr_local = builder.add_local(unit_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(discr_local),
                    targets: SwitchTargets::new(vec![], bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    #[should_panic(expected = "Assert condition must be bool")]
    fn term_invalid_assert_int() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let cond_local = builder.add_local(i32_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Assert {
                    cond: Operand::copy_local(cond_local),
                    expected: true,
                    target: bb1,
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    #[should_panic(expected = "Assert condition must be bool")]
    fn term_invalid_assert_unit() {
        let mut builder = MirTestBuilder::new();
        let unit_ty = builder.types.unit();
        let cond_local = builder.add_local(unit_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::Assert {
                    cond: Operand::copy_local(cond_local),
                    expected: true,
                    target: bb1,
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    fn term_valid_empty_body() {
        let builder = MirTestBuilder::new();
        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }

    #[test]
    fn term_valid_with_error_type() {
        // Error types should not cause validation failures
        let mut builder = MirTestBuilder::new();
        let error_ty = builder.types.error();
        let discr_local = builder.add_local(error_ty, false);

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        builder.set_terminator(
            bb0,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::copy_local(discr_local),
                    targets: SwitchTargets::new(vec![], bb1),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb1, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types); // Should not panic
    }

    #[test]
    fn term_valid_return_goto_unreachable() {
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::return_(0..0));
        builder.set_terminator(bb2, Terminator::unreachable(0..0));

        let (body, types) = builder.build();
        validate_terminators(&body, &types);
    }
}
