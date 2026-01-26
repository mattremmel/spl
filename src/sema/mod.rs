//! Semantic analysis phase for SPL.
//!
//! This module provides the symbol table infrastructure for name resolution and type checking.
//!
//! # Contract Checking Philosophy
//!
//! This module uses `debug_assert!()` for **internal invariants** (programmer bugs)
//! and `Diagnostic` for **user errors** (invalid source code).
//!
//! ## When to use `debug_assert!()`
//!
//! Use `debug_assert!()` for conditions that should never be false unless there's
//! a bug in the compiler itself:
//!
//! - **ID validity**: `ScopeId`, `DefId`, and `TypeId` are created by the compiler,
//!   not from user input. If they're invalid, it's a compiler bug.
//! - **Scope balance**: `exit_scope()` being called at root is a compiler bug.
//! - **Internal data structure invariants**: Parent chain integrity, etc.
//!
//! These assertions are disabled in release builds for performance, but the
//! protected operations would result in undefined behavior or panics anyway
//! (out-of-bounds indexing), so the assertions serve as documentation and
//! help catch bugs early in debug builds.
//!
//! ## When to use `Diagnostic`
//!
//! Use `Diagnostic` for conditions caused by user code:
//!
//! - **Out-of-bounds tuple/array access**: `tuple.5` on a 3-element tuple
//! - **Type mismatches**: `let x: i32 = "hello"`
//! - **Undefined symbols**: Using a variable before declaration
//! - **Duplicate definitions**: Defining the same name twice in a scope
//!
//! These produce error messages that help users fix their code.
//!
//! # Diagnostic Strategy
//!
//! Semantic analysis produces [`Diagnostic`](crate::Diagnostic) errors through an
//! imperative collection pattern, where diagnostics are accumulated as analysis
//! proceeds.
//!
//! ## Flow Through Analysis Phases
//!
//! Diagnostics flow through the resolution and inference phases:
//!
//! 1. **Resolution Phase** ([`ResolveResult`]): Name resolution collects diagnostics
//!    for undefined symbols, duplicate definitions, and visibility violations.
//!    These are stored in `ResolveResult::diagnostics`.
//!
//! 2. **Inference Phase** ([`InferResult`]): Type inference inherits resolution
//!    diagnostics and adds type errors (mismatches, missing annotations, etc.).
//!    The combined diagnostics are available in `InferResult::diagnostics`.
//!
//! ## Imperative Collection Pattern
//!
//! Rather than using a `Result` type that short-circuits on the first error,
//! semantic analysis uses imperative collection:
//!
//! ```text
//! // Pseudocode for the pattern:
//! fn analyze() -> AnalysisResult {
//!     let mut diagnostics = Vec::new();
//!
//!     // Continue analysis even after errors
//!     if let Err(e) = check_something() {
//!         diagnostics.push(e.into_diagnostic());
//!         // Don't return early - keep analyzing
//!     }
//!
//!     // More analysis...
//!
//!     AnalysisResult { diagnostics, ... }
//! }
//! ```
//!
//! This pattern enables:
//! - **Multiple errors**: Users see all errors in one compilation, not just the first
//! - **Continued analysis**: Type inference can proceed on valid portions of code
//! - **Error recovery**: Later phases receive partial results and can handle gaps
//!
//! ## Builder Pattern for Diagnostics
//!
//! Rich diagnostics are constructed using the [`Diagnostic`](crate::Diagnostic)
//! builder pattern, which supports:
//! - Primary span and message
//! - Secondary labels with additional context
//! - Severity levels (error, warning, note)
//!
//! See the [`diagnostic`](crate::diagnostic) module for the full API.

#[cfg(test)]
mod contract_tests;

pub mod infer;
pub mod module;
pub mod resolver;
pub mod scope;
pub mod symbol;
pub mod types;

