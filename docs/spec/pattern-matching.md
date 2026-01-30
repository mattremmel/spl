# Pattern Matching

This document specifies pattern matching in SPL, including pattern syntax, matching semantics, exhaustiveness checking, and binding modes.

## Overview

Patterns appear in several contexts:
- `let` bindings: `let Point(x, y) = point;`
- `match` expressions: `match value { Some(x) => x, None => 0 }`
- `for` loops: `for (key, value) in map { ... }`
- `is` expressions: `if value is Some(x) { ... }`
- Function parameters (irrefutable only): `fn foo((a, b): (i32, i32)) { ... }`

## 1. Pattern Syntax

### 1.1 Identifier Patterns

```spl
let x = 42;              // Bind to x
let mut y = 42;          // Bind to mutable y
let _ = expensive();     // Wildcard: ignore value (drops immediately)
```

### 1.2 Literal Patterns

```spl
match n {
    0 => "zero",
    1 => "one",
    -1 => "negative one",
    _ => "other",
}

match c {
    'a' => "lowercase a",
    'A' => "uppercase a",
    _ => "other",
}

match flag {
    true => "yes",
    false => "no",
}
```

### 1.3 Range Patterns

```spl
match n {
    0..10 => "single digit",      // 0 to 9 (exclusive end)
    10..=99 => "two digits",      // 10 to 99 (inclusive end)
    100.. => "three or more",     // 100 and above (open end)
    _ => "negative",
}

match c {
    'a'..='z' => "lowercase",
    'A'..='Z' => "uppercase",
    '0'..='9' => "digit",
    _ => "other",
}
```

### 1.4 Tuple Patterns

```spl
let (x, y) = (1, 2);
let (a, _, c) = (1, 2, 3);       // Ignore middle element
let (first, ..) = (1, 2, 3, 4);  // Ignore rest
let (.., last) = (1, 2, 3, 4);   // Ignore all but last
let (first, .., last) = tuple;   // First and last only
```

### 1.5 Struct Patterns

```spl
struct Point(x: i32, y: i32)

let Point(x, y) = point;              // Destructure all fields
let Point(x, ..) = point;             // Destructure some, ignore rest
let Point(x: a, y: b) = point;        // Rename bindings
let Point(x, y: _) = point;           // Ignore specific field
```

**Field shorthand:** When the binding name matches the field name:
```spl
let Point(x, y) = point;  // Equivalent to Point(x: x, y: y)
```

### 1.6 Enum Patterns

```spl
enum Option{ Some(T), None } where T

match opt {
    Some(value) => use(value),
    None => default(),
}

enum Result{ Ok(T), Err(E) } where T, E

match result {
    Ok(data) => process(data),
    Err(e) => handle_error(e),
}

// Unit variants
enum Color{ Red, Green, Blue }

match color {
    Color.Red => "#ff0000",
    Color.Green => "#00ff00",
    Color.Blue => "#0000ff",
}
```

### 1.7 Slice Patterns

```spl
let [first, second, third] = arr;     // Exact length match
let [first, ..] = arr;                // At least one element
let [first, .., last] = arr;          // At least two elements
let [first, ..rest] = arr;            // Bind rest to slice
let [.., second_last, last] = arr;    // Last two elements
let [] = arr;                         // Empty slice
```

**Rest patterns can only appear once:**
```spl
let [..a, middle, ..b] = arr;  // ERROR: multiple rest patterns
```

### 1.8 Reference Patterns

```spl
fn process(r: &Point): () {
    let &Point(x, y) = r;  // Dereference and destructure
}

fn modify(r: &mut i32): () {
    let &mut value = r;    // Dereference mutable reference
}
```

### 1.9 Or Patterns

```spl
match value {
    1 | 2 | 3 => "small",
    4 | 5 | 6 => "medium",
    _ => "large",
}

match option {
    Some(0) | None => "empty or zero",
    Some(n) => format("value: {}", n),
}

// All alternatives must bind the same names with compatible types
match result {
    Ok(n) | Err(n) => use(n),  // OK if both n have same type
}
```

### 1.10 Grouped Patterns

Parentheses clarify precedence:
```spl
match value {
    (1 | 2) => "one or two",     // Or-pattern
    (x,) => "single tuple",       // 1-tuple (not grouped pattern)
}
```

---

## 2. Refutability

Patterns are either **refutable** (may fail to match) or **irrefutable** (always match).

### 2.1 Irrefutable Patterns

Required in contexts where matching must succeed:
- `let` bindings
- Function parameters
- `for` loop variables

