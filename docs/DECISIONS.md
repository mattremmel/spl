# SPL Language Design Decisions

This document summarizes key design decisions made for SPL, based on systematic review of the language specification.

## 1. Lexical Grammar

### 1.1 Keywords (38 total)
Added: `enum`, `trait`, `const`, `static`, `unsafe`, `async`, `await`, `yield`

### 1.2 Operators
- Added `!` for error propagation (Try operator)
- `as` is reserved for import renaming only, not type casting

### 1.3 Comments
Block comments nest (like Rust), allowing code with comments to be commented out.

### 1.4 Numeric Suffixes
- All type suffixes supported: `i8`-`i128`, `u8`-`u128`, `isize`, `usize`, `f32`, `f64`
- Underscore before suffix allowed: `42_i64`, `3.14_f32`

---

## 2. Syntax Grammar

### 2.1 Explicit Return/Break
No implicit tail expressions. All returns must use `return`, all block values must use `break`. Semicolons have no semantic significance.

```spl
fn double(x: i32): i32 {
    return x * 2;
}

let result = {
    let temp = compute();
    break temp * 2;
};
```

### 2.2 Enum Syntax
Enums use braces for declarations, consistent with struct syntax:

```spl
enum Option{Some(T), None} where T

enum Message{
    Quit,
    Move(x: i32, y: i32),  // named fields
    Write(String),          // tuple variant
}
```

### 2.3 Delimiter Philosophy: Braces vs Parentheses

SPL uses a consistent delimiter philosophy throughout the language:

**Braces `{}` = code blocks and item lists**
- Enum body: `enum Color{Red, Green, Blue}`
- Trait body: `trait Clone { fn clone(&self): Self; }`
- Impl body: `impl Point { fn new(): Self { ... } }`
- Function body: `fn foo() { ... }`

**Parentheses `()` = data shapes**
- Struct fields: `struct Point(x: i32, y: i32)`
- Enum variant data: `Some(T)`, `Move(x: i32, y: i32)`
- Tuples: `(i32, String)`
- Function params: `fn foo(x: i32, y: i32)`
- Generic args: `Vec(T: i32)`
- Instantiation: `Point(x: 1, y: 2)`
- Future named tuples: `(x: i32, y: i32)`

Named vs positional fields are distinguished by presence of `:` after identifier:
- Named: `Point(x: i32, y: i32)` - has name `:` type
- Positional: `Pair(i32, i32)` - just types

### 2.4 Struct Syntax
All structs use parentheses for their fields:

```spl
struct Point(x: i32, y: i32)   // named fields
struct Pair(i32, i32)           // positional fields
struct Empty()                  // unit struct
```

Declaration mirrors usage:
- Declaration: `struct Point(x: i32, y: i32)`
- Instantiation: `Point(x: 1, y: 2)`
- Pattern: `let Point(x, y) = p`

### 2.5 Trait Syntax
Traits use braces:

```spl
trait Clone {
    fn clone(&self): Self;
}

trait Iterator {
    type Item;
    fn next(&mut self): Self.Item?;
}
```

### 2.6 Trait Implementation
`impl Trait for Type` syntax:

```spl
impl Clone for Point {
    fn clone(&self): Self {
        return Self(x = self.x, y = self.y);
    }
}
```

### 2.7 Associated Types
Traits support associated types.

### 2.8 Function Return Types
**Mandatory** - all functions must declare their return type, including `()` for unit.

### 2.9 Closures
Escaping vs non-escaping semantics with move-by-default. See [ADR-012](designs/012-closures.md).

**Non-escaping closures** (passed to `map`, `filter`, etc.): borrow by default
**Escaping closures** (stored, returned, spawned): move by default, `~` for clone

```spl
// Non-escaping: borrows threshold
items.filter(|x| x > threshold);

// Escaping: data moved, config cloned
spawn(|data, ~config| process(data, config));

// Clone all shorthand
spawn(clone |a, b, c| { ... });
```

Single-expression closures allow implicit return: `|a, b| a + b`

### 2.10 Type Aliases
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

### 4.1 Intersection Semantics with Optional Lifetime Markers
References can be function parameters and can be returned from functions. References cannot be stored in structs. By default, returned references are conservatively assumed to borrow from **all** input references (intersection). Optional `'name` markers can specify exact provenance.

