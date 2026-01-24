//! String operations - stdlib candidates, not intrinsics.
//!
//! # Why No String Intrinsics?
//!
//! All string operations can be implemented in SPL using the memory intrinsics
//! (`__memcmp`, `__memcpy`, `__alloc`) and basic arithmetic. No special runtime
//! support is needed.
//!
//! # String Representation
//!
//! SPL strings are (pointer, length) pairs. This representation enables all
//! operations to be implemented with pointer arithmetic and memory primitives.
//!
//! # Stdlib Implementations
//!
//! ```text
//! fn str_len(s: String) -> Int {
//!     s.len  // Just access the length field
//! }
//!
//! fn str_eq(a: String, b: String) -> Bool {
//!     if a.len != b.len { return false }
//!     __memcmp(a.ptr, b.ptr, a.len) == 0
//! }
//!
//! fn str_cmp(a: String, b: String) -> Int {
//!     let min_len = if a.len < b.len { a.len } else { b.len }
//!     let cmp = __memcmp(a.ptr, b.ptr, min_len)
//!     if cmp != 0 { return cmp }
//!     a.len - b.len  // Shorter string is "less"
//! }
//!
//! fn str_starts_with(s: String, prefix: String) -> Bool {
//!     if prefix.len > s.len { return false }
//!     __memcmp(s.ptr, prefix.ptr, prefix.len) == 0
//! }
//!
//! fn str_ends_with(s: String, suffix: String) -> Bool {
//!     if suffix.len > s.len { return false }
//!     __memcmp(s.ptr + s.len - suffix.len, suffix.ptr, suffix.len) == 0
//! }
//!
//! fn str_find(haystack: String, needle: String) -> Int {
//!     if needle.len == 0 { return 0 }
//!     if needle.len > haystack.len { return -1 }
//!     for i in 0..=(haystack.len - needle.len) {
//!         if __memcmp(haystack.ptr + i, needle.ptr, needle.len) == 0 {
//!             return i
//!         }
//!     }
//!     -1
//! }
//!
//! fn str_contains(haystack: String, needle: String) -> Bool {
//!     str_find(haystack, needle) >= 0
//! }
//!
//! fn str_concat(a: String, b: String) -> String {
//!     let buf = __alloc(a.len + b.len)
//!     __memcpy(buf, a.ptr, a.len)
//!     __memcpy(buf + a.len, b.ptr, b.len)
//!     String { ptr: buf, len: a.len + b.len }
//! }
//!
//! fn str_slice(s: String, start: Int, end: Int) -> String {
//!     // For a view (no alloc): String { ptr: s.ptr + start, len: end - start }
//!     // For a copy:
//!     let len = end - start
//!     let buf = __alloc(len)
//!     __memcpy(buf, s.ptr + start, len)
//!     String { ptr: buf, len: len }
//! }
//!
//! fn str_char_at(s: String, index: Int) -> Char {
//!     // UTF-8 decoding - check first byte to determine char width
//!     let b = s.ptr[index]
//!     if b < 0x80 { return b as Char }  // ASCII
//!     // ... handle 2/3/4 byte sequences
//! }
//! ```

use super::Runtime;

/// Register string intrinsics (none - all are stdlib candidates).
pub fn register(_runtime: &mut Runtime) {
    // No string intrinsics - all operations can be implemented in SPL
    // using __memcmp, __memcpy, __alloc, and basic arithmetic.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_adds_no_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        // No string intrinsics should exist
        assert!(!runtime.contains("__str_len"));
        assert!(!runtime.contains("__str_eq"));
        assert!(!runtime.contains("__str_cmp"));
        assert!(!runtime.contains("__str_find"));
        assert!(!runtime.contains("__str_contains"));
        assert!(!runtime.contains("__str_starts_with"));
        assert!(!runtime.contains("__str_ends_with"));
        assert!(!runtime.contains("__str_char_at"));
        assert!(!runtime.contains("__str_concat"));
        assert!(!runtime.contains("__str_slice"));
    }
}
