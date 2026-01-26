//! I/O intrinsic functions.
//!
//! Minimal I/O primitives: raw byte output to stdout/stderr.
//!
//! # Genuine Intrinsics
//!
//! - `__print_str`: Write bytes to stdout (syscall wrapper)
//! - `__eprint_str`: Write bytes to stderr (syscall wrapper)
//!
//! # Stdlib Candidates
//!
//! All formatting functions can be implemented in SPL:
//!
//! ```text
//! fn print_int(x: Int) {
//!     let s = int_to_string(x);  // stdlib function
//!     __print_str(s.ptr, s.len);
//!     __free(s.ptr);
//! }
//!
//! fn print_float(x: Float) {
//!     let s = __float_to_string(x);  // intrinsic (complex algorithm)
//!     __print_str(s.ptr, s.len);
//!     __free(s.ptr);
//! }
//!
//! fn print_bool(b: Bool) {
//!     if b { __print_str("true", 4) } else { __print_str("false", 5) }
//! }
//!
//! fn print_char(c: Char) {
//!     let buf: [u8; 4];  // UTF-8 encode on stack
//!     let len = utf8_encode(c, &buf);
//!     __print_str(&buf, len);
//! }
//!
//! fn print_newline() {
//!     __print_str("\n", 1);
//! }
//!
//! fn read_line() -> String {
//!     // Use raw read syscall, accumulate until newline
//! }
//!
//! fn flush() {
//!     // Usually unnecessary with unbuffered syscalls
//! }
//! ```
//!
//! # Self-Hosting
//!
//! ## Raw syscalls (Linux `x86_64`)
//! ```text
//! // SYS_write = 1
//! mov rax, 1      // syscall number
//! mov rdi, 1      // fd = stdout (2 for stderr)
//! mov rsi, buf    // buffer pointer
//! mov rdx, len    // buffer length
//! syscall
//! ```

use std::io::Write;

use cranelift_codegen::ir::{Type, types};

use super::{Runtime, default_call_conv, make_signature};

/// Register I/O intrinsics.
///
/// # Parameters
/// - `ptr_ty`: The pointer type for this target (I32 for 32-bit, I64 for 64-bit)
pub fn register(runtime: &mut Runtime, ptr_ty: Type) {
    let call_conv = default_call_conv();

    // __print_str: (ptr, len: I64) -> ()
    // Raw byte output to stdout (syscall wrapper)
    runtime.register(
        "__print_str",
        __print_str as *const u8,
        make_signature(call_conv, &[ptr_ty, types::I64], &[]),
    );

    // __eprint_str: (ptr, len: I64) -> ()
    // Raw byte output to stderr (syscall wrapper)
    runtime.register(
        "__eprint_str",
        __eprint_str as *const u8,
        make_signature(call_conv, &[ptr_ty, types::I64], &[]),
    );

    // __print_newline: () -> ()
    // Print a newline to stdout
    runtime.register(
        "__print_newline",
        __print_newline as *const u8,
        make_signature(call_conv, &[], &[]),
    );
}

/// Write bytes to stdout.
///
/// # Safety
///
/// The pointer must be valid for `len` bytes if non-null.
pub extern "C" fn __print_str(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        return;
    }
    // SAFETY: Caller guarantees ptr is valid for len bytes
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let _ = std::io::stdout().write_all(slice);
    let _ = std::io::stdout().flush();
}

/// Write bytes to stderr.
///
/// # Safety
///
/// The pointer must be valid for `len` bytes if non-null.
pub extern "C" fn __eprint_str(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        return;
    }
    // SAFETY: Caller guarantees ptr is valid for len bytes
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let _ = std::io::stderr().write_all(slice);
    let _ = std::io::stderr().flush();
}

/// Print a newline to stdout.
pub extern "C" fn __print_newline() {
    let _ = std::io::stdout().write_all(b"\n");
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    #[test]
    fn print_str_does_not_panic() {
        let s = "Hello, World!";
        __print_str(s.as_ptr(), s.len() as i64);
    }

    #[test]
    fn print_str_null_ptr_does_not_panic() {
        __print_str(std::ptr::null(), 10);
    }

    #[test]
    fn print_str_zero_len_does_not_panic() {
        let s = "Hello";
        __print_str(s.as_ptr(), 0);
    }

    #[test]
    fn print_str_negative_len_does_not_panic() {
        let s = "Hello";
        __print_str(s.as_ptr(), -5);
    }

    #[test]
    fn eprint_str_does_not_panic() {
        let s = "error message";
        __eprint_str(s.as_ptr(), s.len() as i64);
    }

    #[test]
    fn eprint_str_null_does_not_panic() {
        __eprint_str(std::ptr::null(), 10);
    }

    #[test]
    fn register_adds_io_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        assert!(runtime.contains("__print_str"));
        assert!(runtime.contains("__eprint_str"));
    }

    #[test]
    fn print_str_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__print_str").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn eprint_str_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__eprint_str").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert!(func.signature.returns.is_empty());
    }

    // ==================== Pointer type tests (32-bit simulation) ====================

    #[test]
    fn print_str_signature_uses_pointer_type() {
        let mut runtime = Runtime::new();
        let ptr_ty = types::I32; // Simulate 32-bit platform
        register(&mut runtime, ptr_ty);

        let func = runtime.get("__print_str").unwrap();
        assert_eq!(func.signature.params[0].value_type, ptr_ty); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
    }

    #[test]
    fn eprint_str_signature_uses_pointer_type() {
        let mut runtime = Runtime::new();
        let ptr_ty = types::I32;
        register(&mut runtime, ptr_ty);

        let func = runtime.get("__eprint_str").unwrap();
        assert_eq!(func.signature.params[0].value_type, ptr_ty); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
    }
}
