//! Expression parser using Pratt parsing.
//!
//! Implements precedence climbing for SPL expressions.

use crate::parser::{CompletedMarker, Parser};
use crate::syntax::SyntaxKind;

/// Parse an expression.
pub fn expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    expr_bp(p, 0)
}

/// Parse an expression with minimum binding power.
fn expr_bp(
    p: &mut Parser<'_>,
    min_bp: u8,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let mut lhs = match lhs(p)? {
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
            lhs = infix_expr(p, lhs, r_bp)?;
            continue;
        }

        // Not an operator we recognize, stop
        break;
    }

    Ok(Some(lhs))
}

/// Parse an expression, but don't allow struct expressions.
/// Used in control flow contexts where `identifier {` should be parsed as
/// identifier followed by block, not as a struct expression.
pub(crate) fn expr_no_struct(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    expr_no_struct_bp(p, 0)
}

/// Parse an expression with minimum binding power, disallowing struct expressions.
fn expr_no_struct_bp(
    p: &mut Parser<'_>,
    min_bp: u8,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let mut lhs = match lhs_no_struct(p)? {
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
            lhs = infix_expr(p, lhs, r_bp)?;
            continue;
        }

        // Not an operator we recognize, stop
        break;
    }

    Ok(Some(lhs))
}

/// Parse the left-hand side of an expression (prefix or primary).
fn lhs(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let current = match p.current() {
        Some(kind) => kind,
        None => return Ok(None),
    };

    // Check for prefix operators
    if let Some(((), r_bp)) = prefix_bp(current) {
        return prefix_expr(p, r_bp);
    }

    // Otherwise, parse a primary expression
    primary_expr(p)
}

/// Parse the left-hand side of an expression, disallowing struct expressions.
fn lhs_no_struct(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let current = match p.current() {
        Some(kind) => kind,
        None => return Ok(None),
    };

    // Check for prefix operators
    if let Some(((), r_bp)) = prefix_bp(current) {
        return prefix_expr_no_struct(p, r_bp);
    }

    // Otherwise, parse a primary expression (no struct)
    primary_expr_no_struct(p)
}

