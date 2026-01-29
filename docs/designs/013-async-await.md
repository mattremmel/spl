# ADR-013: Async/Await and Runtime Model

**Status:** Draft
**Date:** 2026-01-28

## Context

SPL needs a concurrency story. The keywords `async` and `await` are already reserved (see [lexical-grammar.md](../spec/lexical-grammar.md)), and DECISIONS.md specifies "Built-in runtime (Go-style simplicity). `async fn main()` just works."

### Goals

1. **Simplicity**: No runtime selection, no async traits, no Pin/Unpin complexity
2. **Safety**: Leverage second-class references to avoid lifetime issues
3. **Efficiency**: Zero-cost abstractions where possible, predictable allocation
4. **Composability**: Work well with generators (ADR-011) and closures (ADR-012)

### Constraints from Existing Decisions

- **Second-class references**: References can only be function parameters, never stored or returned. This means references cannot be held across await points naturally.
- **Move-by-default**: Escaping closures move captures. Async blocks are escaping.
- **Panic = abort**: No unwinding simplifies cancellation.
- **Generators**: Already designed (ADR-011). Async shares implementation.

## Decision

### 1. Async Functions

Async functions are declared with the `async` keyword before `fn`:

```spl
async fn fetch_data(url: String): Result(Data, Error) {
    let response = http.get(url).await?;
    let data = response.json().await?;
    return Ok(data);
}

async fn main() {
    let result = fetch_data("https://api.example.com").await;
    match result {
        Ok(data) => println(data),
        Err(e) => println("Error: " + e.message()),
    }
}
```

**Semantics:**
- Calling an async function returns a `Future(T)` immediately
- The function body doesn't execute until the future is awaited or spawned
- `async fn main()` is the program entry point when using async

### 2. Await Expression

The `.await` postfix operator suspends execution until a future completes:

```spl
let result = some_async_fn().await;
```

**Postfix rationale**: Postfix (`.await`) chains naturally with method calls and `?`:

```spl
// Clean chaining
let data = client
    .get(url)
    .await?
    .json()
    .await?;

// Compare to prefix (rejected):
// let data = await (await client.get(url))?.json()?;
```

### 3. Async Blocks

Async blocks create anonymous futures that capture variables from the enclosing scope:

```spl
let url = String::from("https://example.com");
let timeout = Duration::from_secs(30);

// Basic async block - captures by move (default)
let future = async {
    return fetch_with_timeout(url, timeout).await;
};
// url and timeout are moved into the future
```

**Capture semantics** (consistent with escaping closures):
- **Move by default**: Non-Copy types are moved into the future
- **Copy types copied**: `i32`, `bool`, etc. are copied automatically
- **Clone shorthand**: Use `async clone { ... }` to clone all captures

```spl
// Clone all captured variables
let client = Arc::new(HttpClient::new());
let url = String::from("https://example.com");

let future = async clone {
    return client.get(url).await;
};
// client and url still valid here (were cloned)
```

**Explicit capture list**: For per-variable control, use bracket syntax:

```spl
let client = Arc::new(HttpClient::new());
let url = String::from("https://example.com");

// ~client = clone, url = move
let future = async [~client, url] {
    return client.get(url).await;
};
// client still valid (cloned), url moved
```

### Async Closures

Async closures are closures that return futures. They take parameters AND can capture:

```spl
// Async closure with parameter
let fetch = async fn(url: String): Response {
    return http.get(url).await;
};

// Call the async closure
let response = fetch("https://example.com").await;

// Async closure with captures
let client = Arc::new(HttpClient::new());
let fetch = async [~client] fn(url: String): Response {
    return client.get(url).await;
};
```

**Note**: Async closures use `async fn(...)` syntax to distinguish from async blocks.

### 4. Future Type

`Future(T)` is the type of an async computation that will eventually produce `T`:

```spl
// Future is a built-in type
async fn compute(): i32 {
    return 42;
}

let f: Future(i32) = compute();  // Returns immediately
let value: i32 = f.await;        // Blocks until complete
```

Futures are:
- **Lazy**: Don't start executing until awaited or spawned
- **Single-completion**: Produce exactly one value
- **Cancellable**: Dropping a future cancels it (see Cancellation)

### 5. Built-in Runtime

SPL includes a built-in async runtime. No configuration needed:

```spl
async fn main() {
    // Just works - runtime is built-in
    let result = fetch_data().await;
}
```

**Runtime characteristics:**
- Work-stealing thread pool (defaults to number of CPU cores)
- Cooperative scheduling at await points
- Automatic I/O integration (async file, network, timers)

