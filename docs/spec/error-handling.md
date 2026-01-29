# SPL Error Handling

This document specifies error handling in SPL, including the `?` operator, the `Try` trait, and cross-type error conversion.

## Overview

SPL uses the `Result` and `Option` types for error handling, with the `?` operator for concise error propagation. The `Try` trait enables `?` to work uniformly across these types and user-defined types.

**Key Concepts:**

- **`Result(T: T, E: E)`**: Represents success (`Ok(T)`) or failure (`Err(E)`)
- **`Option(T: T)`**: Represents presence (`Some(T)`) or absence (`None`)
- **`?` operator**: Early return on failure, unwrap on success
- **`Try` trait**: Enables `?` for any conforming type
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

## 2. The `?` Operator

The `?` operator provides concise error propagation. When applied to a `Try` type:

- On success: extracts and returns the inner value
- On failure: returns early from the enclosing function with the error

### Basic Usage

```spl
fn read_config(path: &str): Result(T: Config, E: IoError) {
    let contents = fs.read_to_string(path)?;  // Early return on Err
    let config = parse_config(contents)?;     // Early return on Err
    return Ok(config);
}

fn find_user_email(users: &[User], id: UserId): Option(T: String) {
    let user = users.iter().find(|u| u.id == id)?;  // Early return None
    let email = user.email.clone()?;                 // Early return None
    return Some(email);
}
```

### Desugaring

The `?` operator desugars to a match with early return:

```spl
// This:
let value = expr?;

// Desugars to:
let value = match Try.branch(expr) {
    ControlFlow.Continue(v) => v,
    ControlFlow.Break(r) => return FromResidual.from_residual(r),
};
```

---

## 3. The Try Trait

The `Try` trait defines how a type participates in `?` operations.

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
            Ok(v) => ControlFlow.Continue(v),
            Err(e) => ControlFlow.Break(e),
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
            Some(v) => ControlFlow.Continue(v),
            None => ControlFlow.Break(()),
        }
    }

    fn from_output(output: T): Self {
        return Some(output);
    }
}
```

---

## 4. Cross-Type Conversion

When using `?` in a function returning a different `Try` type, conversion happens via `FromResidual`.

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
    let contents = fs.read_to_string(path)?;
    // ParseError converts to AppError via From trait
    let data = parse(contents)?;
    return Ok(data);
}
```

### Option to Result Conversion

Options can be used with `?` in Result-returning functions:

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
    let x: Option(T: i32) = get_optional();
    let value = x.ok_or(Error.NotFound)?;  // Explicit error
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

## 5. Error Type Unification

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
    let contents = fs.read_to_string(path)?;  // IoError -> AppError
    let parsed = json.parse(contents)?;       // ParseError -> AppError

    if !validate(parsed) {
        return Err(AppError.Validation("invalid data"));
    }

    return Ok(transform(parsed));
}
```

---

## 6. Closures and `?`

The `?` operator in closures returns from the closure, not the enclosing function:

```spl
fn process_items(items: Vec(T: Item)): Result(T: Vec(T: Output), E: Error) {
    // ? returns from the closure
    let outputs = items.iter()
        .map(|item| {
            let processed = transform(item)?;
            return Ok(processed);
        })
        .collect()?;  // collect handles the Result from each closure
    return Ok(outputs);
}
```

---

## 7. Custom Try Implementations

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
            Success(v) => ControlFlow.Continue(v),
            ClientError(code, msg) => ControlFlow.Break((code, msg, false)),
            ServerError(code, msg) => ControlFlow.Break((code, msg, true)),
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

## 8. ControlFlow Methods

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

    fn continue_value(self): Option(T: C) {
        match self {
            Continue(c) => Some(c),
            Break(_) => None,
        }
    }

    fn break_value(self): Option(T: B) {
        match self {
            Continue(_) => None,
            Break(b) => Some(b),
        }
    }
}
```

---

## 9. Prelude

The following are in the prelude (no import needed):

**Types:**
- `Option`, `Result`, `ControlFlow`

**Variants:**
- `Some`, `None`, `Ok`, `Err`, `Continue`, `Break`

**Traits:**
- `Try`, `FromResidual`

---

## Summary

| Feature | Description |
|---------|-------------|
| `Result(T: T, E: E)` | Success or error type |
| `Option(T: T)` | Present or absent type |
| `expr?` | Early return on failure, unwrap on success |
| `Try` trait | Enables `?` for a type |
| `FromResidual` trait | Enables cross-type `?` conversion |
| `ok_or(err)` | Convert Option to Result with explicit error |
| `From` trait | Enables automatic error type conversion |

### Common Patterns

```spl
// Basic error propagation
let value = fallible_operation()?;

// Option to Result with explicit error
let value = optional_value.ok_or(Error.NotFound)?;

// Error type unification via From
impl From(T: SourceError) for TargetError { ... }
```
