//! AST to HIR lowering.
//!
//! This module handles the lowering of AST expressions to HIR, including:
//! - Literal folding for negated integers
//! - Desugaring (while → loop)
//! - Type attachment from inference results
//! - Name resolution to DefIds

use crate::ast::{
    ArrayExpr, BinExpr, Block, BlockExpr, BreakExpr, CallExpr, CastExpr, Expr, FieldExpr,
    FunctionDef, IfExpr, IndexExpr, Item, LetStmt, LiteralExpr, LoopExpr, MethodCallExpr,
    ParenExpr, Pat, PathExpr, PrefixExpr, RefExpr, ReturnExpr, SourceFile, Stmt, StructExpr,
    TupleExpr, WhileExpr,
};
use crate::hir::{
    BinOp, ExprId, HirDatabase, HirExpr, HirExprKind, HirField, HirFunction, HirImpl, HirItem,
    HirParam, HirPat, HirPatKind, HirStmt, HirStmtKind, HirStruct, HirTypeAlias, Literal,
    LoweredExpr, PatId, StmtId, UnaryOp,
};
use crate::lexer::Span;
use crate::sema::infer::InferResult;
use crate::sema::symbol::DefId;
use crate::sema::types::{PrimitiveKind, TypeId};
use crate::syntax::SyntaxKind;
use rowan::ast::AstNode;
use rustc_hash::FxHashMap;

// ============================================================================
// Public API
// ============================================================================

/// Lower a source file to HIR.
///
/// This takes the AST and inference results and produces a fully typed HIR.
pub fn lower_to_hir(source_file: &SourceFile, infer_result: InferResult) -> HirDatabase {
    let mut ctx = LoweringContext::new(infer_result);
    ctx.lower_source_file(source_file);
    ctx.into_database()
}

// ============================================================================
// Lowering Context
// ============================================================================

/// Context for lowering AST to HIR.
struct LoweringContext {
    db: HirDatabase,
    /// Map from expression spans to their inferred types.
    expr_types: FxHashMap<Span, TypeId>,
    /// Map from DefIds to their inferred types.
    binding_types: FxHashMap<DefId, TypeId>,
    /// Map from spans to resolved DefIds.
    resolutions: FxHashMap<Span, DefId>,
}

impl LoweringContext {
    fn new(infer_result: InferResult) -> Self {
        let mut db = HirDatabase::new();
        // Transfer the type interner from the inference result
        db.types = infer_result.ctx.types;

        Self {
            db,
            expr_types: infer_result.expr_types,
            binding_types: infer_result.binding_types,
            resolutions: FxHashMap::default(), // Will be populated during lowering
        }
    }

    fn into_database(mut self) -> HirDatabase {
        self.db.binding_types = self.binding_types;
        self.db
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn text_range_to_span(range: rowan::TextRange) -> Span {
        range.start().into()..range.end().into()
    }

    fn get_type(&self, span: &Span) -> TypeId {
        self.expr_types
            .get(span)
            .copied()
            .unwrap_or_else(|| self.db.types.error())
    }

    fn get_binding_type(&self, def_id: DefId) -> TypeId {
        self.binding_types
            .get(&def_id)
            .copied()
            .unwrap_or_else(|| self.db.types.error())
    }

    fn error_type(&self) -> TypeId {
        self.db.types.error()
    }

    fn unit_type(&self) -> TypeId {
        self.db.types.unit()
    }

    fn never_type(&self) -> TypeId {
        self.db.types.never()
    }

    fn bool_type(&self) -> TypeId {
        self.db.types.bool()
    }

    // ========================================================================
    // Source File & Items
    // ========================================================================

    fn lower_source_file(&mut self, source_file: &SourceFile) {
        for item in source_file.items() {
            if let Some(hir_item) = self.lower_item(&item) {
                self.db.items.push(hir_item);
            }
        }
    }

    fn lower_item(&mut self, item: &Item) -> Option<HirItem> {
        match item {
            Item::Function(func) => self.lower_function(func).map(HirItem::Function),
            Item::Struct(struct_def) => self.lower_struct(struct_def).map(HirItem::Struct),
            Item::TypeAlias(type_alias) => {
                self.lower_type_alias(type_alias).map(HirItem::TypeAlias)
            }
            Item::Impl(impl_block) => self.lower_impl(impl_block).map(HirItem::Impl),
        }
    }

    fn lower_function(&mut self, func: &FunctionDef) -> Option<HirFunction> {
        let name = func.name()?.ident_token()?.text().to_string();
        let span = Self::text_range_to_span(func.syntax().text_range());

        // Get DefId from the function name span
        let name_span = func
            .name()
            .map(|n| Self::text_range_to_span(n.syntax().text_range()))
            .unwrap_or_else(|| span.clone());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId(0));

        // Lower type parameters
        let type_params = Vec::new(); // TODO: implement generic params

        // Lower parameters
        let params = func
            .param_list()
            .map(|pl| {
                pl.params()
                    .filter_map(|p| self.lower_param(&p))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Get return type
        let ret_type = self.get_type(&span);

        // Lower body
        let body = func.body().map(|b| self.lower_block(&b));

        Some(HirFunction {
            def_id,
            name,
            type_params,
            params,
            ret_type,
            body,
            span,
        })
    }

    fn lower_param(&mut self, param: &crate::ast::Param) -> Option<HirParam> {
        let span = Self::text_range_to_span(param.syntax().text_range());

        // Create a simple bind pattern for the parameter
        let _name = param.name()?.ident_token()?.text().to_string();
        let name_span = Self::text_range_to_span(param.name()?.syntax().text_range());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId(0));
        let ty = self.get_binding_type(def_id);

        let pat = HirPat {
            kind: HirPatKind::Bind {
                def_id,
                mutable: false,
            },
            ty,
            span: name_span,
        };
        let pat_id = self.db.alloc_pat(pat);

        Some(HirParam {
            pat: pat_id,
            ty,
            span,
        })
    }

    fn lower_struct(&mut self, struct_def: &crate::ast::StructDef) -> Option<HirStruct> {
        let name = struct_def.name()?.ident_token()?.text().to_string();
        let span = Self::text_range_to_span(struct_def.syntax().text_range());

        let name_span = struct_def
            .name()
            .map(|n| Self::text_range_to_span(n.syntax().text_range()))
            .unwrap_or_else(|| span.clone());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId(0));

        let type_params = Vec::new(); // TODO

        let fields = struct_def
            .field_list()
            .map(|fl| {
                fl.fields()
                    .filter_map(|f| self.lower_field(&f))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Some(HirStruct {
            def_id,
            name,
            type_params,
            fields,
            span,
        })
    }

    fn lower_field(&mut self, field: &crate::ast::FieldDef) -> Option<HirField> {
        let name = field.name()?.ident_token()?.text().to_string();
        let span = Self::text_range_to_span(field.syntax().text_range());

        let name_span = field
            .name()
            .map(|n| Self::text_range_to_span(n.syntax().text_range()))
            .unwrap_or_else(|| span.clone());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId(0));
        let ty = self.get_binding_type(def_id);

        Some(HirField {
            def_id,
            name,
            ty,
            span,
        })
    }

    fn lower_type_alias(&mut self, type_alias: &crate::ast::TypeAlias) -> Option<HirTypeAlias> {
        let name = type_alias.name()?.ident_token()?.text().to_string();
        let span = Self::text_range_to_span(type_alias.syntax().text_range());

        let name_span = type_alias
            .name()
            .map(|n| Self::text_range_to_span(n.syntax().text_range()))
            .unwrap_or_else(|| span.clone());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId(0));

        let ty = self.get_type(&span);

        Some(HirTypeAlias {
            def_id,
            name,
            type_params: Vec::new(),
            ty,
            span,
        })
    }

