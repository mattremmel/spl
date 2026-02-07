# SPL Memory Model

This document defines the memory model for SPL (Simple Programming Language). SPL uses an ownership-based memory model inspired by Rust, with provisions for future extensions to support multiple memory management strategies.

## Overview

SPL's memory model provides:

- **Ownership**: Every value has a single owner
- **Move semantics**: Values are moved by default on assignment
- **Copy types**: Small, trivially-copyable types are copied implicitly
- **Second-class references with intersection semantics**: References can be parameters or returned, but never stored in structs. Returned references are assumed to borrow from all input references
- **Place expressions**: Compile-time representation of memory locations
- **No garbage collector** (v1): Memory is managed through ownership and scoping
- **Panic = unwind**: Panics unwind the stack, running destructors for cleanup. Aborts at FFI boundaries.

### Design Philosophy

SPL's memory model is designed to be:

1. **Safe by default**: Prevent use-after-free, double-free, and data races at compile time
2. **Simple**: Intersection semantics eliminate lifetime annotation complexity
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
| `Never` | Yes | Zero-size (never instantiated) |
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

## 3. References and Intersection Semantics

SPL uses **second-class references** with **intersection semantics**: references can be function parameters or returned from functions, but cannot be stored in struct fields (see below for references as generic type parameters, and section 8 for `#[scoped]` types that can hold references). When a function returns a reference, it is assumed to borrow from **all** input references. The returned reference is valid only while all inputs remain valid.

### The Intersection Rule

A function may return `&T` or `&mut T` if it has at least one reference parameter (including `&self` or `&mut self`). The returned reference is conservatively assumed to borrow from all input references.

```spl
// OK: single input ref (&self)
fn first(&self): &T {
    return &self.data[0];
}

// OK: single input ref (s: &str)
fn trim(s: &str): &str {
    // return borrows from s
}

// OK: multiple input refs - output borrows from BOTH
fn longer(a: &str, b: &str): &str {
    if a.len() > b.len() { return a; }
    return b;
}

// ERROR: no input refs - cannot return ref to local
fn make_ref(): &i32 {
    let x = 42;
    return &x;  // compile error: no input ref to borrow from
}

// OK: Reference as function parameter (no return)
fn process(data: &str) {
    println(data);
}

// NOT ALLOWED: Cannot store a reference in a struct
// struct Parser(input: &str)
```

### Why Intersection Semantics Work

Without references in structs, the conservative assumption (output borrows from all inputs) is sound and practical:

1. **Sound**: The output cannot outlive any input it might borrow from
2. **No annotations needed**: The compiler assumes the most restrictive case
3. **Practical**: Most use cases work naturally; the restriction only affects edge cases

```spl
// The output is valid as long as BOTH inputs are valid
let s1 = "hello";
let result: &str;
{
    let s2 = "world!";
    result = longer(s1, s2);  // Borrows from both s1 AND s2
    println(result);          // OK: both s1 and s2 still valid
}
// result is now invalid (s2 dropped), even if it actually pointed to s1
```

| Property | How It's Satisfied |
|----------|-------------------|
| "Can't escape" | Output ref is tied to all input refs—it cannot outlive any of them |
| "Known lifetimes" | Intersection of all input lifetimes (most restrictive) |
| "No annotations" | The compiler infers the relationship automatically |

### Optional Lifetime Markers

When intersection semantics are too conservative, you can use **lifetime markers** to specify exactly which inputs the output borrows from. This uses Rust-style `'name` syntax:

```spl
// Default (no markers): output borrows from BOTH a and b
fn longer(a: &str, b: &str): &str {
    if a.len() > b.len() { return a; }
    return b;
}

// With markers: output borrows ONLY from 'a
fn first_if_longer(a: &'a str, b: &str): &'a str {
    if a.len() > b.len() { return a; }
    return "";  // Return static string, not b
}
```

**Lifetime markers are optional precision, not required annotation.** Most code uses the default intersection behavior.

### Lifetime Marker Rules

1. **Markers are function-local**: They only exist at function signatures, not in types
2. **Same marker = same lifetime group**: Multiple parameters can share a marker
3. **Unmarked inputs use intersection**: If no markers, all inputs are grouped together
4. **Output must specify its source**: A marked return type must use a declared marker

