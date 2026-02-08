# Concurrency

This document specifies SPL's concurrency model, including task spawning, channels, synchronization primitives, and the runtime.

## Overview

SPL uses a **Go-style concurrency model**:
- **No function coloring** - Any function can yield; no async/sync divide
- **Built-in runtime** - M:N threading with work-stealing scheduler
- **Task isolation** - Tasks don't share mutable state by default
- **Channels** - Primary communication mechanism between tasks

## 1. Task Spawning

### 1.1 Basic Spawning

```spl
use std.task.spawn;

// Spawn a task
let handle = spawn(|| {
    // Task body
    return compute_result();
});

// Wait for result
let result = handle.await();
```

**Signature:**

```spl
fn spawn(f: F): JoinHandle(T: T) where F: FnOnce(): T + Send, T: Send
```

Task closures must capture only `Send` values, and the return type must be `Send`, since the value is transferred across task boundaries.

### 1.2 JoinHandle

`spawn` returns a `JoinHandle(T: T)` for the task's result:

```spl
struct JoinHandle(T) where T: Send {
    // Wait for task completion, get result
    fn await(self): T;

    // Wait with timeout
    fn await_timeout(self, duration: Duration): Option(T: T);

    // Wait, returning error if task panicked or was cancelled
    fn try_await(self): Result(T: T, E: TaskError);

    // Check if task has completed
    fn is_finished(&self): bool;

    // Cancel the task
    fn cancel(&self): ();

    // Detach: let task run to completion without joining
    fn detach(self): ();
}

enum TaskError {
    Panicked(String),   // Task panicked with message
    Cancelled,          // Task was cancelled
}
```

### 1.3 Task Closure Captures

Task closures follow escaping closure rules (see [closures.md](closures.md)):

```spl
use std.task.spawn;

let data = load_data();
let config = Config.new();

// data is moved, config is cloned via capture list
let handle = spawn(@[config: config.clone()] || {
    return process(data, config);
});
```

### 1.4 Spawning with Options

```spl
use std.task.{spawn, TaskOptions};

let handle = spawn(
    TaskOptions(name: "worker-1", stack_size: 1024 * 1024),
    || work()
);
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `name` | `String?` | `None` | Task name for debugging |
| `stack_size` | `usize?` | `None` | Initial stack size |

---

## 2. Channels

Channels are the primary mechanism for communication between tasks.

### 2.1 Unbounded Channels

```spl
use std.channel.unbounded;

let (tx, rx) = unbounded();

spawn(|| {
    for i in 0..10 {
        tx.send(i);
    }
});

for value in rx {
    println("{}", value);
}
```

### 2.2 Bounded Channels

```spl
use std.channel.bounded;

// Buffer up to 10 messages
let (tx, rx) = bounded(10);

spawn(|| {
    for i in 0..100 {
        tx.send(i);  // Blocks when buffer is full
    }
});

while rx.recv() is Some(value) {
    process(value);
}
```

### 2.3 Channel Types

```spl
// Sender side
struct Sender(T) where T: Send {
    fn send(&self, value: T): ();
    fn try_send(&self, value: T): Result(T: (), E: SendError(T: T));
    fn is_closed(&self): bool;
}

// Receiver side
struct Receiver(T) where T: Send {
    fn recv(&self): Option(T: T);
    fn try_recv(&self): Result(T: T, E: TryRecvError);
    fn is_empty(&self): bool;
}

enum SendError(T) where T: Send {
    Disconnected(value: T),  // Channel closed, returns unsent value
    Full(value: T),          // Bounded channel full (try_send only)
}

enum TryRecvError {
    Empty,        // No message available
    Disconnected, // Channel closed
}
```

Values sent through channels cross task boundaries, so channel element types must be `Send`. This ensures thread-safety for all cross-task value transfers.

### 2.4 Multiple Producers

Senders can be cloned for multiple producers:

```spl
let (tx, rx) = unbounded();

for i in 0..4 {
    let tx = tx.clone();
    spawn(|| {
        tx.send(format("from worker {}", i));
    });
}

drop(tx);  // Drop original sender

