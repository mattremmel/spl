# SPL Foreign Function Interface (FFI)

This document specifies how SPL code interoperates with foreign code, primarily C.

## Overview

SPL provides FFI capabilities for:
- Calling functions in native libraries
- Exposing SPL functions to native code
- Sharing data structures across language boundaries

All FFI operations are inherently unsafe because the compiler cannot verify foreign code. Calling extern functions requires an `unsafe` block (see [unsafe.md](unsafe.md)).

---

## 1. Extern Blocks

Extern blocks declare functions implemented in foreign code.

### Syntax

```
extern_block = "#[" link_attr "]"* "extern" abi_string "{" extern_fn* "}"
abi_string   = "\"C\""
extern_fn    = "fn" identifier "(" param_list ("," "...")? ")" (":" type)? ";"
```

### Example

```spl
extern "C" {
    fn malloc(size: usize): MutPtr(T: u8);
    fn free(ptr: MutPtr(T: u8));
    fn strlen(s: Ptr(T: u8)): usize;
}
```

### Calling Extern Functions

All extern function calls require an unsafe context:

```spl
let p = unsafe { malloc(1024) };
if !p.is_null() {
    // use the memory
    unsafe { free(p) };
}
```

### ABI

SPL currently supports only the `"C"` ABI, which uses the platform's native C calling convention:

| Platform | Convention |
|----------|------------|
| x86-64 Linux/macOS/BSD | System V AMD64 ABI |
| x86-64 Windows | Microsoft x64 |
| AArch64 Linux/macOS | AAPCS64 |
| AArch64 Windows | ARM64 Windows ABI |

**Future:** Additional ABIs (`"C-unwind"`, `"system"`, `"stdcall"`) may be added if needed.

---

## 2. Variadic Functions

C functions like `printf` accept a variable number of arguments.

### Syntax

```spl
extern "C" {
    fn printf(fmt: Ptr(T: u8), ...): i32;
}
```

### Rules

| Rule | Rationale |
|------|-----------|
| `...` must be the last parameter | Matches C |
| At least one fixed parameter required | C requirement; needed to locate varargs |
| Only allowed in extern blocks | SPL functions cannot be variadic |
| No compile-time type checking of varargs | Matches C behavior |

### Type Promotion

Variadic arguments are promoted according to C rules:

| SPL Type | Promoted To |
|----------|-------------|
| `i8`, `i16` | `i32` |
| `u8`, `u16` | `u32` |
| `f32` | `f64` |
| pointers | unchanged |
| `i32`, `i64`, etc. | unchanged |

### Example

```spl
use std.ffi.{c_int};

extern "C" {
    fn printf(fmt: Ptr(T: u8), ...): c_int;
}

unsafe {
    printf(c"Hello %s, you are %d years old\n".as_ptr(), name.as_ptr(), age);
}
```

---

## 3. Type Mapping

### Primitive Types

| C Type | SPL Type | Size (bytes) |
|--------|----------|--------------|
| `char` | `i8` | 1 |
| `signed char` | `i8` | 1 |
| `unsigned char` | `u8` | 1 |
| `short` | `i16` | 2 |
| `unsigned short` | `u16` | 2 |
| `int` | `i32` | 4 |
| `unsigned int` | `u32` | 4 |
| `long` | `c_long` / `isize` | 4 or 8 |
| `unsigned long` | `c_ulong` / `usize` | 4 or 8 |
| `long long` | `i64` | 8 |
| `unsigned long long` | `u64` | 8 |
| `size_t` | `usize` | 4 or 8 |
| `ssize_t` / `ptrdiff_t` | `isize` | 4 or 8 |
| `float` | `f32` | 4 |
| `double` | `f64` | 8 |
| `void*` | `MutPtr(T: u8)` | 4 or 8 |
| `const void*` | `Ptr(T: u8)` | 4 or 8 |
| `T*` | `MutPtr(T: T)` | 4 or 8 |
| `const T*` | `Ptr(T: T)` | 4 or 8 |

### The `std.ffi` Module

The `std.ffi` module provides platform-correct C type aliases:

```spl
// std.ffi
pub type c_char = i8;       // Note: may be u8 on some platforms
pub type c_schar = i8;
pub type c_uchar = u8;
pub type c_short = i16;
pub type c_ushort = u16;
pub type c_int = i32;
pub type c_uint = u32;
pub type c_long = isize;    // 4 bytes on Windows, 8 on Unix
pub type c_ulong = usize;
pub type c_longlong = i64;
pub type c_ulonglong = u64;
pub type c_float = f32;
pub type c_double = f64;
pub type c_void = ();
```

**Important:** Use `c_long`/`c_ulong` instead of `isize`/`usize` when interfacing with C code that uses `long`, as the size differs between platforms (4 bytes on Windows LLP64, 8 bytes on Unix LP64).

### C-Compatible Struct Layout

Structs passed across FFI boundaries must use `#[repr(C)]`:

```spl
#[repr(C)]
struct Point(
    x: f64,
    y: f64,
)

extern "C" {
    fn distance(a: Ptr(T: Point), b: Ptr(T: Point)): f64;
}
```

**`#[repr(C)]` guarantees:**
- Fields laid out in declaration order
- Alignment and padding match C compiler
- No field reordering or layout optimization

**Without `#[repr(C)]`:** SPL may reorder fields, use different padding, or apply other optimizations.

### C-Compatible Enums

```spl
#[repr(C)]
enum Color{
    Red = 0,
    Green = 1,
    Blue = 2,
}
```

### Packed Structs

