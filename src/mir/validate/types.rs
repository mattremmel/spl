//! Type validation for MIR.
//!
//! Validates type consistency of operations:
//! - Assignment types match
//! - Binary operations have compatible operand types
//! - Unary operations are valid for operand types

use crate::mir::Body;
use crate::mir::operand::{BinOp, Operand, Rvalue, UnOp};
use crate::mir::statement::StatementKind;
use crate::mir::types::{Place, PlaceElem};
use crate::sema::types::{PrimitiveKind, Type, TypeId, TypeInterner};

/// Validate type consistency of operations in a MIR body.
///
/// # Panics
///
/// Panics if:
/// - Assignment LHS type doesn't match RHS type
/// - Binary operation operands have incompatible types
/// - Unary operation operand has invalid type
pub fn validate_types(body: &Body, types: &TypeInterner) {
    for (block_idx, block) in body.basic_blocks.iter().enumerate() {
        for (stmt_idx, stmt) in block.statements.iter().enumerate() {
            let ctx = format!("BasicBlock({}).statements[{}]", block_idx, stmt_idx);
            validate_statement_types(&stmt.kind, body, types, &ctx);
        }
    }
}

fn validate_statement_types(
    kind: &StatementKind,
    body: &Body,
    types: &TypeInterner,
    context: &str,
) {
    if let StatementKind::Assign(place, rvalue) = kind {
        let place_ty = get_place_type(place, body, types);
        let rvalue_ty = get_rvalue_type(rvalue, body, types);

        // Skip validation for error types (they unify with everything)
        if place_ty == types.error() || rvalue_ty == types.error() {
            return;
        }

        if place_ty != rvalue_ty {
            panic!(
                "Type validation failed: type mismatch in assignment at {}: \
                place has type {:?} but rvalue has type {:?}",
                context,
                types.get(place_ty),
                types.get(rvalue_ty)
            );
        }

        // Validate rvalue-specific type constraints
        validate_rvalue_type_constraints(rvalue, body, types, context);
    }
}

fn validate_rvalue_type_constraints(
    rvalue: &Rvalue,
    body: &Body,
    types: &TypeInterner,
    context: &str,
) {
    match rvalue {
        Rvalue::BinaryOp(op, lhs, rhs) => {
            let lhs_ty = get_operand_type(lhs, body, types);
            let rhs_ty = get_operand_type(rhs, body, types);

            // Skip if error types
            if lhs_ty == types.error() || rhs_ty == types.error() {
                return;
            }

            // For now, validate that operands have the same type for most binops
            // (In a real compiler, we'd have more nuanced rules per operator)
            if !is_binop_valid(op, lhs_ty, rhs_ty, types) {
                panic!(
                    "Type validation failed: incompatible types for {:?} at {}: \
                    lhs has type {:?}, rhs has type {:?}",
                    op,
                    context,
                    types.get(lhs_ty),
                    types.get(rhs_ty)
                );
            }
        }
        Rvalue::UnaryOp(op, operand) => {
            let operand_ty = get_operand_type(operand, body, types);

            if operand_ty == types.error() {
                return;
            }

            if !is_unop_valid(op, operand_ty, types) {
                panic!(
                    "Type validation failed: {:?} requires {} type at {}: \
                    operand has type {:?}",
                    op,
                    unop_type_requirement(op),
                    context,
                    types.get(operand_ty)
                );
            }
        }
        _ => {}
    }
}

fn is_binop_valid(op: &BinOp, lhs_ty: TypeId, rhs_ty: TypeId, types: &TypeInterner) -> bool {
    // Operands should have the same type (simplified rule)
    if lhs_ty != rhs_ty {
        return false;
    }

    let lhs_type = types.get(lhs_ty);

    match op {
        // Arithmetic ops require numeric types
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => is_numeric_type(lhs_type),
        // Bitwise ops require integer types (or bool for And/Or/Xor)
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
            is_integer_type(lhs_type) || matches!(lhs_type, Type::Primitive(PrimitiveKind::Bool))
        }
        BinOp::Shl | BinOp::Shr => is_integer_type(lhs_type),
        // Comparison ops work on most types
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => true,
    }
}

