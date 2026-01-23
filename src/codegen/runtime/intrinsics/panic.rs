//! Panic intrinsic functions.
//!
//! Functions for aborting program execution.

use super::{Runtime, default_call_conv, make_signature};

/// Register all panic intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __abort: () -> !
    // Note: We use an empty return list since Cranelift doesn't have a "never" type
    runtime.register(
        "__abort",
        __abort as *const u8,
        make_signature(call_conv, &[], &[]),
    );
}

/// Abort program execution.
///
/// This function will panic and never return.
pub extern "C" fn __abort() -> ! {
    panic!("__abort called: program aborted");
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: We cannot test __abort with #[should_panic] because extern "C" functions
    // cannot unwind. The panic will cause a process abort instead.

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_abort() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__abort"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn abort_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__abort").unwrap();
        assert!(func.signature.params.is_empty());
        // Cranelift doesn't have a "never" type, so returns is empty
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn abort_has_valid_pointer() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__abort").unwrap();
        assert!(!func.ptr.is_null());
    }
}
