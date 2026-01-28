//! HIR expression representation.
//!
//! HIR (High-level IR) expressions are the typed, desugared form of AST
//! expressions. They serve as the bridge between syntactic analysis and
//! code generation.
//!
//! # Arena Allocation
//!
//! Expressions are allocated in an arena and referenced by `ExprId` indices.
//! This design provides:
//! - **Efficient storage**: No per-node allocation overhead
//! - **Cache-friendly**: Expressions stored contiguously in memory
//! - **Simple references**: `ExprId` is a `u32`, cheap to copy and compare
//!
//! Child expressions are referenced by `ExprId`, creating a flat structure
//! rather than nested `Box<HirExpr>` pointers.
//!
//! # Desugaring
//!
//! Some AST constructs are desugared during HIR lowering:
//! - `while cond { body }` → `loop { if !cond { break; } body }`
//! - Compound assignments may be expanded
//!
//! This simplifies later phases by reducing the number of expression kinds.
//!
//! # Type Attachment
//!
//! Every `HirExpr` carries its inferred `TypeId`. This means downstream
//! phases (MIR lowering, codegen) can query types directly without
//! re-running inference or maintaining separate type maps.
//!
//! # Error Recovery
//!
//! The `HirExprKind::Missing` variant represents expressions that couldn't
//! be lowered (due to parse/resolution/type errors). This allows the HIR
//! to remain well-formed even when parts of the source have errors.

use spl_lexer::Span;
use spl_sema::{DefId, TypeId};
use la_arena::Idx;

use crate::StmtId;
use crate::pat::PatId;

/// A stable identifier for expressions in the HIR arena.
pub type ExprId = Idx<HirExpr>;

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    // Comparison
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    // Logical
    And,
    Or,
    // Assignment
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    RemAssign,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Logical negation `!`
    Not,
    /// Arithmetic negation `-`
    Neg,
    /// Dereference `*`
    Deref,
}

/// HIR expression with type attached.
#[derive(Debug, Clone)]
pub struct HirExpr {
    pub kind: HirExprKind,
    pub ty: TypeId,
    pub span: Span,
}

/// Literal values in HIR.
#[derive(Debug, Clone)]
pub enum Literal {
    Int(i128),
    Float(f64),
    Bool(bool),
    Char(char),
    String(String),
}

/// HIR expression kinds.
#[derive(Debug, Clone)]
pub enum HirExprKind {
    /// A literal value.
    Literal(Literal),

    /// A variable reference (resolved to `DefId`).
    Var(DefId),

    /// A binary operation.
    Binary { op: BinOp, lhs: ExprId, rhs: ExprId },

    /// A unary operation.
    Unary { op: UnaryOp, operand: ExprId },

    /// Reference expression `&expr` or `&mut expr`.
    Ref { mutable: bool, operand: ExprId },

    /// If expression with optional else branch.
    If {
        condition: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
    },

    /// Loop expression (while is desugared to this).
    /// `loop { body }` where body may contain `if !cond { break; }` for while loops.
    Loop { body: ExprId },

    /// Break with optional value.
    Break { value: Option<ExprId> },

    /// Continue.
    Continue,

    /// Return with optional value.
    Return { value: Option<ExprId> },

    /// Yield with optional value (exits block expression).
    Yield { value: Option<ExprId> },

    /// Block expression containing statements and optional tail expression.
    Block {
        stmts: Vec<StmtId>,
        tail: Option<ExprId>,
    },

    /// Function call.
    Call { callee: ExprId, args: Vec<ExprId> },

    /// Method call.
    MethodCall {
        receiver: ExprId,
        method: String,
        args: Vec<ExprId>,
    },

    /// Field access.
    Field { base: ExprId, field: String },

    /// Tuple field access (e.g., `t.0`).
    TupleField { base: ExprId, index: u32 },

    /// Array indexing.
    Index { base: ExprId, index: ExprId },

    /// Struct expression.
    Struct {
        def_id: DefId,
        fields: Vec<(String, ExprId)>,
    },

    /// Array literal.
    Array { elements: Vec<ExprId> },

