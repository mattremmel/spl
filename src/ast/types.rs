//! Type syntax AST nodes.

use crate::ast::{Path, ast_enum, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

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

    pub fn len(&self) -> Option<crate::ast::Expr> {
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
