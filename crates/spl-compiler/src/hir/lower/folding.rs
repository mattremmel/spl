//! Literal folding and parsing for HIR lowering.
//!
//! This module handles:
//! - Folding negated integer/float literals at compile time
//! - Parsing literal values from source text

use crate::ast::{BinExpr, Expr, LiteralExpr, PrefixExpr};
use crate::hir::LoweredExpr;
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
/// - Boolean negation: `!true`, `!false`, `!!true`
/// - Binary arithmetic: `1 + 2`, `3 * 4`, etc.
/// - Comparison operators: `1 < 2`, `3 == 3`, etc.
/// - Logical operators: `true && false`, `true || false`
pub fn try_lower_expr(expr: &Expr) -> (LoweredExpr, bool) {
    match expr {
        Expr::Literal(lit) => {
            if let Some(lowered) = try_lower_literal(lit) {
                return (lowered, true);
            }
        }
        Expr::Prefix(prefix) => {
            // Try negated literal first (MINUS operator)
            if let Some(lowered) = lower_negated_literal(prefix) {
                return (lowered, true);
            }
            // Try boolean NOT operator
            if let Some(lowered) = lower_not_literal(prefix) {
                return (lowered, true);
            }
        }
        Expr::Binary(bin) => {
            if let Some(lowered) = lower_binary_literal(bin) {
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

/// Try to lower a literal expression.
fn try_lower_literal(lit: &LiteralExpr) -> Option<LoweredExpr> {
    let token = lit.token()?;
    let text = token.text();
    let span = text_range_to_span(lit.syntax().text_range());

    match token.kind() {
        SyntaxKind::TRUE_KW => Some(LoweredExpr::BoolLiteral { value: true, span }),
        SyntaxKind::FALSE_KW => Some(LoweredExpr::BoolLiteral { value: false, span }),
        SyntaxKind::INT_LITERAL => {
            let (suffix, _) = parse_int_suffix(text);
            let value = parse_int_literal_value(text)?;
            Some(LoweredExpr::IntLiteral {
                value,
                suffix,
                span,
            })
        }
        SyntaxKind::FLOAT_LITERAL => {
            let (suffix, value) = parse_float_literal(text)?;
            Some(LoweredExpr::FloatLiteral {
                value,
                suffix,
                span,
            })
        }
        _ => None,
    }
}

/// Try to lower a NOT expression on a literal.
fn lower_not_literal(prefix: &PrefixExpr) -> Option<LoweredExpr> {
    // Check if this is a NOT operator
    let op_token = prefix.op_token()?;
    if op_token.kind() != SyntaxKind::BANG {
        return None;
    }

    let inner = prefix.expr()?;
    let span = text_range_to_span(prefix.syntax().text_range());

    // Try to recursively lower the inner expression
    let (inner_lowered, was_lowered) = try_lower_expr(&inner);

    if was_lowered {
        match inner_lowered {
            LoweredExpr::BoolLiteral { value, .. } => {
                return Some(LoweredExpr::BoolLiteral {
                    value: !value,
                    span,
                });
            }
            _ => return None, // NOT only applies to booleans
        }
    }

    None
}

/// Try to lower a prefix expression that might be a negated literal.
fn lower_negated_literal(prefix: &PrefixExpr) -> Option<LoweredExpr> {
    // Check if this is a negation operator
    let op_token = prefix.op_token()?;
    if op_token.kind() != SyntaxKind::MINUS {
        return None;
    }

    let inner = prefix.expr()?;
    let span = text_range_to_span(prefix.syntax().text_range());

    // Try to recursively lower the inner expression
    // This handles cases like: `--128i8`, `-(1 + 2)`, `-(-(128i8))`
    let (inner_lowered, was_lowered) = try_lower_expr(&inner);

    if was_lowered {
        return match inner_lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => Some(LoweredExpr::IntLiteral {
                value: -value,
                suffix,
                span,
            }),
            LoweredExpr::FloatLiteral { value, suffix, .. } => Some(LoweredExpr::FloatLiteral {
                value: -value,
                suffix,
                span,
            }),
            _ => None, // Can't negate booleans
        };
    }

    None
}

/// Convert a rowan `TextRange` to our Span type.
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

// ============================================================================
// Binary Expression Folding
// ============================================================================

/// Try to fold a binary expression with literal operands.
fn lower_binary_literal(bin: &BinExpr) -> Option<LoweredExpr> {
    let lhs = bin.lhs()?;
    let rhs = bin.rhs()?;
    let op_token = bin.op_token()?;
    let span = text_range_to_span(bin.syntax().text_range());

    // Try to lower both operands
    let (lhs_lowered, lhs_ok) = try_lower_expr(&lhs);
    let (rhs_lowered, rhs_ok) = try_lower_expr(&rhs);

    if !lhs_ok || !rhs_ok {
        return None; // Both operands must be foldable
    }

    match op_token.kind() {
        // Arithmetic operators
        SyntaxKind::PLUS => fold_arithmetic(
            &lhs_lowered,
            &rhs_lowered,
            span,
            i128::checked_add,
            |a, b| a + b,
        ),
        SyntaxKind::MINUS => fold_arithmetic(
            &lhs_lowered,
            &rhs_lowered,
            span,
            i128::checked_sub,
            |a, b| a - b,
        ),
        SyntaxKind::STAR => fold_arithmetic(
            &lhs_lowered,
            &rhs_lowered,
            span,
            i128::checked_mul,
            |a, b| a * b,
        ),
        SyntaxKind::SLASH => fold_div(&lhs_lowered, &rhs_lowered, span),
        SyntaxKind::PERCENT => fold_rem(&lhs_lowered, &rhs_lowered, span),

        // Comparison operators
        SyntaxKind::EQ_EQ => fold_comparison(
            &lhs_lowered,
            &rhs_lowered,
            span,
            |a, b| a == b,
            |a: f64, b: f64| a == b,
            |a, b| a == b,
        ),
        SyntaxKind::NE => fold_comparison(
            &lhs_lowered,
            &rhs_lowered,
            span,
            |a, b| a != b,
            |a: f64, b: f64| a != b,
            |a, b| a != b,
        ),
        SyntaxKind::LT => fold_comparison(
            &lhs_lowered,
            &rhs_lowered,
            span,
            |a, b| a < b,
            |a: f64, b: f64| a < b,
            |a, b| !a && b,
        ),
        SyntaxKind::GT => fold_comparison(
            &lhs_lowered,
            &rhs_lowered,
            span,
            |a, b| a > b,
            |a: f64, b: f64| a > b,
            |a, b| a && !b,
        ),
        SyntaxKind::LE => fold_comparison(
            &lhs_lowered,
            &rhs_lowered,
            span,
            |a, b| a <= b,
            |a: f64, b: f64| a <= b,
            |a, b| !a | b,
        ),
        SyntaxKind::GE => fold_comparison(
            &lhs_lowered,
            &rhs_lowered,
            span,
            |a, b| a >= b,
            |a: f64, b: f64| a >= b,
            |a, b| a || !b,
        ),

        // Logical operators
        SyntaxKind::AND_AND => fold_logical_and(&lhs_lowered, &rhs_lowered, span),
        SyntaxKind::OR_OR => fold_logical_or(&lhs_lowered, &rhs_lowered, span),

        _ => None,
    }
}

/// Fold arithmetic operations on integer or float literals.
fn fold_arithmetic<F, G>(
    lhs: &LoweredExpr,
    rhs: &LoweredExpr,
    span: Span,
    int_op: F,
    float_op: G,
) -> Option<LoweredExpr>
where
    F: FnOnce(i128, i128) -> Option<i128>,
    G: FnOnce(f64, f64) -> f64,
{
    match (lhs, rhs) {
        (
            LoweredExpr::IntLiteral {
                value: lv,
                suffix: ls,
                ..
            },
            LoweredExpr::IntLiteral {
                value: rv,
                suffix: rs,
                ..
            },
        ) => {
            // Use the suffix from whichever has one, or None if neither
            let suffix = ls.or(*rs);
            let result = int_op(*lv, *rv)?; // Returns None on overflow
            Some(LoweredExpr::IntLiteral {
                value: result,
                suffix,
                span,
            })
        }
        (
            LoweredExpr::FloatLiteral {
                value: lv,
                suffix: ls,
                ..
            },
            LoweredExpr::FloatLiteral {
                value: rv,
                suffix: rs,
                ..
            },
        ) => {
            let suffix = ls.or(*rs);
            let result = float_op(*lv, *rv);
            Some(LoweredExpr::FloatLiteral {
                value: result,
                suffix,
                span,
            })
        }
        _ => None,
    }
}

/// Fold division operations (handles div by zero).
fn fold_div(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span) -> Option<LoweredExpr> {
    match (lhs, rhs) {
        (
            LoweredExpr::IntLiteral {
                value: lv,
                suffix: ls,
                ..
            },
            LoweredExpr::IntLiteral {
                value: rv,
                suffix: rs,
                ..
            },
        ) => {
            // Integer division by zero -> passthrough (runtime error)
            if *rv == 0 {
                return None;
            }
            let suffix = ls.or(*rs);
            let result = lv.checked_div(*rv)?;
            Some(LoweredExpr::IntLiteral {
                value: result,
                suffix,
                span,
            })
        }
        (
            LoweredExpr::FloatLiteral {
                value: lv,
                suffix: ls,
                ..
            },
            LoweredExpr::FloatLiteral {
                value: rv,
                suffix: rs,
                ..
            },
        ) => {
            // Float division by zero produces infinity (valid, fold it)
            let suffix = ls.or(*rs);
            let result = lv / rv;
            Some(LoweredExpr::FloatLiteral {
                value: result,
                suffix,
                span,
            })
        }
        _ => None,
    }
}

/// Fold remainder operations (handles div by zero).
fn fold_rem(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span) -> Option<LoweredExpr> {
    match (lhs, rhs) {
        (
            LoweredExpr::IntLiteral {
                value: lv,
                suffix: ls,
                ..
            },
            LoweredExpr::IntLiteral {
                value: rv,
                suffix: rs,
                ..
            },
        ) => {
            // Integer remainder by zero -> passthrough (runtime error)
            if *rv == 0 {
                return None;
            }
            let suffix = ls.or(*rs);
            let result = lv.checked_rem(*rv)?;
            Some(LoweredExpr::IntLiteral {
                value: result,
                suffix,
                span,
            })
        }
        _ => None, // No float remainder in folding
    }
}

/// Fold comparison operations.
fn fold_comparison<F, G, H>(
    lhs: &LoweredExpr,
    rhs: &LoweredExpr,
    span: Span,
    int_cmp: F,
    float_cmp: G,
    bool_cmp: H,
) -> Option<LoweredExpr>
where
    F: FnOnce(i128, i128) -> bool,
    G: FnOnce(f64, f64) -> bool,
    H: FnOnce(bool, bool) -> bool,
{
    match (lhs, rhs) {
        (LoweredExpr::IntLiteral { value: lv, .. }, LoweredExpr::IntLiteral { value: rv, .. }) => {
            let result = int_cmp(*lv, *rv);
            Some(LoweredExpr::BoolLiteral {
                value: result,
                span,
            })
        }
        (
            LoweredExpr::FloatLiteral { value: lv, .. },
            LoweredExpr::FloatLiteral { value: rv, .. },
        ) => {
            let result = float_cmp(*lv, *rv);
            Some(LoweredExpr::BoolLiteral {
                value: result,
                span,
            })
        }
        (
            LoweredExpr::BoolLiteral { value: lv, .. },
            LoweredExpr::BoolLiteral { value: rv, .. },
        ) => {
            let result = bool_cmp(*lv, *rv);
            Some(LoweredExpr::BoolLiteral {
                value: result,
                span,
            })
        }
        _ => None,
    }
}

/// Fold logical AND operations.
fn fold_logical_and(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span) -> Option<LoweredExpr> {
    match (lhs, rhs) {
        (
            LoweredExpr::BoolLiteral { value: lv, .. },
            LoweredExpr::BoolLiteral { value: rv, .. },
        ) => Some(LoweredExpr::BoolLiteral {
            value: *lv && *rv,
            span,
        }),
        _ => None,
    }
}

/// Fold logical OR operations.
fn fold_logical_or(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span) -> Option<LoweredExpr> {
    match (lhs, rhs) {
        (
            LoweredExpr::BoolLiteral { value: lv, .. },
            LoweredExpr::BoolLiteral { value: rv, .. },
        ) => Some(LoweredExpr::BoolLiteral {
            value: *lv || *rv,
            span,
        }),
        _ => None,
    }
}
