# SPL Type System

This document defines the type system of SPL (Simple Programming Language). SPL uses a static, nominal type system with type inference, inspired by Rust.

## Overview

SPL's type system provides:

- **Static typing**: All types are known at compile time
- **Nominal typing**: Types are distinguished by name, not structure
- **Type inference**: Types can often be inferred from context
- **Generics**: Parametric polymorphism with monomorphization
- **No implicit coercions**: Numeric conversions require explicit casts

---

## 1. Primitive Types

### Integer Types

SPL provides signed and unsigned integers of various sizes. All integers use two's complement representation for signed types.

| Type   | Size (bytes) | Range |
|--------|--------------|-------|
| `i8`   | 1 | -128 to 127 |
| `i16`  | 2 | -32,768 to 32,767 |
| `i32`  | 4 | -2,147,483,648 to 2,147,483,647 |
| `i64`  | 8 | -9,223,372,036,854,775,808 to 9,223,372,036,854,775,807 |
| `i128` | 16 | -2^127 to 2^127 - 1 |
| `u8`   | 1 | 0 to 255 |
| `u16`  | 2 | 0 to 65,535 |
| `u32`  | 4 | 0 to 4,294,967,295 |
| `u64`  | 8 | 0 to 18,446,744,073,709,551,615 |
| `u128` | 16 | 0 to 2^128 - 1 |

**Default integer type**: Integer literals without a suffix default to `i32`.

```spl
let x = 42;       // x: i32
let y = 42i64;    // y: i64
let z = 255u8;    // z: u8
```

### Floating-Point Types

SPL provides IEEE 754 floating-point types.

| Type  | Size (bytes) | Precision | Range |
|-------|--------------|-----------|-------|
| `f32` | 4 | ~6-7 decimal digits | ~1.2e-38 to ~3.4e38 |
| `f64` | 8 | ~15-16 decimal digits | ~2.2e-308 to ~1.8e308 |

**Default float type**: Float literals without a suffix default to `f64`.

```spl
let pi = 3.14159;      // pi: f64
let e = 2.71828f32;    // e: f32
let sci = 1.5e10;      // sci: f64
```

### Boolean Type

The `bool` type represents a boolean value.

| Type   | Size (bytes) | Values |
|--------|--------------|--------|
| `bool` | 1 | `true`, `false` |

```spl
let flag: bool = true;
let done = false;      // done: bool (inferred)
```

### Character Type

The `char` type represents a Unicode scalar value (any Unicode code point except surrogates).

| Type   | Size (bytes) | Range |
|--------|--------------|-------|
| `char` | 4 | U+0000 to U+D7FF and U+E000 to U+10FFFF |

```spl
let letter = 'a';      // letter: char
let emoji = '🦀';      // emoji: char
let newline = '\n';    // newline: char
```

### Unit Type

The unit type `()` represents the absence of a meaningful value. It has exactly one value, also written `()`.

| Type | Size (bytes) | Values |
|------|--------------|--------|
| `()` | 0 | `()` |

Functions that don't return a value implicitly return `()`.

```spl
fn print_hello() {
    // Implicitly returns ()
}

fn explicit_unit() -> () {
    ()
}
```

### Never Type

The never type `!` represents computations that never complete. This type has no values and is used for diverging functions.

| Type | Size | Values |
|------|------|--------|
| `!`  | N/A  | none   |

A function returning `!` never returns normally (it panics, loops forever, or exits the program).

```spl
fn panic(msg: &str) -> ! {
    // Terminates the program
}

fn infinite() -> ! {
    loop { }
}
```

The never type coerces to any other type, enabling code like:

```spl
let x: i32 = if condition {
    42
} else {
    panic("unreachable")  // ! coerces to i32
};
```

---

## 2. String Types

### String Slice (`str`)

The `str` type is an unsized type representing a sequence of UTF-8 encoded bytes. It cannot be used directly and must always appear behind a reference.

| Type   | Sized | Description |
|--------|-------|-------------|
| `str`  | No    | UTF-8 string slice |
| `&str` | Yes   | Immutable reference to string data |

