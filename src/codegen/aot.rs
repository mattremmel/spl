//! Ahead-of-time compilation context.
//!
//! This module provides `AotContext` for compiling SPL programs to object files
//! that can be linked into standalone executables.
//!
//! # Intrinsics Strategy
//!
//! JIT and AOT handle intrinsics differently:
//!
//! - **JIT**: Calls Rust function pointers registered in `Runtime`
//! - **AOT**: Emits calls to libc/libm symbols, resolved at link time
//!
//! This means AOT binaries have no SPL runtime dependency - just standard `-lc -lm`.
//!
//! ## Intrinsic to libc Mapping
//!
//! | Intrinsic | libc/libm | Header |
//! |-----------|-----------|--------|
//! | `__alloc` | `malloc` | `<stdlib.h>` |
//! | `__realloc` | `realloc` | `<stdlib.h>` |
//! | `__free` | `free` | `<stdlib.h>` |
//! | `__memcpy` | `memcpy` | `<string.h>` |
//! | `__memset` | `memset` | `<string.h>` |
//! | `__memcmp` | `memcmp` | `<string.h>` |
//! | `__print_str` | `write(1, ptr, len)` | `<unistd.h>` |
//! | `__eprint_str` | `write(2, ptr, len)` | `<unistd.h>` |
//! | `__exit` | `_exit` | `<unistd.h>` |
//! | `__abort` | `abort` | `<stdlib.h>` |
//! | `__getenv` | `getenv` | `<stdlib.h>` |
//! | `__clock_ns` | `clock_gettime` | `<time.h>` |
//! | `__str_to_float` | `strtod` | `<stdlib.h>` |
//! | `__float_to_string` | `snprintf` | `<stdio.h>` |
//! | `__sin`, `__cos`, etc. | `sin`, `cos`, etc. | `<math.h>` (libm) |
//! | `__pow`, `__exp`, `__log` | `pow`, `exp`, `log` | `<math.h>` (libm) |
//!
//! ## Special Cases
//!
//! - **`__argc`/`__argv`**: Passed from `main(argc, argv)` or captured to globals
//! - **`__breakpoint`**: Emitted as inline assembly (`int3` on x86, `brk` on ARM)
//!
//! ## Linking
//!
//! AOT executables link with:
//! ```text
//! cc program.o -o program -lc -lm
//! ```
//!
//! The `LinkOptions` builder supports this:
//! ```ignore
//! LinkOptions::new().library("c").library("m")
//! ```

use cranelift_codegen::Context as ClifContext;
use cranelift_codegen::ir::Function;
use cranelift_frontend::FunctionBuilderContext;
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use super::error::CodegenError;
use super::target::TargetConfig;
use super::types::TypeMapper;

/// AOT compilation context for generating object files.
///
/// Similar to `CodegenContext` but targets object file output instead of JIT.
pub struct AotContext {
    /// The object module that holds compiled code.
    module: ObjectModule,

    /// Cranelift compilation context (reusable between functions).
    ctx: ClifContext,

    /// Function builder context (reusable between functions).
    func_ctx: FunctionBuilderContext,

    /// The target configuration.
    target: TargetConfig,
}

impl AotContext {
    /// Create a new AOT context for the native host target.
    pub fn new() -> Result<Self, CodegenError> {
        Self::with_target(TargetConfig::native_aot()?)
    }

