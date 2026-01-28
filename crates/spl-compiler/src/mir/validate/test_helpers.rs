//! Test helpers for MIR validation.
//!
//! This module provides a lightweight `MirTestBuilder` for constructing MIR
//! in tests without requiring the full compilation pipeline.

use crate::mir::body::{BasicBlockData, Body, LocalDecl};
use crate::mir::operand::{Constant, Operand};
use crate::mir::statement::Statement;
use crate::mir::terminator::{BasicBlock, Terminator};
use crate::mir::types::Local;
use crate::sema::types::{TypeId, TypeInterner};

/// A builder for constructing MIR bodies in tests.
///
/// This provides a minimal API for creating MIR bodies with specific
/// characteristics for testing validation.
pub struct MirTestBuilder {
    body: Body,
    /// Public access to the type interner for test setup.
    pub types: TypeInterner,
}

impl Default for MirTestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MirTestBuilder {
    /// Create a new builder with unit return type.
    pub fn new() -> Self {
        let types = TypeInterner::new();
        let body = Body::new(types.unit());
        MirTestBuilder { body, types }
    }

    /// Set the return type (replaces the return local).
    pub fn with_return_ty(mut self, ty: TypeId) -> Self {
        self.body.locals[0] = LocalDecl::new(ty, true);
        self
    }

    /// Add a local variable and return its ID.
    pub fn add_local(&mut self, ty: TypeId, mutable: bool) -> Local {
        self.body.alloc_local(LocalDecl::new(ty, mutable))
    }

    /// Add a named local variable and return its ID.
    pub fn add_named_local(&mut self, ty: TypeId, mutable: bool, name: &str) -> Local {
        self.body
            .alloc_local(LocalDecl::with_name(ty, mutable, name))
    }

    /// Add a new basic block and return its ID.
    pub fn add_block(&mut self) -> BasicBlock {
        self.body.alloc_block()
    }

    /// Add a statement to a block.
    pub fn add_statement(&mut self, block: BasicBlock, stmt: Statement) {
        self.body.block_mut(block).push_statement(stmt);
    }

    /// Set the terminator for a block.
    pub fn set_terminator(&mut self, block: BasicBlock, term: Terminator) {
        self.body.block_mut(block).set_terminator(term);
    }

    /// Get direct mutable access to a block (for advanced test scenarios).
    pub fn block_mut(&mut self, block: BasicBlock) -> &mut BasicBlockData {
        self.body.block_mut(block)
    }

    /// Build the final body and type interner.
    pub fn build(self) -> (Body, TypeInterner) {
        (self.body, self.types)
    }

    /// Get the number of locals (including return place).
    pub fn num_locals(&self) -> usize {
        self.body.num_locals()
    }

    /// Get the number of blocks.
    pub fn num_blocks(&self) -> usize {
        self.body.num_blocks()
    }

    /// Create an i32-typed integer constant operand (convenience for tests).
    pub fn const_i32(&self, value: i128) -> Operand {
        Operand::Constant(Constant::Int(value, self.types.i32()))
    }

    /// Create an i64-typed integer constant operand (convenience for tests).
    #[allow(dead_code)]
    pub fn const_i64(&self, value: i128) -> Operand {
        Operand::Constant(Constant::Int(value, self.types.i64()))
    }

    /// Create a typed integer constant operand.
    #[allow(dead_code)]
    pub fn const_int(&self, value: i128, ty: TypeId) -> Operand {
        Operand::Constant(Constant::Int(value, ty))
    }

    /// Create a typed float constant operand.
    #[allow(dead_code)]
    pub fn const_float(&self, value: f64, ty: TypeId) -> Operand {
        Operand::Constant(Constant::Float(value, ty))
    }

    /// Create an f64-typed float constant operand (convenience for tests).
    #[allow(dead_code)]
    pub fn const_f64(&self, value: f64) -> Operand {
        Operand::Constant(Constant::Float(value, self.types.f64()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::operand::Rvalue;
    use crate::mir::statement::Statement;
    use crate::mir::terminator::Terminator;
    use crate::mir::types::Place;

    #[test]
    fn builder_creates_valid_empty_body() {
        let builder = MirTestBuilder::new();
        let (body, _) = builder.build();

        assert_eq!(body.num_locals(), 1); // Return place
        assert_eq!(body.num_blocks(), 0); // No blocks yet
    }

    #[test]
    fn builder_with_return_ty() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        builder = builder.with_return_ty(i32_ty);
        let (body, _) = builder.build();

        assert_eq!(body.return_ty(), i32_ty);
    }

    #[test]
    fn builder_add_local() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();

        let local1 = builder.add_local(i32_ty, false);
        let local2 = builder.add_local(i32_ty, true);

        assert_eq!(local1, Local(1));
        assert_eq!(local2, Local(2));
        assert_eq!(builder.num_locals(), 3); // return + 2 locals
    }

    #[test]
    fn builder_add_named_local() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();

        let local = builder.add_named_local(i32_ty, true, "my_var");

        assert_eq!(local, Local(1));
        let (body, _) = builder.build();
        assert_eq!(body.local_decl(local).name, Some("my_var".to_string()));
    }

    #[test]
    fn builder_add_block() {
        let mut builder = MirTestBuilder::new();

        let bb0 = builder.add_block();
        let bb1 = builder.add_block();

        assert_eq!(bb0, BasicBlock(0));
        assert_eq!(bb1, BasicBlock(1));
        assert_eq!(builder.num_blocks(), 2);
    }

    #[test]
    fn builder_add_statement_and_terminator() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();

        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(Local::RETURN_PLACE),
                Rvalue::Use(builder.const_i32(42)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        assert_eq!(body.block(bb).statements.len(), 1);
        assert!(body.block(bb).is_terminated());
    }

    #[test]
    fn builder_default() {
        let builder = MirTestBuilder::default();
        let (body, types) = builder.build();

        assert_eq!(body.num_locals(), 1);
        assert_eq!(body.return_ty(), types.unit());
    }

    #[test]
    fn builder_block_mut_access() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();

        // Use block_mut for direct manipulation
        builder.block_mut(bb).push_statement(Statement::nop(0..0));
        builder.block_mut(bb).push_statement(Statement::nop(0..0));
        builder
            .block_mut(bb)
            .set_terminator(Terminator::return_(0..0));

        let (body, _) = builder.build();
        assert_eq!(body.block(bb).statements.len(), 2);
    }
}
