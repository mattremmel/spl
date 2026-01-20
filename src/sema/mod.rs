//! Semantic analysis phase for SPL.
//!
//! This module provides the symbol table infrastructure for name resolution and type checking.

pub mod scope;
pub mod symbol;

pub use scope::{Scope, ScopeId, ScopeKind};
pub use symbol::{DefId, Symbol, SymbolKind, Visibility};

use crate::lexer::Span;
use lasso::{Rodeo, Spur};

/// The central context for semantic analysis.
///
/// Owns the string interner, symbol table, and scope hierarchy.
pub struct SemanticContext {
    interner: Rodeo,
    symbols: Vec<Symbol>,
    scopes: Vec<Scope>,
    current_scope: ScopeId,
}

impl Default for SemanticContext {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticContext {
    /// Create a new semantic context with a root module scope.
    pub fn new() -> Self {
        let mut scopes = Vec::new();
        let root_scope = Scope::new(ScopeId(0), ScopeKind::Module, None);
        scopes.push(root_scope);

        Self {
            interner: Rodeo::default(),
            symbols: Vec::new(),
            scopes,
            current_scope: ScopeId(0),
        }
    }

    // ===== Interning =====

    /// Intern a string, returning a unique identifier.
    pub fn intern(&mut self, s: &str) -> Spur {
        self.interner.get_or_intern(s)
    }

    /// Resolve an interned string back to its original value.
    pub fn resolve(&self, spur: Spur) -> &str {
        self.interner.resolve(&spur)
    }

    // ===== Scope Management =====

    /// Enter a new scope of the given kind, returning its ID.
    pub fn enter_scope(&mut self, kind: ScopeKind) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        let scope = Scope::new(id, kind, Some(self.current_scope));
        self.scopes.push(scope);
        self.current_scope = id;
        id
    }

    /// Exit the current scope, returning to the parent scope.
    ///
    /// # Panics
    /// Panics if called when already at the root scope.
    pub fn exit_scope(&mut self) {
        let current = &self.scopes[self.current_scope.0 as usize];
        self.current_scope = current
            .parent
            .expect("cannot exit root scope");
    }

    /// Get the current scope ID.
    pub fn current_scope_id(&self) -> ScopeId {
        self.current_scope
    }

    /// Get a scope by its ID.
    pub fn get_scope(&self, scope_id: ScopeId) -> &Scope {
        &self.scopes[scope_id.0 as usize]
    }

    // ===== Symbol Definition & Lookup =====

    /// Define a new symbol in the current scope.
    ///
    /// Returns `Ok(DefId)` if the symbol was successfully defined, or `Err(DefId)`
    /// with the existing definition's ID if a symbol with the same name already
    /// exists in the current scope.
    pub fn define(
        &mut self,
        name: Spur,
        kind: SymbolKind,
        visibility: Visibility,
        span: Span,
    ) -> Result<DefId, DefId> {
        let def_id = DefId(self.symbols.len() as u32);
        let scope_id = self.current_scope;

        // Try to define in current scope
        let scope = &mut self.scopes[scope_id.0 as usize];
        scope.define(name, def_id)?;

        // Success - create the symbol
        let symbol = Symbol::new(def_id, name, kind, visibility, span, scope_id);
        self.symbols.push(symbol);

        Ok(def_id)
    }

    /// Look up a symbol by name, searching the current scope and all parent scopes.
    ///
    /// Returns the DefId of the first matching symbol found, or None if not found.
    pub fn lookup(&self, name: Spur) -> Option<DefId> {
        let mut scope_id = Some(self.current_scope);

        while let Some(id) = scope_id {
            let scope = &self.scopes[id.0 as usize];
            if let Some(def_id) = scope.lookup(name) {
                return Some(def_id);
            }
            scope_id = scope.parent;
        }

        None
    }

    /// Look up a symbol in a specific scope only (not parent scopes).
    pub fn lookup_in_scope(&self, name: Spur, scope_id: ScopeId) -> Option<DefId> {
        self.scopes[scope_id.0 as usize].lookup(name)
    }

