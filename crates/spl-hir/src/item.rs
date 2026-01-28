//! HIR item representation.
//!
//! This module defines the HIR item types for functions, structs, etc.

use spl_lexer::Span;
use spl_sema::{DefId, TypeId};

use crate::{ExprId, PatId};

/// HIR items (top-level definitions).
#[derive(Debug, Clone)]
pub enum HirItem {
    Function(HirFunction),
    Struct(HirStruct),
    TypeAlias(HirTypeAlias),
    Impl(HirImpl),
}

/// HIR function definition.
#[derive(Debug, Clone)]
pub struct HirFunction {
    pub def_id: DefId,
    pub name: String,
    pub type_params: Vec<DefId>,
    pub params: Vec<HirParam>,
    pub ret_type: TypeId,
    pub body: Option<ExprId>,
    pub span: Span,
}

/// HIR function parameter.
#[derive(Debug, Clone)]
pub struct HirParam {
    pub pat: PatId,
    pub ty: TypeId,
    pub span: Span,
}

/// HIR struct definition.
#[derive(Debug, Clone)]
pub struct HirStruct {
    pub def_id: DefId,
    pub name: String,
    pub type_params: Vec<DefId>,
    pub fields: Vec<HirField>,
    pub span: Span,
}

/// HIR struct field.
#[derive(Debug, Clone)]
pub struct HirField {
    pub def_id: DefId,
    pub name: String,
    pub ty: TypeId,
    pub span: Span,
}

/// HIR type alias.
#[derive(Debug, Clone)]
pub struct HirTypeAlias {
    pub def_id: DefId,
    pub name: String,
    pub type_params: Vec<DefId>,
    pub ty: TypeId,
    pub span: Span,
}

/// HIR impl block.
#[derive(Debug, Clone)]
pub struct HirImpl {
    pub type_params: Vec<DefId>,
    pub self_ty: TypeId,
    pub items: Vec<HirItem>,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use spl_sema::TypeInterner;
    use la_arena::Arena;

    // =========================================================================
    // HirFunction Tests
    // =========================================================================

    #[test]
    fn hir_function_with_body() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();
        let mut pats: Arena<crate::pat::HirPat> = Arena::new();

        let param_pat = crate::pat::HirPat {
            kind: crate::pat::HirPatKind::Bind {
                def_id: DefId::new(1),
                mutable: false,
            },
            ty: i32_ty,
            span: 0..1,
        };
        let pat_id = pats.alloc(param_pat);

        let func = HirFunction {
            def_id: DefId::new(0),
            name: "add".to_string(),
            type_params: vec![],
            params: vec![HirParam {
                pat: pat_id,
                ty: i32_ty,
                span: 0..5,
            }],
            ret_type: i32_ty,
            body: Some(crate::ExprId::from_raw(la_arena::RawIdx::from_u32(0))),
            span: 0..20,
        };

