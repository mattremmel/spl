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
    fn next(&mut self): Option(Self.Item);
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
`?` works with any type implementing `Try` trait (includes Option and Result).

Follows Rust semantics: `expr?.field` means early-return then field access (parses as `(expr?).field`).

**Future consideration:** Optional chaining (`?.` as single operator) for expressions like `user?.address?.city` that return `Option(String)` without early return. Would need distinct syntax to avoid confusion with early-return `?` (e.g., `&.` like Ruby).

---

## 6. Advanced Features

### 6.1 Iteration
Interior iteration with coroutines/generators. `for` loops desugar to generator-based iteration. See [ADR-011](designs/011-iteration-and-generators.md) for full design.

**Phase 1 (Initial):** Generator methods that consume `self` for ergonomic chaining:
```spl
vec.iter()
    .filter(|n| n > 0)
    .map(|n| n * 2)
    .for_each(|n| println(n));
```

**Phase 2 (Future):** First-class references for `&self` receivers, enabling zero-copy borrowing iteration.

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
| Block value | Implicit tail | Explicit `yield` |
| Semicolons | Semantic | Syntactic only |
| References | First-class | Second-class (params only) |
| Lifetimes | `'a` annotations | None needed |
| Overflow | Debug trap, release wrap | Always trap |
| Type casting | `as` | Methods |
| Panic | Unwind or abort | Unwind (abort at FFI) |
| Iteration | Exterior (iterators) | Interior (generators) |
| Closures | Borrow default, `move` all | Move default, `~` for clone |
| Concurrency | async/await (colored) | No coloring (Go-style) |
| Runtime | External (tokio, etc.) | Built-in |
| Macros | `macro!()` | `macro()` |
