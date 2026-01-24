//! Statement AST nodes.

use crate::ast::{Expr, Pat, Type, ast_enum, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

ast_node!(Block);
ast_node!(LetStmt);
ast_node!(ExprStmt);

ast_enum!(
    /// Statement enum - all statement variants.
    Stmt {
        Let(LetStmt),
        Expr(ExprStmt),
    }
);

// === Typed accessors ===

impl Block {
    pub fn statements(&self) -> impl Iterator<Item = Stmt> {
        children(&self.0)
    }

    pub fn tail_expr(&self) -> Option<Expr> {
        // The tail expression is the last expression without a semicolon.
        // It must be the very last meaningful child in the block (ignoring trivia).
        // We find the last child that is either an Expr or a Stmt, and only
        // return it if it's an Expr.
        let last_child = self
            .0
            .children()
            .filter(|child| Expr::can_cast(child.kind()) || Stmt::can_cast(child.kind()))
            .last()?;
        Expr::cast(last_child)
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
