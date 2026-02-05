//! Primary expression parsing: literals, paths, structs, collections.

use crate::{CompletedMarker, Marker, Parser};
use spl_syntax::SyntaxKind;

use super::control_flow::{
    block_expr, break_expr, continue_expr, for_expr, if_expr, labeled_expr, loop_expr, return_expr,
    throw_expr, unsafe_expr, while_expr, yield_expr,
};
use super::expr;

/// Parse a primary expression.
///
/// The `depth` parameter tracks recursion depth to prevent stack overflow.
pub(super) fn primary_expr(
    p: &mut Parser<'_>,
    allow_struct: bool,
    _depth: usize,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    match_token!(p, {
        // Literals
        INT_LITERAL | FLOAT_LITERAL
        | STRING_LITERAL | RAW_STRING_LITERAL | BYTE_STRING_LITERAL
        | RAW_BYTE_STRING_LITERAL | C_STRING_LITERAL
        | CHAR_LITERAL | BYTE_CHAR_LITERAL
        | TRUE_KW | FALSE_KW => {
            Ok(Some(literal_expr(p)))
        },
        // Identifier / path (including path-starting keywords like module, super)
        // Note: 'crate' keyword was removed - use '$' for package root
        IDENT | SELF_VALUE_KW | SELF_TYPE_KW | MODULE_KW | SUPER_KW => {
            if allow_struct {
                path_or_struct_expr(p)
            } else {
                path_expr_only(p)
            }
        },
        // Enum shorthand: .Variant or .Variant(args)
        DOT => {
            // Check if followed by identifier (enum shorthand)
            if p.peek(1) == Some(SyntaxKind::IDENT) {
                enum_shorthand_expr(p)
            } else {
                Ok(None) // Not a primary expression (could be range like `..`)
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
        YIELD_KW => yield_expr(p),
        // Unsafe block
        UNSAFE_KW => unsafe_expr(p),
        // Throw expression
        THROW_KW => throw_expr(p),
        // Match expression
        MATCH_KW => match_expr(p),
        // Closure expressions: || body, |params| body
        PIPE | OR_OR => closure_expr(p),
        // Closure with capture list: @[captures] |params| body
        AT => {
            if p.peek(1) == Some(SyntaxKind::L_BRACKET) {
                closure_expr(p)
            } else {
                Ok(None)
            }
        },
        // Dollar expression: $ represents array length in index expressions
        DOLLAR => Ok(Some(dollar_expr(p))),
        // Labeled expression: 'label: loop/while/for/block
        TICK => labeled_expr(p),
        _ => Ok(None),
    })
}

/// Parse a closure expression.
///
/// Grammar:
/// ```ebnf
/// ClosureExpr = [ "@" CaptureList ] ClosureParams ClosureBody ;
/// CaptureList = "[" [ Capture { "," Capture } [ "," ] ] "]" ;
/// Capture = IDENTIFIER | IDENTIFIER ":" Expression ;
/// ClosureParams = "||" | "|" [ ParamList ] "|" ;
/// ClosureBody = Block | Expression ;
/// ```
fn closure_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let m = p.start();

    // Optional capture list: @[captures]
    if p.at(SyntaxKind::AT)
        && let Err(e) = capture_list(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Closure params: || or |params|
    if let Err(e) = closure_params(p) {
        m.abandon(p);
        return Err(e);
    }

    // Closure body: block or expression
    if let Err(e) = expr(p) {
        m.abandon(p);
        return Err(e);
    }

    Ok(Some(m.complete(p, SyntaxKind::ClosureExpr)))
}

/// Parse a capture list: @[x, y, val: expr]
fn capture_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    if let Err(e) = p.expect(SyntaxKind::AT) {
        m.abandon(p);
        return Err(e);
    }

    if let Err(e) = p.expect(SyntaxKind::L_BRACKET) {
        m.abandon(p);
        return Err(e);
    }

    while !p.at(SyntaxKind::R_BRACKET) && p.current().is_some() {
        if let Err(e) = capture(p) {
            m.abandon(p);
            return Err(e);
        }
        if !p.at(SyntaxKind::R_BRACKET) && !p.eat(SyntaxKind::COMMA) {
            break;
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACKET) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::CaptureList))
}

