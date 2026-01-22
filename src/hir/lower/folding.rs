//! Literal folding and parsing for HIR lowering.
//!
//! This module handles:
//! - Folding negated integer/float literals at compile time
//! - Parsing literal values from source text

use crate::hir::LoweredExpr;
use crate::ast::{Expr, LiteralExpr, PrefixExpr};
use crate::lexer::Span;
use crate::sema::types::PrimitiveKind;
use crate::syntax::SyntaxKind;
use rowan::ast::AstNode;

// ============================================================================
// Literal Parsing Helpers
// ============================================================================

pub(super) fn parse_int_literal_value(text: &str) -> Option<i128> {
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

pub(super) fn parse_float_literal_value(text: &str) -> Option<f64> {
    let num_text = if let Some(stripped) = text.strip_suffix("f32") {
        stripped
    } else if let Some(stripped) = text.strip_suffix("f64") {
        stripped
    } else {
        text
    };
    num_text.replace('_', "").parse().ok()
}

pub(super) fn parse_char_literal(text: &str) -> Option<char> {
    // Strip quotes and handle escape sequences
    let inner = text.strip_prefix('\'')?.strip_suffix('\'')?;
    if inner.starts_with('\\') {
        match inner.chars().nth(1)? {
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            '\\' => Some('\\'),
            '\'' => Some('\''),
            '0' => Some('\0'),
            _ => inner.chars().nth(1),
        }
    } else {
        inner.chars().next()
    }
}

pub(super) fn parse_string_literal(text: &str) -> String {
    // Strip quotes - basic implementation
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| {
            s.replace("\\n", "\n")
                .replace("\\r", "\r")
                .replace("\\t", "\t")
                .replace("\\\\", "\\")
                .replace("\\\"", "\"")
        })
        .unwrap_or_default()
}

// ============================================================================
// Literal Folding API
// ============================================================================

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
pub(super) fn parse_int_suffix(text: &str) -> (Option<PrimitiveKind>, bool) {
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
