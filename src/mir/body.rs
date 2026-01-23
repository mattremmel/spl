//! MIR function body representation.
//!
//! This module defines the Body type which represents a complete MIR function,
//! including its basic blocks, local variables, and control flow graph.

use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;

use super::statement::Statement;
use super::terminator::{BasicBlock, Terminator};
use super::types::Local;

/// A local variable declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalDecl {
    /// The type of the local.
    pub ty: TypeId,
    /// Whether the local is mutable.
    pub mutable: bool,
    /// Optional name for debugging/display.
    pub name: Option<String>,
}

impl LocalDecl {
    /// Create a new local declaration.
    pub fn new(ty: TypeId, mutable: bool) -> Self {
        LocalDecl {
            ty,
            mutable,
            name: None,
        }
    }

    /// Create a local declaration with a name.
    pub fn with_name(ty: TypeId, mutable: bool, name: impl Into<String>) -> Self {
        LocalDecl {
            ty,
            mutable,
            name: Some(name.into()),
        }
    }
}

/// Data for a basic block.
#[derive(Clone, Debug)]
pub struct BasicBlockData {
    /// The statements in this block, executed sequentially.
    pub statements: Vec<Statement>,
    /// The terminator that ends this block.
    pub terminator: Option<Terminator>,
}

impl BasicBlockData {
    /// Create a new empty basic block.
    pub fn new() -> Self {
        BasicBlockData {
            statements: Vec::new(),
            terminator: None,
        }
    }

    /// Returns true if this block has a terminator.
    pub fn is_terminated(&self) -> bool {
        self.terminator.is_some()
    }

    /// Add a statement to this block.
    pub fn push_statement(&mut self, stmt: Statement) {
        self.statements.push(stmt);
    }

    /// Set the terminator for this block.
    pub fn set_terminator(&mut self, terminator: Terminator) {
        self.terminator = Some(terminator);
    }

    /// Get all successor blocks of this basic block.
    pub fn successors(&self) -> Vec<BasicBlock> {
        self.terminator
            .as_ref()
            .map(|t| t.successors())
            .unwrap_or_default()
    }
}

impl Default for BasicBlockData {
    fn default() -> Self {
        Self::new()
    }
}

/// A MIR function body.
#[derive(Clone, Debug)]
pub struct Body {
    /// Definition ID of the function (if from HIR lowering).
    pub def_id: Option<DefId>,
    /// Name of the function (if from HIR lowering).
    pub name: Option<String>,
    /// All basic blocks in this function.
    pub basic_blocks: Vec<BasicBlockData>,
    /// Local variable declarations.
    /// Local 0 is always the return place.
    pub locals: Vec<LocalDecl>,
    /// Number of function arguments (excluding return place).
    pub arg_count: usize,
}

impl Body {
    /// Create a new MIR body with the given return type.
    ///
    /// The return place (Local 0) is automatically created.
    pub fn new(return_ty: TypeId) -> Self {
        let return_local = LocalDecl::new(return_ty, true);
        Body {
            def_id: None,
            name: None,
            basic_blocks: Vec::new(),
            locals: vec![return_local],
            arg_count: 0,
        }
    }

    /// Create a new MIR body with a return type and argument types.
    ///
    /// Arguments are allocated as locals 1..=arg_count.
    pub fn with_args(return_ty: TypeId, arg_types: &[(TypeId, bool)]) -> Self {
        let mut locals = vec![LocalDecl::new(return_ty, true)];
        for (ty, mutable) in arg_types {
            locals.push(LocalDecl::new(*ty, *mutable));
        }
        Body {
            def_id: None,
            name: None,
            basic_blocks: Vec::new(),
            locals,
            arg_count: arg_types.len(),
        }
    }

    /// Allocate a new local variable and return its ID.
    pub fn alloc_local(&mut self, decl: LocalDecl) -> Local {
        let idx = self.locals.len() as u32;
        self.locals.push(decl);
        Local(idx)
    }

    /// Allocate a new basic block and return its ID.
    pub fn alloc_block(&mut self) -> BasicBlock {
        let idx = self.basic_blocks.len() as u32;
        self.basic_blocks.push(BasicBlockData::new());
        BasicBlock(idx)
    }

