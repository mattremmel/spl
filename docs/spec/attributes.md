# Attributes

This document specifies attributes in SPL, including syntax, built-in attributes, conditional compilation, and derive macros.

## Overview

Attributes provide metadata for items. They enable:
- Compiler directives (`#[inline]`, `#[cold]`)
- Conditional compilation (`#[cfg(...)]`)
- Derive macros (`#[derive(Clone, Debug)]`)
- FFI configuration (`#[repr(C)]`, `#[link(...)]`)
- Documentation (`#[doc = "..."]`)
- Lints (`#[allow(...)]`, `#[warn(...)]`)

## 1. Attribute Syntax

### 1.1 Outer Attributes

Apply to the following item:

```spl
#[derive(Clone, Debug)]
struct Point(x: i32, y: i32)

#[inline]
fn fast_path(): () {
    // ...
}
```

### 1.2 Inner Attributes

Apply to the enclosing item (typically at file/module scope):

```spl
// At the top of a file
#![allow(unused_variables)]
#![doc = "This module provides..."]

fn example(): () {
    let x = 42;  // No warning about unused x
}
```

### 1.3 Attribute Arguments

Attributes support several argument forms:

```spl
// No arguments
#[inline]

// Simple identifier list
#[derive(Clone, Debug, PartialEq)]

// Key-value pairs
#[doc = "Documentation string"]
#[link(name = "sqlite3")]

// Nested attributes
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]

// Mixed
#[repr(C, align(8))]
```

---

## 2. Built-in Attributes

### 2.1 Code Generation

| Attribute | Target | Description |
|-----------|--------|-------------|
| `#[inline]` | Functions | Suggest inlining |
| `#[inline(always)]` | Functions | Force inlining |
| `#[inline(never)]` | Functions | Prevent inlining |
| `#[cold]` | Functions | Mark as unlikely to be called |
| `#[no_mangle]` | Functions | Preserve symbol name |
| `#[export_name = "..."]` | Functions | Use custom symbol name |

```spl
#[inline(always)]
fn critical_path(x: i32): i32 {
    return x * 2;
}

#[cold]
fn error_handler(msg: &str): () {
    panic(msg);
}

#[no_mangle]
pub fn spl_init(): () {
    // Can be called from C as spl_init()
}

#[export_name = "my_custom_name"]
pub fn internal_name(): () {
    // Symbol is "my_custom_name"
}
```

### 2.2 Memory Layout

| Attribute | Target | Description |
|-----------|--------|-------------|
| `#[repr(C)]` | Structs, Enums | C-compatible layout |
| `#[repr(transparent)]` | Structs | Same layout as single field |
| `#[repr(packed)]` | Structs | No padding between fields |
| `#[repr(align(N))]` | Structs | Minimum alignment of N bytes |
| `#[repr(u8)]` etc. | Enums | Discriminant type |

```spl
#[repr(C)]
struct CPoint(x: f64, y: f64)

#[repr(transparent)]
struct Wrapper(inner: u64)

#[repr(C, packed)]
struct PackedData(
    a: u8,
    b: u32,
    c: u8,
)  // Size is 6 bytes, not 8

#[repr(C, align(16))]
struct Aligned(data: [u8; 12])

#[repr(u8)]
enum Status {
    Inactive = 0,
    Active = 1,
    Pending = 2,
}
```

### 2.3 FFI and Linking

| Attribute | Target | Description |
|-----------|--------|-------------|
| `#[link(name = "...")]` | Extern blocks | Link to native library |
| `#[link(name = "...", kind = "...")]` | Extern blocks | Specify link kind |
| `#[link_name = "..."]` | Extern functions | Use different symbol name |

Link kinds:
- `"dylib"` (default) - Dynamic library
- `"static"` - Static library
- `"framework"` - macOS framework
- `"raw-dylib"` - Windows raw dynamic linking

```spl
#[link(name = "sqlite3")]
extern "C" {
    fn sqlite3_open(filename: Ptr(u8), db: MutPtr(MutPtr(Sqlite3))): i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: Ptr(())): ();
}

extern "C" {
    #[link_name = "secret_internal_name"]
    fn public_api(): ();
}
```

### 2.4 Testing

| Attribute | Target | Description |
|-----------|--------|-------------|
| `#[test]` | Functions | Mark as test |
| `#[ignore]` | Test functions | Skip by default |
| `#[should_panic]` | Test functions | Expect panic |
| `#[bench]` | Functions | Mark as benchmark |

