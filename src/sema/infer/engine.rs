//! The type inference engine.

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
pub(super) struct InferEngine {
    pub(super) ctx: SemanticContext,
    pub(super) resolutions: FxHashMap<Span, DefId>,
    /// Map from expression spans to their inferred types.
    pub(super) expr_types: FxHashMap<Span, TypeId>,
    /// Map from local bindings (DefId) to their inferred types.
    pub(super) binding_types: FxHashMap<DefId, TypeId>,
    /// Type substitution table for union-find.
    pub(super) substitution: FxHashMap<TypeVar, TypeId>,
    /// Collected diagnostics.
    pub(super) diagnostics: Vec<Diagnostic>,
    /// Function signatures collected in first pass.
    pub(super) fn_signatures: FxHashMap<DefId, FnSignature>,
    /// Struct field info collected in first pass.
    pub(super) struct_fields: FxHashMap<DefId, Vec<(String, TypeId)>>,
    /// Struct type parameters collected in first pass.
    pub(super) struct_type_params: FxHashMap<DefId, Vec<DefId>>,
    /// Map from struct DefId to its methods' DefIds.
    pub(super) struct_methods: FxHashMap<DefId, Vec<DefId>>,
    /// Type alias targets collected in first pass (alias DefId -> target type).
    pub(super) type_alias_targets: FxHashMap<DefId, TypeId>,
    /// Current function's return type (for return statements).
    pub(super) current_return_type: Option<TypeId>,
    /// Current loop's break type (for break statements with values).
    pub(super) current_loop_break_type: Option<TypeId>,
    /// Whether the current loop has a break statement (for detecting infinite loops).
    pub(super) current_loop_has_break: bool,
    /// Current impl block's Self type (for resolving Self in type positions).
    pub(super) current_self_type: Option<TypeId>,
    /// The kind of the innermost loop (for break/continue validation).
    pub(super) current_loop_kind: Option<LoopKind>,
    /// Map from method call expression spans to their resolved method DefIds.
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
