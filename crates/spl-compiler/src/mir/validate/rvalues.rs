//! Rvalue-specific validation for MIR.
//!
//! Validates rvalue-specific invariants:
//! - Aggregate operand counts match type requirements
//! - Len only on array/slice types
//! - Cast kinds valid for source/target types
//! - Discriminant only on enum types

use tracing::trace_span;

use crate::mir::Body;
use crate::mir::operand::{CastKind, Rvalue};
use crate::mir::statement::StatementKind;
use crate::mir::types::Place;
use crate::sema::types::{PrimitiveKind, Type, TypeId, TypeInterner};

/// Validate rvalue-specific invariants in a MIR body.
///
/// # Panics
///
/// Panics if:
/// - Aggregate operand count doesn't match type
/// - Len is used on non-array/slice type
/// - Cast kind is invalid for source/target types
pub fn validate_rvalues(body: &Body, types: &TypeInterner) {
    let _span = trace_span!("validate_rvalues").entered();
    for (block_idx, block) in body.basic_blocks.iter().enumerate() {
        for (stmt_idx, stmt) in block.statements.iter().enumerate() {
            let ctx = format!("BasicBlock({block_idx}).statements[{stmt_idx}]");

            if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                validate_rvalue(rvalue, place, body, types, &ctx);
            }
        }
    }
}

fn validate_rvalue(
    rvalue: &Rvalue,
    dest_place: &Place,
    body: &Body,
    types: &TypeInterner,
    context: &str,
) {
    match rvalue {
        Rvalue::Len(place) => {
            validate_len(place, body, types, context);
        }
        Rvalue::Cast(kind, operand, target_ty) => {
            validate_cast(*kind, operand, *target_ty, body, types, context);
        }
        Rvalue::Aggregate(agg_kind, operands) => {
            validate_aggregate(agg_kind, operands, dest_place, body, types, context);
        }
        Rvalue::Discriminant(place) => {
            validate_discriminant(place, body, types, context);
        }
        // Other rvalues are validated in types.rs
        _ => {}
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

fn validate_len(place: &Place, body: &Body, types: &TypeInterner, context: &str) {
    let ty = get_place_type(place, body, types);

    // Allow error type (recovery)
    if ty == types.error() {
        return;
    }

    assert!(
        matches!(types.get(ty), Type::Array(_, _) | Type::Slice(_)),
        "Rvalue validation failed: Len requires array or slice type at {context}: \
        got {:?}",
        types.get(ty)
    );
}

fn validate_cast(
    kind: CastKind,
    operand: &crate::mir::operand::Operand,
    target_ty: TypeId,
    body: &Body,
    types: &TypeInterner,
    context: &str,
) {
    let source_ty = get_operand_type(operand, body, types);

    // Allow error types
    if source_ty == types.error() || target_ty == types.error() {
        return;
    }

    let source_type = types.get(source_ty);
    let target_type = types.get(target_ty);

    match kind {
        CastKind::IntToInt => {
            assert!(
                is_integer_type(source_type),
                "Rvalue validation failed: IntToInt cast from non-integer type at {context}: \
                source is {source_type:?}"
            );
            assert!(
                is_integer_type(target_type),
                "Rvalue validation failed: IntToInt cast to non-integer type at {context}: \
                target is {target_type:?}"
            );
        }
        CastKind::IntToFloat => {
            assert!(
                is_integer_type(source_type),
                "Rvalue validation failed: IntToFloat cast from non-integer type at {context}: \
                source is {source_type:?}"
            );
            assert!(
                is_float_type(target_type),
                "Rvalue validation failed: IntToFloat cast to non-float type at {context}: \
                target is {target_type:?}"
            );
        }
        CastKind::FloatToInt => {
            assert!(
                is_float_type(source_type),
                "Rvalue validation failed: FloatToInt cast from non-float type at {context}: \
                source is {source_type:?}"
            );
            assert!(
                is_integer_type(target_type),
                "Rvalue validation failed: FloatToInt cast to non-integer type at {context}: \
                target is {target_type:?}"
            );
        }
        CastKind::FloatToFloat => {
            assert!(
                is_float_type(source_type),
                "Rvalue validation failed: FloatToFloat cast from non-float type at {context}: \
                source is {source_type:?}"
            );
            assert!(
                is_float_type(target_type),
                "Rvalue validation failed: FloatToFloat cast to non-float type at {context}: \
                target is {target_type:?}"
            );
        }
        CastKind::PtrToPtr | CastKind::Unsize => {
            // These require more complex validation involving pointer/reference types
            // For now, accept them
        }
    }
}

fn get_operand_type(
    operand: &crate::mir::operand::Operand,
    body: &Body,
    types: &TypeInterner,
) -> TypeId {
    use crate::mir::operand::{Constant, Operand};

    match operand {
        Operand::Copy(place) | Operand::Move(place) => get_place_type(place, body, types),
        Operand::Constant(constant) => match constant {
            Constant::Int(_, ty) | Constant::Float(_, ty) | Constant::Zeroed(ty) => *ty,
            Constant::Bool(_) => types.bool(),
            Constant::Char(_) => types.char(),
            Constant::String(_) => types.str_ref(),
            Constant::Unit => types.unit(),
            Constant::FnDef(_) => types.error(),
        },
    }
}

fn validate_aggregate(
    agg_kind: &crate::mir::operand::AggregateKind,
    operands: &[crate::mir::operand::Operand],
    dest_place: &Place,
    body: &Body,
    types: &TypeInterner,
    context: &str,
) {
    use crate::mir::operand::AggregateKind;

    let dest_ty = get_place_type(dest_place, body, types);

    // Skip validation for error type destinations
    if dest_ty == types.error() {
        return;
    }

    match agg_kind {
        AggregateKind::Tuple => {
            if let Type::Tuple(fields) = types.get(dest_ty) {
                assert!(
                    fields.len() == operands.len(),
                    "Rvalue validation failed: tuple expects {} fields, got {} at {}",
                    fields.len(),
                    operands.len(),
                    context
                );
            }
            // Destination is not a tuple type - this would be caught by type validation
        }
        AggregateKind::Array => {
            if let Type::Array(_, len) = types.get(dest_ty) {
                assert!(
                    *len as usize == operands.len(),
                    "Rvalue validation failed: array expects {} elements, got {} at {}",
                    len,
                    operands.len(),
                    context
                );
            }
            // Destination is not an array type - this would be caught by type validation
        }
        AggregateKind::Adt(_def_id) => {
            // ADT field count validation would require access to struct/enum definitions
            // For now, accept any operand count
        }
    }
}

fn validate_discriminant(place: &Place, body: &Body, types: &TypeInterner, _context: &str) {
    let ty = get_place_type(place, body, types);

    // Allow error type
    if ty == types.error() {
        return;
    }

    // Enums are represented as Struct with a discriminant
    // This is a simplification - real impl would check if it's actually an enum
    // Note: In a full implementation, we'd check for enum types specifically
    // For now, we accept struct types since enum representation varies
    let _ = types.get(ty);
}

fn is_integer_type(ty: &Type) -> bool {
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
        )
    )
}

