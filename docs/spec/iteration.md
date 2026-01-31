# SPL Iteration and Generators

This document specifies iteration mechanisms in SPL, including `for` loops, the `IndexIter` trait, internal iteration methods, and generators.

## Overview

SPL's iteration model is built on **intersection semantics**: functions can return references that are assumed to borrow from all input references. This enables traits like `IndexIter` to have methods that return references to collection elements.

- **`for` loops** use the `IndexIter` trait for reference iteration over indexed collections
- **Internal iteration** (`.each()`, `.map()`) uses closures for functional-style processing
- **Generators** yield owned values only
- **External iterators** (`Iterator` trait) work with owned/consumed values

### Why Two Traits?

SPL has second-class references, which creates a fundamental design constraint:

| Trait | Purpose | Why It Works |
|-------|---------|--------------|
| `IndexIter` | Reference iteration over indexed collections | `get(&self, i): &T` creates fresh borrow each call |
| `Iterator` | Value iteration (generators, ranges) | Yields owned values, no reference storage needed |

**Non-indexed types** (hashmaps, linked lists, trees):
- Use internal iteration: `.each(fn(&T))` for reference access
- Use `IntoIterator` → `Iterator` for consuming iteration
- **Future:** `RefIterator` trait will enable `for x in &hashmap` syntax

---

## 1. For Loops

The `for` loop is the primary iteration mechanism. It has special compiler support to safely handle references.

### Syntax

```ebnf
ForExpr = "for" Pattern "in" Expression Block ;
```

### Forms

| Form | Iteration Type | Collection After |
|------|----------------|------------------|
| `for x in &collection` | By reference (`&T`) | Preserved |
| `for x in &mut collection` | By mutable reference (`&mut T`) | Preserved |
| `for x in collection` | By value (`T`) | Consumed |

### Examples

```spl
let vec: Vec(T: i32) = [1, 2, 3, 4, 5];

// Iterate by reference (most common)
for item in &vec {
    println(item);  // item: &i32
}
// vec still valid here

// Iterate by mutable reference
let mut vec2: Vec(T: i32) = [1, 2, 3];
for item in &mut vec2 {
    *item *= 2;  // item: &mut i32
}
// vec2 is now [2, 4, 6]

// Iterate by value (consumes)
for item in vec {
    println(item);  // item: i32 (owned)
}
// vec is no longer valid
```

### Desugaring

The compiler transforms `for` loops into method calls on the `IndexIter` trait:

```spl
// Source
for item in &collection {
    body
}

// Desugars to
{
    let mut __i: usize = 0;
    let __len: usize = collection.len();
    while __i < __len {
        let item: &T = collection.get(__i);
        body
        __i += 1;
    }
}
```

The `get(&self): &T` method is legal because there is an input reference (`&self`) for the output to borrow from.

For mutable iteration:

```spl
// Source
for item in &mut collection {
    body
}

// Desugars to
{
    let mut __i: usize = 0;
    let __len: usize = collection.len();
    while __i < __len {
        let item: &mut T = collection.get_mut(__i);
        body
        __i += 1;
    }
}
```

For consuming iteration:

```spl
// Source
for item in collection {
    body
}

// Desugars to (conceptually)
{
    let mut __iter = collection.into_iter();
    while __iter.next() is Some(item) {
        body
    }
}
```

### Desugaring: IndexIter vs Iterator

The `for` loop uses different mechanisms depending on the type:

| Syntax | Trait | Desugaring | Notes |
|--------|-------|------------|-------|
| `for x in &coll` | `IndexIter` | IndexIter while loop with `get(i)` | Random-access collections |
| `for x in &mut coll` | `IndexIter` | IndexIter while loop with `get_mut(i)` | Random-access collections |
| `for x in coll` | `Iterator` / `IntoIterator` | While loop with `next()` | Generators, ranges, consuming |

**Why two mechanisms?**

- **IndexIter (random-access)**: Enables reference iteration (`for x in &collection`) using the `get(&self, i): &T` method. Intersection semantics make this safe—each `get()` call returns a reference tied to the collection's lifetime. Requires O(1) indexed access.

- **Iterator (next-based)**: Used for generators, ranges, and types without random access. Returns owned values, not references. Also used for consuming iteration (`for x in collection`).

