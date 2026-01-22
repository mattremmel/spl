//! MIR operands and rvalues.
//!
//! Operands are the inputs to MIR operations. Rvalues produce values
//! that can be assigned to places.

use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;

use super::types::{Local, Place};

/// Borrow kind for references.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BorrowKind {
    /// A shared/immutable borrow: `&T`.
    Shared,
    /// A mutable borrow: `&mut T`.
    Mut,
}

/// A constant value in MIR.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    /// An integer literal.
    Int(i128),
    /// A floating-point literal.
    Float(f64),
    /// A boolean literal.
    Bool(bool),
    /// A character literal.
    Char(char),
    /// A string literal.
    String(String),
    /// The unit value `()`.
    Unit,
    /// A reference to a function definition.
    FnDef(DefId),
    /// A typed zero value (for initialization).
    Zeroed(TypeId),
}

/// An operand in MIR - either a copy, move, or constant.
#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    /// Copy the value from a place (for Copy types).
    Copy(Place),
    /// Move the value from a place (transfers ownership).
    Move(Place),
    /// A constant value.
    Constant(Constant),
}

impl Operand {
    /// Create a copy operand from a local.
    pub fn copy_local(local: Local) -> Self {
        Operand::Copy(Place::from_local(local))
    }

    /// Create a move operand from a local.
    pub fn move_local(local: Local) -> Self {
        Operand::Move(Place::from_local(local))
    }

    /// Create an integer constant operand.
    pub fn const_int(value: i128) -> Self {
        Operand::Constant(Constant::Int(value))
    }

    /// Create a boolean constant operand.
    pub fn const_bool(value: bool) -> Self {
        Operand::Constant(Constant::Bool(value))
    }
}

/// Binary operators for MIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Unary operators for MIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    /// Logical/bitwise NOT
    Not,
    /// Arithmetic negation
    Neg,
}

/// Cast kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CastKind {
    /// Numeric cast (int/float conversions, widening, narrowing).
    IntToInt,
    IntToFloat,
    FloatToInt,
    FloatToFloat,
    /// Pointer casts.
    PtrToPtr,
    /// Unsizing (array to slice, concrete to dyn).
    Unsize,
}

/// Aggregate kinds for constructing compound values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AggregateKind {
    /// A tuple.
    Tuple,
    /// An array.
    Array,
    /// An ADT (struct/enum).
    Adt(DefId),
}

/// An rvalue is the right-hand side of an assignment.
///
/// Rvalues produce values but don't denote memory locations.
#[derive(Clone, Debug, PartialEq)]
pub enum Rvalue {
    /// Use an operand directly.
    Use(Operand),
    /// Create a reference to a place.
    Ref(BorrowKind, Place),
    /// Get the address of a place (raw pointer).
    AddressOf(bool, Place), // bool = mutability
    /// Binary operation.
    BinaryOp(BinOp, Operand, Operand),
    /// Unary operation.
    UnaryOp(UnOp, Operand),
    /// Cast operation.
    Cast(CastKind, Operand, TypeId),
    /// Get the length of an array/slice.
    Len(Place),
    /// Create an aggregate value (tuple, struct, array).
    Aggregate(AggregateKind, Vec<Operand>),
    /// Discriminant read (for enums).
    Discriminant(Place),
}

impl Rvalue {
    /// Create an rvalue from a simple operand.
    pub fn use_operand(operand: Operand) -> Self {
        Rvalue::Use(operand)
    }

    /// Create a shared reference.
    pub fn ref_shared(place: Place) -> Self {
        Rvalue::Ref(BorrowKind::Shared, place)
    }

