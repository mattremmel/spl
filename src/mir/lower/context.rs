//! MIR lowering context for HIR to MIR conversion.

use super::builder::MirBuilder;
use super::helpers::{
    determine_cast_kind, hir_binop_to_mir, hir_unop_to_mir, literal_to_operand, lower_literal,
};
use crate::hir::{
    BinOp as HirBinOp, ExprId, HirDatabase, HirExprKind, HirFunction, HirItem, HirPatKind,
    HirStmtKind, StmtId,
};
use crate::lexer::Span;
use crate::mir::body::Body;
use crate::mir::operand::{AggregateKind, BinOp, BorrowKind, Constant, Operand, Rvalue};
use crate::mir::statement::Statement;
use crate::mir::terminator::{BasicBlock, SwitchTargets, TerminatorKind};
use crate::mir::types::{FieldIdx, Local, Place, PlaceElem};
use crate::sema::symbol::DefId;
use crate::sema::types::Type;
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
    pub(crate) local_map: FxHashMap<DefId, Local>,
    /// Lowered function bodies.
    pub bodies: Vec<Body>,
    /// Stack of active loop contexts for break/continue target resolution.
    pub(crate) loop_stack: Vec<LoopContext>,
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

    /// Resolve a field name to its index within a struct.
    fn resolve_field_index(&self, struct_def_id: DefId, field_name: &str) -> u32 {
        for item in &self.hir.items {
            if let HirItem::Struct(s) = item
                && s.def_id == struct_def_id
            {
                for (idx, field) in s.fields.iter().enumerate() {
                    if field.name == field_name {
                        return idx as u32;
                    }
                }
                panic!("Field '{}' not found in struct '{}'", field_name, s.name);
            }
        }
        panic!(
            "Struct with DefId {:?} not found for field '{}'",
            struct_def_id, field_name
        );
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
                self.lower_binary_expr(builder, *op, *lhs, *rhs, ty, span)
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
                    // Deref - produces a place projection
                    let operand_place = self.lower_expr_to_place(builder, *operand);
                    Place {
                        local: operand_place.local,
                        projection: {
                            let mut proj = operand_place.projection;
                            proj.push(PlaceElem::Deref);
                            proj
                        },
                    }
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
            } => self.lower_if_expr(builder, *condition, *then_branch, *else_branch, ty, span),
            HirExprKind::Loop { body } => self.lower_loop_expr(builder, *body, ty, span),
            HirExprKind::Break { value } => {
                self.lower_break_expr(builder, value.as_ref().copied(), span)
            }
            HirExprKind::Continue => self.lower_continue_expr(builder, span),
            HirExprKind::Call { callee, args } => {
                self.lower_call_expr(builder, *callee, args, ty, span)
            }
            HirExprKind::MethodCall {
                receiver,
                method: _,
                args,
            } => self.lower_method_call_expr(builder, expr_id, *receiver, args, ty, span),
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
                let base_place = self.lower_expr_to_place(builder, *base);
                let base_ty = self.hir.expr(*base).ty;

                // Get struct DefId from base type and resolve field index
                let field_idx = match self.hir.types.get(base_ty) {
                    Type::Struct(def_id, _) => self.resolve_field_index(*def_id, field),
                    _ => panic!(
                        "Field access on non-struct type: {:?}",
                        self.hir.types.get(base_ty)
                    ),
                };

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
            HirExprKind::Is {
                scrutinee,
                pattern,
                negated,
            } => {
                // For now, `is` expressions produce a boolean result
                // TODO: Implement proper pattern matching lowering
                // Currently just evaluates scrutinee and returns true/false based on pattern
                let _scrutinee_val = self.lower_expr_as_operand(builder, *scrutinee);

                // Check if pattern is wildcard or catch-all binding (always matches)
                let pat = self.hir.pat(*pattern);
                let always_matches = matches!(
                    pat.kind,
                    crate::hir::HirPatKind::Wildcard | crate::hir::HirPatKind::Bind { .. }
                );

                let result = if always_matches {
                    !negated
                } else {
                    // For literal patterns, we'd need to compare values
                    // For now, return false for non-wildcard patterns (conservative)
                    *negated
                };

                let temp = builder.alloc_temp(ty);
                let place = Place::from_local(temp);
                builder.push_statement(Statement::assign(
                    place.clone(),
                    Rvalue::Use(Operand::Constant(Constant::Bool(result))),
                    span,
                ));
                place
            }
            HirExprKind::Match { scrutinee, arms } => {
                // For now, match expressions produce the first arm's result
                // TODO: Implement proper pattern matching with branching
                let _scrutinee_val = self.lower_expr_as_operand(builder, *scrutinee);

                if let Some((_pat, _guard, body)) = arms.first() {
                    // Just lower the first arm's body for now
                    self.lower_expr_to_place(builder, *body)
                } else {
                    // No arms - return zeroed value
                    let temp = builder.alloc_temp(ty);
                    let place = Place::from_local(temp);
                    builder.push_statement(Statement::assign(
                        place.clone(),
                        Rvalue::Use(Operand::Constant(Constant::Zeroed(ty))),
                        span,
                    ));
                    place
                }
            }
            HirExprKind::Missing => {
                // Missing expressions (from error recovery) produce a zeroed value
                let temp = builder.alloc_temp(ty);
                let place = Place::from_local(temp);
                builder.push_statement(Statement::assign(
                    place.clone(),
                    Rvalue::Use(Operand::Constant(Constant::Zeroed(ty))),
                    span,
                ));
                place
            }
        }
    }

    /// Lower a binary expression.
    fn lower_binary_expr(
        &mut self,
        builder: &mut MirBuilder,
        op: HirBinOp,
        lhs: ExprId,
        rhs: ExprId,
        ty: crate::sema::types::TypeId,
        span: Span,
    ) -> Place {
        // Handle short-circuit operators specially
        if op == HirBinOp::And {
            self.lower_short_circuit_and(builder, lhs, rhs, ty, span)
        } else if op == HirBinOp::Or {
            self.lower_short_circuit_or(builder, lhs, rhs, ty, span)
        } else if let Some(mir_op) = hir_binop_to_mir(op) {
            // Simple binary operation
            let lhs_operand = self.lower_expr_as_operand(builder, lhs);
            let rhs_operand = self.lower_expr_as_operand(builder, rhs);

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
            // Assignment operators (=, +=, -=, etc.)
            self.lower_assignment_expr(builder, op, lhs, rhs, span)
        }
    }

    /// Lower short-circuit AND.
    fn lower_short_circuit_and(
        &mut self,
        builder: &mut MirBuilder,
        lhs: ExprId,
        rhs: ExprId,
        ty: crate::sema::types::TypeId,
        span: Span,
    ) -> Place {
        let result_place = Place::from_local(builder.alloc_temp(ty));

        // Evaluate LHS
        let lhs_operand = self.lower_expr_as_operand(builder, lhs);

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
        let rhs_operand = self.lower_expr_as_operand(builder, rhs);
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
    }

    /// Lower short-circuit OR.
    fn lower_short_circuit_or(
        &mut self,
        builder: &mut MirBuilder,
        lhs: ExprId,
        rhs: ExprId,
        ty: crate::sema::types::TypeId,
        span: Span,
    ) -> Place {
        let result_place = Place::from_local(builder.alloc_temp(ty));

        // Evaluate LHS
        let lhs_operand = self.lower_expr_as_operand(builder, lhs);

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
        let rhs_operand = self.lower_expr_as_operand(builder, rhs);
        builder.push_statement(Statement::assign(
            result_place.clone(),
            Rvalue::Use(rhs_operand),
            span.clone(),
        ));
        builder.set_terminator(TerminatorKind::Goto(merge_bb), span);

        // Continue in merge block
        builder.switch_to_block(merge_bb);
        result_place
    }

    /// Lower an assignment expression.
    fn lower_assignment_expr(
        &mut self,
        builder: &mut MirBuilder,
        op: HirBinOp,
        lhs: ExprId,
        rhs: ExprId,
        span: Span,
    ) -> Place {
        // Lower LHS to a place (target) and RHS to an operand (value)
        let target_place = self.lower_expr_to_place(builder, lhs);
        let rhs_operand = self.lower_expr_as_operand(builder, rhs);

        match op {
            HirBinOp::Assign => {
                // Simple assignment: target = value
                builder.push_statement(Statement::assign(
                    target_place.clone(),
                    Rvalue::Use(rhs_operand),
                    span,
                ));
            }
            HirBinOp::AddAssign => {
                // Compound assignment: target = target + value
                let lhs_operand = Operand::Copy(target_place.clone());
                builder.push_statement(Statement::assign(
                    target_place.clone(),
                    Rvalue::BinaryOp(BinOp::Add, lhs_operand, rhs_operand),
                    span,
                ));
            }
            HirBinOp::SubAssign => {
                let lhs_operand = Operand::Copy(target_place.clone());
                builder.push_statement(Statement::assign(
                    target_place.clone(),
                    Rvalue::BinaryOp(BinOp::Sub, lhs_operand, rhs_operand),
                    span,
                ));
            }
            HirBinOp::MulAssign => {
                let lhs_operand = Operand::Copy(target_place.clone());
                builder.push_statement(Statement::assign(
                    target_place.clone(),
                    Rvalue::BinaryOp(BinOp::Mul, lhs_operand, rhs_operand),
                    span,
                ));
            }
            HirBinOp::DivAssign => {
                let lhs_operand = Operand::Copy(target_place.clone());
                builder.push_statement(Statement::assign(
                    target_place.clone(),
                    Rvalue::BinaryOp(BinOp::Div, lhs_operand, rhs_operand),
                    span,
                ));
            }
            HirBinOp::RemAssign => {
                let lhs_operand = Operand::Copy(target_place.clone());
                builder.push_statement(Statement::assign(
                    target_place.clone(),
                    Rvalue::BinaryOp(BinOp::Rem, lhs_operand, rhs_operand),
                    span,
                ));
            }
            _ => unreachable!("Non-assignment op should have been handled above"),
        }

        // Assignment expressions return unit
        let unit_ty = self.hir.types.unit();
        let temp = builder.alloc_temp(unit_ty);
        Place::from_local(temp)
    }

    /// Lower an if expression.
    fn lower_if_expr(
        &mut self,
        builder: &mut MirBuilder,
        condition: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
        ty: crate::sema::types::TypeId,
        span: Span,
    ) -> Place {
        // Allocate result place
        let result_place = Place::from_local(builder.alloc_temp(ty));

        // Lower condition
        let cond_operand = self.lower_expr_as_operand(builder, condition);

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
        let then_operand = self.lower_expr_as_operand(builder, then_branch);
        // Only add fallthrough if the branch didn't diverge (e.g., break/continue/return)
        if !builder.is_current_block_terminated() {
            builder.push_statement(Statement::assign(
                result_place.clone(),
                Rvalue::Use(then_operand),
                span.clone(),
            ));
            builder.set_terminator(TerminatorKind::Goto(join_bb), span.clone());
        }

        // Lower else branch (if present)
        if let Some(else_expr) = else_branch {
            builder.switch_to_block(else_bb);
            let else_operand = self.lower_expr_as_operand(builder, else_expr);
            // Only add fallthrough if the branch didn't diverge
            if !builder.is_current_block_terminated() {
                builder.push_statement(Statement::assign(
                    result_place.clone(),
                    Rvalue::Use(else_operand),
                    span.clone(),
                ));
                builder.set_terminator(TerminatorKind::Goto(join_bb), span.clone());
            }
        }

        // Continue in join block
        builder.switch_to_block(join_bb);

        result_place
    }

    /// Lower a loop expression.
    fn lower_loop_expr(
        &mut self,
        builder: &mut MirBuilder,
        body: ExprId,
        ty: crate::sema::types::TypeId,
        span: Span,
    ) -> Place {
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
        let _ = self.lower_expr_to_place(builder, body);

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

    /// Lower a break expression.
    fn lower_break_expr(
        &mut self,
        builder: &mut MirBuilder,
        value: Option<ExprId>,
        span: Span,
    ) -> Place {
        // Get current loop context
        let loop_ctx = self
            .loop_stack
            .last()
            .expect("break outside of loop")
            .clone();

        // Lower break value (if any) and assign to result place
        if let Some(val_id) = value {
            let operand = self.lower_expr_as_operand(builder, val_id);
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

    /// Lower a continue expression.
    fn lower_continue_expr(&mut self, builder: &mut MirBuilder, span: Span) -> Place {
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

    /// Lower a call expression.
    fn lower_call_expr(
        &mut self,
        builder: &mut MirBuilder,
        callee: ExprId,
        args: &[ExprId],
        ty: crate::sema::types::TypeId,
        span: Span,
    ) -> Place {
        // Lower callee - check if it's a direct function ref or expression
        let func_operand = self.lower_callee_operand(builder, callee);

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

    /// Lower a method call expression.
    fn lower_method_call_expr(
        &mut self,
        builder: &mut MirBuilder,
        expr_id: ExprId,
        receiver: ExprId,
        args: &[ExprId],
        ty: crate::sema::types::TypeId,
        span: Span,
    ) -> Place {
        // Look up resolved method DefId
        let method_def_id = self
            .hir
            .method_resolutions
            .get(&expr_id)
            .copied()
            .unwrap_or(DefId(0)); // Fallback for unresolved

        // Receiver becomes first argument
        let receiver_operand = self.lower_expr_as_operand(builder, receiver);
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
                    // Optimize simple binding case: avoid intermediate temp
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
                    // Complex patterns need place-based destructuring
                    HirPatKind::Tuple { .. }
                    | HirPatKind::Struct { .. }
                    | HirPatKind::Ref { .. } => {
                        if let Some(init_id) = init {
                            // Lower the initializer to a place
                            let init_place = self.lower_expr_to_place(builder, *init_id);
                            // Lower the pattern, binding variables to projections of init_place
                            self.lower_pattern(builder, *pat, init_place, span);
                        } else {
                            // No initializer - just allocate locals for bindings without assigning
                            self.lower_pattern_without_init(builder, *pat, span);
                        }
                    }
                    HirPatKind::Literal(_) | HirPatKind::Missing => {
                        // Literal patterns are for matching, Missing is error recovery
                        if let Some(init_id) = init {
                            let _ = self.lower_expr_to_place(builder, *init_id);
                        }
                    }
                }
            }
            HirStmtKind::Expr { expr, has_semi: _ } => {
                // Evaluate expression for side effects
                let _ = self.lower_expr_to_place(builder, *expr);
            }
        }
    }

    /// Lower a pattern, binding its variables to parts of the given place.
    ///
    /// This recursively destructures the pattern, creating field/deref projections
    /// as needed to extract values from the source place.
    fn lower_pattern(
        &mut self,
        builder: &mut MirBuilder,
        pat_id: crate::hir::PatId,
        source: Place,
        span: Span,
    ) {
        let pat = self.hir.pat(pat_id);
        match &pat.kind {
            HirPatKind::Bind { def_id, mutable } => {
                // Allocate a local for this binding
                let local = builder.alloc_local(pat.ty, *mutable, None);
                self.local_map.insert(*def_id, local);
                builder.push_statement(Statement::storage_live(local, span.clone()));

                // Copy from source place to the new local
                let dest = Place::from_local(local);
                builder.push_statement(Statement::assign(
                    dest,
                    Rvalue::Use(Operand::Copy(source)),
                    span,
                ));
            }
            HirPatKind::Wildcard => {
                // Wildcard doesn't bind anything - just evaluate source for side effects
                // (source is already evaluated)
            }
            HirPatKind::Tuple { elements } => {
                // For each element pattern, project the field and recursively lower
                for (idx, elem_pat_id) in elements.iter().enumerate() {
                    let field_place = Place {
                        local: source.local,
                        projection: {
                            let mut proj = source.projection.clone();
                            proj.push(PlaceElem::Field(FieldIdx(idx as u32)));
                            proj
                        },
                    };
                    self.lower_pattern(builder, *elem_pat_id, field_place, span.clone());
                }
            }
            HirPatKind::Struct {
                def_id,
                fields,
                rest: _,
            } => {
                // For each field pattern, look up the field index and project
                for (field_name, field_pat_id) in fields {
                    let field_idx = self.resolve_field_index(*def_id, field_name);
                    let field_place = Place {
                        local: source.local,
                        projection: {
                            let mut proj = source.projection.clone();
                            proj.push(PlaceElem::Field(FieldIdx(field_idx)));
                            proj
                        },
                    };
                    self.lower_pattern(builder, *field_pat_id, field_place, span.clone());
                }
            }
            HirPatKind::Ref { mutable: _, inner } => {
                // Deref the source to get the inner value
                let deref_place = Place {
                    local: source.local,
                    projection: {
                        let mut proj = source.projection.clone();
                        proj.push(PlaceElem::Deref);
                        proj
                    },
                };
                self.lower_pattern(builder, *inner, deref_place, span);
            }
            HirPatKind::Literal(_) => {
                // Literal patterns are for matching, not binding - nothing to do
            }
            HirPatKind::Missing => {
                // Missing pattern - nothing to bind
            }
        }
    }

    /// Lower a pattern without an initializer (just allocate locals).
    fn lower_pattern_without_init(
        &mut self,
        builder: &mut MirBuilder,
        pat_id: crate::hir::PatId,
        span: Span,
    ) {
        let pat = self.hir.pat(pat_id);
        match &pat.kind {
            HirPatKind::Bind { def_id, mutable } => {
                let local = builder.alloc_local(pat.ty, *mutable, None);
                self.local_map.insert(*def_id, local);
                builder.push_statement(Statement::storage_live(local, span));
            }
            HirPatKind::Tuple { elements } => {
                for elem_pat_id in elements {
                    self.lower_pattern_without_init(builder, *elem_pat_id, span.clone());
                }
            }
            HirPatKind::Struct { fields, .. } => {
                for (_, field_pat_id) in fields {
                    self.lower_pattern_without_init(builder, *field_pat_id, span.clone());
                }
            }
            HirPatKind::Ref { inner, .. } => {
                self.lower_pattern_without_init(builder, *inner, span);
            }
            HirPatKind::Wildcard | HirPatKind::Literal(_) | HirPatKind::Missing => {
                // Nothing to allocate
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

        // Preserve function metadata (def_id and name) from HIR
        builder.finish_with_metadata(func.params.len(), func.def_id, func.name.clone())
    }
}