for msg in rx {
    println(msg);
}
```

### 2.5 Multiple Consumers

For multiple consumers, use a dedicated type:

```spl
use std.channel.broadcast;

let (tx, rx) = broadcast(16);  // Broadcast channel

let rx1 = rx.clone();
let rx2 = rx.clone();

spawn(|| {
    for msg in rx1 {
        handle_a(msg);
    }
});

spawn(|| {
    for msg in rx2 {
        handle_b(msg);
    }
});

tx.send(message);  // Both receivers get it
```

### 2.6 One-shot Channels

For single-value communication:

```spl
use std.channel.oneshot;

let (tx, rx) = oneshot();

spawn(|| {
    let result = expensive_computation();
    tx.send(result);  // Can only be called once
});

let value = rx.await();  // Wait for the single value
```

---

## 3. Select

`select` waits on multiple channel operations.

> **Note:** `select` is a compiler-recognized function in `std.task` with special support for its block argument, similar to how `format`/`print` are compiler intrinsics. No new keyword is needed -- `select` is not a reserved word but receives compiler support for validating and transforming its block syntax into the appropriate channel multiplexing code.

### 3.1 Basic Select

```spl
use std.task.select;

select {
    msg = rx1.recv() => {
        handle_a(msg);
    },
    msg = rx2.recv() => {
        handle_b(msg);
    },
}
```

### 3.2 Select with Send

```spl
select {
    tx.send(value) => {
        println("sent");
    },
    msg = rx.recv() => {
        println("received: {}", msg);
    },
}
```

### 3.3 Select with Default

Non-blocking select:

```spl
select {
    msg = rx.recv() => {
        handle(msg);
    },
    default => {
        // No message ready
        do_other_work();
    },
}
```

### 3.4 Select with Timeout

```spl
use std.time.Duration;

select {
    msg = rx.recv() => {
        handle(msg);
    },
    timeout(Duration.from_secs(5)) => {
        println("timed out");
    },
}
```

### 3.5 Select in Loop

```spl
loop {
    select {
        cmd = cmd_rx.recv() => {
            if cmd is Command.Quit {
                break;
            }
            execute(cmd);
        },
        data = data_rx.recv() => {
            process(data);
        },
        timeout(Duration.from_secs(60)) => {
            heartbeat();
        },
    }
}
```

---

## 4. Synchronization Primitives

### 4.1 Mutex

```spl
use std.sync.Mutex;

let counter = Mutex.new(0);

// Scoped locking
{
    let mut guard = counter.lock();
    *guard += 1;
}  // Lock released here

// Or with closure
counter.with_lock(|value| {
    *value += 1;
});
```

### 4.2 RwLock

```spl
use std.sync.RwLock;

let data = RwLock.new(Vec.new());

// Multiple readers
{
    let guard = data.read();
    println("length: {}", guard.len());
}

// Exclusive writer
{
    let mut guard = data.write();
    guard.push(42);
}
```

### 4.3 Atomic Types

```spl
use std.sync.atomic.{AtomicU64, Ordering};

let counter = AtomicU64.new(0);

counter.fetch_add(1, Ordering.SeqCst);
let value = counter.load(Ordering.SeqCst);

// Compare-and-swap
let old = counter.compare_exchange(
    expected: 5,
    new: 10,
    success: Ordering.SeqCst,
    failure: Ordering.SeqCst
);
```

Available atomic types:
- `AtomicBool`
- `AtomicI8`, `AtomicI16`, `AtomicI32`, `AtomicI64`, `AtomicIsize`
- `AtomicU8`, `AtomicU16`, `AtomicU32`, `AtomicU64`, `AtomicUsize`
- `AtomicPtr(T: T)`

**Memory Ordering:**

```spl
enum Ordering {
    Relaxed,  // No ordering constraints
    Acquire,  // Subsequent reads see writes before the paired Release
    Release,  // Prior writes become visible to the paired Acquire
    AcqRel,   // Combined Acquire + Release
    SeqCst,   // Total ordering across all threads
}
```

| Ordering | Use case |
|----------|----------|
| `Relaxed` | Counters, statistics — no cross-variable ordering needed |
| `Acquire` | Reading a lock/flag before accessing shared data |
| `Release` | Writing shared data before releasing a lock/flag |
| `AcqRel` | Read-modify-write operations that both acquire and release |
| `SeqCst` | Default. Use when unsure — provides strongest guarantees |

When in doubt, use `Ordering.SeqCst`. Weaker orderings are an optimization for performance-critical code.

### 4.4 Once

```spl
use std.sync.Once;

