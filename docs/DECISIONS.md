# SPL Language Design Decisions

This document summarizes key design decisions made for SPL, based on systematic review of the language specification.

## 1. Lexical Grammar

### 1.1 Keywords (38 total)
Added: `enum`, `trait`, `const`, `static`, `unsafe`, `async`, `await`, `yield`

### 1.2 Operators
- Added `?` for error propagation (Try operator)
- `as` is reserved for import renaming only, not type casting

### 1.3 Comments
Block comments nest (like Rust), allowing code with comments to be commented out.

### 1.4 Numeric Suffixes
- All type suffixes supported: `i8`-`i128`, `u8`-`u128`, `isize`, `usize`, `f32`, `f64`
- Underscore before suffix allowed: `42_i64`, `3.14_f32`

---

## 2. Syntax Grammar

### 2.1 Explicit Return/Yield
No implicit tail expressions. All returns must use `return`, all block values must use `yield`. Semicolons have no semantic significance.

```spl
fn double(x: i32): i32 {
    return x * 2;
}

let result = {
    let temp = compute();
    yield temp * 2;
};
```

### 2.2 Enum Syntax
Enums use parentheses, consistent with struct syntax:

```spl
enum Option(T)(Some(T), None) where T

enum Message(
    Quit,
    Move(x: i32, y: i32),  // named fields
    Write(String),          // tuple variant
)
```

### 2.3 Struct/Enum Fields
- Type-only = tuple style: `struct Point(i32, i32)`
- With `:` = named fields: `struct Point(x: i32, y: i32)`

### 2.4 Trait Syntax
Traits use braces:

```spl
trait Clone {
    fn clone(&self): Self;
}

trait Iterator {
    type Item;
    fn next(&mut self): Option(Self.Item);
}
```

### 2.5 Trait Implementation
`impl Trait for Type` syntax:

```spl
impl Clone for Point {
    fn clone(&self): Self {
        return Self(x = self.x, y = self.y);
    }
}
```

### 2.6 Associated Types
Traits support associated types.

### 2.7 Function Return Types
**Mandatory** - all functions must declare their return type, including `()` for unit.

### 2.8 Closures
Deferred - will design later.

### 2.9 Type Aliases
Use where clause with optional constraints:

```spl
type Pair(T) = (T, T) where T
type ClonePair(T) = (T, T) where T: Clone
```

---

## 3. Type System

### 3.1 Platform-Sized Integers
Include `isize` and `usize` for pointer-sized integers.

### 3.2 Built-in Types
- `decimal` - exact decimal arithmetic for financial calculations
- `bigint` - arbitrary precision integers

### 3.3 Prelude (Minimal)
`Option`, `Some`, `None`, `Result`, `Ok`, `Err`, `Vec`, `String`, `decimal`, `print`, `println`

### 3.4 Derive Syntax
Attribute syntax: `#[derive(Copy, Clone)]`

### 3.5 Attribute Syntax
Rust-style: `#[outer]` for items, `#![inner]` for module-level

---

## 4. Memory Model

### 4.1 Second-Class References
References can only be function parameters - never stored in structs or returned from functions. This eliminates lifetime annotations entirely.

```spl
// OK: reference as parameter
fn process(data: &str) { }

// NOT ALLOWED:
// fn get_ref(s: &str): &str { }
// struct Parser(input: &str)
```

### 4.2 Integer Overflow
Always trap - no silent wrapping. Use explicit methods for wrapping/saturating:

```spl
let x: u8 = 255;
// x + 1              // Panic!
x.wrapping_add(1)     // 0
x.saturating_add(1)   // 255
x.checked_add(1)      // None
```

### 4.3 Type Conversions
Methods instead of `as` keyword:

```spl
let wide: i64 = x.widen();
let truncated: i8 = x.truncate();
let saturated: i8 = x.saturate();
let checked: Option(i8) = x.try_into();
```

### 4.4 Panic Behavior
Panic = abort only. No unwinding, simpler implementation.

---

## 5. Error Handling

### 5.1 Try Operator
`?` works with any type implementing `Try` trait (includes Option and Result).

---

## 6. Advanced Features

### 6.1 Iteration
Interior iteration with coroutines/generators. `for` loops desugar to generator-based iteration.

```spl
my_vec.each(|item| {
    process(item);
});
```

### 6.2 Tail Call Optimization
Guaranteed for all tail calls.

### 6.3 Variadic Functions
Native support: `fn print(args: ...Display)` - reduces macro dependency.

### 6.4 Async/Await
Built-in runtime (Go-style simplicity). `async fn main()` just works.

### 6.5 Macros
Look like regular function calls - no `!` required.

---

## Key Differences from Rust

| Feature | Rust | SPL |
|---------|------|-----|
| Generics | `<T>` | `where T` |
| Paths | `::` | `.` |
| Return type | `->` | `:` |
| Type application | `Vec<i32>` | `Vec(i32)` |
| Struct literal | `Point { x: 1 }` | `Point(x = 1)` |
| Pattern matching | `if let Some(x) = v` | `if v is Some(x)` |
| Return | Implicit tail | Explicit `return` |
| Block value | Implicit tail | Explicit `yield` |
| Semicolons | Semantic | Syntactic only |
| References | First-class | Second-class (params only) |
| Lifetimes | `'a` annotations | None needed |
| Overflow | Debug trap, release wrap | Always trap |
| Type casting | `as` | Methods |
| Panic | Unwind or abort | Abort only |
| Iteration | Exterior (iterators) | Interior (generators) |
| Async runtime | External | Built-in |
| Macros | `macro!()` | `macro()` |
