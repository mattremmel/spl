//! HIR to MIR lowering.
//!
//! This module provides the infrastructure for lowering HIR (High-level IR)
//! to MIR (Mid-level IR). The lowering process converts nested expressions
//! into a flat, control-flow-graph representation suitable for borrow checking
//! and optimization.

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
