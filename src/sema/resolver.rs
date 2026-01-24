//! Name resolution for SPL.
//!
//! Resolves identifiers to their declarations using a two-pass approach.
//!
//! # Why Two Passes?
//!
//! SPL allows forward references: a function can call another function that's
//! defined later in the file. To support this, we must know about all top-level
//! definitions before we try to resolve references to them.
//!
//! # Pass 1: Collection
//!
//! Walks top-level items and registers their names in the symbol table:
//! - Functions → `SymbolKind::Function`
//! - Structs → `SymbolKind::Struct` (plus fields as `SymbolKind::Field`)
//! - Type aliases → `SymbolKind::TypeAlias`
//! - Impl blocks → creates scope for methods, registers each method
//!
//! After pass 1, all top-level names are known but bodies are not yet analyzed.
//!
//! # Pass 2: Resolution
//!
//! Walks the full AST and resolves name references:
//! - Enters scopes for functions, blocks, loops, etc.
//! - Binds local variables and parameters as they're encountered
//! - Resolves `NameRef` nodes by looking up names in the scope chain
//! - Records resolutions in a `Span → DefId` map for later phases
//!
//! # Error Handling
//!
//! Resolution errors (undefined names, duplicate definitions) are collected
//! as diagnostics rather than failing immediately. This allows reporting
//! multiple errors and enables partial analysis of valid code regions.

