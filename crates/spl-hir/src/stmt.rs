//! HIR statement representation.
//!
//! This module defines the HIR statement types which are arena-allocated.

use spl_lexer::Span;
use spl_sema::TypeId;
use la_arena::Idx;

use crate::{ExprId, PatId};

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

#[cfg(test)]
mod tests {
    use super::*;
    use spl_sema::TypeInterner;

    // =========================================================================
    // HirStmtKind::Let Tests
    // =========================================================================

    #[test]
    fn hir_stmt_kind_let_full() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let kind = HirStmtKind::Let {
            pat: PatId::from_raw(la_arena::RawIdx::from_u32(0)),
            ty: Some(i32_ty),
            init: Some(ExprId::from_raw(la_arena::RawIdx::from_u32(0))),
        };

        match kind {
            HirStmtKind::Let { ty, init, .. } => {
                assert!(ty.is_some());
                assert!(init.is_some());
            }
            HirStmtKind::Expr { .. } => panic!("expected Let"),
        }
    }

    #[test]
    fn hir_stmt_kind_let_no_type() {
        let kind = HirStmtKind::Let {
            pat: PatId::from_raw(la_arena::RawIdx::from_u32(0)),
            ty: None,
            init: Some(ExprId::from_raw(la_arena::RawIdx::from_u32(0))),
        };

        match kind {
            HirStmtKind::Let { ty, init, .. } => {
                assert!(ty.is_none());
                assert!(init.is_some());
            }
            HirStmtKind::Expr { .. } => panic!("expected Let"),
        }
    }

    #[test]
    fn hir_stmt_kind_let_no_init() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let kind = HirStmtKind::Let {
            pat: PatId::from_raw(la_arena::RawIdx::from_u32(0)),
            ty: Some(i32_ty),
            init: None,
        };

        match kind {
            HirStmtKind::Let { ty, init, .. } => {
                assert!(ty.is_some());
                assert!(init.is_none());
            }
            HirStmtKind::Expr { .. } => panic!("expected Let"),
        }
    }

    // =========================================================================
    // HirStmtKind::Expr Tests
    // =========================================================================

    #[test]
    fn hir_stmt_kind_expr_with_semi() {
        let kind = HirStmtKind::Expr {
            expr: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
            has_semi: true,
        };

        match kind {
            HirStmtKind::Expr { has_semi, .. } => assert!(has_semi),
            HirStmtKind::Let { .. } => panic!("expected Expr"),
        }
    }

    #[test]
    fn hir_stmt_kind_expr_without_semi() {
        let kind = HirStmtKind::Expr {
            expr: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
            has_semi: false,
        };

        match kind {
            HirStmtKind::Expr { has_semi, .. } => assert!(!has_semi),
            HirStmtKind::Let { .. } => panic!("expected Expr"),
        }
    }

    // =========================================================================
    // HirStmt Tests
    // =========================================================================

    #[test]
    fn hir_stmt_construction() {
        let stmt = HirStmt {
            kind: HirStmtKind::Expr {
                expr: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
                has_semi: true,
            },
            span: 0..10,
        };

        assert_eq!(stmt.span, 0..10);
        assert!(matches!(stmt.kind, HirStmtKind::Expr { .. }));
    }

    #[test]
    fn hir_stmt_clone() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let stmt = HirStmt {
            kind: HirStmtKind::Let {
                pat: PatId::from_raw(la_arena::RawIdx::from_u32(0)),
                ty: Some(i32_ty),
                init: None,
            },
            span: 0..15,
        };

        let stmt2 = stmt.clone();

        assert_eq!(stmt.span, stmt2.span);
        match (&stmt.kind, &stmt2.kind) {
            (HirStmtKind::Let { ty: ty1, .. }, HirStmtKind::Let { ty: ty2, .. }) => {
                assert_eq!(ty1, ty2);
            }
            _ => panic!("expected matching Let statements"),
        }
    }

    #[test]
    fn hir_stmt_kind_variants_distinct() {
        let let_stmt = HirStmtKind::Let {
            pat: PatId::from_raw(la_arena::RawIdx::from_u32(0)),
            ty: None,
            init: None,
        };

        let expr_stmt = HirStmtKind::Expr {
            expr: ExprId::from_raw(la_arena::RawIdx::from_u32(0)),
            has_semi: true,
        };

        // Verify they are distinguishable
        assert!(matches!(let_stmt, HirStmtKind::Let { .. }));
        assert!(matches!(expr_stmt, HirStmtKind::Expr { .. }));
        assert!(!matches!(let_stmt, HirStmtKind::Expr { .. }));
        assert!(!matches!(expr_stmt, HirStmtKind::Let { .. }));
    }

    #[test]
    fn stmt_id_type() {
        // StmtId is just an index type from la_arena
        let id = StmtId::from_raw(la_arena::RawIdx::from_u32(42));
        assert_eq!(id.into_raw().into_u32(), 42);
    }
}