    /// Create a new AOT context for a specific target.
    pub fn with_target(target: TargetConfig) -> Result<Self, CodegenError> {
        let builder = ObjectBuilder::new(
            target.isa().clone(),
            "spl_module",
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

        let module = ObjectModule::new(builder);
        let ctx = module.make_context();
        let func_ctx = FunctionBuilderContext::new();

        Ok(Self {
            module,
            ctx,
            func_ctx,
            target,
        })
    }

    /// Create a type mapper for the current target.
    pub fn type_mapper(&self) -> TypeMapper {
        TypeMapper::new(self.module.isa().pointer_type())
    }

    /// Declare a function in the module.
    ///
    /// Returns the function ID that can be used to define the function.
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
    pub fn compilation_context(&mut self) -> &mut ClifContext {
        &mut self.ctx
    }

    /// Get a mutable reference to the function builder context.
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
    pub fn builder_context(&mut self) -> (&mut Function, &mut FunctionBuilderContext) {
        (&mut self.ctx.func, &mut self.func_ctx)
    }

    /// Get mutable references to the function, function builder context, and module.
    pub fn builder_context_with_module(
        &mut self,
    ) -> (
        &mut Function,
        &mut FunctionBuilderContext,
        &mut ObjectModule,
    ) {
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

    /// Finish compilation and return the object file bytes.
    ///
    /// Consumes the context and returns the raw object file data.
    pub fn finish(self) -> Vec<u8> {
        let product = self.module.finish();
        product.emit().expect("failed to emit object file")
    }

    /// Get a reference to the underlying module.
    pub fn module(&self) -> &ObjectModule {
        &self.module
    }

    /// Get the target configuration.
    pub fn target(&self) -> &TargetConfig {
        &self.target
    }

    /// Create a new signature with the target's default calling convention.
    pub fn new_signature(&self) -> cranelift_codegen::ir::Signature {
        self.module.make_signature()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;
    use cranelift_codegen::ir::{AbiParam, InstBuilder};
    use cranelift_frontend::FunctionBuilder;
    use object::{Object, ObjectSymbol};

    // =========================================================================
    // Phase 1: AOT Context Infrastructure
    // =========================================================================

    #[test]
    fn test_aot_context_creation_native_target() {
        let ctx = AotContext::new();
        assert!(ctx.is_ok(), "failed to create AOT context: {:?}", ctx.err());
    }

    #[test]
    fn test_aot_context_creation_cross_target() {
        // Create for aarch64-unknown-linux-gnu
        let triple: target_lexicon::Triple = "aarch64-unknown-linux-gnu".parse().unwrap();
        let target = TargetConfig::for_aot(triple);
        assert!(target.is_ok());

        let ctx = AotContext::with_target(target.unwrap());
        assert!(
            ctx.is_ok(),
            "failed to create cross AOT context: {:?}",
            ctx.err()
        );
    }

    #[test]
    fn test_declare_function_in_aot_context() {
        let mut ctx = AotContext::new().unwrap();
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
    fn test_aot_type_mapper() {
        let ctx = AotContext::new().unwrap();
        let mapper = ctx.type_mapper();
        let ptr_type = mapper.pointer_type();
        assert!(ptr_type == types::I32 || ptr_type == types::I64);
    }

    // =========================================================================
    // Phase 2: Object File Generation
    // =========================================================================

    #[test]
    fn test_emit_object_bytes_simple_function() {
        let mut ctx = AotContext::new().unwrap();

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

        ctx.define_function(func_id).unwrap();

        // Emit object file
        let object_bytes = ctx.finish();
        assert!(!object_bytes.is_empty(), "object file should not be empty");
    }

    #[test]
    fn test_object_file_has_expected_symbol() {
        let mut ctx = AotContext::new().unwrap();

        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let func_id = ctx.declare_function("my_exported_fn", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry_block = builder.create_block();
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let val = builder.ins().iconst(types::I32, 0);
            builder.ins().return_(&[val]);

            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        let object_bytes = ctx.finish();

        // Parse the object file and check for the symbol
        let obj = object::File::parse(&*object_bytes).expect("failed to parse object file");

        let symbol_names: Vec<_> = obj.symbols().filter_map(|s| s.name().ok()).collect();

        // Symbol might have platform-specific prefix (e.g., "_" on macOS)
        let has_symbol = symbol_names
            .iter()
            .any(|name| name.contains("my_exported_fn"));
        assert!(
            has_symbol,
            "expected symbol 'my_exported_fn' in object file, found: {:?}",
            symbol_names
        );
    }

    #[test]
    fn test_object_file_is_valid_format() {
        let mut ctx = AotContext::new().unwrap();

        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let func_id = ctx.declare_function("test_fn", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry_block = builder.create_block();
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let val = builder.ins().iconst(types::I32, 123);
            builder.ins().return_(&[val]);

            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        let object_bytes = ctx.finish();

        // Verify it's a valid object file
        let obj = object::File::parse(&*object_bytes);
        assert!(
            obj.is_ok(),
            "object file should be parseable: {:?}",
            obj.err()
        );

        let obj = obj.unwrap();

        // Check it's the right format for the host platform
        #[cfg(target_os = "macos")]
        assert!(
            matches!(obj.format(), object::BinaryFormat::MachO),
            "expected Mach-O format on macOS"
        );

        #[cfg(target_os = "linux")]
        assert!(
            matches!(obj.format(), object::BinaryFormat::Elf),
            "expected ELF format on Linux"
        );
    }

    #[test]
    fn test_multiple_functions_in_object() {
        let mut ctx = AotContext::new().unwrap();

        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        // Declare and define two functions
        let func1_id = ctx.declare_function("func_one", &sig.clone()).unwrap();
        let func2_id = ctx.declare_function("func_two", &sig.clone()).unwrap();

        // Define func_one
        ctx.compilation_context().func.signature = sig.clone();
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

        // Define func_two
        ctx.compilation_context().func.signature = sig;
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

        let object_bytes = ctx.finish();

        // Parse and verify both symbols exist
        let obj = object::File::parse(&*object_bytes).expect("failed to parse object file");
        let symbol_names: Vec<_> = obj.symbols().filter_map(|s| s.name().ok()).collect();

        let has_func_one = symbol_names.iter().any(|name| name.contains("func_one"));
        let has_func_two = symbol_names.iter().any(|name| name.contains("func_two"));

        assert!(has_func_one, "expected 'func_one' symbol");
        assert!(has_func_two, "expected 'func_two' symbol");
    }

    #[test]
    fn test_function_with_parameters() {
        let mut ctx = AotContext::new().unwrap();

        // fn add(a: i32, b: i32) -> i32
        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig.returns.push(AbiParam::new(types::I32));

        let func_id = ctx.declare_function("add", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let a = builder.block_params(entry)[0];
            let b = builder.block_params(entry)[1];
            let sum = builder.ins().iadd(a, b);
            builder.ins().return_(&[sum]);

            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        let object_bytes = ctx.finish();

        // Just verify it produces valid output
        let obj = object::File::parse(&*object_bytes);
        assert!(obj.is_ok());
    }

    #[test]
    fn test_accessor_methods() {
        let ctx = AotContext::new().unwrap();

        // Verify accessors work
        let _ = ctx.module();
        let _ = ctx.target();
        let _ = ctx.new_signature();
    }

    #[test]
    fn test_void_function() {
        let mut ctx = AotContext::new().unwrap();

        // Function with no return value
        let sig = ctx.new_signature();
        let func_id = ctx.declare_function("void_fn", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            builder.ins().return_(&[]);

            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        let object_bytes = ctx.finish();

        let obj = object::File::parse(&*object_bytes);
        assert!(obj.is_ok());
    }

    #[test]
    fn test_i64_function() {
        let mut ctx = AotContext::new().unwrap();

        // Function returning i64
        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I64));

        let func_id = ctx.declare_function("i64_fn", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let val = builder.ins().iconst(types::I64, 0x1_0000_0000i64);
            builder.ins().return_(&[val]);

            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        let object_bytes = ctx.finish();

        let obj = object::File::parse(&*object_bytes);
        assert!(obj.is_ok());
    }

    #[test]
    fn test_f64_function() {
        let mut ctx = AotContext::new().unwrap();

        // Function returning f64
        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::F64));

        let func_id = ctx.declare_function("f64_fn", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let val = builder.ins().f64const(std::f64::consts::PI);
            builder.ins().return_(&[val]);

            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        let object_bytes = ctx.finish();

        let obj = object::File::parse(&*object_bytes);
        assert!(obj.is_ok());
    }

    #[test]
    fn test_declare_same_function_twice() {
        let mut ctx = AotContext::new().unwrap();
        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let id1 = ctx.declare_function("same_name", &sig).unwrap();
        let id2 = ctx.declare_function("same_name", &sig).unwrap();

        // Declaring same name and signature returns same ID
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_many_functions() {
        let mut ctx = AotContext::new().unwrap();

        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        // Declare and define 10 functions
        let mut func_ids = Vec::new();
        for i in 0..10 {
            let id = ctx
                .declare_function(&format!("fn_{}", i), &sig.clone())
                .unwrap();
            func_ids.push(id);
        }

        for (i, func_id) in func_ids.iter().enumerate() {
            ctx.compilation_context().func.signature = sig.clone();
            {
                let (func, func_ctx) = ctx.builder_context();
                let mut builder = FunctionBuilder::new(func, func_ctx);
                let entry = builder.create_block();
                builder.switch_to_block(entry);
                builder.seal_block(entry);
                let val = builder.ins().iconst(types::I32, i as i64);
                builder.ins().return_(&[val]);
                builder.finalize();
            }
            ctx.define_function(*func_id).unwrap();
        }

        let object_bytes = ctx.finish();

        let obj = object::File::parse(&*object_bytes).expect("parse failed");
        let symbol_names: Vec<_> = obj.symbols().filter_map(|s| s.name().ok()).collect();

        // Verify all 10 functions exist
        for i in 0..10 {
            assert!(
                symbol_names
                    .iter()
                    .any(|n| n.contains(&format!("fn_{}", i))),
                "missing fn_{} in {:?}",
                i,
                symbol_names
            );
        }
    }

    #[test]
    fn test_func_mut_and_func_accessors() {
        let mut ctx = AotContext::new().unwrap();
        let mut sig = ctx.new_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let _ = ctx.declare_function("test", &sig).unwrap();
        ctx.compilation_context().func.signature = sig;

        // Test func() accessor
        let func_ref = ctx.func();
        assert!(func_ref.signature.returns.len() == 1);

        // Test func_mut() accessor
        let func_mut_ref = ctx.func_mut();
        func_mut_ref
            .signature
            .params
            .push(AbiParam::new(types::I32));
        assert_eq!(ctx.func().signature.params.len(), 1);
    }
}