/// Parse a prefix expression.
fn prefix_expr(
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
fn prefix_expr_no_struct(
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
fn infix_expr(
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
fn postfix_expr(
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
fn arg_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
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

/// Parse field access or method call: expr.field or expr.method(args)
fn field_or_method_expr(
    p: &mut Parser<'_>,
    lhs: CompletedMarker,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = lhs.precede(p);
    p.expect(SyntaxKind::DOT)?;

    // Expect identifier
    if !p.at(SyntaxKind::IDENT) {
        return Err(p.error_at_current("expected identifier after '.'".to_string()));
    }
    p.bump();

    // Check for method call
    if p.at(SyntaxKind::L_PAREN) {
        arg_list(p)?;
        Ok(m.complete(p, SyntaxKind::MethodCallExpr))
    } else {
        Ok(m.complete(p, SyntaxKind::FieldExpr))
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

/// Parse a primary expression.
fn primary_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let current = match p.current() {
        Some(kind) => kind,
        None => return Ok(None),
    };

    match current {
        // Literals
        SyntaxKind::INT_LITERAL
        | SyntaxKind::FLOAT_LITERAL
        | SyntaxKind::STRING_LITERAL
        | SyntaxKind::CHAR_LITERAL
        | SyntaxKind::TRUE_KW
        | SyntaxKind::FALSE_KW => literal_expr(p),

        // Identifier / path
        SyntaxKind::IDENT | SyntaxKind::SELF_VALUE_KW | SyntaxKind::SELF_TYPE_KW => {
            path_or_struct_expr(p)
        }

        // Grouped or tuple expression
        SyntaxKind::L_PAREN => paren_or_tuple_expr(p),

        // Array expression
        SyntaxKind::L_BRACKET => array_expr(p),

        // Block expression
        SyntaxKind::L_BRACE => block_expr(p),

        // Control flow
        SyntaxKind::IF_KW => if_expr(p),
        SyntaxKind::WHILE_KW => while_expr(p),
        SyntaxKind::FOR_KW => for_expr(p),
        SyntaxKind::LOOP_KW => loop_expr(p),
        SyntaxKind::BREAK_KW => break_expr(p),
        SyntaxKind::CONTINUE_KW => continue_expr(p),
        SyntaxKind::RETURN_KW => return_expr(p),

        _ => Ok(None),
    }
}

/// Parse a primary expression, disallowing struct expressions.
fn primary_expr_no_struct(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let current = match p.current() {
        Some(kind) => kind,
        None => return Ok(None),
    };

    match current {
        // Literals
        SyntaxKind::INT_LITERAL
        | SyntaxKind::FLOAT_LITERAL
        | SyntaxKind::STRING_LITERAL
        | SyntaxKind::CHAR_LITERAL
        | SyntaxKind::TRUE_KW
        | SyntaxKind::FALSE_KW => literal_expr(p),

        // Identifier / path - use path_expr_only instead of path_or_struct_expr
        SyntaxKind::IDENT | SyntaxKind::SELF_VALUE_KW | SyntaxKind::SELF_TYPE_KW => {
            path_expr_only(p)
        }

        // Grouped or tuple expression
        SyntaxKind::L_PAREN => paren_or_tuple_expr(p),

        // Array expression
        SyntaxKind::L_BRACKET => array_expr(p),

        // Block expression
        SyntaxKind::L_BRACE => block_expr(p),

        // Control flow
        SyntaxKind::IF_KW => if_expr(p),
        SyntaxKind::WHILE_KW => while_expr(p),
        SyntaxKind::FOR_KW => for_expr(p),
        SyntaxKind::LOOP_KW => loop_expr(p),
        SyntaxKind::BREAK_KW => break_expr(p),
        SyntaxKind::CONTINUE_KW => continue_expr(p),
        SyntaxKind::RETURN_KW => return_expr(p),

        _ => Ok(None),
    }
}

/// Parse a literal expression.
fn literal_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.bump();
    Ok(Some(m.complete(p, SyntaxKind::LiteralExpr)))
}

/// Parse a path or struct expression.
fn path_or_struct_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in expression position)
    super::path::path_no_generics(p)?;

    // Check for struct expression: Path { fields }
    if p.at(SyntaxKind::L_BRACE) {
        // Could be struct expression or block - for now treat as struct
        // A more sophisticated parser would need context
        return struct_expr_rest(p, m);
    }

    Ok(Some(m.complete(p, SyntaxKind::PathExpr)))
}

/// Parse a path expression only (no struct expression).
/// Used in control flow contexts where `identifier {` should be parsed as
/// identifier followed by block, not as a struct expression.
fn path_expr_only(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in expression position)
    super::path::path_no_generics(p)?;

    // NO struct check - just return PathExpr
    Ok(Some(m.complete(p, SyntaxKind::PathExpr)))
}

/// Parse the rest of a struct expression after the path.
fn struct_expr_rest(
    p: &mut Parser<'_>,
    m: crate::parser::Marker,
) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    p.expect(SyntaxKind::L_BRACE)?;

    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        struct_field(p)?;
        if !p.at(SyntaxKind::R_BRACE) && !p.eat(SyntaxKind::COMMA) {
            break;
        }
    }

    p.expect(SyntaxKind::R_BRACE)?;
    Ok(Some(m.complete(p, SyntaxKind::StructExpr)))
}

/// Parse a struct field: name or name: expr
fn struct_field(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::IDENT)?;

    if p.eat(SyntaxKind::COLON) {
        let _ = expr(p)?;
    }

    Ok(m.complete(p, SyntaxKind::StructExprField))
}

/// Parse a parenthesized or tuple expression.
fn paren_or_tuple_expr(
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
fn array_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
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

/// Parse a block expression.
fn block_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    block(p)?;
    Ok(Some(m.complete(p, SyntaxKind::BlockExpr)))
}

