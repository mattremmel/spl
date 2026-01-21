//! Expression lowering for literal folding.
//!
//! This module handles the lowering of AST expressions to detect patterns
//! like negated integer literals that need special handling during type inference.

use crate::ast::{Expr, LiteralExpr, PrefixExpr};
use crate::hir::LoweredExpr;
use crate::lexer::Span;
use crate::sema::types::PrimitiveKind;
use crate::syntax::SyntaxKind;
use rowan::ast::AstNode;

/// Try to lower an expression for literal folding.
///
/// Returns `(LoweredExpr, was_lowered)` where `was_lowered` is true if the expression
/// was successfully lowered to a folded form.
///
/// Currently handles:
/// - Negated integer literals: `-128i8`, `-(128i8)`, `(-(128i8))`
/// - Negated float literals: `-1.0f32`, `-(1.0f64)`
pub fn try_lower_expr(expr: &Expr) -> (LoweredExpr, bool) {
    match expr {
        Expr::Prefix(prefix) => {
            if let Some(lowered) = lower_negated_literal(prefix) {
                return (lowered, true);
            }
        }
        Expr::Paren(paren) => {
            // Try to lower the inner expression (handles `(-(128i8))`)
            if let Some(inner) = paren.expr() {
                return try_lower_expr(&inner);
            }
        }
        _ => {}
    }
    (LoweredExpr::Passthrough, false)
}

/// Try to lower a prefix expression that might be a negated literal.
///
/// Handles patterns like:
/// - `-128i8` (direct negation of literal)
/// - `-(128i8)` (negation of parenthesized literal)
/// - `-(-128i8)` (double negation)
fn lower_negated_literal(prefix: &PrefixExpr) -> Option<LoweredExpr> {
    // Check if this is a negation operator
    let op_token = prefix.op_token()?;
    if op_token.kind() != SyntaxKind::MINUS {
        return None;
    }

    let inner = prefix.expr()?;

    // First, try to recursively lower the inner expression
    // This handles double negation: `--128i8`
    if let Expr::Prefix(inner_prefix) = &inner
        && let Some(inner_lowered) = lower_negated_literal(inner_prefix)
    {
        // We have a nested negation - negate the result
        return match inner_lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                let span = text_range_to_span(prefix.syntax().text_range());
                Some(LoweredExpr::IntLiteral {
                    value: -value,
                    suffix,
                    span,
                })
            }
            LoweredExpr::FloatLiteral { value, suffix, .. } => {
                let span = text_range_to_span(prefix.syntax().text_range());
                Some(LoweredExpr::FloatLiteral {
                    value: -value,
                    suffix,
                    span,
                })
            }
            LoweredExpr::Passthrough => None,
        };
    }

    // Unwrap parentheses to find the inner literal or nested negation
    let unwrapped = unwrap_parens(&inner)?;

    match &unwrapped {
        Expr::Literal(lit) => lower_negated_numeric_literal(prefix, lit),
        Expr::Prefix(inner_prefix) => {
            // Handle `-(-(128i8))` - negation of parenthesized negation
            if let Some(inner_lowered) = lower_negated_literal(inner_prefix) {
                return match inner_lowered {
                    LoweredExpr::IntLiteral { value, suffix, .. } => {
                        let span = text_range_to_span(prefix.syntax().text_range());
                        Some(LoweredExpr::IntLiteral {
                            value: -value,
                            suffix,
                            span,
                        })
                    }
                    LoweredExpr::FloatLiteral { value, suffix, .. } => {
                        let span = text_range_to_span(prefix.syntax().text_range());
                        Some(LoweredExpr::FloatLiteral {
                            value: -value,
                            suffix,
                            span,
                        })
                    }
                    LoweredExpr::Passthrough => None,
                };
            }
            None
        }
        _ => None,
    }
}

/// Lower a negated numeric literal expression.
fn lower_negated_numeric_literal(prefix: &PrefixExpr, lit: &LiteralExpr) -> Option<LoweredExpr> {
    let token = lit.token()?;
    let text = token.text();
    let span = text_range_to_span(prefix.syntax().text_range());

    match token.kind() {
        SyntaxKind::INT_LITERAL => {
            let (suffix, _has_suffix) = parse_int_suffix(text);
            let value = parse_int_literal_value(text)?;
            Some(LoweredExpr::IntLiteral {
                value: -value,
                suffix,
                span,
            })
        }
        SyntaxKind::FLOAT_LITERAL => {
            let (suffix, value) = parse_float_literal(text)?;
            Some(LoweredExpr::FloatLiteral {
                value: -value,
                suffix,
                span,
            })
        }
        _ => None,
    }
}

