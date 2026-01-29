# ADR-012: Closures and Capture Semantics

**Status:** Accepted
**Date:** 2026-01-28

## Context

SPL needs closures for functional programming patterns, callbacks, and integration with generators (ADR-011). The key design question is capture semantics—how closures access variables from their enclosing scope.

### Rust's Pain Points

1. **All-or-nothing `move`**: Either everything is borrowed OR everything is moved
2. **Clone dance**: Repetitive `let x = x.clone();` before `move` closures
3. **No per-variable control**: Can't say "move this, clone that" without workarounds

```rust
// Rust: The "clone dance" - verbose and error-prone
let data = Arc::clone(&data);
let config = Arc::clone(&config);
let logger = Arc::clone(&logger);
thread::spawn(move || {
    // finally use them
});

// Rust: Workaround for mixed capture
let ref_settings = &settings;
let channel = channel;
move || {
    channel.send(ref_settings.value);  // awkward
}
```

### C++ Comparison

C++ lambdas offer explicit per-variable control:

```cpp
[x, &y, z = z.clone()]() { ... }  // x by value, y by ref, z cloned
```

This granularity is useful but verbose for common cases.

### SPL's Advantage: Second-Class References

SPL's second-class references (refs can only be function parameters) dramatically simplifies closure design:

**Closures cannot store references.** Period.

This means captured variables can only be:
- **Owned values** (moved into the closure)
- **Copied values** (for Copy types)

There's no "capture by reference for storage" option. This eliminates Rust's lifetime complexity for closures entirely.

However, closures can still *receive* references as parameters, and *borrow temporarily* in non-escaping contexts.

## Decision

### Core Principles

1. **Escaping vs Non-Escaping**: Different defaults based on whether closure escapes
2. **Move by Default**: Escaping closures move captures (no hidden allocations)
3. **Explicit Clone**: Use `~` sigil when cloning is intended
4. **Consistent with SPL Philosophy**: Explicit about costs, no surprises

### Escaping vs Non-Escaping Closures

**Non-escaping closures** are used immediately and don't outlive the current scope:
- Passed to `map`, `filter`, `for_each`, etc.
- Called within the receiving function, then discarded
- Can borrow from enclosing scope (reference is temporary)

**Escaping closures** outlive their creation context:
- Stored in structs
- Returned from functions
- Passed to `spawn`, async runtimes, etc.
- Must own their captures (move or clone)

The compiler infers escaping from function signatures and usage.

### Capture Behavior

| Type | Non-Escaping | Escaping (default) | Escaping (explicit) |
|------|--------------|-------------------|---------------------|
| Copy types | copy | copy | copy |
| Non-Copy types | borrow | **move** | `~` = clone |

### Syntax

#### Basic Closure

```spl
// Closure with inferred parameter types
let add = |a, b| { return a + b; };

// Closure with explicit parameter types
let add = |a: i32, b: i32| { return a + b; };

// Single-expression closure (implicit return for expression-only body)
let add = |a, b| a + b;

// No parameters
let greet = || { println("Hello"); };
```

#### Capture Modifiers

```spl
// ~ = clone (capture a clone at closure creation time)
// no modifier = move (for escaping) or borrow (for non-escaping)

let data = Arc.new([1, 2, 3]);

// Escaping: data is moved (default)
let f = |data| process(data);
// data no longer valid

// Escaping: data is cloned
let f = |~data| process(data);
// data still valid

// Non-escaping: data is borrowed (default)
items.map(|x| x + data.len());
// data still valid
```

#### Clone-All Shorthand

```spl
// Clone all captured non-Copy variables
let f = clone |data, config| { ... };
```

#### Move-All (Explicit, Same as Default)

```spl
// Explicit move all (for clarity, same as default for escaping)
let f = move |data, config| { ... };
```

### Why Not `x.clone()` in Capture Position?

One might ask: why not just write `data.clone()` instead of `~data`?

**The timing problem**: Captures happen at closure *creation* time, but expressions in the body execute at *call* time.

```spl
let data = Arc.new([1, 2, 3]);

// WRONG: clone happens every call
let f = || {
    process(data.clone());  // clones each time f() is called!
};
f();  // clone
f();  // clone again

// RIGHT: clone happens once at capture
let f = |~data| {
    process(data);  // uses the single clone
};
f();  // no clone
f();  // no clone
```

This is exactly Rust's problem—the "clone dance" exists because there's no way to say "evaluate at capture time." The `~` sigil provides this cleanly.

### Closure Traits

SPL has three closure traits:

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

The compiler determines which trait a closure implements based on how it uses captures:

```spl
let data = vec![1, 2, 3];
let mut count = 0;

// Implements Fn - only reads captures
let get_len = || { return data.len(); };

// Implements FnMut - mutates captures
let increment = || { count = count + 1; };

// Implements FnOnce - consumes captures
let consume = || { return take_ownership(data); };
```

