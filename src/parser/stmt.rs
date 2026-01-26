//! Statement parser for SPL.
//!
//! Parses let statements, expression statements, and blocks.

use crate::parser::{CompletedMarker, Parser};
use crate::syntax::SyntaxKind;

use super::expr;
use super::pattern;

/// Check if the current token can start a statement or expression.
/// Used to distinguish between missing semicolons and block-ending expressions.
/// Block-ending expressions (if, while, for, loop, blocks) don't need semicolons
/// when followed by another statement.
fn can_start_stmt_or_expr(p: &mut Parser<'_>) -> bool {
    matches!(
        p.current(),
        Some(
            SyntaxKind::IDENT
                | SyntaxKind::INT_LITERAL
                | SyntaxKind::FLOAT_LITERAL
                | SyntaxKind::STRING_LITERAL
                | SyntaxKind::CHAR_LITERAL
                | SyntaxKind::TRUE_KW
                | SyntaxKind::FALSE_KW
                | SyntaxKind::IF_KW
                | SyntaxKind::WHILE_KW
                | SyntaxKind::FOR_KW
                | SyntaxKind::LOOP_KW
                | SyntaxKind::RETURN_KW
                | SyntaxKind::BREAK_KW
                | SyntaxKind::CONTINUE_KW
                | SyntaxKind::L_PAREN
                | SyntaxKind::L_BRACKET
                | SyntaxKind::L_BRACE
                | SyntaxKind::AMP
                | SyntaxKind::STAR
                | SyntaxKind::MINUS
                | SyntaxKind::BANG
                | SyntaxKind::LET_KW
        )
    )
}

