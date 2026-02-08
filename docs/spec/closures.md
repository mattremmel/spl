# SPL Closures

This document specifies closure syntax and capture semantics in SPL.

## Overview

Closures are anonymous functions that can capture variables from their enclosing scope. SPL's closure design leverages second-class references to eliminate lifetime complexity while providing explicit control over capture behavior.

**Key Principles:**

- **Escaping vs Non-escaping**: Different defaults based on whether the closure outlives its creation context
- **Move by default**: Escaping closures move non-Copy captures (no hidden allocations)
- **Borrow by default**: Non-escaping closures borrow non-Copy captures (zero-cost)
- **Explicit captures**: Use `@[...]` capture lists for explicit control over capture behavior

---

## 1. Syntax

### Basic Closures

```ebnf
ClosureExpr = [ "@" CaptureList ] ClosureParams ClosureBody ;

CaptureList = "[" [ Capture { "," Capture } [ "," ] ] "]" ;

Capture = IDENTIFIER                       (* shorthand: x means x: x *)
        | IDENTIFIER ":" Expression ;      (* explicit: name: expr *)

ClosureParams = "||"
              | "|" [ ClosureParamList ] "|" ;

ClosureParamList = ClosureParam { "," ClosureParam } [ "," ] ;

ClosureParam = [ "mut" ] IDENTIFIER [ ":" Type ] ;

ClosureBody = Block | Expression ;
```

### Examples

```spl
// No parameters
let greet = || { println("Hello"); };

// With parameters (types inferred)
let add = |a, b| a + b;

// With explicit parameter types
let add = |a: i32, b: i32| a + b;

// Multi-statement body (requires block)
let compute = |x| {
    let doubled = x * 2;
    return doubled + 1;
};

// Single-expression body (implicit return)
let square = |x| x * x;
```

---

## 2. Escaping vs Non-Escaping Closures

The compiler classifies closures based on whether they escape their creation context.

### Non-Escaping Closures

Non-escaping closures are used immediately and don't outlive the current scope:

- Passed to `map`, `filter`, `each`, etc.
- Called within the receiving function, then discarded
- Can borrow from enclosing scope (reference is temporary)

```spl
let data = vec![1, 2, 3];
let threshold = 10;

// Non-escaping: used immediately by filter, then discarded
let filtered = data.iter()
    .filter(|x| x > threshold)  // threshold borrowed
    .collect();

// data and threshold still valid
println(data.len());
```

### Escaping Closures

Escaping closures outlive their creation context:

- Stored in structs or collections
- Returned from functions
- Passed to `spawn`, async runtimes, etc.
- Must own their captures (move or clone)

```spl
fn make_adder(n: i32): fn(i32): i32 {
    // Escaping: returned from function
    // n is Copy, so it's copied into the closure
    return |x| x + n;
}

fn make_greeter(greeting: String): fn(String): String {
    // Escaping: greeting is moved into closure
    return |name| greeting + ", " + name;
    // greeting no longer valid here
}
```

### How Escaping is Determined

A closure is **escaping** if any of the following conditions hold:

1. **Returned from a function**: The closure is used as a return value
2. **Stored in a struct field or collection**: Assigned to a field of a non-scoped struct, or pushed into a `Vec`, `HashMap`, etc.
3. **Assigned to a variable with wider scope**: The variable outlives the closure's creation site (e.g., assigned to a variable declared in an outer scope)
4. **Passed to a function parameter typed as `fn(...)`** (owned function type) or a generic bound requiring `'static` or `Send`
5. **Passed to known escaping APIs**: `spawn`, stored in `Arc`, etc.

