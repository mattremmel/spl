//! Conversion intrinsic functions.
//!
//! Functions for converting between types. Numeric conversions are implemented;
//! string conversions are stubs that need memory allocation.
//!
//! # Implemented Conversions
//!
//! - `__int_to_float`: Convert integer to float
//! - `__float_to_int`: Convert float to integer (truncates toward zero)
//! - `__char_to_int`: Get Unicode code point from character
//! - `__int_to_char`: Convert code point to character
//! - `__bool_to_int`: Convert boolean to 0/1
//!
//! # Stub Conversions (need allocation)
//!
//! - `__int_to_string`: Format integer as string
//! - `__float_to_string`: Format float as string
//! - `__bool_to_string`: Format boolean as "true"/"false"
//!
//! # Why Stubs for String Conversions?
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

    // ==================== Numeric conversions ====================

    // __int_to_float: (I64) -> F64
    runtime.register(
        "__int_to_float",
        __int_to_float as *const u8,
        make_signature(call_conv, &[types::I64], &[types::F64]),
    );

    // __float_to_int: (F64) -> I64
    runtime.register(
        "__float_to_int",
        __float_to_int as *const u8,
        make_signature(call_conv, &[types::F64], &[types::I64]),
    );

    // __char_to_int: (I32) -> I64
    runtime.register(
        "__char_to_int",
        __char_to_int as *const u8,
        make_signature(call_conv, &[types::I32], &[types::I64]),
    );

    // __int_to_char: (I64) -> I32 (returns -1 for invalid code points)
    runtime.register(
        "__int_to_char",
        __int_to_char as *const u8,
        make_signature(call_conv, &[types::I64], &[types::I32]),
    );

    // __bool_to_int: (I8) -> I64
    runtime.register(
        "__bool_to_int",
        __bool_to_int as *const u8,
        make_signature(call_conv, &[types::I8], &[types::I64]),
    );

    // ==================== String parsing ====================

    // __str_to_int: (ptr, len) -> I64 (returns 0 on parse failure)
    runtime.register(
        "__str_to_int",
        __str_to_int as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[types::I64]),
    );

    // __str_to_float: (ptr, len) -> F64 (returns NaN on parse failure)
    runtime.register(
        "__str_to_float",
        __str_to_float as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[types::F64]),
    );

    // ==================== String formatting (stubs) ====================

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

    // __bool_to_string: (I8) -> (*const u8, I64)
    runtime.register(
        "__bool_to_string",
        __bool_to_string as *const u8,
        make_signature(call_conv, &[types::I8], &[types::I64, types::I64]),
    );
}

// ==================== Numeric conversions ====================

/// Convert an integer to a float.
pub extern "C" fn __int_to_float(value: i64) -> f64 {
    value as f64
}

/// Convert a float to an integer.
///
/// Truncates toward zero. Returns 0 for NaN.
/// Saturates to i64::MIN/MAX for values outside the representable range.
pub extern "C" fn __float_to_int(value: f64) -> i64 {
    if value.is_nan() {
        0
    } else if value >= i64::MAX as f64 {
        i64::MAX
    } else if value <= i64::MIN as f64 {
        i64::MIN
    } else {
        value as i64
    }
}

/// Get the Unicode code point from a character.
pub extern "C" fn __char_to_int(value: i32) -> i64 {
    value as i64
}

/// Convert a code point to a character.
///
/// Returns the character code point if valid, or -1 if invalid.
pub extern "C" fn __int_to_char(value: i64) -> i32 {
    if value < 0 || value > u32::MAX as i64 {
        return -1;
    }
    match char::from_u32(value as u32) {
        Some(c) => c as i32,
        None => -1,
    }
}

/// Convert a boolean to an integer.
///
/// Returns 1 for true (non-zero), 0 for false.
pub extern "C" fn __bool_to_int(value: i8) -> i64 {
    if value != 0 { 1 } else { 0 }
}

// ==================== String parsing ====================

/// Parse a string as an integer.
///
/// Returns the parsed value, or 0 if parsing fails.
pub extern "C" fn __str_to_int(ptr: *const u8, len: i64) -> i64 {
    if ptr.is_null() || len <= 0 {
        return 0;
    }

    // SAFETY: Caller guarantees valid pointer
    unsafe {
        let slice = std::slice::from_raw_parts(ptr, len as usize);
        match std::str::from_utf8(slice) {
            Ok(s) => s.trim().parse::<i64>().unwrap_or(0),
            Err(_) => 0,
        }
    }
}

