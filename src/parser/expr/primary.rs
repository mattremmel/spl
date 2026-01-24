//! Primary expression parsing: literals, paths, structs, collections.

use crate::parser::{CompletedMarker, Marker, Parser};
use crate::syntax::SyntaxKind;

use super::control_flow::{
    block_expr, break_expr, continue_expr, for_expr, if_expr, loop_expr, return_expr, while_expr,
};
use super::expr;

/// Parse a primary expression.
///
/// The `depth` parameter tracks recursion depth to prevent stack overflow.
pub(super) fn primary_expr(
    p: &mut Parser<'_>,
    allow_struct: bool,
    _depth: usize,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    match_token!(p, {
        // Literals
        INT_LITERAL | FLOAT_LITERAL | STRING_LITERAL | CHAR_LITERAL | TRUE_KW | FALSE_KW => {
            literal_expr(p)
        },
        // Identifier / path (including path-starting keywords like module, super, crate)
        IDENT | SELF_VALUE_KW | SELF_TYPE_KW | MODULE_KW | SUPER_KW | CRATE_KW => {
            if allow_struct {
                path_or_struct_expr(p)
            } else {
                path_expr_only(p)
            }
        },
        // Grouped or tuple expression
        L_PAREN => paren_or_tuple_expr(p),
        // Array expression
        L_BRACKET => array_expr(p),
        // Block expression
        L_BRACE => block_expr(p),
        // Control flow
        IF_KW => if_expr(p),
        WHILE_KW => while_expr(p),
        FOR_KW => for_expr(p),
        LOOP_KW => loop_expr(p),
        BREAK_KW => break_expr(p),
        CONTINUE_KW => continue_expr(p),
        RETURN_KW => return_expr(p),
        // Match expression
        MATCH_KW => match_expr(p),
        _ => Ok(None),
    })
}

/// Parse a match expression: `match expr { arms }`
pub(super) fn match_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::MATCH_KW)?;

    // Scrutinee expression (no struct expressions allowed)
    super::expr_no_struct(p)?;

    // Match body
    p.expect(SyntaxKind::L_BRACE)?;

    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        match_arm(p)?;
    }

    p.expect(SyntaxKind::R_BRACE)?;

    Ok(Some(m.complete(p, SyntaxKind::MatchExpr)))
}

/// Parse a match arm: `pattern [if guard] => expr,`
fn match_arm(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Pattern
    crate::parser::pattern::pattern(p)?;

    // Optional guard: `if condition`
    if p.eat(SyntaxKind::IF_KW) {
        expr(p)?;
    }

    // Arrow and body expression
    p.expect(SyntaxKind::FAT_ARROW)?;
    expr(p)?;

    // Match arms are separated by commas
    p.eat(SyntaxKind::COMMA);

    Ok(m.complete(p, SyntaxKind::MatchArm))
}

/// Parse a literal expression.
pub(super) fn literal_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.bump();
    Ok(Some(m.complete(p, SyntaxKind::LiteralExpr)))
}

/// Parse a path or apply expression.
///
/// `ApplyExpr` is a unified syntax for both function calls and struct instantiation.
/// The parser produces `ApplyExpr` for `Path(args...)` syntax, and semantic analysis
/// determines whether it's a function call or struct based on what the path resolves to.
pub(super) fn path_or_struct_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in expression position)
    crate::parser::path::path_no_generics(p)?;

    // Check for apply expression: Path(args...)
    // This unified syntax handles both function calls and struct instantiation.
    // Semantic analysis will determine which it is based on the path's resolution.
    if p.at(SyntaxKind::L_PAREN) {
        return apply_expr_rest(p, m);
    }

    Ok(Some(m.complete(p, SyntaxKind::PathExpr)))
}

/// Parse a path expression only (no struct expression).
/// Used in control flow contexts where `identifier {` should be parsed as
/// identifier followed by block, not as a struct expression.
pub(super) fn path_expr_only(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in expression position)
    crate::parser::path::path_no_generics(p)?;

    // NO struct check - just return PathExpr
    Ok(Some(m.complete(p, SyntaxKind::PathExpr)))
}