/// Parse a single capture: identifier or identifier: expr
fn capture(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Parse name
    if let Err(e) = crate::item::name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional: expression
    if p.eat(SyntaxKind::COLON)
        && let Err(e) = expr(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::Capture))
}

/// Parse closure params: || or |param, param, ...|
fn closure_params(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Handle || (empty params)
    if p.eat(SyntaxKind::OR_OR) {
        return Ok(m.complete(p, SyntaxKind::ClosureParams));
    }

    // Otherwise we expect |params|
    if let Err(e) = p.expect(SyntaxKind::PIPE) {
        m.abandon(p);
        return Err(e);
    }

    // Parse parameters
    while !p.at(SyntaxKind::PIPE) && p.current().is_some() {
        if let Err(e) = closure_param(p) {
            m.abandon(p);
            return Err(e);
        }
        if !p.at(SyntaxKind::PIPE) && !p.eat(SyntaxKind::COMMA) {
            break;
        }
    }

    if let Err(e) = p.expect(SyntaxKind::PIPE) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::ClosureParams))
}

/// Parse a single closure parameter: [mut] name [: type]
fn closure_param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional mut
    p.eat(SyntaxKind::MUT_KW);

    // Parse name
    if let Err(e) = crate::item::name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional type annotation
    if p.eat(SyntaxKind::COLON)
        && let Err(e) = crate::stmt::type_annotation(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::ClosureParam))
}

/// Parse an enum shorthand expression: `.Variant` or `.Variant(args)`
fn enum_shorthand_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let m = p.start();

    // Consume the leading dot
    if let Err(e) = p.expect(SyntaxKind::DOT) {
        m.abandon(p);
        return Err(e);
    }

    // Parse the variant name
    if let Err(e) = crate::item::name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional argument list: (arg, ...)
    if p.at(SyntaxKind::L_PAREN) {
        if let Err(e) = p.expect(SyntaxKind::L_PAREN) {
            m.abandon(p);
            return Err(e);
        }

        while !p.at(SyntaxKind::R_PAREN) && p.current().is_some() {
            if let Err(e) = call_arg(p) {
                m.abandon(p);
                return Err(e);
            }
            if !p.at(SyntaxKind::R_PAREN) && !p.eat(SyntaxKind::COMMA) {
                break;
            }
        }

        if let Err(e) = p.expect(SyntaxKind::R_PAREN) {
            m.abandon(p);
            return Err(e);
        }
    }

    Ok(Some(m.complete(p, SyntaxKind::EnumShorthandExpr)))
}

