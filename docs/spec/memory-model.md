# SPL Memory Model

This document defines the memory model for SPL (Simple Programming Language). SPL uses an ownership-based memory model inspired by Rust, with provisions for future extensions to support multiple memory management strategies.

## Overview

SPL's memory model provides:

- **Ownership**: Every value has a single owner
- **Move semantics**: Values are moved by default on assignment
- **Copy types**: Small, trivially-copyable types are copied implicitly
- **Second-class references**: References can only be function parameters, never stored or returned
- **No garbage collector** (v1): Memory is managed through ownership and scoping
- **Panic = unwind**: Panics unwind the stack, running destructors for cleanup. Aborts at FFI boundaries.

### Design Philosophy

SPL's memory model is designed to be:

1. **Safe by default**: Prevent use-after-free, double-free, and data races at compile time
2. **Simple**: Second-class references eliminate lifetime complexity
3. **Zero-cost abstractions**: No runtime overhead for ownership tracking
4. **Predictable**: Clear rules for when memory is allocated and freed

---

## 1. Ownership

### The Ownership Rules

Every value in SPL has exactly one **owner** at any point in time:

1. Each value has a variable that is its owner
2. There can only be one owner at a time
3. When the owner goes out of scope, the value is dropped

```spl
fn example() {
    let s = String.from("hello");  // s owns the String
    // s is valid here
}  // s goes out of scope, String is dropped
```

### Move Semantics

By default, assigning a value to another variable **moves** ownership:

```spl
let s1 = String.from("hello");
let s2 = s1;  // s1 is moved to s2
// s1 is no longer valid here
println(s2);  // OK
println(s1);  // ERROR: use of moved value
```

Move semantics apply to:
- Assignment (`let x = y`)
- Function arguments (`foo(x)`)
- Function returns (`return x`)
- Struct/tuple construction

```spl
fn take_ownership(s: String) {
    // s owns the String here
}  // s dropped here

fn example() {
    let s = String.from("hello");
    take_ownership(s);  // s moved into function
    // s is no longer valid
}
```

### Why Move by Default?

Move semantics provide several benefits:

1. **No implicit deep copies**: Expensive operations are explicit
2. **Clear ownership**: Always know who is responsible for cleanup
3. **Memory safety**: Prevents use-after-free without garbage collection
4. **Enables optimization**: Compiler can reuse memory

---

## 2. Copy Types

Some types are small and trivially copyable. These implement the `Copy` trait and are copied implicitly rather than moved.

### Built-in Copy Types

| Type | Copy? | Reason |
|------|-------|--------|
| `i8`, `i16`, `i32`, `i64`, `i128` | Yes | Small, stack-allocated |
| `u8`, `u16`, `u32`, `u64`, `u128` | Yes | Small, stack-allocated |
| `f32`, `f64` | Yes | Small, stack-allocated |
| `bool` | Yes | Single byte |
| `char` | Yes | Four bytes |
| `()` | Yes | Zero-size |
| `!` | Yes | Zero-size (never instantiated) |
| `&T` | Yes | Pointer-sized |
| `&mut T` | No | Exclusive access semantics |
| `[T; N]` | If `T: Copy` | Copies element-by-element |
| `(T, U, ...)` | If all elements `Copy` | Copies each element |
| `fn(...): T` | Yes | Function type |
| `decimal` | Yes | Fixed-size decimal (128-bit) |
| `bigint` | No | Heap-allocated, arbitrary precision |

### Copy Semantics

When a `Copy` type is assigned, the bits are copied and both values remain valid:

```spl
let x: i32 = 42;
let y = x;  // x is copied, not moved
println(x);  // OK: x is still valid
println(y);  // OK: y has a copy
```

### User-Defined Copy Types

Structs can opt into `Copy` semantics if all their fields are `Copy`:

```spl
#[derive(Copy)]
struct Point(
    x: f64,
    y: f64,
)

let p1 = Point(x: 1.0, y: 2.0);
let p2 = p1;  // Copied, not moved
// Both p1 and p2 are valid
```

**Restrictions on Copy:**
- All fields must be `Copy`
- Cannot implement `Drop` (custom destructor)
- Must be a "plain old data" type

### Clone: Explicit Copying

Types that are expensive to copy can implement `Clone` for explicit copying:

```spl
let s1 = String.from("hello");
let s2 = s1.clone();  // Explicit deep copy
// Both s1 and s2 are valid
```

`Clone` is always explicit - the programmer must call `.clone()`.

---

## 3. Second-Class References

SPL uses **second-class references**: references can only be function parameters, never stored in structs or returned from functions. This dramatically simplifies the borrow checker and eliminates the need for lifetime annotations.

