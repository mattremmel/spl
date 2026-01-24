//! String intrinsic functions.
//!
//! Functions for string operations including comparison and searching.
//!
//! # String Representation
//!
//! SPL strings are represented as (pointer, length) pairs, not null-terminated.
//! This is more efficient and allows embedded nulls, but requires passing both
//! values through the ABI.
//!
//! # Query Operations
//!
//! - `__str_len`: Returns the length parameter
//! - `__str_eq`: Compare two strings for equality
//! - `__str_cmp`: Compare two strings lexicographically
//! - `__str_find`: Find substring in string
//! - `__str_contains`: Check if string contains substring
//! - `__str_starts_with`: Check if string starts with prefix
//! - `__str_ends_with`: Check if string ends with suffix
//! - `__str_char_at`: Get character at index
//!
//! # Future Stdlib Candidates
//!
//! The following can be implemented in SPL using `__alloc`/`__memcpy`:
//!
//! - `str_concat`: Concatenate two strings
//! - `str_slice`: Extract substring
//! - `to_upper`: Convert to uppercase (ASCII)
//! - `to_lower`: Convert to lowercase (ASCII)
//! - `int_to_string`: Format integer as string
//! - `bool_to_string`: Format boolean as "true"/"false"

use cranelift_codegen::ir::types;

use super::{Runtime, default_call_conv, make_signature};

/// Register all string intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // ==================== Query operations ====================

    // __str_len: (*const u8, I64) -> I64
    runtime.register(
        "__str_len",
        __str_len as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[types::I64]),
    );

    // __str_eq: (ptr1, len1, ptr2, len2) -> I8 (bool)
    runtime.register(
        "__str_eq",
        __str_eq as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64, types::I64],
            &[types::I8],
        ),
    );

    // __str_cmp: (ptr1, len1, ptr2, len2) -> I64 (-1, 0, 1)
    runtime.register(
        "__str_cmp",
        __str_cmp as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64, types::I64],
            &[types::I64],
        ),
    );

    // __str_find: (haystack_ptr, haystack_len, needle_ptr, needle_len) -> I64 (index or -1)
    runtime.register(
        "__str_find",
        __str_find as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64, types::I64],
            &[types::I64],
        ),
    );

    // __str_contains: (haystack_ptr, haystack_len, needle_ptr, needle_len) -> I8
    runtime.register(
        "__str_contains",
        __str_contains as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64, types::I64],
            &[types::I8],
        ),
    );

    // __str_starts_with: (str_ptr, str_len, prefix_ptr, prefix_len) -> I8
    runtime.register(
        "__str_starts_with",
        __str_starts_with as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64, types::I64],
            &[types::I8],
        ),
    );

    // __str_ends_with: (str_ptr, str_len, suffix_ptr, suffix_len) -> I8
    runtime.register(
        "__str_ends_with",
        __str_ends_with as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64, types::I64],
            &[types::I8],
        ),
    );

    // __str_char_at: (ptr, len, index) -> I32 (char or -1)
    runtime.register(
        "__str_char_at",
        __str_char_at as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64],
            &[types::I32],
        ),
    );
}

// ==================== Query operations ====================

/// Get the length of a string.
///
/// Returns the provided length, or 0 if the pointer is null.
pub extern "C" fn __str_len(ptr: *const u8, len: i64) -> i64 {
    if ptr.is_null() { 0 } else { len }
}

/// Compare two strings for equality.
///
/// Returns 1 if equal, 0 otherwise.
pub extern "C" fn __str_eq(ptr1: *const u8, len1: i64, ptr2: *const u8, len2: i64) -> i8 {
    if len1 != len2 {
        return 0;
    }
    if ptr1.is_null() && ptr2.is_null() {
        return 1;
    }
    if ptr1.is_null() || ptr2.is_null() {
        return 0;
    }
    if len1 <= 0 {
        return 1; // Both empty
    }

    // SAFETY: Caller guarantees valid pointers
    unsafe {
        let slice1 = std::slice::from_raw_parts(ptr1, len1 as usize);
        let slice2 = std::slice::from_raw_parts(ptr2, len2 as usize);
        if slice1 == slice2 { 1 } else { 0 }
    }
}

/// Compare two strings lexicographically.
///
/// Returns:
/// - negative if str1 < str2
/// - 0 if str1 == str2
/// - positive if str1 > str2
pub extern "C" fn __str_cmp(ptr1: *const u8, len1: i64, ptr2: *const u8, len2: i64) -> i64 {
    if ptr1.is_null() && ptr2.is_null() {
        return 0;
    }
    if ptr1.is_null() {
        return -1;
    }
    if ptr2.is_null() {
        return 1;
    }

    // SAFETY: Caller guarantees valid pointers
    unsafe {
        let slice1 = if len1 > 0 {
            std::slice::from_raw_parts(ptr1, len1 as usize)
        } else {
            &[]
        };
        let slice2 = if len2 > 0 {
            std::slice::from_raw_parts(ptr2, len2 as usize)
        } else {
            &[]
        };

        match slice1.cmp(slice2) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }
}

