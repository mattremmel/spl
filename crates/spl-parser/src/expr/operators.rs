//! Operator parsing: prefix, infix, and postfix expressions.
//!
//! This module handles the operator-specific parsing logic within the Pratt
//! parsing framework (see `mod.rs` for the core algorithm). Each operator
//! type has distinct handling:
//!
//! # Prefix Operators
//!
//! Prefix operators (`!`, `-`, `+`, `*`, `&`, `..`) bind to the expression that follows.
//! Special cases:
//! - `&` may be followed by `mut` to form `&mut expr`
//! - `..` creates a range-from expression (`..end`)
//!
//! # Infix Operators
//!
//! Binary operators with an operand on each side. The `r_bp` (right binding power)
//! passed from `expr_bp` determines what can be parsed as the right operand.
//! Special cases:
//! - `as` parses a type on the right side, not an expression
//! - `..` creates a range expression (`start..end`)
//!
//! # Postfix Operators
//!
//! Operators that follow an expression:
//! - `(args)`: function/method call
//! - `[index]`: array/slice indexing
//! - `.field` or `.method()`: field access or method call

use crate::{CompletedMarker, Parser};
use spl_syntax::SyntaxKind;

use super::{expr, expr_bp};

/// Parse a prefix expression.
///
/// Takes `r_bp` (right binding power) to determine how tightly this prefix
/// operator binds to following operators. For example, `-a.b` parses as
/// `-(a.b)` because prefix operators have lower precedence than postfix.
///
/// The `depth` parameter tracks recursion depth to prevent stack overflow.
pub(super) fn prefix_expr(
    p: &mut Parser<'_>,
    r_bp: u8,
    allow_struct: bool,
    depth: usize,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let m = p.start();
    let op = p.current().unwrap();

    // Handle `&` specially: it may be followed by `mut` to form `&mut expr`.
    // We can't treat `&` and `mut` as separate tokens in the binding power
    // table because `mut` isn't an operator - it's a keyword modifier.
    if op == SyntaxKind::AMP {
        p.bump(); // &
        p.eat(SyntaxKind::MUT_KW); // optional mut
        if let Err(e) = expr_bp(p, r_bp, allow_struct, depth) {
            m.abandon(p);
            return Err(e);
        }
        return Ok(Some(m.complete(p, SyntaxKind::RefExpr)));
    }

    // Handle range prefix specially (..expr, ..=expr, .., or ..=)
    if op == SyntaxKind::DOT_DOT || op == SyntaxKind::DOT_DOT_EQ {
        p.bump(); // .. or ..=
        if let Err(e) = expr_bp(p, r_bp, allow_struct, depth) {
            m.abandon(p);
            return Err(e);
        }
        return Ok(Some(m.complete(p, SyntaxKind::RangeExpr)));
    }

    // Regular prefix operator
    p.bump();
    if let Err(e) = expr_bp(p, r_bp, allow_struct, depth) {
        m.abandon(p);
        return Err(e);
    }

    let kind = match op {
        SyntaxKind::BANG
        | SyntaxKind::MINUS
        | SyntaxKind::PLUS
        | SyntaxKind::STAR
        | SyntaxKind::TILDE => SyntaxKind::PrefixExpr,
        _ => unreachable!("unexpected prefix operator: {:?}", op),
    };

    Ok(Some(m.complete(p, kind)))
}

/// Parse an infix expression.
///
/// The `depth` parameter tracks recursion depth to prevent stack overflow.
pub(super) fn infix_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
    r_bp: u8,
    allow_struct: bool,
    depth: usize,
) -> Result<CompletedMarker, crate::ParseError> {
    let m = lhs.precede(p);
    let op = p.current().unwrap();

    // Handle 'as' cast specially
    if op == SyntaxKind::AS_KW {
        p.bump();
        // Parse type (simplified: just an identifier for now)
        if let Err(e) = type_expr(p) {
            m.abandon(p);
            return Err(e);
        }
        return Ok(m.complete(p, SyntaxKind::CastExpr));
    }

    // Handle 'is' pattern matching specially: `expr is Pattern`
    // Note: 'is not' syntax was removed - use `!(expr is Pattern)` instead
    if op == SyntaxKind::IS_KW {
        p.bump(); // is

        // Parse pattern
        if let Err(e) = crate::pattern::pattern(p) {
            m.abandon(p);
            return Err(e);
        }
        return Ok(m.complete(p, SyntaxKind::IsExpr));
    }

    // Regular binary operator
    p.bump();
    if let Err(e) = expr_bp(p, r_bp, allow_struct, depth) {
        m.abandon(p);
        return Err(e);
    }

    // Determine the node kind based on operator
    let kind = match op {
        SyntaxKind::DOT_DOT | SyntaxKind::DOT_DOT_EQ => SyntaxKind::RangeExpr,
        _ => SyntaxKind::BinExpr,
    };

    Ok(m.complete(p, kind))
}

/// Parse a postfix expression.
pub(super) fn postfix_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
    op: SyntaxKind,
) -> Result<CompletedMarker, crate::ParseError> {
    match op {
        SyntaxKind::L_PAREN => call_expr(p, lhs),
        SyntaxKind::L_BRACKET => index_or_slice_expr(p, lhs),
        SyntaxKind::DOT => field_or_method_expr(p, lhs),
        SyntaxKind::QUESTION_DOT => optional_field_expr(p, lhs),
        SyntaxKind::BANG => try_expr(p, lhs),
        _ => unreachable!("unexpected postfix operator: {:?}", op),
    }
}

/// Parse try/propagate expression: expr!
#[allow(clippy::unnecessary_wraps)] // Consistent with other postfix_expr handlers
fn try_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::ParseError> {
    let m = lhs.precede(p);
    p.bump(); // consume !
    Ok(m.complete(p, SyntaxKind::TryExpr))
}

