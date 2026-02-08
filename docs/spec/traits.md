# Traits

This document specifies traits in SPL, including trait definition, implementation, bounds, associated types, default implementations, and trait coherence rules.

## Overview

Traits define shared behavior that types can implement. They enable:
- **Polymorphism**: Write code that works with any type implementing a trait
- **Operator overloading**: Implement traits like `Add` to define `+` for custom types
- **Marker traits**: Signal properties like `Copy` or `Send` without methods
- **Extension**: Add methods to types via trait implementations

## 1. Trait Definition

### 1.1 Basic Trait

```spl
trait Greet {
    fn greet(&self): String;
}
```

### 1.2 Trait with Multiple Methods

```spl
trait Shape {
    fn area(&self): f64;
    fn perimeter(&self): f64;
    fn name(&self): &str;
}
```

### 1.3 Trait with Associated Types

```spl
trait Iterator {
    type Item;

    fn next(&mut self): Self.Item?;
}

trait Container {
    type Item;
    type Iter: Iterator(Item: Self.Item);

    fn iter(&self): Self.Iter;
    fn len(&self): usize;
    fn is_empty(&self): bool {
        return self.len() == 0;
    }
}
```

### 1.4 Trait with Type Parameters

```spl
trait From(T) where T {
    fn from(value: T): Self;
}

trait Into(T) where T {
    fn into(self): T;
}

trait Add(RHS) where RHS {
    type Output;
    fn add(self, rhs: RHS): Self.Output;
}
```

### 1.5 Trait with Associated Constants

```spl
trait Bounded {
    const MIN: Self;
    const MAX: Self;
}

impl Bounded for i32 {
    const MIN: i32 = -2147483648;
    const MAX: i32 = 2147483647;
}
```

---

## 2. Trait Implementation

### 2.1 Basic Implementation

```spl
struct Circle(radius: f64)

impl Greet for Circle {
    fn greet(&self): String {
        return format("I'm a circle with radius {}", self.radius);
    }
}
```

### 2.2 Generic Implementation

```spl
// Implement for all types T
impl Greet for Vec(T: T) where T: Debug {
    fn greet(&self): String {
        return format("I'm a vec with {} items", self.len());
    }
}
```

### 2.3 Conditional Implementation

```spl
// Only implement Clone for Option if T is Clone
impl Clone for Option(T: T) where T: Clone {
    fn clone(&self): Self {
        return match self {
            Some(v) => Some(v.clone()),
            None => None,
        };
    }
}

// Only implement Debug for Result if both T and E are Debug
impl Debug for Result(T: T, E: E) where T: Debug, E: Debug {
    fn fmt(&self, f: &mut Formatter): Result(T: (), E: Error) {
        return match self {
            Ok(v) => write(f, "Ok({})", v),
            Err(e) => write(f, "Err({})", e),
        };
    }
}
```

### 2.4 Blanket Implementation

```spl
// Implement ToString for anything that implements Display
impl ToString for T where T: Display {
    fn to_string(&self): String {
        return format("{}", self);
    }
}

// Implement Into(T: U) for anything that implements From(T: T)
impl Into(T: U) for T where T, U: From(T: T) {
    fn into(self): U {
        return U.from(self);
    }
}
```

---

## 3. Default Implementations

### 3.1 Methods with Default Bodies

```spl
trait Iterator {
    type Item;

    // Required method (no body)
    fn next(&mut self): Self.Item?;

    // Default implementations (can be overridden)
    fn count(&mut self): usize {
        let mut n = 0;
        while self.next().is_some() {
            n += 1;
        }
        return n;
    }

    fn last(&mut self): Self.Item? {
        let mut last = None;
        while self.next() is Some(x) {
            last = Some(x);
        }
        return last;
    }

    fn nth(&mut self, n: usize): Self.Item? {
        for _ in 0..n {
            self.next();
        }
        return self.next();
    }
}
```

### 3.2 Overriding Defaults