/// Find the first occurrence of a substring.
///
/// Returns the byte index of the first match, or -1 if not found.
pub extern "C" fn __str_find(
    haystack_ptr: *const u8,
    haystack_len: i64,
    needle_ptr: *const u8,
    needle_len: i64,
) -> i64 {
    if haystack_ptr.is_null() || needle_ptr.is_null() {
        return -1;
    }
    if needle_len <= 0 {
        return 0; // Empty needle matches at start
    }
    if haystack_len <= 0 || needle_len > haystack_len {
        return -1;
    }

    // SAFETY: Caller guarantees valid pointers
    unsafe {
        let haystack = std::slice::from_raw_parts(haystack_ptr, haystack_len as usize);
        let needle = std::slice::from_raw_parts(needle_ptr, needle_len as usize);

        // Simple search (could be optimized with Boyer-Moore, etc.)
        for i in 0..=(haystack_len - needle_len) as usize {
            if haystack[i..i + needle.len()] == *needle {
                return i as i64;
            }
        }
        -1
    }
}

/// Check if a string contains a substring.
///
/// Returns 1 if found, 0 otherwise.
pub extern "C" fn __str_contains(
    haystack_ptr: *const u8,
    haystack_len: i64,
    needle_ptr: *const u8,
    needle_len: i64,
) -> i8 {
    if __str_find(haystack_ptr, haystack_len, needle_ptr, needle_len) >= 0 {
        1
    } else {
        0
    }
}

/// Check if a string starts with a prefix.
///
/// Returns 1 if true, 0 otherwise.
pub extern "C" fn __str_starts_with(
    str_ptr: *const u8,
    str_len: i64,
    prefix_ptr: *const u8,
    prefix_len: i64,
) -> i8 {
    if prefix_len <= 0 {
        return 1; // Empty prefix always matches
    }
    if str_ptr.is_null() || prefix_ptr.is_null() {
        return 0;
    }
    if prefix_len > str_len {
        return 0;
    }

    // SAFETY: Caller guarantees valid pointers
    unsafe {
        let str_slice = std::slice::from_raw_parts(str_ptr, prefix_len as usize);
        let prefix_slice = std::slice::from_raw_parts(prefix_ptr, prefix_len as usize);
        if str_slice == prefix_slice { 1 } else { 0 }
    }
}

/// Check if a string ends with a suffix.
///
/// Returns 1 if true, 0 otherwise.
pub extern "C" fn __str_ends_with(
    str_ptr: *const u8,
    str_len: i64,
    suffix_ptr: *const u8,
    suffix_len: i64,
) -> i8 {
    if suffix_len <= 0 {
        return 1; // Empty suffix always matches
    }
    if str_ptr.is_null() || suffix_ptr.is_null() {
        return 0;
    }
    if suffix_len > str_len {
        return 0;
    }

    // SAFETY: Caller guarantees valid pointers
    unsafe {
        let start = (str_len - suffix_len) as usize;
        let str_slice = std::slice::from_raw_parts(str_ptr.add(start), suffix_len as usize);
        let suffix_slice = std::slice::from_raw_parts(suffix_ptr, suffix_len as usize);
        if str_slice == suffix_slice { 1 } else { 0 }
    }
}