/// Parse optional field access: `expr?.field`
/// On None, short-circuits to None. On `Some(v)`, accesses field on v.
/// Method calls work naturally: `expr?.method()` becomes `CallExpr(OptionalFieldExpr, args)`.
fn optional_field_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::ParseError> {
    let m = lhs.precede(p);
    if let Err(e) = p.expect(SyntaxKind::QUESTION_DOT) {
        m.abandon(p);
        return Err(e);
    }

    // Must be followed by an identifier (field or method name)
    if !p.at(SyntaxKind::IDENT) {
        m.abandon(p);
        return Err(p.error_at_current("expected identifier after '?.'".to_string()));
    }
    p.bump(); // consume identifier

    // Complete as OptionalFieldExpr
    let optional_field = m.complete(p, SyntaxKind::OptionalFieldExpr);

    // Check for method call - if followed by (, wrap in CallExpr
    if p.at(SyntaxKind::L_PAREN) {
        call_expr(p, optional_field)
    } else {
        Ok(optional_field)
    }
}

/// Parse a call expression: expr(args)
/// Uses the unified `call_expr_rest` for parsing arguments with named arg support.
fn call_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::ParseError> {
    let m = lhs.precede(p);
    // call_expr_rest expects the marker and handles L_PAREN, args, R_PAREN
    match super::primary::call_expr_rest(p, m) {
        Ok(Some(cm)) => Ok(cm),
        Ok(None) => unreachable!("call_expr_rest should always return Some"),
        Err(e) => Err(e),
    }
}

/// Parse index or slice expression: expr[idx] or expr[start:end]
fn index_or_slice_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::ParseError> {
    let m = lhs.precede(p);
    if let Err(e) = p.expect(SyntaxKind::L_BRACKET) {
        m.abandon(p);
        return Err(e);
    }

    // Parse optional start expression (skip if immediately at colon)
    let is_slice = p.at(SyntaxKind::COLON);
    if !is_slice && let Err(e) = expr(p) {
        m.abandon(p);
        return Err(e);
    }

    // Determine if slice (has colon) or index (no colon)
    if is_slice || p.at(SyntaxKind::COLON) {
        p.bump(); // consume :

        // Parse optional end expression ($ is now handled as DollarExpr via expr())
        if !p.at(SyntaxKind::R_BRACKET)
            && let Err(e) = expr(p)
        {
            m.abandon(p);
            return Err(e);
        }
        if let Err(e) = p.expect(SyntaxKind::R_BRACKET) {
            m.abandon(p);
            return Err(e);
        }
        Ok(m.complete(p, SyntaxKind::SliceExpr))
    } else {
        if let Err(e) = p.expect(SyntaxKind::R_BRACKET) {
            m.abandon(p);
            return Err(e);
        }
        Ok(m.complete(p, SyntaxKind::IndexExpr))
    }
}

/// Parse field access or method call: expr.field or expr.method(args) or expr.0 (tuple)
/// Method calls are now unified as `CallExpr` with `FieldExpr` as callee.
fn field_or_method_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::ParseError> {
    let m = lhs.precede(p);
    if let Err(e) = p.expect(SyntaxKind::DOT) {
        m.abandon(p);
        return Err(e);
    }

    // Accept identifier or integer literal (for tuple field access like t.0)
    if p.at(SyntaxKind::IDENT) {
        p.bump();
        // First complete as FieldExpr
        let field_expr = m.complete(p, SyntaxKind::FieldExpr);
        // Check for method call - if followed by (, wrap in CallExpr
        if p.at(SyntaxKind::L_PAREN) {
            call_expr(p, field_expr)
        } else {
            Ok(field_expr)
        }
    } else if p.at(SyntaxKind::INT_LITERAL) {
        // Tuple field access: t.0, t.1, etc.
        p.bump();
        Ok(m.complete(p, SyntaxKind::FieldExpr))
    } else {
        m.abandon(p);
        Err(p.error_at_current("expected identifier or integer after '.'".to_string()))
    }
}

