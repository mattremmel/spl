//! Expression parser using Pratt parsing.
//!
//! Implements precedence climbing for SPL expressions.

mod control_flow;
mod operators;
mod primary;

use crate::parser::{CompletedMarker, Parser};
use crate::syntax::SyntaxKind;

use operators::{infix_expr, postfix_expr, prefix_expr};
use primary::primary_expr;

pub(crate) use control_flow::block;

/// Parse an expression.
pub fn expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    expr_bp(p, 0, true)
}

/// Parse an expression, but don't allow struct expressions.
/// Used in control flow contexts where `identifier {` should be parsed as
/// identifier followed by block, not as a struct expression.
pub(crate) fn expr_no_struct(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    expr_bp(p, 0, false)
}

/// Parse an expression with minimum binding power.
fn expr_bp(
    p: &mut Parser<'_>,
    min_bp: u8,
    allow_struct: bool,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    #[cfg(debug_assertions)]
    let start_offset = p.current_offset();

    let mut lhs = match lhs(p, allow_struct)? {
        Some(lhs) => lhs,
        None => return Ok(None),
    };

    while let Some(op) = p.current() {
        // Check for postfix operators first (highest precedence)
        if let Some((l_bp, ())) = postfix_bp(op) {
            if l_bp < min_bp {
                break;
            }
            lhs = postfix_expr(p, lhs, op)?;
            continue;
        }

        // Check for infix operators
        if let Some((l_bp, r_bp)) = infix_bp(op) {
            if l_bp < min_bp {
                break;
            }
            lhs = infix_expr(p, lhs, r_bp, allow_struct)?;
            continue;
        }

        // Not an operator we recognize, stop
        break;
    }

    debug_assert!(
        p.current_offset() > start_offset,
        "postcondition: parser must advance when successfully parsing an expression"
    );

    Ok(Some(lhs))
}

/// Parse the left-hand side of an expression (prefix or primary).
fn lhs(
    p: &mut Parser<'_>,
    allow_struct: bool,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let current = match p.current() {
        Some(kind) => kind,
        None => return Ok(None),
    };

    // Check for prefix operators
    if let Some(((), r_bp)) = prefix_bp(current) {
        return prefix_expr(p, r_bp, allow_struct);
    }

    // Otherwise, parse a primary expression
    primary_expr(p, allow_struct)
}

// === Binding power tables ===

/// Prefix operator binding power ((), right).
fn prefix_bp(op: SyntaxKind) -> Option<((), u8)> {
    match op {
        SyntaxKind::BANG | SyntaxKind::MINUS | SyntaxKind::STAR => Some(((), 19)), // Unary: prec 10
        SyntaxKind::AMP => Some(((), 19)),                                         // Reference
        SyntaxKind::DOT_DOT => Some(((), 12)), // Range prefix: same r_bp as infix range
        _ => None,
    }
}

/// Infix operator binding power (left, right).
fn infix_bp(op: SyntaxKind) -> Option<(u8, u8)> {
    match op {
        // Assignment (prec 1, right assoc): left < right
        SyntaxKind::EQ
        | SyntaxKind::PLUS_EQ
        | SyntaxKind::MINUS_EQ
        | SyntaxKind::STAR_EQ
        | SyntaxKind::SLASH_EQ
        | SyntaxKind::PERCENT_EQ => Some((2, 1)),

        // Logical OR (prec 2, left assoc)
        SyntaxKind::OR_OR => Some((3, 4)),

        // Logical AND (prec 3, left assoc)
        SyntaxKind::AND_AND => Some((5, 6)),

        // Equality (prec 4, left assoc)
        SyntaxKind::EQ_EQ | SyntaxKind::NE => Some((7, 8)),

        // Comparison (prec 5, left assoc)
        SyntaxKind::LT | SyntaxKind::GT | SyntaxKind::LE | SyntaxKind::GE => Some((9, 10)),

        // Range (prec 6, left assoc)
        SyntaxKind::DOT_DOT => Some((11, 12)),

        // Additive (prec 7, left assoc)
        SyntaxKind::PLUS | SyntaxKind::MINUS => Some((13, 14)),

        // Multiplicative (prec 8, left assoc)
        SyntaxKind::STAR | SyntaxKind::SLASH | SyntaxKind::PERCENT => Some((15, 16)),

        // Cast (prec 9, left assoc)
        SyntaxKind::AS_KW => Some((17, 18)),

        _ => None,
    }
}

