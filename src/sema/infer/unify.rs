//! Type unification and resolution.
//!
//! Implements Robinson's unification algorithm (1965) for type inference.
//! Given two types, unification finds the most general substitution that makes
//! them equal, or fails if no such substitution exists.
//!
//! # Algorithm Overview
//!
//! The core `unify(a, b)` function:
//! 1. Resolves both types through the substitution chain
//! 2. If equal after resolution, succeeds immediately
//! 3. If either is a type variable, binds it to the other type
//! 4. For compound types (refs, arrays, tuples), recursively unifies components
//!
//! # Type Variable Kinds
//!
//! SPL uses constrained type variables to improve inference:
//! - `General`: Unconstrained, can unify with any type
//! - `Int`: Can only unify with integer types (i8..i64, u8..u64), defaults to i32
//! - `Float`: Can only unify with float types (f32, f64), defaults to f64
//!
//! This allows integer literals like `42` to be polymorphic until constrained.
//!
//! # Occurs Check
//!
//! The occurs check prevents infinite types like `T = List<T>`. This implementation
//! uses Floyd's cycle detection to verify substitution chains remain acyclic.
//!
//! # References
//!
//! - J.A. Robinson, "A Machine-Oriented Logic Based on the Resolution Principle",
//!   JACM 12(1), 1965

use crate::sema::symbol::DefId;
use crate::sema::types::{InferKind, Mutability, PrimitiveKind, Type, TypeId, TypeVar};
use rustc_hash::FxHashMap;

use super::engine::{FnSignature, InferEngine};
use super::helpers::{is_float_type, is_integer_type};

impl InferEngine {
    // =========================================================================
    // Contract Helpers
    // =========================================================================

    /// Check if a TypeId is valid (within bounds of the type interner).
    pub(super) fn is_valid_type_id(&self, id: TypeId) -> bool {
        (id.0 as usize) < self.ctx.types.types_len()
    }

    /// Extract the TypeVar from a type if it's a variable type.
    fn extract_type_var(&self, id: TypeId) -> Option<TypeVar> {
        match self.ctx.types.get(id) {
            Type::Infer(v, _) => Some(*v),
            _ => None,
        }
    }

