//! MIR builder for constructing function bodies.

use crate::lexer::Span;
use crate::mir::body::{BasicBlockData, Body, LocalDecl};
use crate::mir::statement::Statement;
use crate::mir::terminator::{BasicBlock, Terminator, TerminatorKind};
use crate::mir::types::Local;
use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;

/// Builder for constructing MIR bodies incrementally.
///
/// `MirBuilder` provides a convenient API for building MIR function bodies
/// one statement at a time. It manages local allocation, basic block creation,
/// and ensures the return place is always Local(0).
pub struct MirBuilder {
    /// Local variable declarations.
    pub locals: Vec<LocalDecl>,
    /// Basic blocks in the function.
    pub basic_blocks: Vec<BasicBlockData>,
    /// Index of the current block we're building.
    current_block: usize,
    /// Number of arguments (for finish).
    arg_count: usize,
}

impl MirBuilder {
    /// Create a new `MirBuilder` with the given return type.
    ///
    /// The return place (Local 0) is automatically created as mutable.
    /// An entry block (`BasicBlock` 0) is also created.
    pub fn new(ret_ty: TypeId) -> Self {
        // Create return place as Local(0)
        let return_local = LocalDecl::new(ret_ty, true);

        // Create entry block
        let entry_block = BasicBlockData::new();

        MirBuilder {
            locals: vec![return_local],
            basic_blocks: vec![entry_block],
            current_block: 0,
            arg_count: 0,
        }
    }

    /// Allocate a new local variable.
    ///
    /// Returns the Local ID for the newly allocated variable.
    pub fn alloc_local(&mut self, ty: TypeId, mutable: bool, name: Option<String>) -> Local {
        let idx = self.locals.len() as u32;
        let decl = match name {
            Some(n) => LocalDecl::with_name(ty, mutable, n),
            None => LocalDecl::new(ty, mutable),
        };
        self.locals.push(decl);
        Local(idx)
    }

    /// Allocate a temporary variable (immutable, unnamed).
    ///
    /// Returns the Local ID for the newly allocated temporary.
    pub fn alloc_temp(&mut self, ty: TypeId) -> Local {
        self.alloc_local(ty, false, None)
    }

    /// Push a statement to the current basic block.
    pub fn push_statement(&mut self, stmt: Statement) {
        self.basic_blocks[self.current_block].push_statement(stmt);
    }

    /// Get a reference to the current basic block.
    pub fn current_block(&self) -> &BasicBlockData {
        &self.basic_blocks[self.current_block]
    }

    /// Get a mutable reference to the current basic block.
    pub fn current_block_mut(&mut self) -> &mut BasicBlockData {
        &mut self.basic_blocks[self.current_block]
    }

    /// Set the terminator for the current basic block.
    pub fn set_terminator(&mut self, kind: TerminatorKind, span: Span) {
        let terminator = Terminator::new(kind, span);
        self.basic_blocks[self.current_block].set_terminator(terminator);
    }

    /// Check if the current basic block already has a terminator.
    pub fn is_current_block_terminated(&self) -> bool {
        self.basic_blocks[self.current_block].is_terminated()
    }

    /// Allocate a new basic block and return its ID.
    pub fn alloc_block(&mut self) -> BasicBlock {
        let idx = self.basic_blocks.len() as u32;
        self.basic_blocks.push(BasicBlockData::new());
        BasicBlock(idx)
    }

    /// Switch to building a different basic block.
    pub fn switch_to_block(&mut self, bb: BasicBlock) {
        self.current_block = bb.index() as usize;
    }

    /// Set the argument count for the function.
    pub fn set_arg_count(&mut self, count: usize) {
        self.arg_count = count;
    }

    /// Finish building and produce a MIR Body.
    ///
    /// Consumes the builder and returns the completed Body.
    pub fn finish(self, arg_count: usize) -> Body {
        Body {
            def_id: None,
            name: None,
            basic_blocks: self.basic_blocks,
            locals: self.locals,
            arg_count,
        }
    }

    /// Finish building and produce a MIR Body with function metadata.
    ///
    /// Consumes the builder and returns the completed Body with `def_id` and name.
    pub fn finish_with_metadata(self, arg_count: usize, def_id: DefId, name: String) -> Body {
        Body {
            def_id: Some(def_id),
            name: Some(name),
            basic_blocks: self.basic_blocks,
            locals: self.locals,
            arg_count,
        }
    }
}
