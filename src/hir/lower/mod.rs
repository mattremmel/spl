//! AST to HIR lowering.
//!
//! This module handles the lowering of AST expressions to HIR, including:
//! - Literal folding for negated integers
//! - Desugaring (while → loop)
//! - Type attachment from inference results
//! - Name resolution to DefIds
//!
//! # Error Handling: Fallback Values
//!
//! HIR lowering uses a **fallback strategy** for error recovery. When lowering
//! encounters missing or malformed AST nodes (typically from earlier parse or
//! resolution errors), it produces placeholder values rather than failing:
//!
//! - [`HirExprKind::Missing`]: Placeholder for expressions that couldn't be lowered
//! - Error type ([`TypeId`] for the error type): Used when type information is
//!   unavailable or invalid
//! - `DefId(0)`: Default definition ID when resolution data is missing
//!
//! ## No Diagnostics Emitted
//!
//! HIR lowering **does not emit any diagnostics**. All user-facing errors should
//! have been reported during earlier phases:
//!
//! - **Parse errors**: Reported by the parser
//! - **Resolution errors**: Reported during name resolution
//! - **Type errors**: Reported during type inference
//!
//! By the time lowering runs, any malformed nodes are the result of earlier
//! errors that have already been reported to the user.
//!
//! ## Graceful Degradation
//!
//! This fallback approach enables graceful degradation:
//!
//! 1. **Partial compilation**: Valid portions of code can be lowered and
//!    potentially analyzed further, even when other parts have errors.
//!
//! 2. **IDE support**: Language servers can provide features (completion,
//!    hover info) on valid code regions while other regions have errors.
//!
//! 3. **Stable downstream phases**: MIR lowering and other consumers receive
//!    well-formed HIR (with `Missing` placeholders) rather than dealing with
//!    `Option` or `Result` types throughout.
//!
//! The tradeoff is that `Missing` nodes and error types must be handled
//! appropriately by downstream phases.

mod folding;
#[cfg(test)]
mod tests;

pub use folding::try_lower_expr;

use crate::ast::{
    ArrayExpr, BinExpr, Block, BlockExpr, BreakExpr, CallExpr, CastExpr, Expr, ExternFn, FieldExpr,
    FunctionDef, IfExpr, IndexExpr, IsExpr, Item, LetStmt, LiteralExpr, LoopExpr, MatchExpr,
    MethodCallExpr, ParenExpr, Pat, PathExpr, PrefixExpr, RefExpr, ReturnExpr, SourceFile, Stmt,
    StructExpr, TupleExpr, WhileExpr,
};
use crate::hir::{
    BinOp, ExprId, HirDatabase, HirExpr, HirExprKind, HirField, HirFunction, HirImpl, HirItem,
    HirParam, HirPat, HirPatKind, HirStmt, HirStmtKind, HirStruct, HirTypeAlias, Literal,
    LoweredExpr, PatId, StmtId, UnaryOp,
};
use crate::lexer::Span;
use crate::sema::infer::InferResult;
use crate::sema::symbol::DefId;
use crate::sema::types::{Type, TypeId};
use crate::syntax::SyntaxKind;
use folding::{
    parse_char_literal, parse_float_literal_value, parse_int_literal_value, parse_string_literal,
};
use rowan::ast::AstNode;
use rustc_hash::FxHashMap;

// ============================================================================
// Public API
// ============================================================================

/// Lower a source file to HIR.
///
/// This takes the AST and inference results and produces a fully typed HIR.
pub fn lower_to_hir(source_file: &SourceFile, infer_result: &InferResult) -> HirDatabase {
    let mut ctx = LoweringContext::new(infer_result);
    ctx.lower_source_file(source_file);
    ctx.into_database()
}

/// Lower a multi-file package to HIR.
///
/// Like `lower_to_hir`, but processes all source files in the package and its
/// subpackages.
pub fn lower_package_to_hir(
    package: &crate::package::Package,
    infer_result: &InferResult,
) -> HirDatabase {
    let mut ctx = LoweringContext::new(infer_result);
    lower_package_files(package, &mut ctx);
    ctx.into_database()
}

