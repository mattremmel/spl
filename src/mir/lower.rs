//! HIR to MIR lowering.
//!
//! This module provides the infrastructure for lowering HIR (High-level IR)
//! to MIR (Mid-level IR). The lowering process converts nested expressions
//! into a flat, control-flow-graph representation suitable for borrow checking
//! and optimization.

use crate::hir::{ExprId, HirDatabase, HirExprKind, HirFunction, HirItem, Literal};
use crate::lexer::Span;
use crate::mir::body::{BasicBlockData, Body, LocalDecl};
use crate::mir::operand::{Constant, Operand, Rvalue};
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
            HirExprKind::Block { stmts, tail } => {
                // Lower statements (for now, skip them since we only handle literals)
                let _ = stmts;

                // If there's a tail expression, lower it
                if let Some(tail_id) = tail {
                    self.lower_expr_to_place(builder, *tail_id)
                } else {
                    // Unit return - create a unit place
                    let unit_ty = self.hir.types.unit();
                    let temp = builder.alloc_temp(unit_ty);
                    Place::from_local(temp)
                }
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
            _ => {
                // For other expressions, lower to place then copy
                let place = self.lower_expr_to_place(builder, expr_id);
                Operand::Copy(place)
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
}