```spl
// Multiple inputs in same lifetime group
fn either(a: &'x str, b: &'x str, default: &str): &'x str {
    // Can return a OR b (both are 'x)
    // Cannot return default (not in 'x group)
    if a.is_empty() { return b; }
    return a;
}

// Mixed: some marked, some not
fn get_or_default(primary: &'a str, fallback: &str): &'a str {
    // Can only return primary or static strings
    // Cannot return fallback
    if primary.is_empty() {
        return "default";  // OK: static string outlives everything
    }
    return primary;
}
```

### Provenance Tracking

The compiler tracks the **provenance** (origin) of each reference through the function body:

```spl
fn example(a: &'x str, b: &str): &'x str {
    // Direct returns
    return a;              // OK: a is 'x
    return b;              // ERROR: b is not 'x

    // Derived references
    let slice = &a[0..5];  // slice inherits 'x from a
    return slice;          // OK

    // Conditionals
    let x = if cond { a } else { b };
    return x;              // ERROR: x might be from b

    // Function calls
    let r = some_fn(a, b); // r has provenance of both (intersection)
    return r;              // ERROR: r might be from b
}
```

**Provenance propagates through calls:**

```spl
// Inner function with markers
fn inner(x: &'a str, y: &str): &'a str { ... }

fn outer(a: &'x str, b: &str): &'x str {
    let r = inner(a, b);   // r has 'x provenance (from a via 'a)
    return r;              // OK: r is 'x
}

// Inner function without markers (intersection)
fn inner_default(x: &str, y: &str): &str { ... }

fn outer2(a: &'x str, b: &str): &'x str {
    let r = inner_default(a, b);  // r has provenance of both
    return r;                      // ERROR: r might be from b
}
```

### What Lifetime Markers Are NOT

SPL's lifetime markers are simpler than Rust's full lifetime system:

| Feature | Rust | SPL |
|---------|------|-----|
| Lifetime parameters on functions | Yes | Yes (optional) |
| Default behavior | Must annotate | Intersection |
| Lifetime relationships (`'a: 'b`) | Yes | **No** |
| Lifetimes in struct types | Yes | **No** |
| Lifetimes in type aliases | Yes | **No** |
| Higher-rank bounds (`for<'a>`) | Yes | **No** |

```spl
// NOT ALLOWED in SPL:

// No lifetime bounds/relationships
// fn foo(a: &'a str, b: &'b str): &'a str where 'b: 'a

// No lifetimes in structs
// struct Parser(input: &'a str)

// No lifetimes in type aliases
// type StrRef('a) = &'a str
```

The markers exist purely to **narrow the intersection default** at function boundaries. They don't create a lifetime polymorphism system.

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

References must not outlive their referent. The intersection rule ensures this at compile time:

```spl
// ERROR: no input reference, so cannot return a reference
fn dangling(): &i32 {
    let x = 42;
    return &x;  // compile error: no input ref to borrow from
}
```

### References as Generic Type Parameters

References cannot be stored in struct fields, but they **can** be used as generic type parameters. This distinction is important:

```spl
// NOT ALLOWED: reference as struct field
// struct Parser(input: &str)  // compile error

// ALLOWED: reference as generic type parameter
fn get_opt(&self, i: usize): Option(&T) {
    if i < self.len() {
        return Some(&self.data[i]);
    }
    return None;
}
```

When `T = &SomeType`, the `Option` variant holds a reference. This is permitted because:
1. The `Option(&T)` is a return value, not stored in a struct field
2. The reference inside still follows the intersection rule (borrows from all input refs)
3. The `Option` wrapper doesn't extend the reference's lifetime

### Tuple Returns with References

Multiple references can be returned in a tuple. They are all assumed to borrow from all input references:

```spl
// OK: both references borrow from &self
fn first_and_last(&self): (&T, &T) {
    return (&self.data[0], &self.data[self.data.len() - 1]);
}

// OK: both &str borrow from input s
fn split_at(s: &str, mid: usize): (&str, &str) {
    return (&s[..mid], &s[mid..]);
}

// OK: multiple inputs - all outputs borrow from all inputs
fn get_both(a: &Container, b: &Container, idx: usize): (&T, &T) {
    return (a.get(idx), b.get(idx));  // Both refs valid while both a AND b valid
}
```