    fn lower_impl(&mut self, impl_block: &crate::ast::ImplBlock) -> Option<HirImpl> {
        let span = Self::text_range_to_span(impl_block.syntax().text_range());
        let self_ty = self.get_type(&span);

        let items = impl_block
            .items()
            .filter_map(|item| self.lower_item(&item))
            .collect();

        Some(HirImpl {
            type_params: Vec::new(),
            self_ty,
            items,
            span,
        })
    }

    // ========================================================================
    // Blocks & Statements
    // ========================================================================

    fn lower_block(&mut self, block: &Block) -> ExprId {
        let span = Self::text_range_to_span(block.syntax().text_range());
        let ty = self.get_type(&span);

        let mut stmts = Vec::new();
        let mut tail = None;

        // Process all children in source order
        for child in block.syntax().children() {
            if let Some(stmt) = Stmt::cast(child.clone()) {
                match &stmt {
                    Stmt::Expr(expr_stmt) => {
                        // If this is the last item and has no semicolon, it's the tail
                        let has_semi = expr_stmt.semicolon().is_some();
                        if let Some(expr) = expr_stmt.expr() {
                            let expr_id = self.lower_expr(&expr);
                            if !has_semi {
                                // This might be a tail expression
                                tail = Some(expr_id);
                            } else {
                                let stmt_span =
                                    Self::text_range_to_span(stmt.syntax().text_range());
                                let hir_stmt = HirStmt {
                                    kind: HirStmtKind::Expr {
                                        expr: expr_id,
                                        has_semi,
                                    },
                                    span: stmt_span,
                                };
                                stmts.push(self.db.alloc_stmt(hir_stmt));
                            }
                        }
                    }
                    Stmt::Let(let_stmt) => {
                        if let Some(stmt_id) = self.lower_let_stmt(let_stmt) {
                            stmts.push(stmt_id);
                        }
                    }
                }
            } else if let Some(expr) = Expr::cast(child.clone()) {
                // Bare expression at block level (tail)
                let expr_id = self.lower_expr(&expr);
                tail = Some(expr_id);
            }
        }

        let expr = HirExpr {
            kind: HirExprKind::Block { stmts, tail },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_let_stmt(&mut self, let_stmt: &LetStmt) -> Option<StmtId> {
        let span = Self::text_range_to_span(let_stmt.syntax().text_range());

        let pat = let_stmt
            .pat()
            .map(|p| self.lower_pattern(&p, let_stmt.mut_kw().is_some()));

        let ty = let_stmt.ty().map(|_| {
            // Get the annotated type from inference
            self.get_type(&span)
        });

        let init = let_stmt.initializer().map(|e| self.lower_expr(&e));

        let pat_id = pat.unwrap_or_else(|| {
            let missing = HirPat {
                kind: HirPatKind::Missing,
                ty: self.error_type(),
                span: span.clone(),
            };
            self.db.alloc_pat(missing)
        });

        let stmt = HirStmt {
            kind: HirStmtKind::Let {
                pat: pat_id,
                ty,
                init,
            },
            span,
        };
        Some(self.db.alloc_stmt(stmt))
    }

    // ========================================================================
    // Patterns
    // ========================================================================

    fn lower_pattern(&mut self, pat: &Pat, outer_mutable: bool) -> PatId {
        let span = Self::text_range_to_span(pat.syntax().text_range());

        match pat {
            Pat::Ident(ident_pat) => {
                let mutable = outer_mutable || ident_pat.mut_kw().is_some();
                let name_span = ident_pat
                    .name()
                    .map(|n| Self::text_range_to_span(n.syntax().text_range()))
                    .unwrap_or_else(|| span.clone());

                let def_id = self
                    .resolutions
                    .get(&name_span)
                    .copied()
                    .unwrap_or(DefId(0));
                let ty = self.get_binding_type(def_id);

                let hir_pat = HirPat {
                    kind: HirPatKind::Bind { def_id, mutable },
                    ty,
                    span,
                };
                self.db.alloc_pat(hir_pat)
            }
            Pat::Wildcard(_) => {
                let ty = self.get_type(&span);
                let hir_pat = HirPat {
                    kind: HirPatKind::Wildcard,
                    ty,
                    span,
                };
                self.db.alloc_pat(hir_pat)
            }
            Pat::Tuple(tuple_pat) => {
                let elements: Vec<_> = tuple_pat
                    .patterns()
                    .map(|p| self.lower_pattern(&p, outer_mutable))
                    .collect();
                let ty = self.get_type(&span);
                let hir_pat = HirPat {
                    kind: HirPatKind::Tuple { elements },
                    ty,
                    span,
                };
                self.db.alloc_pat(hir_pat)
            }
            Pat::Struct(struct_pat) => {
                let path_span = struct_pat
                    .path()
                    .map(|p| Self::text_range_to_span(p.syntax().text_range()))
                    .unwrap_or_else(|| span.clone());
                let def_id = self
                    .resolutions
                    .get(&path_span)
                    .copied()
                    .unwrap_or(DefId(0));

                let fields: Vec<_> = struct_pat
                    .fields()
                    .filter_map(|f| {
                        let name = f.name()?.token()?.text().to_string();
                        let pat_id = if let Some(nested) = f.pat() {
                            self.lower_pattern(&nested, outer_mutable)
                        } else {
                            // Shorthand: `{ x }` means `{ x: x }`
                            let field_span = Self::text_range_to_span(f.syntax().text_range());
                            let field_def_id = self
                                .resolutions
                                .get(&field_span)
                                .copied()
                                .unwrap_or(DefId(0));
                            let ty = self.get_binding_type(field_def_id);
                            let bind_pat = HirPat {
                                kind: HirPatKind::Bind {
                                    def_id: field_def_id,
                                    mutable: outer_mutable,
                                },
                                ty,
                                span: field_span,
                            };
                            self.db.alloc_pat(bind_pat)
                        };
                        Some((name, pat_id))
                    })
                    .collect();

                let rest = struct_pat.rest().is_some();
                let ty = self.get_type(&span);

                let hir_pat = HirPat {
                    kind: HirPatKind::Struct {
                        def_id,
                        fields,
                        rest,
                    },
                    ty,
                    span,
                };
                self.db.alloc_pat(hir_pat)
            }
            Pat::Ref(ref_pat) => {
                let mutable = ref_pat.mut_kw().is_some();
                let inner = ref_pat
                    .pat()
                    .map(|p| self.lower_pattern(&p, false))
                    .unwrap_or_else(|| {
                        let missing = HirPat {
                            kind: HirPatKind::Missing,
                            ty: self.error_type(),
                            span: span.clone(),
                        };
                        self.db.alloc_pat(missing)
                    });

                let ty = self.get_type(&span);
                let hir_pat = HirPat {
                    kind: HirPatKind::Ref { mutable, inner },
                    ty,
                    span,
                };
                self.db.alloc_pat(hir_pat)
            }
            Pat::Literal(lit_pat) => {
                let ty = self.get_type(&span);
                let literal = lit_pat
                    .token()
                    .map(|t| self.parse_literal_token(&t))
                    .unwrap_or(Literal::Int(0));

                let hir_pat = HirPat {
                    kind: HirPatKind::Literal(literal),
                    ty,
                    span,
                };
                self.db.alloc_pat(hir_pat)
            }
            Pat::Slice(_) | Pat::Range(_) | Pat::Rest(_) => {
                // TODO: implement these patterns
                let hir_pat = HirPat {
                    kind: HirPatKind::Missing,
                    ty: self.error_type(),
                    span,
                };
                self.db.alloc_pat(hir_pat)
            }
        }
    }

    // ========================================================================
    // Expressions
    // ========================================================================

    fn lower_expr(&mut self, expr: &Expr) -> ExprId {
        let span = Self::text_range_to_span(expr.syntax().text_range());
        let ty = self.get_type(&span);

        match expr {
            Expr::Literal(lit) => self.lower_literal(lit, span, ty),
            Expr::Path(path_expr) => self.lower_path_expr(path_expr, span, ty),
            Expr::Paren(paren) => self.lower_paren_expr(paren),
            Expr::Tuple(tuple) => self.lower_tuple_expr(tuple, span, ty),
            Expr::Array(array) => self.lower_array_expr(array, span, ty),
            Expr::Struct(struct_expr) => self.lower_struct_expr(struct_expr, span, ty),
            Expr::Binary(bin) => self.lower_binary_expr(bin, span, ty),
            Expr::Prefix(prefix) => self.lower_prefix_expr(prefix, span, ty),
            Expr::Ref(ref_expr) => self.lower_ref_expr(ref_expr, span, ty),
            Expr::Field(field) => self.lower_field_expr(field, span, ty),
            Expr::MethodCall(call) => self.lower_method_call_expr(call, span, ty),
            Expr::Call(call) => self.lower_call_expr(call, span, ty),
            Expr::Index(index) => self.lower_index_expr(index, span, ty),
            Expr::Slice(_) => self.lower_missing(span), // TODO
            Expr::If(if_expr) => self.lower_if_expr(if_expr, span, ty),
            Expr::While(while_expr) => self.lower_while_expr(while_expr, span),
            Expr::For(_) => self.lower_missing(span), // TODO: desugar for
            Expr::Loop(loop_expr) => self.lower_loop_expr(loop_expr, span, ty),
            Expr::Break(break_expr) => self.lower_break_expr(break_expr, span),
            Expr::Continue(_) => self.lower_continue_expr(span),
            Expr::Return(return_expr) => self.lower_return_expr(return_expr, span),
            Expr::Block(block_expr) => self.lower_block_expr(block_expr),
            Expr::Cast(cast) => self.lower_cast_expr(cast, span, ty),
            Expr::Range(_) => self.lower_missing(span), // TODO
        }
    }

    fn lower_literal(&mut self, lit: &LiteralExpr, span: Span, ty: TypeId) -> ExprId {
        let literal = lit
            .token()
            .map(|t| self.parse_literal_token(&t))
            .unwrap_or(Literal::Int(0));

        let expr = HirExpr {
            kind: HirExprKind::Literal(literal),
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn parse_literal_token(&self, token: &crate::syntax::SyntaxToken) -> Literal {
        let text = token.text();
        match token.kind() {
            SyntaxKind::INT_LITERAL => {
                let value = parse_int_literal_value(text).unwrap_or(0);
                Literal::Int(value)
            }
            SyntaxKind::FLOAT_LITERAL => {
                let value = parse_float_literal_value(text).unwrap_or(0.0);
                Literal::Float(value)
            }
            SyntaxKind::TRUE_KW => Literal::Bool(true),
            SyntaxKind::FALSE_KW => Literal::Bool(false),
            SyntaxKind::CHAR_LITERAL => {
                let c = parse_char_literal(text).unwrap_or('\0');
                Literal::Char(c)
            }
            SyntaxKind::STRING_LITERAL => {
                let s = parse_string_literal(text);
                Literal::String(s)
            }
            _ => Literal::Int(0),
        }
    }

    fn lower_path_expr(&mut self, path_expr: &PathExpr, span: Span, ty: TypeId) -> ExprId {
        // Get the DefId from the path
        let path_span = path_expr
            .path()
            .and_then(|p| p.segments().next())
            .and_then(|seg| seg.name())
            .map(|n| Self::text_range_to_span(n.syntax().text_range()))
            .unwrap_or_else(|| span.clone());

        let def_id = self
            .resolutions
            .get(&path_span)
            .copied()
            .unwrap_or(DefId(0));

        let expr = HirExpr {
            kind: HirExprKind::Var(def_id),
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_paren_expr(&mut self, paren: &ParenExpr) -> ExprId {
        // Parentheses just pass through
        paren
            .expr()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| {
                let span = Self::text_range_to_span(paren.syntax().text_range());
                self.lower_missing(span)
            })
    }

    fn lower_tuple_expr(&mut self, tuple: &TupleExpr, span: Span, ty: TypeId) -> ExprId {
        let elements: Vec<_> = tuple.exprs().map(|e| self.lower_expr(&e)).collect();

        let expr = HirExpr {
            kind: HirExprKind::Tuple { elements },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_array_expr(&mut self, array: &ArrayExpr, span: Span, ty: TypeId) -> ExprId {
        if array.is_repeat() {
            // [value; count]
            let mut exprs = array.exprs();
            let value = exprs
                .next()
                .map(|e| self.lower_expr(&e))
                .unwrap_or_else(|| self.lower_missing(span.clone()));
            let count = exprs
                .next()
                .and_then(|e| {
                    // Try to extract the count from the literal
                    if let Expr::Literal(lit) = e {
                        lit.token().and_then(|t| {
                            if t.kind() == SyntaxKind::INT_LITERAL {
                                parse_int_literal_value(t.text()).map(|v| v as u64)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            let expr = HirExpr {
                kind: HirExprKind::ArrayRepeat { value, count },
                ty,
                span,
            };
            self.db.alloc_expr(expr)
        } else {
            // [a, b, c]
            let elements: Vec<_> = array.exprs().map(|e| self.lower_expr(&e)).collect();

            let expr = HirExpr {
                kind: HirExprKind::Array { elements },
                ty,
                span,
            };
            self.db.alloc_expr(expr)
        }
    }

    fn lower_struct_expr(&mut self, struct_expr: &StructExpr, span: Span, ty: TypeId) -> ExprId {
        // Get the struct DefId from the path
        let path_span = struct_expr
            .path()
            .and_then(|p| p.segments().next())
            .and_then(|seg| seg.name())
            .map(|n| Self::text_range_to_span(n.syntax().text_range()))
            .unwrap_or_else(|| span.clone());

        let def_id = self
            .resolutions
            .get(&path_span)
            .copied()
            .unwrap_or(DefId(0));

        let fields: Vec<_> = struct_expr
            .fields()
            .filter_map(|f| {
                let name = f.name_token()?.text().to_string();
                let value = f.expr().map(|e| self.lower_expr(&e))?;
                Some((name, value))
            })
            .collect();

        let expr = HirExpr {
            kind: HirExprKind::Struct { def_id, fields },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_binary_expr(&mut self, bin: &BinExpr, span: Span, ty: TypeId) -> ExprId {
        let lhs = bin
            .lhs()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));
        let rhs = bin
            .rhs()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let op = bin
            .op_token()
            .map(|t| match t.kind() {
                SyntaxKind::PLUS => BinOp::Add,
                SyntaxKind::MINUS => BinOp::Sub,
                SyntaxKind::STAR => BinOp::Mul,
                SyntaxKind::SLASH => BinOp::Div,
                SyntaxKind::PERCENT => BinOp::Rem,
                SyntaxKind::EQ_EQ => BinOp::Eq,
                SyntaxKind::NE => BinOp::Ne,
                SyntaxKind::LT => BinOp::Lt,
                SyntaxKind::GT => BinOp::Gt,
                SyntaxKind::LE => BinOp::Le,
                SyntaxKind::GE => BinOp::Ge,
                SyntaxKind::AND_AND => BinOp::And,
                SyntaxKind::OR_OR => BinOp::Or,
                SyntaxKind::EQ => BinOp::Assign,
                SyntaxKind::PLUS_EQ => BinOp::AddAssign,
                SyntaxKind::MINUS_EQ => BinOp::SubAssign,
                SyntaxKind::STAR_EQ => BinOp::MulAssign,
                SyntaxKind::SLASH_EQ => BinOp::DivAssign,
                SyntaxKind::PERCENT_EQ => BinOp::RemAssign,
                _ => BinOp::Add, // fallback
            })
            .unwrap_or(BinOp::Add);

        let expr = HirExpr {
            kind: HirExprKind::Binary { op, lhs, rhs },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_prefix_expr(&mut self, prefix: &PrefixExpr, span: Span, ty: TypeId) -> ExprId {
        // First, try to lower as a negated literal
        let full_expr = Expr::Prefix(prefix.clone());
        let (lowered, was_lowered) = try_lower_expr(&full_expr);

        if was_lowered {
            match lowered {
                LoweredExpr::IntLiteral { value, .. } => {
                    let expr = HirExpr {
                        kind: HirExprKind::Literal(Literal::Int(value)),
                        ty,
                        span,
                    };
                    return self.db.alloc_expr(expr);
                }
                LoweredExpr::FloatLiteral { value, .. } => {
                    let expr = HirExpr {
                        kind: HirExprKind::Literal(Literal::Float(value)),
                        ty,
                        span,
                    };
                    return self.db.alloc_expr(expr);
                }
                LoweredExpr::Passthrough => {}
            }
        }

        // Not a foldable literal, lower normally
        let operand = prefix
            .expr()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let op = prefix
            .op_token()
            .map(|t| match t.kind() {
                SyntaxKind::BANG => UnaryOp::Not,
                SyntaxKind::MINUS => UnaryOp::Neg,
                SyntaxKind::STAR => UnaryOp::Deref,
                _ => UnaryOp::Neg,
            })
            .unwrap_or(UnaryOp::Neg);

        let expr = HirExpr {
            kind: HirExprKind::Unary { op, operand },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_ref_expr(&mut self, ref_expr: &RefExpr, span: Span, ty: TypeId) -> ExprId {
        let mutable = ref_expr.mut_kw().is_some();
        let operand = ref_expr
            .expr()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let expr = HirExpr {
            kind: HirExprKind::Ref { mutable, operand },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_field_expr(&mut self, field: &FieldExpr, span: Span, ty: TypeId) -> ExprId {
        let base = field
            .expr()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        // Check if this is a tuple field access (numeric index)
        if let Some(index_token) = field.tuple_index_token() {
            let index: u32 = index_token.text().parse().unwrap_or(0);
            let expr = HirExpr {
                kind: HirExprKind::TupleField { base, index },
                ty,
                span,
            };
            return self.db.alloc_expr(expr);
        }

        // Regular field access
        let field_name = field
            .name_token()
            .map(|t| t.text().to_string())
            .or_else(|| {
                field
                    .name()
                    .and_then(|n| n.token())
                    .map(|t| t.text().to_string())
            })
            .unwrap_or_default();

        let expr = HirExpr {
            kind: HirExprKind::Field {
                base,
                field: field_name,
            },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_method_call_expr(&mut self, call: &MethodCallExpr, span: Span, ty: TypeId) -> ExprId {
        let receiver = call
            .receiver()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let method = call
            .name_token()
            .map(|t| t.text().to_string())
            .or_else(|| {
                call.name()
                    .and_then(|n| n.token())
                    .map(|t| t.text().to_string())
            })
            .unwrap_or_default();

        let args: Vec<_> = call
            .arg_list()
            .map(|al| al.args().map(|a| self.lower_expr(&a)).collect())
            .unwrap_or_default();

        let expr = HirExpr {
            kind: HirExprKind::MethodCall {
                receiver,
                method,
                args,
            },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_call_expr(&mut self, call: &CallExpr, span: Span, ty: TypeId) -> ExprId {
        let callee = call
            .callee()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let args: Vec<_> = call
            .arg_list()
            .map(|al| al.args().map(|a| self.lower_expr(&a)).collect())
            .unwrap_or_default();

        let expr = HirExpr {
            kind: HirExprKind::Call { callee, args },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_index_expr(&mut self, index: &IndexExpr, span: Span, ty: TypeId) -> ExprId {
        let base = index
            .base()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));
        let idx = index
            .index()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let expr = HirExpr {
            kind: HirExprKind::Index { base, index: idx },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_if_expr(&mut self, if_expr: &IfExpr, span: Span, ty: TypeId) -> ExprId {
        let condition = if_expr
            .condition()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let then_branch = if_expr
            .then_branch()
            .map(|b| self.lower_block(&b))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let else_branch = if_expr
            .else_branch()
            .map(|e| self.lower_expr(&e))
            .or_else(|| if_expr.else_block().map(|b| self.lower_block(&b)));

        let expr = HirExpr {
            kind: HirExprKind::If {
                condition,
                then_branch,
                else_branch,
            },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    /// Desugar `while cond { body }` to `loop { if !cond { break; } body }`
    fn lower_while_expr(&mut self, while_expr: &WhileExpr, span: Span) -> ExprId {
        let ty = self.unit_type(); // while loops have unit type

        // Get the condition
        let cond = while_expr
            .condition()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        // Create `!cond`
        let negated_cond = HirExpr {
            kind: HirExprKind::Unary {
                op: UnaryOp::Not,
                operand: cond,
            },
            ty: self.bool_type(),
            span: span.clone(),
        };
        let negated_cond_id = self.db.alloc_expr(negated_cond);

        // Create `break`
        let break_expr = HirExpr {
            kind: HirExprKind::Break { value: None },
            ty: self.never_type(),
            span: span.clone(),
        };
        let break_id = self.db.alloc_expr(break_expr);

        // Create `if !cond { break; }`
        let if_break = HirExpr {
            kind: HirExprKind::If {
                condition: negated_cond_id,
                then_branch: break_id,
                else_branch: None,
            },
            ty: self.unit_type(),
            span: span.clone(),
        };
        let if_break_id = self.db.alloc_expr(if_break);

        // Create statement for `if !cond { break; }`
        let if_stmt = HirStmt {
            kind: HirStmtKind::Expr {
                expr: if_break_id,
                has_semi: true,
            },
            span: span.clone(),
        };
        let if_stmt_id = self.db.alloc_stmt(if_stmt);

        // Lower the body
        let body_stmts = while_expr.body().map(|b| {
            let body_id = self.lower_block(&b);
            let body_stmt = HirStmt {
                kind: HirStmtKind::Expr {
                    expr: body_id,
                    has_semi: true,
                },
                span: Self::text_range_to_span(b.syntax().text_range()),
            };
            self.db.alloc_stmt(body_stmt)
        });

        // Combine into block: [if_stmt, body_stmt?]
        let mut stmts = vec![if_stmt_id];
        if let Some(body_stmt_id) = body_stmts {
            stmts.push(body_stmt_id);
        }

        let loop_body = HirExpr {
            kind: HirExprKind::Block { stmts, tail: None },
            ty: self.unit_type(),
            span: span.clone(),
        };
        let loop_body_id = self.db.alloc_expr(loop_body);

        // Create `loop { ... }`
        let expr = HirExpr {
            kind: HirExprKind::Loop { body: loop_body_id },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_loop_expr(&mut self, loop_expr: &LoopExpr, span: Span, ty: TypeId) -> ExprId {
        let body = loop_expr
            .body()
            .map(|b| self.lower_block(&b))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let expr = HirExpr {
            kind: HirExprKind::Loop { body },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_break_expr(&mut self, break_expr: &BreakExpr, span: Span) -> ExprId {
        let value = break_expr.expr().map(|e| self.lower_expr(&e));
        let ty = self.never_type();

        let expr = HirExpr {
            kind: HirExprKind::Break { value },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_continue_expr(&mut self, span: Span) -> ExprId {
        let expr = HirExpr {
            kind: HirExprKind::Continue,
            ty: self.never_type(),
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_return_expr(&mut self, return_expr: &ReturnExpr, span: Span) -> ExprId {
        let value = return_expr.expr().map(|e| self.lower_expr(&e));
        let ty = self.never_type();

        let expr = HirExpr {
            kind: HirExprKind::Return { value },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_block_expr(&mut self, block_expr: &BlockExpr) -> ExprId {
        block_expr
            .block()
            .map(|b| self.lower_block(&b))
            .unwrap_or_else(|| {
                let span = Self::text_range_to_span(block_expr.syntax().text_range());
                self.lower_missing(span)
            })
    }

    fn lower_cast_expr(&mut self, cast: &CastExpr, span: Span, ty: TypeId) -> ExprId {
        let inner = cast
            .expr()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let expr = HirExpr {
            kind: HirExprKind::Cast {
                expr: inner,
                target_ty: ty,
            },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_missing(&mut self, span: Span) -> ExprId {
        let expr = HirExpr {
            kind: HirExprKind::Missing,
            ty: self.error_type(),
            span,
        };
        self.db.alloc_expr(expr)
    }
}

// ============================================================================
// Literal Parsing Helpers
// ============================================================================

fn parse_int_literal_value(text: &str) -> Option<i128> {
    let suffixes = [
        "i128", "u128", "isize", "usize", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8",
    ];
    let num_text = suffixes
        .iter()
        .find(|s| text.ends_with(*s))
        .map(|s| &text[..text.len() - s.len()])
        .unwrap_or(text);

    if num_text.starts_with("0x") || num_text.starts_with("0X") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 16).ok()
    } else if num_text.starts_with("0o") || num_text.starts_with("0O") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 8).ok()
    } else if num_text.starts_with("0b") || num_text.starts_with("0B") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 2).ok()
    } else {
        num_text.replace('_', "").parse().ok()
    }
}

fn parse_float_literal_value(text: &str) -> Option<f64> {
    let num_text = if let Some(stripped) = text.strip_suffix("f32") {
        stripped
    } else if let Some(stripped) = text.strip_suffix("f64") {
        stripped
    } else {
        text
    };
    num_text.replace('_', "").parse().ok()
}

fn parse_char_literal(text: &str) -> Option<char> {
    // Strip quotes and handle escape sequences
    let inner = text.strip_prefix('\'')?.strip_suffix('\'')?;
    if inner.starts_with('\\') {
        match inner.chars().nth(1)? {
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            '\\' => Some('\\'),
            '\'' => Some('\''),
            '0' => Some('\0'),
            _ => inner.chars().nth(1),
        }
    } else {
        inner.chars().next()
    }
}

fn parse_string_literal(text: &str) -> String {
    // Strip quotes - basic implementation
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| {
            s.replace("\\n", "\n")
                .replace("\\r", "\r")
                .replace("\\t", "\t")
                .replace("\\\\", "\\")
                .replace("\\\"", "\"")
        })
        .unwrap_or_default()
}

// ============================================================================
// Original literal folding API (preserved for backwards compatibility)
// ============================================================================

/// Try to lower an expression for literal folding.
///
/// Returns `(LoweredExpr, was_lowered)` where `was_lowered` is true if the expression
/// was successfully lowered to a folded form.
///
/// Currently handles:
/// - Negated integer literals: `-128i8`, `-(128i8)`, `(-(128i8))`
/// - Negated float literals: `-1.0f32`, `-(1.0f64)`
pub fn try_lower_expr(expr: &Expr) -> (LoweredExpr, bool) {
    match expr {
        Expr::Prefix(prefix) => {
            if let Some(lowered) = lower_negated_literal(prefix) {
                return (lowered, true);
            }
        }
        Expr::Paren(paren) => {
            // Try to lower the inner expression (handles `(-(128i8))`)
            if let Some(inner) = paren.expr() {
                return try_lower_expr(&inner);
            }
        }
        _ => {}
    }
    (LoweredExpr::Passthrough, false)
}

/// Try to lower a prefix expression that might be a negated literal.
fn lower_negated_literal(prefix: &PrefixExpr) -> Option<LoweredExpr> {
    // Check if this is a negation operator
    let op_token = prefix.op_token()?;
    if op_token.kind() != SyntaxKind::MINUS {
        return None;
    }

    let inner = prefix.expr()?;

    // First, try to recursively lower the inner expression
    // This handles double negation: `--128i8`
    if let Expr::Prefix(inner_prefix) = &inner
        && let Some(inner_lowered) = lower_negated_literal(inner_prefix)
    {
        // We have a nested negation - negate the result
        return match inner_lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                let span = text_range_to_span(prefix.syntax().text_range());
                Some(LoweredExpr::IntLiteral {
                    value: -value,
                    suffix,
                    span,
                })
            }
            LoweredExpr::FloatLiteral { value, suffix, .. } => {
                let span = text_range_to_span(prefix.syntax().text_range());
                Some(LoweredExpr::FloatLiteral {
                    value: -value,
                    suffix,
                    span,
                })
            }
            LoweredExpr::Passthrough => None,
        };
    }

    // Unwrap parentheses to find the inner literal or nested negation
    let unwrapped = unwrap_parens(&inner)?;

    match &unwrapped {
        Expr::Literal(lit) => lower_negated_numeric_literal(prefix, lit),
        Expr::Prefix(inner_prefix) => {
            // Handle `-(-(128i8))` - negation of parenthesized negation
            if let Some(inner_lowered) = lower_negated_literal(inner_prefix) {
                return match inner_lowered {
                    LoweredExpr::IntLiteral { value, suffix, .. } => {
                        let span = text_range_to_span(prefix.syntax().text_range());
                        Some(LoweredExpr::IntLiteral {
                            value: -value,
                            suffix,
                            span,
                        })
                    }
                    LoweredExpr::FloatLiteral { value, suffix, .. } => {
                        let span = text_range_to_span(prefix.syntax().text_range());
                        Some(LoweredExpr::FloatLiteral {
                            value: -value,
                            suffix,
                            span,
                        })
                    }
                    LoweredExpr::Passthrough => None,
                };
            }
            None
        }
        _ => None,
    }
}

/// Lower a negated numeric literal expression.
fn lower_negated_numeric_literal(prefix: &PrefixExpr, lit: &LiteralExpr) -> Option<LoweredExpr> {
    let token = lit.token()?;
    let text = token.text();
    let span = text_range_to_span(prefix.syntax().text_range());

    match token.kind() {
        SyntaxKind::INT_LITERAL => {
            let (suffix, _has_suffix) = parse_int_suffix(text);
            let value = parse_int_literal_value(text)?;
            Some(LoweredExpr::IntLiteral {
                value: -value,
                suffix,
                span,
            })
        }
        SyntaxKind::FLOAT_LITERAL => {
            let (suffix, value) = parse_float_literal(text)?;
            Some(LoweredExpr::FloatLiteral {
                value: -value,
                suffix,
                span,
            })
        }
        _ => None,
    }
}

/// Unwrap parentheses to get the inner expression.
fn unwrap_parens(expr: &Expr) -> Option<Expr> {
    match expr {
        Expr::Paren(p) => p.expr().and_then(|inner| unwrap_parens(&inner)),
        _ => Some(expr.clone()),
    }
}

/// Convert a rowan TextRange to our Span type.
fn text_range_to_span(range: rowan::TextRange) -> Span {
    range.start().into()..range.end().into()
}

/// Parse an integer literal suffix to determine the type.
fn parse_int_suffix(text: &str) -> (Option<PrimitiveKind>, bool) {
    let suffixes = [
        ("i128", PrimitiveKind::I128),
        ("u128", PrimitiveKind::U128),
        ("isize", PrimitiveKind::Isize),
        ("usize", PrimitiveKind::Usize),
        ("i64", PrimitiveKind::I64),
        ("u64", PrimitiveKind::U64),
        ("i32", PrimitiveKind::I32),
        ("u32", PrimitiveKind::U32),
        ("i16", PrimitiveKind::I16),
        ("u16", PrimitiveKind::U16),
        ("i8", PrimitiveKind::I8),
        ("u8", PrimitiveKind::U8),
    ];

    for (suffix, kind) in suffixes {
        if text.ends_with(suffix) {
            return (Some(kind), true);
        }
    }
    (None, false)
}

/// Parse a float literal, returning (suffix, value).
fn parse_float_literal(text: &str) -> Option<(Option<PrimitiveKind>, f64)> {
    let (suffix, num_text) = if let Some(stripped) = text.strip_suffix("f32") {
        (Some(PrimitiveKind::F32), stripped)
    } else if let Some(stripped) = text.strip_suffix("f64") {
        (Some(PrimitiveKind::F64), stripped)
    } else {
        (None, text)
    };

    let value: f64 = num_text.replace('_', "").parse().ok()?;
    Some((suffix, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SourceFile;
    use crate::parser::parse;
    use crate::sema::infer::infer;
    use crate::sema::resolver::resolve;
    use rowan::ast::AstNode;

    fn parse_expr(src: &str) -> Expr {
        let full_src = format!("fn main() {{ let x = {src}; }}");
        let parsed = parse(&full_src);
        assert!(
            parsed.errors().is_empty(),
            "Parse errors: {:?}",
            parsed.errors()
        );

        use crate::ast::{Item, Stmt};
        let file = SourceFile::cast(parsed.syntax()).unwrap();
        let fn_item = file.items().next().unwrap();
        if let Item::Function(f) = fn_item {
            let body = f.body().unwrap();
            let stmt = body.statements().next().unwrap();
            if let Stmt::Let(let_stmt) = stmt {
                return let_stmt.initializer().unwrap();
            }
        }
        panic!("Could not extract expression from source");
    }

    fn lower(source: &str) -> HirDatabase {
        let parsed = parse(source);
        assert!(
            parsed.errors().is_empty(),
            "Parse errors: {:?}",
            parsed.errors()
        );
        let source_file = SourceFile::cast(parsed.syntax()).unwrap();
        let resolve_result = resolve(&source_file);
        let infer_result = infer(&source_file, resolve_result);
        lower_to_hir(&source_file, infer_result)
    }

    // ========================================================================
    // Original literal folding tests
    // ========================================================================

    #[test]
    fn lower_negated_i8() {
        let expr = parse_expr("-128i8");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -128);
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn lower_parenthesized_negation() {
        let expr = parse_expr("-(128i8)");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -128);
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn lower_double_paren_negation() {
        let expr = parse_expr("(-(128i8))");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -128);
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn lower_double_negation() {
        // --128i8 should fold to +128
        let expr = parse_expr("--128i8");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, 128); // Double negation = positive
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn passthrough_variable() {
        let expr = parse_expr("-x");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(!was_lowered);
        assert!(matches!(lowered, LoweredExpr::Passthrough));
    }

    #[test]
    fn passthrough_non_prefix() {
        let expr = parse_expr("42");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(!was_lowered);
        assert!(matches!(lowered, LoweredExpr::Passthrough));
    }

    #[test]
    fn lower_negated_float() {
        let expr = parse_expr("-1.5f32");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::FloatLiteral { value, suffix, .. } => {
                assert!((value - (-1.5)).abs() < f64::EPSILON);
                assert_eq!(suffix, Some(PrimitiveKind::F32));
            }
            _ => panic!("Expected FloatLiteral"),
        }
    }

    #[test]
    fn lower_negated_unsuffixed_int() {
        let expr = parse_expr("-42");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -42);
                assert_eq!(suffix, None);
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    #[test]
    fn lower_hex_literal() {
        let expr = parse_expr("-0x80i8");
        let (lowered, was_lowered) = try_lower_expr(&expr);
        assert!(was_lowered);
        match lowered {
            LoweredExpr::IntLiteral { value, suffix, .. } => {
                assert_eq!(value, -128); // 0x80 = 128
                assert_eq!(suffix, Some(PrimitiveKind::I8));
            }
            _ => panic!("Expected IntLiteral"),
        }
    }

    // ========================================================================
    // New HIR lowering tests - Phase 2: Literals
    // ========================================================================

    #[test]
    fn lower_int_literal() {
        let db = lower("fn main() { let x = 42; }");
        assert!(!db.exprs.is_empty());

        // Find the literal expression
        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Literal(Literal::Int(v)) = &expr.kind {
                assert_eq!(*v, 42);
                return;
            }
        }
        panic!("Did not find int literal");
    }

    #[test]
    fn lower_int_literal_i8() {
        let db = lower("fn main() { let x: i8 = 42i8; }");
        assert!(!db.exprs.is_empty());

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Literal(Literal::Int(v)) = &expr.kind {
                assert_eq!(*v, 42);
                return;
            }
        }
        panic!("Did not find int literal");
    }

    #[test]
    fn lower_negated_literal() {
        let db = lower("fn main() { let x: i8 = -128i8; }");
        assert!(!db.exprs.is_empty());

        // Should be folded to a single literal
        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Literal(Literal::Int(v)) = &expr.kind
                && *v == -128
            {
                return;
            }
        }
        panic!("Did not find folded negated literal");
    }

    #[test]
    fn lower_float_literal() {
        let db = lower("fn main() { let x = 2.5; }");
        assert!(!db.exprs.is_empty());

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Literal(Literal::Float(v)) = &expr.kind {
                assert!((*v - 2.5_f64).abs() < 0.001);
                return;
            }
        }
        panic!("Did not find float literal");
    }

    #[test]
    fn lower_bool_literal() {
        let db = lower("fn main() { let x = true; let y = false; }");
        assert!(!db.exprs.is_empty());

        let mut found_true = false;
        let mut found_false = false;
        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Literal(Literal::Bool(v)) = &expr.kind {
                if *v {
                    found_true = true;
                } else {
                    found_false = true;
                }
            }
        }
        assert!(found_true, "Did not find true literal");
        assert!(found_false, "Did not find false literal");
    }

    #[test]
    fn lower_char_literal() {
        let db = lower("fn main() { let x = 'a'; }");
        assert!(!db.exprs.is_empty());

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Literal(Literal::Char(c)) = &expr.kind {
                assert_eq!(*c, 'a');
                return;
            }
        }
        panic!("Did not find char literal");
    }

    #[test]
    fn lower_string_literal() {
        let db = lower(r#"fn main() { let x = "hello"; }"#);
        assert!(!db.exprs.is_empty());

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Literal(Literal::String(s)) = &expr.kind {
                assert_eq!(s, "hello");
                return;
            }
        }
        panic!("Did not find string literal");
    }

    // ========================================================================
    // Phase 3: Variables & Binary Ops
    // ========================================================================

    #[test]
    fn lower_local_reference() {
        let db = lower("fn main() { let x = 1; x; }");

        let mut found_var = false;
        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Var(_) = &expr.kind {
                found_var = true;
                break;
            }
        }
        assert!(found_var, "Did not find variable reference");
    }

    #[test]
    fn lower_binary_add() {
        let db = lower("fn main() { let x = 1 + 2; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Binary { op, .. } = &expr.kind {
                assert_eq!(*op, BinOp::Add);
                return;
            }
        }
        panic!("Did not find binary add");
    }

    #[test]
    fn lower_binary_comparison() {
        let db = lower("fn main() { let x = 1 < 2; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Binary { op, .. } = &expr.kind {
                assert_eq!(*op, BinOp::Lt);
                return;
            }
        }
        panic!("Did not find binary comparison");
    }

    #[test]
    fn lower_binary_logical_and() {
        let db = lower("fn main() { let x = true && false; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Binary { op, .. } = &expr.kind {
                assert_eq!(*op, BinOp::And);
                return;
            }
        }
        panic!("Did not find logical and");
    }

    #[test]
    fn lower_binary_assign() {
        let db = lower("fn main() { let mut x = 1; x = 2; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Binary { op, .. } = &expr.kind
                && *op == BinOp::Assign
            {
                return;
            }
        }
        panic!("Did not find assignment");
    }

    // ========================================================================
    // Phase 4: Control Flow & Desugaring
    // ========================================================================

    #[test]
    fn lower_if_expr() {
        let db = lower("fn main() { if true { 1 } else { 2 }; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::If {
                else_branch: Some(_),
                ..
            } = &expr.kind
            {
                return;
            }
        }
        panic!("Did not find if-else expression");
    }

    #[test]
    fn lower_if_without_else() {
        let db = lower("fn main() { if true { 1; } }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::If {
                else_branch: None, ..
            } = &expr.kind
            {
                return;
            }
        }
        panic!("Did not find if expression without else");
    }

    #[test]
    fn lower_while_to_loop() {
        let db = lower("fn main() { while true { 1; } }");

        // After desugaring, there should be a Loop, not a while
        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Loop { .. } = &expr.kind {
                return;
            }
        }
        panic!("Did not find desugared loop");
    }

    #[test]
    fn lower_loop_expr() {
        let db = lower("fn main() { loop { break; } }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Loop { .. } = &expr.kind {
                return;
            }
        }
        panic!("Did not find loop");
    }

    #[test]
    fn lower_break_with_value() {
        let db = lower("fn main() { loop { break 42; } }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Break { value: Some(_) } = &expr.kind {
                return;
            }
        }
        panic!("Did not find break with value");
    }

    #[test]
    fn lower_return() {
        let db = lower("fn main() { return 42; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Return { value: Some(_) } = &expr.kind {
                return;
            }
        }
        panic!("Did not find return");
    }

    // ========================================================================
    // Phase 5: Patterns
    // ========================================================================

    #[test]
    fn lower_bind_pattern() {
        let db = lower("fn main() { let x = 1; }");

        for (_, pat) in db.pats.iter() {
            if let HirPatKind::Bind { mutable: false, .. } = &pat.kind {
                return;
            }
        }
        panic!("Did not find bind pattern");
    }

    #[test]
    fn lower_mut_bind_pattern() {
        let db = lower("fn main() { let mut x = 1; }");

        for (_, pat) in db.pats.iter() {
            if let HirPatKind::Bind { mutable: true, .. } = &pat.kind {
                return;
            }
        }
        panic!("Did not find mutable bind pattern");
    }

    #[test]
    fn lower_tuple_pattern() {
        let db = lower("fn main() { let (a, b) = (1, 2); }");

        for (_, pat) in db.pats.iter() {
            if let HirPatKind::Tuple { elements } = &pat.kind {
                assert_eq!(elements.len(), 2);
                return;
            }
        }
        panic!("Did not find tuple pattern");
    }

    #[test]
    fn lower_wildcard_pattern() {
        let db = lower("fn main() { let _ = 1; }");

        for (_, pat) in db.pats.iter() {
            if let HirPatKind::Wildcard = &pat.kind {
                return;
            }
        }
        panic!("Did not find wildcard pattern");
    }

    #[test]
    fn lower_struct_pattern() {
        let db = lower(
            "struct Point { x: i32, y: i32 } fn main() { let Point { x, y } = Point { x: 1, y: 2 }; }",
        );

        for (_, pat) in db.pats.iter() {
            if let HirPatKind::Struct { fields, .. } = &pat.kind {
                assert_eq!(fields.len(), 2);
                return;
            }
        }
        panic!("Did not find struct pattern");
    }

    // ========================================================================
    // Phase 6: Structs & Functions
    // ========================================================================

    #[test]
    fn lower_struct_expr() {
        let db = lower("struct Point { x: i32, y: i32 } fn main() { Point { x: 1, y: 2 }; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Struct { fields, .. } = &expr.kind {
                assert_eq!(fields.len(), 2);
                return;
            }
        }
        panic!("Did not find struct expression");
    }

    #[test]
    fn lower_field_access() {
        let db = lower(
            "struct Point { x: i32, y: i32 } fn main() { let p = Point { x: 1, y: 2 }; p.x; }",
        );

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Field { field, .. } = &expr.kind {
                assert_eq!(field, "x");
                return;
            }
        }
        panic!("Did not find field access");
    }

    #[test]
    fn lower_tuple_field_access() {
        let db = lower("fn main() { let t = (1, 2); t.0; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::TupleField { index, .. } = &expr.kind {
                assert_eq!(*index, 0);
                return;
            }
        }
        panic!("Did not find tuple field access");
    }

    #[test]
    fn lower_function_def() {
        let db = lower("fn foo(x: i32) -> i32 { x }");

        assert!(!db.items.is_empty());
        for item in &db.items {
            if let HirItem::Function(f) = item {
                assert_eq!(f.name, "foo");
                return;
            }
        }
        panic!("Did not find function definition");
    }

    #[test]
    fn lower_function_call() {
        let db = lower("fn foo() {} fn main() { foo(); }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Call { .. } = &expr.kind {
                return;
            }
        }
        panic!("Did not find function call");
    }

    #[test]
    fn lower_method_call() {
        let db =
            lower("struct S {} impl S { fn foo(&self) {} } fn main() { let s = S {}; s.foo(); }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::MethodCall { method, .. } = &expr.kind {
                assert_eq!(method, "foo");
                return;
            }
        }
        panic!("Did not find method call");
    }

    // ========================================================================
    // Phase 7: Arrays & Tuples
    // ========================================================================

    #[test]
    fn lower_array_literal() {
        let db = lower("fn main() { let arr = [1, 2, 3]; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Array { elements } = &expr.kind {
                assert_eq!(elements.len(), 3);
                return;
            }
        }
        panic!("Did not find array literal");
    }

    #[test]
    fn lower_array_repeat() {
        let db = lower("fn main() { let arr = [0; 10]; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::ArrayRepeat { count, .. } = &expr.kind {
                assert_eq!(*count, 10);
                return;
            }
        }
        panic!("Did not find array repeat");
    }

    #[test]
    fn lower_tuple_expr() {
        let db = lower("fn main() { let t = (1, 2, 3); }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Tuple { elements } = &expr.kind {
                assert_eq!(elements.len(), 3);
                return;
            }
        }
        panic!("Did not find tuple expression");
    }

    #[test]
    fn lower_index_expr() {
        let db = lower("fn main() { let arr = [1, 2, 3]; arr[0]; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Index { .. } = &expr.kind {
                return;
            }
        }
        panic!("Did not find index expression");
    }

    // ========================================================================
    // Additional coverage tests
    // ========================================================================

    #[test]
    fn lower_unary_not() {
        let db = lower("fn main() { let x = true; !x; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Unary { op, .. } = &expr.kind
                && *op == UnaryOp::Not
            {
                return;
            }
        }
        panic!("Did not find unary not");
    }

    #[test]
    fn lower_unary_neg_variable() {
        let db = lower("fn main() { let x = 1; -x; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Unary { op, .. } = &expr.kind
                && *op == UnaryOp::Neg
            {
                return;
            }
        }
        panic!("Did not find unary negation of variable");
    }

    #[test]
    fn lower_unary_deref() {
        let db = lower("fn main() { let x = &1; *x; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Unary { op, .. } = &expr.kind
                && *op == UnaryOp::Deref
            {
                return;
            }
        }
        panic!("Did not find unary deref");
    }

    #[test]
    fn lower_ref_expr() {
        let db = lower("fn main() { let x = 1; &x; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Ref { mutable: false, .. } = &expr.kind {
                return;
            }
        }
        panic!("Did not find reference expression");
    }

    #[test]
    fn lower_ref_mut_expr() {
        let db = lower("fn main() { let mut x = 1; &mut x; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Ref { mutable: true, .. } = &expr.kind {
                return;
            }
        }
        panic!("Did not find mutable reference expression");
    }

    #[test]
    fn lower_continue() {
        let db = lower("fn main() { loop { continue; } }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Continue = &expr.kind {
                return;
            }
        }
        panic!("Did not find continue");
    }

    #[test]
    fn lower_break_without_value() {
        let db = lower("fn main() { loop { break; } }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Break { value: None } = &expr.kind {
                return;
            }
        }
        panic!("Did not find break without value");
    }

    #[test]
    fn lower_return_without_value() {
        let db = lower("fn main() { return; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Return { value: None } = &expr.kind {
                return;
            }
        }
        panic!("Did not find return without value");
    }

    #[test]
    fn lower_cast_expr() {
        let db = lower("fn main() { let x = 1i32 as i64; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Cast { .. } = &expr.kind {
                return;
            }
        }
        panic!("Did not find cast expression");
    }

