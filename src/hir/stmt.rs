//! HIR statement representation.
//!
//! This module defines the HIR statement types which are arena-allocated.

use crate::lexer::Span;
use crate::sema::types::TypeId;
use la_arena::Idx;

use super::{ExprId, PatId};

/// A stable identifier for statements in the HIR arena.
pub type StmtId = Idx<HirStmt>;

/// HIR statement.
#[derive(Debug, Clone)]
pub struct HirStmt {
    pub kind: HirStmtKind,
    pub span: Span,
}

/// HIR statement kinds.
#[derive(Debug, Clone)]
pub enum HirStmtKind {
    /// Let binding: `let pat: ty = init;`
    Let {
        pat: PatId,
        ty: Option<TypeId>,
        init: Option<ExprId>,
    },

    /// Expression statement (with or without trailing semicolon).
    Expr {
        expr: ExprId,
        /// Whether the statement has a trailing semicolon.
        /// If false, the expression's value is the block's value.
        has_semi: bool,
    },
}
