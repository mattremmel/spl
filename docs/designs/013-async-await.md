# ADR-013: Concurrency Model and Runtime

**Status:** Accepted
**Date:** 2026-01-28

## Context

SPL needs a concurrency story. The keywords `async` and `await` are reserved in the lexical grammar, and DECISIONS.md specifies "Built-in runtime (Go-style simplicity)."

This ADR defines SPL's concurrency model based on lessons learned from Go, Rust, Java (Project Loom), and other languages.

### Goals

1. **No function coloring** - Any function can yield; no async/sync divide
2. **Simplicity** - Built-in runtime, no executor selection, "just works"
3. **Safety** - Task isolation, proper resource cleanup on failure
4. **Performance** - Competitive with Go, optional thread-per-core mode
5. **Explicit costs** - No hidden allocations or magic behavior

### Key Influences

- **Go**: Goroutines, growable stacks, built-in runtime, async preemption
- **Rust**: Explicit spawning, structured concurrency, cancellation via drop
- **Java Loom**: Virtual threads, blocking detection
- **Erlang**: Task isolation, "let it crash" philosophy

### Module Organization

Concurrency primitives are organized in the standard library:

```spl
std.task        // Task spawning, joining, scopes
std.channel     // All channel types
std.sync        // Mutex, RwLock, Barrier, etc.
std.runtime     // Runtime configuration
```

Usage patterns:
```spl
// Import module, use qualified names
use std.task;
task.spawn(|| work());

// Import specific functions
use std.task.spawn;
spawn(|| work());

// Import multiple items
use std.task.{spawn, scope, JoinHandle};
```

---

## Decision

### 1. No Function Coloring

Unlike Rust's async/await, SPL does not distinguish between "async" and "sync" functions at the type level. Any function can yield to the scheduler.

```spl
use std.task.spawn;

// No async keyword needed - any function can yield
fn fetch(url: String): Response {
    let conn = TcpStream.connect(url);  // May yield
    let response = conn.read_all();      // May yield
    return response;
}

// main() runs inside an implicit runtime
fn main(): () {
    let handle = spawn(|| fetch("https://example.com"));
    let result = handle.await();
}
```

**Rationale**: Function coloring creates an ecosystem split (async vs sync libraries) and viral annotation burden. Go proved that a unified model works at scale.

**Tradeoff**: Cannot statically know which functions may yield, so all functions must be compiled with yield-point support.

---

### 2. Stack Management

SPL uses **growable contiguous stacks** with **adaptive initial sizing**, following Go 1.19+.

#### Stack Model

| Property | Value |
|----------|-------|
| Initial size | Adaptive (2KB - 32KB based on recent usage) |
| Minimum size | 2KB (8KB on Windows) |
| Maximum size | 1MB (configurable) |
| Growth strategy | 2x, copy to new contiguous allocation |
| Shrink | At GC, if usage < 25% of allocated |

#### How It Works

1. **Prologue check**: Every function entry checks if stack space is sufficient
2. **Growth**: If insufficient, allocate 2x larger stack, copy contents, update pointers
3. **Adaptive sizing**: Runtime tracks average stack usage; new tasks start at that size
4. **No hot-split**: Contiguous copying (Go 1.3+) eliminates the hot-split problem

```spl
use std.task.spawn;

// Task starts with adaptive size (e.g., 8KB if recent average)
spawn(|| {
    // Deep recursion triggers growth: 8KB -> 16KB -> 32KB
    recursive_algorithm(data);
});
```

