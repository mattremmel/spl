//! Scope management for the semantic analysis phase.

use lasso::Spur;
use rustc_hash::FxHashMap;

use super::symbol::DefId;

/// A unique identifier for each scope in the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

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

    /// Define a symbol in this scope. Returns Err with the existing DefId if already defined.
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
