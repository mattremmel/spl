//! Runtime support for JIT-compiled code.
//!
//! This module provides the `Runtime` struct for registering external functions
//! that can be called from JIT-compiled code.

pub mod intrinsics;

use cranelift_codegen::ir::Signature;
use rustc_hash::FxHashMap;

/// Information about a runtime function that can be called from JIT code.
pub struct RuntimeFunction {
    /// The name of the function (used for symbol resolution).
    pub name: &'static str,
    /// The function pointer.
    pub ptr: *const u8,
    /// The Cranelift signature.
    pub signature: Signature,
}

/// Runtime environment for JIT-compiled code.
///
/// The runtime manages external functions (written in Rust) that can be
/// called from JIT-compiled code. Functions are registered with their
/// names, pointers, and signatures.
pub struct Runtime {
    functions: FxHashMap<&'static str, RuntimeFunction>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    /// Create a new empty runtime.
    pub fn new() -> Self {
        Runtime {
            functions: FxHashMap::default(),
        }
    }

    /// Register a runtime function.
    ///
    /// # Arguments
    /// * `name` - The symbol name for the function
    /// * `ptr` - Pointer to the function
    /// * `signature` - The Cranelift signature describing the function's type
    pub fn register(&mut self, name: &'static str, ptr: *const u8, signature: Signature) {
        self.functions.insert(
            name,
            RuntimeFunction {
                name,
                ptr,
                signature,
            },
        );
    }

    /// Get a runtime function by name.
    pub fn get(&self, name: &str) -> Option<&RuntimeFunction> {
        self.functions.get(name)
    }

    /// Iterate over all registered runtime functions.
    pub fn iter(&self) -> impl Iterator<Item = &RuntimeFunction> {
        self.functions.values()
    }

    /// Check if a function is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Get the number of registered functions.
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Check if the runtime has no registered functions.
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::{AbiParam, types};
    use cranelift_codegen::isa::CallConv;

    fn make_signature(call_conv: CallConv) -> Signature {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig.returns.push(AbiParam::new(types::I32));
        sig
    }

    extern "C" fn test_add(a: i32, b: i32) -> i32 {
        a + b
    }

    extern "C" fn test_mul(a: i32, b: i32) -> i32 {
        a * b
    }

    #[test]
    fn runtime_new_is_empty() {
        let runtime = Runtime::new();
        assert!(runtime.is_empty());
        assert_eq!(runtime.len(), 0);
    }

    #[test]
    fn runtime_default_is_empty() {
        let runtime = Runtime::default();
        assert!(runtime.is_empty());
    }

    #[test]
    fn runtime_register_function() {
        let mut runtime = Runtime::new();
        let sig = make_signature(CallConv::SystemV);

        runtime.register("spl_add", test_add as *const u8, sig);

        assert!(!runtime.is_empty());
        assert_eq!(runtime.len(), 1);
        assert!(runtime.contains("spl_add"));
    }

    #[test]
    fn runtime_get_registered_function() {
        let mut runtime = Runtime::new();
        let sig = make_signature(CallConv::SystemV);

        runtime.register("spl_add", test_add as *const u8, sig);

        let func = runtime.get("spl_add");
        assert!(func.is_some());
        let func = func.unwrap();
        assert_eq!(func.name, "spl_add");
        assert_eq!(func.ptr, test_add as *const u8);
    }

    #[test]
    fn runtime_get_unregistered_returns_none() {
        let runtime = Runtime::new();
        assert!(runtime.get("nonexistent").is_none());
    }

    #[test]
    fn runtime_register_multiple_functions() {
        let mut runtime = Runtime::new();
        let sig1 = make_signature(CallConv::SystemV);
        let sig2 = make_signature(CallConv::SystemV);

        runtime.register("spl_add", test_add as *const u8, sig1);
        runtime.register("spl_mul", test_mul as *const u8, sig2);

        assert_eq!(runtime.len(), 2);
        assert!(runtime.contains("spl_add"));
        assert!(runtime.contains("spl_mul"));
    }

    #[test]
    fn runtime_iter_functions() {
        let mut runtime = Runtime::new();
        let sig1 = make_signature(CallConv::SystemV);
        let sig2 = make_signature(CallConv::SystemV);

        runtime.register("spl_add", test_add as *const u8, sig1);
        runtime.register("spl_mul", test_mul as *const u8, sig2);

        let names: Vec<_> = runtime.iter().map(|f| f.name).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"spl_add"));
        assert!(names.contains(&"spl_mul"));
    }

    #[test]
    fn runtime_overwrite_function() {
        let mut runtime = Runtime::new();
        let sig1 = make_signature(CallConv::SystemV);
        let sig2 = make_signature(CallConv::SystemV);

        runtime.register("spl_op", test_add as *const u8, sig1);
        runtime.register("spl_op", test_mul as *const u8, sig2);

        assert_eq!(runtime.len(), 1);
        let func = runtime.get("spl_op").unwrap();
        assert_eq!(func.ptr, test_mul as *const u8);
    }

    #[test]
    fn runtime_function_signature_is_stored() {
        let mut runtime = Runtime::new();
        let mut sig = Signature::new(CallConv::SystemV);
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));

        runtime.register("test_fn", test_add as *const u8, sig.clone());

        let func = runtime.get("test_fn").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn runtime_function_fields_accessible() {
        let mut runtime = Runtime::new();
        let sig = make_signature(CallConv::SystemV);
        let ptr = test_add as *const u8;

        runtime.register("my_func", ptr, sig.clone());

        let func = runtime.get("my_func").unwrap();
        // All fields are public and accessible
        assert_eq!(func.name, "my_func");
        assert_eq!(func.ptr, ptr);
        assert_eq!(func.signature.params.len(), sig.params.len());
    }

    #[test]
    fn runtime_contains_returns_false_for_nonexistent() {
        let runtime = Runtime::new();
        assert!(!runtime.contains("does_not_exist"));
        assert!(!runtime.contains(""));
    }

    #[test]
    fn runtime_get_with_empty_name() {
        let runtime = Runtime::new();
        assert!(runtime.get("").is_none());
    }

    #[test]
    fn runtime_iter_empty() {
        let runtime = Runtime::new();
        let count = runtime.iter().count();
        assert_eq!(count, 0);
    }

    #[test]
    fn runtime_signature_with_no_params_no_returns() {
        let mut runtime = Runtime::new();
        let sig = Signature::new(CallConv::SystemV);

        runtime.register("void_fn", test_add as *const u8, sig);

        let func = runtime.get("void_fn").unwrap();
        assert!(func.signature.params.is_empty());
        assert!(func.signature.returns.is_empty());
    }
}