### Nested References (`&&T`)

Nested references (references to references) are **not permitted** in SPL. This keeps the borrowing model simple and avoids complex lifetime interactions:

```spl
// NOT ALLOWED
// fn get_ref_to_ref(&self): &&T { ... }  // compile error

// Instead, return the inner reference directly
fn get(&self): &T { ... }
```

### Element Aliasing

The standard borrowing rules apply to collection elements. You cannot have multiple mutable references to the same collection, even if they target different indices:

```spl
let mut vec = [1, 2, 3];

// ERROR: cannot have two mutable borrows of vec
let x: &mut i32 = &mut vec[0];
let y: &mut i32 = &mut vec[1];  // compile error: vec already mutably borrowed

// OK: use separate scopes
{
    let x: &mut i32 = &mut vec[0];
    *x = 10;
}
{
    let y: &mut i32 = &mut vec[1];
    *y = 20;
}
```

For simultaneous access to multiple elements, use methods like `split_at_mut` that return disjoint slices, or iterator-based approaches.

### Reborrowing

A mutable reference can be temporarily reborrowed:

```spl
fn take_ref(r: &i32) { }

let mut x = 42;
let mr = &mut x;
take_ref(mr);   // Implicit reborrow as &i32
*mr = 100;      // mr still valid after reborrow ends
```

### Dereferencing References

The `*` (dereference) operator accesses the value pointed to by a reference:

```spl
let x = 42;
let r: &i32 = &x;
let value = *r;        // Read through reference: value = 42

let mut y = 100;
let mr: &mut i32 = &mut y;
*mr = 200;             // Write through mutable reference
println(y);            // 200
```

**Dereference rules:**
- `*r` where `r: &T` produces a value of type `T` (read)
- `*mr = value` where `mr: &mut T` assigns to the referent (write)
- The compiler auto-dereferences in many contexts (method calls, field access)

**Note:** Raw pointers (`Ptr(T: T)` and `MutPtr(T: T)`) do NOT use `*` for dereferencing. Instead, they use explicit `.read()` and `.write()` methods within `unsafe` blocks. See [unsafe.md](unsafe.md) for details.

---

## 4. Place Expressions

A **place expression** represents a location in memory (similar to Rust's place expressions / C++'s lvalues). Unlike value expressions which produce values, place expressions identify where values are stored.

### Syntax

```ebnf
PlaceExpr = IDENTIFIER
          | PlaceExpr "." IDENTIFIER
          | PlaceExpr "[" Expression "]"
          | "*" Expression
          ;
```

### Place Expression Forms

| Form | Description |
|------|-------------|
| `x` | Variable |
| `obj.field` | Field access |
| `collection[i]` | Index operation |
| `*ptr` | Dereference |
| `matrix[i][j].field` | Compound place |

### Evaluation

Place expressions are not evaluated to values. Instead, they are used contextually:

| Context | Behavior |
|---------|----------|
| `&place` | Creates immutable reference to location |
| `&mut place` | Creates mutable reference to location |
| `place = expr` | Stores value at location |
| Value context | Reads from location (copy or move) |

### Examples

```spl
let mut vec: Vec(T: i32) = [1, 2, 3];

// Places in different contexts
let r: &i32 = &vec[0];      // &place -> creates reference
vec[1] = 42;                 // place = expr -> assigns to location
let x: i32 = vec[2];         // value context -> reads (copy)

// Compound places
let mut matrix: [[i32; 3]; 3] = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
matrix[1][2] = 100;          // nested index
let cell: &i32 = &matrix[0][1];

struct Point(x: i32, y: i32)
let mut p = Point(x: 1, y: 2);
p.x = 10;                    // field place
let px: &i32 = &p.x;
```

### Places and the Index Trait

The `collection[i]` syntax is desugared using the `Index` and `IndexMut` traits (see [traits.md](traits.md)):