```spl
let greeting: &str = "Hello, world!";
let name: &str = "SPL";
```

### Owned String (`String`)

The `String` type is a heap-allocated, growable UTF-8 string. Unlike Rust, `String` is a builtin type in SPL rather than a standard library type.

| Type     | Sized | Description |
|----------|-------|-------------|
| `String` | Yes   | Owned, heap-allocated UTF-8 string |

```spl
let mut s = String::from("Hello");
s.push_str(", world!");
```

---

## 3. Compound Types

### Arrays

Arrays are fixed-size, contiguous sequences of elements of the same type. The size is part of the type.

**Syntax**: `[T; N]` where `T` is the element type and `N` is the length (a compile-time constant).

| Type       | Description |
|------------|-------------|
| `[T; N]`   | Array of N elements of type T |
| `[i32; 5]` | Array of 5 32-bit integers |

```spl
let arr: [i32; 3] = [1, 2, 3];
let zeros: [u8; 100] = [0; 100];  // Repeat syntax
let first = arr[0];               // Indexing
```

Arrays are stack-allocated and have a known size at compile time.

### Slices

Slices are dynamically-sized views into a contiguous sequence. Like `str`, slices are unsized and must appear behind a reference.

**Syntax**: `[T]` for the unsized slice type, `&[T]` for a slice reference.

| Type    | Sized | Description |
|---------|-------|-------------|
| `[T]`   | No    | Slice of elements of type T |
| `&[T]`  | Yes   | Reference to a slice |
| `&mut [T]` | Yes | Mutable reference to a slice |

```spl
let arr = [1, 2, 3, 4, 5];
let slice: &[i32] = arr[1:4];    // [2, 3, 4]
let full: &[i32] = arr[:];       // Full slice
```

### Tuples

Tuples are heterogeneous, fixed-size collections of values.

**Syntax**: `(T, U, ...)` where each position can have a different type.

| Type         | Description |
|--------------|-------------|
| `()`         | Unit type (empty tuple) |
| `(T,)`       | Single-element tuple |
| `(T, U)`     | Two-element tuple |
| `(T, U, V)`  | Three-element tuple |

```spl
let pair: (i32, f64) = (42, 3.14);
let triple = (1, "hello", true);   // (i32, &str, bool)
let unit: () = ();

// Destructuring
let (x, y) = pair;

// Field access (future feature)
// let first = pair.0;
```

### Structs

Structs are named product types with named fields.

**Syntax**: Defined with `struct` keyword, instantiated with `StructName { field: value }`.

```spl
struct Point {
    x: f64,
    y: f64,
}

struct Rectangle {
    top_left: Point,
    width: f64,
    height: f64,
}

let p = Point { x: 1.0, y: 2.0 };
let r = Rectangle {
    top_left: Point { x: 0.0, y: 0.0 },
    width: 10.0,
    height: 5.0,
};

// Field shorthand
let x = 3.0;
let y = 4.0;
let p2 = Point { x, y };  // Same as Point { x: x, y: y }
```

### Function Pointers

Function pointer types represent the type of a function.

**Syntax**: `fn(Args) -> Return`

| Type               | Description |
|--------------------|-------------|
| `fn()`             | Function taking no args, returning unit |
| `fn(i32) -> bool`  | Function taking i32, returning bool |
| `fn(T, U) -> V`    | Generic function pointer |

```spl
fn add(a: i32, b: i32) -> i32 {
    a + b
}

let f: fn(i32, i32) -> i32 = add;
let result = f(2, 3);  // 5

type Predicate = fn(i32) -> bool;
type BinaryOp = fn(i32, i32) -> i32;
```

---

## 4. Reference Types

References are pointers to values with borrowing semantics. Detailed borrowing rules are defined in the memory model (see `memory-model.md`).

### Immutable References (`&T`)

An immutable reference provides read-only access to a value. Multiple immutable references to the same value can exist simultaneously.

