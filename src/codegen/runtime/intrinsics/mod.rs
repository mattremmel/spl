//! Intrinsic functions for SPL's standard library.
//!
//! This module provides built-in functions that can be called from JIT-compiled code.
//! All intrinsic names follow the `__` (double underscore) prefix convention.

mod convert;
mod io;
mod math;
mod panic;
mod string;

use cranelift_codegen::ir::{AbiParam, Signature};
use cranelift_codegen::isa::CallConv;

use super::Runtime;

/// Register all intrinsic functions with the runtime.
pub fn register_all(runtime: &mut Runtime) {
    math::register(runtime);
    io::register(runtime);
    panic::register(runtime);
    convert::register(runtime);
    string::register(runtime);
}

/// Create a Cranelift signature with the given parameter and return types.
pub fn make_signature(
    call_conv: CallConv,
    params: &[cranelift_codegen::ir::Type],
    returns: &[cranelift_codegen::ir::Type],
) -> Signature {
    let mut sig = Signature::new(call_conv);
    for &ty in params {
        sig.params.push(AbiParam::new(ty));
    }
    for &ty in returns {
        sig.returns.push(AbiParam::new(ty));
    }
    sig
}

/// Get the default calling convention for intrinsics.
#[cfg(target_family = "unix")]
pub fn default_call_conv() -> CallConv {
    CallConv::SystemV
}

#[cfg(target_family = "windows")]
pub fn default_call_conv() -> CallConv {
    CallConv::WindowsFastcall
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    #[test]
    fn register_all_adds_intrinsics_to_runtime() {
        let mut runtime = Runtime::new();
        assert!(runtime.is_empty());

        register_all(&mut runtime);

        assert!(!runtime.is_empty());
        // Should have at least the core intrinsics
        assert!(runtime.len() >= 10);
    }

    #[test]
    fn all_intrinsics_have_valid_signatures() {
        let mut runtime = Runtime::new();
        register_all(&mut runtime);

        for func in runtime.iter() {
            // Name should not be empty
            assert!(!func.name.is_empty(), "Intrinsic has empty name");
            // Function pointer should not be null
            assert!(!func.ptr.is_null(), "Intrinsic {} has null pointer", func.name);
        }
    }

    #[test]
    fn intrinsic_names_follow_double_underscore_prefix_convention() {
        let mut runtime = Runtime::new();
        register_all(&mut runtime);

        for func in runtime.iter() {
            assert!(
                func.name.starts_with("__"),
                "Intrinsic '{}' does not follow __ prefix convention",
                func.name
            );
        }
    }

    #[test]
    fn make_signature_no_params_no_returns() {
        let sig = make_signature(CallConv::SystemV, &[], &[]);
        assert!(sig.params.is_empty());
        assert!(sig.returns.is_empty());
    }

    #[test]
    fn make_signature_with_params() {
        let sig = make_signature(CallConv::SystemV, &[types::I64, types::I64], &[types::I64]);
        assert_eq!(sig.params.len(), 2);
        assert_eq!(sig.params[0].value_type, types::I64);
        assert_eq!(sig.params[1].value_type, types::I64);
        assert_eq!(sig.returns.len(), 1);
        assert_eq!(sig.returns[0].value_type, types::I64);
    }

    #[test]
    fn make_signature_preserves_call_conv() {
        let sig = make_signature(CallConv::SystemV, &[], &[]);
        assert_eq!(sig.call_conv, CallConv::SystemV);
    }
}