static INIT: Once = Once.new();
static mut CONFIG: Config? = None;

fn get_config(): &Config {
    INIT.call_once(|| {
        unsafe {
            CONFIG = Some(load_config());
        }
    });
    unsafe { CONFIG.as_ref().unwrap() }
}
```

### 4.5 Barrier

```spl
use std.sync.Barrier;

let barrier = Barrier.new(4);

for i in 0..4 {
    let barrier = barrier.clone();
    spawn(|| {
        work_phase_1(i);
        barrier.wait();  // All tasks sync here
        work_phase_2(i);
    });
}
```

### 4.6 Condition Variables

```spl
use std.sync.{Mutex, Condvar};

let pair = (Mutex.new(false), Condvar.new());
let (lock, cvar) = pair;

// Waiting task
let lock_clone = lock.clone();
let cvar_clone = cvar.clone();
spawn(|| {
    let mut guard = lock_clone.lock();
    while !*guard {
        guard = cvar_clone.wait(guard);
    }
    println("condition met!");
});

// Notifying task
{
    let mut guard = lock.lock();
    *guard = true;
    cvar.notify_one();
}
```

**Condition variable semantics:**

- **Lock release during wait:** `cvar.wait(guard)` atomically releases the mutex and suspends the task. When the task is woken, the mutex is re-acquired before `wait()` returns.
- **Spurious wakeups:** `wait()` may return without a corresponding `notify_one()` or `notify_all()`. Always check the condition in a loop (as shown above with `while !*guard`).
- **Mutex drop while waiting:** If the mutex is dropped while a task is waiting on the condvar, the waiting task panics on re-acquire. Ensure the mutex outlives all condvar waiters.

---

## 5. Task Cancellation

### 5.1 Cancellation via Handle

```spl
let handle = spawn(|| long_running_work());

// Cancel the task
handle.cancel();

// Wait and check result
match handle.try_await() {
    Ok(result) => println("completed: {}", result),
    Err(TaskError.Cancelled) => println("was cancelled"),
    Err(TaskError.Panicked(msg)) => println("panicked: {}", msg),
}
```

### 5.2 Cancellation Tokens

For cooperative cancellation:

```spl
use std.task.CancellationToken;

let token = CancellationToken.new();
let token_clone = token.clone();

let handle = spawn(|| {
    while !token_clone.is_cancelled() {
        do_work();
    }
    return partial_result();
});

// Later...
token.cancel();
let result = handle.await();
```

### 5.3 Cancellation Semantics

When a task is cancelled:
1. Cancellation flag is set
2. Task continues until next yield point
3. At yield point, task unwinds (destructors run)
4. `JoinHandle.try_await()` returns `Err(TaskError.Cancelled)`

**Yield points:**
- Channel operations (`send`, `recv`)
- `task.yield_now()`
- Explicit cancellation checks
- I/O operations

**Panic vs cancellation ordering:**

- A panic takes precedence over a pending cancellation. If a task panics while a cancellation is pending, the panic is reported, not the cancellation.
- `try_await()` returns `Err(TaskError.Panicked(msg))`, not `Err(TaskError.Cancelled)`, in this case.
- A pending cancellation is ignored during panic unwinding — destructors run to completion as part of the panic, not the cancellation.

---

## 6. Scoped Tasks

### 6.1 Task Scope

Scoped tasks ensure all spawned tasks complete before the scope exits:

```spl
use std.task.scope;

let data = vec![1, 2, 3, 4];

scope(|s| {
    for chunk in data.chunks(2) {
        s.spawn(|| {
            process_chunk(chunk);
        });
    }
});  // All tasks complete before this line

