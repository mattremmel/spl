//! Expression parser using Pratt parsing.
//!
//! Implements Vaughan Pratt's "Top Down Operator Precedence" parsing (1973)
//! for SPL expressions. This approach elegantly handles operator precedence
//! and associativity without grammar ambiguity.
//!
//! # Binding Power Theory
//!
//! Each operator has a left binding power (`l_bp`) and right binding power (`r_bp`).
//! The parser collects operands until it encounters an operator with `l_bp` less
//! than the current minimum, then returns to let the caller handle it.
//!
//! Associativity emerges from the relationship between `l_bp` and `r_bp`:
//! - **Left-associative** (`l_bp > r_bp`): `a + b + c` parses as `(a + b) + c`
//! - **Right-associative** (`l_bp < r_bp`): `a = b = c` parses as `a = (b = c)`
//!
//! # The Parsing Loop
//!
//! The core loop in `expr_bp` maintains this invariant:
//! > "lhs holds the leftmost operand that binds at least as tightly as min_bp"
//!
//! When we see an operator:
//! 1. If its `l_bp` < `min_bp`, stop (this operand belongs to caller)
//! 2. Otherwise, recursively parse RHS with `r_bp` as the new `min_bp`
//! 3. Combine into a new lhs and continue
//!
//! # References
//!
//! - V. Pratt, "Top Down Operator Precedence", POPL 1973
//! - <https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html>

mod control_flow;
mod operators;
mod primary;

use crate::{CompletedMarker, Parser};
use spl_syntax::SyntaxKind;

use operators::{infix_expr, postfix_expr, prefix_expr};
use primary::primary_expr;

pub(crate) use control_flow::block;

/// Parse an expression.
pub fn expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    expr_bp(p, 0, true, 0)
}

/// Parse an expression, but don't allow struct expressions.
/// Used in control flow contexts where `identifier {` should be parsed as
/// identifier followed by block, not as a struct expression.
pub(crate) fn expr_no_struct(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    expr_bp(p, 0, false, 0)
}