```spl
// OK: reference as parameter
fn process(data: &str) { }

// OK: single input ref, can return ref (borrows from input)
fn first(&self): &T { return &self.data[0]; }
fn trim(s: &str): &str { ... }

// OK: multiple input refs - default intersection (borrows from ALL)
fn longer(a: &str, b: &str): &str {
    if a.len() > b.len() { return a; }
    return b;
}
// Usage: result valid only while BOTH a AND b are valid

// OK: optional lifetime markers for precision
fn first_only(a: &'x str, b: &str): &'x str {
    return a;  // Compiler enforces: cannot return b
}
// Usage: result valid while 'x (a) is valid, regardless of b

// NOT ALLOWED: store in struct
// struct Parser(input: &str)
```

**Key simplifications vs Rust:**
- Markers are optional (intersection default)
- No lifetime relationships (`'a: 'b`)
- No lifetimes in struct types
- No higher-rank bounds

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
let checked: i8? = x.try_into();
```

### 4.4 Panic Behavior
Panic unwinds the stack, running destructors for proper resource cleanup. This enables:
- Task isolation (panicked tasks don't crash the whole program)
- Guaranteed destructor execution (files closed, locks released)
- Optional recovery via `catch_panic`

```spl
fn example() {
    let file = File.create("data.txt");
    let guard = mutex.lock();

    panic("something went wrong");

    // Destructors run during unwind:
    // - guard released (no deadlock)
    // - file closed (no leak)
}

// Catch and recover if needed
let result = catch_panic(|| risky_operation());
```

**FFI boundary**: Panic aborts if it would unwind across FFI boundaries (undefined behavior). SPL automatically catches at `extern fn` call sites.

---

## 5. Error Handling

### 5.1 Try Operator
`!` works with any type implementing `Try` trait (includes Option and Result).

SPL uses `!` for try/propagate (early return on error) and `?.` for optional chaining (short-circuit to None without early return). See [error-handling.md](spec/error-handling.md) for full details.

---

## 6. Advanced Features

### 6.1 Iteration
Interior iteration with coroutines/generators. `for` loops desugar to `Iterable` trait calls or `Iterator.next()`. See [ADR-011](designs/011-iteration-and-generators.md) for full design.

The `Iterable` trait's `get(&self): &T` method is enabled by intersection semantics:
```spl
// for item in &collection desugars to:
let mut __i: usize = 0;
while __i < collection.len() {
    let item: &T = collection.get(__i);  // Borrows from collection
    body
    __i += 1;
}
```

Functional-style iteration with chaining:
```spl
vec.iter()
    .filter(|n| n > 0)
    .map(|n| n * 2)
    .for_each(|n| println(n));
```

### 6.2 Tail Call Optimization
Guaranteed for all tail calls.

### 6.3 Variadic Functions
Native support: `fn print(args: ...Display)` - reduces macro dependency.

### 6.4 Concurrency
**No function coloring** - any function can yield, no async/sync distinction. Built-in runtime with Go-style simplicity. See [ADR-013](designs/013-async-await.md) for full design.

Key points:
- `std.task.spawn()` for concurrent tasks, returns `JoinHandle(T)`
- Growable stacks with adaptive sizing (Go 1.19+ model)
- Async preemption for tight loops (Go 1.14+ model)
- Task isolation with unwinding (panic in task doesn't crash program)
- Drop `JoinHandle` = cancel task

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
| Named struct decl | `struct Point { x: i32 }` | `struct Point(x: i32)` |
| Tuple struct decl | `struct Pair(i32, i32);` | `struct Pair(i32, i32)` |
| Struct literal | `Point { x: 1 }` | `Point(x: 1)` |
| Pattern matching | `if let Some(x) = v` | `if v is Some(x)` |
| Return | Implicit tail | Explicit `return` |
| Block value | Implicit tail | Explicit `break` |
| Semicolons | Semantic | Syntactic only |
| References | First-class | Second-class (no struct storage) |
| Lifetimes | `'a` required | `'a` optional (intersection default) |
| Overflow | Debug trap, release wrap | Always trap |
| Type casting | `as` | Methods |
| Panic | Unwind or abort | Unwind (abort at FFI) |
| Iteration | Exterior (iterators) | Interior (generators) |
| Closures | Borrow default, `move` all | Move default, `~` for clone |
| Concurrency | async/await (colored) | No coloring (Go-style) |
| Runtime | External (tokio, etc.) | Built-in |
| Macros | `macro!()` | `macro()` |