pub use infer::{InferResult, infer, infer_package};
pub use resolver::{ResolveResult, Resolver, resolve, resolve_package};
pub use scope::{Scope, ScopeId, ScopeKind};
pub use symbol::{DefId, Symbol, SymbolKind, Visibility};
pub use types::{Mutability, PrimitiveKind, Type, TypeId, TypeInterner, TypeVar};

use crate::lexer::Span;
use lasso::{Rodeo, Spur};

pub use module::{ModuleId, ModuleTree, PathResolveError};

/// The central context for semantic analysis.
///
/// Owns the string interner, symbol table, scope hierarchy, and type interner.
pub struct SemanticContext {
    /// String interner for symbol names.
    pub interner: Rodeo,
    symbols: Vec<Symbol>,
    scopes: Vec<Scope>,
    current_scope: ScopeId,
    /// Type interner for semantic types.
    pub types: TypeInterner,
    /// Module tree for cross-module resolution (None for single-file mode).
    pub module_tree: Option<ModuleTree>,
    /// Current module ID within the module tree.
    pub current_module: ModuleId,
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
            types: TypeInterner::new(),
            module_tree: None,
            current_module: ModuleId(0),
        }
    }

    /// Create a new semantic context with a module tree for multi-file compilation.
    pub fn with_module_tree(module_tree: ModuleTree, current_module: ModuleId) -> Self {
        let mut scopes = Vec::new();
        let root_scope = Scope::new(ScopeId(0), ScopeKind::Module, None);
        scopes.push(root_scope);

        Self {
            interner: Rodeo::default(),
            symbols: Vec::new(),
            scopes,
            current_scope: ScopeId(0),
            types: TypeInterner::new(),
            module_tree: Some(module_tree),
            current_module,
        }
    }

    // ===== Interning =====

    /// Intern a string, returning a unique identifier.
    pub fn intern(&mut self, s: &str) -> Spur {
        self.interner.get_or_intern(s)
    }

    /// Try to get an interned string's Spur without inserting.
    /// Returns None if the string has not been interned.
    pub fn try_get_interned(&self, s: &str) -> Option<Spur> {
        self.interner.get(s)
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
        debug_assert!(
            !self.is_at_root_scope(),
            "precondition: cannot exit root scope"
        );

        let current = &self.scopes[self.current_scope.0 as usize];
        self.current_scope = current.parent.expect("cannot exit root scope");
    }

    /// Set the current scope to an existing scope by ID.
    ///
    /// This is used to switch between module scopes during multi-file resolution.
    pub fn set_current_scope(&mut self, scope_id: ScopeId) {
        debug_assert!(
            self.is_valid_scope_id(scope_id),
            "precondition: scope_id {} must be valid (< {})",
            scope_id.0,
            self.scopes.len()
        );
        self.current_scope = scope_id;
    }

    /// Get the current scope ID.
    pub fn current_scope_id(&self) -> ScopeId {
        self.current_scope
    }

    /// Get a scope by its ID.
    pub fn get_scope(&self, scope_id: ScopeId) -> &Scope {
        debug_assert!(
            self.is_valid_scope_id(scope_id),
            "precondition: scope_id {} must be valid (< {})",
            scope_id.0,
            self.scopes.len()
        );
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
        is_mutable: bool,
    ) -> Result<DefId, DefId> {
        let def_id = DefId(self.symbols.len() as u32);
        let scope_id = self.current_scope;

        // Try to define in current scope
        let scope = &mut self.scopes[scope_id.0 as usize];
        scope.define(name, def_id)?;

        // Success - create the symbol
        let symbol = Symbol::new(def_id, name, kind, visibility, span, scope_id, is_mutable);
        self.symbols.push(symbol);

        Ok(def_id)
    }

    /// Define an alias (import binding) to an existing `DefId` in the current scope.
    ///
    /// Unlike `define`, this doesn't create a new symbol. Instead, it creates a
    /// name binding that resolves to an existing `DefId`. This is used for imports
    /// where we want `use foo as bar` to make `bar` resolve to the same `DefId` as `foo`.
    ///
    /// Returns `Ok(())` if the alias was created, or `Err(DefId)` if the name
    /// already exists in the current scope.
    pub fn define_alias(
        &mut self,
        name: Spur,
        target_def_id: DefId,
        _visibility: Visibility,
        _span: Span,
    ) -> Result<(), DefId> {
        debug_assert!(
            self.is_valid_def_id(target_def_id),
            "precondition: target_def_id {} must be valid (< {})",
            target_def_id.0,
            self.symbols.len()
        );

        let scope = &mut self.scopes[self.current_scope.0 as usize];
        scope.define(name, target_def_id)
    }

    /// Look up a symbol by name, searching the current scope and all parent scopes.
    ///
    /// Returns the `DefId` of the first matching symbol found, or None if not found.
    pub fn lookup(&self, name: Spur) -> Option<DefId> {
        let mut scope_id = Some(self.current_scope);

        while let Some(id) = scope_id {
            debug_assert!(
                self.is_valid_scope_id(id),
                "invariant: scope chain contains invalid scope_id {}",
                id.0
            );
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
        debug_assert!(
            self.is_valid_scope_id(scope_id),
            "precondition: scope_id {} must be valid (< {})",
            scope_id.0,
            self.scopes.len()
        );
        self.scopes[scope_id.0 as usize].lookup(name)
    }

    /// Get a symbol by its `DefId`.
    pub fn get_symbol(&self, def_id: DefId) -> &Symbol {
        debug_assert!(
            self.is_valid_def_id(def_id),
            "precondition: def_id {} must be valid (< {})",
            def_id.0,
            self.symbols.len()
        );
        &self.symbols[def_id.0 as usize]
    }

    /// Iterate over all symbols in the symbol table.
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter()
    }

    // ===== Contract Helpers =====

    /// Returns true if currently at the root scope (scope 0).
    /// Used for contract assertions to prevent exiting the root scope.
    pub fn is_at_root_scope(&self) -> bool {
        self.current_scope.0 == 0
    }

    /// Returns the current scope depth (number of scopes from root).
    /// Used for contract assertions to verify scope balance.
    pub fn scope_depth(&self) -> usize {
        let mut depth = 0;
        let mut scope_id = Some(self.current_scope);
        while let Some(id) = scope_id {
            if id.0 == 0 {
                break;
            }
            depth += 1;
            scope_id = self.scopes[id.0 as usize].parent;
        }
        depth
    }

    // ===== ID Validation Helpers =====

    /// Returns true if the given `ScopeId` is valid (within bounds).
    /// Used in debug assertions; trivial enough to always compile.
    fn is_valid_scope_id(&self, scope_id: ScopeId) -> bool {
        (scope_id.0 as usize) < self.scopes.len()
    }

    /// Returns true if the given `DefId` is valid (within bounds).
    /// Used in debug assertions; trivial enough to always compile.
    fn is_valid_def_id(&self, def_id: DefId) -> bool {
        (def_id.0 as usize) < self.symbols.len()
    }
}

