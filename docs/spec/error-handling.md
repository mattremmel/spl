# SPL Error Handling

This document specifies error handling in SPL, including the `!` operator, the `Try` trait, optional chaining (`?.`), nullish coalescing (`??`), and cross-type error conversion.

## Overview

SPL uses the `Result` and `Option` types for error handling, with the `!` operator for concise error propagation. The `Try` trait enables `!` to work uniformly across these types and user-defined types.

**Key Concepts:**

- **`Result(T: T, E: E)`**: Represents success (`Ok(T)`) or failure (`Err(E)`)
- **`Option(T: T)`**: Represents presence (`Some(T)`) or absence (`None`)
- **`!` operator**: Early return on failure, unwrap on success (postfix try)
- **`?.` operator**: Optional chaining - access field/method if Some, else None
- **`??` operator**: Nullish coalescing - unwrap or use default
- **`Try` trait**: Enables `!` for any conforming type
- **`FromResidual` trait**: Enables cross-type error conversion

---

## 1. Core Types

### Result

```spl
enum Result{
    Ok(T),
    Err(E),
} where T, E
```

Represents an operation that may succeed with a value of type `T` or fail with an error of type `E`.

### Option

```spl
enum Option{
    Some(T),
    None,
} where T
```

Represents an optional value that may be present (`Some`) or absent (`None`).

### ControlFlow

```spl
enum ControlFlow{
    Continue(C),
    Break(B),
} where C, B
```

Represents a control flow decision: continue execution with a value, or break/return early with a value. Used internally by the `Try` trait.

---

## 2. The `!` Operator (Try/Propagate)

The `!` operator provides concise error propagation. When applied to a `Try` type:

- On success: extracts and returns the inner value
- On failure: returns early from the enclosing function with the error

### Basic Usage

```spl
fn read_config(path: &str): Result(T: Config, E: IoError) {
    let contents = fs.read_to_string(path)!;  // Early return on Err
    let config = parse_config(contents)!;     // Early return on Err
    return Ok(config);
}

fn find_user_email(users: &[User], id: UserId): String? {
    let user = users.iter().find(|u| u.id == id)!;  // Early return None
    let email = user.email.clone()!;                 // Early return None
    return Some(email);
}
```

### Desugaring

The `!` operator desugars to a match with early return:

```spl
// This:
let value = expr!;

// Desugars to:
let value = match Try.branch(expr) {
    Continue(v) => v,
    Break(r) => return FromResidual.from_residual(r),
};
```

### Precedence

The `!` operator is a **postfix** operator at the same precedence level as `.`, `?.`, `()`, and `[]`. It binds tighter than all binary operators:

```spl
foo.bar()!.baz()     // ((foo.bar())!).baz() - try, then method call
result! + 1          // (result!) + 1 - try binds to result
a && b!              // a && (b!) - try binds to b
items.get(0)!        // (items.get(0))! - try the result of get
```

See [syntax-grammar.md](syntax-grammar.md) for the full precedence table.

---

## 3. Optional Chaining (`?.`)

The `?.` operator provides safe navigation through optional values. When applied to an `Option`:

- On `Some(v)`: accesses the field/method on `v`, wrapping result in `Some`
- On `None`: short-circuits and returns `None`

### Basic Usage

```spl
fn get_email(user: User?): String? {
    return user?.email;           // None if user is None
}

fn get_manager_name(user: User?): String? {
    return user?.manager?.name;   // Chain through multiple optionals
}

fn call_method(obj: Service?): Response? {
    return obj?.process();        // Method call on optional
}
```

### Desugaring

```spl
// This:
let email = user?.email;

// Desugars to:
let email = match user {
    Some(u) => Some(u.email),
    None => None,
};
```

### Extracting with `!` vs Chaining with `?.`

| Operator | Input | Output | On None |
|----------|-------|--------|---------|
| `!` | `T?` | `T` | Early return from function |
| `?.` | `T?` | `U?` | Propagates None in expression |

