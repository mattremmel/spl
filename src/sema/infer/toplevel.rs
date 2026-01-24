//! Top-level type inference for source files and functions.

use crate::ast::{Expr, FunctionDef, Item, SourceFile, WhereClause};
use crate::diagnostic::Diagnostic;
use crate::sema::symbol::DefId;
use crate::sema::types::{Mutability, PrimitiveKind, Type, TypeId};
use rowan::ast::AstNode;
use rustc_hash::FxHashSet;

use super::engine::{FnSignature, InferEngine, LoopKind, ParamInfo};
use super::helpers::text_range_to_span;
use super::{SelfParam, SelfParamKind};

impl InferEngine {
    // =========================================================================
    // Top-Level Inference
    // =========================================================================

    pub(super) fn infer_source_file(&mut self, source_file: &SourceFile) {
        // First pass: collect function signatures and struct info
        for item in source_file.items() {
            match &item {
                Item::Function(func) => self.collect_function_signature(func),
                Item::Struct(struct_def) => self.collect_struct_info(struct_def),
                Item::TypeAlias(type_alias) => self.collect_type_alias_info(type_alias),
                Item::Impl(impl_block) => {
                    // Get the struct this impl is for
                    let struct_def_id = self.get_impl_struct_def_id(impl_block);

                    // Collect impl block type parameters from where clause
                    let mut impl_type_params = Vec::new();
                    if let Some(where_clause) = impl_block.where_clause() {
                        self.collect_type_params_from_where(&where_clause, &mut impl_type_params);
                    }

                    // Create type args from impl type params (as Type::Param)
                    let type_args: Vec<TypeId> = impl_type_params
                        .iter()
                        .map(|&def_id| self.ctx.types.mk_param(def_id))
                        .collect();

                    // Create the struct type with type args
                    let struct_ty =
                        struct_def_id.map(|id| self.ctx.types.mk_struct(id, type_args.clone()));

                    // Set current_self_type so that `Self` in signatures resolves correctly
                    self.current_self_type = struct_ty;

                    for item in impl_block.items() {
                        if let Item::Function(func) = item {
                            self.collect_function_signature(&func);

                            // Register this method with its struct and update self_ty
                            if let Some(struct_id) = struct_def_id
                                && let Some(method_def_id) = self.get_function_def_id(&func)
                            {
                                self.struct_methods
                                    .entry(struct_id)
                                    .or_default()
                                    .push(method_def_id);

                                // Update self_ty in the method signature
                                if let Some(sig) = self.fn_signatures.get_mut(&method_def_id)
                                    && let Some(ref mut sp) = sig.self_param
                                    && let Some(sty) = struct_ty
                                {
                                    // Apply the appropriate wrapper based on receiver kind
                                    sp.self_ty = match sp.kind {
                                        SelfParamKind::Ref => {
                                            self.ctx.types.mk_ref(Mutability::Shared, sty)
                                        }
                                        SelfParamKind::RefMut => {
                                            self.ctx.types.mk_ref(Mutability::Mutable, sty)
                                        }
                                        SelfParamKind::Owned => sty,
                                    };
                                }

                                // Add impl type params to method signature
                                if let Some(sig) = self.fn_signatures.get_mut(&method_def_id) {
                                    for &param_def_id in impl_type_params.iter().rev() {
                                        if !sig.type_params.contains(&param_def_id) {
                                            sig.type_params.insert(0, param_def_id);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Clear current_self_type after processing impl block
                    self.current_self_type = None;
                }
            }
        }

        // Check for recursive types (infinite size structs)
        self.check_recursive_types();

        // Check for type alias cycles
        self.check_type_alias_cycles();

        // Second pass: infer function bodies
        for item in source_file.items() {
            match &item {
                Item::Function(func) => self.infer_function(func),
                Item::Impl(impl_block) => {
                    for item in impl_block.items() {
                        if let Item::Function(func) = item {
                            self.infer_function(&func);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn collect_function_signature(&mut self, func: &FunctionDef) {
        let name = match func.name() {
            Some(n) => n,
            None => return,
        };

        let token = match name.ident_token() {
            Some(t) => t,
            None => return,
        };

        let span = text_range_to_span(token.text_range());
        let def_id = match self.resolutions.get(&span) {
            Some(id) => *id,
            None => return,
        };

        // Collect type parameters from where clause
        let mut type_params = Vec::new();
        if let Some(where_clause) = func.where_clause() {
            self.collect_type_params_from_where(&where_clause, &mut type_params);
        }

        // Collect parameters
        let mut params = Vec::new();
        let mut self_param = None;
        if let Some(param_list) = func.param_list() {
            // Handle self parameter
            if let Some(sp) = param_list.self_param() {
                let kind = if sp.mut_kw().is_some() {
                    SelfParamKind::RefMut
                } else if sp.amp().is_some() {
                    SelfParamKind::Ref
                } else {
                    SelfParamKind::Owned
                };
                // Self type will be resolved when we have the impl block's type
                self_param = Some(SelfParam {
                    kind,
                    self_ty: self.fresh_type_var(),
                });
            }

            for param in param_list.params() {
                let param_name = param
                    .name()
                    .and_then(|n| n.ident_token())
                    .map(|t| t.text().to_string())
                    .unwrap_or_default();
                let param_ty = param
                    .ty()
                    .map(|t| self.ast_type_to_type_id(&t))
                    .unwrap_or_else(|| self.fresh_type_var());
                // Get external label (None if `_`, explicit label, or defaults to param name)
                let label = param.external_label();
                params.push(ParamInfo {
                    label,
                    name: param_name,
                    ty: param_ty,
                });
            }
        }

        // Get return type
        let ret = func
            .ret_type()
            .map(|t| self.ast_type_to_type_id(&t))
            .unwrap_or_else(|| self.ctx.types.unit());

        self.fn_signatures.insert(
            def_id,
            FnSignature {
                self_param,
                type_params,
                params,
                ret,
            },
        );
    }

    /// Check for recursive types (structs that contain themselves without indirection).
    fn check_recursive_types(&mut self) {
        // Build a dependency graph: struct -> structs it directly contains (not via reference)
        let struct_ids: Vec<DefId> = self.struct_fields.keys().copied().collect();

        for &struct_id in &struct_ids {
            // Check if this struct is part of a cycle using DFS
            let mut visited = FxHashSet::default();
            let mut in_progress = FxHashSet::default();
            let mut path = Vec::new();

            if self.has_recursive_type(struct_id, &mut visited, &mut in_progress, &mut path) {
                // Found a cycle - report error
                let symbol = self.ctx.get_symbol(struct_id);
                let name = self.ctx.resolve(symbol.name);
                self.diagnostics.push(
                    Diagnostic::error(format!("recursive type `{}` has infinite size", name))
                        .with_label(symbol.span.clone(), "recursive without indirection"),
                );
            }
        }
    }

    /// Check if a struct type is part of a recursive cycle.
    /// Returns true if a cycle is detected.
    fn has_recursive_type(
        &self,
        struct_id: DefId,
        visited: &mut FxHashSet<DefId>,
        in_progress: &mut FxHashSet<DefId>,
        path: &mut Vec<DefId>,
    ) -> bool {
        if in_progress.contains(&struct_id) {
            // Found a cycle
            return true;
        }
        if visited.contains(&struct_id) {
            // Already checked, no cycle
            return false;
        }

        in_progress.insert(struct_id);
        path.push(struct_id);

        // Check all fields of this struct
        if let Some(fields) = self.struct_fields.get(&struct_id) {
            for (_, field_ty) in fields {
                // Get the directly contained struct types (not through references)
                if let Some(contained_id) = self.get_direct_struct_dependency(*field_ty)
                    && self.has_recursive_type(contained_id, visited, in_progress, path)
                {
                    return true;
                }
            }
        }

        in_progress.remove(&struct_id);
        path.pop();
        visited.insert(struct_id);
        false
    }

    /// Get the struct DefId if this type directly contains a struct (not through a reference).
    /// Returns None if the type is a reference, primitive, or other non-struct type.
    fn get_direct_struct_dependency(&self, type_id: TypeId) -> Option<DefId> {
        let ty = self.ctx.types.get(type_id);
        match ty {
            Type::Struct(def_id, _) => Some(*def_id),
            Type::Ref(_, _) => None, // References break the cycle
            Type::Array(elem, _) => self.get_direct_struct_dependency(*elem),
            Type::Tuple(elems) => {
                // Check if any tuple element contains a struct directly
                for elem in elems {
                    if let Some(id) = self.get_direct_struct_dependency(*elem) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Collect type alias information.
    fn collect_type_alias_info(&mut self, type_alias: &crate::ast::TypeAlias) {
        let name = match type_alias.name() {
            Some(n) => n,
            None => return,
        };

        let token = match name.ident_token() {
            Some(t) => t,
            None => return,
        };

        let span = text_range_to_span(token.text_range());
        let def_id = match self.resolutions.get(&span) {
            Some(id) => *id,
            None => return,
        };

        // Get the target type
        if let Some(ty) = type_alias.ty() {
            let target_ty = self.ast_type_to_type_id(&ty);
            self.type_alias_targets.insert(def_id, target_ty);
        }
    }

    /// Check for cyclic type alias definitions.
    fn check_type_alias_cycles(&mut self) {
        let alias_ids: Vec<DefId> = self.type_alias_targets.keys().copied().collect();
        let mut cyclic_aliases = Vec::new();

        for &alias_id in &alias_ids {
            let mut visited = FxHashSet::default();
            let mut in_progress = FxHashSet::default();

            if self.has_alias_cycle(alias_id, &mut visited, &mut in_progress) {
                let symbol = self.ctx.get_symbol(alias_id);
                let name = self.ctx.resolve(symbol.name);
                self.diagnostics.push(
                    Diagnostic::error(format!("cyclic type alias definition for `{}`", name))
                        .with_label(symbol.span.clone(), "cyclic reference"),
                );
                cyclic_aliases.push(alias_id);
            }
        }

        // Replace cyclic alias targets with Error to prevent infinite recursion in resolve_type
        let error_ty = self.ctx.types.error();
        for alias_id in cyclic_aliases {
            self.type_alias_targets.insert(alias_id, error_ty);
        }
    }

    /// Check if a type alias is part of a cycle.
    fn has_alias_cycle(
        &self,
        alias_id: DefId,
        visited: &mut FxHashSet<DefId>,
        in_progress: &mut FxHashSet<DefId>,
    ) -> bool {
        if in_progress.contains(&alias_id) {
            return true;
        }
        if visited.contains(&alias_id) {
            return false;
        }

        in_progress.insert(alias_id);

        // Get the target type for this alias
        if let Some(&target_ty) = self.type_alias_targets.get(&alias_id) {
            // Check if the target references another alias
            if let Some(referenced_alias) = self.get_referenced_alias(target_ty)
                && self.has_alias_cycle(referenced_alias, visited, in_progress)
            {
                return true;
            }
        }

        in_progress.remove(&alias_id);
        visited.insert(alias_id);
        false
    }

    /// Get the alias DefId if this type directly references a type alias.
    /// Also traverses arrays and tuples to find aliases in compound types.
    fn get_referenced_alias(&self, type_id: TypeId) -> Option<DefId> {
        let ty = self.ctx.types.get(type_id);
        match ty {
            // mk_struct is used for both structs and type aliases
            // Check if the DefId is actually a type alias
            Type::Struct(def_id, _) => {
                if self.type_alias_targets.contains_key(def_id) {
                    Some(*def_id)
                } else {
                    None
                }
            }
            // Type::Alias is also used for type aliases
            Type::Alias(def_id, _) => {
                if self.type_alias_targets.contains_key(def_id) {
                    Some(*def_id)
                } else {
                    None
                }
            }
            // Traverse array element types
            Type::Array(elem_id, _) => self.get_referenced_alias(*elem_id),
            // Traverse tuple field types
            Type::Tuple(fields) => fields.iter().find_map(|f| self.get_referenced_alias(*f)),
            _ => None,
        }
    }

    fn collect_struct_info(&mut self, struct_def: &crate::ast::StructDef) {
        let name = match struct_def.name() {
            Some(n) => n,
            None => return,
        };

        let token = match name.ident_token() {
            Some(t) => t,
            None => return,
        };

        let span = text_range_to_span(token.text_range());
        let def_id = match self.resolutions.get(&span) {
            Some(id) => *id,
            None => return,
        };

        // Collect type parameters from where clause
        let mut type_params = Vec::new();
        if let Some(where_clause) = struct_def.where_clause() {
            self.collect_type_params_from_where(&where_clause, &mut type_params);
        }
        self.struct_type_params.insert(def_id, type_params);

        let mut fields = Vec::new();
        if let Some(field_list) = struct_def.field_list() {
            for field in field_list.fields() {
                let field_name = field
                    .name()
                    .and_then(|n| n.ident_token())
                    .map(|t| t.text().to_string())
                    .unwrap_or_default();
                let field_ty = field
                    .ty()
                    .map(|t| self.ast_type_to_type_id(&t))
                    .unwrap_or_else(|| self.fresh_type_var());
                fields.push((field_name, field_ty));
            }
        }

        self.struct_fields.insert(def_id, fields);
    }

    /// Get the struct DefId for an impl block.
    fn get_impl_struct_def_id(&self, impl_block: &crate::ast::ImplBlock) -> Option<DefId> {
        let ty = impl_block.self_ty()?;
        // For a path type like `impl S`, get the struct's DefId
        if let crate::ast::Type::Path(path_type) = ty {
            let path = path_type.path()?;
            let segment = path.segments().next()?;
            let name_ref = segment.name()?;
            let token = name_ref.ident_token()?;
            let span = text_range_to_span(token.text_range());
            self.resolutions.get(&span).copied()
        } else {
            None
        }
    }

    /// Get the DefId for a function definition.
    fn get_function_def_id(&self, func: &FunctionDef) -> Option<DefId> {
        let name = func.name()?;
        let token = name.ident_token()?;
        let span = text_range_to_span(token.text_range());
        self.resolutions.get(&span).copied()
    }

    /// Collect type parameters from a where clause.
    fn collect_type_params_from_where(
        &self,
        where_clause: &WhereClause,
        type_params: &mut Vec<DefId>,
    ) {
        for param in where_clause.type_params() {
            if let Some(name) = param.name()
                && let Some(token) = name.ident_token()
            {
                let span = text_range_to_span(token.text_range());
                if let Some(&param_def_id) = self.resolutions.get(&span) {
                    type_params.push(param_def_id);
                }
            }
        }
    }

    fn infer_function(&mut self, func: &FunctionDef) {
        let name = match func.name() {
            Some(n) => n,
            None => return,
        };

        let token = match name.ident_token() {
            Some(t) => t,
            None => return,
        };

        let span = text_range_to_span(token.text_range());
        let def_id = match self.resolutions.get(&span) {
            Some(id) => *id,
            None => return,
        };

        // Get signature
        let sig = match self.fn_signatures.get(&def_id) {
            Some(s) => s.clone(),
            None => return,
        };

        // Set current return type
        self.current_return_type = Some(sig.ret);

        // Bind parameters by looking up their DefIds from the AST
        if let Some(param_list) = func.param_list() {
            // Handle self parameter if present
            if let Some(self_param) = param_list.self_param() {
                // Find the impl block's struct type from the function's context
                if let Some(self_param_info) = &sig.self_param {
                    // Get the self keyword's span and look up its DefId
                    if let Some(self_token) = self_param.self_kw() {
                        let self_span = text_range_to_span(self_token.text_range());
                        if let Some(&self_def_id) = self.resolutions.get(&self_span) {
                            // Determine self type based on receiver kind
                            let self_ty = self_param_info.self_ty;
                            self.binding_types.insert(self_def_id, self_ty);
                        }
                    }
                }
            }

            // Bind regular parameters
            let param_types: Vec<_> = sig.params.iter().map(|p| p.ty).collect();

            for (param, param_ty) in param_list.params().zip(param_types.iter()) {
                if let Some(param_name) = param.name()
                    && let Some(token) = param_name.ident_token()
                {
                    let param_span = text_range_to_span(token.text_range());
                    if let Some(&param_def_id) = self.resolutions.get(&param_span) {
                        self.binding_types.insert(param_def_id, *param_ty);
                    }
                }
            }
        }

        // Infer body
        if let Some(body) = func.body() {
            let body_ty = self.synth_block(&body);

            // Check for implicit return with statements (disallowed)
            let has_statements = body.statements().next().is_some();
            let has_tail = body.tail_expr().is_some();

            if has_statements && has_tail {
                let resolved_ret = self.resolve_type(sig.ret);
                let ret_is_non_unit = !self.is_unit_type(resolved_ret);

                if ret_is_non_unit {
                    // Check if tail is already an explicit return expression
                    if let Some(tail_expr) = body.tail_expr()
                        && !matches!(tail_expr, Expr::Return(_))
                    {
                        let span = text_range_to_span(tail_expr.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error(
                                "implicit return not allowed when function body contains statements",
                            )
                            .with_label(span, "add `return` here"),
                        );
                    }
                }
            }

            // Check return type matches
            if !self.unify(sig.ret, body_ty) {
                let body_span = text_range_to_span(body.syntax().text_range());

                // Check if this is a "missing return" case:
                // - Expected return type is non-unit
                // - Body type is unit (no tail expression, no explicit return)
                let resolved_ret = self.resolve_type(sig.ret);
                let resolved_body = self.resolve_type(body_ty);

                let ret_is_non_unit = !self.is_unit_type(resolved_ret);
                let body_is_unit = self.is_unit_type(resolved_body);

                if ret_is_non_unit && body_is_unit {
                    self.diagnostics.push(
                        Diagnostic::error("not all code paths return a value")
                            .with_label(body_span, "missing return"),
                    );
                } else {
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch: return type doesn't match body")
                            .with_label(body_span, "body has wrong type"),
                    );
                }
            }
        }

        self.current_return_type = None;
    }

    /// Check if a type is the unit type (either Primitive::Unit or empty tuple)
    fn is_unit_type(&self, type_id: TypeId) -> bool {
        match self.ctx.types.get(type_id) {
            Type::Primitive(PrimitiveKind::Unit) => true,
            Type::Tuple(elems) => elems.is_empty(),
            _ => false,
        }
    }
}

// Suppress unused warning for LoopKind - it's used in synth.rs
#[allow(dead_code)]
fn _use_loop_kind(_: LoopKind) {}