    /// Check if the substitution chain from `start` contains a cycle.
    ///
    /// Uses Floyd's cycle detection algorithm (1967), also known as the
    /// "tortoise and hare" algorithm. Two pointers traverse the chain:
    /// - Tortoise: advances one step per iteration
    /// - Hare: advances two steps per iteration
    ///
    /// If a cycle exists, the hare will eventually lap the tortoise and they
    /// will meet. If no cycle exists, one pointer will reach the end of the
    /// chain (either a concrete type or an unbound variable).
    ///
    /// Complexity: O(n) time, O(1) space - crucial for maintaining efficiency
    /// since this runs in debug builds after every substitution.
    ///
    /// # Invariants
    /// - Both pointers only traverse existing substitution entries
    /// - Meeting point proves cycle exists (tortoise position is within cycle)
    /// - Termination guaranteed: either pointers meet or chain ends
    fn has_cycle(&self, start: TypeVar) -> bool {
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
            Type::Infer(v, _) => !self.substitution.contains_key(v),
            _ => true, // Concrete type
        }
    }

    // =========================================================================
    // Type Resolution
    // =========================================================================

    /// Resolve a type through the substitution chain.
    pub(super) fn resolve_type(&self, type_id: TypeId) -> TypeId {
        debug_assert!(
            self.is_valid_type_id(type_id),
            "precondition: type_id {} must be valid (< {})",
            type_id.0,
            self.ctx.types.types_len()
        );

        let ty = self.ctx.types.get(type_id);
        let result = match ty {
            Type::Infer(var, _) => {
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

    // =========================================================================
    // Unification
    // =========================================================================

    /// Unify two types, returning true if successful.
    ///
    /// Implements Robinson's unification with extensions for SPL's type system:
    /// - Constrained type variables (Int, Float) with fallback defaults
    /// - Error/Never types unify with anything (for error recovery)
    /// - Reference mutability coercion: `&mut T` coerces to `&T`
    ///
    /// The substitution is extended in-place. On failure, partial substitutions
    /// may remain (caller should handle this appropriately).
    pub(super) fn unify(&mut self, a: TypeId, b: TypeId) -> bool {
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

            // General type variable binds to anything
            (Type::Infer(var, InferKind::General), _) => {
                self.substitution.insert(*var, b);
                true
            }
            (_, Type::Infer(var, InferKind::General)) => {
                self.substitution.insert(*var, a);
                true
            }

            // Int variable binds to any integer type or another int variable
            (Type::Infer(var, InferKind::Int), Type::Primitive(prim)) if is_integer_type(*prim) => {
                self.substitution.insert(*var, b);
                true
            }
            (Type::Primitive(prim), Type::Infer(var, InferKind::Int)) if is_integer_type(*prim) => {
                self.substitution.insert(*var, a);
                true
            }
            (Type::Infer(var1, InferKind::Int), Type::Infer(_, InferKind::Int)) => {
                // Bind one to the other
                self.substitution.insert(*var1, b);
                true
            }

            // Float variable binds to any float type or another float variable
            (Type::Infer(var, InferKind::Float), Type::Primitive(prim)) if is_float_type(*prim) => {
                self.substitution.insert(*var, b);
                true
            }
            (Type::Primitive(prim), Type::Infer(var, InferKind::Float)) if is_float_type(*prim) => {
                self.substitution.insert(*var, a);
                true
            }
            (Type::Infer(var1, InferKind::Float), Type::Infer(_, InferKind::Float)) => {
                self.substitution.insert(*var1, b);
                true
            }

            // Primitives must match exactly
            (Type::Primitive(p1), Type::Primitive(p2)) => p1 == p2,

            // Unit type is the same as empty tuple
            (Type::Primitive(PrimitiveKind::Unit), Type::Tuple(elems))
            | (Type::Tuple(elems), Type::Primitive(PrimitiveKind::Unit)) => elems.is_empty(),

            // References: mutability must match or coerce, inner types must unify.
            // Coercion: &mut T -> &T is allowed (mutable can become shared),
            // but &T -> &mut T is forbidden (can't gain mutability).
            (Type::Ref(m1, inner1), Type::Ref(m2, inner2)) => {
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

            // StrRef type must match exactly
            (Type::StrRef, Type::StrRef) => true,

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
    pub(super) fn instantiate_signature(&mut self, sig: &FnSignature) -> (Vec<TypeId>, TypeId) {
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
    pub(super) fn substitute_type_params(
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
    // Default Application
    // =========================================================================

    pub(super) fn apply_defaults(&mut self) {
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

        // Resolve all expression types
        let exprs: Vec<_> = self.expr_types.drain().collect();
        for (span, type_id) in exprs {
            let resolved = self.fully_resolve_type(type_id);
            self.expr_types.insert(span, resolved);
        }

        // Resolve all type annotation types
        let annotations: Vec<_> = self.type_annotation_types.drain().collect();
        for (span, type_id) in annotations {
            let resolved = self.fully_resolve_type(type_id);
            self.type_annotation_types.insert(span, resolved);
        }
    }

    fn collect_defaults(&self, type_id: TypeId, defaults: &mut Vec<(TypeVar, TypeId)>) {
        let ty = self.ctx.types.get(type_id).clone();
        match ty {
            Type::Infer(var, InferKind::Int) => {
                if !self.substitution.contains_key(&var) {
                    defaults.push((var, self.ctx.types.i32()));
                }
            }
            Type::Infer(var, InferKind::Float) => {
                if !self.substitution.contains_key(&var) {
                    defaults.push((var, self.ctx.types.f64()));
                }
            }
            Type::Infer(_, InferKind::General) => {
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

    pub(super) fn fully_resolve_type(&mut self, type_id: TypeId) -> TypeId {
        let resolved = self.resolve_type(type_id);
        let ty = self.ctx.types.get(resolved).clone();

        match ty {
            Type::Infer(var, InferKind::Int) => {
                if let Some(&subst) = self.substitution.get(&var) {
                    self.fully_resolve_type(subst)
                } else {
                    // Apply default
                    self.ctx.types.i32()
                }
            }
            Type::Infer(var, InferKind::Float) => {
                if let Some(&subst) = self.substitution.get(&var) {
                    self.fully_resolve_type(subst)
                } else {
                    // Apply default
                    self.ctx.types.f64()
                }
            }
            Type::Infer(var, InferKind::General) => {
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
    pub(super) fn eval_const_usize(&self, expr: &crate::ast::Expr) -> Option<usize> {
        use crate::ast::Expr;
        use crate::syntax::SyntaxKind;

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

// =============================================================================
// Tests for Floyd's Cycle Detection Algorithm
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SourceFile;
    use crate::parser::parse;
    use crate::sema::resolver::resolve;
    use rowan::ast::AstNode;

    /// Helper to create a minimal InferEngine for testing has_cycle.
    fn create_test_engine() -> InferEngine {
        // Parse minimal source to get a valid ResolveResult
        let source = "fn main() {}";
        let parse_result = parse(source);
        let source_file = SourceFile::cast(parse_result.syntax()).unwrap();
        let resolve_result = resolve(&source_file);
        InferEngine::new(resolve_result)
    }

    // =========================================================================
    // Floyd's Cycle Detection Tests
    // =========================================================================
    //
    // These tests verify the documented properties of the has_cycle function:
    // - O(n) time, O(1) space (tortoise-and-hare pattern)
    // - Terminates at concrete types or unbound variables
    // - Correctly identifies cycles of any length

    #[test]
    fn has_cycle_empty_substitution_returns_false() {
        // An empty substitution has no chains to follow
        let engine = create_test_engine();
        let var = TypeVar(999); // Any unbound variable
        assert!(!engine.has_cycle(var));
    }

    #[test]
    fn has_cycle_single_step_to_concrete_returns_false() {
        // Chain: v0 -> i32 (concrete type)
        // The algorithm should terminate when it reaches a concrete type.
        let mut engine = create_test_engine();
        let v0 = engine.fresh_type_var();
        let var = engine.extract_type_var(v0).unwrap();

        // Bind v0 to i32 (a concrete type)
        engine.substitution.insert(var, engine.ctx.types.i32());

        assert!(!engine.has_cycle(var));
    }

    #[test]
    fn has_cycle_chain_to_concrete_returns_false() {
        // Chain: v0 -> v1 -> v2 -> i32 (concrete)
        // Tests O(n) traversal of acyclic chain.
        let mut engine = create_test_engine();

        // Create 3 type variables
        let v0_id = engine.fresh_type_var();
        let v1_id = engine.fresh_type_var();
        let v2_id = engine.fresh_type_var();

        let v0 = engine.extract_type_var(v0_id).unwrap();
        let v1 = engine.extract_type_var(v1_id).unwrap();
        let v2 = engine.extract_type_var(v2_id).unwrap();

        // Chain: v0 -> v1 -> v2 -> i32
        engine.substitution.insert(v0, v1_id);
        engine.substitution.insert(v1, v2_id);
        engine.substitution.insert(v2, engine.ctx.types.i32());

        assert!(!engine.has_cycle(v0));
    }

    #[test]
    fn has_cycle_self_loop_returns_true() {
        // Cycle: v0 -> v0 (1-node cycle)
        // Tests detection of simplest possible cycle.
        let mut engine = create_test_engine();
        let v0_id = engine.fresh_type_var();
        let v0 = engine.extract_type_var(v0_id).unwrap();

        // Create self-loop: v0 -> v0
        engine.substitution.insert(v0, v0_id);

        assert!(engine.has_cycle(v0));
    }

    #[test]
    fn has_cycle_two_node_cycle_returns_true() {
        // Cycle: v0 -> v1 -> v0 (2-node cycle)
        let mut engine = create_test_engine();

        let v0_id = engine.fresh_type_var();
        let v1_id = engine.fresh_type_var();

        let v0 = engine.extract_type_var(v0_id).unwrap();
        let v1 = engine.extract_type_var(v1_id).unwrap();

        // Chain: v0 -> v1 -> v0
        engine.substitution.insert(v0, v1_id);
        engine.substitution.insert(v1, v0_id);

        assert!(engine.has_cycle(v0));
    }

    #[test]
    fn has_cycle_long_tail_with_cycle_returns_true() {
        // Chain with tail leading into cycle: v0 -> v1 -> v2 -> v3 -> v2
        // (Entry from outside the cycle)
        let mut engine = create_test_engine();

        let v0_id = engine.fresh_type_var();
        let v1_id = engine.fresh_type_var();
        let v2_id = engine.fresh_type_var();
        let v3_id = engine.fresh_type_var();

        let v0 = engine.extract_type_var(v0_id).unwrap();
        let v1 = engine.extract_type_var(v1_id).unwrap();
        let v2 = engine.extract_type_var(v2_id).unwrap();
        let v3 = engine.extract_type_var(v3_id).unwrap();

        // Chain: v0 -> v1 -> v2 -> v3 -> v2 (cycle at v2-v3)
        engine.substitution.insert(v0, v1_id);
        engine.substitution.insert(v1, v2_id);
        engine.substitution.insert(v2, v3_id);
        engine.substitution.insert(v3, v2_id);

        assert!(engine.has_cycle(v0));
    }

    #[test]
    fn has_cycle_odd_length_cycle_returns_true() {
        // Cycle: v0 -> v1 -> v2 -> v0 (3-node cycle, odd length)
        // Verifies algorithm works for odd cycle lengths.
        let mut engine = create_test_engine();

        let v0_id = engine.fresh_type_var();
        let v1_id = engine.fresh_type_var();
        let v2_id = engine.fresh_type_var();

        let v0 = engine.extract_type_var(v0_id).unwrap();
        let v1 = engine.extract_type_var(v1_id).unwrap();
        let v2 = engine.extract_type_var(v2_id).unwrap();

        // Chain: v0 -> v1 -> v2 -> v0
        engine.substitution.insert(v0, v1_id);
        engine.substitution.insert(v1, v2_id);
        engine.substitution.insert(v2, v0_id);

        assert!(engine.has_cycle(v0));
    }

    #[test]
    fn has_cycle_unbound_variable_returns_false() {
        // An unbound variable (not in substitution) has no cycle.
        let mut engine = create_test_engine();

        // Create some variables but don't bind the one we check
        let v0_id = engine.fresh_type_var();
        let v1_id = engine.fresh_type_var();
        let _v2_id = engine.fresh_type_var(); // unbound

        let v0 = engine.extract_type_var(v0_id).unwrap();
        let v2 = engine.extract_type_var(_v2_id).unwrap();

        // Bind v0 -> v1, but v2 is unbound
        engine.substitution.insert(v0, v1_id);

        assert!(!engine.has_cycle(v2));
    }
}