    /// Get a reference to a basic block by ID.
    pub fn block(&self, bb: BasicBlock) -> &BasicBlockData {
        &self.basic_blocks[bb.index() as usize]
    }

    /// Get a mutable reference to a basic block by ID.
    pub fn block_mut(&mut self, bb: BasicBlock) -> &mut BasicBlockData {
        &mut self.basic_blocks[bb.index() as usize]
    }

    /// Get a reference to a local declaration by ID.
    pub fn local_decl(&self, local: Local) -> &LocalDecl {
        &self.locals[local.index() as usize]
    }

    /// Get the return place (Local 0).
    pub fn return_place(&self) -> Local {
        Local::RETURN_PLACE
    }

    /// Get the return type.
    pub fn return_ty(&self) -> TypeId {
        self.locals[0].ty
    }

    /// Get argument locals (Local 1..=arg_count).
    pub fn args(&self) -> impl Iterator<Item = Local> {
        (1..=self.arg_count).map(|i| Local(i as u32))
    }

    /// Get all user locals (everything after arguments).
    pub fn user_locals(&self) -> impl Iterator<Item = Local> {
        let start = 1 + self.arg_count;
        (start..self.locals.len()).map(|i| Local(i as u32))
    }

    /// Returns the number of basic blocks.
    pub fn num_blocks(&self) -> usize {
        self.basic_blocks.len()
    }

    /// Returns the number of locals (including return place and args).
    pub fn num_locals(&self) -> usize {
        self.locals.len()
    }

