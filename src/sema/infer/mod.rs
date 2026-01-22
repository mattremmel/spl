//! Bidirectional type inference for SPL.
//!
//! This module implements a bidirectional type inference algorithm that:
//! - Synthesizes types bottom-up from expressions
//! - Checks types top-down from expected types
//! - Unifies type constraints to resolve inference variables

use crate::ast::{
    ArrayExpr, BinExpr, Block, BlockExpr, BreakExpr, CallExpr, CastExpr, ContinueExpr, Expr,
    FieldExpr, ForExpr, FunctionDef, IfExpr, IndexExpr, Item, LetStmt, LiteralExpr, LoopExpr,
    MethodCallExpr, ParenExpr, Pat, PathExpr, PrefixExpr, RangeExpr, RefExpr, ReturnExpr,
    SliceExpr, SourceFile, Stmt, StructExpr, TupleExpr, WhileExpr,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{LoweredExpr, lower::try_lower_expr};
use crate::lexer::Span;
use crate::sema::resolver::ResolveResult;
use crate::sema::symbol::DefId;
use crate::sema::types::{InferKind, Mutability, PrimitiveKind, Type, TypeId, TypeVar};
use crate::sema::{SemanticContext, SymbolKind};
use crate::syntax::SyntaxKind;
use rowan::ast::AstNode;
use rustc_hash::FxHashMap;

#[cfg(test)]
mod tests;

/// Result of type inference.
pub struct InferResult {
    /// The semantic context with symbol table and types.
    pub ctx: SemanticContext,
    /// Map from expression spans to their inferred types.
    pub expr_types: FxHashMap<Span, TypeId>,
    /// Map from local bindings (DefId) to their inferred types.
    pub binding_types: FxHashMap<DefId, TypeId>,
    /// Map from method call expression spans to their resolved method DefIds.
    pub method_resolutions: FxHashMap<Span, DefId>,
    /// Diagnostics produced during inference.
    pub diagnostics: Vec<Diagnostic>,
}

impl InferResult {
    /// Display the type of the last let binding in the source (by position).
    /// Used for testing.
    pub fn display_first_binding(&self) -> String {
        // Find the last binding by source position (largest span start)
        let mut best: Option<(DefId, TypeId, usize)> = None;

        for (&def_id, &type_id) in &self.binding_types {
            let symbol = self.ctx.get_symbol(def_id);
            // Skip built-in primitives (they have span 0..0)
            if symbol.span == (0..0) {
                continue;
            }
            // Skip functions
            if symbol.kind == SymbolKind::Function {
                continue;
            }
            let span_start = symbol.span.start;
            match &best {
                Some((_, _, best_start)) if span_start <= *best_start => {}
                _ => {
                    best = Some((def_id, type_id, span_start));
                }
            }
        }

        match best {
            Some((_, type_id, _)) => self.type_to_string(type_id),
            None => "???".to_string(),
        }
    }

    /// Convert a type ID to a human-readable string.
    pub fn type_to_string(&self, type_id: TypeId) -> String {
        let ty = self.ctx.types.get(type_id);
        self.type_repr(ty, type_id)
    }

    fn type_repr(&self, ty: &Type, _type_id: TypeId) -> String {
        match ty {
            Type::Primitive(prim) => prim.as_str().to_string(),
            Type::Var(var) => format!("?{}", var.0),
            Type::IntVar(var) => format!("?int{}", var.0),
            Type::FloatVar(var) => format!("?float{}", var.0),
            Type::Infer(var, kind) => match kind {
                InferKind::General => format!("?{}", var.0),
                InferKind::Int => format!("?int{}", var.0),
                InferKind::Float => format!("?float{}", var.0),
            },
            Type::Ref(mutability, inner) => {
                let inner_str = self.type_to_string(*inner);
                match mutability {
                    Mutability::Shared => format!("&{}", inner_str),
                    Mutability::Mutable => format!("&mut {}", inner_str),
                }
            }
            Type::Array(elem, len) => {
                let elem_str = self.type_to_string(*elem);
                format!("[{}; {}]", elem_str, len)
            }
            Type::Slice(elem) => {
                let elem_str = self.type_to_string(*elem);
                format!("[{}]", elem_str)
            }
            Type::Tuple(elems) => {
                if elems.is_empty() {
                    "()".to_string()
                } else if elems.len() == 1 {
                    let elem_str = self.type_to_string(elems[0]);
                    format!("({},)", elem_str)
                } else {
                    let elem_strs: Vec<_> = elems.iter().map(|e| self.type_to_string(*e)).collect();
                    format!("({})", elem_strs.join(", "))
                }
            }
            Type::Struct(def_id, _type_args) => {
                let symbol = self.ctx.get_symbol(*def_id);
                self.ctx.resolve(symbol.name).to_string()
            }
            Type::FnPtr { params, ret } => {
                let param_strs: Vec<_> = params.iter().map(|p| self.type_to_string(*p)).collect();
                let ret_str = self.type_to_string(*ret);
                format!("fn({}) -> {}", param_strs.join(", "), ret_str)
            }
            Type::String => "String".to_string(),
            Type::Error => "<error>".to_string(),
            Type::Alias(_, _) => "<alias>".to_string(),
            Type::Param(def_id) => {
                let symbol = self.ctx.get_symbol(*def_id);
                self.ctx.resolve(symbol.name).to_string()
            }
            Type::SelfType => "Self".to_string(),
        }
    }
}

/// Run type inference on a source file.
///
/// Takes the resolved AST and produces type assignments for all expressions and bindings.
pub fn infer(source_file: &SourceFile, resolve_result: ResolveResult) -> InferResult {
    let mut engine = InferEngine::new(resolve_result);
    engine.infer_source_file(source_file);
    engine.apply_defaults();
    engine.into_result()
}

/// The kind of loop for break/continue validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopKind {
    /// `loop { }` - allows break with value
    Loop,
    /// `while cond { }` - no break value allowed
    While,
    /// `for x in iter { }` - no break value allowed
    For,
}

/// The type inference engine.
struct InferEngine {
    ctx: SemanticContext,
    resolutions: FxHashMap<Span, DefId>,
    /// Map from expression spans to their inferred types.
    expr_types: FxHashMap<Span, TypeId>,
    /// Map from local bindings (DefId) to their inferred types.
    binding_types: FxHashMap<DefId, TypeId>,
    /// Type substitution table for union-find.
    substitution: FxHashMap<TypeVar, TypeId>,
    /// Collected diagnostics.
    diagnostics: Vec<Diagnostic>,
    /// Function signatures collected in first pass.
    fn_signatures: FxHashMap<DefId, FnSignature>,
    /// Struct field info collected in first pass.
    struct_fields: FxHashMap<DefId, Vec<(String, TypeId)>>,
    /// Struct type parameters collected in first pass.
    struct_type_params: FxHashMap<DefId, Vec<DefId>>,
    /// Map from struct DefId to its methods' DefIds.
    struct_methods: FxHashMap<DefId, Vec<DefId>>,
    /// Type alias targets collected in first pass (alias DefId -> target type).
    type_alias_targets: FxHashMap<DefId, TypeId>,
    /// Current function's return type (for return statements).
    current_return_type: Option<TypeId>,
    /// Current loop's break type (for break statements with values).
    current_loop_break_type: Option<TypeId>,
    /// Whether the current loop has a break statement (for detecting infinite loops).
    current_loop_has_break: bool,
    /// Current impl block's Self type (for resolving Self in type positions).
    current_self_type: Option<TypeId>,
    /// The kind of the innermost loop (for break/continue validation).
    current_loop_kind: Option<LoopKind>,
    /// Map from method call expression spans to their resolved method DefIds.
    method_resolutions: FxHashMap<Span, DefId>,
}

/// The kind of receiver for a method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelfParamKind {
    /// `self` - takes ownership
    Owned,
    /// `&self` - shared reference
    Ref,
    /// `&mut self` - mutable reference
    RefMut,
}

/// A self parameter for method signatures.
#[derive(Clone, Debug)]
pub struct SelfParam {
    pub kind: SelfParamKind,
    pub self_ty: TypeId,
}

/// Function signature information.
#[derive(Clone)]
struct FnSignature {
    /// The self parameter for methods, if present.
    /// Currently stored for future use (e.g., proper receiver type checking).
    #[allow(dead_code)]
    self_param: Option<SelfParam>,
    /// Type parameters for generic functions (e.g., `T` in `fn foo<T>()`).
    type_params: Vec<DefId>,
    params: Vec<(String, TypeId)>,
    ret: TypeId,
}

impl InferEngine {
    fn new(resolve_result: ResolveResult) -> Self {
        Self {
            ctx: resolve_result.ctx,
            resolutions: resolve_result.resolutions,
            expr_types: FxHashMap::default(),
            binding_types: FxHashMap::default(),
            substitution: FxHashMap::default(),
            diagnostics: resolve_result.diagnostics,
            fn_signatures: FxHashMap::default(),
            struct_fields: FxHashMap::default(),
            struct_type_params: FxHashMap::default(),
            struct_methods: FxHashMap::default(),
            type_alias_targets: FxHashMap::default(),
            current_return_type: None,
            current_loop_break_type: None,
            current_loop_has_break: false,
            current_self_type: None,
            current_loop_kind: None,
            method_resolutions: FxHashMap::default(),
        }
    }

    fn into_result(self) -> InferResult {
        InferResult {
            ctx: self.ctx,
            expr_types: self.expr_types,
            binding_types: self.binding_types,
            method_resolutions: self.method_resolutions,
            diagnostics: self.diagnostics,
        }
    }

