//! Statement AST nodes.

use crate::ast::{Expr, Pat, Type, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

ast_node!(Block);
ast_node!(LetStmt);
ast_node!(ExprStmt);

/// Statement enum - all statement variants.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Stmt {
    Let(LetStmt),
    Expr(ExprStmt),
}

impl AstNode for Stmt {
    type Language = crate::syntax::Lang;

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(kind, SyntaxKind::LetStmt | SyntaxKind::ExprStmt)
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::LetStmt => Some(Stmt::Let(LetStmt(node))),
            SyntaxKind::ExprStmt => Some(Stmt::Expr(ExprStmt(node))),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Stmt::Let(it) => it.syntax(),
            Stmt::Expr(it) => it.syntax(),
        }
    }
}

// === Typed accessors ===

impl Block {
    pub fn statements(&self) -> impl Iterator<Item = Stmt> {
        children(&self.0)
    }

    pub fn tail_expr(&self) -> Option<Expr> {
        // The tail expression is the last expression without a semicolon
        // This is determined by the parser, so we just look for an Expr child
        // that isn't wrapped in ExprStmt
        self.0.children().filter_map(Expr::cast).last()
    }
}

impl LetStmt {
    pub fn let_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::LET_KW)
    }

    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }

    pub fn pat(&self) -> Option<Pat> {
        child(&self.0)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.0)
    }

    pub fn initializer(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl ExprStmt {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn semicolon(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::SEMI)
    }
}