**No runtime selection**: Unlike Rust (tokio, async-std, smol), SPL has exactly one runtime. This enables:
- Ecosystem compatibility (all async libraries work together)
- Simpler learning curve
- Consistent behavior across all code

### 6. Spawning Tasks

Spawn concurrent tasks that run independently:

```spl
async fn main() {
    // spawn returns a JoinHandle
    let handle = spawn(fetch_data("url1"));

    // Do other work concurrently
    let result2 = fetch_data("url2").await;

    // Wait for spawned task
    let result1 = handle.await;
}
```

**spawn with captured data:**

```spl
// Spawn a future that captures data
let data = load_data();
let handle = spawn(async {
    return process(data).await;  // data moved into future
});

// Or with explicit clone
let shared_data = Arc::new(load_data());
let handle = spawn(async clone {
    return process(shared_data).await;  // shared_data cloned
});
```

### 7. Structured Concurrency

SPL provides structured concurrency primitives for safe concurrent execution:

#### join - Wait for all

```spl
async fn fetch_all(urls: Vec(String)): Vec(Result(Data, Error)) {
    // join! waits for all futures, returns tuple
    let (a, b, c) = join!(
        fetch(urls[0]),
        fetch(urls[1]),
        fetch(urls[2]),
    );

    return vec![a, b, c];
}
```

#### select - Wait for first

```spl
async fn fetch_with_timeout(url: String, timeout: Duration): Result(Data, Error) {
    // select! returns when first future completes, cancels others
    select! {
        result = fetch(url) => result,
        _ = sleep(timeout) => Err(Error::Timeout),
    }
}
```

#### Scoped tasks

```spl
async fn parallel_process(items: Vec(Item)): Vec(Result) {
    // scope ensures all spawned tasks complete before returning
    let results = scope(async fn(s: Scope) {
        let handles: Vec(_) = items.iter()
            .map(|item| {
                let item = item;  // Move into closure
                s.spawn(async { process(item).await })
            })
            .collect();

        // Wait for all
        return handles.iter()
            .map(|h| h.await)
            .collect();
    }).await;

    return results;
}
```

Scoped tasks guarantee:
- All tasks complete (or are cancelled) before the scope exits
- References to scope-local data are safe (though still second-class)

### 8. Channels

Channels provide communication between async tasks:

#### Bounded channels

```spl
async fn producer_consumer() {
    // Bounded channel (backpressure)
    let (tx, rx) = channel(10);  // Buffer size 10

    // Producer - tx moved into the async block
    spawn(async {
        for i in 0..100 {
            tx.send(i).await;  // Waits if buffer full
        }
    });

    // Consumer
    while let Some(value) = rx.recv().await {
        process(value);
    }
}
```

#### Unbounded channels

```spl
let (tx, rx) = unbounded_channel();  // No backpressure
```

#### Multiple producers/consumers

```spl
let (tx, rx) = channel(10);
let tx2 = tx.clone();  // Multiple producers

// Receivers are NOT cloneable - single consumer
// For multiple consumers, use a different pattern
```

#### oneshot channels

```spl
// Single value, single use
let (tx, rx) = oneshot();
spawn(async {
    let result = compute().await;
    tx.send(result);  // Only one send allowed, tx moved in
});
let value = rx.await;  // Receives the single value
```

### 9. Synchronization Primitives

#### Mutex

```spl
let counter = Arc::new(Mutex::new(0));

async fn increment(counter: Arc(Mutex(i32))) {
    let mut guard = counter.lock().await;
    *guard += 1;
    // guard dropped, lock released
}
```

**Note**: Mutex guards cannot be held across await points (see Reference Safety).

#### RwLock

```spl
let data = Arc::new(RwLock::new(HashMap::new()));

async fn read(data: Arc(RwLock(HashMap(String, i32)))): Option(i32) {
    let guard = data.read().await;
    return guard.get("key").copied();
}

async fn write(data: Arc(RwLock(HashMap(String, i32))), key: String, value: i32) {
    let mut guard = data.write().await;
    guard.insert(key, value);
}
```

#### Semaphore

```spl
let sem = Arc::new(Semaphore::new(10));  // 10 permits

async fn limited_operation(sem: Arc(Semaphore)) {
    let permit = sem.acquire().await;
    // Do work while holding permit
    do_work().await;
    // permit dropped, returned to semaphore
}
```

### 10. Cancellation

Futures can be cancelled by dropping them:

```spl
async fn cancellable_work() {
    let handle = spawn(long_running_task());

    // Cancel after timeout
    if timeout_elapsed() {
        drop(handle);  // Task is cancelled
        return;
    }

    let result = handle.await;
}
```

**Cancellation semantics:**
- Dropping a future cancels it at the next await point
- Already-running code between awaits completes
- Destructors run for any owned values
- Since panic = abort, no unwinding complexity

