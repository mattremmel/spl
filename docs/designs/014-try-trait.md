# ADR-014: Try Trait and Error Propagation

**Status:** Accepted
**Date:** 2026-01-28

## Context

SPL's `!` operator enables concise error propagation. DECISIONS.md states "`!` works with any type implementing `Try` trait (includes Option and Result)." This ADR specifies the Try trait and its semantics.

### Goals

1. **Unified error propagation** - `!` works with Option, Result, and custom types
2. **Cross-type conversion** - Option can be used with `!` in Result-returning functions
3. **Simplicity** - Simpler than Rust's Try/FromResidual split
4. **Explicit semantics** - Clear mental model for what `!` does

### Current State

- `enum Option{Some(T), None} where T`
- `enum Result{Ok(T), Err(E)} where T, E`
- Both types are in the prelude
- `!` operator is lexically defined but behavior unspecified

---

## Decision

### 1. Try Trait Definition

```spl
trait Try {
    type Output;        // The success value type
    type Residual;      // The early-return value type

    // Extract success value or return residual for early return
    fn branch(self): ControlFlow(C: Self.Output, B: Self.Residual);

    // Construct success case from output
    fn from_output(output: Self.Output): Self;
}
```

Where `ControlFlow` is:

```spl
enum ControlFlow{
    Continue(C),    // Continue execution with value C
    Break(B),       // Early return with value B
} where C, B
```

**Rationale**: The `branch` method determines whether to continue or early-return. Unlike Rust's split Try/FromResidual design, conversion happens through a separate mechanism (see Section 3).

---

### 2. Result Implementation

```spl
impl Try for Result(T: T, E: E) where T, E {
    type Output = T;
    type Residual = E;

    fn branch(self): ControlFlow(C: T, B: E) {
        match self {
            Ok(v) => .Continue(v),
            Err(e) => .Break(e),
        }
    }

    fn from_output(output: T): Self {
        return Ok(output);
    }
}
```

Usage:

```spl
fn read_config(path: &str): Result(T: Config, E: IoError) {
    let contents = fs.read_to_string(path)!;  // Early return on Err
    let config = parse_config(contents)!;     // Early return on Err
    return Ok(config);
}
```

The `!` operator on `Result(T: T, E: E)`:
1. Calls `branch()` on the value
2. If `Continue(v)`, evaluates to `v`
3. If `Break(e)`, returns `Err(e)` from the enclosing function

---

### 3. Option Implementation

```spl
impl Try for Option(T: T) where T {
    type Output = T;
    type Residual = ();      // None carries no information

    fn branch(self): ControlFlow(C: T, B: ()) {
        match self {
            Some(v) => .Continue(v),
            None => .Break(()),
        }
    }

    fn from_output(output: T): Self {
        return Some(output);
    }
}
```

Usage in Option-returning functions:

```spl
fn find_user_email(users: &[User], id: UserId): String? {
    let user = users.iter().find(|u| u.id == id)!;  // Early return None
    let email = user.email.clone()!;                 // Early return None
    return Some(email);
}
```

---

### 4. Cross-Type Conversion with FromResidual

When using `!` in a function returning a different Try type, conversion must occur. This is handled by `FromResidual`:

```spl
trait FromResidual(R) {
    fn from_residual(residual: R): Self;
}
```

#### Option to Result Conversion

```spl
impl FromResidual(R: ()) for Result(T: T, E: E) where T, E: Default {
    fn from_residual(residual: ()): Self {
        return Err(E.default());
    }
}
```

This allows:

```spl
fn process(): Result(T: i32, E: Error) {
    let x: i32? = get_optional();
    let value = x!;  // None becomes Err(Error.default())
    return Ok(value * 2);
}
```

#### Explicit Conversion (Preferred)

For clarity and control over the error value, explicit conversion is preferred:

```spl
fn process(): Result(T: i32, E: Error) {
    let x: i32? = get_optional();
    let value = x.ok_or(Error.NotFound)!;  // Explicit error
    return Ok(value * 2);
}
```

**Standard conversion methods on Option:**

```spl
impl Option(T: T) where T {
    // Convert to Result with explicit error
    fn ok_or(self, err: E): Result(T: T, E: E) where E {
        match self {
            Some(v) => Ok(v),
            None => Err(err),
        }
    }

    // Convert to Result with lazy error
    fn ok_or_else(self, f: fn(): E): Result(T: T, E: E) where E {
        match self {
            Some(v) => Ok(v),
            None => Err(f()),
        }
    }
}
```

