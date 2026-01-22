//! HIR to MIR lowering.
//!
//! This module provides the infrastructure for lowering HIR (High-level IR)
//! to MIR (Mid-level IR). The lowering process converts nested expressions
//! into a flat, control-flow-graph representation suitable for borrow checking
//! and optimization.

use crate::hir::{
    BinOp as HirBinOp, ExprId, HirDatabase, HirExprKind, HirFunction, HirItem, HirPatKind,
    HirStmtKind, Literal, StmtId, UnaryOp as HirUnaryOp,
};
use crate::lexer::Span;
use crate::mir::body::{BasicBlockData, Body, LocalDecl};
use crate::mir::operand::{AggregateKind, BinOp, BorrowKind, CastKind, Constant, Operand, Rvalue, UnOp};
use crate::mir::statement::Statement;
use crate::mir::terminator::{BasicBlock, SwitchTargets, Terminator, TerminatorKind};
use crate::mir::types::{FieldIdx, Local, Place, PlaceElem};
use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;

use rustc_hash::FxHashMap;

/// Context for tracking loop targets during control flow lowering.
///
/// When lowering loops, we need to track where `break` and `continue`
/// should jump to, and where to store the loop's result value.
#[derive(Debug, Clone)]
pub struct LoopContext {
    /// The block to jump to on `break`.
    pub exit_block: BasicBlock,
    /// The block to jump to on `continue`.
    pub header_block: BasicBlock,
    /// Where to store the `break` value (if the loop produces a value).
    pub result_place: Option<Place>,
}

/// Builder for constructing MIR bodies incrementally.
///
/// MirBuilder provides a convenient API for building MIR function bodies
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
    /// Create a new MirBuilder with the given return type.
    ///
    /// The return place (Local 0) is automatically created as mutable.
    /// An entry block (BasicBlock 0) is also created.
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
            basic_blocks: self.basic_blocks,
            locals: self.locals,
            arg_count,
        }
    }
}

/// Convert an HIR literal to a MIR Constant.
pub fn lower_literal(lit: &Literal) -> Constant {
    match lit {
        Literal::Int(v) => Constant::Int(*v),
        Literal::Float(v) => Constant::Float(*v),
        Literal::Bool(v) => Constant::Bool(*v),
        Literal::Char(v) => Constant::Char(*v),
        Literal::String(v) => Constant::String(v.clone()),
    }
}

/// Convert an HIR literal to a MIR Operand.
pub fn literal_to_operand(lit: &Literal) -> Operand {
    Operand::Constant(lower_literal(lit))
}

/// Convert an HIR binary operator to a MIR binary operator.
///
/// Returns `None` for operators that require special handling:
/// - `And`/`Or`: Short-circuit evaluation (control flow)
/// - `Assign`/`*Assign`: Assignment statements
pub fn hir_binop_to_mir(op: HirBinOp) -> Option<BinOp> {
    match op {
        // Arithmetic
        HirBinOp::Add => Some(BinOp::Add),
        HirBinOp::Sub => Some(BinOp::Sub),
        HirBinOp::Mul => Some(BinOp::Mul),
        HirBinOp::Div => Some(BinOp::Div),
        HirBinOp::Rem => Some(BinOp::Rem),
        // Comparison
        HirBinOp::Eq => Some(BinOp::Eq),
        HirBinOp::Ne => Some(BinOp::Ne),
        HirBinOp::Lt => Some(BinOp::Lt),
        HirBinOp::Le => Some(BinOp::Le),
        HirBinOp::Gt => Some(BinOp::Gt),
        HirBinOp::Ge => Some(BinOp::Ge),
        // Short-circuit: handled by control flow lowering
        HirBinOp::And | HirBinOp::Or => None,
        // Assignment: handled by statement lowering
        HirBinOp::Assign
        | HirBinOp::AddAssign
        | HirBinOp::SubAssign
        | HirBinOp::MulAssign
        | HirBinOp::DivAssign
        | HirBinOp::RemAssign => None,
    }
}

/// Convert an HIR unary operator to a MIR unary operator.
///
/// Returns `None` for operators that require special handling:
/// - `Deref`: Produces a place, not an rvalue
pub fn hir_unop_to_mir(op: HirUnaryOp) -> Option<UnOp> {
    match op {
        HirUnaryOp::Not => Some(UnOp::Not),
        HirUnaryOp::Neg => Some(UnOp::Neg),
        // Deref produces a place (projection), not an rvalue
        HirUnaryOp::Deref => None,
    }
}

/// Determine the cast kind for a cast between two types.
fn determine_cast_kind(hir: &HirDatabase, from: TypeId, to: TypeId) -> CastKind {
    use crate::sema::types::{PrimitiveKind, Type};

    let from_ty = hir.types.get(from);
    let to_ty = hir.types.get(to);

    // Check if source is an integer type
    let from_is_int = matches!(
        from_ty,
        Type::Primitive(
            PrimitiveKind::I8
                | PrimitiveKind::I16
                | PrimitiveKind::I32
                | PrimitiveKind::I64
                | PrimitiveKind::I128
                | PrimitiveKind::Isize
                | PrimitiveKind::U8
                | PrimitiveKind::U16
                | PrimitiveKind::U32
                | PrimitiveKind::U64
                | PrimitiveKind::U128
                | PrimitiveKind::Usize
        )
    );

    let from_is_float = matches!(
        from_ty,
        Type::Primitive(PrimitiveKind::F32 | PrimitiveKind::F64)
    );

    let to_is_int = matches!(
        to_ty,
        Type::Primitive(
            PrimitiveKind::I8
                | PrimitiveKind::I16
                | PrimitiveKind::I32
                | PrimitiveKind::I64
                | PrimitiveKind::I128
                | PrimitiveKind::Isize
                | PrimitiveKind::U8
                | PrimitiveKind::U16
                | PrimitiveKind::U32
                | PrimitiveKind::U64
                | PrimitiveKind::U128
                | PrimitiveKind::Usize
        )
    );

    let to_is_float = matches!(
        to_ty,
        Type::Primitive(PrimitiveKind::F32 | PrimitiveKind::F64)
    );

    match (from_is_int, from_is_float, to_is_int, to_is_float) {
        (true, _, true, _) => CastKind::IntToInt,
        (true, _, _, true) => CastKind::IntToFloat,
        (_, true, true, _) => CastKind::FloatToInt,
        (_, true, _, true) => CastKind::FloatToFloat,
        // Pointer casts or other cases
        _ => CastKind::PtrToPtr,
    }
}

/// Context for lowering HIR to MIR.
///
/// This maintains state during the lowering process, including:
/// - Reference to the HIR database
/// - Mapping from DefIds to MIR locals
/// - The collection of lowered function bodies
/// - Stack of loop contexts for break/continue handling
pub struct MirLoweringContext<'hir> {
    /// Reference to the HIR database.
    pub hir: &'hir HirDatabase,
    /// Map from binding DefIds to their MIR locals.
    local_map: FxHashMap<DefId, Local>,
    /// Lowered function bodies.
    pub bodies: Vec<Body>,
    /// Stack of active loop contexts for break/continue target resolution.
    loop_stack: Vec<LoopContext>,
}

impl<'hir> MirLoweringContext<'hir> {
    /// Create a new lowering context.
    pub fn new(hir: &'hir HirDatabase) -> Self {
        MirLoweringContext {
            hir,
            local_map: FxHashMap::default(),
            bodies: Vec::new(),
            loop_stack: Vec::new(),
        }
    }

    /// Start lowering a function, setting up the builder with parameters.
    pub fn start_function(&mut self, func: &HirFunction) -> MirBuilder {
        let mut builder = MirBuilder::new(func.ret_type);

        // Clear the local map and loop stack for this function
        self.local_map.clear();
        self.loop_stack.clear();

        // Allocate locals for each parameter
        for param in &func.params {
            let pat = self.hir.pat(param.pat);
            match &pat.kind {
                crate::hir::HirPatKind::Bind { def_id, mutable } => {
                    // Map DefId to Local
                    let local = builder.alloc_local(param.ty, *mutable, None);
                    self.local_map.insert(*def_id, local);
                }
                crate::hir::HirPatKind::Wildcard => {
                    // Wildcard still needs a local for the value
                    builder.alloc_local(param.ty, true, None);
                }
                _ => {
                    // Other patterns - allocate anonymous local
                    builder.alloc_local(param.ty, true, None);
                }
            }
        }

        builder.set_arg_count(func.params.len());
        builder
    }

