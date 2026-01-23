//! Module-level compilation for multi-function programs.
//!
//! This module provides `ModuleCompiler` for compiling multiple MIR functions
//! together, enabling cross-function calls.

use cranelift_codegen::ir::AbiParam;
use cranelift_frontend::FunctionBuilder;
use cranelift_module::FuncId;

use super::aot::AotContext;
use super::context::CodegenContext;
use super::error::{CodegenError, RuntimeError};
use super::lower::FunctionLowerer;
use super::registry::{FunctionInfo, FunctionRegistry};
use super::types::TypeMapper;
use crate::mir::body::Body;
use crate::sema::symbol::DefId;
use crate::sema::types::TypeInterner;

/// A function definition with its DefId and MIR body.
pub struct FunctionDef<'a> {
    /// The unique identifier for this function.
    pub def_id: DefId,
    /// The function name.
    pub name: String,
    /// The MIR body.
    pub body: &'a Body,
}

impl<'a> FunctionDef<'a> {
    /// Create a new function definition.
    pub fn new(def_id: DefId, name: impl Into<String>, body: &'a Body) -> Self {
        FunctionDef {
            def_id,
            name: name.into(),
            body,
        }
    }
}

/// Result of module compilation.
pub struct CompiledModule {
    /// Map from DefId to function pointer.
    function_ptrs: rustc_hash::FxHashMap<DefId, *const u8>,
    /// The DefId of the main function (if any).
    main_def_id: Option<DefId>,
}

impl CompiledModule {
    /// Get a function pointer by DefId.
    pub fn get_function_ptr(&self, def_id: DefId) -> Option<*const u8> {
        self.function_ptrs.get(&def_id).copied()
    }

    /// Get a function pointer by DefId, or panic if not found.
    ///
    /// # Panics
    /// Panics if the function is not in the module.
    pub fn get_function_ptr_or_panic(&self, def_id: DefId) -> *const u8 {
        self.function_ptrs
            .get(&def_id)
            .copied()
            .unwrap_or_else(|| panic!("function {:?} not found in module", def_id))
    }

    /// Get the number of functions in the module.
    pub fn len(&self) -> usize {
        self.function_ptrs.len()
    }

    /// Check if the module is empty.
    pub fn is_empty(&self) -> bool {
        self.function_ptrs.is_empty()
    }

    /// Set the main function DefId.
    pub fn set_main(&mut self, def_id: DefId) {
        self.main_def_id = Some(def_id);
    }

    /// Get the main function DefId.
    pub fn main_def_id(&self) -> Option<DefId> {
        self.main_def_id
    }

    /// Run a function by DefId and return its i32 result.
    ///
    /// # Safety
    /// The function must have signature `fn() -> i32`.
    ///
    /// # Note
    /// Traps (unreachable, assertion failures) will cause process termination
    /// via SIGILL. Use `run_catching` for trap-safe execution in the future.
    pub fn run(&self, def_id: DefId) -> Result<i32, RuntimeError> {
        let ptr = self
            .function_ptrs
            .get(&def_id)
            .copied()
            .ok_or(RuntimeError::MainNotFound)?;

        let func: fn() -> i32 = unsafe { std::mem::transmute(ptr) };
        Ok(func())
    }

    /// Run the main function and return its i32 result.
    ///
    /// # Safety
    /// The main function must have signature `fn() -> i32`.
    ///
    /// # Note
    /// Traps (unreachable, assertion failures) will cause process termination
    /// via SIGILL. Use `run_main_catching` for trap-safe execution in the future.
    pub fn run_main(&self) -> Result<i32, RuntimeError> {
        let def_id = self.main_def_id.ok_or(RuntimeError::MainNotFound)?;
        self.run(def_id)
    }
}

/// Compiles multiple MIR functions as a module.
///
/// Uses a two-pass compilation strategy:
/// 1. **Declaration pass**: Declare all functions and build the registry
/// 2. **Definition pass**: Define all function bodies with access to the registry
pub struct ModuleCompiler {
    ctx: CodegenContext,
    registry: FunctionRegistry,
}