/// Parse the rest of an apply expression after the path.
/// Syntax: Path(arg, name: value, ...)
///
/// Arguments can be:
/// - Named: `name: value`
/// - Positional: just `value`
///
/// Struct update syntax `..base` is also supported for struct instantiation.
fn apply_expr_rest(
    p: &mut Parser<'_>,
    m: Marker,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    p.expect(SyntaxKind::L_PAREN)?;

    while !p.at(SyntaxKind::R_PAREN) && p.current().is_some() {
        // Check for struct update syntax: ..base
        if p.at(SyntaxKind::DOT_DOT) {
            struct_update_base(p)?;
            // Struct update base must be last
            break;
        }

        apply_arg(p)?;
        if !p.at(SyntaxKind::R_PAREN) && !p.eat(SyntaxKind::COMMA) {
            break;
        }
    }

    p.expect(SyntaxKind::R_PAREN)?;
    Ok(Some(m.complete(p, SyntaxKind::ApplyExpr)))
}

/// Parse struct update base: ..expr
fn struct_update_base(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::DOT_DOT)?;
    let _ = expr(p)?;
    Ok(m.complete(p, SyntaxKind::StructUpdateBase))
}

/// Parse an apply argument: either `name: expr` (named) or just `expr` (positional).
fn apply_arg(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Check for named argument: IDENT :
    if p.at(SyntaxKind::IDENT) && p.peek(1) == Some(SyntaxKind::COLON) {
        p.bump(); // name
        p.bump(); // :
    }

    // Parse the value expression
    let _ = expr(p)?;

    Ok(m.complete(p, SyntaxKind::ApplyArg))
}

/// Parse a parenthesized or tuple expression.
pub(super) fn paren_or_tuple_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;

    // Empty tuple
    if p.at(SyntaxKind::R_PAREN) {
        p.bump();
        return Ok(Some(m.complete(p, SyntaxKind::TupleExpr)));
    }

    // Parse first expression
    let _ = expr(p)?;

    // Check for tuple (comma) or just grouped expression
    if p.at(SyntaxKind::COMMA) {
        // Tuple
        while p.eat(SyntaxKind::COMMA) {
            if p.at(SyntaxKind::R_PAREN) {
                break; // trailing comma
            }
            let _ = expr(p)?;
        }
        p.expect(SyntaxKind::R_PAREN)?;
        Ok(Some(m.complete(p, SyntaxKind::TupleExpr)))
    } else {
        // Grouped expression
        p.expect(SyntaxKind::R_PAREN)?;
        Ok(Some(m.complete(p, SyntaxKind::ParenExpr)))
    }
}