```spl
struct Range(start: i32, end: i32)

impl Iterator for Range {
    type Item = i32;

    fn next(&mut self): i32? {
        if self.start < self.end {
            let n = self.start;
            self.start += 1;
            return Some(n);
        }
        return None;
    }

    // Override default count() with O(1) implementation
    fn count(&mut self): usize {
        if self.start < self.end {
            let count = (self.end - self.start) as usize;
            self.start = self.end;
            return count;
        }
        return 0;
    }
}
```

### 3.3 Default Methods Calling Other Methods

```spl
trait PartialOrd: PartialEq {
    fn partial_cmp(&self, other: &Self): Option(T: Ordering);

    // Defaults implemented in terms of partial_cmp
    fn lt(&self, other: &Self): bool {
        return self.partial_cmp(other) is Some(Ordering.Less);
    }

    fn le(&self, other: &Self): bool {
        return match self.partial_cmp(other) {
            Some(Ordering.Less) | Some(Ordering.Equal) => true,
            _ => false,
        };
    }

    fn gt(&self, other: &Self): bool {
        return self.partial_cmp(other) is Some(Ordering.Greater);
    }

    fn ge(&self, other: &Self): bool {
        return match self.partial_cmp(other) {
            Some(Ordering.Greater) | Some(Ordering.Equal) => true,
            _ => false,
        };
    }
}
```

---

## 4. Supertraits

### 4.1 Single Supertrait

```spl
// Eq requires PartialEq
trait Eq: PartialEq { }

// Copy requires Clone
trait Copy: Clone { }

// Ord requires Eq and PartialOrd
trait Ord: Eq + PartialOrd {
    fn cmp(&self, other: &Self): Ordering;
}
```

### 4.2 Supertrait Methods

Implementing a trait with supertraits requires implementing all supertraits first:

```spl
struct Point(x: i32, y: i32)

// Must implement PartialEq before Eq
impl PartialEq for Point {
    fn eq(&self, other: &Self): bool {
        return self.x == other.x && self.y == other.y;
    }
}

// Now can implement Eq
impl Eq for Point { }
```

### 4.3 Using Supertrait Methods

```spl
fn needs_ord(a: &T, b: &T): Ordering where T: Ord {
    // Can use Ord methods
    let cmp = a.cmp(b);

    // Can also use PartialOrd methods (supertrait)
    let less = a.lt(b);

    // Can also use PartialEq methods (supertrait of PartialOrd)
    let equal = a.eq(b);

    return cmp;
}
```

---

## 5. Marker Traits

Marker traits have no methods but signal type properties:

### 5.1 Copy

```spl
// Copy indicates bitwise copy semantics
trait Copy: Clone { }

// Types that implement Copy are copied, not moved
#[derive(Clone, Copy)]
struct Point(x: i32, y: i32)

let a = Point(x: 1, y: 2);
let b = a;  // Copy, a is still valid
use(a);     // OK
```

### 5.2 Sized

```spl
// Sized indicates the type has a known size at compile time
// Most types are Sized by default
trait Sized { }

// Use ?Sized for potentially unsized types
fn print_it(value: &T) where T: ?Sized + Debug {
    println("{:?}", value);
}

print_it(&42);       // &i32
print_it("hello");   // &str (unsized)
```

### 5.3 Send and Sync

```spl
// Send: safe to send between threads
trait Send { }

// Sync: safe to share references between threads
trait Sync { }

// Most types implement Send and Sync automatically
// Types with interior mutability may not be Sync
// Types with thread-local state may not be Send
```

### 5.4 Unpin

```spl
// Unpin: safe to move after pinning
trait Unpin { }

// Most types are Unpin by default
// Self-referential types are !Unpin
```

---

## 6. Negative Trait Bounds

### 6.1 Opt-Out of Auto Traits

```spl
// Rc is not Send or Sync
struct Rc(T) where T {
    // ...
}

impl !Send for Rc(T: T) where T { }
impl !Sync for Rc(T: T) where T { }
```

### 6.2 Negative Bounds in Where Clauses

```spl
// This function only accepts non-Copy types
fn takes_move_only(value: T) where T: !Copy {
    // value is moved
}
```

---

## 7. Trait Objects (Dynamic Dispatch)

### 7.1 Creating Trait Objects

