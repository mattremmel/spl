//! Type syntax AST nodes.

use crate::{Path, ast_enum, ast_node, child, children, token};
use spl_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

ast_node!(RefType);
ast_node!(ArrayType);
ast_node!(SliceType);
ast_node!(TupleType);
ast_node!(FnPtrType);
ast_node!(PathType);
ast_node!(NeverType);

ast_enum!(
    /// Type enum - all type syntax variants.
    Type {
        Ref(RefType),
        Array(ArrayType),
        Slice(SliceType),
        Tuple(TupleType),
        FnPtr(FnPtrType),
        Path(PathType),
        Never(NeverType),
    }
);

// === Typed accessors ===

impl RefType {
    pub fn amp(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::AMP)
    }

    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.0)
    }
}

impl ArrayType {
    pub fn elem_ty(&self) -> Option<Type> {
        child(&self.0)
    }

    pub fn len(&self) -> Option<crate::Expr> {
        child(&self.0)
    }
}

impl SliceType {
    pub fn elem_ty(&self) -> Option<Type> {
        child(&self.0)
    }
}

impl TupleType {
    pub fn types(&self) -> impl Iterator<Item = Type> {
        children(&self.0)
    }
}

impl FnPtrType {
    pub fn param_types(&self) -> impl Iterator<Item = Type> {
        // Parameters are listed before the return type
        children(&self.0)
    }

    pub fn ret_type(&self) -> Option<Type> {
        // Return type is typically the last Type child after the arrow
        children::<Type>(&self.0).last()
    }
}

impl PathType {
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spl_parser::parse;
    use rowan::ast::AstNode;

    /// Helper to parse source and find first type of a specific kind.
    fn parse_type<T: AstNode<Language = spl_syntax::Lang>>(source: &str) -> T {
        let parsed = parse(source);
        assert!(
            parsed.errors().is_empty(),
            "parse errors: {:?}",
            parsed.errors()
        );
        parsed
            .syntax()
            .descendants()
            .find_map(T::cast)
            .expect("expected type not found")
    }

    // =========================================================================
    // RefType Tests
    // =========================================================================

    #[test]
    fn ref_type_immutable() {
        let ty: RefType = parse_type("fn foo(x: &i32) {}");
        assert!(ty.amp().is_some());
        assert!(ty.mut_kw().is_none());
        assert!(ty.ty().is_some());
    }

    #[test]
    fn ref_type_mutable() {
        let ty: RefType = parse_type("fn foo(x: &mut i32) {}");
        assert!(ty.amp().is_some());
        assert!(ty.mut_kw().is_some());
    }

    // =========================================================================
    // ArrayType Tests
    // =========================================================================

    #[test]
    fn array_type() {
        let ty: ArrayType = parse_type("fn foo(x: [i32; 10]) {}");
        assert!(ty.elem_ty().is_some());
        assert!(ty.len().is_some());
    }

    // =========================================================================
    // SliceType Tests
    // =========================================================================

    #[test]
    fn slice_type() {
        let ty: SliceType = parse_type("fn foo(x: [i32]) {}");
        assert!(ty.elem_ty().is_some());
    }

    // =========================================================================
    // TupleType Tests
    // =========================================================================

    #[test]
    fn tuple_type_empty() {
        // SPL uses `:` for return type, and unit is just `()` param type
        let ty: TupleType = parse_type("fn foo(x: ()) {}");
        assert_eq!(ty.types().count(), 0);
    }

    #[test]
    fn tuple_type_multiple() {
        let ty: TupleType = parse_type("fn foo(x: (i32, bool, char)) {}");
        assert_eq!(ty.types().count(), 3);
    }

    // =========================================================================
    // FnPtrType Tests
    // =========================================================================

    #[test]
    fn fn_ptr_type_no_params() {
        let ty: FnPtrType = parse_type("fn foo(f: fn() -> i32) {}");
        // param_types returns all types, last one is return type
        let types: Vec<_> = ty.param_types().collect();
        assert!(!types.is_empty()); // At least the return type
        assert!(ty.ret_type().is_some());
    }

    #[test]
    fn fn_ptr_type_with_params() {
        let ty: FnPtrType = parse_type("fn foo(f: fn(i32, bool) -> char) {}");
        assert!(ty.ret_type().is_some());
    }

    // =========================================================================
    // PathType Tests
    // =========================================================================

    #[test]
    fn path_type_simple() {
        let ty: PathType = parse_type("fn foo(x: MyType) {}");
        assert!(ty.path().is_some());
    }

    #[test]
    fn path_type_qualified() {
        // SPL uses `.` for module paths, not `::`
        let ty: PathType = parse_type("fn foo(x: std.io.Result) {}");
        let path = ty.path().expect("expected path");
        assert_eq!(path.segments().count(), 3);
    }

    // =========================================================================
    // NeverType Tests
    // =========================================================================

    #[test]
    fn never_type() {
        // SPL uses `: !` for return type, not `-> !`
        let ty: NeverType = parse_type("fn foo(): ! { loop {} }");
        assert!(ty.syntax().kind() == spl_syntax::SyntaxKind::NeverType);
    }

    // =========================================================================
    // Type Enum Tests
    // =========================================================================

    #[test]
    fn type_enum_ref_variant() {
        let parsed = parse("fn foo(x: &i32) {}");
        let ty = parsed
            .syntax()
            .descendants()
            .find_map(Type::cast)
            .expect("expected Type");
        assert!(matches!(ty, Type::Ref(_)));
    }

    #[test]
    fn type_enum_path_variant() {
        let parsed = parse("fn foo(x: i32) {}");
        let ty = parsed
            .syntax()
            .descendants()
            .find_map(Type::cast)
            .expect("expected Type");
        assert!(matches!(ty, Type::Path(_)));
    }
}