/// Parse a block with statements.
pub(crate) fn block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    super::stmt::block(p)
}

/// Parse an if expression.
fn if_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::IF_KW)?;
    let _ = expr_no_struct(p)?;
    block(p)?;

    if p.eat(SyntaxKind::ELSE_KW) {
        if p.at(SyntaxKind::IF_KW) {
            // else if
            let _ = if_expr(p)?;
        } else {
            // else block
            block(p)?;
        }
    }

    Ok(Some(m.complete(p, SyntaxKind::IfExpr)))
}

/// Parse a while expression.
fn while_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::WHILE_KW)?;
    let _ = expr_no_struct(p)?;
    block(p)?;
    Ok(Some(m.complete(p, SyntaxKind::WhileExpr)))
}

/// Parse a for expression.
fn for_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::FOR_KW)?;

    // Pattern (supports full pattern syntax including tuples)
    if let Err(err) = super::pattern::pattern(p) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = p.expect(SyntaxKind::IN_KW) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = expr_no_struct(p) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = block(p) {
        m.abandon(p);
        return Err(err);
    }

    Ok(Some(m.complete(p, SyntaxKind::ForExpr)))
}

/// Parse a loop expression.
fn loop_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::LOOP_KW)?;
    block(p)?;
    Ok(Some(m.complete(p, SyntaxKind::LoopExpr)))
}

/// Parse a break expression.
fn break_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::BREAK_KW)?;

    // Optional value
    if p.current().is_some()
        && !p.at(SyntaxKind::SEMI)
        && !p.at(SyntaxKind::R_BRACE)
        && !p.at(SyntaxKind::R_PAREN)
    {
        let _ = expr(p)?;
    }

    Ok(Some(m.complete(p, SyntaxKind::BreakExpr)))
}

/// Parse a continue expression.
fn continue_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::CONTINUE_KW)?;
    Ok(Some(m.complete(p, SyntaxKind::ContinueExpr)))
}

/// Parse a return expression.
fn return_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::RETURN_KW)?;

    // Optional value
    if p.current().is_some()
        && !p.at(SyntaxKind::SEMI)
        && !p.at(SyntaxKind::R_BRACE)
        && !p.at(SyntaxKind::R_PAREN)
    {
        let _ = expr(p)?;
    }

    Ok(Some(m.complete(p, SyntaxKind::ReturnExpr)))
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
    super::path::path_no_generics(p)?;

    Ok(m.complete(p, SyntaxKind::PathType))
}

// === Binding power tables ===