        assert_eq!(func.name, "add");
        assert_eq!(func.params.len(), 1);
        assert!(func.body.is_some());
        assert!(func.type_params.is_empty());
    }

    #[test]
    fn hir_function_no_body() {
        let types = TypeInterner::new();
        let unit_ty = types.unit();

        let func = HirFunction {
            def_id: DefId::new(0),
            name: "extern_fn".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: None,
            span: 0..10,
        };

        assert!(func.body.is_none());
    }

    #[test]
    fn hir_function_with_type_params() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let func = HirFunction {
            def_id: DefId::new(0),
            name: "generic_fn".to_string(),
            type_params: vec![DefId::new(1), DefId::new(2)],
            params: vec![],
            ret_type: i32_ty,
            body: None,
            span: 0..10,
        };

        assert_eq!(func.type_params.len(), 2);
    }

    // =========================================================================
    // HirStruct Tests
    // =========================================================================

    #[test]
    fn hir_struct_with_fields() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();
        let f64_ty = types.f64();

        let s = HirStruct {
            def_id: DefId::new(0),
            name: "Point".to_string(),
            type_params: vec![],
            fields: vec![
                HirField {
                    def_id: DefId::new(1),
                    name: "x".to_string(),
                    ty: i32_ty,
                    span: 0..5,
                },
                HirField {
                    def_id: DefId::new(2),
                    name: "y".to_string(),
                    ty: f64_ty,
                    span: 6..11,
                },
            ],
            span: 0..20,
        };

        assert_eq!(s.name, "Point");
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].name, "x");
        assert_eq!(s.fields[1].name, "y");
    }

    #[test]
    fn hir_struct_unit() {
        let s = HirStruct {
            def_id: DefId::new(0),
            name: "Unit".to_string(),
            type_params: vec![],
            fields: vec![],
            span: 0..10,
        };

        assert!(s.fields.is_empty());
    }

    #[test]
    fn hir_struct_generic() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let s = HirStruct {
            def_id: DefId::new(0),
            name: "Container".to_string(),
            type_params: vec![DefId::new(1)],
            fields: vec![HirField {
                def_id: DefId::new(2),
                name: "value".to_string(),
                ty: i32_ty,
                span: 0..5,
            }],
            span: 0..20,
        };

        assert_eq!(s.type_params.len(), 1);
    }

    // =========================================================================
    // HirTypeAlias Tests
    // =========================================================================

    #[test]
    fn hir_type_alias_simple() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let alias = HirTypeAlias {
            def_id: DefId::new(0),
            name: "MyInt".to_string(),
            type_params: vec![],
            ty: i32_ty,
            span: 0..15,
        };

        assert_eq!(alias.name, "MyInt");
        assert!(alias.type_params.is_empty());
    }

    #[test]
    fn hir_type_alias_generic() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let alias = HirTypeAlias {
            def_id: DefId::new(0),
            name: "Result".to_string(),
            type_params: vec![DefId::new(1), DefId::new(2)],
            ty: i32_ty, // Placeholder
            span: 0..20,
        };

        assert_eq!(alias.type_params.len(), 2);
    }

    // =========================================================================
    // HirImpl Tests
    // =========================================================================

    #[test]
    fn hir_impl_empty() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let impl_block = HirImpl {
            type_params: vec![],
            self_ty: i32_ty,
            items: vec![],
            span: 0..10,
        };

        assert!(impl_block.items.is_empty());
        assert!(impl_block.type_params.is_empty());
    }

    #[test]
    fn hir_impl_with_methods() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let method = HirFunction {
            def_id: DefId::new(1),
            name: "get".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: None,
            span: 5..15,
        };

        let impl_block = HirImpl {
            type_params: vec![],
            self_ty: i32_ty,
            items: vec![HirItem::Function(method)],
            span: 0..20,
        };

        assert_eq!(impl_block.items.len(), 1);
        match &impl_block.items[0] {
            HirItem::Function(f) => assert_eq!(f.name, "get"),
            _ => panic!("expected function"),
        }
    }

    // =========================================================================
    // HirItem Tests
    // =========================================================================

    #[test]
    fn hir_item_function_variant() {
        let types = TypeInterner::new();
        let unit_ty = types.unit();

        let func = HirFunction {
            def_id: DefId::new(0),
            name: "main".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: None,
            span: 0..10,
        };

        let item = HirItem::Function(func);
        assert!(matches!(item, HirItem::Function(_)));
    }

    #[test]
    fn hir_item_struct_variant() {
        let s = HirStruct {
            def_id: DefId::new(0),
            name: "S".to_string(),
            type_params: vec![],
            fields: vec![],
            span: 0..5,
        };

        let item = HirItem::Struct(s);
        assert!(matches!(item, HirItem::Struct(_)));
    }

    #[test]
    fn hir_item_clone() {
        let types = TypeInterner::new();
        let unit_ty = types.unit();

        let func = HirFunction {
            def_id: DefId::new(0),
            name: "test".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: None,
            span: 0..10,
        };

        let item = HirItem::Function(func);
        let item2 = item.clone();

        match (item, item2) {
            (HirItem::Function(a), HirItem::Function(b)) => {
                assert_eq!(a.name, b.name);
            }
            _ => panic!("expected functions"),
        }
    }

    // =========================================================================
    // HirParam Tests
    // =========================================================================

    #[test]
    fn hir_param_construction() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let param = HirParam {
            pat: crate::PatId::from_raw(la_arena::RawIdx::from_u32(0)),
            ty: i32_ty,
            span: 0..5,
        };

        assert_eq!(param.ty, i32_ty);
    }

    // =========================================================================
    // HirField Tests
    // =========================================================================

    #[test]
    fn hir_field_construction() {
        let types = TypeInterner::new();
        let bool_ty = types.bool();

        let field = HirField {
            def_id: DefId::new(0),
            name: "active".to_string(),
            ty: bool_ty,
            span: 0..10,
        };

        assert_eq!(field.name, "active");
        assert_eq!(field.ty, bool_ty);
    }
}