/// Parse an array expression.
pub(super) fn array_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_BRACKET)?;

    // Empty array
    if p.at(SyntaxKind::R_BRACKET) {
        p.bump();
        return Ok(Some(m.complete(p, SyntaxKind::ArrayExpr)));
    }

    // Parse first expression
    let _ = expr(p)?;

    // Check for repeat syntax [expr; count]
    if p.at(SyntaxKind::SEMI) {
        p.bump();
        let _ = expr(p)?;
        p.expect(SyntaxKind::R_BRACKET)?;
        return Ok(Some(m.complete(p, SyntaxKind::ArrayExpr)));
    }

    // Array literal [a, b, c]
    while p.eat(SyntaxKind::COMMA) {
        if p.at(SyntaxKind::R_BRACKET) {
            break;
        }
        let _ = expr(p)?;
    }

    p.expect(SyntaxKind::R_BRACKET)?;
    Ok(Some(m.complete(p, SyntaxKind::ArrayExpr)))
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::check_expr;
    use expect_test::expect;

    #[test]
    fn literal_int() {
        check_expr(
            "42",
            &expect![[r#"
                LiteralExpr@0..2
                  INT_LITERAL@0..2 "42"
            "#]],
        );
    }

    #[test]
    fn literal_float() {
        check_expr(
            "3.14",
            &expect![[r#"
                LiteralExpr@0..4
                  FLOAT_LITERAL@0..4 "3.14"
            "#]],
        );
    }

    #[test]
    fn literal_string() {
        check_expr(
            r#""hello""#,
            &expect![[r#"
                LiteralExpr@0..7
                  STRING_LITERAL@0..7 "\"hello\""
            "#]],
        );
    }

    #[test]
    fn literal_bool() {
        check_expr(
            "true",
            &expect![[r#"
                LiteralExpr@0..4
                  TRUE_KW@0..4 "true"
            "#]],
        );
    }

    #[test]
    fn literal_char() {
        check_expr(
            "'a'",
            &expect![[r#"
                LiteralExpr@0..3
                  CHAR_LITERAL@0..3 "'a'"
            "#]],
        );
    }

    #[test]
    fn literal_char_escape() {
        check_expr(
            r"'\n'",
            &expect![[r#"
                LiteralExpr@0..4
                  CHAR_LITERAL@0..4 "'\\n'"
            "#]],
        );
    }

    #[test]
    fn literal_false() {
        check_expr(
            "false",
            &expect![[r#"
                LiteralExpr@0..5
                  FALSE_KW@0..5 "false"
            "#]],
        );
    }

    #[test]
    fn literal_int_zero() {
        check_expr(
            "0",
            &expect![[r#"
                LiteralExpr@0..1
                  INT_LITERAL@0..1 "0"
            "#]],
        );
    }

    #[test]
    fn literal_int_large() {
        check_expr(
            "999999999999",
            &expect![[r#"
                LiteralExpr@0..12
                  INT_LITERAL@0..12 "999999999999"
            "#]],
        );
    }

    #[test]
    fn literal_int_underscores() {
        check_expr(
            "1_000_000",
            &expect![[r#"
                LiteralExpr@0..9
                  INT_LITERAL@0..9 "1_000_000"
            "#]],
        );
    }

    #[test]
    fn literal_int_hex() {
        check_expr(
            "0xFF",
            &expect![[r#"
                LiteralExpr@0..4
                  INT_LITERAL@0..4 "0xFF"
            "#]],
        );
    }

    #[test]
    fn literal_int_binary() {
        check_expr(
            "0b1010",
            &expect![[r#"
                LiteralExpr@0..6
                  INT_LITERAL@0..6 "0b1010"
            "#]],
        );
    }

    #[test]
    fn literal_int_octal() {
        check_expr(
            "0o755",
            &expect![[r#"
                LiteralExpr@0..5
                  INT_LITERAL@0..5 "0o755"
            "#]],
        );
    }

    #[test]
    fn literal_float_zero() {
        check_expr(
            "0.0",
            &expect![[r#"
                LiteralExpr@0..3
                  FLOAT_LITERAL@0..3 "0.0"
            "#]],
        );
    }

    #[test]
    fn literal_float_exponent() {
        check_expr(
            "1e10",
            &expect![[r#"
                LiteralExpr@0..4
                  FLOAT_LITERAL@0..4 "1e10"
            "#]],
        );
    }

    #[test]
    fn literal_float_exponent_positive() {
        check_expr(
            "1e+10",
            &expect![[r#"
                LiteralExpr@0..5
                  FLOAT_LITERAL@0..5 "1e+10"
            "#]],
        );
    }

    #[test]
    fn literal_float_exponent_negative() {
        check_expr(
            "2e-3",
            &expect![[r#"
                LiteralExpr@0..4
                  FLOAT_LITERAL@0..4 "2e-3"
            "#]],
        );
    }

    #[test]
    fn literal_float_full() {
        check_expr(
            "2.5e-3",
            &expect![[r#"
                LiteralExpr@0..6
                  FLOAT_LITERAL@0..6 "2.5e-3"
            "#]],
        );
    }

    #[test]
    fn literal_string_empty() {
        check_expr(
            r#""""#,
            &expect![[r#"
                LiteralExpr@0..2
                  STRING_LITERAL@0..2 "\"\""
            "#]],
        );
    }

    #[test]
    fn literal_string_escapes() {
        check_expr(
            r#""\n\t\r\\\"\'""#,
            &expect![[r#"
                LiteralExpr@0..14
                  STRING_LITERAL@0..14 "\"\\n\\t\\r\\\\\\\"\\'\""
            "#]],
        );
    }

    #[test]
    fn literal_char_newline() {
        check_expr(
            r"'\n'",
            &expect![[r#"
                LiteralExpr@0..4
                  CHAR_LITERAL@0..4 "'\\n'"
            "#]],
        );
    }

    #[test]
    fn literal_char_tab() {
        check_expr(
            r"'\t'",
            &expect![[r#"
                LiteralExpr@0..4
                  CHAR_LITERAL@0..4 "'\\t'"
            "#]],
        );
    }

    #[test]
    fn literal_char_null() {
        check_expr(
            r"'\0'",
            &expect![[r#"
                LiteralExpr@0..4
                  CHAR_LITERAL@0..4 "'\\0'"
            "#]],
        );
    }

    #[test]
    fn paren_expr() {
        check_expr(
            "(1+2)",
            &expect![[r#"
                ParenExpr@0..5
                  L_PAREN@0..1 "("
                  BinExpr@1..4
                    LiteralExpr@1..2
                      INT_LITERAL@1..2 "1"
                    PLUS@2..3 "+"
                    LiteralExpr@3..4
                      INT_LITERAL@3..4 "2"
                  R_PAREN@4..5 ")"
            "#]],
        );
    }

    #[test]
    fn tuple_expr() {
        check_expr(
            "(1, 2)",
            &expect![[r#"
                TupleExpr@0..6
                  L_PAREN@0..1 "("
                  LiteralExpr@1..2
                    INT_LITERAL@1..2 "1"
                  COMMA@2..3 ","
                  LiteralExpr@3..5
                    WHITESPACE@3..4 " "
                    INT_LITERAL@4..5 "2"
                  R_PAREN@5..6 ")"
            "#]],
        );
    }

    #[test]
    fn tuple_empty() {
        check_expr(
            "()",
            &expect![[r#"
                TupleExpr@0..2
                  L_PAREN@0..1 "("
                  R_PAREN@1..2 ")"
            "#]],
        );
    }

    #[test]
    fn tuple_single_with_comma() {
        check_expr(
            "(1,)",
            &expect![[r#"
                TupleExpr@0..4
                  L_PAREN@0..1 "("
                  LiteralExpr@1..2
                    INT_LITERAL@1..2 "1"
                  COMMA@2..3 ","
                  R_PAREN@3..4 ")"
            "#]],
        );
    }

    #[test]
    fn tuple_trailing_comma() {
        check_expr(
            "(1, 2, 3,)",
            &expect![[r#"
                TupleExpr@0..10
                  L_PAREN@0..1 "("
                  LiteralExpr@1..2
                    INT_LITERAL@1..2 "1"
                  COMMA@2..3 ","
                  LiteralExpr@3..5
                    WHITESPACE@3..4 " "
                    INT_LITERAL@4..5 "2"
                  COMMA@5..6 ","
                  LiteralExpr@6..8
                    WHITESPACE@6..7 " "
                    INT_LITERAL@7..8 "3"
                  COMMA@8..9 ","
                  R_PAREN@9..10 ")"
            "#]],
        );
    }

    #[test]
    fn tuple_nested() {
        check_expr(
            "((1, 2), (3, 4))",
            &expect![[r#"
                TupleExpr@0..16
                  L_PAREN@0..1 "("
                  TupleExpr@1..7
                    L_PAREN@1..2 "("
                    LiteralExpr@2..3
                      INT_LITERAL@2..3 "1"
                    COMMA@3..4 ","
                    LiteralExpr@4..6
                      WHITESPACE@4..5 " "
                      INT_LITERAL@5..6 "2"
                    R_PAREN@6..7 ")"
                  COMMA@7..8 ","
                  TupleExpr@8..15
                    WHITESPACE@8..9 " "
                    L_PAREN@9..10 "("
                    LiteralExpr@10..11
                      INT_LITERAL@10..11 "3"
                    COMMA@11..12 ","
                    LiteralExpr@12..14
                      WHITESPACE@12..13 " "
                      INT_LITERAL@13..14 "4"
                    R_PAREN@14..15 ")"
                  R_PAREN@15..16 ")"
            "#]],
        );
    }

    #[test]
    fn array_expr() {
        check_expr(
            "[1, 2, 3]",
            &expect![[r#"
                ArrayExpr@0..9
                  L_BRACKET@0..1 "["
                  LiteralExpr@1..2
                    INT_LITERAL@1..2 "1"
                  COMMA@2..3 ","
                  LiteralExpr@3..5
                    WHITESPACE@3..4 " "
                    INT_LITERAL@4..5 "2"
                  COMMA@5..6 ","
                  LiteralExpr@6..8
                    WHITESPACE@6..7 " "
                    INT_LITERAL@7..8 "3"
                  R_BRACKET@8..9 "]"
            "#]],
        );
    }

    #[test]
    fn array_empty() {
        check_expr(
            "[]",
            &expect![[r#"
                ArrayExpr@0..2
                  L_BRACKET@0..1 "["
                  R_BRACKET@1..2 "]"
            "#]],
        );
    }

    #[test]
    fn array_repeat_syntax() {
        check_expr(
            "[0; 10]",
            &expect![[r#"
                ArrayExpr@0..7
                  L_BRACKET@0..1 "["
                  LiteralExpr@1..2
                    INT_LITERAL@1..2 "0"
                  SEMI@2..3 ";"
                  LiteralExpr@3..6
                    WHITESPACE@3..4 " "
                    INT_LITERAL@4..6 "10"
                  R_BRACKET@6..7 "]"
            "#]],
        );
    }

    #[test]
    fn array_single_element() {
        check_expr(
            "[42]",
            &expect![[r#"
                ArrayExpr@0..4
                  L_BRACKET@0..1 "["
                  LiteralExpr@1..3
                    INT_LITERAL@1..3 "42"
                  R_BRACKET@3..4 "]"
            "#]],
        );
    }

    #[test]
    fn array_trailing_comma() {
        check_expr(
            "[1, 2, 3,]",
            &expect![[r#"
                ArrayExpr@0..10
                  L_BRACKET@0..1 "["
                  LiteralExpr@1..2
                    INT_LITERAL@1..2 "1"
                  COMMA@2..3 ","
                  LiteralExpr@3..5
                    WHITESPACE@3..4 " "
                    INT_LITERAL@4..5 "2"
                  COMMA@5..6 ","
                  LiteralExpr@6..8
                    WHITESPACE@6..7 " "
                    INT_LITERAL@7..8 "3"
                  COMMA@8..9 ","
                  R_BRACKET@9..10 "]"
            "#]],
        );
    }

    #[test]
    fn array_nested() {
        check_expr(
            "[[1, 2], [3, 4]]",
            &expect![[r#"
                ArrayExpr@0..16
                  L_BRACKET@0..1 "["
                  ArrayExpr@1..7
                    L_BRACKET@1..2 "["
                    LiteralExpr@2..3
                      INT_LITERAL@2..3 "1"
                    COMMA@3..4 ","
                    LiteralExpr@4..6
                      WHITESPACE@4..5 " "
                      INT_LITERAL@5..6 "2"
                    R_BRACKET@6..7 "]"
                  COMMA@7..8 ","
                  ArrayExpr@8..15
                    WHITESPACE@8..9 " "
                    L_BRACKET@9..10 "["
                    LiteralExpr@10..11
                      INT_LITERAL@10..11 "3"
                    COMMA@11..12 ","
                    LiteralExpr@12..14
                      WHITESPACE@12..13 " "
                      INT_LITERAL@13..14 "4"
                    R_BRACKET@14..15 "]"
                  R_BRACKET@15..16 "]"
            "#]],
        );
    }

    #[test]
    fn struct_expr_simple() {
        check_expr(
            "Point { x: 1, y: 2 }",
            &expect![[r#"
                PathExpr@0..6
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Point"
                  WHITESPACE@5..6 " "
            "#]],
        );
    }

    #[test]
    fn struct_expr_shorthand() {
        check_expr(
            "Point { x, y }",
            &expect![[r#"
                PathExpr@0..6
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Point"
                  WHITESPACE@5..6 " "
            "#]],
        );
    }

    #[test]
    fn struct_trailing_comma() {
        check_expr(
            "Point { x: 1, y: 2, }",
            &expect![[r#"
                PathExpr@0..6
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Point"
                  WHITESPACE@5..6 " "
            "#]],
        );
    }

    #[test]
    fn struct_mixed_shorthand() {
        check_expr(
            "Point { x, y: other_y }",
            &expect![[r#"
                PathExpr@0..6
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Point"
                  WHITESPACE@5..6 " "
            "#]],
        );
    }

    #[test]
    fn struct_nested() {
        check_expr(
            "Outer { inner: Inner { x: 1 } }",
            &expect![[r#"
                PathExpr@0..6
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Outer"
                  WHITESPACE@5..6 " "
            "#]],
        );
    }

    #[test]
    fn struct_with_path() {
        check_expr(
            "module.Point { x: 1 }",
            &expect![[r#"
                PathExpr@0..13
                  Path@0..12
                    PathSegment@0..6
                      NameRef@0..6
                        MODULE_KW@0..6 "module"
                    DOT@6..7 "."
                    PathSegment@7..12
                      NameRef@7..12
                        IDENT@7..12 "Point"
                  WHITESPACE@12..13 " "
            "#]],
        );
    }

    #[test]
    fn self_value_expr() {
        check_expr(
            "self",
            &expect![[r#"
                PathExpr@0..4
                  Path@0..4
                    PathSegment@0..4
                      NameRef@0..4
                        SELF_VALUE_KW@0..4 "self"
            "#]],
        );
    }

    #[test]
    fn self_field_access() {
        check_expr(
            "self.x",
            &expect![[r#"
                PathExpr@0..6
                  Path@0..6
                    PathSegment@0..4
                      NameRef@0..4
                        SELF_VALUE_KW@0..4 "self"
                    DOT@4..5 "."
                    PathSegment@5..6
                      NameRef@5..6
                        IDENT@5..6 "x"
            "#]],
        );
    }

    #[test]
    fn block_expr_empty() {
        check_expr(
            "{ }",
            &expect![[r#"
                BlockExpr@0..3
                  Block@0..3
                    L_BRACE@0..1 "{"
                    WHITESPACE@1..2 " "
                    R_BRACE@2..3 "}"
            "#]],
        );
    }

    #[test]
    fn block_expr_simple() {
        check_expr(
            "{ 42 }",
            &expect![[r#"
                BlockExpr@0..6
                  Block@0..6
                    L_BRACE@0..1 "{"
                    LiteralExpr@1..4
                      WHITESPACE@1..2 " "
                      INT_LITERAL@2..4 "42"
                    WHITESPACE@4..5 " "
                    R_BRACE@5..6 "}"
            "#]],
        );
    }

    // === New Syntax: Match Expression ===

    #[test]
    fn match_expr_simple() {
        check_expr(
            "match x { 1 => 2, }",
            &expect![[r#"
                MatchExpr@0..19
                  MATCH_KW@0..5 "match"
                  PathExpr@5..7
                    Path@5..7
                      PathSegment@5..7
                        NameRef@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                  WHITESPACE@7..8 " "
                  L_BRACE@8..9 "{"
                  MatchArm@9..17
                    LiteralPat@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "1"
                    WHITESPACE@11..12 " "
                    FAT_ARROW@12..14 "=>"
                    LiteralExpr@14..16
                      WHITESPACE@14..15 " "
                      INT_LITERAL@15..16 "2"
                    COMMA@16..17 ","
                  WHITESPACE@17..18 " "
                  R_BRACE@18..19 "}"
            "#]],
        );
    }

    #[test]
    fn match_expr_multiple_arms() {
        check_expr(
            "match x { Some(v) => v, None => 0, }",
            &expect![[r#"
                MatchExpr@0..36
                  MATCH_KW@0..5 "match"
                  PathExpr@5..7
                    Path@5..7
                      PathSegment@5..7
                        NameRef@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                  WHITESPACE@7..8 " "
                  L_BRACE@8..9 "{"
                  MatchArm@9..23
                    TuplePat@9..17
                      Path@9..14
                        PathSegment@9..14
                          NameRef@9..14
                            WHITESPACE@9..10 " "
                            IDENT@10..14 "Some"
                      L_PAREN@14..15 "("
                      IdentPat@15..16
                        Name@15..16
                          IDENT@15..16 "v"
                      R_PAREN@16..17 ")"
                    WHITESPACE@17..18 " "
                    FAT_ARROW@18..20 "=>"
                    PathExpr@20..22
                      Path@20..22
                        PathSegment@20..22
                          NameRef@20..22
                            WHITESPACE@20..21 " "
                            IDENT@21..22 "v"
                    COMMA@22..23 ","
                  MatchArm@23..34
                    IdentPat@23..28
                      Name@23..28
                        WHITESPACE@23..24 " "
                        IDENT@24..28 "None"
                    WHITESPACE@28..29 " "
                    FAT_ARROW@29..31 "=>"
                    LiteralExpr@31..33
                      WHITESPACE@31..32 " "
                      INT_LITERAL@32..33 "0"
                    COMMA@33..34 ","
                  WHITESPACE@34..35 " "
                  R_BRACE@35..36 "}"
            "#]],
        );
    }

    #[test]
    fn match_expr_with_guard() {
        check_expr(
            "match x { n if n > 0 => 1, _ => 0, }",
            &expect![[r#"
                MatchExpr@0..36
                  MATCH_KW@0..5 "match"
                  PathExpr@5..7
                    Path@5..7
                      PathSegment@5..7
                        NameRef@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                  WHITESPACE@7..8 " "
                  L_BRACE@8..9 "{"
                  MatchArm@9..26
                    IdentPat@9..11
                      Name@9..11
                        WHITESPACE@9..10 " "
                        IDENT@10..11 "n"
                    WHITESPACE@11..12 " "
                    IF_KW@12..14 "if"
                    BinExpr@14..20
                      PathExpr@14..16
                        Path@14..16
                          PathSegment@14..16
                            NameRef@14..16
                              WHITESPACE@14..15 " "
                              IDENT@15..16 "n"
                      WHITESPACE@16..17 " "
                      GT@17..18 ">"
                      LiteralExpr@18..20
                        WHITESPACE@18..19 " "
                        INT_LITERAL@19..20 "0"
                    WHITESPACE@20..21 " "
                    FAT_ARROW@21..23 "=>"
                    LiteralExpr@23..25
                      WHITESPACE@23..24 " "
                      INT_LITERAL@24..25 "1"
                    COMMA@25..26 ","
                  MatchArm@26..34
                    WildcardPat@26..28
                      WHITESPACE@26..27 " "
                      IDENT@27..28 "_"
                    WHITESPACE@28..29 " "
                    FAT_ARROW@29..31 "=>"
                    LiteralExpr@31..33
                      WHITESPACE@31..32 " "
                      INT_LITERAL@32..33 "0"
                    COMMA@33..34 ","
                  WHITESPACE@34..35 " "
                  R_BRACE@35..36 "}"
            "#]],
        );
    }
}
