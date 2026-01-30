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

| Type    | Size (bytes) | Range |
|---------|--------------|-------|
| `i8`    | 1 | -128 to 127 |
| `i16`   | 2 | -32,768 to 32,767 |
| `i32`   | 4 | -2,147,483,648 to 2,147,483,647 |
| `i64`   | 8 | -9,223,372,036,854,775,808 to 9,223,372,036,854,775,807 |
| `i128`  | 16 | -2^127 to 2^127 - 1 |
| `isize` | Platform | Pointer-sized signed integer |
| `u8`    | 1 | 0 to 255 |
| `u16`   | 2 | 0 to 65,535 |
| `u32`   | 4 | 0 to 4,294,967,295 |
| `u64`   | 8 | 0 to 18,446,744,073,709,551,615 |
| `u128`  | 16 | 0 to 2^128 - 1 |
| `usize` | Platform | Pointer-sized unsigned integer |

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

### Decimal Type

The `decimal` type provides exact decimal arithmetic for financial and monetary calculations, avoiding the precision issues of binary floating-point. It is included in the prelude.

| Type      | Description |
|-----------|-------------|
| `decimal` | Exact decimal floating-point (IEEE 754 decimal128 or similar) |

```spl
let price: decimal = 0.10 + 0.20;  // Exactly 0.30
let tax = price * 0.0825;          // Precise decimal arithmetic
```

**Arithmetic Operators:**

| Operator | Description |
|----------|-------------|
| `+` | Addition |
| `-` | Subtraction |
| `*` | Multiplication |
| `/` | Division |
| `%` | Remainder |
| `-` (unary) | Negation |

**Comparison:** All comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) are supported.

**Conversions:**
- From integer types: `let d: decimal = 42.widen();`
- From float types: `let d: decimal = 3.14.to_decimal();` (may lose precision)
- To float types: `let f: f64 = d.to_f64();` (may lose precision)

### Arbitrary Precision Integer

The `bigint` type provides arbitrary precision integers that never overflow.

| Type     | Description |
|----------|-------------|
| `bigint` | Arbitrary precision integer (grows as needed) |

```spl
let huge: bigint = 999999999999999999999999999999bigint;
let result = huge * huge;  // No overflow
```

**Arithmetic Operators:**

| Operator | Description |
|----------|-------------|
| `+` | Addition |
| `-` | Subtraction |
| `*` | Multiplication |
| `/` | Division (truncates toward zero) |
| `%` | Remainder |
| `-` (unary) | Negation |

**Comparison:** All comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) are supported.

**Conversions:**
- From fixed-size integers: `let b: bigint = 42.to_bigint();`
- To fixed-size integers: `let n: i64 = b.try_into()!;` (returns `Err` if out of range)

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

fn explicit_unit(): () {
    return ();
}
```

### Never Type

The never type `!` represents computations that never complete. This type has no values and is used for diverging functions.

| Type | Size | Values |
|------|------|--------|
| `!`  | N/A  | none   |

A function returning `!` never returns normally (it panics, loops forever, or exits the program).

```spl
fn panic(msg: &str): ! {
    // Terminates the program
}

fn infinite(): ! {
    loop { }
}
```

The never type coerces to any other type, enabling code like:

```spl
let x: i32 = if condition { 42 } else { panic("unreachable") };
```

When a block contains a single expression, the value is implicit. Multi-statement blocks require explicit `yield` (or `return` in functions).

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
let mut s = String.from("Hello");
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

**Syntax**: Defined with `struct` keyword using parentheses, instantiated with `StructName(field: value)`.

```spl
struct Point(
    x: f64,
    y: f64,
)

struct Rectangle(
    top_left: Point,
    width: f64,
    height: f64,
)

let p = Point(x: 1.0, y: 2.0);
let r = Rectangle(
    top_left: Point(x: 0.0, y: 0.0),
    width: 10.0,
    height: 5.0,
);

