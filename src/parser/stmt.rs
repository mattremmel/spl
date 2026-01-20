//! Statement parser for SPL.
//!
//! Parses let statements, expression statements, and blocks.

use crate::parser::{CompletedMarker, Parser};
use crate::syntax::SyntaxKind;

use super::expr;

/// Parse a let statement: `let [mut] pattern [: type] [= expr];`
fn let_stmt(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::LET_KW)?;

    // Optional mut
    p.eat(SyntaxKind::MUT_KW);

    // Pattern (simplified: just identifier for now)
    pattern(p)?;

    // Optional type annotation
    if p.eat(SyntaxKind::COLON) {
        type_annotation(p)?;
    }

    // Optional initializer
    if p.eat(SyntaxKind::EQ) {
        let _ = expr::expr(p)?;
    }

    p.expect(SyntaxKind::SEMI)?;
    Ok(m.complete(p, SyntaxKind::LetStmt))
}

/// Parse a pattern (simplified: just identifier or wildcard for now).
fn pattern(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    match p.current() {
        Some(SyntaxKind::IDENT) => {
            p.bump();
            Ok(m.complete(p, SyntaxKind::IdentPat))
        }
        _ => {
            let err = p.error_at_current("expected pattern".to_string());
            m.abandon(p);
            Err(err)
        }
    }
}

/// Parse a type annotation (simplified: path types for now).
fn type_annotation(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Reference type: &T or &mut T
    if p.at(SyntaxKind::AMP) {
        p.bump();
        p.eat(SyntaxKind::MUT_KW);
        type_annotation(p)?;
        return Ok(m.complete(p, SyntaxKind::RefType));
    }

    // Array type: [T; N] or slice type: [T]
    if p.at(SyntaxKind::L_BRACKET) {
        p.bump();
        type_annotation(p)?;
        if p.eat(SyntaxKind::SEMI) {
            // Array type [T; N]
            let _ = expr::expr(p)?;
            p.expect(SyntaxKind::R_BRACKET)?;
            return Ok(m.complete(p, SyntaxKind::ArrayType));
        } else {
            // Slice type [T]
            p.expect(SyntaxKind::R_BRACKET)?;
            return Ok(m.complete(p, SyntaxKind::SliceType));
        }
    }

    // Tuple type: (T1, T2, ...)
    if p.at(SyntaxKind::L_PAREN) {
        p.bump();
        if !p.at(SyntaxKind::R_PAREN) {
            type_annotation(p)?;
            while p.eat(SyntaxKind::COMMA) {
                if p.at(SyntaxKind::R_PAREN) {
                    break;
                }
                type_annotation(p)?;
            }
        }
        p.expect(SyntaxKind::R_PAREN)?;
        return Ok(m.complete(p, SyntaxKind::TupleType));
    }

    // Path type: identifier or path::to::Type
    if !p.at(SyntaxKind::IDENT) {
        let err = p.error_at_current("expected type".to_string());
        m.abandon(p);
        return Err(err);
    }
    p.bump();

    while p.at(SyntaxKind::COLON_COLON) {
        p.bump();
        if !p.at(SyntaxKind::IDENT) {
            return Err(p.error_at_current("expected identifier after '::'".to_string()));
        }
        p.bump();
    }

    Ok(m.complete(p, SyntaxKind::PathType))
}

/// Parse a block with statements: `{ stmt* [expr] }`
pub(crate) fn block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_BRACE)?;

    // Parse statements until we hit the closing brace
    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        // Try to parse a statement
        match p.current() {
            Some(SyntaxKind::LET_KW) => {
                let_stmt(p)?;
            }
            Some(_) => {
                // Try to parse an expression
                let expr_m = p.start();
                let expr_result = expr::expr(p)?;

                if expr_result.is_none() {
                    // Couldn't parse an expression - error recovery
                    expr_m.abandon(p);
                    // Skip the problematic token
                    if p.current().is_some() && !p.at(SyntaxKind::R_BRACE) {
                        p.bump();
                    }
                    continue;
                }

                // Check for semicolon
                if p.eat(SyntaxKind::SEMI) {
                    // Expression statement
                    expr_m.complete(p, SyntaxKind::ExprStmt);
                } else if p.at(SyntaxKind::R_BRACE) {
                    // Tail expression (no semicolon, at end of block)
                    // Don't wrap in ExprStmt - the expression is directly a child of Block
                    expr_m.abandon(p);
                } else {
                    // Missing semicolon - this is an error
                    // For now, abandon and let the outer loop continue
                    expr_m.abandon(p);
                }
            }
            None => break,
        }
    }

    p.expect(SyntaxKind::R_BRACE)?;
    Ok(m.complete(p, SyntaxKind::Block))
}

#[cfg(test)]
mod tests {
    use crate::parser::tests::check_expr;
    use expect_test::expect;

