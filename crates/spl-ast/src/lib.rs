//! Typed AST wrappers over the untyped syntax tree.

use rowan::ast::AstNode;
use spl_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

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
/// Generates a struct that wraps `SyntaxNode` and implements `rowan::ast::AstNode`.
/// The `SyntaxKind` variant must match the struct name.
macro_rules! ast_node {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(SyntaxNode);

        impl rowan::ast::AstNode for $name {
            type Language = spl_syntax::Lang;

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

/// Macro to define AST enum wrappers implementing `AstNode`.
///
/// Generates an enum where each variant wraps a typed AST node struct,
/// with automatic `AstNode` implementation that dispatches to variants.
///
/// # Syntax
/// ```ignore
/// ast_enum!(EnumName {
///     VariantName(StructType),
///     ...
/// });
/// ```
///
/// The `SyntaxKind` is derived from the struct type name (e.g., `BinExpr` -> `SyntaxKind::BinExpr`).
macro_rules! ast_enum {
    (
        $(#[$meta:meta])*
        $enum_name:ident {
            $($variant:ident($struct_ty:ident)),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $enum_name {
            $($variant($struct_ty),)*
        }

        impl rowan::ast::AstNode for $enum_name {
            type Language = spl_syntax::Lang;

            fn can_cast(kind: SyntaxKind) -> bool {
                matches!(kind, $(SyntaxKind::$struct_ty)|*)
            }

            fn cast(node: SyntaxNode) -> Option<Self> {
                match node.kind() {
                    $(SyntaxKind::$struct_ty => Some($enum_name::$variant($struct_ty(node))),)*
                    _ => None,
                }
            }

            fn syntax(&self) -> &SyntaxNode {
                match self {
                    $($enum_name::$variant(it) => it.syntax(),)*
                }
            }
        }
    };
}
pub(crate) use ast_enum;

// === Support functions ===

/// Get the first child node of type N.
pub fn child<N: AstNode<Language = spl_syntax::Lang>>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

/// Get all children of type N.
pub fn children<N: AstNode<Language = spl_syntax::Lang>>(
    parent: &SyntaxNode,
) -> impl Iterator<Item = N> {
    parent.children().filter_map(N::cast)
}

/// Get the first token of a specific kind.
pub fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|it| it.kind() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rowan::ast::AstNode;
    use spl_parser::parse;

    #[test]
    fn ast_enum_can_cast() {
        // Verify can_cast works for all Stmt variants
        assert!(Stmt::can_cast(SyntaxKind::LetStmt));
        assert!(Stmt::can_cast(SyntaxKind::ExprStmt));
        assert!(!Stmt::can_cast(SyntaxKind::Block));
    }

    #[test]
    fn ast_enum_cast_roundtrip() {
        let source = "fn main() { let x = 1; }";
        let parsed = parse(source);
        let root = parsed.syntax();

        // Find a LetStmt node
        let let_node = root
            .descendants()
            .find(|n| n.kind() == SyntaxKind::LetStmt)
            .unwrap();

        // Cast to Stmt enum
        let stmt = Stmt::cast(let_node.clone()).unwrap();

        // Verify syntax() returns the same node
        assert_eq!(stmt.syntax(), &let_node);
    }

    #[test]
    fn ast_enum_cast_wrong_kind_returns_none() {
        let source = "fn main() {}";
        let parsed = parse(source);
        let root = parsed.syntax();

        // Find a Block node (not a Stmt)
        let block_node = root
            .descendants()
            .find(|n| n.kind() == SyntaxKind::Block)
            .unwrap();

        // Stmt::cast should return None
        assert!(Stmt::cast(block_node).is_none());
    }
}