/// Get the character at a byte index.
///
/// Returns the Unicode code point at the given byte index, or -1 if:
/// - index is out of bounds
/// - index points to the middle of a multi-byte character
/// - the string is invalid UTF-8
pub extern "C" fn __str_char_at(ptr: *const u8, len: i64, index: i64) -> i32 {
    if ptr.is_null() || index < 0 || index >= len {
        return -1;
    }

    // SAFETY: Caller guarantees valid pointer
    unsafe {
        let slice = std::slice::from_raw_parts(ptr, len as usize);
        match std::str::from_utf8(slice) {
            Ok(s) => {
                // Find the character that starts at this byte index
                for (byte_idx, ch) in s.char_indices() {
                    if byte_idx == index as usize {
                        return ch as i32;
                    }
                    if byte_idx > index as usize {
                        break; // Passed the index, must be middle of char
                    }
                }
                -1
            }
            Err(_) => -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ==================== Direct call tests ====================

    #[test]
    fn str_len_returns_length() {
        let s = "Hello";
        let len = __str_len(s.as_ptr(), s.len() as i64);
        assert_eq!(len, 5);
    }

    #[test]
    fn str_len_null_returns_zero() {
        let len = __str_len(std::ptr::null(), 10);
        assert_eq!(len, 0);
    }

    #[test]
    fn str_len_empty_string() {
        let s = "";
        let len = __str_len(s.as_ptr(), 0);
        assert_eq!(len, 0);
    }

    #[test]
    fn str_eq_equal_strings() {
        let s1 = "Hello";
        let s2 = "Hello";
        assert_eq!(
            __str_eq(s1.as_ptr(), s1.len() as i64, s2.as_ptr(), s2.len() as i64),
            1
        );
    }

    #[test]
    fn str_eq_different_strings() {
        let s1 = "Hello";
        let s2 = "World";
        assert_eq!(
            __str_eq(s1.as_ptr(), s1.len() as i64, s2.as_ptr(), s2.len() as i64),
            0
        );
    }

    #[test]
    fn str_eq_different_lengths() {
        let s1 = "Hello";
        let s2 = "Hell";
        assert_eq!(
            __str_eq(s1.as_ptr(), s1.len() as i64, s2.as_ptr(), s2.len() as i64),
            0
        );
    }

    #[test]
    fn str_cmp_equal() {
        let s1 = "abc";
        let s2 = "abc";
        assert_eq!(
            __str_cmp(s1.as_ptr(), s1.len() as i64, s2.as_ptr(), s2.len() as i64),
            0
        );
    }

    #[test]
    fn str_cmp_less_than() {
        let s1 = "abc";
        let s2 = "abd";
        assert!(__str_cmp(s1.as_ptr(), s1.len() as i64, s2.as_ptr(), s2.len() as i64) < 0);
    }

    #[test]
    fn str_cmp_greater_than() {
        let s1 = "abd";
        let s2 = "abc";
        assert!(__str_cmp(s1.as_ptr(), s1.len() as i64, s2.as_ptr(), s2.len() as i64) > 0);
    }

    #[test]
    fn str_cmp_prefix() {
        let s1 = "abc";
        let s2 = "abcd";
        assert!(__str_cmp(s1.as_ptr(), s1.len() as i64, s2.as_ptr(), s2.len() as i64) < 0);
    }

    #[test]
    fn str_find_found() {
        let haystack = "Hello, World!";
        let needle = "World";
        assert_eq!(
            __str_find(
                haystack.as_ptr(),
                haystack.len() as i64,
                needle.as_ptr(),
                needle.len() as i64
            ),
            7
        );
    }

    #[test]
    fn str_find_not_found() {
        let haystack = "Hello, World!";
        let needle = "Foo";
        assert_eq!(
            __str_find(
                haystack.as_ptr(),
                haystack.len() as i64,
                needle.as_ptr(),
                needle.len() as i64
            ),
            -1
        );
    }

    #[test]
    fn str_find_at_start() {
        let haystack = "Hello";
        let needle = "He";
        assert_eq!(
            __str_find(
                haystack.as_ptr(),
                haystack.len() as i64,
                needle.as_ptr(),
                needle.len() as i64
            ),
            0
        );
    }

    #[test]
    fn str_find_empty_needle() {
        let haystack = "Hello";
        let needle = "";
        assert_eq!(
            __str_find(
                haystack.as_ptr(),
                haystack.len() as i64,
                needle.as_ptr(),
                0
            ),
            0
        );
    }

    #[test]
    fn str_contains_true() {
        let haystack = "Hello, World!";
        let needle = "World";
        assert_eq!(
            __str_contains(
                haystack.as_ptr(),
                haystack.len() as i64,
                needle.as_ptr(),
                needle.len() as i64
            ),
            1
        );
    }

    #[test]
    fn str_contains_false() {
        let haystack = "Hello, World!";
        let needle = "Foo";
        assert_eq!(
            __str_contains(
                haystack.as_ptr(),
                haystack.len() as i64,
                needle.as_ptr(),
                needle.len() as i64
            ),
            0
        );
    }

    #[test]
    fn str_starts_with_true() {
        let s = "Hello, World!";
        let prefix = "Hello";
        assert_eq!(
            __str_starts_with(s.as_ptr(), s.len() as i64, prefix.as_ptr(), prefix.len() as i64),
            1
        );
    }

    #[test]
    fn str_starts_with_false() {
        let s = "Hello, World!";
        let prefix = "World";
        assert_eq!(
            __str_starts_with(s.as_ptr(), s.len() as i64, prefix.as_ptr(), prefix.len() as i64),
            0
        );
    }

    #[test]
    fn str_ends_with_true() {
        let s = "Hello, World!";
        let suffix = "World!";
        assert_eq!(
            __str_ends_with(s.as_ptr(), s.len() as i64, suffix.as_ptr(), suffix.len() as i64),
            1
        );
    }

    #[test]
    fn str_ends_with_false() {
        let s = "Hello, World!";
        let suffix = "Hello";
        assert_eq!(
            __str_ends_with(s.as_ptr(), s.len() as i64, suffix.as_ptr(), suffix.len() as i64),
            0
        );
    }

    #[test]
    fn str_char_at_ascii() {
        let s = "Hello";
        assert_eq!(__str_char_at(s.as_ptr(), s.len() as i64, 0), 'H' as i32);
        assert_eq!(__str_char_at(s.as_ptr(), s.len() as i64, 4), 'o' as i32);
    }

    #[test]
    fn str_char_at_unicode() {
        let s = "Héllo";
        assert_eq!(__str_char_at(s.as_ptr(), s.len() as i64, 0), 'H' as i32);
        assert_eq!(__str_char_at(s.as_ptr(), s.len() as i64, 1), 'é' as i32);
        // Index 2 is middle of 'é' (2-byte char), should return -1
        assert_eq!(__str_char_at(s.as_ptr(), s.len() as i64, 2), -1);
        assert_eq!(__str_char_at(s.as_ptr(), s.len() as i64, 3), 'l' as i32);
    }

    #[test]
    fn str_char_at_out_of_bounds() {
        let s = "Hello";
        assert_eq!(__str_char_at(s.as_ptr(), s.len() as i64, 10), -1);
        assert_eq!(__str_char_at(s.as_ptr(), s.len() as i64, -1), -1);
    }

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_string_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__str_len"));
        assert!(runtime.contains("__str_eq"));
        assert!(runtime.contains("__str_cmp"));
        assert!(runtime.contains("__str_find"));
        assert!(runtime.contains("__str_contains"));
        assert!(runtime.contains("__str_starts_with"));
        assert!(runtime.contains("__str_ends_with"));
        assert!(runtime.contains("__str_char_at"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn str_len_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__str_len").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn str_eq_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__str_eq").unwrap();
        assert_eq!(func.signature.params.len(), 4);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I8);
    }

    #[test]
    fn str_cmp_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__str_cmp").unwrap();
        assert_eq!(func.signature.params.len(), 4);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn str_char_at_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__str_char_at").unwrap();
        assert_eq!(func.signature.params.len(), 3);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert_eq!(func.signature.params[2].value_type, types::I64); // index
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I32);
    }

    // ==================== Edge case tests ====================

    #[test]
    fn str_len_with_negative_length() {
        let s = "Hello";
        // Even with negative length, we just return it
        let len = __str_len(s.as_ptr(), -5);
        assert_eq!(len, -5);
    }

    #[test]
    fn str_eq_both_null() {
        assert_eq!(__str_eq(std::ptr::null(), 0, std::ptr::null(), 0), 1);
    }

    #[test]
    fn str_cmp_null_handling() {
        let s = "hello";
        assert_eq!(__str_cmp(std::ptr::null(), 0, std::ptr::null(), 0), 0);
        assert!(__str_cmp(std::ptr::null(), 0, s.as_ptr(), s.len() as i64) < 0);
        assert!(__str_cmp(s.as_ptr(), s.len() as i64, std::ptr::null(), 0) > 0);
    }

    // ==================== JIT integration tests ====================

    #[test]
    fn jit_call_str_len() {
        use crate::codegen::context::CodegenContext;
        use cranelift_codegen::ir::{AbiParam, InstBuilder};
        use cranelift_frontend::FunctionBuilder;
        use cranelift_module::{Linkage, Module};
        use std::mem;

        let mut runtime = Runtime::new();
        register(&mut runtime);
        let len_sig = runtime.get("__str_len").unwrap().signature.clone();

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        // Create wrapper: fn test(ptr: i64, len: i64) -> i64 { __str_len(ptr, len) }
        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.params.push(AbiParam::new(types::I64));
        wrapper_sig.params.push(AbiParam::new(types::I64));
        wrapper_sig.returns.push(AbiParam::new(types::I64));
        let wrapper_id = ctx.declare_function("test", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();

            let len_func_id = module
                .declare_function("__str_len", Linkage::Import, &len_sig)
                .unwrap();

            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(len_func_id, builder.func);
            let ptr = builder.block_params(entry)[0];
            let len = builder.block_params(entry)[1];
            let call = builder.ins().call(func_ref, &[ptr, len]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let test: fn(i64, i64) -> i64 = unsafe { mem::transmute(ptr) };

        let s = "Hello";
        let result = test(s.as_ptr() as i64, s.len() as i64);
        assert_eq!(result, 5);

        // Test with null
        let result = test(0, 10);
        assert_eq!(result, 0);
    }
}
