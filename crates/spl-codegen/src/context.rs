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
use super::runtime::Runtime;
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

    /// Create a new JIT compilation context with runtime function support.
    ///
    /// This variant allows registering external Rust functions that can be
    /// called from JIT-compiled code.
    pub fn new_jit_with_runtime(runtime: &Runtime) -> Result<Self, CodegenError> {
        let target = TargetConfig::native()?;

        let mut builder = JITBuilder::with_isa(
            target.isa().clone(),
            cranelift_module::default_libcall_names(),
        );

        // Register runtime symbols
        for func in runtime.iter() {
            builder.symbol(func.name, func.ptr);
        }

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

    /// Get the default calling convention for this target.
    pub fn call_conv(&self) -> cranelift_codegen::isa::CallConv {
        self.module.isa().default_call_conv()
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
        self.declare_function_with_linkage(name, signature, Linkage::Export)
    }

    /// Declare a function in the module with the specified linkage.
    ///
    /// Use `Linkage::Export` for functions defined in this module.
    /// Use `Linkage::Import` for external functions (e.g., runtime intrinsics).
    pub fn declare_function_with_linkage(
        &mut self,
        name: &str,
        signature: &cranelift_codegen::ir::Signature,
        linkage: Linkage,
    ) -> Result<FuncId, CodegenError> {
        self.module
            .declare_function(name, linkage, signature)
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

    #[test]
    fn new_jit_with_empty_runtime() {
        let runtime = Runtime::new();
        let ctx = CodegenContext::new_jit_with_runtime(&runtime);
        assert!(ctx.is_ok());
    }

    #[test]
    fn new_jit_with_runtime_registers_symbols() {
        extern "C" fn external_add(a: i32, b: i32) -> i32 {
            a + b
        }

        let mut runtime = Runtime::new();
        let mut sig =
            cranelift_codegen::ir::Signature::new(cranelift_codegen::isa::CallConv::SystemV);
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig.returns.push(AbiParam::new(types::I32));
        runtime.register("external_add", external_add as *const u8, sig);

        let ctx = CodegenContext::new_jit_with_runtime(&runtime);
        assert!(ctx.is_ok());
    }

    #[test]
    fn call_external_function_from_jit() {
        use cranelift_module::Module;

        extern "C" fn external_mul(a: i32, b: i32) -> i32 {
            a * b
        }

        // Set up runtime with external function
        let mut runtime = Runtime::new();
        let mut external_sig =
            cranelift_codegen::ir::Signature::new(cranelift_codegen::isa::CallConv::SystemV);
        external_sig.params.push(AbiParam::new(types::I32));
        external_sig.params.push(AbiParam::new(types::I32));
        external_sig.returns.push(AbiParam::new(types::I32));
        runtime.register(
            "external_mul",
            external_mul as *const u8,
            external_sig.clone(),
        );

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        // Declare the external function in the module
        let external_func_id = ctx
            .module
            .declare_function("external_mul", Linkage::Import, &external_sig)
            .unwrap();

        // Create a wrapper function that calls the external
        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.returns.push(AbiParam::new(types::I32));

        let wrapper_id = ctx.declare_function("wrapper", &wrapper_sig).unwrap();

        // Build wrapper: fn wrapper() -> i32 { external_mul(6, 7) }
        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            // Import the external function
            let func_ref = module.declare_func_in_func(external_func_id, builder.func);

            // Call external_mul(6, 7)
            let arg1 = builder.ins().iconst(types::I32, 6);
            let arg2 = builder.ins().iconst(types::I32, 7);
            let call = builder.ins().call(func_ref, &[arg1, arg2]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let wrapper: fn() -> i32 = unsafe { mem::transmute(ptr) };

        assert_eq!(wrapper(), 42); // 6 * 7 = 42
    }

    #[test]
    fn call_external_function_no_params() {
        use cranelift_module::Module;
        use std::sync::atomic::{AtomicI32, Ordering};

        static CALL_COUNT: AtomicI32 = AtomicI32::new(0);

        extern "C" fn get_value() -> i32 {
            CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            42
        }

        // Reset counter
        CALL_COUNT.store(0, Ordering::SeqCst);

        let mut runtime = Runtime::new();
        let mut external_sig =
            cranelift_codegen::ir::Signature::new(cranelift_codegen::isa::CallConv::SystemV);
        external_sig.returns.push(AbiParam::new(types::I32));
        runtime.register("get_value", get_value as *const u8, external_sig.clone());

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        let external_func_id = ctx
            .module
            .declare_function("get_value", Linkage::Import, &external_sig)
            .unwrap();

        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.returns.push(AbiParam::new(types::I32));
        let wrapper_id = ctx.declare_function("wrapper", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(external_func_id, builder.func);
            let call = builder.ins().call(func_ref, &[]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let wrapper: fn() -> i32 = unsafe { mem::transmute(ptr) };

        assert_eq!(wrapper(), 42);
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn call_external_function_with_side_effect() {
        use cranelift_module::Module;
        use std::sync::atomic::{AtomicI32, Ordering};

        static COUNTER: AtomicI32 = AtomicI32::new(0);

        extern "C" fn increment_counter() -> i32 {
            COUNTER.fetch_add(1, Ordering::SeqCst)
        }

        // Reset counter
        COUNTER.store(0, Ordering::SeqCst);

        let mut runtime = Runtime::new();
        let mut external_sig =
            cranelift_codegen::ir::Signature::new(cranelift_codegen::isa::CallConv::SystemV);
        external_sig.returns.push(AbiParam::new(types::I32));
        runtime.register(
            "increment_counter",
            increment_counter as *const u8,
            external_sig.clone(),
        );

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        let external_func_id = ctx
            .module
            .declare_function("increment_counter", Linkage::Import, &external_sig)
            .unwrap();

        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.returns.push(AbiParam::new(types::I32));
        let wrapper_id = ctx.declare_function("wrapper", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(external_func_id, builder.func);
            let call = builder.ins().call(func_ref, &[]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let wrapper: fn() -> i32 = unsafe { mem::transmute(ptr) };

        // Each call should increment the counter
        assert_eq!(wrapper(), 0); // Returns old value (0), counter becomes 1
        assert_eq!(wrapper(), 1); // Returns old value (1), counter becomes 2
        assert_eq!(wrapper(), 2); // Returns old value (2), counter becomes 3
        assert_eq!(COUNTER.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn call_external_function_i64() {
        use cranelift_module::Module;

        extern "C" fn add_i64(a: i64, b: i64) -> i64 {
            a + b
        }

        let mut runtime = Runtime::new();
        let mut external_sig =
            cranelift_codegen::ir::Signature::new(cranelift_codegen::isa::CallConv::SystemV);
        external_sig.params.push(AbiParam::new(types::I64));
        external_sig.params.push(AbiParam::new(types::I64));
        external_sig.returns.push(AbiParam::new(types::I64));
        runtime.register("add_i64", add_i64 as *const u8, external_sig.clone());

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        let external_func_id = ctx
            .module
            .declare_function("add_i64", Linkage::Import, &external_sig)
            .unwrap();

        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.returns.push(AbiParam::new(types::I64));
        let wrapper_id = ctx.declare_function("wrapper", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(external_func_id, builder.func);
            let arg1 = builder.ins().iconst(types::I64, 1_000_000_000_000i64);
            let arg2 = builder.ins().iconst(types::I64, 2_000_000_000_000i64);
            let call = builder.ins().call(func_ref, &[arg1, arg2]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let wrapper: fn() -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(wrapper(), 3_000_000_000_000i64);
    }

    #[test]
    fn multiple_runtime_functions() {
        use cranelift_module::Module;

        extern "C" fn op_add(a: i32, b: i32) -> i32 {
            a + b
        }
        extern "C" fn op_sub(a: i32, b: i32) -> i32 {
            a - b
        }

        let mut runtime = Runtime::new();
        let mut sig =
            cranelift_codegen::ir::Signature::new(cranelift_codegen::isa::CallConv::SystemV);
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig.returns.push(AbiParam::new(types::I32));

        runtime.register("op_add", op_add as *const u8, sig.clone());
        runtime.register("op_sub", op_sub as *const u8, sig.clone());

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        let add_func_id = ctx
            .module
            .declare_function("op_add", Linkage::Import, &sig)
            .unwrap();
        let sub_func_id = ctx
            .module
            .declare_function("op_sub", Linkage::Import, &sig)
            .unwrap();

        // fn wrapper() -> i32 { op_add(10, 5) + op_sub(10, 5) } = 15 + 5 = 20
        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.returns.push(AbiParam::new(types::I32));
        let wrapper_id = ctx.declare_function("wrapper", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let add_ref = module.declare_func_in_func(add_func_id, builder.func);
            let sub_ref = module.declare_func_in_func(sub_func_id, builder.func);

            let ten = builder.ins().iconst(types::I32, 10);
            let five = builder.ins().iconst(types::I32, 5);

            let add_call = builder.ins().call(add_ref, &[ten, five]);
            let add_result = builder.inst_results(add_call)[0];

            let sub_call = builder.ins().call(sub_ref, &[ten, five]);
            let sub_result = builder.inst_results(sub_call)[0];

            let total = builder.ins().iadd(add_result, sub_result);

            builder.ins().return_(&[total]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let wrapper: fn() -> i32 = unsafe { mem::transmute(ptr) };

        assert_eq!(wrapper(), 20); // (10+5) + (10-5) = 15 + 5 = 20
    }
}
