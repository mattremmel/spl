//! Symbol definitions for the semantic analysis phase.
//!
//! A symbol represents a named entity in the program: functions, structs,
//! variables, parameters, etc. Each symbol has a unique `DefId` that serves
//! as its identity throughout compilation.
//!
//! # DefId: The Universal Identifier
//!
//! `DefId` is a simple index into the symbol table. Once assigned, it never
//! changes, making it safe to store in:
//! - Resolution maps (span → DefId)
//! - Type information (binding_types: DefId → TypeId)
//! - HIR nodes (variable references store DefId, not names)
//!
//! # Symbol Kinds
//!
//! The `SymbolKind` distinguishes different definition types:
//! - **Function**: A function definition (including methods)
//! - **Struct**: A struct type definition
//! - **TypeAlias**: A type alias (`type Foo = Bar`)
//! - **Impl**: An impl block (tracked for method lookup)
//! - **Local**: A local variable (`let x = ...`)
//! - **Parameter**: A function parameter
//! - **Field**: A struct field
//! - **TypeParam**: A generic type parameter (`<T>`)
//! - **SelfParam**: The `self` parameter in methods
//!
//! # Mutability
//!
//! The `is_mutable` flag tracks whether a binding was declared with `mut`.
//! This is used by later phases to validate mutation and borrow checking.

use crate::lexer::Span;
use lasso::Spur;

use super::scope::ScopeId;

/// A unique identifier for each definition in the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefId(pub u32);

/// The kind of symbol being defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    TypeAlias,
    Impl,
    Local,
    Parameter,
    Field,
    TypeParam,
    SelfParam,
}

/// Visibility of a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
    Crate,
    Super,
    PubSelf,
}

/// A symbol in the symbol table.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub def_id: DefId,
    pub name: Spur,
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub span: Span,
    pub scope_id: ScopeId,
    /// Whether this symbol was declared with `mut`.
    pub is_mutable: bool,
}

impl Symbol {
    pub fn new(
        def_id: DefId,
        name: Spur,
        kind: SymbolKind,
        visibility: Visibility,
        span: Span,
        scope_id: ScopeId,
        is_mutable: bool,
    ) -> Self {
        Self {
            def_id,
            name,
            kind,
            visibility,
            span,
            scope_id,
            is_mutable,
        }
    }
}
