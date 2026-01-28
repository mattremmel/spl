//! Literal folding for type inference.
//!
//! This module handles compile-time evaluation of constant expressions like:
//! - Negated integer literals: `-128i8`
//! - Boolean negation: `!true`
//! - Binary operations on literals: `1 + 2`

use spl_ast::{BinExpr, Expr, LiteralExpr, PrefixExpr};
use spl_lexer::Span;
use crate::types::PrimitiveKind;
use spl_syntax::SyntaxKind;
use rowan::ast::AstNode;

/// A lowered expression for literals that need folding.
#[derive(Debug, Clone)]
pub enum LoweredExpr {
    /// An integer literal with its (possibly negated) value and optional type suffix.
    IntLiteral {
        value: i128,
        suffix: Option<PrimitiveKind>,
        span: Span,
    },
    /// A float literal with its (possibly negated) value and optional type suffix.
    FloatLiteral {
        value: f64,
        suffix: Option<PrimitiveKind>,
        span: Span,
    },
    /// A boolean literal value.
    BoolLiteral { value: bool, span: Span },
    /// Not foldable - use AST directly.
    Passthrough,
}

/// Try to lower an expression for literal folding.
pub fn try_lower_expr(expr: &Expr) -> (LoweredExpr, bool) {
    match expr {
        Expr::Literal(lit) => {
            if let Some(lowered) = try_lower_literal(lit) {
                return (lowered, true);
            }
        }
        Expr::Prefix(prefix) => {
            if let Some(lowered) = lower_negated_literal(prefix) {
                return (lowered, true);
            }
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
            if let Some(inner) = paren.expr() {
                return try_lower_expr(&inner);
            }
        }
        _ => {}
    }
    (LoweredExpr::Passthrough, false)
}

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
            Some(LoweredExpr::IntLiteral { value, suffix, span })
        }
        SyntaxKind::FLOAT_LITERAL => {
            let (suffix, value) = parse_float_literal(text)?;
            Some(LoweredExpr::FloatLiteral { value, suffix, span })
        }
        _ => None,
    }
}

fn lower_not_literal(prefix: &PrefixExpr) -> Option<LoweredExpr> {
    let op_token = prefix.op_token()?;
    if op_token.kind() != SyntaxKind::BANG {
        return None;
    }

    let inner = prefix.expr()?;
    let span = text_range_to_span(prefix.syntax().text_range());
    let (inner_lowered, was_lowered) = try_lower_expr(&inner);

    if was_lowered
        && let LoweredExpr::BoolLiteral { value, .. } = inner_lowered
    {
        return Some(LoweredExpr::BoolLiteral { value: !value, span });
    }
    None
}

fn lower_negated_literal(prefix: &PrefixExpr) -> Option<LoweredExpr> {
    let op_token = prefix.op_token()?;
    if op_token.kind() != SyntaxKind::MINUS {
        return None;
    }

    let inner = prefix.expr()?;
    let span = text_range_to_span(prefix.syntax().text_range());
    let (inner_lowered, was_lowered) = try_lower_expr(&inner);

    if was_lowered {
        return match inner_lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                Some(LoweredExpr::IntLiteral { value: -value, suffix, span })
            }
            LoweredExpr::FloatLiteral { value, suffix, .. } => {
                Some(LoweredExpr::FloatLiteral { value: -value, suffix, span })
            }
            _ => None,
        };
    }
    None
}

fn lower_binary_literal(bin: &BinExpr) -> Option<LoweredExpr> {
    let lhs = bin.lhs()?;
    let rhs = bin.rhs()?;
    let op_token = bin.op_token()?;
    let span = text_range_to_span(bin.syntax().text_range());

    let (lhs_lowered, lhs_ok) = try_lower_expr(&lhs);
    let (rhs_lowered, rhs_ok) = try_lower_expr(&rhs);

    if !lhs_ok || !rhs_ok {
        return None;
    }

    match op_token.kind() {
        SyntaxKind::PLUS => fold_arithmetic(&lhs_lowered, &rhs_lowered, span, i128::checked_add, |a, b| a + b),
        SyntaxKind::MINUS => fold_arithmetic(&lhs_lowered, &rhs_lowered, span, i128::checked_sub, |a, b| a - b),
        SyntaxKind::STAR => fold_arithmetic(&lhs_lowered, &rhs_lowered, span, i128::checked_mul, |a, b| a * b),
        SyntaxKind::SLASH => fold_div(&lhs_lowered, &rhs_lowered, span),
        SyntaxKind::PERCENT => fold_rem(&lhs_lowered, &rhs_lowered, span),
        SyntaxKind::EQ_EQ => fold_comparison(&lhs_lowered, &rhs_lowered, span, |a, b| a == b, |a: f64, b: f64| a == b, |a, b| a == b),
        SyntaxKind::NE => fold_comparison(&lhs_lowered, &rhs_lowered, span, |a, b| a != b, |a: f64, b: f64| a != b, |a, b| a != b),
        SyntaxKind::LT => fold_comparison(&lhs_lowered, &rhs_lowered, span, |a, b| a < b, |a: f64, b: f64| a < b, |a, b| !a && b),
        SyntaxKind::GT => fold_comparison(&lhs_lowered, &rhs_lowered, span, |a, b| a > b, |a: f64, b: f64| a > b, |a, b| a && !b),
        SyntaxKind::LE => fold_comparison(&lhs_lowered, &rhs_lowered, span, |a, b| a <= b, |a: f64, b: f64| a <= b, |a, b| !a | b),
        SyntaxKind::GE => fold_comparison(&lhs_lowered, &rhs_lowered, span, |a, b| a >= b, |a: f64, b: f64| a >= b, |a, b| a || !b),
        SyntaxKind::AND_AND => fold_logical_and(&lhs_lowered, &rhs_lowered, span),
        SyntaxKind::OR_OR => fold_logical_or(&lhs_lowered, &rhs_lowered, span),
        _ => None,
    }
}

