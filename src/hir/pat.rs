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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sema::types::TypeInterner;

    // =========================================================================
    // HirPatKind Tests
    // =========================================================================

    #[test]
    fn hir_pat_kind_bind_immutable() {
        let kind = HirPatKind::Bind {
            def_id: DefId(0),
            mutable: false,
        };
        match kind {
            HirPatKind::Bind { mutable, .. } => assert!(!mutable),
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn hir_pat_kind_bind_mutable() {
        let kind = HirPatKind::Bind {
            def_id: DefId(0),
            mutable: true,
        };
        match kind {
            HirPatKind::Bind { mutable, .. } => assert!(mutable),
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn hir_pat_kind_wildcard() {
        let kind = HirPatKind::Wildcard;
        assert!(matches!(kind, HirPatKind::Wildcard));
    }

    #[test]
    fn hir_pat_kind_tuple_empty() {
        let kind = HirPatKind::Tuple { elements: vec![] };
        match kind {
            HirPatKind::Tuple { elements } => assert!(elements.is_empty()),
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn hir_pat_kind_tuple_with_elements() {
        let kind = HirPatKind::Tuple {
            elements: vec![
                PatId::from_raw(la_arena::RawIdx::from_u32(0)),
                PatId::from_raw(la_arena::RawIdx::from_u32(1)),
            ],
        };
        match kind {
            HirPatKind::Tuple { elements } => assert_eq!(elements.len(), 2),
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn hir_pat_kind_struct_without_rest() {
        let kind = HirPatKind::Struct {
            def_id: DefId(0),
            fields: vec![
                ("x".to_string(), PatId::from_raw(la_arena::RawIdx::from_u32(0))),
                ("y".to_string(), PatId::from_raw(la_arena::RawIdx::from_u32(1))),
            ],
            rest: false,
        };
        match kind {
            HirPatKind::Struct { fields, rest, .. } => {
                assert_eq!(fields.len(), 2);
                assert!(!rest);
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn hir_pat_kind_struct_with_rest() {
        let kind = HirPatKind::Struct {
            def_id: DefId(0),
            fields: vec![("x".to_string(), PatId::from_raw(la_arena::RawIdx::from_u32(0)))],
            rest: true,
        };
        match kind {
            HirPatKind::Struct { rest, .. } => assert!(rest),
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn hir_pat_kind_ref_immutable() {
        let kind = HirPatKind::Ref {
            mutable: false,
            inner: PatId::from_raw(la_arena::RawIdx::from_u32(0)),
        };
        match kind {
            HirPatKind::Ref { mutable, .. } => assert!(!mutable),
            _ => panic!("expected Ref"),
        }
    }

    #[test]
    fn hir_pat_kind_ref_mutable() {
        let kind = HirPatKind::Ref {
            mutable: true,
            inner: PatId::from_raw(la_arena::RawIdx::from_u32(0)),
        };
        match kind {
            HirPatKind::Ref { mutable, .. } => assert!(mutable),
            _ => panic!("expected Ref"),
        }
    }

    #[test]
    fn hir_pat_kind_literal_int() {
        let kind = HirPatKind::Literal(super::super::expr::Literal::Int(42));
        match kind {
            HirPatKind::Literal(super::super::expr::Literal::Int(v)) => assert_eq!(v, 42),
            _ => panic!("expected Literal Int"),
        }
    }

    #[test]
    fn hir_pat_kind_literal_bool() {
        let kind = HirPatKind::Literal(super::super::expr::Literal::Bool(true));
        match kind {
            HirPatKind::Literal(super::super::expr::Literal::Bool(v)) => assert!(v),
            _ => panic!("expected Literal Bool"),
        }
    }

    #[test]
    fn hir_pat_kind_missing() {
        let kind = HirPatKind::Missing;
        assert!(matches!(kind, HirPatKind::Missing));
    }

    // =========================================================================
    // HirPat Tests
    // =========================================================================

    #[test]
    fn hir_pat_construction() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let pat = HirPat {
            kind: HirPatKind::Wildcard,
            ty: i32_ty,
            span: 0..1,
        };

        assert!(matches!(pat.kind, HirPatKind::Wildcard));
        assert_eq!(pat.ty, i32_ty);
        assert_eq!(pat.span, 0..1);
    }

    #[test]
    fn hir_pat_clone() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let pat = HirPat {
            kind: HirPatKind::Bind {
                def_id: DefId(0),
                mutable: false,
            },
            ty: i32_ty,
            span: 0..5,
        };

        let pat2 = pat.clone();

        assert_eq!(pat.ty, pat2.ty);
        assert_eq!(pat.span, pat2.span);
        match (&pat.kind, &pat2.kind) {
            (
                HirPatKind::Bind { def_id: a, mutable: m1 },
                HirPatKind::Bind { def_id: b, mutable: m2 },
            ) => {
                assert_eq!(a, b);
                assert_eq!(m1, m2);
            }
            _ => panic!("expected matching Bind patterns"),
        }
    }

    #[test]
    fn hir_pat_kind_variants_distinct() {
        // Ensure all pattern kinds are distinguishable
        let patterns = [
            HirPatKind::Bind { def_id: DefId(0), mutable: false },
            HirPatKind::Wildcard,
            HirPatKind::Tuple { elements: vec![] },
            HirPatKind::Struct { def_id: DefId(0), fields: vec![], rest: false },
            HirPatKind::Ref { mutable: false, inner: PatId::from_raw(la_arena::RawIdx::from_u32(0)) },
            HirPatKind::Literal(super::super::expr::Literal::Int(0)),
            HirPatKind::Missing,
        ];

        // Just verify they can all be created and matched
        for pat in &patterns {
            match pat {
                HirPatKind::Bind { .. } => {}
                HirPatKind::Wildcard => {}
                HirPatKind::Tuple { .. } => {}
                HirPatKind::Struct { .. } => {}
                HirPatKind::Ref { .. } => {}
                HirPatKind::Literal(_) => {}
                HirPatKind::Missing => {}
            }
        }
    }
}