    /// Array repeat: `[expr; count]`.
    ArrayRepeat { value: ExprId, count: u64 },

    /// Tuple expression.
    Tuple { elements: Vec<ExprId> },

    /// Cast expression: `expr as Type`.
    Cast { expr: ExprId, target_ty: TypeId },

    /// Is expression: `expr is pattern` or `expr is not pattern`.
    Is {
        scrutinee: ExprId,
        pattern: PatId,
        negated: bool,
    },

    /// Match expression: `match expr { pattern => body, ... }`.
    Match {
        scrutinee: ExprId,
        /// Arms as (pattern, optional guard, body).
        arms: Vec<(PatId, Option<ExprId>, ExprId)>,
    },

    /// Missing expression (for error recovery).
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // BinOp Tests
    // =========================================================================

    #[test]
    fn binop_arithmetic_variants() {
        let ops = [BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Div, BinOp::Rem];
        // All should be distinct
        for (i, a) in ops.iter().enumerate() {
            for (j, b) in ops.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn binop_comparison_variants() {
        let ops = [
            BinOp::Eq,
            BinOp::Ne,
            BinOp::Lt,
            BinOp::Gt,
            BinOp::Le,
            BinOp::Ge,
        ];
        for (i, a) in ops.iter().enumerate() {
            for (j, b) in ops.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn binop_logical_variants() {
        assert_ne!(BinOp::And, BinOp::Or);
    }

    #[test]
    fn binop_assignment_variants() {
        let ops = [
            BinOp::Assign,
            BinOp::AddAssign,
            BinOp::SubAssign,
            BinOp::MulAssign,
            BinOp::DivAssign,
            BinOp::RemAssign,
        ];
        for (i, a) in ops.iter().enumerate() {
            for (j, b) in ops.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn binop_is_copy() {
        let op = BinOp::Add;
        let op2 = op; // Copy
        assert_eq!(op, op2);
    }

    // =========================================================================
    // UnaryOp Tests
    // =========================================================================

    #[test]
    fn unaryop_variants_distinct() {
        let ops = [UnaryOp::Not, UnaryOp::Neg, UnaryOp::Deref];
        for (i, a) in ops.iter().enumerate() {
            for (j, b) in ops.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn unaryop_is_copy() {
        let op = UnaryOp::Not;
        let op2 = op; // Copy
        assert_eq!(op, op2);
    }

    // =========================================================================
    // Literal Tests
    // =========================================================================

    #[test]
    fn literal_int() {
        let lit = Literal::Int(42);
        match lit {
            Literal::Int(v) => assert_eq!(v, 42),
            _ => panic!("expected Int"),
        }
    }

    #[test]
    fn literal_float() {
        let lit = Literal::Float(2.5);
        match lit {
            Literal::Float(v) => assert!((v - 2.5).abs() < f64::EPSILON),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn literal_bool() {
        assert!(matches!(Literal::Bool(true), Literal::Bool(true)));
        assert!(matches!(Literal::Bool(false), Literal::Bool(false)));
    }

    #[test]
    fn literal_char() {
        let lit = Literal::Char('x');
        match lit {
            Literal::Char(c) => assert_eq!(c, 'x'),
            _ => panic!("expected Char"),
        }
    }

    #[test]
    fn literal_string() {
        let lit = Literal::String("hello".to_string());
        match lit {
            Literal::String(s) => assert_eq!(s, "hello"),
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn literal_clone() {
        let lit = Literal::String("test".to_string());
        let lit2 = lit.clone();
        match (lit, lit2) {
            (Literal::String(a), Literal::String(b)) => assert_eq!(a, b),
            _ => panic!("expected strings"),
        }
    }

    // =========================================================================
    // HirExprKind Tests
    // =========================================================================

    #[test]
    fn hir_expr_kind_literal() {
        let kind = HirExprKind::Literal(Literal::Int(100));
        assert!(matches!(kind, HirExprKind::Literal(Literal::Int(100))));
    }

    #[test]
    fn hir_expr_kind_binary() {
        let kind = HirExprKind::Binary {
            op: BinOp::Add,
            lhs: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
            rhs: ExprId::from_raw(la_arena::RawIdx::from_u32(1)),
        };
        match kind {
            HirExprKind::Binary { op, .. } => assert_eq!(op, BinOp::Add),
            _ => panic!("expected Binary"),
        }
    }

    #[test]
    fn hir_expr_kind_unary() {
        let kind = HirExprKind::Unary {
            op: UnaryOp::Neg,
            operand: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
        };
        match kind {
            HirExprKind::Unary { op, .. } => assert_eq!(op, UnaryOp::Neg),
            _ => panic!("expected Unary"),
        }
    }

    #[test]
    fn hir_expr_kind_if() {
        let kind = HirExprKind::If {
            condition: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
            then_branch: ExprId::from_raw(la_arena::RawIdx::from_u32(1)),
            else_branch: Some(ExprId::from_raw(la_arena::RawIdx::from_u32(2))),
        };
        match kind {
            HirExprKind::If { else_branch, .. } => assert!(else_branch.is_some()),
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn hir_expr_kind_if_no_else() {
        let kind = HirExprKind::If {
            condition: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
            then_branch: ExprId::from_raw(la_arena::RawIdx::from_u32(1)),
            else_branch: None,
        };
        match kind {
            HirExprKind::If { else_branch, .. } => assert!(else_branch.is_none()),
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn hir_expr_kind_block() {
        let kind = HirExprKind::Block {
            stmts: vec![],
            tail: None,
        };
        match kind {
            HirExprKind::Block { stmts, tail } => {
                assert!(stmts.is_empty());
                assert!(tail.is_none());
            }
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn hir_expr_kind_call() {
        let kind = HirExprKind::Call {
            callee: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
            args: vec![
                ExprId::from_raw(la_arena::RawIdx::from_u32(1)),
                ExprId::from_raw(la_arena::RawIdx::from_u32(2)),
            ],
        };
        match kind {
            HirExprKind::Call { args, .. } => assert_eq!(args.len(), 2),
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn hir_expr_kind_struct() {
        let kind = HirExprKind::Struct {
            def_id: DefId::new(0),
            fields: vec![
                (
                    "x".to_string(),
                    ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
                ),
                (
                    "y".to_string(),
                    ExprId::from_raw(la_arena::RawIdx::from_u32(1)),
                ),
            ],
        };
        match kind {
            HirExprKind::Struct { fields, .. } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "x");
                assert_eq!(fields[1].0, "y");
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn hir_expr_kind_array() {
        let kind = HirExprKind::Array {
            elements: vec![
                ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
                ExprId::from_raw(la_arena::RawIdx::from_u32(1)),
                ExprId::from_raw(la_arena::RawIdx::from_u32(2)),
            ],
        };
        match kind {
            HirExprKind::Array { elements } => assert_eq!(elements.len(), 3),
            _ => panic!("expected Array"),
        }
    }

    #[test]
    fn hir_expr_kind_array_repeat() {
        let kind = HirExprKind::ArrayRepeat {
            value: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
            count: 10,
        };
        match kind {
            HirExprKind::ArrayRepeat { count, .. } => assert_eq!(count, 10),
            _ => panic!("expected ArrayRepeat"),
        }
    }

    #[test]
    fn hir_expr_kind_tuple() {
        let kind = HirExprKind::Tuple {
            elements: vec![
                ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
                ExprId::from_raw(la_arena::RawIdx::from_u32(1)),
            ],
        };
        match kind {
            HirExprKind::Tuple { elements } => assert_eq!(elements.len(), 2),
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn hir_expr_kind_missing() {
        let kind = HirExprKind::Missing;
        assert!(matches!(kind, HirExprKind::Missing));
    }

    #[test]
    fn hir_expr_kind_clone() {
        let kind = HirExprKind::Literal(Literal::Int(42));
        let kind2 = kind.clone();
        match (kind, kind2) {
            (HirExprKind::Literal(Literal::Int(a)), HirExprKind::Literal(Literal::Int(b))) => {
                assert_eq!(a, b);
            }
            _ => panic!("expected matching literals"),
        }
    }
}