The compiler automatically selects the appropriate desugaring based on the type.

**Non-indexed collections** (hashmaps, linked lists, trees) use:
- `.each(fn(&T))` for reference iteration (internal iteration)
- `IntoIterator` for consuming iteration
- **Future:** `RefIterator` trait for `for x in &hashmap` syntax

### Break and Continue

`break` and `continue` work as expected:

```spl
for item in &vec {
    if *item == target {
        break;
    }
    if *item < 0 {
        continue;
    }
    process(item);
}
```

### Loop Labels

Labels use postfix colon for definition and prefix colon for reference:

```spl
outer: for row in &matrix {
    for cell in row {
        if *cell == 0 {
            break :outer;
        }
    }
}
```

---

## 2. The IndexIter Trait

Types that support reference iteration via `for x in &collection` implement `IndexIter`. This trait requires random/indexed access (O(1) element retrieval by index).

### Definition

```spl
trait IndexIter {
    type Item;

    /// Number of elements
    fn len(&self): usize;

    /// Returns reference to element at index.
    /// The returned reference borrows from &self.
    /// Panics if index >= len().
    fn get(&self, index: usize): &Self.Item;

    /// Returns mutable reference to element at index.
    /// The returned reference borrows from &mut self.
    /// Panics if index >= len().
    fn get_mut(&mut self, index: usize): &mut Self.Item;
}
```

The `get` and `get_mut` methods return references. With intersection semantics, the output reference is tied to the input's lifetime (`&self` or `&mut self`).

### Why "IndexIter"?

The name emphasizes both **indexed access** and **iteration**: types implementing `IndexIter` support efficient O(1) random access by index, enabling index-based iteration via `get(i)` calls. This distinguishes it from the `Index` trait (which provides the `[]` subscript operator) and from `Iterator` (which yields owned values via `next()`).

Types without indexed access (hashmaps, linked lists, trees) should not implement `IndexIter`. Instead, they use:
- `.each(fn(&T))` for reference iteration
- `IntoIterator` for consuming iteration

### Standard Implementations

```spl
impl IndexIter for Vec(T: T) where T {
    type Item = T;

    fn len(&self): usize { self.length }
    fn get(&self, index: usize): &T { &self.data[index] }
    fn get_mut(&mut self, index: usize): &mut T { &mut self.data[index] }
}

impl IndexIter for [T; N] where T {
    type Item = T;

    fn len(&self): usize { N }
    fn get(&self, index: usize): &T { &self[index] }
    fn get_mut(&mut self, index: usize): &mut T { &mut self[index] }
}

impl IndexIter for String {
    type Item = char;

    fn len(&self): usize { self.char_count() }
    fn get(&self, index: usize): &char { ... }
    fn get_mut(&mut self, index: usize): &mut char { ... }
}
```

### Bounds Checking

Index access via `get()` and `get_mut()` panics if `index >= len()`. Use `get_opt()` for checked access:

```spl
trait IndexIter {
    // ... previous methods ...

    /// Returns Some(&element) if index is valid, None otherwise.
    /// The Option contains a reference that borrows from &self.
    fn get_opt(&self, index: usize): Option(&Self.Item) {
        if index < self.len() { Some(self.get(index)) } else { None }
    }
}
```

The `Option(&Self.Item)` return type is legal because there is an input reference (`&self`) for the contained reference to borrow from.

---

## 3. Ranges

Ranges are built-in types that implement `Iterator`, yielding owned values.

### Range Types

| Syntax | Type | Values | Description |
|--------|------|--------|-------------|
| `a..b` | `Range(T: T)` | a, a+1, ..., b-1 | Exclusive end |
| `a..=b` | `RangeInclusive(T: T)` | a, a+1, ..., b | Inclusive end |
| `a..` | `RangeFrom(T: T)` | a, a+1, ... | Unbounded end |
| `..b` | `RangeTo(T: T)` | N/A | For slicing only |
| `..=b` | `RangeToInclusive(T: T)` | N/A | For slicing only |
| `..` | `RangeFull` | N/A | For slicing only |

### Examples

