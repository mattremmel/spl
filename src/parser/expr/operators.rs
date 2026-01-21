//! Operator parsing: prefix, infix, and postfix expressions.

use crate::parser::{CompletedMarker, Parser};
use crate::syntax::SyntaxKind;

use super::{expr, expr_bp, expr_no_struct_bp};

/// Parse a prefix expression.
pub(super) fn prefix_expr(
    p: &mut Parser<'_>,
    r_bp: u8,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    let op = p.current().unwrap();

    // Handle &mut specially
    if op == SyntaxKind::AMP {
        p.bump(); // &
        p.eat(SyntaxKind::MUT_KW); // optional mut
        let _ = expr_bp(p, r_bp)?;
        return Ok(Some(m.complete(p, SyntaxKind::RefExpr)));
    }

    // Regular prefix operator
    p.bump();
    let _ = expr_bp(p, r_bp)?;

    let kind = match op {
        SyntaxKind::BANG | SyntaxKind::MINUS | SyntaxKind::STAR => SyntaxKind::PrefixExpr,
        _ => unreachable!("unexpected prefix operator: {:?}", op),
    };

    Ok(Some(m.complete(p, kind)))
}

/// Parse a prefix expression, disallowing struct expressions.
pub(super) fn prefix_expr_no_struct(
    p: &mut Parser<'_>,
    r_bp: u8,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    let op = p.current().unwrap();

    // Handle &mut specially
    if op == SyntaxKind::AMP {
        p.bump(); // &
        p.eat(SyntaxKind::MUT_KW); // optional mut
        let _ = expr_no_struct_bp(p, r_bp)?;
        return Ok(Some(m.complete(p, SyntaxKind::RefExpr)));
    }

    // Regular prefix operator
    p.bump();
    let _ = expr_no_struct_bp(p, r_bp)?;

    let kind = match op {
        SyntaxKind::BANG | SyntaxKind::MINUS | SyntaxKind::STAR => SyntaxKind::PrefixExpr,
        _ => unreachable!("unexpected prefix operator: {:?}", op),
    };

    Ok(Some(m.complete(p, kind)))
}

/// Parse an infix expression.
pub(super) fn infix_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
    r_bp: u8,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = lhs.precede(p);
    let op = p.current().unwrap();

    // Handle 'as' cast specially
    if op == SyntaxKind::AS_KW {
        p.bump();
        // Parse type (simplified: just an identifier for now)
        type_expr(p)?;
        return Ok(m.complete(p, SyntaxKind::CastExpr));
    }

    // Regular binary operator
    p.bump();
    let _ = expr_bp(p, r_bp)?;

    // Determine the node kind based on operator
    let kind = match op {
        SyntaxKind::DOT_DOT => SyntaxKind::RangeExpr,
        _ => SyntaxKind::BinExpr,
    };

    Ok(m.complete(p, kind))
}

/// Parse a postfix expression.
pub(super) fn postfix_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
    op: SyntaxKind,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    match op {
        SyntaxKind::L_PAREN => call_expr(p, lhs),
        SyntaxKind::L_BRACKET => index_or_slice_expr(p, lhs),
        SyntaxKind::DOT => field_or_method_expr(p, lhs),
        SyntaxKind::COLON_COLON => path_expr(p, lhs),
        _ => unreachable!("unexpected postfix operator: {:?}", op),
    }
}

/// Parse a call expression: expr(args)
fn call_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = lhs.precede(p);
    arg_list(p)?;
    Ok(m.complete(p, SyntaxKind::CallExpr))
}

/// Parse an argument list: (expr, expr, ...)
pub(super) fn arg_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;

    while !p.at(SyntaxKind::R_PAREN) && p.current().is_some() {
        let _ = expr(p)?;
        if !p.at(SyntaxKind::R_PAREN) {
            p.expect(SyntaxKind::COMMA)?;
        }
    }

    p.expect(SyntaxKind::R_PAREN)?;
    Ok(m.complete(p, SyntaxKind::ArgList))
}

/// Parse index or slice expression: expr[idx] or expr[start:end]
fn index_or_slice_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = lhs.precede(p);
    p.expect(SyntaxKind::L_BRACKET)?;

    // Parse optional start expression (skip if immediately at colon)
    let is_slice = p.at(SyntaxKind::COLON);
    if !is_slice {
        let _ = expr(p)?;
    }

    // Determine if slice (has colon) or index (no colon)
    if is_slice || p.at(SyntaxKind::COLON) {
        p.bump(); // consume :

        // Parse optional end expression
        if !p.at(SyntaxKind::R_BRACKET) {
            if p.at(SyntaxKind::DOLLAR) {
                p.bump(); // $ (slice to end)
            } else {
                let _ = expr(p)?;
            }
        }
        p.expect(SyntaxKind::R_BRACKET)?;
        Ok(m.complete(p, SyntaxKind::SliceExpr))
    } else {
        p.expect(SyntaxKind::R_BRACKET)?;
        Ok(m.complete(p, SyntaxKind::IndexExpr))
    }
}