fn is_unop_valid(op: &UnOp, operand_ty: TypeId, types: &TypeInterner) -> bool {
    let operand_type = types.get(operand_ty);

    match op {
        UnOp::Neg => is_signed_numeric_type(operand_type),
        UnOp::Not => {
            is_integer_type(operand_type)
                || matches!(operand_type, Type::Primitive(PrimitiveKind::Bool))
        }
    }
}

fn unop_type_requirement(op: &UnOp) -> &'static str {
    match op {
        UnOp::Neg => "numeric",
        UnOp::Not => "integer or bool",
    }
}

fn is_numeric_type(ty: &Type) -> bool {
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
                | PrimitiveKind::F32
                | PrimitiveKind::F64
        )
    )
}

fn is_signed_numeric_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Primitive(
            PrimitiveKind::I8
                | PrimitiveKind::I16
                | PrimitiveKind::I32
                | PrimitiveKind::I64
                | PrimitiveKind::I128
                | PrimitiveKind::Isize
                | PrimitiveKind::F32
                | PrimitiveKind::F64
        )
    )
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

/// Get the type of a place.
fn get_place_type(place: &Place, body: &Body, types: &TypeInterner) -> TypeId {
    let mut ty = body.local_decl(place.local).ty;

    for elem in &place.projection {
        ty = project_type(ty, elem, types);
    }

    ty
}

/// Project a type through a projection element.
fn project_type(ty: TypeId, elem: &PlaceElem, types: &TypeInterner) -> TypeId {
    match elem {
        PlaceElem::Deref => {
            match types.get(ty) {
                Type::Ref(_, inner) => *inner,
                // Raw pointers and other indirections would go here
                _ => types.error(), // Return error type for invalid projections
            }
        }
        PlaceElem::Field(field_idx) => {
            match types.get(ty) {
                Type::Tuple(fields) => {
                    let idx = field_idx.index() as usize;
                    if idx < fields.len() {
                        fields[idx]
                    } else {
                        types.error()
                    }
                }
                Type::Struct(_, _) => {
                    // For now, return error - would need struct field info
                    types.error()
                }
                _ => types.error(),
            }
        }
        PlaceElem::Index(_) => match types.get(ty) {
            Type::Array(elem, _) | Type::Slice(elem) => *elem,
            _ => types.error(),
        },
        PlaceElem::ConstantIndex { .. } => match types.get(ty) {
            Type::Array(elem, _) | Type::Slice(elem) => *elem,
            _ => types.error(),
        },
        PlaceElem::Subslice { .. } => ty, // Subslice preserves type
        PlaceElem::Downcast(_) => ty,     // Downcast preserves base type
    }
}

/// Get the type of an operand.
fn get_operand_type(operand: &Operand, body: &Body, types: &TypeInterner) -> TypeId {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => get_place_type(place, body, types),
        Operand::Constant(constant) => get_constant_type(constant, types),
    }
}

/// Get the type of a constant.
fn get_constant_type(constant: &crate::mir::operand::Constant, types: &TypeInterner) -> TypeId {
    use crate::mir::operand::Constant;

    match constant {
        Constant::Int(_, ty) => *ty,
        Constant::Float(_, ty) => *ty,
        Constant::Bool(_) => types.bool(),
        Constant::Char(_) => types.char(),
        Constant::String(_) => types.str_ref(),
        Constant::Unit => types.unit(),
        Constant::FnDef(_) => types.error(), // Would need function signature lookup
        Constant::Zeroed(ty) => *ty,
    }
}