#### Result Error Conversion

For converting between Result types with different error types:

```spl
impl FromResidual(R: E1) for Result(T: T, E: E2) where T, E1, E2, E2: From(T: E1) {
    fn from_residual(residual: E1): Self {
        return Err(E2.from(residual));
    }
}
```

This enables:

```spl
fn read_and_parse(path: &str): Result(T: Config, E: AppError) {
    // IoError automatically converts to AppError via From trait
    let contents = fs.read_to_string(path)!;
    // ParseError automatically converts to AppError via From trait
    let config = parse(contents)!;
    return Ok(config);
}
```

---

### 5. Early Return Semantics

The `!` operator is syntactic sugar for a match expression with early return:

```spl
// This:
let value = expr!;

// Desugars to:
let value = match Try.branch(expr) {
    .Continue(v) => v,
    .Break(r) => return FromResidual.from_residual(r),
};
```

Key points:
- `!` always triggers an **early return** from the enclosing function
- The return type must implement `FromResidual` for the residual type
- Type inference determines which `FromResidual` impl to use

---

### 6. Interaction with Blocks and Closures

#### Labeled Blocks (Future Consideration)

For returning from a block rather than the function:

```spl
let result = 'block: {
    let x = operation()!'block;  // Returns from block, not function
    yield process(x);
};
```

This is **not in the initial design** but reserved for future consideration.

#### Closures

`!` in closures returns from the closure, not the enclosing function:

```spl
fn process_items(items: Vec(T: Item)): Result(T: Vec(T: Output), E: Error) {
    // ! returns from the closure, which is correct here
    let outputs = items.iter()
        .map(|item| {
            let processed = transform(item)!;
            return Ok(processed);
        })
        .collect()!;  // collect handles the Result from each iteration
    return Ok(outputs);
}
```

---

### 7. Interaction with Async (No Function Coloring)

Since SPL has no function coloring (see ADR-013), `!` works identically in all contexts:

```spl
use std.task.spawn;

fn fetch_data(url: String): Result(T: Data, E: Error) {
    let response = http.get(url)!;  // May yield, ! works normally
    let data = parse(response.body())!;
    return Ok(data);
}

fn main(): () {
    let handle = spawn(|| fetch_data("https://api.example.com"));
    match handle.try_await() {
        Ok(data) => process(data),
        Err(e) => log_error(e),
    }
}
```

No special `!` handling is needed for async code because:
- Any function can yield (no async/await keywords)
- `!` semantics are purely about control flow, not execution model
- Task boundaries are explicit via `spawn()`, not implicit in `!`

---

### 8. Custom Try Implementations

Users can implement Try for custom types:

```spl
enum Response{
    Success(T),
    ClientError(u16, String),
    ServerError(u16, String),
} where T

impl Try for Response(T: T) where T {
    type Output = T;
    type Residual = (u16, String, bool);  // (code, message, is_server_error)

    fn branch(self): ControlFlow(C: T, B: Self.Residual) {
        match self {
            Success(v) => .Continue(v),
            ClientError(code, msg) => .Break((code, msg, false)),
            ServerError(code, msg) => .Break((code, msg, true)),
        }
    }

    fn from_output(output: T): Self {
        return Success(output);
    }
}

impl FromResidual(R: (u16, String, bool)) for Response(T: T) where T {
    fn from_residual(r: (u16, String, bool)): Self {
        let (code, msg, is_server) = r;
        if is_server {
            return ServerError(code, msg);
        } else {
            return ClientError(code, msg);
        }
    }
}
```

---

### 9. ControlFlow Type

`ControlFlow` is a general-purpose enum for control flow decisions, also useful for iteration (see ADR-011):

```spl
enum ControlFlow{
    Continue(C),
    Break(B),
} where C, B

impl ControlFlow(C: C, B: B) where C, B {
    fn is_continue(&self): bool {
        match self {
            Continue(_) => true,
            Break(_) => false,
        }
    }

    fn is_break(&self): bool {
        return !self.is_continue();
    }

    fn continue_value(self): C? {
        match self {
            Continue(c) => Some(c),
            Break(_) => None,
        }
    }

    fn break_value(self): B? {
        match self {
            Continue(_) => None,
            Break(b) => Some(b),
        }
    }
}
```

---

### 10. Prelude and Imports

The following are in the prelude (no import needed):

```spl
// Types
Option, Result, ControlFlow

// Variants (usable without qualification)
Some, None, Ok, Err, Continue, Break

// Traits
Try, FromResidual
```