/// Parse a string as a float.
///
/// Returns the parsed value, or NaN if parsing fails.
pub extern "C" fn __str_to_float(ptr: *const u8, len: i64) -> f64 {
    if ptr.is_null() || len <= 0 {
        return f64::NAN;
    }

    // SAFETY: Caller guarantees valid pointer
    unsafe {
        let slice = std::slice::from_raw_parts(ptr, len as usize);
        match std::str::from_utf8(slice) {
            Ok(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
            Err(_) => f64::NAN,
        }
    }
}

// ==================== String formatting (stubs) ====================

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

/// Convert a boolean to a string (stub).
///
/// Returns (null, 0) as this is not yet implemented.
/// When implemented, will return "true" or "false".
pub extern "C" fn __bool_to_string(_value: i8) -> StringResult {
    StringResult {
        ptr: std::ptr::null(),
        len: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ==================== Numeric conversion tests ====================

    #[test]
    fn int_to_float_positive() {
        assert_eq!(__int_to_float(42), 42.0);
    }

    #[test]
    fn int_to_float_negative() {
        assert_eq!(__int_to_float(-42), -42.0);
    }

    #[test]
    fn int_to_float_zero() {
        assert_eq!(__int_to_float(0), 0.0);
    }

    #[test]
    fn float_to_int_positive() {
        assert_eq!(__float_to_int(42.9), 42);
    }

    #[test]
    fn float_to_int_negative() {
        assert_eq!(__float_to_int(-42.9), -42);
    }

    #[test]
    fn float_to_int_nan() {
        assert_eq!(__float_to_int(f64::NAN), 0);
    }

    #[test]
    fn float_to_int_infinity() {
        assert_eq!(__float_to_int(f64::INFINITY), i64::MAX);
        assert_eq!(__float_to_int(f64::NEG_INFINITY), i64::MIN);
    }

    #[test]
    fn char_to_int_ascii() {
        assert_eq!(__char_to_int('A' as i32), 65);
    }

    #[test]
    fn char_to_int_unicode() {
        assert_eq!(__char_to_int('λ' as i32), 955);
    }

    #[test]
    fn int_to_char_valid() {
        assert_eq!(__int_to_char(65), 'A' as i32);
        assert_eq!(__int_to_char(955), 'λ' as i32);
    }

    #[test]
    fn int_to_char_invalid() {
        assert_eq!(__int_to_char(-1), -1);
        assert_eq!(__int_to_char(0xD800), -1); // Surrogate
        assert_eq!(__int_to_char(0x110000), -1); // Beyond max code point
    }

    #[test]
    fn bool_to_int_true() {
        assert_eq!(__bool_to_int(1), 1);
        assert_eq!(__bool_to_int(-1), 1); // Non-zero is true
        assert_eq!(__bool_to_int(42), 1);
    }

    #[test]
    fn bool_to_int_false() {
        assert_eq!(__bool_to_int(0), 0);
    }

    // ==================== String parsing tests ====================

    #[test]
    fn str_to_int_positive() {
        let s = "42";
        assert_eq!(__str_to_int(s.as_ptr(), s.len() as i64), 42);
    }

    #[test]
    fn str_to_int_negative() {
        let s = "-42";
        assert_eq!(__str_to_int(s.as_ptr(), s.len() as i64), -42);
    }

    #[test]
    fn str_to_int_with_whitespace() {
        let s = "  42  ";
        assert_eq!(__str_to_int(s.as_ptr(), s.len() as i64), 42);
    }

    #[test]
    fn str_to_int_invalid() {
        let s = "hello";
        assert_eq!(__str_to_int(s.as_ptr(), s.len() as i64), 0);
    }

    #[test]
    fn str_to_int_null() {
        assert_eq!(__str_to_int(std::ptr::null(), 10), 0);
    }

    #[test]
    fn str_to_float_positive() {
        let s = "2.5";
        let result = __str_to_float(s.as_ptr(), s.len() as i64);
        assert_eq!(result, 2.5);
    }

    #[test]
    fn str_to_float_negative() {
        let s = "-2.5";
        assert_eq!(__str_to_float(s.as_ptr(), s.len() as i64), -2.5);
    }

    #[test]
    fn str_to_float_scientific() {
        let s = "1.5e10";
        assert_eq!(__str_to_float(s.as_ptr(), s.len() as i64), 1.5e10);
    }

    #[test]
    fn str_to_float_invalid() {
        let s = "hello";
        assert!(__str_to_float(s.as_ptr(), s.len() as i64).is_nan());
    }

    #[test]
    fn str_to_float_null() {
        assert!(__str_to_float(std::ptr::null(), 10).is_nan());
    }

    // ==================== String formatting stub tests ====================

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

    #[test]
    fn bool_to_string_returns_null() {
        let result = __bool_to_string(1);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_convert_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        // Numeric conversions
        assert!(runtime.contains("__int_to_float"));
        assert!(runtime.contains("__float_to_int"));
        assert!(runtime.contains("__char_to_int"));
        assert!(runtime.contains("__int_to_char"));
        assert!(runtime.contains("__bool_to_int"));

        // String parsing
        assert!(runtime.contains("__str_to_int"));
        assert!(runtime.contains("__str_to_float"));

        // String formatting (stubs)
        assert!(runtime.contains("__int_to_string"));
        assert!(runtime.contains("__float_to_string"));
        assert!(runtime.contains("__bool_to_string"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn int_to_float_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__int_to_float").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::F64);
    }

    #[test]
    fn float_to_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__float_to_int").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::F64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn str_to_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__str_to_int").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn str_to_float_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__str_to_float").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::F64);
    }

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
}
