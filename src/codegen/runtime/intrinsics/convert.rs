//! Type conversion intrinsics.
//!
//! # Genuine Intrinsics
//!
//! Only float parsing/formatting require intrinsics due to algorithm complexity:
//!
//! - `__str_to_float`: Float parsing (strtod-like algorithms)
//! - `__float_to_string`: Float formatting (Grisu/Ryu algorithms)
//!
//! # Compiler Codegen (not intrinsics)
//!
//! These should be emitted directly by the compiler as single instructions:
//!
//! ```text
//! int_to_float  -> cvtsi2sd (x86) / scvtf (ARM)
//! float_to_int  -> cvttsd2si (x86) / fcvtzs (ARM)
//! char_to_int   -> zero-extend or no-op (char is already an int)
//! bool_to_int   -> zero-extend or no-op
//! ```
//!
//! # Stdlib Candidates
//!
//! ```text
//! fn int_to_char(code: Int) -> Option<Char> {
//!     // Unicode validation: check valid ranges
//!     if code < 0 || code > 0x10FFFF { return None }
//!     if code >= 0xD800 && code <= 0xDFFF { return None }  // Surrogates
//!     Some(code as Char)
//! }
//!
//! fn str_to_int(s: String) -> Option<Int> {
//!     let mut result = 0
//!     let mut negative = false
//!     let mut i = 0
//!     // Skip whitespace, handle sign, accumulate digits
//!     while i < s.len {
//!         let c = s.ptr[i]
//!         if c >= '0' && c <= '9' {
//!             result = result * 10 + (c - '0')
//!         }
//!         i = i + 1
//!     }
//!     if negative { -result } else { result }
//! }
//!
//! fn int_to_string(n: Int) -> String {
//!     // Division/modulo loop, build digits in reverse
//!     let buf = __alloc(21)  // Max i64 digits + sign
//!     let mut i = 20
//!     let negative = n < 0
//!     let mut n = if negative { -n } else { n }
//!     while n > 0 || i == 20 {
//!         buf[i] = '0' + (n % 10)
//!         n = n / 10
//!         i = i - 1
//!     }
//!     if negative { buf[i] = '-'; i = i - 1 }
//!     String { ptr: buf + i + 1, len: 20 - i }
//! }
//!
//! fn bool_to_string(b: Bool) -> String {
//!     if b { "true" } else { "false" }
//! }
//! ```

use cranelift_codegen::ir::{Type, types};

use super::{Runtime, default_call_conv, make_signature};

/// FFI-safe string result (ptr, len).
#[repr(C)]
pub struct StringResult {
    pub ptr: *const u8,
    pub len: i64,
}

/// Register conversion intrinsics.
///
/// # Parameters
/// - `ptr_ty`: The pointer type for this target (I32 for 32-bit, I64 for 64-bit)
pub fn register(runtime: &mut Runtime, ptr_ty: Type) {
    let call_conv = default_call_conv();

    // __str_to_float: (ptr, len: I64) -> F64 (returns NaN on parse failure)
    // Float parsing is genuinely complex (strtod algorithms)
    runtime.register(
        "__str_to_float",
        __str_to_float as *const u8,
        make_signature(call_conv, &[ptr_ty, types::I64], &[types::F64]),
    );

    // __float_to_string: (F64) -> (ptr, I64)
    // Float formatting is genuinely complex (Grisu/Ryu algorithms)
    runtime.register(
        "__float_to_string",
        __float_to_string as *const u8,
        make_signature(call_conv, &[types::F64], &[ptr_ty, types::I64]),
    );
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

/// Convert a float to a string.
///
/// Allocates a new string containing the decimal representation of the value.
/// Special values: returns "NaN" for NaN, "inf" or "-inf" for infinities.
///
/// # Ownership
///
/// Caller owns the returned string and must call `__free(result.ptr)` when done.
pub extern "C" fn __float_to_string(value: f64) -> StringResult {
    use super::memory::{__alloc, __memcpy};

    let s = value.to_string();
    let len = s.len() as i64;
    let ptr = __alloc(len);

    if ptr.is_null() {
        return StringResult {
            ptr: std::ptr::null(),
            len: 0,
        };
    }

    __memcpy(ptr, s.as_ptr().cast_mut(), len);
    StringResult { ptr, len }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

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

    #[test]
    fn float_to_string_positive() {
        let result = __float_to_string(3.25);
        assert!(!result.ptr.is_null());
        let s = unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                result.ptr,
                result.len as usize,
            ))
        };
        assert!(s.starts_with("3.25"));
        super::super::memory::__free(result.ptr as *mut u8);
    }

    #[test]
    fn float_to_string_integer() {
        let result = __float_to_string(42.0);
        assert!(!result.ptr.is_null());
        let s = unsafe { std::slice::from_raw_parts(result.ptr, result.len as usize) };
        assert_eq!(s, b"42");
        super::super::memory::__free(result.ptr as *mut u8);
    }

    #[test]
    fn float_to_string_nan() {
        let result = __float_to_string(f64::NAN);
        let s = unsafe { std::slice::from_raw_parts(result.ptr, result.len as usize) };
        assert_eq!(s, b"NaN");
        super::super::memory::__free(result.ptr as *mut u8);
    }

    #[test]
    fn float_to_string_infinity() {
        let result = __float_to_string(f64::INFINITY);
        let s = unsafe { std::slice::from_raw_parts(result.ptr, result.len as usize) };
        assert_eq!(s, b"inf");
        super::super::memory::__free(result.ptr as *mut u8);

        let result = __float_to_string(f64::NEG_INFINITY);
        let s = unsafe { std::slice::from_raw_parts(result.ptr, result.len as usize) };
        assert_eq!(s, b"-inf");
        super::super::memory::__free(result.ptr as *mut u8);
    }

    #[test]
    fn float_to_string_negative() {
        let result = __float_to_string(-2.5);
        let s = unsafe { std::slice::from_raw_parts(result.ptr, result.len as usize) };
        assert_eq!(s, b"-2.5");
        super::super::memory::__free(result.ptr as *mut u8);
    }

    #[test]
    fn register_adds_convert_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        assert!(runtime.contains("__str_to_float"));
        assert!(runtime.contains("__float_to_string"));
    }

    #[test]
    fn str_to_float_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__str_to_float").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert_eq!(func.signature.params[1].value_type, types::I64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::F64);
    }

    #[test]
    fn float_to_string_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime, types::I64);

        let func = runtime.get("__float_to_string").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::F64);
        assert_eq!(func.signature.returns.len(), 2);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
        assert_eq!(func.signature.returns[1].value_type, types::I64);
    }

    // ==================== Pointer type tests (32-bit simulation) ====================

    #[test]
    fn str_to_float_signature_uses_pointer_type() {
        let mut runtime = Runtime::new();
        let ptr_ty = types::I32; // Simulate 32-bit platform
        register(&mut runtime, ptr_ty);

        let func = runtime.get("__str_to_float").unwrap();
        assert_eq!(func.signature.params[0].value_type, ptr_ty); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert_eq!(func.signature.returns[0].value_type, types::F64); // result
    }

    #[test]
    fn float_to_string_signature_uses_pointer_type() {
        let mut runtime = Runtime::new();
        let ptr_ty = types::I32;
        register(&mut runtime, ptr_ty);

        let func = runtime.get("__float_to_string").unwrap();
        assert_eq!(func.signature.params[0].value_type, types::F64); // value
        assert_eq!(func.signature.returns[0].value_type, ptr_ty); // result ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // result len
    }
}
