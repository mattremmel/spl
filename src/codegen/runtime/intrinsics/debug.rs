//! Debug intrinsic functions.
//!
//! Functions for debugging and assertions. These help catch bugs during
//! development and can be optimized out in release builds.
//!
//! # Current Implementation
//!
//! Uses Rust's panic infrastructure for assertion failures.
//!
//! # Self-Hosting Alternatives
//!
//! ## libc
//! ```c
//! void __assert(int8_t condition, const char* msg, int64_t msg_len) {
//!     if (!condition) {
//!         write(STDERR_FILENO, "Assertion failed: ", 18);
//!         write(STDERR_FILENO, msg, msg_len);
//!         write(STDERR_FILENO, "\n", 1);
//!         abort();
//!     }
//! }
//!
//! void __unreachable() {
//!     write(STDERR_FILENO, "unreachable code reached\n", 25);
//!     abort();
//! }
//! ```
//!
//! ## Compile-time optimization
//! In release builds, `__assert` can be a no-op and `__unreachable` can
//! emit an undefined instruction (ud2 on x86) or just be omitted entirely,
//! allowing the optimizer to assume the code path is never taken.

use cranelift_codegen::ir::types;

use super::{Runtime, default_call_conv, make_signature};

/// Register all debug intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __assert: (I8, I64, I64) -> () - condition, msg_ptr, msg_len
    runtime.register(
        "__assert",
        __assert as *const u8,
        make_signature(call_conv, &[types::I8, types::I64, types::I64], &[]),
    );

    // __assert_eq_int: (I64, I64) -> ()
    runtime.register(
        "__assert_eq_int",
        __assert_eq_int as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[]),
    );

    // __assert_eq_float: (F64, F64) -> ()
    runtime.register(
        "__assert_eq_float",
        __assert_eq_float as *const u8,
        make_signature(call_conv, &[types::F64, types::F64], &[]),
    );

    // __unreachable: () -> !
    runtime.register(
        "__unreachable",
        __unreachable as *const u8,
        make_signature(call_conv, &[], &[]),
    );

    // __debug_print_int: (I64) -> () - prints to stderr with label
    runtime.register(
        "__debug_print_int",
        __debug_print_int as *const u8,
        make_signature(call_conv, &[types::I64], &[]),
    );

    // __debug_print_ptr: (I64) -> () - prints pointer in hex to stderr
    runtime.register(
        "__debug_print_ptr",
        __debug_print_ptr as *const u8,
        make_signature(call_conv, &[types::I64], &[]),
    );

    // __breakpoint: () -> () - debugger breakpoint (if debugger attached)
    runtime.register(
        "__breakpoint",
        __breakpoint as *const u8,
        make_signature(call_conv, &[], &[]),
    );
}

/// Assert that a condition is true.
///
/// If the condition is false (0), prints the message and aborts.
///
/// # Safety
///
/// `msg_ptr` must be valid for `msg_len` bytes if the assertion fails.
pub extern "C" fn __assert(condition: i8, msg_ptr: *const u8, msg_len: i64) {
    if condition != 0 {
        return;
    }

    let msg = if msg_ptr.is_null() || msg_len <= 0 {
        "assertion failed"
    } else {
        // SAFETY: Caller guarantees valid pointer on failure path
        unsafe {
            let slice = std::slice::from_raw_parts(msg_ptr, msg_len as usize);
            std::str::from_utf8(slice).unwrap_or("assertion failed (invalid utf8)")
        }
    };

    eprintln!("Assertion failed: {msg}");
    std::process::abort();
}

/// Assert that two integers are equal.
///
/// If not equal, prints both values and aborts.
pub extern "C" fn __assert_eq_int(a: i64, b: i64) {
    if a == b {
        return;
    }

    eprintln!("Assertion failed: {a} != {b}");
    std::process::abort();
}

/// Assert that two floats are equal (exact comparison).
///
/// If not equal, prints both values and aborts.
/// Note: Use approximate comparison for floating-point calculations.
pub extern "C" fn __assert_eq_float(a: f64, b: f64) {
    if a == b {
        return;
    }

    eprintln!("Assertion failed: {a} != {b}");
    std::process::abort();
}

