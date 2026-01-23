//! Code generation context for JIT compilation.
//!
//! This module provides the main compilation context that manages the JIT module,
//! Cranelift compilation context, and function builder context.

use cranelift_codegen::Context as ClifContext;
use cranelift_codegen::ir::Function;
use cranelift_frontend::FunctionBuilderContext;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use super::error::CodegenError;
use super::target::TargetConfig;
use super::types::TypeMapper;

/// Code generation context for JIT compilation.
///
/// Manages the JIT module and provides methods for declaring, defining,
/// and finalizing functions.
pub struct CodegenContext {
    /// The JIT module that holds compiled code.
    module: JITModule,

    /// Cranelift compilation context (reusable between functions).
    ctx: ClifContext,

    /// Function builder context (reusable between functions).
    func_ctx: FunctionBuilderContext,
}

impl CodegenContext {
    /// Create a new JIT compilation context for the native host.
    pub fn new_jit() -> Result<Self, CodegenError> {
        let target = TargetConfig::native()?;

        let builder = JITBuilder::with_isa(
            target.isa().clone(),
            cranelift_module::default_libcall_names(),
        );

        let module = JITModule::new(builder);
        let ctx = module.make_context();
        let func_ctx = FunctionBuilderContext::new();

        Ok(Self {
            module,
            ctx,
            func_ctx,
        })
    }

    /// Create a type mapper for the current target.
    pub fn type_mapper(&self) -> TypeMapper {
        TypeMapper::new(self.module.isa().pointer_type())
    }

    /// Declare a function in the module.
    ///
    /// Returns the function ID that can be used to define the function
    /// or get a function pointer after finalization.
    pub fn declare_function(
        &mut self,
        name: &str,
        signature: &cranelift_codegen::ir::Signature,
    ) -> Result<FuncId, CodegenError> {
        self.module
            .declare_function(name, Linkage::Export, signature)
            .map_err(|e| CodegenError::ModuleError(e.to_string()))
    }

    /// Get a mutable reference to the compilation context.
    ///
    /// Use this to set up the function before building.
    pub fn compilation_context(&mut self) -> &mut ClifContext {
        &mut self.ctx
    }

    /// Get a mutable reference to the function builder context.
    ///
    /// Use this when creating a `FunctionBuilder`.
    pub fn func_builder_context(&mut self) -> &mut FunctionBuilderContext {
        &mut self.func_ctx
    }

    /// Get the function from the context.
    pub fn func(&self) -> &Function {
        &self.ctx.func
    }

    /// Get a mutable reference to the function.
    pub fn func_mut(&mut self) -> &mut Function {
        &mut self.ctx.func
    }

    /// Get mutable references to both the function and the function builder context.
    ///
    /// This is useful for creating a `FunctionBuilder` without borrow conflicts.
    pub fn builder_context(&mut self) -> (&mut Function, &mut FunctionBuilderContext) {
        (&mut self.ctx.func, &mut self.func_ctx)
    }

    /// Get mutable references to the function, function builder context, and module.
    ///
    /// This is useful for multi-function compilation where function calls need
    /// to import function references into the current function.
    pub fn builder_context_with_module(
        &mut self,
    ) -> (&mut Function, &mut FunctionBuilderContext, &mut JITModule) {
        (&mut self.ctx.func, &mut self.func_ctx, &mut self.module)
    }

    /// Define a function in the module.
    ///
    /// Call this after building the function body with a `FunctionBuilder`.
    pub fn define_function(&mut self, func_id: FuncId) -> Result<(), CodegenError> {
        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

        self.module.clear_context(&mut self.ctx);
        Ok(())
    }

    /// Finalize all defined functions.
    ///
    /// This must be called after all functions have been defined and before
    /// getting function pointers.
    pub fn finalize(&mut self) {
        self.module.finalize_definitions().expect("finalize failed");
    }

    /// Get a function pointer for a defined function.
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid as long as this `CodegenContext`
    /// (and its `JITModule`) is alive. The caller must ensure the function
    /// signature matches the expected type.
    pub fn get_function_ptr(&self, func_id: FuncId) -> *const u8 {
        self.module.get_finalized_function(func_id)
    }

    /// Get the default calling convention for this target.
    pub fn default_call_conv(&self) -> cranelift_codegen::ir::types::Type {
        self.module.isa().pointer_type()
    }

    /// Get a reference to the underlying module.
    pub fn module(&self) -> &JITModule {
        &self.module
    }

    /// Create a new signature with the target's default calling convention.
    pub fn new_signature(&self) -> cranelift_codegen::ir::Signature {
        self.module.make_signature()
    }
}

