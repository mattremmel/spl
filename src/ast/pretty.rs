//! AST pretty-printer for debugging.
//!
//! Provides a human-readable representation of the typed AST.

use crate::ast::*;
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
            Item::Struct(s) => self.print_struct(s),
            Item::Impl(i) => self.print_impl(i),
            Item::TypeAlias(t) => self.print_type_alias(t),
        }
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
            if let Some(generics) = func.generic_params() {
                p.print_generic_params(&generics);
            }
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
            if let Some(generics) = s.generic_params() {
                p.print_generic_params(&generics);
            }
            if let Some(fields) = s.field_list() {
                p.print_field_list(&fields);
            }
        });
    }

    fn print_impl(&mut self, imp: &ImplBlock) {
        self.line("ImplBlock");
        self.indented(|p| {
            if let Some(generics) = imp.generic_params() {
                p.print_generic_params(&generics);
            }
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
            if let Some(generics) = alias.generic_params() {
                p.print_generic_params(&generics);
            }
            if let Some(ty) = alias.ty() {
                p.print_type(&ty);
            }
        });
    }

    fn print_generic_params(&mut self, params: &GenericParams) {
        self.line("GenericParams");
        self.indented(|p| {
            for param in params.params() {
                let name = param
                    .name()
                    .and_then(|n| n.ident_token())
                    .map(|t| t.text().to_string())
                    .unwrap_or_else(|| "?".to_string());
                p.line(&format!("GenericParam \"{name}\""));
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
            Expr::Struct(s) => self.print_struct_expr(s),
            Expr::Binary(b) => self.print_binary_expr(b),
            Expr::Prefix(p) => self.print_prefix_expr(p),
            Expr::Ref(r) => self.print_ref_expr(r),
            Expr::Field(f) => self.print_field_expr(f),
            Expr::MethodCall(m) => self.print_method_call_expr(m),
            Expr::Call(c) => self.print_call_expr(c),
            Expr::Index(i) => self.print_index_expr(i),
            Expr::Slice(s) => self.print_slice_expr(s),
            Expr::If(i) => self.print_if_expr(i),
            Expr::While(w) => self.print_while_expr(w),
            Expr::For(f) => self.print_for_expr(f),
            Expr::Loop(l) => self.print_loop_expr(l),
            Expr::Break(b) => self.print_break_expr(b),
            Expr::Continue(_) => self.line("ContinueExpr"),
            Expr::Return(r) => self.print_return_expr(r),
            Expr::Block(b) => self.print_block_expr(b),
            Expr::Cast(c) => self.print_cast_expr(c),
            Expr::Range(r) => self.print_range_expr(r),
        }
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
        let path_str = segments.join("::");
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

    fn print_struct_expr(&mut self, s: &StructExpr) {
        let path_str = s
            .path()
            .map(|p| {
                p.segments()
                    .filter_map(|s| {
                        s.name()
                            .and_then(|n| n.token())
                            .map(|t| t.text().to_string())
                    })
                    .collect::<Vec<_>>()
                    .join("::")
            })
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("StructExpr \"{path_str}\""));
        self.indented(|p| {
            for field in s.fields() {
                let name = field
                    .name_token()
                    .map(|t| t.text().to_string())
                    .or_else(|| {
                        field
                            .name()
                            .and_then(|n| n.token())
                            .map(|t| t.text().to_string())
                    })
                    .unwrap_or_else(|| "?".to_string());
                p.line(&format!("Field \"{name}\""));
                p.indented(|p| {
                    if let Some(expr) = field.expr() {
                        p.print_expr(&expr);
                    }
                });
            }
            if let Some(base) = s.update_base() {
                p.line("UpdateBase");
                p.indented(|p| {
                    if let Some(expr) = base.expr() {
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

    fn print_method_call_expr(&mut self, method: &MethodCallExpr) {
        let name = method
            .name_token()
            .map(|t| t.text().to_string())
            .or_else(|| {
                method
                    .name()
                    .and_then(|n| n.token())
                    .map(|t| t.text().to_string())
            })
            .unwrap_or_else(|| "?".to_string());
        self.line(&format!("MethodCallExpr \".{name}()\""));
        self.indented(|p| {
            if let Some(recv) = method.receiver() {
                p.line("Receiver");
                p.indented(|p| p.print_expr(&recv));
            }
            if let Some(args) = method.arg_list() {
                p.print_arg_list(&args);
            }
        });
    }

    fn print_call_expr(&mut self, call: &CallExpr) {
        self.line("CallExpr");
        self.indented(|p| {
            if let Some(callee) = call.callee() {
                p.line("Callee");
                p.indented(|p| p.print_expr(&callee));
            }
            if let Some(args) = call.arg_list() {
                p.print_arg_list(&args);
            }
        });
    }

    fn print_arg_list(&mut self, args: &ArgList) {
        self.line("ArgList");
        self.indented(|p| {
            for arg in args.args() {
                p.print_expr(&arg);
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

    fn print_block_expr(&mut self, block_expr: &BlockExpr) {
        self.line("BlockExpr");
        self.indented(|p| {
            if let Some(block) = block_expr.block() {
                p.print_block(&block);
            }
        });
    }

    fn print_cast_expr(&mut self, cast: &CastExpr) {
        self.line("CastExpr");
        self.indented(|p| {
            if let Some(expr) = cast.expr() {
                p.print_expr(&expr);
            }
            if let Some(ty) = cast.ty() {
                p.line("TargetType");
                p.indented(|p| p.print_type(&ty));
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
                p.line("Start");
                p.indented(|p| p.print_pattern(&start));
            }
            if let Some(end) = range.end() {
                p.line("End");
                p.indented(|p| p.print_pattern(&end));
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
                    .join("::")
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
mod tests {
    use super::*;
    use crate::parser::parse;
    use expect_test::{Expect, expect};

    fn check(source: &str, expected: &Expect) {
        let parsed = parse(source);
        let source_file = SourceFile::cast(parsed.syntax()).unwrap();
        let output = pretty_print(&source_file);
        expected.assert_eq(&output);
    }

    mod items {
        use super::*;

        #[test]
        fn function_empty() {
            check(
                "fn foo() {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                        Block
                "#]],
            );
        }

        #[test]
        fn function_with_params() {
            check(
                "fn add(x: i32, y: i32) -> i32 { x + y }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "add"
                        ParamList
                          Param "x"
                            Path "i32"
                          Param "y"
                            Path "i32"
                        ReturnType
                          Path "i32"
                        Block
                          TailExpr
                            BinaryExpr "+"
                              Path "x"
                              Path "y"
                "#]],
            );
        }

        #[test]
        fn function_with_self() {
            check(
                "fn method(&self) {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "method"
                        ParamList
                          SelfParam "&self"
                        Block
                "#]],
            );
        }

        #[test]
        fn function_with_mut_self() {
            check(
                "fn method(&mut self) {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "method"
                        ParamList
                          SelfParam "&mut self"
                        Block
                "#]],
            );
        }

        #[test]
        fn function_generic() {
            check(
                "fn id<T>(x: T) -> T { x }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "id"
                        GenericParams
                          GenericParam "T"
                        ParamList
                          Param "x"
                            Path "T"
                        ReturnType
                          Path "T"
                        Block
                          TailExpr
                            Path "x"
                "#]],
            );
        }

        #[test]
        fn function_visibility() {
            check(
                "pub fn public() {}",
                &expect![[r#"
                    SourceFile
                      pub FunctionDef "public"
                        ParamList
                        Block
                "#]],
            );
        }

        #[test]
        fn struct_empty_braces() {
            check(
                "struct Empty {}",
                &expect![[r#"
                    SourceFile
                      StructDef "Empty"
                        FieldList
                "#]],
            );
        }

        /// Unit struct syntax: `struct Empty;`
        #[test]
        fn struct_unit() {
            check(
                "struct Empty;",
                &expect![[r#"
                    SourceFile
                      StructDef "Empty"
                "#]],
            );
        }

        #[test]
        fn struct_with_fields() {
            check(
                "struct Point { x: i32, y: i32 }",
                &expect![[r#"
                    SourceFile
                      StructDef "Point"
                        FieldList
                          FieldDef "x"
                            Path "i32"
                          FieldDef "y"
                            Path "i32"
                "#]],
            );
        }

        /// Tuple struct syntax: `struct Pair(i32, i32);`
        #[test]
        fn struct_tuple() {
            check(
                "struct Pair(i32, i32);",
                &expect![[r#"
                    SourceFile
                      StructDef "Pair"
                        FieldList
                          FieldDef "0"
                            Path "i32"
                          FieldDef "1"
                            Path "i32"
                "#]],
            );
        }

        #[test]
        fn struct_generic() {
            check(
                "struct Box<T> { value: T }",
                &expect![[r#"
                    SourceFile
                      StructDef "Box"
                        GenericParams
                          GenericParam "T"
                        FieldList
                          FieldDef "value"
                            Path "T"
                "#]],
            );
        }

        #[test]
        fn impl_block() {
            check(
                "impl Foo { fn bar() {} }",
                &expect![[r#"
                    SourceFile
                      ImplBlock
                        SelfType
                          Path "Foo"
                        FunctionDef "bar"
                          ParamList
                          Block
                "#]],
            );
        }

        #[test]
        fn impl_with_self() {
            check(
                "impl Foo { fn method(&self) {} }",
                &expect![[r#"
                    SourceFile
                      ImplBlock
                        SelfType
                          Path "Foo"
                        FunctionDef "method"
                          ParamList
                            SelfParam "&self"
                          Block
                "#]],
            );
        }

        #[test]
        fn type_alias() {
            check(
                "type Int = i32;",
                &expect![[r#"
                    SourceFile
                      TypeAlias "Int"
                        Path "i32"
                "#]],
            );
        }

        #[test]
        fn type_alias_with_generic_target() {
            check(
                "type OptInt = Option<i32>;",
                &expect![[r#"
                    SourceFile
                      TypeAlias "OptInt"
                        Path "Option"
                          GenericArgs
                            Path "i32"
                "#]],
            );
        }

        /// Generic type alias: `type Result<T> = Option<T>;`
        #[test]
        fn type_alias_generic() {
            check(
                "type Result<T> = Option<T>;",
                &expect![[r#"
                    SourceFile
                      TypeAlias "Result"
                        GenericParams
                          GenericParam "T"
                        Path "Option"
                          GenericArgs
                            Path "T"
                "#]],
            );
        }
    }

    mod statements {
        use super::*;

        #[test]
        fn let_simple() {
            check(
                "fn main() { let x = 1; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            IdentPat "x"
                            Initializer
                              Literal 1
                "#]],
            );
        }

        #[test]
        fn let_with_type() {
            check(
                "fn main() { let x: i32 = 1; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            IdentPat "x"
                            TypeAnnotation
                              Path "i32"
                            Initializer
                              Literal 1
                "#]],
            );
        }

        #[test]
        fn let_mut() {
            check(
                "fn main() { let mut x = 1; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt mut
                            IdentPat "x"
                            Initializer
                              Literal 1
                "#]],
            );
        }

        #[test]
        fn let_pattern() {
            check(
                "fn main() { let (a, b) = pair; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            TuplePat
                              IdentPat "a"
                              IdentPat "b"
                            Initializer
                              Path "pair"
                "#]],
            );
        }

        #[test]
        fn expr_stmt() {
            check(
                "fn main() { foo(); }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          ExprStmt
                            CallExpr
                              Callee
                                Path "foo"
                              ArgList
                "#]],
            );
        }

        #[test]
        fn expr_stmt_no_semi() {
            check(
                "fn main() { x }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            Path "x"
                "#]],
            );
        }
    }

    mod expressions {
        use super::*;

        #[test]
        fn literal_int() {
            check(
                "fn main() { 42 }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            Literal 42
                "#]],
            );
        }

        #[test]
        fn literal_float() {
            check(
                "fn main() { 3.14 }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            Literal 3.14
                "#]],
            );
        }

        #[test]
        fn literal_string() {
            check(
                r#"fn main() { "hello" }"#,
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            Literal "hello"
                "#]],
            );
        }

        #[test]
        fn literal_char() {
            check(
                "fn main() { 'a' }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            Literal 'a'
                "#]],
            );
        }

        #[test]
        fn literal_bool() {
            check(
                "fn main() { true }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            Literal true
                "#]],
            );
        }

        #[test]
        fn path_simple() {
            check(
                "fn main() { foo }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            Path "foo"
                "#]],
            );
        }

        #[test]
        fn path_qualified() {
            check(
                "fn main() { foo::bar::baz }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            Path "foo::bar::baz"
                "#]],
            );
        }

        #[test]
        fn paren_expr() {
            check(
                "fn main() { (1 + 2) }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            ParenExpr
                              BinaryExpr "+"
                                Literal 1
                                Literal 2
                "#]],
            );
        }

        #[test]
        fn tuple_expr() {
            check(
                "fn main() { (1, 2, 3) }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            TupleExpr
                              Literal 1
                              Literal 2
                              Literal 3
                "#]],
            );
        }

        #[test]
        fn array_expr() {
            check(
                "fn main() { [1, 2, 3] }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            ArrayExpr
                              Literal 1
                              Literal 2
                              Literal 3
                "#]],
            );
        }

        #[test]
        fn array_repeat_expr() {
            check(
                "fn main() { [0; 10] }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            ArrayRepeatExpr
                              Literal 0
                              Literal 10
                "#]],
            );
        }

        #[test]
        fn struct_expr() {
            check(
                "fn main() { Point { x: 1, y: 2 } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            StructExpr "Point"
                              Field "x"
                                Literal 1
                              Field "y"
                                Literal 2
                "#]],
            );
        }

        #[test]
        fn struct_expr_update() {
            check(
                "fn main() { Point { x: 1, ..other } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            StructExpr "Point"
                              Field "x"
                                Literal 1
                              UpdateBase
                                Path "other"
                "#]],
            );
        }

        #[test]
        fn binary_arithmetic() {
            check(
                "fn main() { 1 + 2 * 3 }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            BinaryExpr "+"
                              Literal 1
                              BinaryExpr "*"
                                Literal 2
                                Literal 3
                "#]],
            );
        }

        #[test]
        fn binary_comparison() {
            check(
                "fn main() { a < b }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            BinaryExpr "<"
                              Path "a"
                              Path "b"
                "#]],
            );
        }

        #[test]
        fn binary_logical() {
            check(
                "fn main() { a && b || c }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            BinaryExpr "||"
                              BinaryExpr "&&"
                                Path "a"
                                Path "b"
                              Path "c"
                "#]],
            );
        }

        #[test]
        fn prefix_neg() {
            check(
                "fn main() { -x }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            PrefixExpr "-"
                              Path "x"
                "#]],
            );
        }

        #[test]
        fn prefix_not() {
            check(
                "fn main() { !x }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            PrefixExpr "!"
                              Path "x"
                "#]],
            );
        }

        #[test]
        fn prefix_deref() {
            check(
                "fn main() { *x }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            PrefixExpr "*"
                              Path "x"
                "#]],
            );
        }

        #[test]
        fn ref_expr() {
            check(
                "fn main() { &x }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            RefExpr "&"
                              Path "x"
                "#]],
            );
        }

        #[test]
        fn ref_mut_expr() {
            check(
                "fn main() { &mut x }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            RefExpr "&mut "
                              Path "x"
                "#]],
            );
        }

        #[test]
        fn field_access() {
            check(
                "fn main() { point.x }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            FieldExpr ".x"
                              Path "point"
                "#]],
            );
        }

        #[test]
        fn tuple_field_access() {
            check(
                "fn main() { tuple.0 }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            FieldExpr ".0"
                              Path "tuple"
                "#]],
            );
        }

        #[test]
        fn method_call() {
            check(
                "fn main() { x.foo() }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            MethodCallExpr ".foo()"
                              Receiver
                                Path "x"
                              ArgList
                "#]],
            );
        }

        #[test]
        fn method_call_with_args() {
            check(
                "fn main() { x.foo(1, 2) }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            MethodCallExpr ".foo()"
                              Receiver
                                Path "x"
                              ArgList
                                Literal 1
                                Literal 2
                "#]],
            );
        }

        #[test]
        fn call_expr() {
            check(
                "fn main() { foo(1, 2) }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            CallExpr
                              Callee
                                Path "foo"
                              ArgList
                                Literal 1
                                Literal 2
                "#]],
            );
        }

        #[test]
        fn index_expr() {
            check(
                "fn main() { arr[0] }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            IndexExpr
                              Base
                                Path "arr"
                              Index
                                Literal 0
                "#]],
            );
        }

        #[test]
        fn if_expr() {
            check(
                "fn main() { if x { 1 } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            IfExpr
                              Condition
                                Path "x"
                              Then
                                Block
                                  TailExpr
                                    Literal 1
                "#]],
            );
        }

        #[test]
        fn if_else() {
            check(
                "fn main() { if x { 1 } else { 2 } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            IfExpr
                              Condition
                                Path "x"
                              Then
                                Block
                                  TailExpr
                                    Literal 1
                              Else
                                Block
                                  TailExpr
                                    Literal 2
                "#]],
            );
        }

        #[test]
        fn if_else_if() {
            check(
                "fn main() { if x { 1 } else if y { 2 } else { 3 } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            IfExpr
                              Condition
                                Path "x"
                              Then
                                Block
                                  TailExpr
                                    Literal 1
                              Else
                                IfExpr
                                  Condition
                                    Path "y"
                                  Then
                                    Block
                                      TailExpr
                                        Literal 2
                                  Else
                                    Block
                                      TailExpr
                                        Literal 3
                "#]],
            );
        }

        #[test]
        fn while_expr() {
            check(
                "fn main() { while x { y; } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            WhileExpr
                              Condition
                                Path "x"
                              Block
                                ExprStmt
                                  Path "y"
                "#]],
            );
        }

        #[test]
        fn for_expr() {
            check(
                "fn main() { for i in items { i; } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            ForExpr
                              Pattern
                                IdentPat "i"
                              Iterable
                                Path "items"
                              Block
                                ExprStmt
                                  Path "i"
                "#]],
            );
        }

        #[test]
        fn loop_expr() {
            check(
                "fn main() { loop { break; } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            LoopExpr
                              Block
                                ExprStmt
                                  BreakExpr
                "#]],
            );
        }

        #[test]
        fn break_expr() {
            check(
                "fn main() { loop { break; } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            LoopExpr
                              Block
                                ExprStmt
                                  BreakExpr
                "#]],
            );
        }

        #[test]
        fn break_with_value() {
            check(
                "fn main() { loop { break 42; } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            LoopExpr
                              Block
                                ExprStmt
                                  BreakExpr
                                    Literal 42
                "#]],
            );
        }

        #[test]
        fn continue_expr() {
            check(
                "fn main() { loop { continue; } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            LoopExpr
                              Block
                                ExprStmt
                                  ContinueExpr
                "#]],
            );
        }

        #[test]
        fn return_expr() {
            check(
                "fn main() { return 42; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          ExprStmt
                            ReturnExpr
                              Literal 42
                "#]],
            );
        }

        #[test]
        fn return_void() {
            check(
                "fn main() { return; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          ExprStmt
                            ReturnExpr
                "#]],
            );
        }

        #[test]
        fn block_expr() {
            check(
                "fn main() { { 1 } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            BlockExpr
                              Block
                                TailExpr
                                  Literal 1
                "#]],
            );
        }

        #[test]
        fn block_with_tail() {
            check(
                "fn main() { { let x = 1; x } }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            BlockExpr
                              Block
                                LetStmt
                                  IdentPat "x"
                                  Initializer
                                    Literal 1
                                TailExpr
                                  Path "x"
                "#]],
            );
        }

        #[test]
        fn cast_expr() {
            check(
                "fn main() { x as i64 }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            CastExpr
                              Path "x"
                              TargetType
                                Path "i64"
                "#]],
            );
        }

        /// RangeFull expression: `..`
        #[test]
        fn range_full() {
            check(
                "fn main() { .. }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            RangeFullExpr
                "#]],
            );
        }

        #[test]
        fn range_from() {
            check(
                "fn main() { 1.. }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            RangeFromExpr
                              Start
                                Literal 1
                "#]],
            );
        }

        /// RangeTo expression: `..10`
        #[test]
        fn range_to() {
            check(
                "fn main() { ..10 }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            RangeToExpr
                              End
                                Literal 10
                "#]],
            );
        }

        #[test]
        fn range_expr() {
            check(
                "fn main() { 1..10 }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            RangeExpr
                              Start
                                Literal 1
                              End
                                Literal 10
                "#]],
            );
        }
    }

    mod types {
        use super::*;

        #[test]
        fn type_path() {
            check(
                "fn foo() -> i32 {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                        ReturnType
                          Path "i32"
                        Block
                "#]],
            );
        }

        #[test]
        fn type_path_generic() {
            check(
                "fn foo() -> Vec<i32> {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                        ReturnType
                          Path "Vec"
                            GenericArgs
                              Path "i32"
                        Block
                "#]],
            );
        }

        #[test]
        fn type_ref() {
            check(
                "fn foo(x: &i32) {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                          Param "x"
                            RefType "&"
                              Path "i32"
                        Block
                "#]],
            );
        }

        #[test]
        fn type_ref_mut() {
            check(
                "fn foo(x: &mut i32) {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                          Param "x"
                            RefType "&mut "
                              Path "i32"
                        Block
                "#]],
            );
        }

        #[test]
        fn type_array() {
            check(
                "fn foo(x: [i32; 5]) {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                          Param "x"
                            ArrayType
                              ElementType
                                Path "i32"
                              Length
                                Literal 5
                        Block
                "#]],
            );
        }

        #[test]
        fn type_slice() {
            check(
                "fn foo(x: &[i32]) {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                          Param "x"
                            RefType "&"
                              SliceType
                                Path "i32"
                        Block
                "#]],
            );
        }

        #[test]
        fn type_tuple() {
            check(
                "fn foo(x: (i32, bool)) {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                          Param "x"
                            TupleType
                              Path "i32"
                              Path "bool"
                        Block
                "#]],
            );
        }

        #[test]
        fn type_fn_ptr() {
            check(
                "fn foo(f: fn(i32) -> bool) {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                          Param "f"
                            FnPtrType
                              Params
                                Path "i32"
                              ReturnType
                                Path "bool"
                        Block
                "#]],
            );
        }

        #[test]
        fn type_never() {
            check(
                "fn foo() -> ! {}",
                &expect![[r#"
                    SourceFile
                      FunctionDef "foo"
                        ParamList
                        ReturnType
                          NeverType "!"
                        Block
                "#]],
            );
        }
    }

    mod patterns {
        use super::*;

        #[test]
        fn pat_ident() {
            check(
                "fn main() { let x = 1; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            IdentPat "x"
                            Initializer
                              Literal 1
                "#]],
            );
        }

        #[test]
        fn pat_ident_mut() {
            check(
                "fn main() { let mut x = 1; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt mut
                            IdentPat "x"
                            Initializer
                              Literal 1
                "#]],
            );
        }

        #[test]
        fn pat_wildcard() {
            check(
                "fn main() { let _ = 1; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            WildcardPat "_"
                            Initializer
                              Literal 1
                "#]],
            );
        }

        #[test]
        fn pat_tuple() {
            check(
                "fn main() { let (a, b) = pair; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            TuplePat
                              IdentPat "a"
                              IdentPat "b"
                            Initializer
                              Path "pair"
                "#]],
            );
        }

        #[test]
        fn pat_struct() {
            check(
                "fn main() { let Point { x, y } = p; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            StructPat "Point"
                              Field "x" (shorthand)
                              Field "y" (shorthand)
                            Initializer
                              Path "p"
                "#]],
            );
        }

        #[test]
        fn pat_struct_with_binding() {
            check(
                "fn main() { let Point { x: a, y: b } = p; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            StructPat "Point"
                              Field "x"
                                IdentPat "a"
                              Field "y"
                                IdentPat "b"
                            Initializer
                              Path "p"
                "#]],
            );
        }

        #[test]
        fn pat_ref() {
            check(
                "fn main() { let &x = r; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            RefPat "&"
                              IdentPat "x"
                            Initializer
                              Path "r"
                "#]],
            );
        }

        #[test]
        fn pat_ref_mut() {
            check(
                "fn main() { let &mut x = r; }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            RefPat "&mut "
                              IdentPat "x"
                            Initializer
                              Path "r"
                "#]],
            );
        }
    }

    mod integration {
        use super::*;

        #[test]
        fn complete_program() {
            check(
                r#"
struct Point { x: i32, y: i32 }

impl Point {
    fn new(x: i32, y: i32) -> Point {
        Point { x: x, y: y }
    }

    fn distance(&self) -> i32 {
        self.x + self.y
    }
}

fn main() {
    let p = Point::new(3, 4);
    p.distance();
}
"#,
                &expect![[r#"
                    SourceFile
                      StructDef "Point"
                        FieldList
                          FieldDef "x"
                            Path "i32"
                          FieldDef "y"
                            Path "i32"
                      ImplBlock
                        SelfType
                          Path "Point"
                        FunctionDef "new"
                          ParamList
                            Param "x"
                              Path "i32"
                            Param "y"
                              Path "i32"
                          ReturnType
                            Path "Point"
                          Block
                            TailExpr
                              StructExpr "Point"
                                Field "x"
                                  Path "x"
                                Field "y"
                                  Path "y"
                        FunctionDef "distance"
                          ParamList
                            SelfParam "&self"
                          ReturnType
                            Path "i32"
                          Block
                            TailExpr
                              BinaryExpr "+"
                                FieldExpr ".x"
                                  Path "self"
                                FieldExpr ".y"
                                  Path "self"
                      FunctionDef "main"
                        ParamList
                        Block
                          LetStmt
                            IdentPat "p"
                            Initializer
                              CallExpr
                                Callee
                                  Path "Point::new"
                                ArgList
                                  Literal 3
                                  Literal 4
                          ExprStmt
                            MethodCallExpr ".distance()"
                              Receiver
                                Path "p"
                              ArgList
                "#]],
            );
        }

        #[test]
        fn nested_expressions() {
            check(
                "fn main() { ((1 + 2) * (3 - 4)) / 5 }",
                &expect![[r#"
                    SourceFile
                      FunctionDef "main"
                        ParamList
                        Block
                          TailExpr
                            BinaryExpr "/"
                              ParenExpr
                                BinaryExpr "*"
                                  ParenExpr
                                    BinaryExpr "+"
                                      Literal 1
                                      Literal 2
                                  ParenExpr
                                    BinaryExpr "-"
                                      Literal 3
                                      Literal 4
                              Literal 5
                "#]],
            );
        }

        #[test]
        fn complex_function() {
            check(
                r#"
fn process<T>(items: &[T], filter: fn(T) -> bool) -> i32 {
    let mut count = 0;
    for item in items {
        if filter(item) {
            count = count + 1;
        }
    }
    count
}
"#,
                &expect![[r#"
                    SourceFile
                      FunctionDef "process"
                        GenericParams
                          GenericParam "T"
                        ParamList
                          Param "items"
                            RefType "&"
                              SliceType
                                Path "T"
                          Param "filter"
                            FnPtrType
                              Params
                                Path "T"
                              ReturnType
                                Path "bool"
                        ReturnType
                          Path "i32"
                        Block
                          LetStmt mut
                            IdentPat "count"
                            Initializer
                              Literal 0
                          ExprStmt (no semi)
                            ForExpr
                              Pattern
                                IdentPat "item"
                              Iterable
                                Path "items"
                              Block
                                TailExpr
                                  IfExpr
                                    Condition
                                      CallExpr
                                        Callee
                                          Path "filter"
                                        ArgList
                                          Path "item"
                                    Then
                                      Block
                                        ExprStmt
                                          BinaryExpr "="
                                            Path "count"
                                            BinaryExpr "+"
                                              Path "count"
                                              Literal 1
                          TailExpr
                            Path "count"
                "#]],
            );
        }
    }
}