```spl
// Exclusive range
for i in 0..5 {
    println(i);  // 0, 1, 2, 3, 4
}

// Inclusive range
for i in 0..=5 {
    println(i);  // 0, 1, 2, 3, 4, 5
}

// Character ranges
for c in 'a'..='z' {
    println(c);
}

// Range methods
let r = 0..10;
let contains = r.contains(&5);  // true
let count = r.len();            // 10
```

### Range Implementation

Ranges implement `Iterator` rather than `IndexIter` because they produce computed values rather than references to stored data:

```spl
struct Range(
    start: T,
    end: T,
) where T: Step

impl Iterator for Range(T: T) where T: Step {
    type Item = T;

    fn next(&mut self): T? {
        if self.start < self.end {
            let value = self.start;
            self.start = self.start.forward(1);
            return Some(value);
        }
        return None;
    }
}

impl Range(T: T) where T: Step {
    fn len(&self): usize {
        self.end.steps_from(&self.start)
    }

    fn contains(&self, value: &T): bool {
        value >= &self.start && value < &self.end
    }
}
```

The `Step` trait defines types that can be iterated:

```spl
trait Step {
    fn steps_from(&self, start: &Self): usize;
    fn forward(&self, count: usize): Self;
    fn backward(&self, count: usize): Self;
}
```

Implemented for all integer types and `char`.

---

## 4. Internal Iteration

Internal iteration uses closures to process elements. References stay scoped within the closure.

### Core Methods

```spl
impl Vec(T: T) where T {
    /// Apply function to each element
    fn each(&self, f: fn(&T)) {
        for item in self {
            f(item);
        }
    }

    /// Apply function to each element (mutable)
    fn each_mut(&mut self, f: fn(&mut T)) {
        for item in &mut self {
            f(item);
        }
    }

    /// Iterate with index
    fn enumerate(&self): Enumerate(&Self) {
        Enumerate(inner: self, index: 0)
    }
}
```

### Lazy Adapters

Adapters build a pipeline that executes on terminal operations:

```spl
impl Vec(T: T) where T {
    /// Create lazy iterator adapter
    fn iter(&self): Iter(S: &Vec(T: T)) {
        Iter(source: self)
    }
}

struct Iter(source: S) where S

impl Iter(S: S) where S: IndexIter {
    /// Transform elements
    fn map(self, f: fn(&S.Item): U): Map(S: Self, U: U) where U {
        Map(source: self, func: f)
    }

    /// Filter elements
    fn filter(self, pred: fn(&S.Item): bool): Filter(S: Self) {
        Filter(source: self, predicate: pred)
    }

    /// Take first n elements
    fn take(self, n: usize): Take(S: Self) {
        Take(source: self, count: n)
    }

    /// Skip first n elements
    fn skip(self, n: usize): Skip(S: Self) {
        Skip(source: self, count: n)
    }

    /// Flatten nested iterables
    fn flatten(self): Flatten(S: Self) where S.Item: IndexIter {
        Flatten(source: self)
    }

    /// Chain with another iterable
    fn chain(self, other: O): Chain(A: Self, B: O) where O: IndexIter(Item: S.Item) {
        Chain(first: self, second: other)
    }

    /// Zip with another iterable
    fn zip(self, other: O): Zip(A: Self, B: O) where O: IndexIter {
        Zip(first: self, second: other)
    }
}
```

### Terminal Operations

Terminal operations consume the adapter and produce a result:

```spl
impl Iter(S: S) /* and all adapters */ {
    /// Collect into a Vec
    fn collect(self): Vec(T: T) where T: Clone {
        let mut result: Vec(T: T) = Vec.new();
        self.each(|item| result.push(item.clone()));
        result
    }

    /// Apply function to each element
    fn each(self, f: fn(&S.Item)) {
        // Execute the pipeline
    }

    /// Reduce to single value
    fn fold(self, init: U, f: fn(U, &S.Item): U): U where U {
        let mut acc = init;
        self.each(|item| acc = f(acc, item));
        acc
    }

    /// Sum all elements
    fn sum(self): S.Item where S.Item: Add {
        self.fold(0, |acc, x| acc + x)
    }

    /// Find first matching element
    fn find(self, pred: fn(&S.Item): bool): S.Item? {
        // Returns owned clone of found element
    }

    /// Check if any element matches
    fn any(self, pred: fn(&S.Item): bool): bool {
        // Short-circuits on first match
    }

    /// Check if all elements match
    fn all(self, pred: fn(&S.Item): bool): bool {
        // Short-circuits on first non-match
    }

    /// Count elements
    fn count(self): usize { ... }

    /// Get first element
    fn first(self): S.Item? { ... }

    /// Get last element
    fn last(self): S.Item? { ... }

    /// Get nth element
    fn nth(self, n: usize): S.Item? { ... }

    /// Get min element
    fn min(self): S.Item? where S.Item: Ord { ... }

    /// Get max element
    fn max(self): S.Item? where S.Item: Ord { ... }
}
```