```spl
trait Draw {
    fn draw(&self): ();
}

struct Circle(radius: f64)
struct Square(side: f64)

impl Draw for Circle {
    fn draw(&self): () { /* ... */ }
}

impl Draw for Square {
    fn draw(&self): () { /* ... */ }
}

// Trait object: &Draw
fn draw_shape(shape: &Draw): () {
    shape.draw();  // Dynamic dispatch
}

let c = Circle(radius: 5.0);
let s = Square(side: 10.0);

draw_shape(&c);
draw_shape(&s);
```

### 7.2 Boxed Trait Objects

```spl
// Store heterogeneous types
let shapes: Vec(T: Box(Draw)) = [
    Box.new(Circle(radius: 5.0)),
    Box.new(Square(side: 10.0)),
];

for shape in shapes.iter() {
    shape.draw();
}
```

### 7.3 Object Safety

Not all traits can be used as trait objects. A trait is **object-safe** if:

1. All methods have `self`, `&self`, or `&mut self` receiver
2. No methods use `Self` in return position
3. No associated functions (methods without `self`)
4. No generic methods

```spl
// Object-safe
trait Draw {
    fn draw(&self): ();
}

// NOT object-safe (returns Self)
trait Clone {
    fn clone(&self): Self;
}

// NOT object-safe (no self parameter)
trait Default {
    fn default(): Self;
}

// NOT object-safe (generic method)
trait Convert {
    fn convert(self): T where T;
}
```

### 7.4 Working Around Object Safety

```spl
// Clone is not object-safe, but we can make a clonable trait object
trait CloneBox {
    fn clone_box(&self): Box(CloneBox);
}

impl CloneBox for T where T: Clone + 'static {
    fn clone_box(&self): Box(CloneBox) {
        return Box.new(self.clone());
    }
}
```

---

## 8. Orphan Rules and Coherence

### 8.1 The Orphan Rule

You can implement a trait for a type only if:
- You defined the trait, OR
- You defined the type

```spl
// Your crate defines MyTrait
trait MyTrait {
    fn do_thing(&self): ();
}

// OK: implementing your trait for external type
impl MyTrait for String {
    fn do_thing(&self): () { }
}

// OK: implementing external trait for your type
struct MyType(value: i32)

impl Debug for MyType {
    fn fmt(&self, f: &mut Formatter): Result(T: (), E: Error) {
        return write(f, "MyType({})", self.value);
    }
}

// ERROR: implementing external trait for external type
// impl Debug for String { }  // Not allowed!
```

### 8.2 Newtype Pattern

To implement external traits for external types, use a wrapper:

```spl
// Wrapper around Vec
struct MyVec(inner: Vec(T: T)) where T

// Now you can implement any trait for MyVec
impl MyTrait for MyVec(T: T) where T {
    fn do_thing(&self): () { }
}
```

### 8.3 Coherence

The compiler ensures at most one implementation of a trait for any given type:

```spl
impl Debug for MyType { }
impl Debug for MyType { }  // ERROR: conflicting implementations
```

---

## 9. Extension Traits

### 9.1 Adding Methods to Existing Types

```spl
// Extension trait for &str
trait StrExt {
    fn is_blank(&self): bool;
    fn word_count(&self): usize;
}

impl StrExt for str {
    fn is_blank(&self): bool {
        return self.trim().is_empty();
    }

    fn word_count(&self): usize {
        return self.split_whitespace().count();
    }
}

// Usage
let s = "hello world";
let words = s.word_count();  // 2
```

### 9.2 Extension Traits for Generic Types

```spl
trait IteratorExt: Iterator {
    fn intersperse(self, sep: Self.Item): Intersperse(I: Self) where Self.Item: Clone;
}

impl IteratorExt for I where I: Iterator {
    fn intersperse(self, sep: Self.Item): Intersperse(I: Self) where Self.Item: Clone {
        return Intersperse(iter: self, sep: sep);
    }
}
```

---

## 10. Derive Macros

### 10.1 Derivable Traits

Standard traits that can be derived:

```spl
#[derive(Clone)]      // Implement Clone by cloning each field
#[derive(Copy)]       // Implement Copy (requires Clone)
#[derive(Debug)]      // Implement Debug for formatting
#[derive(Default)]    // Implement Default with default values
#[derive(PartialEq)]  // Implement PartialEq by comparing fields
#[derive(Eq)]         // Implement Eq (requires PartialEq)
#[derive(PartialOrd)] // Implement PartialOrd by comparing fields
#[derive(Ord)]        // Implement Ord (requires PartialOrd + Eq)
#[derive(Hash)]       // Implement Hash by hashing each field
```

### 10.2 Derive Requirements

```spl
// All fields must implement the trait being derived
#[derive(Clone, Debug)]
struct Wrapper(
    data: Vec(T: i32),  // Vec implements Clone and Debug
    name: String,       // String implements Clone and Debug
)

// ERROR if a field doesn't implement the trait
#[derive(Copy)]  // ERROR: String doesn't implement Copy
struct BadCopy(name: String)
```

### 10.3 Custom Derive (Procedural Macros)

Custom derives are implemented as procedural macros:

```spl
#[derive(Serialize, Deserialize)]  // From serde
struct Config(
    name: String,
    value: i32,
)
```

---

## 11. Index Traits

The `Index` and `IndexMut` traits enable the `collection[i]` subscript syntax.

### 11.1 Index Trait

```spl
trait Index(Idx) where Idx {
    type Output;

    /// Returns a reference to the element at `idx`.
    /// Panics if `idx` is out of bounds.
    fn index(&self, idx: Idx): &Self.Output;
}
```

The `index` method returns `&Self.Output` and is legal because there is an input reference (`&self`) for the output to borrow from.

### 11.2 IndexMut Trait

```spl
trait IndexMut(Idx): Index(Idx) where Idx {
    /// Returns a mutable reference to the element at `idx`.
    /// Panics if `idx` is out of bounds.
    fn index_mut(&mut self, idx: Idx): &mut Self.Output;
}
```

### 11.3 Desugaring

The subscript syntax desugars to method calls:

| Syntax | Desugaring |
|--------|------------|
| `collection[i]` (value context) | `*collection.index(i)` |
| `&collection[i]` | `collection.index(i)` |
| `&mut collection[i]` | `collection.index_mut(i)` |
| `collection[i] = v` | `*collection.index_mut(i) = v` |

### 11.4 Standard Implementations

```spl
impl Index(Idx: usize) for Vec(T: T) where T {
    type Output = T;

    fn index(&self, idx: usize): &T {
        if idx >= self.len() {
            panic("index out of bounds");
        }
        // Return reference to internal storage
    }
}

impl IndexMut(Idx: usize) for Vec(T: T) where T {
    fn index_mut(&mut self, idx: usize): &mut T {
        if idx >= self.len() {
            panic("index out of bounds");
        }
        // Return mutable reference to internal storage
    }
}

impl Index(Idx: usize) for [T; N] where T {
    type Output = T;

    fn index(&self, idx: usize): &T {
        if idx >= N {
            panic("index out of bounds");
        }
        // Return reference to element
    }
}

impl IndexMut(Idx: usize) for [T; N] where T {
    fn index_mut(&mut self, idx: usize): &mut T {
        if idx >= N {
            panic("index out of bounds");
        }
        // Return mutable reference to element
    }
}
```

### 11.5 Range Indexing

Collections can also implement `Index` for range types to support slicing:

```spl
impl Index(Idx: Range(T: usize)) for Vec(T: T) where T {
    type Output = [T];

    fn index(&self, idx: Range(T: usize)): &[T] {
        // Return slice reference
    }
}

// Usage
let vec: Vec(T: i32) = [1, 2, 3, 4, 5];
let slice: &[i32] = &vec[1..4];  // [2, 3, 4]
```

---

## 12. RefIterator Trait

The `RefIterator` trait enables reference iteration over non-indexed collections (HashMap, LinkedList, Tree, etc.) using scoped types.

### 12.1 Trait Definition

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

### 12.2 How It Works with Scoped Types

Unlike `Iterator` (which yields owned values) or `IndexIterator` (which uses random access), `RefIterator` implementations are **scoped types** that can hold references to their source collection:

```spl
#[scoped]
struct HashMapIter(
    source: &HashMap(K, V),  // Allowed: struct is scoped
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
}
```