// Field shorthand
let x = 3.0;
let y = 4.0;
let p2 = Point(x, y);  // Same as Point(x: x, y: y)
```

### Function Types

Function types represent any callable with a given signature, including named functions and closures (with or without captures).

**Syntax**: `fn(Args): Return`

| Type               | Description |
|--------------------|-------------|
| `fn()`             | Callable taking no args, returning unit |
| `fn(i32): bool`    | Callable taking i32, returning bool |
| `fn(T, U): V`      | Generic callable type |

```spl
fn add(a: i32, b: i32): i32 {
    return a + b;
}

let f: fn(i32, i32): i32 = add;
let result = f(2, 3);  // 5

// Closures (including capturing) also use fn types
let x = 10;
let add_x: fn(i32): i32 = |n| n + x;

type Predicate = fn(i32): bool;
type BinaryOp = fn(i32, i32): i32;
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

**Second-Class References:** SPL uses second-class references—references can only be function parameters, never stored in structs or returned from functions. This eliminates the need for lifetime annotations. See [memory-model.md](memory-model.md) for full details.

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

**Function return types**: Return type annotations are mandatory for non-unit returns. Functions must explicitly declare their return type.

```spl
fn five(): i32 { 5 }              // Single expression: implicit return
fn greet(): &str { return "hi"; } // Explicit return also works
fn compute(): i32 {               // Multi-statement: explicit return required
    let x = 5;
    return x * 2;
}
fn nothing() { }                  // Unit return type can be omitted
```

**Generic instantiation**: Type parameters are inferred from usage context.

```spl
let v = Vec.new();       // Vec(?)
v.push(42);              // Now Vec(T: i32)

let p = Point(x: 1.0, y: 2.0);  // Point(f64)
```

### Required Annotations

Some positions always require explicit type annotations:

**Function parameters**: Must always be annotated.

```spl
fn add(a: i32, b: i32): i32 { a + b }  // Required
fn bad(a, b) { a + b }                  // ERROR: missing types
```

**Struct fields**: Must always be annotated.

```spl
struct Point(
    x: f64,    // Required
    y: f64,    // Required
)
```

**Ambiguous contexts**: When inference cannot determine a unique type.

```spl
let x = Vec.new();  // ERROR: cannot infer type
let x: Vec(T: i32) = Vec.new();  // OK

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

### Enum Variant Shorthand

SPL supports Swift-style enum variant shorthand (`.Variant`) when the enum type can be inferred from context. This eliminates redundancy when the type is already known.

**Valid inference contexts:**

| Context | Example | Inferred From |
|---------|---------|---------------|
| Match arms | `match color { .Red => ... }` | Scrutinee type |
| Function arguments | `set_color(.Blue)` | Parameter type |
| Variable binding | `let c: Color = .Green` | Explicit annotation |
| Return statement | `return .Ok(value)` | Function return type |
| Binary comparison | `if color == .Red` | Other operand type |
| Assignment | `color = .Blue` | Left-hand side type |
| Array/collection literals | `let colors: Vec(T: Color) = [.Red, .Blue]` | Collection element type |

**Examples:**

```spl
enum Status{Pending, Active, Complete}

// Match - type from scrutinee
fn describe(s: Status): &str {
    match s {
        .Pending => "waiting",
        .Active => "in progress",
        .Complete => "done",
    }
}

// Function argument - type from parameter
fn update(s: Status) { ... }
update(.Active)

// Return - type from signature
fn default_status(): Status {
    return .Pending;
}

// Comparison - type from other operand
fn is_done(s: Status): bool {
    return s == .Complete;
}

// With variant data
enum Result{Ok(T), Err(E)} where T, E