#### Cancellation tokens

For explicit cancellation checking:

```spl
async fn long_task(cancel: CancellationToken) {
    for i in 0..1000 {
        // Check for cancellation
        if cancel.is_cancelled() {
            return;  // Clean exit
        }

        do_work(i).await;
    }
}

// Usage
let token = CancellationToken::new();
let handle = spawn(long_task(token.clone()));

// Later...
token.cancel();  // Signal cancellation
handle.await;    // Task exits cleanly
```

### 11. Reference Safety Across Await Points

Second-class references naturally prevent holding references across await points:

```spl
async fn safe() {
    let data = vec![1, 2, 3];

    // References are fine within a single "segment"
    let r = &data[0];
    println(*r);

    // Await point here
    some_async_op().await;

    // Cannot use 'r' here - it doesn't exist
    // (This is natural - r was a local, not stored anywhere)

    // Must re-borrow
    let r2 = &data[0];
    println(*r2);
}
```

Because references cannot be stored (second-class), there's no way to accidentally hold a reference across an await. The future's state machine only stores owned values.

**Compare to Rust**: Rust requires complex `Pin`/`Unpin` machinery because futures can hold references. SPL avoids this entirely.

### 12. Async Generators

Async generators combine async and generators to produce async streams:

```spl
async gen fn fetch_pages(url: String): Page {
    let mut page = 1;
    loop {
        let response = http.get(url + "?page=" + page.to_string()).await;
        if response.is_empty() {
            break;
        }
        yield response.into_page();
        page += 1;
    }
}

// Consuming an async generator
async fn process_all_pages(url: String) {
    for page in fetch_pages(url).await {
        process(page);
    }
}
```

**Stream type:**

```spl
// async gen fn returns a Stream
async gen fn numbers(): i32 { ... }
let s: Stream(i32) = numbers();

// Streams are async iterators
while let Some(n) = s.next().await {
    println(n);
}
```

### 13. Error Handling

Async functions work naturally with `Result` and `?`:

```spl
async fn fetch_and_parse(url: String): Result(Data, Error) {
    let response = http.get(url).await?;  // Propagate error
    if response.status != 200 {
        return Err(Error::HttpError(response.status));
    }
    let data = parse(response.body)?;  // Sync error propagation
    return Ok(data);
}
```