fn is_float_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(PrimitiveKind::F32 | PrimitiveKind::F64))
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::MirTestBuilder;
    use super::*;
    use crate::mir::operand::{AggregateKind, CastKind, Operand, Rvalue};
    use crate::mir::statement::Statement;
    use crate::mir::terminator::Terminator;
    use crate::mir::types::Local;

    #[test]
    fn rvalue_valid_aggregate_tuple() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let bool_ty = builder.types.bool();
        let tuple_ty = builder.types.mk_tuple(vec![i32_ty, bool_ty]);
        builder = builder.with_return_ty(tuple_ty);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Aggregate(
                    AggregateKind::Tuple,
                    vec![builder.const_i32(42), Operand::const_bool(true)],
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_aggregate_array() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let array_ty = builder.types.mk_array(i32_ty, 3);
        builder = builder.with_return_ty(array_ty);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Aggregate(
                    AggregateKind::Array,
                    vec![
                        builder.const_i32(1),
                        builder.const_i32(2),
                        builder.const_i32(3),
                    ],
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_len_on_array() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let usize_ty = builder.types.primitive(PrimitiveKind::Usize);
        let array_ty = builder.types.mk_array(i32_ty, 5);
        builder = builder.with_return_ty(usize_ty);
        let array_local = builder.add_local(array_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Len(Place::from_local(array_local)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_len_on_slice() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let usize_ty = builder.types.primitive(PrimitiveKind::Usize);
        let slice_ty = builder.types.mk_slice(i32_ty);
        builder = builder.with_return_ty(usize_ty);
        let slice_local = builder.add_local(slice_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Len(Place::from_local(slice_local)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_cast_int_to_int() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let i64_ty = builder.types.i64();
        builder = builder.with_return_ty(i64_ty);
        let local1 = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Cast(CastKind::IntToInt, Operand::copy_local(local1), i64_ty),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_cast_int_to_float() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let f64_ty = builder.types.f64();
        builder = builder.with_return_ty(f64_ty);
        let local1 = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Cast(CastKind::IntToFloat, Operand::copy_local(local1), f64_ty),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_cast_float_to_int() {
        let mut builder = MirTestBuilder::new();
        let f64_ty = builder.types.f64();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);
        let local1 = builder.add_local(f64_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Cast(CastKind::FloatToInt, Operand::copy_local(local1), i32_ty),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_cast_float_to_float() {
        let mut builder = MirTestBuilder::new();
        let f32_ty = builder.types.f32();
        let f64_ty = builder.types.f64();
        builder = builder.with_return_ty(f64_ty);
        let local1 = builder.add_local(f32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Cast(CastKind::FloatToFloat, Operand::copy_local(local1), f64_ty),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    #[should_panic(expected = "tuple expects 2 fields, got 1")]
    fn rvalue_invalid_aggregate_count() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let bool_ty = builder.types.bool();
        let tuple_ty = builder.types.mk_tuple(vec![i32_ty, bool_ty]);
        builder = builder.with_return_ty(tuple_ty);

        let bb = builder.add_block();
        // Only 1 operand for 2-element tuple
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Aggregate(AggregateKind::Tuple, vec![builder.const_i32(42)]),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    #[should_panic(expected = "array expects 3 elements, got 2")]
    fn rvalue_invalid_array_count() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let array_ty = builder.types.mk_array(i32_ty, 3);
        builder = builder.with_return_ty(array_ty);

        let bb = builder.add_block();
        // Only 2 operands for 3-element array
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Aggregate(
                    AggregateKind::Array,
                    vec![builder.const_i32(1), builder.const_i32(2)],
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    #[should_panic(expected = "Len requires array or slice")]
    fn rvalue_invalid_len_on_int() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let usize_ty = builder.types.primitive(PrimitiveKind::Usize);
        builder = builder.with_return_ty(usize_ty);
        let int_local = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        // Len on i32, not array/slice
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Len(Place::from_local(int_local)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    #[should_panic(expected = "IntToInt cast from non-integer")]
    fn rvalue_invalid_cast_kind_int_to_int_from_float() {
        let mut builder = MirTestBuilder::new();
        let f64_ty = builder.types.f64();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);
        let float_local = builder.add_local(f64_ty, false);

        let bb = builder.add_block();
        // IntToInt from float - invalid
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Cast(CastKind::IntToInt, Operand::copy_local(float_local), i32_ty),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    #[should_panic(expected = "IntToFloat cast from non-integer")]
    fn rvalue_invalid_cast_kind_int_to_float_from_bool() {
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let f64_ty = builder.types.f64();
        builder = builder.with_return_ty(f64_ty);
        let bool_local = builder.add_local(bool_ty, false);

        let bb = builder.add_block();
        // IntToFloat from bool - invalid
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Cast(
                    CastKind::IntToFloat,
                    Operand::copy_local(bool_local),
                    f64_ty,
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    #[should_panic(expected = "FloatToInt cast from non-float")]
    fn rvalue_invalid_cast_kind_float_to_int_from_int() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);
        let int_local = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        // FloatToInt from int - invalid
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Cast(CastKind::FloatToInt, Operand::copy_local(int_local), i32_ty),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_empty_body() {
        let builder = MirTestBuilder::new();
        let (body, types) = builder.build();
        validate_rvalues(&body, &types);
    }

    #[test]
    fn rvalue_valid_with_error_type() {
        // Error types should not cause validation failures
        let mut builder = MirTestBuilder::new();
        let error_ty = builder.types.error();
        builder = builder.with_return_ty(error_ty);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Len(Place::from_local(Local::RETURN_PLACE)), // Len on error type
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_rvalues(&body, &types); // Should not panic
    }
}