```spl
let x = 42;
let r: &i32 = &x;
let r2: &i32 = &x;  // OK: multiple immutable references
```

### Mutable References (`&mut T`)

A mutable reference provides exclusive read-write access to a value. Only one mutable reference to a value can exist at a time.

```spl
let mut x = 42;
let r: &mut i32 = &mut x;
*r = 100;  // Modify through reference
```

### Reference Rules Summary

| Reference Type | Aliasing | Mutation |
|----------------|----------|----------|
| `&T`           | Many allowed | Read-only |
| `&mut T`       | Exclusive | Read-write |

---

## 5. Type Inference

SPL uses local type inference, determining types from context without requiring explicit annotations everywhere.

### Inference Contexts

**Variable bindings**: Types are inferred from the initializer.

```spl
let x = 5;           // x: i32 (default integer type)
let y = 3.14;        // y: f64 (default float type)
let flag = true;     // flag: bool
let s = "hello";     // s: &str
```

**Function return types**: If the return type annotation is omitted, it is inferred from the function body.

```spl
fn five() { 5 }          // Returns i32
fn greet() { "hi" }      // Returns &str
fn nothing() { }         // Returns ()
```

**Generic instantiation**: Type parameters are inferred from usage context.

```spl
let v = Vec::new();      // Vec<?>
v.push(42);              // Now Vec<i32>

let p = Point { x: 1.0, y: 2.0 };  // Point<f64>
```

### Required Annotations

Some positions always require explicit type annotations:

**Function parameters**: Must always be annotated.

```spl
fn add(a: i32, b: i32) -> i32 { a + b }  // Required
fn bad(a, b) { a + b }                    // ERROR: missing types
```

**Struct fields**: Must always be annotated.

```spl
struct Point {
    x: f64,    // Required
    y: f64,    // Required
}
```

**Ambiguous contexts**: When inference cannot determine a unique type.

```spl
let x = Vec::new();  // ERROR: cannot infer type
let x: Vec<i32> = Vec::new();  // OK

let n = "42".parse();      // ERROR: cannot infer result type
let n: i32 = "42".parse(); // OK
```

### Literal Typing

**Integer literals**:
- Default type: `i32`
- Suffix overrides: `42u8`, `100i64`, `0xFFu32`
- Context determines type: `let x: u8 = 5;` makes `5` a `u8`

**Float literals**:
- Default type: `f64`
- Suffix overrides: `3.14f32`
- Context determines type: `let x: f32 = 1.0;` makes `1.0` an `f32`

```spl
let a = 42;          // i32
let b = 42i64;       // i64
let c: u8 = 42;      // u8
let d = 3.14;        // f64
let e = 3.14f32;     // f32
let f: f32 = 2.0;    // f32
```

### Inference Algorithm

SPL uses a Hindley-Milner style inference algorithm with the following steps:

1. **Constraint generation**: Walk the AST, generating type constraints from expressions
2. **Unification**: Solve constraints by unifying type variables
3. **Defaulting**: Apply defaults for unconstrained numeric types (`i32`, `f64`)
4. **Error if ambiguous**: Report errors for types that cannot be determined

---

## 6. Generics

Generics enable writing code that works with multiple types through type parameters.

### Type Parameters

Type parameters are declared in angle brackets after the item name.

```spl
struct Point<T> {
    x: T,
    y: T,
}

fn identity<T>(x: T) -> T {
    x
}

impl<T> Point<T> {
    fn new(x: T, y: T) -> Point<T> {
        Point { x, y }
    }
}
```

### Generic Instantiation

Generic types become concrete through instantiation, either explicitly or through inference.

```spl
// Explicit instantiation
let p: Point<i32> = Point { x: 1, y: 2 };

// Inferred instantiation
let q = Point { x: 1.0, y: 2.0 };  // Point<f64>

// Turbofish syntax for function calls
let id = identity::<i32>(42);
```

### Monomorphization

SPL uses monomorphization: each unique instantiation of a generic generates specialized code at compile time.