    // =========================================================================
    // Union-Find Operations
    // =========================================================================

    /// Create a fresh type variable.
    fn fresh_type_var(&mut self) -> TypeId {
        self.ctx.types.fresh_type_var()
    }

    /// Create a fresh integer type variable (defaults to i32 if unconstrained).
    fn fresh_int_var(&mut self) -> TypeId {
        self.ctx.types.fresh_int_var()
    }

    /// Create a fresh float type variable (defaults to f64 if unconstrained).
    fn fresh_float_var(&mut self) -> TypeId {
        self.ctx.types.fresh_float_var()
    }

    // =========================================================================
    // Contract Helpers
    // =========================================================================

    /// Check if a TypeId is valid (within bounds of the type interner).
    fn is_valid_type_id(&self, id: TypeId) -> bool {
        (id.0 as usize) < self.ctx.types.types_len()
    }

    /// Extract the TypeVar from a type if it's a variable type.
    fn extract_type_var(&self, id: TypeId) -> Option<TypeVar> {
        match self.ctx.types.get(id) {
            Type::Var(v) | Type::IntVar(v) | Type::FloatVar(v) => Some(*v),
            _ => None,
        }
    }

    /// Check if following the substitution chain from `start` forms a cycle.
    /// Uses Floyd's tortoise-and-hare algorithm.
    fn has_cycle(&self, start: TypeVar) -> bool {
        // Tortoise moves one step at a time, hare moves two steps
        let mut tortoise = start;
        let mut hare = start;

        loop {
            // Move tortoise one step
            let tortoise_next = match self.substitution.get(&tortoise) {
                Some(&type_id) => self.extract_type_var(type_id),
                None => return false, // End of chain, no cycle
            };

            tortoise = match tortoise_next {
                Some(v) => v,
                None => return false, // Reached concrete type, no cycle
            };

            // Move hare two steps
            for _ in 0..2 {
                let hare_next = match self.substitution.get(&hare) {
                    Some(&type_id) => self.extract_type_var(type_id),
                    None => return false, // End of chain, no cycle
                };

                hare = match hare_next {
                    Some(v) => v,
                    None => return false, // Reached concrete type, no cycle
                };
            }

            // If they meet, there's a cycle
            if tortoise == hare {
                return true;
            }
        }
    }

    /// Check if the resolved type is concrete or an unbound variable.
    /// Returns true if the type is concrete or an unbound type variable.
    #[cfg(debug_assertions)]
    fn is_resolved_or_unbound(&self, type_id: TypeId) -> bool {
        match self.ctx.types.get(type_id) {
            Type::Var(v) | Type::IntVar(v) | Type::FloatVar(v) => {
                !self.substitution.contains_key(v)
            }
            _ => true, // Concrete type
        }
    }

    /// Resolve a type through the substitution chain.
    fn resolve_type(&self, type_id: TypeId) -> TypeId {
        debug_assert!(
            self.is_valid_type_id(type_id),
            "precondition: type_id {} must be valid (< {})",
            type_id.0,
            self.ctx.types.types_len()
        );

        let ty = self.ctx.types.get(type_id);
        let result = match ty {
            Type::Var(var) | Type::IntVar(var) | Type::FloatVar(var) => {
                if let Some(&subst) = self.substitution.get(var) {
                    self.resolve_type(subst)
                } else {
                    type_id
                }
            }
            // Resolve type aliases stored as Struct or Alias
            Type::Struct(def_id, _) | Type::Alias(def_id, _) => {
                if let Some(&target) = self.type_alias_targets.get(def_id) {
                    self.resolve_type(target)
                } else {
                    type_id
                }
            }
            _ => type_id,
        };

        debug_assert!(
            self.is_resolved_or_unbound(result),
            "postcondition: resolve_type must return concrete type or unbound variable"
        );

        result
    }

    /// Unify two types, returning true if successful.
    fn unify(&mut self, a: TypeId, b: TypeId) -> bool {
        debug_assert!(
            self.is_valid_type_id(a),
            "precondition: type a ({}) must be valid",
            a.0
        );
        debug_assert!(
            self.is_valid_type_id(b),
            "precondition: type b ({}) must be valid",
            b.0
        );

        let a = self.resolve_type(a);
        let b = self.resolve_type(b);

        if a == b {
            return true;
        }

        let ty_a = self.ctx.types.get(a).clone();
        let ty_b = self.ctx.types.get(b).clone();

        let result = match (&ty_a, &ty_b) {
            // Error type unifies with anything
            (Type::Error, _) | (_, Type::Error) => true,

            // Never type unifies with anything (it's the bottom type)
            (Type::Primitive(PrimitiveKind::Never), _)
            | (_, Type::Primitive(PrimitiveKind::Never)) => true,

            // Type variable binds to anything
            (Type::Var(var), _) => {
                self.substitution.insert(*var, b);
                true
            }
            (_, Type::Var(var)) => {
                self.substitution.insert(*var, a);
                true
            }

            // Int variable binds to any integer type or another int variable
            (Type::IntVar(var), Type::Primitive(prim)) if is_integer_type(*prim) => {
                self.substitution.insert(*var, b);
                true
            }
            (Type::Primitive(prim), Type::IntVar(var)) if is_integer_type(*prim) => {
                self.substitution.insert(*var, a);
                true
            }
            (Type::IntVar(var1), Type::IntVar(_var2)) => {
                // Bind one to the other
                self.substitution.insert(*var1, b);
                true
            }

            // Float variable binds to any float type or another float variable
            (Type::FloatVar(var), Type::Primitive(prim)) if is_float_type(*prim) => {
                self.substitution.insert(*var, b);
                true
            }
            (Type::Primitive(prim), Type::FloatVar(var)) if is_float_type(*prim) => {
                self.substitution.insert(*var, a);
                true
            }
            (Type::FloatVar(var1), Type::FloatVar(_var2)) => {
                self.substitution.insert(*var1, b);
                true
            }

            // Primitives must match exactly
            (Type::Primitive(p1), Type::Primitive(p2)) => p1 == p2,

            // Unit type is the same as empty tuple
            (Type::Primitive(PrimitiveKind::Unit), Type::Tuple(elems))
            | (Type::Tuple(elems), Type::Primitive(PrimitiveKind::Unit)) => elems.is_empty(),

            // References must match in mutability and inner type
            (Type::Ref(m1, inner1), Type::Ref(m2, inner2)) => {
                // Allow coercion from &mut to & (but not vice versa)
                let mutability_ok =
                    m1 == m2 || (*m1 == Mutability::Mutable && *m2 == Mutability::Shared);
                mutability_ok && self.unify(*inner1, *inner2)
            }

            // Arrays must match in element type and length
            (Type::Array(elem1, len1), Type::Array(elem2, len2)) => {
                len1 == len2 && self.unify(*elem1, *elem2)
            }

            // Slices must match in element type
            (Type::Slice(elem1), Type::Slice(elem2)) => self.unify(*elem1, *elem2),

            // Tuples must match in arity and element types
            (Type::Tuple(elems1), Type::Tuple(elems2)) => {
                if elems1.len() != elems2.len() {
                    return false;
                }
                for (e1, e2) in elems1.iter().zip(elems2.iter()) {
                    if !self.unify(*e1, *e2) {
                        return false;
                    }
                }
                true
            }

            // Structs must have same DefId and unifiable type args
            (Type::Struct(def1, args1), Type::Struct(def2, args2)) => {
                if def1 != def2 || args1.len() != args2.len() {
                    return false;
                }
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    if !self.unify(*a1, *a2) {
                        return false;
                    }
                }
                true
            }

            // Function pointers must match in params and return type
            (
                Type::FnPtr {
                    params: p1,
                    ret: r1,
                },
                Type::FnPtr {
                    params: p2,
                    ret: r2,
                },
            ) => {
                if p1.len() != p2.len() {
                    return false;
                }
                for (a, b) in p1.iter().zip(p2.iter()) {
                    if !self.unify(*a, *b) {
                        return false;
                    }
                }
                self.unify(*r1, *r2)
            }

            // String type must match exactly
            (Type::String, Type::String) => true,

            // Everything else fails
            _ => false,
        };

        // Postcondition: no cycles in the substitution
        #[cfg(debug_assertions)]
        if result {
            for &var in self.substitution.keys() {
                debug_assert!(
                    !self.has_cycle(var),
                    "invariant: unify must not create cycles in substitution"
                );
            }
        }

