//! Typed AST wrappers over the untyped syntax tree.

use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

mod expressions;
mod items;
mod patterns;
pub mod pretty;
mod statements;
mod types;

pub use expressions::*;
pub use items::*;
pub use patterns::*;
pub use statements::*;
pub use types::*;

/// Macro to define simple AST node wrappers.
///
/// Generates a struct that wraps SyntaxNode and implements rowan::ast::AstNode.
/// The SyntaxKind variant must match the struct name.
macro_rules! ast_node {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(SyntaxNode);

        impl rowan::ast::AstNode for $name {
            type Language = crate::syntax::Lang;

            fn can_cast(kind: <Self::Language as rowan::Language>::Kind) -> bool {
                kind == SyntaxKind::$name
            }

            fn cast(node: SyntaxNode) -> Option<Self> {
                if Self::can_cast(node.kind()) {
                    Some(Self(node))
                } else {
                    None
                }
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.0
            }
        }
    };
}
pub(crate) use ast_node;

// === Support functions ===

/// Get the first child node of type N.
pub fn child<N: AstNode<Language = crate::syntax::Lang>>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

/// Get all children of type N.
pub fn children<N: AstNode<Language = crate::syntax::Lang>>(
    parent: &SyntaxNode,
) -> impl Iterator<Item = N> {
    parent.children().filter_map(N::cast)
}

/// Get the first token of a specific kind.
pub fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|it| it.into_token())
        .find(|it| it.kind() == kind)
}
