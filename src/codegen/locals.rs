//! Local variable storage mapping for code generation.
//!
//! This module tracks how MIR locals are represented in Cranelift:
//! - Scalar values use Cranelift Variables (SSA values)
//! - Compound types use stack slots
//! - ZSTs require no storage

use cranelift_codegen::ir::StackSlot;
use cranelift_frontend::Variable;
use rustc_hash::FxHashMap;

use crate::mir::Local;

/// Storage location for a local variable in generated code.
#[derive(Clone, Copy, Debug)]
pub enum LocalStorage {
    /// A Cranelift variable (for scalar values that fit in registers).
    Variable(Variable),

    /// A stack slot (for compound types that need memory).
    StackSlot(StackSlot),

    /// Zero-sized type (no storage needed).
    Zst,
}

/// Maps MIR locals to their Cranelift storage locations.
///
/// Each function has its own `LocalMap` that tracks how each MIR `Local`
/// is represented in the generated code.
#[derive(Debug, Default)]
pub struct LocalMap {
    /// Map from MIR Local to storage location.
    storage: FxHashMap<Local, LocalStorage>,

    /// Counter for generating unique Cranelift Variable indices.
    next_variable: u32,
}

impl LocalMap {
    /// Create a new empty local map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a new Cranelift Variable for a scalar local.
    ///
    /// Returns the Variable that was allocated and stores the mapping.
    pub fn alloc_variable(&mut self, local: Local) -> Variable {
        let var = Variable::from_u32(self.next_variable);
        self.next_variable += 1;
        self.storage.insert(local, LocalStorage::Variable(var));
        var
    }

    /// Set a local to use a stack slot.
    pub fn set_stack_slot(&mut self, local: Local, slot: StackSlot) {
        self.storage.insert(local, LocalStorage::StackSlot(slot));
    }

    /// Mark a local as a ZST (no storage needed).
    pub fn set_zst(&mut self, local: Local) {
        self.storage.insert(local, LocalStorage::Zst);
    }

    /// Get the storage location for a local.
    ///
    /// Returns `None` if the local hasn't been allocated yet.
    pub fn get(&self, local: Local) -> Option<LocalStorage> {
        self.storage.get(&local).copied()
    }

    /// Clear all mappings (for reuse between functions).
    pub fn clear(&mut self) {
        self.storage.clear();
        self.next_variable = 0;
    }

    /// Returns the number of locals currently mapped.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns true if no locals are mapped.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::entity::EntityRef;

