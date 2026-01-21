//! High-level Intermediate Representation (future).
//!
//! HIR differs from AST:
//! - Names resolved to DefIds
//! - Types attached to all expressions
//! - Desugared constructs (for → while, etc.)
//! - Arena-allocated with stable IDs

pub mod lower;

use crate::lexer::Span;
use crate::sema::types::PrimitiveKind;

/// A lowered expression for literals that need folding.
///
/// This is used to handle negated integer literals like `-128i8` and `-(128i8)`.
/// Most expressions pass through unchanged (Passthrough variant).
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
    /// Not foldable - use AST directly.
    Passthrough,
}
