//! Debug intrinsic functions.
//!
//! # Genuine Intrinsics
//!
//! - `__breakpoint`: Debugger interrupt instruction (int3/brk)
//!
//! # Stdlib Candidates
//!
//! Debug printing and assertions can be implemented in SPL:
//!
//! ```text
//! fn debug_print_int(value: Int) {
//!     __eprint_str("[DEBUG] int: ");
//!     let s = int_to_string(value);
//!     __eprint_str(s.ptr, s.len);
//!     __eprint_str("\n", 1);
//!     __free(s.ptr);
//! }
//!
//! fn debug_print_ptr(ptr: Int) {
//!     __eprint_str("[DEBUG] ptr: 0x");
//!     let s = int_to_hex_string(ptr);
//!     __eprint_str(s.ptr, s.len);
//!     __eprint_str("\n", 1);
//!     __free(s.ptr);
//! }
//!
//! fn assert(condition: Bool, msg: String) {
//!     if !condition {
//!         __eprint_str("Assertion failed: ", 18);
//!         __eprint_str(msg.ptr, msg.len);
//!         __eprint_str("\n", 1);
//!         __abort();
//!     }
//! }
//!
//! fn unreachable() -> ! {
//!     __eprint_str("unreachable code reached\n", 25);
//!     __abort();
//! }
//! ```

use super::{Runtime, default_call_conv, make_signature};

/// Register debug intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __breakpoint: () -> () - debugger breakpoint (if debugger attached)
    runtime.register(
        "__breakpoint",
        __breakpoint as *const u8,
        make_signature(call_conv, &[], &[]),
    );
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

    #[test]
    fn register_adds_breakpoint() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__breakpoint"));
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