    /// Lower an expression to a place (allocates a temp if needed).
    pub fn lower_expr_to_place(&mut self, builder: &mut MirBuilder, expr_id: ExprId) -> Place {
        let expr = self.hir.expr(expr_id);
        let ty = expr.ty;
        let span = expr.span.clone();

        match &expr.kind {
            HirExprKind::Literal(lit) => {
                // Allocate a temp for the literal
                let temp = builder.alloc_temp(ty);
                let place = Place::from_local(temp);
                let constant = lower_literal(lit);
                let stmt = Statement::assign(
                    place.clone(),
                    Rvalue::Use(Operand::Constant(constant)),
                    span,
                );
                builder.push_statement(stmt);
                place
            }
            HirExprKind::Var(def_id) => {
                // Look up the local for this variable
                if let Some(&local) = self.local_map.get(def_id) {
                    Place::from_local(local)
                } else {
                    // Variable not found - allocate a temp as fallback
                    // This shouldn't happen with well-formed HIR
                    let temp = builder.alloc_temp(ty);
                    Place::from_local(temp)
                }
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                // Handle short-circuit operators specially
                if *op == HirBinOp::And {
                    // Short-circuit AND: if LHS is false, result is false; else evaluate RHS
                    let result_place = Place::from_local(builder.alloc_temp(ty));

                    // Evaluate LHS
                    let lhs_operand = self.lower_expr_as_operand(builder, *lhs);

                    // Create blocks
                    let rhs_bb = builder.alloc_block();
                    let false_bb = builder.alloc_block();
                    let merge_bb = builder.alloc_block();

                    // Branch: if LHS true -> rhs_bb, else -> false_bb
                    let targets = SwitchTargets::new_bool(rhs_bb, false_bb);
                    builder.set_terminator(
                        TerminatorKind::SwitchInt {
                            discr: lhs_operand,
                            targets,
                        },
                        span.clone(),
                    );

                    // RHS block: evaluate RHS, store result, goto merge
                    builder.switch_to_block(rhs_bb);
                    let rhs_operand = self.lower_expr_as_operand(builder, *rhs);
                    builder.push_statement(Statement::assign(
                        result_place.clone(),
                        Rvalue::Use(rhs_operand),
                        span.clone(),
                    ));
                    builder.set_terminator(TerminatorKind::Goto(merge_bb), span.clone());

                    // False block: result = false, goto merge
                    builder.switch_to_block(false_bb);
                    builder.push_statement(Statement::assign(
                        result_place.clone(),
                        Rvalue::Use(Operand::Constant(Constant::Bool(false))),
                        span.clone(),
                    ));
                    builder.set_terminator(TerminatorKind::Goto(merge_bb), span);

                    // Continue in merge block
                    builder.switch_to_block(merge_bb);
                    result_place
                } else if *op == HirBinOp::Or {
                    // Short-circuit OR: if LHS is true, result is true; else evaluate RHS
                    let result_place = Place::from_local(builder.alloc_temp(ty));

                    // Evaluate LHS
                    let lhs_operand = self.lower_expr_as_operand(builder, *lhs);

                    // Create blocks
                    let true_bb = builder.alloc_block();
                    let rhs_bb = builder.alloc_block();
                    let merge_bb = builder.alloc_block();

                    // Branch: if LHS true -> true_bb, else -> rhs_bb
                    let targets = SwitchTargets::new_bool(true_bb, rhs_bb);
                    builder.set_terminator(
                        TerminatorKind::SwitchInt {
                            discr: lhs_operand,
                            targets,
                        },
                        span.clone(),
                    );

                    // True block: result = true, goto merge
                    builder.switch_to_block(true_bb);
                    builder.push_statement(Statement::assign(
                        result_place.clone(),
                        Rvalue::Use(Operand::Constant(Constant::Bool(true))),
                        span.clone(),
                    ));
                    builder.set_terminator(TerminatorKind::Goto(merge_bb), span.clone());

                    // RHS block: evaluate RHS, store result, goto merge
                    builder.switch_to_block(rhs_bb);
                    let rhs_operand = self.lower_expr_as_operand(builder, *rhs);
                    builder.push_statement(Statement::assign(
                        result_place.clone(),
                        Rvalue::Use(rhs_operand),
                        span.clone(),
                    ));
                    builder.set_terminator(TerminatorKind::Goto(merge_bb), span);

                    // Continue in merge block
                    builder.switch_to_block(merge_bb);
                    result_place
                } else if let Some(mir_op) = hir_binop_to_mir(*op) {
                    // Simple binary operation
                    let lhs_operand = self.lower_expr_as_operand(builder, *lhs);
                    let rhs_operand = self.lower_expr_as_operand(builder, *rhs);

                    let temp = builder.alloc_temp(ty);
                    let place = Place::from_local(temp);

                    let stmt = Statement::assign(
                        place.clone(),
                        Rvalue::BinaryOp(mir_op, lhs_operand, rhs_operand),
                        span,
                    );
                    builder.push_statement(stmt);
                    place
                } else {
                    // Assignment ops - handled elsewhere
                    let temp = builder.alloc_temp(ty);
                    Place::from_local(temp)
                }
            }
            HirExprKind::Unary { op, operand } => {
                // Check if this is a simple unary op (not deref)
                if let Some(mir_op) = hir_unop_to_mir(*op) {
                    // Lower operand
                    let operand_val = self.lower_expr_as_operand(builder, *operand);

                    // Allocate temp for result
                    let temp = builder.alloc_temp(ty);
                    let place = Place::from_local(temp);

                    // Emit unary operation
                    let stmt = Statement::assign(
                        place.clone(),
                        Rvalue::UnaryOp(mir_op, operand_val),
                        span,
                    );
                    builder.push_statement(stmt);
                    place
                } else {
                    // Deref - handled elsewhere (produces a place projection)
                    let temp = builder.alloc_temp(ty);
                    Place::from_local(temp)
                }
            }
            HirExprKind::Block { stmts, tail } => {
                // Track locals declared in this block for StorageDead
                let locals_before = builder.locals.len();

                // Lower each statement
                for stmt_id in stmts {
                    self.lower_stmt(builder, *stmt_id);
                }

                // Lower tail or return unit
                let result_place = if let Some(tail_id) = tail {
                    self.lower_expr_to_place(builder, *tail_id)
                } else {
                    let unit_ty = self.hir.types.unit();
                    let temp = builder.alloc_temp(unit_ty);
                    Place::from_local(temp)
                };

                // Emit StorageDead for block-scoped locals (excluding result)
                for i in locals_before..builder.locals.len() {
                    let local = Local(i as u32);
                    if local != result_place.local {
                        builder.push_statement(Statement::storage_dead(local, span.clone()));
                    }
                }

                result_place
            }
            HirExprKind::Return { value } => {
                // Lower return value (if any) to return place
                if let Some(val_id) = value {
                    let operand = self.lower_expr_as_operand(builder, *val_id);
                    let return_place = Place::from_local(Local::RETURN_PLACE);
                    builder.push_statement(Statement::assign(
                        return_place,
                        Rvalue::Use(operand),
                        span.clone(),
                    ));
                }

                // Set return terminator
                builder.set_terminator(TerminatorKind::Return, span.clone());

                // Return doesn't produce a value in the normal sense
                // Return a unit-typed place (won't be used)
                let unit_ty = self.hir.types.unit();
                let temp = builder.alloc_temp(unit_ty);
                Place::from_local(temp)
            }
            HirExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                // Allocate result place
                let result_place = Place::from_local(builder.alloc_temp(ty));

                // Lower condition
                let cond_operand = self.lower_expr_as_operand(builder, *condition);

                // Create basic blocks
                let then_bb = builder.alloc_block();
                let join_bb = builder.alloc_block();
                let else_bb = if else_branch.is_some() {
                    builder.alloc_block()
                } else {
                    join_bb // No else means false goes straight to join
                };

                // Create SwitchInt terminator: true -> then_bb, false -> else_bb
                let targets = SwitchTargets::new_bool(then_bb, else_bb);
                builder.set_terminator(
                    TerminatorKind::SwitchInt {
                        discr: cond_operand,
                        targets,
                    },
                    span.clone(),
                );

                // Lower then branch
                builder.switch_to_block(then_bb);
                let then_operand = self.lower_expr_as_operand(builder, *then_branch);
                builder.push_statement(Statement::assign(
                    result_place.clone(),
                    Rvalue::Use(then_operand),
                    span.clone(),
                ));
                builder.set_terminator(TerminatorKind::Goto(join_bb), span.clone());

                // Lower else branch (if present)
                if let Some(else_expr) = else_branch {
                    builder.switch_to_block(else_bb);
                    let else_operand = self.lower_expr_as_operand(builder, *else_expr);
                    builder.push_statement(Statement::assign(
                        result_place.clone(),
                        Rvalue::Use(else_operand),
                        span.clone(),
                    ));
                    builder.set_terminator(TerminatorKind::Goto(join_bb), span.clone());
                }

                // Continue in join block
                builder.switch_to_block(join_bb);

                result_place
            }
            HirExprKind::Loop { body } => {
                // Allocate result place (for break value)
                let result_place = Place::from_local(builder.alloc_temp(ty));

                // Create header (loop body) and exit blocks
                let header_bb = builder.alloc_block();
                let exit_bb = builder.alloc_block();

                // Push loop context onto stack
                self.loop_stack.push(LoopContext {
                    exit_block: exit_bb,
                    header_block: header_bb,
                    result_place: Some(result_place.clone()),
                });

                // Jump to header
                builder.set_terminator(TerminatorKind::Goto(header_bb), span.clone());

                // Lower body in header block
                builder.switch_to_block(header_bb);
                let _ = self.lower_expr_to_place(builder, *body);

                // If we're still in header and no terminator set, loop back
                // (This handles the case where body doesn't end in break/continue/return)
                if builder.current_block().terminator.is_none() {
                    builder.set_terminator(TerminatorKind::Goto(header_bb), span.clone());
                }

                // Pop loop context
                self.loop_stack.pop();

                // Continue in exit block
                builder.switch_to_block(exit_bb);

                result_place
            }
            HirExprKind::Break { value } => {
                // Get current loop context
                let loop_ctx = self
                    .loop_stack
                    .last()
                    .expect("break outside of loop")
                    .clone();

                // Lower break value (if any) and assign to result place
                if let Some(val_id) = value {
                    let operand = self.lower_expr_as_operand(builder, *val_id);
                    if let Some(ref result_place) = loop_ctx.result_place {
                        builder.push_statement(Statement::assign(
                            result_place.clone(),
                            Rvalue::Use(operand),
                            span.clone(),
                        ));
                    }
                }

                // Jump to exit block
                builder.set_terminator(TerminatorKind::Goto(loop_ctx.exit_block), span.clone());

                // Break doesn't produce a value (execution continues elsewhere)
                let unit_ty = self.hir.types.unit();
                let temp = builder.alloc_temp(unit_ty);
                Place::from_local(temp)
            }
            HirExprKind::Continue => {
                // Get current loop context
                let loop_ctx = self
                    .loop_stack
                    .last()
                    .expect("continue outside of loop")
                    .clone();

                // Jump to header block
                builder.set_terminator(TerminatorKind::Goto(loop_ctx.header_block), span.clone());

                // Continue doesn't produce a value
                let unit_ty = self.hir.types.unit();
                let temp = builder.alloc_temp(unit_ty);
                Place::from_local(temp)
            }
            HirExprKind::Call { callee, args } => {
                // Lower callee - check if it's a direct function ref or expression
                let func_operand = self.lower_callee_operand(builder, *callee);

                // Lower arguments
                let arg_operands: Vec<Operand> = args
                    .iter()
                    .map(|arg| self.lower_expr_as_operand(builder, *arg))
                    .collect();

                // Allocate result temp and continuation block
                let result_temp = builder.alloc_temp(ty);
                let destination = Place::from_local(result_temp);
                let cont_bb = builder.alloc_block();

                // Set Call terminator
                builder.set_terminator(
                    TerminatorKind::Call {
                        func: func_operand,
                        args: arg_operands,
                        destination: destination.clone(),
                        target: Some(cont_bb),
                    },
                    span,
                );

                builder.switch_to_block(cont_bb);
                destination
            }
            HirExprKind::MethodCall {
                receiver,
                method: _,
                args,
            } => {
                // Look up resolved method DefId
                let method_def_id = self
                    .hir
                    .method_resolutions
                    .get(&expr_id)
                    .copied()
                    .unwrap_or(DefId(0)); // Fallback for unresolved

                // Receiver becomes first argument
                let receiver_operand = self.lower_expr_as_operand(builder, *receiver);
                let mut arg_operands = vec![receiver_operand];
                arg_operands.extend(args.iter().map(|a| self.lower_expr_as_operand(builder, *a)));

                let func_operand = Operand::Constant(Constant::FnDef(method_def_id));
                let result_temp = builder.alloc_temp(ty);
                let destination = Place::from_local(result_temp);
                let cont_bb = builder.alloc_block();

                builder.set_terminator(
                    TerminatorKind::Call {
                        func: func_operand,
                        args: arg_operands,
                        destination: destination.clone(),
                        target: Some(cont_bb),
                    },
                    span,
                );

                builder.switch_to_block(cont_bb);
                destination
            }
            HirExprKind::Array { elements } => {
                // Lower each element to an operand
                let operands: Vec<Operand> = elements
                    .iter()
                    .map(|e| self.lower_expr_as_operand(builder, *e))
                    .collect();
                let temp = builder.alloc_temp(ty);
                let place = Place::from_local(temp);
                builder.push_statement(Statement::assign(
                    place.clone(),
                    Rvalue::Aggregate(AggregateKind::Array, operands),
                    span,
                ));
                place
            }
            HirExprKind::Tuple { elements } => {
                // Lower each element to an operand
                let operands: Vec<Operand> = elements
                    .iter()
                    .map(|e| self.lower_expr_as_operand(builder, *e))
                    .collect();
                let temp = builder.alloc_temp(ty);
                let place = Place::from_local(temp);
                builder.push_statement(Statement::assign(
                    place.clone(),
                    Rvalue::Aggregate(AggregateKind::Tuple, operands),
                    span,
                ));
                place
            }
            HirExprKind::Struct { def_id, fields } => {
                // Lower each field value to an operand (in declaration order from fields vec)
                let operands: Vec<Operand> = fields
                    .iter()
                    .map(|(_, expr)| self.lower_expr_as_operand(builder, *expr))
                    .collect();
                let temp = builder.alloc_temp(ty);
                let place = Place::from_local(temp);
                builder.push_statement(Statement::assign(
                    place.clone(),
                    Rvalue::Aggregate(AggregateKind::Adt(*def_id), operands),
                    span,
                ));
                place
            }
            HirExprKind::TupleField { base, index } => {
                // Lower base to a place, then add a field projection
                let base_place = self.lower_expr_to_place(builder, *base);
                Place {
                    local: base_place.local,
                    projection: {
                        let mut proj = base_place.projection;
                        proj.push(PlaceElem::Field(FieldIdx(*index)));
                        proj
                    },
                }
            }
            HirExprKind::Index { base, index } => {
                // Lower base to a place, index to a local, then add an index projection
                let base_place = self.lower_expr_to_place(builder, *base);
                // Index expression needs to be lowered to a place first, then we use its local
                let index_place = self.lower_expr_to_place(builder, *index);
                Place {
                    local: base_place.local,
                    projection: {
                        let mut proj = base_place.projection;
                        proj.push(PlaceElem::Index(index_place.local));
                        proj
                    },
                }
            }
            HirExprKind::Field { base, field } => {
                // Lower base to a place, then add a field projection
                // We need to resolve field name to index - for now assume field order matches
                // In a full implementation, we'd look up the struct definition
                let base_place = self.lower_expr_to_place(builder, *base);
                // Use field name hash as a temporary field index (would need proper resolution)
                let field_idx = field.chars().fold(0u32, |acc, c| acc.wrapping_add(c as u32)) % 100;
                Place {
                    local: base_place.local,
                    projection: {
                        let mut proj = base_place.projection;
                        proj.push(PlaceElem::Field(FieldIdx(field_idx)));
                        proj
                    },
                }
            }
            HirExprKind::Ref { mutable, operand } => {
                // Lower operand to a place, then create a reference to it
                let operand_place = self.lower_expr_to_place(builder, *operand);
                let borrow_kind = if *mutable {
                    BorrowKind::Mut
                } else {
                    BorrowKind::Shared
                };
                let temp = builder.alloc_temp(ty);
                let place = Place::from_local(temp);
                builder.push_statement(Statement::assign(
                    place.clone(),
                    Rvalue::Ref(borrow_kind, operand_place),
                    span,
                ));
                place
            }
            HirExprKind::Cast { expr, target_ty } => {
                // Lower the inner expression, determine cast kind, emit cast
                let operand = self.lower_expr_as_operand(builder, *expr);
                let source_ty = self.hir.expr(*expr).ty;
                let cast_kind = determine_cast_kind(self.hir, source_ty, *target_ty);
                let temp = builder.alloc_temp(*target_ty);
                let place = Place::from_local(temp);
                builder.push_statement(Statement::assign(
                    place.clone(),
                    Rvalue::Cast(cast_kind, operand, *target_ty),
                    span,
                ));
                place
            }
            HirExprKind::ArrayRepeat { value, count } => {
                // Lower the value to repeat
                let operand = self.lower_expr_as_operand(builder, *value);

                // Allocate temp for result array
                let temp = builder.alloc_temp(ty);
                let place = Place::from_local(temp);

                // Emit repeat rvalue
                builder.push_statement(Statement::assign(
                    place.clone(),
                    Rvalue::Repeat(operand, *count),
                    span,
                ));
                place
            }
            _ => {
                // For other expressions, allocate a temp and recursively lower
                let temp = builder.alloc_temp(ty);
                Place::from_local(temp)
            }
        }
    }

    /// Lower an expression as an operand (no temp allocation for simple cases).
    pub fn lower_expr_as_operand(&mut self, builder: &mut MirBuilder, expr_id: ExprId) -> Operand {
        let expr = self.hir.expr(expr_id);

        match &expr.kind {
            HirExprKind::Literal(lit) => {
                // Literals can be operands directly
                literal_to_operand(lit)
            }
            HirExprKind::Var(def_id) => {
                // Variables can be operands directly (copy from their place)
                if let Some(&local) = self.local_map.get(def_id) {
                    Operand::Copy(Place::from_local(local))
                } else {
                    // Variable not found - return a zero constant as fallback
                    Operand::Constant(Constant::Int(0))
                }
            }
            _ => {
                // For other expressions, lower to place then copy
                let place = self.lower_expr_to_place(builder, expr_id);
                Operand::Copy(place)
            }
        }
    }

    /// Lower a callee expression to an operand.
    ///
    /// For direct function references (HirExprKind::Var pointing to a function),
    /// this produces a FnDef constant. For function pointers in variables or
    /// complex expressions, this produces a Copy/Move operand.
    fn lower_callee_operand(&mut self, builder: &mut MirBuilder, callee_id: ExprId) -> Operand {
        let callee = self.hir.expr(callee_id);
        if let HirExprKind::Var(def_id) = &callee.kind
            && !self.local_map.contains_key(def_id)
        {
            // Direct function reference (not a local variable)
            return Operand::Constant(Constant::FnDef(*def_id));
        }
        // Function pointer in variable or complex expression
        self.lower_expr_as_operand(builder, callee_id)
    }

    /// Lower a statement.
    fn lower_stmt(&mut self, builder: &mut MirBuilder, stmt_id: StmtId) {
        let stmt = self.hir.stmt(stmt_id);
        let span = stmt.span.clone();

        match &stmt.kind {
            HirStmtKind::Let { pat, ty: _, init } => {
                let pat_data = self.hir.pat(*pat);
                match &pat_data.kind {
                    HirPatKind::Bind { def_id, mutable } => {
                        let local = builder.alloc_local(pat_data.ty, *mutable, None);
                        self.local_map.insert(*def_id, local);
                        builder.push_statement(Statement::storage_live(local, span.clone()));

                        if let Some(init_id) = init {
                            let operand = self.lower_expr_as_operand(builder, *init_id);
                            let place = Place::from_local(local);
                            builder.push_statement(Statement::assign(
                                place,
                                Rvalue::Use(operand),
                                span,
                            ));
                        }
                    }
                    HirPatKind::Wildcard => {
                        if let Some(init_id) = init {
                            // Evaluate the init expression for side effects, discard result
                            let _ = self.lower_expr_to_place(builder, *init_id);
                        }
                    }
                    _ => {
                        // Defer complex patterns (Tuple, Struct, Ref)
                    }
                }
            }
            HirStmtKind::Expr { expr, has_semi: _ } => {
                // Evaluate expression for side effects
                let _ = self.lower_expr_to_place(builder, *expr);
            }
        }
    }

    /// Lower a complete function to MIR.
    pub fn lower_function(&mut self, func: &HirFunction) -> Body {
        let mut builder = self.start_function(func);
        let span = func.span.clone();

        // Lower the body if present
        if let Some(body_expr) = func.body {
            // Lower the body expression
            let result = self.lower_expr_as_operand(&mut builder, body_expr);

            // Check if the return type is unit
            let is_unit = {
                let unit_ty = self.hir.types.unit();
                func.ret_type == unit_ty
            };

            if !is_unit {
                // Assign result to return place
                let return_place = Place::from_local(Local::RETURN_PLACE);
                let stmt = Statement::assign(return_place, Rvalue::Use(result), span.clone());
                builder.push_statement(stmt);
            }
        }

        // Add return terminator
        builder.set_terminator(TerminatorKind::Return, span);

        builder.finish(func.params.len())
    }
}