use crate::DefId;
use crate::ast::{
    ApplyExpr, Block, Expr, FieldDef, FunctionDef, GenericParam, ImplBlock, Item, LetStmt, Name,
    NameRef, Param, ParamList, Pat, Path, PathSegment, SelfParam, SourceFile, Stmt, StructDef,
    StructExpr, StructExprField, StructPat, StructPatField, Type, TypeAlias, WhereClause,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::Span;
use crate::sema::{ScopeKind, SemanticContext, SymbolKind, Visibility};
use rowan::ast::AstNode;
use rustc_hash::FxHashMap;

/// Result of name resolution.
pub struct ResolveResult {
    /// The semantic context with symbol table.
    pub ctx: SemanticContext,
    /// Map from NameRef locations to their resolved DefIds.
    pub resolutions: FxHashMap<Span, DefId>,
    /// Diagnostics produced during resolution.
    pub diagnostics: Vec<Diagnostic>,
}

/// Name resolver for SPL programs.
pub struct Resolver<'ctx> {
    ctx: &'ctx mut SemanticContext,
    resolutions: FxHashMap<Span, DefId>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ctx> Resolver<'ctx> {
    /// Create a new resolver.
    pub fn new(ctx: &'ctx mut SemanticContext) -> Self {
        Self {
            ctx,
            resolutions: FxHashMap::default(),
            diagnostics: Vec::new(),
        }
    }

    /// Resolve names in a source file.
    ///
    /// Returns the resolutions map and diagnostics.
    pub fn resolve(
        mut self,
        source_file: &SourceFile,
    ) -> (FxHashMap<Span, DefId>, Vec<Diagnostic>) {
        // Pass 1: Collect top-level definitions
        self.collect_source_file(source_file);

        #[cfg(debug_assertions)]
        let resolutions_after_pass1 = self.resolutions.len();

        // Pass 2: Resolve all references
        self.resolve_source_file(source_file);

        debug_assert!(
            self.resolutions.len() >= resolutions_after_pass1,
            "invariant: pass 2 can only add resolutions, never remove (before: {}, after: {})",
            resolutions_after_pass1,
            self.resolutions.len()
        );

        (self.resolutions, self.diagnostics)
    }

    // ===== Helper Methods =====

    fn text_range_to_span(range: rowan::TextRange) -> Span {
        range.start().into()..range.end().into()
    }

    fn get_ident_token(name: &Name) -> Option<crate::syntax::SyntaxToken> {
        name.ident_token()
    }

    fn get_name_ref_token(name_ref: &NameRef) -> Option<crate::syntax::SyntaxToken> {
        // Use token() to handle both IDENT and SELF_VALUE_KW (for `self`)
        name_ref.token()
    }

    fn error_undefined(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::error(format!("cannot find `{name}` in this scope"))
                .with_label(span, "not found in this scope"),
        );
    }

    fn error_duplicate(&mut self, name: &str, span: Span, first_span: Span) {
        self.diagnostics.push(
            Diagnostic::error(format!("the name `{name}` is defined multiple times"))
                .with_label(span, "redefined here")
                .with_secondary_label(first_span, "first definition here"),
        );
    }

    fn define_name(
        &mut self,
        name: &Name,
        kind: SymbolKind,
        visibility: Visibility,
        is_mutable: bool,
    ) -> Option<DefId> {
        let token = Self::get_ident_token(name)?;
        let name_text = token.text().to_string();
        let span = Self::text_range_to_span(token.text_range());
        let interned = self.ctx.intern(&name_text);

        match self
            .ctx
            .define(interned, kind, visibility, span.clone(), is_mutable)
        {
            Ok(def_id) => {
                // Store span → DefId mapping for inference phase
                self.resolutions.insert(span.clone(), def_id);

                debug_assert!(
                    self.ctx
                        .lookup_in_scope(interned, self.ctx.current_scope_id())
                        == Some(def_id),
                    "postcondition: name must be defined in current scope after define_name"
                );

                Some(def_id)
            }
            Err(existing_def_id) => {
                let existing = self.ctx.get_symbol(existing_def_id);
                self.error_duplicate(&name_text, span, existing.span.clone());
                None
            }
        }
    }

    fn resolve_name_ref(&mut self, name_ref: &NameRef) -> Option<DefId> {
        let token = Self::get_name_ref_token(name_ref)?;
        let name_text = token.text().to_string();
        let span = Self::text_range_to_span(token.text_range());
        let interned = self.ctx.intern(&name_text);

        match self.ctx.lookup(interned) {
            Some(def_id) => {
                self.resolutions.insert(span, def_id);
                Some(def_id)
            }
            None => {
                self.error_undefined(&name_text, span);
                None
            }
        }
    }

    fn convert_visibility(&self, vis: &Option<crate::ast::Visibility>) -> Visibility {
        match vis {
            None => Visibility::Private,
            Some(v) => {
                if v.crate_kw().is_some() {
                    Visibility::Crate
                } else if v.super_kw().is_some() {
                    Visibility::Super
                } else if v.self_kw().is_some() {
                    Visibility::PubSelf
                } else {
                    Visibility::Public
                }
            }
        }
    }

    // ===== Pass 1: Definition Collection =====

    fn collect_source_file(&mut self, source_file: &SourceFile) {
        for item in source_file.items() {
            self.collect_item(&item);
        }
    }

    fn collect_item(&mut self, item: &Item) {
        match item {
            Item::Function(func) => self.collect_function(func),
            Item::Struct(struct_def) => self.collect_struct(struct_def),
            Item::TypeAlias(type_alias) => self.collect_type_alias(type_alias),
            Item::Impl(impl_block) => self.collect_impl_block(impl_block),
        }
    }

    fn collect_function(&mut self, func: &FunctionDef) {
        if let Some(name) = func.name() {
            let vis = self.convert_visibility(&func.visibility());
            self.define_name(&name, SymbolKind::Function, vis, false);
        }
    }

    fn collect_struct(&mut self, struct_def: &StructDef) {
        if let Some(name) = struct_def.name() {
            let vis = self.convert_visibility(&struct_def.visibility());
            self.define_name(&name, SymbolKind::Struct, vis, false);
        }
    }

    fn collect_type_alias(&mut self, type_alias: &TypeAlias) {
        if let Some(name) = type_alias.name() {
            let vis = self.convert_visibility(&type_alias.visibility());
            self.define_name(&name, SymbolKind::TypeAlias, vis, false);
        }
    }

    fn collect_impl_block(&mut self, impl_block: &ImplBlock) {
        // Create a synthetic name for the impl block using its span
        let span = impl_block.syntax().text_range();
        let synthetic_name = self
            .ctx
            .intern(&format!("impl@{}", u32::from(span.start())));
        let impl_span = Self::text_range_to_span(span);

        // Define the impl block with its own DefId
        if let Ok(def_id) = self.ctx.define(
            synthetic_name,
            SymbolKind::Impl,
            Visibility::Private,
            impl_span.clone(),
            false,
        ) {
            // Store span → DefId mapping
            self.resolutions.insert(impl_span, def_id);
        }

        // Enter impl scope and collect methods
        self.ctx.enter_scope(ScopeKind::Impl);

        for item in impl_block.items() {
            if let Item::Function(func) = item
                && let Some(name) = func.name()
            {
                let vis = self.convert_visibility(&func.visibility());
                self.define_name(&name, SymbolKind::Function, vis, false);
            }
        }

        self.ctx.exit_scope();
    }

    // ===== Pass 2: Resolution =====

    fn resolve_source_file(&mut self, source_file: &SourceFile) {
        for item in source_file.items() {
            self.resolve_item(&item);
        }
    }

    fn resolve_item(&mut self, item: &Item) {
        match item {
            Item::Function(func) => self.resolve_function(func),
            Item::Struct(struct_def) => self.resolve_struct(struct_def),
            Item::TypeAlias(type_alias) => self.resolve_type_alias(type_alias),
            Item::Impl(impl_block) => self.resolve_impl_block(impl_block),
        }
    }

    fn resolve_function(&mut self, func: &FunctionDef) {
        self.ctx.enter_scope(ScopeKind::Function);

        // Define generic parameters from where clause
        if let Some(where_clause) = func.where_clause() {
            self.define_where_clause(&where_clause);
        }

        // Define parameters
        if let Some(params) = func.param_list() {
            self.define_params(&params);
        }

        // Resolve return type
        if let Some(ret_ty) = func.ret_type() {
            self.resolve_type(&ret_ty);
        }

        // Resolve body
        if let Some(body) = func.body() {
            self.resolve_block(&body);
        }

        self.ctx.exit_scope();
    }

    fn resolve_struct(&mut self, struct_def: &StructDef) {
        // Enter a scope for the struct's generic parameters and fields
        self.ctx.enter_scope(ScopeKind::Block);

        // Define generic parameters from where clause
        if let Some(where_clause) = struct_def.where_clause() {
            self.define_where_clause(&where_clause);
        }

        // Define and resolve fields
        if let Some(field_list) = struct_def.field_list() {
            for field in field_list.fields() {
                self.resolve_field_def(&field);
            }
        }

        self.ctx.exit_scope();
    }

    fn resolve_field_def(&mut self, field: &FieldDef) {
        // Define the field name
        if let Some(name) = field.name() {
            let vis = self.convert_visibility(&field.visibility());
            self.define_name(&name, SymbolKind::Field, vis, false);
        }

        // Resolve the field type
        if let Some(ty) = field.ty() {
            self.resolve_type(&ty);
        }
    }

    fn resolve_type_alias(&mut self, type_alias: &TypeAlias) {
        // Enter scope for generic parameters
        self.ctx.enter_scope(ScopeKind::Block);

        // Define generic parameters from where clause
        if let Some(where_clause) = type_alias.where_clause() {
            self.define_where_clause(&where_clause);
        }

        // Resolve the aliased type
        if let Some(ty) = type_alias.ty() {
            self.resolve_type(&ty);
        }

        // Exit scope
        self.ctx.exit_scope();
    }

    fn resolve_impl_block(&mut self, impl_block: &ImplBlock) {
        self.ctx.enter_scope(ScopeKind::Impl);

        // Define generic parameters from where clause
        if let Some(where_clause) = impl_block.where_clause() {
            self.define_where_clause(&where_clause);
        }

        // Resolve self type
        if let Some(self_ty) = impl_block.self_ty() {
            self.resolve_type(&self_ty);
        }

        // Resolve items
        for item in impl_block.items() {
            self.resolve_item(&item);
        }

        self.ctx.exit_scope();
    }

    fn define_where_clause(&mut self, where_clause: &WhereClause) {
        for param in where_clause.type_params() {
            self.define_generic_param(&param);

            // Resolve each bound path (e.g., Clone, Debug in `T: Clone + Debug`)
            for bound in param.bounds() {
                if let Some(path) = bound.path() {
                    self.resolve_path(&path);
                }
            }
        }
    }

    fn define_generic_param(&mut self, param: &GenericParam) {
        if let Some(name) = param.name() {
            self.define_name(&name, SymbolKind::TypeParam, Visibility::Private, false);
        }
    }

    fn define_params(&mut self, params: &ParamList) {
        // Handle self parameter
        if let Some(self_param) = params.self_param() {
            self.define_self_param(&self_param);
        }

        // Handle regular parameters
        for param in params.params() {
            self.define_param(&param);
        }
    }

    fn define_self_param(&mut self, self_param: &SelfParam) {
        // Define `self` as a special parameter
        let interned = self.ctx.intern("self");

        // Get the span from the self keyword token
        let span = self_param
            .self_kw()
            .map(|t| Self::text_range_to_span(t.text_range()))
            .unwrap_or(0..0);

        // Self is immutable by default (even `&mut self` - the self binding itself isn't reassignable)
        if let Ok(def_id) = self.ctx.define(
            interned,
            SymbolKind::SelfParam,
            Visibility::Private,
            span.clone(),
            false,
        ) {
            // Store the mapping from span to DefId so inference can bind the type
            self.resolutions.insert(span, def_id);
        }
    }

    fn define_param(&mut self, param: &Param) {
        // Define the parameter name - parameters are immutable by default
        if let Some(name) = param.name() {
            self.define_name(&name, SymbolKind::Parameter, Visibility::Private, false);
        }

        // Resolve the parameter type
        if let Some(ty) = param.ty() {
            self.resolve_type(&ty);
        }
    }

    fn resolve_block(&mut self, block: &Block) {
        use rowan::ast::AstNode;

        #[cfg(debug_assertions)]
        let scope_depth_before = self.ctx.scope_depth();

        self.ctx.enter_scope(ScopeKind::Block);

        // Process all children in source order (statements and bare expressions)
        // This is important because bare expressions (like `while` without semicolon)
        // must be resolved in order with surrounding statements.
        for child in block.syntax().children() {
            // Try to cast as a statement first
            if let Some(stmt) = Stmt::cast(child.clone()) {
                self.resolve_stmt(&stmt);
            } else if let Some(expr) = Expr::cast(child.clone()) {
                // Bare expression (not wrapped in ExprStmt), including tail expressions
                self.resolve_expr(&expr);
            }
        }

        self.ctx.exit_scope();

        debug_assert_eq!(
            self.ctx.scope_depth(),
            scope_depth_before,
            "invariant: scope must be balanced after resolve_block"
        );
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(let_stmt) => self.resolve_let_stmt(let_stmt),
            Stmt::Expr(expr_stmt) => {
                if let Some(expr) = expr_stmt.expr() {
                    self.resolve_expr(&expr);
                }
            }
        }
    }

    fn resolve_let_stmt(&mut self, let_stmt: &LetStmt) {
        // Resolve struct type paths in pattern FIRST (source order)
        if let Some(pat) = let_stmt.pat() {
            self.resolve_pattern_types(&pat);
        }

        // Resolve the initializer
        if let Some(init) = let_stmt.initializer() {
            self.resolve_expr(&init);
        }

        // Resolve the type annotation
        if let Some(ty) = let_stmt.ty() {
            self.resolve_type(&ty);
        }

        // Define the pattern bindings
        // For `let mut x = ...`, the `mut` is at the LetStmt level
        let outer_mutable = let_stmt.mut_kw().is_some();
        if let Some(pat) = let_stmt.pat() {
            self.define_pattern(&pat, outer_mutable);
        }
    }

    // ===== Expression Resolution =====

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(_) => {}
            Expr::Path(path_expr) => {
                if let Some(path) = path_expr.path() {
                    self.resolve_path(&path);
                }
            }
            Expr::Paren(paren_expr) => {
                if let Some(inner) = paren_expr.expr() {
                    self.resolve_expr(&inner);
                }
            }
            Expr::Tuple(tuple_expr) => {
                for expr in tuple_expr.exprs() {
                    self.resolve_expr(&expr);
                }
            }
            Expr::Array(array_expr) => {
                for expr in array_expr.exprs() {
                    self.resolve_expr(&expr);
                }
            }
            Expr::Struct(struct_expr) => self.resolve_struct_expr(struct_expr),
            Expr::Apply(apply_expr) => self.resolve_apply_expr(apply_expr),
            Expr::Binary(bin_expr) => {
                if let Some(lhs) = bin_expr.lhs() {
                    self.resolve_expr(&lhs);
                }
                if let Some(rhs) = bin_expr.rhs() {
                    self.resolve_expr(&rhs);
                }
            }
            Expr::Prefix(prefix_expr) => {
                if let Some(inner) = prefix_expr.expr() {
                    self.resolve_expr(&inner);
                }
            }
            Expr::Ref(ref_expr) => {
                if let Some(inner) = ref_expr.expr() {
                    self.resolve_expr(&inner);
                }
            }
            Expr::Field(field_expr) => {
                // Resolve the base expression
                // Note: Field name resolution requires type info (deferred to type checking)
                if let Some(base) = field_expr.expr() {
                    self.resolve_expr(&base);
                }
            }
            Expr::MethodCall(method_call) => {
                // Resolve the receiver
                // Note: Method resolution requires type info (deferred to type checking)
                if let Some(receiver) = method_call.receiver() {
                    self.resolve_expr(&receiver);
                }
                // Resolve arguments
                if let Some(arg_list) = method_call.arg_list() {
                    for arg in arg_list.args() {
                        self.resolve_expr(&arg);
                    }
                }
            }
            Expr::Call(call_expr) => {
                if let Some(callee) = call_expr.callee() {
                    self.resolve_expr(&callee);
                }
                if let Some(arg_list) = call_expr.arg_list() {
                    for arg in arg_list.args() {
                        self.resolve_expr(&arg);
                    }
                }
            }
            Expr::Index(index_expr) => {
                if let Some(base) = index_expr.base() {
                    self.resolve_expr(&base);
                }
                if let Some(index) = index_expr.index() {
                    self.resolve_expr(&index);
                }
            }
            Expr::Slice(slice_expr) => {
                if let Some(base) = slice_expr.base() {
                    self.resolve_expr(&base);
                }
                if let Some(start) = slice_expr.start() {
                    self.resolve_expr(&start);
                }
                if let Some(end) = slice_expr.end() {
                    self.resolve_expr(&end);
                }
            }
            Expr::If(if_expr) => {
                if let Some(cond) = if_expr.condition() {
                    self.resolve_expr(&cond);
                }
                if let Some(then_branch) = if_expr.then_branch() {
                    self.resolve_block(&then_branch);
                }
                // else_branch() handles else-if chains (returns Expr)
                // else_block() handles simple else { ... } (returns Block directly)
                if let Some(else_branch) = if_expr.else_branch() {
                    self.resolve_expr(&else_branch);
                } else if let Some(else_block) = if_expr.else_block() {
                    self.resolve_block(&else_block);
                }
            }
            Expr::While(while_expr) => {
                if let Some(cond) = while_expr.condition() {
                    self.resolve_expr(&cond);
                }
                if let Some(body) = while_expr.body() {
                    self.resolve_block(&body);
                }
            }
            Expr::For(for_expr) => self.resolve_for_expr(for_expr),
            Expr::Loop(loop_expr) => {
                if let Some(body) = loop_expr.body() {
                    self.resolve_block(&body);
                }
            }
            Expr::Break(break_expr) => {
                if let Some(value) = break_expr.expr() {
                    self.resolve_expr(&value);
                }
            }
            Expr::Continue(_) => {}
            Expr::Return(return_expr) => {
                if let Some(value) = return_expr.expr() {
                    self.resolve_expr(&value);
                }
            }
            Expr::Block(block_expr) => {
                if let Some(block) = block_expr.block() {
                    self.resolve_block(&block);
                }
            }
            Expr::Cast(cast_expr) => {
                if let Some(inner) = cast_expr.expr() {
                    self.resolve_expr(&inner);
                }
                if let Some(ty) = cast_expr.ty() {
                    self.resolve_type(&ty);
                }
            }
            Expr::Range(range_expr) => {
                if let Some(start) = range_expr.start() {
                    self.resolve_expr(&start);
                }
                if let Some(end) = range_expr.end() {
                    self.resolve_expr(&end);
                }
            }
            // New syntax - pattern matching expressions
            Expr::Is(is_expr) => {
                // Resolve the expression being matched
                if let Some(lhs) = is_expr.lhs() {
                    self.resolve_expr(&lhs);
                }
                // Resolve pattern types (e.g., struct patterns)
                if let Some(pat) = is_expr.pattern() {
                    self.resolve_pattern_types(&pat);
                }
                // Note: `is` patterns don't introduce bindings that escape the expression
            }
            Expr::Match(match_expr) => {
                // Resolve the scrutinee
                if let Some(scrutinee) = match_expr.scrutinee() {
                    self.resolve_expr(&scrutinee);
                }
                // Resolve each arm
                for arm in match_expr.arms() {
                    // Create a new scope for pattern bindings in this arm
                    self.ctx.enter_scope(ScopeKind::Block);

                    // Resolve pattern types and define pattern bindings
                    if let Some(pat) = arm.pattern() {
                        self.resolve_pattern_types(&pat);
                        self.define_pattern(&pat, false);
                    }

                    // Resolve guard (can reference pattern bindings)
                    if let Some(guard) = arm.guard() {
                        self.resolve_expr(&guard);
                    }

                    // Resolve body (can reference pattern bindings)
                    if let Some(body) = arm.body() {
                        self.resolve_expr(&body);
                    }

                    self.ctx.exit_scope();
                }
            }
        }
    }

    fn resolve_struct_expr(&mut self, struct_expr: &StructExpr) {
        // Resolve the struct type path
        if let Some(path) = struct_expr.path() {
            self.resolve_path(&path);
        }

        // Resolve field values
        for field in struct_expr.fields() {
            self.resolve_struct_expr_field(&field);
        }
    }

    fn resolve_struct_expr_field(&mut self, field: &StructExprField) {
        // Note: Field name resolution requires type info (deferred to type checking)
        // Just resolve the value expression
        if let Some(expr) = field.expr() {
            self.resolve_expr(&expr);
        }
    }

    fn resolve_apply_expr(&mut self, apply_expr: &ApplyExpr) {
        // Resolve the path (could be struct type or function)
        if let Some(path) = apply_expr.path() {
            self.resolve_path(&path);
        }

        // Resolve argument values
        for arg in apply_expr.args() {
            // Note: Named argument name resolution requires type info (deferred to type checking)
            // Just resolve the value expression
            if let Some(value) = arg.value() {
                self.resolve_expr(&value);
            }
        }
    }

    fn resolve_for_expr(&mut self, for_expr: &crate::ast::ForExpr) {
        // Resolve pattern struct types FIRST (source order, in outer scope)
        if let Some(pat) = for_expr.pat() {
            self.resolve_pattern_types(&pat);
        }

        // Resolve iterable (in outer scope)
        if let Some(iterable) = for_expr.iterable() {
            self.resolve_expr(&iterable);
        }

        // Enter for-loop scope
        self.ctx.enter_scope(ScopeKind::ForLoop);

        // Define the loop variable bindings (immutable by default)
        if let Some(pat) = for_expr.pat() {
            self.define_pattern(&pat, false);
        }

        // Resolve body
        if let Some(body) = for_expr.body() {
            self.resolve_block(&body);
        }

        self.ctx.exit_scope();
    }

    // ===== Path Resolution =====

    fn resolve_path(&mut self, path: &Path) {
        // For now, we only resolve the first segment as a simple name lookup
        // Multi-segment paths (qualified paths) would need module resolution
        if let Some(first_segment) = path.segments().next() {
            self.resolve_path_segment(&first_segment);
        }
    }

    fn resolve_path_segment(&mut self, segment: &PathSegment) {
        if let Some(name_ref) = segment.name() {
            self.resolve_name_ref(&name_ref);
        }

        // Resolve generic arguments
        if let Some(generic_args) = segment.generic_args() {
            for ty in generic_args.args() {
                self.resolve_type(&ty);
            }
        }
    }

    // ===== Pattern Resolution =====

    /// Resolve type paths in patterns without defining bindings.
    /// This ensures struct type errors are reported in source order.
    fn resolve_pattern_types(&mut self, pat: &Pat) {
        match pat {
            Pat::Struct(struct_pat) => {
                // StructPat always has a Path (even for simple `Point { x }`)
                if let Some(path) = struct_pat.path() {
                    self.resolve_path(&path);
                }
                for field in struct_pat.fields() {
                    if let Some(nested) = field.pat() {
                        self.resolve_pattern_types(&nested);
                    }
                }
            }
            Pat::Tuple(tuple_pat) => {
                for inner in tuple_pat.patterns() {
                    self.resolve_pattern_types(&inner);
                }
            }
            Pat::Slice(slice_pat) => {
                for inner in slice_pat.patterns() {
                    self.resolve_pattern_types(&inner);
                }
            }
            Pat::Ref(ref_pat) => {
                if let Some(inner) = ref_pat.pat() {
                    self.resolve_pattern_types(&inner);
                }
            }
            Pat::Ident(_) | Pat::Wildcard(_) | Pat::Literal(_) | Pat::Rest(_) | Pat::Range(_) => {}
        }
    }

    fn define_pattern(&mut self, pat: &Pat, outer_mutable: bool) {
        match pat {
            Pat::Ident(ident_pat) => {
                // Get IDENT token directly from IdentPat (may be wrapped in Name or direct)
                let token = ident_pat
                    .name()
                    .and_then(|n| Self::get_ident_token(&n))
                    .or_else(|| {
                        crate::ast::token(ident_pat.syntax(), crate::syntax::SyntaxKind::IDENT)
                    });

                // Check if the pattern has a `mut` keyword (for nested patterns like `(mut a, b)`)
                // or if the outer binding is mutable (for `let mut x = ...`)
                let is_mutable = outer_mutable || ident_pat.mut_kw().is_some();

                if let Some(token) = token {
                    let name_text = token.text().to_string();
                    let span = Self::text_range_to_span(token.text_range());
                    let interned = self.ctx.intern(&name_text);
                    match self.ctx.define(
                        interned,
                        SymbolKind::Local,
                        Visibility::Private,
                        span.clone(),
                        is_mutable,
                    ) {
                        Ok(def_id) => {
                            // Store span → DefId mapping for inference phase
                            self.resolutions.insert(span, def_id);
                        }
                        Err(existing_def_id) => {
                            let existing = self.ctx.get_symbol(existing_def_id);
                            self.error_duplicate(&name_text, span, existing.span.clone());
                        }
                    }
                }
            }
            Pat::Wildcard(_) => {}
            Pat::Literal(_) => {}
            Pat::Range(range_pat) => {
                // Patterns in ranges don't inherit outer mutability
                if let Some(start) = range_pat.start() {
                    self.define_pattern(&start, false);
                }
                if let Some(end) = range_pat.end() {
                    self.define_pattern(&end, false);
                }
            }
            Pat::Tuple(tuple_pat) => {
                // For `let mut (a, b) = ...`, all bindings are mutable
                // For `let (mut a, b) = ...`, only `a` is mutable (handled by ident_pat.mut_kw())
                for inner in tuple_pat.patterns() {
                    self.define_pattern(&inner, outer_mutable);
                }
            }
            Pat::Slice(slice_pat) => {
                for inner in slice_pat.patterns() {
                    self.define_pattern(&inner, outer_mutable);
                }
            }
            Pat::Struct(struct_pat) => self.define_struct_pattern(struct_pat, outer_mutable),
            Pat::Ref(ref_pat) => {
                if let Some(inner) = ref_pat.pat() {
                    self.define_pattern(&inner, outer_mutable);
                }
            }
            Pat::Rest(_) => {}
        }
    }

    fn define_struct_pattern(&mut self, struct_pat: &StructPat, outer_mutable: bool) {
        // Note: struct path already resolved in resolve_pattern_types
        // Define bindings in struct pattern fields
        for field in struct_pat.fields() {
            self.define_struct_pat_field(&field, outer_mutable);
        }
    }

    fn define_struct_pat_field(&mut self, field: &StructPatField, outer_mutable: bool) {
        // If there's a nested pattern, define it
        // If not, the field name itself becomes a binding
        if let Some(pat) = field.pat() {
            self.define_pattern(&pat, outer_mutable);
        } else if let Some(name_ref) = field.name() {
            // Shorthand syntax: `Point { x, y }` means `Point { x: x, y: y }`
            // The field name (NameRef) becomes a local binding
            if let Some(token) = name_ref.token() {
                let text = token.text().to_string();
                let span = Self::text_range_to_span(token.text_range());
                let interned = self.ctx.intern(&text);
                if let Ok(def_id) = self.ctx.define(
                    interned,
                    SymbolKind::Local,
                    Visibility::Private,
                    span.clone(),
                    outer_mutable,
                ) {
                    // Store span → DefId mapping for inference phase
                    self.resolutions.insert(span, def_id);
                }
            }
        }
    }

    // ===== Type Resolution =====

    fn resolve_type(&mut self, ty: &Type) {
        match ty {
            Type::Path(path_type) => {
                if let Some(path) = path_type.path() {
                    // Skip resolution for Self type - it's handled at inference time
                    if let Some(segment) = path.segments().next()
                        && let Some(name_ref) = segment.name()
                        && let Some(token) = name_ref.token()
                        && token.kind() == crate::syntax::SyntaxKind::SELF_TYPE_KW
                    {
                        return; // Self is handled during inference
                    }
                    self.resolve_path(&path);
                }
            }
            Type::Ref(ref_type) => {
                if let Some(inner) = ref_type.ty() {
                    self.resolve_type(&inner);
                }
            }
            Type::Array(array_type) => {
                if let Some(elem_ty) = array_type.elem_ty() {
                    self.resolve_type(&elem_ty);
                }
                if let Some(len_expr) = array_type.len() {
                    self.resolve_expr(&len_expr);
                }
            }
            Type::Slice(slice_type) => {
                if let Some(elem_ty) = slice_type.elem_ty() {
                    self.resolve_type(&elem_ty);
                }
            }
            Type::Tuple(tuple_type) => {
                for inner in tuple_type.types() {
                    self.resolve_type(&inner);
                }
            }
            Type::FnPtr(fn_ptr) => {
                for param_ty in fn_ptr.param_types() {
                    self.resolve_type(&param_ty);
                }
                if let Some(ret_ty) = fn_ptr.ret_type() {
                    self.resolve_type(&ret_ty);
                }
            }
            Type::Never(_) => {
                // Never type has no inner types to resolve
            }
        }
    }
}