---

## Rationale

### Why Separate Try and FromResidual?

Rust's design insight: the type you're *coming from* (`Try::branch`) is different from the type you're *converting to* (`FromResidual::from_residual`). This separation enables:

- Option `!` in Result functions (converts `()` residual to error)
- Result `!` with different error types (converts via `From`)
- Custom types mixing with standard types

### Why ControlFlow Instead of Either?

`ControlFlow` with `Continue`/`Break` naming is more intuitive for control flow operations than generic `Left`/`Right`. The same type works for:

- `!` operator (Try trait)
- `try_fold` and similar iteration methods
- Early exit from any computation

### Why Explicit ok_or is Preferred?

Implicit Option-to-Result conversion via `FromResidual` requires a `Default` error or similar. This can hide the actual failure reason:

```spl
// Implicit: What went wrong?
let value = maybe_value!;  // Error is generic "default"

// Explicit: Clear about the failure
let value = maybe_value.ok_or(Error.ConfigNotFound)!;
```

SPL provides both options but documentation should encourage explicit conversion.

### Why No Labeled Block Returns Initially?

Labeled blocks for `!` (`expr!'label`) add complexity. The common case (early function return) is well-served by the simple design. Labeled blocks can be added later if there's demonstrated need.

---

## Consequences

### Positive

- Unified `!` for Option and Result
- Type-safe error conversion via `From` trait
- Custom types can participate in error propagation
- No async-specific handling needed
- Clear desugaring model

### Negative

- Two traits (Try, FromResidual) instead of one
- Implicit conversions can hide error sources
- More complex type inference for cross-type `!`

### Migration from Rust

Rust developers will find the model familiar but simpler:
- No `#![feature(try_trait_v2)]` needed
- SPL's Try is close to Rust's stabilized design
- Same patterns (`!`, `ok_or`, `From` for errors) work

---

## Examples

### Basic Error Propagation

```spl
fn load_config(): Result(T: Config, E: Error) {
    let path = env.var("CONFIG_PATH").ok_or(Error.MissingEnv("CONFIG_PATH"))!;
    let contents = fs.read_to_string(path)!;
    let config = toml.parse(contents)!;
    return Ok(config);
}
```

### Error Type Unification

```spl
enum AppError{
    Io(IoError),
    Parse(ParseError),
    Validation(String),
}

impl From(T: IoError) for AppError {
    fn from(e: IoError): Self {
        return AppError.Io(e);
    }
}

impl From(T: ParseError) for AppError {
    fn from(e: ParseError): Self {
        return AppError.Parse(e);
    }
}

fn process_file(path: &str): Result(T: Data, E: AppError) {
    let contents = fs.read_to_string(path)!;  // IoError -> AppError
    let parsed = json.parse(contents)!;       // ParseError -> AppError

    if !validate(parsed) {
        return Err(AppError.Validation("invalid data"));
    }

    return Ok(transform(parsed));
}
```

### Option Chaining

```spl
fn get_user_city(db: &Database, user_id: UserId): String? {
    let user = db.find_user(user_id)!;
    let address = user.address!;
    let city = address.city.clone()!;
    return Some(city);
}
```

### Mixing Option and Result

```spl
fn fetch_optional_config(): Result(T: Config, E: Error) {
    // Explicit conversion preferred
    let path = env.var("OPTIONAL_CONFIG").ok_or(Error.NoConfig)!;
    let contents = fs.read_to_string(path)!;
    return toml.parse(contents);
}
```

---

## Open Questions

1. **NeverShortCircuit** - Should there be a "never fails" wrapper for infallible operations in `!` chains?
2. **try blocks** - Should SPL support `try { }` blocks that collect `!` results?
3. **Error context** - Should there be built-in support for error context/wrapping (like anyhow's `.context()`)?

Note: Optional chaining (`?.`) and nullish coalescing (`??`) have been added to SPL. See [error-handling.md](../spec/error-handling.md) for details. The `!` operator is for try/propagate (early return), while `?.` chains through `None` without early return.

---

## References

- [ADR-011: Iteration and Generators](011-iteration-and-generators.md)
- [ADR-013: Concurrency Model](013-async-await.md)
- [Rust RFC 3058: try_trait_v2](https://rust-lang.github.io/rfcs/3058-try-trait-v2.html)
- [Rust ControlFlow](https://doc.rust-lang.org/std/ops/enum.ControlFlow.html)