**Panic in async:**
- Panic aborts the entire program (SPL's panic = abort policy)
- No special handling needed for panics in spawned tasks
- Use `Result` for recoverable errors

### 14. Timeouts and Deadlines

Built-in timeout support:

```spl
async fn with_timeout() {
    // Timeout wrapper
    match timeout(Duration::from_secs(5), slow_operation()).await {
        Ok(result) => println("Got: " + result.to_string()),
        Err(Elapsed) => println("Timed out"),
    }

    // Or use select
    select! {
        result = slow_operation() => handle(result),
        _ = sleep(Duration::from_secs(5)) => println("Timeout"),
    }
}
```

### 15. I/O Integration

The runtime provides async I/O primitives:

```spl
use std.fs.File;
use std.net.TcpStream;

async fn file_example() {
    let file = File::open("data.txt").await?;
    let contents = file.read_to_string().await?;
}

async fn network_example() {
    let stream = TcpStream::connect("example.com:80").await?;
    stream.write_all(b"GET / HTTP/1.1\r\n\r\n").await?;
    let response = stream.read_to_string().await?;
}
```

---

## Rationale

### Why Built-in Runtime?

**Go's success**: Go's built-in runtime is a major reason for its popularity in concurrent/networked applications. Developers don't need to choose between competing runtimes or worry about compatibility.

**Rust's pain point**: Rust's async ecosystem fragmentation (tokio vs async-std vs smol) causes:
- Library compatibility issues
- Learning curve (which runtime to choose?)
- Feature fragmentation

SPL avoids this by providing exactly one runtime.

### Why No Pin/Unpin?

Rust's `Pin` exists to enable self-referential futures (futures holding references to themselves). SPL's second-class references make this impossible:

- References can only be function parameters
- Futures cannot store references
- No self-referential data structures

This is a fundamental simplification that eliminates an entire category of complexity.

### Why Postfix .await?

1. **Chains with ?**: `foo().await?` reads left-to-right
2. **Chains with methods**: `response.json().await`
3. **Consistent with other postfix**: `.await` joins `?`, `[]`, `()` as postfix operators

### Why Explicit Captures in Async Blocks?

Async blocks are escaping (the future outlives its creation context). Following closure semantics (ADR-012):

- Move by default prevents hidden clones
- `~` makes cloning explicit
- Consistent mental model with closures

### Why Structured Concurrency?

Unstructured spawning (fire-and-forget tasks) leads to:
- Resource leaks
- Orphaned tasks
- Difficult reasoning about program state

Structured concurrency (`scope`, `join!`, `select!`) ensures:
- Tasks have clear ownership
- Resources are cleaned up predictably
- Program flow is understandable

---

## Consequences

### Positive

- **Simple mental model**: No runtime selection, no Pin/Unpin
- **Ecosystem coherence**: All async code compatible
- **Reference safety for free**: Second-class refs prevent issues
- **Consistent with closures**: Same capture semantics
- **Structured by default**: Safe concurrent patterns

### Negative

- **Less flexibility**: Can't choose specialized runtime
- **Runtime overhead**: Even sync-only programs include runtime
- **Different from Rust**: Learning curve for Rust users
- **Binary size**: Built-in runtime adds baseline size

### Implementation Complexity

- **Coroutine transformation**: Share implementation with generators
- **Runtime**: Work-stealing scheduler, I/O integration
- **Compiler support**: async fn desugaring, Future type

---

## Comparison with Other Languages

| Feature | SPL | Rust | Go | Kotlin |
|---------|-----|------|----|----|
| Runtime | Built-in | External | Built-in | Built-in |
| Syntax | async/await | async/await | goroutines | suspend |
| Channels | Yes | Library | Built-in | Built-in |
| Structured concurrency | Yes | Library | No | Yes |
| Cancellation | Drop-based | Drop-based | Context | Cooperative |
| Reference across await | N/A (impossible) | Pin required | N/A | N/A |

---

## Open Questions

1. **Thread-local async?** Should there be a single-threaded mode for simpler applications?

2. **Async trait methods?** How do traits with async methods work? (May need separate ADR)

3. **Async drop?** Should destructors be async-aware for cleanup?

4. **Blocking in async context?** How to handle/prevent blocking calls in async code?

5. **Priority scheduling?** Should tasks have priorities?

---

## Examples

### Simple HTTP Server

```spl
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (stream, addr) = listener.accept().await?;
        spawn(handle_connection(stream));
    }
}

async fn handle_connection(stream: TcpStream) {
    let request = read_request(stream).await;
    let response = process_request(request).await;
    stream.write_all(response.as_bytes()).await;
}
```

### Parallel Web Scraper

```spl
async fn scrape_all(urls: Vec(String)): Vec(Result(Page, Error)) {
    let results = scope(async fn(s: Scope) {
        let handles: Vec(_) = urls.iter()
            .map(|url| {
                let url = url.clone();
                s.spawn(async { fetch_and_parse(url).await })
            })
            .collect();

        return handles.iter()
            .map(|h| h.await)
            .collect();
    }).await;

    return results;
}
```

### Producer-Consumer Pipeline

```spl
async fn pipeline() {
    let (tx1, rx1) = channel(100);
    let (tx2, rx2) = channel(100);

    // Stage 1: Produce data (tx1 moved in)
    spawn(async {
        for i in 0..1000 {
            tx1.send(generate_item(i)).await;
        }
    });

    // Stage 2: Transform (rx1 and tx2 moved in)
    spawn(async {
        while let Some(item) = rx1.recv().await {
            tx2.send(transform(item)).await;
        }
    });

    // Stage 3: Consume (rx2 moved in)
    while let Some(item) = rx2.recv().await {
        save(item).await;
    }
}
```

### Timeout with Retry

```spl
async fn fetch_with_retry(
    url: String,
    max_retries: i32,
    timeout: Duration,
): Result(Response, Error) {
    let mut attempts = 0;

    loop {
        attempts += 1;

        match timeout(timeout, http.get(url.clone())).await {
            Ok(Ok(response)) => {
                return Ok(response);
            },
            Ok(Err(e)) if attempts < max_retries => {
                sleep(Duration::from_millis(100 * attempts)).await;
                continue;
            },
            Ok(Err(e)) => {
                return Err(e);
            },
            Err(Elapsed) if attempts < max_retries => {
                continue;
            },
            Err(Elapsed) => {
                return Err(Error::Timeout);
            },
        }
    }
}
```

---

## References

- [ADR-011: Iteration and Generators](011-iteration-and-generators.md)
- [ADR-012: Closures and Capture Semantics](012-closures.md)
- [DECISIONS.md §6.4](../DECISIONS.md) - Async/Await decision
- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [Go Concurrency Patterns](https://go.dev/blog/pipelines)
- [Kotlin Coroutines](https://kotlinlang.org/docs/coroutines-overview.html)
- [Structured Concurrency (Nathaniel Smith)](https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/)