        result
    }

    // =========================================================================
    // Generic Instantiation
    // =========================================================================

    /// Instantiate a generic function signature with fresh type variables.
    /// Returns (instantiated_param_types, instantiated_return_type).
    fn instantiate_signature(&mut self, sig: &FnSignature) -> (Vec<TypeId>, TypeId) {
        if sig.type_params.is_empty() {
            // No generics, return as-is
            let param_types: Vec<_> = sig.params.iter().map(|(_, t)| *t).collect();
            return (param_types, sig.ret);
        }

        // Create fresh type variables for each type parameter
        let mut subst: FxHashMap<DefId, TypeId> = FxHashMap::default();
        for &param_def_id in &sig.type_params {
            subst.insert(param_def_id, self.fresh_type_var());
        }

        // Substitute in parameter types
        let param_types: Vec<_> = sig
            .params
            .iter()
            .map(|(_, t)| self.substitute_type_params(*t, &subst))
            .collect();

        // Substitute in return type
        let ret = self.substitute_type_params(sig.ret, &subst);

        (param_types, ret)
    }

    /// Substitute type parameters with their instantiated types.
    fn substitute_type_params(
        &mut self,
        type_id: TypeId,
        subst: &FxHashMap<DefId, TypeId>,
    ) -> TypeId {
        let ty = self.ctx.types.get(type_id).clone();
        match ty {
            Type::Param(def_id) => {
                // Substitute if we have a mapping
                subst.get(&def_id).copied().unwrap_or(type_id)
            }
            Type::Ref(mutability, inner) => {
                let new_inner = self.substitute_type_params(inner, subst);
                if new_inner == inner {
                    type_id
                } else {
                    self.ctx.types.mk_ref(mutability, new_inner)
                }
            }
            Type::Array(elem, len) => {
                let new_elem = self.substitute_type_params(elem, subst);
                if new_elem == elem {
                    type_id
                } else {
                    self.ctx.types.mk_array(new_elem, len)
                }
            }
            Type::Slice(elem) => {
                let new_elem = self.substitute_type_params(elem, subst);
                if new_elem == elem {
                    type_id
                } else {
                    self.ctx.types.mk_slice(new_elem)
                }
            }
            Type::Tuple(elems) => {
                let new_elems: Vec<_> = elems
                    .iter()
                    .map(|e| self.substitute_type_params(*e, subst))
                    .collect();
                if new_elems == elems {
                    type_id
                } else {
                    self.ctx.types.mk_tuple(new_elems)
                }
            }
            Type::Struct(def_id, type_args) => {
                let new_args: Vec<_> = type_args
                    .iter()
                    .map(|a| self.substitute_type_params(*a, subst))
                    .collect();
                if new_args == type_args {
                    type_id
                } else {
                    self.ctx.types.mk_struct(def_id, new_args)
                }
            }
            Type::Alias(def_id, type_args) => {
                let new_args: Vec<_> = type_args
                    .iter()
                    .map(|a| self.substitute_type_params(*a, subst))
                    .collect();
                if new_args == type_args {
                    type_id
                } else {
                    self.ctx.types.mk_alias(def_id, new_args)
                }
            }
            Type::FnPtr { params, ret } => {
                let new_params: Vec<_> = params
                    .iter()
                    .map(|p| self.substitute_type_params(*p, subst))
                    .collect();
                let new_ret = self.substitute_type_params(ret, subst);
                if new_params == params && new_ret == ret {
                    type_id
                } else {
                    self.ctx.types.mk_fn_ptr(new_params, new_ret)
                }
            }
            // Primitives, variables, error, string, selftype don't need substitution
            _ => type_id,
        }
    }

    // =========================================================================
    // Mutability Checking
    // =========================================================================

    /// Check if an expression is a valid assignment target (a mutable place).
    /// Returns an error message if not assignable, None if OK.
    fn check_assignable(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Path(path_expr) => {
                // Look up the path to get the DefId
                let Some(path) = path_expr.path() else {
                    return Some("invalid assignment target".to_string());
                };
                let Some(segment) = path.segments().next() else {
                    return Some("invalid assignment target".to_string());
                };
                let Some(name_ref) = segment.name() else {
                    return Some("invalid assignment target".to_string());
                };
                let Some(token) = name_ref.token() else {
                    return Some("invalid assignment target".to_string());
                };
                let span = text_range_to_span(token.text_range());

                if let Some(&def_id) = self.resolutions.get(&span) {
                    let symbol = self.ctx.get_symbol(def_id);
                    if !symbol.is_mutable {
                        let name = self.ctx.resolve(symbol.name);
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
                    if let Some(&base_ty) = self.expr_types.get(&base_span) {
                        let resolved = self.resolve_type(base_ty);
                        let ty = self.ctx.types.get(resolved);
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
                        .expr_types
                        .get(&text_range_to_span(inner.syntax().text_range()))?;
                    let resolved = self.resolve_type(*inner_ty);
                    let ty = self.ctx.types.get(resolved);
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
    fn check_mutable_borrow(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Path(path_expr) => {
                // Look up the path to get the DefId
                let Some(path) = path_expr.path() else {
                    return Some("cannot take mutable reference of a temporary value".to_string());
                };
                let Some(segment) = path.segments().next() else {
                    return Some("cannot take mutable reference of a temporary value".to_string());
                };
                let Some(name_ref) = segment.name() else {
                    return Some("cannot take mutable reference of a temporary value".to_string());
                };
                let Some(token) = name_ref.token() else {
                    return Some("cannot take mutable reference of a temporary value".to_string());
                };
                let span = text_range_to_span(token.text_range());

                if let Some(&def_id) = self.resolutions.get(&span) {
                    let symbol = self.ctx.get_symbol(def_id);
                    if !symbol.is_mutable {
                        let name = self.ctx.resolve(symbol.name);
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
                    if let Some(&base_ty) = self.expr_types.get(&base_span) {
                        let resolved = self.resolve_type(base_ty);
                        let ty = self.ctx.types.get(resolved);
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
                    if let Some(&inner_ty) = self.expr_types.get(&inner_span) {
                        let resolved = self.resolve_type(inner_ty);
                        let ty = self.ctx.types.get(resolved);
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
    fn synth_expr(&mut self, expr: &Expr) -> TypeId {
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
                LoweredExpr::Passthrough => unreachable!(),
            };
            self.expr_types.insert(span, type_id);
            return type_id;
        }

        let type_id = match expr {
            Expr::Literal(lit) => self.synth_literal(lit),
            Expr::Path(path_expr) => self.synth_path(path_expr),
            Expr::Paren(paren) => self.synth_paren(paren),
            Expr::Tuple(tuple) => self.synth_tuple(tuple),
            Expr::Array(array) => self.synth_array(array),
            Expr::Struct(struct_expr) => self.synth_struct(struct_expr),
            Expr::Binary(bin) => self.synth_binary(bin),
            Expr::Prefix(prefix) => self.synth_prefix(prefix),
            Expr::Ref(ref_expr) => self.synth_ref(ref_expr),
            Expr::Field(field) => self.synth_field(field),
            Expr::MethodCall(method) => self.synth_method_call(method),
            Expr::Call(call) => self.synth_call(call),
            Expr::Index(index) => self.synth_index(index),
            Expr::Slice(slice) => self.synth_slice(slice),
            Expr::If(if_expr) => self.synth_if(if_expr),
            Expr::While(while_expr) => self.synth_while(while_expr),
            Expr::For(for_expr) => self.synth_for(for_expr),
            Expr::Loop(loop_expr) => self.synth_loop(loop_expr),
            Expr::Break(break_expr) => self.synth_break(break_expr),
            Expr::Continue(continue_expr) => self.synth_continue(continue_expr),
            Expr::Return(return_expr) => self.synth_return(return_expr),
            Expr::Block(block_expr) => self.synth_block_expr(block_expr),
            Expr::Cast(cast) => self.synth_cast(cast),
            Expr::Range(range) => self.synth_range(range),
        };
        self.expr_types.insert(span, type_id);
        type_id
    }

    fn synth_literal(&mut self, lit: &LiteralExpr) -> TypeId {
        let token = match lit.token() {
            Some(t) => t,
            None => return self.ctx.types.error(),
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
                    self.ctx.types.primitive(kind)
                } else {
                    // No suffix - create an int inference variable
                    self.fresh_int_var()
                }
            }
            SyntaxKind::FLOAT_LITERAL => {
                let text = token.text();
                if text.ends_with("f32") {
                    self.ctx.types.primitive(PrimitiveKind::F32)
                } else if text.ends_with("f64") {
                    self.ctx.types.primitive(PrimitiveKind::F64)
                } else {
                    // No suffix - create a float inference variable
                    self.fresh_float_var()
                }
            }
            SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW => self.ctx.types.bool(),
            SyntaxKind::CHAR_LITERAL => self.ctx.types.char(),
            SyntaxKind::STRING_LITERAL => self.ctx.types.string(),
            _ => self.ctx.types.error(),
        }
    }

    /// Synthesize type for a lowered integer literal (from HIR lowering).
    fn synth_lowered_int(
        &mut self,
        value: i128,
        suffix: Option<PrimitiveKind>,
        span: Span,
    ) -> TypeId {
        if let Some(kind) = suffix {
            if let Err(msg) = kind.validate_int_literal_range(value) {
                self.diagnostics
                    .push(Diagnostic::error(&msg).with_label(span, "literal out of range"));
            }
            self.ctx.types.primitive(kind)
        } else {
            self.fresh_int_var()
        }
    }

    /// Synthesize type for a lowered float literal (from HIR lowering).
    fn synth_lowered_float(&mut self, suffix: Option<PrimitiveKind>) -> TypeId {
        match suffix {
            Some(PrimitiveKind::F32) => self.ctx.types.primitive(PrimitiveKind::F32),
            Some(PrimitiveKind::F64) => self.ctx.types.primitive(PrimitiveKind::F64),
            _ => self.fresh_float_var(),
        }
    }

    fn synth_path(&mut self, path_expr: &PathExpr) -> TypeId {
        let path = match path_expr.path() {
            Some(p) => p,
            None => return self.ctx.types.error(),
        };

        // Get the span of the first segment to look up the resolution
        let segment = match path.segments().next() {
            Some(s) => s,
            None => return self.ctx.types.error(),
        };

        let name_ref = match segment.name() {
            Some(n) => n,
            None => return self.ctx.types.error(),
        };

        // Use token() instead of ident_token() to handle `self` keyword
        let token = match name_ref.token() {
            Some(t) => t,
            None => return self.ctx.types.error(),
        };

        let span = text_range_to_span(token.text_range());

        // Look up the resolved DefId
        let def_id = match self.resolutions.get(&span) {
            Some(id) => *id,
            None => return self.ctx.types.error(),
        };

        // Get the type from binding_types
        if let Some(&type_id) = self.binding_types.get(&def_id) {
            return type_id;
        }

        // Check if it's a function
        if let Some(sig) = self.fn_signatures.get(&def_id).cloned() {
            // Instantiate generic functions with fresh type variables
            let (param_types, ret_ty) = self.instantiate_signature(&sig);
            return self.ctx.types.mk_fn_ptr(param_types, ret_ty);
        }

        // Unknown binding - return error
        self.ctx.types.error()
    }

    fn synth_paren(&mut self, paren: &ParenExpr) -> TypeId {
        match paren.expr() {
            Some(inner) => self.synth_expr(&inner),
            None => self.ctx.types.error(),
        }
    }

    fn synth_tuple(&mut self, tuple: &TupleExpr) -> TypeId {
        let elem_types: Vec<TypeId> = tuple.exprs().map(|e| self.synth_expr(&e)).collect();
        self.ctx.types.mk_tuple(elem_types)
    }

    fn synth_array(&mut self, array: &ArrayExpr) -> TypeId {
        let exprs: Vec<_> = array.exprs().collect();
        if exprs.is_empty() {
            // Empty array needs type annotation
            let elem = self.fresh_type_var();
            return self.ctx.types.mk_array(elem, 0);
        }

        // Check for repeat syntax [elem; count]
        if array.is_repeat() && exprs.len() == 2 {
            // First expression is the element value
            let elem_type = self.synth_expr(&exprs[0]);
            // Second expression is the count - evaluate as constant
            let count = self.eval_const_usize(&exprs[1]).unwrap_or(0);
            let result = self.ctx.types.mk_array(elem_type, count as u64);

            debug_assert!(
                matches!(self.ctx.types.get(result), Type::Array(_, _)),
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
            if !self.unify(first_type, elem_type) {
                let span = text_range_to_span(expr.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("type mismatch: array elements must have the same type")
                        .with_label(span, "has different type"),
                );
            }
        }

        let result = self.ctx.types.mk_array(first_type, exprs.len() as u64);

        debug_assert!(
            matches!(self.ctx.types.get(result), Type::Array(_, _)),
            "postcondition: synth_array must return Array type"
        );

        result
    }

    fn synth_struct(&mut self, struct_expr: &StructExpr) -> TypeId {
        let path = match struct_expr.path() {
            Some(p) => p,
            None => return self.ctx.types.error(),
        };

        // Get the struct's DefId
        let segment = match path.segments().next() {
            Some(s) => s,
            None => return self.ctx.types.error(),
        };

        let name_ref = match segment.name() {
            Some(n) => n,
            None => return self.ctx.types.error(),
        };

        let token = match name_ref.ident_token() {
            Some(t) => t,
            None => return self.ctx.types.error(),
        };

        let span = text_range_to_span(token.text_range());
        let def_id = match self.resolutions.get(&span) {
            Some(id) => *id,
            None => return self.ctx.types.error(),
        };

        // Resolve type alias to actual struct if needed
        let struct_def_id = if let Some(&target_ty) = self.type_alias_targets.get(&def_id) {
            let resolved = self.resolve_type(target_ty);
            match self.ctx.types.get(resolved) {
                Type::Struct(actual_def_id, _) => *actual_def_id,
                _ => {
                    // Alias doesn't resolve to struct - emit error
                    self.diagnostics.push(
                        Diagnostic::error("type alias does not refer to a struct")
                            .with_label(span, "expected struct type"),
                    );
                    return self.ctx.types.error();
                }
            }
        } else {
            def_id
        };

        // Get struct type params and create substitution map
        let type_params = self
            .struct_type_params
            .get(&struct_def_id)
            .cloned()
            .unwrap_or_default();
        let mut subst: FxHashMap<DefId, TypeId> = FxHashMap::default();
        let mut type_args = Vec::new();
        for param_def_id in &type_params {
            let fresh_var = self.fresh_type_var();
            subst.insert(*param_def_id, fresh_var);
            type_args.push(fresh_var);
        }

        // Get struct field info and substitute type params
        let fields_info = self
            .struct_fields
            .get(&struct_def_id)
            .cloned()
            .unwrap_or_default();
        let instantiated_fields: Vec<(String, TypeId)> = fields_info
            .iter()
            .map(|(name, ty)| (name.clone(), self.substitute_type_params(*ty, &subst)))
            .collect();
        let field_map: FxHashMap<_, _> = instantiated_fields.iter().cloned().collect();

        // Check for struct update syntax: ..base
        let has_update_base = if let Some(update_base) = struct_expr.update_base() {
            if let Some(base_expr) = update_base.expr() {
                let base_ty = self.synth_expr(&base_expr);
                let expected_struct_ty = self.ctx.types.mk_struct(struct_def_id, type_args.clone());
                if !self.unify(base_ty, expected_struct_ty) {
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

        // Check fields in struct expression
        let mut seen_fields = std::collections::HashSet::new();
        for field in struct_expr.fields() {
            // Try name_token() first (raw IDENT), then fall back to name() (NameRef)
            let field_name = match field.name_token() {
                Some(t) => t.text().to_string(),
                None => match field.name().and_then(|n| n.ident_token()) {
                    Some(t) => t.text().to_string(),
                    None => continue,
                },
            };

            if let Some(&expected_type) = field_map.get(&field_name) {
                seen_fields.insert(field_name.clone());
                if let Some(value_expr) = field.expr() {
                    self.check_expr(&value_expr, expected_type);
                }
            } else {
                let field_span = text_range_to_span(field.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error(format!("unknown field `{}`", field_name))
                        .with_label(field_span, "unknown field"),
                );
            }
        }

        // Check for missing fields (only if no update base)
        if !has_update_base {
            for (field_name, _) in &instantiated_fields {
                if !seen_fields.contains(field_name) {
                    let expr_span = text_range_to_span(struct_expr.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!("missing field `{}`", field_name))
                            .with_label(expr_span, "missing field"),
                    );
                }
            }
        }

        // Postcondition: either all fields provided or update base present
        debug_assert!(
            has_update_base
                || seen_fields.len() == instantiated_fields.len()
                || !self.diagnostics.is_empty(),
            "postcondition: struct expr must have all fields or update base (or emit diagnostic)"
        );

        let result = self.ctx.types.mk_struct(struct_def_id, type_args);

        debug_assert!(
            matches!(self.ctx.types.get(result), Type::Struct(_, _)),
            "postcondition: synth_struct must return Struct type"
        );

        result
    }

    fn synth_binary(&mut self, bin: &BinExpr) -> TypeId {
        let op = match bin.op_token() {
            Some(t) => t,
            None => return self.ctx.types.error(),
        };

        let lhs = match bin.lhs() {
            Some(e) => e,
            None => return self.ctx.types.error(),
        };

        let rhs = match bin.rhs() {
            Some(e) => e,
            None => return self.ctx.types.error(),
        };

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
                let lhs_type = self.ctx.types.get(lhs_resolved).clone();
                let is_lhs_numeric = match &lhs_type {
                    Type::IntVar(_) | Type::FloatVar(_) => true,
                    Type::Primitive(p) => is_numeric_type(*p),
                    _ => false,
                };
                if !is_lhs_numeric {
                    let span = text_range_to_span(lhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("cannot apply binary operator to non-numeric type")
                            .with_label(span, "not a numeric type"),
                    );
                    return self.ctx.types.error();
                }

                if !self.unify(lhs_ty, rhs_ty) {
                    let span = text_range_to_span(rhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch in binary operation")
                            .with_label(span, "mismatched operand types"),
                    );
                    return self.ctx.types.error();
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

                if !self.unify(lhs_ty, rhs_ty) {
                    let span = text_range_to_span(rhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch in comparison")
                            .with_label(span, "mismatched operand types"),
                    );
                }

                self.ctx.types.bool()
            }

            // Logical operators - operands and result are bool
            SyntaxKind::AND_AND | SyntaxKind::OR_OR => {
                let lhs_ty = self.synth_expr(&lhs);
                let rhs_ty = self.synth_expr(&rhs);
                let bool_ty = self.ctx.types.bool();

                if !self.unify(lhs_ty, bool_ty) {
                    let span = text_range_to_span(lhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch: expected bool for logical operator")
                            .with_label(span, "not a bool"),
                    );
                }
                if !self.unify(rhs_ty, bool_ty) {
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

                if !self.unify(lhs_ty, rhs_ty) {
                    let span = text_range_to_span(rhs.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("type mismatch in assignment")
                            .with_label(span, "mismatched types"),
                    );
                }

                self.ctx.types.unit()
            }

            _ => self.ctx.types.error(),
        }
    }

    fn synth_prefix(&mut self, prefix: &PrefixExpr) -> TypeId {
        let op = match prefix.op_token() {
            Some(t) => t,
            None => return self.ctx.types.error(),
        };

        let inner = match prefix.expr() {
            Some(e) => e,
            None => return self.ctx.types.error(),
        };

        let inner_ty = self.synth_expr(&inner);

        match op.kind() {
            SyntaxKind::MINUS => {
                // Negation is valid for numeric types
                // Note: Negated suffixed literals (e.g., -128i8) are handled by HIR lowering
                let resolved = self.resolve_type(inner_ty);
                let ty = self.ctx.types.get(resolved).clone();
                match &ty {
                    Type::IntVar(_) | Type::FloatVar(_) => inner_ty,
                    Type::Primitive(p) if is_numeric_type(*p) => inner_ty,
                    _ => {
                        let span = text_range_to_span(inner.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error("cannot apply unary `-` to non-numeric type")
                                .with_label(span, "not a numeric type"),
                        );
                        self.ctx.types.error()
                    }
                }
            }
            SyntaxKind::BANG => {
                // Logical not is valid for bool
                let bool_ty = self.ctx.types.bool();
                if !self.unify(inner_ty, bool_ty) {
                    let span = text_range_to_span(inner.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error("cannot apply unary `!` to non-bool type")
                            .with_label(span, "not a bool"),
                    );
                    return self.ctx.types.error();
                }
                bool_ty
            }
            SyntaxKind::STAR => {
                // Dereference
                let resolved = self.resolve_type(inner_ty);
                let ty = self.ctx.types.get(resolved).clone();
                match ty {
                    Type::Ref(_, inner) => inner,
                    _ => {
                        let span = text_range_to_span(inner.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error("cannot dereference non-reference type")
                                .with_label(span, "not a reference"),
                        );
                        self.ctx.types.error()
                    }
                }
            }
            _ => self.ctx.types.error(),
        }
    }

    fn synth_ref(&mut self, ref_expr: &RefExpr) -> TypeId {
        let inner = match ref_expr.expr() {
            Some(e) => e,
            None => return self.ctx.types.error(),
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

        self.ctx.types.mk_ref(mutability, inner_ty)
    }

    fn synth_field(&mut self, field: &FieldExpr) -> TypeId {
        const MAX_DEREF: usize = 100;

        let base = match field.expr() {
            Some(e) => e,
            None => return self.ctx.types.error(),
        };

        let base_ty = self.synth_expr(&base);
        let resolved = self.resolve_type(base_ty);
        let mut base_type = self.ctx.types.get(resolved).clone();

        // Auto-deref references for field access
        #[cfg(debug_assertions)]
        let mut deref_count = 0;

        while let Type::Ref(_, inner) = &base_type {
            #[cfg(debug_assertions)]
            {
                deref_count += 1;
                debug_assert!(
                    deref_count < MAX_DEREF,
                    "invariant: auto-deref must terminate (hit {} derefs)",
                    MAX_DEREF
                );
            }

            let inner_resolved = self.resolve_type(*inner);
            base_type = self.ctx.types.get(inner_resolved).clone();
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
                    None => return self.ctx.types.error(),
                },
            },
        };

        // Check if it's a tuple index
        if let Ok(idx) = field_name.parse::<usize>()
            && let Type::Tuple(elems) = &base_type
            && idx < elems.len()
        {
            return elems[idx];
        }

        // Handle struct field access
        if let Type::Struct(def_id, type_args) = &base_type {
            let def_id = *def_id;
            let type_args = type_args.clone();

            // Build substitution map from struct's type params to type args
            let type_params = self
                .struct_type_params
                .get(&def_id)
                .cloned()
                .unwrap_or_default();
            let mut subst: FxHashMap<DefId, TypeId> = FxHashMap::default();
            for (param_def_id, type_arg) in type_params.iter().zip(type_args.iter()) {
                subst.insert(*param_def_id, *type_arg);
            }

            if let Some(fields) = self.struct_fields.get(&def_id).cloned() {
                for (name, ty) in fields {
                    if name == field_name {
                        // Substitute type parameters in field type
                        return self.substitute_type_params(ty, &subst);
                    }
                }
            }
            let span = text_range_to_span(field.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error(format!("no field `{}` on struct", field_name))
                    .with_label(span, "unknown field"),
            );
            return self.ctx.types.error();
        }

        let span = text_range_to_span(field.syntax().text_range());
        self.diagnostics.push(
            Diagnostic::error("field access on non-struct type").with_label(span, "not a struct"),
        );
        self.ctx.types.error()
    }

    fn synth_method_call(&mut self, method: &MethodCallExpr) -> TypeId {
        let receiver = match method.receiver() {
            Some(e) => e,
            None => return self.ctx.types.error(),
        };

        let receiver_ty = self.synth_expr(&receiver);

        // Get method name from raw IDENT token first, then try NameRef
        let method_name = match method.name_token() {
            Some(t) => t.text().to_string(),
            None => match method.name() {
                Some(n) => match n.ident_token() {
                    Some(t) => t.text().to_string(),
                    None => return self.ctx.types.error(),
                },
                None => return self.ctx.types.error(),
            },
        };

        // Resolve receiver type to find struct DefId
        let resolved = self.resolve_type(receiver_ty);
        let receiver_type = self.ctx.types.get(resolved).clone();

        // Handle reference receivers (auto-deref) and get type args
        let (struct_def_id, receiver_type_args) = match &receiver_type {
            Type::Struct(def_id, type_args) => (Some(*def_id), type_args.clone()),
            Type::Ref(_, inner) => {
                let inner_resolved = self.resolve_type(*inner);
                let inner_type = self.ctx.types.get(inner_resolved);
                if let Type::Struct(def_id, type_args) = inner_type {
                    (Some(*def_id), type_args.clone())
                } else {
                    (None, vec![])
                }
            }
            _ => (None, vec![]),
        };

        // Look up method in struct_def_id's methods
        if let Some(def_id) = struct_def_id {
            // Get struct type params for building substitution map
            let struct_type_params = self
                .struct_type_params
                .get(&def_id)
                .cloned()
                .unwrap_or_default();

            // Build substitution map from struct's type params to receiver's type args
            let mut subst: FxHashMap<DefId, TypeId> = FxHashMap::default();
            for (param_def_id, type_arg) in struct_type_params.iter().zip(receiver_type_args.iter())
            {
                subst.insert(*param_def_id, *type_arg);
            }

            // Get the list of methods for this struct
            let method_def_ids = self
                .struct_methods
                .get(&def_id)
                .cloned()
                .unwrap_or_default();

            // Search for method with matching name
            let mut found_method: Option<(FnSignature, DefId)> = None;
            for method_def_id in method_def_ids {
                let symbol = self.ctx.get_symbol(method_def_id);
                let fn_name = self.ctx.resolve(symbol.name);
                if fn_name == method_name
                    && let Some(sig) = self.fn_signatures.get(&method_def_id).cloned()
                {
                    found_method = Some((sig, method_def_id));
                    break;
                }
            }

            if let Some((sig, resolved_method_def_id)) = found_method {
                // Store the resolved method DefId for MIR lowering
                let method_span = text_range_to_span(method.syntax().text_range());
                self.method_resolutions
                    .insert(method_span, resolved_method_def_id);
                // Map impl type params to receiver type args by position.
                // sig.type_params structure: [impl_params..., method_params...]
                // where impl_params.len() == struct_type_params.len()
                let impl_param_count = struct_type_params.len();
                for (i, &param_def_id) in sig.type_params.iter().enumerate() {
                    if subst.contains_key(&param_def_id) {
                        continue;
                    }
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
                let params: Vec<(String, TypeId)> = sig
                    .params
                    .iter()
                    .map(|(name, ty)| (name.clone(), self.substitute_type_params(*ty, &subst)))
                    .collect();
                let ret = self.substitute_type_params(sig.ret, &subst);

                // Check arguments
                if let Some(arg_list) = method.arg_list() {
                    let args: Vec<_> = arg_list.args().collect();
                    // params contains only regular params (self is handled separately)
                    let expected_args = &params;

                    if args.len() != expected_args.len() {
                        let span = text_range_to_span(method.syntax().text_range());
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "expected {} argument{}, found {}",
                                expected_args.len(),
                                if expected_args.len() == 1 { "" } else { "s" },
                                args.len()
                            ))
                            .with_label(span, "wrong number of arguments"),
                        );
                    } else {
                        for (arg, (_, expected_ty)) in args.iter().zip(expected_args.iter()) {
                            self.check_expr(arg, *expected_ty);
                        }
                    }
                }
                return ret;
            }
        }

        // Method not found
        let span = text_range_to_span(method.syntax().text_range());
        self.diagnostics.push(
            Diagnostic::error(format!("method `{}` not found", method_name))
                .with_label(span, "unknown method"),
        );
        self.ctx.types.error()
    }

    fn synth_call(&mut self, call: &CallExpr) -> TypeId {
        let callee = match call.callee() {
            Some(e) => e,
            None => return self.ctx.types.error(),
        };

        let callee_ty = self.synth_expr(&callee);
        let resolved = self.resolve_type(callee_ty);
        let callee_type = self.ctx.types.get(resolved).clone();

        // Check if callee is a function
        let (param_types, ret_ty) = match callee_type {
            Type::FnPtr { params, ret } => (params, ret),
            _ => {
                // Check if it's a path to a function
                if let Expr::Path(path_expr) = &callee
                    && let Some(path) = path_expr.path()
                {
                    let segments: Vec<_> = path.segments().collect();

                    // Handle single-segment path (simple function call like `foo()`)
                    if segments.len() == 1 {
                        if let Some(name_ref) = segments[0].name()
                            && let Some(token) = name_ref.token()
                        {
                            let span = text_range_to_span(token.text_range());
                            if let Some(&def_id) = self.resolutions.get(&span)
                                && let Some(sig) = self.fn_signatures.get(&def_id).cloned()
                            {
                                let (param_types, ret_ty) = self.instantiate_signature(&sig);
                                return self.check_call_args(call, &param_types, ret_ty);
                            }
                        }
                    }
                    // Handle two-segment path (associated function like `S::new()`)
                    else if segments.len() == 2 {
                        // Get the type name from the first segment
                        if let Some(type_name_ref) = segments[0].name()
                            && let Some(type_token) = type_name_ref.token()
                        {
                            let type_span = text_range_to_span(type_token.text_range());
                            if let Some(&struct_def_id) = self.resolutions.get(&type_span)
                                && let Some(fn_name_ref) = segments[1].name()
                                && let Some(fn_token) = fn_name_ref.token()
                            {
                                let fn_name = fn_token.text().to_string();
                                // Look up the function in the struct's methods
                                if let Some(methods) =
                                    self.struct_methods.get(&struct_def_id).cloned()
                                {
                                    for method_def_id in methods {
                                        let symbol = self.ctx.get_symbol(method_def_id);
                                        let method_name = self.ctx.resolve(symbol.name);
                                        if method_name == fn_name
                                            && let Some(sig) =
                                                self.fn_signatures.get(&method_def_id).cloned()
                                        {
                                            let (param_types, ret_ty) =
                                                self.instantiate_signature(&sig);
                                            return self.check_call_args(
                                                call,
                                                &param_types,
                                                ret_ty,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let span = text_range_to_span(callee.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("value is not a function").with_label(span, "not a function"),
                );
                return self.ctx.types.error();
            }
        };

        self.check_call_args(call, &param_types, ret_ty)
    }

    fn check_call_args(
        &mut self,
        call: &CallExpr,
        param_types: &[TypeId],
        ret_ty: TypeId,
    ) -> TypeId {
        let args: Vec<_> = call
            .arg_list()
            .map(|al| al.args().collect())
            .unwrap_or_default();

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
            self.check_expr(arg, *expected_ty);
        }

        ret_ty
    }

    fn synth_index(&mut self, index: &IndexExpr) -> TypeId {
        let base = match index.base() {
            Some(e) => e,
            None => return self.ctx.types.error(),
        };

        let idx = match index.index() {
            Some(e) => e,
            None => return self.ctx.types.error(),
        };

        let base_ty = self.synth_expr(&base);
        let _ = self.synth_expr(&idx); // Check index expression

        let resolved = self.resolve_type(base_ty);
        let base_type = self.ctx.types.get(resolved).clone();

        match base_type {
            Type::Array(elem, len) => {
                // Check constant index bounds
                if let Some(idx_val) = self.eval_const_usize(&idx)
                    && idx_val >= len as usize
                {
                    let span = text_range_to_span(idx.syntax().text_range());
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "index {} is out of bounds for array of length {}",
                            idx_val, len
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
                self.ctx.types.error()
            }
        }
    }

    fn synth_slice(&mut self, slice: &SliceExpr) -> TypeId {
        let base = match slice.base() {
            Some(e) => e,
            None => return self.ctx.types.error(),
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
        let base_type = self.ctx.types.get(resolved).clone();

        match base_type {
            Type::Array(elem, _) | Type::Slice(elem) => self.ctx.types.mk_slice(elem),
            _ => {
                let span = text_range_to_span(base.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("cannot slice this type").with_label(span, "not sliceable"),
                );
                self.ctx.types.error()
            }
        }
    }

    fn synth_if(&mut self, if_expr: &IfExpr) -> TypeId {
        // Check condition is bool
        if let Some(cond) = if_expr.condition() {
            let cond_ty = self.synth_expr(&cond);
            let bool_ty = self.ctx.types.bool();
            if !self.unify(cond_ty, bool_ty) {
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
            self.ctx.types.unit()
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
            return self.ctx.types.unit();
        };

        // Unify branches
        if !self.unify(then_ty, else_ty) {
            let span = text_range_to_span(if_expr.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error("type mismatch between if branches")
                    .with_label(span, "branches have different types"),
            );
        }

        then_ty
    }

    fn synth_while(&mut self, while_expr: &WhileExpr) -> TypeId {
        // Check condition is bool
        if let Some(cond) = while_expr.condition() {
            let cond_ty = self.synth_expr(&cond);
            let bool_ty = self.ctx.types.bool();
            if !self.unify(cond_ty, bool_ty) {
                let span = text_range_to_span(cond.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("while condition must be bool")
                        .with_label(span, "expected bool"),
                );
            }
        }

        // Set loop context for break/continue validation
        let old_loop_kind = self.current_loop_kind.replace(LoopKind::While);

        // Synthesize body
        if let Some(body) = while_expr.body() {
            self.synth_block(&body);
        }

        // Restore loop context
        self.current_loop_kind = old_loop_kind;

        // While loops always return unit
        self.ctx.types.unit()
    }

    fn synth_for(&mut self, for_expr: &ForExpr) -> TypeId {
        // Synthesize iterable
        if let Some(iterable) = for_expr.iterable() {
            self.synth_expr(&iterable);
        }

        // Define loop variable binding
        if let Some(pat) = for_expr.pat() {
            // For now, assume the pattern is a simple identifier
            // TODO: Handle complex patterns
            let elem_ty = self.fresh_type_var();
            self.define_pattern(&pat, elem_ty);
        }

        // Set loop context for break/continue validation
        let old_loop_kind = self.current_loop_kind.replace(LoopKind::For);

        // Synthesize body
        if let Some(body) = for_expr.body() {
            self.synth_block(&body);
        }

        // Restore loop context
        self.current_loop_kind = old_loop_kind;

        // For loops always return unit
        self.ctx.types.unit()
    }

    fn synth_loop(&mut self, loop_expr: &LoopExpr) -> TypeId {
        // Create a fresh type variable for the loop's break value
        let break_ty = self.fresh_type_var();
        let old_break_ty = self.current_loop_break_type.replace(break_ty);
        let old_has_break = self.current_loop_has_break;
        self.current_loop_has_break = false;
        // Set loop context for break/continue validation
        let old_loop_kind = self.current_loop_kind.replace(LoopKind::Loop);

        if let Some(body) = loop_expr.body() {
            self.synth_block(&body);
        }

        let has_break = self.current_loop_has_break;
        self.current_loop_break_type = old_break_ty;
        self.current_loop_has_break = old_has_break;
        // Restore loop context
        self.current_loop_kind = old_loop_kind;

        // If no break was found, this is an infinite loop - return never type
        // If break with value exists, return that type
        if has_break {
            break_ty
        } else {
            self.ctx.types.never()
        }
    }

    fn synth_break(&mut self, break_expr: &BreakExpr) -> TypeId {
        let span = text_range_to_span(break_expr.syntax().text_range());

        // Check if we're inside a loop
        let Some(loop_kind) = self.current_loop_kind else {
            self.diagnostics.push(
                Diagnostic::error("break outside of loop")
                    .with_label(span, "`break` can only be used inside a loop"),
            );
            return self.ctx.types.never();
        };

        // Mark that we found a break in the current loop
        self.current_loop_has_break = true;

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
            if let Some(break_ty) = self.current_loop_break_type
                && !self.unify(break_ty, value_ty)
            {
                let value_span = text_range_to_span(value.syntax().text_range());
                self.diagnostics.push(
                    Diagnostic::error("type mismatch in break value")
                        .with_label(value_span, "mismatched types"),
                );
            }
        } else if let Some(break_ty) = self.current_loop_break_type {
            // Break without value - unify with unit
            let unit_ty = self.ctx.types.unit();
            let _ = self.unify(break_ty, unit_ty);
        }
        // Break is a diverging expression
        self.ctx.types.never()
    }

    fn synth_continue(&mut self, continue_expr: &ContinueExpr) -> TypeId {
        // Check if we're inside a loop
        if self.current_loop_kind.is_none() {
            let span = text_range_to_span(continue_expr.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error("continue outside of loop")
                    .with_label(span, "`continue` can only be used inside a loop"),
            );
        }
        // Continue is a diverging expression
        self.ctx.types.never()
    }

    fn synth_return(&mut self, return_expr: &ReturnExpr) -> TypeId {
        let value_ty = if let Some(value) = return_expr.expr() {
            self.synth_expr(&value)
        } else {
            self.ctx.types.unit()
        };

        if let Some(ret_ty) = self.current_return_type
            && !self.unify(ret_ty, value_ty)
        {
            let span = text_range_to_span(return_expr.syntax().text_range());
            self.diagnostics.push(
                Diagnostic::error("type mismatch in return")
                    .with_label(span, "mismatched return type"),
            );
        }

        // Return is a diverging expression
        self.ctx.types.never()
    }

    fn synth_block_expr(&mut self, block_expr: &BlockExpr) -> TypeId {
        match block_expr.block() {
            Some(block) => self.synth_block(&block),
            None => self.ctx.types.unit(),
        }
    }

    fn synth_cast(&mut self, cast: &CastExpr) -> TypeId {
        // Synthesize the source expression
        let source_ty = match cast.expr() {
            Some(expr) => self.synth_expr(&expr),
            None => return self.ctx.types.error(),
        };

        // Get the target type
        let target_ty = match cast.ty() {
            Some(ty) => self.ast_type_to_type_id(&ty),
            None => return self.ctx.types.error(),
        };

        // Validate the cast
        let resolved_source = self.resolve_type(source_ty);
        let resolved_target = self.resolve_type(target_ty);

        if !self.is_valid_cast(resolved_source, resolved_target) {
            let span = text_range_to_span(cast.syntax().text_range());
            let source_str = self.type_to_string(resolved_source);
            let target_str = self.type_to_string(resolved_target);
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "invalid cast from `{}` to `{}`",
                    source_str, target_str
                ))
                .with_label(span, "invalid cast"),
            );
        }

        target_ty
    }

    /// Check if a cast from source type to target type is valid.
    fn is_valid_cast(&self, source: TypeId, target: TypeId) -> bool {
        let source_ty = self.ctx.types.get(source);
        let target_ty = self.ctx.types.get(target);

        match (source_ty, target_ty) {
            // Error type can be cast to anything (to avoid cascading errors)
            (Type::Error, _) | (_, Type::Error) => true,

            // Numeric types can be cast to each other
            (Type::Primitive(s), Type::Primitive(t)) => is_numeric_type(*s) && is_numeric_type(*t),

            // Type variables are allowed (inference not complete)
            (Type::Var(_), _)
            | (_, Type::Var(_))
            | (Type::IntVar(_), _)
            | (_, Type::IntVar(_))
            | (Type::FloatVar(_), _)
            | (_, Type::FloatVar(_)) => true,

            // All other casts are invalid
            _ => false,
        }
    }

    /// Convert a type to a string for error messages.
    fn type_to_string(&self, type_id: TypeId) -> String {
        let ty = self.ctx.types.get(type_id);
        match ty {
            Type::Primitive(prim) => prim.as_str().to_string(),
            Type::Var(var) => format!("?{}", var.0),
            Type::IntVar(var) => format!("?int{}", var.0),
            Type::FloatVar(var) => format!("?float{}", var.0),
            Type::Infer(var, kind) => match kind {
                InferKind::General => format!("?{}", var.0),
                InferKind::Int => format!("?int{}", var.0),
                InferKind::Float => format!("?float{}", var.0),
            },
            Type::Ref(mutability, inner) => {
                let inner_str = self.type_to_string(*inner);
                match mutability {
                    Mutability::Shared => format!("&{}", inner_str),
                    Mutability::Mutable => format!("&mut {}", inner_str),
                }
            }
            Type::Array(elem, len) => {
                let elem_str = self.type_to_string(*elem);
                format!("[{}; {}]", elem_str, len)
            }
            Type::Slice(elem) => {
                let elem_str = self.type_to_string(*elem);
                format!("[{}]", elem_str)
            }
            Type::Tuple(elems) => {
                if elems.is_empty() {
                    "()".to_string()
                } else {
                    let elem_strs: Vec<_> = elems.iter().map(|e| self.type_to_string(*e)).collect();
                    format!("({})", elem_strs.join(", "))
                }
            }
            Type::Struct(def_id, _) => {
                let symbol = self.ctx.get_symbol(*def_id);
                self.ctx.resolve(symbol.name).to_string()
            }
            Type::FnPtr { params, ret } => {
                let param_strs: Vec<_> = params.iter().map(|p| self.type_to_string(*p)).collect();
                let ret_str = self.type_to_string(*ret);
                format!("fn({}) -> {}", param_strs.join(", "), ret_str)
            }
            Type::String => "String".to_string(),
            Type::Error => "<error>".to_string(),
            Type::Alias(_, _) => "<alias>".to_string(),
            Type::Param(def_id) => {
                let symbol = self.ctx.get_symbol(*def_id);
                self.ctx.resolve(symbol.name).to_string()
            }
            Type::SelfType => "Self".to_string(),
        }
    }

    fn synth_range(&mut self, _range: &RangeExpr) -> TypeId {
        // Range expressions have a Range type
        // For now, return a placeholder
        // TODO: Implement Range type properly
        self.fresh_type_var()
    }

    fn synth_block(&mut self, block: &Block) -> TypeId {
        use rowan::ast::AstNode;

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
                            let inner = self.ctx.types.get(resolved);
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
                let inner = self.ctx.types.get(resolved);
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
            self.ctx.types.never()
        } else {
            self.ctx.types.unit()
        };

        // Postcondition: if block diverges and has no tail, result must be never type
        #[cfg(debug_assertions)]
        if diverges && block.tail_expr().is_none() {
            let resolved = self.resolve_type(result);
            debug_assert!(
                matches!(
                    self.ctx.types.get(resolved),
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
    fn check_expr(&mut self, expr: &Expr, expected: TypeId) {
        let actual = self.synth_expr(expr);
        if !self.unify(actual, expected) {
            let span = text_range_to_span(expr.syntax().text_range());
            self.diagnostics
                .push(Diagnostic::error("type mismatch").with_label(span, "mismatched types"));
        } else {
            // After successful unification, validate integer literal ranges
            self.validate_literal_range(expr, expected);
        }
    }

    /// Validate that an integer literal is in range for its resolved type.
    fn validate_literal_range(&mut self, expr: &Expr, expected: TypeId) {
        // Extract the literal value from the expression, handling negation
        let (value, span) = match self.extract_int_literal_value(expr) {
            Some(v) => v,
            None => return,
        };

        // Get the resolved type
        let resolved = self.resolve_type(expected);
        let ty = self.ctx.types.get(resolved).clone();

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
    fn extract_int_literal_value(&self, expr: &Expr) -> Option<(i128, Span)> {
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
    fn infer_let_stmt(&mut self, let_stmt: &LetStmt) -> bool {
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
            let inner = self.ctx.types.get(resolved);
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

    fn define_pattern(&mut self, pat: &Pat, ty: TypeId) {
        match pat {
            Pat::Ident(ident_pat) => {
                // Get the DefId from the resolution
                let token = ident_pat.name().and_then(|n| n.ident_token()).or_else(|| {
                    use crate::ast::token;
                    token(ident_pat.syntax(), SyntaxKind::IDENT)
                });

                if let Some(token) = token {
                    let span = text_range_to_span(token.text_range());
                    // The resolver already defined this binding, we just need to record its type
                    // Look up in resolutions first
                    if let Some(&def_id) = self.resolutions.get(&span) {
                        self.binding_types.insert(def_id, ty);
                    } else {
                        // Try to find by name in current scope
                        let name = token.text();
                        let interned = self.ctx.intern(name);
                        if let Some(def_id) = self.ctx.lookup(interned) {
                            self.binding_types.insert(def_id, ty);
                        }
                    }
                }
            }
            Pat::Tuple(tuple_pat) => {
                let resolved = self.resolve_type(ty);
                let ty_data = self.ctx.types.get(resolved).clone();
                if let Type::Tuple(elem_types) = ty_data {
                    for (inner_pat, elem_ty) in tuple_pat.patterns().zip(elem_types.iter()) {
                        self.define_pattern(&inner_pat, *elem_ty);
                    }
                }
            }
            Pat::Struct(_struct_pat) => {
                // TODO: Handle struct patterns
            }
            Pat::Wildcard(_) => {
                // Wildcard doesn't bind anything
            }
            _ => {}
        }
    }

    // =========================================================================
    // AST Type to TypeId Conversion
    // =========================================================================

    fn ast_type_to_type_id(&mut self, ty: &crate::ast::Type) -> TypeId {
        match ty {
            crate::ast::Type::Path(path_type) => {
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
                                    if let Some(self_ty) = self.current_self_type {
                                        return self_ty;
                                    }
                                    // Self used outside impl block - error
                                    return self.ctx.types.error();
                                }

                                // Check for primitive types
                                if let Some(prim) = PrimitiveKind::from_name(name) {
                                    return self.ctx.types.primitive(prim);
                                }

                                // Check for String
                                if name == "String" {
                                    return self.ctx.types.string();
                                }

                                // Look up in resolutions
                                let span = text_range_to_span(token.text_range());
                                if let Some(&def_id) = self.resolutions.get(&span) {
                                    // Check if it's a type parameter or a struct
                                    let symbol = self.ctx.get_symbol(def_id);
                                    if symbol.kind == SymbolKind::TypeParam {
                                        return self.ctx.types.mk_param(def_id);
                                    }
                                    // It's a struct or type alias - parse generic arguments
                                    let type_args: Vec<TypeId> = segment
                                        .generic_args()
                                        .map(|args| {
                                            args.args()
                                                .map(|t| self.ast_type_to_type_id(&t))
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    return self.ctx.types.mk_struct(def_id, type_args);
                                }
                            }
                        }
                    }
                }
                self.ctx.types.error()
            }
            crate::ast::Type::Ref(ref_type) => {
                let mutability = if ref_type.mut_kw().is_some() {
                    Mutability::Mutable
                } else {
                    Mutability::Shared
                };
                if let Some(inner) = ref_type.ty() {
                    let inner_ty = self.ast_type_to_type_id(&inner);
                    self.ctx.types.mk_ref(mutability, inner_ty)
                } else {
                    self.ctx.types.error()
                }
            }
            crate::ast::Type::Array(array_type) => {
                if let Some(elem_ty) = array_type.elem_ty() {
                    let elem = self.ast_type_to_type_id(&elem_ty);
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
                    self.ctx.types.mk_array(elem, len)
                } else {
                    self.ctx.types.error()
                }
            }
            crate::ast::Type::Slice(slice_type) => {
                if let Some(elem_ty) = slice_type.elem_ty() {
                    let elem = self.ast_type_to_type_id(&elem_ty);
                    self.ctx.types.mk_slice(elem)
                } else {
                    self.ctx.types.error()
                }
            }
            crate::ast::Type::Tuple(tuple_type) => {
                let elems: Vec<_> = tuple_type
                    .types()
                    .map(|t| self.ast_type_to_type_id(&t))
                    .collect();
                self.ctx.types.mk_tuple(elems)
            }
            crate::ast::Type::FnPtr(fn_ptr) => {
                let params: Vec<_> = fn_ptr
                    .param_types()
                    .map(|t| self.ast_type_to_type_id(&t))
                    .collect();
                let ret = fn_ptr
                    .ret_type()
                    .map(|t| self.ast_type_to_type_id(&t))
                    .unwrap_or_else(|| self.ctx.types.unit());
                self.ctx.types.mk_fn_ptr(params, ret)
            }
            crate::ast::Type::Never(_) => self.ctx.types.never(),
        }
    }

    // =========================================================================
    // Top-Level Inference
    // =========================================================================

    fn infer_source_file(&mut self, source_file: &SourceFile) {
        // First pass: collect function signatures and struct info
        for item in source_file.items() {
            match &item {
                Item::Function(func) => self.collect_function_signature(func),
                Item::Struct(struct_def) => self.collect_struct_info(struct_def),
                Item::TypeAlias(type_alias) => self.collect_type_alias_info(type_alias),
                Item::Impl(impl_block) => {
                    // Get the struct this impl is for
                    let struct_def_id = self.get_impl_struct_def_id(impl_block);

                    // Collect impl block type parameters
                    let mut impl_type_params = Vec::new();
                    if let Some(generics) = impl_block.generic_params() {
                        for param in generics.params() {
                            if let Some(name) = param.name()
                                && let Some(token) = name.ident_token()
                            {
                                let span = text_range_to_span(token.text_range());
                                if let Some(&param_def_id) = self.resolutions.get(&span) {
                                    impl_type_params.push(param_def_id);
                                }
                            }
                        }
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

    fn collect_function_signature(&mut self, func: &FunctionDef) {
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

        // Collect type parameters
        let mut type_params = Vec::new();
        if let Some(generics) = func.generic_params() {
            for param in generics.params() {
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
                params.push((param_name, param_ty));
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
        use rustc_hash::FxHashSet;

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
        visited: &mut rustc_hash::FxHashSet<DefId>,
        in_progress: &mut rustc_hash::FxHashSet<DefId>,
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
        use rustc_hash::FxHashSet;

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
        visited: &mut rustc_hash::FxHashSet<DefId>,
        in_progress: &mut rustc_hash::FxHashSet<DefId>,
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

        // Collect type parameters
        let mut type_params = Vec::new();
        if let Some(generics) = struct_def.generic_params() {
            for param in generics.params() {
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
            let param_types: Vec<_> = sig.params.iter().map(|(_, ty)| *ty).collect();

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

    // =========================================================================
    // Default Application
    // =========================================================================

    fn apply_defaults(&mut self) {
        // Collect all type variables that haven't been resolved
        let mut defaults: Vec<(TypeVar, TypeId)> = Vec::new();

        // Go through all bindings and apply defaults
        for (_def_id, &type_id) in self.binding_types.iter() {
            self.collect_defaults(type_id, &mut defaults);
        }

        // Apply defaults
        for (var, default) in defaults {
            self.substitution.entry(var).or_insert(default);
        }

        // Resolve all binding types
        // First collect all the bindings to avoid borrow conflicts
        let bindings: Vec<_> = self.binding_types.drain().collect();
        for (def_id, type_id) in bindings {
            let resolved = self.fully_resolve_type(type_id);
            self.binding_types.insert(def_id, resolved);
        }
    }

    fn collect_defaults(&self, type_id: TypeId, defaults: &mut Vec<(TypeVar, TypeId)>) {
        let ty = self.ctx.types.get(type_id).clone();
        match ty {
            Type::IntVar(var) => {
                if !self.substitution.contains_key(&var) {
                    defaults.push((var, self.ctx.types.i32()));
                }
            }
            Type::FloatVar(var) => {
                if !self.substitution.contains_key(&var) {
                    defaults.push((var, self.ctx.types.f64()));
                }
            }
            Type::Var(_var) => {
                // General type variables don't have defaults - this is an error
                // For now, we'll leave them as-is
            }
            Type::Ref(_, inner) => self.collect_defaults(inner, defaults),
            Type::Array(elem, _) => self.collect_defaults(elem, defaults),
            Type::Slice(elem) => self.collect_defaults(elem, defaults),
            Type::Tuple(elems) => {
                for elem in elems {
                    self.collect_defaults(elem, defaults);
                }
            }
            Type::Struct(_, args) => {
                for arg in args {
                    self.collect_defaults(arg, defaults);
                }
            }
            Type::FnPtr { params, ret } => {
                for param in params {
                    self.collect_defaults(param, defaults);
                }
                self.collect_defaults(ret, defaults);
            }
            _ => {}
        }
    }

    fn fully_resolve_type(&mut self, type_id: TypeId) -> TypeId {
        let resolved = self.resolve_type(type_id);
        let ty = self.ctx.types.get(resolved).clone();

        match ty {
            Type::IntVar(var) => {
                if let Some(&subst) = self.substitution.get(&var) {
                    self.fully_resolve_type(subst)
                } else {
                    // Apply default
                    self.ctx.types.i32()
                }
            }
            Type::FloatVar(var) => {
                if let Some(&subst) = self.substitution.get(&var) {
                    self.fully_resolve_type(subst)
                } else {
                    // Apply default
                    self.ctx.types.f64()
                }
            }
            Type::Var(var) => {
                if let Some(&subst) = self.substitution.get(&var) {
                    self.fully_resolve_type(subst)
                } else {
                    resolved
                }
            }
            Type::Ref(mutability, inner) => {
                let inner_resolved = self.fully_resolve_type(inner);
                self.ctx.types.mk_ref(mutability, inner_resolved)
            }
            Type::Array(elem, len) => {
                let elem_resolved = self.fully_resolve_type(elem);
                self.ctx.types.mk_array(elem_resolved, len)
            }
            Type::Slice(elem) => {
                let elem_resolved = self.fully_resolve_type(elem);
                self.ctx.types.mk_slice(elem_resolved)
            }
            Type::Tuple(elems) => {
                let resolved_elems: Vec<_> =
                    elems.iter().map(|e| self.fully_resolve_type(*e)).collect();
                self.ctx.types.mk_tuple(resolved_elems)
            }
            Type::Struct(def_id, args) => {
                let resolved_args: Vec<_> =
                    args.iter().map(|a| self.fully_resolve_type(*a)).collect();
                self.ctx.types.mk_struct(def_id, resolved_args)
            }
            Type::FnPtr { params, ret } => {
                let resolved_params: Vec<_> =
                    params.iter().map(|p| self.fully_resolve_type(*p)).collect();
                let resolved_ret = self.fully_resolve_type(ret);
                self.ctx.types.mk_fn_ptr(resolved_params, resolved_ret)
            }
            _ => resolved,
        }
    }

    /// Evaluate an expression as a constant usize value.
    /// Used for array repeat counts like `[0; 5]`.
    fn eval_const_usize(&self, expr: &Expr) -> Option<usize> {
        match expr {
            Expr::Literal(lit) => {
                let token = lit.token()?;
                if token.kind() == SyntaxKind::INT_LITERAL {
                    // Parse the integer, stripping any type suffix
                    let text = token.text();
                    let num_text = text.trim_end_matches(|c: char| c.is_alphabetic());
                    num_text.parse().ok()
                } else {
                    None
                }
            }
            Expr::Paren(paren) => paren.expr().and_then(|e| self.eval_const_usize(&e)),
            _ => None,
        }
    }
}

// =========================================================================
// Helper Functions
// =========================================================================

fn text_range_to_span(range: rowan::TextRange) -> Span {
    range.start().into()..range.end().into()
}

fn is_integer_type(prim: PrimitiveKind) -> bool {
    matches!(
        prim,
        PrimitiveKind::I8
            | PrimitiveKind::I16
            | PrimitiveKind::I32
            | PrimitiveKind::I64
            | PrimitiveKind::I128
            | PrimitiveKind::Isize
            | PrimitiveKind::U8
            | PrimitiveKind::U16
            | PrimitiveKind::U32
            | PrimitiveKind::U64
            | PrimitiveKind::U128
            | PrimitiveKind::Usize
    )
}

fn is_float_type(prim: PrimitiveKind) -> bool {
    matches!(prim, PrimitiveKind::F32 | PrimitiveKind::F64)
}

fn is_numeric_type(prim: PrimitiveKind) -> bool {
    is_integer_type(prim) || is_float_type(prim)
}

/// Parse an integer literal suffix to determine the type.
/// Returns (Some(kind), true) if there's a suffix, (None, false) otherwise.
fn parse_int_suffix(text: &str) -> (Option<PrimitiveKind>, bool) {
    // Check suffixes in order of length (longest first to avoid i12 matching i1)
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

/// Parse the numeric value of an integer literal (stripping any suffix).
fn parse_int_literal_value(text: &str) -> Option<i128> {
    // Strip the type suffix (e.g., u8, i32, usize)
    // Must check longer suffixes first to avoid partial matches
    let suffixes = [
        "i128", "u128", "isize", "usize", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8",
    ];
    let num_text = suffixes
        .iter()
        .find(|s| text.ends_with(*s))
        .map(|s| &text[..text.len() - s.len()])
        .unwrap_or(text);

    // Handle hex, octal, binary prefixes
    if num_text.starts_with("0x") || num_text.starts_with("0X") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 16).ok()
    } else if num_text.starts_with("0o") || num_text.starts_with("0O") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 8).ok()
    } else if num_text.starts_with("0b") || num_text.starts_with("0B") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 2).ok()
    } else {
        // Decimal - remove underscores
        num_text.replace('_', "").parse().ok()
    }
}
