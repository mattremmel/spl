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
3. **Explicit Capture Lists**: Use `@[...]` prefix for explicit capture control
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

| Type | Non-Escaping | Escaping |
|------|--------------|----------|
| Copy types | copy | copy |
| Non-Copy types | **borrow** | **move** |

### Syntax

#### Basic Closure (Implicit Captures)

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

#### Explicit Capture Lists

The `@[...]` prefix provides explicit control over what variables a closure captures:

```spl
// No captures allowed (pure function)
let add = @[] |x: i32, y: i32| x + y;

// Explicit captures - y and z captured, x is a parameter
let f = @[y, z] |x| x + y + z;

// Capture with expression - evaluated at closure creation time
let f = @[c: counter.clone()] || {
    c.set(c.get() + 1);
};

// Multiple capture expressions
let f = @[y: y.clone(), z: z.some_op()] |x| body;
```

#### Capture List Grammar

```ebnf
Capture = IDENTIFIER                    (* shorthand: y means y: y *)
        | IDENTIFIER ":" Expression ;   (* explicit: y: y.clone() *)
```

- `x` — shorthand, captures `x` and binds as `x`
- `name: expr` — evaluates `expr` at creation, binds result to `name`

This resolves ambiguity: `@[x.y]` is invalid; use `@[y: x.y]` instead.

#### Capture Semantics

Explicit captures follow the same escaping rules as implicit captures:

| Context | Identifier Capture (`@[x]`) | Expression Capture (`@[x: expr]`) |
|---------|----------------------------|-----------------------------------|
| Non-escaping | borrowed | evaluated at creation |
| Escaping | moved | evaluated at creation |

Expression captures (with `:`) are always evaluated at closure creation time, regardless of escaping context. This is the key mechanism for avoiding Rust's "clone dance".

### Why Explicit Capture Lists?

One might ask: why not just write `data.clone()` in the body?

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
let f = @[data: data.clone()] || {
    process(data);  // uses the single clone
};
f();  // no clone
f();  // no clone
```

This is exactly Rust's problem—the "clone dance" exists because there's no way to say "evaluate at capture time." The `@[name: expr]` syntax provides this cleanly.

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

### Function Types

The `fn(...)` type represents any callable with a matching signature, including closures with captures:

```spl
let add: fn(i32, i32): i32 = |a, b| a + b;  // Non-capturing closure

let x = 10;
let add_x: fn(i32): i32 = |a| a + x;  // Capturing closure - also valid
```

---

## Examples

### Example 1: Iterator Chains (Non-Escaping)

```spl
fn process_users(users: Vec(T: User), min_age: i32): Vec(T: String) {
    return users.iter()
        .filter(|u| u.age >= min_age)      // min_age borrowed
        .map(|u| u.name.clone())
        .collect();
    // min_age still valid
}
```

### Example 2: Stored Callbacks (Escaping)

```spl
fn create_button(counter: Arc(T: Cell(T: i32))): Button {
    return Button(
        on_click: @[c: counter.clone()] || {
            c.set(c.get() + 1);
        },
        on_hover: @[c: counter.clone()] || {
            println("Count: " + c.get().to_string());
        },
    );
    // counter still valid
}
```

### Example 3: Thread Spawning (Mixed Move and Clone)

```spl
fn parallel_process(data: Vec(T: Item), config: Arc(T: Config)) {
    for chunk in data.chunks(100) {
        spawn(@[chunk, cfg: config.clone()] || {
            for item in chunk {
                process_item(item, cfg);
            }
        });
    }
    // config still valid (cloned), data consumed (chunks moved)
}
```

### Example 4: Pure Function (No Captures)

```spl
let add = @[] |x: i32, y: i32| x + y;  // guaranteed no captures
```

### Example 5: Returning Closures (Escaping)

```spl
fn make_adder(n: i32): fn(i32): i32 {
    // n is Copy, captured implicitly
    return |x| x + n;
}

