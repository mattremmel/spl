//! Statement AST nodes.

use crate::{Expr, Pat, Type, ast_enum, ast_node, child, children, token};
use rowan::ast::AstNode;
use spl_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceFile;
    use rowan::ast::AstNode;
    use spl_parser::parse;

    /// Helper to parse source and get the function body block.
    fn parse_block(source: &str) -> Block {
        let parsed = parse(source);
        assert!(
            parsed.errors().is_empty(),
            "parse errors: {:?}",
            parsed.errors()
        );
        let source_file = SourceFile::cast(parsed.syntax()).expect("expected SourceFile");
        source_file
            .items()
            .find_map(|item| match item {
                crate::Item::Function(f) => f.body(),
                _ => None,
            })
            .expect("expected function body")
    }

    /// Helper to parse source and find first `LetStmt`.
    fn parse_let_stmt(source: &str) -> LetStmt {
        let parsed = parse(source);
        assert!(
            parsed.errors().is_empty(),
            "parse errors: {:?}",
            parsed.errors()
        );
        parsed
            .syntax()
            .descendants()
            .find_map(LetStmt::cast)
            .expect("expected LetStmt")
    }

    // =========================================================================
    // Block Tests
    // =========================================================================

    #[test]
    fn block_empty() {
        let block = parse_block("fn main() {}");
        assert_eq!(block.statements().count(), 0);
        assert!(block.tail_expr().is_none());
    }

    #[test]
    fn block_with_statements() {
        let block = parse_block("fn main() { let x = 1; let y = 2; }");
        assert_eq!(block.statements().count(), 2);
        assert!(block.tail_expr().is_none());
    }

    #[test]
    fn block_with_tail_expr() {
        let block = parse_block("fn main() { 42 }");
        assert!(block.tail_expr().is_some());
    }

    #[test]
    fn block_statements_and_tail() {
        let block = parse_block("fn main() { let x = 1; x + 1 }");
        assert_eq!(block.statements().count(), 1);
        assert!(block.tail_expr().is_some());
    }

    #[test]
    fn block_trailing_semicolon_no_tail() {
        let block = parse_block("fn main() { 42; }");
        // With semicolon, the expression becomes a statement, not a tail
        assert!(block.tail_expr().is_none());
    }

    // =========================================================================
    // LetStmt Tests
    // =========================================================================

    #[test]
    fn let_stmt_simple() {
        let stmt = parse_let_stmt("fn main() { let x = 1; }");
        assert!(stmt.let_kw().is_some());
        assert!(stmt.pat().is_some());
        assert!(stmt.ty().is_none());
        assert!(stmt.initializer().is_some());
    }

    #[test]
    fn let_stmt_with_type() {
        let stmt = parse_let_stmt("fn main() { let x: i32 = 1; }");
        assert!(stmt.pat().is_some());
        assert!(stmt.ty().is_some());
        assert!(stmt.initializer().is_some());
    }

    #[test]
    fn let_stmt_no_initializer() {
        let stmt = parse_let_stmt("fn main() { let x: i32; }");
        assert!(stmt.pat().is_some());
        assert!(stmt.ty().is_some());
        assert!(stmt.initializer().is_none());
    }

    #[test]
    fn let_stmt_mutable() {
        let stmt = parse_let_stmt("fn main() { let mut x = 1; }");
        assert!(stmt.mut_kw().is_some());
    }

    // =========================================================================
    // ExprStmt Tests
    // =========================================================================

    #[test]
    fn expr_stmt_with_semi() {
        let parsed = parse("fn main() { foo(); }");
        let expr_stmt: ExprStmt = parsed
            .syntax()
            .descendants()
            .find_map(ExprStmt::cast)
            .expect("expected ExprStmt");
        assert!(expr_stmt.expr().is_some());
        assert!(expr_stmt.semicolon().is_some());
    }

    // =========================================================================
    // Stmt Enum Tests
    // =========================================================================

    #[test]
    fn stmt_enum_let_variant() {
        let block = parse_block("fn main() { let x = 1; }");
        let stmt = block.statements().next().expect("expected statement");
        assert!(matches!(stmt, Stmt::Let(_)));
    }

    #[test]
    fn stmt_enum_expr_variant() {
        let block = parse_block("fn main() { foo(); }");
        let stmt = block.statements().next().expect("expected statement");
        assert!(matches!(stmt, Stmt::Expr(_)));
    }
}