    /// Validate that all blocks have terminators and all successors are valid.
    ///
    /// Returns `Ok(())` if valid, or `Err` with a description of the problem.
    pub fn validate(&self) -> Result<(), String> {
        // Check all blocks have terminators
        for (idx, block) in self.basic_blocks.iter().enumerate() {
            if block.terminator.is_none() {
                return Err(format!("BasicBlock {} has no terminator", idx));
            }
        }

        // Check all successors are valid block indices
        for (idx, block) in self.basic_blocks.iter().enumerate() {
            for successor in block.successors() {
                if successor.index() as usize >= self.basic_blocks.len() {
                    return Err(format!(
                        "BasicBlock {} has invalid successor {}",
                        idx,
                        successor.index()
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::operand::{Operand, Rvalue};
    use crate::mir::statement::Statement;
    use crate::mir::terminator::{Terminator, TerminatorKind};
    use crate::mir::types::Place;

    #[test]
    fn local_decl_with_type() {
        let ty = TypeId(5);
        let decl = LocalDecl::new(ty, false);

        assert_eq!(decl.ty, ty);
        assert!(!decl.mutable);
        assert!(decl.name.is_none());
    }

    #[test]
    fn local_decl_with_name() {
        let ty = TypeId(5);
        let decl = LocalDecl::with_name(ty, true, "my_var");

        assert_eq!(decl.ty, ty);
        assert!(decl.mutable);
        assert_eq!(decl.name, Some("my_var".to_string()));
    }

    #[test]
    fn basic_block_data_starts_empty() {
        let block = BasicBlockData::new();

        assert!(block.statements.is_empty());
        assert!(block.terminator.is_none());
        assert!(!block.is_terminated());
    }

    #[test]
    fn basic_block_data_default() {
        let block = BasicBlockData::default();
        assert!(block.statements.is_empty());
        assert!(block.terminator.is_none());
    }

    #[test]
    fn basic_block_data_push_statement() {
        let mut block = BasicBlockData::new();
        let stmt = Statement::nop(0..0);

        block.push_statement(stmt.clone());

        assert_eq!(block.statements.len(), 1);
        assert_eq!(block.statements[0], stmt);
    }

    #[test]
    fn basic_block_data_set_terminator() {
        let mut block = BasicBlockData::new();
        let term = Terminator::return_(0..0);

        block.set_terminator(term.clone());

        assert!(block.is_terminated());
        assert_eq!(block.terminator, Some(term));
    }

    #[test]
    fn basic_block_data_successors() {
        let mut block = BasicBlockData::new();

        // No terminator -> no successors
        assert!(block.successors().is_empty());

        // Set terminator
        block.set_terminator(Terminator::goto(BasicBlock(5), 0..0));
        assert_eq!(block.successors(), vec![BasicBlock(5)]);
    }

    #[test]
    fn mir_body_has_return_place() {
        let return_ty = TypeId(10);
        let body = Body::new(return_ty);

        assert_eq!(body.locals.len(), 1);
        assert_eq!(body.locals[0].ty, return_ty);
        assert_eq!(body.return_ty(), return_ty);
        assert_eq!(body.return_place(), Local::RETURN_PLACE);
    }

    #[test]
    fn mir_body_alloc_local() {
        let mut body = Body::new(TypeId(0));

        let local1 = body.alloc_local(LocalDecl::new(TypeId(1), false));
        let local2 = body.alloc_local(LocalDecl::new(TypeId(2), true));

        assert_eq!(local1, Local(1));
        assert_eq!(local2, Local(2));
        assert_eq!(body.num_locals(), 3);
    }

    #[test]
    fn mir_body_alloc_block() {
        let mut body = Body::new(TypeId(0));

        let bb0 = body.alloc_block();
        let bb1 = body.alloc_block();
        let bb2 = body.alloc_block();

        assert_eq!(bb0, BasicBlock(0));
        assert_eq!(bb1, BasicBlock(1));
        assert_eq!(bb2, BasicBlock(2));
        assert_eq!(body.num_blocks(), 3);
    }

    #[test]
    fn mir_body_block_access() {
        let mut body = Body::new(TypeId(0));
        let bb = body.alloc_block();

        // Mutable access
        body.block_mut(bb).push_statement(Statement::nop(0..0));

        // Immutable access
        assert_eq!(body.block(bb).statements.len(), 1);
    }

    #[test]
    fn mir_body_local_decl_access() {
        let mut body = Body::new(TypeId(0));
        let local = body.alloc_local(LocalDecl::with_name(TypeId(5), true, "x"));

        let decl = body.local_decl(local);
        assert_eq!(decl.ty, TypeId(5));
        assert!(decl.mutable);
        assert_eq!(decl.name, Some("x".to_string()));
    }

    #[test]
    fn mir_body_with_args() {
        let return_ty = TypeId(0);
        let args = vec![(TypeId(1), false), (TypeId(2), true)];
        let body = Body::with_args(return_ty, &args);

        assert_eq!(body.arg_count, 2);
        assert_eq!(body.num_locals(), 3); // return + 2 args

        let arg_locals: Vec<_> = body.args().collect();
        assert_eq!(arg_locals, vec![Local(1), Local(2)]);

        assert_eq!(body.local_decl(Local(1)).ty, TypeId(1));
        assert!(!body.local_decl(Local(1)).mutable);
        assert_eq!(body.local_decl(Local(2)).ty, TypeId(2));
        assert!(body.local_decl(Local(2)).mutable);
    }

    #[test]
    fn mir_body_user_locals() {
        let body = Body::with_args(TypeId(0), &[(TypeId(1), false), (TypeId(2), false)]);
        let user_locals: Vec<_> = body.user_locals().collect();

        // No user locals yet (only return + args)
        assert!(user_locals.is_empty());
    }

    #[test]
    fn mir_body_user_locals_after_alloc() {
        let mut body = Body::with_args(TypeId(0), &[(TypeId(1), false)]);
        body.alloc_local(LocalDecl::new(TypeId(10), false));
        body.alloc_local(LocalDecl::new(TypeId(11), true));

        let user_locals: Vec<_> = body.user_locals().collect();
        assert_eq!(user_locals, vec![Local(2), Local(3)]);
    }

    #[test]
    fn body_all_blocks_have_terminators_valid() {
        let mut body = Body::new(TypeId(0));
        let bb = body.alloc_block();
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        assert!(body.validate().is_ok());
    }

    #[test]
    fn body_all_blocks_have_terminators_invalid() {
        let mut body = Body::new(TypeId(0));
        body.alloc_block(); // No terminator

        let result = body.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no terminator"));
    }

    #[test]
    fn body_successors_are_valid() {
        let mut body = Body::new(TypeId(0));
        let bb0 = body.alloc_block();
        let bb1 = body.alloc_block();

        body.block_mut(bb0)
            .set_terminator(Terminator::goto(bb1, 0..0));
        body.block_mut(bb1)
            .set_terminator(Terminator::return_(0..0));

        assert!(body.validate().is_ok());
    }

    #[test]
    fn body_successors_invalid() {
        let mut body = Body::new(TypeId(0));
        let bb0 = body.alloc_block();

        // Point to non-existent block
        body.block_mut(bb0)
            .set_terminator(Terminator::goto(BasicBlock(999), 0..0));

        let result = body.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid successor"));
    }

    #[test]
    fn body_validate_empty_is_valid() {
        let body = Body::new(TypeId(0));
        // Empty body (no blocks) is valid
        assert!(body.validate().is_ok());
    }

    #[test]
    fn body_simple_function() {
        // Build: fn foo() -> i32 { 42 }
        let return_ty = TypeId(1); // i32
        let mut body = Body::new(return_ty);

        // Allocate entry block
        let entry = body.alloc_block();

        // _0 = 42
        let return_place = Place::from_local(body.return_place());
        let assign = Statement::assign(return_place, Rvalue::Use(Operand::const_int(42)), 0..5);
        body.block_mut(entry).push_statement(assign);

        // return
        body.block_mut(entry)
            .set_terminator(Terminator::return_(5..10));

        assert!(body.validate().is_ok());
        assert_eq!(body.num_blocks(), 1);
        assert_eq!(body.num_locals(), 1);
    }

    #[test]
    fn body_with_branch() {
        // Build:
        // fn foo(cond: bool) -> i32 {
        //     if cond { 1 } else { 2 }
        // }
        let return_ty = TypeId(1);
        let bool_ty = TypeId(2);
        let mut body = Body::with_args(return_ty, &[(bool_ty, false)]);

        let entry = body.alloc_block();
        let then_block = body.alloc_block();
        let else_block = body.alloc_block();
        let join_block = body.alloc_block();

        // Entry: switch on arg
        let cond = Operand::copy_local(Local(1));
        body.block_mut(entry).set_terminator(Terminator::new(
            TerminatorKind::SwitchInt {
                discr: cond,
                targets: crate::mir::terminator::SwitchTargets::new_bool(then_block, else_block),
            },
            0..0,
        ));

        // Then: _0 = 1; goto join
        body.block_mut(then_block).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(1)),
            0..0,
        ));
        body.block_mut(then_block)
            .set_terminator(Terminator::goto(join_block, 0..0));

        // Else: _0 = 2; goto join
        body.block_mut(else_block).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(2)),
            0..0,
        ));
        body.block_mut(else_block)
            .set_terminator(Terminator::goto(join_block, 0..0));

        // Join: return
        body.block_mut(join_block)
            .set_terminator(Terminator::return_(0..0));

        assert!(body.validate().is_ok());
        assert_eq!(body.num_blocks(), 4);
    }

