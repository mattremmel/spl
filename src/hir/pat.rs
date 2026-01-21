//! HIR pattern representation.
//!
//! This module defines the HIR pattern types which are arena-allocated
//! and have bindings resolved to DefIds.

use crate::lexer::Span;
use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;
use la_arena::Idx;

/// A stable identifier for patterns in the HIR arena.
pub type PatId = Idx<HirPat>;

/// HIR pattern.
#[derive(Debug, Clone)]
pub struct HirPat {
    pub kind: HirPatKind,
    pub ty: TypeId,
    pub span: Span,
}

/// HIR pattern kinds.
#[derive(Debug, Clone)]
pub enum HirPatKind {
    /// Binding pattern: `x` or `mut x`.
    Bind { def_id: DefId, mutable: bool },

    /// Wildcard pattern: `_`.
    Wildcard,

    /// Tuple pattern: `(a, b, c)`.
    Tuple { elements: Vec<PatId> },

    /// Struct pattern: `Point { x, y }`.
    Struct {
        def_id: DefId,
        fields: Vec<(String, PatId)>,
        /// Whether `..` was present (ignoring remaining fields).
        rest: bool,
    },

    /// Reference pattern: `&pat` or `&mut pat`.
    Ref { mutable: bool, inner: PatId },

    /// Literal pattern (for match expressions).
    Literal(super::expr::Literal),

    /// Missing pattern (for error recovery).
    Missing,
}