/// Indicate unreachable code.
///
/// This function should never be called. If reached, it aborts with an error.
/// Useful for marking code paths that should be impossible.
pub extern "C" fn __unreachable() -> ! {
    eprintln!("unreachable code reached");
    std::process::abort();
}

/// Print an integer to stderr for debugging.
///
/// Useful for quick debug output that doesn't interfere with stdout.
pub extern "C" fn __debug_print_int(value: i64) {
    eprintln!("[DEBUG] int: {value}");
}

/// Print a pointer value in hexadecimal to stderr.
///
/// Useful for debugging memory issues.
pub extern "C" fn __debug_print_ptr(ptr: i64) {
    eprintln!("[DEBUG] ptr: {ptr:#018x}");
}

/// Trigger a debugger breakpoint.
///
/// If a debugger is attached, this will pause execution.
/// If no debugger is attached, this is a no-op on most platforms.
pub extern "C" fn __breakpoint() {
    // On x86/x86_64, this would ideally emit int3, but that requires inline assembly
    // For now, we just use a compiler intrinsic if available
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // SAFETY: int3 is a debug interrupt, safe to execute
        unsafe {
            std::arch::asm!("int3", options(nomem, nostack));
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: brk is the ARM equivalent of int3
        unsafe {
            std::arch::asm!("brk #0", options(nomem, nostack));
        }
    }

    // On other architectures, this is a no-op
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
    {
        // No-op
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ==================== Direct call tests ====================

    // Note: Functions that abort cannot be tested directly.
    // We test the non-aborting paths and registration.

    #[test]
    fn assert_true_does_not_abort() {
        let msg = "should not see this";
        __assert(1, msg.as_ptr(), msg.len() as i64);
        // If we get here, the test passes
    }

    #[test]
    fn assert_eq_int_equal_does_not_abort() {
        __assert_eq_int(42, 42);
        __assert_eq_int(-1, -1);
        __assert_eq_int(0, 0);
    }

    #[test]
    fn assert_eq_float_equal_does_not_abort() {
        __assert_eq_float(2.5, 2.5);
        __assert_eq_float(-1.0, -1.0);
        __assert_eq_float(0.0, 0.0);
    }

    #[test]
    fn debug_print_int_does_not_panic() {
        __debug_print_int(42);
        __debug_print_int(-1);
        __debug_print_int(i64::MAX);
        __debug_print_int(i64::MIN);
    }

    #[test]
    fn debug_print_ptr_does_not_panic() {
        __debug_print_ptr(0);
        __debug_print_ptr(0x1234_5678_9ABC_DEF0);
        __debug_print_ptr(-1);
    }

    // Note: breakpoint test would interfere with test runner if debugger attached
    // We skip testing it directly

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_debug_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__assert"));
        assert!(runtime.contains("__assert_eq_int"));
        assert!(runtime.contains("__assert_eq_float"));
        assert!(runtime.contains("__unreachable"));
        assert!(runtime.contains("__debug_print_int"));
        assert!(runtime.contains("__debug_print_ptr"));
        assert!(runtime.contains("__breakpoint"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn assert_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__assert").unwrap();
        assert_eq!(func.signature.params.len(), 3);
        assert_eq!(func.signature.params[0].value_type, types::I8); // condition
        assert_eq!(func.signature.params[1].value_type, types::I64); // msg_ptr
        assert_eq!(func.signature.params[2].value_type, types::I64); // msg_len
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn assert_eq_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__assert_eq_int").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert_eq!(func.signature.params[1].value_type, types::I64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn assert_eq_float_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__assert_eq_float").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::F64);
        assert_eq!(func.signature.params[1].value_type, types::F64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn unreachable_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__unreachable").unwrap();
        assert!(func.signature.params.is_empty());
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn debug_print_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__debug_print_int").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn debug_print_ptr_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__debug_print_ptr").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn breakpoint_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__breakpoint").unwrap();
        assert!(func.signature.params.is_empty());
        assert!(func.signature.returns.is_empty());
    }
}