fn parse(input: &str): Result(T: i32, E: ParseError) {
    if input.is_empty() {
        return .Err(ParseError.Empty);
    }
    return .Ok(input.parse_int());
}
```

**Invalid contexts (explicit type required):**

```spl
let x = .Red;              // ERROR: cannot infer enum type
foo(.Red, .Blue);          // ERROR if foo is generic over enum type
```

The shorthand improves readability when the enum type is contextually obvious, reducing visual noise while maintaining type safety.

---

## 6. Generics

Generics enable writing code that works with multiple types through type parameters.

### Type Parameters

Type parameters are declared in `where` clauses. Unlike Rust (which uses `<T>` syntax), SPL's `where` clause both **declares** and optionally **constrains** type parameters.

```spl
// `where T` declares type parameter T (no constraints)
struct Point(x: T, y: T) where T

// `where T` declares T for use in parameters and return type
fn identity(x: T): T where T {
    return x;
}

// `where T` declares T for the impl block
impl Point(T: T) where T {
    fn new(x: T, y: T): Point(T: T) {
        return Point(x: x, y: y);
    }
}

// Multiple type parameters
struct Pair(first: A, second: B) where A, B
```

### Trait Bounds

Trait bounds constrain type parameters to types that implement specific traits. Bounds are specified with `:` in the `where` clause.

```spl
// T must implement Clone
fn duplicate(x: &T): T where T: Clone {
    return x.clone();
}

// Multiple bounds with +
fn print_and_clone(x: &T): T where T: Clone + Debug {
    println(x.debug());
    return x.clone();
}

// Multiple type parameters with different bounds
fn convert(input: T): U where T: Into(Target: U), U {
    return input.into();
}

// Bounds on struct type parameters
struct SortedVec(items: Vec(T: T)) where T: Ord

impl SortedVec(T: T) where T: Ord {
    fn insert(&mut self, item: T) {
        // Can use comparison because T: Ord
        // ...
    }
}
```

**Common Trait Bounds:**

| Bound | Meaning |
|-------|---------|
| `T: Clone` | T can be explicitly cloned |
| `T: Copy` | T can be implicitly copied |
| `T: Debug` | T can be formatted for debugging |
| `T: Default` | T has a default value |
| `T: Eq` | T supports equality comparison |
| `T: Ord` | T supports ordering comparison |
| `T: Hash` | T can be hashed |
| `T: Send` | T can be sent between threads |
| `T: Sync` | T can be shared between threads |

### Associated Types

Traits can declare associated types—type placeholders that implementors define.

```spl
trait Iterator {
    type Item;  // Associated type

    fn next(&mut self): Self.Item?;
}

// Implementor specifies the associated type
impl Iterator for Counter {
    type Item = i32;

    fn next(&mut self): i32? {
        // ...
    }
}
```

**Using Associated Types in Bounds:**

```spl
// Constrain the associated type
fn sum_all(iter: &mut I): i32 where I: Iterator(Item: i32) {
    let mut total = 0;
    while iter.next() is Some(n) {
        total += n;
    }
    return total;
}

// Access associated type with Self.TypeName
trait Container {
    type Item;

    fn contains(&self, item: &Self.Item): bool;
}
```

**Associated Types vs Type Parameters:**

| Feature | Associated Type | Type Parameter |
|---------|-----------------|----------------|
| Syntax | `trait Foo { type Bar; }` | `trait Foo(T) where T` |
| Determined by | Implementor | Caller |
| Multiple impls | One per type | Many per type |
| Use case | Output types | Input types |

```spl
// Associated type: one Item type per Iterator impl
trait Iterator {
    type Item;
    fn next(&mut self): Self.Item?;
}

// Type parameter: can implement Add for many RHS types
trait Add(RHS) where RHS {
    type Output;
    fn add(self, rhs: RHS): Self.Output;
}
```

### Generic Instantiation

Generic types become concrete through instantiation, either explicitly or through inference.

```spl
// Explicit instantiation
let p: Point(T: i32) = Point(x: 1, y: 2);

// Inferred instantiation
let q = Point(x: 1.0, y: 2.0);  // Point(T: f64)

