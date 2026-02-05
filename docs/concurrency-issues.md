# Concurrency Spec Soundness Review

## Summary

The concurrency spec (`docs/spec/concurrency.md`) has **several critical soundness issues** that must be addressed before implementation. This review identifies issues and proposes fixes.

---

## Critical Issues

### 1. Missing Send/Sync Enforcement (CRITICAL)

**Problem:** `spawn()` and channel types have no thread-safety constraints.

**Current spec (line 21):**

```spl
let handle = spawn(|| { ... });  // No Send bound mentioned
```

**What's missing:**

- `JoinHandle(T)` should require `T: Send` (result crosses threads)
- `Sender(T)/Receiver(T)` should require `T: Send` (values cross threads)
- Task closures must capture only `Send` values

**Evidence from traits.md (lines 347-356):**

```spl
trait Send { }  // safe to send between threads
trait Sync { }  // safe to share references between threads
```

These exist but are never referenced in the concurrency spec.

**Risk:** Without enforcement, non-thread-safe types (like `Rc`, `RefCell`) can be sent across threads, causing data races.

**Fix:** Add explicit bounds:

```spl
fn spawn(f: F): JoinHandle(T) where F: FnOnce() -> T + Send, T: Send

struct Sender(T) where T: Send { ... }
struct Receiver(T) where T: Send { ... }
```

---

### 2. Scoped Task Borrowing Violates Ownership (CRITICAL)

**Problem:** Section 6.2 shows two closures both capturing `results` mutably.

**Current spec (lines 528-540):**

```spl
let mut results = Vec.new();

scope(|s| {
    s.spawn(|| {
        results.push(compute_a());  // &mut results
    });
    s.spawn(|| {
        results.push(compute_b());  // &mut results
    });
});
```

**Why this is unsound:**

- Both closures hold `&mut results`
- Both tasks may run concurrently
- This violates "exactly one mutable reference" rule from memory-model.md

**The spec's claim (line 542):** "The borrow checker ensures safety - multiple mutable borrows would be rejected."

This is self-contradictory - the example demonstrates exactly what the comment claims is rejected.

**Fix options:**

1. **Remove the example** - it's not achievable with standard ownership
2. **Use synchronization** - require `Mutex` for shared mutable state:
   ```spl
   let results = Mutex.new(Vec.new());
   scope(|s| {
       s.spawn(|~results| results.lock().push(compute_a()));
       s.spawn(|~results| results.lock().push(compute_b()));
   });
   ```
3. **Add scoped closure documentation** - if scoped closures have special rules (sequential access or interior mutability), document them explicitly

---

### 3. Generic Type Syntax Inconsistency (HIGH)

**Problem:** Concurrency spec uses `T: T` notation inconsistent with type-system.md.

**Current:**

```spl
struct JoinHandle(T: T) where T { ... }      // line 35
fn await_timeout(self, ...): Option(T: T);   // line 40
enum SendError(T) where T { ... }            // line 153
```

**Per type-system.md (section 6):**

```spl
struct Point(x: T, y: T) where T  // fields have names
enum Option(T) where T            // generic parameter
```

**Fix:** Use consistent syntax:

```spl
struct JoinHandle(T) where T: Send { ... }
fn await_timeout(self, duration: Duration): Option(T);
```

---

### 4. Scoped vs Escaping Closure Distinction Missing (HIGH)

**Problem:** No documentation distinguishing capture rules for:

- `spawn(closure)` - escaping, must own captures
- `s.spawn(closure)` within `scope()` - can borrow from enclosing scope

**From closures.md section 5:** "Closures cannot store references."

But scoped tasks in section 6.2 explicitly borrow. The mechanism enabling this is not documented.

**Fix:** Add a section explaining scoped closure semantics, referencing:

- memory-model.md section 8 on scoped types
- How `Scope` API prevents closure escape
- Lifetime bounds that enable borrowing

---

## Medium Priority Issues

### 5. Atomic Memory Ordering Unspecified

**Problem:** Section 4.3 shows `Ordering.SeqCst` without defining the `Ordering` enum or memory model semantics.

**Fix:** Add `Ordering` enum definition and brief explanation:

```spl
enum Ordering {
    Relaxed,    // No ordering guarantees
    Acquire,    // Acquire semantics (read-side)
    Release,    // Release semantics (write-side)
    AcqRel,     // Acquire + Release
    SeqCst,     // Sequentially consistent (strongest)
}
```

### 6. Condition Variable Semantics Incomplete

**Problem:** Section 4.6 doesn't specify:

- Can `wait()` wake spuriously?
- Is the lock released during wait?
- What happens if mutex is dropped while waiting?

**Fix:** Add note: "Like POSIX condition variables, `wait()` may wake spuriously. Always use in a loop checking the condition."

### 7. Panic vs Cancellation Ordering

**Problem:** Unclear what happens when a task panics while being cancelled, or vice versa.

**Fix:** Document priority: cancellation flag is checked at yield points; if task panics before reaching yield point, panic takes precedence.

---

## Low Priority Issues

### 8. Select Syntax Ambiguity

The `select` block syntax (lines 240-292) uses custom syntax not defined in any grammar spec. Should be documented or referenced.

### 9. ThreadLocal Syntax

Section 7.4 uses `thread_local! { ... }` macro syntax that's not documented elsewhere.

---

## Proposed Changes

| File           | Section  | Change                                 |
| -------------- | -------- | -------------------------------------- |
| concurrency.md | §1       | Add Send/Sync requirements to overview |
| concurrency.md | §1.2     | Add `where T: Send` to JoinHandle      |
| concurrency.md | §2.3     | Add `where T: Send` to Sender/Receiver |
| concurrency.md | §4.3     | Define Ordering enum                   |
| concurrency.md | §4.6     | Document spurious wakeup               |
| concurrency.md | §6.2     | Fix borrowing example or add Mutex     |
| concurrency.md | New §6.3 | Add "Scoped Closure Semantics" section |
| closures.md    | §7 (new) | Add "Scoped Closures" section          |

---

## Verification

After fixes:

1. All generic types use consistent syntax
2. All types crossing thread boundaries have `Send` bounds
3. Scoped task examples don't violate ownership rules
4. Atomics section defines memory ordering
5. Cross-references to closures.md and memory-model.md are accurate

---

## References

- ADR-013: Concurrency Model (source design document)
- docs/spec/closures.md (capture semantics)
- docs/spec/memory-model.md (ownership rules)
- docs/spec/traits.md (Send/Sync definitions)
- docs/spec/type-system.md (generic syntax)