#[cfg(test)]
mod tests {
    use std::mem;

    use super::*;
    use cranelift_codegen::ir::AbiParam;
    use cranelift_codegen::ir::InstBuilder;
    use cranelift_codegen::ir::types;
    use cranelift_frontend::FunctionBuilder;

    #[test]
    fn codegen_context_creates() {
        let ctx = CodegenContext::new_jit();
        assert!(ctx.is_ok(), "failed to create context: {:?}", ctx.err());
    }

    #[test]
    fn type_mapper_has_correct_pointer_type() {
        let ctx = CodegenContext::new_jit().unwrap();
        let mapper = ctx.type_mapper();

        // Pointer type should be either I32 or I64 depending on platform
        let ptr_type = mapper.pointer_type();
        assert!(ptr_type == types::I32 || ptr_type == types::I64);
    }

    #[test]
    fn new_signature_has_call_conv() {
        let ctx = CodegenContext::new_jit().unwrap();
        let sig = ctx.new_signature();

        // Signature should have a valid calling convention
        let _ = sig.call_conv;
    }

    #[test]
    fn declare_function() {
        let mut ctx = CodegenContext::new_jit().unwrap();
        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let result = ctx.declare_function("test_fn", &sig);
        assert!(
            result.is_ok(),
            "failed to declare function: {:?}",
            result.err()
        );
    }

    #[test]
    fn full_jit_workflow() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        // Declare a function that returns 42
        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let func_id = ctx.declare_function("returns_42", &sig).unwrap();

        // Build the function
        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let val = builder.ins().iconst(types::I32, 42);
            builder.ins().return_(&[val]);

            builder.finalize();
        }

        // Define and finalize
        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        // Get function pointer and call it
        let ptr = ctx.get_function_ptr(func_id);
        let func: fn() -> i32 = unsafe { mem::transmute(ptr) };
        assert_eq!(func(), 42);
    }

    #[test]
    fn jit_function_with_params() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        // Declare a function that adds two i32s
        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig.returns.push(AbiParam::new(types::I32));

        let func_id = ctx.declare_function("add_i32", &sig).unwrap();

        // Build the function
        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let a = builder.block_params(entry_block)[0];
            let b = builder.block_params(entry_block)[1];
            let sum = builder.ins().iadd(a, b);
            builder.ins().return_(&[sum]);

            builder.finalize();
        }

        // Define and finalize
        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        // Get function pointer and call it
        let ptr = ctx.get_function_ptr(func_id);
        let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

        assert_eq!(func(10, 32), 42);
        assert_eq!(func(0, 0), 0);
        assert_eq!(func(-1, 1), 0);
    }

    #[test]
    fn multiple_functions() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        // Declare two functions
        let mut sig1 = ctx.new_signature();
        sig1.returns.push(AbiParam::new(types::I32));
        let func1_id = ctx.declare_function("returns_1", &sig1).unwrap();

        let mut sig2 = ctx.new_signature();
        sig2.returns.push(AbiParam::new(types::I32));
        let func2_id = ctx.declare_function("returns_2", &sig2).unwrap();

        // Build first function
        ctx.compilation_context().func.signature = sig1;
        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);
            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);
            let val = builder.ins().iconst(types::I32, 1);
            builder.ins().return_(&[val]);
            builder.finalize();
        }
        ctx.define_function(func1_id).unwrap();

        // Build second function
        ctx.compilation_context().func.signature = sig2;
        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);
            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);
            let val = builder.ins().iconst(types::I32, 2);
            builder.ins().return_(&[val]);
            builder.finalize();
        }
        ctx.define_function(func2_id).unwrap();

        ctx.finalize();

        // Test both functions
        let ptr1 = ctx.get_function_ptr(func1_id);
        let ptr2 = ctx.get_function_ptr(func2_id);

        let func1: fn() -> i32 = unsafe { mem::transmute(ptr1) };
        let func2: fn() -> i32 = unsafe { mem::transmute(ptr2) };

        assert_eq!(func1(), 1);
        assert_eq!(func2(), 2);
    }

    #[test]
    fn module_accessor() {
        let ctx = CodegenContext::new_jit().unwrap();
        let module = ctx.module();

        // Should be able to access ISA through module
        let _ = module.isa();
    }

    #[test]
    fn declare_duplicate_function() {
        let mut ctx = CodegenContext::new_jit().unwrap();
        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let id1 = ctx.declare_function("same_name", &sig).unwrap();
        let id2 = ctx.declare_function("same_name", &sig).unwrap();

        // Declaring with same name and signature should return same ID
        assert_eq!(id1, id2);
    }
}
