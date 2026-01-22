//! HIR to MIR lowering.
//!
//! This module provides the infrastructure for lowering HIR (High-level IR)
//! to MIR (Mid-level IR). The lowering process converts nested expressions
//! into a flat, control-flow-graph representation suitable for borrow checking
//! and optimization.
//!
//! # Error Handling: Panic on Invariant Violation
//!
//! MIR lowering uses `panic!()` for error handling. This is intentional.
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
//! ## Why Panic Instead of Result?
//!
//! 1. **Invalid MIR is worse than crashing**: If MIR lowering tried to continue
//!    after an invariant violation, it would produce malformed MIR. This could
//!    cause silent miscompilation, incorrect codegen, or confusing downstream
//!    panics. Failing fast makes bugs easier to diagnose.
//!
//! 2. **No user-actionable errors**: At this stage, there's nothing the user
//!    can do to fix an invariant violation. The appropriate response is to
//!    file a bug report, not modify their source code.
//!
//! 3. **Simpler code**: Using `panic!()` instead of `Result` keeps the lowering
//!    code focused on the happy path, without error propagation boilerplate.
//!
//! ## Handling `Missing` HIR Nodes
//!
//! The one exception is [`HirExprKind::Missing`](crate::hir::HirExprKind::Missing),
//! which represents expressions that couldn't be lowered from the AST (due to
//! earlier errors). MIR lowering should handle these gracefully, typically by
//! producing undefined/poison values, since the user has already been notified
//! of the original error.

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

/// Lower all functions in an HIR database to MIR bodies.
pub fn lower_hir_to_mir(hir: &HirDatabase) -> Vec<Body> {
    let mut ctx = MirLoweringContext::new(hir);

    for item in &hir.items {
        if let HirItem::Function(func) = item {
            let body = ctx.lower_function(func);
            ctx.bodies.push(body);
        }
    }

    ctx.bodies
}
