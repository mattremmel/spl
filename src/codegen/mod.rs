//! Native code generation via Cranelift.
//!
//! This module provides JIT and AOT compilation support for SPL programs using
//! the Cranelift code generator. The compilation pipeline is:
//!
//! ```text
//! MIR Bodies → Cranelift IR → Machine Code
//! ```
//!
//! # Module Structure
//!
//! - [`error`]: Error types for code generation
//! - [`target`]: Target ISA configuration (native and cross-compilation)
//! - [`types`]: SPL type to Cranelift type mapping
//! - [`locals`]: Local variable storage allocation
//! - [`context`]: JIT compilation context
//!
//! # Usage
//!
//! For JIT compilation:
//!
//! ```ignore
//! use spl::codegen::CodegenContext;
//!
//! let mut ctx = CodegenContext::new_jit()?;
//! // ... declare and define functions ...
//! ctx.finalize();
//! let ptr = ctx.get_function_ptr(func_id);
//! ```

pub mod context;
pub mod error;
pub mod locals;
pub mod lower;
pub mod target;
pub mod types;

pub use context::CodegenContext;
pub use error::CodegenError;
pub use locals::{LocalMap, LocalStorage};
pub use lower::FunctionLowerer;
pub use target::TargetConfig;
pub use types::TypeMapper;

use crate::mir::Body;

/// JIT compile MIR bodies and return a function pointer to the entry point.
///
/// Note: For single-function JIT compilation with full control, use
/// `FunctionLowerer::compile()` directly. This function is intended for
/// compiling multiple MIR bodies as a complete program.
///
/// Currently returns an error as multi-function compilation requires
/// additional infrastructure (function linking, etc.).
pub fn codegen_jit(_bodies: &[Body]) -> Result<*const u8, CodegenError> {
    Err(CodegenError::Internal(
        "Multi-function compilation not yet implemented. Use FunctionLowerer::compile() for single functions.".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codegen_context_creates() {
        let ctx = CodegenContext::new_jit();
        assert!(
            ctx.is_ok(),
            "failed to create codegen context: {:?}",
            ctx.err()
        );
    }

    #[test]
    fn codegen_jit_stub_returns_error() {
        let result = codegen_jit(&[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Multi-function"));
    }
}