### The Second-Class Rule

```spl
// OK: Reference as function parameter
fn process(data: &str) {
    println(data);
}

// NOT ALLOWED: Cannot return a reference
// fn get_ref(s: &str): &str { return s; }

// NOT ALLOWED: Cannot store a reference in a struct
// struct Parser { input: &str }
```

### Why Second-Class References?

1. **No lifetime annotations**: Lifetimes are always local to a function
2. **Simpler borrow checker**: No inference across function boundaries
3. **Easier to learn**: References are just a calling convention
4. **Interior iteration**: Use generators/coroutines instead of iterators holding references

### Reference Types

| Reference | Alias | Mutate |
|-----------|-------|--------|
| `&T` | Many allowed | No |
| `&mut T` | Exclusive | Yes |

### Creating References

```spl
let x = 42;
let r: &i32 = &x;       // Immutable borrow

let mut y = 100;
let mr: &mut i32 = &mut y;  // Mutable borrow
*mr = 200;              // Modify through reference
```

### The Borrowing Rules

At any given time, you can have **either**:
- Any number of immutable references (`&T`), OR
- Exactly one mutable reference (`&mut T`)

But **not both**.

```spl
let mut x = 42;

// OK: Multiple immutable references
let r1 = &x;
let r2 = &x;
println(r1, r2);

// OK: Single mutable reference
let mr = &mut x;
*mr = 100;

// ERROR: Cannot have both
let r = &x;
let mr = &mut x;  // ERROR: cannot borrow as mutable
println(r);       // while immutable borrow is active
```

### Reference Validity

References must not outlive their referent:

```spl
fn dangling(): &i32 {
    let x = 42;
    &x  // ERROR: x does not live long enough
}  // x dropped here, reference would be invalid
```

### Reborrowing

A mutable reference can be temporarily reborrowed:

```spl
fn take_ref(r: &i32) { }

let mut x = 42;
let mr = &mut x;
take_ref(mr);   // Implicit reborrow as &i32
*mr = 100;      // mr still valid after reborrow ends
```

---

## 4. Scopes and Lifetimes

### Lexical Scopes

Values are dropped when their owning variable goes out of scope:

```spl
{
    let s = String.from("hello");
    // s is valid
}  // s dropped here

// s is not valid here
```

### Non-Lexical Lifetimes (NLL)

SPL uses non-lexical lifetimes: a borrow ends when it's last used, not at the end of the scope.

```spl
let mut x = 42;
let r = &x;
println(r);  // Last use of r
// r's borrow ends here (NLL)

let mr = &mut x;  // OK: no conflict with r
*mr = 100;
```

### No Lifetime Annotations

Because references are second-class (parameter-only), SPL does not need lifetime annotations. The borrow checker only needs to verify that:

1. References don't escape the function (can't return or store them)
2. Borrowing rules are followed within the function body

```spl
// This is the only valid pattern for references:
fn process(data: &[i32]) {
    // Use data within function body
    for item in data {
        println(item);
    }
}
// Reference 'data' is gone after function returns
```

---

## 5. Drop and Destructors

### The Drop Trait

Types can implement `Drop` to run custom cleanup code:

```spl
struct FileHandle(
    fd: i32,
)

impl Drop for FileHandle {
    fn drop(&mut self) {
        // Close the file descriptor
        close(self.fd);
    }
}

fn example() {
    let f = FileHandle(fd: open("file.txt"));
    // Use f...
}  // f.drop() called automatically
```

### Drop Order

Values are dropped in reverse order of declaration:

```spl
{
    let a = A.new();  // Created first
    let b = B.new();  // Created second
}  // b dropped first, then a
```

Struct fields are dropped in declaration order.

### Early Drop

Use `drop(value)` to drop a value before end of scope:

```spl
let lock = mutex.lock();
// Critical section
drop(lock);  // Release lock early
// More code without lock held
```

---

## 6. Closures and Capture

Closures are anonymous functions that can capture variables from their enclosing scope. SPL's closure design leverages second-class references to eliminate lifetime complexity.

For complete closure specification, see [closures.md](closures.md).

### Summary

SPL distinguishes between **escaping** and **non-escaping** closures:

| Context | Non-Copy Capture Behavior |
|---------|---------------------------|
| Non-escaping (e.g., `map`, `filter`) | Borrow by default |
| Escaping (stored, returned, spawned) | Move by default |

The `~` sigil clones a capture at closure creation time:

```spl
let data = Arc.new(vec![1, 2, 3]);

// Escaping: data moved (default)
let f = || process(data);
// data no longer valid

// Escaping: data cloned
let f = |~data| process(data);
// data still valid
```