// Explicit type application (type args first, then value args)
let id = identity(T: i32, 42);
```

### Monomorphization

SPL uses monomorphization: each unique instantiation of a generic generates specialized code at compile time.

```spl
Point(T: i32)   // Generates code for Point with i32 fields
Point(T: f64)   // Generates separate code for Point with f64 fields
```

This means:
- `Point(T: i32)` and `Point(T: f64)` are completely distinct types
- No runtime overhead for generics
- Code size increases with more instantiations

### Impl Block Patterns

SPL's `where` clause approach to generics in impl blocks supports all the same patterns as Rust's `impl<T>` syntax, while eliminating redundant declarations.

**Generic impl** - The `where` clause declares type parameters for the impl block:

```spl
impl Point(T: T) where T {
    fn new(x: T, y: T): Point(T: T) {
        return Point(x: x, y: y);
    }
}
```

**Conditional impl** - Bounds in the `where` clause restrict which types the impl applies to:

```spl
impl Container(T: T) where T: Clone {
    fn clone_all(&self): Vec(T: T) {
        return self.items.clone();
    }
}
```

**Concrete impl** - No `where` clause needed when implementing for a specific type:

```spl
impl Box(T: u32) {
    fn special_method(&self): u32 {
        return self.value * 2;
    }
}

impl Box(T: String) {
    fn special_method(&self): usize {
        return self.value.len();
    }
}
```

**Different parameter names** - The impl can use different names than the struct definition to emphasize the distinction between parameter name and type variable:

```spl
struct Foo(val: T) where T

// R is the type variable, T is the parameter name from the struct
impl Foo(T: R) where R {
    fn bar(val: R) {}
}
```

Here `T` is the **parameter name** (from the struct definition), and `R` is the **type variable** declared by `where R`. The syntax `Foo(T: R)` means "Foo with its T parameter set to type R".

**Method-level generics** - Methods can introduce additional type parameters beyond the impl block:

```spl
impl Vec(T: T) where T {
    fn convert(&self): Vec(T: U) where U, T: Into(Target: U) {
        // Convert each element from T to U
    }
}
```

**Trait impl with bounds** - Implementing traits conditionally:

```spl
impl Clone for Option(T: T) where T: Clone {
    fn clone(&self): Self {
        return match self {
            Some(v) => Some(v.clone()),
            None => None,
        };
    }
}
```

**Multiple concrete trait implementations** - Different implementations for different type instantiations:

```spl
trait Cost {
    fn pretty_cost(&self): String;
}

struct Price(val: T) where T

impl Cost for Price(T: USD) {
    fn pretty_cost(&self): String {
        return format("${}", self.val.amount);
    }
}

impl Cost for Price(T: EUR) {
    fn pretty_cost(&self): String {
        return format("{}€", self.val.amount);
    }
}
```

### Self Type

Within an `impl` block, `Self` refers to the implementing type.

```spl
impl Point(T: T) where T {
    fn origin(): Self {
        return Self(x: 0, y: 0);  // Self = Point(T: T)
    }

    fn clone(&self): Self {
        return Self(x: self.x, y: self.y);
    }
}
```

`Self` is equivalent to the full type path with its type parameters (`Point(T: T)` in the example above).

### Trait Objects (Future)

SPL currently uses monomorphization exclusively for generics. **Trait objects** (`dyn Trait`) for dynamic dispatch are planned for a future version.

Trait objects would enable:
- Heterogeneous collections: `Vec(dyn Draw)` containing different types implementing `Draw`
- Runtime polymorphism without generics
- Reduced code size (at the cost of indirect calls)

Until then, use enums for heterogeneous collections:

```spl
enum Shape{
    Circle(Circle),
    Rectangle(Rectangle),
}