    /// Create a mutable reference.
    pub fn ref_mut(place: Place) -> Self {
        Rvalue::Ref(BorrowKind::Mut, place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operand_copy_from_place() {
        let place = Place::from_local(Local(1));
        let op = Operand::Copy(place.clone());

        match op {
            Operand::Copy(p) => assert_eq!(p, place),
            _ => panic!("expected Copy"),
        }
    }

    #[test]
    fn operand_move_from_place() {
        let place = Place::from_local(Local(2));
        let op = Operand::Move(place.clone());

        match op {
            Operand::Move(p) => assert_eq!(p, place),
            _ => panic!("expected Move"),
        }
    }

    #[test]
    fn operand_constant_int() {
        let op = Operand::Constant(Constant::Int(42));

        match op {
            Operand::Constant(Constant::Int(v)) => assert_eq!(v, 42),
            _ => panic!("expected Constant::Int"),
        }
    }

    #[test]
    fn operand_constant_bool() {
        let op = Operand::const_bool(true);

        match op {
            Operand::Constant(Constant::Bool(v)) => assert!(v),
            _ => panic!("expected Constant::Bool"),
        }
    }

    #[test]
    fn operand_helper_copy_local() {
        let local = Local(5);
        let op = Operand::copy_local(local);

        match op {
            Operand::Copy(place) => {
                assert_eq!(place.local, local);
                assert!(place.is_local());
            }
            _ => panic!("expected Copy"),
        }
    }

    #[test]
    fn operand_helper_move_local() {
        let local = Local(5);
        let op = Operand::move_local(local);

        match op {
            Operand::Move(place) => {
                assert_eq!(place.local, local);
                assert!(place.is_local());
            }
            _ => panic!("expected Move"),
        }
    }

    #[test]
    fn operand_helper_const_int() {
        let op = Operand::const_int(-100);

        match op {
            Operand::Constant(Constant::Int(v)) => assert_eq!(v, -100),
            _ => panic!("expected Constant::Int"),
        }
    }

    #[test]
    fn rvalue_binary_op() {
        let lhs = Operand::const_int(10);
        let rhs = Operand::const_int(20);
        let rv = Rvalue::BinaryOp(BinOp::Add, lhs.clone(), rhs.clone());

        match rv {
            Rvalue::BinaryOp(BinOp::Add, l, r) => {
                assert_eq!(l, lhs);
                assert_eq!(r, rhs);
            }
            _ => panic!("expected BinaryOp::Add"),
        }
    }

    #[test]
    fn rvalue_ref_shared() {
        let place = Place::from_local(Local(1));
        let rv = Rvalue::Ref(BorrowKind::Shared, place.clone());

        match rv {
            Rvalue::Ref(BorrowKind::Shared, p) => assert_eq!(p, place),
            _ => panic!("expected Ref(Shared, _)"),
        }
    }

    #[test]
    fn rvalue_ref_mutable() {
        let place = Place::from_local(Local(1));
        let rv = Rvalue::Ref(BorrowKind::Mut, place.clone());

        match rv {
            Rvalue::Ref(BorrowKind::Mut, p) => assert_eq!(p, place),
            _ => panic!("expected Ref(Mut, _)"),
        }
    }

    #[test]
    fn rvalue_helper_ref_shared() {
        let place = Place::from_local(Local(3));
        let rv = Rvalue::ref_shared(place.clone());

        match rv {
            Rvalue::Ref(BorrowKind::Shared, p) => assert_eq!(p, place),
            _ => panic!("expected Ref(Shared, _)"),
        }
    }

    #[test]
    fn rvalue_helper_ref_mut() {
        let place = Place::from_local(Local(3));
        let rv = Rvalue::ref_mut(place.clone());

        match rv {
            Rvalue::Ref(BorrowKind::Mut, p) => assert_eq!(p, place),
            _ => panic!("expected Ref(Mut, _)"),
        }
    }

    #[test]
    fn rvalue_unary_op() {
        let operand = Operand::const_int(5);
        let rv = Rvalue::UnaryOp(UnOp::Neg, operand.clone());

        match rv {
            Rvalue::UnaryOp(UnOp::Neg, op) => assert_eq!(op, operand),
            _ => panic!("expected UnaryOp::Neg"),
        }
    }

    #[test]
    fn rvalue_use_operand() {
        let operand = Operand::copy_local(Local(1));
        let rv = Rvalue::use_operand(operand.clone());

        match rv {
            Rvalue::Use(op) => assert_eq!(op, operand),
            _ => panic!("expected Use"),
        }
    }

    #[test]
    fn constant_fn_def() {
        let def_id = DefId(42);
        let constant = Constant::FnDef(def_id);

        match constant {
            Constant::FnDef(id) => assert_eq!(id, def_id),
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn constant_unit() {
        let constant = Constant::Unit;
        assert_eq!(constant, Constant::Unit);
    }

    #[test]
    fn constant_string() {
        let s = "hello".to_string();
        let constant = Constant::String(s.clone());

        match constant {
            Constant::String(v) => assert_eq!(v, s),
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn constant_char() {
        let constant = Constant::Char('x');

        match constant {
            Constant::Char(c) => assert_eq!(c, 'x'),
            _ => panic!("expected Char"),
        }
    }

    #[test]
    fn constant_float() {
        let constant = Constant::Float(2.5);

        match constant {
            Constant::Float(f) => assert!((f - 2.5).abs() < f64::EPSILON),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn borrow_kind_equality() {
        assert_eq!(BorrowKind::Shared, BorrowKind::Shared);
        assert_eq!(BorrowKind::Mut, BorrowKind::Mut);
        assert_ne!(BorrowKind::Shared, BorrowKind::Mut);
    }

    #[test]
    fn binop_equality() {
        assert_eq!(BinOp::Add, BinOp::Add);
        assert_ne!(BinOp::Add, BinOp::Sub);
        assert_eq!(BinOp::Eq, BinOp::Eq);
        assert_ne!(BinOp::Eq, BinOp::Ne);
    }

    #[test]
    fn unop_equality() {
        assert_eq!(UnOp::Not, UnOp::Not);
        assert_eq!(UnOp::Neg, UnOp::Neg);
        assert_ne!(UnOp::Not, UnOp::Neg);
    }

    #[test]
    fn cast_kind_equality() {
        assert_eq!(CastKind::IntToInt, CastKind::IntToInt);
        assert_ne!(CastKind::IntToInt, CastKind::IntToFloat);
    }

    #[test]
    fn rvalue_cast() {
        let operand = Operand::const_int(42);
        let target_ty = TypeId(5);
        let rv = Rvalue::Cast(CastKind::IntToFloat, operand.clone(), target_ty);

        match rv {
            Rvalue::Cast(CastKind::IntToFloat, op, ty) => {
                assert_eq!(op, operand);
                assert_eq!(ty, target_ty);
            }
            _ => panic!("expected Cast"),
        }
    }

    #[test]
    fn rvalue_len() {
        let place = Place::from_local(Local(1));
        let rv = Rvalue::Len(place.clone());

        match rv {
            Rvalue::Len(p) => assert_eq!(p, place),
            _ => panic!("expected Len"),
        }
    }

    #[test]
    fn rvalue_discriminant() {
        let place = Place::from_local(Local(1));
        let rv = Rvalue::Discriminant(place.clone());

        match rv {
            Rvalue::Discriminant(p) => assert_eq!(p, place),
            _ => panic!("expected Discriminant"),
        }
    }

    #[test]
    fn rvalue_address_of() {
        let place = Place::from_local(Local(1));
        let rv = Rvalue::AddressOf(true, place.clone());

        match rv {
            Rvalue::AddressOf(mutable, p) => {
                assert!(mutable);
                assert_eq!(p, place);
            }
            _ => panic!("expected AddressOf"),
        }
    }

    #[test]
    fn aggregate_kind_tuple() {
        let kind = AggregateKind::Tuple;
        assert_eq!(kind, AggregateKind::Tuple);
    }

    #[test]
    fn aggregate_kind_struct() {
        let def_id = DefId(10);
        let kind = AggregateKind::Adt(def_id);

        match kind {
            AggregateKind::Adt(id) => assert_eq!(id, def_id),
            _ => panic!("expected Adt"),
        }
    }

    #[test]
    fn aggregate_kind_array() {
        let kind = AggregateKind::Array;
        assert_eq!(kind, AggregateKind::Array);
    }

    #[test]
    fn rvalue_aggregate_tuple() {
        let ops = vec![Operand::const_int(1), Operand::const_bool(true)];
        let rv = Rvalue::Aggregate(AggregateKind::Tuple, ops.clone());

        match rv {
            Rvalue::Aggregate(AggregateKind::Tuple, operands) => {
                assert_eq!(operands.len(), 2);
            }
            _ => panic!("expected Aggregate(Tuple, _)"),
        }
    }

    #[test]
    fn rvalue_aggregate_struct() {
        let def_id = DefId(5);
        let ops = vec![Operand::const_int(42)];
        let rv = Rvalue::Aggregate(AggregateKind::Adt(def_id), ops);

        match rv {
            Rvalue::Aggregate(AggregateKind::Adt(id), operands) => {
                assert_eq!(id, def_id);
                assert_eq!(operands.len(), 1);
            }
            _ => panic!("expected Aggregate(Adt, _)"),
        }
    }

    #[test]
    fn constant_zeroed() {
        let ty = TypeId(3);
        let constant = Constant::Zeroed(ty);

        match constant {
            Constant::Zeroed(t) => assert_eq!(t, ty),
            _ => panic!("expected Zeroed"),
        }
    }
}