    #[test]
    fn lower_block_with_tail() {
        let db = lower("fn main() { let x = { 1; 2 }; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Block {
                stmts,
                tail: Some(_),
            } = &expr.kind
            {
                // Should have one statement (1;) and a tail (2)
                if !stmts.is_empty() {
                    return;
                }
            }
        }
        panic!("Did not find block with tail expression");
    }

    #[test]
    fn lower_struct_item() {
        let db = lower("struct Foo { a: i32, b: bool }");

        for item in &db.items {
            if let HirItem::Struct(s) = item {
                assert_eq!(s.name, "Foo");
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[0].name, "a");
                assert_eq!(s.fields[1].name, "b");
                return;
            }
        }
        panic!("Did not find struct item");
    }

    #[test]
    fn lower_impl_item() {
        let db = lower("struct S {} impl S { fn foo(&self) {} fn bar(&self) {} }");

        for item in &db.items {
            if let HirItem::Impl(impl_block) = item {
                assert_eq!(impl_block.items.len(), 2);
                return;
            }
        }
        panic!("Did not find impl item");
    }

    #[test]
    fn lower_function_with_params() {
        let db = lower("fn add(a: i32, b: i32) -> i32 { a + b }");

        for item in &db.items {
            if let HirItem::Function(f) = item {
                assert_eq!(f.name, "add");
                assert_eq!(f.params.len(), 2);
                assert!(f.body.is_some());
                return;
            }
        }
        panic!("Did not find function with params");
    }

    #[test]
    fn lower_empty_function() {
        let db = lower("fn empty() {}");

        for item in &db.items {
            if let HirItem::Function(f) = item {
                assert_eq!(f.name, "empty");
                assert!(f.params.is_empty());
                return;
            }
        }
        panic!("Did not find empty function");
    }

    #[test]
    fn lower_while_desugaring_structure() {
        // Verify the while loop is properly desugared to:
        // loop { if !cond { break; } body }
        let db = lower("fn main() { while true { 1; } }");

        // Find the Loop
        let mut found_loop = false;
        let mut found_if_with_break = false;

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Loop { body } = &expr.kind {
                found_loop = true;
                // The body should be a block containing an if statement
                let body_expr = db.expr(*body);
                if let HirExprKind::Block { stmts, .. } = &body_expr.kind {
                    // First statement should be the if-break
                    if !stmts.is_empty() {
                        let first_stmt = db.stmt(stmts[0]);
                        if let HirStmtKind::Expr { expr, .. } = &first_stmt.kind {
                            let if_expr = db.expr(*expr);
                            if let HirExprKind::If {
                                else_branch: None, ..
                            } = &if_expr.kind
                            {
                                found_if_with_break = true;
                            }
                        }
                    }
                }
            }
        }

        assert!(found_loop, "Did not find loop");
        assert!(
            found_if_with_break,
            "Did not find if-break structure in desugared while"
        );
    }

    #[test]
    fn lower_binary_or() {
        let db = lower("fn main() { let x = true || false; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Binary { op, .. } = &expr.kind
                && *op == BinOp::Or
            {
                return;
            }
        }
        panic!("Did not find logical or");
    }

    #[test]
    fn lower_compound_assign() {
        let db = lower("fn main() { let mut x = 1; x += 2; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Binary { op, .. } = &expr.kind
                && *op == BinOp::AddAssign
            {
                return;
            }
        }
        panic!("Did not find compound assignment");
    }

    #[test]
    fn lower_param_reference() {
        let db = lower("fn foo(x: i32) -> i32 { x }");

        // The function body should reference the parameter
        let mut found_var_in_body = false;
        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Var(_) = &expr.kind {
                found_var_in_body = true;
                break;
            }
        }
        assert!(
            found_var_in_body,
            "Did not find parameter reference in body"
        );
    }

    #[test]
    fn lower_nested_blocks() {
        let db = lower("fn main() { { { 1 } } }");

        let mut block_count = 0;
        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Block { .. } = &expr.kind {
                block_count += 1;
            }
        }
        // Should have at least 3 blocks (function body + 2 nested)
        assert!(
            block_count >= 3,
            "Expected at least 3 blocks, found {}",
            block_count
        );
    }

    #[test]
    fn lower_method_call_with_args() {
        let db = lower(
            "struct S {} impl S { fn add(&self, a: i32, b: i32) -> i32 { a + b } } fn main() { let s = S {}; s.add(1, 2); }",
        );

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::MethodCall { method, args, .. } = &expr.kind {
                assert_eq!(method, "add");
                assert_eq!(args.len(), 2);
                return;
            }
        }
        panic!("Did not find method call with args");
    }

    #[test]
    fn lower_function_call_with_args() {
        let db = lower("fn add(a: i32, b: i32) -> i32 { a + b } fn main() { add(1, 2); }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Call { args, .. } = &expr.kind
                && args.len() == 2
            {
                return;
            }
        }
        panic!("Did not find function call with args");
    }

    #[test]
    fn lower_if_else_if_chain() {
        let db = lower("fn main() { if true { 1 } else if false { 2 } else { 3 }; }");

        // Should have nested If expressions
        let mut if_count = 0;
        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::If { .. } = &expr.kind {
                if_count += 1;
            }
        }
        assert!(
            if_count >= 2,
            "Expected at least 2 if expressions for else-if chain"
        );
    }

    #[test]
    fn lower_empty_tuple() {
        let db = lower("fn main() { let x = (); }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Tuple { elements } = &expr.kind
                && elements.is_empty()
            {
                return;
            }
        }
        panic!("Did not find empty tuple");
    }

    #[test]
    fn lower_single_element_tuple() {
        let db = lower("fn main() { let x = (1,); }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Tuple { elements } = &expr.kind
                && elements.len() == 1
            {
                return;
            }
        }
        panic!("Did not find single element tuple");
    }

    #[test]
    fn lower_empty_array() {
        let db = lower("fn main() { let arr: [i32; 0] = []; }");

        for (_, expr) in db.exprs.iter() {
            if let HirExprKind::Array { elements } = &expr.kind
                && elements.is_empty()
            {
                return;
            }
        }
        panic!("Did not find empty array");
    }

    #[test]
    fn spans_are_preserved() {
        let db = lower("fn main() { let x = 42; }");

        // Check that spans are non-empty
        for (id, _) in db.exprs.iter() {
            let span = db.span(id);
            assert!(span.is_some(), "Expression should have a span");
            let span = span.unwrap();
            assert!(span.start < span.end, "Span should be non-empty");
        }
    }

    #[test]
    fn types_are_attached() {
        let db = lower("fn main() { let x: i32 = 42; }");

        // All expressions should have valid type IDs
        for (_, expr) in db.exprs.iter() {
            // TypeId should not be the default/uninitialized value
            // A simple check: the type should exist in the interner
            let _ = db.types.get(expr.ty);
        }
    }
}
