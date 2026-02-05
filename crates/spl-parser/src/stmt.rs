//! Statement parser for SPL.
//!
//! Parses let statements, expression statements, and blocks.

use crate::{CompletedMarker, Parser};
use spl_syntax::SyntaxKind;

use super::expr;
use super::item;
use super::pattern;

/// Consume optional trailing semicolon.
///
/// Per SPL spec: "Statement terminators are inferred from newlines, but can be
/// explicitly terminated with a semicolon. Semicolons are only required when
/// writing multiple statements on the same line."
///
/// Since the lexer doesn't distinguish newlines from whitespace, we make
/// semicolons universally optional and rely on statement-start detection
/// for recovery.
pub(crate) fn eat_optional_semicolon(p: &mut Parser<'_>) {
    p.eat(SyntaxKind::SEMI);
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
                | SyntaxKind::HASH
        )
    ) || p.current().is_some_and(item::is_item_start)
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

    eat_optional_semicolon(p);
    Ok(m.complete(p, SyntaxKind::LetStmt))
}

/// Parse a tuple type element: either `name: Type` (named) or just `Type` (positional).
///
/// Named elements have the form `IDENT COLON Type`. This is unambiguous because:
/// - Paths use `.` in SPL (e.g., `path.Type`), not `::`
/// - Type annotations always follow a single `:`
fn tuple_type_element(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Check if this is a named element: IDENT followed by COLON
    let is_named = p.at(SyntaxKind::IDENT) && p.peek_at(1, SyntaxKind::COLON);

    if is_named {
        // Named element: name: Type
        crate::item::name(p)?;
        p.expect(SyntaxKind::COLON)?;
        type_annotation(p)?;
    } else {
        // Positional element: just Type
        type_annotation(p)?;
    }

    Ok(m.complete(p, SyntaxKind::TupleTypeElement))
}

/// Parse a type annotation: `BaseType [ "?" ]`
///
/// The optional `?` postfix makes the type optional (sugar for `Option(T: T)`).
pub(crate) fn type_annotation(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let base = base_type(p)?;

    // Optional postfix: T? = Option(T: T)
    if p.at(SyntaxKind::QUESTION) {
        let m = base.precede(p);
        p.bump(); // consume `?`
        return Ok(m.complete(p, SyntaxKind::OptionalType));
    }

    Ok(base)
}