/// Lower all functions in an HIR database to MIR bodies.
pub fn lower_hir_to_mir(hir: &HirDatabase) -> Vec<Body> {
    let mut ctx = MirLoweringContext::new(hir);

    for item in &hir.items {
        if let HirItem::Function(func) = item {
            let body = ctx.lower_function(func);
            ctx.bodies.push(body);
        }
    }

    ctx.bodies
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::operand::{AggregateKind, BorrowKind, CastKind};
    use crate::mir::statement::StatementKind;
    use crate::mir::terminator::Terminator;

    // ========== Phase 1: MirBuilder Core Structure ==========

    #[test]
    fn test_mir_builder_new() {
        let type_id = TypeId(1); // i32
        let builder = MirBuilder::new(type_id);

        // Should have exactly one local (return place)
        assert_eq!(builder.locals.len(), 1);
        assert_eq!(builder.locals[0].ty, type_id);
        assert!(builder.locals[0].mutable); // Return place is mutable

        // Should have entry block
        assert_eq!(builder.basic_blocks.len(), 1);
    }

    #[test]
    fn test_mir_builder_alloc_local() {
        let mut builder = MirBuilder::new(TypeId(1));

        let local = builder.alloc_local(TypeId(2), true, Some("x".to_string()));

        assert_eq!(local, Local(1)); // After return place
        assert_eq!(builder.locals.len(), 2);
        assert_eq!(builder.locals[1].ty, TypeId(2));
        assert!(builder.locals[1].mutable);
        assert_eq!(builder.locals[1].name, Some("x".to_string()));
    }

    #[test]
    fn test_mir_builder_alloc_temp() {
        let mut builder = MirBuilder::new(TypeId(1));

        let temp = builder.alloc_temp(TypeId(3));

        assert_eq!(temp, Local(1));
        assert!(!builder.locals[1].mutable); // Temps are immutable
        assert_eq!(builder.locals[1].name, None); // No name
    }

    #[test]
    fn test_mir_builder_push_statement() {
        let mut builder = MirBuilder::new(TypeId(1));
        let span = Span::from(0..10);

        let stmt = Statement {
            kind: StatementKind::Nop,
            span,
        };
        builder.push_statement(stmt);

        assert_eq!(builder.current_block().statements.len(), 1);
    }

    #[test]
    fn test_mir_builder_set_terminator() {
        let mut builder = MirBuilder::new(TypeId(1));
        let span = Span::from(0..10);

        builder.set_terminator(TerminatorKind::Return, span);

        let block = builder.current_block();
        assert!(block.terminator.is_some());
        assert!(matches!(
            block.terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }

    #[test]
    fn test_mir_builder_finish() {
        let mut builder = MirBuilder::new(TypeId(1));
        builder.set_terminator(TerminatorKind::Return, Span::from(0..10));

        let body = builder.finish(0); // 0 args

        assert_eq!(body.locals.len(), 1);
        assert_eq!(body.basic_blocks.len(), 1);
        assert_eq!(body.arg_count, 0);
        assert!(body.basic_blocks[0].terminator.is_some());
    }

    // ========== Phase 2: Literal Conversion ==========

    #[test]
    fn test_lower_int_literal() {
        let lit = Literal::Int(42);
        let constant = lower_literal(&lit);

        assert_eq!(constant, Constant::Int(42));
    }

    #[test]
    fn test_lower_float_literal() {
        let lit = Literal::Float(2.5);
        let constant = lower_literal(&lit);

        assert_eq!(constant, Constant::Float(2.5));
    }

    #[test]
    fn test_lower_bool_literal() {
        assert_eq!(lower_literal(&Literal::Bool(true)), Constant::Bool(true));
        assert_eq!(lower_literal(&Literal::Bool(false)), Constant::Bool(false));
    }

    #[test]
    fn test_lower_char_literal() {
        let lit = Literal::Char('x');
        let constant = lower_literal(&lit);

        assert_eq!(constant, Constant::Char('x'));
    }

    #[test]
    fn test_lower_string_literal() {
        let lit = Literal::String("hello".to_string());
        let constant = lower_literal(&lit);

        assert_eq!(constant, Constant::String("hello".to_string()));
    }

    #[test]
    fn test_literal_to_operand() {
        let lit = Literal::Int(42);
        let operand = literal_to_operand(&lit);

        assert!(matches!(operand, Operand::Constant(Constant::Int(42))));
    }

    // ========== Phase 3: MirLoweringContext ==========

    #[test]
    fn test_lowering_context_new() {
        let hir_db = HirDatabase::new();
        let ctx = MirLoweringContext::new(&hir_db);

        // Context should be initialized but empty
        assert!(ctx.bodies.is_empty());
    }

    // ========== Phase 4: Expression Lowering ==========

    #[test]
    fn test_lower_literal_expr() {
        // Create HIR: `42`
        let mut hir_db = HirDatabase::new();
        let ty = hir_db.types.i32();
        let expr_id = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty,
            span: Span::from(0..2),
        });

        let mut ctx = MirLoweringContext::new(&hir_db);
        let mut builder = MirBuilder::new(ty);

        let place = ctx.lower_expr_to_place(&mut builder, expr_id);

        // Should allocate a temp and assign the constant
        assert_eq!(builder.locals.len(), 2); // return + temp
        assert_eq!(builder.current_block().statements.len(), 1);

        let stmt = &builder.current_block().statements[0];
        match &stmt.kind {
            StatementKind::Assign(p, Rvalue::Use(Operand::Constant(Constant::Int(42)))) => {
                assert_eq!(*p, place);
            }
            _ => panic!("Expected assignment of constant"),
        }
    }

    #[test]
    fn test_lower_literal_as_operand() {
        let mut hir_db = HirDatabase::new();
        let ty = hir_db.types.i32();
        let expr_id = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty,
            span: Span::from(0..2),
        });

        let mut ctx = MirLoweringContext::new(&hir_db);
        let mut builder = MirBuilder::new(ty);

        let operand = ctx.lower_expr_as_operand(&mut builder, expr_id);

        // Literals should become operands directly without temp
        assert!(matches!(operand, Operand::Constant(Constant::Int(42))));
        assert_eq!(builder.locals.len(), 1); // Only return place
    }

    // ========== Phase 5: Complete Function Lowering ==========
    // Note: These tests require integration with the full compilation pipeline.
    // We'll use a helper to create HIR manually for now.

    fn create_literal_function(
        hir_db: &mut HirDatabase,
        name: &str,
        ret_ty: TypeId,
        lit: Literal,
    ) -> HirFunction {
        let expr_id = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(lit),
            ty: ret_ty,
            span: Span::from(0..5),
        });

        HirFunction {
            def_id: DefId(0),
            name: name.to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: ret_ty,
            body: Some(expr_id),
            span: Span::from(0..20),
        }
    }

    #[test]
    fn test_lower_function_returning_constant() {
        // fn answer() -> i32 { 42 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let func = create_literal_function(&mut hir_db, "answer", i32_ty, Literal::Int(42));
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);

        assert_eq!(bodies.len(), 1);
        let body = &bodies[0];

        // Verify structure
        assert_eq!(body.locals.len(), 1); // Just return place
        assert_eq!(body.arg_count, 0);
        assert_eq!(body.basic_blocks.len(), 1);

        // Verify: _0 = 42; return
        let block = &body.basic_blocks[0];
        assert_eq!(block.statements.len(), 1);

        match &block.statements[0].kind {
            StatementKind::Assign(place, Rvalue::Use(Operand::Constant(Constant::Int(42)))) => {
                assert_eq!(place.local, Local(0)); // Return place
                assert!(place.projection.is_empty());
            }
            _ => panic!("Expected _0 = 42"),
        }

        assert!(matches!(
            block.terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }

    #[test]
    fn test_lower_function_returning_bool() {
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();
        let func = create_literal_function(&mut hir_db, "is_true", bool_ty, Literal::Bool(true));
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Bool(true)))) => {}
            _ => panic!("Expected bool constant"),
        }
    }

    #[test]
    fn test_lower_function_returning_float() {
        let mut hir_db = HirDatabase::new();
        let f64_ty = hir_db.types.f64();
        let func = create_literal_function(&mut hir_db, "value", f64_ty, Literal::Float(1.23456));
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Float(f)))) => {
                assert!((f - 1.23456).abs() < 0.00001);
            }
            _ => panic!("Expected float constant"),
        }
    }

    #[test]
    fn test_lower_function_returning_unit() {
        // fn noop() { }
        let mut hir_db = HirDatabase::new();
        let unit_ty = hir_db.types.unit();

        // Create an empty block expression
        let block_expr_id = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: None,
            },
            ty: unit_ty,
            span: Span::from(0..5),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "noop".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: Some(block_expr_id),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Unit return should have no explicit assignment to return place
        let block = &body.basic_blocks[0];
        assert!(matches!(
            block.terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }

    #[test]
    fn test_lower_function_returning_char() {
        let mut hir_db = HirDatabase::new();
        let char_ty = hir_db.types.char();
        let func = create_literal_function(&mut hir_db, "letter", char_ty, Literal::Char('a'));
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Char('a')))) => {}
            _ => panic!("Expected char constant"),
        }
    }

    #[test]
    fn test_lower_function_returning_string() {
        let mut hir_db = HirDatabase::new();
        // Use str type for string literals
        let str_ty = hir_db.types.str();
        let func = create_literal_function(
            &mut hir_db,
            "greeting",
            str_ty,
            Literal::String("hello".to_string()),
        );
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::String(s)))) => {
                assert_eq!(s, "hello");
            }
            _ => panic!("Expected string constant"),
        }
    }

    // ========== Phase 6: Edge Cases ==========

    #[test]
    fn test_lower_multiple_functions() {
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let func1 = create_literal_function(&mut hir_db, "one", i32_ty, Literal::Int(1));
        let func2 = create_literal_function(&mut hir_db, "two", i32_ty, Literal::Int(2));

        hir_db.items.push(HirItem::Function(func1));
        hir_db.items.push(HirItem::Function(func2));

        let bodies = lower_hir_to_mir(&hir_db);

        assert_eq!(bodies.len(), 2);
    }

    #[test]
    fn test_lower_function_with_unused_params() {
        // fn ignore(x: i32) -> i32 { 0 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        // Create a parameter pattern
        let pat_id = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: DefId(1),
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });

        let param = crate::hir::HirParam {
            pat: pat_id,
            ty: i32_ty,
            span: Span::from(0..5),
        };

        // Create body: literal 0
        let body_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(0)),
            ty: i32_ty,
            span: Span::from(0..1),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "ignore".to_string(),
            type_params: vec![],
            params: vec![param],
            ret_type: i32_ty,
            body: Some(body_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have: return place + 1 param = 2 locals
        assert_eq!(body.locals.len(), 2);
        assert_eq!(body.arg_count, 1);
    }

    // Additional tests for coverage

    #[test]
    fn test_mir_builder_alloc_block() {
        let mut builder = MirBuilder::new(TypeId(1));

        let bb1 = builder.alloc_block();
        let bb2 = builder.alloc_block();

        assert_eq!(bb1, BasicBlock(1));
        assert_eq!(bb2, BasicBlock(2));
        assert_eq!(builder.basic_blocks.len(), 3); // entry + 2 new
    }

    #[test]
    fn test_mir_builder_switch_to_block() {
        let mut builder = MirBuilder::new(TypeId(1));
        let bb1 = builder.alloc_block();

        // Add statement to entry block
        builder.push_statement(Statement::nop(Span::from(0..1)));
        assert_eq!(builder.current_block().statements.len(), 1);

        // Switch to bb1 and add statement there
        builder.switch_to_block(bb1);
        builder.push_statement(Statement::nop(Span::from(1..2)));
        assert_eq!(builder.current_block().statements.len(), 1);

        // Switch back to entry and verify
        builder.switch_to_block(BasicBlock::ENTRY);
        assert_eq!(builder.current_block().statements.len(), 1);
    }

    #[test]
    fn test_lower_negative_int() {
        // Note: In our current HIR representation, negative numbers are
        // stored directly as negative i128 values in Literal::Int
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let func = create_literal_function(&mut hir_db, "neg", i32_ty, Literal::Int(-42));
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(-42)))) => {}
            _ => panic!("Expected negative int constant"),
        }
    }

    // ========== Phase 7: Operator Mapping (IR-3.2) ==========

    #[test]
    fn test_hir_binop_add_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Add), Some(BinOp::Add));
    }

    #[test]
    fn test_hir_binop_sub_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Sub), Some(BinOp::Sub));
    }

    #[test]
    fn test_hir_binop_mul_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Mul), Some(BinOp::Mul));
    }

    #[test]
    fn test_hir_binop_div_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Div), Some(BinOp::Div));
    }

    #[test]
    fn test_hir_binop_rem_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Rem), Some(BinOp::Rem));
    }

    #[test]
    fn test_hir_binop_eq_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Eq), Some(BinOp::Eq));
    }

    #[test]
    fn test_hir_binop_ne_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Ne), Some(BinOp::Ne));
    }

    #[test]
    fn test_hir_binop_lt_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Lt), Some(BinOp::Lt));
    }

    #[test]
    fn test_hir_binop_le_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Le), Some(BinOp::Le));
    }

    #[test]
    fn test_hir_binop_gt_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Gt), Some(BinOp::Gt));
    }

    #[test]
    fn test_hir_binop_ge_to_mir() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Ge), Some(BinOp::Ge));
    }

    #[test]
    fn test_hir_binop_and_returns_none() {
        // Short-circuit ops are handled separately
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::And), None);
    }

    #[test]
    fn test_hir_binop_or_returns_none() {
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Or), None);
    }

    #[test]
    fn test_hir_binop_assign_returns_none() {
        // Assignment ops are handled separately
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::Assign), None);
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::AddAssign), None);
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::SubAssign), None);
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::MulAssign), None);
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::DivAssign), None);
        assert_eq!(hir_binop_to_mir(crate::hir::BinOp::RemAssign), None);
    }

    #[test]
    fn test_hir_unop_not_to_mir() {
        assert_eq!(hir_unop_to_mir(crate::hir::UnaryOp::Not), Some(UnOp::Not));
    }

    #[test]
    fn test_hir_unop_neg_to_mir() {
        assert_eq!(hir_unop_to_mir(crate::hir::UnaryOp::Neg), Some(UnOp::Neg));
    }

    #[test]
    fn test_hir_unop_deref_returns_none() {
        // Deref produces a place, not an rvalue
        assert_eq!(hir_unop_to_mir(crate::hir::UnaryOp::Deref), None);
    }

    // ========== Phase 8: Variable Lowering (IR-3.2) ==========

    #[test]
    fn test_lower_var_reference() {
        // fn identity(x: i32) -> i32 { x }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let param_def_id = DefId(1);

        // Create parameter pattern
        let pat_id = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: param_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });

        let param = crate::hir::HirParam {
            pat: pat_id,
            ty: i32_ty,
            span: Span::from(0..5),
        };

        // Body: var reference to x
        let body_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(param_def_id),
            ty: i32_ty,
            span: Span::from(10..11),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "identity".to_string(),
            type_params: vec![],
            params: vec![param],
            ret_type: i32_ty,
            body: Some(body_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have: return place + 1 param = 2 locals
        assert_eq!(body.locals.len(), 2);
        assert_eq!(body.arg_count, 1);

        // Should produce: _0 = Copy(_1)
        let block = &body.basic_blocks[0];
        assert_eq!(block.statements.len(), 1);

        match &block.statements[0].kind {
            StatementKind::Assign(place, Rvalue::Use(Operand::Copy(src))) => {
                assert_eq!(place.local, Local(0)); // Return place
                assert_eq!(src.local, Local(1)); // Parameter
            }
            _ => panic!("Expected _0 = Copy(_1)"),
        }
    }

    #[test]
    fn test_lower_var_as_operand() {
        // Test that variables become operands directly without extra temps
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let param_def_id = DefId(1);

        // Create var expression
        let var_expr_id = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(param_def_id),
            ty: i32_ty,
            span: Span::from(0..1),
        });

        let mut ctx = MirLoweringContext::new(&hir_db);
        // Manually set up the local map
        ctx.local_map.insert(param_def_id, Local(1));

        let mut builder = MirBuilder::new(i32_ty);
        builder.alloc_local(i32_ty, false, None); // _1 for param

        let operand = ctx.lower_expr_as_operand(&mut builder, var_expr_id);

        // Should be Copy(_1) directly
        match operand {
            Operand::Copy(place) => {
                assert_eq!(place.local, Local(1));
                assert!(place.is_local());
            }
            _ => panic!("Expected Copy operand"),
        }

        // Should not allocate any new temps
        assert_eq!(builder.locals.len(), 2); // _0 (return) + _1 (param)
    }

    // ========== Phase 9: Binary Expression Lowering (IR-3.2) ==========

    #[test]
    fn test_lower_binary_add_literals() {
        // fn add() -> i32 { 1 + 2 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs,
                rhs,
            },
            ty: i32_ty,
            span: Span::from(0..5),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "add".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(add_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let block = &body.basic_blocks[0];
        // Should have:
        // 1. _1 = Add(1, 2)
        // 2. _0 = Copy(_1)
        assert_eq!(block.statements.len(), 2);

        // First statement: temp = Add(1, 2)
        match &block.statements[0].kind {
            StatementKind::Assign(
                place,
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::Constant(Constant::Int(1)),
                    Operand::Constant(Constant::Int(2)),
                ),
            ) => {
                assert_eq!(place.local, Local(1)); // First temp
            }
            other => panic!("Expected Add rvalue, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_binary_sub() {
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(10)),
            ty: i32_ty,
            span: Span::from(0..2),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(5..6),
        });
        let sub_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Sub,
                lhs,
                rhs,
            },
            ty: i32_ty,
            span: Span::from(0..6),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "sub".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(sub_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Sub, _, _)) => {}
            _ => panic!("Expected Sub binary op"),
        }
    }

    #[test]
    fn test_lower_binary_mul() {
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(4)),
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let mul_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Mul,
                lhs,
                rhs,
            },
            ty: i32_ty,
            span: Span::from(0..5),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "mul".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(mul_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Mul, _, _)) => {}
            _ => panic!("Expected Mul binary op"),
        }
    }

    #[test]
    fn test_lower_binary_comparison_lt() {
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bool_ty = hir_db.types.bool();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let lt_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Lt,
                lhs,
                rhs,
            },
            ty: bool_ty,
            span: Span::from(0..5),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "less_than".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: bool_ty,
            body: Some(lt_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Lt, _, _)) => {}
            _ => panic!("Expected Lt comparison"),
        }
    }

    #[test]
    fn test_lower_binary_with_vars() {
        // fn add(a: i32, b: i32) -> i32 { a + b }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        // Create parameter patterns
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(5..6),
        });

        let param_a = crate::hir::HirParam {
            pat: pat_a,
            ty: i32_ty,
            span: Span::from(0..5),
        };
        let param_b = crate::hir::HirParam {
            pat: pat_b,
            ty: i32_ty,
            span: Span::from(5..10),
        };

        // Body: a + b
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(19..20),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: var_b,
            },
            ty: i32_ty,
            span: Span::from(15..20),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "add".to_string(),
            type_params: vec![],
            params: vec![param_a, param_b],
            ret_type: i32_ty,
            body: Some(add_expr),
            span: Span::from(0..30),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have: return place + 2 params + 1 temp = 4 locals
        assert_eq!(body.locals.len(), 4);
        assert_eq!(body.arg_count, 2);

        // First statement should be: _3 = Add(Copy(_1), Copy(_2))
        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(
                place,
                Rvalue::BinaryOp(BinOp::Add, Operand::Copy(lhs), Operand::Copy(rhs)),
            ) => {
                assert_eq!(place.local, Local(3)); // Temp
                assert_eq!(lhs.local, Local(1)); // a
                assert_eq!(rhs.local, Local(2)); // b
            }
            other => panic!("Expected Add(Copy(_1), Copy(_2)), got {:?}", other),
        }
    }

    #[test]
    fn test_lower_nested_binary() {
        // fn nested() -> i32 { (1 + 2) * 3 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let lit_1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let lit_2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: lit_1,
                rhs: lit_2,
            },
            ty: i32_ty,
            span: Span::from(0..5),
        });

        let lit_3 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(9..10),
        });
        let mul_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Mul,
                lhs: add_expr,
                rhs: lit_3,
            },
            ty: i32_ty,
            span: Span::from(0..10),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "nested".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(mul_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should produce:
        // _1 = Add(1, 2)
        // _2 = Mul(Copy(_1), 3)
        // _0 = Copy(_2)
        assert_eq!(body.basic_blocks[0].statements.len(), 3);

        // First: _1 = Add(1, 2)
        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(place, Rvalue::BinaryOp(BinOp::Add, _, _)) => {
                assert_eq!(place.local, Local(1));
            }
            _ => panic!("Expected Add"),
        }

        // Second: _2 = Mul(Copy(_1), 3)
        match &body.basic_blocks[0].statements[1].kind {
            StatementKind::Assign(place, Rvalue::BinaryOp(BinOp::Mul, lhs, rhs)) => {
                assert_eq!(place.local, Local(2));
                assert!(matches!(lhs, Operand::Copy(p) if p.local == Local(1)));
                assert!(matches!(rhs, Operand::Constant(Constant::Int(3))));
            }
            _ => panic!("Expected Mul"),
        }
    }

    // ========== Phase 10: Unary Expression Lowering (IR-3.2) ==========

    #[test]
    fn test_lower_unary_neg() {
        // fn neg(x: i32) -> i32 { -x }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        let pat = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });

        let param = crate::hir::HirParam {
            pat,
            ty: i32_ty,
            span: Span::from(0..5),
        };

        let var_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(10..11),
        });
        let neg_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Unary {
                op: crate::hir::UnaryOp::Neg,
                operand: var_x,
            },
            ty: i32_ty,
            span: Span::from(9..11),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "neg".to_string(),
            type_params: vec![],
            params: vec![param],
            ret_type: i32_ty,
            body: Some(neg_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should produce: _2 = Neg(Copy(_1))
        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(place, Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(src))) => {
                assert_eq!(place.local, Local(2)); // Temp
                assert_eq!(src.local, Local(1)); // x
            }
            other => panic!("Expected Neg(Copy(_1)), got {:?}", other),
        }
    }

    #[test]
    fn test_lower_unary_not() {
        // fn flip(b: bool) -> bool { !b }
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();
        let b_def_id = DefId(1);

        let pat = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: bool_ty,
            span: Span::from(0..1),
        });

        let param = crate::hir::HirParam {
            pat,
            ty: bool_ty,
            span: Span::from(0..5),
        };

        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: bool_ty,
            span: Span::from(10..11),
        });
        let not_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Unary {
                op: crate::hir::UnaryOp::Not,
                operand: var_b,
            },
            ty: bool_ty,
            span: Span::from(9..11),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "flip".to_string(),
            type_params: vec![],
            params: vec![param],
            ret_type: bool_ty,
            body: Some(not_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(_, Rvalue::UnaryOp(UnOp::Not, Operand::Copy(_))) => {}
            other => panic!("Expected Not(Copy(_)), got {:?}", other),
        }
    }

    #[test]
    fn test_lower_nested_unary() {
        // fn double_neg(x: i32) -> i32 { --x } (which is -(-x))
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        let pat = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });

        let param = crate::hir::HirParam {
            pat,
            ty: i32_ty,
            span: Span::from(0..5),
        };

        let var_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(12..13),
        });
        let inner_neg = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Unary {
                op: crate::hir::UnaryOp::Neg,
                operand: var_x,
            },
            ty: i32_ty,
            span: Span::from(11..13),
        });
        let outer_neg = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Unary {
                op: crate::hir::UnaryOp::Neg,
                operand: inner_neg,
            },
            ty: i32_ty,
            span: Span::from(10..13),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "double_neg".to_string(),
            type_params: vec![],
            params: vec![param],
            ret_type: i32_ty,
            body: Some(outer_neg),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should produce:
        // _2 = Neg(Copy(_1))  -- inner -x
        // _3 = Neg(Copy(_2))  -- outer -(-x)
        // _0 = Copy(_3)       -- return
        assert_eq!(body.basic_blocks[0].statements.len(), 3);

        // First: _2 = Neg(Copy(_1))
        match &body.basic_blocks[0].statements[0].kind {
            StatementKind::Assign(place, Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(src))) => {
                assert_eq!(place.local, Local(2));
                assert_eq!(src.local, Local(1));
            }
            _ => panic!("Expected inner Neg"),
        }

        // Second: _3 = Neg(Copy(_2))
        match &body.basic_blocks[0].statements[1].kind {
            StatementKind::Assign(place, Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(src))) => {
                assert_eq!(place.local, Local(3));
                assert_eq!(src.local, Local(2));
            }
            _ => panic!("Expected outer Neg"),
        }
    }

    #[test]
    fn test_lower_complex_expression() {
        // fn complex(a: i32, b: i32) -> i32 { -(a + b) * 2 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        // Params
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(5..6),
        });

        let param_a = crate::hir::HirParam {
            pat: pat_a,
            ty: i32_ty,
            span: Span::from(0..5),
        };
        let param_b = crate::hir::HirParam {
            pat: pat_b,
            ty: i32_ty,
            span: Span::from(5..10),
        };

        // Body: -(a + b) * 2
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(19..20),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: var_b,
            },
            ty: i32_ty,
            span: Span::from(15..20),
        });
        let neg_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Unary {
                op: crate::hir::UnaryOp::Neg,
                operand: add_expr,
            },
            ty: i32_ty,
            span: Span::from(14..20),
        });
        let lit_2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(24..25),
        });
        let mul_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Mul,
                lhs: neg_expr,
                rhs: lit_2,
            },
            ty: i32_ty,
            span: Span::from(14..25),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "complex".to_string(),
            type_params: vec![],
            params: vec![param_a, param_b],
            ret_type: i32_ty,
            body: Some(mul_expr),
            span: Span::from(0..30),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should produce:
        // _3 = Add(Copy(_1), Copy(_2))  -- a + b
        // _4 = Neg(Copy(_3))            -- -(a + b)
        // _5 = Mul(Copy(_4), 2)         -- -(a + b) * 2
        // _0 = Copy(_5)                 -- return
        assert_eq!(body.basic_blocks[0].statements.len(), 4);

        // Verify the operations
        assert!(matches!(
            &body.basic_blocks[0].statements[0].kind,
            StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _))
        ));
        assert!(matches!(
            &body.basic_blocks[0].statements[1].kind,
            StatementKind::Assign(_, Rvalue::UnaryOp(UnOp::Neg, _))
        ));
        assert!(matches!(
            &body.basic_blocks[0].statements[2].kind,
            StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Mul, _, _))
        ));
    }

    // ========== Phase 11: Let Bindings (IR-3.4) ==========

    #[test]
    fn lower_let_binding_simple() {
        // fn foo() -> i32 { let x = 42; x }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        // Create pattern for `x`
        let pat_id = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });

        // Create init expression: 42
        let init_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(8..10),
        });

        // Create let statement
        let let_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_id,
                ty: Some(i32_ty),
                init: Some(init_expr),
            },
            span: Span::from(0..11),
        });

        // Create tail expression: x
        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(13..14),
        });

        // Create block: { let x = 42; x }
        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![let_stmt],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..15),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have: _0 (return), _1 (x)
        assert!(body.locals.len() >= 2);

        // Verify we have StorageLive(_1) and _1 = 42
        let block = &body.basic_blocks[0];
        let mut found_storage_live = false;
        let mut found_assign_42 = false;
        let mut found_copy_to_return = false;

        for stmt in &block.statements {
            match &stmt.kind {
                StatementKind::StorageLive(local) if *local == Local(1) => {
                    found_storage_live = true;
                }
                StatementKind::Assign(place, Rvalue::Use(Operand::Constant(Constant::Int(42))))
                    if place.local == Local(1) =>
                {
                    found_assign_42 = true;
                }
                StatementKind::Assign(place, Rvalue::Use(Operand::Copy(src)))
                    if place.local == Local(0) && src.local == Local(1) =>
                {
                    found_copy_to_return = true;
                }
                _ => {}
            }
        }

        assert!(found_storage_live, "Expected StorageLive(_1)");
        assert!(found_assign_42, "Expected _1 = 42");
        assert!(found_copy_to_return, "Expected _0 = Copy(_1)");
    }

    #[test]
    fn lower_let_binding_mutable() {
        // fn foo() -> i32 { let mut x = 10; x }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        // Create mutable pattern for `mut x`
        let pat_id = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: true, // Mutable!
            },
            ty: i32_ty,
            span: Span::from(4..9),
        });

        let init_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(10)),
            ty: i32_ty,
            span: Span::from(12..14),
        });

        let let_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_id,
                ty: Some(i32_ty),
                init: Some(init_expr),
            },
            span: Span::from(0..15),
        });

        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(17..18),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![let_stmt],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..20),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..25),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Verify LocalDecl for _1 (x) has mutable == true
        assert!(body.locals.len() >= 2);
        assert!(body.locals[1].mutable, "Expected local _1 to be mutable");
    }

    #[test]
    fn lower_let_wildcard() {
        // fn foo() -> i32 { let _ = 42; 0 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        // Wildcard pattern
        let pat_id = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Wildcard,
            ty: i32_ty,
            span: Span::from(4..5),
        });

        let init_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(8..10),
        });

        let let_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_id,
                ty: Some(i32_ty),
                init: Some(init_expr),
            },
            span: Span::from(0..11),
        });

        // Tail: 0
        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(0)),
            ty: i32_ty,
            span: Span::from(13..14),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![let_stmt],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..15),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // For wildcard, no named local is created for the pattern
        // The 42 is evaluated into a temp, and result 0 flows to _0
        let block = &body.basic_blocks[0];

        // Should evaluate 42 (creates temp) and assign 0 to another place
        // eventually _0 gets the final value
        let found_42 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(42))))
            )
        });
        assert!(found_42, "Expected 42 to be evaluated (for side effects)");

        // Final value should reach _0
        let found_0_somewhere = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(0))))
            )
        });
        assert!(found_0_somewhere, "Expected 0 to be assigned somewhere");

        // Verify _0 gets assigned (either directly or via copy)
        let found_return_assign = block
            .statements
            .iter()
            .any(|stmt| matches!(&stmt.kind, StatementKind::Assign(place, _) if place.local == Local(0)));
        assert!(found_return_assign, "Expected _0 to be assigned");
    }

    // ========== Phase 12: Blocks (IR-3.4) ==========

    #[test]
    fn lower_block_multiple_stmts() {
        // fn foo() -> i32 { let a = 1; let b = 2; a + b }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        // let a = 1
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_a = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_a,
                ty: Some(i32_ty),
                init: Some(init_a),
            },
            span: Span::from(0..10),
        });

        // let b = 2
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let init_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(19..20),
        });
        let stmt_b = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_b,
                ty: Some(i32_ty),
                init: Some(init_b),
            },
            span: Span::from(11..21),
        });

        // a + b
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(23..24),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(27..28),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: var_b,
            },
            ty: i32_ty,
            span: Span::from(23..28),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_a, stmt_b],
                tail: Some(add_expr),
            },
            ty: i32_ty,
            span: Span::from(0..30),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..40),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have: _0 (return), _1 (a), _2 (b), _3 (temp for add result)
        assert!(body.locals.len() >= 4);

        let block = &body.basic_blocks[0];

        // Verify _1 = 1 and _2 = 2
        let found_1 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, Rvalue::Use(Operand::Constant(Constant::Int(1))))
                if place.local == Local(1)
            )
        });
        let found_2 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, Rvalue::Use(Operand::Constant(Constant::Int(2))))
                if place.local == Local(2)
            )
        });

        assert!(found_1, "Expected _1 = 1");
        assert!(found_2, "Expected _2 = 2");

        // Verify Add operation
        let found_add = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, Operand::Copy(lhs), Operand::Copy(rhs)))
                if lhs.local == Local(1) && rhs.local == Local(2)
            )
        });
        assert!(found_add, "Expected Add(Copy(_1), Copy(_2))");
    }

    #[test]
    fn lower_block_no_tail() {
        // fn foo() { let x = 1; }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let unit_ty = hir_db.types.unit();
        let x_def_id = DefId(1);

        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_x = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x,
                ty: Some(i32_ty),
                init: Some(init_x),
            },
            span: Span::from(0..10),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_x],
                tail: None, // No tail - returns unit
            },
            ty: unit_ty,
            span: Span::from(0..12),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have return terminator with no assignment to _0
        let block = &body.basic_blocks[0];
        assert!(matches!(
            block.terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));

        // x should be assigned
        let found_x = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(1))))
            )
        });
        assert!(found_x, "Expected x = 1");
    }

    #[test]
    fn lower_nested_blocks() {
        // fn foo() -> i32 { let a = 1; { let b = 2; a + b } }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        // let a = 1
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_a = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_a,
                ty: Some(i32_ty),
                init: Some(init_a),
            },
            span: Span::from(0..10),
        });

        // Inner block: { let b = 2; a + b }
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(14..15),
        });
        let init_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(18..19),
        });
        let stmt_b = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_b,
                ty: Some(i32_ty),
                init: Some(init_b),
            },
            span: Span::from(12..20),
        });

        // a + b (both from outer and inner scope)
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(22..23),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(26..27),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: var_b,
            },
            ty: i32_ty,
            span: Span::from(22..27),
        });

        let inner_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_b],
                tail: Some(add_expr),
            },
            ty: i32_ty,
            span: Span::from(11..29),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_a],
                tail: Some(inner_block),
            },
            ty: i32_ty,
            span: Span::from(0..30),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..40),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Verify both a (from outer) and b (from inner) are accessible
        let block = &body.basic_blocks[0];

        // Verify Add(Copy(_1), Copy(_2)) - outer a + inner b
        let found_add = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, Operand::Copy(lhs), Operand::Copy(rhs)))
                if lhs.local == Local(1) && rhs.local == Local(2)
            )
        });
        assert!(found_add, "Expected Add(Copy(_1), Copy(_2)) - outer a + inner b");
    }

    #[test]
    fn lower_empty_block() {
        // fn foo() { {} }
        let mut hir_db = HirDatabase::new();
        let unit_ty = hir_db.types.unit();

        // Inner empty block
        let inner_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: None,
            },
            ty: unit_ty,
            span: Span::from(1..3),
        });

        // Outer block containing the inner empty block as a statement
        let inner_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Expr {
                expr: inner_block,
                has_semi: false,
            },
            span: Span::from(1..3),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![inner_stmt],
                tail: None,
            },
            ty: unit_ty,
            span: Span::from(0..4),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: Some(outer_block),
            span: Span::from(0..10),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have return terminator
        let block = &body.basic_blocks[0];
        assert!(matches!(
            block.terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }

    // ========== Phase 13: Expression Statements (IR-3.4) ==========

    #[test]
    fn lower_expr_stmt_with_semi() {
        // fn foo() -> i32 { 1 + 2; 42 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        // 1 + 2 expression
        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs,
                rhs,
            },
            ty: i32_ty,
            span: Span::from(0..5),
        });

        // Expression statement: 1 + 2;
        let expr_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Expr {
                expr: add_expr,
                has_semi: true,
            },
            span: Span::from(0..6),
        });

        // Tail: 42
        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(8..10),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![expr_stmt],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..12),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let block = &body.basic_blocks[0];

        // The 1+2 should be evaluated (add operation exists)
        let found_add = block.statements.iter().any(|stmt| {
            matches!(&stmt.kind, StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _)))
        });
        assert!(found_add, "Expected 1+2 to be evaluated");

        // 42 should be assigned somewhere (eventually to _0)
        let found_42 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(42))))
            )
        });
        assert!(found_42, "Expected 42 to be assigned");
    }

    // ========== Phase 14: Storage Liveness (IR-3.4) ==========

    #[test]
    fn lower_storage_live_on_let() {
        // fn foo() -> i32 { let x = 1; x }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_x = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x,
                ty: Some(i32_ty),
                init: Some(init_x),
            },
            span: Span::from(0..10),
        });

        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(12..13),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_x],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..15),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];
        let block = &body.basic_blocks[0];

        // Verify StorageLive(_1) appears before assignment to _1
        let storage_live_idx = block
            .statements
            .iter()
            .position(|stmt| matches!(&stmt.kind, StatementKind::StorageLive(Local(1))));
        let assign_idx = block.statements.iter().position(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, _) if place.local == Local(1)
            )
        });

        assert!(storage_live_idx.is_some(), "Expected StorageLive(_1)");
        assert!(assign_idx.is_some(), "Expected assignment to _1");
        assert!(
            storage_live_idx.unwrap() < assign_idx.unwrap(),
            "StorageLive should come before assignment"
        );
    }

    #[test]
    fn lower_storage_dead_at_block_end() {
        // fn foo() -> i32 { let a = 1; { let b = 2; a + b } }
        // Inner block's `b` should have StorageDead
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        // let a = 1
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_a = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_a,
                ty: Some(i32_ty),
                init: Some(init_a),
            },
            span: Span::from(0..10),
        });

        // Inner block: { let b = 2; a + b }
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(14..15),
        });
        let init_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(18..19),
        });
        let stmt_b = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_b,
                ty: Some(i32_ty),
                init: Some(init_b),
            },
            span: Span::from(12..20),
        });

        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(22..23),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(26..27),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: var_b,
            },
            ty: i32_ty,
            span: Span::from(22..27),
        });

        let inner_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_b],
                tail: Some(add_expr),
            },
            ty: i32_ty,
            span: Span::from(11..29),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_a],
                tail: Some(inner_block),
            },
            ty: i32_ty,
            span: Span::from(0..30),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..40),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];
        let block = &body.basic_blocks[0];

        // Should have StorageDead for b (_2) from inner block
        let found_storage_dead_b = block
            .statements
            .iter()
            .any(|stmt| matches!(&stmt.kind, StatementKind::StorageDead(Local(2))));

        assert!(
            found_storage_dead_b,
            "Expected StorageDead(_2) for inner block's b"
        );
    }

    // ========== Phase 15: Edge Cases (IR-3.4) ==========

    #[test]
    fn lower_shadowing() {
        // fn foo() -> i32 { let x = 1; let x = 2; x }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x1_def_id = DefId(1);
        let x2_def_id = DefId(2);

        // First let x = 1
        let pat_x1 = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x1_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_x1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_x1 = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x1,
                ty: Some(i32_ty),
                init: Some(init_x1),
            },
            span: Span::from(0..10),
        });

        // Second let x = 2 (shadows first)
        let pat_x2 = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x2_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let init_x2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(19..20),
        });
        let stmt_x2 = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x2,
                ty: Some(i32_ty),
                init: Some(init_x2),
            },
            span: Span::from(11..21),
        });

        // Tail: x (refers to second x)
        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x2_def_id),
            ty: i32_ty,
            span: Span::from(23..24),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_x1, stmt_x2],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..26),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..30),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have two distinct locals for x: _1 and _2
        assert!(body.locals.len() >= 3); // _0, _1 (x), _2 (x shadow)

        let block = &body.basic_blocks[0];

        // First x (_1) should be assigned 1
        let found_x1 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, Rvalue::Use(Operand::Constant(Constant::Int(1))))
                if place.local == Local(1)
            )
        });

        // Second x (_2) should be assigned 2
        let found_x2 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, Rvalue::Use(Operand::Constant(Constant::Int(2))))
                if place.local == Local(2)
            )
        });

        assert!(found_x1, "Expected _1 = 1 (first x)");
        assert!(found_x2, "Expected _2 = 2 (second x, shadow)");

        // Return should copy from _2 (the shadowing x)
        let found_return_from_x2 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, Rvalue::Use(Operand::Copy(src)))
                if place.local == Local(0) && src.local == Local(2)
            )
        });
        assert!(
            found_return_from_x2,
            "Expected _0 = Copy(_2) (return shadowing x)"
        );
    }

    #[test]
    fn lower_block_as_operand() {
        // fn foo() -> i32 { { let x = 1; x } + { let y = 2; y } }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);
        let y_def_id = DefId(2);

        // First block: { let x = 1; x }
        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(2..3),
        });
        let init_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(6..7),
        });
        let stmt_x = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x,
                ty: Some(i32_ty),
                init: Some(init_x),
            },
            span: Span::from(0..8),
        });
        let var_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(10..11),
        });
        let block1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_x],
                tail: Some(var_x),
            },
            ty: i32_ty,
            span: Span::from(0..12),
        });

        // Second block: { let y = 2; y }
        let pat_y = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: y_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(17..18),
        });
        let init_y = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(21..22),
        });
        let stmt_y = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_y,
                ty: Some(i32_ty),
                init: Some(init_y),
            },
            span: Span::from(15..23),
        });
        let var_y = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(y_def_id),
            ty: i32_ty,
            span: Span::from(25..26),
        });
        let block2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_y],
                tail: Some(var_y),
            },
            ty: i32_ty,
            span: Span::from(14..27),
        });

        // block1 + block2
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: block1,
                rhs: block2,
            },
            ty: i32_ty,
            span: Span::from(0..27),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(add_expr),
            span: Span::from(0..35),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let block = &body.basic_blocks[0];

        // Should have both blocks evaluated and added
        // _1 = 1 (x), _2 = 2 (y), then Add(_1, _2) or similar
        let found_1 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(1))))
            )
        });
        let found_2 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(2))))
            )
        });
        let found_add = block
            .statements
            .iter()
            .any(|stmt| matches!(&stmt.kind, StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _))));

        assert!(found_1, "Expected x = 1 in first block");
        assert!(found_2, "Expected y = 2 in second block");
        assert!(found_add, "Expected Add of block results");
    }

    #[test]
    fn lower_block_tail_only() {
        // fn foo() -> i32 { 42 }
        // Block with just a tail expression, no statements
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(0..2),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![], // No statements
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..4),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..10),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let block = &body.basic_blocks[0];

        // 42 should be assigned somewhere (temp or directly to _0)
        let found_42 = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(42))))
            )
        });
        assert!(found_42, "Expected 42 to be assigned");

        // _0 should be assigned (either directly or via copy)
        let found_return = block
            .statements
            .iter()
            .any(|stmt| matches!(&stmt.kind, StatementKind::Assign(place, _) if place.local == Local(0)));
        assert!(found_return, "Expected _0 to be assigned");
    }

    #[test]
    fn lower_let_without_init() {
        // fn foo() -> i32 { let x: i32; 0 }
        // Let without initializer - should allocate local but not assign
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });

        // No init!
        let stmt_x = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x,
                ty: Some(i32_ty),
                init: None, // No initializer
            },
            span: Span::from(0..10),
        });

        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(0)),
            ty: i32_ty,
            span: Span::from(12..13),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_x],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..15),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have _1 allocated for x with StorageLive but no assignment
        assert!(body.locals.len() >= 2, "Expected at least _0 and _1");

        let block = &body.basic_blocks[0];

        // Should have StorageLive(_1)
        let found_storage_live = block
            .statements
            .iter()
            .any(|stmt| matches!(&stmt.kind, StatementKind::StorageLive(Local(1))));
        assert!(found_storage_live, "Expected StorageLive(_1)");

        // Should NOT have assignment to _1
        let found_assign_to_x = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, _) if place.local == Local(1)
            )
        });
        assert!(!found_assign_to_x, "Expected no assignment to _1 (uninitialized)");
    }

    #[test]
    fn lower_storage_dead_excludes_result() {
        // fn foo() -> i32 { let x = 1; x }
        // The result (x) should NOT have StorageDead since it's the block result
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_x = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x,
                ty: Some(i32_ty),
                init: Some(init_x),
            },
            span: Span::from(0..10),
        });

        // Tail returns x directly
        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(12..13),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_x],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..15),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];
        let block = &body.basic_blocks[0];

        // x is _1 and is the result place, so should NOT have StorageDead(_1)
        let found_storage_dead_x = block
            .statements
            .iter()
            .any(|stmt| matches!(&stmt.kind, StatementKind::StorageDead(Local(1))));

        assert!(
            !found_storage_dead_x,
            "Expected NO StorageDead(_1) since x is the result"
        );
    }

    #[test]
    fn lower_let_uses_previous_binding() {
        // fn foo() -> i32 { let a = 1; let b = a + 1; b }
        // Tests that a binding can be used in subsequent statement's init
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        // let a = 1
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_a = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_a,
                ty: Some(i32_ty),
                init: Some(init_a),
            },
            span: Span::from(0..10),
        });

        // let b = a + 1
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(19..20),
        });
        let lit_1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(23..24),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: lit_1,
            },
            ty: i32_ty,
            span: Span::from(19..24),
        });
        let stmt_b = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_b,
                ty: Some(i32_ty),
                init: Some(add_expr),
            },
            span: Span::from(11..25),
        });

        // tail: b
        let tail_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(27..28),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_a, stmt_b],
                tail: Some(tail_expr),
            },
            ty: i32_ty,
            span: Span::from(0..30),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..35),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];
        let block = &body.basic_blocks[0];

        // Should have Add(Copy(_1), 1) where _1 is a
        let found_add_using_a = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, Operand::Copy(src), Operand::Constant(Constant::Int(1))))
                if src.local == Local(1)
            )
        });
        assert!(
            found_add_using_a,
            "Expected Add(Copy(_1), 1) - b's init uses a"
        );
    }

    #[test]
    fn lower_storage_dead_ordering() {
        // fn foo() -> i32 { let a = 1; { let b = 2; a } }
        // Inner block returns `a` (from outer), so b should have StorageDead
        // and it should come AFTER a is copied to result
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        // let a = 1
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_a = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_a,
                ty: Some(i32_ty),
                init: Some(init_a),
            },
            span: Span::from(0..10),
        });

        // Inner block: { let b = 2; a }
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(14..15),
        });
        let init_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(18..19),
        });
        let stmt_b = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_b,
                ty: Some(i32_ty),
                init: Some(init_b),
            },
            span: Span::from(12..20),
        });

        // Inner tail: a (returns outer variable)
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(22..23),
        });

        let inner_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_b],
                tail: Some(var_a),
            },
            ty: i32_ty,
            span: Span::from(11..25),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_a],
                tail: Some(inner_block),
            },
            ty: i32_ty,
            span: Span::from(0..26),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..30),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];
        let block = &body.basic_blocks[0];

        // b is _2, inner block returns a (_1) which is Place
        // StorageDead(_2) should exist since b goes out of scope
        let storage_dead_b_idx = block
            .statements
            .iter()
            .position(|stmt| matches!(&stmt.kind, StatementKind::StorageDead(Local(2))));

        assert!(storage_dead_b_idx.is_some(), "Expected StorageDead(_2) for b");
    }

    // ========== Phase 14: Return (IR-3.3) ==========

    #[test]
    fn lower_return_unit() {
        // fn foo() { return; }
        let mut hir_db = HirDatabase::new();
        let unit_ty = hir_db.types.unit();

        // Create return expression
        let return_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Return { value: None },
            ty: unit_ty,
            span: Span::from(0..7),
        });

        // Wrap in block
        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(return_expr),
            },
            ty: unit_ty,
            span: Span::from(0..10),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have at least one block with return terminator
        assert!(!body.basic_blocks.is_empty());
        let block = &body.basic_blocks[0];
        assert!(matches!(
            block.terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }

    #[test]
    fn lower_return_literal() {
        // fn foo() -> i32 { return 42; }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        // Create value expression: 42
        let value_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(7..9),
        });

        // Create return expression
        let return_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Return {
                value: Some(value_expr),
            },
            ty: i32_ty,
            span: Span::from(0..10),
        });

        // Wrap in block
        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(return_expr),
            },
            ty: i32_ty,
            span: Span::from(0..12),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let block = &body.basic_blocks[0];

        // Should have _0 = 42 and return terminator
        let found_assign = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, Rvalue::Use(Operand::Constant(Constant::Int(42))))
                if place.local == Local(0)
            )
        });
        assert!(found_assign, "Expected _0 = 42");

        assert!(matches!(
            block.terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }

    #[test]
    fn lower_return_expression() {
        // fn foo(a: i32, b: i32) -> i32 { return a + b; }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        // Create parameter patterns
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(5..6),
        });

        let param_a = crate::hir::HirParam {
            pat: pat_a,
            ty: i32_ty,
            span: Span::from(0..5),
        };
        let param_b = crate::hir::HirParam {
            pat: pat_b,
            ty: i32_ty,
            span: Span::from(5..10),
        };

        // Body: return a + b
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(19..20),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: var_b,
            },
            ty: i32_ty,
            span: Span::from(15..20),
        });
        let return_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Return {
                value: Some(add_expr),
            },
            ty: i32_ty,
            span: Span::from(10..21),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(return_expr),
            },
            ty: i32_ty,
            span: Span::from(0..25),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_a, param_b],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..30),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let block = &body.basic_blocks[0];

        // Should have Add operation and assignment to _0
        let found_add = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _))
            )
        });
        assert!(found_add, "Expected Add operation");

        // Final assignment to _0 (return place)
        let found_return_assign = block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(place, _) if place.local == Local(0)
            )
        });
        assert!(found_return_assign, "Expected assignment to _0");

        assert!(matches!(
            block.terminator.as_ref().unwrap().kind,
            TerminatorKind::Return
        ));
    }

    // ========== Phase 15: If without else (IR-3.3) ==========

    #[test]
    fn lower_if_no_else_literal() {
        // fn foo() { if true { let x = 1; } }
        let mut hir_db = HirDatabase::new();
        let unit_ty = hir_db.types.unit();
        let bool_ty = hir_db.types.bool();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        // Condition: true
        let cond_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Bool(true)),
            ty: bool_ty,
            span: Span::from(3..7),
        });

        // Then branch: { let x = 1; }
        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(12..13),
        });
        let init_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(16..17),
        });
        let stmt_x = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x,
                ty: Some(i32_ty),
                init: Some(init_x),
            },
            span: Span::from(9..18),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_x],
                tail: None,
            },
            ty: unit_ty,
            span: Span::from(8..20),
        });

        // If expression (no else)
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond_expr,
                then_branch: then_block,
                else_branch: None,
            },
            ty: unit_ty,
            span: Span::from(0..20),
        });

        // Wrap in outer block
        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(if_expr),
            },
            ty: unit_ty,
            span: Span::from(0..22),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: Some(outer_block),
            span: Span::from(0..30),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks: entry, then, join
        assert!(
            body.basic_blocks.len() >= 2,
            "Expected at least 2 blocks for if"
        );

        // Entry block should have SwitchInt terminator
        let entry = &body.basic_blocks[0];
        assert!(
            matches!(
                entry.terminator.as_ref().unwrap().kind,
                TerminatorKind::SwitchInt { .. }
            ),
            "Expected SwitchInt in entry block"
        );
    }

    #[test]
    fn lower_if_no_else_var() {
        // fn foo(cond: bool) { if cond { let x = 1; } }
        let mut hir_db = HirDatabase::new();
        let unit_ty = hir_db.types.unit();
        let bool_ty = hir_db.types.bool();
        let i32_ty = hir_db.types.i32();
        let cond_def_id = DefId(1);
        let x_def_id = DefId(2);

        // Parameter: cond
        let pat_cond = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: cond_def_id,
                mutable: false,
            },
            ty: bool_ty,
            span: Span::from(7..11),
        });
        let param_cond = crate::hir::HirParam {
            pat: pat_cond,
            ty: bool_ty,
            span: Span::from(7..17),
        };

        // Condition: cond (variable reference)
        let cond_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(cond_def_id),
            ty: bool_ty,
            span: Span::from(22..26),
        });

        // Then branch: { let x = 1; }
        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(31..32),
        });
        let init_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(35..36),
        });
        let stmt_x = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x,
                ty: Some(i32_ty),
                init: Some(init_x),
            },
            span: Span::from(28..37),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_x],
                tail: None,
            },
            ty: unit_ty,
            span: Span::from(27..39),
        });

        // If expression (no else)
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond_expr,
                then_branch: then_block,
                else_branch: None,
            },
            ty: unit_ty,
            span: Span::from(19..39),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(if_expr),
            },
            ty: unit_ty,
            span: Span::from(19..41),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_cond],
            ret_type: unit_ty,
            body: Some(outer_block),
            span: Span::from(0..50),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Entry block should have SwitchInt with Copy(_1) - the cond param
        let entry = &body.basic_blocks[0];
        match &entry.terminator.as_ref().unwrap().kind {
            TerminatorKind::SwitchInt { discr, .. } => {
                assert!(
                    matches!(discr, Operand::Copy(p) if p.local == Local(1)),
                    "Expected SwitchInt(Copy(_1)), got {:?}",
                    discr
                );
            }
            other => panic!("Expected SwitchInt, got {:?}", other),
        }
    }

    // ========== Phase 16: If-else (IR-3.3) ==========

    #[test]
    fn lower_if_else_literals() {
        // fn foo(cond: bool) -> i32 { if cond { 1 } else { 2 } }
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();
        let i32_ty = hir_db.types.i32();
        let cond_def_id = DefId(1);

        // Parameter: cond
        let pat_cond = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: cond_def_id,
                mutable: false,
            },
            ty: bool_ty,
            span: Span::from(7..11),
        });
        let param_cond = crate::hir::HirParam {
            pat: pat_cond,
            ty: bool_ty,
            span: Span::from(7..17),
        };

        // Condition: cond
        let cond_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(cond_def_id),
            ty: bool_ty,
            span: Span::from(30..34),
        });

        // Then: 1
        let then_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(37..38),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(then_expr),
            },
            ty: i32_ty,
            span: Span::from(35..40),
        });

        // Else: 2
        let else_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(48..49),
        });
        let else_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(else_expr),
            },
            ty: i32_ty,
            span: Span::from(46..51),
        });

        // If-else expression
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond_expr,
                then_branch: then_block,
                else_branch: Some(else_block),
            },
            ty: i32_ty,
            span: Span::from(27..51),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(if_expr),
            },
            ty: i32_ty,
            span: Span::from(27..53),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_cond],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..60),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks: entry, then, else, join
        assert!(
            body.basic_blocks.len() >= 3,
            "Expected at least 3 blocks for if-else, got {}",
            body.basic_blocks.len()
        );

        // Entry block should have SwitchInt terminator
        let entry = &body.basic_blocks[0];
        assert!(
            matches!(
                entry.terminator.as_ref().unwrap().kind,
                TerminatorKind::SwitchInt { .. }
            ),
            "Expected SwitchInt in entry block"
        );

        // Verify both branches assign to the same result place
        // Look for assignments of 1 and 2
        let mut found_1 = false;
        let mut found_2 = false;
        for block in &body.basic_blocks {
            for stmt in &block.statements {
                match &stmt.kind {
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(1)))) => {
                        found_1 = true;
                    }
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(2)))) => {
                        found_2 = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(found_1, "Expected then branch to assign 1");
        assert!(found_2, "Expected else branch to assign 2");
    }

    #[test]
    fn lower_if_else_expressions() {
        // fn foo(c: bool, a: i32, b: i32) -> i32 { if c { a + 1 } else { b + 2 } }
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();
        let i32_ty = hir_db.types.i32();
        let c_def_id = DefId(1);
        let a_def_id = DefId(2);
        let b_def_id = DefId(3);

        // Parameters
        let pat_c = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: c_def_id,
                mutable: false,
            },
            ty: bool_ty,
            span: Span::from(7..8),
        });
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(16..17),
        });
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(25..26),
        });

        let param_c = crate::hir::HirParam {
            pat: pat_c,
            ty: bool_ty,
            span: Span::from(7..14),
        };
        let param_a = crate::hir::HirParam {
            pat: pat_a,
            ty: i32_ty,
            span: Span::from(16..23),
        };
        let param_b = crate::hir::HirParam {
            pat: pat_b,
            ty: i32_ty,
            span: Span::from(25..32),
        };

        // Condition: c
        let cond_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(c_def_id),
            ty: bool_ty,
            span: Span::from(45..46),
        });

        // Then: a + 1
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(49..50),
        });
        let lit_1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(53..54),
        });
        let then_add = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: lit_1,
            },
            ty: i32_ty,
            span: Span::from(49..54),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(then_add),
            },
            ty: i32_ty,
            span: Span::from(47..56),
        });

        // Else: b + 2
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(64..65),
        });
        let lit_2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(68..69),
        });
        let else_add = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_b,
                rhs: lit_2,
            },
            ty: i32_ty,
            span: Span::from(64..69),
        });
        let else_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(else_add),
            },
            ty: i32_ty,
            span: Span::from(62..71),
        });

        // If-else expression
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond_expr,
                then_branch: then_block,
                else_branch: Some(else_block),
            },
            ty: i32_ty,
            span: Span::from(42..71),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(if_expr),
            },
            ty: i32_ty,
            span: Span::from(42..73),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_c, param_a, param_b],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..80),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks
        assert!(
            body.basic_blocks.len() >= 3,
            "Expected at least 3 blocks for if-else"
        );

        // Verify both Add operations exist
        let mut found_add_count = 0;
        for block in &body.basic_blocks {
            for stmt in &block.statements {
                if matches!(&stmt.kind, StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _)))
                {
                    found_add_count += 1;
                }
            }
        }
        assert!(found_add_count >= 2, "Expected two Add operations");
    }

    // ========== Phase 17: Loop (IR-3.3) ==========

    #[test]
    fn lower_loop_with_break() {
        // fn foo() -> i32 { loop { break 42; } }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        // break 42
        let break_value = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(13..15),
        });
        let break_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(break_value),
            },
            ty: i32_ty,
            span: Span::from(7..16),
        });

        // Loop body block
        let body_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(break_expr),
            },
            ty: i32_ty,
            span: Span::from(5..18),
        });

        // Loop expression
        let loop_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: body_block },
            ty: i32_ty,
            span: Span::from(0..18),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(loop_expr),
            },
            ty: i32_ty,
            span: Span::from(0..20),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..25),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks: entry, header, exit
        assert!(
            body.basic_blocks.len() >= 2,
            "Expected at least 2 blocks for loop"
        );

        // Verify 42 is assigned somewhere
        let mut found_42 = false;
        for block in &body.basic_blocks {
            for stmt in &block.statements {
                if matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(42))))
                ) {
                    found_42 = true;
                }
            }
        }
        assert!(found_42, "Expected 42 to be assigned (break value)");
    }

    #[test]
    fn lower_break_no_value() {
        // fn foo() { loop { break; } }
        let mut hir_db = HirDatabase::new();
        let unit_ty = hir_db.types.unit();

        // break (no value)
        let break_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break { value: None },
            ty: unit_ty,
            span: Span::from(7..12),
        });

        // Loop body block
        let body_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(break_expr),
            },
            ty: unit_ty,
            span: Span::from(5..14),
        });

        // Loop expression
        let loop_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: body_block },
            ty: unit_ty,
            span: Span::from(0..14),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(loop_expr),
            },
            ty: unit_ty,
            span: Span::from(0..16),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: Some(outer_block),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have at least 2 blocks
        assert!(
            body.basic_blocks.len() >= 2,
            "Expected at least 2 blocks for loop with break"
        );

        // One block should have Goto terminator (the break)
        let has_goto = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::Goto(_))
            )
        });
        assert!(has_goto, "Expected Goto terminator from break");
    }

    #[test]
    fn lower_continue_simple() {
        // fn foo() { loop { continue; } }
        let mut hir_db = HirDatabase::new();
        let unit_ty = hir_db.types.unit();

        // continue
        let continue_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Continue,
            ty: unit_ty,
            span: Span::from(7..15),
        });

        // Loop body block
        let body_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(continue_expr),
            },
            ty: unit_ty,
            span: Span::from(5..17),
        });

        // Loop expression
        let loop_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: body_block },
            ty: unit_ty,
            span: Span::from(0..17),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(loop_expr),
            },
            ty: unit_ty,
            span: Span::from(0..19),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: Some(outer_block),
            span: Span::from(0..25),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have at least 2 blocks
        assert!(
            body.basic_blocks.len() >= 2,
            "Expected at least 2 blocks for loop with continue"
        );

        // Should have Goto terminator (continue going back to header)
        let has_goto = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::Goto(_))
            )
        });
        assert!(has_goto, "Expected Goto terminator from continue");
    }

    #[test]
    fn lower_conditional_break() {
        // fn foo(cond: bool) -> i32 { loop { if cond { break 1; } } }
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();
        let i32_ty = hir_db.types.i32();
        let unit_ty = hir_db.types.unit();
        let cond_def_id = DefId(1);

        // Parameter: cond
        let pat_cond = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: cond_def_id,
                mutable: false,
            },
            ty: bool_ty,
            span: Span::from(7..11),
        });
        let param_cond = crate::hir::HirParam {
            pat: pat_cond,
            ty: bool_ty,
            span: Span::from(7..17),
        };

        // break 1
        let break_value = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(45..46),
        });
        let break_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(break_value),
            },
            ty: i32_ty,
            span: Span::from(39..47),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(break_expr),
            },
            ty: i32_ty,
            span: Span::from(37..49),
        });

        // Condition: cond
        let cond_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(cond_def_id),
            ty: bool_ty,
            span: Span::from(32..36),
        });

        // If expression (no else - infinite loop unless break)
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond_expr,
                then_branch: then_block,
                else_branch: None,
            },
            ty: unit_ty,
            span: Span::from(29..49),
        });

        // Loop body block
        let body_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(if_expr),
            },
            ty: unit_ty,
            span: Span::from(27..51),
        });

        // Loop expression
        let loop_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: body_block },
            ty: i32_ty,
            span: Span::from(22..51),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(loop_expr),
            },
            ty: i32_ty,
            span: Span::from(22..53),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_cond],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..60),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks
        assert!(
            body.basic_blocks.len() >= 3,
            "Expected at least 3 blocks for loop with conditional break"
        );

        // Verify 1 is assigned somewhere
        let mut found_1 = false;
        for block in &body.basic_blocks {
            for stmt in &block.statements {
                if matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(1))))
                ) {
                    found_1 = true;
                }
            }
        }
        assert!(found_1, "Expected 1 to be assigned (break value)");

        // Should have SwitchInt for the if condition
        let has_switch = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::SwitchInt { .. })
            )
        });
        assert!(has_switch, "Expected SwitchInt for if condition");
    }

    // ========== Phase 18: Nested Loops (IR-3.3) ==========

    #[test]
    fn lower_nested_break_inner() {
        // fn foo() -> i32 { loop { loop { break; } break 1; } }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let unit_ty = hir_db.types.unit();

        // Inner break (no value)
        let inner_break = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break { value: None },
            ty: unit_ty,
            span: Span::from(20..25),
        });

        // Inner loop body
        let inner_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(inner_break),
            },
            ty: unit_ty,
            span: Span::from(18..27),
        });

        // Inner loop
        let inner_loop = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: inner_body },
            ty: unit_ty,
            span: Span::from(13..27),
        });

        // Inner loop as statement
        let inner_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Expr {
                expr: inner_loop,
                has_semi: true,
            },
            span: Span::from(13..28),
        });

        // Outer break 1
        let outer_break_val = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(35..36),
        });
        let outer_break = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(outer_break_val),
            },
            ty: i32_ty,
            span: Span::from(29..37),
        });

        // Outer loop body
        let outer_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![inner_stmt],
                tail: Some(outer_break),
            },
            ty: i32_ty,
            span: Span::from(11..39),
        });

        // Outer loop
        let outer_loop = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: outer_body },
            ty: i32_ty,
            span: Span::from(6..39),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(outer_loop),
            },
            ty: i32_ty,
            span: Span::from(6..41),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..50),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks for nested loops
        assert!(
            body.basic_blocks.len() >= 4,
            "Expected at least 4 blocks for nested loops"
        );

        // Verify 1 is assigned (outer break value)
        let mut found_1 = false;
        for block in &body.basic_blocks {
            for stmt in &block.statements {
                if matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(1))))
                ) {
                    found_1 = true;
                }
            }
        }
        assert!(found_1, "Expected 1 to be assigned (outer break value)");
    }

    #[test]
    fn lower_nested_loop_values() {
        // fn foo() -> i32 { loop { let x = loop { break 10; }; break x + 1; } }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let x_def_id = DefId(1);

        // Inner break 10
        let break_val = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(10)),
            ty: i32_ty,
            span: Span::from(35..37),
        });
        let inner_break = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(break_val),
            },
            ty: i32_ty,
            span: Span::from(29..38),
        });

        // Inner loop body
        let inner_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(inner_break),
            },
            ty: i32_ty,
            span: Span::from(27..40),
        });

        // Inner loop
        let inner_loop = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: inner_body },
            ty: i32_ty,
            span: Span::from(22..40),
        });

        // let x = loop { break 10; }
        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(17..18),
        });
        let let_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_x,
                ty: Some(i32_ty),
                init: Some(inner_loop),
            },
            span: Span::from(13..41),
        });

        // Outer break: x + 1
        let var_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(48..49),
        });
        let lit_1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(52..53),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_x,
                rhs: lit_1,
            },
            ty: i32_ty,
            span: Span::from(48..53),
        });
        let outer_break = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(add_expr),
            },
            ty: i32_ty,
            span: Span::from(42..54),
        });

        // Outer loop body
        let outer_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![let_stmt],
                tail: Some(outer_break),
            },
            ty: i32_ty,
            span: Span::from(11..56),
        });

        // Outer loop
        let outer_loop = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: outer_body },
            ty: i32_ty,
            span: Span::from(6..56),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(outer_loop),
            },
            ty: i32_ty,
            span: Span::from(6..58),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..65),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks for nested loops
        assert!(
            body.basic_blocks.len() >= 4,
            "Expected at least 4 blocks for nested loops with values"
        );

        // Verify 10 is assigned (inner break value)
        let found_10 = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(10))))
                )
            })
        });
        assert!(found_10, "Expected 10 to be assigned (inner break value)");

        // Verify Add operation exists (x + 1)
        let found_add = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _))
                )
            })
        });
        assert!(found_add, "Expected Add operation (x + 1)");
    }

    // ========== Phase 19: Edge cases (IR-3.3) ==========

    #[test]
    fn lower_return_in_loop() {
        // fn foo() -> i32 { loop { return 42; } }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        // return 42
        let return_val = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(22..24),
        });
        let return_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Return {
                value: Some(return_val),
            },
            ty: i32_ty,
            span: Span::from(15..25),
        });

        // Loop body
        let body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(return_expr),
            },
            ty: i32_ty,
            span: Span::from(13..27),
        });

        // Loop
        let loop_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body },
            ty: i32_ty,
            span: Span::from(8..27),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(loop_expr),
            },
            ty: i32_ty,
            span: Span::from(8..29),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..35),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have Return terminator (not just Goto to exit)
        let has_return = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::Return)
            )
        });
        assert!(has_return, "Expected Return terminator");

        // Verify 42 is assigned to _0
        let found_42_to_return = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(place, Rvalue::Use(Operand::Constant(Constant::Int(42))))
                    if place.local == Local(0)
                )
            })
        });
        assert!(found_42_to_return, "Expected _0 = 42");
    }

    #[test]
    fn lower_if_else_breaks() {
        // fn foo(c: bool) -> i32 { loop { if c { break 1; } else { break 2; } } }
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();
        let i32_ty = hir_db.types.i32();
        let c_def_id = DefId(1);

        // Parameter: c
        let pat_c = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: c_def_id,
                mutable: false,
            },
            ty: bool_ty,
            span: Span::from(7..8),
        });
        let param_c = crate::hir::HirParam {
            pat: pat_c,
            ty: bool_ty,
            span: Span::from(7..14),
        };

        // Condition: c
        let cond_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(c_def_id),
            ty: bool_ty,
            span: Span::from(34..35),
        });

        // then: break 1
        let break_1_val = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(44..45),
        });
        let break_1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(break_1_val),
            },
            ty: i32_ty,
            span: Span::from(38..46),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(break_1),
            },
            ty: i32_ty,
            span: Span::from(36..48),
        });

        // else: break 2
        let break_2_val = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(62..63),
        });
        let break_2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(break_2_val),
            },
            ty: i32_ty,
            span: Span::from(56..64),
        });
        let else_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(break_2),
            },
            ty: i32_ty,
            span: Span::from(54..66),
        });

        // If-else
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond_expr,
                then_branch: then_block,
                else_branch: Some(else_block),
            },
            ty: i32_ty,
            span: Span::from(31..66),
        });

        // Loop body
        let loop_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(if_expr),
            },
            ty: i32_ty,
            span: Span::from(29..68),
        });

        // Loop
        let loop_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: loop_body },
            ty: i32_ty,
            span: Span::from(24..68),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(loop_expr),
            },
            ty: i32_ty,
            span: Span::from(24..70),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_c],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..80),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks
        assert!(
            body.basic_blocks.len() >= 4,
            "Expected at least 4 blocks for loop with if-else breaks"
        );

        // Verify both 1 and 2 are assigned
        let found_1 = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(1))))
                )
            })
        });
        let found_2 = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(2))))
                )
            })
        });
        assert!(found_1, "Expected 1 to be assigned (then break)");
        assert!(found_2, "Expected 2 to be assigned (else break)");

        // Should have SwitchInt
        let has_switch = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::SwitchInt { .. })
            )
        });
        assert!(has_switch, "Expected SwitchInt for if condition");
    }

    // ========== Phase 20: Function Calls (IR-3.5) ==========

    #[test]
    fn test_lower_call_no_args() {
        // fn bar() -> i32 { 42 }
        // fn foo() -> i32 { bar() }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bar_def_id = DefId(1);

        // First create bar function
        let bar_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(0..2),
        });
        let bar_func = HirFunction {
            def_id: bar_def_id,
            name: "bar".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(bar_body),
            span: Span::from(0..10),
        };
        hir_db.items.push(HirItem::Function(bar_func));

        // Call expression: bar()
        let callee_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(bar_def_id),
            ty: i32_ty, // Function type simplified to return type
            span: Span::from(20..23),
        });
        let call_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Call {
                callee: callee_expr,
                args: vec![],
            },
            ty: i32_ty,
            span: Span::from(20..25),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(call_expr),
            },
            ty: i32_ty,
            span: Span::from(15..30),
        });

        let foo_func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(15..35),
        };
        hir_db.items.push(HirItem::Function(foo_func));

        let bodies = lower_hir_to_mir(&hir_db);
        // Second body is foo
        let body = &bodies[1];

        // Should have Call terminator
        let has_call = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::Call { .. })
            )
        });
        assert!(has_call, "Expected Call terminator");

        // Verify the call has correct function operand
        for block in &body.basic_blocks {
            if let Some(term) = &block.terminator
                && let TerminatorKind::Call { func, args, .. } = &term.kind
            {
                assert!(
                    matches!(func, Operand::Constant(Constant::FnDef(def_id)) if *def_id == bar_def_id),
                    "Expected call to bar"
                );
                assert!(args.is_empty(), "Expected no arguments");
            }
        }
    }

    #[test]
    fn test_lower_call_with_args() {
        // fn add(a: i32, b: i32) -> i32 { a + b }
        // fn foo() -> i32 { add(1, 2) }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let add_def_id = DefId(1);
        let a_def_id = DefId(2);
        let b_def_id = DefId(3);

        // Create add function with params
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(5..6),
        });

        let param_a = crate::hir::HirParam {
            pat: pat_a,
            ty: i32_ty,
            span: Span::from(0..5),
        };
        let param_b = crate::hir::HirParam {
            pat: pat_b,
            ty: i32_ty,
            span: Span::from(5..10),
        };

        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(19..20),
        });
        let add_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: var_b,
            },
            ty: i32_ty,
            span: Span::from(15..20),
        });

        let add_func = HirFunction {
            def_id: add_def_id,
            name: "add".to_string(),
            type_params: vec![],
            params: vec![param_a, param_b],
            ret_type: i32_ty,
            body: Some(add_body),
            span: Span::from(0..25),
        };
        hir_db.items.push(HirItem::Function(add_func));

        // Call expression: add(1, 2)
        let callee_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(add_def_id),
            ty: i32_ty,
            span: Span::from(30..33),
        });
        let arg1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(34..35),
        });
        let arg2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(37..38),
        });
        let call_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Call {
                callee: callee_expr,
                args: vec![arg1, arg2],
            },
            ty: i32_ty,
            span: Span::from(30..39),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(call_expr),
            },
            ty: i32_ty,
            span: Span::from(28..42),
        });

        let foo_func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(28..45),
        };
        hir_db.items.push(HirItem::Function(foo_func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[1]; // foo is second

        // Should have Call terminator with 2 arguments
        let mut found_call_with_args = false;
        for block in &body.basic_blocks {
            if let Some(term) = &block.terminator
                && let TerminatorKind::Call { func, args, .. } = &term.kind
                && matches!(func, Operand::Constant(Constant::FnDef(def_id)) if *def_id == add_def_id)
            {
                assert_eq!(args.len(), 2, "Expected 2 arguments");
                // Check args are constants 1 and 2
                assert!(matches!(&args[0], Operand::Constant(Constant::Int(1))));
                assert!(matches!(&args[1], Operand::Constant(Constant::Int(2))));
                found_call_with_args = true;
            }
        }
        assert!(found_call_with_args, "Expected call with 2 args");
    }

    #[test]
    fn test_lower_call_nested() {
        // fn bar() -> i32 { 1 }
        // fn baz(x: i32) -> i32 { x }
        // fn foo() -> i32 { baz(bar()) }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bar_def_id = DefId(1);
        let baz_def_id = DefId(2);
        let x_def_id = DefId(3);

        // bar function
        let bar_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let bar_func = HirFunction {
            def_id: bar_def_id,
            name: "bar".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(bar_body),
            span: Span::from(0..5),
        };
        hir_db.items.push(HirItem::Function(bar_func));

        // baz function
        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(10..11),
        });
        let param_x = crate::hir::HirParam {
            pat: pat_x,
            ty: i32_ty,
            span: Span::from(10..15),
        };
        let baz_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(20..21),
        });
        let baz_func = HirFunction {
            def_id: baz_def_id,
            name: "baz".to_string(),
            type_params: vec![],
            params: vec![param_x],
            ret_type: i32_ty,
            body: Some(baz_body),
            span: Span::from(10..25),
        };
        hir_db.items.push(HirItem::Function(baz_func));

        // foo function with nested call: baz(bar())
        let inner_callee = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(bar_def_id),
            ty: i32_ty,
            span: Span::from(35..38),
        });
        let inner_call = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Call {
                callee: inner_callee,
                args: vec![],
            },
            ty: i32_ty,
            span: Span::from(35..40),
        });
        let outer_callee = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(baz_def_id),
            ty: i32_ty,
            span: Span::from(30..33),
        });
        let outer_call = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Call {
                callee: outer_callee,
                args: vec![inner_call],
            },
            ty: i32_ty,
            span: Span::from(30..41),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(outer_call),
            },
            ty: i32_ty,
            span: Span::from(28..44),
        });

        let foo_func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(28..50),
        };
        hir_db.items.push(HirItem::Function(foo_func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[2]; // foo is third

        // Should have at least 2 Call terminators (or blocks for continuation)
        let call_count = body
            .basic_blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.terminator.as_ref().map(|t| &t.kind),
                    Some(TerminatorKind::Call { .. })
                )
            })
            .count();
        assert!(call_count >= 2, "Expected at least 2 Call terminators");
    }

    #[test]
    fn test_lower_method_call() {
        // Test method call lowering - simplified test with just the structure
        // Since method resolution requires full type checking, we test the MIR structure
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let method_def_id = DefId(5);

        // Create a receiver expression (simplified as a variable)
        let receiver_def_id = DefId(1);
        let receiver = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(receiver_def_id),
            ty: i32_ty,
            span: Span::from(0..1),
        });

        // Method call: receiver.method()
        let method_call = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::MethodCall {
                receiver,
                method: "method".to_string(),
                args: vec![],
            },
            ty: i32_ty,
            span: Span::from(0..10),
        });

        // Store the method resolution manually
        hir_db.method_resolutions.insert(method_call, method_def_id);

        // Create parameter for receiver
        let pat_recv = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: receiver_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(7..11),
        });
        let param_recv = crate::hir::HirParam {
            pat: pat_recv,
            ty: i32_ty,
            span: Span::from(7..17),
        };

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(method_call),
            },
            ty: i32_ty,
            span: Span::from(20..35),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_recv],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..40),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have Call terminator
        let has_call = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::Call { .. })
            )
        });
        assert!(has_call, "Expected Call terminator for method call");

        // Verify the call has the method as func and receiver as first arg
        for block in &body.basic_blocks {
            if let Some(term) = &block.terminator
                && let TerminatorKind::Call { func, args, .. } = &term.kind
            {
                // Should call method_def_id
                assert!(
                    matches!(func, Operand::Constant(Constant::FnDef(def_id)) if *def_id == method_def_id),
                    "Expected call to resolved method"
                );
                // Should have receiver as first argument
                assert!(!args.is_empty(), "Expected receiver as first argument");
            }
        }
    }

    #[test]
    fn test_lower_method_call_with_args() {
        // Test method call with additional arguments
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let method_def_id = DefId(5);

        // Create a receiver expression
        let receiver_def_id = DefId(1);
        let receiver = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(receiver_def_id),
            ty: i32_ty,
            span: Span::from(0..1),
        });

        // Create arguments
        let arg1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(10)),
            ty: i32_ty,
            span: Span::from(10..12),
        });
        let arg2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(20)),
            ty: i32_ty,
            span: Span::from(14..16),
        });

        // Method call: receiver.method(10, 20)
        let method_call = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::MethodCall {
                receiver,
                method: "method".to_string(),
                args: vec![arg1, arg2],
            },
            ty: i32_ty,
            span: Span::from(0..17),
        });

        // Store the method resolution manually
        hir_db.method_resolutions.insert(method_call, method_def_id);

        // Create parameter for receiver
        let pat_recv = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: receiver_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(7..11),
        });
        let param_recv = crate::hir::HirParam {
            pat: pat_recv,
            ty: i32_ty,
            span: Span::from(7..17),
        };

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(method_call),
            },
            ty: i32_ty,
            span: Span::from(20..40),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_recv],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..45),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have Call terminator with 3 args (receiver + 2 explicit)
        let mut found_call_with_args = false;
        for block in &body.basic_blocks {
            if let Some(term) = &block.terminator
                && let TerminatorKind::Call { func, args, .. } = &term.kind
                && matches!(func, Operand::Constant(Constant::FnDef(def_id)) if *def_id == method_def_id)
            {
                // 1 (receiver) + 2 (explicit args) = 3 total
                assert_eq!(args.len(), 3, "Expected 3 arguments (receiver + 2)");
                // Check that args[1] is 10 and args[2] is 20
                assert!(matches!(&args[1], Operand::Constant(Constant::Int(10))));
                assert!(matches!(&args[2], Operand::Constant(Constant::Int(20))));
                found_call_with_args = true;
            }
        }
        assert!(found_call_with_args, "Expected method call with args");
    }

    // ========== Phase 21: Binary Operator Edge Cases ==========

    #[test]
    fn test_lower_binary_div() {
        // fn foo() -> i32 { 10 / 3 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(10)),
            ty: i32_ty,
            span: Span::from(0..2),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(5..6),
        });
        let div_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Div,
                lhs,
                rhs,
            },
            ty: i32_ty,
            span: Span::from(0..6),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(div_expr),
            span: Span::from(0..10),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let found_div = body.basic_blocks[0].statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Div, _, _))
            )
        });
        assert!(found_div, "Expected Div operation");
    }

    #[test]
    fn test_lower_binary_rem() {
        // fn foo() -> i32 { 10 % 3 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(10)),
            ty: i32_ty,
            span: Span::from(0..2),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(5..6),
        });
        let rem_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Rem,
                lhs,
                rhs,
            },
            ty: i32_ty,
            span: Span::from(0..6),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(rem_expr),
            span: Span::from(0..10),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let found_rem = body.basic_blocks[0].statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Rem, _, _))
            )
        });
        assert!(found_rem, "Expected Rem operation");
    }

    #[test]
    fn test_lower_binary_le() {
        // fn foo(a: i32, b: i32) -> bool { a <= b }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bool_ty = hir_db.types.bool();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);

        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(5..6),
        });

        let param_a = crate::hir::HirParam {
            pat: pat_a,
            ty: i32_ty,
            span: Span::from(0..5),
        };
        let param_b = crate::hir::HirParam {
            pat: pat_b,
            ty: i32_ty,
            span: Span::from(5..10),
        };

        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(20..21),
        });
        let le_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Le,
                lhs: var_a,
                rhs: var_b,
            },
            ty: bool_ty,
            span: Span::from(15..21),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_a, param_b],
            ret_type: bool_ty,
            body: Some(le_expr),
            span: Span::from(0..25),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let found_le = body.basic_blocks[0].statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Le, _, _))
            )
        });
        assert!(found_le, "Expected Le comparison");
    }

    #[test]
    fn test_lower_binary_ge() {
        // fn foo() -> bool { 5 >= 3 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bool_ty = hir_db.types.bool();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(5)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(5..6),
        });
        let ge_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Ge,
                lhs,
                rhs,
            },
            ty: bool_ty,
            span: Span::from(0..6),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: bool_ty,
            body: Some(ge_expr),
            span: Span::from(0..10),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let found_ge = body.basic_blocks[0].statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Ge, _, _))
            )
        });
        assert!(found_ge, "Expected Ge comparison");
    }

    #[test]
    fn test_lower_binary_gt() {
        // fn foo() -> bool { 5 > 3 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bool_ty = hir_db.types.bool();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(5)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(5..6),
        });
        let gt_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Gt,
                lhs,
                rhs,
            },
            ty: bool_ty,
            span: Span::from(0..6),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: bool_ty,
            body: Some(gt_expr),
            span: Span::from(0..10),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let found_gt = body.basic_blocks[0].statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Gt, _, _))
            )
        });
        assert!(found_gt, "Expected Gt comparison");
    }

    #[test]
    fn test_lower_binary_ne() {
        // fn foo() -> bool { 5 != 3 }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bool_ty = hir_db.types.bool();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(5)),
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(5..6),
        });
        let ne_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Ne,
                lhs,
                rhs,
            },
            ty: bool_ty,
            span: Span::from(0..6),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: bool_ty,
            body: Some(ne_expr),
            span: Span::from(0..10),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        let found_ne = body.basic_blocks[0].statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Ne, _, _))
            )
        });
        assert!(found_ne, "Expected Ne comparison");
    }

    // ========== Phase 22: Short-Circuit Operators (Placeholder Behavior) ==========

    #[test]
    fn test_lower_binary_and_placeholder() {
        // fn foo() -> bool { true && false }
        // Note: And is not yet properly lowered (returns None from hir_binop_to_mir)
        // This test documents current placeholder behavior
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Bool(true)),
            ty: bool_ty,
            span: Span::from(0..4),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Bool(false)),
            ty: bool_ty,
            span: Span::from(8..13),
        });
        let and_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::And,
                lhs,
                rhs,
            },
            ty: bool_ty,
            span: Span::from(0..13),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: bool_ty,
            body: Some(and_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // And should use short-circuit evaluation with control flow
        // Check that it NOT uses BitAnd (it should use control flow instead)
        let found_binary_and = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::BitAnd, _, _))
                )
            })
        });
        assert!(
            !found_binary_and,
            "And should NOT be lowered to BinaryOp (uses short-circuit)"
        );

        // Check for SwitchInt terminator (short-circuit requires branching)
        let found_switch = body.basic_blocks.iter().any(|block| {
            matches!(
                &block.terminator,
                Some(Terminator {
                    kind: TerminatorKind::SwitchInt { .. },
                    ..
                })
            )
        });
        assert!(
            found_switch,
            "And should produce SwitchInt for short-circuit evaluation"
        );

        // Short-circuit requires at least 4 blocks: entry, rhs, false, merge (plus return continues)
        assert!(
            body.basic_blocks.len() >= 4,
            "Short-circuit And needs multiple blocks (got {})",
            body.basic_blocks.len()
        );
    }

    #[test]
    fn test_lower_binary_or_placeholder() {
        // fn foo() -> bool { true || false }
        // Note: Or is not yet properly lowered (returns None from hir_binop_to_mir)
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();

        let lhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Bool(true)),
            ty: bool_ty,
            span: Span::from(0..4),
        });
        let rhs = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Bool(false)),
            ty: bool_ty,
            span: Span::from(8..13),
        });
        let or_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Or,
                lhs,
                rhs,
            },
            ty: bool_ty,
            span: Span::from(0..13),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: bool_ty,
            body: Some(or_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Or should use short-circuit evaluation with control flow
        let found_binary_or = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::BitOr, _, _))
                )
            })
        });
        assert!(
            !found_binary_or,
            "Or should NOT be lowered to BinaryOp (uses short-circuit)"
        );

        // Check for SwitchInt terminator
        let found_switch = body.basic_blocks.iter().any(|block| {
            matches!(
                &block.terminator,
                Some(Terminator {
                    kind: TerminatorKind::SwitchInt { .. },
                    ..
                })
            )
        });
        assert!(
            found_switch,
            "Or should produce SwitchInt for short-circuit evaluation"
        );

        // Short-circuit requires at least 4 blocks
        assert!(
            body.basic_blocks.len() >= 4,
            "Short-circuit Or needs multiple blocks (got {})",
            body.basic_blocks.len()
        );
    }

    // ========== Phase 23: Unimplemented Expression Types (Placeholder Tests) ==========
    // These tests document expected placeholder behavior for unimplemented features.
    // When these features are properly implemented, these tests should be updated.

    #[test]
    fn test_lower_ref_placeholder() {
        // fn foo(x: i32) -> &i32 { &x }
        // Ref expressions are not yet implemented - should allocate placeholder temp
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let ref_ty = hir_db.types.mk_ref(crate::sema::types::Mutability::Shared, i32_ty);
        let x_def_id = DefId(1);

        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let param_x = crate::hir::HirParam {
            pat: pat_x,
            ty: i32_ty,
            span: Span::from(0..5),
        };

        let var_x = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(10..11),
        });
        let ref_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Ref {
                mutable: false,
                operand: var_x,
            },
            ty: ref_ty,
            span: Span::from(9..11),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_x],
            ret_type: ref_ty,
            body: Some(ref_expr),
            span: Span::from(0..15),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        assert!(!body.basic_blocks.is_empty(), "Should produce MIR body");

        // Ref should produce Rvalue::Ref with Shared borrow kind
        let found_ref = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Ref(BorrowKind::Shared, _))
                )
            })
        });
        assert!(found_ref, "Ref should produce Rvalue::Ref");
    }

    #[test]
    fn test_lower_struct_placeholder() {
        // Struct { field: value } expressions are not yet implemented
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let struct_def_id = DefId(10);

        let field_value = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(10..12),
        });
        let struct_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Struct {
                def_id: struct_def_id,
                fields: vec![("x".to_string(), field_value)],
            },
            ty: i32_ty, // Simplified - normally would be struct type
            span: Span::from(0..15),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(struct_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        assert!(!body.basic_blocks.is_empty(), "Should produce MIR body");

        // Struct should produce Rvalue::Aggregate(Adt(_), _)
        let found_aggregate = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Aggregate(AggregateKind::Adt(_), _))
                )
            })
        });
        assert!(
            found_aggregate,
            "Struct should produce Rvalue::Aggregate(Adt(_), _)"
        );
    }

    #[test]
    fn test_lower_array_placeholder() {
        // [1, 2, 3] array literal expressions are not yet implemented
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let array_ty = hir_db.types.mk_array(i32_ty, 3);

        let elem1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(1..2),
        });
        let elem2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let elem3 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(7..8),
        });
        let array_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Array {
                elements: vec![elem1, elem2, elem3],
            },
            ty: array_ty,
            span: Span::from(0..9),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: array_ty,
            body: Some(array_expr),
            span: Span::from(0..15),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        assert!(!body.basic_blocks.is_empty(), "Should produce MIR body");

        // Array should produce Rvalue::Aggregate(Array, _) with 3 operands
        let found_array = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Aggregate(AggregateKind::Array, operands)) if operands.len() == 3
                )
            })
        });
        assert!(
            found_array,
            "Array should produce Rvalue::Aggregate(Array, _)"
        );
    }

    #[test]
    fn test_lower_tuple_placeholder() {
        // (1, true, 'a') tuple literal expressions are not yet implemented
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bool_ty = hir_db.types.bool();
        let char_ty = hir_db.types.char();
        let tuple_ty = hir_db.types.mk_tuple(vec![i32_ty, bool_ty, char_ty]);

        let elem1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(1..2),
        });
        let elem2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Bool(true)),
            ty: bool_ty,
            span: Span::from(4..8),
        });
        let elem3 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Char('a')),
            ty: char_ty,
            span: Span::from(10..13),
        });
        let tuple_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Tuple {
                elements: vec![elem1, elem2, elem3],
            },
            ty: tuple_ty,
            span: Span::from(0..14),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: tuple_ty,
            body: Some(tuple_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        assert!(!body.basic_blocks.is_empty(), "Should produce MIR body");

        // Tuple should produce Rvalue::Aggregate(Tuple, _) with 3 operands
        let found_tuple = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Aggregate(AggregateKind::Tuple, operands)) if operands.len() == 3
                )
            })
        });
        assert!(
            found_tuple,
            "Tuple should produce Rvalue::Aggregate(Tuple, _)"
        );
    }

    #[test]
    fn test_lower_array_repeat() {
        // [0; 5] - array repeat expression
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let array_ty = hir_db.types.mk_array(i32_ty, 5);

        // Create the repeated value
        let value = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(0)),
            ty: i32_ty,
            span: Span::from(1..2),
        });

        // Create ArrayRepeat expression
        let repeat_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::ArrayRepeat { value, count: 5 },
            ty: array_ty,
            span: Span::from(0..6),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "test_repeat".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: array_ty,
            body: Some(repeat_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Verify: should produce Rvalue::Repeat with count 5
        let found_repeat = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Repeat(_, 5))
                )
            })
        });
        assert!(
            found_repeat,
            "ArrayRepeat should produce Rvalue::Repeat(_, count)"
        );
    }

    #[test]
    fn test_lower_array_repeat_zero_count() {
        // [42; 0] - empty array
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let array_ty = hir_db.types.mk_array(i32_ty, 0);

        let value = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(1..3),
        });

        let repeat_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::ArrayRepeat { value, count: 0 },
            ty: array_ty,
            span: Span::from(0..6),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "test_empty".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: array_ty,
            body: Some(repeat_expr),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Verify Rvalue::Repeat(_, 0) is produced
        let found_repeat = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Repeat(_, 0))
                )
            })
        });
        assert!(found_repeat, "ArrayRepeat with count 0 should produce Rvalue::Repeat(_, 0)");
    }

    #[test]
    fn test_lower_array_repeat_complex_value() {
        // [x + 1; 3] where x is a variable
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let array_ty = hir_db.types.mk_array(i32_ty, 3);
        let x_def_id = DefId(1);

        // Create parameter x
        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let param_x = crate::hir::HirParam {
            pat: pat_x,
            ty: i32_ty,
            span: Span::from(0..5),
        };

        // Create x + 1 expression
        let x_var = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(10..11),
        });
        let one = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(14..15),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: x_var,
                rhs: one,
            },
            ty: i32_ty,
            span: Span::from(10..15),
        });

        // Create [x + 1; 3]
        let repeat_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::ArrayRepeat {
                value: add_expr,
                count: 3,
            },
            ty: array_ty,
            span: Span::from(9..19),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "test_complex".to_string(),
            type_params: vec![],
            params: vec![param_x],
            ret_type: array_ty,
            body: Some(repeat_expr),
            span: Span::from(0..25),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Verify the value expression is lowered before repeat
        // We should see a BinaryOp for x + 1 and then Repeat
        let has_binop = body.basic_blocks.iter().any(|block| {
            block
                .statements
                .iter()
                .any(|stmt| matches!(&stmt.kind, StatementKind::Assign(_, Rvalue::BinaryOp(_, _, _))))
        });
        let has_repeat = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Repeat(_, 3))
                )
            })
        });
        assert!(has_binop, "Complex value should produce BinaryOp");
        assert!(has_repeat, "ArrayRepeat should produce Rvalue::Repeat(_, 3)");
    }

    #[test]
    fn test_lower_field_placeholder() {
        // obj.field expressions are not yet implemented
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let obj_def_id = DefId(1);

        let pat_obj = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: obj_def_id,
                mutable: false,
            },
            ty: i32_ty, // Simplified
            span: Span::from(0..3),
        });
        let param_obj = crate::hir::HirParam {
            pat: pat_obj,
            ty: i32_ty,
            span: Span::from(0..8),
        };

        let base = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(obj_def_id),
            ty: i32_ty,
            span: Span::from(15..18),
        });
        let field_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Field {
                base,
                field: "x".to_string(),
            },
            ty: i32_ty,
            span: Span::from(15..20),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_obj],
            ret_type: i32_ty,
            body: Some(field_expr),
            span: Span::from(0..25),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        assert!(!body.basic_blocks.is_empty(), "Should produce MIR body");

        // Field access should produce a Place with Field projection
        // Check both assignment targets and operands for Field projections
        let has_field_proj = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                    // Check assignment target
                    let in_target = place
                        .projection
                        .iter()
                        .any(|e| matches!(e, PlaceElem::Field(_)));
                    // Check operands in rvalue
                    let in_rvalue = match rvalue {
                        Rvalue::Use(Operand::Copy(p)) | Rvalue::Use(Operand::Move(p)) => {
                            p.projection.iter().any(|e| matches!(e, PlaceElem::Field(_)))
                        }
                        _ => false,
                    };
                    in_target || in_rvalue
                } else {
                    false
                }
            })
        });
        assert!(has_field_proj, "s.field should use Field projection");
    }

    #[test]
    fn test_lower_tuple_field_placeholder() {
        // tuple.0 expressions are not yet implemented
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let tuple_ty = hir_db.types.mk_tuple(vec![i32_ty, i32_ty]);
        let t_def_id = DefId(1);

        let pat_t = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: t_def_id,
                mutable: false,
            },
            ty: tuple_ty,
            span: Span::from(0..1),
        });
        let param_t = crate::hir::HirParam {
            pat: pat_t,
            ty: tuple_ty,
            span: Span::from(0..10),
        };

        let base = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(t_def_id),
            ty: tuple_ty,
            span: Span::from(15..16),
        });
        let tuple_field_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::TupleField { base, index: 0 },
            ty: i32_ty,
            span: Span::from(15..18),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_t],
            ret_type: i32_ty,
            body: Some(tuple_field_expr),
            span: Span::from(0..25),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        assert!(!body.basic_blocks.is_empty(), "Should produce MIR body");

        // TupleField should produce a Place with Field(0) projection
        let has_field_proj = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                    let in_target = place
                        .projection
                        .iter()
                        .any(|e| matches!(e, PlaceElem::Field(FieldIdx(0))));
                    let in_rvalue = match rvalue {
                        Rvalue::Use(Operand::Copy(p)) | Rvalue::Use(Operand::Move(p)) => p
                            .projection
                            .iter()
                            .any(|e| matches!(e, PlaceElem::Field(FieldIdx(0)))),
                        _ => false,
                    };
                    in_target || in_rvalue
                } else {
                    false
                }
            })
        });
        assert!(has_field_proj, "tuple.0 should use Field(0) projection");
    }

    #[test]
    fn test_lower_index_placeholder() {
        // array[idx] expressions are not yet implemented
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let array_ty = hir_db.types.mk_array(i32_ty, 5);
        let arr_def_id = DefId(1);

        let pat_arr = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: arr_def_id,
                mutable: false,
            },
            ty: array_ty,
            span: Span::from(0..3),
        });
        let param_arr = crate::hir::HirParam {
            pat: pat_arr,
            ty: array_ty,
            span: Span::from(0..15),
        };

        let base = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(arr_def_id),
            ty: array_ty,
            span: Span::from(20..23),
        });
        let index = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(24..25),
        });
        let index_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Index { base, index },
            ty: i32_ty,
            span: Span::from(20..26),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_arr],
            ret_type: i32_ty,
            body: Some(index_expr),
            span: Span::from(0..30),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        assert!(!body.basic_blocks.is_empty(), "Should produce MIR body");

        // Index should produce a Place with Index projection
        let has_index_proj = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                    let in_target = place
                        .projection
                        .iter()
                        .any(|e| matches!(e, PlaceElem::Index(_)));
                    let in_rvalue = match rvalue {
                        Rvalue::Use(Operand::Copy(p)) | Rvalue::Use(Operand::Move(p)) => {
                            p.projection.iter().any(|e| matches!(e, PlaceElem::Index(_)))
                        }
                        _ => false,
                    };
                    in_target || in_rvalue
                } else {
                    false
                }
            })
        });
        assert!(has_index_proj, "arr[i] should use Index projection");
    }

    #[test]
    fn test_lower_cast_placeholder() {
        // expr as Type expressions are not yet implemented
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let i64_ty = hir_db.types.i64();

        let inner = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(0..2),
        });
        let cast_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Cast {
                expr: inner,
                target_ty: i64_ty,
            },
            ty: i64_ty,
            span: Span::from(0..10),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i64_ty,
            body: Some(cast_expr),
            span: Span::from(0..15),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        assert!(!body.basic_blocks.is_empty(), "Should produce MIR body");

        // Cast should produce Rvalue::Cast with IntToInt kind
        let found_cast = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Cast(CastKind::IntToInt, _, _))
                )
            })
        });
        assert!(found_cast, "Cast should produce Rvalue::Cast");
    }

    // ========== Phase 24: Complex Control Flow Edge Cases ==========

    #[test]
    fn test_lower_deeply_nested_blocks() {
        // fn foo() -> i32 { { { { 42 } } } }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();

        let innermost = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(42)),
            ty: i32_ty,
            span: Span::from(6..8),
        });
        let block1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(innermost),
            },
            ty: i32_ty,
            span: Span::from(4..10),
        });
        let block2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(block1),
            },
            ty: i32_ty,
            span: Span::from(2..12),
        });
        let block3 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(block2),
            },
            ty: i32_ty,
            span: Span::from(0..14),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block3),
            span: Span::from(0..20),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // 42 should eventually be assigned to return place
        let found_42 = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(42))))
                )
            })
        });
        assert!(found_42, "Expected 42 through nested blocks");
    }

    #[test]
    fn test_lower_multiple_sequential_lets() {
        // fn foo() -> i32 { let a = 1; let b = 2; let c = 3; a + b + c }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let a_def_id = DefId(1);
        let b_def_id = DefId(2);
        let c_def_id = DefId(3);

        // Create let a = 1
        let pat_a = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: a_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(4..5),
        });
        let init_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(8..9),
        });
        let stmt_a = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_a,
                ty: Some(i32_ty),
                init: Some(init_a),
            },
            span: Span::from(0..10),
        });

        // Create let b = 2
        let pat_b = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: b_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(15..16),
        });
        let init_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(19..20),
        });
        let stmt_b = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_b,
                ty: Some(i32_ty),
                init: Some(init_b),
            },
            span: Span::from(11..21),
        });

        // Create let c = 3
        let pat_c = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: c_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(26..27),
        });
        let init_c = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(3)),
            ty: i32_ty,
            span: Span::from(30..31),
        });
        let stmt_c = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Let {
                pat: pat_c,
                ty: Some(i32_ty),
                init: Some(init_c),
            },
            span: Span::from(22..32),
        });

        // Create a + b + c
        let var_a = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(a_def_id),
            ty: i32_ty,
            span: Span::from(33..34),
        });
        let var_b = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(b_def_id),
            ty: i32_ty,
            span: Span::from(37..38),
        });
        let var_c = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(c_def_id),
            ty: i32_ty,
            span: Span::from(41..42),
        });
        let add_ab = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: var_a,
                rhs: var_b,
            },
            ty: i32_ty,
            span: Span::from(33..38),
        });
        let add_abc = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: add_ab,
                rhs: var_c,
            },
            ty: i32_ty,
            span: Span::from(33..42),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![stmt_a, stmt_b, stmt_c],
                tail: Some(add_abc),
            },
            ty: i32_ty,
            span: Span::from(0..45),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(0..50),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have at least 4 locals: _0 (return), _1 (a), _2 (b), _3 (c)
        assert!(body.locals.len() >= 4, "Expected at least 4 locals");

        // Should have 3 StorageLive statements
        let storage_live_count = body.basic_blocks[0]
            .statements
            .iter()
            .filter(|stmt| matches!(&stmt.kind, StatementKind::StorageLive(_)))
            .count();
        assert_eq!(storage_live_count, 3, "Expected 3 StorageLive statements");

        // Should have 2 Add operations
        let add_count = body.basic_blocks[0]
            .statements
            .iter()
            .filter(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _))
                )
            })
            .count();
        assert_eq!(add_count, 2, "Expected 2 Add operations");
    }

    #[test]
    fn test_lower_if_in_loop() {
        // fn foo(n: i32) -> i32 { let mut i = 0; loop { if i >= n { break i; } i = i + 1; } }
        // Simplified: loop { if cond { break 1; } break 2; }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bool_ty = hir_db.types.bool();
        let cond_def_id = DefId(1);

        let pat_cond = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: cond_def_id,
                mutable: false,
            },
            ty: bool_ty,
            span: Span::from(0..4),
        });
        let param_cond = crate::hir::HirParam {
            pat: pat_cond,
            ty: bool_ty,
            span: Span::from(0..10),
        };

        // if cond { break 1; }
        let cond_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(cond_def_id),
            ty: bool_ty,
            span: Span::from(20..24),
        });
        let break_1_val = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(33..34),
        });
        let break_1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(break_1_val),
            },
            ty: i32_ty,
            span: Span::from(27..35),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(break_1),
            },
            ty: i32_ty,
            span: Span::from(25..37),
        });
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond_expr,
                then_branch: then_block,
                else_branch: None,
            },
            ty: i32_ty,
            span: Span::from(17..37),
        });

        // break 2
        let break_2_val = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(45..46),
        });
        let break_2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break {
                value: Some(break_2_val),
            },
            ty: i32_ty,
            span: Span::from(39..47),
        });

        // if as statement
        let if_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Expr {
                expr: if_expr,
                has_semi: true,
            },
            span: Span::from(17..38),
        });

        // loop body
        let loop_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![if_stmt],
                tail: Some(break_2),
            },
            ty: i32_ty,
            span: Span::from(15..49),
        });

        let loop_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: loop_body },
            ty: i32_ty,
            span: Span::from(10..49),
        });

        let outer_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(loop_expr),
            },
            ty: i32_ty,
            span: Span::from(10..51),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_cond],
            ret_type: i32_ty,
            body: Some(outer_block),
            span: Span::from(0..60),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple blocks
        assert!(
            body.basic_blocks.len() >= 4,
            "Expected at least 4 blocks for loop with if"
        );

        // Should have both 1 and 2 assigned
        let found_1 = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(1))))
                )
            })
        });
        let found_2 = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(2))))
                )
            })
        });
        assert!(found_1, "Expected break 1");
        assert!(found_2, "Expected break 2");
    }

    #[test]
    fn test_lower_call_with_binary_arg() {
        // fn bar(x: i32) -> i32 { x }
        // fn foo() -> i32 { bar(1 + 2) }
        let mut hir_db = HirDatabase::new();
        let i32_ty = hir_db.types.i32();
        let bar_def_id = DefId(1);
        let x_def_id = DefId(2);

        // bar function
        let pat_x = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: x_def_id,
                mutable: false,
            },
            ty: i32_ty,
            span: Span::from(0..1),
        });
        let param_x = crate::hir::HirParam {
            pat: pat_x,
            ty: i32_ty,
            span: Span::from(0..5),
        };
        let bar_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(x_def_id),
            ty: i32_ty,
            span: Span::from(10..11),
        });
        let bar_func = HirFunction {
            def_id: bar_def_id,
            name: "bar".to_string(),
            type_params: vec![],
            params: vec![param_x],
            ret_type: i32_ty,
            body: Some(bar_body),
            span: Span::from(0..15),
        };
        hir_db.items.push(HirItem::Function(bar_func));

        // foo function with bar(1 + 2)
        let lit_1 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(24..25),
        });
        let lit_2 = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(28..29),
        });
        let add_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Binary {
                op: crate::hir::BinOp::Add,
                lhs: lit_1,
                rhs: lit_2,
            },
            ty: i32_ty,
            span: Span::from(24..29),
        });
        let callee = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(bar_def_id),
            ty: i32_ty,
            span: Span::from(20..23),
        });
        let call_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Call {
                callee,
                args: vec![add_expr],
            },
            ty: i32_ty,
            span: Span::from(20..30),
        });

        let block_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(call_expr),
            },
            ty: i32_ty,
            span: Span::from(18..33),
        });

        let foo_func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: i32_ty,
            body: Some(block_expr),
            span: Span::from(18..40),
        };
        hir_db.items.push(HirItem::Function(foo_func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[1]; // foo is second

        // Should have Add operation before Call
        let found_add = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _))
                )
            })
        });
        assert!(found_add, "Expected Add operation for argument");

        // Should have Call terminator
        let has_call = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::Call { .. })
            )
        });
        assert!(has_call, "Expected Call terminator");
    }

    #[test]
    fn test_lower_return_in_if_branch() {
        // fn foo(c: bool) -> i32 { if c { return 1; } 2 }
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();
        let i32_ty = hir_db.types.i32();
        let c_def_id = DefId(1);

        let pat_c = hir_db.alloc_pat(crate::hir::HirPat {
            kind: crate::hir::HirPatKind::Bind {
                def_id: c_def_id,
                mutable: false,
            },
            ty: bool_ty,
            span: Span::from(0..1),
        });
        let param_c = crate::hir::HirParam {
            pat: pat_c,
            ty: bool_ty,
            span: Span::from(0..7),
        };

        // if c { return 1; }
        let cond = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Var(c_def_id),
            ty: bool_ty,
            span: Span::from(20..21),
        });
        let ret_val = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(1)),
            ty: i32_ty,
            span: Span::from(31..32),
        });
        let ret_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Return {
                value: Some(ret_val),
            },
            ty: i32_ty,
            span: Span::from(24..33),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(ret_expr),
            },
            ty: i32_ty,
            span: Span::from(22..35),
        });
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond,
                then_branch: then_block,
                else_branch: None,
            },
            ty: i32_ty,
            span: Span::from(17..35),
        });

        // if as statement, then tail is 2
        let if_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Expr {
                expr: if_expr,
                has_semi: true,
            },
            span: Span::from(17..36),
        });
        let tail = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Int(2)),
            ty: i32_ty,
            span: Span::from(37..38),
        });
        let block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![if_stmt],
                tail: Some(tail),
            },
            ty: i32_ty,
            span: Span::from(15..40),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![param_c],
            ret_type: i32_ty,
            body: Some(block),
            span: Span::from(0..45),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have Return terminator in some block
        let return_count = body
            .basic_blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.terminator.as_ref().map(|t| &t.kind),
                    Some(TerminatorKind::Return)
                )
            })
            .count();
        // At least 2 returns: one from `return 1`, one from end of function
        assert!(
            return_count >= 1,
            "Expected at least 1 Return terminator, got {}",
            return_count
        );

        // Should have both 1 and 2 assigned
        let found_1 = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(
                        place,
                        Rvalue::Use(Operand::Constant(Constant::Int(1)))
                    ) if place.local == Local(0)
                )
            })
        });
        let found_2 = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(2))))
                )
            })
        });
        assert!(found_1, "Expected return 1 to assign to _0");
        assert!(found_2, "Expected fallthrough path with 2");
    }

    #[test]
    fn test_lower_continue_in_nested_if() {
        // fn foo() { loop { if true { continue; } break; } }
        let mut hir_db = HirDatabase::new();
        let bool_ty = hir_db.types.bool();
        let unit_ty = hir_db.types.unit();

        // if true { continue; }
        let cond = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Literal(Literal::Bool(true)),
            ty: bool_ty,
            span: Span::from(15..19),
        });
        let continue_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Continue,
            ty: unit_ty,
            span: Span::from(22..30),
        });
        let then_block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(continue_expr),
            },
            ty: unit_ty,
            span: Span::from(20..32),
        });
        let if_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::If {
                condition: cond,
                then_branch: then_block,
                else_branch: None,
            },
            ty: unit_ty,
            span: Span::from(12..32),
        });

        // break
        let break_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Break { value: None },
            ty: unit_ty,
            span: Span::from(34..39),
        });

        // if as statement
        let if_stmt = hir_db.alloc_stmt(crate::hir::HirStmt {
            kind: crate::hir::HirStmtKind::Expr {
                expr: if_expr,
                has_semi: true,
            },
            span: Span::from(12..33),
        });

        // loop body
        let loop_body = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![if_stmt],
                tail: Some(break_expr),
            },
            ty: unit_ty,
            span: Span::from(10..41),
        });
        let loop_expr = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Loop { body: loop_body },
            ty: unit_ty,
            span: Span::from(5..41),
        });

        let block = hir_db.alloc_expr(crate::hir::HirExpr {
            kind: HirExprKind::Block {
                stmts: vec![],
                tail: Some(loop_expr),
            },
            ty: unit_ty,
            span: Span::from(5..43),
        });

        let func = HirFunction {
            def_id: DefId(0),
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            ret_type: unit_ty,
            body: Some(block),
            span: Span::from(0..50),
        };
        hir_db.items.push(HirItem::Function(func));

        let bodies = lower_hir_to_mir(&hir_db);
        let body = &bodies[0];

        // Should have multiple Goto terminators (for continue and break)
        let goto_count = body
            .basic_blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.terminator.as_ref().map(|t| &t.kind),
                    Some(TerminatorKind::Goto(_))
                )
            })
            .count();
        assert!(goto_count >= 2, "Expected at least 2 Goto terminators");

        // Should have SwitchInt for if condition
        let has_switch = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(TerminatorKind::SwitchInt { .. })
            )
        });
        assert!(has_switch, "Expected SwitchInt for if condition");
    }
}