### 12.3 For-Loop Desugaring

```spl
// Source
for (k, v) in &hashmap {
    process(k, v);
}

// Desugars to
{
    let mut __iter = hashmap.ref_iter();
    while __iter.next() is Some((k, v)) {
        process(k, v);
    }
}
```

### 12.4 Standard Adapters

Adapters are also scoped types that implement `RefIterator`:

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
struct Take(inner: I, remaining: usize) where I: RefIterator

#[scoped]
struct Skip(inner: I, remaining: usize) where I: RefIterator

#[scoped]
struct Map(inner: I, transform: fn(&I.Item): U) where I: RefIterator, U
```

### 12.5 Chaining Example

```spl
let map: HashMap(K: String, V: i32) = /* ... */;

// Full chaining support
let result: Vec(T: i32) = map.ref_iter()
    .filter(|kv| kv.1 > 10)
    .take(5)
    .map(|kv| kv.1.clone())
    .collect();
```

### 12.6 Terminal Operations

```spl
impl RefIterator {
    fn collect(&mut self): Vec(T: Self.Item) where Self.Item: Clone;
    fn count(&mut self): usize;
    fn find(&mut self, pred: fn(&Self.Item): bool): Option(Self.Item) where Self.Item: Clone;
    fn for_each(&mut self, f: fn(&Self.Item));
    fn any(&mut self, pred: fn(&Self.Item): bool): bool;
    fn all(&mut self, pred: fn(&Self.Item): bool): bool;
}
```

### 12.7 RefIteratorMut for Mutable Iteration

```spl
trait RefIteratorMut {
    type Item;

    fn next(&mut self): Option(&mut Self.Item);
}

// Usage
for (k, v) in &mut hashmap {
    *v += 1;
}
```

### 12.8 Comparison with Other Traits

| Trait | Yields | Storage | Use Case |
|-------|--------|---------|----------|
| `IndexIterator` | `&T` / `&mut T` | N/A (indexing) | Random-access: Vec, arrays |
| `RefIterator` | `&T` | Scoped types | Sequential: HashMap, LinkedList |
| `Iterator` | `T` (owned) | Regular structs | Generators, ranges, consuming |

See [iteration.md](iteration.md) for complete iteration documentation.

---

## 13. Trait Aliases (Future)

Trait aliases for combining common bounds:

```spl
// Trait alias (proposed syntax)
trait Debug + Clone + Send = DebugCloneSend;

// Instead of:
fn process(x: T) where T: Debug + Clone + Send { }

// Could write:
fn process(x: T) where T: DebugCloneSend { }
```

---

## 14. Summary

| Feature | Syntax | Description |
|---------|--------|-------------|
| Trait definition | `trait Foo { ... }` | Define a trait |
| Implementation | `impl Foo for Bar { ... }` | Implement trait for type |
| Supertrait | `trait Foo: Bar { ... }` | Foo requires Bar |
| Associated type | `type Item;` | Type defined by implementor |
| Associated const | `const MAX: Self;` | Constant defined by implementor |
| Type parameter | `trait Foo(T) where T` | Generic trait |
| Default method | `fn foo(&self) { ... }` | Method with default body |
| Trait object | `&Foo` | Dynamic dispatch |
| Marker trait | `trait Copy: Clone { }` | No methods, signals property |
| Blanket impl | `impl Foo for T where T: Bar` | Implement for all matching types |
| Negative impl | `impl !Send for Foo` | Opt out of auto trait |
| RefIterator | `trait RefIterator { ... }` | Reference iteration via scoped types |

---

## References

- [type-system.md](type-system.md) - Generics and bounds
- [syntax-grammar.md](syntax-grammar.md) - Trait syntax
- [standard-library.md](standard-library.md) - Standard traits
- [attributes.md](attributes.md) - Derive macros and `#[scoped]` attribute
- [iteration.md](iteration.md) - Complete iteration documentation
- [memory-model.md](memory-model.md) - Scoped types and second-class references
- [ADR-015: Scoped Types and RefIterator](../designs/015-scoped-types-refiterator.md) - Design rationale
