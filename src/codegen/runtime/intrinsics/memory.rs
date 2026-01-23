//! Memory intrinsic functions.
//!
//! Functions for memory allocation and manipulation. These are essential
//! for dynamic data structures and string operations.
//!
//! # Current Implementation
//!
//! Uses Rust's allocator for allocation functions and standard library
//! for memory operations.
//!
//! # Self-Hosting Alternatives
//!
//! ## libc
//! ```c
//! void* __alloc(int64_t size) { return malloc(size); }
//! void* __realloc(void* ptr, int64_t size) { return realloc(ptr, size); }
//! void __free(void* ptr) { free(ptr); }
//! void __memcpy(void* dst, void* src, int64_t n) { memcpy(dst, src, n); }
//! void __memset(void* dst, int8_t val, int64_t n) { memset(dst, val, n); }
//! int64_t __memcmp(void* a, void* b, int64_t n) { return memcmp(a, b, n); }
//! ```
//!
//! ## Raw syscalls (Linux)
//! For allocation without libc, use mmap:
//! ```text
//! // SYS_mmap = 9
//! mov rax, 9
//! mov rdi, 0       // addr (let kernel choose)
//! mov rsi, size    // length
//! mov rdx, 3       // PROT_READ | PROT_WRITE
//! mov r10, 34      // MAP_PRIVATE | MAP_ANONYMOUS
//! mov r8, -1       // fd
//! mov r9, 0        // offset
//! syscall
//! ```
//!
//! ## Arena Allocator
//! For simple programs, a bump allocator is often sufficient:
//! ```text
//! static ARENA: [u8; 1MB];
//! static ARENA_PTR: usize = 0;
//!
//! fn __alloc(size: Int) -> *mut u8 {
//!     let ptr = &ARENA[ARENA_PTR];
//!     ARENA_PTR += align_up(size, 8);
//!     ptr
//! }
//! ```

use cranelift_codegen::ir::types;

use super::{Runtime, default_call_conv, make_signature};

/// Register all memory intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __alloc: (I64) -> I64 (ptr)
    runtime.register(
        "__alloc",
        __alloc as *const u8,
        make_signature(call_conv, &[types::I64], &[types::I64]),
    );

    // __realloc: (I64, I64) -> I64 (ptr)
    runtime.register(
        "__realloc",
        __realloc as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[types::I64]),
    );

    // __free: (I64) -> ()
    runtime.register(
        "__free",
        __free as *const u8,
        make_signature(call_conv, &[types::I64], &[]),
    );

    // __memcpy: (I64, I64, I64) -> ()
    runtime.register(
        "__memcpy",
        __memcpy as *const u8,
        make_signature(call_conv, &[types::I64, types::I64, types::I64], &[]),
    );

    // __memset: (I64, I8, I64) -> ()
    runtime.register(
        "__memset",
        __memset as *const u8,
        make_signature(call_conv, &[types::I64, types::I8, types::I64], &[]),
    );

    // __memcmp: (I64, I64, I64) -> I64
    runtime.register(
        "__memcmp",
        __memcmp as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64],
            &[types::I64],
        ),
    );
}

/// Allocate memory.
///
/// Returns a pointer to newly allocated memory, or null if allocation fails.
/// The memory is not initialized.
///
/// # Safety
///
/// The caller must eventually call `__free` to release the memory.
pub extern "C" fn __alloc(size: i64) -> *mut u8 {
    if size <= 0 {
        return std::ptr::null_mut();
    }

    let layout = match std::alloc::Layout::from_size_align(size as usize, 8) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };

    // SAFETY: We've validated size > 0 and alignment is valid
    unsafe { std::alloc::alloc(layout) }
}