    #[test]
    fn local_map_new_is_empty() {
        let map = LocalMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn local_map_default_is_empty() {
        let map = LocalMap::default();
        assert!(map.is_empty());
    }

    #[test]
    fn alloc_variable() {
        let mut map = LocalMap::new();
        let local = Local::new(0);

        let var = map.alloc_variable(local);

        assert_eq!(var.index(), 0);
        assert_eq!(map.len(), 1);

        match map.get(local) {
            Some(LocalStorage::Variable(v)) => assert_eq!(v.index(), 0),
            other => panic!("expected Variable, got {:?}", other),
        }
    }

    #[test]
    fn alloc_multiple_variables() {
        let mut map = LocalMap::new();

        let var0 = map.alloc_variable(Local::new(0));
        let var1 = map.alloc_variable(Local::new(1));
        let var2 = map.alloc_variable(Local::new(2));

        // Variables should have sequential indices
        assert_eq!(var0.index(), 0);
        assert_eq!(var1.index(), 1);
        assert_eq!(var2.index(), 2);

        assert_eq!(map.len(), 3);
    }

    #[test]
    fn set_stack_slot() {
        let mut map = LocalMap::new();
        let local = Local::new(5);
        let slot = StackSlot::from_u32(42);

        map.set_stack_slot(local, slot);

        match map.get(local) {
            Some(LocalStorage::StackSlot(s)) => assert_eq!(s.as_u32(), 42),
            other => panic!("expected StackSlot, got {:?}", other),
        }
    }

    #[test]
    fn set_zst() {
        let mut map = LocalMap::new();
        let local = Local::new(10);

        map.set_zst(local);

        match map.get(local) {
            Some(LocalStorage::Zst) => {}
            other => panic!("expected Zst, got {:?}", other),
        }
    }

    #[test]
    fn get_unmapped_returns_none() {
        let map = LocalMap::new();
        assert!(map.get(Local::new(0)).is_none());
    }

    #[test]
    fn clear_removes_all() {
        let mut map = LocalMap::new();

        map.alloc_variable(Local::new(0));
        map.alloc_variable(Local::new(1));
        map.set_zst(Local::new(2));

        assert_eq!(map.len(), 3);

        map.clear();

        assert!(map.is_empty());
        assert!(map.get(Local::new(0)).is_none());
        assert!(map.get(Local::new(1)).is_none());
        assert!(map.get(Local::new(2)).is_none());
    }

    #[test]
    fn clear_resets_variable_counter() {
        let mut map = LocalMap::new();

        let var0 = map.alloc_variable(Local::new(0));
        assert_eq!(var0.index(), 0);

        map.clear();

        // After clear, next variable should start from 0 again
        let var_new = map.alloc_variable(Local::new(0));
        assert_eq!(var_new.index(), 0);
    }

    #[test]
    fn overwrite_existing_local() {
        let mut map = LocalMap::new();
        let local = Local::new(0);

        // First allocate as variable
        let _var = map.alloc_variable(local);
        assert!(matches!(map.get(local), Some(LocalStorage::Variable(_))));

        // Then overwrite as ZST
        map.set_zst(local);
        assert!(matches!(map.get(local), Some(LocalStorage::Zst)));

        // Length should still be 1 (same local)
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn local_storage_is_copy() {
        let storage = LocalStorage::Variable(Variable::from_u32(0));
        let storage_copy = storage;

        // Both should be usable
        match storage {
            LocalStorage::Variable(v) => assert_eq!(v.index(), 0),
            _ => panic!("expected Variable"),
        }
        match storage_copy {
            LocalStorage::Variable(v) => assert_eq!(v.index(), 0),
            _ => panic!("expected Variable"),
        }
    }

    #[test]
    fn local_storage_debug() {
        let var = LocalStorage::Variable(Variable::from_u32(5));
        let slot = LocalStorage::StackSlot(StackSlot::from_u32(10));
        let zst = LocalStorage::Zst;

        // Just ensure debug formatting works
        let _ = format!("{:?}", var);
        let _ = format!("{:?}", slot);
        let _ = format!("{:?}", zst);
    }

    #[test]
    fn local_map_debug() {
        let mut map = LocalMap::new();
        map.alloc_variable(Local::new(0));

        let debug_str = format!("{:?}", map);
        assert!(debug_str.contains("LocalMap"));
    }

    #[test]
    fn mixed_storage_types() {
        let mut map = LocalMap::new();

        // Return place as ZST (unit)
        map.set_zst(Local::RETURN_PLACE);

        // Scalar locals as variables
        let _var1 = map.alloc_variable(Local::new(1));
        let _var2 = map.alloc_variable(Local::new(2));

        // Compound type as stack slot
        map.set_stack_slot(Local::new(3), StackSlot::from_u32(0));

        assert_eq!(map.len(), 4);

        assert!(matches!(
            map.get(Local::RETURN_PLACE),
            Some(LocalStorage::Zst)
        ));
        assert!(matches!(
            map.get(Local::new(1)),
            Some(LocalStorage::Variable(_))
        ));
        assert!(matches!(
            map.get(Local::new(2)),
            Some(LocalStorage::Variable(_))
        ));
        assert!(matches!(
            map.get(Local::new(3)),
            Some(LocalStorage::StackSlot(_))
        ));
    }
}
