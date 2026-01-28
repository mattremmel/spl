//! Function registry for multi-function compilation.
//!
//! Maps SPL function `DefId`s to Cranelift `FuncId`s for cross-function calls.

use cranelift_codegen::ir::Signature;
use cranelift_module::FuncId;
use rustc_hash::FxHashMap;

use crate::sema::symbol::DefId;

/// Information about a compiled function.
#[derive(Clone, Debug)]
pub struct FunctionInfo {
    /// The Cranelift function ID.
    pub func_id: FuncId,
    /// The function signature.
    pub signature: Signature,
}

impl FunctionInfo {
    /// Create a new function info.
    pub fn new(func_id: FuncId, signature: Signature) -> Self {
        FunctionInfo { func_id, signature }
    }
}

/// Registry mapping `DefId`s to Cranelift function information.
///
/// Used during multi-function compilation to resolve function references
/// when lowering call instructions.
#[derive(Default)]
pub struct FunctionRegistry {
    functions: FxHashMap<DefId, FunctionInfo>,
}

impl FunctionRegistry {
    /// Create a new empty function registry.
    pub fn new() -> Self {
        FunctionRegistry {
            functions: FxHashMap::default(),
        }
    }

    /// Register a function.
    pub fn register(&mut self, def_id: DefId, info: FunctionInfo) {
        self.functions.insert(def_id, info);
    }

    /// Look up a function by its `DefId`.
    pub fn get(&self, def_id: DefId) -> Option<&FunctionInfo> {
        self.functions.get(&def_id)
    }

    /// Check if a function is registered.
    pub fn contains(&self, def_id: DefId) -> bool {
        self.functions.contains_key(&def_id)
    }

    /// Get the number of registered functions.
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Iterate over all registered functions.
    pub fn iter(&self) -> impl Iterator<Item = (&DefId, &FunctionInfo)> {
        self.functions.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::isa::CallConv;

    fn test_signature() -> Signature {
        Signature::new(CallConv::SystemV)
    }

    #[test]
    fn registry_new_is_empty() {
        let registry = FunctionRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn registry_register_and_get() {
        let mut registry = FunctionRegistry::new();
        let def_id = DefId::new(42);
        let func_id = FuncId::from_u32(1);
        let sig = test_signature();

        registry.register(def_id, FunctionInfo::new(func_id, sig.clone()));

        assert!(registry.contains(def_id));
        assert_eq!(registry.len(), 1);

        let info = registry.get(def_id).unwrap();
        assert_eq!(info.func_id, func_id);
    }

    #[test]
    fn registry_get_nonexistent() {
        let registry = FunctionRegistry::new();
        assert!(registry.get(DefId::new(999)).is_none());
        assert!(!registry.contains(DefId::new(999)));
    }

    #[test]
    fn registry_multiple_functions() {
        let mut registry = FunctionRegistry::new();

        for i in 0..5 {
            let def_id = DefId::new(i);
            let func_id = FuncId::from_u32(i);
            registry.register(def_id, FunctionInfo::new(func_id, test_signature()));
        }

        assert_eq!(registry.len(), 5);
        for i in 0..5 {
            assert!(registry.contains(DefId::new(i)));
        }
    }

    #[test]
    fn registry_iter() {
        let mut registry = FunctionRegistry::new();
        registry.register(
            DefId::new(1),
            FunctionInfo::new(FuncId::from_u32(1), test_signature()),
        );
        registry.register(
            DefId::new(2),
            FunctionInfo::new(FuncId::from_u32(2), test_signature()),
        );

        let count = registry.iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn registry_default() {
        let registry = FunctionRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn function_info_fields() {
        let func_id = FuncId::from_u32(5);
        let sig = test_signature();
        let info = FunctionInfo::new(func_id, sig.clone());

        assert_eq!(info.func_id, func_id);
        // Signature comparison requires matching call conventions
        assert_eq!(info.signature.call_conv, sig.call_conv);
    }
}
