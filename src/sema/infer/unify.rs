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

use super::UnifyError;
use super::engine::{FnSignature, InferEngine};
use super::helpers::{is_float_type, is_integer_type};

/// Maximum depth for type resolution to prevent stack overflow on deeply nested
/// alias chains or pathological substitution patterns.
const MAX_RESOLVE_DEPTH: usize = 256;

impl<'a> InferEngine<'a> {
    // =========================================================================
    // Contract Helpers
    // =========================================================================

    /// Check if a `TypeId` is valid (within bounds of the type interner).
    pub(super) fn is_valid_type_id(&self, id: TypeId) -> bool {
        (id.index() as usize) < self.types.types_len()
    }

    /// Extract the `TypeVar` from a type if it's a variable type.
    fn extract_type_var(&self, id: TypeId) -> Option<TypeVar> {
        match self.types.get(id) {
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
    /// Used in debug assertions; trivial enough to always compile.
    fn is_resolved_or_unbound(&self, type_id: TypeId) -> bool {
        match self.types.get(type_id) {
            Type::Infer(v, _) => !self.substitution.contains_key(v),
            _ => true, // Concrete type
        }
    }

    /// Check if a type variable occurs within a type.
    ///
    /// This is the "occurs check" that prevents creation of infinite types.
    /// For example, unifying ?T with (?T,) would create ?T = (?T,) = ((?T,),) = ...
    ///
    /// Returns true if `var` appears anywhere within `type_id`.
    fn occurs_in(&self, var: TypeVar, type_id: TypeId) -> bool {
        let resolved = self.resolve_type(type_id);
        let ty = self.types.get(resolved);
        match ty {
            Type::Infer(v, _) => *v == var,
            Type::Tuple(tys) => tys.iter().any(|t| self.occurs_in(var, *t)),
            Type::Array(elem, _)
            | Type::Slice(elem)
            | Type::Ref(_, elem)
            | Type::RawPtr(_, elem) => self.occurs_in(var, *elem),
            Type::Struct(_, args) | Type::Alias(_, args) => {
                args.iter().any(|t| self.occurs_in(var, *t))
            }
            Type::FnPtr { params, ret } => {
                params.iter().any(|t| self.occurs_in(var, *t)) || self.occurs_in(var, *ret)
            }
            // Primitives, error, str ref, params, self type, modules don't contain type variables
            Type::Primitive(_)
            | Type::Error
            | Type::StrRef
            | Type::Param(_)
            | Type::SelfType
            | Type::Module(_) => false,
        }
    }

    // =========================================================================
    // Type Resolution
    // =========================================================================

    /// Resolve a type through the substitution chain.
    pub(super) fn resolve_type(&self, type_id: TypeId) -> TypeId {
        self.resolve_type_inner(type_id, 0)
    }

    /// Internal depth-limited type resolution.
    ///
    /// Returns the unresolved type if depth limit is exceeded to prevent
    /// stack overflow on deeply nested alias chains.
    fn resolve_type_inner(&self, type_id: TypeId, depth: usize) -> TypeId {
        debug_assert!(
            self.is_valid_type_id(type_id),
            "precondition: type_id {} must be valid (< {})",
            type_id.index(),
            self.types.types_len()
        );

        // Return unresolved to avoid stack overflow
        if depth > MAX_RESOLVE_DEPTH {
            return type_id;
        }

        let ty = self.types.get(type_id);
        let result = match ty {
            Type::Infer(var, _) => {
                if let Some(&subst) = self.substitution.get(var) {
                    self.resolve_type_inner(subst, depth + 1)
                } else {
                    type_id
                }
            }
            // Resolve type aliases stored as Struct or Alias
            Type::Struct(def_id, _) | Type::Alias(def_id, _) => {
                if let Some(&target) = self.defs.type_alias_targets.get(def_id) {
                    self.resolve_type_inner(target, depth + 1)
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

    /// Unify two types, returning `Ok(())` if successful or an error describing the mismatch.
    ///
    /// Implements Robinson's unification with extensions for SPL's type system:
    /// - Constrained type variables (Int, Float) with fallback defaults
    /// - Error/Never types unify with anything (for error recovery)
    /// - Reference mutability coercion: `&mut T` coerces to `&T`
    ///
    /// The substitution is extended in-place. On failure, partial substitutions
    /// may remain (caller should handle this appropriately).
    pub(super) fn unify(&mut self, a: TypeId, b: TypeId) -> Result<(), UnifyError> {
        debug_assert!(
            self.is_valid_type_id(a),
            "precondition: type a ({}) must be valid",
            a.index()
        );
        debug_assert!(
            self.is_valid_type_id(b),
            "precondition: type b ({}) must be valid",
            b.index()
        );

        let a = self.resolve_type(a);
        let b = self.resolve_type(b);

        if a == b {
            return Ok(());
        }

        let ty_a = self.types.get(a).clone();
        let ty_b = self.types.get(b).clone();

        let result: Result<(), UnifyError> = match (&ty_a, &ty_b) {
            // Error type and Never type unify with anything (Never is the bottom type)
            // StrRef with StrRef also returns Ok
            (Type::Error, _)
            | (_, Type::Error)
            | (Type::Primitive(PrimitiveKind::Never), _)
            | (_, Type::Primitive(PrimitiveKind::Never))
            | (Type::StrRef, Type::StrRef) => Ok(()),

            // General type variable binds to anything (with occurs check)
            (Type::Infer(var, InferKind::General), _) => {
                if self.occurs_in(*var, b) {
                    return Err(UnifyError::InfiniteType { var: a, ty: b });
                }
                self.substitution.insert(*var, b);
                Ok(())
            }
            (_, Type::Infer(var, InferKind::General)) => {
                if self.occurs_in(*var, a) {
                    return Err(UnifyError::InfiniteType { var: b, ty: a });
                }
                self.substitution.insert(*var, a);
                Ok(())
            }

            // Int variable binds to any integer type or another int variable (with occurs check)
            (Type::Infer(var, InferKind::Int), Type::Primitive(prim)) if is_integer_type(*prim) => {
                // No occurs check needed - primitives can't contain type variables
                self.substitution.insert(*var, b);
                Ok(())
            }
            (Type::Primitive(prim), Type::Infer(var, InferKind::Int)) if is_integer_type(*prim) => {
                // No occurs check needed - primitives can't contain type variables
                self.substitution.insert(*var, a);
                Ok(())
            }
            // Bind Int or Float type vars to each other (same variable is OK - identity)
            (Type::Infer(var1, InferKind::Int), Type::Infer(var2, InferKind::Int))
            | (Type::Infer(var1, InferKind::Float), Type::Infer(var2, InferKind::Float)) => {
                if var1 != var2 {
                    self.substitution.insert(*var1, b);
                }
                Ok(())
            }
            // Int variable vs non-integer type
            (Type::Infer(_, InferKind::Int), _) => Err(UnifyError::ConstraintViolation {
                kind: InferKind::Int,
                actual: b,
            }),
            (_, Type::Infer(_, InferKind::Int)) => Err(UnifyError::ConstraintViolation {
                kind: InferKind::Int,
                actual: a,
            }),

            // Float variable binds to any float type (with occurs check)
            (Type::Infer(var, InferKind::Float), Type::Primitive(prim)) if is_float_type(*prim) => {
                // No occurs check needed - primitives can't contain type variables
                self.substitution.insert(*var, b);
                Ok(())
            }
            (Type::Primitive(prim), Type::Infer(var, InferKind::Float)) if is_float_type(*prim) => {
                // No occurs check needed - primitives can't contain type variables
                self.substitution.insert(*var, a);
                Ok(())
            }
            // Float variable vs non-float type
            (Type::Infer(_, InferKind::Float), _) => Err(UnifyError::ConstraintViolation {
                kind: InferKind::Float,
                actual: b,
            }),
            (_, Type::Infer(_, InferKind::Float)) => Err(UnifyError::ConstraintViolation {
                kind: InferKind::Float,
                actual: a,
            }),

            // Primitives must match exactly
            (Type::Primitive(p1), Type::Primitive(p2)) => {
                if p1 == p2 {
                    Ok(())
                } else {
                    Err(UnifyError::TypeMismatch {
                        expected: a,
                        actual: b,
                    })
                }
            }

            // Unit type is the same as empty tuple
            (Type::Primitive(PrimitiveKind::Unit), Type::Tuple(elems))
            | (Type::Tuple(elems), Type::Primitive(PrimitiveKind::Unit)) => {
                if elems.is_empty() {
                    Ok(())
                } else {
                    Err(UnifyError::TypeMismatch {
                        expected: a,
                        actual: b,
                    })
                }
            }

            // References and raw pointers: mutability must match or coerce, inner types must unify.
            // With unify(expected, actual) semantics:
            // - Coercion allows actual (&mut T) to satisfy expected (&T)
            // - But actual (&T) cannot satisfy expected (&mut T)
            (Type::Ref(m1, inner1), Type::Ref(m2, inner2))
            | (Type::RawPtr(m1, inner1), Type::RawPtr(m2, inner2)) => {
                // m1 = expected mutability, m2 = actual mutability
                // Allow if same, or if actual is mutable and expected is shared
                let mutability_ok =
                    m1 == m2 || (*m2 == Mutability::Mutable && *m1 == Mutability::Shared);
                if !mutability_ok {
                    return Err(UnifyError::MutabilityMismatch {
                        expected: *m1,
                        actual: *m2,
                    });
                }
                self.unify(*inner1, *inner2)
            }

            // Arrays must match in element type and length
            (Type::Array(elem1, len1), Type::Array(elem2, len2)) => {
                if len1 != len2 {
                    return Err(UnifyError::ArrayLengthMismatch {
                        expected: *len1,
                        actual: *len2,
                    });
                }
                self.unify(*elem1, *elem2)
            }

            // Slices must match in element type
            (Type::Slice(elem1), Type::Slice(elem2)) => self.unify(*elem1, *elem2),

            // Tuples must match in arity and element types
            (Type::Tuple(elems1), Type::Tuple(elems2)) => {
                if elems1.len() != elems2.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: elems1.len(),
                        actual: elems2.len(),
                    });
                }
                for (e1, e2) in elems1.iter().zip(elems2.iter()) {
                    self.unify(*e1, *e2)?;
                }
                Ok(())
            }

            // Structs must have same DefId and unifiable type args
            (Type::Struct(def1, args1), Type::Struct(def2, args2)) => {
                if def1 != def2 {
                    return Err(UnifyError::TypeMismatch {
                        expected: a,
                        actual: b,
                    });
                }
                if args1.len() != args2.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: args1.len(),
                        actual: args2.len(),
                    });
                }
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    self.unify(*a1, *a2)?;
                }
                Ok(())
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
                    return Err(UnifyError::ArityMismatch {
                        expected: p1.len(),
                        actual: p2.len(),
                    });
                }
                for (param_a, param_b) in p1.iter().zip(p2.iter()) {
                    self.unify(*param_a, *param_b)?;
                }
                self.unify(*r1, *r2)
            }