/// Parse field access or method call: expr.field or expr.method(args) or expr.0 (tuple)
fn field_or_method_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = lhs.precede(p);
    p.expect(SyntaxKind::DOT)?;

    // Accept identifier or integer literal (for tuple field access like t.0)
    if p.at(SyntaxKind::IDENT) {
        p.bump();
        // Check for method call (only for identifier, not for tuple index)
        if p.at(SyntaxKind::L_PAREN) {
            arg_list(p)?;
            Ok(m.complete(p, SyntaxKind::MethodCallExpr))
        } else {
            Ok(m.complete(p, SyntaxKind::FieldExpr))
        }
    } else if p.at(SyntaxKind::INT_LITERAL) {
        // Tuple field access: t.0, t.1, etc.
        p.bump();
        Ok(m.complete(p, SyntaxKind::FieldExpr))
    } else {
        Err(p.error_at_current("expected identifier or integer after '.'".to_string()))
    }
}

/// Parse path continuation: expr::name or expr::name(args)
fn path_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = lhs.precede(p);
    p.expect(SyntaxKind::COLON_COLON)?;

    // Expect identifier
    if !p.at(SyntaxKind::IDENT) {
        return Err(p.error_at_current("expected identifier after '::'".to_string()));
    }
    p.bump();

    // Check for call
    if p.at(SyntaxKind::L_PAREN) {
        arg_list(p)?;
        Ok(m.complete(p, SyntaxKind::CallExpr))
    } else {
        Ok(m.complete(p, SyntaxKind::PathExpr))
    }
}

/// Parse a type expression (simplified for cast).
fn type_expr(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Simplified: just parse an identifier path (no generics for now)
    if !p.at(SyntaxKind::IDENT) {
        m.abandon(p);
        return Err(p.error_at_current("expected type".to_string()));
    }

    // Use structured path parsing (no generics for cast expressions)
    crate::parser::path::path_no_generics(p)?;

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
                    ArgList@4..6
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
                  FieldExpr@1..10
                    PathExpr@1..4
                      Path@1..4
                        PathSegment@1..4
                          NameRef@1..4
                            IDENT@1..4 "obj"
                    DOT@4..5 "."
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
                  ArgList@3..9
                    L_PAREN@3..4 "("
                    LiteralExpr@4..5
                      INT_LITERAL@4..5 "1"
                    COMMA@5..6 ","
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
                  ArgList@3..10
                    L_PAREN@3..4 "("
                    PathExpr@4..5
                      Path@4..5
                        PathSegment@4..5
                          NameRef@4..5
                            IDENT@4..5 "a"
                    COMMA@5..6 ","
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
                  ArgList@3..15
                    L_PAREN@3..4 "("
                    CallExpr@4..14
                      PathExpr@4..7
                        Path@4..7
                          PathSegment@4..7
                            NameRef@4..7
                              IDENT@4..7 "bar"
                      ArgList@7..14
                        L_PAREN@7..8 "("
                        CallExpr@8..13
                          PathExpr@8..11
                            Path@8..11
                              PathSegment@8..11
                                NameRef@8..11
                                  IDENT@8..11 "baz"
                          ArgList@11..13
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
                      ArgList@7..9
                        L_PAREN@7..8 "("
                        R_PAREN@8..9 ")"
                    R_PAREN@9..10 ")"
                  ArgList@10..15
                    L_PAREN@10..11 "("
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
                FieldExpr@0..7
                  PathExpr@0..5
                    Path@0..5
                      PathSegment@0..5
                        NameRef@0..5
                          IDENT@0..5 "point"
                  DOT@5..6 "."
                  IDENT@6..7 "x"
            "#]],
        );
    }

    #[test]
    fn method_call_expr() {
        check_expr(
            "point.distance()",
            &expect![[r#"
                MethodCallExpr@0..16
                  PathExpr@0..5
                    Path@0..5
                      PathSegment@0..5
                        NameRef@0..5
                          IDENT@0..5 "point"
                  DOT@5..6 "."
                  IDENT@6..14 "distance"
                  ArgList@14..16
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
                FieldExpr@0..9
                  FieldExpr@0..7
                    FieldExpr@0..5
                      PathExpr@0..3
                        Path@0..3
                          PathSegment@0..3
                            NameRef@0..3
                              IDENT@0..3 "obj"
                      DOT@3..4 "."
                      IDENT@4..5 "a"
                    DOT@5..6 "."
                    IDENT@6..7 "b"
                  DOT@7..8 "."
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
                    ArgList@7..9
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
                MethodCallExpr@0..15
                  MethodCallExpr@0..11
                    MethodCallExpr@0..7
                      PathExpr@0..3
                        Path@0..3
                          PathSegment@0..3
                            NameRef@0..3
                              IDENT@0..3 "obj"
                      DOT@3..4 "."
                      IDENT@4..5 "a"
                      ArgList@5..7
                        L_PAREN@5..6 "("
                        R_PAREN@6..7 ")"
                    DOT@7..8 "."
                    IDENT@8..9 "b"
                    ArgList@9..11
                      L_PAREN@9..10 "("
                      R_PAREN@10..11 ")"
                  DOT@11..12 "."
                  IDENT@12..13 "c"
                  ArgList@13..15
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
                  FieldExpr@0..7
                    PathExpr@0..3
                      Path@0..3
                        PathSegment@0..3
                          NameRef@0..3
                            IDENT@0..3 "obj"
                    DOT@3..4 "."
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
            "std::vec::Vec",
            &expect![[r#"
                PathExpr@0..13
                  Path@0..13
                    PathSegment@0..3
                      NameRef@0..3
                        IDENT@0..3 "std"
                    COLON_COLON@3..5 "::"
                    PathSegment@5..8
                      NameRef@5..8
                        IDENT@5..8 "vec"
                    COLON_COLON@8..10 "::"
                    PathSegment@10..13
                      NameRef@10..13
                        IDENT@10..13 "Vec"
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
}
