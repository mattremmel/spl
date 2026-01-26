//! The type inference engine.
//!
//! This module implements bidirectional type inference for SPL, combining
//! bottom-up inference (synthesizing types from expressions) with top-down
//! checking (propagating expected types downward).
//!
//! # Architecture
//!
//! The inference engine operates in two passes over the AST:
//!
//! 1. **Collection pass**: Gathers function signatures, struct definitions,
//!    and type alias targets. This allows forward references and mutual
//!    recursion between functions.
//!
//! 2. **Inference pass**: Walks the AST, creating type variables for unknowns
//!    and unifying types as constraints are discovered.
//!
//! # Key Data Structures
//!
//! - `substitution`: Maps type variables to their resolved types (union-find)
//! - `expr_types`: Maps expression spans to inferred types
//! - `binding_types`: Maps `DefIds` (locals, params) to their types
//! - `fn_signatures`: Pre-collected function signatures for call resolution
//!
//! # Bidirectional Flow
//!
//! Some expressions synthesize their type (literals, variables), while others
//! check against an expected type (return values, let bindings with annotations).
//! The engine tracks `current_return_type` and `current_loop_break_type` to
//! propagate these expectations.
//!
//! # Error Recovery
//!
//! When type errors occur, the engine records a diagnostic and continues with
//! an error type. This allows reporting multiple errors per compilation and
//! enables downstream phases to handle partial results gracefully.

use crate::diagnostic::Diagnostic;
use crate::lexer::Span;
use crate::sema::SemanticContext;
use crate::sema::resolver::ResolveResult;
use crate::sema::symbol::DefId;
use crate::sema::types::{Mutability, PrimitiveKind, TypeId, TypeInterner, TypeVar};
use rustc_hash::FxHashMap;

use super::{InferResult, SelfParam};

/// How to lower an intrinsic method during HIR lowering.
#[derive(Clone, Debug)]
pub enum IntrinsicKind {
    /// Lower to tuple field access (e.g., `str.ptr()` -> field 0)
    FieldAccess(u32),
}

/// The kind of loop for break/continue validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LoopKind {
    /// `loop { }` - allows break with value
    Loop,
    /// `while cond { }` - no break value allowed
    While,
    /// `for x in iter { }` - no break value allowed
    For,
}

/// Parameter information including label for named parameters.
#[derive(Clone)]
pub(super) struct ParamInfo {
    /// External label: None = positional (`_`), Some = labeled
    pub(super) label: Option<String>,
    /// Internal parameter name
    pub(super) name: String,
    /// Parameter type
    pub(super) ty: TypeId,
}

/// Function signature information.
#[derive(Clone)]
pub(super) struct FnSignature {
    /// The self parameter for methods, if present.
    /// Currently stored for future use (e.g., proper receiver type checking).
    #[allow(dead_code)]
    pub(super) self_param: Option<SelfParam>,
    /// Type parameters for generic functions (e.g., `T` in `fn foo<T>()`).
    pub(super) type_params: Vec<DefId>,
    /// Parameters with their labels, names, and types.
    pub(super) params: Vec<ParamInfo>,
    pub(super) ret: TypeId,
}