```spl
Point<i32>   // Generates code for Point with i32 fields
Point<f64>   // Generates separate code for Point with f64 fields
```

This means:
- `Point<i32>` and `Point<f64>` are completely distinct types
- No runtime overhead for generics
- Code size increases with more instantiations

### Self Type

Within an `impl` block, `Self` refers to the implementing type.

```spl
impl<T> Point<T> {
    fn origin() -> Self {
        Self { x: 0, y: 0 }  // Self = Point<T>
    }

    fn clone(&self) -> Self {
        Self { x: self.x, y: self.y }
    }
}
```

`Self` is equivalent to the full type path with its type parameters (`Point<T>` in the example above).

---

## 7. Type Coercions

SPL distinguishes between implicit coercions (automatic) and explicit casts (using `as`).

### Implicit Coercions

SPL performs very few implicit coercions to maintain type safety.

| From | To | Description |
|------|----|-------------|
| `&mut T` | `&T` | Mutable to immutable reference |
| `!` | Any type | Never type to any type |

```spl
fn take_ref(r: &i32) { }

let mut x = 42;
let r: &mut i32 = &mut x;
take_ref(r);  // &mut i32 coerces to &i32

let y: i32 = if true { 1 } else { panic("!") };  // ! coerces to i32
```

**No implicit numeric coercions**: Unlike C, SPL never implicitly converts between numeric types.

```spl
let x: i32 = 42;
let y: i64 = x;    // ERROR: no implicit conversion
let y: i64 = x as i64;  // OK: explicit cast
```

### Explicit Casts (`as`)

The `as` operator performs explicit type conversions.

**Numeric casts**:

| Cast | Behavior |
|------|----------|
| Smaller → Larger int | Zero/sign extension |
| Larger → Smaller int | Truncation |
| Int → Float | Closest representable value |
| Float → Int | Truncation toward zero |
| Float → Float | Precision change |

```spl
let a: i32 = 1000;
let b: i64 = a as i64;     // Sign extension: 1000
let c: i8 = a as i8;       // Truncation: -24 (overflow)
let d: f64 = a as f64;     // 1000.0
let e: i32 = 3.7 as i32;   // 3 (truncation toward zero)
let f: f32 = 1.5f64 as f32; // Precision loss possible
```

**Other casts** (future features):
- Pointer casts (in unsafe context)
- Reference to raw pointer

---

## 8. Type Equality and Compatibility

### Nominal Typing

SPL uses nominal typing: types are distinguished by their names, not their structure.

```spl
struct Meters { value: f64 }
struct Feet { value: f64 }

let m = Meters { value: 100.0 };
let f: Feet = m;  // ERROR: Meters != Feet, despite same structure
```

Two types are equal if and only if they have the same name (and same type arguments for generics).

### Generic Type Equality

Generic types are equal only when their type arguments are equal.

```spl
Point<i32> == Point<i32>   // Equal
Point<i32> != Point<i64>   // Not equal
Point<i32> != Point<u32>   // Not equal
Vec<String> != Vec<&str>   // Not equal
```

### Type Aliases

Type aliases are transparent: they create a new name for an existing type, not a new type.

```spl
type Int = i32;
type Pair<T> = (T, T);

let x: Int = 42;
let y: i32 = x;           // OK: Int and i32 are the same type

let p: Pair<i32> = (1, 2);
let q: (i32, i32) = p;    // OK: Pair<i32> is (i32, i32)
```

### Type Compatibility Rules

| Types | Compatible? | Reason |
|-------|-------------|--------|
| `i32` and `i32` | Yes | Same type |
| `i32` and `i64` | No | Different types |
| `&T` and `&mut T` | One-way | `&mut T` coerces to `&T` |
| `[T; 3]` and `[T; 4]` | No | Different sizes |
| `Point<i32>` and `Point<i64>` | No | Different type arguments |
| `type A = i32` and `i32` | Yes | Alias is transparent |

---

## 9. Sized and Unsized Types

Most types in SPL have a known size at compile time. These are called **sized types**. Some types do not have a known size and are called **unsized** or **dynamically sized types (DSTs)**.

