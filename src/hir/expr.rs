//! HIR expression representation.
//!
//! This module defines the HIR expression types which are arena-allocated
//! and have types attached after inference.

use crate::lexer::Span;
use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;
use la_arena::Idx;

use super::StmtId;

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

    /// A variable reference (resolved to DefId).
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

    /// Missing expression (for error recovery).
    Missing,
}
