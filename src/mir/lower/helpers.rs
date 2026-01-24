//! Helper functions for HIR to MIR lowering.

use crate::hir::{BinOp as HirBinOp, HirDatabase, Literal, UnaryOp as HirUnaryOp};
use crate::mir::operand::{BinOp, CastKind, Constant, Operand, UnOp};
use crate::sema::types::{PrimitiveKind, Type, TypeId};

/// Convert an HIR literal to a MIR Constant with the specified type.
pub fn lower_literal(lit: &Literal, ty: TypeId) -> Constant {
    match lit {
        Literal::Int(v) => Constant::Int(*v, ty),
        Literal::Float(v) => Constant::Float(*v, ty),
        Literal::Bool(v) => Constant::Bool(*v),
        Literal::Char(v) => Constant::Char(*v),
        Literal::String(v) => Constant::String(v.clone()),
    }
}

/// Convert an HIR literal to a MIR Operand with the specified type.
pub fn literal_to_operand(lit: &Literal, ty: TypeId) -> Operand {
    Operand::Constant(lower_literal(lit, ty))
}

/// Convert an HIR binary operator to a MIR binary operator.
///
/// Returns `None` for operators that require special handling:
/// - `And`/`Or`: Short-circuit evaluation (control flow)
/// - `Assign`/`*Assign`: Assignment statements
pub fn hir_binop_to_mir(op: HirBinOp) -> Option<BinOp> {
    match op {
        // Arithmetic
        HirBinOp::Add => Some(BinOp::Add),
        HirBinOp::Sub => Some(BinOp::Sub),
        HirBinOp::Mul => Some(BinOp::Mul),
        HirBinOp::Div => Some(BinOp::Div),
        HirBinOp::Rem => Some(BinOp::Rem),
        // Comparison
        HirBinOp::Eq => Some(BinOp::Eq),
        HirBinOp::Ne => Some(BinOp::Ne),
        HirBinOp::Lt => Some(BinOp::Lt),
        HirBinOp::Le => Some(BinOp::Le),
        HirBinOp::Gt => Some(BinOp::Gt),
        HirBinOp::Ge => Some(BinOp::Ge),
        // Short-circuit: handled by control flow lowering
        HirBinOp::And | HirBinOp::Or => None,
        // Assignment: handled by statement lowering
        HirBinOp::Assign
        | HirBinOp::AddAssign
        | HirBinOp::SubAssign
        | HirBinOp::MulAssign
        | HirBinOp::DivAssign
        | HirBinOp::RemAssign => None,
    }
}

/// Convert an HIR unary operator to a MIR unary operator.
///
/// Returns `None` for operators that require special handling:
/// - `Deref`: Produces a place, not an rvalue
pub fn hir_unop_to_mir(op: HirUnaryOp) -> Option<UnOp> {
    match op {
        HirUnaryOp::Not => Some(UnOp::Not),
        HirUnaryOp::Neg => Some(UnOp::Neg),
        // Deref produces a place (projection), not an rvalue
        HirUnaryOp::Deref => None,
    }
}

/// Determine the cast kind for a cast between two types.
pub fn determine_cast_kind(hir: &HirDatabase, from: TypeId, to: TypeId) -> CastKind {
    let from_ty = hir.types.get(from);
    let to_ty = hir.types.get(to);

    // Check if source is an integer type
    let from_is_int = matches!(
        from_ty,
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
    );

    let from_is_float = matches!(
        from_ty,
        Type::Primitive(PrimitiveKind::F32 | PrimitiveKind::F64)
    );

    let to_is_int = matches!(
        to_ty,
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
    );

    let to_is_float = matches!(
        to_ty,
        Type::Primitive(PrimitiveKind::F32 | PrimitiveKind::F64)
    );

    match (from_is_int, from_is_float, to_is_int, to_is_float) {
        (true, _, true, _) => CastKind::IntToInt,
        (true, _, _, true) => CastKind::IntToFloat,
        (_, true, true, _) => CastKind::FloatToInt,
        (_, true, _, true) => CastKind::FloatToFloat,
        // Pointer casts or other cases
        _ => CastKind::PtrToPtr,
    }
}
