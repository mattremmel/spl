//! High-level Intermediate Representation (HIR).
//!
//! HIR differs from AST:
//! - Names resolved to `DefId`s
//! - Types attached to all expressions
//! - Desugared constructs (while → loop, etc.)
//! - Arena-allocated with stable IDs

pub mod expr;
pub mod item;
pub mod lower;
pub mod pat;
pub mod stmt;

pub use expr::{BinOp, ExprId, HirExpr, HirExprKind, Literal, UnaryOp};
pub use item::{HirField, HirFunction, HirImpl, HirItem, HirParam, HirStruct, HirTypeAlias};
pub use pat::{HirPat, HirPatKind, PatId};
pub use stmt::{HirStmt, HirStmtKind, StmtId};

use crate::lexer::Span;
use crate::sema::symbol::DefId;
use crate::sema::types::{PrimitiveKind, TypeId, TypeInterner};
use la_arena::Arena;
use rustc_hash::FxHashMap;

/// A lowered expression for literals that need folding.
///
/// This is used to handle constant folding at compile time, including:
/// - Negated integer literals like `-128i8` and `-(128i8)`
/// - Boolean operations like `!true`
/// - Arithmetic on literals like `1 + 2`
///
/// Most expressions pass through unchanged (Passthrough variant).
#[derive(Debug, Clone)]
pub enum LoweredExpr {
    /// An integer literal with its (possibly negated) value and optional type suffix.
    IntLiteral {
        value: i128,
        suffix: Option<PrimitiveKind>,
        span: Span,
    },
    /// A float literal with its (possibly negated) value and optional type suffix.
    FloatLiteral {
        value: f64,
        suffix: Option<PrimitiveKind>,
        span: Span,
    },
    /// A boolean literal value.
    BoolLiteral { value: bool, span: Span },
    /// Not foldable - use AST directly.
    Passthrough,
}

/// The HIR database containing all arena-allocated HIR nodes.
#[derive(Debug)]
pub struct HirDatabase {
    /// Arena for expressions.
    pub exprs: Arena<HirExpr>,
    /// Arena for statements.
    pub stmts: Arena<HirStmt>,
    /// Arena for patterns.
    pub pats: Arena<HirPat>,
    /// Top-level items.
    pub items: Vec<HirItem>,
    /// Type interner for all types in the HIR.
    pub types: TypeInterner,
    /// Map from binding `DefId`s to their types.
    pub binding_types: FxHashMap<DefId, TypeId>,
    /// Map from expression IDs to their source spans (for diagnostics).
    pub expr_spans: FxHashMap<ExprId, Span>,
    /// Map from method call expression IDs to their resolved method `DefId`s.
    pub method_resolutions: FxHashMap<ExprId, DefId>,
}

impl Default for HirDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl HirDatabase {
    /// Create a new empty HIR database.
    pub fn new() -> Self {
        Self {
            exprs: Arena::new(),
            stmts: Arena::new(),
            pats: Arena::new(),
            items: Vec::new(),
            types: TypeInterner::new(),
            binding_types: FxHashMap::default(),
            expr_spans: FxHashMap::default(),
            method_resolutions: FxHashMap::default(),
        }
    }

    /// Allocate an expression in the arena and return its ID.
    pub fn alloc_expr(&mut self, expr: HirExpr) -> ExprId {
        let span = expr.span.clone();
        let id = self.exprs.alloc(expr);
        self.expr_spans.insert(id, span);
        id
    }

    /// Allocate a statement in the arena and return its ID.
    pub fn alloc_stmt(&mut self, stmt: HirStmt) -> StmtId {
        self.stmts.alloc(stmt)
    }

    /// Allocate a pattern in the arena and return its ID.
    pub fn alloc_pat(&mut self, pat: HirPat) -> PatId {
        self.pats.alloc(pat)
    }

    /// Get an expression by ID.
    pub fn expr(&self, id: ExprId) -> &HirExpr {
        &self.exprs[id]
    }

    /// Get a statement by ID.
    pub fn stmt(&self, id: StmtId) -> &HirStmt {
        &self.stmts[id]
    }

    /// Get a pattern by ID.
    pub fn pat(&self, id: PatId) -> &HirPat {
        &self.pats[id]
    }

    /// Get the span for an expression.
    pub fn span(&self, id: ExprId) -> Option<&Span> {
        self.expr_spans.get(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hir_arena_alloc_expr() {
        let mut db = HirDatabase::new();
        let ty = db.types.i32();

        let expr = HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty,
            span: 0..2,
        };

        let id = db.alloc_expr(expr);

        let retrieved = db.expr(id);
        assert_eq!(retrieved.ty, ty);
        match &retrieved.kind {
            HirExprKind::Literal(Literal::Int(v)) => assert_eq!(*v, 42),
            _ => panic!("expected int literal"),
        }
    }

    #[test]
    fn expr_id_stable_after_allocations() {
        let mut db = HirDatabase::new();
        let ty = db.types.i32();

        // Allocate first expression
        let expr1 = HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty,
            span: 0..1,
        };
        let id1 = db.alloc_expr(expr1);

        // Allocate more expressions
        for i in 2..100 {
            let expr = HirExpr {
                kind: HirExprKind::Literal(Literal::Int(i)),
                ty,
                span: 0..1,
            };
            db.alloc_expr(expr);
        }

        // Original ID should still work and return correct value
        let retrieved = db.expr(id1);
        match &retrieved.kind {
            HirExprKind::Literal(Literal::Int(v)) => assert_eq!(*v, 1),
            _ => panic!("expected int literal"),
        }
    }

    #[test]
    fn hir_arena_alloc_stmt() {
        let mut db = HirDatabase::new();
        let ty = db.types.i32();
        let error_ty = db.types.error();

        // Create a pattern
        let pat = HirPat {
            kind: HirPatKind::Wildcard,
            ty: error_ty,
            span: 0..1,
        };
        let pat_id = db.alloc_pat(pat);

        // Create an expression
        let expr = HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty,
            span: 0..2,
        };
        let expr_id = db.alloc_expr(expr);

        // Create a let statement
        let stmt = HirStmt {
            kind: HirStmtKind::Let {
                pat: pat_id,
                ty: Some(ty),
                init: Some(expr_id),
            },
            span: 0..10,
        };
        let stmt_id = db.alloc_stmt(stmt);

        let retrieved = db.stmt(stmt_id);
        match &retrieved.kind {
            HirStmtKind::Let { init, .. } => {
                assert!(init.is_some());
            }
            HirStmtKind::Expr { .. } => panic!("expected let statement"),
        }
    }
}
