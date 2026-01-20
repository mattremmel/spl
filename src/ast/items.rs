//! Item AST nodes: functions, structs, impls, type aliases.

use crate::ast::{Block, Path, Type, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

ast_node!(SourceFile);
ast_node!(FunctionDef);
ast_node!(StructDef);
ast_node!(ImplBlock);
ast_node!(TypeAlias);
ast_node!(ParamList);
ast_node!(Param);
ast_node!(SelfParam);
ast_node!(GenericParams);
ast_node!(GenericParam);
ast_node!(GenericArgs);
ast_node!(FieldList);
ast_node!(FieldDef);
ast_node!(Name);
ast_node!(NameRef);
ast_node!(Visibility);

// === Typed accessors ===

impl SourceFile {
    pub fn items(&self) -> impl Iterator<Item = Item> {
        children(&self.0)
    }
}

/// Top-level item (function, struct, impl, type alias).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    Function(FunctionDef),
    Struct(StructDef),
    Impl(ImplBlock),
    TypeAlias(TypeAlias),
}

impl AstNode for Item {
    type Language = crate::syntax::Lang;

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::FunctionDef
                | SyntaxKind::StructDef
                | SyntaxKind::ImplBlock
                | SyntaxKind::TypeAlias
        )
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::FunctionDef => Some(Item::Function(FunctionDef(node))),
            SyntaxKind::StructDef => Some(Item::Struct(StructDef(node))),
            SyntaxKind::ImplBlock => Some(Item::Impl(ImplBlock(node))),
            SyntaxKind::TypeAlias => Some(Item::TypeAlias(TypeAlias(node))),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Item::Function(it) => it.syntax(),
            Item::Struct(it) => it.syntax(),
            Item::Impl(it) => it.syntax(),
            Item::TypeAlias(it) => it.syntax(),
        }
    }
}

impl FunctionDef {
    pub fn visibility(&self) -> Option<Visibility> {
        child(&self.0)
    }

    pub fn fn_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::FN_KW)
    }

    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }

    pub fn generic_params(&self) -> Option<GenericParams> {
        child(&self.0)
    }

    pub fn param_list(&self) -> Option<ParamList> {
        child(&self.0)
    }

    pub fn ret_type(&self) -> Option<Type> {
        child(&self.0)
    }

    pub fn body(&self) -> Option<Block> {
        child(&self.0)
    }
}

impl StructDef {
    pub fn visibility(&self) -> Option<Visibility> {
        child(&self.0)
    }

    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }

    pub fn generic_params(&self) -> Option<GenericParams> {
        child(&self.0)
    }

    pub fn field_list(&self) -> Option<FieldList> {
        child(&self.0)
    }
}

impl ImplBlock {
    pub fn generic_params(&self) -> Option<GenericParams> {
        child(&self.0)
    }

    pub fn self_ty(&self) -> Option<Type> {
        child(&self.0)
    }

    pub fn items(&self) -> impl Iterator<Item = Item> {
        children(&self.0)
    }
}

impl TypeAlias {
    pub fn visibility(&self) -> Option<Visibility> {
        child(&self.0)
    }

    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.0)
    }
}

impl FieldList {
    pub fn fields(&self) -> impl Iterator<Item = FieldDef> {
        children(&self.0)
    }
}

impl FieldDef {
    pub fn visibility(&self) -> Option<Visibility> {
        child(&self.0)
    }

    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.0)
    }
}

impl ParamList {
    pub fn self_param(&self) -> Option<SelfParam> {
        child(&self.0)
    }

    pub fn params(&self) -> impl Iterator<Item = Param> {
        children(&self.0)
    }
}

impl Param {
    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.0)
    }
}

impl SelfParam {
    pub fn amp(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::AMP)
    }

    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }
}

impl GenericParams {
    pub fn params(&self) -> impl Iterator<Item = GenericParam> {
        children(&self.0)
    }
}

impl GenericParam {
    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }
}

impl GenericArgs {
    pub fn args(&self) -> impl Iterator<Item = Type> {
        children(&self.0)
    }
}

impl Name {
    pub fn ident_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
    }
}

impl NameRef {
    pub fn ident_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
    }
}

impl Visibility {
    pub fn pub_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::PUB_KW)
    }

    pub fn crate_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::CRATE_KW)
    }

    pub fn super_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::SUPER_KW)
    }

    pub fn self_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::SELF_VALUE_KW)
    }

    pub fn in_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IN_KW)
    }

    /// Returns the path for `pub(in path)` visibility.
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }
}