let shapes: Vec(T: Shape) = [Shape.Circle(c), Shape.Rectangle(r)];
```

---

## 7. Type Coercions

SPL distinguishes between implicit coercions (automatic) and explicit conversions (using methods).

### Implicit Coercions

SPL performs very few implicit coercions to maintain type safety.

| From | To | Description |
|------|----|-------------|
| `&mut T` | `&T` | Mutable to immutable reference |
| `!` | Any type | Never type to any type |
| `[T; N]` | `Vec(T: T)` | Array to Vec (when target type is known) |

```spl
fn take_ref(r: &i32) { }

let mut x = 42;
let r: &mut i32 = &mut x;
take_ref(r);  // &mut i32 coerces to &i32

let y: i32 = if true { 1 } else { panic("!") };  // ! coerces to i32

// Array to Vec coercion
let v: Vec(T: i32) = [1, 2, 3];  // Array literal coerces to Vec
fn take_vec(v: Vec(T: i32)) { }
take_vec([1, 2, 3]);             // Coerced at call site

let arr = [1, 2, 3];          // No coercion: arr is [i32; 3]
```

**Array to Vec coercion rules:**

1. **Array literals are always arrays**: `[1, 2, 3]` creates a `[i32; 3]`, not a `Vec`
2. **Coercion requires known target type**: Only happens when the expected type is explicitly `Vec(T: T)`
3. **No coercion without context**: `let x = [1, 2, 3]` creates an array, never a Vec

| Expression | Type | Reason |
|------------|------|--------|
| `let x = [1, 2, 3]` | `[i32; 3]` | No target type, stays as array |
| `let x: Vec(T: i32) = [1, 2, 3]` | `Vec(T: i32)` | Target type triggers coercion |
| `foo([1, 2, 3])` where `foo(v: Vec(T: i32))` | `Vec(T: i32)` | Parameter type triggers coercion |
| `let x: [i32; 3] = [1, 2, 3]` | `[i32; 3]` | Target type is array, no coercion |

**No implicit numeric coercions**: Unlike C, SPL never implicitly converts between numeric types.

```spl
let x: i32 = 42;
let y: i64 = x;           // ERROR: no implicit conversion
let y: i64 = x.widen();   // OK: explicit widening conversion
```

### Explicit Conversions (Methods)

SPL uses methods for explicit type conversions instead of a cast operator. This makes the intent clear and prevents accidental lossy conversions.

**Conversion Methods**:

| Method | Behavior |
|--------|----------|
| `.widen()` | Safe widening (infers target type) |
| `.truncate()` | Explicit lossy truncation |
| `.saturate()` | Clamp to target type's range |
| `.try_into()` | Fallible conversion returning `Result` |
| `.reinterpret()` | Bit reinterpretation |

```spl
let a: i32 = 1000;
let b: i64 = a.widen();           // Sign extension: 1000
let c: i8 = a.truncate();         // Explicit truncation: -24
let d: i8 = a.saturate();         // Clamped to 127
let e: Result(T: i8, E: TryFromIntError) = a.try_into(); // Err (out of range)
let f: f64 = a.widen();           // 1000.0

let g: f64 = 3.7;
let h: i32 = g.truncate();        // 3 (truncation toward zero)
let i: u32 = a.reinterpret();     // Bit reinterpretation
```

**Integer Overflow:**
All integer operations trap on overflow by default. Use explicit methods for wrapping or saturating arithmetic:

```spl
let x: u8 = 255;
// let y = x + 1;              // Panic: overflow!
let y = x.wrapping_add(1);     // 0 (wraps)
let z = x.saturating_add(1);   // 255 (saturates)
let w = x.checked_add(1);      // None
```

---

## 8. Type Equality and Compatibility

### Nominal Typing

SPL uses nominal typing: types are distinguished by their names, not their structure.

```spl
struct Meters(value: f64)
struct Feet(value: f64)

