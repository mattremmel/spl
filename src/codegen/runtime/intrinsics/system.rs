//! System intrinsic functions.
//!
//! Functions for interacting with the operating system: process control,
//! environment variables, command-line arguments, and timing.
//!
//! # Current Implementation
//!
//! Uses Rust's standard library for OS interaction.
//!
//! # Self-Hosting Alternatives
//!
//! ## libc
//! ```c
//! void __exit(int64_t code) { exit(code); }
//! char* __getenv(const char* name) { return getenv(name); }
//! ```
//!
//! ## Raw syscalls (Linux x86_64)
//! ```text
//! // __exit: SYS_exit_group = 231
//! mov rax, 231
//! mov rdi, code
//! syscall
//!
//! // __clock: SYS_clock_gettime = 228
//! mov rax, 228
//! mov rdi, 0        // CLOCK_REALTIME
//! mov rsi, &timespec
//! syscall
//! ```
//!
//! ## Command-line arguments
//! Arguments are typically passed via the stack at program start (Linux):
//! ```text
//! [argc: i64]
//! [argv[0]: *const u8]
//! [argv[1]: *const u8]
//! ...
//! [null]
//! ```
//! A minimal runtime captures these at `_start` and stores them globally.

use std::sync::OnceLock;
use std::time::Instant;

use cranelift_codegen::ir::types;

use super::convert::StringResult;
use super::{Runtime, default_call_conv, make_signature};

// Global start time for clock measurements
static START_TIME: OnceLock<Instant> = OnceLock::new();

fn get_start_time() -> &'static Instant {
    START_TIME.get_or_init(Instant::now)
}

/// Register all system intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __exit: (I64) -> ! (never returns, but Cranelift doesn't have never type)
    runtime.register(
        "__exit",
        __exit as *const u8,
        make_signature(call_conv, &[types::I64], &[]),
    );

    // __getenv: (I64, I64) -> (I64, I64) - name (ptr, len) -> value (ptr, len)
    runtime.register(
        "__getenv",
        __getenv as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64],
            &[types::I64, types::I64],
        ),
    );

    // __argc: () -> I64
    runtime.register(
        "__argc",
        __argc as *const u8,
        make_signature(call_conv, &[], &[types::I64]),
    );

    // __argv: (I64) -> (I64, I64) - index -> (ptr, len)
    runtime.register(
        "__argv",
        __argv as *const u8,
        make_signature(call_conv, &[types::I64], &[types::I64, types::I64]),
    );

    // __clock_ns: () -> I64 (nanoseconds since program start)
    runtime.register(
        "__clock_ns",
        __clock_ns as *const u8,
        make_signature(call_conv, &[], &[types::I64]),
    );

    // __clock_ms: () -> I64 (milliseconds since program start)
    runtime.register(
        "__clock_ms",
        __clock_ms as *const u8,
        make_signature(call_conv, &[], &[types::I64]),
    );
}

/// Exit the program with a status code.
///
/// This function never returns.
pub extern "C" fn __exit(code: i64) -> ! {
    std::process::exit(code as i32);
}

/// Get an environment variable by name.
///
/// Returns (null, 0) if the variable is not set.
///
/// # Safety
///
/// The returned pointer is only valid until the environment is modified.
pub extern "C" fn __getenv(name_ptr: *const u8, name_len: i64) -> StringResult {
    if name_ptr.is_null() || name_len <= 0 {
        return StringResult {
            ptr: std::ptr::null(),
            len: 0,
        };
    }

    // SAFETY: Caller guarantees valid pointer
    let name_slice = unsafe { std::slice::from_raw_parts(name_ptr, name_len as usize) };
    let name = match std::str::from_utf8(name_slice) {
        Ok(s) => s,
        Err(_) => {
            return StringResult {
                ptr: std::ptr::null(),
                len: 0,
            }
        }
    };

    match std::env::var(name) {
        Ok(value) => {
            // Leak the string to get a stable pointer
            // In a real implementation, we'd use an arena or have the caller free it
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
///
/// # Safety
///
/// The returned pointer is valid for the lifetime of the program.
pub extern "C" fn __argv(index: i64) -> StringResult {
    if index < 0 {
        return StringResult {
            ptr: std::ptr::null(),
            len: 0,
        };
    }

    match std::env::args().nth(index as usize) {
        Some(arg) => {
            // Leak the string to get a stable pointer
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
///
/// Uses a monotonic clock, suitable for measuring durations.
pub extern "C" fn __clock_ns() -> i64 {
    let start = get_start_time();
    start.elapsed().as_nanos() as i64
}

/// Get milliseconds elapsed since program start.
///
/// Uses a monotonic clock, suitable for measuring durations.
pub extern "C" fn __clock_ms() -> i64 {
    let start = get_start_time();
    start.elapsed().as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ==================== Direct call tests ====================

    // Note: __exit cannot be tested directly as it terminates the process

    #[test]
    fn argc_returns_positive() {
        // When running tests, argc is at least 1 (the test binary)
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
        // PATH is almost always set
        let name = "PATH";
        let result = __getenv(name.as_ptr(), name.len() as i64);
        // PATH might not be set in some CI environments, so we just check it doesn't crash
        // If it is set, it should return a valid string
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
    fn clock_ms_returns_non_negative() {
        let t = __clock_ms();
        assert!(t >= 0);
    }

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_system_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__exit"));
        assert!(runtime.contains("__getenv"));
        assert!(runtime.contains("__argc"));
        assert!(runtime.contains("__argv"));
        assert!(runtime.contains("__clock_ns"));
        assert!(runtime.contains("__clock_ms"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn exit_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__exit").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn getenv_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__getenv").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert_eq!(func.signature.returns.len(), 2);
        assert_eq!(func.signature.returns[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // len
    }

    #[test]
    fn argc_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__argc").unwrap();
        assert!(func.signature.params.is_empty());
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn argv_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__argv").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64); // index
        assert_eq!(func.signature.returns.len(), 2);
        assert_eq!(func.signature.returns[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // len
    }

    #[test]
    fn clock_ns_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__clock_ns").unwrap();
        assert!(func.signature.params.is_empty());
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn clock_ms_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__clock_ms").unwrap();
        assert!(func.signature.params.is_empty());
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }
}