// =============================================================================
// Visibility Helpers
// =============================================================================

/// Check if an item defined in `item_module` with `visibility` is visible from `accessor_module`.
///
/// # Visibility Rules
///
/// | Visibility | Rule |
/// |------------|------|
/// | Private (default) | Visible within current module and descendant submodules |
/// | Public (pub) | Visible to all |
/// | Crate (pub(crate)) | Future - visible within crate only (treated as Public for now) |
/// | Super (pub(super)) | Future - visible to parent module (treated as Public for now) |
/// | `PubSelf` (pub(self)) | Future - same as Private (treated as Public for now) |
///
/// Key semantics:
/// - Child modules CAN access parent's private items (descendants see ancestors' private items)
/// - Sibling modules CANNOT access each other's private items
/// - Ancestors can access items in descendants only if public
pub fn is_visible(
    visibility: Visibility,
    item_module: ModuleId,
    accessor_module: ModuleId,
    tree: &ModuleTree,
) -> bool {
    match visibility {
        // Public and future visibility levels are treated as public
        Visibility::Public | Visibility::Crate | Visibility::Super | Visibility::PubSelf => true,
        Visibility::Private => {
            // Private items are visible to same module and descendants
            // i.e., accessor must be item_module or a descendant of item_module
            is_same_or_descendant(accessor_module, item_module, tree)
        }
    }
}