### Usage Examples

```spl
let numbers: Vec(T: i32) = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

// Filter and map
let evens_doubled: Vec(T: i32) = numbers
    .iter()
    .filter(|x| x % 2 == 0)
    .map(|x| x * 2)
    .collect();
// [4, 8, 12, 16, 20]

// Sum with condition
let sum_of_small: i32 = numbers
    .iter()
    .filter(|x| x < &5)
    .sum();
// 10

// Find first match
let first_even: i32? = numbers
    .iter()
    .find(|x| x % 2 == 0);
// Some(2)

// Check conditions
let has_negative = numbers.iter().any(|x| *x < 0);  // false
let all_positive = numbers.iter().all(|x| *x > 0);  // true

// Enumerate
numbers.enumerate().each(|(i, item)| {
    println("Index: ", i, " Value: ", item);
});

// Zip
let a = [1, 2, 3];
let b = ["a", "b", "c"];
a.iter().zip(b.iter()).each(|(num, letter)| {
    println(num, letter);
});
```

---

## 5. Generators

Generators are functions that can yield multiple values lazily. They provide a convenient way to create custom iteration patterns.

### Syntax

```ebnf
GeneratorDef = "gen" "fn" IDENTIFIER "(" [ ParamList ] ")" ":" Type [ WhereClause ] Block ;

YieldExpr = "yield" Expression ;
```

> **Note:** The `: Type` in a generator signature specifies the **yield type** (the type of values produced by `yield`), not the return type. The actual return type is `Generator(T: Type)`. This follows the intuition that the type annotation describes "what you get" from calling the function—for generators, that's the yielded values.

### Basic Generators

```spl
// Simple generator
gen fn countdown(from: i32): i32 {
    let mut n = from;
    while n > 0 {
        yield n;
        n -= 1;
    }
}

// Usage
for n in countdown(5) {
    println(n);  // 5, 4, 3, 2, 1
}
```

### Generator Type

A generator function returns a `Generator(T: T)` type:

```spl
gen fn countdown(from: i32): i32 { ... }

// Equivalent to returning:
fn countdown(from: i32): Generator(T: i32) { ... }
```

The `Generator(T: T)` type can be used in `for` loops:

```spl
struct Generator(
    // Internal state (compiler-generated)
) where T
```

> **Note:** Generators are consumed during iteration. Unlike indexable collections, generators maintain internal state and produce values on-demand. The `for` loop over a generator uses the `Iterator` trait (with `next()` returning owned values), not `IndexIter`. See "Consuming Iteration" below for the `Iterator` trait.

### Infinite Generators

Generators can be infinite:

```spl
gen fn naturals(): i32 {
    let mut n = 0;
    loop {
        yield n;
        n += 1;
    }
}

// Must use take() or break to terminate
for n in naturals().take(10) {
    println(n);  // 0, 1, 2, ..., 9
}
```

### Generators with Parameters

```spl
gen fn range_by(start: i32, end: i32, step: i32): i32 {
    let mut n = start;
    while n < end {
        yield n;
        n += step;
    }
}

for n in range_by(0, 100, 10) {
    println(n);  // 0, 10, 20, ..., 90
}
```

### Fibonacci Example

```spl
gen fn fibonacci(): i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    loop {
        yield a;
        let next = a + b;
        a = b;
        b = next;
    }
}

// First 10 Fibonacci numbers
let fibs: Vec(T: i64) = fibonacci().take(10).collect();
// [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]
```

### Generator Adapters

Generators work with all iterator adapters:

```spl
let evens = naturals()
    .filter(|n| n % 2 == 0)
    .take(5)
    .collect();
// [0, 2, 4, 6, 8]

let squares = naturals()
    .map(|n| n * n)
    .take_while(|n| *n < 100)
    .collect();
// [0, 1, 4, 9, 16, 25, 36, 49, 64, 81]
```