            // Everything else fails (StrRef must match exactly)
            _ => Err(UnifyError::TypeMismatch {
                expected: a,
                actual: b,
            }),
        };

        // Postcondition: no cycles in the substitution
        #[cfg(debug_assertions)]
        if result.is_ok() {
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
    /// Returns (`instantiated_param_types`, `instantiated_return_type`).
    pub(super) fn instantiate_signature(&mut self, sig: &FnSignature) -> (Vec<TypeId>, TypeId) {
        if sig.type_params.is_empty() {
            // No generics, return as-is
            let param_types: Vec<_> = sig.params.iter().map(|p| p.ty).collect();
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
            .map(|p| self.substitute_type_params(p.ty, &subst))
            .collect();

        // Substitute in return type
        let ret = self.substitute_type_params(sig.ret, &subst);

        (param_types, ret)
    }

    /// Instantiate a generic function signature with fresh type variables.
    /// Returns (`instantiated_param_infos_with_labels`, `instantiated_return_type`).
    pub(super) fn instantiate_signature_with_labels(
        &mut self,
        sig: &FnSignature,
    ) -> (Vec<super::engine::ParamInfo>, TypeId) {
        use super::engine::ParamInfo;

        if sig.type_params.is_empty() {
            // No generics, return as-is
            return (sig.params.clone(), sig.ret);
        }

        // Create fresh type variables for each type parameter
        let mut subst: FxHashMap<DefId, TypeId> = FxHashMap::default();
        for &param_def_id in &sig.type_params {
            subst.insert(param_def_id, self.fresh_type_var());
        }

        // Substitute in parameter types, preserving labels
        let param_infos: Vec<_> = sig
            .params
            .iter()
            .map(|p| ParamInfo {
                label: p.label.clone(),
                name: p.name.clone(),
                ty: self.substitute_type_params(p.ty, &subst),
            })
            .collect();

        // Substitute in return type
        let ret = self.substitute_type_params(sig.ret, &subst);

        (param_infos, ret)
    }

    /// Substitute type parameters with their instantiated types.
    pub(super) fn substitute_type_params(
        &mut self,
        type_id: TypeId,
        subst: &FxHashMap<DefId, TypeId>,
    ) -> TypeId {
        let ty = self.types.get(type_id).clone();
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
                    self.types.mk_ref(mutability, new_inner)
                }
            }
            Type::RawPtr(mutability, pointee) => {
                let new_pointee = self.substitute_type_params(pointee, subst);
                if new_pointee == pointee {
                    type_id
                } else {
                    self.types.mk_raw_ptr(mutability, new_pointee)
                }
            }
            Type::Array(elem, len) => {
                let new_elem = self.substitute_type_params(elem, subst);
                if new_elem == elem {
                    type_id
                } else {
                    self.types.mk_array(new_elem, len)
                }
            }
            Type::Slice(elem) => {
                let new_elem = self.substitute_type_params(elem, subst);
                if new_elem == elem {
                    type_id
                } else {
                    self.types.mk_slice(new_elem)
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
                    self.types.mk_tuple(new_elems)
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
                    self.types.mk_struct(def_id, new_args)
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
                    self.types.mk_alias(def_id, new_args)
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
                    self.types.mk_fn_ptr(new_params, new_ret)
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
        for &type_id in self.results.binding_types.values() {
            self.collect_defaults(type_id, &mut defaults);
        }

        // Apply defaults
        for (var, default) in defaults {
            self.substitution.entry(var).or_insert(default);
        }

        // Resolve all binding types
        // First collect all the bindings to avoid borrow conflicts
        let bindings: Vec<_> = self.results.binding_types.drain().collect();
        for (def_id, type_id) in bindings {
            let resolved = self.fully_resolve_type(type_id);
            self.results.binding_types.insert(def_id, resolved);
        }

        // Resolve all expression types
        let exprs: Vec<_> = self.results.expr_types.drain().collect();
        for (span, type_id) in exprs {
            let resolved = self.fully_resolve_type(type_id);
            self.results.expr_types.insert(span, resolved);
        }

        // Resolve all type annotation types
        let annotations: Vec<_> = self.results.type_annotation_types.drain().collect();
        for (span, type_id) in annotations {
            let resolved = self.fully_resolve_type(type_id);
            self.results.type_annotation_types.insert(span, resolved);
        }
    }

    fn collect_defaults(&self, type_id: TypeId, defaults: &mut Vec<(TypeVar, TypeId)>) {
        let ty = self.types.get(type_id).clone();
        match ty {
            Type::Infer(var, InferKind::Int) => {
                if !self.substitution.contains_key(&var) {
                    defaults.push((var, self.types.i32()));
                }
            }
            Type::Infer(var, InferKind::Float) => {
                if !self.substitution.contains_key(&var) {
                    defaults.push((var, self.types.f64()));
                }
            }
            Type::Ref(_, inner)
            | Type::RawPtr(_, inner)
            | Type::Array(inner, _)
            | Type::Slice(inner) => self.collect_defaults(inner, defaults),
            Type::Tuple(elems) | Type::Struct(_, elems) => {
                for elem in elems {
                    self.collect_defaults(elem, defaults);
                }
            }
            Type::FnPtr { params, ret } => {
                for param in params {
                    self.collect_defaults(param, defaults);
                }
                self.collect_defaults(ret, defaults);
            }
            // General type variables don't have defaults; other types have no defaults
            _ => {}
        }
    }

    pub(super) fn fully_resolve_type(&mut self, type_id: TypeId) -> TypeId {
        let resolved = self.resolve_type(type_id);
        let ty = self.types.get(resolved).clone();

        match ty {
            Type::Infer(var, InferKind::Int) => {
                if let Some(&subst) = self.substitution.get(&var) {
                    self.fully_resolve_type(subst)
                } else {
                    // Apply default
                    self.types.i32()
                }
            }
            Type::Infer(var, InferKind::Float) => {
                if let Some(&subst) = self.substitution.get(&var) {
                    self.fully_resolve_type(subst)
                } else {
                    // Apply default
                    self.types.f64()
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
                self.types.mk_ref(mutability, inner_resolved)
            }
            Type::RawPtr(mutability, pointee) => {
                let pointee_resolved = self.fully_resolve_type(pointee);
                self.types.mk_raw_ptr(mutability, pointee_resolved)
            }
            Type::Array(elem, len) => {
                let elem_resolved = self.fully_resolve_type(elem);
                self.types.mk_array(elem_resolved, len)
            }
            Type::Slice(elem) => {
                let elem_resolved = self.fully_resolve_type(elem);
                self.types.mk_slice(elem_resolved)
            }
            Type::Tuple(elems) => {
                let resolved_elems: Vec<_> =
                    elems.iter().map(|e| self.fully_resolve_type(*e)).collect();
                self.types.mk_tuple(resolved_elems)
            }
            Type::Struct(def_id, args) => {
                let resolved_args: Vec<_> =
                    args.iter().map(|a| self.fully_resolve_type(*a)).collect();
                self.types.mk_struct(def_id, resolved_args)
            }
            Type::FnPtr { params, ret } => {
                let resolved_params: Vec<_> =
                    params.iter().map(|p| self.fully_resolve_type(*p)).collect();
                let resolved_ret = self.fully_resolve_type(ret);
                self.types.mk_fn_ptr(resolved_params, resolved_ret)
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
    use crate::sema::infer::UnifyError;
    use crate::sema::resolver::ResolveResult;
    use crate::sema::resolver::resolve;
    use crate::sema::symbol::DefId;
    use rowan::ast::AstNode;

    /// Helper to create a minimal `ResolveResult` for testing.
    fn create_test_resolve_result() -> ResolveResult {
        // Parse minimal source to get a valid ResolveResult
        let source = "fn main() {}";
        let parse_result = parse(source);
        let source_file = SourceFile::cast(parse_result.syntax()).unwrap();
        resolve(&source_file)
    }

    /// Helper to create a minimal `InferEngine` for testing `has_cycle`.
    fn create_test_engine(resolve_result: &ResolveResult) -> InferEngine<'_> {
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
        let resolve_result = create_test_resolve_result();
        let engine = create_test_engine(&resolve_result);
        let var = TypeVar::new(999); // Any unbound variable
        assert!(!engine.has_cycle(var));
    }

    #[test]
    fn has_cycle_single_step_to_concrete_returns_false() {
        // Chain: v0 -> i32 (concrete type)
        // The algorithm should terminate when it reaches a concrete type.
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let v0 = engine.fresh_type_var();
        let var = engine.extract_type_var(v0).unwrap();

        // Bind v0 to i32 (a concrete type)
        engine.substitution.insert(var, engine.types.i32());

        assert!(!engine.has_cycle(var));
    }

    #[test]
    fn has_cycle_chain_to_concrete_returns_false() {
        // Chain: v0 -> v1 -> v2 -> i32 (concrete)
        // Tests O(n) traversal of acyclic chain.
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);

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
        engine.substitution.insert(v2, engine.types.i32());

        assert!(!engine.has_cycle(v0));
    }

    #[test]
    fn has_cycle_self_loop_returns_true() {
        // Cycle: v0 -> v0 (1-node cycle)
        // Tests detection of simplest possible cycle.
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let v0_id = engine.fresh_type_var();
        let v0 = engine.extract_type_var(v0_id).unwrap();

        // Create self-loop: v0 -> v0
        engine.substitution.insert(v0, v0_id);

        assert!(engine.has_cycle(v0));
    }

    #[test]
    fn has_cycle_two_node_cycle_returns_true() {
        // Cycle: v0 -> v1 -> v0 (2-node cycle)
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);

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
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);

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
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);

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
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);

        // Create some variables but don't bind the one we check
        let v0_id = engine.fresh_type_var();
        let v1_id = engine.fresh_type_var();
        let v2_id = engine.fresh_type_var(); // unbound

        let v0 = engine.extract_type_var(v0_id).unwrap();
        let v2 = engine.extract_type_var(v2_id).unwrap();

        // Bind v0 -> v1, but v2 is unbound
        engine.substitution.insert(v0, v1_id);

        assert!(!engine.has_cycle(v2));
    }

    // =========================================================================
    // Unification Error Tests
    // =========================================================================
    //
    // These tests verify that unify() returns appropriate UnifyError variants
    // for different type mismatch scenarios.

    #[test]
    fn unify_err_primitive_mismatch() {
        // i32 vs bool should fail with TypeMismatch
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let i32_ty = engine.types.i32();
        let bool_ty = engine.types.bool();

        let result = engine.unify(i32_ty, bool_ty);
        assert!(matches!(result, Err(UnifyError::TypeMismatch { .. })));
    }

    #[test]
    fn unify_err_mutability_shared_to_mut() {
        // unify(expected=&mut i32, actual=&i32) should fail
        // Can't use a shared reference where mutable is expected
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let i32_ty = engine.types.i32();
        let shared_ref = engine.types.mk_ref(Mutability::Shared, i32_ty);
        let mut_ref = engine.types.mk_ref(Mutability::Mutable, i32_ty);

        // unify(expected=&mut, actual=&) should fail - can't gain mutability
        let result = engine.unify(mut_ref, shared_ref);
        assert!(matches!(result, Err(UnifyError::MutabilityMismatch { .. })));
    }

    #[test]
    fn unify_err_tuple_arity() {
        // (i32, i32) vs (i32,) should fail with ArityMismatch
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let i32_ty = engine.types.i32();
        let tuple2 = engine.types.mk_tuple(vec![i32_ty, i32_ty]);
        let tuple1 = engine.types.mk_tuple(vec![i32_ty]);

        let result = engine.unify(tuple2, tuple1);
        assert!(matches!(
            result,
            Err(UnifyError::ArityMismatch {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn unify_err_fn_ptr_arity() {
        // fn(i32) vs fn(i32, i32) should fail with ArityMismatch
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let i32_ty = engine.types.i32();
        let unit_ty = engine.types.unit();
        let fn1 = engine.types.mk_fn_ptr(vec![i32_ty], unit_ty);
        let fn2 = engine.types.mk_fn_ptr(vec![i32_ty, i32_ty], unit_ty);

        let result = engine.unify(fn1, fn2);
        assert!(matches!(
            result,
            Err(UnifyError::ArityMismatch {
                expected: 1,
                actual: 2
            })
        ));
    }

    #[test]
    fn unify_err_array_length() {
        // [i32; 5] vs [i32; 10] should fail with ArrayLengthMismatch
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let i32_ty = engine.types.i32();
        let arr5 = engine.types.mk_array(i32_ty, 5);
        let arr10 = engine.types.mk_array(i32_ty, 10);

        let result = engine.unify(arr5, arr10);
        assert!(matches!(
            result,
            Err(UnifyError::ArrayLengthMismatch {
                expected: 5,
                actual: 10
            })
        ));
    }

    #[test]
    fn unify_err_int_var_vs_bool() {
        // {int} vs bool should fail with ConstraintViolation
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let int_var = engine.fresh_int_var();
        let bool_ty = engine.types.bool();

        let result = engine.unify(int_var, bool_ty);
        assert!(matches!(
            result,
            Err(UnifyError::ConstraintViolation {
                kind: InferKind::Int,
                ..
            })
        ));
    }

    #[test]
    fn unify_err_float_var_vs_int() {
        // {float} vs i32 should fail with ConstraintViolation
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let float_var = engine.fresh_float_var();
        let i32_ty = engine.types.i32();

        let result = engine.unify(float_var, i32_ty);
        assert!(matches!(
            result,
            Err(UnifyError::ConstraintViolation {
                kind: InferKind::Float,
                ..
            })
        ));
    }

    #[test]
    fn unify_err_int_var_vs_float_var() {
        // {int} vs {float} should fail with ConstraintViolation
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let int_var = engine.fresh_int_var();
        let float_var = engine.fresh_float_var();

        let result = engine.unify(int_var, float_var);
        assert!(
            result.is_err(),
            "expected unification of {{int}} and {{float}} to fail"
        );
    }

    #[test]
    fn unify_err_struct_def_mismatch() {
        // Foo vs Bar (different struct DefIds) should fail with TypeMismatch
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let foo_id = DefId::new(1000); // Fake DefId for struct Foo
        let bar_id = DefId::new(1001); // Fake DefId for struct Bar
        let foo_ty = engine.types.mk_struct(foo_id, vec![]);
        let bar_ty = engine.types.mk_struct(bar_id, vec![]);

        let result = engine.unify(foo_ty, bar_ty);
        assert!(matches!(result, Err(UnifyError::TypeMismatch { .. })));
    }

    #[test]
    fn unify_ok_same_type() {
        // i32 vs i32 should succeed
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let i32_ty = engine.types.i32();

        let result = engine.unify(i32_ty, i32_ty);
        assert!(result.is_ok());
    }

    #[test]
    fn unify_ok_mut_to_shared() {
        // unify(expected=&i32, actual=&mut i32) should succeed
        // Mutable reference can satisfy shared reference expectation
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let i32_ty = engine.types.i32();
        let mut_ref = engine.types.mk_ref(Mutability::Mutable, i32_ty);
        let shared_ref = engine.types.mk_ref(Mutability::Shared, i32_ty);

        // unify(expected=&, actual=&mut) should succeed - &mut can coerce to &
        let result = engine.unify(shared_ref, mut_ref);
        assert!(result.is_ok());
    }

    #[test]
    fn unify_ok_int_var_to_i64() {
        // {int} vs i64 should succeed
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let int_var = engine.fresh_int_var();
        let i64_ty = engine.types.i64();

        let result = engine.unify(int_var, i64_ty);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Phase 3: Occurs Check Tests
    // =========================================================================

    #[test]
    fn unify_err_occurs_check_tuple() {
        // ?T vs (?T,) should fail with InfiniteType
        // Unifying these would create ?T = (?T,) = ((?T,),) = ...
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let var = engine.fresh_type_var();
        let tuple_containing_var = engine.types.mk_tuple(vec![var]);

        let result = engine.unify(var, tuple_containing_var);
        assert!(
            matches!(result, Err(UnifyError::InfiniteType { .. })),
            "expected InfiniteType error, got {result:?}"
        );
    }

    #[test]
    fn unify_err_occurs_check_array() {
        // ?T vs [?T; 5] should fail with InfiniteType
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let var = engine.fresh_type_var();
        let array_containing_var = engine.types.mk_array(var, 5);

        let result = engine.unify(var, array_containing_var);
        assert!(
            matches!(result, Err(UnifyError::InfiniteType { .. })),
            "expected InfiniteType error, got {result:?}"
        );
    }

    #[test]
    fn unify_err_occurs_check_ref() {
        // ?T vs &?T should fail with InfiniteType
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let var = engine.fresh_type_var();
        let ref_to_var = engine.types.mk_ref(Mutability::Shared, var);

        let result = engine.unify(var, ref_to_var);
        assert!(
            matches!(result, Err(UnifyError::InfiniteType { .. })),
            "expected InfiniteType error, got {result:?}"
        );
    }

    #[test]
    fn unify_err_occurs_check_nested() {
        // ?T vs ((?T,),) should fail with InfiniteType (nested occurrence)
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let var = engine.fresh_type_var();
        let inner_tuple = engine.types.mk_tuple(vec![var]);
        let outer_tuple = engine.types.mk_tuple(vec![inner_tuple]);

        let result = engine.unify(var, outer_tuple);
        assert!(
            matches!(result, Err(UnifyError::InfiniteType { .. })),
            "expected InfiniteType error, got {result:?}"
        );
    }

    #[test]
    fn unify_ok_different_vars_in_tuple() {
        // ?T vs (?U,) should succeed - different type variables
        let resolve_result = create_test_resolve_result();
        let mut engine = create_test_engine(&resolve_result);
        let var_t = engine.fresh_type_var();
        let var_u = engine.fresh_type_var();
        let tuple_containing_u = engine.types.mk_tuple(vec![var_u]);

        let result = engine.unify(var_t, tuple_containing_u);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }
}