| Syntax | Desugaring |
|--------|------------|
| `collection[i]` (value context) | `*collection.index(i)` |
| `&collection[i]` | `collection.index(i)` |
| `&mut collection[i]` | `collection.index_mut(i)` |
| `collection[i] = v` | `*collection.index_mut(i) = v` |

The `Index` trait's `index(&self): &T` method is legal because there is an input reference (`&self`) for the output to borrow from.

---

## 5. Scopes and Lifetimes

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

### Lifetime Annotations Are Optional

Unlike Rust, SPL does not require lifetime annotations. The borrow checker uses intersection semantics by default, with optional markers for precision:

1. References cannot be stored in structs
2. Returned references must have at least one input reference to borrow from
3. **Default**: Returned references borrow from all input references (intersection)
4. **Optional**: Use `'name` markers to specify exact provenance
5. Borrowing rules are followed within function bodies

```spl
// No annotations needed - intersection semantics
fn process(data: &[i32]) {
    for item in data {
        println(item);
    }
}

// Single input - no ambiguity
fn first(data: &[i32]): &i32 {
    return &data[0];
}

// Multiple inputs - default intersection (output borrows from both)
fn longer(a: &str, b: &str): &str {
    if a.len() > b.len() { return a; }
    return b;
}

// Optional: explicit markers when you need precision
fn first_only(a: &'x str, b: &str): &'x str {
    return a;  // Compiler enforces: cannot return b
}
```

---

## 6. Drop and Destructors

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

## 7. Closures and Capture

Closures are anonymous functions that can capture variables from their enclosing scope. SPL's closure design leverages second-class references to eliminate lifetime complexity.

For complete closure specification, see [closures.md](closures.md).

### Summary

SPL distinguishes between **escaping** and **non-escaping** closures:

| Context | Non-Copy Capture Behavior |
|---------|---------------------------|
| Non-escaping (e.g., `map`, `filter`) | Borrow by default |
| Escaping (stored, returned, spawned) | Move by default |

Use `@[...]` capture lists for explicit control over capture behavior:

```spl
let data = Arc.new(vec![1, 2, 3]);

// Escaping: data moved (default)
let f = || process(data);
// data no longer valid

// Escaping: data cloned via capture list
let f = @[data: data.clone()] || process(data);
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

## 8. Scoped Types

**Scoped types** are a special category of types that can hold references but are compiler-enforced to never escape their creation scope. This enables patterns like reference iterators while maintaining memory safety without lifetime annotations.

### The `#[scoped]` Attribute

```spl
#[scoped]
struct HashMapIter(
    source: &HashMap(K, V),  // Allowed: struct is scoped
    position: BucketPos,
) where K, V
```

The `#[scoped]` attribute marks a struct as non-escaping. Unlike regular structs (which cannot hold references), scoped structs can store references because the compiler guarantees they cannot outlive those references.

### Scoped Type Rules

| Rule | Rationale |
|------|-----------|
| Cannot be stored in non-scoped structs | Prevents ref escape via embedding |
| Cannot be returned from non-scoped functions | Prevents ref escape via return |
| Cannot be sent to other tasks | Task-safety (no dangling refs) |
| Must be used within lexical scope of creation | Prevents ref outliving source |
| Can only be passed to functions expecting scoped types | Callee must respect scope |

### Valid Uses

```spl
// Creating and using within scope - OK
{
    let map: HashMap(K: String, V: i32) = /* ... */;
    let iter = map.ref_iter();  // Creates scoped HashMapIter

    while iter.next() is Some((k, v)) {
        println(k, v);
    }
}  // iter dropped, borrow of map ends

// Passing to functions that consume scoped types - OK
fn process_all(iter: I) where I: RefIterator {
    while iter.next() is Some(item) {
        process(item);
    }
}

let iter = map.ref_iter();
process_all(iter);  // iter moved into function, consumed there
```

### Invalid Uses

```spl
// ERROR: Returning scoped type from non-scoped function
fn make_iter(map: &HashMap): HashMapIter {
    return map.ref_iter();
}
// Error: cannot return scoped type `HashMapIter` from non-scoped function

// ERROR: Storing scoped type in non-scoped struct
struct IterHolder(
    iter: HashMapIter,
)
// Error: cannot store scoped type `HashMapIter` in non-scoped struct

// ERROR: Sending scoped type to another task
let iter = map.ref_iter();
spawn(|| {
    for item in iter { ... }
});
// Error: scoped type `HashMapIter` cannot be sent to another task
```