### Early Return in Generators

`return` in a generator ends iteration:

```spl
gen fn until_zero(vec: &Vec(T: i32)): i32 {
    for item in vec {
        if *item == 0 {
            return;  // Stop iteration
        }
        yield *item;
    }
}
```

### Delegation with `yield from` (Future)

```spl
gen fn flatten(nested: &Vec(T: Vec(T: i32))): i32 {
    for inner in nested {
        yield from inner.iter();  // Yield all from inner
    }
}
```

### Generator Constraints

**Generators yield owned values only.** This maintains compatibility with second-class references:

```spl
// ALLOWED: yields owned i32
gen fn squares(): i32 {
    for i in 0.. {
        yield i * i;
    }
}

// NOT ALLOWED: cannot yield references
// gen fn refs(vec: &Vec(T: i32)): &i32 {
//     for item in vec {
//         yield item;  // ERROR: cannot yield reference
//     }
// }

// WORKAROUND: yield cloned values
gen fn cloned(vec: &Vec(T: i32)): i32 {
    for item in vec {
        yield *item;  // Yield copied value
    }
}
```

### Generator State

Generators are stateful. Each call to iteration advances the state:

```spl
let gen = countdown(3);

// Manual iteration
gen.next();  // Some(3)
gen.next();  // Some(2)
gen.next();  // Some(1)
gen.next();  // None
gen.next();  // None (stays exhausted)
```

Generators cannot be restarted. Create a new generator to iterate again.

### Generator Completion and Panic Behavior

**Completion semantics:**
- After a generator returns (reaches end of function or explicit `return`), it enters the "completed" state
- Subsequent calls to `next()` always return `None`
- A completed generator cannot be resumed or restarted

**Panic behavior:**
- If a generator panics during execution, the panic propagates to the caller
- The generator enters a "poisoned" state and cannot be resumed
- Subsequent calls to `next()` on a poisoned generator will panic with "generator panicked previously"
- Destructors for captured state run during unwinding (if `panic=unwind`)

```spl
gen fn might_panic(): i32 {
    yield 1;
    panic("oops");  // Panic during iteration
    yield 2;        // Never reached
}

let gen = might_panic();
gen.next();  // Some(1)
gen.next();  // PANIC: "oops" - propagates to caller
// gen is now poisoned
```

**Memory layout:**
- Generator state is stored inline (no heap allocation for the state machine itself)
- Captured variables are stored in the generator struct
- Generator size depends on captured state, similar to closures

---

## 6. Consuming Iteration

For types that cannot be indexed (trees, hash maps), provide consuming iterators:

### IntoIterator Trait

```spl
trait IntoIterator {
    type Item;
    type Iter: Iterator(Item: Self.Item);

    fn into_iter(self): Self.Iter;
}

trait Iterator {
    type Item;

    fn next(&mut self): Self.Item?;
}
```

### HashMap Example

```spl
impl IntoIterator for HashMap(K: K, V: V) where K, V {
    type Item = (K, V);
    type Iter = HashMapIter(K: K, V: V);

    fn into_iter(self): HashMapIter(K: K, V: V) {
        HashMapIter(map: self, index: 0)
    }
}

// Usage - consumes the map
for (key, value) in map {
    println(key, value);
}
// map is no longer valid

// For non-consuming iteration, use internal iteration
map.each(|key, value| {
    println(key, value);
});
// map still valid
```

### Tree Example

```spl
impl BinaryTree(T: T) where T {
    // Internal iteration (non-consuming)
    fn traverse_inorder(&self, f: fn(&T)) {
        fn visit(node: &Node(T: T), f: fn(&T)) {
            if node.left is Some(left) {
                visit(left, f);
            }
            f(&node.value);
            if node.right is Some(right) {
                visit(right, f);
            }
        }
        if self.root is Some(root) {
            visit(root, f);
        }
    }

    // Generator-based (non-consuming, yields clones)
    gen fn inorder(&self): T where T: Clone {
        // Implementation uses internal stack
    }
}

// Usage
tree.traverse_inorder(|value| println(value));

for value in tree.inorder() {
    println(value);  // value is cloned
}
```

