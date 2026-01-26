//! HIR to MIR lowering.
//!
//! This module provides the infrastructure for lowering HIR (High-level IR)
//! to MIR (Mid-level IR). The lowering process converts nested expressions
//! into a flat, control-flow-graph representation suitable for borrow checking
//! and optimization.
//!
//! # Error Handling
//!
//! MIR lowering returns [`IceResult`] for error handling. Errors at this stage
//! indicate compiler bugs (Internal Compiler Errors), not user errors.
//!
//! ## Assumes Well-Formed Input
//!
//! By the time code reaches MIR lowering, it has passed through:
//! 1. **Parsing**: Syntax is valid (or errors were reported and recovered)
//! 2. **Name resolution**: All identifiers resolve to valid definitions
//! 3. **Type inference**: All expressions have valid types
//! 4. **HIR lowering**: AST is converted to well-formed HIR
//!
//! MIR lowering assumes its input HIR is **well-formed and type-checked**.
//! Any violation of this assumption indicates a **compiler bug**, not a user error.
//!
//! ## Why `IceError` Instead of Panic?
//!
//! 1. **Better diagnostics**: `IceError` captures context (spans, `DefId`s) that
//!    helps developers diagnose compiler bugs.
//!
//! 2. **Graceful degradation**: The compiler can report the ICE as a diagnostic
//!    rather than crashing, improving user experience.
//!
//! 3. **Testability**: Errors can be tested without catching panics.
//!
//! ## Handling `Missing` HIR Nodes
//!
//! [`HirExprKind::Missing`](crate::hir::HirExprKind::Missing) represents
//! expressions that couldn't be lowered from the AST (due to earlier errors).
//! MIR lowering handles these gracefully by producing undefined/poison values,
//! since the user has already been notified of the original error.

mod builder;
mod context;
mod helpers;
#[cfg(test)]
mod tests;

pub use builder::MirBuilder;
pub use context::{LoopContext, MirLoweringContext};
pub use helpers::{hir_binop_to_mir, hir_unop_to_mir, literal_to_operand, lower_literal};

use crate::hir::{HirDatabase, HirItem};
use crate::mir::body::Body;
use crate::mir::error::IceResult;

/// Lower all functions in an HIR database to MIR bodies.
///
/// # Errors
///
/// Returns [`IceError`](crate::mir::IceError) if the HIR database contains
/// invariant violations:
/// - Unresolved types (`Type::Error` in non-error-recovery positions)
/// - Missing expressions (`HirExprKind::Missing`) in invalid contexts
/// - Invalid `DefId` references (variables not in scope)
/// - Malformed struct field references
///
/// These conditions indicate bugs in earlier compiler phases (parsing,
/// name resolution, type inference, or HIR lowering), not user errors.
/// The user should have already received error diagnostics for any
/// issues in their source code.
pub fn lower_hir_to_mir(hir: &HirDatabase) -> IceResult<Vec<Body>> {
    let mut ctx = MirLoweringContext::new(hir);

    for item in &hir.items {
        if let HirItem::Function(func) = item {
            let body = ctx.lower_function(func)?;
            ctx.bodies.push(body);
        }
    }

    Ok(ctx.bodies)
}