// Safe to use data here
```

### 6.2 Borrowing in Scoped Tasks

Scoped tasks can borrow **immutably** from the enclosing scope. Multiple tasks may share read-only access to the same data:

```spl
let data = vec![1, 2, 3, 4, 5, 6];

scope(|s| {
    for chunk in data.chunks(2) {
        s.spawn(|| {
            // Each task borrows its chunk immutably
            let sum = chunk.iter().sum();
            sum
        });
    }
});
// data is still valid here
```

For **shared mutable state**, use a synchronization primitive such as `Mutex`:

```spl
use std.sync.Mutex;

let results = Mutex.new(Vec.new());

scope(|s| {
    s.spawn(|| {
        let value = compute_a();
        results.lock().push(value);
    });
    s.spawn(|| {
        let value = compute_b();
        results.lock().push(value);
    });
});

// results contains both values
let final_results = results.into_inner();
```

**Note:** Shared mutation across concurrent tasks always requires synchronization. The borrow checker rejects multiple mutable borrows to the same data, so patterns like two tasks both calling `results.push()` on a bare `Vec` are compile errors. Use `Mutex`, `RwLock`, or channels to coordinate writes.

### 6.3 Scoped vs Unscoped Task Closures

`spawn()` and `s.spawn()` (within `scope()`) have different closure requirements:

| | `spawn()` | `s.spawn()` (scoped) |
|---|---|---|
| **Closure kind** | Escaping — must own all captures | Non-escaping within scope — can borrow immutably |
| **`Send` bound** | Required (`F: Send`, `T: Send`) | Not required (tasks join before scope exits) |
| **Mutable shared state** | Via `Arc(T: Mutex(T: T))` or channels | Via `Mutex` (no `Arc` needed — scope guarantees lifetime) |
| **Lifetime** | Unbounded — task may outlive caller | Bounded — all tasks complete before `scope()` returns |

Because scoped tasks are guaranteed to complete before the enclosing `scope()` call returns, they can borrow from the enclosing stack frame without requiring `Send` or ownership transfer. This is the same non-escaping closure mechanism described in [closures.md](closures.md) §5.1 -- the borrow exists only for the duration of the `scope()` call.

**`Sync` requirement for shared references:** While `Send` is not required for scoped tasks (data stays within the creating thread's logical scope and is not transferred), any data accessed via shared reference (`&T`) by multiple scoped tasks must be `Sync`. This is because multiple tasks may read concurrently from different OS threads, and `Sync` guarantees that `&T` is safe to share across threads. For example, `&Vec(T: i32)` is safe (since `Vec(T: i32)` is `Sync`), but `&Cell(T: i32)` is not (since `Cell` is not `Sync` -- it allows mutation through `&self` without synchronization).

See also [memory-model.md](memory-model.md) §8 for scoped type semantics.

---

## 7. Runtime

### 7.1 Runtime Model

SPL uses an **M:N threading model**:
- M tasks (lightweight, user-space)
- N OS threads (typically equal to CPU cores)
- Work-stealing scheduler for load balancing

### 7.2 Stack Management

Tasks use **growable stacks**:
- Start small (e.g., 8KB)
- Grow on demand
- Shrink when no longer needed
- Adaptive sizing based on task history

### 7.3 Preemption

SPL uses **cooperative preemption with async preemption backup**:

1. **Cooperative:** Tasks yield at natural points (I/O, channel ops)
2. **Async backup:** Long-running computations are preempted via signals

```spl
// This tight loop won't starve other tasks
fn compute(): () {
    for i in 0..1000000000 {
        // Runtime may preempt here after ~10ms
        work(i);
    }
}
```

### 7.4 Task-Local Storage

Per-task storage allows each task to maintain its own instance of a value. Task-local storage is provided as a library function via `std.task.local`:

```spl
use std.task.local;

let CACHE = local(|| RefCell.new(HashMap.new()));