    #[test]
    fn block_with_let_stmt() {
        check_expr(
            "{ let x = 1; }",
            &expect![[r#"
                BlockExpr@0..14
                  Block@0..14
                    L_BRACE@0..1 "{"
                    LetStmt@1..12
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      WHITESPACE@7..8 " "
                      EQ@8..9 "="
                      LiteralExpr@9..11
                        WHITESPACE@9..10 " "
                        INT_LITERAL@10..11 "1"
                      SEMI@11..12 ";"
                    WHITESPACE@12..13 " "
                    R_BRACE@13..14 "}"
            "#]],
        );
    }

    #[test]
    fn block_with_let_mut() {
        check_expr(
            "{ let mut x = 1; }",
            &expect![[r#"
                BlockExpr@0..18
                  Block@0..18
                    L_BRACE@0..1 "{"
                    LetStmt@1..16
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      WHITESPACE@5..6 " "
                      MUT_KW@6..9 "mut"
                      IdentPat@9..11
                        WHITESPACE@9..10 " "
                        IDENT@10..11 "x"
                      WHITESPACE@11..12 " "
                      EQ@12..13 "="
                      LiteralExpr@13..15
                        WHITESPACE@13..14 " "
                        INT_LITERAL@14..15 "1"
                      SEMI@15..16 ";"
                    WHITESPACE@16..17 " "
                    R_BRACE@17..18 "}"
            "#]],
        );
    }

    #[test]
    fn block_with_type_annotation() {
        check_expr(
            "{ let x: i32 = 1; }",
            &expect![[r#"
                BlockExpr@0..19
                  Block@0..19
                    L_BRACE@0..1 "{"
                    LetStmt@1..17
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..12
                        WHITESPACE@8..9 " "
                        IDENT@9..12 "i32"
                      WHITESPACE@12..13 " "
                      EQ@13..14 "="
                      LiteralExpr@14..16
                        WHITESPACE@14..15 " "
                        INT_LITERAL@15..16 "1"
                      SEMI@16..17 ";"
                    WHITESPACE@17..18 " "
                    R_BRACE@18..19 "}"
            "#]],
        );
    }

    #[test]
    fn block_with_expr_stmt() {
        check_expr(
            "{ foo(); }",
            &expect![[r#"
                BlockExpr@0..10
                  Block@0..10
                    L_BRACE@0..1 "{"
                    ExprStmt@1..8
                      CallExpr@1..7
                        PathExpr@1..5
                          WHITESPACE@1..2 " "
                          IDENT@2..5 "foo"
                        ArgList@5..7
                          L_PAREN@5..6 "("
                          R_PAREN@6..7 ")"
                      SEMI@7..8 ";"
                    WHITESPACE@8..9 " "
                    R_BRACE@9..10 "}"
            "#]],
        );
    }

    #[test]
    fn block_with_tail_expr() {
        check_expr(
            "{ x + 1 }",
            &expect![[r#"
                BlockExpr@0..9
                  Block@0..9
                    L_BRACE@0..1 "{"
                    BinExpr@1..7
                      PathExpr@1..3
                        WHITESPACE@1..2 " "
                        IDENT@2..3 "x"
                      WHITESPACE@3..4 " "
                      PLUS@4..5 "+"
                      LiteralExpr@5..7
                        WHITESPACE@5..6 " "
                        INT_LITERAL@6..7 "1"
                    WHITESPACE@7..8 " "
                    R_BRACE@8..9 "}"
            "#]],
        );
    }

    #[test]
    fn block_with_multiple_stmts() {
        check_expr(
            "{ let x = 1; let y = 2; x + y }",
            &expect![[r#"
                BlockExpr@0..31
                  Block@0..31
                    L_BRACE@0..1 "{"
                    LetStmt@1..12
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      WHITESPACE@7..8 " "
                      EQ@8..9 "="
                      LiteralExpr@9..11
                        WHITESPACE@9..10 " "
                        INT_LITERAL@10..11 "1"
                      SEMI@11..12 ";"
                    LetStmt@12..23
                      WHITESPACE@12..13 " "
                      LET_KW@13..16 "let"
                      IdentPat@16..18
                        WHITESPACE@16..17 " "
                        IDENT@17..18 "y"
                      WHITESPACE@18..19 " "
                      EQ@19..20 "="
                      LiteralExpr@20..22
                        WHITESPACE@20..21 " "
                        INT_LITERAL@21..22 "2"
                      SEMI@22..23 ";"
                    BinExpr@23..29
                      PathExpr@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "x"
                      WHITESPACE@25..26 " "
                      PLUS@26..27 "+"
                      PathExpr@27..29
                        WHITESPACE@27..28 " "
                        IDENT@28..29 "y"
                    WHITESPACE@29..30 " "
                    R_BRACE@30..31 "}"
            "#]],
        );
    }

    #[test]
    fn block_with_expr_stmt_and_tail() {
        check_expr(
            "{ foo(); bar() }",
            &expect![[r#"
                BlockExpr@0..16
                  Block@0..16
                    L_BRACE@0..1 "{"
                    ExprStmt@1..8
                      CallExpr@1..7
                        PathExpr@1..5
                          WHITESPACE@1..2 " "
                          IDENT@2..5 "foo"
                        ArgList@5..7
                          L_PAREN@5..6 "("
                          R_PAREN@6..7 ")"
                      SEMI@7..8 ";"
                    CallExpr@8..14
                      PathExpr@8..12
                        WHITESPACE@8..9 " "
                        IDENT@9..12 "bar"
                      ArgList@12..14
                        L_PAREN@12..13 "("
                        R_PAREN@13..14 ")"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }

    #[test]
    fn let_without_initializer() {
        check_expr(
            "{ let x: i32; }",
            &expect![[r#"
                BlockExpr@0..15
                  Block@0..15
                    L_BRACE@0..1 "{"
                    LetStmt@1..13
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..12
                        WHITESPACE@8..9 " "
                        IDENT@9..12 "i32"
                      SEMI@12..13 ";"
                    WHITESPACE@13..14 " "
                    R_BRACE@14..15 "}"
            "#]],
        );
    }

    #[test]
    fn ref_type_annotation() {
        check_expr(
            "{ let x: &i32 = y; }",
            &expect![[r#"
                BlockExpr@0..20
                  Block@0..20
                    L_BRACE@0..1 "{"
                    LetStmt@1..18
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      COLON@7..8 ":"
                      RefType@8..13
                        WHITESPACE@8..9 " "
                        AMP@9..10 "&"
                        PathType@10..13
                          IDENT@10..13 "i32"
                      WHITESPACE@13..14 " "
                      EQ@14..15 "="
                      PathExpr@15..17
                        WHITESPACE@15..16 " "
                        IDENT@16..17 "y"
                      SEMI@17..18 ";"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn mut_ref_type_annotation() {
        check_expr(
            "{ let x: &mut i32 = y; }",
            &expect![[r#"
                BlockExpr@0..24
                  Block@0..24
                    L_BRACE@0..1 "{"
                    LetStmt@1..22
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      COLON@7..8 ":"
                      RefType@8..17
                        WHITESPACE@8..9 " "
                        AMP@9..10 "&"
                        MUT_KW@10..13 "mut"
                        PathType@13..17
                          WHITESPACE@13..14 " "
                          IDENT@14..17 "i32"
                      WHITESPACE@17..18 " "
                      EQ@18..19 "="
                      PathExpr@19..21
                        WHITESPACE@19..20 " "
                        IDENT@20..21 "y"
                      SEMI@21..22 ";"
                    WHITESPACE@22..23 " "
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn array_type_annotation() {
        check_expr(
            "{ let x: [i32; 5]; }",
            &expect![[r#"
                BlockExpr@0..20
                  Block@0..20
                    L_BRACE@0..1 "{"
                    LetStmt@1..18
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      COLON@7..8 ":"
                      ArrayType@8..17
                        WHITESPACE@8..9 " "
                        L_BRACKET@9..10 "["
                        PathType@10..13
                          IDENT@10..13 "i32"
                        SEMI@13..14 ";"
                        LiteralExpr@14..16
                          WHITESPACE@14..15 " "
                          INT_LITERAL@15..16 "5"
                        R_BRACKET@16..17 "]"
                      SEMI@17..18 ";"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn slice_type_annotation() {
        check_expr(
            "{ let x: [i32]; }",
            &expect![[r#"
                BlockExpr@0..17
                  Block@0..17
                    L_BRACE@0..1 "{"
                    LetStmt@1..15
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      COLON@7..8 ":"
                      SliceType@8..14
                        WHITESPACE@8..9 " "
                        L_BRACKET@9..10 "["
                        PathType@10..13
                          IDENT@10..13 "i32"
                        R_BRACKET@13..14 "]"
                      SEMI@14..15 ";"
                    WHITESPACE@15..16 " "
                    R_BRACE@16..17 "}"
            "#]],
        );
    }

    #[test]
    fn tuple_type_annotation() {
        check_expr(
            "{ let x: (i32, bool); }",
            &expect![[r#"
                BlockExpr@0..23
                  Block@0..23
                    L_BRACE@0..1 "{"
                    LetStmt@1..21
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "x"
                      COLON@7..8 ":"
                      TupleType@8..20
                        WHITESPACE@8..9 " "
                        L_PAREN@9..10 "("
                        PathType@10..13
                          IDENT@10..13 "i32"
                        COMMA@13..14 ","
                        PathType@14..19
                          WHITESPACE@14..15 " "
                          IDENT@15..19 "bool"
                        R_PAREN@19..20 ")"
                      SEMI@20..21 ";"
                    WHITESPACE@21..22 " "
                    R_BRACE@22..23 "}"
            "#]],
        );
    }
}