/// Parse a match expression: `match expr { arms }`
pub(super) fn match_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::MATCH_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Scrutinee expression (no struct expressions allowed)
    if let Err(e) = super::expr_no_struct(p) {
        m.abandon(p);
        return Err(e);
    }

    // Match body
    if let Err(e) = p.expect(SyntaxKind::L_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        if let Err(e) = match_arm(p) {
            m.abandon(p);
            return Err(e);
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    Ok(Some(m.complete(p, SyntaxKind::MatchExpr)))
}

/// Parse a match arm: `pattern [if guard] => expr,`
fn match_arm(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Pattern
    if let Err(e) = crate::pattern::pattern(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional guard: `if condition`
    if p.eat(SyntaxKind::IF_KW)
        && let Err(e) = expr(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Arrow and body expression
    if let Err(e) = p.expect(SyntaxKind::FAT_ARROW) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = expr(p) {
        m.abandon(p);
        return Err(e);
    }

    // Match arms are separated by commas
    p.eat(SyntaxKind::COMMA);

    Ok(m.complete(p, SyntaxKind::MatchArm))
}

/// Parse a literal expression.
pub(super) fn literal_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump();
    m.complete(p, SyntaxKind::LiteralExpr)
}

/// Parse a dollar expression: `$` represents array length in index expressions.
/// Enables `arr[$-1]` syntax for last element access.
fn dollar_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // consume $
    m.complete(p, SyntaxKind::DollarExpr)
}

/// Parse a path or call expression.
///
/// `CallExpr` is a unified syntax for function calls, struct instantiation, and method calls.
/// The parser produces `CallExpr` for `Path(args...)` syntax, and semantic analysis
/// determines whether it's a function call or struct based on what the path resolves to.
pub(super) fn path_or_struct_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in expression position)
    if let Err(e) = crate::path::path_no_generics(p) {
        m.abandon(p);
        return Err(e);
    }

    // First complete as PathExpr (this wraps the Path in PathExpr)
    let path_expr = m.complete(p, SyntaxKind::PathExpr);

    // Check for call expression: PathExpr(args...)
    // This unified syntax handles both function calls and struct instantiation.
    // Semantic analysis will determine which it is based on the path's resolution.
    if p.at(SyntaxKind::L_PAREN) {
        // Wrap PathExpr in CallExpr
        let m = path_expr.precede(p);
        return call_expr_rest(p, m);
    }

    Ok(Some(path_expr))
}

/// Parse a path expression only (no struct expression).
/// Used in control flow contexts where `identifier {` should be parsed as
/// identifier followed by block, not as a struct expression.
pub(super) fn path_expr_only(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in expression position)
    if let Err(e) = crate::path::path_no_generics(p) {
        m.abandon(p);
        return Err(e);
    }

    // NO struct check - just return PathExpr
    Ok(Some(m.complete(p, SyntaxKind::PathExpr)))
}

/// Parse the rest of a call expression after the callee.
/// Syntax: callee(arg, name: value, ...)
///
/// Arguments can be:
/// - Named: `name: value`
/// - Positional: just `value`
///
/// Struct update syntax `...base` is also supported for struct instantiation.
pub(super) fn call_expr_rest(
    p: &mut Parser<'_>,
    m: Marker,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    if let Err(e) = p.expect(SyntaxKind::L_PAREN) {
        m.abandon(p);
        return Err(e);
    }

    while !p.at(SyntaxKind::R_PAREN) && p.current().is_some() {
        // Check for struct update syntax: ...base
        if p.at(SyntaxKind::ELLIPSIS) {
            if let Err(e) = struct_update_base(p) {
                m.abandon(p);
                return Err(e);
            }
            // Struct update base must be last
            break;
        }

        if let Err(e) = call_arg(p) {
            m.abandon(p);
            return Err(e);
        }
        if !p.at(SyntaxKind::R_PAREN) && !p.eat(SyntaxKind::COMMA) {
            break;
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_PAREN) {
        m.abandon(p);
        return Err(e);
    }
    Ok(Some(m.complete(p, SyntaxKind::CallExpr)))
}

/// Parse struct update base: ...expr
fn struct_update_base(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::ELLIPSIS) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = expr(p) {
        m.abandon(p);
        return Err(e);
    }
    Ok(m.complete(p, SyntaxKind::StructUpdateBase))
}

/// Parse a call argument: type arg, named value arg, or positional arg.
///
/// Per spec: `NamedArg = UPPER_IDENT ":" Type | LOWER_IDENT ":" Expression`
/// - Uppercase `IDENT :` → type argument (parsed as `Name : Type`, emits `TypeArg`)
/// - Lowercase `IDENT :` → named value argument (parsed as `name : expr`, emits `CallArg`)
/// - No `IDENT :` prefix → positional argument (parsed as `expr`, emits `CallArg`)
fn call_arg(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    // Check for type argument: UPPER_IDENT ":"
    if crate::path::is_upper_ident(p) && p.peek(1) == Some(SyntaxKind::COLON) {
        let m = p.start();
        if let Err(e) = crate::item::name(p) {
            m.abandon(p);
            return Err(e);
        }
        if let Err(e) = p.expect(SyntaxKind::COLON) {
            m.abandon(p);
            return Err(e);
        }
        if let Err(e) = crate::stmt::type_annotation(p) {
            m.abandon(p);
            return Err(e);
        }
        return Ok(m.complete(p, SyntaxKind::TypeArg));
    }

    let m = p.start();

    // Check for named value argument: lowercase IDENT ":"
    if p.at(SyntaxKind::IDENT) && p.peek(1) == Some(SyntaxKind::COLON) {
        p.bump(); // name
        p.bump(); // :
    }

    // Parse the value expression
    if let Err(e) = expr(p) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::CallArg))
}

