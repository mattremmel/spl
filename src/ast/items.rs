//! Item AST nodes: functions, structs, impls, type aliases.

use crate::ast::{Block, Path, Type, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

ast_node!(SourceFile);
ast_node!(FunctionDef);
ast_node!(StructDef);
ast_node!(ImplBlock);
ast_node!(TypeAlias);
ast_node!(ExternBlock);
ast_node!(ExternFn);
ast_node!(ParamList);
ast_node!(Param);
ast_node!(SelfParam);
ast_node!(GenericParam);
ast_node!(GenericArgs);
ast_node!(FieldList);
ast_node!(FieldDef);
ast_node!(Name);
ast_node!(NameRef);
ast_node!(Visibility);
ast_node!(WhereClause);
ast_node!(TypeBound);
ast_node!(LabelSpec);

// === Typed accessors ===

impl SourceFile {
    pub fn items(&self) -> impl Iterator<Item = Item> {
        children(&self.0)
    }
}

/// Top-level item (function, struct, impl, type alias, extern block).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    Function(FunctionDef),
    Struct(StructDef),
    Impl(ImplBlock),
    TypeAlias(TypeAlias),
    Extern(ExternBlock),
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
                | SyntaxKind::ExternBlock
        )
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::FunctionDef => Some(Item::Function(FunctionDef(node))),
            SyntaxKind::StructDef => Some(Item::Struct(StructDef(node))),
            SyntaxKind::ImplBlock => Some(Item::Impl(ImplBlock(node))),
            SyntaxKind::TypeAlias => Some(Item::TypeAlias(TypeAlias(node))),
            SyntaxKind::ExternBlock => Some(Item::Extern(ExternBlock(node))),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Item::Function(it) => it.syntax(),
            Item::Struct(it) => it.syntax(),
            Item::Impl(it) => it.syntax(),
            Item::TypeAlias(it) => it.syntax(),
            Item::Extern(it) => it.syntax(),
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

    pub fn param_list(&self) -> Option<ParamList> {
        child(&self.0)
    }

    pub fn ret_type(&self) -> Option<Type> {
        child(&self.0)
    }

    pub fn where_clause(&self) -> Option<WhereClause> {
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

    pub fn field_list(&self) -> Option<FieldList> {
        child(&self.0)
    }

    pub fn where_clause(&self) -> Option<WhereClause> {
        child(&self.0)
    }
}

impl ImplBlock {
    pub fn self_ty(&self) -> Option<Type> {
        child(&self.0)
    }

    pub fn where_clause(&self) -> Option<WhereClause> {
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

    pub fn where_clause(&self) -> Option<WhereClause> {
        child(&self.0)
    }
}

impl ExternBlock {
    pub fn extern_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::EXTERN_KW)
    }

    /// Get the ABI string (e.g., "C").
    pub fn abi(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::STRING_LITERAL)
    }

    /// Get extern function declarations.
    pub fn extern_fns(&self) -> impl Iterator<Item = ExternFn> {
        children(&self.0)
    }
}

impl ExternFn {
    pub fn visibility(&self) -> Option<Visibility> {
        child(&self.0)
    }

    pub fn fn_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::FN_KW)
    }

    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }

    pub fn param_list(&self) -> Option<ParamList> {
        child(&self.0)
    }

    pub fn ret_type(&self) -> Option<Type> {
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
    /// Get the label spec if present (`_` or external label).
    pub fn label(&self) -> Option<LabelSpec> {
        child(&self.0)
    }

    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.0)
    }

    /// Get the external label for this parameter.
    /// - Returns `None` if label spec is `_` (positional parameter)
    /// - Returns the explicit label if a label spec is provided
    /// - Returns the parameter name as default label if no label spec
    pub fn external_label(&self) -> Option<String> {
        if let Some(label_spec) = self.label() {
            label_spec.label_text()
        } else {
            // Default: use param name as external label
            self.name()?.ident_token().map(|t| t.text().to_string())
        }
    }
}

impl LabelSpec {
    /// Check if this is the underscore label (positional parameter).
    pub fn is_underscore(&self) -> bool {
        self.ident_token().map(|t| t.text() == "_").unwrap_or(false)
    }

    /// Get the identifier token.
    pub fn ident_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
    }

    /// Get the label text, or `None` if this is underscore.
    pub fn label_text(&self) -> Option<String> {
        let token = self.ident_token()?;
        let text = token.text();
        if text == "_" {
            None
        } else {
            Some(text.to_string())
        }
    }
}

impl SelfParam {
    pub fn amp(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::AMP)
    }

    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }

    pub fn self_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::SELF_VALUE_KW)
    }
}

impl GenericParam {
    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }

    /// Get type bounds (e.g., Clone, Debug in `T: Clone + Debug`).
    pub fn bounds(&self) -> impl Iterator<Item = TypeBound> {
        children(&self.0)
    }
}

impl GenericArgs {
    pub fn args(&self) -> impl Iterator<Item = Type> {
        children(&self.0)
    }
}

impl Name {
    pub fn ident_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT).or_else(|| token(&self.0, SyntaxKind::INT_LITERAL))
    }
}

impl NameRef {
    pub fn ident_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
    }

    /// Get the token for this name reference.
    /// Returns an IDENT, SELF_VALUE_KW (for `self`), or SELF_TYPE_KW (for `Self`) token.
    pub fn token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
            .or_else(|| token(&self.0, SyntaxKind::SELF_VALUE_KW))
            .or_else(|| token(&self.0, SyntaxKind::SELF_TYPE_KW))
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

impl WhereClause {
    /// Get the `where` keyword token.
    pub fn where_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::WHERE_KW)
    }

    /// Get the type parameters with their bounds.
    pub fn type_params(&self) -> impl Iterator<Item = GenericParam> {
        children(&self.0)
    }
}

impl TypeBound {
    /// Get the path of the trait bound.
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }
}