```spl
fn example(user: User?): String throws Error {
    // ! extracts value or returns early
    let u = user!;              // u: User, returns if None

    // ?. chains through optionals
    let name = user?.name;      // name: String?, None propagates

    return u.name;
}
```

---

## 4. Nullish Coalescing (`??`)

The `??` operator provides a default value when an `Option` is `None`:

```spl
let name = user?.name ?? "Anonymous";
let count = map.get(key) ?? 0;
let config = load_config() ?? Config.default();
```

### Desugaring

```spl
// This:
let name = expr ?? default;

// Desugars to:
let name = match expr {
    Some(v) => v,
    None => default,
};
```

### Precedence

`??` has lower precedence than `||`, allowing:

```spl
let value = config.primary ?? config.fallback ?? default;  // Right-associative chain
let flag = opt_bool ?? false || other_condition;           // ?? binds tighter than ||
```

### Lazy Evaluation

The `??` operator uses **lazy evaluation**: the right-hand side is only evaluated if the left-hand side is `None`. This matches `||` behavior and enables efficient patterns:

```spl
let config = cached ?? load_from_disk();      // load_from_disk() only called if cached is None
let user = find_by_id(id) ?? create_default(); // create_default() only called if not found
```

### Comparison with Methods

| Syntax | Equivalent Method |
|--------|-------------------|
| `opt ?? default` | `opt.unwrap_or_else(\|\| default)` |

---

## 5. The Try Trait

The `Try` trait defines how a type participates in `!` operations.

```spl
trait Try {
    type Output;     // The success value type
    type Residual;   // The early-return value type

    /// Extract success value or return residual for early return
    fn branch(self): ControlFlow(C: Self.Output, B: Self.Residual);

    /// Construct success case from output
    fn from_output(output: Self.Output): Self;
}
```

### Result Implementation

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

### Option Implementation

```spl
impl Try for Option(T: T) where T {
    type Output = T;
    type Residual = ();  // None carries no information

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

---

## 6. Cross-Type Conversion

When using `!` in a function returning a different `Try` type, conversion happens via `FromResidual`.

### The FromResidual Trait

```spl
trait FromResidual(R) {
    fn from_residual(residual: R): Self;
}
```

### Result Error Conversion

Results with different error types convert automatically if the target error implements `From`:

```spl
impl FromResidual(R: E1) for Result(T: T, E: E2) where T, E1, E2, E2: From(T: E1) {
    fn from_residual(residual: E1): Self {
        return Err(E2.from(residual));
    }
}
```

This enables:

```spl
fn process(path: &str): Result(T: Data, E: AppError) {
    // IoError converts to AppError via From trait
    let contents = fs.read_to_string(path)!;
    // ParseError converts to AppError via From trait
    let data = parse(contents)!;
    return Ok(data);
}
```

### Option to Result Conversion

Options can be used with `!` in Result-returning functions:

```spl
impl FromResidual(R: ()) for Result(T: T, E: E) where T, E: Default {
    fn from_residual(residual: ()): Self {
        return Err(E.default());
    }
}
```

**Preferred: Explicit conversion** for clarity about the error value:

```spl
fn process(): Result(T: i32, E: Error) {
    let x: i32? = get_optional();
    let value = x.ok_or(Error.NotFound)!;  // Explicit error
    return Ok(value * 2);
}
```

### Conversion Methods on Option

```spl
impl Option(T: T) where T {
    /// Convert to Result with explicit error
    fn ok_or(self, err: E): Result(T: T, E: E) where E {
        match self {
            Some(v) => Ok(v),
            None => Err(err),
        }
    }

    /// Convert to Result with lazy error
    fn ok_or_else(self, f: fn(): E): Result(T: T, E: E) where E {
        match self {
            Some(v) => Ok(v),
            None => Err(f()),
        }
    }
}
```

---

## 7. Error Type Unification

A common pattern is defining an application error type that unifies multiple error sources:

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

---

## 8. Closures and `!`

The `!` operator in closures returns from the closure, not the enclosing function:

```spl
fn process_items(items: Vec(T: Item)): Result(T: Vec(T: Output), E: Error) {
    // ! returns from the closure
    let outputs = items.iter()
        .map(|item| {
            let processed = transform(item)!;
            return Ok(processed);
        })
        .collect()!;  // collect handles the Result from each closure
    return Ok(outputs);
}
```

---

## 9. Custom Try Implementations

Users can implement `Try` for custom types:

```spl
enum Response{
    Success(T),
    ClientError(u16, String),
    ServerError(u16, String),
} where T

