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
//! - [`aot`]: AOT (ahead-of-time) compilation context for object files
//! - [`context`]: JIT compilation context
//! - [`error`]: Error types for code generation
//! - [`link`]: Linker abstraction for creating executables
//! - [`locals`]: Local variable storage allocation
//! - [`module`]: Module-level compilation (JIT and AOT)
//! - [`target`]: Target ISA configuration (native and cross-compilation)
//! - [`types`]: SPL type to Cranelift type mapping
//!
//! # Usage
//!
//! ## JIT Compilation
//!
//! For immediate execution:
//!
//! ```ignore
//! use spl::codegen::CodegenContext;
//!
//! let mut ctx = CodegenContext::new_jit()?;
//! // ... declare and define functions ...
//! ctx.finalize();
//! let ptr = ctx.get_function_ptr(func_id);
//! ```
//!
//! ## AOT Compilation
//!
//! For generating object files and executables:
//!
//! ```ignore
//! use spl::codegen::{AotModuleCompiler, link_object_to_executable};
//! use std::path::Path;
//!
//! // Compile to object file
//! let obj = AotModuleCompiler::compile(&functions, &types)?;
//! let object_bytes = obj.into_bytes();
//!
//! // Link to executable
//! link_object_to_executable(&object_bytes, Path::new("output"), None)?;
//! ```

pub mod aot;
pub mod context;
pub mod error;
pub mod layout;
pub mod link;
pub mod locals;
pub mod lower;
pub mod module;
pub mod registry;
pub mod runtime;
pub mod target;
pub mod types;

pub use aot::AotContext;
pub use context::CodegenContext;
pub use error::{CodegenError, RuntimeError, TRAP_ASSERT_FAILED, TRAP_RESUME, TRAP_UNREACHABLE};
pub use layout::{LayoutComputer, TypeLayout};
pub use link::{CcLinker, LinkError, LinkOptions, Linker, link_object_to_executable};
pub use locals::{LocalMap, LocalStorage};
pub use lower::FunctionLowerer;
pub use module::{AotModuleCompiler, CompiledModule, CompiledObjectFile, FunctionDef, ModuleCompiler};
pub use registry::{FunctionInfo, FunctionRegistry};
pub use runtime::{Runtime, RuntimeFunction};
pub use target::TargetConfig;
pub use types::TypeMapper;

use crate::mir::Body;
use crate::sema::symbol::DefId;
use crate::sema::types::TypeInterner;

/// JIT compile MIR bodies and return a compiled module.
///
/// Note: For single-function JIT compilation with full control, use
/// `FunctionLowerer::compile()` directly. This function is intended for
/// compiling multiple MIR bodies as a complete program.
///
/// # Arguments
/// * `bodies` - Slice of (DefId, name, Body) tuples to compile
/// * `types` - The type interner for type lookups
///
/// # Returns
/// A compiled module with function pointers for each function.
pub fn codegen_jit(
    bodies: &[(DefId, &str, &Body)],
    types: &TypeInterner,
) -> Result<CompiledModule, CodegenError> {
    let function_defs: Vec<_> = bodies
        .iter()
        .map(|(def_id, name, body)| FunctionDef::new(*def_id, *name, body))
        .collect();

    ModuleCompiler::compile(&function_defs, types)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::body::Body;
    use crate::mir::operand::{Operand, Rvalue};
    use crate::mir::statement::Statement;
    use crate::mir::terminator::Terminator;
    use crate::mir::types::{Local, Place};
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
    fn codegen_jit_empty_compiles() {
        let types = TypeInterner::new();
        let result = codegen_jit(&[], &types);
        assert!(result.is_ok());
        let module = result.unwrap();
        assert!(module.is_empty());
    }

    #[test]
    fn codegen_jit_single_function() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(42)),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let result = codegen_jit(&[(DefId(1), "test_fn", &body)], &types);
        assert!(result.is_ok());
        let module = result.unwrap();
        assert_eq!(module.len(), 1);

        let ptr = module.get_function_ptr(DefId(1)).unwrap();
        let func: fn() -> i32 = unsafe { std::mem::transmute(ptr) };
        assert_eq!(func(), 42);
    }
}