---

## 7. Reference Iteration with RefIterator

For non-indexed collections (HashMap, LinkedList, BTreeMap, Tree, etc.) that cannot provide O(1) random access, SPL uses the `RefIterator` trait combined with **scoped types** to enable `for x in &collection` syntax.

### The Problem

Non-indexed collections cannot implement `IndexIter` because:
- `IndexIter` requires O(1) random access via `get(&self, i): &T`
- HashMap, LinkedList, Tree etc. only support sequential traversal

A traditional external iterator would need to store a reference to the collection:

```spl
// NOT ALLOWED: References cannot be stored in structs
struct HashMapIter(
    source: &HashMap(K, V),  // ERROR: second-class reference rule
    position: BucketPos,
)
```

### The Solution: Scoped Types

Scoped types are a special category of types that **can** hold references but are compiler-enforced to never escape their creation scope:

```spl
#[scoped]  // Marks this type as non-escaping
struct HashMapIter(
    source: &HashMap(K, V),  // ALLOWED: struct is scoped
    position: BucketPos,
) where K, V
```

### Scoped Type Rules

A `#[scoped]` type has these restrictions:

| Rule | Example |
|------|---------|
| Cannot be stored in non-scoped structs | `struct Foo(iter: HashMapIter)` - ERROR |
| Cannot be returned from non-scoped functions | `fn make_iter(): HashMapIter` - ERROR |
| Cannot be sent to other threads | `spawn(\|\| use(iter))` - ERROR |
| Must be used within lexical scope of creation | See examples below |

### The RefIterator Trait

```spl
trait RefIterator {
    type Item;

    /// Returns Some(&item) while items remain, None when exhausted.
    /// The returned reference borrows from &mut self (intersection semantics).
    fn next(&mut self): Option(&Self.Item);

    /// Number of remaining items, if known.
    fn size_hint(&self): (usize, Option(usize)) {
        return (0, None);
    }
}
```

### For-Loop Desugaring with RefIterator

```spl
// Source
for (k, v) in &hashmap {
    process(k, v);
}

// Desugars to
{
    let mut __iter = hashmap.ref_iter();  // scoped type
    while __iter.next() is Some((k, v)) {
        process(k, v);
    }
}  // __iter dropped, borrow of hashmap ends
```

### Standard Adapters

Adapters are also scoped types that wrap other RefIterators:

```spl
#[scoped]
struct Filter(
    inner: I,
    predicate: fn(&I.Item): bool,
) where I: RefIterator

impl RefIterator for Filter(I: I) where I: RefIterator {
    type Item = I.Item;

    fn next(&mut self): Option(&Self.Item) {
        while self.inner.next() is Some(item) {
            if (self.predicate)(item) {
                return Some(item);
            }
        }
        return None;
    }
}

#[scoped]
struct Take(
    inner: I,
    remaining: usize,
) where I: RefIterator

#[scoped]
struct Skip(
    inner: I,
    remaining: usize,
) where I: RefIterator

#[scoped]
struct Map(
    inner: I,
    transform: fn(&I.Item): U,
) where I: RefIterator, U
```

### Chaining Example

```spl
let map: HashMap(K: String, V: i32) = /* ... */;

// Full chaining works because all intermediate types are scoped
let result: Vec(T: i32) = map.ref_iter()      // HashMapIter (scoped)
    .filter(|kv| kv.1 > 10)                    // Filter (scoped)
    .take(5)                                   // Take (scoped)
    .map(|kv| kv.1.clone())                    // Map yields owned values
    .collect();                                // Collects owned values
```

### Terminal Operations

Terminal operations consume the iterator and produce owned values:

```spl
impl RefIterator {
    /// Collect clones into a Vec
    fn collect(&mut self): Vec(T: Self.Item) where Self.Item: Clone {
        let mut result = Vec.new();
        while self.next() is Some(item) {
            result.push(item.clone());
        }
        return result;
    }

    /// Count items
    fn count(&mut self): usize {
        let mut n = 0;
        while self.next() is Some(_) {
            n += 1;
        }
        return n;
    }

    /// Find first matching item (cloned)
    fn find(&mut self, pred: fn(&Self.Item): bool): Option(Self.Item)
    where Self.Item: Clone {
        while self.next() is Some(item) {
            if pred(item) {
                return Some(item.clone());
            }
        }
        return None;
    }

    /// Apply function to each element
    fn for_each(&mut self, f: fn(&Self.Item)) {
        while self.next() is Some(item) {
            f(item);
        }
    }

    /// Check if any element satisfies predicate
    fn any(&mut self, pred: fn(&Self.Item): bool): bool {
        while self.next() is Some(item) {
            if pred(item) {
                return true;
            }
        }
        return false;
    }

    /// Check if all elements satisfy predicate
    fn all(&mut self, pred: fn(&Self.Item): bool): bool {
        while self.next() is Some(item) {
            if !pred(item) {
                return false;
            }
        }
        return true;
    }
}
```

### HashMap RefIterator Example

```spl
impl HashMap(K: K, V: V) where K, V {
    /// Returns a scoped reference iterator over key-value pairs.
    fn ref_iter(&self): HashMapIter(K: K, V: V) {
        return HashMapIter(source: self, position: BucketPos.start());
    }
}

#[scoped]
struct HashMapIter(
    source: &HashMap(K, V),
    position: BucketPos,
) where K, V

impl RefIterator for HashMapIter(K: K, V: V) where K, V {
    type Item = (K, V);

    fn next(&mut self): Option(&(K, V)) {
        while self.position.is_valid() {
            if self.source.bucket_occupied(self.position) {
                let entry = self.source.get_entry(self.position);
                self.position.advance();
                return Some(entry);
            }
            self.position.advance();
        }
        return None;
    }

    fn size_hint(&self): (usize, Option(usize)) {
        let remaining = self.source.len() - self.position.count_before();
        return (0, Some(remaining));
    }
}
```

### Mutable Reference Iteration

For mutable iteration, use `RefIteratorMut`:

```spl
trait RefIteratorMut {
    type Item;

    fn next(&mut self): Option(&mut Self.Item);
}

// Usage
for (k, v) in &mut hashmap {
    *v += 1;  // Mutate values in place
}
```

### What Cannot Be Done with RefIterator

Scoped types have restrictions that prevent certain patterns:

```spl
// ERROR: Cannot return scoped type from non-scoped function
fn make_iter(map: &HashMap): HashMapIter {
    return map.ref_iter();  // compile error
}

// ERROR: Cannot store scoped type in non-scoped struct
struct IterHolder(
    iter: HashMapIter,  // compile error
)

// ERROR: Cannot send scoped type to another thread
let iter = map.ref_iter();
spawn(|| {
    for item in iter { ... }  // compile error
});

// CORRECT: Use within lexical scope
{
    let iter = map.ref_iter();
    for item in iter {
        process(item);
    }
}  // iter dropped here, all borrows end
```

### Trait Comparison

| Trait | For | Yields | Storage | Use Case |
|-------|-----|--------|---------|----------|
| `IndexIter` | Random-access collections | `&T` / `&mut T` | N/A (uses indexing) | Vec, arrays, strings |
| `RefIterator` | Sequential collections | `&T` | Scoped types | HashMap, LinkedList, Tree |
| `Iterator` | Value sequences | `T` (owned) | Regular structs | Generators, ranges, consuming |

---

## 8. Summary

### Iteration Mechanisms

| Mechanism | Reference Safe | Composable | Use Case |
|-----------|---------------|------------|----------|
| `for x in &coll` (IndexIter) | Yes (scoped) | No | Random-access iteration by reference |
| `for x in &coll` (RefIterator) | Yes (scoped) | Yes | Sequential iteration by reference |
| `for x in &mut coll` | Yes (scoped) | No | Mutating iteration |
| `for x in coll` | N/A (owned) | No | Consuming iteration |
| `.iter().map().filter()` | Yes (closures) | Yes | Functional chains |
| `.ref_iter().filter().take()` | Yes (scoped) | Yes | RefIterator chains |
| `.each(\|x\| ...)` | Yes (closure) | No | Side effects |
| `gen fn` | N/A (owned) | Yes | Custom sequences |
| `.into_iter()` | N/A (owned) | Limited | Consuming non-indexable types |

### Key Traits

