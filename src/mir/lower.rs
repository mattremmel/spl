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
use crate::mir::operand::{BinOp, Constant, Operand, Rvalue, UnOp};
use crate::mir::statement::Statement;
use crate::mir::terminator::{BasicBlock, Terminator, TerminatorKind};
use crate::mir::types::{Local, Place};
use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;

use rustc_hash::FxHashMap;

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

/// Context for lowering HIR to MIR.
///
/// This maintains state during the lowering process, including:
/// - Reference to the HIR database
/// - Mapping from DefIds to MIR locals
/// - The collection of lowered function bodies
pub struct MirLoweringContext<'hir> {
    /// Reference to the HIR database.
    pub hir: &'hir HirDatabase,
    /// Map from binding DefIds to their MIR locals.
    local_map: FxHashMap<DefId, Local>,
    /// Lowered function bodies.
    pub bodies: Vec<Body>,
}

impl<'hir> MirLoweringContext<'hir> {
    /// Create a new lowering context.
    pub fn new(hir: &'hir HirDatabase) -> Self {
        MirLoweringContext {
            hir,
            local_map: FxHashMap::default(),
            bodies: Vec::new(),
        }
    }

    /// Start lowering a function, setting up the builder with parameters.
    pub fn start_function(&mut self, func: &HirFunction) -> MirBuilder {
        let mut builder = MirBuilder::new(func.ret_type);

        // Clear the local map for this function
        self.local_map.clear();

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
                // Check if this is a simple binary op (not short-circuit or assignment)
                if let Some(mir_op) = hir_binop_to_mir(*op) {
                    // Lower operands
                    let lhs_operand = self.lower_expr_as_operand(builder, *lhs);
                    let rhs_operand = self.lower_expr_as_operand(builder, *rhs);

                    // Allocate temp for result
                    let temp = builder.alloc_temp(ty);
                    let place = Place::from_local(temp);

                    // Emit binary operation
                    let stmt = Statement::assign(
                        place.clone(),
                        Rvalue::BinaryOp(mir_op, lhs_operand, rhs_operand),
                        span,
                    );
                    builder.push_statement(stmt);
                    place
                } else {
                    // Short-circuit or assignment ops - handled elsewhere
                    // For now, allocate a temp as placeholder
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
    use crate::mir::statement::StatementKind;

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
}