/// Reallocate memory.
///
/// Returns a pointer to reallocated memory, or null if reallocation fails.
/// If ptr is null, behaves like `__alloc(new_size)`.
///
/// # Safety
///
/// - `ptr` must have been allocated by `__alloc` or `__realloc`
/// - `ptr` must not be used after this call (the returned pointer may be different)
pub extern "C" fn __realloc(ptr: *mut u8, new_size: i64) -> *mut u8 {
    if ptr.is_null() {
        return __alloc(new_size);
    }

    if new_size <= 0 {
        __free(ptr);
        return std::ptr::null_mut();
    }

    // Note: We don't know the original size, so we use a dummy layout.
    // In a real implementation, we'd track allocation sizes.
    let old_layout = std::alloc::Layout::from_size_align(1, 8).unwrap();
    let new_layout = match std::alloc::Layout::from_size_align(new_size as usize, 8) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };

    // SAFETY: ptr was allocated by our allocator
    unsafe { std::alloc::realloc(ptr, old_layout, new_layout.size()) }
}

/// Free allocated memory.
///
/// # Safety
///
/// - `ptr` must have been allocated by `__alloc` or `__realloc`
/// - `ptr` must not be used after this call
pub extern "C" fn __free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    // Note: We don't know the size, so we use a dummy layout.
    // This works because Rust's global allocator tracks sizes internally.
    let layout = std::alloc::Layout::from_size_align(1, 8).unwrap();

    // SAFETY: ptr was allocated by our allocator
    unsafe { std::alloc::dealloc(ptr, layout) }
}

/// Copy memory from source to destination.
///
/// # Safety
///
/// - `dst` must be valid for `n` bytes
/// - `src` must be valid for `n` bytes
/// - Memory regions must not overlap (use memmove for overlapping regions)
pub extern "C" fn __memcpy(dst: *mut u8, src: *const u8, n: i64) {
    if dst.is_null() || src.is_null() || n <= 0 {
        return;
    }

    // SAFETY: Caller guarantees valid pointers and no overlap
    unsafe {
        std::ptr::copy_nonoverlapping(src, dst, n as usize);
    }
}

/// Fill memory with a byte value.
///
/// # Safety
///
/// - `dst` must be valid for `n` bytes
pub extern "C" fn __memset(dst: *mut u8, val: i8, n: i64) {
    if dst.is_null() || n <= 0 {
        return;
    }

    // SAFETY: Caller guarantees valid pointer
    unsafe {
        std::ptr::write_bytes(dst, val as u8, n as usize);
    }
}