### Closure Traits

Closures implement traits based on how they use captures:

| Trait | Requirement |
|-------|-------------|
| `Fn` | Only reads captures |
| `FnMut` | May mutate captures |
| `FnOnce` | May consume captures |

**Hierarchy:** `Fn` ⊂ `FnMut` ⊂ `FnOnce`

The `fn(Args): Return` type represents any callable with matching signature, including closures with captures.

---

## 7. Memory Layout

### Stack vs Heap

| Allocation | Used For | Characteristics |
|------------|----------|-----------------|
| Stack | Local variables, fixed-size types | Fast, automatic, LIFO |
| Heap | Dynamic data, growable collections | Flexible, manual (via ownership) |

### Type Layout

Primitives have fixed, known sizes:

```
i8, u8, bool: 1 byte
i16, u16: 2 bytes
i32, u32, f32, char: 4 bytes
i64, u64, f64: 8 bytes
i128, u128: 16 bytes
```

References are pointer-sized (8 bytes on 64-bit).

Structs are laid out with padding for alignment:

```spl
struct Example(
    a: u8,   // 1 byte
    // 3 bytes padding
    b: u32,  // 4 bytes
    c: u8,   // 1 byte
    // 7 bytes padding
    d: u64,  // 8 bytes
)  // Total: 24 bytes
```

### Unsized Types

Some types don't have a known size at compile time:

| Type | Description | Used As |
|------|-------------|---------|
| `str` | String slice | `&str` (fat pointer) |
| `[T]` | Slice | `&[T]` (fat pointer) |

Fat pointers contain (pointer, length):

```
&str: 16 bytes (8 byte ptr + 8 byte len)
&[T]: 16 bytes (8 byte ptr + 8 byte len)
```

---

## 8. Interior Mutability (Future)

Some patterns require mutation through shared references. SPL will provide controlled escape hatches:

### Cell Types (Planned)

```spl
// Single-threaded interior mutability
Cell(T: T)      // For Copy types, get/set
RefCell(T: T)   // Runtime borrow checking

// Thread-safe interior mutability
Mutex(T: T)     // Mutual exclusion
RwLock(T: T)    // Reader-writer lock
Atomic*         // Lock-free atomics
```

### Unsafe Blocks (Planned)

For low-level control, `unsafe` blocks bypass some compiler checks:

```spl
unsafe {
    // Raw pointer operations
    // Calling unsafe functions
    // Accessing mutable statics
}
```

---

## 9. Future Extensions

SPL's memory model is designed to support multiple memory management strategies in future versions.

### 8.1 Interior Iteration with Generators

SPL will use generators/coroutines for iteration, avoiding the need for iterator objects that hold references:

```spl
// Generator-based iteration (future syntax)
gen fn iterate(vec: &Vec(T: T)): T where T {
    for i in 0..vec.len() {
        yield vec[i];
    }
}

// Usage
for item in iterate(&my_vec) {
    process(item);
}

// Or with method syntax
my_vec.each(|item| {
    process(item);
});
```

### 8.2 Region-Based Memory

Inspired by Cyclone and Vale, regions provide arena-style allocation:

```spl
// Hypothetical syntax
region r {
    let x = r.alloc(Point(x: 1.0, y: 2.0));
    let y = r.alloc(Point(x: 3.0, y: 4.0));
    // All allocations freed when region ends
}
```

Benefits:
- Bulk deallocation (faster than individual frees)
- No fragmentation within region
- Simplified lifetime reasoning

### 8.3 Allocator-Aware Types (Zig-style)

Types that accept custom allocators:

```spl
// Hypothetical syntax
fn process(allocator: Allocator) {
    let list = ArrayList(i32).init(allocator);
    defer list.deinit();

    list.push(1);
    list.push(2);
}

// Stack allocator for temporary work
let buffer: [u8; 1024] = undefined;
let stack_alloc = FixedBufferAllocator.init(&buffer);
process(stack_alloc);

// Or use the heap
process(heap_allocator);
```

### 8.4 Optional Garbage Collection (D-style)

Opt-in GC for specific types or regions:

```spl
// Hypothetical syntax
@gc
struct Node(
    value: i32,
    children: Vec(T: Node),  // Cycles OK with GC
)

// Or region-based GC
gc_region {
    // Allocations here are garbage collected
    let graph = build_cyclic_graph();
}
```

### 8.5 Generational References (Vale-style)

References with generation counts for safe manual memory:

```spl
// Hypothetical syntax
let x = gen_alloc(Point(x: 1.0, y: 2.0));
let r: GenRef(T: Point) = &x;

gen_free(x);  // Explicitly free

// r.get() returns None (generation mismatch)
// instead of use-after-free
```