```spl
// Irrefutable patterns
let x = 42;                      // Identifier always matches
let (a, b) = pair;               // Tuple always matches (given correct type)
let Point(x, y) = point;         // Struct always matches
let _ = value;                   // Wildcard always matches
```

### 2.2 Refutable Patterns

Can fail to match; required in:
- `match` arms
- `is` expressions

```spl
// Refutable patterns
Some(x)      // Fails if None
1 | 2 | 3    // Fails if not 1, 2, or 3
0..10        // Fails if outside range
```

### 2.3 Errors

```spl
// ERROR: refutable pattern in irrefutable context
let Some(x) = maybe_value;  // What if it's None?

// Correct: use match or is
match maybe_value {
    Some(x) => use(x),
    None => handle_none(),
}

// Or with is expression
if maybe_value is Some(x) {
    use(x);
}
```

---

## 3. The `is` Expression

The `is` operator combines pattern matching with boolean conditions.

### 3.1 Basic Usage

```spl
if value is Some(x) {
    // x is bound here
    use(x);
}
// x is not bound here

// Equivalent to:
match value {
    Some(x) => {
        use(x);
    },
    _ => {},
}
```

### 3.2 With Else

```spl
if value is Some(x) {
    use(x);
} else {
    handle_none();
}
```

### 3.3 Chained Conditions

```spl
if value is Some(x) && x > 0 {
    // x bound and positive
}

if a is Some(x) && b is Some(y) {
    // Both x and y bound
}
```

### 3.4 Negation

```spl
if value is !Some(_) {
    // value is None
}

// Equivalent to:
if !(value is Some(_)) {
    // ...
}
```

### 3.5 In While Loops

```spl
while iter.next() is Some(item) {
    process(item);
}
```

---

## 4. Match Expressions

### 4.1 Basic Syntax

```spl
let result = match value {
    pattern1 => expr1,
    pattern2 => expr2,
    _ => default_expr,
};
```

### 4.2 Match Guards

```spl
match value {
    Some(x) if x > 0 => "positive",
    Some(x) if x < 0 => "negative",
    Some(0) => "zero",
    None => "none",
}
```

**Guard scope:** Variables bound in the pattern are available in the guard:
```spl
match pair {
    (x, y) if x == y => "equal",
    (x, y) => "different",
}
```

### 4.3 Binding Modes in Match

```spl
match &option {
    Some(x) => {
        // x: &T (reference to inner value)
    },
    None => {},
}

match &mut option {
    Some(x) => {
        // x: &mut T (mutable reference to inner value)
    },
    None => {},
}
```

### 4.4 Match Ergonomics

When matching on a reference, binding modes adjust automatically:

```spl
let opt: &Option(T: String) = &Some("hello".to_string());

match opt {
    Some(s) => {
        // s: &String (automatically borrowed)
        println(s);
    },
    None => {},
}
```

To override and move out (if allowed):
```spl
match opt {
    &Some(ref s) => { /* s: &String */ },
    &None => {},
}
```

---

## 5. Exhaustiveness Checking

The compiler verifies that match expressions cover all possible values.

### 5.1 Complete Coverage

```spl
enum Color{ Red, Green, Blue }

match color {
    Color.Red => "red",
    Color.Green => "green",
    Color.Blue => "blue",
}  // OK: all variants covered
```

### 5.2 Wildcard Coverage

```spl
match color {
    Color.Red => "red",
    _ => "not red",
}  // OK: _ covers Green and Blue
```

### 5.3 Exhaustiveness Errors

```spl
match color {
    Color.Red => "red",
    Color.Green => "green",
}  // ERROR: non-exhaustive; missing Color.Blue
```

### 5.4 Integer Ranges

```spl
match byte {
    0..=127 => "low",
    128..=255 => "high",
}  // OK: all u8 values covered

match n: i32 {
    0 => "zero",
    1..=100 => "small positive",
    _ => "other",  // Required: i32 has too many values
}
```

### 5.5 Nested Exhaustiveness

```spl
match pair {
    (Some(x), Some(y)) => use(x, y),
    (Some(x), None) => use_x(x),
    (None, Some(y)) => use_y(y),
    (None, None) => neither(),
}  // OK: all 4 combinations covered
```

### 5.6 Guards and Exhaustiveness

Guards are not considered for exhaustiveness because they can have arbitrary runtime conditions:

```spl
match opt {
    Some(x) if x > 0 => "positive",
    Some(x) if x < 0 => "negative",
    // ERROR: non-exhaustive
    // Some(0) and None not covered
}

// Correct:
match opt {
    Some(x) if x > 0 => "positive",
    Some(x) if x < 0 => "negative",
    Some(_) => "zero",
    None => "none",
}
```

---

## 6. Bindings and Scope

### 6.1 Binding Scope in Match

Bindings are scoped to their match arm:

```spl
match value {
    Some(x) => {
        use(x);  // x available here
    },
    None => {
        // x not available here
    },
}
// x not available here
```

### 6.2 Binding Scope in Is

Bindings from `is` are available in the `then` branch only:

```spl
if value is Some(x) {
    use(x);  // x available
} else {
    // x not available
}
// x not available
```

### 6.3 Multiple Bindings

```spl
let Point(x, y) = point;
// Both x and y are bound

match tuple {
    (a, b, c) => {
        // a, b, c all bound
    },
}
```

### 6.4 Shadowing

Patterns can shadow outer bindings:

```spl
let x = 1;
match value {
    Some(x) => {
        // This x shadows outer x
        use(x);
    },
    None => {
        // Outer x still visible
        use(x);
    },
}
// Outer x visible again
```

---

## 7. Special Patterns

### 7.1 At-Patterns (@)

Bind a name to the entire matched value while also destructuring:

```spl
match value {
    opt @ Some(x) => {
        // opt: Option(T: T), x: T
        log("matched: {}", opt);
        use(x);
    },
    None => {},
}

match message {
    msg @ Message(id, payload @ Payload(..)) => {
        log_message(msg);
        process_id(id);
        forward_payload(payload);
    },
}
```

### 7.2 Const Patterns

Named constants can be used as patterns:

```spl
const MAX_SIZE: usize = 1024;

match size {
    0 => "empty",
    MAX_SIZE => "at maximum",
    _ => "other",
}
```

### 7.3 Path Patterns

Qualified paths for enum variants:

```spl
use other_module.Status;

match status {
    Status.Active => "active",
    Status.Inactive => "inactive",
}
```

---

## 8. Pattern Matching and Ownership

### 8.1 Move by Default

Matching moves non-Copy values:

```spl
let opt: Option(T: String) = Some("hello".to_string());

match opt {
    Some(s) => {
        // s: String (moved)
        use(s);
    },
    None => {},
}
// opt is no longer valid
```

### 8.2 Borrowing in Patterns

Use `ref` to borrow instead of move:

```spl
let opt: Option(T: String) = Some("hello".to_string());

match opt {
    Some(ref s) => {
        // s: &String (borrowed)
        println(s);
    },
    None => {},
}
// opt still valid
```

### 8.3 Mutable Borrowing

```spl
let mut opt: Option(T: String) = Some("hello".to_string());

match opt {
    Some(ref mut s) => {
        // s: &mut String
        s.push_str(" world");
    },
    None => {},
}
```

### 8.4 Automatic Reference Matching

When matching on `&T` or `&mut T`, the compiler automatically adjusts:

```spl
fn process(opt: &Option(T: String)): () {
    match opt {
        Some(s) => {
            // s: &String (automatic borrowing)
        },
        None => {},
    }
}
```

---

## 9. Type Inference in Patterns

### 9.1 Inferred Bindings

```spl
let (a, b) = (1, "hello");
// a: i32, b: &str (inferred)
```

### 9.2 Type Annotations

```spl
let (a, b): (i32, String) = (1, "hello".to_string());
```

### 9.3 Partial Annotations

```spl
let Point(x: i32, y) = point;  // Only x annotated
```

---

## 10. Summary

| Pattern | Example | Refutable? |
|---------|---------|------------|
| Identifier | `x`, `mut x` | No |
| Wildcard | `_` | No |
| Literal | `42`, `'a'`, `true` | Yes |
| Range | `0..10`, `'a'..='z'` | Yes |
| Tuple | `(a, b)`, `(x, ..)` | No |
| Struct | `Point(x, y)` | No |
| Enum | `Some(x)`, `None` | Yes |
| Slice | `[a, b]`, `[first, ..]` | Yes (length) |
| Reference | `&x`, `&mut x` | No |
| Or | `A \| B` | Depends |
| At | `x @ pattern` | Depends |

---

## References

- [syntax-grammar.md](syntax-grammar.md) - Pattern grammar definitions
- [type-system.md](type-system.md) - Type inference and binding
- [memory-model.md](memory-model.md) - Ownership and borrowing in patterns
