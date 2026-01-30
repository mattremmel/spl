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

### 1.2 JoinHandle

`spawn` returns a `JoinHandle(T: T)` for the task's result:

```spl
struct JoinHandle(T) where T {
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

// data is moved, config is cloned
let handle = spawn(|data, ~config| {
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

spawn(|tx| {
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

spawn(|tx| {
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
struct Sender(T) where T {
    fn send(&self, value: T): ();
    fn try_send(&self, value: T): Result(T: (), E: SendError(T: T));
    fn is_closed(&self): bool;
}

// Receiver side
struct Receiver(T) where T {
    fn recv(&self): Option(T: T);
    fn try_recv(&self): Result(T: T, E: TryRecvError);
    fn is_empty(&self): bool;
}

enum SendError(T) where T {
    Disconnected(value: T),  // Channel closed, returns unsent value
    Full(value: T),          // Bounded channel full (try_send only)
}

enum TryRecvError {
    Empty,        // No message available
    Disconnected, // Channel closed
}
```

### 2.4 Multiple Producers

Senders can be cloned for multiple producers:

```spl
let (tx, rx) = unbounded();

for i in 0..4 {
    let tx = tx.clone();
    spawn(|tx, i| {
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

spawn(|rx1| {
    for msg in rx1 {
        handle_a(msg);
    }
});

spawn(|rx2| {
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

spawn(|tx| {
    let result = expensive_computation();
    tx.send(result);  // Can only be called once
});

let value = rx.await();  // Wait for the single value
```

---

## 3. Select

`select` waits on multiple channel operations:

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
    spawn(|barrier, i| {
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

// Waiting thread
spawn(|lock, cvar| {
    let mut guard = lock.lock();
    while !*guard {
        guard = cvar.wait(guard);
    }
    println("condition met!");
});

// Notifying thread
{
    let mut guard = lock.lock();
    *guard = true;
    cvar.notify_one();
}
```

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

let handle = spawn(|token_clone| {
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

---

## 6. Scoped Tasks

### 6.1 Task Scope

Scoped tasks ensure all spawned tasks complete before the scope exits:

```spl
use std.task.scope;

let data = vec![1, 2, 3, 4];

scope(|s| {
    for chunk in data.chunks(2) {
        s.spawn(|chunk| {
            process_chunk(chunk);
        });
    }
});  // All tasks complete before this line

// Safe to use data here
```

### 6.2 Borrowing in Scoped Tasks

Scoped tasks can borrow from the enclosing scope:

```spl
let mut results = Vec.new();

scope(|s| {
    s.spawn(|| {
        results.push(compute_a());  // Borrows results
    });
    s.spawn(|| {
        results.push(compute_b());  // Borrows results
    });
});

// results contains both values
```

**Note:** The borrow checker ensures safety - multiple mutable borrows would be rejected.

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

### 7.4 Thread-Local Storage

```spl
use std.task.thread_local;

thread_local! {
    static CACHE: RefCell(HashMap(K: String, V: i32)) = RefCell.new(HashMap.new());
}

fn cached_lookup(key: &str): i32 {
    CACHE.with(|cache| {
        cache.borrow_mut()
            .entry(key.to_string())
            .or_insert_with(|| expensive_compute(key))
            .clone()
    })
}
```

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

fn worker_pool(jobs: Receiver(Job), results: Sender(Result), n: usize): () {
    for _ in 0..n {
        let jobs = jobs.clone();
        let results = results.clone();
        spawn(|jobs, results| {
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
fn fan_out_fan_in(input: Vec(T: Item)): Vec(T: Result) {
    let (result_tx, result_rx) = unbounded();

    for item in input {
        let result_tx = result_tx.clone();
        spawn(|item, result_tx| {
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
fn pipeline(input: Receiver(Raw)): Receiver(Final) {
    let (stage1_tx, stage1_rx) = bounded(10);
    let (stage2_tx, stage2_rx) = bounded(10);

    spawn(|input, stage1_tx| {
        for raw in input {
            stage1_tx.send(parse(raw));
        }
    });

    spawn(|stage1_rx, stage2_tx| {
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

    spawn(|tx, f| {
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