/// The type inference engine.
///
/// Holds all state needed for type inference, including references to the
/// semantic context (symbol table) and its own type interner, inference results,
/// and contextual information about the current position in the AST.
pub(super) struct InferEngine<'a> {
    /// Borrowed reference to the semantic context for symbol/scope lookup.
    pub(super) resolve_ctx: &'a SemanticContext,

    /// Current scope being type-checked. Used for visibility checking.
    /// This is separate from `resolve_ctx.current_scope` because we need to track
    /// which function/module we're checking from, not the scope chain state.
    pub(super) current_inference_scope: crate::sema::ScopeId,

    /// Owned type interner for creating types during inference.
    pub(super) types: TypeInterner,

    /// Name resolutions from the resolver phase (span → `DefId`).
    /// Cloned from `ResolveResult` to allow modification during inference.
    pub(super) resolutions: FxHashMap<Span, DefId>,

    // === Inference Results ===
    /// Map from expression spans to their inferred types.
    /// Populated during the inference pass as expressions are visited.
    pub(super) expr_types: FxHashMap<Span, TypeId>,

    /// Map from local bindings (`DefId`) to their inferred types.
    /// Includes locals, parameters, and other named bindings.
    pub(super) binding_types: FxHashMap<DefId, TypeId>,

    /// Type substitution table implementing union-find for type variables.
    /// When a type variable is unified with a type, an entry is added here.
    /// See `unify.rs` for the unification algorithm.
    pub(super) substitution: FxHashMap<TypeVar, TypeId>,

    /// Collected diagnostics (type errors, etc.).
    pub(super) diagnostics: Vec<Diagnostic>,

    // === Pre-collected Definitions (from collection pass) ===
    /// Function signatures collected in first pass, enabling forward references.
    pub(super) fn_signatures: FxHashMap<DefId, FnSignature>,

    /// Struct field info: maps struct `DefId` to (`field_name`, `field_type`, `field_def_id`) triples.
    pub(super) struct_fields: FxHashMap<DefId, Vec<(String, TypeId, DefId)>>,

    /// Struct type parameters: maps struct `DefId` to its generic param `DefIds`.
    pub(super) struct_type_params: FxHashMap<DefId, Vec<DefId>>,

    /// Methods associated with each struct (struct `DefId` → method `DefIds`).
    pub(super) struct_methods: FxHashMap<DefId, Vec<DefId>>,

    /// Type alias targets collected in first pass (alias `DefId` → resolved type).
    pub(super) type_alias_targets: FxHashMap<DefId, TypeId>,

    // === Context Stack (tracks position in AST) ===
    /// Current function's return type, for checking return statements.
    /// None when outside a function body.
    pub(super) current_return_type: Option<TypeId>,

    /// Current loop's expected break type, for `break expr` statements.
    /// None when outside a loop or in a loop that doesn't support break values.
    pub(super) current_loop_break_type: Option<TypeId>,

    /// Whether the current loop has at least one break statement.
    /// Used to determine if a `loop {}` expression might be infinite.
    pub(super) current_loop_has_break: bool,

    /// Current impl block's Self type, for resolving `Self` in type positions.
    /// Set when entering an impl block, cleared when exiting.
    pub(super) current_self_type: Option<TypeId>,

    /// The kind of innermost loop (loop/while/for).
    /// Used to validate break/continue: only `loop` allows `break value`.
    pub(super) current_loop_kind: Option<LoopKind>,

    /// Method resolutions: maps method call spans to resolved method `DefIds`.
    /// Separate from `resolutions` because method lookup happens during inference.
    pub(super) method_resolutions: FxHashMap<Span, DefId>,

    /// Map from type annotation spans to their `TypeIds`.
    /// Records the types of explicit type annotations like `-> i32`, `: bool`, etc.
    /// These are separate from `expr_types` because type annotations are not expressions.
    pub(super) type_annotation_types: FxHashMap<Span, TypeId>,

    // === Primitive Type Methods ===
    /// Methods on primitive types (`TypeId` → method `DefIds`).
    /// Similar to `struct_methods` but keyed by `TypeId` for primitives like str.
    pub(super) primitive_methods: FxHashMap<TypeId, Vec<DefId>>,

    /// Intrinsic methods that need special lowering during HIR lowering.
    /// Maps method `DefId` to how it should be lowered.
    pub intrinsic_methods: FxHashMap<DefId, IntrinsicKind>,

    /// Names of builtin methods (`DefId` → name).
    /// Used during method resolution since builtins aren't in the symbol table.
    pub(super) builtin_method_names: FxHashMap<DefId, String>,

    /// Map from module `DefId` to its scope ID (for qualified module access).
    /// Used to look up items within inline modules, e.g., `module.Item`.
    pub(super) module_scopes: FxHashMap<DefId, crate::sema::ScopeId>,
}

impl<'a> InferEngine<'a> {
    pub(super) fn new(resolve_result: &'a ResolveResult) -> Self {
        let mut engine = Self {
            resolve_ctx: &resolve_result.ctx,
            current_inference_scope: crate::sema::ScopeId(0), // Start at root scope
            types: TypeInterner::new(),
            resolutions: resolve_result.resolutions.clone(),
            expr_types: FxHashMap::default(),
            binding_types: FxHashMap::default(),
            substitution: FxHashMap::default(),
            diagnostics: Vec::new(), // Fresh diagnostics, not inherited
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
            type_annotation_types: FxHashMap::default(),
            primitive_methods: FxHashMap::default(),
            intrinsic_methods: FxHashMap::default(),
            builtin_method_names: FxHashMap::default(),
            module_scopes: resolve_result.module_scopes.clone(),
        };
        engine.register_builtin_primitive_methods();
        engine
    }