/// Helper to recursively lower all files in a package hierarchy.
fn lower_package_files(package: &crate::package::Package, ctx: &mut LoweringContext) {
    // Lower all source files in this package
    for (_file_id, source_file) in package.compilation_unit().source_files() {
        ctx.lower_source_file(&source_file);
    }

    // Recurse into subpackages
    for subpkg in package.subpackages() {
        lower_package_files(subpkg, ctx);
    }
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
    /// Map from type annotation spans to their resolved TypeIds.
    type_annotation_types: FxHashMap<Span, TypeId>,
    /// Intrinsic methods that need special lowering (e.g., str.ptr() -> field 0).
    intrinsic_methods: FxHashMap<DefId, crate::sema::infer::IntrinsicKind>,
}

impl LoweringContext {
    fn new(infer_result: &InferResult) -> Self {
        let mut db = HirDatabase::new();
        // Clone the type interner from the inference result
        db.types = infer_result.types.clone();

        Self {
            db,
            expr_types: infer_result.expr_types.clone(),
            binding_types: infer_result.binding_types.clone(),
            resolutions: infer_result.resolutions.clone(),
            method_resolutions: infer_result.method_resolutions.clone(),
            type_annotation_types: infer_result.type_annotation_types.clone(),
            intrinsic_methods: infer_result.intrinsic_methods.clone(),
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

    /// Get the type for a type annotation (like `-> i32` or `: bool`).
    fn get_type_annotation(&self, span: &Span) -> TypeId {
        self.type_annotation_types
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
            Item::Extern(extern_block) => {
                // Lower each extern function declaration
                for extern_fn in extern_block.extern_fns() {
                    if let Some(hir_fn) = self.lower_extern_fn(&extern_fn) {
                        self.db.items.push(HirItem::Function(hir_fn));
                    }
                }
                None // Don't return the extern block itself as an item
            }
            Item::Use(_) => {
                // Use declarations are handled during import resolution, not HIR lowering
                None
            }
        }
    }

    fn lower_function(&mut self, func: &FunctionDef) -> Option<HirFunction> {
        let ident_token = func.name()?.ident_token()?;
        let name = ident_token.text().to_string();
        let span = Self::text_range_to_span(func.syntax().text_range());

        // Get DefId from the function name span (use token range to match resolver)
        let name_span = Self::text_range_to_span(ident_token.text_range());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Function '{}' resolved to INVALID DefId at {:?} - resolution phase failed to register this name",
            name,
            name_span
        );

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

        // Get return type from the return type annotation's span, or default to unit
        let ret_type = func
            .ret_type()
            .map(|rt| Self::text_range_to_span(rt.syntax().text_range()))
            .map(|rt_span| self.get_type_annotation(&rt_span))
            .unwrap_or_else(|| self.db.types.unit());

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

    fn lower_extern_fn(&mut self, extern_fn: &ExternFn) -> Option<HirFunction> {
        let ident_token = extern_fn.name()?.ident_token()?;
        let name = ident_token.text().to_string();
        let span = Self::text_range_to_span(extern_fn.syntax().text_range());

        // Get DefId from the function name span
        let name_span = Self::text_range_to_span(ident_token.text_range());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Extern function '{}' resolved to INVALID DefId at {:?} - resolution phase failed to register this name",
            name,
            name_span
        );

        // No type parameters for extern functions (currently)
        let type_params = Vec::new();

        // Lower parameters
        let params = extern_fn
            .param_list()
            .map(|pl| {
                pl.params()
                    .filter_map(|p| self.lower_param(&p))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Get return type
        let ret_type = extern_fn
            .ret_type()
            .map(|rt| Self::text_range_to_span(rt.syntax().text_range()))
            .map(|rt_span| self.get_type_annotation(&rt_span))
            .unwrap_or_else(|| self.db.types.unit());

        // Extern functions have no body
        let body = None;

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
        let ident_token = param.name()?.ident_token()?;
        let _name = ident_token.text().to_string();
        // Use the token's range to match how resolutions are stored
        let name_span = Self::text_range_to_span(ident_token.text_range());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Parameter '{}' resolved to INVALID DefId at {:?} - resolution phase failed to register this binding",
            _name,
            name_span
        );

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
            .and_then(|n| n.resolution_span())
            .unwrap_or_else(|| span.clone());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Struct '{}' resolved to INVALID DefId at {:?} - resolution phase failed to register this type",
            name,
            name_span
        );

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
            .and_then(|n| n.resolution_span())
            .unwrap_or_else(|| span.clone());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Field '{}' resolved to INVALID DefId at {:?} - resolution phase failed to register this field",
            name,
            name_span
        );

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
            .and_then(|n| n.resolution_span())
            .unwrap_or_else(|| span.clone());
        let def_id = self
            .resolutions
            .get(&name_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Type alias '{}' resolved to INVALID DefId at {:?} - resolution phase failed to register this alias",
            name,
            name_span
        );

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