    // Additional coverage tests

    #[test]
    fn body_with_loop() {
        // Build a simple loop: while (cond) { body }
        // bb0: goto bb1
        // bb1: switchInt(cond) -> [0: bb3, otherwise: bb2]
        // bb2: <body>; goto bb1  (back edge!)
        // bb3: return
        let mut body = Body::new(TypeId(0));

        let entry = body.alloc_block();
        let loop_header = body.alloc_block();
        let loop_body = body.alloc_block();
        let exit = body.alloc_block();

        // bb0 -> bb1
        body.block_mut(entry)
            .set_terminator(Terminator::goto(loop_header, 0..0));

        // bb1: conditional branch
        body.block_mut(loop_header).set_terminator(Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::const_bool(true),
                targets: crate::mir::terminator::SwitchTargets::new_bool(loop_body, exit),
            },
            0..0,
        ));

        // bb2: loop body, back edge to header
        body.block_mut(loop_body)
            .set_terminator(Terminator::goto(loop_header, 0..0));

        // bb3: exit
        body.block_mut(exit)
            .set_terminator(Terminator::return_(0..0));

        // Should validate - loops are valid CFGs
        assert!(body.validate().is_ok());
        assert_eq!(body.num_blocks(), 4);
    }

    #[test]
    fn body_self_loop() {
        // A block that loops to itself: bb0: goto bb0
        let mut body = Body::new(TypeId(0));
        let bb = body.alloc_block();

        body.block_mut(bb)
            .set_terminator(Terminator::goto(bb, 0..0));

        // Self-loops are valid
        assert!(body.validate().is_ok());
    }

    #[test]
    fn body_multiple_statements_in_block() {
        let mut body = Body::new(TypeId(0));
        let bb = body.alloc_block();
        let temp = body.alloc_local(LocalDecl::new(TypeId(1), true));

        // Add multiple statements
        body.block_mut(bb)
            .push_statement(Statement::storage_live(temp, 0..0));
        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(temp),
            Rvalue::Use(Operand::const_int(42)),
            0..0,
        ));
        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::copy_local(temp)),
            0..0,
        ));
        body.block_mut(bb)
            .push_statement(Statement::storage_dead(temp, 0..0));
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        assert_eq!(body.block(bb).statements.len(), 4);
        assert!(body.validate().is_ok());
    }

    #[test]
    fn body_entry_block_is_zero() {
        let mut body = Body::new(TypeId(0));

        // Allocate several blocks
        let bb0 = body.alloc_block();
        let bb1 = body.alloc_block();
        let bb2 = body.alloc_block();

        // First allocated block should be entry (index 0)
        assert_eq!(bb0, BasicBlock::ENTRY);
        assert_eq!(bb0.index(), 0);
        assert_eq!(bb1.index(), 1);
        assert_eq!(bb2.index(), 2);
    }

    #[test]
    fn body_return_place_is_mutable() {
        let body = Body::new(TypeId(0));

        // Return place should be mutable (we assign to it)
        assert!(body.local_decl(Local::RETURN_PLACE).mutable);
    }

    #[test]
    fn statement_same_kind_different_span() {
        let stmt1 = Statement::nop(0..5);
        let stmt2 = Statement::nop(10..15);

        // Same kind but different spans are not equal
        assert_ne!(stmt1, stmt2);
    }

    #[test]
    fn body_with_unreachable_block() {
        // A function with an unreachable block (valid but unusual)
        let mut body = Body::new(TypeId(0));

        let entry = body.alloc_block();
        let unreachable = body.alloc_block();

        body.block_mut(entry)
            .set_terminator(Terminator::return_(0..0));
        body.block_mut(unreachable)
            .set_terminator(Terminator::unreachable(0..0));

        // Should still validate - unreachable blocks are allowed
        assert!(body.validate().is_ok());
    }

    #[test]
    fn local_decl_equality() {
        let decl1 = LocalDecl::new(TypeId(1), true);
        let decl2 = LocalDecl::new(TypeId(1), true);
        let decl3 = LocalDecl::new(TypeId(1), false);
        let decl4 = LocalDecl::new(TypeId(2), true);

        assert_eq!(decl1, decl2);
        assert_ne!(decl1, decl3); // different mutability
        assert_ne!(decl1, decl4); // different type
    }

    #[test]
    fn body_validate_multiple_errors() {
        // First error found is returned
        let mut body = Body::new(TypeId(0));

        let _bb0 = body.alloc_block();
        let bb1 = body.alloc_block();

        // bb0 has no terminator
        // bb1 points to invalid block
        body.block_mut(bb1)
            .set_terminator(Terminator::goto(BasicBlock(999), 0..0));

        let result = body.validate();
        assert!(result.is_err());
        // Should report bb0 has no terminator (first check)
        assert!(result.unwrap_err().contains("BasicBlock 0"));
    }

    #[test]
    fn body_with_args_zero_args() {
        let body = Body::with_args(TypeId(0), &[]);

        assert_eq!(body.arg_count, 0);
        assert_eq!(body.num_locals(), 1); // just return place
        assert_eq!(body.args().count(), 0);
    }
}