    /// Register methods on primitive types (str, etc.).
    ///
    /// This creates synthetic `DefIds` for builtin methods and registers them
    /// in the same structures used for struct methods, enabling unified
    /// method resolution.
    fn register_builtin_primitive_methods(&mut self) {
        let str_ty = self.types.str_ref();
        let usize_ty = self.types.primitive(PrimitiveKind::Usize);
        let u8_ty = self.types.primitive(PrimitiveKind::U8);
        let ptr_u8 = self.types.mk_raw_ptr(Mutability::Shared, u8_ty);

        // Create synthetic DefIds for ptr() and len() methods
        let ptr_def_id = self.create_builtin_method("ptr", str_ty, vec![], ptr_u8);
        let len_def_id = self.create_builtin_method("len", str_ty, vec![], usize_ty);

        // Register methods for str type
        self.primitive_methods
            .insert(str_ty, vec![ptr_def_id, len_def_id]);

        // Mark these as intrinsic methods that lower to field access
        self.intrinsic_methods
            .insert(ptr_def_id, IntrinsicKind::FieldAccess(0));
        self.intrinsic_methods
            .insert(len_def_id, IntrinsicKind::FieldAccess(1));
    }

    /// Create a builtin method with a synthetic `DefId` and register its signature.
    ///
    /// Returns a `DefId` for the builtin method. The `DefId` uses a high range
    /// (starting at `u32::MAX` / 2) to avoid conflicts with user-defined symbols.
    fn create_builtin_method(
        &mut self,
        name: &str,
        self_ty: TypeId,
        params: Vec<TypeId>,
        ret: TypeId,
    ) -> DefId {
        use super::SelfParamKind;

        // Use a high range for builtin DefIds to avoid conflicts
        // Start at u32::MAX / 2 and count up based on how many builtins we have
        let builtin_base = u32::MAX / 2;
        let def_id = DefId(builtin_base + self.builtin_method_names.len() as u32);

        // Store the method name for later lookup during resolution
        self.builtin_method_names.insert(def_id, name.to_string());

        // Register the function signature
        self.fn_signatures.insert(
            def_id,
            FnSignature {
                self_param: Some(SelfParam {
                    kind: SelfParamKind::Owned,
                    self_ty,
                }),
                type_params: vec![],
                params: params
                    .into_iter()
                    .map(|ty| ParamInfo {
                        label: None,
                        name: String::new(),
                        ty,
                    })
                    .collect(),
                ret,
            },
        );

        def_id
    }

    pub(super) fn into_result(self) -> InferResult {
        // Verify no INVALID DefIds made it into binding_types
        #[cfg(debug_assertions)]
        for def_id in self.binding_types.keys() {
            debug_assert!(
                def_id.is_valid(),
                "INVALID DefId found in binding_types after inference - resolution phase produced invalid binding"
            );
        }

        InferResult {
            types: self.types,
            expr_types: self.expr_types,
            binding_types: self.binding_types,
            resolutions: self.resolutions,
            method_resolutions: self.method_resolutions,
            type_annotation_types: self.type_annotation_types,
            intrinsic_methods: self.intrinsic_methods,
            diagnostics: self.diagnostics,
        }
    }

    // =========================================================================
    // Type Variable Creation
    // =========================================================================

    /// Create a fresh type variable.
    pub(super) fn fresh_type_var(&mut self) -> TypeId {
        self.types.fresh_type_var()
    }

    /// Create a fresh integer type variable (defaults to i32 if unconstrained).
    pub(super) fn fresh_int_var(&mut self) -> TypeId {
        self.types.fresh_int_var()
    }

    /// Create a fresh float type variable (defaults to f64 if unconstrained).
    pub(super) fn fresh_float_var(&mut self) -> TypeId {
        self.types.fresh_float_var()
    }

    // =========================================================================
    // Scope Helpers
    // =========================================================================

    /// Check if `potential_descendant` scope is the same as or a descendant of `ancestor` scope.
    /// This walks up the scope chain from `potential_descendant` to see if we reach `ancestor`.
    pub(super) fn is_scope_descendant_of(
        &self,
        potential_descendant: crate::sema::ScopeId,
        ancestor: crate::sema::ScopeId,
    ) -> bool {
        let mut current = potential_descendant;
        loop {
            if current == ancestor {
                return true;
            }
            let scope = self.resolve_ctx.get_scope(current);
            match scope.parent {
                Some(parent) => current = parent,
                None => return false, // Reached root without finding ancestor
            }
        }
    }
}