/// Parse an expression with minimum binding power.
///
/// The `min_bp` parameter acts as a precedence floor: we only consume operators
/// whose left binding power meets or exceeds this threshold.
///
/// The `depth` parameter tracks recursion depth to prevent stack overflow on
/// deeply nested expressions. Returns an error if depth exceeds `MAX_EXPR_DEPTH`.
fn expr_bp(
    p: &mut Parser<'_>,
    min_bp: u8,
    allow_struct: bool,
    depth: usize,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    if depth > MAX_EXPR_DEPTH {
        tracing::trace!(depth, max = MAX_EXPR_DEPTH, "expression nesting limit exceeded");
        return Err(p.error_at_current("expression nesting limit exceeded".to_string()));
    }

    let start_offset = p.current_offset();

    let Some(mut lhs) = lhs(p, allow_struct, depth)? else {
        return Ok(None);
    };

    // Loop invariant: lhs is the leftmost expression that binds at min_bp or tighter.
    // We extend it rightward until we hit an operator weaker than min_bp.
    while let Some(op) = p.current() {
        // Check for postfix operators first (highest precedence)
        if let Some((l_bp, ())) = postfix_bp(op) {
            if l_bp < min_bp {
                tracing::trace!(op = ?op, l_bp, min_bp, "postfix below min_bp, stopping");
                break;
            }
            lhs = postfix_expr(p, lhs, op)?;
            continue;
        }

        // Check for infix operators
        if let Some((l_bp, r_bp)) = infix_bp(op) {
            if l_bp < min_bp {
                tracing::trace!(op = ?op, l_bp, min_bp, "infix below min_bp, stopping");
                break;
            }
            tracing::trace!(op = ?op, l_bp, r_bp, depth, "parsing infix expression");
            lhs = infix_expr(p, lhs, r_bp, allow_struct, depth + 1)?;
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
    depth: usize,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let Some(current) = p.current() else {
        return Ok(None);
    };

    // Check for prefix operators
    if let Some(((), r_bp)) = prefix_bp(current) {
        return prefix_expr(p, r_bp, allow_struct, depth + 1);
    }

    // Otherwise, parse a primary expression
    primary_expr(p, allow_struct, depth)
}

// === Binding power tables ===
//
// Binding power constants for the Pratt parser. Each constant is (l_bp, r_bp).
// Higher values bind tighter (multiplicative > additive > comparison > ...).
//
// Associativity encoding:
//   l_bp < r_bp  →  right-associative (e.g., assignment: a = b = c → a = (b = c))
//   l_bp > r_bp  →  left-associative  (e.g., addition: a + b + c → (a + b) + c)
//
// Gaps between levels allow inserting new precedences without renumbering.

const BP_ASSIGN: (u8, u8) = (2, 1); // r_bp < l_bp: right-associative
const BP_COALESCE: (u8, u8) = (4, 3); // right-associative: l_bp > r_bp
const BP_LOGICAL_OR: (u8, u8) = (5, 6);
const BP_LOGICAL_AND: (u8, u8) = (7, 8);
const BP_IS: (u8, u8) = (9, 10); // Pattern matching: `x is Some(v)`
const BP_EQUALITY: (u8, u8) = (11, 12);
const BP_COMPARISON: (u8, u8) = (13, 14);
const BP_BITWISE_OR: (u8, u8) = (15, 16);
const BP_BITWISE_XOR: (u8, u8) = (17, 18);
const BP_BITWISE_AND: (u8, u8) = (19, 20);
const BP_SHIFT: (u8, u8) = (21, 22);
const BP_RANGE: (u8, u8) = (23, 24);
const BP_ADDITIVE: (u8, u8) = (25, 26);
const BP_MULTIPLICATIVE: (u8, u8) = (27, 28);
const BP_EXPONENTIATION: (u8, u8) = (30, 29); // right-associative: l_bp > r_bp
const BP_PREFIX: u8 = 33;
const BP_POSTFIX: u8 = 35;

/// Maximum expression nesting depth to prevent stack overflow on malicious input.
/// 256 levels is sufficient for any reasonable code while preventing `DoS` attacks.
const MAX_EXPR_DEPTH: usize = 256;

/// Prefix operator binding power ((), right).
fn prefix_bp(op: SyntaxKind) -> Option<((), u8)> {
    match op {
        SyntaxKind::BANG
        | SyntaxKind::MINUS
        | SyntaxKind::STAR
        | SyntaxKind::TILDE
        | SyntaxKind::AMP => Some(((), BP_PREFIX)),
        SyntaxKind::DOT_DOT | SyntaxKind::DOT_DOT_EQ => Some(((), BP_RANGE.1)), // Range prefix: same r_bp as infix range
        _ => None,
    }
}

/// Infix operator binding power (left, right).
fn infix_bp(op: SyntaxKind) -> Option<(u8, u8)> {
    match op {
        // Assignment (right-associative)
        SyntaxKind::EQ
        | SyntaxKind::PLUS_EQ
        | SyntaxKind::MINUS_EQ
        | SyntaxKind::STAR_EQ
        | SyntaxKind::STAR_STAR_EQ
        | SyntaxKind::SLASH_EQ
        | SyntaxKind::PERCENT_EQ
        | SyntaxKind::PIPE_EQ
        | SyntaxKind::CARET_EQ
        | SyntaxKind::AMP_EQ
        | SyntaxKind::SHL_EQ
        | SyntaxKind::SHR_EQ => Some(BP_ASSIGN),

        // Coalesce (right-associative)
        SyntaxKind::QUESTION_QUESTION => Some(BP_COALESCE),

        // Logical OR (left-associative)
        SyntaxKind::OR_OR => Some(BP_LOGICAL_OR),

        // Logical AND (left-associative)
        SyntaxKind::AND_AND => Some(BP_LOGICAL_AND),

        // Pattern matching: `x is Pattern` or `x is not Pattern` (left-associative)
        SyntaxKind::IS_KW => Some(BP_IS),

        // Equality (left-associative)
        SyntaxKind::EQ_EQ | SyntaxKind::NE => Some(BP_EQUALITY),

        // Comparison (left-associative)
        SyntaxKind::LT | SyntaxKind::GT | SyntaxKind::LE | SyntaxKind::GE => Some(BP_COMPARISON),

        // Bitwise OR (left-associative)
        SyntaxKind::PIPE => Some(BP_BITWISE_OR),

        // Bitwise XOR (left-associative)
        SyntaxKind::CARET => Some(BP_BITWISE_XOR),

        // Bitwise AND (left-associative) - in infix position, AMP is bitwise AND
        SyntaxKind::AMP => Some(BP_BITWISE_AND),

        // Shift (left-associative)
        SyntaxKind::SHL | SyntaxKind::SHR => Some(BP_SHIFT),

        // Range (left-associative)
        SyntaxKind::DOT_DOT | SyntaxKind::DOT_DOT_EQ => Some(BP_RANGE),

        // Additive (left-associative)
        SyntaxKind::PLUS | SyntaxKind::MINUS => Some(BP_ADDITIVE),

        // Multiplicative (left-associative)
        SyntaxKind::STAR | SyntaxKind::SLASH | SyntaxKind::PERCENT => Some(BP_MULTIPLICATIVE),

        // Exponentiation (right-associative)
        SyntaxKind::STAR_STAR => Some(BP_EXPONENTIATION),

        _ => None,
    }
}

/// Postfix operator binding power (left, ()).
fn postfix_bp(op: SyntaxKind) -> Option<(u8, ())> {
    match op {
        // Postfix (highest precedence)
        SyntaxKind::L_PAREN      // call
        | SyntaxKind::L_BRACKET  // index/slice
        | SyntaxKind::DOT        // field/method
        | SyntaxKind::QUESTION_DOT // optional chaining: expr?.field
        | SyntaxKind::BANG       // try/propagate: expr!
        => Some((BP_POSTFIX, ())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::check_expr;
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

    // =========================================================================
    // Edge Case Tests: EOF and Delimiter Handling
    // =========================================================================
    //
    // These tests verify the documented Pratt parsing behaviors for:
    // - Graceful error recovery at EOF
    // - Handling of unexpected delimiters
    // - Precedence boundary conditions

    #[test]
    fn expr_eof_after_binary_operator() {
        // "1 +" at EOF - should parse partial expression
        check_expr(
            "1 +",
            &expect![[r#"
                BinExpr@0..3
                  LiteralExpr@0..1
                    INT_LITERAL@0..1 "1"
                  WHITESPACE@1..2 " "
                  PLUS@2..3 "+"
            "#]],
        );
    }

    #[test]
    fn expr_eof_after_prefix_operator() {
        // "-" alone at EOF - should produce prefix expression without operand
        check_expr(
            "-",
            &expect![[r#"
                PrefixExpr@0..1
                  MINUS@0..1 "-"
            "#]],
        );
    }

    #[test]
    fn expr_double_minus_as_subtract_negate() {
        // "1 - - 2" parses as 1 - (-2)
        check_expr(
            "1 - - 2",
            &expect![[r#"
                BinExpr@0..7
                  LiteralExpr@0..1
                    INT_LITERAL@0..1 "1"
                  WHITESPACE@1..2 " "
                  MINUS@2..3 "-"
                  PrefixExpr@3..7
                    WHITESPACE@3..4 " "
                    MINUS@4..5 "-"
                    LiteralExpr@5..7
                      WHITESPACE@5..6 " "
                      INT_LITERAL@6..7 "2"
            "#]],
        );
    }

    // =========================================================================
    // Precedence Boundary Tests
    // =========================================================================

    #[test]
    fn precedence_assign_vs_or() {
        // "a = b || c" parses as "a = (b || c)" since || binds tighter than =
        check_expr(
            "a = b || c",
            &expect![[r#"
                BinExpr@0..10
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  EQ@2..3 "="
                  BinExpr@3..10
                    PathExpr@3..5
                      Path@3..5
                        PathSegment@3..5
                          NameRef@3..5
                            WHITESPACE@3..4 " "
                            IDENT@4..5 "b"
                    WHITESPACE@5..6 " "
                    OR_OR@6..8 "||"
                    PathExpr@8..10
                      Path@8..10
                        PathSegment@8..10
                          NameRef@8..10
                            WHITESPACE@8..9 " "
                            IDENT@9..10 "c"
            "#]],
        );
    }

    #[test]
    fn postfix_vs_prefix_precedence() {
        // "-foo()" parses as "-(foo())" since postfix call binds tighter than prefix minus
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
    fn compound_assign_right_assoc() {
        // "a += b += c" parses as "a += (b += c)" since assignment is right-associative
        check_expr(
            "a += b += c",
            &expect![[r#"
                BinExpr@0..11
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  PLUS_EQ@2..4 "+="
                  BinExpr@4..11
                    PathExpr@4..6
                      Path@4..6
                        PathSegment@4..6
                          NameRef@4..6
                            WHITESPACE@4..5 " "
                            IDENT@5..6 "b"
                    WHITESPACE@6..7 " "
                    PLUS_EQ@7..9 "+="
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
    fn negation_with_method_call() {
        // "-foo.bar()" parses as "-(foo.bar())" - postfix chains bind tighter than prefix
        check_expr(
            "-foo.bar()",
            &expect![[r#"
                PrefixExpr@0..10
                  MINUS@0..1 "-"
                  CallExpr@1..10
                    PathExpr@1..8
                      Path@1..8
                        PathSegment@1..4
                          NameRef@1..4
                            IDENT@1..4 "foo"
                        DOT@4..5 "."
                        PathSegment@5..8
                          NameRef@5..8
                            IDENT@5..8 "bar"
                    L_PAREN@8..9 "("
                    R_PAREN@9..10 ")"
            "#]],
        );
    }

    #[test]
    fn range_operator_precedence() {
        // "0..10 + 1" parses as "0..(10 + 1)" since arithmetic binds tighter than range
        check_expr(
            "0..10 + 1",
            &expect![[r#"
                RangeExpr@0..9
                  LiteralExpr@0..1
                    INT_LITERAL@0..1 "0"
                  DOT_DOT@1..3 ".."
                  BinExpr@3..9
                    LiteralExpr@3..5
                      INT_LITERAL@3..5 "10"
                    WHITESPACE@5..6 " "
                    PLUS@6..7 "+"
                    LiteralExpr@7..9
                      WHITESPACE@7..8 " "
                      INT_LITERAL@8..9 "1"
            "#]],
        );
    }

    // === Exponentiation Operator ===

    #[test]
    fn exponentiation_simple() {
        check_expr(
            "2 ** 3",
            &expect![[r#"
                BinExpr@0..6
                  LiteralExpr@0..1
                    INT_LITERAL@0..1 "2"
                  WHITESPACE@1..2 " "
                  STAR_STAR@2..4 "**"
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "3"
            "#]],
        );
    }

    #[test]
    fn exponentiation_right_associative() {
        // 2 ** 3 ** 2 parses as 2 ** (3 ** 2)
        check_expr(
            "2**3**2",
            &expect![[r#"
                BinExpr@0..7
                  LiteralExpr@0..1
                    INT_LITERAL@0..1 "2"
                  STAR_STAR@1..3 "**"
                  BinExpr@3..7
                    LiteralExpr@3..4
                      INT_LITERAL@3..4 "3"
                    STAR_STAR@4..6 "**"
                    LiteralExpr@6..7
                      INT_LITERAL@6..7 "2"
            "#]],
        );
    }

    #[test]
    fn exponentiation_binds_tighter_than_mult() {
        // a * b ** c parses as a * (b ** c)
        check_expr(
            "a*b**c",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  STAR@1..2 "*"
                  BinExpr@2..6
                    PathExpr@2..3
                      Path@2..3
                        PathSegment@2..3
                          NameRef@2..3
                            IDENT@2..3 "b"
                    STAR_STAR@3..5 "**"
                    PathExpr@5..6
                      Path@5..6
                        PathSegment@5..6
                          NameRef@5..6
                            IDENT@5..6 "c"
            "#]],
        );
    }

    // === Coalesce Operator ===

    #[test]
    fn coalesce_simple() {
        check_expr(
            "a ?? b",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  QUESTION_QUESTION@2..4 "??"
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
    fn coalesce_right_associative() {
        // a ?? b ?? c parses as a ?? (b ?? c)
        check_expr(
            "a??b??c",
            &expect![[r#"
                BinExpr@0..7
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  QUESTION_QUESTION@1..3 "??"
                  BinExpr@3..7
                    PathExpr@3..4
                      Path@3..4
                        PathSegment@3..4
                          NameRef@3..4
                            IDENT@3..4 "b"
                    QUESTION_QUESTION@4..6 "??"
                    PathExpr@6..7
                      Path@6..7
                        PathSegment@6..7
                          NameRef@6..7
                            IDENT@6..7 "c"
            "#]],
        );
    }

    #[test]
    fn coalesce_lower_than_logical_or() {
        // a ?? b || c parses as (a ?? b) || c since || binds tighter than ??
        check_expr(
            "a??b||c",
            &expect![[r#"
                BinExpr@0..7
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  QUESTION_QUESTION@1..3 "??"
                  BinExpr@3..7
                    PathExpr@3..4
                      Path@3..4
                        PathSegment@3..4
                          NameRef@3..4
                            IDENT@3..4 "b"
                    OR_OR@4..6 "||"
                    PathExpr@6..7
                      Path@6..7
                        PathSegment@6..7
                          NameRef@6..7
                            IDENT@6..7 "c"
            "#]],
        );
    }

    // === Bitwise Operators ===

    #[test]
    fn bitwise_or_simple() {
        check_expr(
            "a | b",
            &expect![[r#"
                BinExpr@0..5
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  PIPE@2..3 "|"
                  PathExpr@3..5
                    Path@3..5
                      PathSegment@3..5
                        NameRef@3..5
                          WHITESPACE@3..4 " "
                          IDENT@4..5 "b"
            "#]],
        );
    }

    #[test]
    fn bitwise_or_left_associative() {
        // a | b | c parses as (a | b) | c
        check_expr(
            "a|b|c",
            &expect![[r#"
                BinExpr@0..5
                  BinExpr@0..3
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "a"
                    PIPE@1..2 "|"
                    PathExpr@2..3
                      Path@2..3
                        PathSegment@2..3
                          NameRef@2..3
                            IDENT@2..3 "b"
                  PIPE@3..4 "|"
                  PathExpr@4..5
                    Path@4..5
                      PathSegment@4..5
                        NameRef@4..5
                          IDENT@4..5 "c"
            "#]],
        );
    }

    #[test]
    fn bitwise_xor_simple() {
        check_expr(
            "a ^ b",
            &expect![[r#"
                BinExpr@0..5
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  CARET@2..3 "^"
                  PathExpr@3..5
                    Path@3..5
                      PathSegment@3..5
                        NameRef@3..5
                          WHITESPACE@3..4 " "
                          IDENT@4..5 "b"
            "#]],
        );
    }

    #[test]
    fn bitwise_xor_binds_tighter_than_or() {
        // a | b ^ c parses as a | (b ^ c)
        check_expr(
            "a|b^c",
            &expect![[r#"
                BinExpr@0..5
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  PIPE@1..2 "|"
                  BinExpr@2..5
                    PathExpr@2..3
                      Path@2..3
                        PathSegment@2..3
                          NameRef@2..3
                            IDENT@2..3 "b"
                    CARET@3..4 "^"
                    PathExpr@4..5
                      Path@4..5
                        PathSegment@4..5
                          NameRef@4..5
                            IDENT@4..5 "c"
            "#]],
        );
    }

    #[test]
    fn bitwise_and_simple() {
        check_expr(
            "a & b",
            &expect![[r#"
                BinExpr@0..5
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  AMP@2..3 "&"
                  PathExpr@3..5
                    Path@3..5
                      PathSegment@3..5
                        NameRef@3..5
                          WHITESPACE@3..4 " "
                          IDENT@4..5 "b"
            "#]],
        );
    }

    #[test]
    fn bitwise_and_binds_tighter_than_xor() {
        // a ^ b & c parses as a ^ (b & c)
        check_expr(
            "a^b&c",
            &expect![[r#"
                BinExpr@0..5
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  CARET@1..2 "^"
                  BinExpr@2..5
                    PathExpr@2..3
                      Path@2..3
                        PathSegment@2..3
                          NameRef@2..3
                            IDENT@2..3 "b"
                    AMP@3..4 "&"
                    PathExpr@4..5
                      Path@4..5
                        PathSegment@4..5
                          NameRef@4..5
                            IDENT@4..5 "c"
            "#]],
        );
    }

    #[test]
    fn bitwise_and_vs_comparison() {
        // a < b & c parses as a < (b & c) since bitwise ops bind tighter than comparison
        check_expr(
            "a<b&c",
            &expect![[r#"
                BinExpr@0..5
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  LT@1..2 "<"
                  BinExpr@2..5
                    PathExpr@2..3
                      Path@2..3
                        PathSegment@2..3
                          NameRef@2..3
                            IDENT@2..3 "b"
                    AMP@3..4 "&"
                    PathExpr@4..5
                      Path@4..5
                        PathSegment@4..5
                          NameRef@4..5
                            IDENT@4..5 "c"
            "#]],
        );
    }

    // === Shift Operators ===

    #[test]
    fn shift_left_simple() {
        check_expr(
            "a << 2",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  SHL@2..4 "<<"
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "2"
            "#]],
        );
    }

    #[test]
    fn shift_right_simple() {
        check_expr(
            "a >> 2",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  WHITESPACE@1..2 " "
                  SHR@2..4 ">>"
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "2"
            "#]],
        );
    }

    #[test]
    fn shift_left_associative() {
        // a << 2 >> 1 parses as (a << 2) >> 1
        check_expr(
            "a<<2>>1",
            &expect![[r#"
                BinExpr@0..7
                  BinExpr@0..4
                    PathExpr@0..1
                      Path@0..1
                        PathSegment@0..1
                          NameRef@0..1
                            IDENT@0..1 "a"
                    SHL@1..3 "<<"
                    LiteralExpr@3..4
                      INT_LITERAL@3..4 "2"
                  SHR@4..6 ">>"
                  LiteralExpr@6..7
                    INT_LITERAL@6..7 "1"
            "#]],
        );
    }

    #[test]
    fn shift_binds_tighter_than_bitwise_and() {
        // a & b << 2 parses as a & (b << 2)
        check_expr(
            "a&b<<2",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  AMP@1..2 "&"
                  BinExpr@2..6
                    PathExpr@2..3
                      Path@2..3
                        PathSegment@2..3
                          NameRef@2..3
                            IDENT@2..3 "b"
                    SHL@3..5 "<<"
                    LiteralExpr@5..6
                      INT_LITERAL@5..6 "2"
            "#]],
        );
    }

    #[test]
    fn shift_binds_looser_than_additive() {
        // a << 2 + 1 parses as a << (2 + 1) since arithmetic binds tighter
        check_expr(
            "a<<2+1",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  SHL@1..3 "<<"
                  BinExpr@3..6
                    LiteralExpr@3..4
                      INT_LITERAL@3..4 "2"
                    PLUS@4..5 "+"
                    LiteralExpr@5..6
                      INT_LITERAL@5..6 "1"
            "#]],
        );
    }

    // === Compound Assignment Operators ===

    #[test]
    fn compound_pipe_eq() {
        check_expr(
            "x |= 1",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  PIPE_EQ@2..4 "|="
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "1"
            "#]],
        );
    }

    #[test]
    fn compound_caret_eq() {
        check_expr(
            "x ^= 1",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  CARET_EQ@2..4 "^="
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "1"
            "#]],
        );
    }

    #[test]
    fn compound_amp_eq() {
        check_expr(
            "x &= 1",
            &expect![[r#"
                BinExpr@0..6
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  AMP_EQ@2..4 "&="
                  LiteralExpr@4..6
                    WHITESPACE@4..5 " "
                    INT_LITERAL@5..6 "1"
            "#]],
        );
    }

    #[test]
    fn compound_shl_eq() {
        check_expr(
            "x <<= 1",
            &expect![[r#"
                BinExpr@0..7
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  SHL_EQ@2..5 "<<="
                  LiteralExpr@5..7
                    WHITESPACE@5..6 " "
                    INT_LITERAL@6..7 "1"
            "#]],
        );
    }

    #[test]
    fn compound_shr_eq() {
        check_expr(
            "x >>= 1",
            &expect![[r#"
                BinExpr@0..7
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "x"
                  WHITESPACE@1..2 " "
                  SHR_EQ@2..5 ">>="
                  LiteralExpr@5..7
                    WHITESPACE@5..6 " "
                    INT_LITERAL@6..7 "1"
            "#]],
        );
    }

    // === Integration Tests: Precedence Chains ===

    #[test]
    fn precedence_bitwise_chain() {
        // a | b ^ c & d << 2 parses as a | (b ^ (c & (d << 2)))
        check_expr(
            "a|b^c&d<<2",
            &expect![[r#"
                BinExpr@0..10
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  PIPE@1..2 "|"
                  BinExpr@2..10
                    PathExpr@2..3
                      Path@2..3
                        PathSegment@2..3
                          NameRef@2..3
                            IDENT@2..3 "b"
                    CARET@3..4 "^"
                    BinExpr@4..10
                      PathExpr@4..5
                        Path@4..5
                          PathSegment@4..5
                            NameRef@4..5
                              IDENT@4..5 "c"
                      AMP@5..6 "&"
                      BinExpr@6..10
                        PathExpr@6..7
                          Path@6..7
                            PathSegment@6..7
                              NameRef@6..7
                                IDENT@6..7 "d"
                        SHL@7..9 "<<"
                        LiteralExpr@9..10
                          INT_LITERAL@9..10 "2"
            "#]],
        );
    }

    #[test]
    fn precedence_coalesce_with_logical() {
        // a ?? b || c && d parses as (a ?? (b || (c && d)))
        // Since coalesce has lowest precedence (after assign), and || binds tighter than ??, etc.
        check_expr(
            "a??b||c&&d",
            &expect![[r#"
                BinExpr@0..10
                  PathExpr@0..1
                    Path@0..1
                      PathSegment@0..1
                        NameRef@0..1
                          IDENT@0..1 "a"
                  QUESTION_QUESTION@1..3 "??"
                  BinExpr@3..10
                    PathExpr@3..4
                      Path@3..4
                        PathSegment@3..4
                          NameRef@3..4
                            IDENT@3..4 "b"
                    OR_OR@4..6 "||"
                    BinExpr@6..10
                      PathExpr@6..7
                        Path@6..7
                          PathSegment@6..7
                            NameRef@6..7
                              IDENT@6..7 "c"
                      AND_AND@7..9 "&&"
                      PathExpr@9..10
                        Path@9..10
                          PathSegment@9..10
                            NameRef@9..10
                              IDENT@9..10 "d"
            "#]],
        );
    }

    #[test]
    fn precedence_exponentiation_chain_with_mult() {
        // 2 ** 3 ** 2 * 4 parses as (2 ** (3 ** 2)) * 4 since ** is right-assoc
        // and ** binds tighter than *
        check_expr(
            "2**3**2*4",
            &expect![[r#"
                BinExpr@0..9
                  BinExpr@0..7
                    LiteralExpr@0..1
                      INT_LITERAL@0..1 "2"
                    STAR_STAR@1..3 "**"
                    BinExpr@3..7
                      LiteralExpr@3..4
                        INT_LITERAL@3..4 "3"
                      STAR_STAR@4..6 "**"
                      LiteralExpr@6..7
                        INT_LITERAL@6..7 "2"
                  STAR@7..8 "*"
                  LiteralExpr@8..9
                    INT_LITERAL@8..9 "4"
            "#]],
        );
    }

    #[test]
    fn tilde_in_expression() {
        // ~a & b parses as (~a) & b
        check_expr(
            "~a&b",
            &expect![[r#"
                BinExpr@0..4
                  PrefixExpr@0..2
                    TILDE@0..1 "~"
                    PathExpr@1..2
                      Path@1..2
                        PathSegment@1..2
                          NameRef@1..2
                            IDENT@1..2 "a"
                  AMP@2..3 "&"
                  PathExpr@3..4
                    Path@3..4
                      PathSegment@3..4
                        NameRef@3..4
                          IDENT@3..4 "b"
            "#]],
        );
    }
}