fn cached_lookup(key: &str): i32 {
    CACHE.with(|cache| {
        cache.borrow_mut()
            .entry(key.to_string())
            .or_insert_with(|| expensive_compute(key))
            .clone()
    })
}
```

`local(init)` declares a task-local variable initialized lazily by `init` on first access in each task. The `.with()` callback provides a scoped, non-escaping reference to the task's instance — the reference cannot escape the closure (consistent with SPL's second-class reference rules). Task-local values are dropped when the task completes.

---

## 8. Panic Handling in Tasks

### 8.1 Task Isolation

Panics in one task don't affect other tasks:

```spl
let handle = spawn(|| {
    panic("oops!");
});

// Main task continues
let result = handle.try_await();  // Returns Err(TaskError.Panicked("oops!"))
```

### 8.2 Panic Propagation

By default, panics are contained. To propagate:

```spl
let handle = spawn(|| risky_operation());

// Propagate panic if task panicked
handle.await();  // Panics if the task panicked
```

### 8.3 Catch Panics

```spl
use std.panic.catch_unwind;

let result = catch_unwind(|| {
    potentially_panicking_code();
});

match result {
    Ok(value) => println("success: {}", value),
    Err(panic_info) => println("caught panic"),
}
```

---

## 9. Common Patterns

### 9.1 Worker Pool

```spl
use std.channel.bounded;
use std.task.spawn;

fn worker_pool(jobs: Receiver(T: Job), results: Sender(T: JobResult), n: usize): () {
    for _ in 0..n {
        let jobs = jobs.clone();
        let results = results.clone();
        spawn(|| {
            for job in jobs {
                let result = process(job);
                results.send(result);
            }
        });
    }
}
```

### 9.2 Fan-Out/Fan-In

```spl
fn fan_out_fan_in(input: Vec(T: Item)): Vec(T: Output) {
    let (result_tx, result_rx) = unbounded();

    for item in input {
        let result_tx = result_tx.clone();
        spawn(|| {
            let result = process(item);
            result_tx.send(result);
        });
    }

    drop(result_tx);  // Close channel when all senders done
    return result_rx.collect();
}
```

### 9.3 Pipeline

```spl
fn pipeline(input: Receiver(T: Raw)): Receiver(T: Final) {
    let (stage1_tx, stage1_rx) = bounded(10);
    let (stage2_tx, stage2_rx) = bounded(10);

    spawn(|| {
        for raw in input {
            stage1_tx.send(parse(raw));
        }
    });

    spawn(|| {
        for parsed in stage1_rx {
            stage2_tx.send(transform(parsed));
        }
    });

    return stage2_rx;
}
```

### 9.4 Timeout Wrapper

```spl
fn with_timeout(duration: Duration, f: fn(): T): Result(T: T, E: TimeoutError) where T {
    let (tx, rx) = oneshot();

    spawn(|| {
        tx.send(f());
    });

    select {
        result = rx.recv() => {
            return Ok(result);
        },
        timeout(duration) => {
            return Err(TimeoutError);
        },
    }
}
```

---

## 10. Summary

| Feature | Module | Description |
|---------|--------|-------------|
| `spawn` | `std.task` | Create new task |
| `JoinHandle` | `std.task` | Handle to spawned task |
| `scope` | `std.task` | Scoped task spawning |
| `select` | `std.task` | Wait on multiple channels |
| `unbounded` | `std.channel` | Unbounded channel |
| `bounded` | `std.channel` | Bounded channel |
| `oneshot` | `std.channel` | Single-value channel |
| `broadcast` | `std.channel` | Multi-consumer channel |
| `Mutex` | `std.sync` | Mutual exclusion |
| `RwLock` | `std.sync` | Read-write lock |
| `Atomic*` | `std.sync.atomic` | Atomic operations |
| `Barrier` | `std.sync` | Thread barrier |
| `Condvar` | `std.sync` | Condition variable |

---

## References

- [ADR-013: Concurrency Model](../designs/013-async-await.md) - Design rationale
- [closures.md](closures.md) - Task closure capture semantics
- [memory-model.md](memory-model.md) - Ownership with concurrency
- [traits.md](traits.md) - Send, Sync, and other marker traits
- [type-system.md](type-system.md) - Type system and bounds