/// Postfix operator binding power (left, ()).
fn postfix_bp(op: SyntaxKind) -> Option<(u8, ())> {
    match op {
        // Postfix (prec 11, left assoc)
        SyntaxKind::L_PAREN     // call
        | SyntaxKind::L_BRACKET // index/slice
        | SyntaxKind::DOT       // field/method
        | SyntaxKind::COLON_COLON // path
        => Some((21, ())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::check_expr;
    use expect_test::expect;

    #[test]
    fn simple_binary_expr() {
        check_expr(
            "1+2",
            &expect![[r#"
                BinExpr@0..3
                  LiteralExpr@0..1
                    INT_LITERAL@0..1 "1"
                  PLUS@1..2 "+"
                  LiteralExpr@2..3
                    INT_LITERAL@2..3 "2"
            "#]],
        );
    }

    #[test]
    fn left_associative_binary_expr() {
        check_expr(
            "1+2+3",
            &expect![[r#"
                BinExpr@0..5
                  BinExpr@0..3
                    LiteralExpr@0..1
                      INT_LITERAL@0..1 "1"
                    PLUS@1..2 "+"
                    LiteralExpr@2..3
                      INT_LITERAL@2..3 "2"
                  PLUS@3..4 "+"
                  LiteralExpr@4..5
                    INT_LITERAL@4..5 "3"
            "#]],
        );
    }

    #[test]
    fn mixed_binding_power_binary_expr() {
        check_expr(
            "1+2*3",
            &expect![[r#"
                BinExpr@0..5
                  LiteralExpr@0..1
                    INT_LITERAL@0..1 "1"
                  PLUS@1..2 "+"
                  BinExpr@2..5
                    LiteralExpr@2..3
                      INT_LITERAL@2..3 "2"
                    STAR@3..4 "*"
                    LiteralExpr@4..5
                      INT_LITERAL@4..5 "3"
            "#]],
        );
    }

    #[test]
    fn comparison_chain() {
        check_expr(
            "a < b && c > d",
            &expect![[r#"
                BinExpr@0..14
                  BinExpr@0..5
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "a"
                    WHITESPACE@1..2 " "
                    LT@2..3 "<"
                    PathExpr@3..5
                      Path@3..5
                        PathSegment@3..5
                          NameRef@3..5
                            WHITESPACE@3..4 " "
                            IDENT@4..5 "b"
                  WHITESPACE@5..6 " "
                  AND_AND@6..8 "&&"
                  BinExpr@8..14
                    PathExpr@8..10
                      Path@8..10
                        PathSegment@8..10
                          NameRef@8..10
                            WHITESPACE@8..9 " "
                            IDENT@9..10 "c"
                    WHITESPACE@10..11 " "
                    GT@11..12 ">"
                    PathExpr@12..14
                      Path@12..14
                        PathSegment@12..14
                          NameRef@12..14
                            WHITESPACE@12..13 " "
                            IDENT@13..14 "d"
            "#]],
        );
    }

    #[test]
    fn assignment_right_assoc() {
        check_expr(
            "a = b = 1",
            &expect![[r#"
                BinExpr@0..9
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  EQ@2..3 "="
                  BinExpr@3..9
                    PathExpr@3..5
                      Path@3..5
                        PathSegment@3..5
                          NameRef@3..5
                            WHITESPACE@3..4 " "
                            IDENT@4..5 "b"
                    WHITESPACE@5..6 " "
                    EQ@6..7 "="
                    LiteralExpr@7..9
                      WHITESPACE@7..8 " "
                      INT_LITERAL@8..9 "1"
            "#]],
        );
    }

    // === Precedence/Associativity Tests ===

    #[test]
    fn precedence_full_chain() {
        // Tests: a || b && c == d < e + f * g
        // Parses as: a || (b && (c == (d < (e + (f * g)))))
        check_expr(
            "a || b && c == d < e + f * g",
            &expect![[r#"
                BinExpr@0..28
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  OR_OR@2..4 "||"
                  BinExpr@4..28
                    PathExpr@4..6
                      Path@4..6
                        PathSegment@4..6
                          NameRef@4..6
                            WHITESPACE@4..5 " "
                            IDENT@5..6 "b"
                    WHITESPACE@6..7 " "
                    AND_AND@7..9 "&&"
                    BinExpr@9..28
                      PathExpr@9..11
                        Path@9..11
                          PathSegment@9..11
                            NameRef@9..11
                              WHITESPACE@9..10 " "
                              IDENT@10..11 "c"
                      WHITESPACE@11..12 " "
                      EQ_EQ@12..14 "=="
                      BinExpr@14..28
                        PathExpr@14..16
                          Path@14..16
                            PathSegment@14..16
                              NameRef@14..16
                                WHITESPACE@14..15 " "
                                IDENT@15..16 "d"
                        WHITESPACE@16..17 " "
                        LT@17..18 "<"
                        BinExpr@18..28
                          PathExpr@18..20
                            Path@18..20
                              PathSegment@18..20
                                NameRef@18..20
                                  WHITESPACE@18..19 " "
                                  IDENT@19..20 "e"
                          WHITESPACE@20..21 " "
                          PLUS@21..22 "+"
                          BinExpr@22..28
                            PathExpr@22..24
                              Path@22..24
                                PathSegment@22..24
                                  NameRef@22..24
                                    WHITESPACE@22..23 " "
                                    IDENT@23..24 "f"
                            WHITESPACE@24..25 " "
                            STAR@25..26 "*"
                            PathExpr@26..28
                              Path@26..28
                                PathSegment@26..28
                                  NameRef@26..28
                                    WHITESPACE@26..27 " "
                                    IDENT@27..28 "g"
            "#]],
        );
    }

    #[test]
    fn precedence_unary_vs_binary() {
        check_expr(
            "-a + b",
            &expect![[r#"
                BinExpr@0..6
                  PrefixExpr@0..2
                    MINUS@0..1 "-"
                    PathExpr@1..2
                      Path@1..2
                        PathSegment@1..2
                          NameRef@1..2
                            IDENT@1..2 "a"
                  WHITESPACE@2..3 " "
                  PLUS@3..4 "+"
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
    fn associativity_assignment_chain() {
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
    fn mul_div_chain() {
        check_expr(
            "a * b / c % d",
            &expect![[r#"
                BinExpr@0..13
                  BinExpr@0..9
                    BinExpr@0..5
                      PathExpr@0..1
                        Path@0..1
                          PathSegment@0..1
                            NameRef@0..1
                              IDENT@0..1 "a"
                      WHITESPACE@1..2 " "
                      STAR@2..3 "*"
                      PathExpr@3..5
                        Path@3..5
                          PathSegment@3..5
                            NameRef@3..5
                              WHITESPACE@3..4 " "
                              IDENT@4..5 "b"
                    WHITESPACE@5..6 " "
                    SLASH@6..7 "/"
                    PathExpr@7..9
                      Path@7..9
                        PathSegment@7..9
                          NameRef@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "c"
                  WHITESPACE@9..10 " "
                  PERCENT@10..11 "%"
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
    fn add_sub_chain() {
        check_expr(
            "a + b - c",
            &expect![[r#"
                BinExpr@0..9
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
                  MINUS@6..7 "-"
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
    fn logical_and_chain() {
        check_expr(
            "a && b && c && d",
            &expect![[r#"
                BinExpr@0..16
                  BinExpr@0..11
                    BinExpr@0..6
                      PathExpr@0..1
                        Path@0..1
                          PathSegment@0..1
                            NameRef@0..1
                              IDENT@0..1 "a"
                      WHITESPACE@1..2 " "
                      AND_AND@2..4 "&&"
                      PathExpr@4..6
                        Path@4..6
                          PathSegment@4..6
                            NameRef@4..6
                              WHITESPACE@4..5 " "
                              IDENT@5..6 "b"
                    WHITESPACE@6..7 " "
                    AND_AND@7..9 "&&"
                    PathExpr@9..11
                      Path@9..11
                        PathSegment@9..11
                          NameRef@9..11
                            WHITESPACE@9..10 " "
                            IDENT@10..11 "c"
                  WHITESPACE@11..12 " "
                  AND_AND@12..14 "&&"
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
    fn precedence_or_and_and() {
        check_expr(
            "a || b && c",
            &expect![[r#"
                BinExpr@0..11
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  OR_OR@2..4 "||"
                  BinExpr@4..11
                    PathExpr@4..6
                      Path@4..6
                        PathSegment@4..6
                          NameRef@4..6
                            WHITESPACE@4..5 " "
                            IDENT@5..6 "b"
                    WHITESPACE@6..7 " "
                    AND_AND@7..9 "&&"
                    PathExpr@9..11
                      Path@9..11
                        PathSegment@9..11
                          NameRef@9..11
                            WHITESPACE@9..10 " "
                            IDENT@10..11 "c"
            "#]],
        );
    }

    #[test]
    fn precedence_all_arithmetic() {
        check_expr(
            "a + b * c < d",
            &expect![[r#"
                BinExpr@0..13
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
                  WHITESPACE@9..10 " "
                  LT@10..11 "<"
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
    fn deeply_nested_parens() {
        check_expr(
            "((((1))))",
            &expect![[r#"
                ParenExpr@0..9
                  L_PAREN@0..1 "("
                  ParenExpr@1..8
                    L_PAREN@1..2 "("
                    ParenExpr@2..7
                      L_PAREN@2..3 "("
                      ParenExpr@3..6
                        L_PAREN@3..4 "("
                        LiteralExpr@4..5
                          INT_LITERAL@4..5 "1"
                        R_PAREN@5..6 ")"
                      R_PAREN@6..7 ")"
                    R_PAREN@7..8 ")"
                  R_PAREN@8..9 ")"
            "#]],
        );
    }

    #[test]
    fn double_not() {
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
}