/// Parse a parenthesized or tuple expression.
/// Check if we're at a named tuple element: `IDENT ":"` (lowercase identifier).
///
/// Per spec, `TupleExprElement = [ IDENTIFIER ":" ] Expression`.
/// Named elements with `:` make this unambiguously a tuple, not a grouped expression.
fn at_named_element(p: &mut Parser<'_>) -> bool {
    p.at(SyntaxKind::IDENT) && p.peek_at(1, SyntaxKind::COLON)
}

/// Parse a tuple expression element: optional `name:` prefix followed by expression.
fn tuple_expr_element(p: &mut Parser<'_>) -> Result<(), crate::ParseError> {
    if at_named_element(p) {
        // Named element: name: expr
        crate::item::name(p)?;
        p.expect(SyntaxKind::COLON)?;
    }
    expr(p)?;
    Ok(())
}

pub(super) fn paren_or_tuple_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::L_PAREN) {
        m.abandon(p);
        return Err(e);
    }

    // Empty tuple
    if p.at(SyntaxKind::R_PAREN) {
        p.bump();
        return Ok(Some(m.complete(p, SyntaxKind::TupleExpr)));
    }

    // Check if first element is named — if so, this is definitely a tuple
    let first_is_named = at_named_element(p);

    // Parse first element (possibly named)
    if first_is_named {
        if let Err(e) = tuple_expr_element(p) {
            m.abandon(p);
            return Err(e);
        }
    } else {
        // Parse as expression
        if let Err(e) = expr(p) {
            m.abandon(p);
            return Err(e);
        }
    }

    // Check for tuple (comma, or was named) vs grouped expression
    if p.at(SyntaxKind::COMMA) || first_is_named {
        // Tuple — parse remaining elements
        while p.eat(SyntaxKind::COMMA) {
            if p.at(SyntaxKind::R_PAREN) {
                break; // trailing comma
            }
            if let Err(e) = tuple_expr_element(p) {
                m.abandon(p);
                return Err(e);
            }
        }
        if let Err(e) = p.expect(SyntaxKind::R_PAREN) {
            m.abandon(p);
            return Err(e);
        }
        Ok(Some(m.complete(p, SyntaxKind::TupleExpr)))
    } else {
        // Grouped expression
        if let Err(e) = p.expect(SyntaxKind::R_PAREN) {
            m.abandon(p);
            return Err(e);
        }
        Ok(Some(m.complete(p, SyntaxKind::ParenExpr)))
    }
}

