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
//! - `binding_types`: Maps DefIds (locals, params) to their types
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
use crate::sema::resolver::ResolveResult;
use crate::sema::symbol::DefId;
use crate::sema::types::{TypeId, TypeVar};
use crate::sema::SemanticContext;
use rustc_hash::FxHashMap;

use super::{InferResult, SelfParam};

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

/// Function signature information.
#[derive(Clone)]
pub(super) struct FnSignature {
    /// The self parameter for methods, if present.
    /// Currently stored for future use (e.g., proper receiver type checking).
    #[allow(dead_code)]
    pub(super) self_param: Option<SelfParam>,
    /// Type parameters for generic functions (e.g., `T` in `fn foo<T>()`).
    pub(super) type_params: Vec<DefId>,
    pub(super) params: Vec<(String, TypeId)>,
    pub(super) ret: TypeId,
}

/// The type inference engine.
///
/// Holds all state needed for type inference, including the semantic context
/// (symbol table, type interner), inference results, and contextual information
/// about the current position in the AST (current function, loop, etc.).
pub(super) struct InferEngine {
    /// The semantic context containing symbol table and type interner.
    pub(super) ctx: SemanticContext,

    /// Name resolutions from the resolver phase (span → DefId).
    pub(super) resolutions: FxHashMap<Span, DefId>,

    // === Inference Results ===

    /// Map from expression spans to their inferred types.
    /// Populated during the inference pass as expressions are visited.
    pub(super) expr_types: FxHashMap<Span, TypeId>,

    /// Map from local bindings (DefId) to their inferred types.
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

    /// Struct field info: maps struct DefId to (field_name, field_type) pairs.
    pub(super) struct_fields: FxHashMap<DefId, Vec<(String, TypeId)>>,

    /// Struct type parameters: maps struct DefId to its generic param DefIds.
    pub(super) struct_type_params: FxHashMap<DefId, Vec<DefId>>,

    /// Methods associated with each struct (struct DefId → method DefIds).
    pub(super) struct_methods: FxHashMap<DefId, Vec<DefId>>,

    /// Type alias targets collected in first pass (alias DefId → resolved type).
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

    /// Method resolutions: maps method call spans to resolved method DefIds.
    /// Separate from `resolutions` because method lookup happens during inference.
    pub(super) method_resolutions: FxHashMap<Span, DefId>,
}

impl InferEngine {
    pub(super) fn new(resolve_result: ResolveResult) -> Self {
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

    pub(super) fn into_result(self) -> InferResult {
        InferResult {
            ctx: self.ctx,
            expr_types: self.expr_types,
            binding_types: self.binding_types,
            method_resolutions: self.method_resolutions,
            diagnostics: self.diagnostics,
        }
    }

    // =========================================================================
    // Type Variable Creation
    // =========================================================================

    /// Create a fresh type variable.
    pub(super) fn fresh_type_var(&mut self) -> TypeId {
        self.ctx.types.fresh_type_var()
    }

    /// Create a fresh integer type variable (defaults to i32 if unconstrained).
    pub(super) fn fresh_int_var(&mut self) -> TypeId {
        self.ctx.types.fresh_int_var()
    }

    /// Create a fresh float type variable (defaults to f64 if unconstrained).
    pub(super) fn fresh_float_var(&mut self) -> TypeId {
        self.ctx.types.fresh_float_var()
    }
}