### Function Pointer Coercion

Closures that capture nothing coerce to function pointers:

```spl
let add: fn(i32, i32): i32 = |a, b| a + b;  // OK: no captures

let x = 10;
let add_x: fn(i32): i32 = |a| a + x;  // ERROR: captures x
```

---

## Examples

### Example 1: Iterator Chains (Non-Escaping)

```spl
fn process_users(users: Vec(User)): Vec(String) {
    let min_age = 18;
    let department = String.from("Engineering");
    let active_status = Status.Active;

    // ALL of these closures are non-escaping
    // They borrow freely - no annotations needed
    let result = users.iter()
        .filter(|u| u.age >= min_age)              // min_age: copied (Copy)
        .filter(|u| u.department == department)    // department: borrowed
        .filter(|u| u.status == active_status)     // active_status: copied (Copy)
        .map(|u| u.name.clone())
        .collect();

    // All variables still valid
    println("Filtered for: " + department);
    println("Min age was: " + min_age.to_string());

    return result;
}
```

### Example 2: Higher-Order Functions (Non-Escaping)

```spl
// Function signature indicates non-escaping (closure called, not stored)
fn with_retry(attempts: i32, f: fn(): Result(T, Error)): Result(T, Error) where T {
    for _ in 0..attempts {
        match f() {
            Ok(v) => { return Ok(v); },
            Err(_) => { continue; },
        }
    }
    return Err(Error.new("Max retries exceeded"));
}

fn fetch_data(config: Config, client: HttpClient): Result(Data, Error) {
    let url = config.endpoint.clone();
    let timeout = config.timeout;

    // Non-escaping: called within with_retry, then done
    // url borrowed, timeout copied
    let result = with_retry(3, || {
        return client.get(url, timeout);
    });

    // url still valid
    println("Fetched from: " + url);
    return result;
}
```

### Example 3: Stored Callbacks (Escaping)

```spl
struct Button(
    label: String,
    on_click: fn(): (),
    on_hover: fn(): (),
)

fn create_button(label: String, counter: Arc(Cell(i32))): Button {
    // Escaping closures: stored in struct
    // label moved (button owns it), counter must be cloned to share
    return Button(
        label: label,  // moved into struct field, not closure
        on_click: |~counter| {
            counter.set(counter.get() + 1);
        },
        on_hover: |~counter| {
            println("Count: " + counter.get().to_string());
        },
    );

    // counter still valid (was cloned into each closure)
    // label was moved to Button.label, not available here
}
```

### Example 4: Thread Spawning (Escaping)

```spl
fn parallel_process(data: Vec(Item), config: Arc(Config)): Vec(Handle) {
    let handles = Vec.new();

    for chunk in data.chunks(100) {
        // spawn() takes escaping closure
        // chunk moved (we're done with it), config cloned (shared)
        let handle = spawn(|chunk, ~config| {
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

### Example 5: Returning Closures (Escaping)

```spl
fn make_adder(n: i32): fn(i32): i32 {
    // n is Copy, no annotation needed
    return |n, x| x + n;
}

fn make_greeter(greeting: String): fn(String): String {
    // greeting moved into returned closure (natural ownership transfer)
    return |greeting, name| greeting + ", " + name;
}

fn make_logger(prefix: String, suffix: String): fn(String): () {
    // Both moved - we're returning this closure, done with originals
    return |prefix, suffix, msg| {
        println(prefix + msg + suffix);
    };
}

fn make_shared_counter(initial: i32): fn(): i32 {
    let count = Arc.new(Cell.new(initial));
    // Clone to allow making multiple counters from same Arc
    return |~count| {
        let val = count.get();
        count.set(val + 1);
        return val;
    };
}
```

### Example 6: Async Contexts (Escaping)

```spl
async fn fetch_all(urls: Vec(String), client: Arc(HttpClient)): Vec(Response) {
    let timeout = Duration.from_secs(30);  // Copy

    let futures = urls.iter()
        .map(|url| {
            // async block escapes (lives beyond map call)
            // client cloned (shared), url cloned (need to keep iterating)
            async |~client, ~url| {
                return client.get(url, timeout).await;
            }
        })
        .collect();

    return join_all(futures).await;
}
```

### Example 7: Nested Closures

```spl
fn create_processor(config: Config): fn(Vec(i32)): Vec(i32) {
    let multiplier = config.multiplier;  // Copy
    let offset = config.offset;          // Copy
    let label = config.label;            // String - will be moved

    // Outer closure escapes (returned)
    // label moved into outer closure
    return |label, numbers| {
        // Inner closures are non-escaping (used in map)
        // They borrow from outer closure's scope
        let result = numbers.iter()
            .map(|n| n * multiplier)      // multiplier: copied
            .map(|n| n + offset)          // offset: copied
            .collect();

        println(label + ": processed");   // label: owned by outer closure
        return result;
    };
}
```

### Example 8: Event Handlers with Shared State

```spl
struct App(
    state: Arc(Mutex(AppState)),
    handlers: Vec(fn(Event): ()),
)