/// Parse an array expression.
pub(super) fn array_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::L_BRACKET) {
        m.abandon(p);
        return Err(e);
    }

    // Empty array
    if p.at(SyntaxKind::R_BRACKET) {
        p.bump();
        return Ok(Some(m.complete(p, SyntaxKind::ArrayExpr)));
    }

    // Parse first expression
    if let Err(e) = expr(p) {
        m.abandon(p);
        return Err(e);
    }

    // Check for repeat syntax [expr; count]
    if p.at(SyntaxKind::SEMI) {
        p.bump();
        if let Err(e) = expr(p) {
            m.abandon(p);
            return Err(e);
        }
        if let Err(e) = p.expect(SyntaxKind::R_BRACKET) {
            m.abandon(p);
            return Err(e);
        }
        return Ok(Some(m.complete(p, SyntaxKind::ArrayExpr)));
    }

    // Array literal [a, b, c]
    while p.eat(SyntaxKind::COMMA) {
        if p.at(SyntaxKind::R_BRACKET) {
            break;
        }
        if let Err(e) = expr(p) {
            m.abandon(p);
            return Err(e);
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACKET) {
        m.abandon(p);
        return Err(e);
    }
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

    // === Enum Shorthand Expressions ===

    #[test]
    fn enum_shorthand_unit() {
        check_expr(
            ".None",
            &expect![[r#"
                EnumShorthandExpr@0..5
                  DOT@0..1 "."
                  Name@1..5
                    IDENT@1..5 "None"
            "#]],
        );
    }

    #[test]
    fn enum_shorthand_tuple() {
        check_expr(
            ".Some(42)",
            &expect![[r#"
                EnumShorthandExpr@0..9
                  DOT@0..1 "."
                  Name@1..5
                    IDENT@1..5 "Some"
                  L_PAREN@5..6 "("
                  CallArg@6..8
                    LiteralExpr@6..8
                      INT_LITERAL@6..8 "42"
                  R_PAREN@8..9 ")"
            "#]],
        );
    }

    #[test]
    fn enum_shorthand_multiple_args() {
        check_expr(
            ".Point(1, 2)",
            &expect![[r#"
                EnumShorthandExpr@0..12
                  DOT@0..1 "."
                  Name@1..6
                    IDENT@1..6 "Point"
                  L_PAREN@6..7 "("
                  CallArg@7..8
                    LiteralExpr@7..8
                      INT_LITERAL@7..8 "1"
                  COMMA@8..9 ","
                  CallArg@9..11
                    LiteralExpr@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "2"
                  R_PAREN@11..12 ")"
            "#]],
        );
    }

    #[test]
    fn enum_shorthand_nested() {
        check_expr(
            ".Some(.Inner)",
            &expect![[r#"
                EnumShorthandExpr@0..13
                  DOT@0..1 "."
                  Name@1..5
                    IDENT@1..5 "Some"
                  L_PAREN@5..6 "("
                  CallArg@6..12
                    EnumShorthandExpr@6..12
                      DOT@6..7 "."
                      Name@7..12
                        IDENT@7..12 "Inner"
                  R_PAREN@12..13 ")"
            "#]],
        );
    }

    #[test]
    fn enum_shorthand_in_match_arm() {
        check_expr(
            "match x { _ => .Red }",
            &expect![[r#"
                MatchExpr@0..21
                  MATCH_KW@0..5 "match"
                  PathExpr@5..7
                    Path@5..7
                      PathSegment@5..7
                        NameRef@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                  WHITESPACE@7..8 " "
                  L_BRACE@8..9 "{"
                  MatchArm@9..19
                    WildcardPat@9..11
                      WHITESPACE@9..10 " "
                      IDENT@10..11 "_"
                    WHITESPACE@11..12 " "
                    FAT_ARROW@12..14 "=>"
                    EnumShorthandExpr@14..19
                      WHITESPACE@14..15 " "
                      DOT@15..16 "."
                      Name@16..19
                        IDENT@16..19 "Red"
                  WHITESPACE@19..20 " "
                  R_BRACE@20..21 "}"
            "#]],
        );
    }

    // === Closure Expressions ===

    #[test]
    fn closure_empty_params() {
        check_expr(
            "|| 42",
            &expect![[r#"
            ClosureExpr@0..5
              ClosureParams@0..2
                OR_OR@0..2 "||"
              LiteralExpr@2..5
                WHITESPACE@2..3 " "
                INT_LITERAL@3..5 "42"
        "#]],
        );
    }

    #[test]
    fn closure_single_param() {
        check_expr(
            "|x| x",
            &expect![[r#"
            ClosureExpr@0..5
              ClosureParams@0..3
                PIPE@0..1 "|"
                ClosureParam@1..2
                  Name@1..2
                    IDENT@1..2 "x"
                PIPE@2..3 "|"
              PathExpr@3..5
                Path@3..5
                  PathSegment@3..5
                    NameRef@3..5
                      WHITESPACE@3..4 " "
                      IDENT@4..5 "x"
        "#]],
        );
    }

    #[test]
    fn closure_multiple_params() {
        check_expr(
            "|a, b| a + b",
            &expect![[r#"
            ClosureExpr@0..12
              ClosureParams@0..6
                PIPE@0..1 "|"
                ClosureParam@1..2
                  Name@1..2
                    IDENT@1..2 "a"
                COMMA@2..3 ","
                ClosureParam@3..5
                  Name@3..5
                    WHITESPACE@3..4 " "
                    IDENT@4..5 "b"
                PIPE@5..6 "|"
              BinExpr@6..12
                PathExpr@6..8
                  Path@6..8
                    PathSegment@6..8
                      NameRef@6..8
                        WHITESPACE@6..7 " "
                        IDENT@7..8 "a"
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
    fn closure_typed_param() {
        check_expr(
            "|x: i32| x",
            &expect![[r#"
            ClosureExpr@0..10
              ClosureParams@0..8
                PIPE@0..1 "|"
                ClosureParam@1..7
                  Name@1..2
                    IDENT@1..2 "x"
                  COLON@2..3 ":"
                  PathType@3..7
                    Path@3..7
                      PathSegment@3..7
                        NameRef@3..7
                          WHITESPACE@3..4 " "
                          IDENT@4..7 "i32"
                PIPE@7..8 "|"
              PathExpr@8..10
                Path@8..10
                  PathSegment@8..10
                    NameRef@8..10
                      WHITESPACE@8..9 " "
                      IDENT@9..10 "x"
        "#]],
        );
    }

    #[test]
    fn closure_block_body() {
        check_expr(
            "|x| { x + 1 }",
            &expect![[r#"
            ClosureExpr@0..13
              ClosureParams@0..3
                PIPE@0..1 "|"
                ClosureParam@1..2
                  Name@1..2
                    IDENT@1..2 "x"
                PIPE@2..3 "|"
              BlockExpr@3..13
                Block@3..13
                  WHITESPACE@3..4 " "
                  L_BRACE@4..5 "{"
                  BinExpr@5..11
                    PathExpr@5..7
                      Path@5..7
                        PathSegment@5..7
                          NameRef@5..7
                            WHITESPACE@5..6 " "
                            IDENT@6..7 "x"
                    WHITESPACE@7..8 " "
                    PLUS@8..9 "+"
                    LiteralExpr@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "1"
                  WHITESPACE@11..12 " "
                  R_BRACE@12..13 "}"
        "#]],
        );
    }

    #[test]
    fn closure_capture_list() {
        check_expr(
            "@[x, y] |a| a + x",
            &expect![[r#"
            ClosureExpr@0..17
              CaptureList@0..7
                AT@0..1 "@"
                L_BRACKET@1..2 "["
                Capture@2..3
                  Name@2..3
                    IDENT@2..3 "x"
                COMMA@3..4 ","
                Capture@4..6
                  Name@4..6
                    WHITESPACE@4..5 " "
                    IDENT@5..6 "y"
                R_BRACKET@6..7 "]"
              ClosureParams@7..11
                WHITESPACE@7..8 " "
                PIPE@8..9 "|"
                ClosureParam@9..10
                  Name@9..10
                    IDENT@9..10 "a"
                PIPE@10..11 "|"
              BinExpr@11..17
                PathExpr@11..13
                  Path@11..13
                    PathSegment@11..13
                      NameRef@11..13
                        WHITESPACE@11..12 " "
                        IDENT@12..13 "a"
                WHITESPACE@13..14 " "
                PLUS@14..15 "+"
                PathExpr@15..17
                  Path@15..17
                    PathSegment@15..17
                      NameRef@15..17
                        WHITESPACE@15..16 " "
                        IDENT@16..17 "x"
        "#]],
        );
    }

    #[test]
    fn closure_capture_expr() {
        check_expr(
            "@[val: foo()] || val",
            &expect![[r#"
            ClosureExpr@0..20
              CaptureList@0..13
                AT@0..1 "@"
                L_BRACKET@1..2 "["
                Capture@2..12
                  Name@2..5
                    IDENT@2..5 "val"
                  COLON@5..6 ":"
                  CallExpr@6..12
                    PathExpr@6..10
                      Path@6..10
                        PathSegment@6..10
                          NameRef@6..10
                            WHITESPACE@6..7 " "
                            IDENT@7..10 "foo"
                    L_PAREN@10..11 "("
                    R_PAREN@11..12 ")"
                R_BRACKET@12..13 "]"
              ClosureParams@13..16
                WHITESPACE@13..14 " "
                OR_OR@14..16 "||"
              PathExpr@16..20
                Path@16..20
                  PathSegment@16..20
                    NameRef@16..20
                      WHITESPACE@16..17 " "
                      IDENT@17..20 "val"
        "#]],
        );
    }

    #[test]
    fn closure_nested() {
        check_expr(
            "|x| |y| x + y",
            &expect![[r#"
            ClosureExpr@0..13
              ClosureParams@0..3
                PIPE@0..1 "|"
                ClosureParam@1..2
                  Name@1..2
                    IDENT@1..2 "x"
                PIPE@2..3 "|"
              ClosureExpr@3..13
                ClosureParams@3..7
                  WHITESPACE@3..4 " "
                  PIPE@4..5 "|"
                  ClosureParam@5..6
                    Name@5..6
                      IDENT@5..6 "y"
                  PIPE@6..7 "|"
                BinExpr@7..13
                  PathExpr@7..9
                    Path@7..9
                      PathSegment@7..9
                        NameRef@7..9
                          WHITESPACE@7..8 " "
                          IDENT@8..9 "x"
                  WHITESPACE@9..10 " "
                  PLUS@10..11 "+"
                  PathExpr@11..13
                    Path@11..13
                      PathSegment@11..13
                        NameRef@11..13
                          WHITESPACE@11..12 " "
                          IDENT@12..13 "y"
        "#]],
        );
    }

    #[test]
    fn closure_as_arg() {
        check_expr(
            "map(|x| x + 1)",
            &expect![[r#"
            CallExpr@0..14
              PathExpr@0..3
                Path@0..3
                  PathSegment@0..3
                    NameRef@0..3
                      IDENT@0..3 "map"
              L_PAREN@3..4 "("
              CallArg@4..13
                ClosureExpr@4..13
                  ClosureParams@4..7
                    PIPE@4..5 "|"
                    ClosureParam@5..6
                      Name@5..6
                        IDENT@5..6 "x"
                    PIPE@6..7 "|"
                  BinExpr@7..13
                    PathExpr@7..9
                      Path@7..9
                        PathSegment@7..9
                          NameRef@7..9
                            WHITESPACE@7..8 " "
                            IDENT@8..9 "x"
                    WHITESPACE@9..10 " "
                    PLUS@10..11 "+"
                    LiteralExpr@11..13
                      WHITESPACE@11..12 " "
                      INT_LITERAL@12..13 "1"
              R_PAREN@13..14 ")"
        "#]],
        );
    }

    #[test]
    fn closure_trailing_comma() {
        check_expr(
            "|a, b,| a",
            &expect![[r#"
            ClosureExpr@0..9
              ClosureParams@0..7
                PIPE@0..1 "|"
                ClosureParam@1..2
                  Name@1..2
                    IDENT@1..2 "a"
                COMMA@2..3 ","
                ClosureParam@3..5
                  Name@3..5
                    WHITESPACE@3..4 " "
                    IDENT@4..5 "b"
                COMMA@5..6 ","
                PIPE@6..7 "|"
              PathExpr@7..9
                Path@7..9
                  PathSegment@7..9
                    NameRef@7..9
                      WHITESPACE@7..8 " "
                      IDENT@8..9 "a"
        "#]],
        );
    }
}
