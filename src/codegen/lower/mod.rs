//! MIR to Cranelift IR lowering.
//!
//! This module translates MIR bodies into Cranelift IR for JIT compilation.

mod operand;
mod rvalue;
mod statement;
mod terminator;

#[cfg(test)]
mod tests;

use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{
    AbiParam, Block, GlobalValue, InstBuilder, MemFlags, StackSlotData, StackSlotKind, Value,
};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataDescription, DataId, Linkage, Module};
use rustc_hash::FxHashMap;

use crate::codegen::error::CodegenError;
use crate::codegen::layout::LayoutComputer;
use crate::codegen::registry::FunctionRegistry;
use crate::codegen::{CodegenContext, LocalMap, LocalStorage, TypeMapper};
use crate::mir::body::Body;
use crate::mir::terminator::BasicBlock;
use crate::mir::types::Local;
use crate::sema::types::{TypeId, TypeInterner};

/// Lowers MIR to Cranelift IR.
pub struct FunctionLowerer<'a> {
    /// The function builder for creating Cranelift IR.
    builder: FunctionBuilder<'a>,
    /// Maps MIR locals to Cranelift storage.
    local_map: LocalMap,
    /// Maps SPL types to Cranelift types.
    type_mapper: TypeMapper,
    /// Computes type layouts for memory operations.
    layout: LayoutComputer<'a>,
    /// Maps MIR basic blocks to Cranelift blocks.
    block_map: FxHashMap<BasicBlock, Block>,
    /// Reference to the type interner for type lookups.
    types: &'a TypeInterner,
    /// The MIR body being lowered.
    body: &'a Body,
    /// Optional function registry for multi-function compilation.
    func_registry: Option<&'a FunctionRegistry>,
    /// Optional module for importing functions (works with both JIT and AOT).
    module: Option<&'a mut dyn Module>,
    /// Maps string contents to their data IDs for deduplication.
    string_data: FxHashMap<String, DataId>,
    /// Counter for generating unique string data names.
    string_counter: usize,
}

impl<'a> FunctionLowerer<'a> {
    /// Compile a MIR body to native code via JIT.
    ///
    /// Returns a pointer to the compiled function.
    pub fn compile(
        ctx: &mut CodegenContext,
        body: &Body,
        types: &TypeInterner,
        name: &str,
    ) -> Result<*const u8, CodegenError> {
        // Build signature
        let type_mapper = ctx.type_mapper();
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

        // Declare the function
        let func_id = ctx.declare_function(name, &sig)?;

        // Set up the function
        ctx.compilation_context().func.signature = sig;

        // Build the function body (with module access for string constants)
        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();
            let builder = FunctionBuilder::new(func, func_ctx);
            let lowerer =
                FunctionLowerer::new(builder, type_mapper, types, body).set_module(module);
            lowerer.lower_body()?;
        }

        // Define and finalize
        ctx.define_function(func_id)?;
        ctx.finalize();

