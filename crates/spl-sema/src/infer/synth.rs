//! Expression type synthesis and checking.

use crate::folding::{LoweredExpr, try_lower_expr};
use crate::types::{InferKind, Mutability, PrimitiveKind, Type, TypeId};
use crate::{SymbolKind, Visibility};
use rowan::ast::AstNode;
use rustc_hash::FxHashMap;
use spl_ast::{
    ArrayExpr, BinExpr, BlockExpr, BreakExpr, CallExpr, ContinueExpr, Expr, FieldExpr, ForExpr,
    IfExpr, IndexExpr, IsExpr, LoopExpr, MatchExpr, ParenExpr, Pat, PathExpr, PrefixExpr,
    RangeExpr, RefExpr, ReturnExpr, SliceExpr, TupleExpr, WhileExpr, YieldExpr,
};
use spl_ast::{Block, LetStmt, LiteralExpr, Stmt};
use spl_diagnostic::Diagnostic;
use spl_syntax::SyntaxKind;

use tracing::{debug, trace};

use super::UnifyError;
use super::engine::{InferEngine, LoopKind};
use super::helpers::{
    find_similar, is_numeric_type, parse_int_literal_value, parse_int_suffix, text_range_to_span,
};

impl<'a> InferEngine<'a> {
    // =========================================================================
    // Mutability Checking
    // =========================================================================

