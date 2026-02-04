//! Statement parser for SPL.
//!
//! Parses let statements, expression statements, and blocks.

use crate::{CompletedMarker, Parser};
use spl_syntax::SyntaxKind;

use super::expr;
use super::pattern;

/// Check if expression kind requires a semicolon (statement-like expressions).
/// These expressions act like statements and should not be used as implicit tail expressions.
fn requires_semicolon(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ReturnExpr
            | SyntaxKind::BreakExpr
            | SyntaxKind::ContinueExpr
            | SyntaxKind::YieldExpr
    )
}

/// Get a human-readable name for an expression kind (for error messages).
fn expr_kind_name(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::ReturnExpr => "return",
        SyntaxKind::BreakExpr => "break",
        SyntaxKind::ContinueExpr => "continue",
        SyntaxKind::YieldExpr => "yield",
        _ => "expression",
    }
}

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
                | SyntaxKind::YIELD_KW
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
fn let_stmt(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
pub(crate) fn type_annotation(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
        }
        // Slice type [T]
        p.expect(SyntaxKind::R_BRACKET)?;
        return Ok(m.complete(p, SyntaxKind::SliceType));
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

    // Path type: identifier, Self, super, or path::to::Type<Args>
    // Note: 'crate' keyword was removed - use '$' for package root
    if !p.at(SyntaxKind::IDENT) && !p.at(SyntaxKind::SELF_TYPE_KW) && !p.at(SyntaxKind::SUPER_KW) {
        let err = p.error_at_current("expected type".to_string());
        m.abandon(p);
        return Err(err);
    }

    // Use structured path parsing
    match crate::path::path(p) {
        Ok(_) => Ok(m.complete(p, SyntaxKind::PathType)),
        Err(e) => {
            m.abandon(p);
            Err(e)
        }
    }
}

/// Parse a block with statements: `{ stmt* [expr] }`
pub(crate) fn block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
                    Ok(Some(expr_completed)) => {
                        // Successfully parsed an expression
                        if p.eat(SyntaxKind::SEMI) {
                            // Expression statement with semicolon
                            expr_m.complete(p, SyntaxKind::ExprStmt);
                        } else if p.at(SyntaxKind::R_BRACE) {
                            // At end of block - statement-like expressions require semicolons
                            if requires_semicolon(expr_completed.kind()) {
                                let err = p.error_at_current(format!(
                                    "expected ';' after {} expression",
                                    expr_kind_name(expr_completed.kind())
                                ));
                                p.error(err);
                            }
                            // Regular expressions are valid tail expressions
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
    use crate::tests::check_expr;
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
}