/// Resolve names in a source file.
///
/// This is the main entry point for name resolution.
pub fn resolve(source_file: &SourceFile) -> ResolveResult {
    let mut ctx = SemanticContext::new();

    // Pre-define built-in primitive types
    for builtin in &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char", "str",
    ] {
        let name = ctx.intern(builtin);
        // Define with a dummy span since these are built-in
        let _ = ctx.define(name, SymbolKind::Struct, Visibility::Public, 0..0, false);
    }

    // Pre-define built-in traits
    for builtin_trait in &[
        "Clone",
        "Copy",
        "Debug",
        "Default",
        "Eq",
        "Hash",
        "Ord",
        "PartialEq",
        "PartialOrd",
    ] {
        let name = ctx.intern(builtin_trait);
        let _ = ctx.define(name, SymbolKind::Trait, Visibility::Public, 0..0, false);
    }

    let resolver = Resolver::new(&mut ctx);
    let (resolutions, diagnostics) = resolver.resolve(source_file);

    ResolveResult {
        ctx,
        resolutions,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use rowan::ast::AstNode;

    fn check_ok(source: &str) {
        let parse = parse(source);
        assert!(
            parse.errors().is_empty(),
            "parse errors: {:?}",
            parse.errors()
        );
        let source_file = SourceFile::cast(parse.syntax()).expect("expected SourceFile");
        let result = resolve(&source_file);
        if !result.diagnostics.is_empty() {
            for diag in &result.diagnostics {
                eprintln!("{}", diag.message);
            }
            panic!("expected no diagnostics, got {}", result.diagnostics.len());
        }
    }

    fn check_err(source: &str, expected: &[&str]) {
        let parse = parse(source);
        assert!(
            parse.errors().is_empty(),
            "parse errors: {:?}",
            parse.errors()
        );
        let source_file = SourceFile::cast(parse.syntax()).expect("expected SourceFile");
        let result = resolve(&source_file);
        assert!(
            !result.diagnostics.is_empty(),
            "expected errors containing {:?}, got none",
            expected
        );
        for pattern in expected {
            let found = result
                .diagnostics
                .iter()
                .any(|d| d.message.contains(pattern));
            assert!(
                found,
                "expected error containing '{}', got: {:?}",
                pattern,
                result
                    .diagnostics
                    .iter()
                    .map(|d| &d.message)
                    .collect::<Vec<_>>()
            );
        }
    }

    /// Helper to resolve source and return the result for inspection.
    fn resolve_source(source: &str) -> (FxHashMap<Span, DefId>, SemanticContext, Vec<Diagnostic>) {
        let parse = parse(source);
        assert!(
            parse.errors().is_empty(),
            "parse errors: {:?}",
            parse.errors()
        );
        let source_file = SourceFile::cast(parse.syntax()).expect("expected SourceFile");
        let result = resolve(&source_file);
        (result.resolutions, result.ctx, result.diagnostics)
    }

    #[test]
    fn resolve_local_variable() {
        check_ok("fn main() { let x = 1; x; }");
    }

    #[test]
    fn resolve_undefined() {
        check_err("fn main() { y; }", &["cannot find `y`"]);
    }

    #[test]
    fn resolve_function_call() {
        check_ok("fn foo() {} fn main() { foo(); }");
    }

    #[test]
    fn resolve_forward_reference() {
        check_ok("fn a() { b(); } fn b() {}");
    }

    #[test]
    fn resolve_shadowing() {
        check_ok("fn main() { let x = 1; { let x = 2; x; } x; }");
    }

    #[test]
    fn resolve_duplicate_error() {
        check_err(
            "fn main() { let x = 1; let x = 2; }",
            &["defined multiple times"],
        );
    }

    #[test]
    fn resolve_struct_type() {
        check_ok("struct Foo; fn main() { let x: Foo; }");
    }

    #[test]
    fn resolve_function_with_params() {
        check_ok("fn add(a: i32, b: i32) { a + b; }");
    }

    #[test]
    fn resolve_nested_blocks() {
        check_ok("fn main() { let x = 1; { let y: x; { let z = y; z; } } }");
    }

    #[test]
    fn resolve_if_expr() {
        check_ok("fn main() { let x = 1; if x == 0 { x; } else { x + 1; } }");
    }

    #[test]
    fn resolve_while_loop() {
        check_ok("fn main() { let x = 1; while x > 0 { x; } }");
    }

    #[test]
    fn resolve_for_loop() {
        check_ok("fn main() { for i in 0..10 { i; } }");
    }

    #[test]
    fn resolve_for_loop_variable_scope() {
        // Loop variable should not be visible outside the loop
        check_err("fn main() { for i in 0..10 {} i; }", &["cannot find `i`"]);
    }

    #[test]
    fn resolve_struct_expr() {
        check_ok("struct Point(x: i32, y: i32) fn main() { Point(x: 1, y: 2); }");
    }

    #[test]
    fn resolve_type_alias() {
        check_ok("type Int = i32; fn main() { let x: Int = 0; }");
    }

    #[test]
    fn resolve_multiple_functions() {
        check_ok("fn first() { second(); } fn second() { third(); } fn third() { first(); }");
    }

    #[test]
    fn resolve_generic_function() {
        check_ok("fn identity(x: T): T where T { x }");
    }

    #[test]
    fn resolve_return_expr() {
        check_ok("fn main() { let x = 1; return x; }");
    }

    #[test]
    fn resolve_binary_ops() {
        check_ok("fn main() { let a = 1; let b = 2; a + b; a - b; a * b; }");
    }

    #[test]
    fn resolve_undefined_in_binop() {
        check_err(
            "fn main() { let a = 1; a + undefined; }",
            &["cannot find `undefined`"],
        );
    }

    #[test]
    fn resolve_array_expr() {
        check_ok("fn main() { let x = 1; let arr = [x, x + 1, x + 2]; }");
    }

    #[test]
    fn resolve_tuple_expr() {
        check_ok("fn main() { let x = 1; let y = 2; (x, y); }");
    }

    #[test]
    fn resolve_block_scope_exit() {
        check_err(
            "fn main() { { let inner = 1; } inner; }",
            &["cannot find `inner`"],
        );
    }

    #[test]
    fn resolve_duplicate_function() {
        check_err("fn foo() {} fn foo() {}", &["defined multiple times"]);
    }

    #[test]
    fn resolve_duplicate_struct() {
        check_err("struct Foo; struct Foo;", &["defined multiple times"]);
    }

    // ===== Impl blocks and methods =====

    #[test]
    fn resolve_impl_block_simple() {
        check_ok("struct Foo; impl Foo { fn bar() {} }");
    }

    #[test]
    fn resolve_impl_method_with_self() {
        check_ok("struct Foo; impl Foo { fn bar(self) {} }");
    }

    #[test]
    fn resolve_impl_method_with_ref_self() {
        check_ok("struct Foo; impl Foo { fn bar(&self) {} }");
    }

    #[test]
    fn resolve_impl_method_with_mut_self() {
        check_ok("struct Foo; impl Foo { fn bar(&mut self) {} }");
    }

    #[test]
    fn resolve_impl_method_with_params() {
        check_ok("struct Foo; impl Foo { fn bar(&self, x: i32) { x; } }");
    }

    #[test]
    fn resolve_generic_struct() {
        check_ok("struct Wrapper(value: T) where T");
    }

    #[test]
    fn resolve_generic_struct_with_multiple_params() {
        check_ok("struct Pair(first: A, second: B) where A, B");
    }

    #[test]
    fn resolve_generic_impl_block() {
        check_ok("struct Foo(v: T) where T impl Foo(T) where T { fn get(&self): T {} }");
    }

    #[test]
    fn test_impl_block_gets_def_id() {
        let source = r#"
            struct Foo;
            impl Foo {
                fn bar() {}
            }
        "#;
        let (_, ctx, diags) = resolve_source(source);
        assert!(diags.is_empty(), "Should have no errors: {:?}", diags);

        // Find the impl block's DefId
        let impl_symbols: Vec<_> = ctx
            .symbols()
            .filter(|s| s.kind == SymbolKind::Impl)
            .collect();
        assert_eq!(impl_symbols.len(), 1, "Should have one impl block");
    }

    // ===== Loop, break, continue =====

    #[test]
    fn resolve_loop_expr() {
        check_ok("fn main() { loop { break; } }");
    }

    #[test]
    fn resolve_break_with_value() {
        check_ok("fn main() { let x = 1; loop { break x; } }");
    }

    #[test]
    fn resolve_continue_expr() {
        check_ok("fn main() { loop { continue; } }");
    }

    #[test]
    fn resolve_break_undefined() {
        check_err(
            "fn main() { loop { break undefined; } }",
            &["cannot find `undefined`"],
        );
    }

    // ===== Cast, prefix, ref expressions =====

    #[test]
    fn resolve_cast_expr() {
        check_ok("fn main() { let x = 1; x as i64; }");
    }

    #[test]
    fn resolve_cast_to_defined_type() {
        check_ok("struct Foo; fn main() { let x = 1; x as Foo; }");
    }

    #[test]
    fn resolve_cast_to_undefined_type() {
        check_err(
            "fn main() { let x = 1; x as Undefined; }",
            &["cannot find `Undefined`"],
        );
    }

    #[test]
    fn resolve_prefix_not() {
        check_ok("fn main() { let x = true; !x; }");
    }

    #[test]
    fn resolve_prefix_neg() {
        check_ok("fn main() { let x = 1; -x; }");
    }

    #[test]
    fn resolve_ref_expr() {
        check_ok("fn main() { let x = 1; &x; }");
    }

    #[test]
    fn resolve_ref_mut_expr() {
        check_ok("fn main() { let x = 1; &mut x; }");
    }

    // ===== Field access, method calls, index, slice =====

    #[test]
    fn resolve_field_access() {
        check_ok("struct Point(x: i32, y: i32) fn main() { let p: Point; p.x; }");
    }

    #[test]
    fn resolve_field_access_nested() {
        check_ok(
            "struct Inner(v: i32) struct Outer(inner: Inner) fn main() { let o: Outer; o.inner.v; }",
        );
    }

    #[test]
    fn resolve_method_call() {
        check_ok("struct Foo; impl Foo { fn bar(&self) {} } fn main() { let f: Foo; f.bar(); }");
    }

    #[test]
    fn resolve_method_call_with_args() {
        check_ok("struct Foo; fn main() { let f: Foo; let x = 1; f.method(x, x + 1); }");
    }

    #[test]
    fn resolve_index_expr() {
        check_ok("fn main() { let arr = [1, 2, 3]; let i = 0; arr[i]; }");
    }

    #[test]
    fn resolve_index_expr_undefined() {
        check_err(
            "fn main() { let arr = [1, 2, 3]; arr[undefined]; }",
            &["cannot find `undefined`"],
        );
    }

    #[test]
    fn resolve_slice_expr() {
        check_ok("fn main() { let arr = [1, 2, 3]; let a = 0; let b = 2; arr[a..b]; }");
    }

    #[test]
    fn resolve_slice_expr_undefined() {
        check_err(
            "fn main() { let arr = [1, 2, 3]; arr[start..end]; }",
            &["cannot find `start`", "cannot find `end`"],
        );
    }

    // ===== Range expressions =====

    #[test]
    fn resolve_range_expr() {
        check_ok("fn main() { let a = 0; let b = 10; a..b; }");
    }

    #[test]
    fn resolve_range_inclusive() {
        check_ok("fn main() { let a = 0; let b = 10; a..=b; }");
    }

    // ===== Block expressions =====

    #[test]
    fn resolve_block_expr() {
        check_ok("fn main() { let result = { let x = 1; x + 1 }; result; }");
    }

    #[test]
    fn resolve_block_expr_scope() {
        check_err(
            "fn main() { let result = { let x = 1; x }; x; }",
            &["cannot find `x`"],
        );
    }

    // ===== Paren expressions =====

    #[test]
    fn resolve_paren_expr() {
        check_ok("fn main() { let x = 1; (x + 1) * 2; }");
    }

    #[test]
    fn resolve_nested_paren() {
        check_ok("fn main() { let x = 1; ((x)); }");
    }

    // ===== Call expressions with generics =====

    #[test]
    fn resolve_call_generic_function() {
        // Parser doesn't support turbofish yet, so test simple generic function call
        check_ok("fn identity(x: T): T where T { x } fn main() { identity(1); }");
    }

    // ===== Type resolution =====

    #[test]
    fn resolve_ref_type() {
        check_ok("fn foo(x: &i32) {}");
    }

    #[test]
    fn resolve_mut_ref_type() {
        check_ok("fn foo(x: &mut i32) {}");
    }

    #[test]
    fn resolve_array_type() {
        check_ok("fn foo(arr: [i32; 10]) {}");
    }

    #[test]
    fn resolve_array_type_with_literal() {
        // Const generics not yet supported, test literal size
        check_ok("fn foo(arr: [i32; 5]) {}");
    }

    #[test]
    fn resolve_slice_type() {
        check_ok("fn foo(slice: [i32]) {}");
    }

    #[test]
    fn resolve_tuple_type() {
        check_ok("fn foo(pair: (i32, bool)) {}");
    }

    #[test]
    fn resolve_fn_ptr_type() {
        check_ok("fn foo(f: fn(i32) -> bool) {}");
    }

    #[test]
    fn resolve_fn_ptr_type_no_return() {
        check_ok("fn foo(f: fn(i32, i32)) {}");
    }

    #[test]
    fn resolve_nested_generic_type() {
        check_ok("struct Box(v: T) where T fn foo(x: Box(Box(i32))) {}");
    }

    #[test]
    fn resolve_undefined_type() {
        check_err(
            "fn foo(x: UndefinedType) {}",
            &["cannot find `UndefinedType`"],
        );
    }

    #[test]
    fn resolve_undefined_in_ref_type() {
        check_err(
            "fn foo(x: &UndefinedType) {}",
            &["cannot find `UndefinedType`"],
        );
    }

    #[test]
    fn resolve_undefined_in_array_type() {
        check_err(
            "fn foo(x: [UndefinedType; 10]) {}",
            &["cannot find `UndefinedType`"],
        );
    }

    // ===== Pattern resolution =====

    #[test]
    fn resolve_wildcard_pattern() {
        check_ok("fn main() { let _ = 1; }");
    }

    #[test]
    fn resolve_tuple_pattern() {
        check_ok("fn main() { let (a, b) = (1, 2); a + b; }");
    }

    #[test]
    fn resolve_nested_tuple_pattern() {
        check_ok("fn main() { let ((a, b), c) = ((1, 2), 3); a + b + c; }");
    }

    #[test]
    fn resolve_struct_pattern() {
        check_ok(
            "struct Point(x: i32, y: i32) fn main() { let Point(x: a, y: b) = Point(x: 1, y: 2); a + b; }",
        );
    }

    #[test]
    fn resolve_struct_pattern_shorthand() {
        // Shorthand patterns are now parsed as TuplePat (for enum-style patterns)
        // Use explicit naming to get StructPat
        check_ok(
            "struct Point(x: i32, y: i32) fn main() { let Point(x: x, y: y) = Point(x: 1, y: 2); x + y; }",
        );
    }

    #[test]
    fn resolve_struct_pattern_undefined_struct() {
        check_err(
            "fn main() { let UndefinedStruct(x: x) = foo; }",
            &["cannot find `UndefinedStruct`"],
        );
    }

    #[test]
    fn resolve_ref_pattern() {
        check_ok("fn main() { let x = 1; let &y = &x; }");
    }

    #[test]
    fn resolve_slice_pattern() {
        check_ok("fn main() { let [a, b, c] = [1, 2, 3]; a + b + c; }");
    }

    #[test]
    fn resolve_for_loop_tuple_pattern() {
        check_ok("fn main() { let arr = [(1, 2)]; for (i, v) in arr { i + v; } }");
    }

    // ===== Error cases =====

    #[test]
    fn resolve_use_before_definition() {
        // In the same scope, cannot use before let defines it
        check_err("fn main() { x; let x = 1; }", &["cannot find `x`"]);
    }

    #[test]
    fn resolve_duplicate_type_alias() {
        check_err(
            "type Foo = i32; type Foo = i64;",
            &["defined multiple times"],
        );
    }

    #[test]
    fn resolve_duplicate_param() {
        check_err("fn foo(x: i32, x: i32) {}", &["defined multiple times"]);
    }

    #[test]
    fn resolve_duplicate_generic_param() {
        check_err("fn foo() where T, T {}", &["defined multiple times"]);
    }

    #[test]
    fn resolve_multiple_undefined() {
        check_err(
            "fn main() { a + b + c; }",
            &["cannot find `a`", "cannot find `b`", "cannot find `c`"],
        );
    }

    // ===== Visibility modifiers =====

    #[test]
    fn resolve_pub_function() {
        check_ok("pub fn foo() {}");
    }

    #[test]
    fn resolve_pub_struct() {
        check_ok("pub struct Foo;");
    }

    #[test]
    fn resolve_pub_type_alias() {
        check_ok("pub type Int = i32;");
    }

    #[test]
    fn resolve_pub_struct_fields() {
        check_ok("pub struct Foo(pub x: i32, y: i32)");
    }

    // ===== Complex expressions =====

    #[test]
    fn resolve_chained_method_calls() {
        check_ok("struct S; fn main() { let s: S; s.a().b().c(); }");
    }

    #[test]
    fn resolve_nested_if_else() {
        check_ok(
            "fn main() { let x = 1; if x > 0 { if x > 1 { x; } else { x + 1; } } else { x - 1; } }",
        );
    }

    #[test]
    fn resolve_complex_for_loop() {
        check_ok(
            "fn main() { let arr = [1, 2, 3]; for item in arr { let doubled = item * 2; doubled; } }",
        );
    }

    #[test]
    fn resolve_while_with_break() {
        check_ok("fn main() { let x = 0; while x < 10 { if x == 5 { break; } x; } }");
    }

    #[test]
    fn resolve_match_like_if_chain() {
        check_ok("fn main() { let x = 1; if x == 0 { 0; } else if x == 1 { 1; } else { x; } }");
    }

    // ===== Struct expression fields =====

    #[test]
    fn resolve_struct_expr_with_var_fields() {
        check_ok(
            "struct Point(x: i32, y: i32) fn main() { let a = 1; let b = 2; Point(x: a, y: b); }",
        );
    }

    #[test]
    fn resolve_struct_expr_undefined_in_field() {
        check_err(
            "struct Point(x: i32, y: i32) fn main() { Point(x: undef, y: 0); }",
            &["cannot find `undef`"],
        );
    }

    // ===== Return type resolution =====

    #[test]
    fn resolve_return_type() {
        check_ok("fn foo(): i32 { 0 }");
    }

    #[test]
    fn resolve_return_type_custom() {
        check_ok("struct Foo; fn bar(): Foo { Foo {} }");
    }

    #[test]
    fn resolve_return_type_undefined() {
        check_err(
            "fn foo(): UndefinedType {}",
            &["cannot find `UndefinedType`"],
        );
    }

    // ===== Let statement type annotations =====

    #[test]
    fn resolve_let_with_type_annotation() {
        check_ok("fn main() { let x: i32 = 0; }");
    }

    #[test]
    fn resolve_let_type_annotation_undefined() {
        check_err(
            "fn main() { let x: UndefinedType = 0; }",
            &["cannot find `UndefinedType`"],
        );
    }

    #[test]
    fn resolve_let_type_annotation_custom() {
        check_ok("struct Foo; fn main() { let x: Foo; }");
    }

    // ===== Where clause bound validation =====

    #[test]
    fn builtin_trait_clone_exists() {
        check_ok("fn foo(x: T) where T: Clone {}");
    }

    #[test]
    fn builtin_trait_debug_exists() {
        check_ok("fn foo(x: T) where T: Debug {}");
    }

    #[test]
    fn builtin_trait_copy_exists() {
        check_ok("fn foo(x: T) where T: Copy {}");
    }

    #[test]
    fn where_clause_unknown_bound_errors() {
        check_err(
            "fn foo(x: T) where T: UnknownTrait {}",
            &["cannot find `UnknownTrait`"],
        );
    }

    #[test]
    fn where_clause_typo_in_bound_errors() {
        check_err("fn foo(x: T) where T: Cloen {}", &["cannot find `Cloen`"]);
    }

    #[test]
    fn where_clause_multiple_bounds_all_resolve() {
        check_ok("fn foo(x: T) where T: Clone + Debug {}");
    }

    #[test]
    fn where_clause_multiple_bounds_one_fails() {
        check_err(
            "fn foo(x: T) where T: Clone + Bogus {}",
            &["cannot find `Bogus`"],
        );
    }

    #[test]
    fn struct_where_clause_bound_resolves() {
        check_ok("struct Wrapper(value: T) where T: Clone");
    }

    #[test]
    fn struct_where_clause_unknown_bound_errors() {
        check_err(
            "struct Wrapper(value: T) where T: Foo",
            &["cannot find `Foo`"],
        );
    }

    #[test]
    fn impl_where_clause_bound_resolves() {
        check_ok("struct S() impl S where T: Clone {}");
    }

    #[test]
    fn type_alias_where_clause_bound_resolves() {
        check_ok("type Alias = T where T: Clone;");
    }
}