    /// Check if an expression is a valid assignment target (a mutable place).
    /// Returns an error message if not assignable, None if OK.
    pub(super) fn check_assignable(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Path(path_expr) => {
                // Look up the path to get the DefId
                let Some(path) = path_expr.path() else {
                    return Some("invalid assignment target".to_string());
                };
                let segments: Vec<_> = path.segments().collect();
                if segments.is_empty() {
                    return Some("invalid assignment target".to_string());
                }

                // Get the first segment
                let first_segment = &segments[0];
                let Some(name_ref) = first_segment.name() else {
                    return Some("invalid assignment target".to_string());
                };
                let Some(token) = name_ref.token() else {
                    return Some("invalid assignment target".to_string());
                };
                let span = text_range_to_span(token.text_range());

                if let Some(&def_id) = self.resolutions.get(&span) {
                    let symbol = self.resolve_ctx.get_symbol(def_id);

                    // For single-segment paths, check if the variable is mutable
                    if segments.len() == 1 {
                        if !symbol.is_mutable {
                            let name = self.resolve_ctx.resolve(symbol.name);
                            return Some(format!("cannot assign to immutable variable `{name}`"));
                        }
                        return None;
                    }

                    // Multi-segment path (like self.a) - treat as field assignment
                    // Check if base is mutable or is a mutable reference
                    if let Some(&base_ty) = self.results.binding_types.get(&def_id) {
                        let resolved = self.resolve_type(base_ty);
                        let ty = self.types.get(resolved);
                        if let Type::Ref(mutability, _) = ty {
                            return if *mutability == Mutability::Mutable {
                                None // OK - mutable reference
                            } else {
                                Some("cannot assign to field of immutable reference".to_string())
                            };
                        }
                    }
                    // Not a reference - check if the base variable is mutable
                    if !symbol.is_mutable {
                        let name = self.resolve_ctx.resolve(symbol.name);
                        return Some(format!("cannot assign to immutable variable `{name}`"));
                    }
                }
                None
            }
            Expr::Field(field_expr) => {
                // For field assignment (s.a = x), the base must be mutable
                // However, if the base is a mutable reference (&mut T), assignment is allowed
                if let Some(base) = field_expr.expr() {
                    // Check if the base's type is a mutable reference
                    let base_span = text_range_to_span(base.syntax().text_range());
                    if let Some(&base_ty) = self.results.expr_types.get(&base_span) {
                        let resolved = self.resolve_type(base_ty);
                        let ty = self.types.get(resolved);
                        if let Type::Ref(mutability, _) = ty {
                            return if *mutability == Mutability::Mutable {
                                None // OK - mutable reference
                            } else {
                                Some("cannot assign to field of immutable reference".to_string())
                            };
                        }
                    }
                    // Not a reference - check if the base itself is mutable
                    self.check_assignable(&base)
                } else {
                    None
                }
            }
            Expr::Prefix(prefix_expr) => {
                // For deref assignment (*r = x), check the reference is mutable
                if let Some(op) = prefix_expr.op_token()
                    && op.kind() == SyntaxKind::STAR
                    && let Some(inner) = prefix_expr.expr()
                {
                    let inner_ty = self
                        .results
                        .expr_types
                        .get(&text_range_to_span(inner.syntax().text_range()))?;
                    let resolved = self.resolve_type(*inner_ty);
                    let ty = self.types.get(resolved);
                    if let Type::Ref(Mutability::Shared, _) = ty {
                        return Some("cannot assign to immutable reference".to_string());
                    }
                }
                None
            }
            Expr::Index(index_expr) => {
                // For index assignment (arr[i] = x), the base must be mutable
                if let Some(base) = index_expr.base() {
                    self.check_assignable(&base)
                } else {
                    None
                }
            }
            _ => Some("invalid assignment target".to_string()),
        }
    }

    /// Check if we can take a mutable borrow of an expression.
    /// Returns an error message if not borrowable as mutable, None if OK.
    pub(super) fn check_mutable_borrow(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Path(path_expr) => {
                // Look up the path to get the DefId
                let Some(path) = path_expr.path() else {
                    return Some("cannot take mutable reference of a temporary value".to_string());
                };
                let segments: Vec<_> = path.segments().collect();
                if segments.is_empty() {
                    return Some("cannot take mutable reference of a temporary value".to_string());
                }

                let first_segment = &segments[0];
                let Some(name_ref) = first_segment.name() else {
                    return Some("cannot take mutable reference of a temporary value".to_string());
                };
                let Some(token) = name_ref.token() else {
                    return Some("cannot take mutable reference of a temporary value".to_string());
                };
                let span = text_range_to_span(token.text_range());

                if let Some(&def_id) = self.resolutions.get(&span) {
                    let symbol = self.resolve_ctx.get_symbol(def_id);

                    // For single-segment paths, check if variable is mutable
                    if segments.len() == 1 {
                        if !symbol.is_mutable {
                            let name = self.resolve_ctx.resolve(symbol.name);
                            return Some(format!(
                                "cannot borrow `{name}` as mutable, as it is not declared as mutable"
                            ));
                        }
                        return None;
                    }

                    // Multi-segment path (like self.a) - treat as field borrow
                    // Check if base is mutable or is a mutable reference
                    if let Some(&base_ty) = self.results.binding_types.get(&def_id) {
                        let resolved = self.resolve_type(base_ty);
                        let ty = self.types.get(resolved);
                        if let Type::Ref(mutability, _) = ty {
                            return if *mutability == Mutability::Mutable {
                                None // OK - mutable reference
                            } else {
                                Some(
                                    "cannot borrow field of immutable reference as mutable"
                                        .to_string(),
                                )
                            };
                        }
                    }
                    // Not a reference - check if the base variable is mutable
                    if !symbol.is_mutable {
                        let name = self.resolve_ctx.resolve(symbol.name);
                        return Some(format!(
                            "cannot borrow `{name}` as mutable, as it is not declared as mutable"
                        ));
                    }
                }
                None
            }
            Expr::Field(field_expr) => {
                // For &mut s.a, the base must be mutable
                // However, if the base is a mutable reference (&mut T), borrowing is allowed
                if let Some(base) = field_expr.expr() {
                    // Check if the base's type is a mutable reference
                    let base_span = text_range_to_span(base.syntax().text_range());
                    if let Some(&base_ty) = self.results.expr_types.get(&base_span) {
                        let resolved = self.resolve_type(base_ty);
                        let ty = self.types.get(resolved);
                        if let Type::Ref(mutability, _) = ty {
                            return if *mutability == Mutability::Mutable {
                                None // OK - mutable reference
                            } else {
                                Some(
                                    "cannot borrow field of immutable reference as mutable"
                                        .to_string(),
                                )
                            };
                        }
                    }
                    // Not a reference - check if the base itself is mutable
                    self.check_mutable_borrow(&base)
                } else {
                    None
                }
            }
            Expr::Prefix(prefix_expr) => {
                // For &mut *r, the deref target must be mutable
                if let Some(op) = prefix_expr.op_token()
                    && op.kind() == SyntaxKind::STAR
                    && let Some(inner) = prefix_expr.expr()
                {
                    // Check if the reference being dereferenced is mutable
                    let inner_span = text_range_to_span(inner.syntax().text_range());
                    if let Some(&inner_ty) = self.results.expr_types.get(&inner_span) {
                        let resolved = self.resolve_type(inner_ty);
                        let ty = self.types.get(resolved);
                        if let Type::Ref(Mutability::Shared, _) = ty {
                            return Some(
                                "cannot borrow through shared reference as mutable".to_string(),
                            );
                        }
                    }
                }
                None
            }
            Expr::Index(index_expr) => {
                // For &mut arr[i], the base must be mutable
                if let Some(base) = index_expr.base() {
                    self.check_mutable_borrow(&base)
                } else {
                    None
                }
            }
            _ => Some("cannot take mutable reference of a temporary value".to_string()),
        }
    }

    // =========================================================================
    // Type Synthesis (Bottom-up)
    // =========================================================================

    /// Synthesize the type of an expression.
    pub(super) fn synth_expr(&mut self, expr: &Expr) -> TypeId {
        let span = text_range_to_span(expr.syntax().text_range());

        // Try lowering for negated literals first
        let (lowered, was_lowered) = try_lower_expr(expr);
        if was_lowered {
            let type_id = match lowered {
                LoweredExpr::IntLiteral {
                    value,
                    suffix,
                    span: lit_span,
                } => self.synth_lowered_int(value, suffix, lit_span),
                LoweredExpr::FloatLiteral {
                    value: _,
                    suffix,
                    span: _,
                } => self.synth_lowered_float(suffix),
                LoweredExpr::BoolLiteral { .. } => self.types.bool(),
                LoweredExpr::Passthrough => {
                    unreachable!("Passthrough should not appear in binary expression")
                }
            };
            self.results.expr_types.insert(span, type_id);
            return type_id;
        }

        let expr_kind = match expr {
            Expr::Literal(_) => "literal",
            Expr::Path(_) => "path",
            Expr::Paren(_) => "paren",
            Expr::Tuple(_) => "tuple",
            Expr::Array(_) => "array",
            Expr::Call(_) => "call",
            Expr::Binary(_) => "binary",
            Expr::Prefix(_) => "prefix",
            Expr::Ref(_) => "ref",
            Expr::Field(_) => "field",
            Expr::Index(_) => "index",
            Expr::Slice(_) => "slice",
            Expr::If(_) => "if",
            Expr::While(_) => "while",
            Expr::For(_) => "for",
            Expr::Loop(_) => "loop",
            Expr::Break(_) => "break",
            Expr::Continue(_) => "continue",
            Expr::Return(_) => "return",
            Expr::Yield(_) => "yield",
            Expr::Block(_) => "block",
            Expr::Range(_) => "range",
            Expr::Is(_) => "is",
            Expr::Match(_) => "match",
            Expr::EnumShorthand(_) => "enum_shorthand",
            Expr::Try(_) => "try",
            Expr::Closure(_) => "closure",
            Expr::OptionalField(_) => "optional_field",
            Expr::Dollar(_) => "dollar",
            Expr::Unsafe(_) => "unsafe",
            Expr::Throw(_) => "throw",
        };
        trace!(expr_kind, "synthesizing expression");

        let type_id = match expr {
            Expr::Literal(lit) => self.synth_literal(lit),
            Expr::Path(path_expr) => self.synth_path(path_expr),
            Expr::Paren(paren) => self.synth_paren(paren),
            Expr::Tuple(tuple) => self.synth_tuple(tuple),
            Expr::Array(array) => self.synth_array(array),
            Expr::Call(call) => self.synth_call(call),
            Expr::Binary(bin) => self.synth_binary(bin),
            Expr::Prefix(prefix) => self.synth_prefix(prefix),
            Expr::Ref(ref_expr) => self.synth_ref(ref_expr),
            Expr::Field(field) => self.synth_field(field),
            Expr::Index(index) => self.synth_index(index),
            Expr::Slice(slice) => self.synth_slice(slice),
            Expr::If(if_expr) => self.synth_if(if_expr),
            Expr::While(while_expr) => self.synth_while(while_expr),
            Expr::For(for_expr) => self.synth_for(for_expr),
            Expr::Loop(loop_expr) => self.synth_loop(loop_expr),
            Expr::Break(break_expr) => self.synth_break(break_expr),
            Expr::Continue(continue_expr) => self.synth_continue(continue_expr),
            Expr::Return(return_expr) => self.synth_return(return_expr),
            // TODO: yield expression type inference requires block context tracking
            Expr::Yield(yield_expr) => self.synth_yield(yield_expr),
            Expr::Block(block_expr) => self.synth_block_expr(block_expr),

            Expr::Range(range) => self.synth_range(range),
            Expr::Is(is_expr) => self.synth_is(is_expr),
            Expr::Match(match_expr) => self.synth_match(match_expr),
            // Enum shorthand: .Variant or .Variant(args)
            // TODO: Implement full type inference - requires expected type context
            Expr::EnumShorthand(shorthand) => {
                // For now, synthesize arguments to ensure they're type-checked
                for arg in shorthand.args() {
                    if let Some(value) = arg.value() {
                        self.synth_expr(&value);
                    }
                }
                // Return error - full inference requires expected type context
                self.types.error()
            }
            // Try/propagate: expr!
            // TODO: Implement proper Result unwrapping inference
            Expr::Try(try_expr) => {
                // Synthesize the inner expression
                if let Some(inner) = try_expr.expr() {
                    self.synth_expr(&inner);
                }
                // For now, return error - proper inference requires Result type handling
                self.types.error()
            }
            // Closure expression: |params| body or @[captures] |params| body
            // TODO: Implement proper closure type inference
            Expr::Closure(closure_expr) => {
                // Synthesize capture expressions
                if let Some(captures) = closure_expr.capture_list() {
                    for capture in captures.captures() {
                        if let Some(expr) = capture.expr() {
                            self.synth_expr(&expr);
                        }
                    }
                }
                // Synthesize the body
                if let Some(body) = closure_expr.body() {
                    self.synth_expr(&body);
                }
                // For now, return error - proper closure type requires FnOnce/FnMut/Fn trait handling
                self.types.error()
            }
            // Optional field access: expr?.field
            // TODO: Implement proper Option unwrapping inference
            Expr::OptionalField(optional_field) => {
                // Synthesize the base expression
                if let Some(base) = optional_field.expr() {
                    self.synth_expr(&base);
                }
                // For now, return error - proper inference requires Option type handling
                self.types.error()
            }
            // Dollar expression: $ represents array length in index contexts
            // This needs context to know what array it refers to
            // TODO: Implement proper $ inference in index expressions
            Expr::Dollar(_) => {
                // $ is usize (array length), but needs context to validate
                self.types.primitive(PrimitiveKind::Usize)
            }
            // Unsafe expression: unsafe { ... }
            // The type is the type of the block's final expression
            Expr::Unsafe(unsafe_expr) => {
                if let Some(block) = unsafe_expr.block() {
                    self.synth_block(&block)
                } else {
                    self.types.unit()
                }
            }
            // Throw expression: throw expr
            // Throw always diverges (never type), similar to return/break
            Expr::Throw(throw_expr) => {
                // Synthesize the thrown expression to type-check it
                if let Some(inner) = throw_expr.expr() {
                    self.synth_expr(&inner);
                }
                // throw diverges - returns Never type
                self.types.never()
            }
        };
        self.results.expr_types.insert(span, type_id);
        type_id
    }

    fn synth_literal(&mut self, lit: &LiteralExpr) -> TypeId {
        let Some(token) = lit.token() else {
            return self.types.error();
        };

        match token.kind() {
            SyntaxKind::INT_LITERAL => {
                let text = token.text();
                // Check for type suffix and validate range
                let (prim_kind, has_suffix) = parse_int_suffix(text);
                if let Some(kind) = prim_kind {
                    // Has suffix - validate range
                    // For signed types at the negatable boundary (e.g., 128i8), only skip
                    // validation if this literal is the direct operand of a negation prefix.
                    // The prefix handler will validate negated suffixed literals correctly.
                    // Validate range for suffixed integer literals
                    if has_suffix
                        && let Some(value) = parse_int_literal_value(text)
                        && let Err(msg) = kind.validate_int_literal_range(value)
                    {
                        let span = text_range_to_span(token.text_range());
                        self.diagnostics
                            .push(Diagnostic::error(&msg).with_label(span, "literal out of range"));
                    }
                    self.types.primitive(kind)
                } else {
                    // No suffix - create an int inference variable
                    self.fresh_int_var()
                }
            }
            SyntaxKind::FLOAT_LITERAL => {
                let text = token.text();
                if text.ends_with("f32") {
                    self.types.primitive(PrimitiveKind::F32)
                } else if text.ends_with("f64") {
                    self.types.primitive(PrimitiveKind::F64)
                } else {
                    // No suffix - create a float inference variable
                    self.fresh_float_var()
                }
            }
            SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW => self.types.bool(),
            SyntaxKind::CHAR_LITERAL => self.types.char(),
            SyntaxKind::STRING_LITERAL => self.types.str_ref(),
            _ => self.types.error(),
        }
    }

    /// Synthesize type for a lowered integer literal (from HIR lowering).
    fn synth_lowered_int(
        &mut self,
        value: i128,
        suffix: Option<PrimitiveKind>,
        span: spl_lexer::Span,
    ) -> TypeId {
        if let Some(kind) = suffix {
            if let Err(msg) = kind.validate_int_literal_range(value) {
                self.diagnostics
                    .push(Diagnostic::error(&msg).with_label(span, "literal out of range"));
            }
            self.types.primitive(kind)
        } else {
            self.fresh_int_var()
        }
    }

    /// Synthesize type for a lowered float literal (from HIR lowering).
    fn synth_lowered_float(&mut self, suffix: Option<PrimitiveKind>) -> TypeId {
        match suffix {
            Some(PrimitiveKind::F32) => self.types.primitive(PrimitiveKind::F32),
            Some(PrimitiveKind::F64) => self.types.primitive(PrimitiveKind::F64),
            _ => self.fresh_float_var(),
        }
    }

    fn synth_path(&mut self, path_expr: &PathExpr) -> TypeId {
        let Some(path) = path_expr.path() else {
            return self.types.error();
        };

        let segments: Vec<_> = path.segments().collect();
        if segments.is_empty() {
            return self.types.error();
        }

        // Get the type of the first segment (base variable)
        let first_segment = &segments[0];
        let Some(name_ref) = first_segment.name() else {
            return self.types.error();
        };

        // Use token() instead of ident_token() to handle `self` keyword
        let Some(token) = name_ref.token() else {
            return self.types.error();
        };

        let span = text_range_to_span(token.text_range());

        // Look up the resolved DefId
        let def_id = if let Some(id) = self.resolutions.get(&span) {
            *id
        } else {
            debug!("synth_path: no resolution for span");
            return self.types.error();
        };

        debug!(
            def_id = def_id.index(),
            segment_count = segments.len(),
            "synthesizing path"
        );

        // Check if this is a module - if so, return Type::Module
        let symbol = self.resolve_ctx.get_symbol(def_id);
        if symbol.kind == SymbolKind::Module {
            let mut current_type = self.types.mk_module(def_id);

            // If there's only one segment, we're done (just the module name)
            if segments.len() == 1 {
                return current_type;
            }

            // Multi-segment path through module: treat as field accesses
            for segment in segments.iter().skip(1) {
                let field_name = match segment.name() {
                    Some(n) => match n.token() {
                        Some(t) => t.text().to_string(),
                        None => return self.types.error(),
                    },
                    None => return self.types.error(),
                };

                current_type = self.synth_field_access(current_type, &field_name, segment);
            }

            return current_type;
        }

        // Get the type of the first segment (for non-module paths)
        let mut current_type = if let Some(&type_id) = self.results.binding_types.get(&def_id) {
            type_id
        } else if let Some(sig) = self.defs.fn_signatures.get(&def_id).cloned() {
            // It's a function - for multi-segment paths this might be a qualified function call
            let (param_types, ret_ty) = self.instantiate_signature(&sig);
            self.types.mk_fn_ptr(param_types, ret_ty)
        } else {
            return self.types.error();
        };

        // If there's only one segment, we're done
        if segments.len() == 1 {
            return current_type;
        }

        // Multi-segment path: treat as field accesses
        for segment in segments.iter().skip(1) {
            let field_name = match segment.name() {
                Some(n) => match n.token() {
                    Some(t) => t.text().to_string(),
                    None => return self.types.error(),
                },
                None => return self.types.error(),
            };

            // Look up the field in the current type
            current_type = self.synth_field_access(current_type, &field_name, segment);
        }

        current_type
    }

    /// Synthesize the type of a field access on a given base type.
    fn synth_field_access(
        &mut self,
        base_type: TypeId,
        field_name: &str,
        segment: &spl_ast::PathSegment,
    ) -> TypeId {
        // Resolve and auto-deref references for field access
        let resolved = self.resolve_type(base_type);
        let mut base_type_val = self.types.get(resolved).clone();

        const MAX_DEREF: usize = 100;
        #[cfg(debug_assertions)]
        let mut deref_count = 0;

        while let Type::Ref(_, inner) = &base_type_val {
            #[cfg(debug_assertions)]
            {
                deref_count += 1;
                debug_assert!(
                    deref_count < MAX_DEREF,
                    "invariant: auto-deref must terminate (hit {MAX_DEREF} derefs)"
                );
            }
            let inner_resolved = self.resolve_type(*inner);
            base_type_val = self.types.get(inner_resolved).clone();
        }

        // Handle module item access (e.g., module.Item)
        if let Type::Module(module_def_id) = &base_type_val {
            let module_def_id = *module_def_id;

            // Look up the field name in the module's scope
            if let Some(&scope_id) = self.module_scopes.get(&module_def_id) {
                let Some(interned) = self.resolve_ctx.try_get_interned(field_name) else {
                    // Name wasn't interned, so it definitely doesn't exist in the module
                    let span = text_range_to_span(segment.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("cannot find `{field_name}` in module"))
                            .with_label(span, "not found in module"),
                    );
                    return self.types.error();
                };
                if let Some(item_def_id) = self.resolve_ctx.lookup_in_scope(interned, scope_id) {
                    let item_symbol = self.resolve_ctx.get_symbol(item_def_id);

                    // Check visibility
                    if item_symbol.visibility == Visibility::Private {
                        let span = text_range_to_span(segment.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "`{field_name}` is private and not accessible from this module"
                            ))
                            .with_label(span, "private item, not accessible"),
                        );
                        return self.types.error();
                    }

                    // Store the resolution for this path segment
                    let seg_span = text_range_to_span(segment.syntax().text_range());
                    self.resolutions.insert(seg_span, item_def_id);

                    // Determine what kind of item it is and return appropriate type
                    match item_symbol.kind {
                        SymbolKind::Module => {
                            // Nested module access (outer.inner)
                            return self.types.mk_module(item_def_id);
                        }
                        SymbolKind::Struct => {
                            // Struct access for construction (types.Point)
                            return self.types.mk_struct(item_def_id, vec![]);
                        }
                        SymbolKind::Function => {
                            // Function reference - return function pointer type
                            if let Some(sig) = self.defs.fn_signatures.get(&item_def_id).cloned() {
                                let (param_types, ret_ty) = self.instantiate_signature(&sig);
                                return self.types.mk_fn_ptr(param_types, ret_ty);
                            }
                            return self.types.error();
                        }
                        SymbolKind::TypeAlias => {
                            // Type alias - similar to struct
                            return self.types.mk_alias(item_def_id, vec![]);
                        }
                        _ => {
                            // Other kinds (Local, Param, Field, TypeParam) shouldn't be in module scope
                            let span = text_range_to_span(segment.syntax().text_range());
                            self.diagnostics.push(
                                Diagnostic::error(format!(
                                    "`{field_name}` is not a valid item in module"
                                ))
                                .with_label(span, "unexpected item kind"),
                            );
                            return self.types.error();
                        }
                    }
                }

                // Item not found in module
                let span = text_range_to_span(segment.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error(format!("cannot find `{field_name}` in module"))
                        .with_label(span, "not found in module"),
                );
                return self.types.error();
            }

            // Module scope not found (shouldn't happen for valid modules)
            let span = text_range_to_span(segment.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error("internal error: module scope not found")
                    .with_label(span, "module scope missing"),
            );
            return self.types.error();
        }

        // Handle struct field access
        if let Type::Struct(def_id, type_args) = &base_type_val {
            let def_id = *def_id;
            let type_args = type_args.clone();

            // Build substitution map from struct's type params to type args
            let type_params = self
                .defs
                .struct_type_params
                .get(&def_id)
                .cloned()
                .unwrap_or_default();
            let mut subst: FxHashMap<_, _> = FxHashMap::default();
            for (param_def_id, type_arg) in type_params.iter().zip(type_args.iter()) {
                subst.insert(*param_def_id, *type_arg);
            }

            if let Some(fields) = self.defs.struct_fields.get(&def_id).cloned() {
                for (name, ty, field_def_id) in fields {
                    if name == field_name {
                        // Check field visibility
                        if field_def_id.is_valid() {
                            let field_symbol = self.resolve_ctx.get_symbol(field_def_id);
                            if field_symbol.visibility == Visibility::Private {
                                // Private fields: check if accessor is in same module as struct
                                let struct_symbol = self.resolve_ctx.get_symbol(def_id);
                                let current_scope = self.current_inference_scope;
                                let struct_scope = struct_symbol.scope_id;
                                // Check if current scope is NOT the struct's defining scope or a child of it
                                if !self.is_scope_descendant_of(current_scope, struct_scope) {
                                    let span = text_range_to_span(segment.syntax().text_range());
                                    self.diagnostics.push(
                                        Diagnostic::error(format!(
                                            "field `{field_name}` is private"
                                        ))
                                        .with_label(span, "private field"),
                                    );
                                    return self.types.error();
                                }
                            }
                        }
                        // Substitute type parameters in field type
                        let result_ty = self.substitute_type_params(ty, &subst);
                        // Record the type for this field access
                        let seg_span = text_range_to_span(segment.syntax().text_range());
                        self.results.expr_types.insert(seg_span, result_ty);
                        return result_ty;
                    }
                }
            }

            // Field not found
            let span = text_range_to_span(segment.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error(format!("no field `{field_name}` on struct"))
                    .with_label(span, "unknown field"),
            );
            return self.types.error();
        }

        // Not a struct type
        let span = text_range_to_span(segment.syntax().text_range());
        self.diagnostics.push(
            Diagnostic::error("field access on non-struct type").with_label(span, "not a struct"),
        );
        self.types.error()
    }

    fn synth_paren(&mut self, paren: &ParenExpr) -> TypeId {
        match paren.expr() {
            Some(inner) => self.synth_expr(&inner),
            None => self.types.error(),
        }
    }

    fn synth_tuple(&mut self, tuple: &TupleExpr) -> TypeId {
        let elem_types: Vec<TypeId> = tuple.exprs().map(|e| self.synth_expr(&e)).collect();
        self.types.mk_tuple(elem_types)
    }

    fn synth_array(&mut self, array: &ArrayExpr) -> TypeId {
        let exprs: Vec<_> = array.exprs().collect();
        if exprs.is_empty() {
            // Empty array needs type annotation
            let elem = self.fresh_type_var();
            return self.types.mk_array(elem, 0);
        }

        // Check for repeat syntax [elem; count]
        if array.is_repeat() && exprs.len() == 2 {
            // First expression is the element value
            let elem_type = self.synth_expr(&exprs[0]);
            // Second expression is the count - evaluate as constant
            let count = self.eval_const_usize(&exprs[1]).unwrap_or(0);
            let result = self.types.mk_array(elem_type, count as u64);

            debug_assert!(
                matches!(self.types.get(result), Type::Array(_, _)),
                "postcondition: synth_array must return Array type"
            );

            return result;
        }

        // Array literal [a, b, c]
        // Synthesize the first element's type
        let first_type = self.synth_expr(&exprs[0]);

        // Check/unify all elements
        for expr in &exprs[1..] {
            let elem_type = self.synth_expr(expr);
            if let Err(_err) = self.unify(first_type, elem_type) {
                let span = text_range_to_span(expr.syntax().text_range());
                // For array elements, show expected/actual in user-friendly way
                let first_str = self.type_to_string(first_type);
                let elem_str = self.type_to_string(elem_type);
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "type mismatch in array elements: expected `{first_str}`, found `{elem_str}`"
                    ))
                    .with_label(span, "element has incompatible type"),
                );
            }
        }

        let result = self.types.mk_array(first_type, exprs.len() as u64);

        debug_assert!(
            matches!(self.types.get(result), Type::Array(_, _)),
            "postcondition: synth_array must return Array type"
        );

        result
    }

    /// Handle qualified paths like `S.new()` or `instance.method()`
    fn synth_call_qualified_path(
        &mut self,
        call: &CallExpr,
        segments: &[spl_ast::PathSegment],
    ) -> TypeId {
        // Get the first segment
        let first_segment = &segments[0];
        let Some(first_name_ref) = first_segment.name() else {
            return self.types.error();
        };

        let Some(first_token) = first_name_ref.token() else {
            return self.types.error();
        };

        let first_span = text_range_to_span(first_token.text_range());
        let first_def_id = match self.resolutions.get(&first_span) {
            Some(id) => *id,
            None => return self.types.error(),
        };

        // Check if the first segment is a variable (instance method call like `p.distance()`)
        if let Some(&binding_type) = self.results.binding_types.get(&first_def_id) {
            return self.synth_instance_method_call(call, segments, binding_type);
        }

        // Check if the first segment is a module (qualified function call like `module.func()`)
        let first_symbol = self.resolve_ctx.get_symbol(first_def_id);
        if first_symbol.kind == SymbolKind::Module {
            return self.synth_module_qualified_call(call, segments, first_def_id);
        }

        // Otherwise, try to resolve as a type (associated function call like `S.new()`)
        let struct_def_id = if self.defs.struct_fields.contains_key(&first_def_id) {
            first_def_id
        } else if let Some(&target_ty) = self.defs.type_alias_targets.get(&first_def_id) {
            let resolved = self.resolve_type(target_ty);
            if let Type::Struct(actual_def_id, _) = self.types.get(resolved) {
                *actual_def_id
            } else {
                self.diagnostics.push(
                    Diagnostic::error("not a struct type")
                        .with_label(first_span, "expected struct"),
                );
                return self.types.error();
            }
        } else {
            self.diagnostics.push(
                Diagnostic::error("not a struct type").with_label(first_span, "expected struct"),
            );
            return self.types.error();
        };

        // Get the last segment which should be the method name
        let last_segment = &segments[segments.len() - 1];
        let Some(method_name_ref) = last_segment.name() else {
            return self.types.error();
        };

        let Some(method_token) = method_name_ref.token() else {
            return self.types.error();
        };

        let method_name = method_token.text().to_string();

        // Look up the method in the struct's impl
        let method_def_ids = self
            .defs
            .struct_methods
            .get(&struct_def_id)
            .cloned()
            .unwrap_or_default();

        // Collect available method names for error messages
        let available_methods: Vec<&str> = method_def_ids
            .iter()
            .map(|&def_id| {
                let symbol = self.resolve_ctx.get_symbol(def_id);
                self.resolve_ctx.resolve(symbol.name)
            })
            .collect();

        for method_def_id in &method_def_ids {
            let symbol = self.resolve_ctx.get_symbol(*method_def_id);
            let fn_name = self.resolve_ctx.resolve(symbol.name);
            if fn_name == method_name
                && let Some(sig) = self.defs.fn_signatures.get(method_def_id).cloned()
            {
                // Store the resolution for the method
                let method_span = text_range_to_span(method_token.text_range());
                self.results
                    .method_resolutions
                    .insert(method_span, *method_def_id);

                return self.synth_call_as_function(call, &sig);
            }
        }

        // Method not found - provide helpful diagnostic
        let method_span = text_range_to_span(method_token.text_range());
        let struct_symbol = self.resolve_ctx.get_symbol(struct_def_id);
        let type_name = self.resolve_ctx.resolve(struct_symbol.name);
        let diag = self.method_not_found_diagnostic(
            &method_name,
            type_name,
            &available_methods,
            method_span,
        );
        self.diagnostics.push(diag);
        self.types.error()
    }

    /// Handle module-qualified calls like `module.func()` or `outer.inner.func()`
    fn synth_module_qualified_call(
        &mut self,
        call: &CallExpr,
        segments: &[spl_ast::PathSegment],
        initial_module_def_id: crate::symbol::DefId,
    ) -> TypeId {
        // Navigate through nested modules until we reach the final item
        let mut current_module_def_id = initial_module_def_id;

        // Process all segments except the last (which is the function name)
        for segment in segments.iter().take(segments.len() - 1).skip(1) {
            let segment_name = match segment.name() {
                Some(n) => match n.token() {
                    Some(t) => t.text().to_string(),
                    None => return self.types.error(),
                },
                None => return self.types.error(),
            };

            // Look up in the current module's scope
            if let Some(&scope_id) = self.module_scopes.get(&current_module_def_id) {
                let Some(interned) = self.resolve_ctx.try_get_interned(&segment_name) else {
                    let span = text_range_to_span(segment.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("cannot find `{segment_name}` in module"))
                            .with_label(span, "not found in module"),
                    );
                    return self.types.error();
                };

                if let Some(item_def_id) = self.resolve_ctx.lookup_in_scope(interned, scope_id) {
                    let item_symbol = self.resolve_ctx.get_symbol(item_def_id);

                    // Check visibility
                    if item_symbol.visibility == Visibility::Private {
                        let current_scope = self.current_inference_scope;
                        let item_scope = item_symbol.scope_id;
                        // Check if current scope is NOT the item's defining scope or a child of it
                        if !self.is_scope_descendant_of(current_scope, item_scope) {
                            let span = text_range_to_span(segment.syntax().text_range());
                            self.diagnostics.push(
                                Diagnostic::error(format!("`{segment_name}` is private"))
                                    .with_label(span, "private item"),
                            );
                            return self.types.error();
                        }
                    }

                    // Must be a module for intermediate segments
                    if item_symbol.kind != SymbolKind::Module {
                        let span = text_range_to_span(segment.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error(format!("`{segment_name}` is not a module"))
                                .with_label(span, "expected module"),
                        );
                        return self.types.error();
                    }

                    current_module_def_id = item_def_id;
                } else {
                    let span = text_range_to_span(segment.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("cannot find `{segment_name}` in module"))
                            .with_label(span, "not found in module"),
                    );
                    return self.types.error();
                }
            } else {
                return self.types.error();
            }
        }

        // Now handle the final segment (the function/struct being called)
        let last_segment = &segments[segments.len() - 1];
        let last_name = match last_segment.name() {
            Some(n) => match n.token() {
                Some(t) => t.text().to_string(),
                None => return self.types.error(),
            },
            None => return self.types.error(),
        };

        // Look up the final item in the current module's scope
        let Some(&scope_id) = self.module_scopes.get(&current_module_def_id) else {
            return self.types.error();
        };

        let Some(interned) = self.resolve_ctx.try_get_interned(&last_name) else {
            let span = text_range_to_span(last_segment.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error(format!("cannot find `{last_name}` in module"))
                    .with_label(span, "not found in module"),
            );
            return self.types.error();
        };

        let Some(item_def_id) = self.resolve_ctx.lookup_in_scope(interned, scope_id) else {
            let span = text_range_to_span(last_segment.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error(format!("cannot find `{last_name}` in module"))
                    .with_label(span, "not found in module"),
            );
            return self.types.error();
        };

        let item_symbol = self.resolve_ctx.get_symbol(item_def_id);

        // Check visibility
        if item_symbol.visibility == Visibility::Private {
            let current_scope = self.current_inference_scope;
            let item_scope = item_symbol.scope_id;
            // Check if current scope is NOT the item's defining scope or a child of it
            if !self.is_scope_descendant_of(current_scope, item_scope) {
                let span = text_range_to_span(last_segment.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error(format!("`{last_name}` is private"))
                        .with_label(span, "private item"),
                );
                return self.types.error();
            }
        }

        // Handle based on item kind
        match item_symbol.kind {
            SymbolKind::Function => {
                // It's a function - call it
                if let Some(sig) = self.defs.fn_signatures.get(&item_def_id).cloned() {
                    // Store resolution for HIR lowering
                    let method_span = text_range_to_span(last_segment.syntax().text_range());
                    self.results
                        .method_resolutions
                        .insert(method_span, item_def_id);

                    return self.synth_call_as_function(call, &sig);
                }
                self.types.error()
            }
            SymbolKind::Struct => {
                // It's a struct - instantiate it
                self.synth_call_as_struct(call, item_def_id)
            }
            _ => {
                let span = text_range_to_span(last_segment.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error(format!("`{last_name}` is not a function or struct"))
                        .with_label(span, "expected function or struct"),
                );
                self.types.error()
            }
        }
    }

    /// Handle instance method calls like `instance.method()` or `instance.field.method()`
    fn synth_instance_method_call(
        &mut self,
        call: &CallExpr,
        segments: &[spl_ast::PathSegment],
        receiver_type: TypeId,
    ) -> TypeId {
        // Resolve receiver type and auto-deref references
        let resolved = self.resolve_type(receiver_type);
        let mut current_resolved = resolved;
        let mut receiver_type_val = self.types.get(resolved).clone();

        const MAX_DEREF: usize = 100;
        #[cfg(debug_assertions)]
        let mut deref_count = 0;

        while let Type::Ref(_, inner) = &receiver_type_val {
            #[cfg(debug_assertions)]
            {
                deref_count += 1;
                debug_assert!(
                    deref_count < MAX_DEREF,
                    "invariant: auto-deref must terminate (hit {MAX_DEREF} derefs)"
                );
            }
            let inner_resolved = self.resolve_type(*inner);
            current_resolved = inner_resolved;
            receiver_type_val = self.types.get(inner_resolved).clone();
        }

        let method_segment = segments.last().map(|s| s.syntax().text().to_string()).unwrap_or_default();
        debug!(method_name = %method_segment, receiver_type = current_resolved.index(), "resolving method call");

        // Handle intermediate segments as field accesses
        // For c.inner.get(), segments are [c, inner, get]:
        // - First segment (index 0) is the receiver variable
        // - Intermediate segments (indices 1..n-1) are field accesses
        // - Last segment (index n-1) is the method name
        for field_segment in &segments[1..segments.len() - 1] {
            let field_name = match field_segment.name().and_then(|n| n.token()) {
                Some(t) => t.text().to_string(),
                None => return self.types.error(),
            };

            // Look up the field in the current struct type
            let (struct_def_id, type_args) = match &receiver_type_val {
                Type::Struct(def_id, args) => (*def_id, args.clone()),
                Type::Ref(_, inner) => {
                    let inner_resolved = self.resolve_type(*inner);
                    if let Type::Struct(def_id, args) = self.types.get(inner_resolved) {
                        (*def_id, args.clone())
                    } else {
                        let span = text_range_to_span(call.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error("field access on non-struct type")
                                .with_label(span, "not a struct"),
                        );
                        return self.types.error();
                    }
                }
                _ => {
                    let span = text_range_to_span(call.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("field access on non-struct type")
                            .with_label(span, "not a struct"),
                    );
                    return self.types.error();
                }
            };

            // Find the field
            if let Some(fields) = self.defs.struct_fields.get(&struct_def_id).cloned() {
                if let Some((_, field_ty, _)) =
                    fields.iter().find(|(name, _, _)| name == &field_name)
                {
                    // Build substitution map from struct type params to type args
                    let struct_type_params = self
                        .defs
                        .struct_type_params
                        .get(&struct_def_id)
                        .cloned()
                        .unwrap_or_default();
                    let mut subst: rustc_hash::FxHashMap<crate::DefId, TypeId> =
                        rustc_hash::FxHashMap::default();
                    for (param, arg) in struct_type_params.iter().zip(type_args.iter()) {
                        subst.insert(*param, *arg);
                    }

                    // Substitute type params if needed
                    let instantiated_ty = self.substitute_type_params(*field_ty, &subst);
                    current_resolved = self.resolve_type(instantiated_ty);
                    receiver_type_val = self.types.get(current_resolved).clone();

                    // Auto-deref if needed
                    while let Type::Ref(_, inner) = &receiver_type_val {
                        let inner_resolved = self.resolve_type(*inner);
                        current_resolved = inner_resolved;
                        receiver_type_val = self.types.get(inner_resolved).clone();
                    }
                } else {
                    let span = text_range_to_span(call.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("no field `{field_name}` on type"))
                            .with_label(span, "unknown field"),
                    );
                    return self.types.error();
                }
            } else {
                let span = text_range_to_span(call.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("field access on non-struct type")
                        .with_label(span, "not a struct"),
                );
                return self.types.error();
            }
        }

        // Get the method name from the last segment (needed for both opaque and struct methods)
        let last_segment = &segments[segments.len() - 1];
        let Some(method_name_ref) = last_segment.name() else {
            return self.types.error();
        };

        let Some(method_token) = method_name_ref.token() else {
            return self.types.error();
        };

        let method_name = method_token.text().to_string();

        // Check primitive type methods first (e.g., str.ptr(), str.len())
        if let Some(method_def_ids) = self
            .methods
            .primitive_methods
            .get(&current_resolved)
            .cloned()
        {
            for method_def_id in &method_def_ids {
                // Look up method name from builtin_method_names
                let fn_name = match self.methods.builtin_method_names.get(method_def_id) {
                    Some(name) => name.as_str(),
                    None => continue,
                };

                if fn_name == method_name
                    && let Some(sig) = self.defs.fn_signatures.get(method_def_id).cloned()
                {
                    // Check argument count
                    let args: Vec<_> = call.args().collect();
                    if args.len() != sig.params.len() {
                        let span = text_range_to_span(call.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "expected {} argument{}, found {}",
                                sig.params.len(),
                                if sig.params.len() == 1 { "" } else { "s" },
                                args.len()
                            ))
                            .with_label(span, "wrong number of arguments"),
                        );
                        return self.types.error();
                    }
                    // Type check arguments
                    for (arg, param) in args.iter().zip(&sig.params) {
                        if let Some(value) = arg.value() {
                            self.check_expr(&value, param.ty);
                        }
                    }
                    // Store resolution for HIR lowering (same as struct methods)
                    let call_span = text_range_to_span(call.syntax().text_range());
                    self.results
                        .method_resolutions
                        .insert(call_span, *method_def_id);
                    return sig.ret;
                }
            }
            // Method not found on str - collect available methods for suggestions
            let available_methods: Vec<&str> = method_def_ids
                .iter()
                .filter_map(|def_id| {
                    self.methods
                        .builtin_method_names
                        .get(def_id)
                        .map(String::as_str)
                })
                .collect();
            let span = text_range_to_span(call.syntax().text_range());
            let diag =
                self.method_not_found_diagnostic(&method_name, "str", &available_methods, span);
            self.diagnostics.push(diag);
            return self.types.error();
        }

        // Get the struct def_id and type_args from the receiver type
        let (struct_def_id, receiver_type_args) =
            if let Type::Struct(def_id, type_args) = &receiver_type_val {
                (*def_id, type_args.clone())
            } else {
                let span = text_range_to_span(call.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("method call on non-struct type")
                        .with_label(span, "not a struct"),
                );
                return self.types.error();
            };

        // Look up the method in the struct's impl
        let method_def_ids = self
            .defs
            .struct_methods
            .get(&struct_def_id)
            .cloned()
            .unwrap_or_default();

        // Collect available method names for error messages
        let available_methods: Vec<&str> = method_def_ids
            .iter()
            .map(|&def_id| {
                let symbol = self.resolve_ctx.get_symbol(def_id);
                self.resolve_ctx.resolve(symbol.name)
            })
            .collect();

        for method_def_id in &method_def_ids {
            let symbol = self.resolve_ctx.get_symbol(*method_def_id);
            let fn_name = self.resolve_ctx.resolve(symbol.name);
            if fn_name == method_name
                && let Some(sig) = self.defs.fn_signatures.get(method_def_id).cloned()
            {
                // Check method visibility
                if symbol.visibility == Visibility::Private {
                    // Private methods: check if accessor is in same module as the struct/method
                    let struct_symbol = self.resolve_ctx.get_symbol(struct_def_id);
                    let current_scope = self.current_inference_scope;
                    let struct_scope = struct_symbol.scope_id;
                    // Check if current scope is NOT the struct's defining scope or a child of it
                    if !self.is_scope_descendant_of(current_scope, struct_scope) {
                        let span = text_range_to_span(method_token.text_range());
                        self.diagnostics.push(
                            Diagnostic::error(format!("method `{method_name}` is private"))
                                .with_label(span, "private method"),
                        );
                        return self.types.error();
                    }
                }

                // Store the resolution for the method
                let method_span = text_range_to_span(method_token.text_range());
                self.results
                    .method_resolutions
                    .insert(method_span, *method_def_id);

                // Call the method with adjusted argument handling for self parameter
                return self.synth_method_call_with_receiver(
                    call,
                    &sig,
                    struct_def_id,
                    &receiver_type_args,
                );
            }
        }

        // Method not found - provide helpful diagnostic
        let method_span = text_range_to_span(method_token.text_range());
        let struct_symbol = self.resolve_ctx.get_symbol(struct_def_id);
        let type_name = self.resolve_ctx.resolve(struct_symbol.name);
        let diag = self.method_not_found_diagnostic(
            &method_name,
            type_name,
            &available_methods,
            method_span,
        );
        self.diagnostics.push(diag);
        self.types.error()
    }

    /// Synthesize type for an instance method call with a receiver
    fn synth_method_call_with_receiver(
        &mut self,
        call: &CallExpr,
        sig: &super::engine::FnSignature,
        struct_def_id: crate::DefId,
        receiver_type_args: &[TypeId],
    ) -> TypeId {
        debug!(
            struct_def_id = struct_def_id.index(),
            param_count = sig.params.len(),
            return_type = sig.ret.index(),
            "synthesizing method call with receiver"
        );

        // Get struct type params for building substitution map
        let struct_type_params = self
            .defs
            .struct_type_params
            .get(&struct_def_id)
            .cloned()
            .unwrap_or_default();

        // Build substitution map: impl type params -> receiver's type args
        // sig.type_params structure: [impl_params..., method_params...]
        // where impl_params.len() == struct_type_params.len()
        let mut subst: FxHashMap<_, _> = FxHashMap::default();
        let impl_param_count = struct_type_params.len();
        for (i, &param_def_id) in sig.type_params.iter().enumerate() {
            let type_arg = if i < impl_param_count && i < receiver_type_args.len() {
                // This is an impl type param at position i, map to receiver's type arg
                receiver_type_args[i]
            } else {
                // This is a method-specific type param, create fresh type var
                self.fresh_type_var()
            };
            subst.insert(param_def_id, type_arg);
        }

        // Substitute in params and return type
        let param_types: Vec<TypeId> = sig
            .params
            .iter()
            .map(|p| self.substitute_type_params(p.ty, &subst))
            .collect();
        let ret_ty = self.substitute_type_params(sig.ret, &subst);

        // Check argument count
        let args: Vec<_> = call.args().collect();
        if args.len() != param_types.len() {
            let span = text_range_to_span(call.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "expected {} argument{}, found {}",
                    param_types.len(),
                    if param_types.len() == 1 { "" } else { "s" },
                    args.len()
                ))
                .with_label(span, "wrong number of arguments"),
            );
            return ret_ty;
        }

        // Check each argument using check_expr which handles coercion
        for (arg, &expected_ty) in args.iter().zip(param_types.iter()) {
            if let Some(expr) = arg.value() {
                self.check_expr(&expr, expected_ty);
            }
        }

        ret_ty
    }

    /// Synthesize type for an apply expression being used as a function call
    fn synth_call_as_function(
        &mut self,
        call: &CallExpr,
        sig: &super::engine::FnSignature,
    ) -> TypeId {
        let (param_infos, ret_ty) = self.instantiate_signature_with_labels(sig);
        debug!(
            arg_count = call.args().count(),
            param_count = param_infos.len(),
            return_type = ret_ty.index(),
            type_param_count = sig.type_params.len(),
            "resolved function call"
        );

        // Check argument count
        let args: Vec<_> = call.args().collect();
        if args.len() != param_infos.len() {
            let span = text_range_to_span(call.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "expected {} argument{}, found {}",
                    param_infos.len(),
                    if param_infos.len() == 1 { "" } else { "s" },
                    args.len()
                ))
                .with_label(span, "wrong number of arguments"),
            );
            return ret_ty;
        }

        // Check each argument, validating labels and types
        for (arg, param_info) in args.iter().zip(param_infos.iter()) {
            // Get the argument's label (if any)
            let arg_label = arg.name_token().map(|t| t.text().to_string()).or_else(|| {
                arg.name()
                    .and_then(|n| n.token())
                    .map(|t| t.text().to_string())
            });

            let arg_span = text_range_to_span(arg.syntax().text_range());

            // Validate label matches
            match (&param_info.label, &arg_label) {
                (Some(expected), Some(actual)) if expected != actual => {
                    self.diagnostics.push(
                        Diagnostic::error(format!("expected label `{expected}`, found `{actual}`"))
                            .with_label(arg_span, "wrong label"),
                    );
                }
                (Some(expected), None) => {
                    self.diagnostics.push(
                        Diagnostic::error(format!("expected labeled argument `{expected}`"))
                            .with_label(arg_span, "missing label"),
                    );
                }
                (None, Some(actual)) => {
                    self.diagnostics.push(
                        Diagnostic::error(format!("unexpected label `{actual}`"))
                            .with_label(arg_span, "positional parameter"),
                    );
                }
                _ => {} // Labels match (or both are None for positional)
            }

            // Check argument type
            if let Some(expr) = arg.value() {
                self.check_expr(&expr, param_info.ty);
            }
        }

        ret_ty
    }

    /// Synthesize type for an apply expression being used as struct instantiation
    fn synth_call_as_struct(&mut self, call: &CallExpr, struct_def_id: crate::DefId) -> TypeId {
        let field_count = self
            .defs
            .struct_fields
            .get(&struct_def_id)
            .map(Vec::len)
            .unwrap_or(0);
        debug!(
            struct_def_id = struct_def_id.index(),
            field_count,
            arg_count = call.args().count(),
            "synthesizing struct instantiation"
        );

        // Get struct type params and create substitution map
        let type_params = self
            .defs
            .struct_type_params
            .get(&struct_def_id)
            .cloned()
            .unwrap_or_default();

        // Create fresh type variables for each type parameter and build substitution
        let mut subst: FxHashMap<_, _> = FxHashMap::default();
        let mut type_args = Vec::new();
        for param_def_id in &type_params {
            let fresh_var = self.fresh_type_var();
            subst.insert(*param_def_id, fresh_var);
            type_args.push(fresh_var);
        }

        // Get struct field info and substitute type params
        let fields_info = self
            .defs
            .struct_fields
            .get(&struct_def_id)
            .cloned()
            .unwrap_or_default();
        let instantiated_fields: Vec<(String, TypeId)> = fields_info
            .iter()
            .map(|(name, ty, _def_id)| (name.clone(), self.substitute_type_params(*ty, &subst)))
            .collect();
        let field_map: FxHashMap<_, _> = instantiated_fields.iter().cloned().collect();

        // Check for struct update syntax: ...base
        let has_update_base = if let Some(update_base) = call.update_base() {
            if let Some(base_expr) = update_base.expr() {
                let base_ty = self.synth_expr(&base_expr);
                let expected_struct_ty = self.types.mk_struct(struct_def_id, type_args.clone());
                if self.unify(base_ty, expected_struct_ty).is_err() {
                    let span = text_range_to_span(base_expr.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("struct update base has wrong type")
                            .with_label(span, "wrong type"),
                    );
                }
            }
            true
        } else {
            false
        };

        // Track seen fields
        let mut seen_fields = std::collections::HashSet::new();

        // Process each argument (field initializer)
        for arg in call.args() {
            // Get the field name from the argument
            let field_name = if let Some(token) = arg.name_token() {
                // Named argument via token: name = value
                Some(token.text().to_string())
            } else if let Some(name_ref) = arg.name() {
                // Named argument: name = value
                name_ref.token().map(|t| t.text().to_string())
            } else {
                // Positional/shorthand argument - try to get name from value if it's a path
                if let Some(Expr::Path(path_expr)) = arg.value() {
                    if let Some(path) = path_expr.path() {
                        if let Some(seg) = path.segments().next() {
                            if let Some(nr) = seg.name() {
                                nr.token().map(|t| t.text().to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            let Some(field_name) = field_name else {
                continue;
            };

            // Find the field in the struct
            if let Some(&expected_type) = field_map.get(&field_name) {
                if !seen_fields.insert(field_name.clone()) {
                    let span = text_range_to_span(arg.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("duplicate field `{field_name}`"))
                            .with_label(span, "duplicate field"),
                    );
                }

                // Synthesize and check the value expression
                if let Some(value_expr) = arg.value() {
                    self.check_expr(&value_expr, expected_type);
                }
            } else {
                // Unknown field
                let span = text_range_to_span(arg.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error(format!("unknown field `{field_name}`"))
                        .with_label(span, "unknown field"),
                );
            }
        }

        // Check for missing fields (only if no update base)
        if !has_update_base {
            for (field_name, _) in &instantiated_fields {
                if !seen_fields.contains(field_name) {
                    let span = text_range_to_span(call.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("missing field `{field_name}`"))
                            .with_label(span, "missing field"),
                    );
                }
            }
        }

        self.types.mk_struct(struct_def_id, type_args)
    }

    fn synth_binary(&mut self, bin: &BinExpr) -> TypeId {
        let Some(op) = bin.op_token() else {
            return self.types.error();
        };

        let Some(lhs) = bin.lhs() else {
            return self.types.error();
        };

        let Some(rhs) = bin.rhs() else {
            return self.types.error();
        };

        debug!(operator = %op.text(), "synthesizing binary expression");

        match op.kind() {
            // Arithmetic operators - result is same as operands
            SyntaxKind::PLUS
            | SyntaxKind::MINUS
            | SyntaxKind::STAR
            | SyntaxKind::SLASH
            | SyntaxKind::PERCENT => {
                let lhs_ty = self.synth_expr(&lhs);
                let rhs_ty = self.synth_expr(&rhs);

                // Check operand types are numeric
                let lhs_resolved = self.resolve_type(lhs_ty);
                let lhs_type = self.types.get(lhs_resolved).clone();
                let is_lhs_numeric = match &lhs_type {
                    Type::Infer(_, InferKind::Int) | Type::Infer(_, InferKind::Float) => true,
                    Type::Primitive(p) => is_numeric_type(*p),
                    _ => false,
                };
                if !is_lhs_numeric {
                    let span = text_range_to_span(lhs.syntax().text_range());
                    let lhs_str = self.type_to_string(lhs_ty);
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "cannot apply binary operator to `{lhs_str}` (non-numeric type)"
                        ))
                        .with_label(span, format!("type `{lhs_str}` is not numeric")),
                    );
                    return self.types.error();
                }

                if let Err(err) = self.unify(lhs_ty, rhs_ty) {
                    let span = text_range_to_span(rhs.syntax().text_range());
                    let diag = self.unify_error_diagnostic(&err, "arithmetic operands", span);
                    self.diagnostics.push(diag);
                    return self.types.error();
                }

                lhs_ty
            }

            // Comparison operators - result is bool
            SyntaxKind::EQ_EQ
            | SyntaxKind::NE
            | SyntaxKind::LT
            | SyntaxKind::LE
            | SyntaxKind::GT
            | SyntaxKind::GE => {
                let lhs_ty = self.synth_expr(&lhs);
                let rhs_ty = self.synth_expr(&rhs);

                if self.unify(lhs_ty, rhs_ty).is_err() {
                    let span = text_range_to_span(rhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch in comparison")
                            .with_label(span, "mismatched operand types"),
                    );
                }

                self.types.bool()
            }

            // Logical operators - operands and result are bool
            SyntaxKind::AND_AND | SyntaxKind::OR_OR => {
                let lhs_ty = self.synth_expr(&lhs);
                let rhs_ty = self.synth_expr(&rhs);
                let bool_ty = self.types.bool();

                if self.unify(lhs_ty, bool_ty).is_err() {
                    let span = text_range_to_span(lhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch: expected bool for logical operator")
                            .with_label(span, "not a bool"),
                    );
                }
                if self.unify(rhs_ty, bool_ty).is_err() {
                    let span = text_range_to_span(rhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch: expected bool for logical operator")
                            .with_label(span, "not a bool"),
                    );
                }

                bool_ty
            }

            // Assignment operators - result is unit
            SyntaxKind::EQ
            | SyntaxKind::PLUS_EQ
            | SyntaxKind::MINUS_EQ
            | SyntaxKind::STAR_EQ
            | SyntaxKind::SLASH_EQ
            | SyntaxKind::PERCENT_EQ => {
                let lhs_ty = self.synth_expr(&lhs);
                let rhs_ty = self.synth_expr(&rhs);

                // Check mutability of assignment target
                if let Some(err_msg) = self.check_assignable(&lhs) {
                    let span = text_range_to_span(lhs.syntax().text_range());
                    self.diagnostics
                        .push(Diagnostic::error(err_msg).with_label(span, "cannot assign to this"));
                }

                if self.unify(lhs_ty, rhs_ty).is_err() {
                    let span = text_range_to_span(rhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch in assignment")
                            .with_label(span, "mismatched types"),
                    );
                }

                self.types.unit()
            }

            _ => self.types.error(),
        }
    }

    fn synth_prefix(&mut self, prefix: &PrefixExpr) -> TypeId {
        let Some(op) = prefix.op_token() else {
            return self.types.error();
        };

        let Some(inner) = prefix.expr() else {
            return self.types.error();
        };

        let inner_ty = self.synth_expr(&inner);

        match op.kind() {
            SyntaxKind::MINUS => {
                // Negation is valid for numeric types
                // Note: Negated suffixed literals (e.g., -128i8) are handled by HIR lowering
                let resolved = self.resolve_type(inner_ty);
                let ty = self.types.get(resolved).clone();
                match &ty {
                    Type::Infer(_, InferKind::Int) | Type::Infer(_, InferKind::Float) => inner_ty,
                    Type::Primitive(p) if is_numeric_type(*p) => inner_ty,
                    _ => {
                        let span = text_range_to_span(inner.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error("cannot apply unary `-` to non-numeric type")
                                .with_label(span, "not a numeric type"),
                        );
                        self.types.error()
                    }
                }
            }
            SyntaxKind::BANG => {
                // Logical not is valid for bool
                let bool_ty = self.types.bool();
                if self.unify(inner_ty, bool_ty).is_err() {
                    let span = text_range_to_span(inner.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("cannot apply unary `!` to non-bool type")
                            .with_label(span, "not a bool"),
                    );
                    return self.types.error();
                }
                bool_ty
            }
            SyntaxKind::STAR => {
                // Dereference
                let resolved = self.resolve_type(inner_ty);
                let ty = self.types.get(resolved).clone();
                if let Type::Ref(_, inner_ref) = ty {
                    inner_ref
                } else {
                    let span = text_range_to_span(inner.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("cannot dereference non-reference type")
                            .with_label(span, "not a reference"),
                    );
                    self.types.error()
                }
            }
            _ => self.types.error(),
        }
    }

    fn synth_ref(&mut self, ref_expr: &RefExpr) -> TypeId {
        let Some(inner) = ref_expr.expr() else {
            return self.types.error();
        };

        let inner_ty = self.synth_expr(&inner);
        let mutability = if ref_expr.mut_kw().is_some() {
            // Check that the referenced expression is mutable
            if let Some(err_msg) = self.check_mutable_borrow(&inner) {
                let span = text_range_to_span(inner.syntax().text_range());
                self.diagnostics
                    .push(Diagnostic::error(err_msg).with_label(span, "cannot borrow as mutable"));
            }
            Mutability::Mutable
        } else {
            Mutability::Shared
        };

        self.types.mk_ref(mutability, inner_ty)
    }

    fn synth_field(&mut self, field: &FieldExpr) -> TypeId {
        const MAX_DEREF: usize = 100;

        let Some(base) = field.expr() else {
            debug!("synth_field: missing base expression");
            return self.types.error();
        };

        let base_ty = self.synth_expr(&base);
        let resolved = self.resolve_type(base_ty);
        let mut base_type = self.types.get(resolved).clone();

        // Auto-deref references for field access
        #[cfg(debug_assertions)]
        let mut deref_count = 0;

        while let Type::Ref(_, inner) = &base_type {
            #[cfg(debug_assertions)]
            {
                deref_count += 1;
                debug_assert!(
                    deref_count < MAX_DEREF,
                    "invariant: auto-deref must terminate (hit {MAX_DEREF} derefs)"
                );
            }

            let inner_resolved = self.resolve_type(*inner);
            base_type = self.types.get(inner_resolved).clone();
        }

        // Handle tuple field access (e.g., t.0, t.1)
        // Try tuple_index_token first (INT_LITERAL), then name_token (raw IDENT),
        // then fall back to name() (NameRef)
        let field_name = match field.tuple_index_token() {
            Some(t) => t.text().to_string(),
            None => match field.name_token() {
                Some(t) => t.text().to_string(),
                None => match field.name().and_then(|n| n.ident_token()) {
                    Some(t) => t.text().to_string(),
                    None => return self.types.error(),
                },
            },
        };

        debug!(
            receiver_type = resolved.index(),
            field_name = %field_name,
            "synthesizing field access"
        );

        // Check if it's a tuple index
        if let Ok(idx) = field_name.parse::<usize>()
            && let Type::Tuple(elems) = &base_type
            && idx < elems.len()
        {
            return elems[idx];
        }

        // Block field access on opaque types (StrRef) - use methods instead
        if let Ok(idx) = field_name.parse::<usize>()
            && matches!(&base_type, Type::StrRef)
        {
            let span = text_range_to_span(field.syntax().text_range());
            let hint = match idx {
                0 => "use `.ptr()` to access the pointer",
                1 => "use `.len()` to access the length",
                _ => "str has no such field",
            };
            self.diagnostics.push(
                Diagnostic::error(format!("no field `{idx}` on type `str`")).with_label(span, hint),
            );
            return self.types.error();
        }

        // Handle module item access (e.g., module.Struct)
        if let Type::Module(module_def_id) = &base_type {
            let module_def_id = *module_def_id;

            // Look up the field name in the module's scope
            if let Some(&scope_id) = self.module_scopes.get(&module_def_id) {
                let Some(interned) = self.resolve_ctx.try_get_interned(&field_name) else {
                    // Name wasn't interned, so it definitely doesn't exist in the module
                    let span = text_range_to_span(field.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("cannot find `{field_name}` in module"))
                            .with_label(span, "not found in module"),
                    );
                    return self.types.error();
                };
                if let Some(item_def_id) = self.resolve_ctx.lookup_in_scope(interned, scope_id) {
                    let item_symbol = self.resolve_ctx.get_symbol(item_def_id);

                    // Check visibility
                    if item_symbol.visibility == Visibility::Private {
                        let span = text_range_to_span(field.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "`{field_name}` is private and not accessible from this module"
                            ))
                            .with_label(span, "private item, not accessible"),
                        );
                        return self.types.error();
                    }

                    // Determine what kind of item it is and return appropriate type
                    match item_symbol.kind {
                        SymbolKind::Module => {
                            // Nested module access (outer.inner)
                            return self.types.mk_module(item_def_id);
                        }
                        SymbolKind::Struct => {
                            // Struct access for construction (types.Point)
                            return self.types.mk_struct(item_def_id, vec![]);
                        }
                        SymbolKind::Function => {
                            // Function reference - return function pointer type
                            if let Some(sig) = self.defs.fn_signatures.get(&item_def_id).cloned() {
                                let (param_types, ret_ty) = self.instantiate_signature(&sig);
                                return self.types.mk_fn_ptr(param_types, ret_ty);
                            }
                            return self.types.error();
                        }
                        SymbolKind::TypeAlias => {
                            // Type alias - similar to struct
                            return self.types.mk_alias(item_def_id, vec![]);
                        }
                        _ => {
                            // Other kinds shouldn't be in module scope
                            let span = text_range_to_span(field.syntax().text_range());
                            self.diagnostics.push(
                                Diagnostic::error(format!(
                                    "`{field_name}` is not a valid item in module"
                                ))
                                .with_label(span, "unexpected item kind"),
                            );
                            return self.types.error();
                        }
                    }
                }

                // Item not found in module
                let span = text_range_to_span(field.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error(format!("cannot find `{field_name}` in module"))
                        .with_label(span, "not found in module"),
                );
                return self.types.error();
            }

            // Module scope not found (shouldn't happen for valid modules)
            let span = text_range_to_span(field.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error("internal error: module scope not found")
                    .with_label(span, "module scope missing"),
            );
            return self.types.error();
        }

        // Handle struct field access
        if let Type::Struct(def_id, type_args) = &base_type {
            let def_id = *def_id;
            let type_args = type_args.clone();

            // Build substitution map from struct's type params to type args
            let type_params = self
                .defs
                .struct_type_params
                .get(&def_id)
                .cloned()
                .unwrap_or_default();
            let mut subst: FxHashMap<_, _> = FxHashMap::default();
            for (param_def_id, type_arg) in type_params.iter().zip(type_args.iter()) {
                subst.insert(*param_def_id, *type_arg);
            }

            if let Some(fields) = self.defs.struct_fields.get(&def_id).cloned() {
                for (name, ty, field_def_id) in fields {
                    if name == field_name {
                        // Check field visibility
                        if field_def_id.is_valid() {
                            let field_symbol = self.resolve_ctx.get_symbol(field_def_id);
                            if field_symbol.visibility == Visibility::Private {
                                // Private fields: check if accessor is in same module as struct
                                let struct_symbol = self.resolve_ctx.get_symbol(def_id);
                                // If struct is defined in a different scope hierarchy, field may not be accessible
                                // Simple check: if scopes differ significantly, report error
                                // This is a simplified check - proper module hierarchy checking is more complex
                                let current_scope = self.current_inference_scope;
                                let struct_scope = struct_symbol.scope_id;
                                // Check if current scope is NOT the struct's defining scope or a child of it
                                if !self.is_scope_descendant_of(current_scope, struct_scope) {
                                    let span = text_range_to_span(field.syntax().text_range());
                                    self.diagnostics.push(
                                        Diagnostic::error(format!(
                                            "field `{field_name}` is private"
                                        ))
                                        .with_label(span, "private field"),
                                    );
                                    return self.types.error();
                                }
                            }
                        }
                        // Substitute type parameters in field type
                        return self.substitute_type_params(ty, &subst);
                    }
                }
            }
            let span = text_range_to_span(field.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error(format!("no field `{field_name}` on struct"))
                    .with_label(span, "unknown field"),
            );
            return self.types.error();
        }

        let span = text_range_to_span(field.syntax().text_range());
        self.diagnostics.push(
            Diagnostic::error("field access on non-struct type").with_label(span, "not a struct"),
        );
        self.types.error()
    }

    fn synth_call(&mut self, call: &CallExpr) -> TypeId {
        let Some(callee) = call.callee() else {
            debug!("synth_call: missing callee");
            return self.types.error();
        };

        let callee_kind = match &callee {
            Expr::Path(_) => "path",
            Expr::Field(_) => "method",
            _ => "arbitrary",
        };
        debug!(callee_kind, arg_count = call.args().count(), "synthesizing call");

        // Dispatch based on callee type:
        // - PathExpr: function call or struct instantiation (old synth_apply)
        // - FieldExpr: method call (old synth_method_call)
        // - Other: arbitrary callable expression
        match &callee {
            Expr::Path(path_expr) => self.synth_call_path(call, path_expr),
            Expr::Field(field_expr) => self.synth_call_method(call, field_expr),
            _ => self.synth_call_arbitrary(call, &callee),
        }
    }

    /// Handle call where callee is a path (function call or struct instantiation)
    fn synth_call_path(&mut self, call: &CallExpr, path_expr: &PathExpr) -> TypeId {
        let Some(path) = path_expr.path() else {
            return self.types.error();
        };

        let segments: Vec<_> = path.segments().collect();
        if segments.is_empty() {
            return self.types.error();
        }

        // Handle multi-segment paths like `S.new` (associated function call)
        if segments.len() >= 2 {
            return self.synth_call_qualified_path(call, &segments);
        }

        // Single segment path - could be struct instantiation or function call
        let segment = &segments[0];
        let Some(name_ref) = segment.name() else {
            return self.types.error();
        };

        let Some(token) = name_ref.token() else {
            return self.types.error();
        };

        let span = text_range_to_span(token.text_range());
        let def_id = match self.resolutions.get(&span) {
            Some(id) => *id,
            None => return self.types.error(),
        };

        // Determine if this is a function or struct based on resolution
        if let Some(sig) = self.defs.fn_signatures.get(&def_id).cloned() {
            // It's a function call
            return self.synth_call_as_function(call, &sig);
        }

        // Check if it's a struct (has struct fields)
        if self.defs.struct_fields.contains_key(&def_id) {
            return self.synth_call_as_struct(call, def_id);
        }

        // Check if it's a type alias that resolves to a struct
        if let Some(&target_ty) = self.defs.type_alias_targets.get(&def_id) {
            let resolved = self.resolve_type(target_ty);
            if let Type::Struct(actual_def_id, _) = self.types.get(resolved) {
                return self.synth_call_as_struct(call, *actual_def_id);
            }
        }

        // Unknown - emit error
        self.diagnostics.push(
            Diagnostic::error("cannot call: not a function or struct")
                .with_label(span, "not callable or instantiable"),
        );
        self.types.error()
    }

    /// Handle call where callee is a field access (method call)
    fn synth_call_method(&mut self, call: &CallExpr, field_expr: &FieldExpr) -> TypeId {
        let Some(receiver) = field_expr.expr() else {
            return self.types.error();
        };

        let receiver_ty = self.synth_expr(&receiver);

        // Get method name
        let method_name = match field_expr.name_token() {
            Some(t) => t.text().to_string(),
            None => match field_expr.name() {
                Some(n) => match n.ident_token() {
                    Some(t) => t.text().to_string(),
                    None => return self.types.error(),
                },
                None => return self.types.error(),
            },
        };

        // Resolve receiver type to find struct DefId
        let resolved = self.resolve_type(receiver_ty);
        let receiver_type = self.types.get(resolved).clone();

        debug!(
            receiver_type = resolved.index(),
            method_name = %method_name,
            "synthesizing method call"
        );

        // Check primitive type methods first (e.g., str.ptr(), str.len())
        if let Some(method_def_ids) = self.methods.primitive_methods.get(&resolved).cloned() {
            for method_def_id in &method_def_ids {
                let fn_name = match self.methods.builtin_method_names.get(method_def_id) {
                    Some(name) => name.as_str(),
                    None => continue,
                };

                if fn_name == method_name
                    && let Some(sig) = self.defs.fn_signatures.get(method_def_id).cloned()
                {
                    let args: Vec<_> = call.args().collect();
                    if args.len() != sig.params.len() {
                        let span = text_range_to_span(call.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "expected {} argument{}, found {}",
                                sig.params.len(),
                                if sig.params.len() == 1 { "" } else { "s" },
                                args.len()
                            ))
                            .with_label(span, "wrong number of arguments"),
                        );
                        return self.types.error();
                    }
                    // Type check arguments
                    for (arg, param) in args.iter().zip(&sig.params) {
                        if let Some(value) = arg.value() {
                            self.check_expr(&value, param.ty);
                        }
                    }
                    // Store resolution for HIR lowering
                    let call_span = text_range_to_span(call.syntax().text_range());
                    self.results
                        .method_resolutions
                        .insert(call_span, *method_def_id);
                    return sig.ret;
                }
            }
            // Method not found on primitive type - collect available methods for suggestions
            let available_methods: Vec<&str> = method_def_ids
                .iter()
                .filter_map(|def_id| {
                    self.methods
                        .builtin_method_names
                        .get(def_id)
                        .map(String::as_str)
                })
                .collect();
            let type_name = self.type_to_string(resolved);
            let span = text_range_to_span(call.syntax().text_range());
            let diag = self.method_not_found_diagnostic(
                &method_name,
                &type_name,
                &available_methods,
                span,
            );
            self.diagnostics.push(diag);
            return self.types.error();
        }

        // Handle module function call (e.g., math.add(1, 2))
        if let Type::Module(module_def_id) = &receiver_type {
            return self.synth_module_function_call(call, *module_def_id, &method_name);
        }

        // Handle struct method call
        self.synth_struct_method_call(call, &receiver, receiver_ty, &method_name)
    }

    /// Handle module function call (e.g., `module.func()`)
    fn synth_module_function_call(
        &mut self,
        call: &CallExpr,
        module_def_id: crate::DefId,
        method_name: &str,
    ) -> TypeId {
        // Look up the method name in the module's scope
        if let Some(&scope_id) = self.module_scopes.get(&module_def_id) {
            let Some(interned) = self.resolve_ctx.try_get_interned(method_name) else {
                let span = text_range_to_span(call.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error(format!("cannot find function `{method_name}` in module"))
                        .with_label(span, "not found in module"),
                );
                return self.types.error();
            };

            if let Some(fn_def_id) = self.resolve_ctx.lookup_in_scope(interned, scope_id) {
                let fn_symbol = self.resolve_ctx.get_symbol(fn_def_id);

                if fn_symbol.visibility == Visibility::Private {
                    let span = text_range_to_span(call.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("`{method_name}` is private"))
                            .with_label(span, "private item"),
                    );
                    return self.types.error();
                }

                if fn_symbol.kind != SymbolKind::Function {
                    let span = text_range_to_span(call.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("`{method_name}` is not a function"))
                            .with_label(span, "expected a function"),
                    );
                    return self.types.error();
                }

                if let Some(sig) = self.defs.fn_signatures.get(&fn_def_id).cloned() {
                    let call_span = text_range_to_span(call.syntax().text_range());
                    self.results.method_resolutions.insert(call_span, fn_def_id);
                    let (param_types, ret_ty) = self.instantiate_signature(&sig);
                    return self.check_call_args(call, &param_types, ret_ty);
                }
            }
        }

        let span = text_range_to_span(call.syntax().text_range());
        self.diagnostics.push(
            Diagnostic::error(format!("cannot find function `{method_name}` in module"))
                .with_label(span, "not found in module"),
        );
        self.types.error()
    }

    /// Handle struct method call
    fn synth_struct_method_call(
        &mut self,
        call: &CallExpr,
        _receiver: &Expr,
        receiver_ty: TypeId,
        method_name: &str,
    ) -> TypeId {
        let resolved = self.resolve_type(receiver_ty);
        let receiver_type = self.types.get(resolved).clone();

        // Handle reference receivers (auto-deref) and get type args
        let (struct_def_id, receiver_type_args) = match &receiver_type {
            Type::Struct(def_id, type_args) => (Some(*def_id), type_args.clone()),
            Type::Ref(_, inner) => {
                let inner_resolved = self.resolve_type(*inner);
                let inner_type = self.types.get(inner_resolved);
                if let Type::Struct(def_id, type_args) = inner_type {
                    (Some(*def_id), type_args.clone())
                } else {
                    (None, vec![])
                }
            }
            _ => (None, vec![]),
        };

        if let Some(def_id) = struct_def_id {
            let method_def_ids = self
                .defs
                .struct_methods
                .get(&def_id)
                .cloned()
                .unwrap_or_default();

            // Collect available method names for error messages
            let available_methods: Vec<&str> = method_def_ids
                .iter()
                .map(|&method_def_id| {
                    let symbol = self.resolve_ctx.get_symbol(method_def_id);
                    self.resolve_ctx.resolve(symbol.name)
                })
                .collect();

            for method_def_id in &method_def_ids {
                let symbol = self.resolve_ctx.get_symbol(*method_def_id);
                let fn_name = self.resolve_ctx.resolve(symbol.name);
                if fn_name == method_name
                    && let Some(sig) = self.defs.fn_signatures.get(method_def_id).cloned()
                {
                    // Store resolution
                    let method_span = text_range_to_span(call.syntax().text_range());
                    self.results
                        .method_resolutions
                        .insert(method_span, *method_def_id);

                    return self.synth_method_call_with_receiver(
                        call,
                        &sig,
                        def_id,
                        &receiver_type_args,
                    );
                }
            }

            // Method not found on struct - provide helpful diagnostic
            let struct_symbol = self.resolve_ctx.get_symbol(def_id);
            let type_name = self.resolve_ctx.resolve(struct_symbol.name);
            let span = text_range_to_span(call.syntax().text_range());
            let diag =
                self.method_not_found_diagnostic(method_name, type_name, &available_methods, span);
            self.diagnostics.push(diag);
            return self.types.error();
        }

        // No struct type found - show generic error
        let type_name = self.type_to_string(receiver_ty);
        let span = text_range_to_span(call.syntax().text_range());
        let diag = self.method_not_found_diagnostic(method_name, &type_name, &[], span);
        self.diagnostics.push(diag);
        self.types.error()
    }

    /// Handle call with an arbitrary expression as callee (e.g., `(get_fn())(args)`)
    fn synth_call_arbitrary(&mut self, call: &CallExpr, callee: &Expr) -> TypeId {
        let callee_ty = self.synth_expr(callee);
        let resolved = self.resolve_type(callee_ty);
        let callee_type = self.types.get(resolved).clone();

        // Check if callee is a function pointer
        let Type::FnPtr { params, ret } = callee_type else {
            let span = text_range_to_span(callee.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error("value is not a function").with_label(span, "not a function"),
            );
            return self.types.error();
        };
        let (param_types, ret_ty) = (params, ret);

        self.check_call_args(call, &param_types, ret_ty)
    }

    fn check_call_args(
        &mut self,
        call: &CallExpr,
        param_types: &[TypeId],
        ret_ty: TypeId,
    ) -> TypeId {
        let args: Vec<_> = call.args().collect();

        if args.len() != param_types.len() {
            let span = text_range_to_span(call.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "expected {} argument{}, found {}",
                    param_types.len(),
                    if param_types.len() == 1 { "" } else { "s" },
                    args.len()
                ))
                .with_label(span, "wrong number of arguments"),
            );
            return ret_ty;
        }

        for (arg, expected_ty) in args.iter().zip(param_types.iter()) {
            if let Some(value) = arg.value() {
                self.check_expr(&value, *expected_ty);
            }
        }

        ret_ty
    }

    fn synth_index(&mut self, index: &IndexExpr) -> TypeId {
        let Some(base) = index.base() else {
            return self.types.error();
        };

        let Some(idx) = index.index() else {
            return self.types.error();
        };

        let base_ty = self.synth_expr(&base);
        let _ = self.synth_expr(&idx); // Check index expression

        let resolved = self.resolve_type(base_ty);
        let base_type = self.types.get(resolved).clone();

        match base_type {
            Type::Array(elem, len) => {
                // Check constant index bounds
                if let Some(idx_val) = self.eval_const_usize(&idx)
                    && idx_val >= len as usize
                {
                    let span = text_range_to_span(idx.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "index {idx_val} is out of bounds for array of length {len}"
                        ))
                        .with_label(span, "index out of bounds"),
                    );
                }
                elem
            }
            Type::Slice(elem) => elem,
            _ => {
                let span = text_range_to_span(base.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("cannot index into this type")
                        .with_label(span, "not indexable"),
                );
                self.types.error()
            }
        }
    }

    fn synth_slice(&mut self, slice: &SliceExpr) -> TypeId {
        let Some(base) = slice.base() else {
            return self.types.error();
        };

        let base_ty = self.synth_expr(&base);

        // Check range bounds if present
        if let Some(start) = slice.start() {
            self.synth_expr(&start);
        }
        if let Some(end) = slice.end() {
            self.synth_expr(&end);
        }

        let resolved = self.resolve_type(base_ty);
        let base_type = self.types.get(resolved).clone();

        match base_type {
            Type::Array(elem, _) | Type::Slice(elem) => self.types.mk_slice(elem),
            _ => {
                let span = text_range_to_span(base.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("cannot slice this type").with_label(span, "not sliceable"),
                );
                self.types.error()
            }
        }
    }

    fn synth_if(&mut self, if_expr: &IfExpr) -> TypeId {
        let has_else = if_expr.else_branch().is_some() || if_expr.else_block().is_some();
        debug!(has_else, "synthesizing if expression");

        // Check condition is bool
        if let Some(cond) = if_expr.condition() {
            let cond_ty = self.synth_expr(&cond);
            let bool_ty = self.types.bool();
            if self.unify(cond_ty, bool_ty).is_err() {
                let span = text_range_to_span(cond.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("if condition must be bool")
                        .with_label(span, "expected bool"),
                );
            }
        }

        // Synthesize then branch
        let then_ty = if let Some(then_block) = if_expr.then_branch() {
            self.synth_block(&then_block)
        } else {
            self.types.unit()
        };

        // Synthesize else branch (if present)
        // The else branch can be an Expr (for else-if) or a direct Block (for else { ... })
        let else_ty = if let Some(else_expr) = if_expr.else_branch() {
            // else-if case: else_expr is another IfExpr
            self.synth_expr(&else_expr)
        } else if let Some(else_block) = if_expr.else_block() {
            // else { ... } case: direct Block
            self.synth_block(&else_block)
        } else {
            // No else branch - if expression returns unit
            return self.types.unit();
        };

        // Unify branches
        if let Err(err) = self.unify(then_ty, else_ty) {
            let span = text_range_to_span(if_expr.syntax().text_range());
            let diag = self.unify_error_diagnostic(&err, "if/else branches", span);
            self.diagnostics.push(diag);
        }

        then_ty
    }

    fn synth_while(&mut self, while_expr: &WhileExpr) -> TypeId {
        debug!("synthesizing while loop");

        // Check condition is bool
        if let Some(cond) = while_expr.condition() {
            let cond_ty = self.synth_expr(&cond);
            let bool_ty = self.types.bool();
            if self.unify(cond_ty, bool_ty).is_err() {
                let span = text_range_to_span(cond.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("while condition must be bool")
                        .with_label(span, "expected bool"),
                );
            }
        }

        // Set loop context for break/continue validation
        let old_loop_kind = self.ctx.loop_kind.replace(LoopKind::While);

        // Synthesize body
        if let Some(body) = while_expr.body() {
            self.synth_block(&body);
        }

        // Restore loop context
        self.ctx.loop_kind = old_loop_kind;

        // While loops always return unit
        self.types.unit()
    }

    fn synth_for(&mut self, for_expr: &ForExpr) -> TypeId {
        debug!("synthesizing for loop");

        // Synthesize iterable and get element type
        // For range expressions, the synthesized type IS the element type
        let elem_ty = if let Some(iterable) = for_expr.iterable() {
            let ty = self.synth_expr(&iterable);

            // Check if iterating over a range expression
            if matches!(iterable, Expr::Range(_)) {
                // Range iteration requires integer element type
                let int_var = self.types.fresh_int_var();
                if self.unify(ty, int_var).is_err() {
                    let span = text_range_to_span(iterable.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("cannot iterate over non-integer range")
                            .with_label(span, "range element type must be an integer"),
                    );
                }
            }

            ty
        } else {
            self.fresh_type_var()
        };

        // Define loop variable binding with the element type
        if let Some(pat) = for_expr.pat() {
            self.define_pattern(&pat, elem_ty);
        }

        // Set loop context for break/continue validation
        let old_loop_kind = self.ctx.loop_kind.replace(LoopKind::For);

        // Synthesize body
        if let Some(body) = for_expr.body() {
            self.synth_block(&body);
        }

        // Restore loop context
        self.ctx.loop_kind = old_loop_kind;

        // For loops always return unit
        self.types.unit()
    }

    fn synth_loop(&mut self, loop_expr: &LoopExpr) -> TypeId {
        debug!("synthesizing loop");

        // Create a fresh type variable for the loop's break value
        let break_ty = self.fresh_type_var();
        let old_break_ty = self.ctx.loop_break_type.replace(break_ty);
        let old_has_break = self.ctx.loop_has_break;
        self.ctx.loop_has_break = false;
        // Set loop context for break/continue validation
        let old_loop_kind = self.ctx.loop_kind.replace(LoopKind::Loop);

        if let Some(body) = loop_expr.body() {
            self.synth_block(&body);
        }

        let has_break = self.ctx.loop_has_break;
        self.ctx.loop_break_type = old_break_ty;
        self.ctx.loop_has_break = old_has_break;
        // Restore loop context
        self.ctx.loop_kind = old_loop_kind;

        // If no break was found, this is an infinite loop - return never type
        // If break with value exists, return that type
        if has_break {
            break_ty
        } else {
            self.types.never()
        }
    }

    fn synth_break(&mut self, break_expr: &BreakExpr) -> TypeId {
        let span = text_range_to_span(break_expr.syntax().text_range());

        // Check if we're inside a loop
        let Some(loop_kind) = self.ctx.loop_kind else {
            self.diagnostics.push(
                Diagnostic::error("break outside of loop")
                    .with_label(span, "`break` can only be used inside a loop"),
            );
            return self.types.never();
        };

        // Mark that we found a break in the current loop
        self.ctx.loop_has_break = true;

        if let Some(value) = break_expr.expr() {
            // Check if break with value is allowed (only in `loop`, not while/for)
            if loop_kind != LoopKind::Loop {
                let value_span = text_range_to_span(value.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("break with value only allowed in `loop`")
                        .with_label(value_span, "break value not allowed here"),
                );
            }

            let value_ty = self.synth_expr(&value);
            if let Some(break_ty) = self.ctx.loop_break_type
                && self.unify(break_ty, value_ty).is_err()
            {
                let value_span = text_range_to_span(value.syntax().text_range());
                let expected = self.type_to_string(self.resolve_type(break_ty));
                let found = self.type_to_string(self.resolve_type(value_ty));
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "type mismatch in break value: expected `{expected}`, found `{found}`"
                    ))
                    .with_label(value_span, format!("expected `{expected}`")),
                );
            }
        } else if let Some(break_ty) = self.ctx.loop_break_type {
            // Break without value - unify with unit
            let unit_ty = self.types.unit();
            let _ = self.unify(break_ty, unit_ty);
        }
        // Break is a diverging expression
        self.types.never()
    }

    fn synth_continue(&mut self, continue_expr: &ContinueExpr) -> TypeId {
        // Check if we're inside a loop
        if self.ctx.loop_kind.is_none() {
            let span = text_range_to_span(continue_expr.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error("continue outside of loop")
                    .with_label(span, "`continue` can only be used inside a loop"),
            );
        }
        // Continue is a diverging expression
        self.types.never()
    }

    fn synth_return(&mut self, return_expr: &ReturnExpr) -> TypeId {
        let value_ty = if let Some(value) = return_expr.expr() {
            self.synth_expr(&value)
        } else {
            self.types.unit()
        };

        if let Some(ret_ty) = self.ctx.return_type
            && self.unify(ret_ty, value_ty).is_err()
        {
            let span = text_range_to_span(return_expr.syntax().text_range());
            let expected = self.type_to_string(self.resolve_type(ret_ty));
            let found = self.type_to_string(self.resolve_type(value_ty));
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "type mismatch in return: expected `{expected}`, found `{found}`"
                ))
                .with_label(span, format!("expected `{expected}`")),
            );
        }

        // Return is a diverging expression
        self.types.never()
    }

    fn synth_yield(&mut self, yield_expr: &YieldExpr) -> TypeId {
        let span = text_range_to_span(yield_expr.syntax().text_range());

        let Some(yield_ty) = self.ctx.block_yield_type else {
            self.diagnostics.push(
                Diagnostic::error("yield outside of block expression")
                    .with_label(span, "`yield` can only be used inside a block expression"),
            );
            return self.types.never();
        };

        self.ctx.block_has_yield = true;

        let value_ty = if let Some(value) = yield_expr.expr() {
            self.synth_expr(&value)
        } else {
            self.types.unit()
        };

        if self.unify(yield_ty, value_ty).is_err() {
            let value_span = yield_expr
                .expr()
                .map(|e| text_range_to_span(e.syntax().text_range()))
                .unwrap_or(span);
            let expected = self.type_to_string(self.resolve_type(yield_ty));
            let found = self.type_to_string(self.resolve_type(value_ty));
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "type mismatch in yield: expected `{expected}`, found `{found}`"
                ))
                .with_label(value_span, format!("expected `{expected}`")),
            );
        }

        self.types.never()
    }

    fn synth_block_expr(&mut self, block_expr: &BlockExpr) -> TypeId {
        match block_expr.block() {
            Some(block) => {
                debug!("synthesizing block expression");
                // Set up yield context for block expressions
                let yield_ty = self.fresh_type_var();
                let old_yield_ty = self.ctx.block_yield_type.replace(yield_ty);
                let old_has_yield = self.ctx.block_has_yield;
                self.ctx.block_has_yield = false;

                let block_ty = self.synth_block(&block);

                let has_yield = self.ctx.block_has_yield;
                self.ctx.block_yield_type = old_yield_ty;
                self.ctx.block_has_yield = old_has_yield;

                // If block has yield, unify yield type with block type
                if has_yield {
                    if self.unify(yield_ty, block_ty).is_err() {
                        let span = text_range_to_span(block_expr.syntax().text_range());
                        let yield_str = self.type_to_string(self.resolve_type(yield_ty));
                        let block_str = self.type_to_string(self.resolve_type(block_ty));
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "type mismatch: yield type `{yield_str}` does not match block tail type `{block_str}`"
                            ))
                            .with_label(span, "yield and tail expression types must match"),
                        );
                    }
                    yield_ty
                } else {
                    block_ty
                }
            }
            None => self.types.unit(),
        }
    }

    /// Convert a type to a string for error messages.
    pub(super) fn type_to_string(&self, type_id: TypeId) -> String {
        let ty = self.types.get(type_id);
        match ty {
            Type::Primitive(prim) => prim.as_str().to_string(),
            Type::Infer(var, kind) => match kind {
                InferKind::General => format!("?{}", var.index()),
                // Show inference variable kind for constrained inference variables
                // This is more helpful for error messages than showing the default
                InferKind::Int => "{integer}".to_string(),
                InferKind::Float => "{float}".to_string(),
            },
            Type::Ref(mutability, inner) => {
                let inner_str = self.type_to_string(*inner);
                match mutability {
                    Mutability::Shared => format!("&{inner_str}"),
                    Mutability::Mutable => format!("&mut {inner_str}"),
                }
            }
            Type::RawPtr(mutability, pointee) => {
                let pointee_str = self.type_to_string(*pointee);
                match mutability {
                    Mutability::Shared => format!("*{pointee_str}"),
                    Mutability::Mutable => format!("*mut {pointee_str}"),
                }
            }
            Type::Array(elem, len) => {
                let elem_str = self.type_to_string(*elem);
                format!("[{elem_str}; {len}]")
            }
            Type::Slice(elem) => {
                let elem_str = self.type_to_string(*elem);
                format!("[{elem_str}]")
            }
            Type::Tuple(elems) => {
                if elems.is_empty() {
                    "()".to_string()
                } else {
                    let elem_strs: Vec<_> = elems.iter().map(|e| self.type_to_string(*e)).collect();
                    format!("({})", elem_strs.join(", "))
                }
            }
            Type::Struct(def_id, _) | Type::Param(def_id) => {
                let symbol = self.resolve_ctx.get_symbol(*def_id);
                self.resolve_ctx.resolve(symbol.name).to_string()
            }
            Type::FnPtr { params, ret } => {
                let param_strs: Vec<_> = params.iter().map(|p| self.type_to_string(*p)).collect();
                let ret_str = self.type_to_string(*ret);
                format!("fn({}) -> {}", param_strs.join(", "), ret_str)
            }
            Type::StrRef => "str".to_string(),
            Type::Error => "<error>".to_string(),
            Type::Alias(_, _) => "<alias>".to_string(),
            Type::SelfType => "Self".to_string(),
            Type::Module(def_id) => {
                let symbol = self.resolve_ctx.get_symbol(*def_id);
                format!("module {}", self.resolve_ctx.resolve(symbol.name))
            }
        }
    }

    /// Convert a `UnifyError` to a rich diagnostic message with expected/actual types.
    fn unify_error_diagnostic(
        &self,
        err: &UnifyError,
        context: &str,
        span: spl_lexer::Span,
    ) -> Diagnostic {
        match err {
            UnifyError::TypeMismatch { expected, actual } => {
                let exp = self.type_to_string(*expected);
                let act = self.type_to_string(*actual);
                Diagnostic::error(format!(
                    "type mismatch in {context}: expected `{exp}`, found `{act}`"
                ))
                .with_label(span, format!("expected `{exp}`, found `{act}`"))
            }
            UnifyError::MutabilityMismatch { expected, .. } => {
                let exp = if *expected == Mutability::Mutable {
                    "mutable reference"
                } else {
                    "shared reference"
                };
                Diagnostic::error(format!("mutability mismatch in {context}: expected {exp}"))
                    .with_label(span, format!("expected {exp}"))
            }
            UnifyError::ArityMismatch { expected, actual } => Diagnostic::error(format!(
                "type mismatch in {context}: expected {expected} elements, found {actual}"
            ))
            .with_label(
                span,
                format!("expected {expected} elements, found {actual}"),
            ),
            UnifyError::ArrayLengthMismatch { expected, actual } => Diagnostic::error(format!(
                "array length mismatch: expected {expected} elements, found {actual}"
            ))
            .with_label(
                span,
                format!("expected {expected} elements, found {actual}"),
            ),
            UnifyError::ConstraintViolation { kind, actual } => {
                let kind_str = match kind {
                    InferKind::Int => "{integer}",
                    InferKind::Float => "{float}",
                    InferKind::General => "type",
                };
                let act = self.type_to_string(*actual);
                Diagnostic::error(format!(
                    "type mismatch in {context}: expected `{kind_str}`, found `{act}`"
                ))
                .with_label(span, format!("expected `{kind_str}`, found `{act}`"))
            }
            UnifyError::InfiniteType { .. } => {
                Diagnostic::error("infinite type").with_label(span, "recursive type here")
            }
        }
    }

    /// Generate a diagnostic for method not found with suggestions.
    fn method_not_found_diagnostic(
        &self,
        method_name: &str,
        type_name: &str,
        available_methods: &[&str],
        span: spl_lexer::Span,
    ) -> Diagnostic {
        use std::fmt::Write;

        // Build the main message with suggestions
        let mut message = format!("method `{method_name}` not found on type `{type_name}`");

        // Suggest similar method name if one exists
        if let Some(similar) = find_similar(method_name, available_methods, 2) {
            let _ = write!(message, "; did you mean `{similar}`?");
        }

        // List available methods if there are any (in the message for test visibility)
        if !available_methods.is_empty() && available_methods.len() <= 10 {
            let _ = write!(
                message,
                " (available methods: {})",
                available_methods.join(", ")
            );
        }

        Diagnostic::error(message).with_label(span, "method not found")
    }

    fn synth_range(&mut self, range: &RangeExpr) -> TypeId {
        let start_ty = range.start().map(|e| self.synth_expr(&e));
        let end_ty = range.end().map(|e| self.synth_expr(&e));

        match (start_ty, end_ty) {
            (Some(s), Some(e)) => {
                // Unify start and end types
                if self.unify(s, e).is_err() {
                    let span = text_range_to_span(range.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(
                            "type mismatch: range start and end must have the same type",
                        )
                        .with_label(span, "mismatched types in range"),
                    );
                }
                s
            }
            (Some(s), None) => s,
            (None, Some(e)) => e,
            (None, None) => {
                // Open range (..) has no type information
                let span = text_range_to_span(range.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("cannot infer type for open range")
                        .with_label(span, "open range `..` requires type context"),
                );
                self.types.error()
            }
        }
    }

    fn synth_is(&mut self, is_expr: &IsExpr) -> TypeId {
        // Synthesize the left-hand side (scrutinee)
        let scrutinee_ty = if let Some(lhs) = is_expr.lhs() {
            self.synth_expr(&lhs)
        } else {
            self.types.error()
        };

        // Check the pattern against the scrutinee type
        if let Some(pat) = is_expr.pattern() {
            self.check_pattern_type(&pat, scrutinee_ty);
        }

        // `is` expressions always return bool
        self.types.bool()
    }

    fn synth_match(&mut self, match_expr: &MatchExpr) -> TypeId {
        let arm_count = match_expr.arms().count();
        debug!(arm_count, "synthesizing match expression");

        // Synthesize the scrutinee type
        let scrutinee_ty = if let Some(scrutinee) = match_expr.scrutinee() {
            self.synth_expr(&scrutinee)
        } else {
            return self.types.error();
        };

        // Collect arm body types
        let mut arm_types = Vec::new();

        for arm in match_expr.arms() {
            // Scope for match arm pattern bindings is already handled by resolver

            // Check and define pattern bindings
            if let Some(pat) = arm.pattern() {
                self.check_pattern_type(&pat, scrutinee_ty);
                self.define_pattern(&pat, scrutinee_ty);
            }

            // Check guard expression if present (must be bool)
            if let Some(guard) = arm.guard() {
                let guard_ty = self.synth_expr(&guard);
                let bool_ty = self.types.bool();
                if self.unify(guard_ty, bool_ty).is_err() {
                    let span = text_range_to_span(guard.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("match guard must be bool, expected bool")
                            .with_label(span, "expected bool"),
                    );
                }
            }

            // Synthesize body type
            if let Some(body) = arm.body() {
                let body_ty = self.synth_expr(&body);
                arm_types.push(body_ty);
            }
        }

        // Unify all arm types
        if arm_types.is_empty() {
            return self.types.unit();
        }

        let result_ty = arm_types[0];
        for (i, &arm_ty) in arm_types.iter().enumerate().skip(1) {
            if self.unify(result_ty, arm_ty).is_err() {
                let span = text_range_to_span(match_expr.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "type mismatch in match arms: arm {} has different type",
                        i + 1
                    ))
                    .with_label(span, "arm types don't match"),
                );
            }
        }

        result_ty
    }

    /// Check that a pattern is compatible with an expected type.
    /// This doesn't define bindings; it just validates the pattern structure.
    fn check_pattern_type(&mut self, pat: &Pat, expected_ty: TypeId) {
        match pat {
            Pat::Literal(lit_pat) => {
                // Check that the literal type matches the expected type
                if let Some(token) = lit_pat.token() {
                    let lit_ty = match token.kind() {
                        SyntaxKind::INT_LITERAL => {
                            let (prim, _) = parse_int_suffix(token.text());
                            if let Some(kind) = prim {
                                self.types.primitive(kind)
                            } else {
                                // Unsuffixed - create inference var that should unify
                                self.fresh_int_var()
                            }
                        }
                        SyntaxKind::FLOAT_LITERAL => {
                            let text = token.text();
                            if text.ends_with("f32") {
                                self.types.primitive(PrimitiveKind::F32)
                            } else if text.ends_with("f64") {
                                self.types.primitive(PrimitiveKind::F64)
                            } else {
                                self.fresh_float_var()
                            }
                        }
                        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW => self.types.bool(),
                        SyntaxKind::CHAR_LITERAL => self.types.primitive(PrimitiveKind::Char),
                        SyntaxKind::STRING_LITERAL => self.types.str_ref(),
                        _ => self.types.error(),
                    };

                    if self.unify(lit_ty, expected_ty).is_err() {
                        let span = text_range_to_span(token.text_range());
                        self.diagnostics.push(
                            Diagnostic::error("type mismatch in pattern")
                                .with_label(span, "pattern type doesn't match scrutinee"),
                        );
                    }
                }
            }
            Pat::Tuple(tuple_pat) => {
                // Check that expected type is a tuple with matching arity
                let resolved = self.resolve_type(expected_ty);
                let ty_data = self.types.get(resolved).clone();
                if let Type::Tuple(elem_types) = ty_data {
                    let patterns: Vec<_> = tuple_pat.patterns().collect();
                    if patterns.len() == elem_types.len() {
                        for (inner_pat, elem_ty) in patterns.iter().zip(elem_types.iter()) {
                            self.check_pattern_type(inner_pat, *elem_ty);
                        }
                    } else {
                        let span = text_range_to_span(tuple_pat.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "tuple pattern has {} elements, but expected {}",
                                patterns.len(),
                                elem_types.len()
                            ))
                            .with_label(span, "wrong number of elements"),
                        );
                    }
                } else {
                    let span = text_range_to_span(tuple_pat.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("tuple pattern used with non-tuple type")
                            .with_label(span, "expected tuple"),
                    );
                }
            }
            Pat::Struct(struct_pat) => {
                // Check that expected type is a struct with matching fields
                let resolved = self.resolve_type(expected_ty);
                let ty_data = self.types.get(resolved).clone();
                if let Type::Struct(struct_id, type_args) = ty_data {
                    // Build substitution map from struct type params to type args
                    let type_params = self
                        .defs
                        .struct_type_params
                        .get(&struct_id)
                        .cloned()
                        .unwrap_or_default();
                    let mut subst: FxHashMap<_, _> = FxHashMap::default();
                    for (param_def_id, type_arg) in type_params.iter().zip(type_args.iter()) {
                        subst.insert(*param_def_id, *type_arg);
                    }

                    // Get struct fields
                    if let Some(fields) = self.defs.struct_fields.get(&struct_id).cloned() {
                        // Check each pattern field against struct field types
                        for pat_field in struct_pat.fields() {
                            if let Some(name_ref) = pat_field.name()
                                && let Some(token) = name_ref.token()
                            {
                                let field_name = token.text().to_string();
                                // Find matching struct field
                                if let Some((_, field_ty, _)) =
                                    fields.iter().find(|(name, _, _)| name == &field_name)
                                {
                                    // Substitute type params with type args
                                    let instantiated_ty =
                                        self.substitute_type_params(*field_ty, &subst);
                                    // Recursively check nested pattern
                                    if let Some(nested_pat) = pat_field.pat() {
                                        self.check_pattern_type(&nested_pat, instantiated_ty);
                                    }
                                }
                                // Note: Missing fields are a resolver error, not type check error
                            }
                        }
                    }
                } else {
                    let span = text_range_to_span(struct_pat.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("struct pattern used with non-struct type")
                            .with_label(span, "expected struct"),
                    );
                }
            }
            Pat::Ref(ref_pat) => {
                // Check that expected type is a reference
                let resolved = self.resolve_type(expected_ty);
                let ty_data = self.types.get(resolved).clone();
                if let Type::Ref(_, inner_ty) = ty_data {
                    if let Some(inner_pat) = ref_pat.pat() {
                        self.check_pattern_type(&inner_pat, inner_ty);
                    }
                } else {
                    let span = text_range_to_span(ref_pat.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("reference pattern used with non-reference type")
                            .with_label(span, "expected reference"),
                    );
                }
            }
            // Binding patterns, wildcards, and other patterns (Rest, Range, Slice)
            // are always compatible or not fully implemented
            _ => {}
        }
    }

    pub(super) fn synth_block(&mut self, block: &Block) -> TypeId {
        // Track if the block diverges
        let mut diverges = false;
        // Track if we've warned about unreachable code in this block
        let mut warned_unreachable = false;

        // Process all children in source order (statements and bare expressions)
        // This is important because bare expressions (like `while` without semicolon)
        // must be processed in order with surrounding statements.
        for child in block.syntax().children() {
            // Try to cast as a statement first
            if let Some(stmt) = Stmt::cast(child.clone()) {
                // Check for unreachable code
                if diverges && !warned_unreachable {
                    let span = text_range_to_span(child.text_range());
                    self.diagnostics.push(
                        Diagnostic::error("unreachable code")
                            .with_label(span, "unreachable statement"),
                    );
                    warned_unreachable = true;
                }

                match &stmt {
                    Stmt::Expr(expr_stmt) => {
                        if let Some(expr) = expr_stmt.expr() {
                            let ty = self.synth_expr(&expr);
                            // Check if this expression has the never type
                            let resolved = self.resolve_type(ty);
                            let inner = self.types.get(resolved);
                            if matches!(inner, Type::Primitive(PrimitiveKind::Never)) {
                                diverges = true;
                            }
                        }
                    }
                    Stmt::Let(let_stmt) => {
                        let stmt_diverges = self.infer_let_stmt(let_stmt);
                        if stmt_diverges {
                            diverges = true;
                        }
                    }
                }
            } else if let Some(expr) = Expr::cast(child.clone()) {
                // Bare expression (not wrapped in ExprStmt)
                // Check for unreachable code
                if diverges && !warned_unreachable {
                    let span = text_range_to_span(child.text_range());
                    self.diagnostics.push(
                        Diagnostic::error("unreachable code")
                            .with_label(span, "unreachable expression"),
                    );
                    warned_unreachable = true;
                }

                let ty = self.synth_expr(&expr);
                let resolved = self.resolve_type(ty);
                let inner = self.types.get(resolved);
                if matches!(inner, Type::Primitive(PrimitiveKind::Never)) {
                    diverges = true;
                }
            }
        }

        // The block's type is the tail expression's type, or unit if none
        let result = if let Some(tail) = block.tail_expr() {
            self.synth_expr(&tail)
        } else if diverges {
            // If the block diverges, its type is never
            self.types.never()
        } else {
            self.types.unit()
        };

        // Postcondition: if block diverges and has no tail, result must be never type
        #[cfg(debug_assertions)]
        if diverges && block.tail_expr().is_none() {
            let resolved = self.resolve_type(result);
            debug_assert!(
                matches!(
                    self.types.get(resolved),
                    Type::Primitive(PrimitiveKind::Never)
                ),
                "postcondition: diverging block without tail must return never type"
            );
        }

        result
    }

    // =========================================================================
    // Type Checking (Top-down)
    // =========================================================================

    /// Check an expression against an expected type.
    pub(super) fn check_expr(&mut self, expr: &Expr, expected: TypeId) {
        let actual = self.synth_expr(expr);
        // unify(expected, actual): checks if actual can satisfy expected
        // Coercion allows &mut T to satisfy &T, but not vice versa
        if let Err(err) = self.unify(expected, actual) {
            let span = text_range_to_span(expr.syntax().text_range());
            let diag = self.unify_error_diagnostic(&err, "expression", span);
            self.diagnostics.push(diag);
        } else {
            // After successful unification, validate integer literal ranges
            self.validate_literal_range(expr, expected);
        }
    }

    /// Validate that an integer literal is in range for its resolved type.
    fn validate_literal_range(&mut self, expr: &Expr, expected: TypeId) {
        // Extract the literal value from the expression, handling negation
        let Some((value, span)) = self.extract_int_literal_value(expr) else {
            return;
        };

        // Get the resolved type
        let resolved = self.resolve_type(expected);
        let ty = self.types.get(resolved).clone();

        // Validate if it's a concrete integer type
        if let Type::Primitive(kind) = ty
            && let Err(msg) = kind.validate_int_literal_range(value)
        {
            self.diagnostics
                .push(Diagnostic::error(&msg).with_label(span, "literal out of range"));
        }
    }

    /// Extract an integer literal value from an expression, handling negation.
    /// Returns (value, span) if the expression is an integer literal or negated integer literal.
    fn extract_int_literal_value(&self, expr: &Expr) -> Option<(i128, spl_lexer::Span)> {
        match expr {
            Expr::Literal(lit) => {
                let token = lit.token()?;
                if token.kind() != SyntaxKind::INT_LITERAL {
                    return None;
                }
                let text = token.text();
                // Skip suffixed literals - they're validated in synth_literal
                if parse_int_suffix(text).1 {
                    return None;
                }
                let value = parse_int_literal_value(text)?;
                let span = text_range_to_span(token.text_range());
                Some((value, span))
            }
            Expr::Prefix(prefix) => {
                let op = prefix.op_token()?;
                if op.kind() != SyntaxKind::MINUS {
                    return None;
                }
                let inner = prefix.expr()?;
                let (inner_value, _) = self.extract_int_literal_value(&inner)?;
                // For negation, we report the span of the whole prefix expression
                let span = text_range_to_span(expr.syntax().text_range());
                Some((-inner_value, span))
            }
            Expr::Paren(paren) => {
                let inner = paren.expr()?;
                self.extract_int_literal_value(&inner)
            }
            _ => None,
        }
    }

    // =========================================================================
    // Statement Inference
    // =========================================================================

    /// Infer types for a let statement. Returns true if the initializer diverges.
    pub(super) fn infer_let_stmt(&mut self, let_stmt: &LetStmt) -> bool {
        // Get the type annotation if present
        let annotation_ty = let_stmt.ty().map(|ty| self.ast_type_to_type_id(&ty));

        // Synthesize or check the initializer
        let (init_ty, diverges) = if let Some(init) = let_stmt.initializer() {
            let ty = if let Some(expected) = annotation_ty {
                self.check_expr(&init, expected);
                expected
            } else {
                self.synth_expr(&init)
            };

            // Check if initializer diverges (e.g., let x = return 42;)
            let resolved = self.resolve_type(ty);
            let inner = self.types.get(resolved);
            let diverges = matches!(inner, Type::Primitive(PrimitiveKind::Never));

            (ty, diverges)
        } else {
            // No initializer - use annotation or error
            (
                annotation_ty.unwrap_or_else(|| self.fresh_type_var()),
                false,
            )
        };

        // Bind the pattern
        if let Some(pat) = let_stmt.pat() {
            self.define_pattern(&pat, init_ty);
        }

        diverges
    }

    pub(super) fn define_pattern(&mut self, pat: &Pat, ty: TypeId) {
        match pat {
            Pat::Ident(ident_pat) => {
                // Get the DefId from the resolution
                let token = ident_pat.name().and_then(|n| n.ident_token()).or_else(|| {
                    use spl_ast::token;
                    token(ident_pat.syntax(), SyntaxKind::IDENT)
                });

                if let Some(token) = token {
                    let span = text_range_to_span(token.text_range());
                    // The resolver already defined this binding, we just need to record its type
                    if let Some(&def_id) = self.resolutions.get(&span) {
                        self.results.binding_types.insert(def_id, ty);
                    }
                    // If not in resolutions, the resolver didn't define it (error already reported)
                }
            }
            Pat::Tuple(tuple_pat) => {
                let resolved = self.resolve_type(ty);
                let ty_data = self.types.get(resolved).clone();
                if let Type::Tuple(elem_types) = ty_data {
                    for (inner_pat, elem_ty) in tuple_pat.patterns().zip(elem_types.iter()) {
                        self.define_pattern(&inner_pat, *elem_ty);
                    }
                }
            }
            Pat::Struct(struct_pat) => {
                // Define bindings for struct pattern fields
                let resolved = self.resolve_type(ty);
                let ty_data = self.types.get(resolved).clone();
                if let Type::Struct(struct_id, type_args) = ty_data {
                    // Build substitution map from struct type params to type args
                    let type_params = self
                        .defs
                        .struct_type_params
                        .get(&struct_id)
                        .cloned()
                        .unwrap_or_default();
                    let mut subst: FxHashMap<_, _> = FxHashMap::default();
                    for (param_def_id, type_arg) in type_params.iter().zip(type_args.iter()) {
                        subst.insert(*param_def_id, *type_arg);
                    }

                    // Get struct fields
                    if let Some(fields) = self.defs.struct_fields.get(&struct_id).cloned() {
                        // Define bindings for each pattern field
                        for pat_field in struct_pat.fields() {
                            if let Some(name_ref) = pat_field.name()
                                && let Some(token) = name_ref.token()
                            {
                                let field_name = token.text().to_string();
                                // Find matching struct field
                                if let Some((_, field_ty, _)) =
                                    fields.iter().find(|(name, _, _)| name == &field_name)
                                {
                                    // Substitute type params with type args
                                    let instantiated_ty =
                                        self.substitute_type_params(*field_ty, &subst);
                                    // Recursively define bindings for nested pattern
                                    if let Some(nested_pat) = pat_field.pat() {
                                        self.define_pattern(&nested_pat, instantiated_ty);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Wildcard and other patterns don't bind anything
            _ => {}
        }
    }

    // =========================================================================
    // AST Type to TypeId Conversion
    // =========================================================================

    pub(super) fn ast_type_to_type_id(&mut self, ty: &spl_ast::Type) -> TypeId {
        let span = text_range_to_span(ty.syntax().text_range());
        let type_id = self.ast_type_to_type_id_inner(ty);
        self.results.type_annotation_types.insert(span, type_id);
        type_id
    }

    /// Internal helper for type conversion without recording (used recursively).
    fn ast_type_to_type_id_inner(&mut self, ty: &spl_ast::Type) -> TypeId {
        match ty {
            spl_ast::Type::Path(path_type) => {
                let path = path_type.path();
                if let Some(path) = path {
                    let segment = path.segments().next();
                    if let Some(segment) = segment {
                        let name_ref = segment.name();
                        if let Some(name_ref) = name_ref {
                            let token = name_ref.token();
                            if let Some(token) = token {
                                let name = token.text();

                                // Handle Self type
                                if token.kind() == SyntaxKind::SELF_TYPE_KW || name == "Self" {
                                    if let Some(self_ty) = self.ctx.self_type {
                                        return self_ty;
                                    }
                                    // Self used outside impl block - emit diagnostic and return error
                                    let span = text_range_to_span(token.text_range());
                                    self.diagnostics.push(
                                        Diagnostic::error(
                                            "`Self` is only valid inside impl blocks",
                                        )
                                        .with_label(span, "not inside an impl block"),
                                    );
                                    return self.types.error();
                                }

                                // Check for str (string reference type) BEFORE primitives
                                // because str is in PrimitiveKind but we want StrRef for type annotations
                                if name == "str" {
                                    return self.types.str_ref();
                                }

                                // Check for primitive types
                                if let Some(prim) = PrimitiveKind::from_name(name) {
                                    return self.types.primitive(prim);
                                }

                                // Look up in resolutions
                                let span = text_range_to_span(token.text_range());
                                if let Some(&def_id) = self.resolutions.get(&span) {
                                    // Check if it's a type parameter or a struct
                                    let symbol = self.resolve_ctx.get_symbol(def_id);
                                    if symbol.kind == SymbolKind::TypeParam {
                                        return self.types.mk_param(def_id);
                                    }
                                    // It's a struct or type alias - parse generic arguments
                                    let type_args: Vec<TypeId> = segment
                                        .generic_args()
                                        .map(|args| {
                                            args.args()
                                                .map(|t| self.ast_type_to_type_id_inner(&t))
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    return self.types.mk_struct(def_id, type_args);
                                }
                            }
                        }
                    }
                }
                self.types.error()
            }
            spl_ast::Type::Ref(ref_type) => {
                let mutability = if ref_type.mut_kw().is_some() {
                    Mutability::Mutable
                } else {
                    Mutability::Shared
                };
                if let Some(inner) = ref_type.ty() {
                    let inner_ty = self.ast_type_to_type_id_inner(&inner);
                    self.types.mk_ref(mutability, inner_ty)
                } else {
                    self.types.error()
                }
            }
            spl_ast::Type::Array(array_type) => {
                if let Some(elem_ty) = array_type.elem_ty() {
                    let elem = self.ast_type_to_type_id_inner(&elem_ty);
                    // Get length from expression
                    let len = if let Some(len_expr) = array_type.len() {
                        // Try to evaluate as a constant
                        if let Expr::Literal(lit) = Expr::cast(len_expr.syntax().clone())
                            .unwrap_or_else(|| panic!("expected literal"))
                        {
                            if let Some(token) = lit.token() {
                                if token.kind() == SyntaxKind::INT_LITERAL {
                                    token.text().parse().unwrap_or(0)
                                } else {
                                    0
                                }
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    } else {
                        0
                    };
                    self.types.mk_array(elem, len)
                } else {
                    self.types.error()
                }
            }
            spl_ast::Type::Slice(slice_type) => {
                if let Some(elem_ty) = slice_type.elem_ty() {
                    let elem = self.ast_type_to_type_id_inner(&elem_ty);
                    self.types.mk_slice(elem)
                } else {
                    self.types.error()
                }
            }
            spl_ast::Type::Tuple(tuple_type) => {
                let elems: Vec<_> = tuple_type
                    .types()
                    .map(|t| self.ast_type_to_type_id_inner(&t))
                    .collect();
                self.types.mk_tuple(elems)
            }
            spl_ast::Type::FnPtr(fn_ptr) => {
                let params: Vec<_> = fn_ptr
                    .param_types()
                    .map(|t| self.ast_type_to_type_id_inner(&t))
                    .collect();
                let ret = fn_ptr
                    .ret_type()
                    .map(|t| self.ast_type_to_type_id_inner(&t))
                    .unwrap_or_else(|| self.types.unit());
                self.types.mk_fn_ptr(params, ret)
            }
            spl_ast::Type::Never(_) => self.types.never(),
            spl_ast::Type::Optional(opt) => {
                // Optional(T) desugars to Option(T: T) — for now, treat as error
                // until Option is defined in the standard library
                if let Some(inner) = opt.ty() {
                    let _inner_ty = self.ast_type_to_type_id_inner(&inner);
                }
                self.types.error()
            }
        }
    }
}