impl App {
    fn register_handlers(&mut self) {
        let state = self.state.clone();  // Get our own Arc handle

        // Each handler clones the Arc (cheap reference count bump)
        self.handlers.push(|~state, event| {
            let mut s = state.lock();
            s.handle_click(event);
        });

        self.handlers.push(|~state, event| {
            let mut s = state.lock();
            s.handle_keypress(event);
        });

        // state still valid
        println("Registered handlers for: " + state.lock().name);
    }
}
```

### Example 9: Mixing Move and Clone

```spl
fn build_pipeline(
    source: DataSource,       // Move-only, owns resource
    transform: Arc(Transform), // Shared, clone
    sink: Sink,               // Move-only, owns resource
    logger: Arc(Logger),      // Shared, clone
): fn(): Result((), Error) {

    // source and sink moved (pipeline owns them)
    // transform and logger cloned (might be shared elsewhere)
    return |source, ~transform, sink, ~logger| {
        logger.log("Starting pipeline");

        for item in source.read() {
            let transformed = transform.apply(item);
            sink.write(transformed)?;
        }

        logger.log("Pipeline complete");
        return Ok(());
    };

    // transform and logger still valid for other use
    // source and sink moved into pipeline
}
```

### Example 10: What Errors Look Like

```spl
fn demonstrate_errors() {
    let name = String.from("Alice");
    let data = Vec.new();
    let count = 0;  // Copy type

    // --- ERROR: Using variable after move ---

    let sender = |data| {
        send(data);
    };
    println(data.len());  // ERROR!

    // Compiler error:
    // error[E0382]: use of moved value: `data`
    //   --> src/main.spl:8:13
    //   |
    // 5 |     let sender = |data| {
    //   |                   ---- value moved into closure here
    // ...
    // 8 |     println(data.len());
    //   |             ^^^^ value used after move
    //   |
    //   = help: to keep using `data`, capture with clone: |~data|


    // --- CORRECT: Clone to keep using ---

    let sender = |~data| {
        send(data);
    };
    println(data.len());  // OK - data was cloned


    // --- Copy types just work ---

    let incrementer = |count| {
        return count + 1;
    };
    println(count);  // OK - count is Copy, was copied not moved
}
```

### Example 11: Clone-All Shorthand

```spl
fn spawn_many_workers(
    config: Arc(Config),
    logger: Arc(Logger),
    metrics: Arc(Metrics),
) {
    // Without shorthand - repetitive
    for i in 0..10 {
        spawn(|~config, ~logger, ~metrics, i| {
            worker_loop(i, config, logger, metrics);
        });
    }

    // With clone shorthand - cleaner
    for i in 0..10 {
        spawn(clone |config, logger, metrics, i| {
            worker_loop(i, config, logger, metrics);
        });
    }

    // All still valid
    logger.log("Spawned 10 workers");
}
```

---

## Rationale

### Why Move by Default for Escaping Closures?

SPL's design philosophy emphasizes explicitness about costs:
- **Explicit `return`** - no hidden control flow
- **Explicit `yield`** - no hidden block values
- **Overflow traps** - no hidden wrapping
- **No implicit numeric coercions** - no hidden conversions
- **Methods for type conversion** - explicit about conversion type

Clone-by-default would introduce **hidden allocations**—exactly the kind of implicit cost SPL avoids. Move is zero-cost; clone may allocate. Therefore:

- **Move = default** (free, no annotation)
- **Clone = explicit** (`~` sigil, visible cost)

### Why Borrow by Default for Non-Escaping Closures?

Non-escaping closures are called immediately and don't outlive their context. Borrowing is:
- Zero-cost (no clone or move)
- Safe (reference doesn't escape)
- Ergonomic (most common case for `map`, `filter`, etc.)

This doesn't violate second-class references because the borrow is temporary—it exists only during the function call, not stored in the closure.

### Why the `~` Sigil?

- **Concise**: Single character vs `clone` keyword
- **Visually distinct**: Easy to spot in capture list
- **Meaningful**: `~` suggests "approximate copy" or "similar to"
- **Familiar**: Used in some languages for related concepts

Alternatives considered:
- `+x` - could confuse with arithmetic
- `*x` - conflicts with dereference
- `clone x` - verbose in capture position
- `x.clone()` - wrong timing (see "Why Not `x.clone()`" section)

### Why Not Always Require Explicit Capture?

C++ requires explicit capture lists (`[=]`, `[&]`, `[x, &y]`). This is maximally explicit but:
- Verbose for common cases
- Most closures are non-escaping (iterators, callbacks)
- SPL can infer escaping from context

The hybrid approach (implicit for non-escaping, explicit for escaping non-Copy) balances ergonomics with explicitness.

---

## Interaction with Other Features

### Second-Class References

Closures can **receive** references as parameters:

```spl
let print_len = |s: &str| {
    println(s.len());
};
```

Closures can **create** temporary references to captures:

```spl
let data = vec![1, 2, 3];
let get_first = |data| {
    let r: &i32 = &data[0];  // OK: temporary reference
    return *r;
};
```

Closures **cannot return references** (second-class rule):

```spl
let data = vec![1, 2, 3];
// ERROR: Cannot return reference
let bad = |data| {
    return &data[0];  // Compile error
};
```

### Generators

Closures and generators compose naturally:

```spl
let multiplier = 2;