        // Collect all children first so we can check if something is last
        let children: Vec<_> = block.syntax().children().collect();
        let last_idx = children.len().saturating_sub(1);

        // Process all children in source order
        for (idx, child) in children.into_iter().enumerate() {
            let is_last = idx == last_idx;

            if let Some(stmt) = Stmt::cast(child.clone()) {
                match &stmt {
                    Stmt::Expr(expr_stmt) => {
                        let has_semi = expr_stmt.semicolon().is_some();
                        if let Some(expr) = expr_stmt.expr() {
                            let expr_id = self.lower_expr(&expr);
                            // Only treat as tail if no semicolon AND this is the last item
                            if !has_semi && is_last {
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

        let ty = let_stmt.ty().map(|ty_node| {
            // Get the annotated type from inference using the type annotation's span,
            // not the entire let statement span
            let ty_span = Self::text_range_to_span(ty_node.syntax().text_range());
            self.get_type(&ty_span)
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
                // Use the token's range to match how resolutions are stored
                let name_span = ident_pat
                    .name()
                    .and_then(|n| n.ident_token())
                    .map(|t| Self::text_range_to_span(t.text_range()))
                    .unwrap_or_else(|| span.clone());

                let def_id = self
                    .resolutions
                    .get(&name_span)
                    .copied()
                    .unwrap_or(DefId::INVALID);

                debug_assert!(
                    def_id.is_valid(),
                    "Pattern binding resolved to INVALID DefId at {:?} - resolution phase failed to register this binding",
                    name_span
                );

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
                // Use the first segment's name token span for resolution lookup,
                // since that's what the resolver stores
                let path_span = struct_pat
                    .path()
                    .and_then(|p| p.segments().next())
                    .and_then(|seg| seg.name())
                    .and_then(|n| n.token())
                    .map(|t| Self::text_range_to_span(t.text_range()))
                    .unwrap_or_else(|| span.clone());
                let def_id = self
                    .resolutions
                    .get(&path_span)
                    .copied()
                    .unwrap_or(DefId::INVALID);

                debug_assert!(
                    def_id.is_valid(),
                    "Struct pattern path resolved to INVALID DefId at {:?} - resolution phase failed to register this struct type",
                    path_span
                );

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
                                .unwrap_or(DefId::INVALID);
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
            Expr::Apply(apply_expr) => self.lower_apply_expr(apply_expr, span, ty),
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
            Expr::Is(is_expr) => self.lower_is_expr(is_expr, span, ty),
            Expr::Match(match_expr) => self.lower_match_expr(match_expr, span, ty),
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
        let path = match path_expr.path() {
            Some(p) => p,
            None => return self.lower_missing(span),
        };

        let segments: Vec<_> = path.segments().collect();

        if segments.is_empty() {
            return self.lower_missing(span);
        }

        // Single-segment path: just a variable reference
        if segments.len() == 1 {
            let first_segment = &segments[0];
            let first_span = first_segment
                .name()
                .and_then(|n| n.token())
                .map(|t| Self::text_range_to_span(t.text_range()))
                .unwrap_or_else(|| span.clone());

            let def_id = self
                .resolutions
                .get(&first_span)
                .copied()
                .unwrap_or(DefId::INVALID);

            debug_assert!(
                def_id.is_valid(),
                "Path expression resolved to INVALID DefId at {:?} - resolution phase failed to register this name",
                first_span
            );

            return self.db.alloc_expr(HirExpr {
                kind: HirExprKind::Var(def_id),
                ty,
                span,
            });
        }

        // Multi-segment path: base variable + field accesses
        // Get the DefId from the first segment
        let first_segment = &segments[0];
        let first_span = first_segment
            .name()
            .and_then(|n| n.token())
            .map(|t| Self::text_range_to_span(t.text_range()))
            .unwrap_or_else(|| span.clone());

        let def_id = self
            .resolutions
            .get(&first_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Multi-segment path first segment resolved to INVALID DefId at {:?} - resolution phase failed to register this name",
            first_span
        );

        // Start with the first segment as a Var expression
        let first_ty = self.get_binding_type(def_id);
        let mut current = self.db.alloc_expr(HirExpr {
            kind: HirExprKind::Var(def_id),
            ty: first_ty,
            span: first_span.clone(),
        });

        // Chain field accesses for remaining segments
        // For intermediate fields, we don't have type info readily available,
        // so we use the final type for the last field and unknown for intermediates
        let last_idx = segments.len() - 1;
        for (idx, segment) in segments.iter().enumerate().skip(1) {
            let field_name = segment
                .name()
                .and_then(|n| n.token())
                .map(|t| t.text().to_string())
                .unwrap_or_default();

            let field_span = segment.syntax().text_range();
            let field_span = Self::text_range_to_span(field_span);

            // Use the provided type for the final field, error type for intermediates
            let field_ty = if idx == last_idx {
                ty
            } else {
                self.db.types.error()
            };

            current = self.db.alloc_expr(HirExpr {
                kind: HirExprKind::Field {
                    base: current,
                    field: field_name,
                },
                ty: field_ty,
                span: field_span,
            });
        }

        current
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
        // Get the struct DefId from the path - use token range to match resolver
        let path_span = struct_expr
            .path()
            .and_then(|p| p.segments().next())
            .and_then(|seg| seg.name())
            .and_then(|n| n.token())
            .map(|t| Self::text_range_to_span(t.text_range()))
            .unwrap_or_else(|| span.clone());

        let def_id = self
            .resolutions
            .get(&path_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Struct expression path resolved to INVALID DefId at {:?} - resolution phase failed to register this struct type",
            path_span
        );

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

    fn lower_apply_expr(
        &mut self,
        apply_expr: &crate::ast::ApplyExpr,
        span: Span,
        ty: TypeId,
    ) -> ExprId {
        // Check if the expression's type is a struct type - if so, it's struct instantiation
        let is_struct = matches!(self.db.types.get(ty), Type::Struct(_, _));

        if is_struct {
            // Lower as struct instantiation
            self.lower_apply_as_struct(apply_expr, span, ty)
        } else {
            // Lower as function call
            self.lower_apply_as_call(apply_expr, span, ty)
        }
    }

    fn lower_apply_as_struct(
        &mut self,
        apply_expr: &crate::ast::ApplyExpr,
        span: Span,
        ty: TypeId,
    ) -> ExprId {
        // Get the struct DefId from the path - use token range to match resolver
        let path_span = apply_expr
            .path()
            .and_then(|p| p.segments().next())
            .and_then(|seg| seg.name())
            .and_then(|n| n.token())
            .map(|t| Self::text_range_to_span(t.text_range()))
            .unwrap_or_else(|| span.clone());

        let def_id = self
            .resolutions
            .get(&path_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        debug_assert!(
            def_id.is_valid(),
            "Apply expression struct path resolved to INVALID DefId at {:?} - resolution phase failed to register this struct type",
            path_span
        );

        // Lower field initializers from arguments
        let fields: Vec<_> = apply_expr
            .args()
            .filter_map(|arg| {
                // Get field name from named argument
                let name = arg.name_token().map(|t| t.text().to_string()).or_else(|| {
                    arg.name()
                        .and_then(|n| n.token().map(|t| t.text().to_string()))
                })?;
                let value = arg.value().map(|e| self.lower_expr(&e))?;
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

    fn lower_apply_as_call(
        &mut self,
        apply_expr: &crate::ast::ApplyExpr,
        span: Span,
        ty: TypeId,
    ) -> ExprId {
        let path = match apply_expr.path() {
            Some(p) => p,
            None => return self.lower_missing(span),
        };

        let segments: Vec<_> = path.segments().collect();
        if segments.is_empty() {
            return self.lower_missing(span);
        }

        // Check if this is a method call (multi-segment path where first segment is a variable)
        if segments.len() >= 2 {
            let first_segment = &segments[0];
            let first_span = first_segment
                .name()
                .and_then(|n| n.token())
                .map(|t| Self::text_range_to_span(t.text_range()));

            if let Some(ref first_span) = first_span
                && let Some(&first_def_id) = self.resolutions.get(first_span)
                // Check if first segment resolves to a binding (variable)
                && self.binding_types.contains_key(&first_def_id)
            {
                // This is a method call like p.distance()
                return self.lower_apply_as_method_call(apply_expr, &segments, span, ty);
            }
        }

        // Regular function call
        // Build a synthetic callee expression from the path
        let callee = {
            // Get the DefId for the path
            let def_id = path
                .segments()
                .next()
                .and_then(|seg| seg.name())
                .and_then(|n| n.token())
                .map(|t| Self::text_range_to_span(t.text_range()))
                .and_then(|span| self.resolutions.get(&span).copied())
                .unwrap_or(DefId::INVALID);

            // Get the type for the path (function type)
            let path_span = Self::text_range_to_span(path.syntax().text_range());
            let path_ty = self.get_type(&path_span);

            let expr = HirExpr {
                kind: HirExprKind::Var(def_id),
                ty: path_ty,
                span: path_span,
            };
            self.db.alloc_expr(expr)
        };

        // Lower arguments
        let args: Vec<_> = apply_expr
            .args()
            .filter_map(|arg| arg.value().map(|e| self.lower_expr(&e)))
            .collect();

        let expr = HirExpr {
            kind: HirExprKind::Call { callee, args },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_apply_as_method_call(
        &mut self,
        apply_expr: &crate::ast::ApplyExpr,
        segments: &[crate::ast::PathSegment],
        span: Span,
        ty: TypeId,
    ) -> ExprId {
        // Get the receiver (first segment)
        let first_segment = &segments[0];
        let first_span = first_segment
            .name()
            .and_then(|n| n.token())
            .map(|t| Self::text_range_to_span(t.text_range()))
            .unwrap_or_else(|| span.clone());

        let first_def_id = self
            .resolutions
            .get(&first_span)
            .copied()
            .unwrap_or(DefId::INVALID);

        let receiver_ty = self.get_binding_type(first_def_id);
        let receiver = self.db.alloc_expr(HirExpr {
            kind: HirExprKind::Var(first_def_id),
            ty: receiver_ty,
            span: first_span,
        });

        // Check if this is an intrinsic method that needs special lowering
        if let Some(&method_def_id) = self.method_resolutions.get(&span)
            && let Some(intrinsic) = self.intrinsic_methods.get(&method_def_id).cloned()
        {
            match intrinsic {
                crate::sema::infer::IntrinsicKind::FieldAccess(index) => {
                    // Lower intrinsic method call to tuple field access
                    let expr = HirExpr {
                        kind: HirExprKind::TupleField {
                            base: receiver,
                            index,
                        },
                        ty,
                        span,
                    };
                    return self.db.alloc_expr(expr);
                }
            }
        }

        // Get the method name (last segment)
        let last_segment = &segments[segments.len() - 1];
        let method = last_segment
            .name()
            .and_then(|n| n.token())
            .map(|t| t.text().to_string())
            .unwrap_or_default();

        // Get method span for resolution lookup
        let method_span = last_segment
            .name()
            .and_then(|n| n.token())
            .map(|t| Self::text_range_to_span(t.text_range()))
            .unwrap_or_else(|| span.clone());

        // Lower arguments
        let args: Vec<_> = apply_expr
            .args()
            .filter_map(|arg| arg.value().map(|e| self.lower_expr(&e)))
            .collect();

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
        if let Some(&method_def_id) = self.method_resolutions.get(&method_span) {
            self.db.method_resolutions.insert(expr_id, method_def_id);
        }

        expr_id
    }

    fn lower_binary_expr(&mut self, bin: &BinExpr, span: Span, ty: TypeId) -> ExprId {
        // First, try to fold as a constant expression
        let full_expr = Expr::Binary(bin.clone());
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
                LoweredExpr::BoolLiteral { value, .. } => {
                    let expr = HirExpr {
                        kind: HirExprKind::Literal(Literal::Bool(value)),
                        ty,
                        span,
                    };
                    return self.db.alloc_expr(expr);
                }
                LoweredExpr::Passthrough => {}
            }
        }

        // Not foldable, lower normally
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
        // First, try to lower as a foldable expression (negated literal or boolean NOT)
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
                LoweredExpr::BoolLiteral { value, .. } => {
                    let expr = HirExpr {
                        kind: HirExprKind::Literal(Literal::Bool(value)),
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

        // Check if this is an intrinsic method that needs special lowering
        if let Some(&method_def_id) = self.method_resolutions.get(&span)
            && let Some(intrinsic) = self.intrinsic_methods.get(&method_def_id).cloned()
        {
            match intrinsic {
                crate::sema::infer::IntrinsicKind::FieldAccess(index) => {
                    // Lower intrinsic method call to tuple field access
                    let expr = HirExpr {
                        kind: HirExprKind::TupleField {
                            base: receiver,
                            index,
                        },
                        ty,
                        span,
                    };
                    return self.db.alloc_expr(expr);
                }
            }
        }

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

    fn lower_is_expr(&mut self, is_expr: &IsExpr, span: Span, ty: TypeId) -> ExprId {
        let scrutinee = is_expr
            .lhs()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let pattern = is_expr
            .pattern()
            .map(|p| self.lower_pattern(&p, false))
            .unwrap_or_else(|| {
                let missing_pat = HirPat {
                    kind: HirPatKind::Missing,
                    ty: self.error_type(),
                    span: span.clone(),
                };
                self.db.alloc_pat(missing_pat)
            });

        let negated = is_expr.is_negated();

        let expr = HirExpr {
            kind: HirExprKind::Is {
                scrutinee,
                pattern,
                negated,
            },
            ty,
            span,
        };
        self.db.alloc_expr(expr)
    }

    fn lower_match_expr(&mut self, match_expr: &MatchExpr, span: Span, ty: TypeId) -> ExprId {
        let scrutinee = match_expr
            .scrutinee()
            .map(|e| self.lower_expr(&e))
            .unwrap_or_else(|| self.lower_missing(span.clone()));

        let arms: Vec<_> = match_expr
            .arms()
            .map(|arm| {
                let pattern = arm
                    .pattern()
                    .map(|p| self.lower_pattern(&p, false))
                    .unwrap_or_else(|| {
                        let missing_pat = HirPat {
                            kind: HirPatKind::Missing,
                            ty: self.error_type(),
                            span: span.clone(),
                        };
                        self.db.alloc_pat(missing_pat)
                    });

                let guard = arm.guard().map(|g| self.lower_expr(&g));

                let body = arm
                    .body()
                    .map(|b| self.lower_expr(&b))
                    .unwrap_or_else(|| self.lower_missing(span.clone()));

                (pattern, guard, body)
            })
            .collect();

        let expr = HirExpr {
            kind: HirExprKind::Match { scrutinee, arms },
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