/// Parse a type expression (simplified for cast).
fn type_expr(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Simplified: just parse an identifier path (no generics for now)
    if !p.at(SyntaxKind::IDENT) {
        m.abandon(p);
        return Err(p.error_at_current("expected type".to_string()));
    }

    // Use structured path parsing (no generics for cast expressions)
    if let Err(e) = crate::path::path_no_generics(p) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::PathType))
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::check_expr;
    use expect_test::expect;

    #[test]
    fn prefix_negation() {
        check_expr(
            "-42",
            &expect![[r#"
                PrefixExpr@0..3
                  MINUS@0..1 "-"
                  LiteralExpr@1..3
                    INT_LITERAL@1..3 "42"
            "#]],
        );
    }

    #[test]
    fn prefix_not() {
        check_expr(
            "!true",
            &expect![[r#"
                PrefixExpr@0..5
                  BANG@0..1 "!"
                  LiteralExpr@1..5
                    TRUE_KW@1..5 "true"
            "#]],
        );
    }

    #[test]
    fn reference_expr() {
        check_expr(
            "&x",
            &expect![[r#"
                RefExpr@0..2
                  AMP@0..1 "&"
                  PathExpr@1..2
                    Path@1..2
                      PathSegment@1..2
                        NameRef@1..2
                          IDENT@1..2 "x"
            "#]],
        );
    }

    #[test]
    fn mutable_reference_expr() {
        check_expr(
            "&mut x",
            &expect![[r#"
                RefExpr@0..6
                  AMP@0..1 "&"
                  MUT_KW@1..4 "mut"
                  PathExpr@4..6
                    Path@4..6
                      PathSegment@4..6
                        NameRef@4..6
                          WHITESPACE@4..5 " "
                          IDENT@5..6 "x"
            "#]],
        );
    }

    #[test]
    fn prefix_on_paren() {
        check_expr(
            "-(a + b)",
            &expect![[r#"
                PrefixExpr@0..8
                  MINUS@0..1 "-"
                  ParenExpr@1..8
                    L_PAREN@1..2 "("
                    BinExpr@2..7
                      PathExpr@2..3
                        Path@2..3
                          PathSegment@2..3
                            NameRef@2..3
                              IDENT@2..3 "a"
                      WHITESPACE@3..4 " "
                      PLUS@4..5 "+"
                      PathExpr@5..7
                        Path@5..7
                          PathSegment@5..7
                            NameRef@5..7
                              WHITESPACE@5..6 " "
                              IDENT@6..7 "b"
                    R_PAREN@7..8 ")"
            "#]],
        );
    }

    #[test]
    fn ref_paren() {
        check_expr(
            "&(a + b)",
            &expect![[r#"
                RefExpr@0..8
                  AMP@0..1 "&"
                  ParenExpr@1..8
                    L_PAREN@1..2 "("
                    BinExpr@2..7
                      PathExpr@2..3
                        Path@2..3
                          PathSegment@2..3
                            NameRef@2..3
                              IDENT@2..3 "a"
                      WHITESPACE@3..4 " "
                      PLUS@4..5 "+"
                      PathExpr@5..7
                        Path@5..7
                          PathSegment@5..7
                            NameRef@5..7
                              WHITESPACE@5..6 " "
                              IDENT@6..7 "b"
                    R_PAREN@7..8 ")"
            "#]],
        );
    }

    #[test]
    fn prefix_on_call() {
        check_expr(
            "-foo()",
            &expect![[r#"
                PrefixExpr@0..6
                  MINUS@0..1 "-"
                  CallExpr@1..6
                    PathExpr@1..4
                      Path@1..4
                        PathSegment@1..4
                          NameRef@1..4
                            IDENT@1..4 "foo"
                    L_PAREN@4..5 "("
                    R_PAREN@5..6 ")"
            "#]],
        );
    }

    #[test]
    fn ref_field() {
        check_expr(
            "&obj.field",
            &expect![[r#"
                RefExpr@0..10
                  AMP@0..1 "&"
                  PathExpr@1..10
                    Path@1..10
                      PathSegment@1..4
                        NameRef@1..4
                          IDENT@1..4 "obj"
                      DOT@4..5 "."
                      PathSegment@5..10
                        NameRef@5..10
                          IDENT@5..10 "field"
            "#]],
        );
    }

    #[test]
    fn call_expr() {
        check_expr(
            "foo(1, 2)",
            &expect![[r#"
                CallExpr@0..9
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "foo"
                  L_PAREN@3..4 "("
                  CallArg@4..5
                    LiteralExpr@4..5
                      INT_LITERAL@4..5 "1"
                  COMMA@5..6 ","
                  CallArg@6..8
                    LiteralExpr@6..8
                      WHITESPACE@6..7 " "
                      INT_LITERAL@7..8 "2"
                  R_PAREN@8..9 ")"
            "#]],
        );
    }

    #[test]
    fn call_trailing_comma() {
        check_expr(
            "foo(a, b,)",
            &expect![[r#"
                CallExpr@0..10
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "foo"
                  L_PAREN@3..4 "("
                  CallArg@4..5
                    PathExpr@4..5
                      Path@4..5
                        PathSegment@4..5
                          NameRef@4..5
                            IDENT@4..5 "a"
                  COMMA@5..6 ","
                  CallArg@6..8
                    PathExpr@6..8
                      Path@6..8
                        PathSegment@6..8
                          NameRef@6..8
                            WHITESPACE@6..7 " "
                            IDENT@7..8 "b"
                  COMMA@8..9 ","
                  R_PAREN@9..10 ")"
            "#]],
        );
    }

    #[test]
    fn call_nested() {
        check_expr(
            "foo(bar(baz()))",
            &expect![[r#"
                CallExpr@0..15
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "foo"
                  L_PAREN@3..4 "("
                  CallArg@4..14
                    CallExpr@4..14
                      PathExpr@4..7
                        Path@4..7
                          PathSegment@4..7
                            NameRef@4..7
                              IDENT@4..7 "bar"
                      L_PAREN@7..8 "("
                      CallArg@8..13
                        CallExpr@8..13
                          PathExpr@8..11
                            Path@8..11
                              PathSegment@8..11
                                NameRef@8..11
                                  IDENT@8..11 "baz"
                          L_PAREN@11..12 "("
                          R_PAREN@12..13 ")"
                      R_PAREN@13..14 ")"
                  R_PAREN@14..15 ")"
            "#]],
        );
    }

    #[test]
    fn call_on_paren_expr() {
        check_expr(
            "(get_fn())(arg)",
            &expect![[r#"
                CallExpr@0..15
                  ParenExpr@0..10
                    L_PAREN@0..1 "("
                    CallExpr@1..9
                      PathExpr@1..7
                        Path@1..7
                          PathSegment@1..7
                            NameRef@1..7
                              IDENT@1..7 "get_fn"
                      L_PAREN@7..8 "("
                      R_PAREN@8..9 ")"
                    R_PAREN@9..10 ")"
                  L_PAREN@10..11 "("
                  CallArg@11..14
                    PathExpr@11..14
                      Path@11..14
                        PathSegment@11..14
                          NameRef@11..14
                            IDENT@11..14 "arg"
                  R_PAREN@14..15 ")"
            "#]],
        );
    }

    #[test]
    fn field_expr() {
        check_expr(
            "point.x",
            &expect![[r#"
                PathExpr@0..7
                  Path@0..7
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "point"
                    DOT@5..6 "."
                    PathSegment@6..7
                      NameRef@6..7
                        IDENT@6..7 "x"
            "#]],
        );
    }

    #[test]
    fn method_call_expr() {
        check_expr(
            "point.distance()",
            &expect![[r#"
                CallExpr@0..16
                  PathExpr@0..14
                    Path@0..14
                      PathSegment@0..5
                        NameRef@0..5
                          IDENT@0..5 "point"
                      DOT@5..6 "."
                      PathSegment@6..14
                        NameRef@6..14
                          IDENT@6..14 "distance"
                  L_PAREN@14..15 "("
                  R_PAREN@15..16 ")"
            "#]],
        );
    }

    #[test]
    fn field_chain() {
        check_expr(
            "obj.a.b.c",
            &expect![[r#"
                PathExpr@0..9
                  Path@0..9
                    PathSegment@0..3
                      NameRef@0..3
                        IDENT@0..3 "obj"
                    DOT@3..4 "."
                    PathSegment@4..5
                      NameRef@4..5
                        IDENT@4..5 "a"
                    DOT@5..6 "."
                    PathSegment@6..7
                      NameRef@6..7
                        IDENT@6..7 "b"
                    DOT@7..8 "."
                    PathSegment@8..9
                      NameRef@8..9
                        IDENT@8..9 "c"
            "#]],
        );
    }

    #[test]
    fn field_on_call() {
        check_expr(
            "get_obj().field",
            &expect![[r#"
                FieldExpr@0..15
                  CallExpr@0..9
                    PathExpr@0..7
                      Path@0..7
                        PathSegment@0..7
                          NameRef@0..7
                            IDENT@0..7 "get_obj"
                    L_PAREN@7..8 "("
                    R_PAREN@8..9 ")"
                  DOT@9..10 "."
                  IDENT@10..15 "field"
            "#]],
        );
    }

    #[test]
    fn chained_method_calls() {
        check_expr(
            "obj.a().b().c()",
            &expect![[r#"
                CallExpr@0..15
                  FieldExpr@0..13
                    CallExpr@0..11
                      FieldExpr@0..9
                        CallExpr@0..7
                          PathExpr@0..5
                            Path@0..5
                              PathSegment@0..3
                                NameRef@0..3
                                  IDENT@0..3 "obj"
                              DOT@3..4 "."
                              PathSegment@4..5
                                NameRef@4..5
                                  IDENT@4..5 "a"
                          L_PAREN@5..6 "("
                          R_PAREN@6..7 ")"
                        DOT@7..8 "."
                        IDENT@8..9 "b"
                      L_PAREN@9..10 "("
                      R_PAREN@10..11 ")"
                    DOT@11..12 "."
                    IDENT@12..13 "c"
                  L_PAREN@13..14 "("
                  R_PAREN@14..15 ")"
            "#]],
        );
    }

    #[test]
    fn index_expr() {
        check_expr(
            "arr[0]",
            &expect![[r#"
                IndexExpr@0..6
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  LiteralExpr@4..5
                    INT_LITERAL@4..5 "0"
                  R_BRACKET@5..6 "]"
            "#]],
        );
    }

    #[test]
    fn index_with_expr() {
        check_expr(
            "arr[i + 1]",
            &expect![[r#"
                IndexExpr@0..10
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  BinExpr@4..9
                    PathExpr@4..5
                      Path@4..5
                        PathSegment@4..5
                          NameRef@4..5
                            IDENT@4..5 "i"
                    WHITESPACE@5..6 " "
                    PLUS@6..7 "+"
                    LiteralExpr@7..9
                      WHITESPACE@7..8 " "
                      INT_LITERAL@8..9 "1"
                  R_BRACKET@9..10 "]"
            "#]],
        );
    }

    #[test]
    fn index_chained() {
        check_expr(
            "arr[0][1]",
            &expect![[r#"
                IndexExpr@0..9
                  IndexExpr@0..6
                    PathExpr@0..3
                      Path@0..3
                        PathSegment@0..3
                          NameRef@0..3
                            IDENT@0..3 "arr"
                    L_BRACKET@3..4 "["
                    LiteralExpr@4..5
                      INT_LITERAL@4..5 "0"
                    R_BRACKET@5..6 "]"
                  L_BRACKET@6..7 "["
                  LiteralExpr@7..8
                    INT_LITERAL@7..8 "1"
                  R_BRACKET@8..9 "]"
            "#]],
        );
    }

    #[test]
    fn index_on_field() {
        check_expr(
            "obj.arr[0]",
            &expect![[r#"
                IndexExpr@0..10
                  PathExpr@0..7
                    Path@0..7
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "obj"
                      DOT@3..4 "."
                      PathSegment@4..7
                        NameRef@4..7
                          IDENT@4..7 "arr"
                  L_BRACKET@7..8 "["
                  LiteralExpr@8..9
                    INT_LITERAL@8..9 "0"
                  R_BRACKET@9..10 "]"
            "#]],
        );
    }

    #[test]
    fn chained_index_and_field() {
        check_expr(
            "arr[0].field",
            &expect![[r#"
                FieldExpr@0..12
                  IndexExpr@0..6
                    PathExpr@0..3
                      Path@0..3
                        PathSegment@0..3
                          NameRef@0..3
                            IDENT@0..3 "arr"
                    L_BRACKET@3..4 "["
                    LiteralExpr@4..5
                      INT_LITERAL@4..5 "0"
                    R_BRACKET@5..6 "]"
                  DOT@6..7 "."
                  IDENT@7..12 "field"
            "#]],
        );
    }

    #[test]
    fn slice_full() {
        check_expr(
            "arr[:]",
            &expect![[r#"
                SliceExpr@0..6
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  COLON@4..5 ":"
                  R_BRACKET@5..6 "]"
            "#]],
        );
    }

    #[test]
    fn slice_from_start() {
        check_expr(
            "arr[:5]",
            &expect![[r#"
                SliceExpr@0..7
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  COLON@4..5 ":"
                  LiteralExpr@5..6
                    INT_LITERAL@5..6 "5"
                  R_BRACKET@6..7 "]"
            "#]],
        );
    }

    #[test]
    fn slice_from_end() {
        check_expr(
            "arr[2:]",
            &expect![[r#"
                SliceExpr@0..7
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  LiteralExpr@4..5
                    INT_LITERAL@4..5 "2"
                  COLON@5..6 ":"
                  R_BRACKET@6..7 "]"
            "#]],
        );
    }

    #[test]
    fn slice_bounded() {
        check_expr(
            "arr[1:3]",
            &expect![[r#"
                SliceExpr@0..8
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  LiteralExpr@4..5
                    INT_LITERAL@4..5 "1"
                  COLON@5..6 ":"
                  LiteralExpr@6..7
                    INT_LITERAL@6..7 "3"
                  R_BRACKET@7..8 "]"
            "#]],
        );
    }

    #[test]
    fn slice_with_exprs() {
        check_expr(
            "arr[i:j]",
            &expect![[r#"
                SliceExpr@0..8
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  PathExpr@4..5
                    Path@4..5
                      PathSegment@4..5
                        NameRef@4..5
                          IDENT@4..5 "i"
                  COLON@5..6 ":"
                  PathExpr@6..7
                    Path@6..7
                      PathSegment@6..7
                        NameRef@6..7
                          IDENT@6..7 "j"
                  R_BRACKET@7..8 "]"
            "#]],
        );
    }

    #[test]
    fn path_expr() {
        check_expr(
            "std.vec.Vec",
            &expect![[r#"
                PathExpr@0..11
                  Path@0..11
                    PathSegment@0..3
                      NameRef@0..3
                        IDENT@0..3 "std"
                    DOT@3..4 "."
                    PathSegment@4..7
                      NameRef@4..7
                        IDENT@4..7 "vec"
                    DOT@7..8 "."
                    PathSegment@8..11
                      NameRef@8..11
                        IDENT@8..11 "Vec"
            "#]],
        );
    }

    #[test]
    fn range_expr() {
        check_expr(
            "0..10",
            &expect![[r#"
                RangeExpr@0..5
                  LiteralExpr@0..1
                    INT_LITERAL@0..1 "0"
                  DOT_DOT@1..3 ".."
                  LiteralExpr@3..5
                    INT_LITERAL@3..5 "10"
            "#]],
        );
    }

    #[test]
    fn cast_expr() {
        check_expr(
            "42 as f64",
            &expect![[r#"
                CastExpr@0..9
                  LiteralExpr@0..2
                    INT_LITERAL@0..2 "42"
                  WHITESPACE@2..3 " "
                  AS_KW@3..5 "as"
                  PathType@5..9
                    Path@5..9
                      PathSegment@5..9
                        NameRef@5..9
                          WHITESPACE@5..6 " "
                          IDENT@6..9 "f64"
            "#]],
        );
    }

    #[test]
    fn precedence_cast_vs_arithmetic() {
        check_expr(
            "a as i32 + b",
            &expect![[r#"
                BinExpr@0..12
                  CastExpr@0..8
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "a"
                    WHITESPACE@1..2 " "
                    AS_KW@2..4 "as"
                    PathType@4..8
                      Path@4..8
                        PathSegment@4..8
                          NameRef@4..8
                            WHITESPACE@4..5 " "
                            IDENT@5..8 "i32"
                  WHITESPACE@8..9 " "
                  PLUS@9..10 "+"
                  PathExpr@10..12
                    Path@10..12
                      PathSegment@10..12
                        NameRef@10..12
                          WHITESPACE@10..11 " "
                          IDENT@11..12 "b"
            "#]],
        );
    }

    #[test]
    fn logical_or_expr() {
        check_expr(
            "a || b",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  OR_OR@2..4 "||"
                  PathExpr@4..6
                    Path@4..6
                      PathSegment@4..6
                        NameRef@4..6
                          WHITESPACE@4..5 " "
                          IDENT@5..6 "b"
            "#]],
        );
    }

    #[test]
    fn not_equal_expr() {
        check_expr(
            "a != b",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  NE@2..4 "!="
                  PathExpr@4..6
                    Path@4..6
                      PathSegment@4..6
                        NameRef@4..6
                          WHITESPACE@4..5 " "
                          IDENT@5..6 "b"
            "#]],
        );
    }

    #[test]
    fn less_equal_expr() {
        check_expr(
            "a <= b",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  LE@2..4 "<="
                  PathExpr@4..6
                    Path@4..6
                      PathSegment@4..6
                        NameRef@4..6
                          WHITESPACE@4..5 " "
                          IDENT@5..6 "b"
            "#]],
        );
    }

    #[test]
    fn greater_equal_expr() {
        check_expr(
            "a >= b",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  GE@2..4 ">="
                  PathExpr@4..6
                    Path@4..6
                      PathSegment@4..6
                        NameRef@4..6
                          WHITESPACE@4..5 " "
                          IDENT@5..6 "b"
            "#]],
        );
    }

    #[test]
    fn division_expr() {
        check_expr(
            "10 / 2",
            &expect![[r#"
                BinExpr@0..6
                  LiteralExpr@0..2
                    INT_LITERAL@0..2 "10"
                  WHITESPACE@2..3 " "
                  SLASH@3..4 "/"
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "2"
            "#]],
        );
    }

    #[test]
    fn modulo_expr() {
        check_expr(
            "10 % 3",
            &expect![[r#"
                BinExpr@0..6
                  LiteralExpr@0..2
                    INT_LITERAL@0..2 "10"
                  WHITESPACE@2..3 " "
                  PERCENT@3..4 "%"
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "3"
            "#]],
        );
    }

    #[test]
    fn plus_assign_expr() {
        check_expr(
            "x += 1",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  PLUS_EQ@2..4 "+="
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "1"
            "#]],
        );
    }

    #[test]
    fn minus_assign_expr() {
        check_expr(
            "x -= 1",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  MINUS_EQ@2..4 "-="
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "1"
            "#]],
        );
    }

    #[test]
    fn star_assign_expr() {
        check_expr(
            "x *= 2",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  STAR_EQ@2..4 "*="
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "2"
            "#]],
        );
    }

    #[test]
    fn slash_assign_expr() {
        check_expr(
            "x /= 2",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  SLASH_EQ@2..4 "/="
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "2"
            "#]],
        );
    }

    #[test]
    fn percent_assign_expr() {
        check_expr(
            "x %= 3",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  PERCENT_EQ@2..4 "%="
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "3"
            "#]],
        );
    }

    #[test]
    fn range_to_expr() {
        check_expr(
            "..10",
            &expect![[r#"
                RangeExpr@0..4
                  DOT_DOT@0..2 ".."
                  LiteralExpr@2..4
                    INT_LITERAL@2..4 "10"
            "#]],
        );
    }

    #[test]
    fn range_full_expr() {
        check_expr(
            "..",
            &expect![[r#"
                RangeExpr@0..2
                  DOT_DOT@0..2 ".."
            "#]],
        );
    }

    #[test]
    fn range_to_with_path() {
        check_expr(
            "..end",
            &expect![[r#"
                RangeExpr@0..5
                  DOT_DOT@0..2 ".."
                  PathExpr@2..5
                    Path@2..5
                      PathSegment@2..5
                        NameRef@2..5
                          IDENT@2..5 "end"
            "#]],
        );
    }

    // === Operator Precedence Edge Cases ===

    #[test]
    fn precedence_range_vs_arithmetic() {
        // Range has lower precedence than arithmetic: 1+2..3+4 = (1+2)..(3+4)
        check_expr(
            "1+2..3+4",
            &expect![[r#"
                RangeExpr@0..8
                  BinExpr@0..3
                    LiteralExpr@0..1
                      INT_LITERAL@0..1 "1"
                    PLUS@1..2 "+"
                    LiteralExpr@2..3
                      INT_LITERAL@2..3 "2"
                  DOT_DOT@3..5 ".."
                  BinExpr@5..8
                    LiteralExpr@5..6
                      INT_LITERAL@5..6 "3"
                    PLUS@6..7 "+"
                    LiteralExpr@7..8
                      INT_LITERAL@7..8 "4"
            "#]],
        );
    }

    #[test]
    fn precedence_assignment_chain() {
        // Assignment is right associative: a = b = c = 1 = a = (b = (c = 1))
        check_expr(
            "a = b = c = 1",
            &expect![[r#"
                BinExpr@0..13
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  EQ@2..3 "="
                  BinExpr@3..13
                    PathExpr@3..5
                      Path@3..5
                        PathSegment@3..5
                          NameRef@3..5
                            WHITESPACE@3..4 " "
                            IDENT@4..5 "b"
                    WHITESPACE@5..6 " "
                    EQ@6..7 "="
                    BinExpr@7..13
                      PathExpr@7..9
                        Path@7..9
                          PathSegment@7..9
                            NameRef@7..9
                              WHITESPACE@7..8 " "
                              IDENT@8..9 "c"
                      WHITESPACE@9..10 " "
                      EQ@10..11 "="
                      LiteralExpr@11..13
                        WHITESPACE@11..12 " "
                        INT_LITERAL@12..13 "1"
            "#]],
        );
    }

    #[test]
    fn precedence_mixed_logical_comparison() {
        // a && b == c || d = (a && (b == c)) || d
        check_expr(
            "a && b == c || d",
            &expect![[r#"
                BinExpr@0..16
                  BinExpr@0..11
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "a"
                    WHITESPACE@1..2 " "
                    AND_AND@2..4 "&&"
                    BinExpr@4..11
                      PathExpr@4..6
                        Path@4..6
                          PathSegment@4..6
                            NameRef@4..6
                              WHITESPACE@4..5 " "
                              IDENT@5..6 "b"
                      WHITESPACE@6..7 " "
                      EQ_EQ@7..9 "=="
                      PathExpr@9..11
                        Path@9..11
                          PathSegment@9..11
                            NameRef@9..11
                              WHITESPACE@9..10 " "
                              IDENT@10..11 "c"
                  WHITESPACE@11..12 " "
                  OR_OR@12..14 "||"
                  PathExpr@14..16
                    Path@14..16
                      PathSegment@14..16
                        NameRef@14..16
                          WHITESPACE@14..15 " "
                          IDENT@15..16 "d"
            "#]],
        );
    }

    #[test]
    fn precedence_comparison_vs_arithmetic() {
        // a + b < c * d = (a + b) < (c * d)
        check_expr(
            "a + b < c * d",
            &expect![[r#"
                BinExpr@0..13
                  BinExpr@0..5
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "a"
                    WHITESPACE@1..2 " "
                    PLUS@2..3 "+"
                    PathExpr@3..5
                      Path@3..5
                        PathSegment@3..5
                          NameRef@3..5
                            WHITESPACE@3..4 " "
                            IDENT@4..5 "b"
                  WHITESPACE@5..6 " "
                  LT@6..7 "<"
                  BinExpr@7..13
                    PathExpr@7..9
                      Path@7..9
                        PathSegment@7..9
                          NameRef@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "c"
                    WHITESPACE@9..10 " "
                    STAR@10..11 "*"
                    PathExpr@11..13
                      Path@11..13
                        PathSegment@11..13
                          NameRef@11..13
                            WHITESPACE@11..12 " "
                            IDENT@12..13 "d"
            "#]],
        );
    }

    #[test]
    fn precedence_mul_vs_add() {
        // Multiplication has higher precedence than addition: a + b * c = a + (b * c)
        check_expr(
            "a + b * c",
            &expect![[r#"
                BinExpr@0..9
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  PLUS@2..3 "+"
                  BinExpr@3..9
                    PathExpr@3..5
                      Path@3..5
                        PathSegment@3..5
                          NameRef@3..5
                            WHITESPACE@3..4 " "
                            IDENT@4..5 "b"
                    WHITESPACE@5..6 " "
                    STAR@6..7 "*"
                    PathExpr@7..9
                      Path@7..9
                        PathSegment@7..9
                          NameRef@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "c"
            "#]],
        );
    }

    #[test]
    fn precedence_postfix_vs_prefix() {
        // Postfix operations bind tighter than prefix: -arr[0] = -(arr[0])
        check_expr(
            "-arr[0]",
            &expect![[r#"
                PrefixExpr@0..7
                  MINUS@0..1 "-"
                  IndexExpr@1..7
                    PathExpr@1..4
                      Path@1..4
                        PathSegment@1..4
                          NameRef@1..4
                            IDENT@1..4 "arr"
                    L_BRACKET@4..5 "["
                    LiteralExpr@5..6
                      INT_LITERAL@5..6 "0"
                    R_BRACKET@6..7 "]"
            "#]],
        );
    }

    #[test]
    fn precedence_cast_chain() {
        // Cast chains left to right: a as i32 as f64
        check_expr(
            "a as i32 as f64",
            &expect![[r#"
                CastExpr@0..15
                  CastExpr@0..8
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "a"
                    WHITESPACE@1..2 " "
                    AS_KW@2..4 "as"
                    PathType@4..8
                      Path@4..8
                        PathSegment@4..8
                          NameRef@4..8
                            WHITESPACE@4..5 " "
                            IDENT@5..8 "i32"
                  WHITESPACE@8..9 " "
                  AS_KW@9..11 "as"
                  PathType@11..15
                    Path@11..15
                      PathSegment@11..15
                        NameRef@11..15
                          WHITESPACE@11..12 " "
                          IDENT@12..15 "f64"
            "#]],
        );
    }

    #[test]
    fn precedence_unary_chain() {
        // Multiple prefix operators: --x, !!b
        check_expr(
            "!!true",
            &expect![[r#"
                PrefixExpr@0..6
                  BANG@0..1 "!"
                  PrefixExpr@1..6
                    BANG@1..2 "!"
                    LiteralExpr@2..6
                      TRUE_KW@2..6 "true"
            "#]],
        );
    }

    // === New Syntax: `is` Expression ===

    #[test]
    fn is_expr_simple() {
        check_expr(
            "x is Some",
            &expect![[r#"
                IsExpr@0..9
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  IS_KW@2..4 "is"
                  IdentPat@4..9
                    Name@4..9
                      WHITESPACE@4..5 " "
                      IDENT@5..9 "Some"
            "#]],
        );
    }

    #[test]
    fn is_expr_with_binding() {
        check_expr(
            "x is Some(v)",
            &expect![[r#"
                IsExpr@0..12
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  IS_KW@2..4 "is"
                  TuplePat@4..12
                    Path@4..9
                      PathSegment@4..9
                        NameRef@4..9
                          WHITESPACE@4..5 " "
                          IDENT@5..9 "Some"
                    L_PAREN@9..10 "("
                    IdentPat@10..11
                      Name@10..11
                        IDENT@10..11 "v"
                    R_PAREN@11..12 ")"
            "#]],
        );
    }

    #[test]
    fn is_expr_combined_with_and() {
        // x is Some(v) && v > 0
        check_expr(
            "x is Some(v) && v > 0",
            &expect![[r#"
                BinExpr@0..21
                  IsExpr@0..12
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "x"
                    WHITESPACE@1..2 " "
                    IS_KW@2..4 "is"
                    TuplePat@4..12
                      Path@4..9
                        PathSegment@4..9
                          NameRef@4..9
                            WHITESPACE@4..5 " "
                            IDENT@5..9 "Some"
                      L_PAREN@9..10 "("
                      IdentPat@10..11
                        Name@10..11
                          IDENT@10..11 "v"
                      R_PAREN@11..12 ")"
                  WHITESPACE@12..13 " "
                  AND_AND@13..15 "&&"
                  BinExpr@15..21
                    PathExpr@15..17
                      Path@15..17
                        PathSegment@15..17
                          NameRef@15..17
                            WHITESPACE@15..16 " "
                            IDENT@16..17 "v"
                    WHITESPACE@17..18 " "
                    GT@18..19 ">"
                    LiteralExpr@19..21
                      WHITESPACE@19..20 " "
                      INT_LITERAL@20..21 "0"
            "#]],
        );
    }

    // === Bitwise Operators ===

    #[test]
    fn prefix_bitwise_not() {
        check_expr(
            "~42",
            &expect![[r#"
                PrefixExpr@0..3
                  TILDE@0..1 "~"
                  LiteralExpr@1..3
                    INT_LITERAL@1..3 "42"
            "#]],
        );
    }

    #[test]
    fn prefix_double_tilde() {
        check_expr(
            "~~x",
            &expect![[r#"
                PrefixExpr@0..3
                  TILDE@0..1 "~"
                  PrefixExpr@1..3
                    TILDE@1..2 "~"
                    PathExpr@2..3
                      Path@2..3
                        PathSegment@2..3
                          NameRef@2..3
                            IDENT@2..3 "x"
            "#]],
        );
    }

    // === Try/Propagate Operator (postfix !) ===

    #[test]
    fn try_expr_simple() {
        check_expr(
            "foo()!",
            &expect![[r#"
                TryExpr@0..6
                  CallExpr@0..5
                    PathExpr@0..3
                      Path@0..3
                        PathSegment@0..3
                          NameRef@0..3
                            IDENT@0..3 "foo"
                    L_PAREN@3..4 "("
                    R_PAREN@4..5 ")"
                  BANG@5..6 "!"
            "#]],
        );
    }

    #[test]
    fn try_expr_on_path() {
        check_expr(
            "result!",
            &expect![[r#"
                TryExpr@0..7
                  PathExpr@0..6
                    Path@0..6
                      PathSegment@0..6
                        NameRef@0..6
                          IDENT@0..6 "result"
                  BANG@6..7 "!"
            "#]],
        );
    }

    #[test]
    fn try_expr_chained() {
        // foo()!.bar()! parses as ((foo()!).bar())!
        check_expr(
            "foo()!.bar()!",
            &expect![[r#"
                TryExpr@0..13
                  CallExpr@0..12
                    FieldExpr@0..10
                      TryExpr@0..6
                        CallExpr@0..5
                          PathExpr@0..3
                            Path@0..3
                              PathSegment@0..3
                                NameRef@0..3
                                  IDENT@0..3 "foo"
                          L_PAREN@3..4 "("
                          R_PAREN@4..5 ")"
                        BANG@5..6 "!"
                      DOT@6..7 "."
                      IDENT@7..10 "bar"
                    L_PAREN@10..11 "("
                    R_PAREN@11..12 ")"
                  BANG@12..13 "!"
            "#]],
        );
    }

    #[test]
    fn try_expr_precedence_vs_prefix() {
        // -foo()! parses as -(foo()!) since postfix binds tighter
        check_expr(
            "-foo()!",
            &expect![[r#"
                PrefixExpr@0..7
                  MINUS@0..1 "-"
                  TryExpr@1..7
                    CallExpr@1..6
                      PathExpr@1..4
                        Path@1..4
                          PathSegment@1..4
                            NameRef@1..4
                              IDENT@1..4 "foo"
                      L_PAREN@4..5 "("
                      R_PAREN@5..6 ")"
                    BANG@6..7 "!"
            "#]],
        );
    }

    // === Optional Chaining `?.` ===

    #[test]
    fn optional_chain_field() {
        check_expr(
            "x?.foo",
            &expect![[r#"
                OptionalFieldExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  QUESTION_DOT@1..3 "?."
                  IDENT@3..6 "foo"
            "#]],
        );
    }

    #[test]
    fn optional_chain_nested() {
        check_expr(
            "x?.foo?.bar",
            &expect![[r#"
                OptionalFieldExpr@0..11
                  OptionalFieldExpr@0..6
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "x"
                    QUESTION_DOT@1..3 "?."
                    IDENT@3..6 "foo"
                  QUESTION_DOT@6..8 "?."
                  IDENT@8..11 "bar"
            "#]],
        );
    }

    #[test]
    fn optional_chain_after_field() {
        check_expr(
            "x.foo?.bar",
            &expect![[r#"
                OptionalFieldExpr@0..10
                  PathExpr@0..5
                    Path@0..5
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                      DOT@1..2 "."
                      PathSegment@2..5
                        NameRef@2..5
                          IDENT@2..5 "foo"
                  QUESTION_DOT@5..7 "?."
                  IDENT@7..10 "bar"
            "#]],
        );
    }

    #[test]
    fn optional_chain_method() {
        check_expr(
            "x?.foo()",
            &expect![[r#"
                CallExpr@0..8
                  OptionalFieldExpr@0..6
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "x"
                    QUESTION_DOT@1..3 "?."
                    IDENT@3..6 "foo"
                  L_PAREN@6..7 "("
                  R_PAREN@7..8 ")"
            "#]],
        );
    }

    #[test]
    fn optional_chain_method_with_args() {
        check_expr(
            "obj?.process(1, 2)",
            &expect![[r#"
                CallExpr@0..18
                  OptionalFieldExpr@0..12
                    PathExpr@0..3
                      Path@0..3
                        PathSegment@0..3
                          NameRef@0..3
                            IDENT@0..3 "obj"
                    QUESTION_DOT@3..5 "?."
                    IDENT@5..12 "process"
                  L_PAREN@12..13 "("
                  CallArg@13..14
                    LiteralExpr@13..14
                      INT_LITERAL@13..14 "1"
                  COMMA@14..15 ","
                  CallArg@15..17
                    LiteralExpr@15..17
                      WHITESPACE@15..16 " "
                      INT_LITERAL@16..17 "2"
                  R_PAREN@17..18 ")"
            "#]],
        );
    }

    // === Dollar Expression `$` in Index/Slice ===

    #[test]
    fn index_dollar_last() {
        check_expr(
            "arr[$-1]",
            &expect![[r#"
                IndexExpr@0..8
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  BinExpr@4..7
                    DollarExpr@4..5
                      DOLLAR@4..5 "$"
                    MINUS@5..6 "-"
                    LiteralExpr@6..7
                      INT_LITERAL@6..7 "1"
                  R_BRACKET@7..8 "]"
            "#]],
        );
    }

    #[test]
    fn index_dollar_alone() {
        check_expr(
            "arr[$]",
            &expect![[r#"
                IndexExpr@0..6
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  DollarExpr@4..5
                    DOLLAR@4..5 "$"
                  R_BRACKET@5..6 "]"
            "#]],
        );
    }

    #[test]
    fn index_dollar_minus_expr() {
        check_expr(
            "arr[$-n]",
            &expect![[r#"
                IndexExpr@0..8
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  BinExpr@4..7
                    DollarExpr@4..5
                      DOLLAR@4..5 "$"
                    MINUS@5..6 "-"
                    PathExpr@6..7
                      Path@6..7
                        PathSegment@6..7
                          NameRef@6..7
                            IDENT@6..7 "n"
                  R_BRACKET@7..8 "]"
            "#]],
        );
    }

    #[test]
    fn slice_to_dollar() {
        check_expr(
            "arr[1:$]",
            &expect![[r#"
                SliceExpr@0..8
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  LiteralExpr@4..5
                    INT_LITERAL@4..5 "1"
                  COLON@5..6 ":"
                  DollarExpr@6..7
                    DOLLAR@6..7 "$"
                  R_BRACKET@7..8 "]"
            "#]],
        );
    }

    #[test]
    fn slice_dollar_minus() {
        check_expr(
            "arr[1:$-1]",
            &expect![[r#"
                SliceExpr@0..10
                  PathExpr@0..3
                    Path@0..3
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "arr"
                  L_BRACKET@3..4 "["
                  LiteralExpr@4..5
                    INT_LITERAL@4..5 "1"
                  COLON@5..6 ":"
                  BinExpr@6..9
                    DollarExpr@6..7
                      DOLLAR@6..7 "$"
                    MINUS@7..8 "-"
                    LiteralExpr@8..9
                      INT_LITERAL@8..9 "1"
                  R_BRACKET@9..10 "]"
            "#]],
        );
    }
}