        Ok(ctx.get_function_ptr(func_id))
    }

    /// Create a new function lowerer.
    fn new(
        builder: FunctionBuilder<'a>,
        type_mapper: TypeMapper,
        types: &'a TypeInterner,
        body: &'a Body,
    ) -> Self {
        let layout = LayoutComputer::new(types, type_mapper.pointer_type());
        FunctionLowerer {
            builder,
            local_map: LocalMap::new(),
            type_mapper,
            layout,
            block_map: FxHashMap::default(),
            types,
            body,
            func_registry: None,
            module: None,
            string_data: FxHashMap::default(),
            string_counter: 0,
        }
    }

    /// Create a new function lowerer with registry support for multi-function compilation.
    ///
    /// Use `set_registry()` and `set_module()` to configure cross-function calls.
    pub fn with_registry(
        builder: FunctionBuilder<'a>,
        type_mapper: TypeMapper,
        types: &'a TypeInterner,
        body: &'a Body,
    ) -> Self {
        Self::new(builder, type_mapper, types, body)
    }

    /// Set the function registry for resolving function references.
    pub fn set_registry(mut self, registry: &'a FunctionRegistry) -> Self {
        self.func_registry = Some(registry);
        self
    }

    /// Set the module for importing function references (works with both JIT and AOT).
    pub fn set_module(mut self, module: &'a mut dyn Module) -> Self {
        self.module = Some(module);
        self
    }

    /// Lower the entire MIR body to Cranelift IR.
    pub fn lower_body(mut self) -> Result<(), CodegenError> {
        // Create Cranelift blocks for each MIR block
        for i in 0..self.body.num_blocks() {
            let mir_bb = BasicBlock::new(i as u32);
            let clif_block = self.builder.create_block();
            self.block_map.insert(mir_bb, clif_block);
        }

        // Set up the entry block with function parameters
        let entry_block = self.block_map[&BasicBlock::ENTRY];
        self.builder
            .append_block_params_for_function_params(entry_block);
        self.builder.switch_to_block(entry_block);

        // Declare locals as Cranelift variables
        self.declare_locals()?;

        // Initialize arguments from block params
        self.init_arguments(entry_block)?;

        // Seal the entry block (all predecessors known - none for entry)
        self.builder.seal_block(entry_block);

        // Lower each basic block
        for bb_idx in 0..self.body.num_blocks() {
            let mir_bb = BasicBlock::new(bb_idx as u32);
            if bb_idx > 0 {
                let clif_block = self.block_map[&mir_bb];
                self.builder.switch_to_block(clif_block);
            }
            self.lower_block(mir_bb)?;
        }

        // Seal all non-entry blocks (entry was sealed earlier)
        for (&mir_bb, &clif_block) in &self.block_map {
            if mir_bb != BasicBlock::ENTRY {
                self.builder.seal_block(clif_block);
            }
        }

        self.builder.finalize();
        Ok(())
    }

    /// Declare Cranelift variables for all MIR locals.
    fn declare_locals(&mut self) -> Result<(), CodegenError> {
        for i in 0..self.body.num_locals() {
            let local = Local::new(i as u32);
            let decl = self.body.local_decl(local);

            if let Some(clif_ty) = self.type_mapper.map_type(decl.ty, self.types) {
                // Scalar type: use a Cranelift variable (SSA)
                let var = self.local_map.alloc_variable(local);
                self.builder.declare_var(var, clif_ty);
            } else if self.type_mapper.is_zst(decl.ty, self.types) {
                // ZST - no storage needed
                self.local_map.set_zst(local);
            } else {
                // Compound type: allocate a stack slot
                let layout = self.layout.layout_of(decl.ty);
                if layout.size == 0 {
                    // Zero-size layout means ZST
                    self.local_map.set_zst(local);
                } else {
                    let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        layout.size,
                        layout.align.try_into().unwrap_or(0),
                    ));
                    self.local_map.set_stack_slot(local, slot);
                }
            }
        }
        Ok(())
    }

    /// Initialize function arguments from entry block parameters.
    fn init_arguments(&mut self, entry_block: Block) -> Result<(), CodegenError> {
        let block_params = self.builder.block_params(entry_block).to_vec();
        let mut param_idx = 0;

        for arg in self.body.args() {
            let decl = self.body.local_decl(arg);
            if self.type_mapper.map_type(decl.ty, self.types).is_some()
                && let Some(LocalStorage::Variable(var)) = self.local_map.get(arg)
            {
                let val = block_params[param_idx];
                self.builder.def_var(var, val);
                param_idx += 1;
            }
        }
        Ok(())
    }

    /// Lower a single MIR basic block.
    fn lower_block(&mut self, bb: BasicBlock) -> Result<(), CodegenError> {
        let block_data = self.body.block(bb);

        // Lower all statements
        for stmt in &block_data.statements {
            self.lower_statement(stmt)?;
        }

        // Lower the terminator
        if let Some(ref term) = block_data.terminator {
            self.lower_terminator(term)?;
        }

        Ok(())
    }

    /// Get the Cranelift block for a MIR basic block.
    fn get_block(&self, bb: BasicBlock) -> Block {
        self.block_map[&bb]
    }

    /// Get the Cranelift variable for a MIR local.
    fn get_variable(&self, local: Local) -> Option<Variable> {
        match self.local_map.get(local) {
            Some(LocalStorage::Variable(var)) => Some(var),
            _ => None,
        }
    }

    /// Read a value from a local.
    fn use_var(&mut self, local: Local) -> Option<Value> {
        self.get_variable(local)
            .map(|var| self.builder.use_var(var))
    }

    /// Write a value to a local.
    fn def_var(&mut self, local: Local, val: Value) {
        if let Some(var) = self.get_variable(local) {
            self.builder.def_var(var, val);
        }
    }

    /// Get the Cranelift type for a MIR local.
    fn local_type(&self, local: Local) -> Option<types::Type> {
        let decl = self.body.local_decl(local);
        self.type_mapper.map_type(decl.ty, self.types)
    }

    /// Check if a type is a float type.
    fn is_float_type(&self, ty: types::Type) -> bool {
        ty == types::F32 || ty == types::F64
    }

    /// Get the SPL type ID for a local.
    fn local_spl_type(&self, local: Local) -> TypeId {
        self.body.local_decl(local).ty
    }

    /// Get the address of a local's stack slot.
    ///
    /// Returns `None` if the local is not stored in a stack slot.
    fn local_stack_addr(&mut self, local: Local) -> Option<Value> {
        match self.local_map.get(local)? {
            LocalStorage::StackSlot(slot) => {
                let ptr_ty = self.type_mapper.pointer_type();
                Some(self.builder.ins().stack_addr(ptr_ty, slot, 0))
            }
            _ => None,
        }
    }

    /// Load a value from a memory address.
    fn load_from_addr(&mut self, addr: Value, ty: types::Type) -> Value {
        let flags = MemFlags::trusted();
        self.builder.ins().load(ty, flags, addr, 0)
    }

    /// Store a value to a memory address.
    fn store_to_addr(&mut self, addr: Value, val: Value) {
        let flags = MemFlags::trusted();
        self.builder.ins().store(flags, val, addr, 0);
    }

    /// Get the storage kind for a local.
    fn local_storage(&self, local: Local) -> Option<LocalStorage> {
        self.local_map.get(local)
    }

    /// Declare a string constant in the data section and return a GlobalValue for it.
    ///
    /// Strings are deduplicated within the same function.
    fn declare_string_data(&mut self, s: &str) -> Result<GlobalValue, CodegenError> {
        let module = self.module.as_mut().ok_or_else(|| {
            CodegenError::Internal("module required for string constants".to_string())
        })?;

        // Check if we already declared this string
        let data_id = if let Some(&existing_id) = self.string_data.get(s) {
            existing_id
        } else {
            // Generate unique name for this string data
            let name = format!(".str.{}", self.string_counter);
            self.string_counter += 1;

            // Declare the data item (read-only, not TLS)
            let data_id = module
                .declare_data(&name, Linkage::Local, false, false)
                .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

            // Define the data with the string bytes
            let mut desc = DataDescription::new();
            desc.define(s.as_bytes().into());
            module
                .define_data(data_id, &desc)
                .map_err(|e| CodegenError::ModuleError(e.to_string()))?;

            // Cache for deduplication
            self.string_data.insert(s.to_string(), data_id);
            data_id
        };

        // Need to re-borrow module after the if-else block
        let module = self.module.as_mut().ok_or_else(|| {
            CodegenError::Internal("module required for string constants".to_string())
        })?;

        // Import the data into the current function
        let gv = module.declare_data_in_func(data_id, self.builder.func);
        Ok(gv)
    }

    /// Lower a string constant to a destination address.
    ///
    /// Writes the pointer and length to the destination, which should be
    /// a stack slot with space for two pointer-sized values.
    pub(super) fn lower_string_constant_to(
        &mut self,
        s: &str,
        dest_addr: Value,
    ) -> Result<(), CodegenError> {
        let ptr_ty = self.type_mapper.pointer_type();

        // Declare string data and get a GlobalValue
        let gv = self.declare_string_data(s)?;

        // Get the address of the string data
        let str_ptr = self.builder.ins().global_value(ptr_ty, gv);

        // Create the length constant
        let str_len = self.builder.ins().iconst(ptr_ty, s.len() as i64);

        // Store pointer at offset 0
        let flags = MemFlags::trusted();
        self.builder.ins().store(flags, str_ptr, dest_addr, 0);

        // Store length at offset ptr_size (field 1)
        let ptr_size = if ptr_ty == types::I64 { 8 } else { 4 };
        self.builder
            .ins()
            .store(flags, str_len, dest_addr, ptr_size);

        Ok(())
    }
}