impl Try for Response(T: T) where T {
    type Output = T;
    type Residual = (u16, String, bool);  // (code, message, is_server)

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

## 10. ControlFlow Methods

```spl
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

## 11. Prelude

The following are in the prelude (no import needed):

**Types:**
- `Option`, `Result`, `ControlFlow`

**Variants:**
- `Some`, `None`, `Ok`, `Err`, `Continue`, `Break`

**Traits:**
- `Try`, `FromResidual`

---

## 12. The `throws` Keyword

The `throws` keyword provides syntactic sugar for functions that return `Result`, making error-prone functions more visible in signatures and reducing boilerplate.

### Typed `throws`

A function declared with `throws ErrorType` returns `Result(T: ReturnType, E: ErrorType)`:

```spl
// These two declarations are equivalent:
fn read_file(path: &str): String throws IoError { ... }
fn read_file(path: &str): Result(T: String, E: IoError) { ... }
```

**Desugaring:**

| Declaration | Equivalent |
|-------------|------------|
| `fn foo(): T throws E` | `fn foo(): Result(T: T, E: E)` |
| `fn foo() throws E` | `fn foo(): Result(T: (), E: E)` |
| `fn foo(): T throws` | `fn foo(): Result(T: T, E: Error)` |
| `fn foo() throws` | `fn foo(): Result(T: (), E: Error)` |

### Untyped `throws`

When no error type is specified, the function uses the `Error` trait object type, enabling any error to be returned (similar to Rust's `anyhow`):

```spl
fn process_data(input: &str): Data throws {
    let parsed = parse(input)!;      // ParseError converted to Error
    let validated = validate(parsed)!; // ValidationError converted to Error
    return transform(validated);
}
```

**Note:** Untyped `throws` uses a boxed error type that can hold any error implementing the `Error` trait. The `Error` trait is defined in the standard library (see [standard-library.md](standard-library.md)) and provides common error interface methods like `message()` and `source()`.

### Implicit `Ok` Wrapping

In a `throws` function, return values are automatically wrapped in `Ok`:

```spl
fn parse_number(s: &str): i32 throws ParseError {
    if s.is_empty() {
        throw .Empty;
    }
    return s.parse_int();  // Automatically wrapped: Ok(s.parse_int())
}

// Equivalent explicit version:
fn parse_number(s: &str): Result(T: i32, E: ParseError) {
    if s.is_empty() {
        return Err(.Empty);
    }
    return Ok(s.parse_int());
}
```

**Rules:**
- `return value;` in a `throws` function desugars to `return Ok(value);`
- `return;` in a `throws` function (unit return) desugars to `return Ok(());`
- Single-expression function bodies are wrapped: `fn foo(): i32 throws E { 42 }` returns `Ok(42)`
- This follows SPL's general rule: single-expression blocks have implicit values, multi-statement blocks require explicit `return`/`break`

---

## 13. The `throw` Keyword

The `throw` keyword provides concise syntax for returning errors, analogous to `bail!()` in Rust's anyhow:

```spl
throw error_value;
// Desugars to:
return Err(error_value);
```

### Basic Usage

```spl
fn divide(a: i32, b: i32): i32 throws MathError {
    if b == 0 {
        throw .DivisionByZero;
    }
    return a / b;
}
```

### With Error Construction

```spl
fn validate_age(age: i32): () throws ValidationError {
    if age < 0 {
        throw .Invalid("age cannot be negative");
    }
    if age > 150 {
        throw .Invalid("age seems unrealistic");
    }
}
```

### In Untyped `throws` Functions

When using untyped `throws`, any error type can be thrown:

```spl
fn process(path: &str): Data throws {
    if !path.exists() {
        throw IoError.NotFound(path);  // IoError converted to Error
    }
    let content = read_file(path)!;
    if content.is_empty() {
        throw ValidationError.Empty;   // ValidationError converted to Error
    }
    return parse(content)!;
}
```

### `throw` vs `!` Operator

| Use Case | Syntax | When to Use |
|----------|--------|-------------|
| Propagate existing error | `operation()!` | Calling fallible functions |
| Create and return new error | `throw .Foo` | Validation, guards, custom errors |

```spl
fn process_file(path: &str): Config throws ConfigError {
    // Use ! to propagate errors from called functions
    let content = read_file(path)!;

    // Use throw to create new errors
    if content.is_empty() {
        throw .EmptyFile(path);
    }

    return parse_config(content)!;
}
```

### Closures and `throw`

In closures, `throw` desugars to `return Err(expr)` from the **closure body**, not the enclosing function. This matches the behavior of `!` in closures (section 8). A closure that uses `throw` must have a `Result` return type:

```spl
fn validate_all(items: Vec(T: Item)): Result(T: Vec(T: Item), E: Error) {
    // throw inside the closure returns Err from the closure, not from validate_all
    let validated = items.iter()
        .map(|item| {
            if !item.is_valid() {
                throw ValidationError.Invalid(item.id);  // returns Err from closure
            }
            return item.clone();
        })
        .collect()!;  // ! propagates the Err to validate_all
    return Ok(validated);
}
```

---

## 14. Error Type Conversion with `throws`

### Typed `throws` with Conversion

When using typed `throws`, errors from called functions are converted via the `From` trait, just like with explicit `Result`:

```spl
enum AppError{
    Io(IoError),
    Parse(ParseError),
}

impl From(T: IoError) for AppError { ... }
impl From(T: ParseError) for AppError { ... }

fn load_config(path: &str): Config throws AppError {
    let content = read_file(path)!;   // IoError -> AppError via From
    let config = parse(content)!;      // ParseError -> AppError via From
    return config;
}
```

### Untyped `throws` Conversion

With untyped `throws`, any error implementing the `Error` trait is automatically converted:

```spl
fn load_and_process(path: &str): Result throws {
    let config = load_config(path)!;  // AppError -> Error
    let data = fetch_data(config)!;   // NetworkError -> Error
    return process(data)!;            // ProcessError -> Error
}
```

---

## Summary

| Feature | Description |
|---------|-------------|
| `Result(T: T, E: E)` | Success or error type |
| `Option(T: T)` | Present or absent type |
| `expr!` | Early return on failure, unwrap on success (try/propagate) |
| `expr?.field` | Optional chaining - access if Some, else None |
| `expr ?? default` | Nullish coalescing - unwrap or use default |
| `Try` trait | Enables `!` for a type |
| `FromResidual` trait | Enables cross-type `!` conversion |
| `ok_or(err)` | Convert Option to Result with explicit error |
| `From` trait | Enables automatic error type conversion |
| `throws E` | Sugar for `Result(T: T, E: E)` return type |
| `throws` | Sugar for `Result(T: T, E: Error)` (any error) |
| `throw expr` | Sugar for `return Err(expr)` |

### Common Patterns

```spl
// Basic error propagation
let value = fallible_operation()!;

// Optional chaining
let name = user?.profile?.name;

// Nullish coalescing
let name = user?.name ?? "Anonymous";

// Option to Result with explicit error
let value = optional_value.ok_or(Error.NotFound)!;

// Error type unification via From
impl From(T: SourceError) for TargetError { ... }

// Using throws for cleaner signatures
fn process(input: &str): Output throws ProcessError {
    let parsed = parse(input)!;
    if !valid(parsed) {
        throw .Invalid;
    }
    return transform(parsed);
}
```

---

## References

- [ADR-014: Try Trait and Error Propagation](../designs/014-try-trait.md) - Design rationale
- [traits.md](traits.md) - Try and FromResidual traits
- [syntax-grammar.md](syntax-grammar.md) - Operator precedence
- [standard-library.md](standard-library.md) - Option and Result types
