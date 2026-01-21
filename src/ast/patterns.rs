//! Pattern AST nodes.

use crate::ast::{Name, NameRef, Path, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

ast_node!(IdentPat);
ast_node!(WildcardPat);
ast_node!(LiteralPat);
ast_node!(RangePat);
ast_node!(TuplePat);
ast_node!(SlicePat);
ast_node!(StructPat);
ast_node!(RefPat);
ast_node!(RestPat);
ast_node!(StructPatField);

/// Pattern enum - all pattern variants.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pat {
    Ident(IdentPat),
    Wildcard(WildcardPat),
    Literal(LiteralPat),
    Range(RangePat),
    Tuple(TuplePat),
    Slice(SlicePat),
    Struct(StructPat),
    Ref(RefPat),
    Rest(RestPat),
}

impl AstNode for Pat {
    type Language = crate::syntax::Lang;

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::IdentPat
                | SyntaxKind::WildcardPat
                | SyntaxKind::LiteralPat
                | SyntaxKind::RangePat
                | SyntaxKind::TuplePat
                | SyntaxKind::SlicePat
                | SyntaxKind::StructPat
                | SyntaxKind::RefPat
                | SyntaxKind::RestPat
        )
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::IdentPat => Some(Pat::Ident(IdentPat(node))),
            SyntaxKind::WildcardPat => Some(Pat::Wildcard(WildcardPat(node))),
            SyntaxKind::LiteralPat => Some(Pat::Literal(LiteralPat(node))),
            SyntaxKind::RangePat => Some(Pat::Range(RangePat(node))),
            SyntaxKind::TuplePat => Some(Pat::Tuple(TuplePat(node))),
            SyntaxKind::SlicePat => Some(Pat::Slice(SlicePat(node))),
            SyntaxKind::StructPat => Some(Pat::Struct(StructPat(node))),
            SyntaxKind::RefPat => Some(Pat::Ref(RefPat(node))),
            SyntaxKind::RestPat => Some(Pat::Rest(RestPat(node))),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Pat::Ident(it) => it.syntax(),
            Pat::Wildcard(it) => it.syntax(),
            Pat::Literal(it) => it.syntax(),
            Pat::Range(it) => it.syntax(),
            Pat::Tuple(it) => it.syntax(),
            Pat::Slice(it) => it.syntax(),
            Pat::Struct(it) => it.syntax(),
            Pat::Ref(it) => it.syntax(),
            Pat::Rest(it) => it.syntax(),
        }
    }
}

// === Typed accessors ===

impl IdentPat {
    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }

    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }
}

impl WildcardPat {
    // Wildcard is just `_`
}

impl LiteralPat {
    pub fn token(&self) -> Option<SyntaxToken> {
        self.0.first_token()
    }
}

impl RangePat {
    pub fn start(&self) -> Option<Pat> {
        children::<Pat>(&self.0).next()
    }

    pub fn end(&self) -> Option<Pat> {
        children::<Pat>(&self.0).nth(1)
    }
}

impl TuplePat {
    pub fn patterns(&self) -> impl Iterator<Item = Pat> {
        children(&self.0)
    }
}

impl SlicePat {
    pub fn patterns(&self) -> impl Iterator<Item = Pat> {
        children(&self.0)
    }
}

impl StructPat {
    /// Get the struct type as a path (always present, even for simple `Point { }`).
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }

    pub fn fields(&self) -> impl Iterator<Item = StructPatField> {
        children(&self.0)
    }

    pub fn rest(&self) -> Option<RestPat> {
        child(&self.0)
    }
}

impl StructPatField {
    /// Get the field name (always wrapped in NameRef).
    pub fn name(&self) -> Option<NameRef> {
        child(&self.0)
    }

    /// Get the nested pattern (for `field: pattern` syntax).
    /// Returns None for shorthand syntax like `{ x }`.
    pub fn pat(&self) -> Option<Pat> {
        child(&self.0)
    }
}

impl RefPat {
    pub fn amp(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::AMP)
    }

    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }

    pub fn pat(&self) -> Option<Pat> {
        child(&self.0)
    }
}

impl RestPat {
    // Rest is just `..`
}