/// Parse a base type (without optional `?` postfix).
fn base_type(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Never type: Never (keyword-like identifier per spec)
    if p.at(SyntaxKind::IDENT) && p.current_text() == Some("Never") {
        p.bump();
        return Ok(m.complete(p, SyntaxKind::NeverType));
    }

    // Reference type: &T or &mut T
    // Also handle &&T where lexer emits AND_AND instead of two AMPs
    if p.at(SyntaxKind::AMP) || p.at(SyntaxKind::AND_AND) {
        if p.at(SyntaxKind::AND_AND) {
            // &&T: split AND_AND into two reference levels
            p.bump(); // consume `&&`
            // Start inner RefType FIRST, so mut lands on inner ref
            let inner_m = p.start();
            // Optional lifetime on inner: &&'a T
            if p.at(SyntaxKind::TICK) {
                let lt_m = p.start();
                p.bump(); // consume '
                p.expect(SyntaxKind::IDENT)?;
                lt_m.complete(p, SyntaxKind::Lifetime);
            }
            p.eat(SyntaxKind::MUT_KW);
            type_annotation(p)?;
            let _inner = inner_m.complete(p, SyntaxKind::RefType);
        } else {
            p.bump(); // consume `&`
            // Optional lifetime: &'a T
            if p.at(SyntaxKind::TICK) {
                let lt_m = p.start();
                p.bump(); // consume '
                p.expect(SyntaxKind::IDENT)?;
                lt_m.complete(p, SyntaxKind::Lifetime);
            }
            p.eat(SyntaxKind::MUT_KW);
            type_annotation(p)?;
        }
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

    // Tuple type: (T1, T2, ...) or (name: T1, ...)
    if p.at(SyntaxKind::L_PAREN) {
        p.bump();
        p.parse_delimited(SyntaxKind::R_PAREN, |p| {
            tuple_type_element(p)?;
            Ok(())
        })?;
        p.expect(SyntaxKind::R_PAREN)?;
        return Ok(m.complete(p, SyntaxKind::TupleType));
    }

    // Function pointer type: fn(T1, T2): R
    // Also handles: unsafe fn(...), extern "C" fn(...), unsafe extern "C" fn(...)
    let is_fn_type = p.at(SyntaxKind::FN_KW)
        || (p.at(SyntaxKind::UNSAFE_KW)
            && matches!(
                p.peek(1),
                Some(SyntaxKind::FN_KW) | Some(SyntaxKind::EXTERN_KW)
            ))
        || (p.at(SyntaxKind::EXTERN_KW)
            && matches!(
                p.peek(1),
                Some(SyntaxKind::STRING_LITERAL) | Some(SyntaxKind::FN_KW)
            ));
    if is_fn_type {
        p.eat(SyntaxKind::UNSAFE_KW);
        if p.eat(SyntaxKind::EXTERN_KW) {
            p.eat(SyntaxKind::STRING_LITERAL);
        }
        p.expect(SyntaxKind::FN_KW)?;
        // Optional lifetime params: fn('a, 'b)(...)
        if p.at(SyntaxKind::L_PAREN) && p.peek(1) == Some(SyntaxKind::TICK) {
            let lt_list = p.start();
            p.bump(); // (
            loop {
                if p.at(SyntaxKind::R_PAREN) || p.current().is_none() {
                    break;
                }
                let lt_m = p.start();
                p.expect(SyntaxKind::TICK)?;
                p.expect(SyntaxKind::IDENT)?;
                lt_m.complete(p, SyntaxKind::Lifetime);
                if !p.eat(SyntaxKind::COMMA) {
                    break;
                }
            }
            p.expect(SyntaxKind::R_PAREN)?;
            lt_list.complete(p, SyntaxKind::LifetimeParams);
        }
        p.expect(SyntaxKind::L_PAREN)?;
        p.parse_delimited(SyntaxKind::R_PAREN, |p| {
            type_annotation(p)?;
            Ok(())
        })?;
        p.expect(SyntaxKind::R_PAREN)?;
        // Optional return type (colon, per spec)
        if p.eat(SyntaxKind::COLON) {
            type_annotation(p)?;
        }
        return Ok(m.complete(p, SyntaxKind::FnPtrType));
    }

    // Path type: identifier, Self, super, or path::to::Type<Args>
    // Note: 'crate' keyword was removed - use '$' for package root
    if !p.at(SyntaxKind::IDENT)
        && !p.at(SyntaxKind::SELF_TYPE_KW)
        && !p.at(SyntaxKind::SUPER_KW)
        && !p.at(SyntaxKind::DOLLAR)
    {
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
            // Nested items: fn, struct, enum, trait, type, impl, use, extern, module, pub, gen
            Some(k) if crate::item::is_nested_item_start(k, p) => {
                if let Err(err) = crate::item::item(p) {
                    p.recover_to_stmt(err);
                }
            }
            Some(_) => {
                // Try to parse an expression
                let expr_m = p.start();
                match expr::expr(p) {
                    Ok(Some(_expr_completed)) => {
                        // Successfully parsed an expression
                        if p.eat(SyntaxKind::SEMI) {
                            // Expression statement with semicolon
                            expr_m.complete(p, SyntaxKind::ExprStmt);
                        } else if p.at(SyntaxKind::R_BRACE) {
                            // At end of block - valid tail expression (semicolons optional per spec)
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
    use crate::tests::{check_expr, check_item};
    use expect_test::expect;

    // === Named Tuple Type Tests ===

    #[test]
    fn tuple_type_positional_unchanged() {
        // Baseline test: existing positional tuple types should still work
        check_item(
            "type T = (i32, bool);",
            &expect![[r#"
                TypeAlias@0..21
                  TYPE_KW@0..4 "type"
                  Name@4..6
                    WHITESPACE@4..5 " "
                    IDENT@5..6 "T"
                  WHITESPACE@6..7 " "
                  EQ@7..8 "="
                  TupleType@8..20
                    WHITESPACE@8..9 " "
                    L_PAREN@9..10 "("
                    TupleTypeElement@10..13
                      PathType@10..13
                        Path@10..13
                          PathSegment@10..13
                            NameRef@10..13
                              IDENT@10..13 "i32"
                    COMMA@13..14 ","
                    TupleTypeElement@14..19
                      PathType@14..19
                        Path@14..19
                          PathSegment@14..19
                            NameRef@14..19
                              WHITESPACE@14..15 " "
                              IDENT@15..19 "bool"
                    R_PAREN@19..20 ")"
                  SEMI@20..21 ";"
            "#]],
        );
    }

    #[test]
    fn tuple_type_named_single() {
        check_item(
            "type T = (name: i32);",
            &expect![[r#"
                TypeAlias@0..21
                  TYPE_KW@0..4 "type"
                  Name@4..6
                    WHITESPACE@4..5 " "
                    IDENT@5..6 "T"
                  WHITESPACE@6..7 " "
                  EQ@7..8 "="
                  TupleType@8..20
                    WHITESPACE@8..9 " "
                    L_PAREN@9..10 "("
                    TupleTypeElement@10..19
                      Name@10..14
                        IDENT@10..14 "name"
                      COLON@14..15 ":"
                      PathType@15..19
                        Path@15..19
                          PathSegment@15..19
                            NameRef@15..19
                              WHITESPACE@15..16 " "
                              IDENT@16..19 "i32"
                    R_PAREN@19..20 ")"
                  SEMI@20..21 ";"
            "#]],
        );
    }

    #[test]
    fn tuple_type_named_multiple() {
        check_item(
            "type Point = (x: i32, y: i32);",
            &expect![[r#"
                TypeAlias@0..30
                  TYPE_KW@0..4 "type"
                  Name@4..10
                    WHITESPACE@4..5 " "
                    IDENT@5..10 "Point"
                  WHITESPACE@10..11 " "
                  EQ@11..12 "="
                  TupleType@12..29
                    WHITESPACE@12..13 " "
                    L_PAREN@13..14 "("
                    TupleTypeElement@14..20
                      Name@14..15
                        IDENT@14..15 "x"
                      COLON@15..16 ":"
                      PathType@16..20
                        Path@16..20
                          PathSegment@16..20
                            NameRef@16..20
                              WHITESPACE@16..17 " "
                              IDENT@17..20 "i32"
                    COMMA@20..21 ","
                    TupleTypeElement@21..28
                      Name@21..23
                        WHITESPACE@21..22 " "
                        IDENT@22..23 "y"
                      COLON@23..24 ":"
                      PathType@24..28
                        Path@24..28
                          PathSegment@24..28
                            NameRef@24..28
                              WHITESPACE@24..25 " "
                              IDENT@25..28 "i32"
                    R_PAREN@28..29 ")"
                  SEMI@29..30 ";"
            "#]],
        );
    }

    #[test]
    fn tuple_type_mixed() {
        // Mixed: first positional, second named
        check_item(
            "type T = (i32, name: String);",
            &expect![[r#"
                TypeAlias@0..29
                  TYPE_KW@0..4 "type"
                  Name@4..6
                    WHITESPACE@4..5 " "
                    IDENT@5..6 "T"
                  WHITESPACE@6..7 " "
                  EQ@7..8 "="
                  TupleType@8..28
                    WHITESPACE@8..9 " "
                    L_PAREN@9..10 "("
                    TupleTypeElement@10..13
                      PathType@10..13
                        Path@10..13
                          PathSegment@10..13
                            NameRef@10..13
                              IDENT@10..13 "i32"
                    COMMA@13..14 ","
                    TupleTypeElement@14..27
                      Name@14..19
                        WHITESPACE@14..15 " "
                        IDENT@15..19 "name"
                      COLON@19..20 ":"
                      PathType@20..27
                        Path@20..27
                          PathSegment@20..27
                            NameRef@20..27
                              WHITESPACE@20..21 " "
                              IDENT@21..27 "String"
                    R_PAREN@27..28 ")"
                  SEMI@28..29 ";"
            "#]],
        );
    }

    #[test]
    fn tuple_type_trailing_comma() {
        check_item(
            "type T = (x: i32,);",
            &expect![[r#"
                TypeAlias@0..19
                  TYPE_KW@0..4 "type"
                  Name@4..6
                    WHITESPACE@4..5 " "
                    IDENT@5..6 "T"
                  WHITESPACE@6..7 " "
                  EQ@7..8 "="
                  TupleType@8..18
                    WHITESPACE@8..9 " "
                    L_PAREN@9..10 "("
                    TupleTypeElement@10..16
                      Name@10..11
                        IDENT@10..11 "x"
                      COLON@11..12 ":"
                      PathType@12..16
                        Path@12..16
                          PathSegment@12..16
                            NameRef@12..16
                              WHITESPACE@12..13 " "
                              IDENT@13..16 "i32"
                    COMMA@16..17 ","
                    R_PAREN@17..18 ")"
                  SEMI@18..19 ";"
            "#]],
        );
    }

    #[test]
    fn tuple_type_path_not_confused_with_named() {
        // path.Type should NOT be parsed as named - it's a qualified path type
        check_item(
            "type T = (path.Type);",
            &expect![[r#"
                TypeAlias@0..21
                  TYPE_KW@0..4 "type"
                  Name@4..6
                    WHITESPACE@4..5 " "
                    IDENT@5..6 "T"
                  WHITESPACE@6..7 " "
                  EQ@7..8 "="
                  TupleType@8..20
                    WHITESPACE@8..9 " "
                    L_PAREN@9..10 "("
                    TupleTypeElement@10..19
                      PathType@10..19
                        Path@10..19
                          PathSegment@10..14
                            NameRef@10..14
                              IDENT@10..14 "path"
                          DOT@14..15 "."
                          PathSegment@15..19
                            NameRef@15..19
                              IDENT@15..19 "Type"
                    R_PAREN@19..20 ")"
                  SEMI@20..21 ";"
            "#]],
        );
    }

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

    #[test]
    fn let_stmt_no_semicolon() {
        // Per spec: semicolons are optional (inferred from newlines)
        check_expr(
            "{ let x = 1 }",
            &expect![[r#"
                BlockExpr@0..13
                  Block@0..13
                    L_BRACE@0..1 "{"
                    LetStmt@1..11
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
                    WHITESPACE@11..12 " "
                    R_BRACE@12..13 "}"
            "#]],
        );
    }

    #[test]
    fn let_stmt_no_semicolon_with_type() {
        check_expr(
            "{ let x: i32 = 1 }",
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
                    WHITESPACE@16..17 " "
                    R_BRACE@17..18 "}"
            "#]],
        );
    }
}
