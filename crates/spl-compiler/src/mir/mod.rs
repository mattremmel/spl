//! MIR re-exports from spl-mir crate with lowering, optimization, and validation.
//!
//! This module re-exports all types from `spl_mir` and adds the lowering,
//! optimization, pretty printing, and validation modules that depend on spl-compiler types.

pub mod lower;
pub mod optimize;
pub mod pretty;
pub mod validate;

// Re-export everything from spl-mir
pub use spl_mir::*;

// Re-export module-specific types that depend on spl-compiler
pub use lower::{
    MirBuilder, MirLoweringContext, hir_binop_to_mir, hir_unop_to_mir, literal_to_operand,
    lower_hir_to_mir, lower_literal,
};
pub use optimize::{OptimizationContext, OptimizationPass, PassResult, optimize_mir};
pub use pretty::{MirPrinter, pretty_print};
pub use validate::{ValidationContext, validate_mir};