For structs with no padding (matching C's `__attribute__((packed))`):

```spl
#[repr(C, packed)]
struct PackedData(
    a: u8,
    b: u32,  // No padding before this
)
```

**Warning:** Packed structs may require unaligned memory access, which has performance implications and may not be supported on all architectures.

---

## 4. String Handling

C strings are null-terminated byte arrays with no encoding guarantee. SPL provides types for safe interoperation.

### CStr - C String Reference

`CStr` is a lightweight handle to a null-terminated C string. It wraps a pointer and does not own the memory. `CStr` is `Copy` since it's just a pointer internally.

```spl
#[derive(Copy, Clone)]
pub struct CStr(
    ptr: Ptr(T: u8),
)

impl CStr {
    /// Wrap a pointer to a C string.
    ///
    /// # Safety
    /// - `ptr` must point to a valid null-terminated string
    /// - The memory must remain valid while the CStr is used
    pub unsafe fn from_ptr(ptr: Ptr(T: u8)): CStr;

    /// Get as raw bytes (excluding null terminator)
    pub fn as_bytes(&self): &[u8];

    /// Try to interpret as UTF-8
    pub fn to_str(&self): Result(&str, Utf8Error);

    /// Interpret as UTF-8, replacing invalid sequences with U+FFFD
    pub fn to_str_lossy(&self): String;

    /// Length in bytes (not including null terminator)
    pub fn len(&self): usize;

    /// Raw pointer for FFI
    pub fn as_ptr(&self): Ptr(T: u8);
}
```

### CString - Owned C String

`CString` is an owned null-terminated string allocated by SPL.

```spl
pub struct CString(
    ptr: MutPtr(T: u8),
    len: usize,
)

impl CString {
    /// Create from SPL string (adds null terminator).
    /// Returns error if string contains interior null bytes.
    pub fn new(s: &str): Result(T: CString, E: NulError);

    /// Get as CStr (shares the same pointer)
    pub fn as_cstr(&self): CStr;

    /// Get raw pointer for FFI
    pub fn as_ptr(&self): Ptr(T: u8);

    /// Consume and return raw pointer.
    /// Caller is responsible for freeing the memory.
    pub fn into_raw(self): MutPtr(T: u8);

    /// Reconstruct from raw pointer.
    ///
    /// # Safety
    /// - `ptr` must have come from `into_raw()`
    /// - Must not have been freed
    pub unsafe fn from_raw(ptr: MutPtr(T: u8)): CString;
}

impl Drop for CString {
    fn drop(&mut self);  // Frees the buffer
}
```

### C String Literals

The `c"..."` literal syntax creates a `CStr` pointing to static memory:

```spl
let s: CStr = c"Hello, world!";
```

- Compile-time null-terminated
- Points to static memory (valid for program lifetime)
- Guaranteed valid UTF-8 (but API treats as bytes for consistency)
- Returns `CStr` directly (not a reference) since `CStr` is `Copy`

### Encoding Philosophy

C strings are treated as **opaque bytes**, not UTF-8:

```spl
let c_str: CStr = unsafe { CStr.from_ptr(some_c_function()) };

// Must explicitly convert to SPL string
let bytes: &[u8] = c_str.as_bytes();           // Always works
let text: &str = c_str.to_str()?;              // Fails if not UTF-8
let text: String = c_str.to_str_lossy();       // Replaces invalid bytes
```

This design:
- Prevents silent data corruption with non-UTF-8 data
- Forces explicit handling of encoding
- Works correctly with legacy APIs (Windows code pages, etc.)

### Example

```spl
use std.ffi.{CStr, CString};
use std.ptr.Ptr;

extern "C" {
    fn puts(s: Ptr(T: u8)): i32;
    fn getenv(name: Ptr(T: u8)): Ptr(T: u8);
}

fn main() {
    // SPL string -> C (using literal)
    unsafe { puts(c"Hello from SPL!".as_ptr()) };

    // SPL string -> C (dynamic)
    let msg = CString.new("Dynamic string").unwrap();
    unsafe { puts(msg.as_ptr()) };

    // C string -> SPL
    let home = unsafe {
        let ptr = getenv(c"HOME".as_ptr());
        if ptr.is_null() {
            None
        } else {
            Some(CStr.from_ptr(ptr).to_str_lossy())
        }
    };
}
```

---

## 5. Callbacks (SPL Functions Callable from C)

### Exporting Functions

Use `extern "C" fn` to define functions callable from C:

```spl
extern "C" fn compare(a: Ptr(T: i32), b: Ptr(T: i32)): i32 {
    let a_val = unsafe { a.read() };
    let b_val = unsafe { b.read() };
    return a_val - b_val;
}
```

**Rules for `extern "C" fn`:**
- Uses C calling convention
- Can be passed as function pointer to C
- Signature must use C-compatible types only (no generics, no SPL strings)
- Body can use any SPL features
- Panics abort at FFI boundary (see [unsafe.md](unsafe.md))

### The `#[no_mangle]` Attribute

By default, SPL mangles function names to include type information. Use `#[no_mangle]` to preserve the exact function name for FFI:

```spl
// Without no_mangle: symbol might be "_ZN7mylib7my_funcEv" or similar
extern "C" fn my_func(): i32 { return 42; }

// With no_mangle: symbol is exactly "my_func"
#[no_mangle]
extern "C" fn my_func(): i32 { return 42; }
```

**When to use `#[no_mangle]`:**
- Functions called by name from C code
- Plugin entry points
- Shared library exports
- Any function where C code uses the literal name

**When NOT needed:**
- Functions only passed as function pointers (name doesn't matter)
- Internal callbacks within SPL code

### Building Shared Libraries

To build SPL code as a shared library callable from C:

**1. Create a library package:**

```spl
// lib.spl
use std.ptr.Ptr;

#[no_mangle]
pub extern "C" fn mylib_init(): i32 {
    // Initialization code
    return 0;
}

#[no_mangle]
pub extern "C" fn mylib_process(data: Ptr(T: u8), len: usize): i32 {
    // Process data
    return 0;
}

#[no_mangle]
pub extern "C" fn mylib_cleanup() {
    // Cleanup code
}
```

**2. Build as a shared library:**

```bash
splc --crate-type cdylib -o libmylib.so lib.spl
```

**3. Create a C header (manually or with a tool):**

```c
// mylib.h
#ifndef MYLIB_H
#define MYLIB_H

#include <stdint.h>
#include <stddef.h>

int32_t mylib_init(void);
int32_t mylib_process(const uint8_t* data, size_t len);
void mylib_cleanup(void);

#endif
```

**4. Use from C:**

```c
#include "mylib.h"

int main() {
    mylib_init();
    uint8_t data[] = {1, 2, 3};
    mylib_process(data, sizeof(data));
    mylib_cleanup();
    return 0;
}
```

**Crate Types:**

| Type | Output | Use Case |
|------|--------|----------|
| `bin` | Executable | Standalone programs |
| `lib` | SPL library | SPL-to-SPL dependencies |
| `cdylib` | Shared library (.so/.dylib/.dll) | C/FFI consumption |
| `staticlib` | Static library (.a/.lib) | Static linking from C |

### Function Pointer Types

```spl
// Declare a C function pointer type
type Comparator = extern "C" fn(Ptr(T: i32), Ptr(T: i32)): i32;

extern "C" {
    fn qsort(
        base: MutPtr(T: u8),
        count: usize,
        size: usize,
        cmp: Comparator,
    );
}

// Usage
extern "C" fn my_compare(a: Ptr(T: i32), b: Ptr(T: i32)): i32 {
    unsafe { a.read() - b.read() }
}

fn sort_ints(arr: &mut [i32]) {
    unsafe {
        qsort(
            arr.as_mut_ptr().cast(u8),
            arr.len(),
            size_of(i32),
            my_compare,
        );
    }
}
```

### Closures Cannot Cross FFI

Closures capture their environment and have unknown size. C function pointers are just addresses with no environment.

```spl
let multiplier = 2;
let closure = |x| x * multiplier;  // Captures `multiplier`

// ERROR: Cannot pass closure as C function pointer
// qsort(..., closure);
```

**Workaround: Context Pointer Pattern**

Many C APIs provide a `void* user_data` parameter for this purpose:

```spl
extern "C" {
    fn register_callback(
        cb: extern "C" fn(i32, MutPtr(T: u8)),
        user_data: MutPtr(T: u8),
    );
}

#[repr(C)]
struct Context(
    multiplier: i32,
)

extern "C" fn callback(value: i32, user_data: MutPtr(T: u8)) {
    let ctx = unsafe { user_data.cast(T: Context).read() };
    println(value * ctx.multiplier);
}

fn main() {
    let ctx = Context(multiplier: 2);
    unsafe {
        register_callback(callback, (&ctx).cast(u8));
    }
}
```

**Future:** A `std.ffi.Closure` helper for wrapping closures may be added in a future version.

---

## 6. Linking

### The `#[link]` Attribute

Specify native libraries to link against:

```spl
#[link(name = "sqlite3")]
extern "C" {
    fn sqlite3_open(filename: Ptr(T: u8), db: MutPtr(T: MutPtr(T: Sqlite3))): i32;
    fn sqlite3_close(db: MutPtr(T: Sqlite3)): i32;
}
```

### Attribute Options

| Option | Required | Values | Description |
|--------|----------|--------|-------------|
| `name` | Yes | string | Library name (without prefix/suffix) |
| `kind` | No | `"dylib"`, `"static"`, `"framework"` | Link type (default: `"dylib"`) |

### Examples

```spl
// Dynamic library (default)
// Links: libfoo.so (Linux), libfoo.dylib (macOS), foo.dll (Windows)
#[link(name = "foo")]
extern "C" { }

// Static library
// Links: libfoo.a (Unix), foo.lib (Windows)
#[link(name = "foo", kind = "static")]
extern "C" { }

// macOS framework
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" { }
```

### Multiple Libraries

```spl
#[link(name = "ssl")]
#[link(name = "crypto")]
extern "C" {
    fn SSL_new(ctx: MutPtr(T: SslCtx)): MutPtr(T: Ssl);
}
```

### Implicit Linking

The C standard library is implicitly linked. No `#[link]` attribute is needed for:

```spl
// libc functions - no #[link] required
extern "C" {
    fn malloc(size: usize): MutPtr(T: u8);
    fn free(ptr: MutPtr(T: u8));
    fn memcpy(dst: MutPtr(T: u8), src: Ptr(T: u8), n: usize): MutPtr(T: u8);
}
```

### Platform Library Naming

The compiler handles platform-specific library naming:

| `name` | Linux | macOS | Windows |
|--------|-------|-------|---------|
| `"foo"` (dylib) | `libfoo.so` | `libfoo.dylib` | `foo.dll` |
| `"foo"` (static) | `libfoo.a` | `libfoo.a` | `foo.lib` |

---

## 7. Platform Considerations

### Data Type Sizes

| Type | LP64 (Linux/macOS 64-bit) | LLP64 (Windows 64-bit) |
|------|---------------------------|------------------------|
| `c_long` | 8 bytes | 4 bytes |
| `c_ulong` | 8 bytes | 4 bytes |
| `c_longlong` | 8 bytes | 8 bytes |
| pointer | 8 bytes | 8 bytes |

**Key difference:** `long` is 4 bytes on Windows, 8 bytes on Unix. Always use `c_long` from `std.ffi` for portability.

### Alignment

Types in `#[repr(C)]` structs follow C alignment rules:

| Type | Alignment (bytes) |
|------|-------------------|
| `i8`, `u8` | 1 |
| `i16`, `u16` | 2 |
| `i32`, `u32`, `f32` | 4 |
| `i64`, `u64`, `f64` | 8 |
| pointers | 8 (64-bit) / 4 (32-bit) |
| structs | max alignment of fields |

### Conditional Compilation

Use `#[cfg(...)]` for platform-specific code:

```spl
#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "C" {
    fn GetLastError(): u32;
}

#[cfg(target_os = "linux")]
extern "C" {
    fn __errno_location(): MutPtr(T: i32);
}

#[cfg(target_family = "unix")]
extern "C" {
    fn fork(): i32;
}
```

### Available Predicates

| Predicate | Example Values |
|-----------|----------------|
| `target_os` | `"linux"`, `"macos"`, `"windows"`, `"ios"`, `"android"` |
| `target_arch` | `"x86_64"`, `"aarch64"`, `"x86"`, `"arm"` |
| `target_family` | `"unix"`, `"windows"` |

---

## 8. FFI Safety Guidelines

### The Unsafe Contract

When writing FFI code:

1. **Document invariants** - What must be true for the code to be safe?
2. **Validate at boundaries** - Check what you can (null pointers, array bounds)
3. **Minimize unsafe scope** - Keep unsafe blocks as small as possible
4. **Encapsulate** - Wrap unsafe FFI in safe public APIs

### Example: Safe Wrapper

```spl
use std.ffi.{CStr, CString};
use std.ptr.{Ptr, MutPtr};

// Raw C bindings (private)
#[link(name = "mylib")]
extern "C" {
    fn mylib_create(): MutPtr(T: Handle);
    fn mylib_destroy(h: MutPtr(T: Handle));
    fn mylib_process(h: MutPtr(T: Handle), data: Ptr(T: u8), len: usize): i32;
}

// Safe public API
pub struct MyLib(
    handle: MutPtr(T: Handle),
)

impl MyLib {
    pub fn new(): Option(T: MyLib) {
        let h = unsafe { mylib_create() };
        if h.is_null() {
            None
        } else {
            Some(MyLib(handle: h))
        }
    }

    pub fn process(&mut self, data: &[u8]): Result(T: (), E: Error) {
        let result = unsafe {
            mylib_process(self.handle, data.as_ptr(), data.len())
        };
        if result == 0 {
            Ok(())
        } else {
            Err(Error.from_code(result))
        }
    }
}

impl Drop for MyLib {
    fn drop(&mut self) {
        unsafe { mylib_destroy(self.handle) };
    }
}
```

### Portability Checklist

1. Use `std.ffi` types (`c_long`, `c_int`) for C interop, not raw `i32`/`i64`
2. Use `#[repr(C)]` on all structs passed to/from C
3. Use `#[cfg(...)]` for platform-specific bindings
4. Be aware `long` differs between Windows and Unix
5. Avoid `#[repr(C, packed)]` unless necessary (performance cost)
6. Handle string encoding explicitly (don't assume UTF-8 from C)

---

## Summary

| Feature | Syntax/Type |
|---------|-------------|
| Declare foreign function | `extern "C" { fn name(...): T; }` |
| Variadic function | `extern "C" { fn printf(fmt: Ptr(T: u8), ...): i32; }` |
| Call foreign function | `unsafe { malloc(1024) }` |
| C-compatible struct | `#[repr(C)] struct Name()` |
| C-compatible enum | `#[repr(C)] enum Name { }` |
| Export function to C | `extern "C" fn name(...): T { }` |
| Function pointer type | `extern "C" fn(...): T` |
| Link native library | `#[link(name = "foo")]` |
| Link static library | `#[link(name = "foo", kind = "static")]` |
| C string handle (non-owning, Copy) | `CStr` |
| Owned C string | `CString` |
| C string literal | `c"hello"` |
| Platform conditional | `#[cfg(target_os = "linux")]` |

---

## Future Work

The following features may be added in future versions:

- **Additional ABIs:** `"C-unwind"` for callbacks that may panic, `"system"` for Windows API
- **Closure helper:** `std.ffi.Closure` for wrapping closures as C callbacks with context pointers
- **Raw link names:** `#[link(name = "...", raw = true)]` for non-standard library names
