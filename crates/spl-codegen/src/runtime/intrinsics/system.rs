//! System intrinsic functions.
//!
//! Functions for interacting with the operating system.
//!
//! # Genuine Intrinsics
//!
//! - `__exit`: Process termination (syscall)
//! - `__getenv`: Environment variable access (syscall)
//! - `__argc`: Command-line argument count (process state)
//! - `__argv`: Command-line argument access (process state)
//! - `__clock_ns`: High-resolution timer (syscall)
//!
//! # Stdlib Candidates
//!
//! ```text
//! fn clock_ms() -> Int {
//!     __clock_ns() / 1_000_000
//! }
//!
//! fn clock_us() -> Int {
//!     __clock_ns() / 1_000
//! }
//!
//! fn clock_s() -> Int {
//!     __clock_ns() / 1_000_000_000
//! }
//! ```

use std::sync::OnceLock;
use std::time::Instant;

use cranelift_codegen::ir::{Type, types};

use super::convert::StringResult;
use super::{Runtime, default_call_conv, make_signature};

// Global start time for clock measurements
static START_TIME: OnceLock<Instant> = OnceLock::new();

fn get_start_time() -> &'static Instant {
    START_TIME.get_or_init(Instant::now)
}

/// Register system intrinsics.
///
/// # Parameters
/// - `ptr_ty`: The pointer type for this target (I32 for 32-bit, I64 for 64-bit)
pub fn register(runtime: &mut Runtime, ptr_ty: Type) {
    let call_conv = default_call_conv();

    // __exit: (code: I64) -> ! (code is NOT a pointer)
    runtime.register(
        "__exit",
        __exit as *const u8,
        make_signature(call_conv, &[types::I64], &[]),
    );

    // __getenv: (name_ptr: ptr, name_len: I64) -> (ptr, I64)
    runtime.register(
        "__getenv",
        __getenv as *const u8,
        make_signature(call_conv, &[ptr_ty, types::I64], &[ptr_ty, types::I64]),
    );

    // __argc: () -> I64
    runtime.register(
        "__argc",
        __argc as *const u8,
        make_signature(call_conv, &[], &[types::I64]),
    );

    // __argv: (index: I64) -> (ptr, I64)
    runtime.register(
        "__argv",
        __argv as *const u8,
        make_signature(call_conv, &[types::I64], &[ptr_ty, types::I64]),
    );

    // __clock_ns: () -> I64
    runtime.register(
        "__clock_ns",
        __clock_ns as *const u8,
        make_signature(call_conv, &[], &[types::I64]),
    );
}

/// Exit the program with a status code.
pub extern "C" fn __exit(code: i64) -> ! {
    std::process::exit(code as i32);
}

/// Get an environment variable by name.
///
/// Returns (null, 0) if the variable is not set.
pub extern "C" fn __getenv(name_ptr: *const u8, name_len: i64) -> StringResult {
    if name_ptr.is_null() || name_len <= 0 {
        return StringResult {
            ptr: std::ptr::null(),
            len: 0,
        };
    }

    // SAFETY: Caller guarantees valid pointer
    let name_slice = unsafe { std::slice::from_raw_parts(name_ptr, name_len as usize) };
    let Ok(name) = std::str::from_utf8(name_slice) else {
        return StringResult {
            ptr: std::ptr::null(),
            len: 0,
        };
    };

    match std::env::var(name) {
        Ok(value) => {
            // Leak the string to get a stable pointer
            let leaked = Box::leak(value.into_boxed_str());
            StringResult {
                ptr: leaked.as_ptr(),
                len: leaked.len() as i64,
            }
        }
        Err(_) => StringResult {
            ptr: std::ptr::null(),
            len: 0,
        },
    }
}

/// Get the number of command-line arguments.
pub extern "C" fn __argc() -> i64 {
    std::env::args().count() as i64
}

/// Get a command-line argument by index.
///
/// Returns (null, 0) if the index is out of bounds.
pub extern "C" fn __argv(index: i64) -> StringResult {
    if index < 0 {
        return StringResult {
            ptr: std::ptr::null(),
            len: 0,
        };
    }

    match std::env::args().nth(index as usize) {
        Some(arg) => {
            let leaked = Box::leak(arg.into_boxed_str());
            StringResult {
                ptr: leaked.as_ptr(),
                len: leaked.len() as i64,
            }
        }
        None => StringResult {
            ptr: std::ptr::null(),
            len: 0,
        },
    }
}

/// Get nanoseconds elapsed since program start.
pub extern "C" fn __clock_ns() -> i64 {
    let start = get_start_time();
    start.elapsed().as_nanos() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    #[test]
    fn argc_returns_positive() {
        assert!(__argc() >= 1);
    }

    #[test]
    fn argv_index_zero_returns_program_name() {
        let result = __argv(0);
        assert!(!result.ptr.is_null());
        assert!(result.len > 0);
    }

    #[test]
    fn argv_negative_index_returns_null() {
        let result = __argv(-1);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    #[test]
    fn argv_out_of_bounds_returns_null() {
        let result = __argv(10000);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    #[test]
    fn getenv_existing_var() {
        let name = "PATH";
        let result = __getenv(name.as_ptr(), name.len() as i64);
        if !result.ptr.is_null() {
            assert!(result.len > 0);
        }
    }

    #[test]
    fn getenv_nonexistent_var() {
        let name = "__SPL_NONEXISTENT_VAR_12345__";
        let result = __getenv(name.as_ptr(), name.len() as i64);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    #[test]
    fn getenv_null_ptr_returns_null() {
        let result = __getenv(std::ptr::null(), 10);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    #[test]
    fn clock_ns_increases() {
        let t1 = __clock_ns();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let t2 = __clock_ns();
        assert!(t2 > t1);
    }

    #[test]
    fn register_adds_system_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        assert!(runtime.contains("__exit"));
        assert!(runtime.contains("__getenv"));
        assert!(runtime.contains("__argc"));
        assert!(runtime.contains("__argv"));
        assert!(runtime.contains("__clock_ns"));
    }

    #[test]
    fn exit_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__exit").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn getenv_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__getenv").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.returns.len(), 2);
    }

    #[test]
    fn argc_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__argc").unwrap();
        assert!(func.signature.params.is_empty());
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn argv_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__argv").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.returns.len(), 2);
    }

    #[test]
    fn clock_ns_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__clock_ns").unwrap();
        assert!(func.signature.params.is_empty());
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    // ==================== Pointer type tests (32-bit simulation) ====================

    #[test]
    fn getenv_signature_uses_pointer_type() {
        let mut runtime = Runtime::new();
        let ptr_ty = types::I32; // Simulate 32-bit platform
        register(&mut runtime, ptr_ty);

        let func = runtime.get("__getenv").unwrap();
        assert_eq!(func.signature.params[0].value_type, ptr_ty); // name_ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // name_len
        assert_eq!(func.signature.returns[0].value_type, ptr_ty); // result ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // result len
    }

    #[test]
    fn argv_signature_uses_pointer_type() {
        let mut runtime = Runtime::new();
        let ptr_ty = types::I32;
        register(&mut runtime, ptr_ty);

        let func = runtime.get("__argv").unwrap();
        assert_eq!(func.signature.params[0].value_type, types::I64); // index
        assert_eq!(func.signature.returns[0].value_type, ptr_ty); // result ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // result len
    }
}
