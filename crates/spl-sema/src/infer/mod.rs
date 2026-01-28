//! Bidirectional type inference for SPL.
//!
//! This module implements a bidirectional type inference algorithm that:
//! - Synthesizes types bottom-up from expressions
//! - Checks types top-down from expected types
//! - Unifies type constraints to resolve inference variables

mod engine;
mod helpers;
mod synth;
mod toplevel;
mod unify;

#[cfg(test)]
mod tests;

use spl_ast::SourceFile;
use spl_diagnostic::Diagnostic;
use spl_lexer::Span;
use crate::SemanticContext;
use crate::SymbolKind;
use crate::resolver::ResolveResult;
use crate::symbol::DefId;
use crate::types::{InferKind, Mutability, Type, TypeId, TypeInterner};
use rustc_hash::FxHashMap;

use engine::InferEngine;

pub use engine::IntrinsicKind;

/// Error returned when unification fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnifyError {
    /// Two types are fundamentally incompatible.
    TypeMismatch { expected: TypeId, actual: TypeId },

    /// Mutability mismatch (&T vs &mut T).
    MutabilityMismatch {
        expected: Mutability,
        actual: Mutability,
    },

    /// Tuple/function arity mismatch.
    ArityMismatch { expected: usize, actual: usize },

    /// Array length mismatch.
    ArrayLengthMismatch { expected: u64, actual: u64 },

    /// Constrained variable violated (Int var vs non-integer).
    ConstraintViolation { kind: InferKind, actual: TypeId },

    /// Occurs check failed: would create an infinite type.
    /// E.g., trying to unify ?T with (?T,) would create ?T = (?T,) = ((?T,),) = ...
    InfiniteType { var: TypeId, ty: TypeId },
}

/// Result of type inference.
pub struct InferResult {
    /// Type interner with all types created during inference.
    pub types: TypeInterner,
    /// Map from expression spans to their inferred types.
    pub expr_types: FxHashMap<Span, TypeId>,
    /// Map from local bindings (`DefId`) to their inferred types.
    pub binding_types: FxHashMap<DefId, TypeId>,
    /// Map from spans to resolved `DefIds` (preserved from resolution).
    pub resolutions: FxHashMap<Span, DefId>,
    /// Map from method call expression spans to their resolved method `DefIds`.
    pub method_resolutions: FxHashMap<Span, DefId>,
    /// Map from type annotation spans to their resolved `TypeIds`.
    /// Includes return type annotations (-> i32), parameter types (x: bool), etc.
    pub type_annotation_types: FxHashMap<Span, TypeId>,
    /// Intrinsic methods that need special lowering during HIR lowering.
    /// Maps method `DefId` to how it should be lowered (e.g., `str.ptr()` -> field 0).
    pub intrinsic_methods: FxHashMap<DefId, IntrinsicKind>,
    /// Diagnostics produced during inference.
    pub diagnostics: Vec<Diagnostic>,
}

impl InferResult {
    /// Display the type of the last let binding in the source (by position).
    /// Used for testing.
    pub fn display_first_binding(&self, ctx: &SemanticContext) -> String {
        // Find the last binding by source position (largest span start)
        let mut best: Option<(DefId, TypeId, usize)> = None;

        for (&def_id, &type_id) in &self.binding_types {
            let symbol = ctx.get_symbol(def_id);
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
            Some((_, type_id, _)) => self.type_to_string(type_id, ctx),
            None => "???".to_string(),
        }
    }

    /// Convert a type ID to a human-readable string.
    pub fn type_to_string(&self, type_id: TypeId, ctx: &SemanticContext) -> String {
        let ty = self.types.get(type_id);
        self.type_repr(ty, type_id, ctx)
    }

    fn type_repr(&self, ty: &Type, _type_id: TypeId, ctx: &SemanticContext) -> String {
        match ty {
            Type::Primitive(prim) => prim.as_str().to_string(),
            Type::Infer(var, kind) => match kind {
                InferKind::General => format!("?{}", var.index()),
                InferKind::Int => format!("?int{}", var.index()),
                InferKind::Float => format!("?float{}", var.index()),
            },
            Type::Ref(mutability, inner) => {
                let inner_str = self.type_to_string(*inner, ctx);
                match mutability {
                    Mutability::Shared => format!("&{inner_str}"),
                    Mutability::Mutable => format!("&mut {inner_str}"),
                }
            }
            Type::RawPtr(mutability, pointee) => {
                let pointee_str = self.type_to_string(*pointee, ctx);
                match mutability {
                    Mutability::Shared => format!("*{pointee_str}"),
                    Mutability::Mutable => format!("*mut {pointee_str}"),
                }
            }
            Type::Array(elem, len) => {
                let elem_str = self.type_to_string(*elem, ctx);
                format!("[{elem_str}; {len}]")
            }
            Type::Slice(elem) => {
                let elem_str = self.type_to_string(*elem, ctx);
                format!("[{elem_str}]")
            }
            Type::Tuple(elems) => {
                if elems.is_empty() {
                    "()".to_string()
                } else if elems.len() == 1 {
                    let elem_str = self.type_to_string(elems[0], ctx);
                    format!("({elem_str},)")
                } else {
                    let elem_strs: Vec<_> =
                        elems.iter().map(|e| self.type_to_string(*e, ctx)).collect();
                    format!("({})", elem_strs.join(", "))
                }
            }
            Type::Struct(def_id, _type_args) => {
                let symbol = ctx.get_symbol(*def_id);
                ctx.resolve(symbol.name).to_string()
            }
            Type::FnPtr { params, ret } => {
                let param_strs: Vec<_> = params
                    .iter()
                    .map(|p| self.type_to_string(*p, ctx))
                    .collect();
                let ret_str = self.type_to_string(*ret, ctx);
                format!("fn({}) -> {}", param_strs.join(", "), ret_str)
            }
            Type::StrRef => "str".to_string(),
            Type::Error => "<error>".to_string(),
            Type::Alias(_, _) => "<alias>".to_string(),
            Type::Module(def_id) => {
                let symbol = ctx.get_symbol(*def_id);
                format!("module {}", ctx.resolve(symbol.name))
            }
            Type::Param(def_id) => {
                let symbol = ctx.get_symbol(*def_id);
                ctx.resolve(symbol.name).to_string()
            }
            Type::SelfType => "Self".to_string(),
        }
    }
}

/// Run type inference on a source file.
///
/// Takes the resolved AST and produces type assignments for all expressions and bindings.
/// The `ResolveResult` is borrowed, allowing it to be reused after inference completes.
pub fn infer(source_file: &SourceFile, resolve_result: &ResolveResult) -> InferResult {
    let mut engine = InferEngine::new(resolve_result);
    engine.infer_source_file(source_file);
    engine.apply_defaults();
    engine.into_result()
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
