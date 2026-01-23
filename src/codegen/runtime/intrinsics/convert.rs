//! Conversion intrinsic functions (stubs).
//!
//! Functions for converting between types. Currently implemented as stubs
//! that return null/zero values.
//!
//! # Why Stubs?
//!
//! These functions need to allocate memory for their string results, which
//! requires deciding on a memory management strategy:
//!
//! - **Arena allocator**: Fast, but requires manual lifetime management
//! - **Reference counting**: Automatic, but has overhead
//! - **Garbage collection**: Automatic, but complex to implement
//! - **Caller-provided buffer**: No allocation, but less convenient
//!
//! # Implementation Options
//!
//! ## With heap allocation (malloc)
//! ```c
//! StringResult __int_to_string(int64_t x) {
//!     char* buf = malloc(21);  // max i64 digits + sign + null
//!     int len = snprintf(buf, 21, "%lld", x);
//!     return (StringResult){buf, len};
//! }
//! // Caller must free the returned pointer
//! ```
//!
//! ## With arena allocator
//! ```text
//! fn __int_to_string(x: Int) -> String {
//!     let buf = arena_alloc(21);
//!     let len = format_int(x, buf);
//!     String { ptr: buf, len }
//! }
//! // Arena is reset at end of expression/statement
//! ```
//!
//! ## With caller-provided buffer
//! ```text
//! fn __int_to_string(x: Int, buf: *mut u8, capacity: Int) -> Int {
//!     // Returns length written, or -1 if buffer too small
//! }
//! ```
//!
//! # Self-Hosting
//!
//! The implementation chosen here will affect the entire language's string
//! handling. For self-hosting, a simple arena or malloc-based approach is
//! recommended initially, with potential optimization later.

use cranelift_codegen::ir::types;

use super::{Runtime, default_call_conv, make_signature};

/// FFI-safe string result (ptr, len).
#[repr(C)]
pub struct StringResult {
    pub ptr: *const u8,
    pub len: i64,
}

/// Register all conversion intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __int_to_string: (I64) -> (*const u8, I64)
    // Returns (ptr, len) as two I64 values
    runtime.register(
        "__int_to_string",
        __int_to_string as *const u8,
        make_signature(call_conv, &[types::I64], &[types::I64, types::I64]),
    );

    // __float_to_string: (F64) -> (*const u8, I64)
    runtime.register(
        "__float_to_string",
        __float_to_string as *const u8,
        make_signature(call_conv, &[types::F64], &[types::I64, types::I64]),
    );
}

/// Convert an integer to a string (stub).
///
/// Returns (null, 0) as this is not yet implemented.
pub extern "C" fn __int_to_string(_value: i64) -> StringResult {
    // Stub: return null pointer and zero length
    StringResult {
        ptr: std::ptr::null(),
        len: 0,
    }
}

/// Convert a float to a string (stub).
///
/// Returns (null, 0) as this is not yet implemented.
pub extern "C" fn __float_to_string(_value: f64) -> StringResult {
    // Stub: return null pointer and zero length
    StringResult {
        ptr: std::ptr::null(),
        len: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ==================== Direct call tests ====================

    #[test]
    fn int_to_string_returns_null() {
        let result = __int_to_string(42);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    #[test]
    fn float_to_string_returns_null() {
        let result = __float_to_string(2.5);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_convert_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__int_to_string"));
        assert!(runtime.contains("__float_to_string"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn int_to_string_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__int_to_string").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        // Returns (ptr, len) as two I64 values
        assert_eq!(func.signature.returns.len(), 2);
        assert_eq!(func.signature.returns[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // len
    }

    #[test]
    fn float_to_string_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__float_to_string").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::F64);
        assert_eq!(func.signature.returns.len(), 2);
        assert_eq!(func.signature.returns[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // len
    }

    // ==================== Edge case tests ====================

    #[test]
    fn int_to_string_with_zero() {
        let result = __int_to_string(0);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    #[test]
    fn int_to_string_with_negative() {
        let result = __int_to_string(-42);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    #[test]
    fn float_to_string_with_zero() {
        let result = __float_to_string(0.0);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    #[test]
    fn float_to_string_with_negative() {
        let result = __float_to_string(-2.5);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }
}
