//! HIR re-exports from spl-hir crate with lowering functions.
//!
//! This module re-exports all types from `spl_hir` and adds the lowering
//! functions that depend on spl-compiler types.

pub mod lower;

// Re-export everything from spl-hir
pub use spl_hir::*;