### Unsized Types

| Type | Description |
|------|-------------|
| `str` | String slice |
| `[T]` | Slice |

Unsized types have restrictions:
- Cannot be used as local variables directly
- Cannot be passed by value
- Must always appear behind a pointer (`&`, `&mut`, or `Box`)

```spl
let s: str = "hello";      // ERROR: str is unsized
let s: &str = "hello";     // OK: reference to str

let arr: [i32] = [1,2,3];  // ERROR: slice is unsized
let arr: &[i32] = &[1,2,3]; // OK: reference to slice
```

### Sized Bound (Future)

When trait bounds are added, a `Sized` bound will constrain type parameters to sized types:

```spl
fn foo<T>(x: T) { }         // T: Sized implicitly
fn bar<T: ?Sized>(x: &T) { } // T may be unsized
```

---

## Type Summary Table

| Category | Types | Sized | Notes |
|----------|-------|-------|-------|
| Integers | `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128` | Yes | Two's complement |
| Floats | `f32`, `f64` | Yes | IEEE 754 |
| Boolean | `bool` | Yes | `true` or `false` |
| Character | `char` | Yes | Unicode scalar |
| Unit | `()` | Yes | Zero-size |
| Never | `!` | Yes | No values |
| String slice | `str` | No | UTF-8, behind reference |
| Owned string | `String` | Yes | Heap-allocated |
| Array | `[T; N]` | Yes | Fixed size |
| Slice | `[T]` | No | Behind reference |
| Tuple | `(T, U, ...)` | Yes | Heterogeneous |
| Struct | User-defined | Yes | Named fields |
| Reference | `&T`, `&mut T` | Yes | Borrowing |
| Function pointer | `fn(...) -> T` | Yes | Function type |

---

## Examples

### Working with Numeric Types

```spl
fn example_numerics() {
    // Integer operations
    let a: i32 = 100;
    let b: i32 = 7;
    let sum = a + b;        // 107
    let product = a * b;    // 700
    let quotient = a / b;   // 14 (integer division)
    let remainder = a % b;  // 2

    // Explicit conversions
    let big: i64 = a as i64;
    let small: i8 = a as i8;  // Truncation!

    // Float operations
    let x: f64 = 3.14159;
    let y: f64 = 2.0;
    let result = x * y;     // 6.28318

    // Int to float
    let n: i32 = 42;
    let f: f64 = n as f64;  // 42.0
}
```

### Working with References

```spl
fn example_references() {
    let x = 42;
    let r: &i32 = &x;       // Immutable reference

    let mut y = 100;
    let mr: &mut i32 = &mut y;
    *mr = 200;              // Modify through reference

    // Reference coercion
    fn takes_ref(r: &i32) { }
    takes_ref(mr);          // &mut i32 coerces to &i32
}
```

### Generic Types

```spl
struct Pair<T, U> {
    first: T,
    second: U,
}

impl<T, U> Pair<T, U> {
    fn new(first: T, second: U) -> Self {
        Self { first, second }
    }

    fn swap(self) -> Pair<U, T> {
        Pair {
            first: self.second,
            second: self.first,
        }
    }
}

fn example_generics() {
    let p1 = Pair::new(1, "hello");      // Pair<i32, &str>
    let p2 = Pair::new(3.14, true);      // Pair<f64, bool>
    let p3: Pair<i64, i64> = Pair::new(1, 2);  // Explicit types
    let swapped = p1.swap();             // Pair<&str, i32>
}
```

### Type Inference in Practice

```spl
fn example_inference() {
    // Literals default to i32 and f64
    let n = 42;         // i32
    let f = 3.14;       // f64

    // Context propagates type
    let bytes: [u8; 4] = [1, 2, 3, 4];  // Literals are u8

    // Function return inference
    fn compute() { 1 + 2 }  // Returns i32

    // Generic inference from usage
    let mut v = Vec::new();
    v.push(42);         // Now v: Vec<i32>
}
```