| Trait | Purpose |
|-------|---------|
| `IndexIter` | Random-access collections with `get(&self, i): &T` for reference iteration |
| `RefIterator` | Sequential reference iteration via scoped types (hashmaps, trees, linked lists) |
| `Iterator` | Sequential value iteration (owned values only) |
| `IntoIterator` | Converting to consuming iterator |
| `Step` | Types that can form ranges |

### Trait Hierarchy

```
IndexIter          - Random-access reference iteration (Vec, arrays)
    ↓ provides
IntoIterator     - Conversion to consuming iterator
    ↓ produces
Iterator         - Sequential value iteration (generators, ranges)

RefIterator      - Scoped reference iteration (hashmaps, trees, linked lists)
    ↓ uses
#[scoped] types  - Can hold refs but cannot escape scope
```

### Collection → Trait Mapping

| Collection Type | Reference Iteration | Consuming Iteration |
|-----------------|---------------------|---------------------|
| Vec, Array, String | `IndexIter` | `IntoIterator` → `Iterator` |
| Range | N/A | `Iterator` (yields owned) |
| HashMap, BTreeMap | `RefIterator` | `IntoIterator` → `Iterator` |
| LinkedList, Tree | `RefIterator` | `IntoIterator` → `Iterator` |
| Generator | N/A | `Iterator` (yields owned) |

### Design Principles

1. **Intersection semantics**: `get(&self, i): &T` is safe because the output borrows from the input
2. **`for` loops use IndexIter for random-access**: Desugars to `get()`/`get_mut()` calls
3. **`for` loops use RefIterator for sequential-access**: Desugars to `next()` calls on scoped types
4. **Scoped types enable reference iteration**: Types marked `#[scoped]` can hold refs but cannot escape
5. **Internal iteration for closures**: `.each()`, `.map()` etc. scope references via closures
6. **Generators yield owned values**: No reference lifetime issues
7. **Consuming iteration when necessary**: `Iterator` trait for owned value sequences

---

## Examples

### Complete Example

```spl
fn main() {
    // Basic for loop
    let numbers: Vec(T: i32) = [1, 2, 3, 4, 5];

    for n in &numbers {
        println(n);
    }

    // Functional chain
    let result: Vec(T: i32) = numbers
        .iter()
        .filter(|x| x % 2 == 0)
        .map(|x| x * 10)
        .collect();
    println(result);  // [20, 40]

    // Generator
    gen fn powers_of_two(): i64 {
        let mut n: i64 = 1;
        loop {
            yield n;
            n *= 2;
        }
    }

    let powers: Vec(T: i64) = powers_of_two()
        .take(10)
        .collect();
    println(powers);  // [1, 2, 4, 8, 16, 32, 64, 128, 256, 512]

    // Range iteration
    for i in 0..=10 {
        if i % 2 == 0 {
            println(i, " is even");
        }
    }

    // Enumerate
    for (i, n) in numbers.enumerate() {
        println("Index ", i, ": ", n);
    }
}
```

### Custom IndexIter Type

```spl
struct CircularBuffer(
    data: [T; 8],
    head: usize,
    len: usize,
) where T

impl IndexIter for CircularBuffer(T: T) where T {
    type Item = T;

    fn len(&self): usize { self.len }

    fn get(&self, index: usize): &T {
        if index >= self.len {
            panic("index out of bounds");
        }
        let actual = (self.head + index) % 8;
        &self.data[actual]
    }

    fn get_mut(&mut self, index: usize): &mut T {
        if index >= self.len {
            panic("index out of bounds");
        }
        let actual = (self.head + index) % 8;
        &mut self.data[actual]
    }
}

// Now works with for loops
fn example(buf: &CircularBuffer(T: i32)) {
    for item in buf {
        println(item);
    }
}
```

---

## References

- [ADR-011: Iteration and Generators](../designs/011-iteration-and-generators.md) - Design rationale
- [ADR-015: Scoped Types and RefIterator](../designs/015-scoped-types-refiterator.md) - Scoped types design
- [traits.md](traits.md) - IndexIter, Iterator, and RefIterator traits
- [closures.md](closures.md) - Closures in iterator chains
- [memory-model.md](memory-model.md) - Second-class references, scoped types, and iteration
- [attributes.md](attributes.md) - The `#[scoped]` attribute
- [syntax-grammar.md](syntax-grammar.md) - For loop and generator syntax