fn make_greeter(greeting: String): fn(String): String {
    // greeting moved into returned closure (natural ownership transfer)
    return |name| greeting + ", " + name;
}

fn make_shared_counter(initial: i32): fn(): i32 {
    let count = Arc.new(Cell.new(initial));
    // Clone to allow making multiple counters from same Arc
    return @[count: count.clone()] || {
        let val = count.get();
        count.set(val + 1);
        return val;
    };
}
```

### Example 6: Async Contexts (Escaping)

```spl
async fn fetch_all(urls: Vec(T: String), client: Arc(T: HttpClient)): Vec(T: Response) {
    let timeout = Duration.from_secs(30);  // Copy

    let futures = urls.iter()
        .map(|url| {
            // async block escapes (lives beyond map call)
            // client cloned (shared), url cloned (need to keep iterating)
            async @[client: client.clone(), url: url.clone()] || {
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
    // label moved into outer closure implicitly
    return |numbers| {
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
    state: Arc(T: Mutex(T: AppState)),
    handlers: Vec(T: fn(Event): ()),
)

impl App {
    fn register_handlers(&mut self) {
        let state = self.state.clone();  // Get our own Arc handle

        // Each handler clones the Arc (cheap reference count bump)
        self.handlers.push(@[state: state.clone()] |event| {
            let mut s = state.lock();
            s.handle_click(event);
        });

        self.handlers.push(@[state: state.clone()] |event| {
            let mut s = state.lock();
            s.handle_keypress(event);
        });

        // state still valid
        println("Registered handlers for: " + state.lock().name);
    }
}
```

### Example 9: What Errors Look Like

```spl
fn demonstrate_errors() {
    let name = String.from("Alice");
    let data = Vec.new();
    let count = 0;  // Copy type

    // --- ERROR: Using variable after move ---

    let sender = || {
        send(data);
    };
    println(data.len());  // ERROR!

    // Compiler error:
    // error[E0382]: use of moved value: `data`
    //   --> src/main.spl:8:13
    //   |
    // 5 |     let sender = || {
    //   |                  -- value moved into closure here
    // ...
    // 8 |     println(data.len());
    //   |             ^^^^ value used after move
    //   |
    //   = help: to keep using `data`, capture with clone: @[data: data.clone()]


    // --- CORRECT: Clone to keep using ---

    let sender = @[data: data.clone()] || {
        send(data);
    };
    println(data.len());  // OK - data was cloned


    // --- Copy types just work ---

    let incrementer = || {
        return count + 1;
    };
    println(count);  // OK - count is Copy, was copied not moved
}
```

---

## Rationale

### Why Move by Default for Escaping Closures?

SPL's design philosophy emphasizes explicitness about costs:
- **Explicit `return`** - no hidden control flow
- **Explicit `break`** - no hidden block values
- **Overflow traps** - no hidden wrapping
- **No implicit numeric coercions** - no hidden conversions
- **Methods for type conversion** - explicit about conversion type

Clone-by-default would introduce **hidden allocations**—exactly the kind of implicit cost SPL avoids. Move is zero-cost; clone may allocate. Therefore:

- **Move = default** (free, no annotation)
- **Clone = explicit** (`@[name: expr]` syntax, visible cost)

### Why Borrow by Default for Non-Escaping Closures?

Non-escaping closures are called immediately and don't outlive their context. Borrowing is:
- Zero-cost (no clone or move)
- Safe (reference doesn't escape)
- Ergonomic (most common case for `map`, `filter`, etc.)

This doesn't violate second-class references because the borrow is temporary—it exists only during the function call, not stored in the closure.

### Why the `@[...]` Syntax?

The `@[...]` capture list prefix was chosen because:

- **Familiar concept**: Mirrors C++ lambda capture lists (`[x, &y, z = expr]`)
- **Visually distinct**: Easy to spot before the closure parameters
- **Unambiguous**: `@` is already used for forcing value arguments; `@[...]` is clearly different
- **No operator conflicts**: Unlike `~` (which conflicts with bitwise NOT), `@[...]` has no ambiguity
- **Expression support**: The `name: expr` syntax naturally supports arbitrary expressions

Alternatives considered:
- `~x` in parameter list - conflicts with bitwise NOT operator
- `clone |...|` prefix - doesn't support per-variable control
- `move |...|` prefix - all-or-nothing like Rust

### Why Not Always Require Explicit Capture?

C++ requires explicit capture lists (`[=]`, `[&]`, `[x, &y]`). This is maximally explicit but:
- Verbose for common cases
- Most closures are non-escaping (iterators, callbacks)
- SPL can infer escaping from context

The hybrid approach (implicit for non-escaping, `@[...]` for explicit control) balances ergonomics with explicitness.

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
let future = async @[client: client.clone()] || {
    return client.get("/api/data").await;
};
```

---

## Consequences

### Positive

- No "clone dance" boilerplate
- No hidden allocations (move by default)
- Clear mental model (escaping vs non-escaping)
- Per-variable control with `@[name: expr]` when needed
- Consistent with SPL's explicit philosophy
- Leverages second-class refs (no lifetime complexity)
- Syntax mirrors C++ lambda capture lists (familiar concept)

### Negative

- Different from Rust (learning curve for Rust users)
- `@[...]` prefix is new syntax to learn
- Compiler must infer escaping (implementation complexity)
- Two mental models (escaping vs non-escaping)

### Migration from Rust

| Rust Pattern | SPL Equivalent |
|--------------|----------------|
| `\|x\| x + 1` (borrows x) | `\|x\| x + 1` (borrows if non-escaping) |
| `move \|\| use(x)` | `\|\| use(x)` (move is default for escaping) |
| `{ let x = x.clone(); move \|\| use(x) }` | `@[x: x.clone()] \|\| use(x)` |
| `{ let a = a.clone(); let b = b.clone(); move \|\| }` | `@[a: a.clone(), b: b.clone()] \|\|` |

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
let f = @[s: s.clone()] |y| x + y + s.len();

// Compiled (conceptual)
struct __Closure_1(
    x: i32,      // copied (Copy type)
    s: String,   // cloned (capture expression)
)

impl Fn(i32) for __Closure_1 {
    type Output = i32;
    fn call(&self, y: i32): i32 {
        return self.x + y + self.s.len();
    }
}
```

### Capture Analysis

1. **Identify free variables**: Find variables from outer scope used in body
2. **Classify each capture**: Copy type? Non-Copy? Explicit capture expression?
3. **Check escaping**: Does closure escape its creation context?
4. **Apply rules**:
   - Copy → copy
   - Non-Copy + non-escaping → borrow
   - Non-Copy + escaping → move (default)
   - Explicit capture expression → evaluate at creation
5. **Infer trait**: FnOnce/FnMut/Fn based on capture usage

---

## Future Extension

Force move in non-escaping context could be added later if needed:

```spl
@[move x] || ...   // future: force move even for non-escaping
```

This is rare—non-escaping closures almost always want borrow semantics—but the syntax has room to accommodate it.

---

## Open Questions

1. **Async closure syntax** - `async @[...] |x| { ... }` or `@[...] |x| async { ... }`?

---

## References

- [ADR-011: Iteration and Generators](011-iteration-and-generators.md)
- [DECISIONS.md §4.1](../DECISIONS.md) - Second-class references
- [Rust RFC #2407: Clone into closures](https://github.com/rust-lang/rfcs/issues/2407)
- [Rust RFC #3680: Ergonomic ref-counting](https://github.com/rust-lang/rfcs/pull/3680)
- [C++ Lambda Expressions](https://en.cppreference.com/w/cpp/language/lambda.html)
- [Rust Internals: Explicit Captures](https://internals.rust-lang.org/t/explicit-captures-for-closures-and-code-blocks/9675)