fn fold_arithmetic<F, G>(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span, int_op: F, float_op: G) -> Option<LoweredExpr>
where
    F: FnOnce(i128, i128) -> Option<i128>,
    G: FnOnce(f64, f64) -> f64,
{
    match (lhs, rhs) {
        (LoweredExpr::IntLiteral { value: lv, suffix: ls, .. }, LoweredExpr::IntLiteral { value: rv, suffix: rs, .. }) => {
            let suffix = ls.or(*rs);
            let result = int_op(*lv, *rv)?;
            Some(LoweredExpr::IntLiteral { value: result, suffix, span })
        }
        (LoweredExpr::FloatLiteral { value: lv, suffix: ls, .. }, LoweredExpr::FloatLiteral { value: rv, suffix: rs, .. }) => {
            let suffix = ls.or(*rs);
            let result = float_op(*lv, *rv);
            Some(LoweredExpr::FloatLiteral { value: result, suffix, span })
        }
        _ => None,
    }
}

fn fold_div(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span) -> Option<LoweredExpr> {
    match (lhs, rhs) {
        (LoweredExpr::IntLiteral { value: lv, suffix: ls, .. }, LoweredExpr::IntLiteral { value: rv, suffix: rs, .. }) => {
            if *rv == 0 { return None; }
            let suffix = ls.or(*rs);
            let result = lv.checked_div(*rv)?;
            Some(LoweredExpr::IntLiteral { value: result, suffix, span })
        }
        (LoweredExpr::FloatLiteral { value: lv, suffix: ls, .. }, LoweredExpr::FloatLiteral { value: rv, suffix: rs, .. }) => {
            let suffix = ls.or(*rs);
            let result = lv / rv;
            Some(LoweredExpr::FloatLiteral { value: result, suffix, span })
        }
        _ => None,
    }
}

fn fold_rem(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span) -> Option<LoweredExpr> {
    match (lhs, rhs) {
        (LoweredExpr::IntLiteral { value: lv, suffix: ls, .. }, LoweredExpr::IntLiteral { value: rv, suffix: rs, .. }) => {
            if *rv == 0 { return None; }
            let suffix = ls.or(*rs);
            let result = lv.checked_rem(*rv)?;
            Some(LoweredExpr::IntLiteral { value: result, suffix, span })
        }
        _ => None,
    }
}

fn fold_comparison<F, G, H>(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span, int_cmp: F, float_cmp: G, bool_cmp: H) -> Option<LoweredExpr>
where
    F: FnOnce(i128, i128) -> bool,
    G: FnOnce(f64, f64) -> bool,
    H: FnOnce(bool, bool) -> bool,
{
    match (lhs, rhs) {
        (LoweredExpr::IntLiteral { value: lv, .. }, LoweredExpr::IntLiteral { value: rv, .. }) => {
            Some(LoweredExpr::BoolLiteral { value: int_cmp(*lv, *rv), span })
        }
        (LoweredExpr::FloatLiteral { value: lv, .. }, LoweredExpr::FloatLiteral { value: rv, .. }) => {
            Some(LoweredExpr::BoolLiteral { value: float_cmp(*lv, *rv), span })
        }
        (LoweredExpr::BoolLiteral { value: lv, .. }, LoweredExpr::BoolLiteral { value: rv, .. }) => {
            Some(LoweredExpr::BoolLiteral { value: bool_cmp(*lv, *rv), span })
        }
        _ => None,
    }
}

fn fold_logical_and(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span) -> Option<LoweredExpr> {
    match (lhs, rhs) {
        (LoweredExpr::BoolLiteral { value: lv, .. }, LoweredExpr::BoolLiteral { value: rv, .. }) => {
            Some(LoweredExpr::BoolLiteral { value: *lv && *rv, span })
        }
        _ => None,
    }
}

fn fold_logical_or(lhs: &LoweredExpr, rhs: &LoweredExpr, span: Span) -> Option<LoweredExpr> {
    match (lhs, rhs) {
        (LoweredExpr::BoolLiteral { value: lv, .. }, LoweredExpr::BoolLiteral { value: rv, .. }) => {
            Some(LoweredExpr::BoolLiteral { value: *lv || *rv, span })
        }
        _ => None,
    }
}

fn text_range_to_span(range: rowan::TextRange) -> Span {
    range.start().into()..range.end().into()
}

pub fn parse_int_suffix(text: &str) -> (Option<PrimitiveKind>, bool) {
    let suffixes = [
        ("i128", PrimitiveKind::I128), ("u128", PrimitiveKind::U128),
        ("isize", PrimitiveKind::Isize), ("usize", PrimitiveKind::Usize),
        ("i64", PrimitiveKind::I64), ("u64", PrimitiveKind::U64),
        ("i32", PrimitiveKind::I32), ("u32", PrimitiveKind::U32),
        ("i16", PrimitiveKind::I16), ("u16", PrimitiveKind::U16),
        ("i8", PrimitiveKind::I8), ("u8", PrimitiveKind::U8),
    ];
    for (suffix, kind) in suffixes {
        if text.ends_with(suffix) {
            return (Some(kind), true);
        }
    }
    (None, false)
}

pub fn parse_int_literal_value(text: &str) -> Option<i128> {
    let suffixes = ["i128", "u128", "isize", "usize", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8"];
    let num_text = suffixes.iter()
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