/// Prefix operator binding power ((), right).
fn prefix_bp(op: SyntaxKind) -> Option<((), u8)> {
    match op {
        SyntaxKind::BANG | SyntaxKind::MINUS | SyntaxKind::STAR => Some(((), 19)), // Unary: prec 10
        SyntaxKind::AMP => Some(((), 19)),                                         // Reference
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

    // Phase 1: Control Flow Tests

    #[test]
    fn if_expr_simple() {
        check_expr(
            "if x { 1 }",
            &expect![[r#"
                IfExpr@0..10
                  IF_KW@0..2 "if"
                  PathExpr@2..4
                    Path@2..4
                      PathSegment@2..4
                        NameRef@2..4
                          WHITESPACE@2..3 " "
                          IDENT@3..4 "x"
                  Block@4..10
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    LiteralExpr@6..8
                      WHITESPACE@6..7 " "
                      INT_LITERAL@7..8 "1"
                    WHITESPACE@8..9 " "
                    R_BRACE@9..10 "}"
            "#]],
        );
    }

    #[test]
    fn if_else_expr() {
        check_expr(
            "if true { 1 } else { 2 }",
            &expect![[r#"
                IfExpr@0..24
                  IF_KW@0..2 "if"
                  LiteralExpr@2..7
                    WHITESPACE@2..3 " "
                    TRUE_KW@3..7 "true"
                  Block@7..13
                    WHITESPACE@7..8 " "
                    L_BRACE@8..9 "{"
                    LiteralExpr@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "1"
                    WHITESPACE@11..12 " "
                    R_BRACE@12..13 "}"
                  WHITESPACE@13..14 " "
                  ELSE_KW@14..18 "else"
                  Block@18..24
                    WHITESPACE@18..19 " "
                    L_BRACE@19..20 "{"
                    LiteralExpr@20..22
                      WHITESPACE@20..21 " "
                      INT_LITERAL@21..22 "2"
                    WHITESPACE@22..23 " "
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn if_else_if_chain() {
        check_expr(
            "if true { 1 } else if false { 2 } else { 3 }",
            &expect![[r#"
                IfExpr@0..44
                  IF_KW@0..2 "if"
                  LiteralExpr@2..7
                    WHITESPACE@2..3 " "
                    TRUE_KW@3..7 "true"
                  Block@7..13
                    WHITESPACE@7..8 " "
                    L_BRACE@8..9 "{"
                    LiteralExpr@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "1"
                    WHITESPACE@11..12 " "
                    R_BRACE@12..13 "}"
                  WHITESPACE@13..14 " "
                  ELSE_KW@14..18 "else"
                  IfExpr@18..44
                    WHITESPACE@18..19 " "
                    IF_KW@19..21 "if"
                    LiteralExpr@21..27
                      WHITESPACE@21..22 " "
                      FALSE_KW@22..27 "false"
                    Block@27..33
                      WHITESPACE@27..28 " "
                      L_BRACE@28..29 "{"
                      LiteralExpr@29..31
                        WHITESPACE@29..30 " "
                        INT_LITERAL@30..31 "2"
                      WHITESPACE@31..32 " "
                      R_BRACE@32..33 "}"
                    WHITESPACE@33..34 " "
                    ELSE_KW@34..38 "else"
                    Block@38..44
                      WHITESPACE@38..39 " "
                      L_BRACE@39..40 "{"
                      LiteralExpr@40..42
                        WHITESPACE@40..41 " "
                        INT_LITERAL@41..42 "3"
                      WHITESPACE@42..43 " "
                      R_BRACE@43..44 "}"
            "#]],
        );
    }

    #[test]
    fn while_expr_simple() {
        check_expr(
            "while cond { 1 }",
            &expect![[r#"
                WhileExpr@0..16
                  WHILE_KW@0..5 "while"
                  PathExpr@5..10
                    Path@5..10
                      PathSegment@5..10
                        NameRef@5..10
                          WHITESPACE@5..6 " "
                          IDENT@6..10 "cond"
                  Block@10..16
                    WHITESPACE@10..11 " "
                    L_BRACE@11..12 "{"
                    LiteralExpr@12..14
                      WHITESPACE@12..13 " "
                      INT_LITERAL@13..14 "1"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }

    #[test]
    fn for_expr_simple() {
        check_expr(
            "for i in items { x }",
            &expect![[r#"
                ForExpr@0..20
                  FOR_KW@0..3 "for"
                  IdentPat@3..5
                    WHITESPACE@3..4 " "
                    IDENT@4..5 "i"
                  WHITESPACE@5..6 " "
                  IN_KW@6..8 "in"
                  PathExpr@8..14
                    Path@8..14
                      PathSegment@8..14
                        NameRef@8..14
                          WHITESPACE@8..9 " "
                          IDENT@9..14 "items"
                  Block@14..20
                    WHITESPACE@14..15 " "
                    L_BRACE@15..16 "{"
                    PathExpr@16..18
                      Path@16..18
                        PathSegment@16..18
                          NameRef@16..18
                            WHITESPACE@16..17 " "
                            IDENT@17..18 "x"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn for_with_range() {
        check_expr(
            "for i in 0..10 { x }",
            &expect![[r#"
                ForExpr@0..20
                  FOR_KW@0..3 "for"
                  IdentPat@3..5
                    WHITESPACE@3..4 " "
                    IDENT@4..5 "i"
                  WHITESPACE@5..6 " "
                  IN_KW@6..8 "in"
                  RangeExpr@8..14
                    LiteralExpr@8..10
                      WHITESPACE@8..9 " "
                      INT_LITERAL@9..10 "0"
                    DOT_DOT@10..12 ".."
                    LiteralExpr@12..14
                      INT_LITERAL@12..14 "10"
                  Block@14..20
                    WHITESPACE@14..15 " "
                    L_BRACE@15..16 "{"
                    PathExpr@16..18
                      Path@16..18
                        PathSegment@16..18
                          NameRef@16..18
                            WHITESPACE@16..17 " "
                            IDENT@17..18 "x"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn loop_expr_simple() {
        check_expr(
            "loop { x }",
            &expect![[r#"
                LoopExpr@0..10
                  LOOP_KW@0..4 "loop"
                  Block@4..10
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    PathExpr@6..8
                      Path@6..8
                        PathSegment@6..8
                          NameRef@6..8
                            WHITESPACE@6..7 " "
                            IDENT@7..8 "x"
                    WHITESPACE@8..9 " "
                    R_BRACE@9..10 "}"
            "#]],
        );
    }

    #[test]
    fn break_expr_no_value() {
        check_expr(
            "break",
            &expect![[r#"
                BreakExpr@0..5
                  BREAK_KW@0..5 "break"
            "#]],
        );
    }

    #[test]
    fn break_expr_with_value() {
        check_expr(
            "break 42",
            &expect![[r#"
                BreakExpr@0..8
                  BREAK_KW@0..5 "break"
                  LiteralExpr@5..8
                    WHITESPACE@5..6 " "
                    INT_LITERAL@6..8 "42"
            "#]],
        );
    }

    #[test]
    fn continue_expr() {
        check_expr(
            "continue",
            &expect![[r#"
                ContinueExpr@0..8
                  CONTINUE_KW@0..8 "continue"
            "#]],
        );
    }

    #[test]
    fn return_expr_no_value() {
        check_expr(
            "return",
            &expect![[r#"
                ReturnExpr@0..6
                  RETURN_KW@0..6 "return"
            "#]],
        );
    }

    #[test]
    fn return_expr_with_value() {
        check_expr(
            "return x + 1",
            &expect![[r#"
                ReturnExpr@0..12
                  RETURN_KW@0..6 "return"
                  BinExpr@6..12
                    PathExpr@6..8
                      Path@6..8
                        PathSegment@6..8
                          NameRef@6..8
                            WHITESPACE@6..7 " "
                            IDENT@7..8 "x"
                    WHITESPACE@8..9 " "
                    PLUS@9..10 "+"
                    LiteralExpr@10..12
                      WHITESPACE@10..11 " "
                      INT_LITERAL@11..12 "1"
            "#]],
        );
    }

    // Phase 2: Missing Literals & Operators

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

    // Phase 3: Collections & Special Cases

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
    fn struct_expr_simple() {
        check_expr(
            "Point { x: 1, y: 2 }",
            &expect![[r#"
                StructExpr@0..20
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Point"
                  WHITESPACE@5..6 " "
                  L_BRACE@6..7 "{"
                  StructExprField@7..12
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "x"
                    COLON@9..10 ":"
                    LiteralExpr@10..12
                      WHITESPACE@10..11 " "
                      INT_LITERAL@11..12 "1"
                  COMMA@12..13 ","
                  StructExprField@13..18
                    WHITESPACE@13..14 " "
                    IDENT@14..15 "y"
                    COLON@15..16 ":"
                    LiteralExpr@16..18
                      WHITESPACE@16..17 " "
                      INT_LITERAL@17..18 "2"
                  WHITESPACE@18..19 " "
                  R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn struct_expr_shorthand() {
        check_expr(
            "Point { x, y }",
            &expect![[r#"
                StructExpr@0..14
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Point"
                  WHITESPACE@5..6 " "
                  L_BRACE@6..7 "{"
                  StructExprField@7..9
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "x"
                  COMMA@9..10 ","
                  StructExprField@10..12
                    WHITESPACE@10..11 " "
                    IDENT@11..12 "y"
                  WHITESPACE@12..13 " "
                  R_BRACE@13..14 "}"
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
                FieldExpr@0..6
                  PathExpr@0..4
                    Path@0..4
                      PathSegment@0..4
                        NameRef@0..4
                          SELF_VALUE_KW@0..4 "self"
                  DOT@4..5 "."
                  IDENT@5..6 "x"
            "#]],
        );
    }

    // Phase 4: Slice Syntax

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

    // === Phase 1: Precedence/Associativity Tests ===

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

    // === Phase 2: Literal Edge Cases ===

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

    // === Phase 3: Expression Edge Cases ===

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

    // === Control Flow Edge Cases ===

    #[test]
    fn if_complex_condition() {
        check_expr(
            "if a > 0 && b < 10 { x }",
            &expect![[r#"
                IfExpr@0..24
                  IF_KW@0..2 "if"
                  BinExpr@2..18
                    BinExpr@2..8
                      PathExpr@2..4
                        Path@2..4
                          PathSegment@2..4
                            NameRef@2..4
                              WHITESPACE@2..3 " "
                              IDENT@3..4 "a"
                      WHITESPACE@4..5 " "
                      GT@5..6 ">"
                      LiteralExpr@6..8
                        WHITESPACE@6..7 " "
                        INT_LITERAL@7..8 "0"
                    WHITESPACE@8..9 " "
                    AND_AND@9..11 "&&"
                    BinExpr@11..18
                      PathExpr@11..13
                        Path@11..13
                          PathSegment@11..13
                            NameRef@11..13
                              WHITESPACE@11..12 " "
                              IDENT@12..13 "b"
                      WHITESPACE@13..14 " "
                      LT@14..15 "<"
                      LiteralExpr@15..18
                        WHITESPACE@15..16 " "
                        INT_LITERAL@16..18 "10"
                  Block@18..24
                    WHITESPACE@18..19 " "
                    L_BRACE@19..20 "{"
                    PathExpr@20..22
                      Path@20..22
                        PathSegment@20..22
                          NameRef@20..22
                            WHITESPACE@20..21 " "
                            IDENT@21..22 "x"
                    WHITESPACE@22..23 " "
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn if_nested() {
        check_expr(
            "if a { if b { x } }",
            &expect![[r#"
                IfExpr@0..19
                  IF_KW@0..2 "if"
                  PathExpr@2..4
                    Path@2..4
                      PathSegment@2..4
                        NameRef@2..4
                          WHITESPACE@2..3 " "
                          IDENT@3..4 "a"
                  Block@4..19
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    IfExpr@6..17
                      WHITESPACE@6..7 " "
                      IF_KW@7..9 "if"
                      PathExpr@9..11
                        Path@9..11
                          PathSegment@9..11
                            NameRef@9..11
                              WHITESPACE@9..10 " "
                              IDENT@10..11 "b"
                      Block@11..17
                        WHITESPACE@11..12 " "
                        L_BRACE@12..13 "{"
                        PathExpr@13..15
                          Path@13..15
                            PathSegment@13..15
                              NameRef@13..15
                                WHITESPACE@13..14 " "
                                IDENT@14..15 "x"
                        WHITESPACE@15..16 " "
                        R_BRACE@16..17 "}"
                    WHITESPACE@17..18 " "
                    R_BRACE@18..19 "}"
            "#]],
        );
    }

    #[test]
    fn loop_with_break_value() {
        check_expr(
            "loop { break 42 }",
            &expect![[r#"
                LoopExpr@0..17
                  LOOP_KW@0..4 "loop"
                  Block@4..17
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    BreakExpr@6..15
                      WHITESPACE@6..7 " "
                      BREAK_KW@7..12 "break"
                      LiteralExpr@12..15
                        WHITESPACE@12..13 " "
                        INT_LITERAL@13..15 "42"
                    WHITESPACE@15..16 " "
                    R_BRACE@16..17 "}"
            "#]],
        );
    }

    // === Collections Edge Cases ===

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
    fn struct_trailing_comma() {
        check_expr(
            "Point { x: 1, y: 2, }",
            &expect![[r#"
                StructExpr@0..21
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Point"
                  WHITESPACE@5..6 " "
                  L_BRACE@6..7 "{"
                  StructExprField@7..12
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "x"
                    COLON@9..10 ":"
                    LiteralExpr@10..12
                      WHITESPACE@10..11 " "
                      INT_LITERAL@11..12 "1"
                  COMMA@12..13 ","
                  StructExprField@13..18
                    WHITESPACE@13..14 " "
                    IDENT@14..15 "y"
                    COLON@15..16 ":"
                    LiteralExpr@16..18
                      WHITESPACE@16..17 " "
                      INT_LITERAL@17..18 "2"
                  COMMA@18..19 ","
                  WHITESPACE@19..20 " "
                  R_BRACE@20..21 "}"
            "#]],
        );
    }

    #[test]
    fn struct_mixed_shorthand() {
        check_expr(
            "Point { x, y: other_y }",
            &expect![[r#"
                StructExpr@0..23
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Point"
                  WHITESPACE@5..6 " "
                  L_BRACE@6..7 "{"
                  StructExprField@7..9
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "x"
                  COMMA@9..10 ","
                  StructExprField@10..21
                    WHITESPACE@10..11 " "
                    IDENT@11..12 "y"
                    COLON@12..13 ":"
                    PathExpr@13..21
                      Path@13..21
                        PathSegment@13..21
                          NameRef@13..21
                            WHITESPACE@13..14 " "
                            IDENT@14..21 "other_y"
                  WHITESPACE@21..22 " "
                  R_BRACE@22..23 "}"
            "#]],
        );
    }

    #[test]
    fn struct_nested() {
        check_expr(
            "Outer { inner: Inner { x: 1 } }",
            &expect![[r#"
                StructExpr@0..31
                  Path@0..5
                    PathSegment@0..5
                      NameRef@0..5
                        IDENT@0..5 "Outer"
                  WHITESPACE@5..6 " "
                  L_BRACE@6..7 "{"
                  StructExprField@7..29
                    WHITESPACE@7..8 " "
                    IDENT@8..13 "inner"
                    COLON@13..14 ":"
                    StructExpr@14..29
                      Path@14..20
                        PathSegment@14..20
                          NameRef@14..20
                            WHITESPACE@14..15 " "
                            IDENT@15..20 "Inner"
                      WHITESPACE@20..21 " "
                      L_BRACE@21..22 "{"
                      StructExprField@22..27
                        WHITESPACE@22..23 " "
                        IDENT@23..24 "x"
                        COLON@24..25 ":"
                        LiteralExpr@25..27
                          WHITESPACE@25..26 " "
                          INT_LITERAL@26..27 "1"
                      WHITESPACE@27..28 " "
                      R_BRACE@28..29 "}"
                  WHITESPACE@29..30 " "
                  R_BRACE@30..31 "}"
            "#]],
        );
    }

    #[test]
    fn struct_with_path() {
        check_expr(
            "module::Point { x: 1 }",
            &expect![[r#"
                StructExpr@0..22
                  Path@0..13
                    PathSegment@0..6
                      NameRef@0..6
                        IDENT@0..6 "module"
                    COLON_COLON@6..8 "::"
                    PathSegment@8..13
                      NameRef@8..13
                        IDENT@8..13 "Point"
                  WHITESPACE@13..14 " "
                  L_BRACE@14..15 "{"
                  StructExprField@15..20
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "x"
                    COLON@17..18 ":"
                    LiteralExpr@18..20
                      WHITESPACE@18..19 " "
                      INT_LITERAL@19..20 "1"
                  WHITESPACE@20..21 " "
                  R_BRACE@21..22 "}"
            "#]],
        );
    }

    // Phase 5: Complex Cases & Edge Cases

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