/// Check if `potential_descendant` is the same as or a descendant of `ancestor`.
///
/// This walks up the module hierarchy from `potential_descendant` to see if we
/// eventually reach `ancestor`. Returns true if:
/// - `potential_descendant == ancestor` (same module), OR
/// - `potential_descendant` is nested inside `ancestor` (any depth)
pub fn is_same_or_descendant(
    potential_descendant: ModuleId,
    ancestor: ModuleId,
    tree: &ModuleTree,
) -> bool {
    let mut current = potential_descendant;
    loop {
        if current == ancestor {
            return true;
        }
        match tree.get(current).parent() {
            Some(parent) => current = parent,
            None => return false, // Reached root without finding ancestor
        }
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
            .define(name, SymbolKind::Function, Visibility::Public, 0..4, false)
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
        ctx.define(name, SymbolKind::Local, Visibility::Private, 0..1, false)
            .unwrap();
        ctx.exit_scope();

        assert_eq!(ctx.lookup(name), None);
    }

    #[test]
    fn test_shadowing_in_nested_scope() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("x");

        let outer_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 0..1, false)
            .unwrap();

        ctx.enter_scope(ScopeKind::Block);
        let inner_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 10..11, false)
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
            .define(name, SymbolKind::Local, Visibility::Private, 0..1, false)
            .unwrap();

        let result = ctx.define(name, SymbolKind::Local, Visibility::Private, 10..11, false);

        assert_eq!(result, Err(first_def));
    }

    #[test]
    fn test_lookup_finds_symbol_in_parent_scope() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("outer_var");

        let outer_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 0..9, false)
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

        ctx.define(name, SymbolKind::Local, Visibility::Private, 0..1, false)
            .unwrap();

        let inner_scope = ctx.enter_scope(ScopeKind::Block);

        // lookup_in_scope should not find it in the inner scope
        assert_eq!(ctx.lookup_in_scope(name, inner_scope), None);

        // but regular lookup should find it
        assert!(ctx.lookup(name).is_some());
    }

    #[test]
    fn test_symbol_span_stored_correctly() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("my_var");

        let def_id = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 42..50, false)
            .unwrap();

        let symbol = ctx.get_symbol(def_id);
        assert_eq!(symbol.span, 42..50);
    }

    #[test]
    fn test_symbol_scope_id_matches_definition_scope() {
        let mut ctx = SemanticContext::new();
        let root_scope = ctx.current_scope_id();

        let name1 = ctx.intern("root_sym");
        let def1 = ctx
            .define(name1, SymbolKind::Function, Visibility::Public, 0..8, false)
            .unwrap();

        let block_scope = ctx.enter_scope(ScopeKind::Block);
        let name2 = ctx.intern("block_sym");
        let def2 = ctx
            .define(name2, SymbolKind::Local, Visibility::Private, 10..19, false)
            .unwrap();

        assert_eq!(ctx.get_symbol(def1).scope_id, root_scope);
        assert_eq!(ctx.get_symbol(def2).scope_id, block_scope);
    }

    #[test]
    fn test_visibility_default_is_private() {
        assert_eq!(Visibility::default(), Visibility::Private);
    }

    #[test]
    fn test_all_visibility_variants_stored() {
        let mut ctx = SemanticContext::new();

        let visibilities = [
            Visibility::Private,
            Visibility::Public,
            Visibility::Crate,
            Visibility::Super,
            Visibility::PubSelf,
        ];

        for (i, vis) in visibilities.iter().enumerate() {
            let name = ctx.intern(&format!("sym_{i}"));
            let def_id = ctx
                .define(name, SymbolKind::Function, *vis, 0..1, false)
                .unwrap();
            assert_eq!(ctx.get_symbol(def_id).visibility, *vis);
        }
    }

    #[test]
    fn test_symbol_kind_impl() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("_impl");
        let def_id = ctx
            .define(name, SymbolKind::Impl, Visibility::Private, 0..1, false)
            .unwrap();
        assert_eq!(ctx.get_symbol(def_id).kind, SymbolKind::Impl);
    }

    #[test]
    fn test_all_symbol_kinds() {
        let mut ctx = SemanticContext::new();

        let kinds = [
            SymbolKind::Function,
            SymbolKind::Struct,
            SymbolKind::TypeAlias,
            SymbolKind::Impl,
            SymbolKind::Local,
            SymbolKind::Parameter,
            SymbolKind::Field,
            SymbolKind::TypeParam,
            SymbolKind::SelfParam,
        ];

        for (i, kind) in kinds.iter().enumerate() {
            let name = ctx.intern(&format!("sym_{i}"));
            let def_id = ctx
                .define(name, *kind, Visibility::Private, 0..1, false)
                .unwrap();
            assert_eq!(ctx.get_symbol(def_id).kind, *kind);
        }
    }

    #[test]
    fn test_all_scope_kinds() {
        let mut ctx = SemanticContext::new();

        // Root is Module
        assert_eq!(
            ctx.get_scope(ctx.current_scope_id()).kind,
            ScopeKind::Module
        );

        let function_scope = ctx.enter_scope(ScopeKind::Function);
        assert_eq!(ctx.get_scope(function_scope).kind, ScopeKind::Function);

        let block_scope = ctx.enter_scope(ScopeKind::Block);
        assert_eq!(ctx.get_scope(block_scope).kind, ScopeKind::Block);

        ctx.exit_scope();
        ctx.exit_scope();

        let impl_scope = ctx.enter_scope(ScopeKind::Impl);
        assert_eq!(ctx.get_scope(impl_scope).kind, ScopeKind::Impl);

        ctx.exit_scope();

        let for_scope = ctx.enter_scope(ScopeKind::ForLoop);
        assert_eq!(ctx.get_scope(for_scope).kind, ScopeKind::ForLoop);
    }

    #[test]
    fn test_get_scope_returns_correct_scope() {
        let mut ctx = SemanticContext::new();
        let root = ctx.current_scope_id();

        let scope1 = ctx.enter_scope(ScopeKind::Function);
        let scope2 = ctx.enter_scope(ScopeKind::Block);

        // Can retrieve any scope by ID
        assert_eq!(ctx.get_scope(root).kind, ScopeKind::Module);
        assert_eq!(ctx.get_scope(scope1).kind, ScopeKind::Function);
        assert_eq!(ctx.get_scope(scope2).kind, ScopeKind::Block);

        // Parent chain is correct
        assert_eq!(ctx.get_scope(root).parent, None);
        assert_eq!(ctx.get_scope(scope1).parent, Some(root));
        assert_eq!(ctx.get_scope(scope2).parent, Some(scope1));
    }

    #[test]
    fn test_multiple_symbols_in_same_scope() {
        let mut ctx = SemanticContext::new();

        let a = ctx.intern("a");
        let b = ctx.intern("b");
        let c = ctx.intern("c");

        let def_a = ctx
            .define(a, SymbolKind::Local, Visibility::Private, 0..1, false)
            .unwrap();
        let def_b = ctx
            .define(b, SymbolKind::Local, Visibility::Private, 2..3, false)
            .unwrap();
        let def_c = ctx
            .define(c, SymbolKind::Local, Visibility::Private, 4..5, false)
            .unwrap();

        assert_eq!(ctx.lookup(a), Some(def_a));
        assert_eq!(ctx.lookup(b), Some(def_b));
        assert_eq!(ctx.lookup(c), Some(def_c));
    }

    #[test]
    fn test_def_ids_are_unique_and_sequential() {
        let mut ctx = SemanticContext::new();

        let name1 = ctx.intern("first");
        let name2 = ctx.intern("second");
        let name3 = ctx.intern("third");

        let def1 = ctx
            .define(name1, SymbolKind::Local, Visibility::Private, 0..5, false)
            .unwrap();
        let def2 = ctx
            .define(name2, SymbolKind::Local, Visibility::Private, 6..12, false)
            .unwrap();
        let def3 = ctx
            .define(name3, SymbolKind::Local, Visibility::Private, 13..18, false)
            .unwrap();

        assert_eq!(def1.0, 0);
        assert_eq!(def2.0, 1);
        assert_eq!(def3.0, 2);
    }

    #[test]
    fn test_sibling_scopes_do_not_share_symbols() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("x");

        // Enter first sibling scope and define x
        ctx.enter_scope(ScopeKind::Block);
        ctx.define(name, SymbolKind::Local, Visibility::Private, 0..1, false)
            .unwrap();
        ctx.exit_scope();

        // Enter second sibling scope - x should not be visible
        ctx.enter_scope(ScopeKind::Block);
        assert_eq!(ctx.lookup(name), None);

        // Can define x again in this sibling
        let def2 = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 10..11, false)
            .unwrap();
        assert_eq!(ctx.lookup(name), Some(def2));
    }

    #[test]
    fn test_empty_string_interning() {
        let mut ctx = SemanticContext::new();
        let spur1 = ctx.intern("");
        let spur2 = ctx.intern("");

        assert_eq!(spur1, spur2);
        assert_eq!(ctx.resolve(spur1), "");
    }

    #[test]
    fn test_unicode_string_interning() {
        let mut ctx = SemanticContext::new();
        let spur = ctx.intern("héllo_wörld_日本語");

        assert_eq!(ctx.resolve(spur), "héllo_wörld_日本語");
    }

    #[test]
    fn test_deeply_nested_scopes() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("deep");

        let outer_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 0..4, false)
            .unwrap();

        // Nest 10 levels deep
        for _ in 0..10 {
            ctx.enter_scope(ScopeKind::Block);
        }

        // Should still find the symbol from root
        assert_eq!(ctx.lookup(name), Some(outer_def));

        // Exit all scopes
        for _ in 0..10 {
            ctx.exit_scope();
        }

        assert_eq!(ctx.lookup(name), Some(outer_def));
    }

    #[test]
    fn test_default_impl_for_semantic_context() {
        let ctx = SemanticContext::default();
        assert_eq!(ctx.current_scope_id(), ScopeId(0));
        assert_eq!(ctx.get_scope(ScopeId(0)).kind, ScopeKind::Module);
    }

    #[test]
    fn test_duplicate_does_not_create_symbol() {
        let mut ctx = SemanticContext::new();
        let name = ctx.intern("x");

        let first_def = ctx
            .define(name, SymbolKind::Local, Visibility::Private, 0..1, false)
            .unwrap();

        // Try to define again - should fail
        let result = ctx.define(
            name,
            SymbolKind::Function,
            Visibility::Public,
            10..11,
            false,
        );
        assert!(result.is_err());

        // The symbol should still have the original properties
        let symbol = ctx.get_symbol(first_def);
        assert_eq!(symbol.kind, SymbolKind::Local);
        assert_eq!(symbol.visibility, Visibility::Private);
        assert_eq!(symbol.span, 0..1);
    }

    // ===== Visibility Helper Tests =====

    #[test]
    fn test_is_visible_public_always_visible() {
        let tree = ModuleTree::new();
        let root = tree.root_id();

        // Public items visible from same module
        assert!(is_visible(Visibility::Public, root, root, &tree));
    }

    #[test]
    fn test_is_visible_private_same_module() {
        let tree = ModuleTree::new();
        let root = tree.root_id();

        // Private items visible from same module
        assert!(is_visible(Visibility::Private, root, root, &tree));
    }

    #[test]
    fn test_is_visible_private_child_can_access_parent() {
        let mut tree = ModuleTree::new();
        let parent = tree.root_id();
        let child = tree.add_child(parent, "child");

        // Child can see parent's private items
        assert!(is_visible(Visibility::Private, parent, child, &tree));
    }

    #[test]
    fn test_is_visible_private_grandchild_can_access_grandparent() {
        let mut tree = ModuleTree::new();
        let grandparent = tree.root_id();
        let parent = tree.add_child(grandparent, "parent");
        let grandchild = tree.add_child(parent, "grandchild");

        // Grandchild can see grandparent's private items
        assert!(is_visible(
            Visibility::Private,
            grandparent,
            grandchild,
            &tree
        ));
    }

    #[test]
    fn test_is_visible_private_parent_cannot_access_child() {
        let mut tree = ModuleTree::new();
        let parent = tree.root_id();
        let child = tree.add_child(parent, "child");

        // Parent CANNOT see child's private items
        assert!(!is_visible(Visibility::Private, child, parent, &tree));
    }

    #[test]
    fn test_is_visible_private_sibling_cannot_access() {
        let mut tree = ModuleTree::new();
        let parent = tree.root_id();
        let sibling_a = tree.add_child(parent, "a");
        let sibling_b = tree.add_child(parent, "b");

        // Siblings CANNOT see each other's private items
        assert!(!is_visible(
            Visibility::Private,
            sibling_a,
            sibling_b,
            &tree
        ));
        assert!(!is_visible(
            Visibility::Private,
            sibling_b,
            sibling_a,
            &tree
        ));
    }

    #[test]
    fn test_is_visible_private_cousin_cannot_access() {
        let mut tree = ModuleTree::new();
        let root = tree.root_id();
        let uncle = tree.add_child(root, "uncle");
        let parent = tree.add_child(root, "parent");
        let cousin_a = tree.add_child(uncle, "cousin_a");
        let cousin_b = tree.add_child(parent, "cousin_b");

        // Cousins CANNOT see each other's private items
        assert!(!is_visible(Visibility::Private, cousin_a, cousin_b, &tree));
        assert!(!is_visible(Visibility::Private, cousin_b, cousin_a, &tree));
    }

    #[test]
    fn test_is_same_or_descendant_same_module() {
        let tree = ModuleTree::new();
        let root = tree.root_id();

        assert!(is_same_or_descendant(root, root, &tree));
    }

    #[test]
    fn test_is_same_or_descendant_direct_child() {
        let mut tree = ModuleTree::new();
        let parent = tree.root_id();
        let child = tree.add_child(parent, "child");

        // Child is descendant of parent
        assert!(is_same_or_descendant(child, parent, &tree));
        // Parent is NOT descendant of child
        assert!(!is_same_or_descendant(parent, child, &tree));
    }

    #[test]
    fn test_is_same_or_descendant_deep_nesting() {
        let mut tree = ModuleTree::new();
        let root = tree.root_id();
        let a = tree.add_child(root, "a");
        let b = tree.add_child(a, "b");
        let c = tree.add_child(b, "c");

        // c is descendant of all ancestors
        assert!(is_same_or_descendant(c, root, &tree));
        assert!(is_same_or_descendant(c, a, &tree));
        assert!(is_same_or_descendant(c, b, &tree));
        assert!(is_same_or_descendant(c, c, &tree));

        // But ancestors are not descendants of c
        assert!(!is_same_or_descendant(root, c, &tree));
        assert!(!is_same_or_descendant(a, c, &tree));
        assert!(!is_same_or_descendant(b, c, &tree));
    }

    #[test]
    fn test_is_same_or_descendant_siblings_not_descendants() {
        let mut tree = ModuleTree::new();
        let parent = tree.root_id();
        let a = tree.add_child(parent, "a");
        let b = tree.add_child(parent, "b");

        // Siblings are not descendants of each other
        assert!(!is_same_or_descendant(a, b, &tree));
        assert!(!is_same_or_descendant(b, a, &tree));
    }
}
