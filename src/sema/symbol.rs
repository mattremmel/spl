//! Symbol definitions for the semantic analysis phase.

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