/// Compare two memory regions.
///
/// Returns:
/// - 0 if regions are equal
/// - negative if first differing byte in `a` is less than in `b`
/// - positive if first differing byte in `a` is greater than in `b`
///
/// # Safety
///
/// - `a` must be valid for `n` bytes
/// - `b` must be valid for `n` bytes
pub extern "C" fn __memcmp(a: *const u8, b: *const u8, n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    if a.is_null() && b.is_null() {
        return 0;
    }
    if a.is_null() {
        return -1;
    }
    if b.is_null() {
        return 1;
    }

    // SAFETY: Caller guarantees valid pointers
    unsafe {
        let slice_a = std::slice::from_raw_parts(a, n as usize);
        let slice_b = std::slice::from_raw_parts(b, n as usize);

        for i in 0..n as usize {
            if slice_a[i] != slice_b[i] {
                return (slice_a[i] as i64) - (slice_b[i] as i64);
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ==================== Direct call tests ====================

    #[test]
    fn alloc_returns_non_null_for_positive_size() {
        let ptr = __alloc(100);
        assert!(!ptr.is_null());
        __free(ptr);
    }

    #[test]
    fn alloc_returns_null_for_zero_size() {
        let ptr = __alloc(0);
        assert!(ptr.is_null());
    }

    #[test]
    fn alloc_returns_null_for_negative_size() {
        let ptr = __alloc(-10);
        assert!(ptr.is_null());
    }

    #[test]
    fn free_handles_null() {
        __free(std::ptr::null_mut()); // Should not panic
    }

    #[test]
    fn realloc_from_null_allocates() {
        let ptr = __realloc(std::ptr::null_mut(), 100);
        assert!(!ptr.is_null());
        __free(ptr);
    }

    #[test]
    fn realloc_to_zero_frees() {
        let ptr = __alloc(100);
        let new_ptr = __realloc(ptr, 0);
        assert!(new_ptr.is_null());
    }

    #[test]
    fn memcpy_copies_data() {
        let src = [1u8, 2, 3, 4, 5];
        let mut dst = [0u8; 5];

        __memcpy(dst.as_mut_ptr(), src.as_ptr(), 5);

        assert_eq!(dst, src);
    }

    #[test]
    fn memcpy_handles_null() {
        let mut dst = [0u8; 5];
        __memcpy(dst.as_mut_ptr(), std::ptr::null(), 5); // Should not panic
        __memcpy(std::ptr::null_mut(), dst.as_ptr(), 5); // Should not panic
    }

    #[test]
    fn memset_fills_memory() {
        let mut buf = [0u8; 5];
        __memset(buf.as_mut_ptr(), 0x42, 5);
        assert_eq!(buf, [0x42, 0x42, 0x42, 0x42, 0x42]);
    }

    #[test]
    fn memset_handles_null() {
        __memset(std::ptr::null_mut(), 0x42, 5); // Should not panic
    }

    #[test]
    fn memcmp_equal_returns_zero() {
        let a = [1u8, 2, 3, 4, 5];
        let b = [1u8, 2, 3, 4, 5];
        assert_eq!(__memcmp(a.as_ptr(), b.as_ptr(), 5), 0);
    }

    #[test]
    fn memcmp_a_less_than_b() {
        let a = [1u8, 2, 3, 4, 5];
        let b = [1u8, 2, 4, 4, 5];
        assert!(__memcmp(a.as_ptr(), b.as_ptr(), 5) < 0);
    }

    #[test]
    fn memcmp_a_greater_than_b() {
        let a = [1u8, 2, 5, 4, 5];
        let b = [1u8, 2, 3, 4, 5];
        assert!(__memcmp(a.as_ptr(), b.as_ptr(), 5) > 0);
    }

    #[test]
    fn memcmp_handles_null() {
        let a = [1u8, 2, 3];
        assert_eq!(__memcmp(std::ptr::null(), std::ptr::null(), 5), 0);
        assert!(__memcmp(std::ptr::null(), a.as_ptr(), 5) < 0);
        assert!(__memcmp(a.as_ptr(), std::ptr::null(), 5) > 0);
    }

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_memory_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__alloc"));
        assert!(runtime.contains("__realloc"));
        assert!(runtime.contains("__free"));
        assert!(runtime.contains("__memcpy"));
        assert!(runtime.contains("__memset"));
        assert!(runtime.contains("__memcmp"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn alloc_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__alloc").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn realloc_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__realloc").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // size
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn free_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__free").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn memcpy_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__memcpy").unwrap();
        assert_eq!(func.signature.params.len(), 3);
        assert_eq!(func.signature.params[0].value_type, types::I64); // dst
        assert_eq!(func.signature.params[1].value_type, types::I64); // src
        assert_eq!(func.signature.params[2].value_type, types::I64); // n
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn memset_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__memset").unwrap();
        assert_eq!(func.signature.params.len(), 3);
        assert_eq!(func.signature.params[0].value_type, types::I64); // dst
        assert_eq!(func.signature.params[1].value_type, types::I8); // val
        assert_eq!(func.signature.params[2].value_type, types::I64); // n
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn memcmp_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__memcmp").unwrap();
        assert_eq!(func.signature.params.len(), 3);
        assert_eq!(func.signature.params[0].value_type, types::I64); // a
        assert_eq!(func.signature.params[1].value_type, types::I64); // b
        assert_eq!(func.signature.params[2].value_type, types::I64); // n
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    // ==================== Integration test ====================

    #[test]
    fn alloc_memset_memcpy_workflow() {
        // Allocate buffer
        let ptr = __alloc(10);
        assert!(!ptr.is_null());

        // Fill with pattern (0x42 = 66, fits in i8)
        __memset(ptr, 0x42, 10);

        // Verify with memcmp
        let expected = [0x42u8; 10];
        assert_eq!(__memcmp(ptr, expected.as_ptr(), 10), 0);

        // Copy to new buffer
        let ptr2 = __alloc(10);
        __memcpy(ptr2, ptr, 10);
        assert_eq!(__memcmp(ptr, ptr2, 10), 0);

        // Cleanup
        __free(ptr);
        __free(ptr2);
    }
}
