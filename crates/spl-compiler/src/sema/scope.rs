//! Scope management for the semantic analysis phase.
//!
//! Scopes form a tree structure where each scope (except the root module scope)
//! has a parent. Name lookup traverses from the current scope upward through
//! ancestors until a definition is found or the root is reached.
//!
//! # Scope Hierarchy
//!
//! ```text
//! Module (root)
//! ├── Function
//! │   ├── Block (function body)
//! │   │   └── Block (nested if/loop)
//! │   └── ForLoop (special: binds loop variable)
//! └── Impl
//!     └── Function (method)
//! ```
//!
//! # Name Shadowing
//!
//! Inner scopes can shadow names from outer scopes. When a name is defined
//! in an inner scope, lookups from that scope (or its children) will find
//! the inner definition, not the outer one.
//!
//! ```text
//! let x = 1;        // DefId::new(0) in outer scope
//! {
//!     let x = 2;    // DefId::new(1) in inner scope, shadows DefId::new(0)
//!     print(x);     // Resolves to DefId::new(1)
//! }
//! print(x);         // Resolves to DefId::new(0)
//! ```
//!
//! # Scope Kinds
//!
//! The `ScopeKind` distinguishes different scope contexts:
//! - `Module`: Top-level, contains functions/structs/type aliases
//! - `Function`: Function body, where parameters are bound
//! - `Impl`: Implementation block, where `Self` type is available
//! - `Block`: General block (if/loop body, bare block)
//! - `ForLoop`: Special block that introduces the loop variable

use lasso::Spur;
use rustc_hash::FxHashMap;

use super::symbol::DefId;

/// A unique identifier for each scope in the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopeId(u32);

impl ScopeId {
    /// Create a new `ScopeId` with the given index.
    #[inline]
    pub(crate) const fn new(index: u32) -> Self {
        ScopeId(index)
    }

    /// Get the raw index value of this `ScopeId`.
    #[inline]
    pub fn index(self) -> u32 {
        self.0
    }
}

/// The kind of scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Module,
    Function,
    Impl,
    Block,
    ForLoop,
}

/// A scope containing symbol definitions.
#[derive(Clone, Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    symbols: FxHashMap<Spur, DefId>,
}

impl Scope {
    pub fn new(id: ScopeId, kind: ScopeKind, parent: Option<ScopeId>) -> Self {
        Self {
            id,
            kind,
            parent,
            symbols: FxHashMap::default(),
        }
    }

    /// Define a symbol in this scope. Returns Err with the existing `DefId` if already defined.
    pub fn define(&mut self, name: Spur, def_id: DefId) -> Result<(), DefId> {
        if let Some(&existing) = self.symbols.get(&name) {
            Err(existing)
        } else {
            self.symbols.insert(name, def_id);
            Ok(())
        }
    }

    /// Look up a symbol in this scope only (not parent scopes).
    pub fn lookup(&self, name: Spur) -> Option<DefId> {
        self.symbols.get(&name).copied()
    }
}