/// Unwrap parentheses to get the inner expression.
/// Returns the innermost non-paren expression, or None if empty parens.
fn unwrap_parens(expr: &Expr) -> Option<Expr> {
    match expr {
        Expr::Paren(p) => p.expr().and_then(|inner| unwrap_parens(&inner)),
        _ => Some(expr.clone()),
    }
}

/// Convert a rowan TextRange to our Span type.
fn text_range_to_span(range: rowan::TextRange) -> Span {
    range.start().into()..range.end().into()
}

/// Parse an integer literal suffix to determine the type.
fn parse_int_suffix(text: &str) -> (Option<PrimitiveKind>, bool) {
    let suffixes = [
        ("i128", PrimitiveKind::I128),
        ("u128", PrimitiveKind::U128),
        ("isize", PrimitiveKind::Isize),
        ("usize", PrimitiveKind::Usize),
        ("i64", PrimitiveKind::I64),
        ("u64", PrimitiveKind::U64),
        ("i32", PrimitiveKind::I32),
        ("u32", PrimitiveKind::U32),
        ("i16", PrimitiveKind::I16),
        ("u16", PrimitiveKind::U16),
        ("i8", PrimitiveKind::I8),
        ("u8", PrimitiveKind::U8),
    ];

    for (suffix, kind) in suffixes {
        if text.ends_with(suffix) {
            return (Some(kind), true);
        }
    }
    (None, false)
}

/// Parse the numeric value of an integer literal (stripping any suffix).
fn parse_int_literal_value(text: &str) -> Option<i128> {
    let suffixes = [
        "i128", "u128", "isize", "usize", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8",
    ];
    let num_text = suffixes
        .iter()
        .find(|s| text.ends_with(*s))
        .map(|s| &text[..text.len() - s.len()])
        .unwrap_or(text);

    if num_text.starts_with("0x") || num_text.starts_with("0X") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 16).ok()
    } else if num_text.starts_with("0o") || num_text.starts_with("0O") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 8).ok()
    } else if num_text.starts_with("0b") || num_text.starts_with("0B") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 2).ok()
    } else {
        num_text.replace('_', "").parse().ok()
    }
}

/// Parse a float literal, returning (suffix, value).
fn parse_float_literal(text: &str) -> Option<(Option<PrimitiveKind>, f64)> {
    let (suffix, num_text) = if let Some(stripped) = text.strip_suffix("f32") {
        (Some(PrimitiveKind::F32), stripped)
    } else if let Some(stripped) = text.strip_suffix("f64") {
        (Some(PrimitiveKind::F64), stripped)
    } else {
        (None, text)
    };

    let value: f64 = num_text.replace('_', "").parse().ok()?;
    Some((suffix, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn parse_expr(src: &str) -> Expr {
        let full_src = format!("fn main() {{ let x = {src}; }}");
        let parsed = parse(&full_src);
        assert!(
            parsed.errors().is_empty(),
            "Parse errors: {:?}",
            parsed.errors()
        );

        use crate::ast::{Item, SourceFile, Stmt};
        let file = SourceFile::cast(parsed.syntax()).unwrap();
        let fn_item = file.items().next().unwrap();
        if let Item::Function(f) = fn_item {
            let body = f.body().unwrap();
            let stmt = body.statements().next().unwrap();
            if let Stmt::Let(let_stmt) = stmt {
                return let_stmt.initializer().unwrap();
            }
        }
        panic!("Could not extract expression from source");
    }

    #[test]
    fn lower_negated_i8() {
        let expr = parse_expr("-128i8");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -128);
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn lower_parenthesized_negation() {
        let expr = parse_expr("-(128i8)");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -128);
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn lower_double_paren_negation() {
        let expr = parse_expr("(-(128i8))");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -128);
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn lower_double_negation() {
        // --128i8 should fold to +128
        let expr = parse_expr("--128i8");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, 128); // Double negation = positive
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn passthrough_variable() {
        let expr = parse_expr("-x");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(!was_lowered);
        assert!(matches!(lowered, LoweredExpr::Passthrough));
    }

    #[test]
    fn passthrough_non_prefix() {
        let expr = parse_expr("42");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(!was_lowered);
        assert!(matches!(lowered, LoweredExpr::Passthrough));
    }

    #[test]
    fn lower_negated_float() {
        let expr = parse_expr("-1.5f32");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::FloatLiteral { value, suffix, .. } => {
                assert!((value - (-1.5)).abs() < f64::EPSILON);
                assert_eq!(suffix, Some(PrimitiveKind::F32));
            }
            _ => panic!("Expected FloatLiteral"),
        }
    }

    #[test]
    fn lower_negated_unsuffixed_int() {
        let expr = parse_expr("-42");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -42);
                assert_eq!(suffix, None);
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn lower_hex_literal() {
        let expr = parse_expr("-0x80i8");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -128); // 0x80 = 128
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }
}