let m = Meters(value: 100.0);
let f: Feet = m;  // ERROR: Meters != Feet, despite same structure
```

Two types are equal if and only if they have the same name (and same type arguments for generics).

### Generic Type Equality

Generic types are equal only when their type arguments are equal.

```spl
Point(T: i32) == Point(T: i32)   // Equal
Point(T: i32) != Point(T: i64)   // Not equal
Point(T: i32) != Point(T: u32)   // Not equal
Vec(T: String) != Vec(T: &str)   // Not equal
```

### Type Aliases

Type aliases are transparent: they create a new name for an existing type, not a new type.

```spl
type Int = i32;
type Pair(T) = (T, T) where T;

let x: Int = 42;
let y: i32 = x;           // OK: Int and i32 are the same type

let p: Pair(T: i32) = (1, 2);
let q: (i32, i32) = p;    // OK: Pair(T: i32) is (i32, i32)
```

### Optional Type Syntax

The postfix `?` on a type is syntactic sugar for `Option(T: T)`:

| Syntax | Equivalent |
|--------|------------|
| `T?` | `Option(T: T)` |
| `i32?` | `Option(T: i32)` |
| `String?` | `Option(T: String)` |
| `Vec(T: i32)?` | `Option(T: Vec(T: i32))` |

```spl
// These are equivalent:
fn find_user(id: UserId): User? { ... }
fn find_user(id: UserId): Option(T: User) { ... }

// In struct fields:
struct Person(
    name: String,
    email: String?,      // Optional email
    phone: String?,      // Optional phone
)

// In function parameters:
fn greet(name: String, title: String?): () {
    match title {
        Some(t) => println(t + " " + name),
        None => println(name),
    }
}
```

**Nesting:** `T??` is `Option(T: Option(T: T))`, though this is rarely useful.

**Note:** The `?` postfix on types (optional type) is distinct from the `!` postfix operator on expressions (try/propagate). They appear in different syntactic positions and do not conflict. See [error-handling.md](error-handling.md) for full `!` operator semantics.

### Type Compatibility Rules

| Types | Compatible? | Reason |
|-------|-------------|--------|
| `i32` and `i32` | Yes | Same type |
| `i32` and `i64` | No | Different types |
| `&T` and `&mut T` | One-way | `&mut T` coerces to `&T` |
| `[T; 3]` and `[T; 4]` | No | Different sizes |
| `Point(T: i32)` and `Point(T: i64)` | No | Different type arguments |
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
fn foo(x: T) where T { }              // T: Sized implicitly
fn bar(x: &T) where T: ?Sized { }     // T may be unsized
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
| Function type | `fn(...): T` | Yes | Any callable |

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
    let big: i64 = a.widen();
    let small: i8 = a.truncate();  // Explicit truncation

    // Float operations
    let x: f64 = 3.14159;
    let y: f64 = 2.0;
    let result = x * y;     // 6.28318

    // Int to float
    let n: i32 = 42;
    let f: f64 = n.widen();  // 42.0
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
struct Pair(first: T, second: U) where T, U

impl Pair(T: T, U: U) where T, U {
    fn new(first: T, second: U): Self {
        return Self(first: first, second: second);
    }

    fn swap(self): Pair(T: U, U: T) {
        return Pair(
            first: self.second,
            second: self.first,
        );
    }
}

fn example_generics() {
    let p1 = Pair.new(1, "hello");            // Pair(i32, &str)
    let p2 = Pair.new(3.14, true);            // Pair(f64, bool)
    let p3: Pair(i64, i64) = Pair.new(1, 2);  // Explicit types
    let swapped = p1.swap();                  // Pair(&str, i32)
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

    // Generic inference from usage
    let mut v = Vec.new();
    v.push(42);         // Now v: Vec(T: i32)
}
```

---

## References

- [traits.md](traits.md) - Trait definition and implementation
- [syntax-grammar.md](syntax-grammar.md) - Type syntax and generics
- [memory-model.md](memory-model.md) - Ownership and borrowing
- [standard-library.md](standard-library.md) - Standard types and traits
- [ADR-010: Type Interning](../designs/010-type-interning.md) - Type equality implementation
