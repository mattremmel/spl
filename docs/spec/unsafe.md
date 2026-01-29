# SPL Unsafe Operations and Raw Pointers

This document specifies unsafe operations in SPL, including raw pointer types, unsafe blocks, and unsafe functions.

## Overview

SPL is safe by default. The compiler enforces memory safety through ownership, borrowing, and type checking. However, some low-level operations cannot be verified by the compiler and require the programmer to take responsibility for correctness.

The `unsafe` keyword marks code that may violate memory safety invariants. Unsafe code is not inherently wrong—it means the compiler cannot verify its correctness, and the programmer must uphold the required invariants manually.

---

## 1. Unsafe Operations

The following operations require an `unsafe` context:

| Operation | Reason |
|-----------|--------|
| Reading from a raw pointer | Pointer may be null, dangling, or misaligned |
| Writing to a raw pointer | Pointer may be null, dangling, or misaligned |
| Calling an unsafe function | Function has preconditions the compiler cannot verify |
| Calling an extern function | FFI cannot guarantee memory safety |
| Reading or writing a mutable static | Data races possible without synchronization |
| Implementing an unsafe trait | Trait has invariants the compiler cannot verify |

**Note:** SPL does not have union types. C unions accessed via FFI should use `#[repr(C)]` structs with appropriate pointer casts and unsafe access patterns.

### What Remains Safe

These operations do NOT require unsafe:

- Creating raw pointers (only reading/writing is unsafe)
- Pointer arithmetic (produces a pointer, doesn't access memory)
- Comparing pointers
- Pointer-integer conversions
- Casting between pointer types

---

## 2. Unsafe Blocks

An unsafe block enables unsafe operations within its scope.

### Syntax

```
unsafe_block = "unsafe" block
block        = "{" statement* expression? "}"
```

### Example

```spl
use std.ptr.Ptr;

let x = 42;
let p: Ptr(i32) = (&x).as_ptr();

// Safe: creating and comparing pointers
let is_null = p.is_null();

// Unsafe: reading through pointer
let value = unsafe { p.read() };
```

### Semantics

- The unsafe block does not disable any compiler checks
- Only the specific unsafe operations listed above are permitted
- Safe code within an unsafe block is still checked normally
- Unsafe blocks can appear anywhere an expression is expected
- The block evaluates to its final expression (like regular blocks)

### Scoping

Unsafe applies only to the block, not to nested functions:

```spl
unsafe {
    let val = p.read();  // OK: in unsafe block

    fn inner() {
        // p.read() here would be ERROR: not in unsafe context
    }
}
```

---

## 3. Unsafe Functions

Functions that have preconditions the compiler cannot verify should be marked `unsafe`.

### Syntax

```
unsafe_fn = "unsafe" "fn" identifier "(" parameters ")" (":" type)? block
```

### Example

```spl
use std.ptr.Ptr;

/// Reads a value from the given pointer.
///
/// # Safety
/// - `p` must be valid and properly aligned
/// - `p` must point to an initialized value of type T
/// - The memory must not be mutated during this call
unsafe fn read_from(p: Ptr(T)): T where T {
    return p.read();
}
```

### Calling Unsafe Functions

Calling an unsafe function requires an unsafe context:

```spl
let p: Ptr(i32) = get_some_pointer();

// ERROR: call to unsafe function outside unsafe block
let val = read_from(p);

// OK: in unsafe block
let val = unsafe { read_from(p) };
```

### Documentation Convention

Unsafe functions should document their safety requirements in a `# Safety` section.

---

## 4. Raw Pointer Types

SPL provides two raw pointer types in the `std.ptr` module:

| Type | Description |
|------|-------------|
| `Ptr(T)` | Read-only pointer to a value of type T |
| `MutPtr(T)` | Mutable pointer to a value of type T |

Raw pointer types are **not in the prelude**. This signals that they are an escape hatch for low-level code, not everyday types. To use them:

```spl
use std.ptr.{Ptr, MutPtr};
```

### Properties

- Both types are sized (pointer-width: 8 bytes on 64-bit)
- Both types implement `Copy`
- Neither type implements `Drop`
- Raw pointers have no borrowing restrictions
- Multiple pointers to the same location can exist simultaneously
- Raw pointers may be null, dangling, or misaligned

### Creating Pointers

From references using `.as_ptr()` method (safe):

```spl
use std.ptr.{Ptr, MutPtr};

let x = 42;
let p: Ptr(i32) = (&x).as_ptr();           // From immutable reference

let mut y = 100;
let mp: MutPtr(i32) = (&mut y).as_mut_ptr();   // From mutable reference
```

**Note:** References do not implicitly coerce to raw pointers. Use the explicit `.as_ptr()` or `.as_mut_ptr()` methods to convert. This makes the conversion visible and intentional.

Null pointers:

```spl
use std.ptr;

let p: Ptr(i32) = ptr.null();
let mp: MutPtr(i32) = ptr.null_mut();
```

From address:

```spl
use std.ptr;

let p: Ptr(i32) = ptr.from_addr(0x1000);
let mp: MutPtr(i32) = ptr.from_addr_mut(0x1000);
```

### Reading and Writing

All memory access through pointers requires unsafe and uses explicit methods:

```spl
use std.ptr.{Ptr, MutPtr};

let x = 42;
let p: Ptr(i32) = (&x).as_ptr();

// Read through Ptr (unsafe)
let value = unsafe { p.read() };

// Ptr does NOT have write() - compile error
// unsafe { p.write(5) };  // ERROR: method not found

let mut y = 100;
let mp: MutPtr(i32) = (&mut y).as_mut_ptr();

// MutPtr can read AND write
let value = unsafe { mp.read() };
unsafe { mp.write(200) };
```

There is no `*p` dereference operator for raw pointers. This design:
- Makes intent explicit (reading vs writing)
- Enables compile-time checking (`Ptr` vs `MutPtr`)
- Keeps both types cleanly in a module (not primitives)

### Converting Between Pointer Types

```spl
use std.ptr.{Ptr, MutPtr};

let mut x = 42;
let mp: MutPtr(i32) = (&mut x).as_mut_ptr();

// MutPtr -> Ptr (safe, reduces capability)
let p: Ptr(i32) = mp.as_const();

// Ptr -> MutPtr (safe to create, unsafe to use for writing)
let mp2: MutPtr(i32) = p.as_mut();
```

### Ptr(T) Methods

| Method | Signature | Unsafe | Description |
|--------|-----------|--------|-------------|
| `read` | `fn read(&self): T` | Yes | Read value from pointer |
| `read_volatile` | `fn read_volatile(&self): T` | Yes | Volatile read |
| `is_null` | `fn is_null(&self): bool` | No | Check if null |
| `addr` | `fn addr(&self): usize` | No | Get address as integer |
| `cast` | `fn cast(U)(&self): Ptr(U)` | No | Cast to different type |
| `as_mut` | `fn as_mut(&self): MutPtr(T)` | No | Convert to MutPtr |
| `offset` | `fn offset(&self, count: isize): Ptr(T)` | No | Offset by elements |
| `add` | `fn add(&self, count: usize): Ptr(T)` | No | Offset forward |
| `sub` | `fn sub(&self, count: usize): Ptr(T)` | No | Offset backward |
| `byte_add` | `fn byte_add(&self, bytes: usize): Ptr(T)` | No | Offset by bytes |
| `byte_sub` | `fn byte_sub(&self, bytes: usize): Ptr(T)` | No | Offset backward by bytes |

### MutPtr(T) Methods

`MutPtr(T)` has all methods from `Ptr(T)` plus:

| Method | Signature | Unsafe | Description |
|--------|-----------|--------|-------------|
| `write` | `fn write(&self, val: T)` | Yes | Write value to pointer |
| `write_volatile` | `fn write_volatile(&self, val: T)` | Yes | Volatile write |
| `as_const` | `fn as_const(&self): Ptr(T)` | No | Convert to Ptr |

Arithmetic methods on `MutPtr(T)` return `MutPtr(T)`:

| Method | Signature |
|--------|-----------|
| `cast` | `fn cast(U)(&self): MutPtr(U)` |
| `offset` | `fn offset(&self, count: isize): MutPtr(T)` |
| `add` | `fn add(&self, count: usize): MutPtr(T)` |
| `sub` | `fn sub(&self, count: usize): MutPtr(T)` |

---

## 5. Pointer Arithmetic

Pointer arithmetic is safe (it doesn't access memory) but can produce invalid pointers:

```spl
use std.ptr.Ptr;

let arr = [1, 2, 3, 4, 5];
let p: Ptr(i32) = arr.as_ptr();  // Get pointer to first element

// Offset by element count (not bytes)
let second = p.add(1);    // Points to arr[1]
let fourth = p.add(3);    // Points to arr[3]
let back = fourth.sub(2); // Points to arr[1]

// Unsafe to read if out of bounds
let value = unsafe { second.read() };
```

---

## 6. Pointer Comparison

Pointer comparison is safe:

```spl
use std.ptr.Ptr;

let x = 1;
let y = 2;
let a: Ptr(i32) = (&x).as_ptr();
let b: Ptr(i32) = (&y).as_ptr();

if a == b { }      // Equality
if a != b { }      // Inequality
if a < b { }       // Address comparison
if a.is_null() { } // Null check
```

---

## 7. Pointer-Integer Conversion

Pointers can be converted to and from integers:

```spl
use std.ptr.{Ptr, ptr};

let x = 42;
let p: Ptr(i32) = (&x).as_ptr();

// Pointer to integer
let addr: usize = p.addr();

// Integer to pointer
let p2: Ptr(i32) = ptr.from_addr(addr);
```

---

## 8. The `std.ptr` Module

The `std.ptr` module provides the pointer types and helper functions:

### Types

- `Ptr(T)` - Read-only raw pointer
- `MutPtr(T)` - Mutable raw pointer

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `null` | `fn null(T)(): Ptr(T)` | Create null Ptr |
| `null_mut` | `fn null_mut(T)(): MutPtr(T)` | Create null MutPtr |
| `from_addr` | `fn from_addr(T)(addr: usize): Ptr(T)` | Ptr from address |
| `from_addr_mut` | `fn from_addr_mut(T)(addr: usize): MutPtr(T)` | MutPtr from address |
| `copy` | `unsafe fn copy(T)(src: Ptr(T), dst: MutPtr(T), count: usize)` | Copy (may overlap) |
| `copy_nonoverlapping` | `unsafe fn copy_nonoverlapping(T)(...)` | Copy (must not overlap) |
| `write_bytes` | `unsafe fn write_bytes(T)(dst: MutPtr(T), val: u8, count: usize)` | Fill with byte |
| `swap` | `unsafe fn swap(T)(a: MutPtr(T), b: MutPtr(T))` | Swap values |
| `replace` | `unsafe fn replace(T)(dst: MutPtr(T), val: T): T` | Replace, return old |

---

## 9. Extern Functions (FFI)

All extern function calls are implicitly unsafe because the compiler cannot verify foreign code.

### Declaring Extern Functions

```spl
use std.ptr.MutPtr;

extern "C" {
    fn malloc(size: usize): MutPtr(u8);
    fn free(p: MutPtr(u8));
    fn strlen(s: Ptr(u8)): usize;
}
```

### Calling Extern Functions

```spl
let p = unsafe { malloc(1024) };
if !p.is_null() {
    // Use the memory...
    unsafe { free(p) };
}
```

### Panic at FFI Boundary

Per SPL's memory model, panics abort at FFI boundaries rather than unwinding into foreign code. This prevents undefined behavior from unwinding through non-SPL stack frames.

---

## 10. Mutable Statics

Static variables with interior mutability require unsafe to access:

```spl
static mut COUNTER: i32 = 0;

fn increment() {
    unsafe {
        COUNTER += 1;
    }
}
```

Accessing mutable statics is unsafe because:
- Multiple threads could access simultaneously (data race)
- No borrow checking across the program

**Recommendation:** Prefer thread-safe types (`Mutex`, `Atomic*`) over mutable statics.

---

## 11. Unsafe Traits

Some traits have invariants that the compiler cannot verify. These are marked `unsafe trait` and require `unsafe impl`:

```spl
/// Marker for types that can be safely shared between threads.
///
/// # Safety
/// Implementors must ensure that shared access from multiple threads
/// cannot cause data races or memory unsafety.
unsafe trait Sync { }

/// Implementing unsafe trait requires unsafe impl
unsafe impl Sync for MyType { }
```

### Standard Unsafe Traits

| Trait | Invariant |
|-------|-----------|
| `Send` | Safe to transfer between threads |
| `Sync` | Safe to share references between threads |

---

## 12. Unsafe and Closures

Closures can be marked unsafe:

```spl
use std.ptr.Ptr;

let p: Ptr(i32) = get_pointer();

let read_it = unsafe || {
    p.read()
};

// Calling requires unsafe
let value = unsafe { read_it() };
```

---

## 13. Building Safe Abstractions

The purpose of unsafe is to build safe abstractions. A well-designed API uses unsafe internally but exposes a safe interface:

```spl
use std.ptr.{MutPtr, ptr};

struct Vec(T)(
    ptr: MutPtr(T),
    len: usize,
    capacity: usize,
) where T

impl Vec(T) where T {
    // Safe public API
    pub fn push(&mut self, value: T) {
        if self.len == self.capacity {
            self.grow();
        }
        // Unsafe implementation detail
        unsafe {
            self.ptr.add(self.len).write(value);
        }
        self.len += 1;
    }

    // Safe indexing with bounds check
    pub fn get(&self, index: usize): T? {
        if index >= self.len {
            return None;
        }
        // Unsafe implementation detail
        return Some(unsafe { self.ptr.add(index).read() });
    }
}
```

### The Unsafe Contract

When writing unsafe code:

1. **Document invariants**: What must be true for this code to be safe?
2. **Validate inputs**: Check what you can at the boundary
3. **Minimize scope**: Keep unsafe blocks as small as possible
4. **Encapsulate**: Hide unsafe behind safe APIs when possible

---

## Summary

| Feature | Syntax | Requires Unsafe |
|---------|--------|-----------------|
| Pointer types | `Ptr(T)`, `MutPtr(T)` | No (just types) |
| Import pointers | `use std.ptr.{Ptr, MutPtr}` | No |
| Create from ref | `(&x).as_ptr()` | No |
| Pointer arithmetic | `p.add(n)` | No |
| Pointer comparison | `p == q` | No |
| Pointer to int | `p.addr()` | No |
| Int to pointer | `ptr.from_addr(n)` | No |
| Null check | `p.is_null()` | No |
| Cast | `p.cast(U)` | No |
| Ptr → MutPtr | `p.as_mut()` | No |
| MutPtr → Ptr | `mp.as_const()` | No |
| Read from pointer | `p.read()` | **Yes** |
| Write to pointer | `mp.write(val)` | **Yes** |
| Call unsafe fn | `unsafe_fn()` | **Yes** |
| Call extern fn | `extern_fn()` | **Yes** |
| Access mutable static | `STATIC` | **Yes** |
| Implement unsafe trait | `unsafe impl` | **Yes** |

---

## Examples

### Low-Level Memory Copy

```spl
use std.ptr.{Ptr, MutPtr};

/// Copy `count` elements from `src` to `dst`.
///
/// # Safety
/// - `src` must be valid for reads of `count` elements
/// - `dst` must be valid for writes of `count` elements
/// - `src` and `dst` must not overlap
unsafe fn copy(T)(dst: MutPtr(T), src: Ptr(T), count: usize) where T {
    let mut i: usize = 0;
    while i < count {
        dst.add(i).write(src.add(i).read());
        i += 1;
    }
}
```

### Wrapping a C Library

```spl
use std.ptr.{Ptr, MutPtr};

extern "C" {
    fn c_create_handle(): MutPtr(Handle);
    fn c_destroy_handle(h: MutPtr(Handle));
    fn c_process(h: MutPtr(Handle), data: Ptr(u8), len: usize): i32;
}

/// Safe wrapper around C handle.
pub struct SafeHandle(
    inner: MutPtr(Handle),
)

impl SafeHandle {
    pub fn new(): SafeHandle? {
        let p = unsafe { c_create_handle() };
        if p.is_null() {
            return None;
        }
        return Some(SafeHandle(inner: p));
    }

    pub fn process(&mut self, data: &[u8]): i32 {
        return unsafe {
            c_process(self.inner, data.as_ptr(), data.len())
        };
    }
}

impl Drop for SafeHandle {
    fn drop(&mut self) {
        unsafe { c_destroy_handle(self.inner) };
    }
}
```

### Implementing a Simple Box

```spl
use std.ptr.{MutPtr, ptr};

extern "C" {
    fn malloc(size: usize): MutPtr(u8);
    fn free(p: MutPtr(u8));
}

/// Heap-allocated value.
pub struct Box(T)(
    ptr: MutPtr(T),
) where T

impl Box(T) where T {
    pub fn new(value: T): Box(T) {
        let p: MutPtr(T) = unsafe { malloc(size_of(T)) }.cast(T);
        if p.is_null() {
            panic("allocation failed");
        }
        unsafe { p.write(value) };
        return Box(ptr: p);
    }
}

impl Drop for Box(T) where T {
    fn drop(&mut self) {
        unsafe {
            // Read to run destructor, then free
            let _ = self.ptr.read();
            free(self.ptr.cast(u8));
        };
    }
}
```