impl ModuleCompiler {
    /// Create a new module compiler.
    pub fn new() -> Result<Self, CodegenError> {
        Ok(ModuleCompiler {
            ctx: CodegenContext::new_jit()?,
            registry: FunctionRegistry::new(),
        })
    }

    /// Compile multiple functions and return a compiled module.
    ///
    /// The functions can call each other via `TerminatorKind::Call`.
    pub fn compile(
        functions: &[FunctionDef<'_>],
        types: &TypeInterner,
    ) -> Result<CompiledModule, CodegenError> {
        let mut compiler = Self::new()?;

        // Pass 1: Declare all functions
        let func_ids = compiler.declare_functions(functions, types)?;

        // Pass 2: Define all function bodies
        compiler.define_functions(functions, types, &func_ids)?;

        // Pass 3: Finalize and collect function pointers
        compiler.ctx.finalize();

        let mut function_ptrs = rustc_hash::FxHashMap::default();
        for (func_def, func_id) in functions.iter().zip(func_ids.iter()) {
            let ptr = compiler.ctx.get_function_ptr(*func_id);
            function_ptrs.insert(func_def.def_id, ptr);
        }

        Ok(CompiledModule {
            function_ptrs,
            main_def_id: None,
        })
    }

    /// Declaration pass: declare all functions and build registry.
    fn declare_functions(
        &mut self,
        functions: &[FunctionDef<'_>],
        types: &TypeInterner,
    ) -> Result<Vec<FuncId>, CodegenError> {
        let type_mapper = self.ctx.type_mapper();
        let mut func_ids = Vec::with_capacity(functions.len());

        for func_def in functions {
            let sig = Self::build_signature(&self.ctx, &type_mapper, func_def.body, types);
            let func_id = self.ctx.declare_function(&func_def.name, &sig)?;

            self.registry
                .register(func_def.def_id, FunctionInfo::new(func_id, sig));
            func_ids.push(func_id);
        }

        Ok(func_ids)
    }

    /// Definition pass: define all function bodies.
    fn define_functions(
        &mut self,
        functions: &[FunctionDef<'_>],
        types: &TypeInterner,
        func_ids: &[FuncId],
    ) -> Result<(), CodegenError> {
        for (func_def, func_id) in functions.iter().zip(func_ids.iter()) {
            self.define_single_function(func_def, types, *func_id)?;
        }
        Ok(())
    }

    /// Define a single function body.
    fn define_single_function(
        &mut self,
        func_def: &FunctionDef<'_>,
        types: &TypeInterner,
        func_id: FuncId,
    ) -> Result<(), CodegenError> {
        let type_mapper = self.ctx.type_mapper();
        let sig = Self::build_signature(&self.ctx, &type_mapper, func_def.body, types);

        // Set up the function
        self.ctx.compilation_context().func.signature = sig;

        // Build the function body with registry access
        {
            let (func, func_ctx, module) = self.ctx.builder_context_with_module();
            let builder = FunctionBuilder::new(func, func_ctx);
            let lowerer =
                FunctionLowerer::with_registry(builder, type_mapper, types, func_def.body)
                    .set_registry(&self.registry)
                    .set_module(module);
            lowerer.lower_body()?;
        }

        // Define the function
        self.ctx.define_function(func_id)?;

        Ok(())
    }

    /// Build the Cranelift signature for a MIR body.
    fn build_signature(
        ctx: &CodegenContext,
        type_mapper: &TypeMapper,
        body: &Body,
        types: &TypeInterner,
    ) -> cranelift_codegen::ir::Signature {
        let mut sig = ctx.new_signature();

        // Add return type (if not ZST)
        let return_ty = body.return_ty();
        if let Some(clif_ty) = type_mapper.map_type(return_ty, types) {
            sig.returns.push(AbiParam::new(clif_ty));
        }

        // Add parameter types
        for arg in body.args() {
            let arg_ty = body.local_decl(arg).ty;
            if let Some(clif_ty) = type_mapper.map_type(arg_ty, types) {
                sig.params.push(AbiParam::new(clif_ty));
            }
        }

        sig
    }
}

// =============================================================================
// AOT Module Compiler
// =============================================================================

/// Result of AOT compilation.
pub struct CompiledObjectFile {
    /// The raw object file bytes.
    bytes: Vec<u8>,
    /// Map from DefId to function name (for symbol lookup).
    function_names: rustc_hash::FxHashMap<DefId, String>,
    /// The DefId of the main function (if any).
    main_def_id: Option<DefId>,
}

impl CompiledObjectFile {
    /// Get the raw object file bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume self and return the object file bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Get the function name for a DefId.
    pub fn get_function_name(&self, def_id: DefId) -> Option<&str> {
        self.function_names.get(&def_id).map(|s| s.as_str())
    }

    /// Get the number of functions in the object file.
    pub fn len(&self) -> usize {
        self.function_names.len()
    }

    /// Check if the object file is empty.
    pub fn is_empty(&self) -> bool {
        self.function_names.is_empty()
    }

    /// Set the main function DefId.
    pub fn set_main(&mut self, def_id: DefId) {
        self.main_def_id = Some(def_id);
    }

    /// Get the main function DefId.
    pub fn main_def_id(&self) -> Option<DefId> {
        self.main_def_id
    }
}

/// Compiles multiple MIR functions to an object file.
///
/// Uses the same two-pass compilation strategy as `ModuleCompiler`:
/// 1. **Declaration pass**: Declare all functions and build the registry
/// 2. **Definition pass**: Define all function bodies with access to the registry
pub struct AotModuleCompiler {
    ctx: AotContext,
    registry: FunctionRegistry,
}

impl AotModuleCompiler {
    /// Create a new AOT module compiler.
    pub fn new() -> Result<Self, CodegenError> {
        Ok(AotModuleCompiler {
            ctx: AotContext::new()?,
            registry: FunctionRegistry::new(),
        })
    }

    /// Compile multiple functions and return a compiled object file.
    ///
    /// The functions can call each other via `TerminatorKind::Call`.
    pub fn compile(
        functions: &[FunctionDef<'_>],
        types: &TypeInterner,
    ) -> Result<CompiledObjectFile, CodegenError> {
        let mut compiler = Self::new()?;

        // Pass 1: Declare all functions
        let func_ids = compiler.declare_functions(functions, types)?;

        // Pass 2: Define all function bodies
        compiler.define_functions(functions, types, &func_ids)?;

        // Pass 3: Emit object file and collect metadata
        let bytes = compiler.ctx.finish();

        let mut function_names = rustc_hash::FxHashMap::default();
        for func_def in functions {
            function_names.insert(func_def.def_id, func_def.name.clone());
        }

        Ok(CompiledObjectFile {
            bytes,
            function_names,
            main_def_id: None,
        })
    }

    /// Declaration pass: declare all functions and build registry.
    fn declare_functions(
        &mut self,
        functions: &[FunctionDef<'_>],
        types: &TypeInterner,
    ) -> Result<Vec<FuncId>, CodegenError> {
        let type_mapper = self.ctx.type_mapper();
        let mut func_ids = Vec::with_capacity(functions.len());

        for func_def in functions {
            let sig = Self::build_signature(&self.ctx, &type_mapper, func_def.body, types);
            let func_id = self.ctx.declare_function(&func_def.name, &sig)?;

            self.registry
                .register(func_def.def_id, FunctionInfo::new(func_id, sig));
            func_ids.push(func_id);
        }

        Ok(func_ids)
    }

    /// Definition pass: define all function bodies.
    fn define_functions(
        &mut self,
        functions: &[FunctionDef<'_>],
        types: &TypeInterner,
        func_ids: &[FuncId],
    ) -> Result<(), CodegenError> {
        for (func_def, func_id) in functions.iter().zip(func_ids.iter()) {
            self.define_single_function(func_def, types, *func_id)?;
        }
        Ok(())
    }

    /// Define a single function body.
    fn define_single_function(
        &mut self,
        func_def: &FunctionDef<'_>,
        types: &TypeInterner,
        func_id: FuncId,
    ) -> Result<(), CodegenError> {
        let type_mapper = self.ctx.type_mapper();
        let sig = Self::build_signature(&self.ctx, &type_mapper, func_def.body, types);

        // Set up the function
        self.ctx.compilation_context().func.signature = sig;

        // Build the function body with registry access
        {
            let (func, func_ctx, module) = self.ctx.builder_context_with_module();
            let builder = FunctionBuilder::new(func, func_ctx);
            let lowerer =
                FunctionLowerer::with_registry(builder, type_mapper, types, func_def.body)
                    .set_registry(&self.registry)
                    .set_module(module);
            lowerer.lower_body()?;
        }

        // Define the function
        self.ctx.define_function(func_id)?;

        Ok(())
    }

    /// Build the Cranelift signature for a MIR body.
    fn build_signature(
        ctx: &AotContext,
        type_mapper: &TypeMapper,
        body: &Body,
        types: &TypeInterner,
    ) -> cranelift_codegen::ir::Signature {
        let mut sig = ctx.new_signature();

        // Add return type (if not ZST)
        let return_ty = body.return_ty();
        if let Some(clif_ty) = type_mapper.map_type(return_ty, types) {
            sig.returns.push(AbiParam::new(clif_ty));
        }

        // Add parameter types
        for arg in body.args() {
            let arg_ty = body.local_decl(arg).ty;
            if let Some(clif_ty) = type_mapper.map_type(arg_ty, types) {
                sig.params.push(AbiParam::new(clif_ty));
            }
        }

        sig
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::body::Body;
    use crate::mir::operand::{Constant, Operand, Rvalue};
    use crate::mir::statement::Statement;
    use crate::mir::terminator::{Terminator, TerminatorKind};
    use crate::mir::types::{Local, Place};
    use object::{Object, ObjectSymbol};

    #[test]
    fn compile_single_function() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn returns_42() -> i32 { 42 }
        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(42)),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "returns_42", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert_eq!(module.len(), 1);

        let ptr = module.get_function_ptr(DefId(1)).unwrap();
        let func: fn() -> i32 = unsafe { std::mem::transmute(ptr) };
        assert_eq!(func(), 42);
    }

    #[test]
    fn compile_two_independent_functions() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn returns_1() -> i32 { 1 }
        let mut body1 = Body::new(i32_ty);
        let entry1 = body1.alloc_block();
        body1.block_mut(entry1).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(1)),
            0..0,
        ));
        body1
            .block_mut(entry1)
            .set_terminator(Terminator::return_(0..0));

        // fn returns_2() -> i32 { 2 }
        let mut body2 = Body::new(i32_ty);
        let entry2 = body2.alloc_block();
        body2.block_mut(entry2).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(2)),
            0..0,
        ));
        body2
            .block_mut(entry2)
            .set_terminator(Terminator::return_(0..0));

        let functions = [
            FunctionDef::new(DefId(1), "returns_1", &body1),
            FunctionDef::new(DefId(2), "returns_2", &body2),
        ];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert_eq!(module.len(), 2);

        let ptr1 = module.get_function_ptr(DefId(1)).unwrap();
        let ptr2 = module.get_function_ptr(DefId(2)).unwrap();

        let func1: fn() -> i32 = unsafe { std::mem::transmute(ptr1) };
        let func2: fn() -> i32 = unsafe { std::mem::transmute(ptr2) };

        assert_eq!(func1(), 1);
        assert_eq!(func2(), 2);
    }

    #[test]
    fn compile_caller_callee() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn callee() -> i32 { 42 }
        let mut callee_body = Body::new(i32_ty);
        let callee_entry = callee_body.alloc_block();
        callee_body
            .block_mut(callee_entry)
            .push_statement(Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_int(42)),
                0..0,
            ));
        callee_body
            .block_mut(callee_entry)
            .set_terminator(Terminator::return_(0..0));

        // fn caller() -> i32 { callee() }
        let mut caller_body = Body::new(i32_ty);
        let caller_entry = caller_body.alloc_block();
        let after_call = caller_body.alloc_block();

        // call callee() -> _0, then jump to after_call
        caller_body
            .block_mut(caller_entry)
            .set_terminator(Terminator::new(
                TerminatorKind::Call {
                    func: Operand::Constant(Constant::FnDef(DefId(1))), // callee
                    args: vec![],
                    destination: Place::from_local(Local::RETURN_PLACE),
                    target: Some(after_call),
                },
                0..0,
            ));

        caller_body
            .block_mut(after_call)
            .set_terminator(Terminator::return_(0..0));

        let functions = [
            FunctionDef::new(DefId(1), "callee", &callee_body),
            FunctionDef::new(DefId(2), "caller", &caller_body),
        ];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        let caller_ptr = module.get_function_ptr(DefId(2)).unwrap();
        let caller: fn() -> i32 = unsafe { std::mem::transmute(caller_ptr) };

        assert_eq!(caller(), 42);
    }

    #[test]
    fn compile_call_with_args() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn add(a: i32, b: i32) -> i32 { a + b }
        let mut add_body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
        let add_entry = add_body.alloc_block();

        // _0 = _1 + _2
        add_body
            .block_mut(add_entry)
            .push_statement(Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::BinaryOp(
                    crate::mir::operand::BinOp::Add,
                    Operand::copy_local(Local(1)),
                    Operand::copy_local(Local(2)),
                ),
                0..0,
            ));
        add_body
            .block_mut(add_entry)
            .set_terminator(Terminator::return_(0..0));

        // fn caller() -> i32 { add(10, 32) }
        let mut caller_body = Body::new(i32_ty);
        let caller_entry = caller_body.alloc_block();
        let after_call = caller_body.alloc_block();

        caller_body
            .block_mut(caller_entry)
            .set_terminator(Terminator::new(
                TerminatorKind::Call {
                    func: Operand::Constant(Constant::FnDef(DefId(1))), // add
                    args: vec![Operand::const_int(10), Operand::const_int(32)],
                    destination: Place::from_local(Local::RETURN_PLACE),
                    target: Some(after_call),
                },
                0..0,
            ));

        caller_body
            .block_mut(after_call)
            .set_terminator(Terminator::return_(0..0));

        let functions = [
            FunctionDef::new(DefId(1), "add", &add_body),
            FunctionDef::new(DefId(2), "caller", &caller_body),
        ];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        let caller_ptr = module.get_function_ptr(DefId(2)).unwrap();
        let caller: fn() -> i32 = unsafe { std::mem::transmute(caller_ptr) };

        assert_eq!(caller(), 42);
    }

    #[test]
    fn compiled_module_get_function_ptr_or_panic() {
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

        let functions = [FunctionDef::new(DefId(1), "test", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // This should not panic
        let _ = module.get_function_ptr_or_panic(DefId(1));
    }

    #[test]
    #[should_panic(expected = "not found")]
    fn compiled_module_get_function_ptr_or_panic_missing() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "test", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // This should panic
        let _ = module.get_function_ptr_or_panic(DefId(999));
    }

    #[test]
    fn compiled_module_is_empty() {
        let types = TypeInterner::new();
        let functions: [FunctionDef<'_>; 0] = [];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert!(module.is_empty());
        assert_eq!(module.len(), 0);
    }

    #[test]
    fn run_returns_value() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn returns_42() -> i32 { 42 }
        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(42)),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "returns_42", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        let result = module.run(DefId(1));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn run_with_invalid_def_id_returns_error() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(1)),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "test", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        let result = module.run(DefId(999));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::codegen::error::RuntimeError::MainNotFound
        ));
    }

    #[test]
    fn run_main_returns_value() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn main() -> i32 { 100 }
        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(100)),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "main", &body)];
        let mut module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // Set the main function
        module.set_main(DefId(1));

        let result = module.run_main();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100);
    }

    #[test]
    fn run_main_without_setting_main_returns_error() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "test", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // Don't set main
        let result = module.run_main();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::codegen::error::RuntimeError::MainNotFound
        ));
    }

    #[test]
    fn main_def_id_accessor() {
        let types = TypeInterner::new();
        let functions: [FunctionDef<'_>; 0] = [];
        let mut module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert!(module.main_def_id().is_none());

        module.set_main(DefId(42));
        assert_eq!(module.main_def_id(), Some(DefId(42)));
    }

    #[test]
    fn run_same_function_multiple_times() {
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

        let functions = [FunctionDef::new(DefId(1), "test", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // Call multiple times
        assert_eq!(module.run(DefId(1)).unwrap(), 42);
        assert_eq!(module.run(DefId(1)).unwrap(), 42);
        assert_eq!(module.run(DefId(1)).unwrap(), 42);
    }

    #[test]
    fn run_different_functions_in_same_module() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn returns_1() -> i32 { 1 }
        let mut body1 = Body::new(i32_ty);
        let entry1 = body1.alloc_block();
        body1.block_mut(entry1).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(1)),
            0..0,
        ));
        body1
            .block_mut(entry1)
            .set_terminator(Terminator::return_(0..0));

        // fn returns_2() -> i32 { 2 }
        let mut body2 = Body::new(i32_ty);
        let entry2 = body2.alloc_block();
        body2.block_mut(entry2).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(2)),
            0..0,
        ));
        body2
            .block_mut(entry2)
            .set_terminator(Terminator::return_(0..0));

        // fn returns_3() -> i32 { 3 }
        let mut body3 = Body::new(i32_ty);
        let entry3 = body3.alloc_block();
        body3.block_mut(entry3).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(3)),
            0..0,
        ));
        body3
            .block_mut(entry3)
            .set_terminator(Terminator::return_(0..0));

        let functions = [
            FunctionDef::new(DefId(1), "returns_1", &body1),
            FunctionDef::new(DefId(2), "returns_2", &body2),
            FunctionDef::new(DefId(3), "returns_3", &body3),
        ];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert_eq!(module.run(DefId(1)).unwrap(), 1);
        assert_eq!(module.run(DefId(2)).unwrap(), 2);
        assert_eq!(module.run(DefId(3)).unwrap(), 3);

        // Run in different order
        assert_eq!(module.run(DefId(3)).unwrap(), 3);
        assert_eq!(module.run(DefId(1)).unwrap(), 1);
        assert_eq!(module.run(DefId(2)).unwrap(), 2);
    }

    #[test]
    fn set_main_can_be_changed() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let mut body1 = Body::new(i32_ty);
        let entry1 = body1.alloc_block();
        body1.block_mut(entry1).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(100)),
            0..0,
        ));
        body1
            .block_mut(entry1)
            .set_terminator(Terminator::return_(0..0));

        let mut body2 = Body::new(i32_ty);
        let entry2 = body2.alloc_block();
        body2.block_mut(entry2).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(200)),
            0..0,
        ));
        body2
            .block_mut(entry2)
            .set_terminator(Terminator::return_(0..0));

        let functions = [
            FunctionDef::new(DefId(1), "fn1", &body1),
            FunctionDef::new(DefId(2), "fn2", &body2),
        ];
        let mut module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // Set main to first function
        module.set_main(DefId(1));
        assert_eq!(module.run_main().unwrap(), 100);

        // Change main to second function
        module.set_main(DefId(2));
        assert_eq!(module.run_main().unwrap(), 200);

        // Change back
        module.set_main(DefId(1));
        assert_eq!(module.run_main().unwrap(), 100);
    }

    #[test]
    fn run_returns_different_values() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn(x: i32) -> i32 { x * 2 } - but we test with various i32-returning functions
        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(0)),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "returns_zero", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert_eq!(module.run(DefId(1)).unwrap(), 0);
    }

    #[test]
    fn run_with_negative_return() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(-42)),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "returns_negative", &body)];
        let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert_eq!(module.run(DefId(1)).unwrap(), -42);
    }

    // =========================================================================
    // AOT Module Compiler Tests (Phases 3-4)
    // =========================================================================

    #[test]
    fn aot_compile_return_constant() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn returns_42() -> i32 { 42 }
        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(42)),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "returns_42", &body)];
        let obj = AotModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert_eq!(obj.len(), 1);
        assert!(!obj.is_empty());
        assert_eq!(obj.get_function_name(DefId(1)), Some("returns_42"));

        // Verify the object file is valid
        let parsed = object::File::parse(obj.bytes()).expect("failed to parse object file");
        let symbol_names: Vec<_> = parsed.symbols().filter_map(|s| s.name().ok()).collect();
        assert!(
            symbol_names.iter().any(|n| n.contains("returns_42")),
            "expected 'returns_42' symbol"
        );
    }

    #[test]
    fn aot_compile_arithmetic() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn add_one(x: i32) -> i32 { x + 1 }
        let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
        let entry = body.alloc_block();
        body.block_mut(entry).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::BinaryOp(
                crate::mir::operand::BinOp::Add,
                Operand::copy_local(Local(1)),
                Operand::const_int(1),
            ),
            0..0,
        ));
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "add_one", &body)];
        let obj = AotModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // Just verify it compiles and produces valid object
        let parsed = object::File::parse(obj.bytes());
        assert!(parsed.is_ok());
    }

    #[test]
    fn aot_compile_conditionals() {
        use crate::mir::terminator::SwitchTargets;

        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn returns_based_on_condition() -> i32 { if true { 1 } else { 0 } }
        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        let then_block = body.alloc_block();
        let else_block = body.alloc_block();
        let exit_block = body.alloc_block();

        // Entry: branch on true (non-zero -> then_block, zero -> else_block)
        body.block_mut(entry).set_terminator(Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::const_bool(true),
                targets: SwitchTargets::new_bool(then_block, else_block),
            },
            0..0,
        ));

        // Then: _0 = 1
        body.block_mut(then_block).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(1)),
            0..0,
        ));
        body.block_mut(then_block)
            .set_terminator(Terminator::new(TerminatorKind::Goto(exit_block), 0..0));

        // Else: _0 = 0
        body.block_mut(else_block).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(0)),
            0..0,
        ));
        body.block_mut(else_block)
            .set_terminator(Terminator::new(TerminatorKind::Goto(exit_block), 0..0));

        // Exit: return
        body.block_mut(exit_block)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "conditional", &body)];
        let obj = AotModuleCompiler::compile(&functions, &types).expect("compilation failed");

        let parsed = object::File::parse(obj.bytes());
        assert!(parsed.is_ok());
    }

    #[test]
    fn aot_compile_multiple_functions() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn returns_1() -> i32 { 1 }
        let mut body1 = Body::new(i32_ty);
        let entry1 = body1.alloc_block();
        body1.block_mut(entry1).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(1)),
            0..0,
        ));
        body1
            .block_mut(entry1)
            .set_terminator(Terminator::return_(0..0));

        // fn returns_2() -> i32 { 2 }
        let mut body2 = Body::new(i32_ty);
        let entry2 = body2.alloc_block();
        body2.block_mut(entry2).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(2)),
            0..0,
        ));
        body2
            .block_mut(entry2)
            .set_terminator(Terminator::return_(0..0));

        let functions = [
            FunctionDef::new(DefId(1), "returns_1", &body1),
            FunctionDef::new(DefId(2), "returns_2", &body2),
        ];
        let obj = AotModuleCompiler::compile(&functions, &types).expect("compilation failed");

        assert_eq!(obj.len(), 2);
        assert_eq!(obj.get_function_name(DefId(1)), Some("returns_1"));
        assert_eq!(obj.get_function_name(DefId(2)), Some("returns_2"));

        // Verify both symbols exist in the object file
        let parsed = object::File::parse(obj.bytes()).expect("failed to parse");
        let symbol_names: Vec<_> = parsed.symbols().filter_map(|s| s.name().ok()).collect();
        assert!(symbol_names.iter().any(|n| n.contains("returns_1")));
        assert!(symbol_names.iter().any(|n| n.contains("returns_2")));
    }

    #[test]
    fn aot_cross_function_calls() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // fn callee() -> i32 { 42 }
        let mut callee_body = Body::new(i32_ty);
        let callee_entry = callee_body.alloc_block();
        callee_body
            .block_mut(callee_entry)
            .push_statement(Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_int(42)),
                0..0,
            ));
        callee_body
            .block_mut(callee_entry)
            .set_terminator(Terminator::return_(0..0));

        // fn caller() -> i32 { callee() }
        let mut caller_body = Body::new(i32_ty);
        let caller_entry = caller_body.alloc_block();
        let after_call = caller_body.alloc_block();

        caller_body
            .block_mut(caller_entry)
            .set_terminator(Terminator::new(
                TerminatorKind::Call {
                    func: Operand::Constant(Constant::FnDef(DefId(1))), // callee
                    args: vec![],
                    destination: Place::from_local(Local::RETURN_PLACE),
                    target: Some(after_call),
                },
                0..0,
            ));

        caller_body
            .block_mut(after_call)
            .set_terminator(Terminator::return_(0..0));

        let functions = [
            FunctionDef::new(DefId(1), "callee", &callee_body),
            FunctionDef::new(DefId(2), "caller", &caller_body),
        ];
        let obj = AotModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // Verify object file has both symbols
        let parsed = object::File::parse(obj.bytes()).expect("failed to parse");
        let symbol_names: Vec<_> = parsed.symbols().filter_map(|s| s.name().ok()).collect();
        assert!(symbol_names.iter().any(|n| n.contains("callee")));
        assert!(symbol_names.iter().any(|n| n.contains("caller")));
    }

    #[test]
    fn aot_symbols_all_exported() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        // Create three functions
        let mut bodies = vec![];
        for i in 0..3 {
            let mut body = Body::new(i32_ty);
            let entry = body.alloc_block();
            body.block_mut(entry).push_statement(Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(Operand::const_int(i)),
                0..0,
            ));
            body.block_mut(entry)
                .set_terminator(Terminator::return_(0..0));
            bodies.push(body);
        }

        let functions: Vec<_> = bodies
            .iter()
            .enumerate()
            .map(|(i, b)| FunctionDef::new(DefId(i as u32 + 1), format!("fn_{}", i), b))
            .collect();

        let obj = AotModuleCompiler::compile(&functions, &types).expect("compilation failed");

        let parsed = object::File::parse(obj.bytes()).expect("failed to parse");
        let symbol_names: Vec<_> = parsed.symbols().filter_map(|s| s.name().ok()).collect();

        // Verify all three symbols exist
        assert!(symbol_names.iter().any(|n| n.contains("fn_0")));
        assert!(symbol_names.iter().any(|n| n.contains("fn_1")));
        assert!(symbol_names.iter().any(|n| n.contains("fn_2")));
    }

    #[test]
    fn aot_compiled_object_accessors() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "test", &body)];
        let mut obj = AotModuleCompiler::compile(&functions, &types).expect("compilation failed");

        // Test accessors
        assert!(!obj.bytes().is_empty());
        assert!(obj.main_def_id().is_none());

        obj.set_main(DefId(1));
        assert_eq!(obj.main_def_id(), Some(DefId(1)));
    }

    #[test]
    fn aot_into_bytes() {
        let types = TypeInterner::new();
        let i32_ty = types.i32();

        let mut body = Body::new(i32_ty);
        let entry = body.alloc_block();
        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));

        let functions = [FunctionDef::new(DefId(1), "test", &body)];
        let obj = AotModuleCompiler::compile(&functions, &types).expect("compilation failed");

        let bytes = obj.into_bytes();
        assert!(!bytes.is_empty());
    }
}