### 8.6 Linear Types

Types that must be used exactly once:

```spl
// Hypothetical syntax
linear struct FileHandle(
    fd: i32,
)

fn example() {
    let f = open("file.txt");
    // ERROR if f is not used (dropped or passed somewhere)
}
```

### 8.7 Strategy Selection

Future SPL may allow choosing memory strategy per-module or per-type:

```spl
// Module-level default
#![memory(gc)]  // This module uses GC

// Type-level override
#[memory(owned)]  // This type uses ownership
struct Performance(
    data: Vec(T: f64),
)

// Function-level arena
#[arena]
fn batch_process(items: &[Item]) {
    // All allocations use function-scoped arena
}
```

---

## 10. Comparison with Other Languages

| Feature | SPL v1 | Rust | Go | Zig | D |
|---------|--------|------|----|----|---|
| Ownership | Yes | Yes | No | Optional | No |
| Move semantics | Default | Default | No | No | No |
| Borrow checker | Yes (simple) | Yes (full) | No | No | No |
| Lifetime annotations | Inferred | Required | N/A | N/A | N/A |
| GC | No | No | Yes | No | Optional |
| Manual memory | Via unsafe | Via unsafe | No | Yes | Yes |
| Custom allocators | Planned | Yes | No | Yes | Yes |

---

## 11. Summary

### V1 Memory Model

| Aspect | Behavior |
|--------|----------|
| Default semantics | Move |
| Copy types | Primitives, opt-in for structs |
| References | Second-class (parameters only) |
| Borrowing | `&T` (shared) and `&mut T` (exclusive) |
| Lifetimes | No annotations needed |
| Drop | Automatic at scope end |
| Panic | Unwind (abort at FFI boundary) |
| Overflow | Always trap |
| Unsafe | Planned (not in v1) |

### Key Guarantees

1. **No use-after-free**: Ownership prevents accessing freed memory
2. **No double-free**: Single owner means single drop
3. **No data races**: Borrowing rules prevent concurrent mutation
4. **No null pointers**: References are always valid (Option for nullable)
5. **No uninitialized memory**: All values must be initialized
6. **No reference escapes**: References cannot outlive their scope
7. **No silent overflow**: Integer operations trap on overflow

### Extension Path

SPL v1 establishes a foundation that can be extended with:
- Multiple memory strategies (GC, arenas, manual)
- Allocator-aware types
- Unsafe escape hatches for systems programming
- First-class references (if needed for specific use cases)

The core ownership and borrowing model remains stable while additional features layer on top.

---

## Examples

### Basic Ownership

```spl
fn ownership_example() {
    // Stack allocation with move
    let s1 = String.from("hello");
    let s2 = s1;  // s1 moved to s2
    // println(s1);  // ERROR: s1 was moved
    println(s2);     // OK

    // Copy types
    let x = 42;
    let y = x;   // x copied
    println(x);  // OK: x still valid
    println(y);  // OK: y has copy
}
```

### Borrowing Patterns

```spl
fn borrowing_example() {
    let mut data: Vec(T: i32) = [1, 2, 3];  // Array coerces to Vec

    // Immutable borrows for reading
    let sum: i32 = data.iter().sum();
    let len = data.len();

    // Mutable borrow for modification
    data.push(4);

    // Reborrowing
    let slice: &[i32] = &data[1..3];
    println(slice);  // [2, 3]
}
```

### Functions and Ownership

```spl
// Takes ownership
fn consume(s: String) {
    println(s);
}  // s dropped

// Borrows immutably
fn inspect(s: &String) {
    println(s);
}  // borrow ends, s not dropped

// Borrows mutably
fn modify(s: &mut String) {
    s.push_str("!");
}

fn example() {
    let mut s = String.from("hello");

    inspect(&s);      // Borrow
    modify(&mut s);   // Mutable borrow
    consume(s);       // Move
    // s is no longer valid
}
```

### Working Without Returned References

Since references cannot be returned, use these patterns instead:

```spl
// Instead of returning a reference, return an owned value
fn first_word(s: &str): String {
    let bytes = s.as_bytes();
    for (i, byte) in bytes.enumerate() {
        if byte == b' ' {
            return s[0..i].to_string();
        }
    }
    return s.to_string();
}

// Use interior iteration with callbacks
struct Container(
    data: Vec(T: i32),
)

impl Container {
    // Instead of returning &i32, use a callback
    fn with_first(&self, f: fn(&i32)) {
        if self.data.len() > 0 {
            f(&self.data[0]);
        }
    }

    // Or return an owned copy
    fn first(&self): i32? {
        return self.data.first().copied();
    }
}
```