// Generator using captured value
gen fn scaled(items: Vec(i32)): i32 {
    for item in items {
        yield item * multiplier;  // multiplier captured
    }
}

// Closure returning generator
fn make_counter(start: i32): gen i32 {
    return gen |start| {
        let n = start;
        loop {
            yield n;
            n = n + 1;
        }
    };
}
```

### Async

Async blocks follow the same capture rules:

```spl
let client = Arc.new(HttpClient.new());

// Async block is escaping - explicit clone needed
let future = async |~client| {
    return client.get("/api/data").await;
};
```

---

## Consequences

### Positive

- No "clone dance" boilerplate
- No hidden allocations (move by default)
- Clear mental model (escaping vs non-escaping)
- Per-variable control with `~` when needed
- Consistent with SPL's explicit philosophy
- Leverages second-class refs (no lifetime complexity)

### Negative

- Different from Rust (learning curve for Rust users)
- `~` is new syntax to learn
- Compiler must infer escaping (implementation complexity)
- Two mental models (escaping vs non-escaping)

### Migration from Rust

| Rust Pattern | SPL Equivalent |
|--------------|----------------|
| `\|x\| x + 1` (borrows x) | `\|x\| x + 1` (borrows if non-escaping) |
| `move \|\| use(x)` | `\|\| use(x)` (move is default for escaping) |
| `{ let x = x.clone(); move \|\| use(x) }` | `\|~x\| use(x)` |
| `{ let a = a.clone(); let b = b.clone(); move \|\| }` | `clone \|a, b\|` or `\|~a, ~b\|` |

---

## Implementation Notes

### Escape Analysis

The compiler determines if a closure escapes by analyzing:

1. **Function signatures**: Parameter types indicate escaping
   ```spl
   fn non_escaping(f: fn(&T): U)  // Called with borrowed data
   fn escaping(f: fn(): U + 'static)  // Must be 'static, escapes
   ```

2. **Storage**: Assigned to struct field, returned, etc.

3. **Specific functions**: `spawn`, `thread::spawn`, etc. are known to escape

### Closure Representation

Closures compile to anonymous structs:

```spl
// Source
let x = 1;
let s = String.from("hello");
let f = |~s, y| x + y + s.len();

// Compiled (conceptual)
struct __Closure_1 {
    x: i32,      // copied (Copy type)
    s: String,   // cloned (~ modifier)
}

impl Fn(i32) for __Closure_1 {
    type Output = i32;
    fn call(&self, y: i32): i32 {
        return self.x + y + self.s.len();
    }
}
```

### Capture Analysis

1. **Identify free variables**: Find variables from outer scope used in body
2. **Classify each capture**: Copy type? Non-Copy? Has `~` modifier?
3. **Check escaping**: Does closure escape its creation context?
4. **Apply rules**:
   - Copy → copy
   - Non-Copy + non-escaping → borrow
   - Non-Copy + escaping → move (default) or clone (`~`)
5. **Infer trait**: FnOnce/FnMut/Fn based on capture usage

---

## Open Questions

1. **Syntax for type annotations in captures?** - `|~data: Arc(Data)|` or infer?
2. **`clone` vs `~` consistency** - Should `clone |...|` use different keyword?
3. **Async closure syntax** - `async |x| { ... }` or `|x| async { ... }`?

---

## References

- [ADR-011: Iteration and Generators](011-iteration-and-generators.md)
- [DECISIONS.md §4.1](../DECISIONS.md) - Second-class references
- [Rust RFC #2407: Clone into closures](https://github.com/rust-lang/rfcs/issues/2407)
- [Rust RFC #3680: Ergonomic ref-counting](https://github.com/rust-lang/rfcs/pull/3680)
- [C++ Lambda Expressions](https://en.cppreference.com/w/cpp/language/lambda.html)
- [Rust Internals: Explicit Captures](https://internals.rust-lang.org/t/explicit-captures-for-closures-and-code-blocks/9675)
