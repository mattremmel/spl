//! Type syntax AST nodes.

use crate::ast::{Path, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

ast_node!(RefType);
ast_node!(ArrayType);
ast_node!(SliceType);
ast_node!(TupleType);
ast_node!(FnPtrType);
ast_node!(PathType);
ast_node!(NeverType);

/// Type enum - all type syntax variants.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Ref(RefType),
    Array(ArrayType),
    Slice(SliceType),
    Tuple(TupleType),
    FnPtr(FnPtrType),
    Path(PathType),
    Never(NeverType),
}

impl AstNode for Type {
    type Language = crate::syntax::Lang;

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::RefType
                | SyntaxKind::ArrayType
                | SyntaxKind::SliceType
                | SyntaxKind::TupleType
                | SyntaxKind::FnPtrType
                | SyntaxKind::PathType
                | SyntaxKind::NeverType
        )
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::RefType => Some(Type::Ref(RefType(node))),
            SyntaxKind::ArrayType => Some(Type::Array(ArrayType(node))),
            SyntaxKind::SliceType => Some(Type::Slice(SliceType(node))),
            SyntaxKind::TupleType => Some(Type::Tuple(TupleType(node))),
            SyntaxKind::FnPtrType => Some(Type::FnPtr(FnPtrType(node))),
            SyntaxKind::PathType => Some(Type::Path(PathType(node))),
            SyntaxKind::NeverType => Some(Type::Never(NeverType(node))),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Type::Ref(it) => it.syntax(),
            Type::Array(it) => it.syntax(),
            Type::Slice(it) => it.syntax(),
            Type::Tuple(it) => it.syntax(),
            Type::FnPtr(it) => it.syntax(),
            Type::Path(it) => it.syntax(),
            Type::Never(it) => it.syntax(),
        }
    }
}

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
