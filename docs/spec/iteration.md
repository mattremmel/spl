# SPL Iteration and Generators

This document specifies iteration mechanisms in SPL, including `for` loops, the `Iterable` trait, internal iteration methods, and generators.

## Overview

SPL's iteration model is designed around **second-class references**—references cannot be returned from functions or stored in structs. This constraint shapes the iteration design:

- **`for` loops** use compiler magic to keep references scoped to the loop body
- **Internal iteration** (`.each()`, `.map()`) uses closures to scope references
- **Generators** yield owned values only, not references
- **External iterators** are limited to owned/consumed values

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

The compiler transforms `for` loops into indexed `while` loops for types implementing `Iterable`:

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
        let item: &T = &collection[__i];
        body
        __i += 1;
    }
}
```

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
        let item: &mut T = &mut collection[__i];
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

### Desugaring: Iterable vs Iterator

The `for` loop uses different mechanisms depending on the type:

| Type implements | Desugaring | Use case |
|-----------------|------------|----------|
| `Iterable` | Indexed `while` loop with `len()` and `[i]` | Collections with random access (Vec, arrays) |
| `Iterator` | `while next() is Some(item)` loop | Generators, consuming iterators, lazy sequences |

**Why two mechanisms?**

- **Iterable (indexed)**: Enables safe reference iteration (`for x in &collection`) via compiler transformation. The reference `&collection[i]` is created fresh each iteration, staying scoped to the loop body. This respects second-class references.

- **Iterator (next-based)**: Used for generators and types without random access. Returns owned values, not references. Also used for consuming iteration (`for x in collection`).

The compiler automatically selects the appropriate desugaring based on the type.

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

### Loop Labels (Future)

```spl
'outer: for row in &matrix {
    for cell in row {
        if *cell == 0 {
            break 'outer;
        }
    }
}
```

---

## 2. The Iterable Trait

Types that support `for` loop iteration implement `Iterable`.

> **Note:** The trait definition below is *conceptual*. The `for` loop is implemented via compiler magic that transforms the loop into indexed access without literally calling these methods. This avoids violating the second-class reference rule (references cannot be returned from functions). A future version of this specification will provide more detailed semantics.

### Definition (Conceptual)

```spl
trait Iterable {
    type Item;

    /// Number of elements
    fn len(&self): usize;

    /// Access element by index (immutable)
    /// Note: Conceptual - actual for-loop uses compiler transformation
    fn get(&self, index: usize): &Self.Item;

    /// Access element by index (mutable)
    /// Note: Conceptual - actual for-loop uses compiler transformation
    fn get_mut(&mut self, index: usize): &mut Self.Item;
}
```

### Standard Implementations (Conceptual)

These implementations illustrate the contract that iterable types satisfy. The compiler uses this information to transform `for` loops.

```spl
impl Iterable for Vec(T: T) where T {
    type Item = T;

    fn len(&self): usize { self.length }
    fn get(&self, index: usize): &T { &self.data[index] }
    fn get_mut(&mut self, index: usize): &mut T { &mut self.data[index] }
}

impl Iterable for [T; N] where T {
    type Item = T;

    fn len(&self): usize { N }
    fn get(&self, index: usize): &T { &self[index] }
    fn get_mut(&mut self, index: usize): &mut T { &mut self[index] }
}

impl Iterable for String {
    type Item = char;

    fn len(&self): usize { self.char_count() }
    fn get(&self, index: usize): &char { ... }
    fn get_mut(&mut self, index: usize): &mut char { ... }
}
```

### Bounds Checking

Index access via `get()` and `get_mut()` panics if `index >= len()`. Use `get_opt()` for checked access:

```spl
trait Iterable {
    // ... previous methods ...

    /// Note: Like get(), this is conceptual. The return type Option(&Self.Item)
    /// appears to violate second-class references, but the compiler transforms
    /// for-loop usage to avoid actually returning references.
    fn get_opt(&self, index: usize): Option(&Self.Item) {
        if index < self.len() { Some(self.get(index)) } else { None }
    }
}
```

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

Ranges implement `Iterator` rather than `Iterable` because they produce computed values rather than references to stored data:

```spl
struct Range(
    start: T,
    end: T,
) where T: Step

impl Iterator for Range(T: T) where T: Step {
    type Item = T;

    fn next(&mut self): Option(T: T) {
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

impl Iter(S: S) where S: Iterable {
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
    fn flatten(self): Flatten(S: Self) where S.Item: Iterable {
        Flatten(source: self)
    }

    /// Chain with another iterable
    fn chain(self, other: O): Chain(A: Self, B: O) where O: Iterable(Item: S.Item) {
        Chain(first: self, second: other)
    }

    /// Zip with another iterable
    fn zip(self, other: O): Zip(A: Self, B: O) where O: Iterable {
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
    fn find(self, pred: fn(&S.Item): bool): Option(T: S.Item) {
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
    fn first(self): Option(T: S.Item) { ... }

    /// Get last element
    fn last(self): Option(T: S.Item) { ... }

    /// Get nth element
    fn nth(self, n: usize): Option(T: S.Item) { ... }

    /// Get min element
    fn min(self): Option(T: S.Item) where S.Item: Ord { ... }

    /// Get max element
    fn max(self): Option(T: S.Item) where S.Item: Ord { ... }
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
let first_even: Option(T: i32) = numbers
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

> **Note:** Generators are consumed during iteration. Unlike indexable collections, generators maintain internal state and produce values on-demand. The `for` loop over a generator uses the `Iterator` trait (with `next()` returning owned values), not `Iterable`. See "Consuming Iteration" below for the `Iterator` trait.

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

    fn next(&mut self): Option(T: Self.Item);
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

## 7. Summary

### Iteration Mechanisms

| Mechanism | Reference Safe | Composable | Use Case |
|-----------|---------------|------------|----------|
| `for x in &coll` | Yes (scoped) | No | Simple iteration by reference |
| `for x in &mut coll` | Yes (scoped) | No | Mutating iteration |
| `for x in coll` | N/A (owned) | No | Consuming iteration |
| `.iter().map().filter()` | Yes (closures) | Yes | Functional chains |
| `.each(\|x\| ...)` | Yes (closure) | No | Side effects |
| `gen fn` | N/A (owned) | Yes | Custom sequences |
| `.into_iter()` | N/A (owned) | Limited | Consuming non-indexable types |

### Key Traits

| Trait | Purpose |
|-------|---------|
| `Iterable` | Indexable collections (conceptual; enables `for` loop via compiler magic) |
| `Iterator` | External iterator (owned values only) |
| `IntoIterator` | Converting to consuming iterator |
| `Step` | Types that can form ranges |

### Design Principles

1. **Second-class references respected**: References never escape their scope
2. **`for` loops use compiler magic**: Transformed to safe indexed access without calling methods that return references
3. **Internal iteration for references**: Closures scope the borrows
4. **Generators yield owned values**: No reference lifetime issues
5. **Consuming iteration when necessary**: For non-indexable types

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

### Custom Iterable Type

```spl
struct CircularBuffer(
    data: [T; 8],
    head: usize,
    len: usize,
) where T

impl Iterable for CircularBuffer(T: T) where T {
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
