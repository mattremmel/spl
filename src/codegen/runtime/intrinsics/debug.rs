//! Debug intrinsic functions.
//!
//! # Genuine Intrinsics
//!
//! Only operations requiring special instructions or output:
//!
//! - `__debug_print_int`: Debug output to stderr
//! - `__debug_print_ptr`: Debug output to stderr (hex format)
//! - `__breakpoint`: Debugger interrupt instruction (int3/brk)
//!
//! # Stdlib Candidates
//!
//! Assertions are just conditionals + abort, easily written in SPL:
//!
//! ```text
//! fn assert(condition: Bool, msg: String) {
//!     if !condition {
//!         __eprint_str("Assertion failed: ")
//!         __eprint_str(msg)
//!         __eprint_newline()
//!         __abort()
//!     }
//! }
//!
//! fn assert_eq(a: Int, b: Int) {
//!     if a != b {
//!         __eprint_str("Assertion failed: ")
//!         __eprint_int(a)
//!         __eprint_str(" != ")
//!         __eprint_int(b)
//!         __eprint_newline()
//!         __abort()
//!     }
//! }
//!
//! fn unreachable() -> ! {
//!     __eprint_str("unreachable code reached\n")
//!     __abort()
//! }
//! ```

use cranelift_codegen::ir::types;

use super::{Runtime, default_call_conv, make_signature};

/// Register debug intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

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

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
    {
        // No-op on other architectures
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

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

    #[test]
    fn register_adds_debug_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__debug_print_int"));
        assert!(runtime.contains("__debug_print_ptr"));
        assert!(runtime.contains("__breakpoint"));
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