    /// Get a symbol by its DefId.
    pub fn get_symbol(&self, def_id: DefId) -> &Symbol {
        &self.symbols[def_id.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_same_string_returns_same_spur() {
        let mut ctx = SemanticContext::new();
        let spur1 = ctx.intern("foo");
        let spur2 = ctx.intern("foo");
        assert_eq!(spur1, spur2);
    }

    #[test]
    fn test_intern_different_strings_return_different_spurs() {
        let mut ctx = SemanticContext::new();
        let spur1 = ctx.intern("foo");
        let spur2 = ctx.intern("bar");
        assert_ne!(spur1, spur2);
    }

    #[test]
    fn test_resolve_returns_original_string() {
        let mut ctx = SemanticContext::new();
        let spur = ctx.intern("hello");
        assert_eq!(ctx.resolve(spur), "hello");
    }

    #[test]
    fn test_scope_enter_and_exit() {
        let mut ctx = SemanticContext::new();
        let root = ctx.current_scope_id();

        let block = ctx.enter_scope(ScopeKind::Block);
        assert_ne!(root, block);
        assert_eq!(ctx.current_scope_id(), block);

        ctx.exit_scope();
        assert_eq!(ctx.current_scope_id(), root);
    }

    #[test]
    #[should_panic(expected = "cannot exit root scope")]
    fn test_exit_root_scope_panics() {
        let mut ctx = SemanticContext::new();
        ctx.exit_scope();
    }

    #[test]
    fn test_define_and_lookup() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("main");

        let def_id = ctx
            .define(name, SymbolKind::Function, Visibility::Public, 0..4)
            .unwrap();

        let found = ctx.lookup(name);
        assert_eq!(found, Some(def_id));

        let symbol = ctx.get_symbol(def_id);
        assert_eq!(symbol.name, name);
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert_eq!(symbol.visibility, Visibility::Public);
    }

    #[test]
    fn test_lookup_not_found() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("nonexistent");
        assert_eq!(ctx.lookup(name), None);
    }

    #[test]
    fn test_symbol_not_visible_after_scope_exit() {
        let mut ctx = SemanticContext::new();

        ctx.enter_scope(ScopeKind::Block);
        let name = ctx.intern("x");
        ctx.define(name, SymbolKind::Local, Visibility::Private, 0..1)
            .unwrap();
        ctx.exit_scope();

        assert_eq!(ctx.lookup(name), None);
    }

    #[test]
    fn test_shadowing_in_nested_scope() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("x");

        let outer_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 0..1)
            .unwrap();

        ctx.enter_scope(ScopeKind::Block);
        let inner_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 10..11)
            .unwrap();

        // Inner scope sees inner definition
        assert_eq!(ctx.lookup(name), Some(inner_def));

        ctx.exit_scope();

        // Outer scope sees outer definition
        assert_eq!(ctx.lookup(name), Some(outer_def));
    }

    #[test]
    fn test_duplicate_in_same_scope_returns_error() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("x");

        let first_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 0..1)
            .unwrap();

        let result = ctx.define(name, SymbolKind::Local, Visibility::Private, 10..11);

        assert_eq!(result, Err(first_def));
    }

    #[test]
    fn test_lookup_finds_symbol_in_parent_scope() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("outer_var");

        let outer_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 0..9)
            .unwrap();

        ctx.enter_scope(ScopeKind::Block);
        ctx.enter_scope(ScopeKind::Block);

        // Two levels deep, should still find outer_var
        assert_eq!(ctx.lookup(name), Some(outer_def));
    }

    #[test]
    fn test_lookup_in_scope_does_not_search_parents() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("x");

        ctx.define(name, SymbolKind::Local, Visibility::Private, 0..1)
            .unwrap();

        let inner_scope = ctx.enter_scope(ScopeKind::Block);

        // lookup_in_scope should not find it in the inner scope
        assert_eq!(ctx.lookup_in_scope(name, inner_scope), None);

        // but regular lookup should find it
        assert!(ctx.lookup(name).is_some());
    }
}