**References**:
- [Go Stack Evolution](https://medium.com/a-journey-with-go/go-how-does-the-goroutine-stack-size-evolve-447fc02085e5)
- [Go 1.19 Adaptive Sizing](https://github.com/golang/go/commit/016d7552138077741a9c3fdadc73c0179f5d3ff7)

---

### 3. Preemption

SPL uses **cooperative preemption at function calls** plus **async preemption via signals** for tight loops.

#### Cooperative Yield Points

- Function calls (stack check prologue)
- Channel operations
- I/O operations
- Explicit `task.yield()` (or `yield()` if imported)

#### Async Preemption (Go 1.14+ model)

For tight loops that don't hit cooperative yield points:

1. Monitor thread detects tasks running >10ms without yielding
2. Sends `SIGURG` signal to the OS thread
3. Signal handler checks if at a safe point
4. If safe, injects a yield; otherwise sets flag for next safe point

```spl
// Without async preemption, this would starve other tasks
fn compute_forever() {
    loop {
        // No function calls, no yields
        x = x + 1;
    }
}
// With async preemption, scheduler interrupts after 10ms
```

**Safe points** are locations where:
- No partial writes (mid-write-barrier, etc.)
- Sufficient stack space for preempt call
- No runtime locks held

**References**:
- [Go Async Preemption](https://medium.com/a-journey-with-go/go-asynchronous-preemption-b5194227371c)
- [Go Preemption Proposal](https://go.googlesource.com/proposal/+/master/design/24543-non-cooperative-preemption.md)

---

### 4. Task Spawning

Tasks are spawned via `task.spawn()`, which returns a `JoinHandle(T)`.

#### Basic Usage

```spl
use std.task.{spawn, JoinHandle};

// Spawn a task - returns immediately
let handle: JoinHandle(Response) = spawn(|| {
    return fetch(url);
});

// Do other work...

// Await the result - .await() is a method on JoinHandle
let response: Response = handle.await();
```

#### Closure Capture Semantics

Task closures follow ADR-012 capture rules (escaping closures):

```spl
use std.task.spawn;
use std.sync.Arc;

let data = load_data();
let config = Arc.new(Config.load());

// data moved (default), config cloned (~)
let handle = spawn(|data, ~config| {
    return process(data, config);
});

// data is gone, config still valid
println(config.name);
```

#### Configuration via Named Arguments

```spl
use std.task.spawn;

let handle = spawn(
    || fetch(url),
    name = "http-fetch",
    timeout = Duration.from_secs(30),
    cancellation_token = token,
);
```

Available options:

| Option | Type | Description |
|--------|------|-------------|
| `name` | `String` | Task name for debugging/tracing |
| `timeout` | `Duration` | Auto-cancel after duration |
| `cancellation_token` | `CancellationToken` | External cancellation control |
| `priority` | `Priority` | Scheduling priority hint |

#### JoinHandle API

```spl
impl JoinHandle(T) {
    fn await(self): T;                    // Wait for result (may panic if task panicked)
    fn try_await(self): Result(T: T, E: TaskError);  // Wait, get error on panic/cancel
    fn cancel(&self);                     // Request cancellation
    fn is_finished(&self): bool;          // Poll completion
    fn detach(self);                      // Fire-and-forget (task continues)
}

enum TaskError {
    Panicked(String),   // Task panicked with message
    Cancelled,          // Task was cancelled
}
```

#### Note on `.await()`

Unlike Rust's `.await` postfix keyword, SPL's `.await()` is a regular method. This is consistent with "no function coloring" - there's no special syntax needed because any function can yield.

Types that represent pending work (like `JoinHandle`, oneshot receivers) implement an `Awaitable` trait:

```spl
trait Awaitable {
    type Output;
    fn await(self): Self.Output;
}
```

---

### 5. Cancellation

**Dropping a JoinHandle cancels the task.** This prevents orphaned tasks.

```spl
use std.task.spawn;

{
    let handle = spawn(|| long_work());
}  // handle dropped here - task cancelled

// Explicit cancel
let handle = spawn(|| long_work());
handle.cancel();
let result = handle.try_await();  // Returns Err(TaskError.Cancelled)

// Fire-and-forget (explicit opt-in)
let handle = spawn(|| background_work());
handle.detach();  // Task continues, handle consumed
```

#### How Cancellation Works

1. Cancellation flag set on task
2. At next yield point, runtime checks flag
3. Task unwinds (destructors run), terminates
4. Awaiter receives `TaskError.Cancelled`

#### CancellationToken for Cooperative Checking

```spl
use std.task.{spawn, CancellationToken};

let token = CancellationToken.new();

let handle = spawn(
    |~token| {
        for item in large_dataset {
            if token.is_cancelled() {
                cleanup();
                return;
            }
            process(item);
        }
    },
    cancellation_token = token.clone(),
);

// Later: request cancellation
token.cancel();
```

---

### 6. Panic and Task Isolation

**Panic unwinds the task's stack, running destructors.** Other tasks are unaffected.

```spl
use std.task.{spawn, TaskError};

let handle = spawn(|| {
    let file = File.create("temp.txt");
    let guard = mutex.lock();

    panic("something went wrong");

    // Destructors run:
    // - guard released (no deadlock)
    // - file closed (no leak)
});

// Task panic propagates to awaiter
match handle.try_await() {
    Ok(result) => use(result),
    Err(TaskError.Panicked(msg)) => log("Task failed: " + msg),
    Err(TaskError.Cancelled) => log("Task cancelled"),
}

// Or let it propagate (handle.await() panics if task panicked)
let result = handle.await();  // Panics here if task panicked
```

#### Catch and Recover

```spl
let result = catch_panic(|| {
    risky_operation()
});

match result {
    Ok(value) => use(value),
    Err(panic_info) => recover(panic_info),
}
```

#### FFI Boundary

Panic **aborts** if it would unwind across FFI boundaries (undefined behavior):

```spl
extern fn c_callback(f: extern fn());

fn risky() {
    c_callback(|| {
        panic("oops");  // Would unwind through C - ABORTS instead
    });
}
```

---

### 7. Channels

Channels are standard library types, not language primitives.

#### MPSC (Multi-producer, Single-consumer)

```spl
use std.channel;

let (tx, rx) = channel.mpsc(10);  // Bounded, capacity 10
let (tx, rx) = channel.mpsc_unbounded();

// Send
tx.send(value);               // Blocks if full
tx.try_send(value): Result;   // Non-blocking

// Receive
let val = rx.recv();          // Blocks until value
let val = rx.try_recv(): Option;

// Clone sender (multi-producer)
let tx2 = tx.clone();
```

#### MPMC (Multi-producer, Multi-consumer)

```spl
let (tx, rx) = channel.mpmc(10);

let tx2 = tx.clone();  // Multiple producers
let rx2 = rx.clone();  // Multiple consumers
```

#### Oneshot (Single value)

```spl
let (tx, rx) = channel.oneshot();

tx.send(value);        // Consumes tx
let val = rx.await();  // rx is awaitable
```

#### Broadcast (All receivers get all messages)

```spl
let tx = channel.broadcast(16);
let rx1 = tx.subscribe();
let rx2 = tx.subscribe();

tx.send(value);  // Both rx1 and rx2 receive
```

#### Watch (Latest value, observable)

```spl
let (tx, rx) = channel.watch(initial_value);

tx.send(new_value);       // Update value
let val = rx.borrow();    // Current value (no wait)
rx.changed().await();     // Wait for change
```

---

### 8. Structured Concurrency

Functions for managing concurrent task lifetimes.

#### Join: Wait for All

```spl
use std.task.{spawn, join, join_all};

// Heterogeneous (tuple return)
let (user, posts) = join(
    spawn(|| fetch_user(id)),
    spawn(|| fetch_posts(id)),
);

// Homogeneous (vec return)
let handles = urls.iter()
    .map(|url| spawn(|| fetch(url)))
    .collect();
let results: Vec(T: Response) = join_all(handles);
```

#### Select: Wait for First

```spl
use std.task.{spawn, select, sleep};
use std.time.Duration;

// First to complete wins, others cancelled
let result = select(
    spawn(|| fetch(primary_url)),
    spawn(|| fetch(backup_url)),
);

// Timeout pattern
let result = select(
    spawn(|| slow_operation()),
    spawn(|| {
        sleep(Duration.from_secs(5));
        return Err(Error.Timeout);
    }),
);
```

#### Scope: Bounded Task Lifetime

```spl
use std.task.scope;

// All tasks must complete before scope exits
let results = scope(|s| {
    let h1 = s.spawn(|| fetch(url1));
    let h2 = s.spawn(|| fetch(url2));

    return (h1.await(), h2.await());
});
// Guaranteed: all tasks done or cancelled here
```

Scope guarantees:
- Tasks cannot outlive the scope
- On early return/panic, pending tasks are cancelled
- Resources referenced by tasks remain valid

---

### 9. FFI and Blocking

External function calls block the OS thread. SPL handles this automatically.

#### Default: extern fn is Blocking

```spl
// Automatically runs on blocking thread pool
extern fn sqlite_query(db: *mut c_void, sql: *const c_char): i32;

fn query(sql: String): i32 {
    // Runtime moves this to blocking pool
    // Other tasks continue on worker threads
    return sqlite_query(db, sql.as_ptr());
}
```

#### Opt-out for Fast FFI

```spl
// Fast FFI that doesn't need blocking treatment
#[non_blocking]
extern fn fast_crc32(data: *const u8, len: usize): u32;

fn checksum(data: &[u8]): u32 {
    // Runs directly on worker thread
    return fast_crc32(data.as_ptr(), data.len());
}
```

#### Explicit Blocking

For non-FFI blocking operations:

```spl
use std.task.spawn_blocking;
use std.fs;

let result = spawn_blocking(|| {
    // Runs on blocking thread pool
    fs.read_to_string("large_file.txt")
}).await();
```

#### Blocking Pool Behavior

- Separate thread pool from async workers
- Grows on demand (with configurable upper bound)
- Default max: 512 threads
- Tasks awaiting blocking results yield properly

---

### 10. Thread Affinity

For high-performance scenarios requiring thread pinning.

#### Pinned Scope

```spl
use std.task.scope_pinned;

// All tasks in scope run on specified core
scope_pinned(core = 0, |s| {
    s.spawn(|| handle_shard_0());
    s.spawn(|| process_local_data());
});
```

#### Local Scope

```spl
use std.task.scope_local;

// Tasks stay on current thread (no migration)
scope_local(|s| {
    // Can use !Send types safely
    s.spawn(|| use_thread_local_cache());
});
```

#### Use Cases

| Mode | Use Case |
|------|----------|
| Default (work-stealing) | General purpose |
| `scope_pinned(core)` | Sharded data, NUMA-aware |
| `scope_local` | Thread-local caches, !Send types |

#### Future Considerations

- Per-core executors for full isolation
- Soft affinity hints
- NUMA-aware automatic placement
- Core groups (pin to set of cores)

---

### 11. Runtime Configuration

#### Implicit Runtime for main

`fn main()` automatically runs inside a default runtime. No wrapper or attribute needed:

```spl
use std.task.spawn;

fn main(): () {
    // Already inside the runtime - can spawn tasks directly
    let handle = spawn(|| fetch("https://example.com"));
    let result = handle.await();
}
```

The default runtime uses sane defaults and respects environment variable overrides.

#### Customizing the Runtime

For custom configuration, use the `#[runtime]` attribute on main:

```spl
#[runtime(
    worker_threads = 4,
    blocking_threads = 256,
    stack_size = 8 * 1024,
)]
fn main(): () {
    // Runs with custom runtime settings
}
```

#### Programmatic Configuration

For dynamic configuration (e.g., based on config files), use `runtime.set_global()` at the start of main:

```spl
use std.runtime;
use std.task.spawn;

fn main(): () {
    // Must be called before spawning any tasks
    runtime.set_global(
        runtime.builder()
            .worker_threads(config.threads)
            .blocking_threads(config.blocking)
            .on_task_spawn(|info| metrics.record_spawn(info))
            .build()
    );

    // Now use the custom runtime
    let handle = spawn(|| work());
}
```

**Note:** `set_global()` panics if called after any task has been spawned, or if called more than once. This ensures runtime configuration is deterministic.

#### Builder API

```spl
use std.runtime;

runtime.builder()
    .worker_threads(4)           // Default: num_cpus
    .blocking_threads(512)       // Default: 512
    .stack_size(4 * 1024)        // Default: adaptive from 2KB
    .max_stack_size(1024 * 1024) // Default: 1MB
    .build()
```

#### Environment Variable Overrides

| Variable | Default | Description |
|----------|---------|-------------|
| `SPL_WORKER_THREADS` | num_cpus | Worker thread count |
| `SPL_BLOCKING_THREADS` | 512 | Max blocking threads |
| `SPL_MIN_STACK_SIZE` | 2048 | Minimum stack (bytes) |
| `SPL_MAX_STACK_SIZE` | 1048576 | Maximum stack (bytes) |
| `SPL_RUNTIME_DEBUG` | false | Enable debug logging |

Environment variables override builder settings (ops-friendly tuning without recompilation).

```bash
SPL_WORKER_THREADS=2 ./my_server
```

#### Diagnostic Hooks

```spl
use std.runtime;

fn main(): () {
    runtime.set_global(
        runtime.builder()
            .on_task_spawn(|info| log("Spawned: " + info.name))
            .on_task_complete(|info, duration| metrics.record(duration))
            .build()
    );

    run_server();
}
```

---

## Rationale

### Why No Function Coloring?

Go's success demonstrates that a unified concurrency model works. The "what color is your function" problem in Rust/JS creates:
- Ecosystem fragmentation (async vs sync libraries)
- Viral annotations spreading through codebases
- Cognitive overhead choosing between styles

**Tradeoff**: Compiler cannot optimize non-yielding code paths as aggressively.

### Why Growable Stacks?

Stackless coroutines (Rust's approach) require function coloring to know what can yield. With no coloring, we need stackful coroutines. Growable stacks provide:
- Low memory footprint (2KB start vs 1MB OS threads)
- No fixed limit on call depth
- Proven at scale (Go)

### Why Async Preemption?

Cooperative-only preemption allows tight loops to starve the scheduler. Go added async preemption in 1.14 after years of complaints. Adding it from day one avoids those issues.

### Why Drop = Cancel?

Prevents orphaned tasks and resource leaks. Users must explicitly opt into fire-and-forget with `.detach()`. This aligns with SPL's "explicit about costs" philosophy.

### Why Task Isolation?

Production servers need resilience. One bad request shouldn't crash the entire service. Task isolation with proper unwinding (destructors run) provides both safety and reliability.

### Why Auto-Blocking FFI?

Safe by default. Most FFI calls into C libraries may block unpredictably. Treating them as blocking prevents accidental scheduler starvation. Fast FFI can opt-out explicitly.

---

## Consequences

### Positive

- Simple mental model (no async/sync distinction)
- Production-ready isolation and resource cleanup
- Competitive performance with Go
- Explicit control when needed (pinning, configuration)
- Safe FFI interaction by default

### Negative

- Cannot statically reason about yield points
- Stack growth has overhead (prologue checks)
- Larger runtime than Rust's bring-your-own executor
- Unwinding machinery adds complexity and binary size

### Implementation Complexity

- Growable stack allocator with pointer fixup
- Async preemption via signals and safe points
- Work-stealing scheduler
- Blocking thread pool management
- Integration with I/O subsystem

---

## Comparison

| Feature | SPL | Go | Rust |
|---------|-----|-----|------|
| Function coloring | No | No | Yes |
| Stack model | Growable | Growable | Stackless |
| Preemption | Cooperative + async | Cooperative + async | Cooperative only |
| Runtime | Built-in | Built-in | External (tokio, etc.) |
| Task isolation | Yes (unwind) | Yes (recover) | Yes (catch_unwind) |
| Cancellation | Drop-based | Context | Drop-based |
| Channels | Stdlib | Built-in | Stdlib |

---

## Examples

### HTTP Server

```spl
use std.task.spawn;
use std.net.TcpListener;

fn main(): () {
    let listener = TcpListener.bind("127.0.0.1:8080");

    loop {
        let (stream, addr) = listener.accept();
        spawn(|stream| handle_connection(stream));
    }
}

fn handle_connection(stream: TcpStream): () {
    let request = read_request(stream);
    let response = process_request(request);
    stream.write_all(response.as_bytes());
}
```

### Parallel Processing with Scope

```spl
use std.task.scope;

fn process_batch(items: Vec(T: Item)): Vec(T: Result) {
    return scope(|s| {
        let handles: Vec(_) = items.iter()
            .map(|item| s.spawn(|item| process(item)))
            .collect();

        return handles.iter()
            .map(|h| h.await())
            .collect();
    });
}
```

### Producer-Consumer Pipeline

```spl
use std.task.spawn;
use std.channel;

fn main(): () {
    let (tx1, rx1) = channel.mpsc(100);
    let (tx2, rx2) = channel.mpsc(100);

    // Stage 1: Produce
    spawn(|tx1| {
        for i in 0..1000 {
            tx1.send(generate(i));
        }
    });

    // Stage 2: Transform
    spawn(|rx1, tx2| {
        while let Some(item) = rx1.recv() {
            tx2.send(transform(item));
        }
    });

    // Stage 3: Consume
    while let Some(item) = rx2.recv() {
        save(item);
    }
}
```

### Timeout with Retry

```spl
use std.task.{spawn, select, sleep};
use std.time.Duration;

fn fetch_with_retry(url: String, max_retries: i32): Result(T: Response, E: Error) {
    for attempt in 1..=max_retries {
        let result = select(
            spawn(|~url| fetch(url)),
            spawn(|| {
                sleep(Duration.from_secs(5));
                return Err(Error.Timeout);
            }),
        );

        match result {
            Ok(response) => return Ok(response),
            Err(e) if attempt < max_retries => {
                sleep(Duration.from_millis(100 * attempt));
                continue;
            },
            Err(e) => return Err(e),
        }
    }
}
```

---

## Open Questions

1. **Async trait methods** - How do traits with potentially-yielding methods work?
2. **Async drop** - Should destructors be able to yield for cleanup?
3. **Priority inversion** - How to handle priority inheritance with mutexes?
4. **Distributed tracing** - Built-in context propagation for observability?

---

## References

- [ADR-011: Iteration and Generators](011-iteration-and-generators.md)
- [ADR-012: Closures and Capture Semantics](012-closures.md)
- [Go Stack Evolution](https://medium.com/a-journey-with-go/go-how-does-the-goroutine-stack-size-evolve-447fc02085e5)
- [Go Async Preemption](https://medium.com/a-journey-with-go/go-asynchronous-preemption-b5194227371c)
- [Go Non-cooperative Preemption Proposal](https://go.googlesource.com/proposal/+/master/design/24543-non-cooperative-preemption.md)
- [Java Project Loom](https://openjdk.org/projects/loom/)
- [Structured Concurrency](https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/)
- [What Color is Your Function](https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/)