Otherwise the closure is **non-escaping** (borrows by default, zero-cost). Non-escaping closures are those passed to functions that consume them synchronously within the call (e.g., `map`, `filter`, `each`, `sort_by`, or `scope`'s `s.spawn()`).

**Implementation note:** The compiler performs this analysis at the point where the closure is created, based on how the closure value flows through the program. The analysis is conservative — if the compiler cannot prove a closure is non-escaping, it treats it as escaping.

---

## 3. Capture Semantics

### Capture Rules

| Type | Non-Escaping | Escaping (default) | Escaping (explicit) |
|------|--------------|-------------------|---------------------|
| Copy types | copy | copy | copy |
| Non-Copy types | borrow | **move** | `@[x: x.clone()]` = clone |

> **Copy and Clone:** All `Copy` types implicitly implement `Clone` with trivial (bitwise) semantics. Using `.clone()` in a capture expression on a Copy type is valid but redundant. A future lint may warn about unnecessary clones on Copy types.

### Explicit Capture Lists

Use `@[...]` before the closure parameters to explicitly control how variables are captured:

```spl
let data = Arc.new(vec![1, 2, 3]);

// Without capture list: data is moved (escaping default)
let f = || process(data);
// data no longer valid

// With capture list: data is cloned at capture time
let f = @[data: data.clone()] || process(data);
// data still valid (was cloned)
```

**Why clone in a capture list instead of `.clone()` in the body?**

Captures happen at closure *creation* time, but expressions in the body execute at *call* time:

```spl
let data = Arc.new(vec![1, 2, 3]);

// WRONG: clone happens every call
let f = || {
    process(data.clone());  // clones each time f() is called
};
f();  // clone #1
f();  // clone #2

// RIGHT: clone happens once at capture
let f = @[data: data.clone()] || {
    process(data);  // uses the single clone
};
f();  // no clone
f();  // no clone
```

### Capture by Reference

Use `&` in the capture expression to capture by reference:

```spl
let data = vec![1, 2, 3];

// Capture data by reference
let f = @[data: &data] || data.len();
// data still valid (was borrowed)
```

### Shorthand Captures

When the capture name matches the variable name, use the shorthand form:

```spl
// Shorthand: @[x] is equivalent to @[x: x] (moves x)
let f = @[x] |y| x + y;

// Multiple captures with mixed forms
let f = @[config: config.clone(), logger: &logger] || {
    logger.log("Starting with config: " + config.name);
};
```

---

## 4. Closure Traits

Closures implement traits based on how they use their captures:

```spl
trait FnOnce where Args {
    type Output;
    fn call_once(self, args: Args): Self.Output;
}

trait FnMut: FnOnce(Args) where Args {
    fn call_mut(&mut self, args: Args): Self.Output;
}

trait Fn: FnMut(Args) where Args {
    fn call(&self, args: Args): Self.Output;
}
```

**Hierarchy:** `Fn` ⊂ `FnMut` ⊂ `FnOnce`

### Trait Inference

The compiler determines which `Fn*` trait a closure implements based on how the closure body uses its captures:

| Usage of captures | Trait implemented |
|-------------------|-------------------|
| Only reads captures (or no captures) | `Fn` (and `FnMut`, `FnOnce`) |
| Mutates any capture | `FnMut` (and `FnOnce`, but not `Fn`) |
| Moves out of any capture (consumes it) | `FnOnce` only |

```spl
let data = vec![1, 2, 3];
let mut count = 0;

// Implements Fn - only reads captures
let get_len = || data.len();

// Implements FnMut - mutates captures
let increment = || { count += 1; };

// Implements FnOnce - consumes captures
let consume = || take_ownership(data);
```

### Parameter Type Inference

Closure parameter types are inferred from the expected type context. When a closure appears where a specific function type is expected, the compiler infers parameter and return types from that context:

```spl
// Parameter type inferred from Vec.map's signature
let nums = vec![1, 2, 3];
let doubled = nums.iter().map(|x| x * 2);  // x: &i32 inferred from Iterator.Item

// Inferred from explicit fn type annotation
let f: fn(i32, i32): i32 = |a, b| a + b;  // a: i32, b: i32 inferred

// Inferred from function parameter type
fn apply(f: fn(i32): String, x: i32): String { return f(x); }
apply(|n| format("{}", n), 42);  // n: i32 inferred from parameter
```

If no expected type context is available and parameter types are not annotated, the compiler reports a type error.

### Capture Mode Inference

When no explicit capture list (`@[...]`) is provided, the compiler infers how each captured variable is used:

1. **Immutable borrow** (`&T`): The capture is only read (e.g., `x.len()`, `x + 1`)
2. **Mutable borrow** (`&mut T`): The capture is mutated (e.g., `x += 1`, `x.push(v)`)
3. **Move**: The capture is moved out of (e.g., passed to a function taking ownership)

For **non-escaping closures**, the compiler uses the least restrictive mode: borrow if possible, move only if required. For **escaping closures**, non-Copy types are always moved (since the closure must own its captures to outlive the creation scope). Copy types are always copied regardless of escaping classification.

An explicit capture list (`@[...]`) overrides inference for the listed variables. Variables not listed in the capture list use the default inference rules.

### Function Types and the Fn Hierarchy

The `fn(...)` type represents any callable with a matching signature, including named functions and closures that implement `Fn` (read-only captures or no captures):

```spl
// Non-capturing closure
let add: fn(i32, i32): i32 = |a, b| a + b;

// Capturing Fn closure - valid (read-only capture)
let x = 10;
let add_x: fn(i32): i32 = |a| a + x;

// Named function
fn double(n: i32): i32 { return n * 2; }
let f: fn(i32): i32 = double;
```

### 4.1 Calling Convention Safety

Only closures implementing `Fn` (which only read their captures) may be assigned to the `fn(...)` type. Closures that mutate captures (`FnMut`) or consume captures (`FnOnce`) **cannot** be assigned to `fn(...)`:

```spl
let mut count = 0;

// ERROR: FnMut closure cannot be assigned to fn type
// let inc: fn(): i32 = || { count += 1; count };

// ERROR: FnOnce closure cannot be assigned to fn type
// let data = vec![1, 2, 3];
// let consume: fn(): Vec(T: i32) = || take_ownership(data);
```

**Rationale:** A `fn` value can be called multiple times, but an `FnOnce` closure is only safe to call once (it consumes captures). An `FnMut` closure requires exclusive `&mut` access to its captures on each call, which cannot be guaranteed through a shared `fn` value. Restricting `fn` to `Fn` closures prevents these soundness issues.

**Compiler error:**
```
error: cannot assign FnMut closure to `fn` type
  --> src/main.spl:4:30
  |
4 |     let inc: fn(): i32 = || { count += 1; count };
  |                              ^^^^^^^^^^^^^^^^^^^^^^
  |                              closure mutates capture `count`
  |
  = note: `fn` types require `Fn` closures (read-only captures)
  = help: consider using a generic parameter `F: FnMut(): i32` instead
```

**Representation:**

| Closure kind | Representation when assigned to `fn` |
|---|---|
| Non-capturing closure or named function | Thin function pointer (`fn_ptr`) |
| Capturing `Fn` closure | Heap-allocated fat pointer `(fn_ptr, env_ptr)` with `env_ptr` pointing to a reference-counted capture environment |

Non-capturing closures and named functions can be represented as thin pointers because they carry no state. Capturing `Fn` closures are heap-allocated at the point of assignment to `fn` type — the capture environment is boxed and the `fn` value becomes a fat pointer `(fn_ptr, env_ptr)`. The environment is reference-counted to support `fn` values being `Copy`.

**Relationship to `Fn`/`FnMut`/`FnOnce`:** Internally, the compiler tracks which `Fn*` trait each closure implements (based on capture usage analysis above). The `fn(Args): Return` surface type is compatible only with the `Fn` trait. For higher-order functions that need `FnMut` or `FnOnce` semantics, use generic parameters with trait bounds:

```spl
// Accepts any callable, including FnMut
fn apply_mut(f: F) where F: FnMut() {
    f();
}

// Accepts any callable, including FnOnce
fn apply_once(f: F) where F: FnOnce() {
    f();
}
```

---

## 5. Interaction with Second-Class References

SPL's second-class references (refs cannot be stored in structs) simplifies closure design:

**Escaping closures cannot store references.** Non-escaping closures may temporarily borrow from the enclosing scope (the borrow exists only during the function call, not stored in the closure). See section 5 below for details.

However, closures can:

### Receive References as Parameters

```spl
let print_len = |s: &str| {
    println(s.len());
};
```

### Create Temporary References to Captures

```spl
let data = vec![1, 2, 3];
let get_first = || {
    let r: &i32 = &data[0];  // OK: temporary reference
    return *r;
};
```

### Cannot Return References to Captured Data

```spl
let data = vec![1, 2, 3];
// ERROR: Cannot return reference to captured data
let bad = || {
    return &data[0];  // Compile error
};
```

This constraint applies to returning references to **captured** data. Closures receiving reference parameters CAN return references borrowing from those parameters (intersection semantics), same as regular functions:

```spl
// OK: parameter, not capture
let get_first = |data: &Vec(T: i32)| {
    return &data[0];  // Borrows from parameter
};
```

### Non-Escaping Borrows

Non-escaping closures can temporarily borrow from the enclosing scope because the borrow doesn't outlive the function call:

```spl
let name = String.from("Alice");

// Non-escaping: name is borrowed during the map call
items.map(|x| format(name, x));

// name still valid - borrow ended when map returned
println(name);
```

This doesn't violate second-class references because the borrow exists only during the function call, not stored in the closure.

### 5.1 Scoped Closures (Concurrency)

The `scope()` function (see [concurrency.md](concurrency.md) §6) extends non-escaping borrow semantics to concurrent task closures. Within a scope, closures passed to `s.spawn()` are non-escaping — they are guaranteed to complete before `scope()` returns, so they can borrow immutably from the enclosing stack frame:

```spl
use std.task.scope;

let data = vec![1, 2, 3, 4];

scope(|s| {
    for chunk in data.chunks(2) {
        s.spawn(|| {
            // chunk is a borrowed slice — non-escaping, no move required
            process(chunk);
        });
    }
});
// data still valid — borrows ended when scope() returned
```

This contrasts with `spawn()`, which creates an escaping closure that must own (move or clone) all captures and satisfy the `Send` bound. Scoped task closures follow the same non-escaping rules as closures passed to `map`, `filter`, etc. — the borrow is temporary and bounded by the call.

---

## 6. Examples

### Iterator Chains (Non-Escaping)

```spl
fn process_users(users: Vec(T: User)): Vec(T: String) {
    let min_age = 18;
    let department = String.from("Engineering");

    // All closures are non-escaping - borrow freely
    let result = users.iter()
        .filter(|u| u.age >= min_age)           // min_age: copied (Copy)
        .filter(|u| u.department == department) // department: borrowed
        .map(|u| u.name.clone())
        .collect();

    // All variables still valid
    println("Filtered for: " + department);
    return result;
}
```

### Stored Callbacks (Escaping)

```spl
struct Button(
    label: String,
    on_click: fn(): (),
)

fn create_button(label: String, counter: Arc(T: Cell(T: i32))): Button {
    // Escaping: stored in struct
    // counter cloned to allow sharing
    return Button(
        label: label,
        on_click: @[counter: counter.clone()] || {
            counter.set(counter.get() + 1);
        },
    );
    // counter still valid (was cloned)
}
```

### Thread Spawning (Escaping)

```spl
fn parallel_process(data: Vec(T: Item), config: Arc(T: Config)): Vec(T: Handle) {
    let handles = Vec.new();

    for chunk in data.chunks(100) {
        // spawn() takes escaping closure
        // chunk moved, config cloned
        let handle = spawn(@[config: config.clone()] || {
            for item in chunk {
                process_item(item, config);
            }
        });
        handles.push(handle);
    }

    // config still valid (cloned into each closure)
    println("Spawned with config: " + config.name);
    return handles;
}
```

### Returning Closures (Escaping)

```spl
fn make_multiplier(factor: i32): fn(i32): i32 {
    // factor is Copy - just copied
    return |x| x * factor;
}

fn make_prefixer(prefix: String): fn(String): String {
    // prefix moved into closure
    return |s| prefix + s;
    // prefix no longer valid
}

fn make_shared_counter(initial: i32): fn(): i32 {
    let count = Arc.new(Cell.new(initial));
    // Clone Arc to keep local reference
    return @[count: count.clone()] || {
        let val = count.get();
        count.set(val + 1);
        return val;
    };
    // count still valid
}
```

### Mixing Move and Clone

```spl
fn build_pipeline(
    source: DataSource,            // Move-only
    transform: Arc(T: Transform),  // Shared
    sink: Sink,                    // Move-only
    logger: Arc(T: Logger),        // Shared
): fn(): Result(T: (), E: Error) {
    // source and sink moved (pipeline owns them)
    // transform and logger cloned (shared elsewhere)
    return @[transform: transform.clone(), logger: logger.clone()] || {
        logger.log("Starting pipeline");
        for item in source.read() {
            let transformed = transform.apply(item);
            sink.write(transformed)!;
        }
        logger.log("Pipeline complete");
        return Ok(());
    };
    // transform and logger still valid
}
```

---

## 7. Error Messages

### Using Variable After Move

```spl
let data = Vec.new();

let sender = || {
    send(data);  // data moved here
};

println(data.len());  // ERROR: use of moved value
```

**Compiler error:**
```
error[E0382]: use of moved value: `data`
  --> src/main.spl:8:9
  |
4 |     let sender = || {
  |                  -- value moved into closure here
...
8 |     println(data.len());
  |             ^^^^ value used after move
  |
  = help: to keep using `data`, capture with clone: @[data: data.clone()]
```

### Fix with Clone

```spl
let data = Vec.new();

let sender = @[data: data.clone()] || {  // Clone at capture
    send(data);
};

println(data.len());  // OK - data was cloned
```

---

## Summary

| Feature | Syntax | Description |
|---------|--------|-------------|
| Basic closure | `\|a, b\| a + b` | Parameters and expression body |
| No parameters | `\|\| expr` | Empty parameter list |
| Block body | `\|x\| { ... }` | Multi-statement body |
| Type annotation | `\|x: i32\|` | Explicit parameter type |
| Capture by move | `@[x] \|\| ...` | Move `x` into closure |
| Capture by ref | `@[x: &x] \|\| ...` | Borrow `x` into closure |
| Capture with clone | `@[x: x.clone()] \|\| ...` | Clone `x` at capture time |
| Empty captures | `@[] \|\| ...` | No captures |

| Context | Non-Copy Capture Behavior |
|---------|---------------------------|
| Non-escaping | Borrow (default) |
| Escaping | Move (default) |
| Escaping + `@[x: x.clone()]` | Clone |

---

## References

- [ADR-012: Closures and Capture Semantics](../designs/012-closures.md) - Design rationale
- [memory-model.md](memory-model.md) - Ownership and second-class references
- [iteration.md](iteration.md) - Closures in iterator chains
- [concurrency.md](concurrency.md) - Task closure captures
- [syntax-grammar.md](syntax-grammar.md) - Closure syntax