```spl
#[test]
fn test_addition(): () {
    assert_eq(2 + 2, 4);
}

#[test]
#[ignore]
fn slow_test(): () {
    // Only runs with --include-ignored
}

#[test]
#[should_panic]
fn test_panic(): () {
    panic("expected");
}

#[test]
#[should_panic(expected = "out of bounds")]
fn test_specific_panic(): () {
    let v = [1, 2, 3];
    let _ = v[10];
}
```

### 2.5 Documentation

| Attribute | Target | Description |
|-----------|--------|-------------|
| `#[doc = "..."]` | Any item | Documentation |
| `#[doc(hidden)]` | Any item | Hide from documentation |
| `#[doc(alias = "...")]` | Any item | Add search alias |

```spl
#[doc = "A point in 2D space."]
#[doc = ""]
#[doc = "# Examples"]
#[doc = "```"]
#[doc = "let p = Point(x: 1.0, y: 2.0);"]
#[doc = "```"]
struct Point(x: f64, y: f64)

/// Shorthand for #[doc = "..."]
/// Multiple lines become separate #[doc] attributes.
fn documented(): () {
    // ...
}

#[doc(hidden)]
fn internal_helper(): () {
    // Not shown in generated docs
}

#[doc(alias = "len")]
fn length(&self): usize {
    // Searchable as "length" or "len"
}
```

---

## 3. Conditional Compilation

### 3.1 The `cfg` Attribute

```spl
#[cfg(target_os = "linux")]
fn linux_only(): () {
    // Only compiled on Linux
}

#[cfg(target_os = "windows")]
fn windows_only(): () {
    // Only compiled on Windows
}
```

### 3.2 Configuration Predicates

| Predicate | Example | Description |
|-----------|---------|-------------|
| `target_os` | `target_os = "linux"` | Operating system |
| `target_arch` | `target_arch = "x86_64"` | CPU architecture |
| `target_family` | `target_family = "unix"` | OS family |
| `target_env` | `target_env = "gnu"` | ABI environment |
| `target_pointer_width` | `target_pointer_width = "64"` | Pointer size |
| `target_endian` | `target_endian = "little"` | Byte order |
| `feature` | `feature = "async"` | Cargo feature |
| `debug_assertions` | `debug_assertions` | Debug build |
| `test` | `test` | Test build |

### 3.3 Combining Predicates

```spl
// All must be true
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]

// Any must be true
#[cfg(any(target_os = "linux", target_os = "macos"))]

// Negation
#[cfg(not(target_os = "windows"))]

// Complex combinations
#[cfg(all(
    target_family = "unix",
    not(target_os = "macos"),
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
```

### 3.4 `cfg_attr` - Conditional Attributes

Apply attributes only when a condition is true:

```spl
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct Config(name: String, value: i32)

// Multiple attributes
#[cfg_attr(debug_assertions, derive(Debug), allow(dead_code))]
struct Internal(data: Vec(T: u8))
```

### 3.5 Conditional Module Inclusion

```spl
#[cfg(target_os = "linux")]
mod linux_impl;

#[cfg(target_os = "windows")]
mod windows_impl;

#[cfg(target_os = "linux")]
use linux_impl as platform;

#[cfg(target_os = "windows")]
use windows_impl as platform;
```

### 3.6 `cfg!` Macro (Runtime Check)

For runtime branching based on compile-time configuration:

```spl
fn get_path(): String {
    if cfg!(target_os = "windows") {
        return "C:\\Program Files\\app";
    } else {
        return "/usr/local/app";
    }
}
```

Note: The dead branch is still type-checked but not compiled.

---

## 4. Derive Macros

### 4.1 Built-in Derives

| Derive | Description |
|--------|-------------|
| `Clone` | Implement `Clone` trait |
| `Copy` | Implement `Copy` trait (requires `Clone`) |
| `Debug` | Implement `Debug` trait for formatting |
| `Default` | Implement `Default` trait |
| `PartialEq` | Implement `PartialEq` for `==` and `!=` |
| `Eq` | Implement `Eq` (requires `PartialEq`) |
| `PartialOrd` | Implement `PartialOrd` for `<`, `>`, etc. |
| `Ord` | Implement `Ord` (requires `PartialOrd` + `Eq`) |
| `Hash` | Implement `Hash` for hashing |

```spl
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Point(x: i32, y: i32)

#[derive(Clone, Debug, Default)]
struct Config(
    name: String,
    timeout: u64,
    retries: u32,
)
```

### 4.2 Derive Requirements

Some derives have requirements:

```spl
// Copy requires Clone
#[derive(Clone, Copy)]  // OK
struct Small(x: i32)

#[derive(Copy)]  // ERROR: Copy requires Clone
struct Bad(x: i32)