/// Parse a let statement: `let [mut] pattern [: type] [= expr];`
fn let_stmt(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::LET_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Optional mut
    p.eat(SyntaxKind::MUT_KW);

    // Pattern
    if let Err(e) = pattern::pattern(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional type annotation
    if p.eat(SyntaxKind::COLON)
        && let Err(e) = type_annotation(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Optional initializer
    if p.eat(SyntaxKind::EQ)
        && let Err(e) = expr::expr(p)
    {
        m.abandon(p);
        return Err(e);
    }

    if let Err(e) = p.expect(SyntaxKind::SEMI) {
        m.abandon(p);
        return Err(e);
    }
    Ok(m.complete(p, SyntaxKind::LetStmt))
}

/// Parse a type annotation.
pub(crate) fn type_annotation(
    p: &mut Parser<'_>,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Never type: !
    if p.at(SyntaxKind::BANG) {
        p.bump();
        return Ok(m.complete(p, SyntaxKind::NeverType));
    }

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
        p.parse_delimited(SyntaxKind::R_PAREN, |p| {
            type_annotation(p)?;
            Ok(())
        })?;
        p.expect(SyntaxKind::R_PAREN)?;
        return Ok(m.complete(p, SyntaxKind::TupleType));
    }

    // Function pointer type: fn(T1, T2) -> R
    if p.at(SyntaxKind::FN_KW) {
        p.bump();
        p.expect(SyntaxKind::L_PAREN)?;
        p.parse_delimited(SyntaxKind::R_PAREN, |p| {
            type_annotation(p)?;
            Ok(())
        })?;
        p.expect(SyntaxKind::R_PAREN)?;
        // Optional return type
        if p.eat(SyntaxKind::ARROW) {
            type_annotation(p)?;
        }
        return Ok(m.complete(p, SyntaxKind::FnPtrType));
    }

    // Path type: identifier, Self, crate, super, or path::to::Type<Args>
    if !p.at(SyntaxKind::IDENT)
        && !p.at(SyntaxKind::SELF_TYPE_KW)
        && !p.at(SyntaxKind::CRATE_KW)
        && !p.at(SyntaxKind::SUPER_KW)
    {
        let err = p.error_at_current("expected type".to_string());
        m.abandon(p);
        return Err(err);
    }

    // Use structured path parsing
    match crate::parser::path::path(p) {
        Ok(_) => Ok(m.complete(p, SyntaxKind::PathType)),
        Err(e) => {
            m.abandon(p);
            Err(e)
        }
    }
}

/// Parse a block with statements: `{ stmt* [expr] }`
pub(crate) fn block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::L_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    // Parse statements until we hit the closing brace
    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        // Try to parse a statement
        match p.current() {
            Some(SyntaxKind::LET_KW) => {
                if let Err(err) = let_stmt(p) {
                    // Recover to next statement or block end
                    p.recover_to_stmt(err);
                }
            }
            Some(_) => {
                // Try to parse an expression
                let expr_m = p.start();
                match expr::expr(p) {
                    Ok(Some(_)) => {
                        // Successfully parsed an expression
                        if p.eat(SyntaxKind::SEMI) {
                            // Expression statement with semicolon
                            expr_m.complete(p, SyntaxKind::ExprStmt);
                        } else if p.at(SyntaxKind::R_BRACE) {
                            // Tail expression (no semicolon, at end of block)
                            expr_m.abandon(p);
                        } else if can_start_stmt_or_expr(p) {
                            // No semicolon but another expression follows
                            // This is valid for block-ending expressions (if, while, for, loop, block)
                            // which don't require semicolons when used as statements.
                            // Wrap in ExprStmt for consistent AST structure.
                            expr_m.complete(p, SyntaxKind::ExprStmt);
                        } else {
                            // Missing semicolon - emit error but continue
                            let err =
                                p.error_at_current("expected ';' after expression".to_string());
                            p.error(err);
                            expr_m.abandon(p);
                        }
                    }
                    Ok(None) => {
                        // Couldn't parse an expression - skip token
                        expr_m.abandon(p);
                        if p.current().is_some() && !p.at(SyntaxKind::R_BRACE) {
                            let err = p.error_at_current("expected expression".to_string());
                            p.recover_to_stmt(err);
                        }
                    }
                    Err(err) => {
                        // Expression parsing failed - recover
                        expr_m.abandon(p);
                        p.recover_to_stmt(err);
                    }
                }
            }
            None => break,
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACE) {
        m.abandon(p);
        return Err(e);
    }
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
                        Name@5..7
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
                        Name@9..11
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
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..12
                        Path@8..12
                          PathSegment@8..12
                            NameRef@8..12
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
                          Path@1..5
                            PathSegment@1..5
                              NameRef@1..5
                                WHITESPACE@1..2 " "
                                IDENT@2..5 "foo"
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
                        Path@1..3
                          PathSegment@1..3
                            NameRef@1..3
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
                        Name@5..7
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
                        Name@16..18
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
                        Path@23..25
                          PathSegment@23..25
                            NameRef@23..25
                              WHITESPACE@23..24 " "
                              IDENT@24..25 "x"
                      WHITESPACE@25..26 " "
                      PLUS@26..27 "+"
                      PathExpr@27..29
                        Path@27..29
                          PathSegment@27..29
                            NameRef@27..29
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
                          Path@1..5
                            PathSegment@1..5
                              NameRef@1..5
                                WHITESPACE@1..2 " "
                                IDENT@2..5 "foo"
                        L_PAREN@5..6 "("
                        R_PAREN@6..7 ")"
                      SEMI@7..8 ";"
                    CallExpr@8..14
                      PathExpr@8..12
                        Path@8..12
                          PathSegment@8..12
                            NameRef@8..12
                              WHITESPACE@8..9 " "
                              IDENT@9..12 "bar"
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
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..12
                        Path@8..12
                          PathSegment@8..12
                            NameRef@8..12
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
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      RefType@8..13
                        WHITESPACE@8..9 " "
                        AMP@9..10 "&"
                        PathType@10..13
                          Path@10..13
                            PathSegment@10..13
                              NameRef@10..13
                                IDENT@10..13 "i32"
                      WHITESPACE@13..14 " "
                      EQ@14..15 "="
                      PathExpr@15..17
                        Path@15..17
                          PathSegment@15..17
                            NameRef@15..17
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
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      RefType@8..17
                        WHITESPACE@8..9 " "
                        AMP@9..10 "&"
                        MUT_KW@10..13 "mut"
                        PathType@13..17
                          Path@13..17
                            PathSegment@13..17
                              NameRef@13..17
                                WHITESPACE@13..14 " "
                                IDENT@14..17 "i32"
                      WHITESPACE@17..18 " "
                      EQ@18..19 "="
                      PathExpr@19..21
                        Path@19..21
                          PathSegment@19..21
                            NameRef@19..21
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
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      ArrayType@8..17
                        WHITESPACE@8..9 " "
                        L_BRACKET@9..10 "["
                        PathType@10..13
                          Path@10..13
                            PathSegment@10..13
                              NameRef@10..13
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
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      SliceType@8..14
                        WHITESPACE@8..9 " "
                        L_BRACKET@9..10 "["
                        PathType@10..13
                          Path@10..13
                            PathSegment@10..13
                              NameRef@10..13
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
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      TupleType@8..20
                        WHITESPACE@8..9 " "
                        L_PAREN@9..10 "("
                        PathType@10..13
                          Path@10..13
                            PathSegment@10..13
                              NameRef@10..13
                                IDENT@10..13 "i32"
                        COMMA@13..14 ","
                        PathType@14..19
                          Path@14..19
                            PathSegment@14..19
                              NameRef@14..19
                                WHITESPACE@14..15 " "
                                IDENT@15..19 "bool"
                        R_PAREN@19..20 ")"
                      SEMI@20..21 ";"
                    WHITESPACE@21..22 " "
                    R_BRACE@22..23 "}"
            "#]],
        );
    }

    #[test]
    fn generic_type_single_arg() {
        check_expr(
            "{ let x: Vec(i32); }",
            &expect![[r#"
                BlockExpr@0..20
                  Block@0..20
                    L_BRACE@0..1 "{"
                    LetStmt@1..18
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..17
                        Path@8..17
                          PathSegment@8..17
                            NameRef@8..12
                              WHITESPACE@8..9 " "
                              IDENT@9..12 "Vec"
                            GenericArgs@12..17
                              L_PAREN@12..13 "("
                              PathType@13..16
                                Path@13..16
                                  PathSegment@13..16
                                    NameRef@13..16
                                      IDENT@13..16 "i32"
                              R_PAREN@16..17 ")"
                      SEMI@17..18 ";"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn generic_type_multiple_args() {
        check_expr(
            "{ let x: HashMap(String, i32); }",
            &expect![[r#"
                BlockExpr@0..32
                  Block@0..32
                    L_BRACE@0..1 "{"
                    LetStmt@1..30
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..29
                        Path@8..29
                          PathSegment@8..29
                            NameRef@8..16
                              WHITESPACE@8..9 " "
                              IDENT@9..16 "HashMap"
                            GenericArgs@16..29
                              L_PAREN@16..17 "("
                              PathType@17..23
                                Path@17..23
                                  PathSegment@17..23
                                    NameRef@17..23
                                      IDENT@17..23 "String"
                              COMMA@23..24 ","
                              PathType@24..28
                                Path@24..28
                                  PathSegment@24..28
                                    NameRef@24..28
                                      WHITESPACE@24..25 " "
                                      IDENT@25..28 "i32"
                              R_PAREN@28..29 ")"
                      SEMI@29..30 ";"
                    WHITESPACE@30..31 " "
                    R_BRACE@31..32 "}"
            "#]],
        );
    }

    #[test]
    fn generic_type_nested() {
        check_expr(
            "{ let x: Option(Vec(i32)); }",
            &expect![[r#"
                BlockExpr@0..28
                  Block@0..28
                    L_BRACE@0..1 "{"
                    LetStmt@1..26
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..25
                        Path@8..25
                          PathSegment@8..25
                            NameRef@8..15
                              WHITESPACE@8..9 " "
                              IDENT@9..15 "Option"
                            GenericArgs@15..25
                              L_PAREN@15..16 "("
                              PathType@16..24
                                Path@16..24
                                  PathSegment@16..24
                                    NameRef@16..19
                                      IDENT@16..19 "Vec"
                                    GenericArgs@19..24
                                      L_PAREN@19..20 "("
                                      PathType@20..23
                                        Path@20..23
                                          PathSegment@20..23
                                            NameRef@20..23
                                              IDENT@20..23 "i32"
                                      R_PAREN@23..24 ")"
                              R_PAREN@24..25 ")"
                      SEMI@25..26 ";"
                    WHITESPACE@26..27 " "
                    R_BRACE@27..28 "}"
            "#]],
        );
    }

    #[test]
    fn fn_ptr_type_no_args() {
        check_expr(
            "{ let x: fn(); }",
            &expect![[r#"
                BlockExpr@0..16
                  Block@0..16
                    L_BRACE@0..1 "{"
                    LetStmt@1..14
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      FnPtrType@8..13
                        WHITESPACE@8..9 " "
                        FN_KW@9..11 "fn"
                        L_PAREN@11..12 "("
                        R_PAREN@12..13 ")"
                      SEMI@13..14 ";"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }

    #[test]
    fn fn_ptr_type_with_args() {
        check_expr(
            "{ let x: fn(i32, bool); }",
            &expect![[r#"
                BlockExpr@0..25
                  Block@0..25
                    L_BRACE@0..1 "{"
                    LetStmt@1..23
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      FnPtrType@8..22
                        WHITESPACE@8..9 " "
                        FN_KW@9..11 "fn"
                        L_PAREN@11..12 "("
                        PathType@12..15
                          Path@12..15
                            PathSegment@12..15
                              NameRef@12..15
                                IDENT@12..15 "i32"
                        COMMA@15..16 ","
                        PathType@16..21
                          Path@16..21
                            PathSegment@16..21
                              NameRef@16..21
                                WHITESPACE@16..17 " "
                                IDENT@17..21 "bool"
                        R_PAREN@21..22 ")"
                      SEMI@22..23 ";"
                    WHITESPACE@23..24 " "
                    R_BRACE@24..25 "}"
            "#]],
        );
    }

    #[test]
    fn fn_ptr_type_with_return() {
        check_expr(
            "{ let x: fn(i32) -> bool; }",
            &expect![[r#"
                BlockExpr@0..27
                  Block@0..27
                    L_BRACE@0..1 "{"
                    LetStmt@1..25
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      FnPtrType@8..24
                        WHITESPACE@8..9 " "
                        FN_KW@9..11 "fn"
                        L_PAREN@11..12 "("
                        PathType@12..15
                          Path@12..15
                            PathSegment@12..15
                              NameRef@12..15
                                IDENT@12..15 "i32"
                        R_PAREN@15..16 ")"
                        WHITESPACE@16..17 " "
                        ARROW@17..19 "->"
                        PathType@19..24
                          Path@19..24
                            PathSegment@19..24
                              NameRef@19..24
                                WHITESPACE@19..20 " "
                                IDENT@20..24 "bool"
                      SEMI@24..25 ";"
                    WHITESPACE@25..26 " "
                    R_BRACE@26..27 "}"
            "#]],
        );
    }

    #[test]
    fn path_type_with_generics() {
        check_expr(
            "{ let x: std.vec.Vec(T); }",
            &expect![[r#"
                BlockExpr@0..26
                  Block@0..26
                    L_BRACE@0..1 "{"
                    LetStmt@1..24
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..23
                        Path@8..23
                          PathSegment@8..12
                            NameRef@8..12
                              WHITESPACE@8..9 " "
                              IDENT@9..12 "std"
                          DOT@12..13 "."
                          PathSegment@13..16
                            NameRef@13..16
                              IDENT@13..16 "vec"
                          DOT@16..17 "."
                          PathSegment@17..23
                            NameRef@17..20
                              IDENT@17..20 "Vec"
                            GenericArgs@20..23
                              L_PAREN@20..21 "("
                              PathType@21..22
                                Path@21..22
                                  PathSegment@21..22
                                    NameRef@21..22
                                      IDENT@21..22 "T"
                              R_PAREN@22..23 ")"
                      SEMI@23..24 ";"
                    WHITESPACE@24..25 " "
                    R_BRACE@25..26 "}"
            "#]],
        );
    }

    #[test]
    fn ref_to_generic_type() {
        check_expr(
            "{ let x: &Vec(i32); }",
            &expect![[r#"
                BlockExpr@0..21
                  Block@0..21
                    L_BRACE@0..1 "{"
                    LetStmt@1..19
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      RefType@8..18
                        WHITESPACE@8..9 " "
                        AMP@9..10 "&"
                        PathType@10..18
                          Path@10..18
                            PathSegment@10..18
                              NameRef@10..13
                                IDENT@10..13 "Vec"
                              GenericArgs@13..18
                                L_PAREN@13..14 "("
                                PathType@14..17
                                  Path@14..17
                                    PathSegment@14..17
                                      NameRef@14..17
                                        IDENT@14..17 "i32"
                                R_PAREN@17..18 ")"
                      SEMI@18..19 ";"
                    WHITESPACE@19..20 " "
                    R_BRACE@20..21 "}"
            "#]],
        );
    }

    // === Phase 5: Type Annotation Edge Cases ===

    #[test]
    fn ref_to_slice_type() {
        check_expr(
            "{ let x: &[i32]; }",
            &expect![[r#"
                BlockExpr@0..18
                  Block@0..18
                    L_BRACE@0..1 "{"
                    LetStmt@1..16
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      RefType@8..15
                        WHITESPACE@8..9 " "
                        AMP@9..10 "&"
                        SliceType@10..15
                          L_BRACKET@10..11 "["
                          PathType@11..14
                            Path@11..14
                              PathSegment@11..14
                                NameRef@11..14
                                  IDENT@11..14 "i32"
                          R_BRACKET@14..15 "]"
                      SEMI@15..16 ";"
                    WHITESPACE@16..17 " "
                    R_BRACE@17..18 "}"
            "#]],
        );
    }

    #[test]
    fn ref_to_array_type() {
        check_expr(
            "{ let x: &[i32; 5]; }",
            &expect![[r#"
                BlockExpr@0..21
                  Block@0..21
                    L_BRACE@0..1 "{"
                    LetStmt@1..19
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      RefType@8..18
                        WHITESPACE@8..9 " "
                        AMP@9..10 "&"
                        ArrayType@10..18
                          L_BRACKET@10..11 "["
                          PathType@11..14
                            Path@11..14
                              PathSegment@11..14
                                NameRef@11..14
                                  IDENT@11..14 "i32"
                          SEMI@14..15 ";"
                          LiteralExpr@15..17
                            WHITESPACE@15..16 " "
                            INT_LITERAL@16..17 "5"
                          R_BRACKET@17..18 "]"
                      SEMI@18..19 ";"
                    WHITESPACE@19..20 " "
                    R_BRACE@20..21 "}"
            "#]],
        );
    }

    #[test]
    fn array_nested_type() {
        check_expr(
            "{ let x: [[i32; 2]; 3]; }",
            &expect![[r#"
                BlockExpr@0..25
                  Block@0..25
                    L_BRACE@0..1 "{"
                    LetStmt@1..23
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      ArrayType@8..22
                        WHITESPACE@8..9 " "
                        L_BRACKET@9..10 "["
                        ArrayType@10..18
                          L_BRACKET@10..11 "["
                          PathType@11..14
                            Path@11..14
                              PathSegment@11..14
                                NameRef@11..14
                                  IDENT@11..14 "i32"
                          SEMI@14..15 ";"
                          LiteralExpr@15..17
                            WHITESPACE@15..16 " "
                            INT_LITERAL@16..17 "2"
                          R_BRACKET@17..18 "]"
                        SEMI@18..19 ";"
                        LiteralExpr@19..21
                          WHITESPACE@19..20 " "
                          INT_LITERAL@20..21 "3"
                        R_BRACKET@21..22 "]"
                      SEMI@22..23 ";"
                    WHITESPACE@23..24 " "
                    R_BRACE@24..25 "}"
            "#]],
        );
    }

    #[test]
    fn slice_of_tuples_type() {
        check_expr(
            "{ let x: [(i32, i32)]; }",
            &expect![[r#"
                BlockExpr@0..24
                  Block@0..24
                    L_BRACE@0..1 "{"
                    LetStmt@1..22
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      SliceType@8..21
                        WHITESPACE@8..9 " "
                        L_BRACKET@9..10 "["
                        TupleType@10..20
                          L_PAREN@10..11 "("
                          PathType@11..14
                            Path@11..14
                              PathSegment@11..14
                                NameRef@11..14
                                  IDENT@11..14 "i32"
                          COMMA@14..15 ","
                          PathType@15..19
                            Path@15..19
                              PathSegment@15..19
                                NameRef@15..19
                                  WHITESPACE@15..16 " "
                                  IDENT@16..19 "i32"
                          R_PAREN@19..20 ")"
                        R_BRACKET@20..21 "]"
                      SEMI@21..22 ";"
                    WHITESPACE@22..23 " "
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn tuple_empty_type() {
        check_expr(
            "{ let x: (); }",
            &expect![[r#"
                BlockExpr@0..14
                  Block@0..14
                    L_BRACE@0..1 "{"
                    LetStmt@1..12
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      TupleType@8..11
                        WHITESPACE@8..9 " "
                        L_PAREN@9..10 "("
                        R_PAREN@10..11 ")"
                      SEMI@11..12 ";"
                    WHITESPACE@12..13 " "
                    R_BRACE@13..14 "}"
            "#]],
        );
    }

    #[test]
    fn tuple_nested_type() {
        check_expr(
            "{ let x: ((i32, i32), (bool, bool)); }",
            &expect![[r#"
                BlockExpr@0..38
                  Block@0..38
                    L_BRACE@0..1 "{"
                    LetStmt@1..36
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      TupleType@8..35
                        WHITESPACE@8..9 " "
                        L_PAREN@9..10 "("
                        TupleType@10..20
                          L_PAREN@10..11 "("
                          PathType@11..14
                            Path@11..14
                              PathSegment@11..14
                                NameRef@11..14
                                  IDENT@11..14 "i32"
                          COMMA@14..15 ","
                          PathType@15..19
                            Path@15..19
                              PathSegment@15..19
                                NameRef@15..19
                                  WHITESPACE@15..16 " "
                                  IDENT@16..19 "i32"
                          R_PAREN@19..20 ")"
                        COMMA@20..21 ","
                        TupleType@21..34
                          WHITESPACE@21..22 " "
                          L_PAREN@22..23 "("
                          PathType@23..27
                            Path@23..27
                              PathSegment@23..27
                                NameRef@23..27
                                  IDENT@23..27 "bool"
                          COMMA@27..28 ","
                          PathType@28..33
                            Path@28..33
                              PathSegment@28..33
                                NameRef@28..33
                                  WHITESPACE@28..29 " "
                                  IDENT@29..33 "bool"
                          R_PAREN@33..34 ")"
                        R_PAREN@34..35 ")"
                      SEMI@35..36 ";"
                    WHITESPACE@36..37 " "
                    R_BRACE@37..38 "}"
            "#]],
        );
    }

    #[test]
    fn fn_returning_fn_type() {
        check_expr(
            "{ let x: fn() -> fn() -> i32; }",
            &expect![[r#"
                BlockExpr@0..31
                  Block@0..31
                    L_BRACE@0..1 "{"
                    LetStmt@1..29
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      FnPtrType@8..28
                        WHITESPACE@8..9 " "
                        FN_KW@9..11 "fn"
                        L_PAREN@11..12 "("
                        R_PAREN@12..13 ")"
                        WHITESPACE@13..14 " "
                        ARROW@14..16 "->"
                        FnPtrType@16..28
                          WHITESPACE@16..17 " "
                          FN_KW@17..19 "fn"
                          L_PAREN@19..20 "("
                          R_PAREN@20..21 ")"
                          WHITESPACE@21..22 " "
                          ARROW@22..24 "->"
                          PathType@24..28
                            Path@24..28
                              PathSegment@24..28
                                NameRef@24..28
                                  WHITESPACE@24..25 " "
                                  IDENT@25..28 "i32"
                      SEMI@28..29 ";"
                    WHITESPACE@29..30 " "
                    R_BRACE@30..31 "}"
            "#]],
        );
    }

    #[test]
    fn fn_taking_fn_type() {
        check_expr(
            "{ let x: fn(fn(i32) -> bool) -> i32; }",
            &expect![[r#"
                BlockExpr@0..38
                  Block@0..38
                    L_BRACE@0..1 "{"
                    LetStmt@1..36
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      FnPtrType@8..35
                        WHITESPACE@8..9 " "
                        FN_KW@9..11 "fn"
                        L_PAREN@11..12 "("
                        FnPtrType@12..27
                          FN_KW@12..14 "fn"
                          L_PAREN@14..15 "("
                          PathType@15..18
                            Path@15..18
                              PathSegment@15..18
                                NameRef@15..18
                                  IDENT@15..18 "i32"
                          R_PAREN@18..19 ")"
                          WHITESPACE@19..20 " "
                          ARROW@20..22 "->"
                          PathType@22..27
                            Path@22..27
                              PathSegment@22..27
                                NameRef@22..27
                                  WHITESPACE@22..23 " "
                                  IDENT@23..27 "bool"
                        R_PAREN@27..28 ")"
                        WHITESPACE@28..29 " "
                        ARROW@29..31 "->"
                        PathType@31..35
                          Path@31..35
                            PathSegment@31..35
                              NameRef@31..35
                                WHITESPACE@31..32 " "
                                IDENT@32..35 "i32"
                      SEMI@35..36 ";"
                    WHITESPACE@36..37 " "
                    R_BRACE@37..38 "}"
            "#]],
        );
    }

    #[test]
    fn deeply_nested_generics_type() {
        check_expr(
            "{ let x: Result(Option(Vec(T)), Error); }",
            &expect![[r#"
                BlockExpr@0..41
                  Block@0..41
                    L_BRACE@0..1 "{"
                    LetStmt@1..39
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      COLON@7..8 ":"
                      PathType@8..38
                        Path@8..38
                          PathSegment@8..38
                            NameRef@8..15
                              WHITESPACE@8..9 " "
                              IDENT@9..15 "Result"
                            GenericArgs@15..38
                              L_PAREN@15..16 "("
                              PathType@16..30
                                Path@16..30
                                  PathSegment@16..30
                                    NameRef@16..22
                                      IDENT@16..22 "Option"
                                    GenericArgs@22..30
                                      L_PAREN@22..23 "("
                                      PathType@23..29
                                        Path@23..29
                                          PathSegment@23..29
                                            NameRef@23..26
                                              IDENT@23..26 "Vec"
                                            GenericArgs@26..29
                                              L_PAREN@26..27 "("
                                              PathType@27..28
                                                Path@27..28
                                                  PathSegment@27..28
                                                    NameRef@27..28
                                                      IDENT@27..28 "T"
                                              R_PAREN@28..29 ")"
                                      R_PAREN@29..30 ")"
                              COMMA@30..31 ","
                              PathType@31..37
                                Path@31..37
                                  PathSegment@31..37
                                    NameRef@31..37
                                      WHITESPACE@31..32 " "
                                      IDENT@32..37 "Error"
                              R_PAREN@37..38 ")"
                      SEMI@38..39 ";"
                    WHITESPACE@39..40 " "
                    R_BRACE@40..41 "}"
            "#]],
        );
    }
}
