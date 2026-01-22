//! AST to HIR lowering.
//!
//! This module handles the lowering of AST expressions to HIR, including:
//! - Literal folding for negated integers
//! - Desugaring (while → loop)
//! - Type attachment from inference results
//! - Name resolution to DefIds

mod folding;
#[cfg(test)]
mod tests;

pub use folding::try_lower_expr;

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
use crate::sema::types::TypeId;
use crate::syntax::SyntaxKind;
use folding::{parse_char_literal, parse_float_literal_value, parse_int_literal_value, parse_string_literal};
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
    /// Map from method call spans to their resolved method DefIds.
    method_resolutions: FxHashMap<Span, DefId>,
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
            method_resolutions: infer_result.method_resolutions,
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
            span: span.clone(),
        };
        let expr_id = self.db.alloc_expr(expr);

        // Store the resolved method DefId for MIR lowering
        if let Some(&method_def_id) = self.method_resolutions.get(&span) {
            self.db.method_resolutions.insert(expr_id, method_def_id);
        }

        expr_id
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
