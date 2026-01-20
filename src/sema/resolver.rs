//! Name resolution for SPL.
//!
//! Resolves identifiers to their declarations using a two-pass approach:
//! - Pass 1: Collect all top-level definitions (functions, structs, type aliases)
//! - Pass 2: Walk the AST and resolve all name references

use crate::DefId;
use crate::ast::{
    Block, Expr, FieldDef, FunctionDef, GenericParam, GenericParams, ImplBlock, Item, LetStmt,
    Name, NameRef, Param, ParamList, Pat, Path, PathSegment, SelfParam, SourceFile, Stmt,
    StructDef, StructExpr, StructExprField, StructPat, StructPatField, Type, TypeAlias,
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

        // Pass 2: Resolve all references
        self.resolve_source_file(source_file);

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
        name_ref.ident_token()
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
    ) -> Option<DefId> {
        let token = Self::get_ident_token(name)?;
        let name_text = token.text().to_string();
        let span = Self::text_range_to_span(token.text_range());
        let interned = self.ctx.intern(&name_text);

        match self.ctx.define(interned, kind, visibility, span.clone()) {
            Ok(def_id) => Some(def_id),
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
            self.define_name(&name, SymbolKind::Function, vis);
        }
    }

    fn collect_struct(&mut self, struct_def: &StructDef) {
        if let Some(name) = struct_def.name() {
            let vis = self.convert_visibility(&struct_def.visibility());
            self.define_name(&name, SymbolKind::Struct, vis);
        }
    }

    fn collect_type_alias(&mut self, type_alias: &TypeAlias) {
        if let Some(name) = type_alias.name() {
            let vis = self.convert_visibility(&type_alias.visibility());
            self.define_name(&name, SymbolKind::TypeAlias, vis);
        }
    }

    fn collect_impl_block(&mut self, impl_block: &ImplBlock) {
        // Impl blocks don't define a name themselves, but we collect methods
        // Note: We need to enter impl scope to define methods, but method resolution
        // is complex (needs type info). For now, we'll just collect methods as functions.
        self.ctx.enter_scope(ScopeKind::Impl);

        for item in impl_block.items() {
            if let Item::Function(func) = item
                && let Some(name) = func.name()
            {
                let vis = self.convert_visibility(&func.visibility());
                self.define_name(&name, SymbolKind::Function, vis);
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

        // Define generic parameters
        if let Some(generics) = func.generic_params() {
            self.define_generic_params(&generics);
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

        // Define generic parameters
        if let Some(generics) = struct_def.generic_params() {
            self.define_generic_params(&generics);
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
            self.define_name(&name, SymbolKind::Field, vis);
        }

        // Resolve the field type
        if let Some(ty) = field.ty() {
            self.resolve_type(&ty);
        }
    }

    fn resolve_type_alias(&mut self, type_alias: &TypeAlias) {
        // Resolve the aliased type
        if let Some(ty) = type_alias.ty() {
            self.resolve_type(&ty);
        }
    }

    fn resolve_impl_block(&mut self, impl_block: &ImplBlock) {
        self.ctx.enter_scope(ScopeKind::Impl);

        // Define generic parameters
        if let Some(generics) = impl_block.generic_params() {
            self.define_generic_params(&generics);
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

    fn define_generic_params(&mut self, generics: &GenericParams) {
        for param in generics.params() {
            self.define_generic_param(&param);
        }
    }

    fn define_generic_param(&mut self, param: &GenericParam) {
        if let Some(name) = param.name() {
            self.define_name(&name, SymbolKind::TypeParam, Visibility::Private);
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

    fn define_self_param(&mut self, _self_param: &SelfParam) {
        // Define `self` as a special parameter
        let interned = self.ctx.intern("self");
        // Use a dummy span for self - we could compute it from the token if needed
        let _ = self
            .ctx
            .define(interned, SymbolKind::SelfParam, Visibility::Private, 0..0);
    }

    fn define_param(&mut self, param: &Param) {
        // Define the parameter name
        if let Some(name) = param.name() {
            self.define_name(&name, SymbolKind::Parameter, Visibility::Private);
        }

        // Resolve the parameter type
        if let Some(ty) = param.ty() {
            self.resolve_type(&ty);
        }
    }

    fn resolve_block(&mut self, block: &Block) {
        self.ctx.enter_scope(ScopeKind::Block);

        for stmt in block.statements() {
            self.resolve_stmt(&stmt);
        }

        if let Some(tail_expr) = block.tail_expr() {
            self.resolve_expr(&tail_expr);
        }

        self.ctx.exit_scope();
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
        // Resolve the initializer first (before defining the binding)
        if let Some(init) = let_stmt.initializer() {
            self.resolve_expr(&init);
        }

        // Resolve the type annotation
        if let Some(ty) = let_stmt.ty() {
            self.resolve_type(&ty);
        }

        // Define the pattern binding
        if let Some(pat) = let_stmt.pat() {
            self.define_pattern(&pat);
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
                if let Some(else_branch) = if_expr.else_branch() {
                    self.resolve_expr(&else_branch);
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

    fn resolve_for_expr(&mut self, for_expr: &crate::ast::ForExpr) {
        // Resolve iterable first (before entering for scope)
        if let Some(iterable) = for_expr.iterable() {
            self.resolve_expr(&iterable);
        }

        // Enter for-loop scope
        self.ctx.enter_scope(ScopeKind::ForLoop);

        // Define the loop variable
        if let Some(pat) = for_expr.pat() {
            self.define_pattern(&pat);
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

    fn define_pattern(&mut self, pat: &Pat) {
        match pat {
            Pat::Ident(ident_pat) => {
                // Get IDENT token directly from IdentPat (may be wrapped in Name or direct)
                let token = ident_pat
                    .name()
                    .and_then(|n| Self::get_ident_token(&n))
                    .or_else(|| {
                        crate::ast::token(ident_pat.syntax(), crate::syntax::SyntaxKind::IDENT)
                    });

                if let Some(token) = token {
                    let name_text = token.text().to_string();
                    let span = Self::text_range_to_span(token.text_range());
                    let interned = self.ctx.intern(&name_text);
                    match self.ctx.define(
                        interned,
                        SymbolKind::Local,
                        Visibility::Private,
                        span.clone(),
                    ) {
                        Ok(_) => {}
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
                if let Some(start) = range_pat.start() {
                    self.define_pattern(&start);
                }
                if let Some(end) = range_pat.end() {
                    self.define_pattern(&end);
                }
            }
            Pat::Tuple(tuple_pat) => {
                for inner in tuple_pat.patterns() {
                    self.define_pattern(&inner);
                }
            }
            Pat::Slice(slice_pat) => {
                for inner in slice_pat.patterns() {
                    self.define_pattern(&inner);
                }
            }
            Pat::Struct(struct_pat) => self.define_struct_pattern(struct_pat),
            Pat::Ref(ref_pat) => {
                if let Some(inner) = ref_pat.pat() {
                    self.define_pattern(&inner);
                }
            }
            Pat::Rest(_) => {}
        }
    }

    fn define_struct_pattern(&mut self, struct_pat: &StructPat) {
        // Resolve the struct type path
        if let Some(path) = struct_pat.path() {
            self.resolve_path(&path);
        }

        // Define bindings in struct pattern fields
        for field in struct_pat.fields() {
            self.define_struct_pat_field(&field);
        }
    }

    fn define_struct_pat_field(&mut self, field: &StructPatField) {
        // If there's a nested pattern, define it
        // If not, the field name itself becomes a binding
        if let Some(pat) = field.pat() {
            self.define_pattern(&pat);
        } else if let Some(name_ref) = field.name() {
            // Shorthand syntax: `Point { x, y }` means `Point { x: x, y: y }`
            // The name_ref becomes a local binding
            if let Some(token) = Self::get_name_ref_token(&name_ref) {
                let text = token.text().to_string();
                let span = Self::text_range_to_span(token.text_range());
                let interned = self.ctx.intern(&text);
                let _ = self
                    .ctx
                    .define(interned, SymbolKind::Local, Visibility::Private, span);
            }
        }
    }

    // ===== Type Resolution =====

    fn resolve_type(&mut self, ty: &Type) {
        match ty {
            Type::Path(path_type) => {
                if let Some(path) = path_type.path() {
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
        let _ = ctx.define(name, SymbolKind::Struct, Visibility::Public, 0..0);
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
        check_ok("struct Foo {} fn main() { let x: Foo; }");
    }

    #[test]
    fn resolve_function_with_params() {
        check_ok("fn add(a: i32, b: i32) { a + b; }");
    }

    #[test]
    fn resolve_nested_blocks() {
        check_ok("fn main() { let x = 1; { let y = x; { let z = y; z; } } }");
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
        check_ok("struct Point { x: i32, y: i32 } fn main() { Point { x: 1, y: 2 }; }");
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
        check_ok("fn identity<T>(x: T) -> T { x }");
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
        check_err("struct Foo {} struct Foo {}", &["defined multiple times"]);
    }
}