### Scoped Functions

Functions can also be marked `#[scoped]` to indicate they return scoped types:

```spl
#[scoped]
fn filtered_iter(map: &HashMap): Filter(I: HashMapIter) {
    return map.ref_iter().filter(|x| x.1 > 0);
}
```

A scoped function can only be called from within a scope that will contain the result - essentially, the result cannot escape the calling scope.

### Relationship to Second-Class References

Scoped types are an extension of the second-class reference model:

| Type Category | Can Hold Refs? | Can Escape Scope? |
|---------------|----------------|-------------------|
| Regular structs | No | Yes |
| Scoped structs | Yes | No |
| Function parameters | Yes (as refs) | N/A |
| Function returns | Yes (with input ref) | Follows intersection |

### Relationship to Non-Escaping Closures

Scoped types use the same mental model as non-escaping closures:

| Non-Escaping Closures | Scoped Types |
|-----------------------|--------------|
| Can borrow from enclosing scope | Can hold references |
| Cannot be stored or returned | Cannot escape creation scope |
| Used by `map`, `filter`, `each` | Used by `RefIterator` chain |
| Compiler tracks escaping | Compiler tracks scope |

From ADR-012 (Closures):
> "Non-escaping closures can temporarily borrow from the enclosing scope because the borrow doesn't outlive the function call"

The same reasoning applies to scoped structs.

### Primary Use Case: RefIterator

The primary use case for scoped types is the `RefIterator` trait, which enables reference iteration over non-indexed collections:

```spl
trait RefIterator {
    type Item;
    fn next(&mut self): Option(&Self.Item);
}

// Usage
for (k, v) in &hashmap {
    process(k, v);
}

// Chaining
let result = map.ref_iter()
    .filter(|kv| kv.1 > 0)
    .take(10)
    .collect();
```

See [iteration.md](iteration.md) for complete RefIterator documentation.

### Implementation Notes

- **No runtime cost**: Scoped-ness is purely a compile-time property
- **Same memory layout**: Scoped structs have identical layout to non-scoped equivalents
- **Propagates through generics**: `where I: RefIterator` implies `I` is scoped
- **Error messages**: Compiler provides clear messages about escape attempts

---

## 9. Memory Layout

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

## 10. Interior Mutability (Future)

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

## 11. Future Extensions

SPL's memory model is designed to support multiple memory management strategies in future versions.

### 11.1 Interior Iteration with Generators

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

### 11.2 Region-Based Memory

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

### 11.3 Allocator-Aware Types (Zig-style)

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

### 11.4 Optional Garbage Collection (D-style)

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

### 11.5 Generational References (Vale-style)

References with generation counts for safe manual memory:

```spl
// Hypothetical syntax
let x = gen_alloc(Point(x: 1.0, y: 2.0));
let r: GenRef(T: Point) = &x;

gen_free(x);  // Explicitly free

// r.get() returns None (generation mismatch)
// instead of use-after-free
```

### 11.6 Linear Types

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

### 11.7 Strategy Selection

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

## 12. Comparison with Other Languages

| Feature | SPL v1 | Rust | Go | Zig | D |
|---------|--------|------|----|----|---|
| Ownership | Yes | Yes | No | Optional | No |
| Move semantics | Default | Default | No | No | No |
| Borrow checker | Yes (simple) | Yes (full) | No | No | No |
| Lifetime annotations | Optional (intersection default) | Required | N/A | N/A | N/A |
| GC | No | No | Yes | No | Optional |
| Manual memory | Via unsafe | Via unsafe | No | Yes | Yes |
| Custom allocators | Planned | Yes | No | Yes | Yes |

---

## 13. Summary

### V1 Memory Model

| Aspect | Behavior |
|--------|----------|
| Default semantics | Move |
| Copy types | Primitives, opt-in for structs |
| References | Second-class with intersection semantics |
| Scoped types | Can hold refs but cannot escape scope |
| Borrowing | `&T` (shared) and `&mut T` (exclusive) |
| Lifetimes | Optional markers, intersection default |
| Place expressions | Compile-time memory locations |
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
6. **No reference escapes**: Provenance tracking ties outputs to inputs
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