// Eq requires PartialEq
#[derive(PartialEq, Eq)]  // OK
struct Id(value: u64)

// Ord requires PartialOrd and Eq
#[derive(PartialEq, Eq, PartialOrd, Ord)]  // OK
struct Priority(level: i32)
```

### 4.3 Derive on Generics

Derived traits often require bounds on type parameters:

```spl
#[derive(Clone, Debug)]
struct Wrapper(value: T) where T

// Generated impl:
// impl Clone for Wrapper(T: T) where T: Clone { ... }
// impl Debug for Wrapper(T: T) where T: Debug { ... }
```

### 4.4 Derive on Enums

```spl
#[derive(Clone, Debug, PartialEq)]
enum Status {
    Pending,
    Active(since: Timestamp),
    Completed(result: T),
} where T
```

---

## 5. Lint Attributes

### 5.1 Lint Levels

| Attribute | Effect |
|-----------|--------|
| `#[allow(...)]` | Silence the lint |
| `#[warn(...)]` | Emit warning (default for most) |
| `#[deny(...)]` | Emit error |
| `#[forbid(...)]` | Emit error, cannot be overridden |

```spl
#[allow(unused_variables)]
fn work(): () {
    let x = 42;  // No warning
}

#[deny(unsafe_code)]
mod safe_only {
    // Any unsafe code is an error
}

#![forbid(missing_docs)]  // Module-wide, cannot be allowed
```

### 5.2 Common Lints

| Lint | Description |
|------|-------------|
| `unused_variables` | Unused variable bindings |
| `unused_imports` | Unused imports |
| `dead_code` | Unreachable code |
| `unreachable_patterns` | Patterns that can never match |
| `unsafe_code` | Use of unsafe |
| `missing_docs` | Missing documentation |
| `non_camel_case_types` | Type names not in CamelCase |
| `non_snake_case` | Function names not in snake_case |

```spl
#[allow(non_snake_case)]
fn XMLParser(): Parser {
    // ...
}

#[allow(dead_code)]
fn future_feature(): () {
    // Not yet called
}
```

### 5.3 Lint Scoping

Lint attributes can be scoped to specific items:

```spl
fn outer(): () {
    #[allow(unused_variables)]
    let x = 42;

    let y = 42;  // Warning: unused
}
```

---

## 6. Attribute Targets

| Target | Applies To |
|--------|------------|
| Items | Functions, structs, enums, traits, impls, modules |
| Statements | Let bindings, expressions |
| Fields | Struct and enum fields |
| Variants | Enum variants |
| Type parameters | Generic parameters |

### 6.1 Field Attributes

```spl
struct Config(
    #[doc = "The application name"]
    name: String,

    #[serde(default)]  // For serde derive
    timeout: u64,
)
```

### 6.2 Variant Attributes

```spl
enum Command {
    #[doc = "Exit the application"]
    Quit,

    #[deprecated = "Use Move instead"]
    Walk(x: i32, y: i32),

    Move(dx: i32, dy: i32),
}
```

---

## 7. Tool Attributes

Attributes prefixed with a tool name are passed to that tool:

```spl
#[rustfmt::skip]
fn weird_formatting(): () {
    // rustfmt ignores this
}

#[clippy::cognitive_complexity = "50"]
fn complex_but_necessary(): () {
    // ...
}
```

---

## 8. Custom Attributes (Procedural Macros)

User-defined derive and attribute macros can be created:

```spl
// Using a custom derive
#[derive(Serialize, Deserialize)]
struct Message(id: u64, body: String)

// Using an attribute macro
#[route(GET, "/api/users")]
fn list_users(): Response {
    // ...
}
```

The implementation of procedural macros is beyond the scope of this specification.

---

## 9. Summary

| Category | Examples |
|----------|----------|
| Code generation | `#[inline]`, `#[cold]`, `#[no_mangle]` |
| Memory layout | `#[repr(C)]`, `#[repr(packed)]`, `#[repr(align(N))]` |
| FFI | `#[link(name = "...")]`, `#[link_name = "..."]` |
| Testing | `#[test]`, `#[ignore]`, `#[should_panic]` |
| Documentation | `#[doc = "..."]`, `#[doc(hidden)]` |
| Conditional | `#[cfg(...)]`, `#[cfg_attr(...)]` |
| Derives | `#[derive(Clone, Debug)]` |
| Lints | `#[allow(...)]`, `#[warn(...)]`, `#[deny(...)]` |

---

## References

- [syntax-grammar.md](syntax-grammar.md) - Attribute grammar
- [ffi.md](ffi.md) - FFI-related attributes
- [module-system.md](module-system.md) - Conditional compilation with modules
