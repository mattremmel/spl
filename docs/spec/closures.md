# SPL Closures

This document specifies closure syntax and capture semantics in SPL.

## Overview

Closures are anonymous functions that can capture variables from their enclosing scope. SPL's closure design leverages second-class references to eliminate lifetime complexity while providing explicit control over capture behavior.

**Key Principles:**

- **Escaping vs Non-escaping**: Different defaults based on whether the closure outlives its creation context
- **Move by default**: Escaping closures move non-Copy captures (no hidden allocations)
- **Borrow by default**: Non-escaping closures borrow non-Copy captures (zero-cost)
- **Explicit clone**: Use `~` sigil when cloning is intended

---

## 1. Syntax

### Basic Closures

```ebnf
ClosureExpr = [ "clone" | "move" ] ClosureParams ClosureBody ;

ClosureParams = "||"
              | "|" [ ClosureParamList ] "|" ;

ClosureParamList = ClosureParam { "," ClosureParam } [ "," ] ;

ClosureParam = [ "~" ] [ "mut" ] IDENTIFIER [ ":" Type ] ;

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

The compiler infers escaping from:

1. **Function signatures**: Parameter types indicate whether closure escapes
2. **Storage**: Assigned to struct field, returned, stored in collection
3. **Known functions**: `spawn`, `thread::spawn`, etc. are known to require escaping closures

---

## 3. Capture Semantics

### Capture Rules

| Type | Non-Escaping | Escaping (default) | Escaping (explicit) |
|------|--------------|-------------------|---------------------|
| Copy types | copy | copy | copy |
| Non-Copy types | borrow | **move** | `~` = clone |

> **Copy and Clone:** All `Copy` types implicitly implement `Clone` with trivial (bitwise) semantics. Using `~` on a Copy type is valid but redundant—it simply copies the value. A future lint may warn about unnecessary `~` on Copy types.

### The `~` Clone Modifier

Use `~` before a capture to clone it at closure creation time:

```spl
let data = Arc.new(vec![1, 2, 3]);

// Without ~: data is moved (escaping default)
let f = || process(data);
// data no longer valid

// With ~: data is cloned at capture time
let f = |~data| process(data);
// data still valid (was cloned into closure)
```

**Why `~` instead of `.clone()` in the body?**

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
let f = |~data| {
    process(data);  // uses the single clone
};
f();  // no clone
f();  // no clone
```

### Clone-All Shorthand

When cloning multiple captures, use the `clone` keyword:

```spl
// Without shorthand - repetitive
spawn(|~config, ~logger, ~metrics| {
    worker_loop(config, logger, metrics);
});

// With clone shorthand - cleaner
spawn(clone |config, logger, metrics| {
    worker_loop(config, logger, metrics);
});
```

### Move-All (Explicit)

The `move` keyword makes move semantics explicit:

```spl
// Explicit move (same as escaping default, for clarity)
let f = move |data, config| {
    process(data, config);
};
```

**When is `move` useful?**

For escaping closures, `move` is redundant since move is already the default. However, `move` is useful for:

1. **Documentation**: Making capture behavior explicit for readers
2. **Non-escaping closures**: Forcing move semantics when the closure would otherwise borrow
3. **Future-proofing**: If a closure's escaping status changes, explicit `move` preserves the intended behavior

```spl
// Without move: items.each() takes non-escaping closure, so data is borrowed
items.each(|item| use_with(data, item));
// data still valid

// With move: force move even though closure is non-escaping
items.each(move |item| use_with(data, item));
// data no longer valid (was moved)
```

---

## 4. Closure Traits

Closures implement traits based on how they use their captures:

```spl
trait FnOnce(Args) {
    type Output;
    fn call_once(self, args: Args): Self.Output;
}

trait FnMut(Args): FnOnce(Args) {
    fn call_mut(&mut self, args: Args): Self.Output;
}

trait Fn(Args): FnMut(Args) {
    fn call(&self, args: Args): Self.Output;
}
```

**Hierarchy:** `Fn` ⊂ `FnMut` ⊂ `FnOnce`

### Trait Inference

The compiler determines which trait a closure implements:

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

### Function Types

The `fn(...)` type represents any callable with a matching signature, including both plain functions and closures (with or without captures):

```spl
// Non-capturing closure
let add: fn(i32, i32): i32 = |a, b| a + b;

// Capturing closure - also valid
let x = 10;
let add_x: fn(i32): i32 = |a| a + x;

// Named function
fn double(n: i32): i32 { return n * 2; }
let f: fn(i32): i32 = double;
```

This unified model (like Go and Swift) simplifies the type system at the cost of some optimization opportunities. The compiler cannot inline through `fn` types since the concrete callable is not known at compile time.

---

## 5. Interaction with Second-Class References

SPL's second-class references (refs cannot be stored in structs) simplifies closure design:

**Closures cannot store references.** Captured variables can only be owned values or copies.

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
        on_click: |~counter| {
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
        let handle = spawn(|~config| {
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
    return |~count| {
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
    return |~transform, ~logger| {
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
  = help: to keep using `data`, capture with clone: |~data|
```

### Fix with Clone

```spl
let data = Vec.new();

let sender = |~data| {  // Clone at capture
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
| Clone capture | `\|~x\|` | Clone `x` at capture time |
| Clone all | `clone \|x, y\|` | Clone all non-Copy captures |
| Move all | `move \|x, y\|` | Explicit move (escaping default) |

| Context | Non-Copy Capture Behavior |
|---------|---------------------------|
| Non-escaping | Borrow (default) |
| Escaping | Move (default) |
| Escaping + `~` | Clone |
| Escaping + `clone` | Clone all |

---

## References

- [ADR-012: Closures and Capture Semantics](../designs/012-closures.md) - Design rationale
- [memory-model.md](memory-model.md) - Ownership and second-class references
- [iteration.md](iteration.md) - Closures in iterator chains
- [concurrency.md](concurrency.md) - Task closure captures
- [syntax-grammar.md](syntax-grammar.md) - Closure syntax