/// Get the type of an rvalue.
fn get_rvalue_type(rvalue: &Rvalue, body: &Body, types: &TypeInterner) -> TypeId {
    match rvalue {
        Rvalue::Use(operand) => get_operand_type(operand, body, types),
        Rvalue::Ref(_, _, ref_ty) => *ref_ty,
        Rvalue::AddressOf(_, _, ptr_ty) => *ptr_ty,
        Rvalue::BinaryOp(op, lhs, _) => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => types.bool(),
            _ => get_operand_type(lhs, body, types),
        },
        Rvalue::UnaryOp(_, operand) => get_operand_type(operand, body, types),
        Rvalue::Cast(_, _, target_ty) => *target_ty,
        Rvalue::Len(_) => {
            // Len returns usize, but we can't create types without mutable interner
            // Return error type - actual Len validation is in rvalues.rs
            types.error()
        }
        Rvalue::Aggregate(_, _) => {
            // Would need aggregate type info
            types.error()
        }
        Rvalue::Discriminant(_) => {
            // Discriminant is typically isize, but we can't create types without mutable interner
            // Return error type - actual Discriminant validation is in rvalues.rs
            types.error()
        }
        Rvalue::Repeat(operand, count) => {
            let elem_ty = get_operand_type(operand, body, types);
            // Can't create array type without mutable interner
            let _ = count;
            elem_ty
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::MirTestBuilder;
    use super::*;
    use crate::mir::operand::CastKind;
    use crate::mir::statement::Statement;
    use crate::mir::terminator::Terminator;
    use crate::mir::types::Local;

    // Dummy type ID for structural tests (not validation)
    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn type_valid_int_assignment() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(builder.const_i32(42)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_bool_assignment() {
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(bool_ty);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_bool(true)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_local_to_local() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);
        let local1 = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        // local1 = 42
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(local1),
                Rvalue::Use(builder.const_i32(42)),
                0..0,
            ),
        );
        // _0 = local1
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::copy_local(local1)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_binop_add_i32() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);
        let local1 = builder.add_local(i32_ty, false);
        let local2 = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::copy_local(local1),
                    Operand::copy_local(local2),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_binop_comparison() {
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(bool_ty);
        let local1 = builder.add_local(i32_ty, false);
        let local2 = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    BinOp::Lt,
                    Operand::copy_local(local1),
                    Operand::copy_local(local2),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_unop_neg_i32() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);
        let local1 = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::UnaryOp(UnOp::Neg, Operand::copy_local(local1)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_unop_not_bool() {
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(bool_ty);
        let local1 = builder.add_local(bool_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::UnaryOp(UnOp::Not, Operand::copy_local(local1)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_unop_not_i32() {
        // Bitwise NOT on integer
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);
        let local1 = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::UnaryOp(UnOp::Not, Operand::copy_local(local1)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_cast() {
        let mut builder = MirTestBuilder::new();
        let f64_ty = builder.types.f64();
        let i32_ty = builder.types.i32();
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
        validate_types(&body, &types);
    }

    #[test]
    #[should_panic(expected = "type mismatch")]
    fn type_invalid_bool_to_int() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);

        let bb = builder.add_block();
        // Assigning bool to i32 place
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_bool(true)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    #[should_panic(expected = "incompatible types for Add")]
    fn type_invalid_binop_add_bool() {
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(bool_ty);
        let local1 = builder.add_local(bool_ty, false);
        let local2 = builder.add_local(bool_ty, false);

        let bb = builder.add_block();
        // Can't add booleans
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::copy_local(local1),
                    Operand::copy_local(local2),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    #[should_panic(expected = "Neg requires numeric")]
    fn type_invalid_unop_neg_bool() {
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(bool_ty);
        let local1 = builder.add_local(bool_ty, false);

        let bb = builder.add_block();
        // Can't negate boolean
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::UnaryOp(UnOp::Neg, Operand::copy_local(local1)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    #[should_panic(expected = "Neg requires numeric")]
    fn type_invalid_unop_neg_unsigned() {
        let mut builder = MirTestBuilder::new();
        let u32_ty = builder.types.primitive(PrimitiveKind::U32);
        builder = builder.with_return_ty(u32_ty);
        let local1 = builder.add_local(u32_ty, false);

        let bb = builder.add_block();
        // Can't negate unsigned
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::UnaryOp(UnOp::Neg, Operand::copy_local(local1)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_bitwise_bool() {
        // BitAnd, BitOr, BitXor are valid on bool
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(bool_ty);
        let local1 = builder.add_local(bool_ty, false);
        let local2 = builder.add_local(bool_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    BinOp::BitAnd,
                    Operand::copy_local(local1),
                    Operand::copy_local(local2),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    #[should_panic(expected = "incompatible types for Shl")]
    fn type_invalid_shift_bool() {
        // Shifts are not valid on bool
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        builder = builder.with_return_ty(bool_ty);
        let local1 = builder.add_local(bool_ty, false);
        let local2 = builder.add_local(bool_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    BinOp::Shl,
                    Operand::copy_local(local1),
                    Operand::copy_local(local2),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_ref_shared() {
        // This test validates that ref creation doesn't panic
        // (Full ref type validation requires mutable interner)
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        // Create a reference type to use for the Rvalue::Ref
        let ref_i32_ty = builder
            .types
            .mk_ref(crate::sema::types::Mutability::Shared, i32_ty);
        builder = builder.with_return_ty(ref_i32_ty);
        let local1 = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Ref(
                    crate::mir::operand::BorrowKind::Shared,
                    Place::from_local(local1),
                    ref_i32_ty,
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        // Note: This won't fully validate ref types due to interner limitations
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_with_error_type() {
        // Error types should not cause validation failures
        let mut builder = MirTestBuilder::new();
        let error_ty = builder.types.error();
        builder = builder.with_return_ty(error_ty);

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

        let (body, types) = builder.build();
        validate_types(&body, &types); // Should not panic - error type unifies with anything
    }

    #[test]
    fn type_valid_empty_body() {
        let builder = MirTestBuilder::new();
        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    fn type_valid_all_signed_int_types() {
        for prim in [
            PrimitiveKind::I8,
            PrimitiveKind::I16,
            PrimitiveKind::I32,
            PrimitiveKind::I64,
            PrimitiveKind::I128,
            PrimitiveKind::Isize,
        ] {
            let mut builder = MirTestBuilder::new();
            let ty = builder.types.primitive(prim);
            builder = builder.with_return_ty(ty);
            let local1 = builder.add_local(ty, false);

            let bb = builder.add_block();
            builder.add_statement(
                bb,
                Statement::assign(
                    Place::from_local(Local::RETURN_PLACE),
                    Rvalue::UnaryOp(UnOp::Neg, Operand::copy_local(local1)),
                    0..0,
                ),
            );
            builder.set_terminator(bb, Terminator::return_(0..0));

            let (body, types) = builder.build();
            validate_types(&body, &types);
        }
    }

    #[test]
    fn type_valid_float_neg() {
        for prim in [PrimitiveKind::F32, PrimitiveKind::F64] {
            let mut builder = MirTestBuilder::new();
            let ty = builder.types.primitive(prim);
            builder = builder.with_return_ty(ty);
            let local1 = builder.add_local(ty, false);

            let bb = builder.add_block();
            builder.add_statement(
                bb,
                Statement::assign(
                    Place::from_local(Local::RETURN_PLACE),
                    Rvalue::UnaryOp(UnOp::Neg, Operand::copy_local(local1)),
                    0..0,
                ),
            );
            builder.set_terminator(bb, Terminator::return_(0..0));

            let (body, types) = builder.build();
            validate_types(&body, &types);
        }
    }

    // ===== spl-812: Typed constants tests =====

    #[test]
    fn type_valid_i64_constant_to_i64_place() {
        // i64 constant assigned to i64 return place - should pass
        let mut builder = MirTestBuilder::new();
        let i64_ty = builder.types.i64();
        builder = builder.with_return_ty(i64_ty);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::Constant(crate::mir::operand::Constant::Int(
                    42, i64_ty,
                ))),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    #[should_panic(expected = "type mismatch")]
    fn type_invalid_i64_constant_to_i32_place() {
        // i64 constant assigned to i32 return place - should FAIL
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let i64_ty = builder.types.i64();
        builder = builder.with_return_ty(i32_ty);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::Constant(crate::mir::operand::Constant::Int(
                    42, i64_ty,
                ))),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    // ===== spl-ii5: Reference types tests =====

    #[test]
    fn type_valid_ref_to_ref_place() {
        // &i32 assigned to &i32 place - should pass
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let ref_ty = builder
            .types
            .mk_ref(crate::sema::types::Mutability::Shared, i32_ty);
        builder = builder.with_return_ty(ref_ty);
        let local = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Ref(
                    crate::mir::operand::BorrowKind::Shared,
                    Place::from_local(local),
                    ref_ty,
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }

    #[test]
    #[should_panic(expected = "type mismatch")]
    fn type_invalid_ref_to_non_ref_place() {
        // &i32 assigned to i32 place - should FAIL
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let ref_ty = builder
            .types
            .mk_ref(crate::sema::types::Mutability::Shared, i32_ty);
        builder = builder.with_return_ty(i32_ty); // NOT &i32
        let local = builder.add_local(i32_ty, false);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Ref(
                    crate::mir::operand::BorrowKind::Shared,
                    Place::from_local(local),
                    ref_ty,
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, types) = builder.build();
        validate_types(&body, &types);
    }
}