### Intersection Semantics Patterns

Intersection semantics enable returning references from functions with one or more input references:

```spl
// Single input ref - return substring
fn first_word(s: &str): &str {
    let bytes = s.as_bytes();
    for (i, byte) in bytes.enumerate() {
        if byte == b' ' {
            return &s[0..i];
        }
    }
    return s;
}

// Multiple input refs - output borrows from all inputs
fn longer(a: &str, b: &str): &str {
    if a.len() > b.len() { return a; }
    return b;
}

// Single input ref (&self) - return element reference
struct Container(
    data: Vec(T: i32),
)

impl Container {
    // Direct reference return
    fn first(&self): &i32 {
        return &self.data[0];
    }

    // Optional reference for safe access
    fn get(&self, idx: usize): Option(&i32) {
        if idx < self.data.len() {
            return Some(&self.data[idx]);
        }
        return None;
    }

    // Tuple of references (both borrow from &self)
    fn first_and_last(&self): (&i32, &i32) {
        return (&self.data[0], &self.data[self.data.len() - 1]);
    }
}
```

### Intersection Semantics in Practice

```spl
// The output is valid while ALL inputs are valid
let s1 = "hello";
{
    let s2 = "world!";
    let result = longer(s1, s2);  // Borrows from BOTH s1 and s2
    println(result);              // OK: both still valid
}
// result would be invalid here (s2 dropped)

// This is conservative: even if result actually points to s1,
// the compiler assumes it might point to s2
```

### Using Lifetime Markers for Precision

When intersection is too conservative, use markers:

```spl
// Without markers: result borrows from both, invalid after s2 drops
fn longer(a: &str, b: &str): &str { ... }

// With markers: result borrows only from 'a
fn first_if_longer(a: &'a str, b: &str): &'a str {
    if a.len() > b.len() { return a; }
    return "";  // Static string, not b
}

let s1 = "hello";
let result: &str;
{
    let s2 = "hi";
    result = first_if_longer(s1, s2);  // Only borrows from s1
}
println(result);  // OK: s1 still valid, s2 doesn't matter

// Compiler enforces the contract
fn bad_first_if_longer(a: &'a str, b: &str): &'a str {
    return b;  // ERROR: b is not 'a
}
```

### Multiple Inputs in Same Lifetime Group

```spl
// Both a and b are in the 'x group
fn pick_one(a: &'x str, b: &'x str, prefer_first: bool): &'x str {
    if prefer_first { return a; }
    return b;  // OK: b is also 'x
}

// Caller: result valid while both inputs valid
let result = pick_one(s1, s2, true);
// result borrows from 'x group (both s1 and s2)
```

### When Owned Values Are Still Needed

Use owned values when the result must outlive an input:

```spl
// When you need the value to outlive one of the borrows
fn to_uppercase(s: &str): String {
    return s.to_uppercase();  // Owned String, not a borrow
}

// Cross-task scenarios (data must be owned or Arc-wrapped)
fn spawn_with_data(data: String) {
    spawn(|| {
        process(data);  // data moved into task
    });
}

// When you need to store the result longer than the inputs
fn save_longer(a: &str, b: &str): String {
    // If you needed to store the result in a struct or return
    // it from a scope where inputs are dropped, use owned:
    if a.len() > b.len() {
        return a.to_string();
    }
    return b.to_string();
}
```

---

## References

- [closures.md](closures.md) - Closure capture and second-class references
- [iteration.md](iteration.md) - Iteration design including RefIterator
- [attributes.md](attributes.md) - The `#[scoped]` attribute
- [traits.md](traits.md) - RefIterator trait definition
- [unsafe.md](unsafe.md) - Raw pointers and unsafe operations
- [concurrency.md](concurrency.md) - Ownership across tasks
- [ffi.md](ffi.md) - Memory safety at FFI boundaries
- [ADR-015: Scoped Types and RefIterator](../designs/015-scoped-types-refiterator.md) - Design rationale
