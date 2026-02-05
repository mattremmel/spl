//! AST pretty-printer for debugging.
//!
//! Provides a human-readable representation of the typed AST.

use crate::*;
use std::fmt::Write;

/// Pretty-printer for AST nodes.
pub struct AstPrinter {
    output: String,
    indent: usize,
}

impl Default for AstPrinter {
    fn default() -> Self {
        Self::new()
    }
}

impl AstPrinter {
    /// Create a new printer.
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    /// Print a source file.
    pub fn print_source_file(&mut self, source: &SourceFile) {
        self.line("SourceFile");
        self.indented(|p| {
            for item in source.items() {
                p.print_item(&item);
            }
        });
    }

    /// Consume the printer and return the output.
    pub fn finish(self) -> String {
        self.output
    }

    // === Items ===

    fn print_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => self.print_function(f),
            Item::Generator(g) => self.print_generator(g),
            Item::Struct(s) => self.print_struct(s),
            Item::Enum(e) => self.print_enum(e),
            Item::Trait(t) => self.print_trait(t),
            Item::Impl(i) => self.print_impl(i),
            Item::TypeAlias(t) => self.print_type_alias(t),
            Item::Extern(e) => self.print_extern_block(e),
            Item::Use(u) => self.print_use_decl(u),
            Item::Module(m) => self.print_module(m),
        }
    }

    fn print_module(&mut self, module: &crate::ModuleDef) {
        if module.visibility().is_some() {
            self.output.push_str("pub ");
        }
        self.output.push_str("module ");
        if let Some(name) = module.name()
            && let Some(token) = name.ident_token()
        {
            self.output.push_str(token.text());
        }
        self.output.push_str(" {\n");
        for item in module.items() {
            self.print_item(&item);
        }
        self.output.push_str("}\n");
    }

    fn print_use_decl(&mut self, use_decl: &crate::UseDecl) {
        // Print visibility if present
        if use_decl.visibility().is_some() {
            self.output.push_str("pub ");
        }
        // Print the use keyword and tree using the syntax text
        self.output.push_str("use ");
        if let Some(tree) = use_decl.use_tree() {
            self.output.push_str(&tree.syntax().text().to_string());
        }
        self.output.push(';');
    }

    fn print_generator(&mut self, generator: &crate::GeneratorDef) {
        let name = generator
            .name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());

        let vis = if generator.visibility().is_some() {
            "pub "
        } else {
            ""
        };
        self.line(&format!("{vis}GeneratorDef \"{name}\""));

        self.indented(|p| {
            if let Some(params) = generator.param_list() {
                p.print_param_list(&params);
            }
            if let Some(ret_ty) = generator.ret_type() {
                p.line("ReturnType");
                p.indented(|p| p.print_type(&ret_ty));
            }
            if let Some(body) = generator.body() {
                p.print_block(&body);
            }
        });
    }

    fn print_function(&mut self, func: &FunctionDef) {
        let name = func
            .name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());

        let vis = if func.visibility().is_some() {
            "pub "
        } else {
            ""
        };
        self.line(&format!("{vis}FunctionDef \"{name}\""));

        self.indented(|p| {
            if let Some(params) = func.param_list() {
                p.print_param_list(&params);
            }
            if let Some(ret_ty) = func.ret_type() {
                p.line("ReturnType");
                p.indented(|p| p.print_type(&ret_ty));
            }
            if let Some(body) = func.body() {
                p.print_block(&body);
            }
        });
    }

    fn print_struct(&mut self, s: &StructDef) {
        let name = s
            .name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());

        let vis = if s.visibility().is_some() { "pub " } else { "" };
        self.line(&format!("{vis}StructDef \"{name}\""));

        self.indented(|p| {
            if let Some(fields) = s.field_list() {
                p.print_field_list(&fields);
            }
        });
    }

    fn print_enum(&mut self, e: &crate::EnumDef) {
        let name = e
            .name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());

        let vis = if e.visibility().is_some() { "pub " } else { "" };
        self.line(&format!("{vis}EnumDef \"{name}\""));

        self.indented(|p| {
            if let Some(variants) = e.variant_list() {
                for variant in variants.variants() {
                    let vname = variant
                        .name()
                        .and_then(|n| n.ident_token())
                        .map(|t| t.text().to_string())
                        .unwrap_or_else(|| "?".to_string());
                    p.line(&format!("Variant \"{vname}\""));
                    if let Some(fields) = variant.field_list() {
                        p.indented(|p| p.print_field_list(&fields));
                    }
                }
            }
        });
    }

    fn print_trait(&mut self, t: &crate::TraitDef) {
        let name = t
            .name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());

        let vis = if t.visibility().is_some() { "pub " } else { "" };
        let unsafe_kw = if t.is_unsafe() { "unsafe " } else { "" };
        self.line(&format!("{vis}{unsafe_kw}TraitDef \"{name}\""));

        self.indented(|p| {
            for item in t.items() {
                let item_name = item
                    .name()
                    .and_then(|n| n.ident_token())
                    .map(|t| t.text().to_string())
                    .unwrap_or_else(|| "?".to_string());
                p.line(&format!("TraitItem \"{item_name}\""));
            }
        });
    }

    fn print_impl(&mut self, imp: &ImplBlock) {
        self.line("ImplBlock");
        self.indented(|p| {
            if let Some(ty) = imp.self_ty() {
                p.line("SelfType");
                p.indented(|p| p.print_type(&ty));
            }
            for item in imp.items() {
                p.print_item(&item);
            }
        });
    }

    fn print_type_alias(&mut self, alias: &TypeAlias) {
        let name = alias
            .name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());

        let vis = if alias.visibility().is_some() {
            "pub "
        } else {
            ""
        };
        self.line(&format!("{vis}TypeAlias \"{name}\""));

        self.indented(|p| {
            if let Some(ty) = alias.ty() {
                p.print_type(&ty);
            }
        });
    }

    fn print_extern_block(&mut self, extern_block: &ExternBlock) {
        let abi = extern_block
            .abi()
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "\"C\"".to_string());
        self.line(&format!("ExternBlock {abi}"));

        self.indented(|p| {
            for func in extern_block.extern_fns() {
                p.print_extern_fn(&func);
            }
        });
    }

    fn print_extern_fn(&mut self, func: &ExternFn) {
        let name = func
            .name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());

        let vis = if func.visibility().is_some() {
            "pub "
        } else {
            ""
        };
        self.line(&format!("{vis}ExternFn \"{name}\""));

        self.indented(|p| {
            if let Some(params) = func.param_list() {
                p.print_param_list(&params);
            }
            if let Some(ret_ty) = func.ret_type() {
                p.line("ReturnType");
                p.indented(|p| p.print_type(&ret_ty));
            }
        });
    }

    fn print_param_list(&mut self, params: &ParamList) {
        self.line("ParamList");
        self.indented(|p| {
            if let Some(self_param) = params.self_param() {
                let prefix = if self_param.amp().is_some() {
                    if self_param.mut_kw().is_some() {
                        "&mut "
                    } else {
                        "&"
                    }
                } else {
                    ""
                };
                p.line(&format!("SelfParam \"{prefix}self\""));
            }
            for param in params.params() {
                let name = param
                    .name()
                    .and_then(|n| n.ident_token())
                    .map(|t| t.text().to_string())
                    .unwrap_or_else(|| "?".to_string());
                p.line(&format!("Param \"{name}\""));
                p.indented(|p| {
                    if let Some(ty) = param.ty() {
                        p.print_type(&ty);
                    }
                });
            }
        });
    }

    fn print_field_list(&mut self, fields: &FieldList) {
        self.line("FieldList");
        self.indented(|p| {
            for field in fields.fields() {
                let name = field
                    .name()
                    .and_then(|n| n.ident_token())
                    .map(|t| t.text().to_string())
                    .unwrap_or_else(|| "?".to_string());
                let vis = if field.visibility().is_some() {
                    "pub "
                } else {
                    ""
                };
                p.line(&format!("{vis}FieldDef \"{name}\""));
                p.indented(|p| {
                    if let Some(ty) = field.ty() {
                        p.print_type(&ty);
                    }
                });
            }
        });
    }

    // === Statements ===

    fn print_block(&mut self, block: &Block) {
        self.line("Block");
        self.indented(|p| {
            for stmt in block.statements() {
                p.print_stmt(&stmt);
            }
            if let Some(tail) = block.tail_expr() {
                p.line("TailExpr");
                p.indented(|p| p.print_expr(&tail));
            }
        });
    }

    fn print_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(l) => self.print_let_stmt(l),
            Stmt::Expr(e) => self.print_expr_stmt(e),
        }
    }

    fn print_let_stmt(&mut self, stmt: &LetStmt) {
        let label = if stmt.mut_kw().is_some() {
            "LetStmt mut"
        } else {
            "LetStmt"
        };
        self.line(label);
        self.indented(|p| {
            if let Some(pat) = stmt.pat() {
                p.print_pattern(&pat);
            }
            if let Some(ty) = stmt.ty() {
                p.line("TypeAnnotation");
                p.indented(|p| p.print_type(&ty));
            }
            if let Some(init) = stmt.initializer() {
                p.line("Initializer");
                p.indented(|p| p.print_expr(&init));
            }
        });
    }

    fn print_expr_stmt(&mut self, stmt: &ExprStmt) {
        let has_semi = stmt.semicolon().is_some();
        self.line(&format!(
            "ExprStmt{}",
            if has_semi { "" } else { " (no semi)" }
        ));
        self.indented(|p| {
            if let Some(expr) = stmt.expr() {
                p.print_expr(&expr);
            }
        });
    }

    // === Expressions ===

    fn print_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(l) => self.print_literal(l),
            Expr::Path(p) => self.print_path_expr(p),
            Expr::Paren(p) => self.print_paren_expr(p),
            Expr::Tuple(t) => self.print_tuple_expr(t),
            Expr::Array(a) => self.print_array_expr(a),
            Expr::Call(c) => self.print_call_expr(c),
            Expr::Binary(b) => self.print_binary_expr(b),
            Expr::Prefix(p) => self.print_prefix_expr(p),
            Expr::Ref(r) => self.print_ref_expr(r),
            Expr::Field(f) => self.print_field_expr(f),
            Expr::Index(i) => self.print_index_expr(i),
            Expr::Slice(s) => self.print_slice_expr(s),
            Expr::If(i) => self.print_if_expr(i),
            Expr::While(w) => self.print_while_expr(w),
            Expr::For(f) => self.print_for_expr(f),
            Expr::Loop(l) => self.print_loop_expr(l),
            Expr::Break(b) => self.print_break_expr(b),
            Expr::Continue(_) => self.line("ContinueExpr"),
            Expr::Return(r) => self.print_return_expr(r),
            Expr::Yield(y) => self.print_yield_expr(y),
            Expr::Block(b) => self.print_block_expr(b),

            Expr::Range(r) => self.print_range_expr(r),
            Expr::Is(i) => self.print_is_expr(i),
            Expr::Match(m) => self.print_match_expr(m),
            Expr::EnumShorthand(e) => self.print_enum_shorthand_expr(e),
            Expr::Try(t) => self.print_try_expr(t),
            Expr::OptionalField(o) => self.print_optional_field_expr(o),
            Expr::Dollar(_) => self.line("DollarExpr"),
            Expr::Closure(c) => self.print_closure_expr(c),
            Expr::Unsafe(u) => self.print_unsafe_expr(u),
            Expr::Throw(t) => self.print_throw_expr(t),
        }
    }

    fn print_try_expr(&mut self, expr: &crate::TryExpr) {
        self.line("TryExpr");
        self.indented(|p| {
            if let Some(inner) = expr.expr() {
                p.print_expr(&inner);
            }
        });
    }

    fn print_optional_field_expr(&mut self, expr: &crate::OptionalFieldExpr) {
        self.line("OptionalFieldExpr");
        self.indented(|p| {
            if let Some(inner) = expr.expr() {
                p.print_expr(&inner);
            }
            if let Some(name) = expr.name_token() {
                p.line(&format!("field: {}", name.text()));
            }
        });
    }

    fn print_closure_expr(&mut self, expr: &crate::ClosureExpr) {
        self.line("ClosureExpr");
        self.indented(|p| {
            if let Some(captures) = expr.capture_list() {
                p.line("CaptureList");
                p.indented(|pp| {
                    for capture in captures.captures() {
                        let name = capture
                            .name()
                            .and_then(|n| n.ident_token())
                            .map(|t| t.text().to_string())
                            .unwrap_or_else(|| "?".to_string());
                        if let Some(cap_expr) = capture.expr() {
                            pp.line(&format!("Capture \"{name}\" ="));
                            pp.indented(|ppp| {
                                ppp.print_expr(&cap_expr);
                            });
                        } else {
                            pp.line(&format!("Capture \"{name}\""));
                        }
                    }
                });
            }
            if let Some(params) = expr.params() {
                p.line("ClosureParams");
                p.indented(|pp| {
                    for param in params.params() {
                        let name = param
                            .name()
                            .and_then(|n| n.ident_token())
                            .map(|t| t.text().to_string())
                            .unwrap_or_else(|| "?".to_string());
                        if let Some(ty) = param.ty() {
                            pp.line(&format!("Param \"{name}\": {}", ty.syntax()));
                        } else {
                            pp.line(&format!("Param \"{name}\""));
                        }
                    }
                });
            }
            if let Some(body) = expr.body() {
                p.line("Body:");
                p.indented(|pp| {
                    pp.print_expr(&body);
                });
            }
        });
    }

    fn print_unsafe_expr(&mut self, expr: &crate::UnsafeExpr) {
        self.line("UnsafeExpr");
        self.indented(|p| {
            if let Some(block) = expr.block() {
                p.print_block(&block);
            }
        });
    }

    fn print_throw_expr(&mut self, expr: &crate::ThrowExpr) {
        self.line("ThrowExpr");
        self.indented(|p| {
            if let Some(inner) = expr.expr() {
                p.print_expr(&inner);
            }
        });
    }

    fn print_enum_shorthand_expr(&mut self, expr: &crate::EnumShorthandExpr) {
        let name = expr
            .variant_name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("EnumShorthandExpr \".{name}\""));
        self.indented(|p| {
            for arg in expr.args() {
                let arg_name = arg.name_token().map(|t| t.text().to_string()).or_else(|| {
                    arg.name()
                        .and_then(|n| n.token())
                        .map(|t| t.text().to_string())
                });
                if let Some(arg_name) = arg_name {
                    p.line(&format!("NamedArg \"{arg_name}\""));
                } else {
                    p.line("PositionalArg");
                }
                p.indented(|p| {
                    if let Some(value) = arg.value() {
                        p.print_expr(&value);
                    }
                });
            }
        });
    }

    fn print_is_expr(&mut self, is_expr: &IsExpr) {
        // Note: 'is not' syntax was removed - all IsExpr are now non-negated
        self.line("IsExpr");
        self.indented(|p| {
            if let Some(lhs) = is_expr.lhs() {
                p.print_expr(&lhs);
            }
            if let Some(pat) = is_expr.pattern() {
                p.line(&format!("Pattern: {}", pat.syntax()));
            }
        });
    }

    fn print_match_expr(&mut self, match_expr: &MatchExpr) {
        self.line("MatchExpr");
        self.indented(|p| {
            if let Some(scrutinee) = match_expr.scrutinee() {
                p.print_expr(&scrutinee);
            }
            for arm in match_expr.arms() {
                p.line("MatchArm");
                p.indented(|p| {
                    if let Some(guard) = arm.guard() {
                        p.line("Guard:");
                        p.indented(|p| {
                            p.print_expr(&guard);
                        });
                    }
                    if let Some(body) = arm.body() {
                        p.line("Body:");
                        p.indented(|p| {
                            p.print_expr(&body);
                        });
                    }
                });
            }
        });
    }

    fn print_literal(&mut self, lit: &LiteralExpr) {
        let value = lit
            .token()
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("Literal {value}"));
    }

    fn print_path_expr(&mut self, path_expr: &PathExpr) {
        if let Some(path) = path_expr.path() {
            self.print_path(&path);
        }
    }

    fn print_path(&mut self, path: &Path) {
        let segments: Vec<_> = path
            .segments()
            .filter_map(|s| {
                s.name()
                    .and_then(|n| n.token())
                    .map(|t| t.text().to_string())
            })
            .collect();
        let path_str = segments.join(".");
        // Check if there are generic args
        let has_generics = path.segments().any(|s| s.generic_args().is_some());
        if has_generics {
            self.line(&format!("Path \"{path_str}\""));
            self.indented(|p| {
                for seg in path.segments() {
                    if let Some(args) = seg.generic_args() {
                        p.line("GenericArgs");
                        p.indented(|p| {
                            for ty in args.args() {
                                p.print_type(&ty);
                            }
                        });
                    }
                }
            });
        } else {
            self.line(&format!("Path \"{path_str}\""));
        }
    }

    fn print_paren_expr(&mut self, paren: &ParenExpr) {
        self.line("ParenExpr");
        self.indented(|p| {
            if let Some(expr) = paren.expr() {
                p.print_expr(&expr);
            }
        });
    }

    fn print_tuple_expr(&mut self, tuple: &TupleExpr) {
        self.line("TupleExpr");
        self.indented(|p| {
            for expr in tuple.exprs() {
                p.print_expr(&expr);
            }
        });
    }

    fn print_array_expr(&mut self, array: &ArrayExpr) {
        if array.is_repeat() {
            self.line("ArrayRepeatExpr");
        } else {
            self.line("ArrayExpr");
        }
        self.indented(|p| {
            for expr in array.exprs() {
                p.print_expr(&expr);
            }
        });
    }

    fn print_call_expr(&mut self, call: &CallExpr) {
        // Try to extract a meaningful name for the call
        let callee_str = if let Some(callee) = call.callee() {
            match &callee {
                Expr::Path(path_expr) => path_expr
                    .path()
                    .map(|p| {
                        p.segments()
                            .filter_map(|s| {
                                s.name()
                                    .and_then(|n| n.token())
                                    .map(|t| t.text().to_string())
                            })
                            .collect::<Vec<_>>()
                            .join(".")
                    })
                    .unwrap_or_else(|| "?".to_string()),
                Expr::Field(field_expr) => {
                    let field_name = field_expr
                        .name_token()
                        .or_else(|| field_expr.tuple_index_token())
                        .map(|t| t.text().to_string())
                        .or_else(|| {
                            field_expr
                                .name()
                                .and_then(|n| n.token())
                                .map(|t| t.text().to_string())
                        })
                        .unwrap_or_else(|| "?".to_string());
                    format!(".{field_name}")
                }
                _ => "expr".to_string(),
            }
        } else {
            "?".to_string()
        };

        self.line(&format!("CallExpr \"{callee_str}\""));
        self.indented(|p| {
            // Print receiver for method calls (callee is FieldExpr)
            if let Some(Expr::Field(field_expr)) = call.callee()
                && let Some(recv) = field_expr.expr()
            {
                p.line("Receiver");
                p.indented(|p| p.print_expr(&recv));
            }
            // Print arguments
            for arg in call.args() {
                let name = arg.name_token().map(|t| t.text().to_string()).or_else(|| {
                    arg.name()
                        .and_then(|n| n.token())
                        .map(|t| t.text().to_string())
                });
                if let Some(name) = name {
                    p.line(&format!("NamedArg \"{name}\""));
                } else {
                    p.line("PositionalArg");
                }
                p.indented(|p| {
                    if let Some(expr) = arg.value() {
                        p.print_expr(&expr);
                    }
                });
            }
        });
    }

    fn print_binary_expr(&mut self, bin: &BinExpr) {
        let op = bin
            .op_token()
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("BinaryExpr \"{op}\""));
        self.indented(|p| {
            if let Some(lhs) = bin.lhs() {
                p.print_expr(&lhs);
            }
            if let Some(rhs) = bin.rhs() {
                p.print_expr(&rhs);
            }
        });
    }

    fn print_prefix_expr(&mut self, prefix: &PrefixExpr) {
        let op = prefix
            .op_token()
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("PrefixExpr \"{op}\""));
        self.indented(|p| {
            if let Some(expr) = prefix.expr() {
                p.print_expr(&expr);
            }
        });
    }

    fn print_ref_expr(&mut self, r: &RefExpr) {
        let mut_str = if r.mut_kw().is_some() { "mut " } else { "" };
        self.line(&format!("RefExpr \"&{mut_str}\""));
        self.indented(|p| {
            if let Some(expr) = r.expr() {
                p.print_expr(&expr);
            }
        });
    }

    fn print_field_expr(&mut self, field: &FieldExpr) {
        let name = field
            .name_token()
            .or_else(|| field.tuple_index_token())
            .map(|t| t.text().to_string())
            .or_else(|| {
                field
                    .name()
                    .and_then(|n| n.token())
                    .map(|t| t.text().to_string())
            })
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("FieldExpr \".{name}\""));
        self.indented(|p| {
            if let Some(expr) = field.expr() {
                p.print_expr(&expr);
            }
        });
    }

    fn print_index_expr(&mut self, index: &IndexExpr) {
        self.line("IndexExpr");
        self.indented(|p| {
            if let Some(base) = index.base() {
                p.line("Base");
                p.indented(|p| p.print_expr(&base));
            }
            if let Some(idx) = index.index() {
                p.line("Index");
                p.indented(|p| p.print_expr(&idx));
            }
        });
    }

    fn print_slice_expr(&mut self, slice: &SliceExpr) {
        self.line("SliceExpr");
        self.indented(|p| {
            if let Some(base) = slice.base() {
                p.line("Base");
                p.indented(|p| p.print_expr(&base));
            }
            if let Some(start) = slice.start() {
                p.line("Start");
                p.indented(|p| p.print_expr(&start));
            }
            if let Some(end) = slice.end() {
                p.line("End");
                p.indented(|p| p.print_expr(&end));
            }
        });
    }

    fn print_if_expr(&mut self, if_expr: &IfExpr) {
        self.line("IfExpr");
        self.indented(|p| {
            if let Some(cond) = if_expr.condition() {
                p.line("Condition");
                p.indented(|p| p.print_expr(&cond));
            }
            if let Some(then_branch) = if_expr.then_branch() {
                p.line("Then");
                p.indented(|p| p.print_block(&then_branch));
            }
            // Try else_branch as expression first, then as block
            if let Some(else_branch) = if_expr.else_branch() {
                p.line("Else");
                p.indented(|p| p.print_expr(&else_branch));
            } else if let Some(else_block) = if_expr.else_block() {
                p.line("Else");
                p.indented(|p| p.print_block(&else_block));
            }
        });
    }

    fn print_while_expr(&mut self, while_expr: &WhileExpr) {
        self.line("WhileExpr");
        self.indented(|p| {
            if let Some(cond) = while_expr.condition() {
                p.line("Condition");
                p.indented(|p| p.print_expr(&cond));
            }
            if let Some(body) = while_expr.body() {
                p.print_block(&body);
            }
        });
    }

    fn print_for_expr(&mut self, for_expr: &ForExpr) {
        self.line("ForExpr");
        self.indented(|p| {
            if let Some(pat) = for_expr.pat() {
                p.line("Pattern");
                p.indented(|p| p.print_pattern(&pat));
            }
            if let Some(iter) = for_expr.iterable() {
                p.line("Iterable");
                p.indented(|p| p.print_expr(&iter));
            }
            if let Some(body) = for_expr.body() {
                p.print_block(&body);
            }
        });
    }

    fn print_loop_expr(&mut self, loop_expr: &LoopExpr) {
        self.line("LoopExpr");
        self.indented(|p| {
            if let Some(body) = loop_expr.body() {
                p.print_block(&body);
            }
        });
    }

    fn print_break_expr(&mut self, break_expr: &BreakExpr) {
        self.line("BreakExpr");
        if let Some(expr) = break_expr.expr() {
            self.indented(|p| p.print_expr(&expr));
        }
    }

    fn print_return_expr(&mut self, return_expr: &ReturnExpr) {
        self.line("ReturnExpr");
        if let Some(expr) = return_expr.expr() {
            self.indented(|p| p.print_expr(&expr));
        }
    }

    fn print_yield_expr(&mut self, yield_expr: &YieldExpr) {
        self.line("YieldExpr");
        if let Some(expr) = yield_expr.expr() {
            self.indented(|p| p.print_expr(&expr));
        }
    }

    fn print_block_expr(&mut self, block_expr: &BlockExpr) {
        self.line("BlockExpr");
        self.indented(|p| {
            if let Some(block) = block_expr.block() {
                p.print_block(&block);
            }
        });
    }

    fn print_range_expr(&mut self, range: &RangeExpr) {
        let has_start = range.start().is_some();
        let has_end = range.end().is_some();
        let kind = match (has_start, has_end) {
            (true, true) => "RangeExpr",
            (true, false) => "RangeFromExpr",
            (false, true) => "RangeToExpr",
            (false, false) => "RangeFullExpr",
        };
        self.line(kind);
        self.indented(|p| {
            if let Some(start) = range.start() {
                p.line("Start");
                p.indented(|p| p.print_expr(&start));
            }
            if let Some(end) = range.end() {
                p.line("End");
                p.indented(|p| p.print_expr(&end));
            }
        });
    }

    // === Types ===

    fn print_type(&mut self, ty: &Type) {
        match ty {
            Type::Ref(r) => self.print_ref_type(r),
            Type::Array(a) => self.print_array_type(a),
            Type::Slice(s) => self.print_slice_type(s),
            Type::Tuple(t) => self.print_tuple_type(t),
            Type::FnPtr(f) => self.print_fn_ptr_type(f),
            Type::Path(p) => self.print_path_type(p),
            Type::Never(_) => self.line("NeverType \"!\""),
            Type::Optional(o) => {
                self.line("OptionalType \"?\"");
                self.indented(|p| {
                    if let Some(ty) = o.ty() {
                        p.print_type(&ty);
                    }
                });
            }
        }
    }

    fn print_ref_type(&mut self, r: &RefType) {
        let mut_str = if r.mut_kw().is_some() { "mut " } else { "" };
        self.line(&format!("RefType \"&{mut_str}\""));
        self.indented(|p| {
            if let Some(ty) = r.ty() {
                p.print_type(&ty);
            }
        });
    }

    fn print_array_type(&mut self, a: &ArrayType) {
        self.line("ArrayType");
        self.indented(|p| {
            if let Some(elem) = a.elem_ty() {
                p.line("ElementType");
                p.indented(|p| p.print_type(&elem));
            }
            if let Some(len) = a.len() {
                p.line("Length");
                p.indented(|p| p.print_expr(&len));
            }
        });
    }

    fn print_slice_type(&mut self, s: &SliceType) {
        self.line("SliceType");
        self.indented(|p| {
            if let Some(elem) = s.elem_ty() {
                p.print_type(&elem);
            }
        });
    }

    fn print_tuple_type(&mut self, t: &TupleType) {
        self.line("TupleType");
        self.indented(|p| {
            for ty in t.types() {
                p.print_type(&ty);
            }
        });
    }

    fn print_fn_ptr_type(&mut self, f: &FnPtrType) {
        self.line("FnPtrType");
        self.indented(|p| {
            p.line("Params");
            p.indented(|p| {
                // All types except possibly the last are params
                let types: Vec<_> = f.param_types().collect();
                // If there's an arrow, last type is return type
                // For now, treat all but last as params
                for ty in &types[..types.len().saturating_sub(1)] {
                    p.print_type(ty);
                }
            });
            if let Some(ret) = f.ret_type() {
                p.line("ReturnType");
                p.indented(|p| p.print_type(&ret));
            }
        });
    }

    fn print_path_type(&mut self, p: &PathType) {
        if let Some(path) = p.path() {
            self.print_path(&path);
        }
    }

    // === Patterns ===

    fn print_pattern(&mut self, pat: &Pat) {
        match pat {
            Pat::Ident(i) => self.print_ident_pat(i),
            Pat::Wildcard(_) => self.line("WildcardPat \"_\""),
            Pat::Literal(l) => self.print_literal_pat(l),
            Pat::Range(r) => self.print_range_pat(r),
            Pat::Tuple(t) => self.print_tuple_pat(t),
            Pat::Slice(s) => self.print_slice_pat(s),
            Pat::Struct(s) => self.print_struct_pat(s),
            Pat::Ref(r) => self.print_ref_pat(r),
            Pat::Rest(_) => self.line("RestPat \"..\""),
            Pat::EnumShorthand(e) => self.print_enum_shorthand_pat(e),
            Pat::Or(o) => self.print_or_pat(o),
            Pat::Grouped(g) => self.print_grouped_pat(g),
        }
    }

    fn print_ident_pat(&mut self, ident: &IdentPat) {
        let name = ident
            .name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());
        let mut_str = if ident.mut_kw().is_some() { "mut " } else { "" };
        self.line(&format!("IdentPat \"{mut_str}{name}\""));
    }

    fn print_literal_pat(&mut self, lit: &LiteralPat) {
        let value = lit
            .token()
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("LiteralPat {value}"));
    }

    fn print_range_pat(&mut self, range: &RangePat) {
        self.line("RangePat");
        self.indented(|p| {
            if let Some(start) = range.start() {
                p.line(&format!("Start {}", start.text()));
            }
            if let Some(end) = range.end() {
                p.line(&format!("End {}", end.text()));
            }
        });
    }

    fn print_tuple_pat(&mut self, tuple: &TuplePat) {
        self.line("TuplePat");
        self.indented(|p| {
            for pat in tuple.patterns() {
                p.print_pattern(&pat);
            }
        });
    }

    fn print_slice_pat(&mut self, slice: &SlicePat) {
        self.line("SlicePat");
        self.indented(|p| {
            for pat in slice.patterns() {
                p.print_pattern(&pat);
            }
        });
    }

    fn print_struct_pat(&mut self, s: &StructPat) {
        let name = s
            .path()
            .map(|p| {
                p.segments()
                    .filter_map(|seg| {
                        seg.name()
                            .and_then(|n| n.token())
                            .map(|t| t.text().to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("StructPat \"{name}\""));
        self.indented(|p| {
            for field in s.fields() {
                let field_name = field
                    .name()
                    .and_then(|n| n.token())
                    .map(|t| t.text().to_string())
                    .unwrap_or_else(|| "?".to_string());
                if let Some(pat) = field.pat() {
                    p.line(&format!("Field \"{field_name}\""));
                    p.indented(|p| p.print_pattern(&pat));
                } else {
                    // Shorthand: `Point { x, y }` means `Point { x: x, y: y }`
                    p.line(&format!("Field \"{field_name}\" (shorthand)"));
                }
            }
            if s.rest().is_some() {
                p.line("RestPat \"..\"");
            }
        });
    }

    fn print_ref_pat(&mut self, r: &RefPat) {
        let mut_str = if r.mut_kw().is_some() { "mut " } else { "" };
        self.line(&format!("RefPat \"&{mut_str}\""));
        self.indented(|p| {
            if let Some(pat) = r.pat() {
                p.print_pattern(&pat);
            }
        });
    }

    fn print_enum_shorthand_pat(&mut self, pat: &crate::EnumShorthandPat) {
        let name = pat
            .variant_name()
            .and_then(|n| n.ident_token())
            .map(|t| t.text().to_string())
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("EnumShorthandPat \".{name}\""));
        self.indented(|p| {
            for inner in pat.patterns() {
                p.print_pattern(&inner);
            }
        });
    }

    fn print_or_pat(&mut self, pat: &crate::OrPat) {
        self.line("OrPat");
        self.indented(|p| {
            for alt in pat.alternatives() {
                p.print_pattern(&alt);
            }
        });
    }

    fn print_grouped_pat(&mut self, pat: &crate::GroupedPat) {
        self.line("GroupedPat");
        self.indented(|p| {
            if let Some(inner) = pat.inner() {
                p.print_pattern(&inner);
            }
        });
    }

    // === Helpers ===

    fn line(&mut self, text: &str) {
        let _ = writeln!(self.output, "{:indent$}{text}", "", indent = self.indent);
    }

    fn indented<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Self),
    {
        self.indent += 2;
        f(self);
        self.indent -= 2;
    }
}

/// Convenience function to pretty-print a source file.
pub fn pretty_print(source: &SourceFile) -> String {
    let mut printer = AstPrinter::new();
    printer.print_source_file(source);
    printer.finish()
}

#[cfg(test)]
mod tests;
