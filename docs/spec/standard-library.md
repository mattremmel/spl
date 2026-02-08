# Standard Library

This document specifies SPL's standard library organization and core types.

## Overview

The standard library is organized into modules under `std`:

```
std
├── collections     # Data structures
├── io              # Input/output
├── fs              # Filesystem
├── net             # Networking
├── task            # Concurrency primitives
├── channel         # Channel types
├── sync            # Synchronization primitives (Arc, Mutex, RwLock)
├── rc              # Reference counting (Rc)
├── cell            # Interior mutability (Cell, RefCell, UnsafeCell)
├── time            # Time and duration
├── fmt             # Formatting
├── str             # String utilities
├── iter            # Iterator utilities
├── mem             # Memory utilities
├── ptr             # Pointer utilities
├── ops             # Operator traits
├── cmp             # Comparison traits
├── convert         # Conversion traits
├── hash            # Hashing
├── env             # Environment access
├── process         # Process management
├── panic           # Panic handling
└── prelude         # Auto-imported items
```

---

## 1. Prelude

Items automatically imported into every module:

```spl
// Types
Option, Result, String, Vec, Box, decimal

// Enum variants
Some, None, Ok, Err

// Traits
Clone, Copy, Default, Debug, Display
PartialEq, Eq, PartialOrd, Ord
Hash
Iterator, IntoIterator
From, Into, TryFrom, TryInto
Drop

// Other
Try, FromResidual       // Error propagation
ControlFlow             // Control flow enum
```

---

## 2. Core Types

### 2.1 Option

Represents an optional value — either `Some(T)` containing a value, or `None` representing absence.

```spl
enum Option{ Some(T), None } where T
```

#### Core Methods

```spl
impl Option(T: T) where T {
    /// Panics if None. Prefer `unwrap_or`, `unwrap_or_else`, or pattern matching.
    fn unwrap(self): T {
        match self {
            Some(v) => v,
            None => panic("called unwrap() on None"),
        }
    }

    /// Returns the contained value or a default.
    fn unwrap_or(self, default: T): T {
        match self {
            Some(v) => v,
            None => default,
        }
    }

    /// Returns the contained value or computes it from a closure.
    fn unwrap_or_else(self, f: fn(): T): T {
        match self {
            Some(v) => v,
            None => f(),
        }
    }

    /// Panics with a custom message if None.
    fn expect(self, msg: &str): T {
        match self {
            Some(v) => v,
            None => panic(msg),
        }
    }
}
```

#### Query Methods

```spl
impl Option(T: T) where T {
    fn is_some(&self): bool {
        match self { Some(_) => true, None => false }
    }

    fn is_none(&self): bool {
        return !self.is_some();
    }
}
```

#### Transform Methods

```spl
impl Option(T: T) where T {
    /// Maps an `Option(T)` to `Option(U)` by applying `f` to the contained value.
    fn map(self, f: fn(T): U): Option(T: U) where U {
        match self {
            Some(v) => Some(f(v)),
            None => None,
        }
    }

    /// Returns None if self is None, otherwise calls `f` with the value and returns the result.
    fn and_then(self, f: fn(T): Option(T: U)): Option(T: U) where U {
        match self {
            Some(v) => f(v),
            None => None,
        }
    }

    /// Returns None if self is None, otherwise returns `other`.
    fn and(self, other: Option(T: U)): Option(T: U) where U {
        match self {
            Some(_) => other,
            None => None,
        }
    }

    /// Returns self if it contains a value, otherwise returns `other`.
    fn or(self, other: Option(T: T)): Option(T: T) {
        match self {
            Some(v) => Some(v),
            None => other,
        }
    }

    /// Returns self if it contains a value, otherwise calls `f` and returns the result.
    fn or_else(self, f: fn(): Option(T: T)): Option(T: T) {
        match self {
            Some(v) => Some(v),
            None => f(),
        }
    }

    /// Returns None if the option is None, otherwise calls `predicate` and
    /// returns Some(v) if the predicate returns true, else None.
    fn filter(self, predicate: fn(&T): bool): Option(T: T) {
        match self {
            Some(v) if predicate(&v) => Some(v),
            _ => None,
        }
    }

}

impl Option(T: Option(T: U)) where U {
    /// Converts `Option(T: Option(T: U))` to `Option(T: U)`.
    fn flatten(self): Option(T: U) {
        match self {
            Some(inner) => inner,
            None => None,
        }
    }
}
```

#### Conversion Methods

```spl
impl Option(T: T) where T {
    /// Converts Option to Result with an explicit error value.
    fn ok_or(self, err: E): Result(T: T, E: E) where E {
        match self {
            Some(v) => Ok(v),
            None => Err(err),
        }
    }

    /// Converts Option to Result with a lazy error value.
    fn ok_or_else(self, f: fn(): E): Result(T: T, E: E) where E {
        match self {
            Some(v) => Ok(v),
            None => Err(f()),
        }
    }

    /// Converts &Option(T) to Option(&T).
    fn as_ref(&self): Option(T: &T) {
        match self {
            Some(v) => Some(v),
            None => None,
        }
    }

    /// Converts &mut Option(T) to Option(&mut T).
    fn as_mut(&mut self): Option(T: &mut T) {
        match self {
            Some(v) => Some(v),
            None => None,
        }
    }

    /// Takes the value out, leaving None in its place.
    fn take(&mut self): Option(T: T);

    /// Replaces the value with Some(value), returning the old value.
    fn replace(&mut self, value: T): Option(T: T);

    /// Zips self with another Option.
    fn zip(self, other: Option(T: U)): Option(T: (T, U)) where U {
        match (self, other) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }
}
```

#### Optional Chaining (`?.`)

The `?.` operator provides safe navigation through optional values. See [error-handling.md](error-handling.md) section 3 for full details.

```spl
// user?.email desugars to:
match user {
    Some(u) => Some(u.email),
    None => None,
}
```

### 2.2 Result

Represents the outcome of an operation that may fail — either `Ok(T)` on success, or `Err(E)` on failure.

```spl
enum Result{ Ok(T), Err(E) } where T, E
```

#### Core Methods

```spl
impl Result(T: T, E: E) where T, E {
    /// Returns the success value. Panics if Err.
    fn unwrap(self): T {
        match self {
            Ok(v) => v,
            Err(e) => panic("called unwrap() on Err"),
        }
    }

    /// Returns the error value. Panics if Ok.
    fn unwrap_err(self): E {
        match self {
            Ok(_) => panic("called unwrap_err() on Ok"),
            Err(e) => e,
        }
    }

    /// Returns the success value or a default.
    fn unwrap_or(self, default: T): T {
        match self { Ok(v) => v, Err(_) => default }
    }

    /// Returns the success value or computes it from the error.
    fn unwrap_or_else(self, f: fn(E): T): T {
        match self { Ok(v) => v, Err(e) => f(e) }
    }

    /// Panics with a custom message if Err.
    fn expect(self, msg: &str): T {
        match self {
            Ok(v) => v,
            Err(_) => panic(msg),
        }
    }

    /// Panics with a custom message if Ok.
    fn expect_err(self, msg: &str): E {
        match self {
            Ok(_) => panic(msg),
            Err(e) => e,
        }
    }
}
```

#### Query Methods

```spl
impl Result(T: T, E: E) where T, E {
    fn is_ok(&self): bool {
        match self { Ok(_) => true, Err(_) => false }
    }

    fn is_err(&self): bool {
        return !self.is_ok();
    }
}
```

#### Transform Methods

```spl
impl Result(T: T, E: E) where T, E {
    /// Maps the Ok value, leaving Err untouched.
    fn map(self, f: fn(T): U): Result(T: U, E: E) where U {
        match self {
            Ok(v) => Ok(f(v)),
            Err(e) => Err(e),
        }
    }

    /// Maps the Err value, leaving Ok untouched.
    fn map_err(self, f: fn(E): F): Result(T: T, E: F) where F {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(f(e)),
        }
    }

    /// Chains a fallible operation on the Ok value.
    fn and_then(self, f: fn(T): Result(T: U, E: E)): Result(T: U, E: E) where U {
        match self {
            Ok(v) => f(v),
            Err(e) => Err(e),
        }
    }

    /// Returns self if Ok, otherwise calls `f` with the error.
    fn or_else(self, f: fn(E): Result(T: T, E: F)): Result(T: T, E: F) where F {
        match self {
            Ok(v) => Ok(v),
            Err(e) => f(e),
        }
    }
}
```

#### Conversion Methods

```spl
impl Result(T: T, E: E) where T, E {
    /// Converts to Option(T), discarding the error.
    fn ok(self): Option(T: T) {
        match self { Ok(v) => Some(v), Err(_) => None }
    }

    /// Converts to Option(E), discarding the success value.
    fn err(self): Option(T: E) {
        match self { Ok(_) => None, Err(e) => Some(e) }
    }

    /// Converts &Result(T, E) to Result(&T, &E).
    fn as_ref(&self): Result(T: &T, E: &E);

    /// Converts &mut Result(T, E) to Result(&mut T, &mut E).
    fn as_mut(&mut self): Result(T: &mut T, E: &mut E);
}
```

#### Try Operator (`!`)

The `!` postfix operator provides concise error propagation. See [error-handling.md](error-handling.md) section 2 for full details and the `Try`/`FromResidual` traits.

```spl
// value! desugars to:
match Try.branch(value) {
    Continue(v) => v,
    Break(r) => return FromResidual.from_residual(r),
}
```

### 2.3 String

```spl
struct String { /* ... */ }

impl String {
    fn new(): String;
    fn with_capacity(capacity: usize): String;
    fn from_utf8(bytes: Vec(T: u8)): Result(T: String, E: Utf8Error);
    fn from_utf8_lossy(bytes: &[u8]): String;
    fn len(&self): usize;
    fn is_empty(&self): bool;
    fn capacity(&self): usize;
    fn push(&mut self, c: char): ();
    fn push_str(&mut self, s: &str): ();
    fn pop(&mut self): Option(T: char);
    fn clear(&mut self): ();
    fn truncate(&mut self, new_len: usize): ();
    fn as_str(&self): &str;
    fn as_bytes(&self): &[u8];
    fn chars(&self): Iterator(Item: char);
    fn bytes(&self): Iterator(Item: u8);
    fn char_indices(&self): Iterator(Item: (usize, char));
    fn lines(&self): Iterator(Item: &str);
    fn split(&self, pattern: &str): Iterator(Item: &str);
    fn trim(&self): &str;
    fn trim_start(&self): &str;
    fn trim_end(&self): &str;
    fn contains(&self, pattern: &str): bool;
    fn starts_with(&self, prefix: &str): bool;
    fn ends_with(&self, suffix: &str): bool;
    fn replace(&self, from: &str, to: &str): String;
    fn to_lowercase(&self): String;
    fn to_uppercase(&self): String;
}
```

### 2.4 Vec

```spl
struct Vec(T) where T { /* ... */ }

impl Vec(T: T) where T {
    fn new(): Vec(T: T);
    fn with_capacity(capacity: usize): Vec(T: T);
    fn len(&self): usize;
    fn is_empty(&self): bool;
    fn capacity(&self): usize;
    fn push(&mut self, value: T): ();
    fn pop(&mut self): Option(T: T);
    fn insert(&mut self, index: usize, value: T): ();
    fn remove(&mut self, index: usize): T;
    fn clear(&mut self): ();
    fn truncate(&mut self, len: usize): ();
    fn resize(&mut self, new_len: usize, value: T): () where T: Clone;
    fn reserve(&mut self, additional: usize): ();
    fn shrink_to_fit(&mut self): ();
    fn get(&self, index: usize): Option(T: &T);
    fn get_mut(&mut self, index: usize): Option(T: &mut T);
    fn first(&self): Option(T: &T);
    fn last(&self): Option(T: &T);
    fn as_slice(&self): &[T];
    fn as_mut_slice(&mut self): &mut [T];
    fn iter(&self): Iterator(Item: &T);
    fn iter_mut(&mut self): Iterator(Item: &mut T);
    fn sort(&mut self): () where T: Ord;
    fn sort_by(&mut self, compare: fn(&T, &T): Ordering): ();
    fn reverse(&mut self): ();
    fn contains(&self, value: &T): bool where T: PartialEq;
    fn dedup(&mut self): () where T: PartialEq;
}
```

### 2.5 Box

```spl
struct Box(T) where T { /* ... */ }

impl Box(T: T) where T {
    fn new(value: T): Box(T: T);
    fn into_inner(self): T;
}
```

### 2.6 Hash Trait

The `Hash` trait is defined in `std.hash`. Types used as `HashMap` or `HashSet` keys must implement `Hash`.

```spl
// std.hash

/// A trait for types that can be hashed.
trait Hash {
    /// Feeds this value into the given hasher.
    fn hash(&self, hasher: &mut Hasher): ();
}

/// A trait for hash functions.
trait Hasher {
    /// Write a single byte into the hasher.
    fn write(&mut self, bytes: &[u8]): ();

    /// Return the hash value computed so far.
    fn finish(&self): u64;
}
```

`Hash` can be derived for structs and enums where all fields implement `Hash`:

```spl
#[derive(Hash)]
struct Point(x: i32, y: i32)
```

**Built-in implementations:** All primitive integer types, `bool`, `char`, `str`, `String`, `decimal`, tuples (element-wise), arrays (element-wise), `Option(T)` where `T: Hash`, `Result(T, E)` where `T: Hash, E: Hash`.

**Note:** `f32` and `f64` do **not** implement `Hash` because NaN values would violate the contract that `a == b` implies `hash(a) == hash(b)`.

### 2.7 Default Trait

The `Default` trait provides a way to create a default value for a type.

```spl
trait Default {
    /// Returns the default value for this type.
    fn default(): Self;
}
```

`Default` can be derived for structs where all fields implement `Default`:

```spl
#[derive(Default)]
struct Config(
    width: i32,     // defaults to 0
    height: i32,    // defaults to 0
    name: String,   // defaults to ""
)

let config = Config.default();
```

**Built-in implementations:** All numeric types default to `0` (or `0.0` for floats, `0` for `decimal`). `bool` defaults to `false`. `String` defaults to `""`. `Vec(T)` defaults to an empty vector. `Option(T)` defaults to `None`.

### 2.8 Arc and Rc

#### Arc (Atomically Reference Counted)

`Arc` provides shared ownership of a heap-allocated value with atomic reference counting, suitable for concurrent access across tasks.

```spl
// std.sync

struct Arc(T) where T { /* ... */ }

impl Arc(T: T) where T {
    /// Creates a new Arc wrapping the given value.
    fn new(value: T): Arc(T: T);

    /// Creates a new Arc that shares ownership (increments reference count).
    fn clone(&self): Arc(T: T);

    /// Returns the number of strong references to this allocation.
    fn strong_count(&self): usize;

    /// Returns the number of weak references to this allocation.
    fn weak_count(&self): usize;
}
```

**Thread safety:** `Arc(T)` implements `Send + Sync` when `T: Send + Sync`. This makes it the standard way to share data across tasks.

#### Rc (Reference Counted)

`Rc` provides shared ownership of a heap-allocated value with non-atomic reference counting, suitable for single-threaded use only.

```spl
// std.rc

struct Rc(T) where T { /* ... */ }

impl Rc(T: T) where T {
    /// Creates a new Rc wrapping the given value.
    fn new(value: T): Rc(T: T);

    /// Creates a new Rc that shares ownership (increments reference count).
    fn clone(&self): Rc(T: T);

    /// Returns the number of strong references to this allocation.
    fn strong_count(&self): usize;
}
```

**Thread safety:** `Rc` is neither `Send` nor `Sync`. Use `Arc` for cross-task sharing. `Rc` has lower overhead than `Arc` due to non-atomic operations.

### 2.9 Cell, RefCell, and UnsafeCell

These types provide interior mutability -- the ability to mutate data even through shared references.

#### Cell

`Cell` provides interior mutability for `Copy` types by moving values in and out.

```spl
// std.cell

struct Cell(T) where T: Copy { /* ... */ }

impl Cell(T: T) where T: Copy {
    /// Creates a new Cell containing the given value.
    fn new(value: T): Cell(T: T);

    /// Returns a copy of the contained value.
    fn get(&self): T;

    /// Sets the contained value.
    fn set(&self, value: T): ();

    /// Replaces the contained value, returning the old value.
    fn replace(&self, value: T): T;
}
```

#### RefCell

`RefCell` provides interior mutability for any type via runtime borrow checking.

```spl
struct RefCell(T) where T { /* ... */ }

impl RefCell(T: T) where T {
    /// Creates a new RefCell containing the given value.
    fn new(value: T): RefCell(T: T);

    /// Immutably borrows the wrapped value. Panics if already mutably borrowed.
    fn borrow(&self): Ref(T: T);

    /// Mutably borrows the wrapped value. Panics if already borrowed.
    fn borrow_mut(&self): RefMut(T: T);

    /// Attempts to immutably borrow. Returns None if already mutably borrowed.
    fn try_borrow(&self): Option(T: Ref(T: T));

    /// Attempts to mutably borrow. Returns None if already borrowed.
    fn try_borrow_mut(&self): Option(T: RefMut(T: T));
}
```

`Ref(T)` and `RefMut(T)` are scoped guard types that implement `Deref` and `DerefMut` respectively, releasing the borrow when dropped.

#### UnsafeCell

`UnsafeCell` is the fundamental building block for all interior mutability in SPL. It is the only way to obtain a mutable pointer from a shared reference.

```spl
struct UnsafeCell(T) where T { /* ... */ }

impl UnsafeCell(T: T) where T {
    /// Creates a new UnsafeCell containing the given value.
    fn new(value: T): UnsafeCell(T: T);

    /// Returns a raw mutable pointer to the contained value.
    fn get(&self): *mut T;
}
```

**Note:** `UnsafeCell` is only usable in `unsafe` blocks. It is the primitive upon which `Cell`, `RefCell`, `Mutex`, and `RwLock` are built.

### 2.10 Memory Utilities

```spl
// std.mem

/// Returns the size of type T in bytes.
fn size_of(T)(): usize where T;

/// Returns the alignment of type T in bytes.
fn align_of(T)(): usize where T;

/// Swaps the values at two mutable references.
fn swap(T)(a: &mut T, b: &mut T): () where T;

/// Replaces the value at a mutable reference, returning the old value.
fn replace(T)(dest: &mut T, src: T): T where T;

/// Takes the value out of a mutable reference, replacing it with Default.
fn take(T)(dest: &mut T): T where T: Default;

/// Forgets a value without running its destructor.
fn forget(T)(val: T): () where T;
```

---

## 3. Collections

### 3.1 HashMap

```spl
struct HashMap(K, V) where K: Hash + Eq, V { /* ... */ }

impl HashMap(K: K, V: V) where K: Hash + Eq, V {
    /// Creates an empty HashMap.
    fn new(): HashMap(K: K, V: V);
    /// Creates an empty HashMap with the specified capacity.
    fn with_capacity(capacity: usize): HashMap(K: K, V: V);
    /// Inserts a key-value pair. Returns the previous value if the key was present.
    fn insert(&mut self, key: K, value: V): Option(T: V);
    /// Returns a reference to the value for the given key.
    fn get(&self, key: &K): Option(T: &V);
    /// Returns a mutable reference to the value for the given key.
    fn get_mut(&mut self, key: &K): Option(T: &mut V);
    /// Removes a key-value pair, returning the value if present.
    fn remove(&mut self, key: &K): Option(T: V);
    /// Returns true if the map contains the given key.
    fn contains_key(&self, key: &K): bool;
    /// Returns the number of key-value pairs.
    fn len(&self): usize;
    /// Returns true if the map is empty.
    fn is_empty(&self): bool;
    /// Removes all key-value pairs.
    fn clear(&mut self): ();
    /// Applies a function to each key-value pair.
    fn each(&self, f: fn(&K, &V)): ();
    /// Returns a RefIterator over key-value pairs.
    fn ref_iter(&self): HashMapIter(K: K, V: V);
}
```

### 3.2 HashSet

```spl
struct HashSet(T) where T: Hash + Eq { /* ... */ }

impl HashSet(T: T) where T: Hash + Eq {
    /// Creates an empty HashSet.
    fn new(): HashSet(T: T);
    /// Creates an empty HashSet with the specified capacity.
    fn with_capacity(capacity: usize): HashSet(T: T);
    /// Inserts a value. Returns true if the value was not already present.
    fn insert(&mut self, value: T): bool;
    /// Removes a value. Returns true if the value was present.
    fn remove(&mut self, value: &T): bool;
    /// Returns true if the set contains the value.
    fn contains(&self, value: &T): bool;
    /// Returns the number of elements.
    fn len(&self): usize;
    /// Returns true if the set is empty.
    fn is_empty(&self): bool;
    /// Removes all elements.
    fn clear(&mut self): ();
    /// Applies a function to each element.
    fn each(&self, f: fn(&T)): ();
    /// Returns a RefIterator over elements.
    fn ref_iter(&self): HashSetIter(T: T);
    /// Returns the union of two sets.
    fn union(&self, other: &HashSet(T: T)): HashSet(T: T) where T: Clone;
    /// Returns the intersection of two sets.
    fn intersection(&self, other: &HashSet(T: T)): HashSet(T: T) where T: Clone;
    /// Returns the difference (elements in self but not in other).
    fn difference(&self, other: &HashSet(T: T)): HashSet(T: T) where T: Clone;
    /// Returns true if self is a subset of other.
    fn is_subset(&self, other: &HashSet(T: T)): bool;
}
```

### 3.3 BTreeMap

```spl
struct BTreeMap(K, V) where K: Ord, V { /* ... */ }

impl BTreeMap(K: K, V: V) where K: Ord, V {
    /// Creates an empty BTreeMap.
    fn new(): BTreeMap(K: K, V: V);
    /// Inserts a key-value pair. Returns the previous value if the key was present.
    fn insert(&mut self, key: K, value: V): Option(T: V);
    /// Returns a reference to the value for the given key.
    fn get(&self, key: &K): Option(T: &V);
    /// Returns a mutable reference to the value for the given key.
    fn get_mut(&mut self, key: &K): Option(T: &mut V);
    /// Removes a key-value pair, returning the value if present.
    fn remove(&mut self, key: &K): Option(T: V);
    /// Returns true if the map contains the given key.
    fn contains_key(&self, key: &K): bool;
    /// Returns the number of key-value pairs.
    fn len(&self): usize;
    /// Returns true if the map is empty.
    fn is_empty(&self): bool;
    /// Removes all key-value pairs.
    fn clear(&mut self): ();
    /// Applies a function to each key-value pair in sorted order.
    fn each(&self, f: fn(&K, &V)): ();
    /// Returns a RefIterator over key-value pairs in sorted order.
    fn ref_iter(&self): BTreeMapIter(K: K, V: V);
    /// Returns the first (smallest) key-value pair.
    fn first(&self): Option(T: (&K, &V));
    /// Returns the last (largest) key-value pair.
    fn last(&self): Option(T: (&K, &V));
}
```

### 3.4 BTreeSet

```spl
struct BTreeSet(T) where T: Ord { /* ... */ }

impl BTreeSet(T: T) where T: Ord {
    /// Creates an empty BTreeSet.
    fn new(): BTreeSet(T: T);
    /// Inserts a value. Returns true if the value was not already present.
    fn insert(&mut self, value: T): bool;
    /// Removes a value. Returns true if the value was present.
    fn remove(&mut self, value: &T): bool;
    /// Returns true if the set contains the value.
    fn contains(&self, value: &T): bool;
    /// Returns the number of elements.
    fn len(&self): usize;
    /// Returns true if the set is empty.
    fn is_empty(&self): bool;
    /// Removes all elements.
    fn clear(&mut self): ();
    /// Applies a function to each element in sorted order.
    fn each(&self, f: fn(&T)): ();
    /// Returns a RefIterator over elements in sorted order.
    fn ref_iter(&self): BTreeSetIter(T: T);
    /// Returns the first (smallest) element.
    fn first(&self): Option(T: &T);
    /// Returns the last (largest) element.
    fn last(&self): Option(T: &T);
    /// Returns true if self is a subset of other.
    fn is_subset(&self, other: &BTreeSet(T: T)): bool;
}
```

### 3.5 VecDeque

A double-ended queue implemented with a growable ring buffer.

```spl
struct VecDeque(T) where T { /* ... */ }

impl VecDeque(T: T) where T {
    /// Creates an empty VecDeque.
    fn new(): VecDeque(T: T);
    /// Creates an empty VecDeque with the specified capacity.
    fn with_capacity(capacity: usize): VecDeque(T: T);
    /// Appends an element to the back.
    fn push_back(&mut self, value: T): ();
    /// Prepends an element to the front.
    fn push_front(&mut self, value: T): ();
    /// Removes and returns the last element.
    fn pop_back(&mut self): Option(T: T);
    /// Removes and returns the first element.
    fn pop_front(&mut self): Option(T: T);
    /// Returns a reference to the element at the given index.
    fn get(&self, index: usize): Option(T: &T);
    /// Returns a mutable reference to the element at the given index.
    fn get_mut(&mut self, index: usize): Option(T: &mut T);
    /// Returns a reference to the front element.
    fn front(&self): Option(T: &T);
    /// Returns a reference to the back element.
    fn back(&self): Option(T: &T);
    /// Returns the number of elements.
    fn len(&self): usize;
    /// Returns true if the deque is empty.
    fn is_empty(&self): bool;
    /// Removes all elements.
    fn clear(&mut self): ();
    /// Returns true if the deque contains the value.
    fn contains(&self, value: &T): bool where T: PartialEq;
    /// Returns an iterator over elements.
    fn iter(&self): Iterator(Item: &T);
}
```

### 3.6 LinkedList

A doubly-linked list.

```spl
struct LinkedList(T) where T { /* ... */ }

impl LinkedList(T: T) where T {
    /// Creates an empty LinkedList.
    fn new(): LinkedList(T: T);
    /// Appends an element to the back.
    fn push_back(&mut self, value: T): ();
    /// Prepends an element to the front.
    fn push_front(&mut self, value: T): ();
    /// Removes and returns the last element.
    fn pop_back(&mut self): Option(T: T);
    /// Removes and returns the first element.
    fn pop_front(&mut self): Option(T: T);
    /// Returns a reference to the front element.
    fn front(&self): Option(T: &T);
    /// Returns a reference to the back element.
    fn back(&self): Option(T: &T);
    /// Returns the number of elements.
    fn len(&self): usize;
    /// Returns true if the list is empty.
    fn is_empty(&self): bool;
    /// Removes all elements.
    fn clear(&mut self): ();
    /// Returns true if the list contains the value.
    fn contains(&self, value: &T): bool where T: PartialEq;
    /// Applies a function to each element.
    fn each(&self, f: fn(&T)): ();
    /// Returns a RefIterator over elements.
    fn ref_iter(&self): LinkedListIter(T: T);
}
```

### 3.7 BinaryHeap

A priority queue implemented with a binary heap. Elements are ordered by `Ord`, with the greatest element at the top.

```spl
struct BinaryHeap(T) where T: Ord { /* ... */ }

impl BinaryHeap(T: T) where T: Ord {
    /// Creates an empty BinaryHeap.
    fn new(): BinaryHeap(T: T);
    /// Creates an empty BinaryHeap with the specified capacity.
    fn with_capacity(capacity: usize): BinaryHeap(T: T);
    /// Pushes a value onto the heap.
    fn push(&mut self, value: T): ();
    /// Removes and returns the greatest element. Returns None if empty.
    fn pop(&mut self): Option(T: T);
    /// Returns a reference to the greatest element without removing it.
    fn peek(&self): Option(T: &T);
    /// Returns the number of elements.
    fn len(&self): usize;
    /// Returns true if the heap is empty.
    fn is_empty(&self): bool;
    /// Removes all elements.
    fn clear(&mut self): ();
    /// Consumes the heap and returns elements as a sorted Vec.
    fn into_sorted_vec(self): Vec(T: T);
    /// Returns an iterator over elements in arbitrary order.
    fn iter(&self): Iterator(Item: &T);
}
```

---

## 4. I/O

### 4.1 Traits

```spl
trait Read {
    fn read(&mut self, buf: &mut [u8]): Result(T: usize, E: IoError);
}

trait Write {
    fn write(&mut self, buf: &[u8]): Result(T: usize, E: IoError);
    fn flush(&mut self): Result(T: (), E: IoError);
}

trait Seek {
    fn seek(&mut self, pos: SeekFrom): Result(T: u64, E: IoError);
}

trait BufRead: Read {
    fn read_line(&mut self, buf: &mut String): Result(T: usize, E: IoError);
    fn lines(&self): Iterator(Item: Result(T: String, E: IoError));
}
```

### 4.2 Types

```spl
struct Stdin { /* ... */ }
struct Stdout { /* ... */ }
struct Stderr { /* ... */ }
struct BufReader(R) where R: Read { /* ... */ }
struct BufWriter(W) where W: Write { /* ... */ }
```

---

## 5. Filesystem

```spl
// std.fs

fn read_to_string(path: &str): Result(T: String, E: IoError);
fn read(path: &str): Result(T: Vec(T: u8), E: IoError);
fn write(path: &str, contents: &[u8]): Result(T: (), E: IoError);
fn copy(from: &str, to: &str): Result(T: u64, E: IoError);
fn rename(from: &str, to: &str): Result(T: (), E: IoError);
fn remove_file(path: &str): Result(T: (), E: IoError);
fn create_dir(path: &str): Result(T: (), E: IoError);
fn create_dir_all(path: &str): Result(T: (), E: IoError);
fn remove_dir(path: &str): Result(T: (), E: IoError);
fn remove_dir_all(path: &str): Result(T: (), E: IoError);
fn read_dir(path: &str): Result(T: ReadDir, E: IoError);
fn metadata(path: &str): Result(T: Metadata, E: IoError);
fn exists(path: &str): bool;

struct File { /* ... */ }
struct Metadata { /* ... */ }
struct ReadDir { /* ... */ }
struct DirEntry { /* ... */ }
```

---

## 6. Networking

```spl
// std.net

struct TcpStream { /* ... */ }
struct TcpListener { /* ... */ }
struct UdpSocket { /* ... */ }
struct SocketAddr { /* ... */ }
struct IpAddr { /* ... */ }
struct Ipv4Addr { /* ... */ }
struct Ipv6Addr { /* ... */ }
```

---

## 7. Time

```spl
// std.time

struct Duration { /* ... */ }
struct Instant { /* ... */ }
struct SystemTime { /* ... */ }

impl Duration {
    fn from_secs(secs: u64): Duration;
    fn from_millis(millis: u64): Duration;
    fn from_micros(micros: u64): Duration;
    fn from_nanos(nanos: u64): Duration;
    fn as_secs(&self): u64;
    fn as_millis(&self): u128;
    fn subsec_nanos(&self): u32;
}

impl Instant {
    fn now(): Instant;
    fn elapsed(&self): Duration;
    fn duration_since(&self, earlier: Instant): Duration;
}
```

---

## 8. Formatting

```spl
// std.fmt

trait Debug {
    fn fmt(&self, f: &mut Formatter): Result(T: (), E: Error);
}

trait Display {
    fn fmt(&self, f: &mut Formatter): Result(T: (), E: Error);
}

// Compiler intrinsics for formatted output.
// These accept a variable number of arguments and the compiler validates
// the format string at compile time. No new keywords are needed — they are
// compiler-recognized functions, not macros.
format("{}", value)      // Returns String
print("{}", value)       // Print to stdout
println("{}", value)     // Print line to stdout
eprint("{}", value)      // Print to stderr
eprintln("{}", value)    // Print line to stderr
```

### Format String Specifiers

`format`, `print`, `println`, `eprint`, and `eprintln` are **compiler intrinsics** with special compile-time format string validation. The first argument must be a string literal (or a `const` string). The compiler verifies at compile time that the number and types of arguments match the format placeholders.

| Specifier | Meaning | Trait Used |
|-----------|---------|------------|
| `{}` | Display formatting | `Display` |
| `{:?}` | Debug formatting | `Debug` |
| `{name}` | Named argument | `Display` (by name) |
| `{name:?}` | Named argument, debug | `Debug` (by name) |
| `{:.N}` | Precision (N decimal places) | `Display` |
| `{:>N}` | Right-align, width N | `Display` |
| `{:<N}` | Left-align, width N | `Display` |
| `{:^N}` | Center-align, width N | `Display` |
| `{:0N}` | Zero-padded, width N | `Display` |
| `{{` | Literal `{` | N/A |
| `}}` | Literal `}` | N/A |

**Examples:**

```spl
let name = "Alice";
let age = 30;

println("Hello, {}!", name)          // Positional: "Hello, Alice!"
println("{name} is {age} years old") // Named: "Alice is 30 years old"
println("Debug: {:?}", some_value)   // Debug formatting
println("Pi: {:.2}", 3.14159)        // Precision: "Pi: 3.14"
println("{:>10}", "right")           // Right-align: "     right"
println("{:06}", 42)                 // Zero-pad: "000042"
```

**Compile-time validation:**
- Mismatched argument count is a compile error
- Using `{}` with a type that does not implement `Display` is a compile error
- Using `{:?}` with a type that does not implement `Debug` is a compile error
- Named arguments must match parameter names or `let` bindings in scope

---

## 9. Iterator Traits

The `Iterator` trait provides sequential value iteration. It yields owned values via `next()`. See [iteration.md](iteration.md) for the full iteration model including `IndexIterator` and `RefIterator`.

### 9.1 Iterator

```spl
trait Iterator {
    type Item;

    /// Returns the next element, or None if exhausted.
    fn next(&mut self): Self.Item?;
}
```

### 9.2 Provided Adapter Methods

Adapters are lazy — they build a pipeline that executes only when a consumer is called.

```spl
impl Iterator {
    /// Transforms each element.
    fn map(self, f: fn(Self.Item): U): Iterator(Item: U) where U;

    /// Keeps only elements satisfying the predicate.
    fn filter(self, predicate: fn(&Self.Item): bool): Iterator(Item: Self.Item);

    /// Filters and maps in a single step.
    fn filter_map(self, f: fn(Self.Item): Option(T: U)): Iterator(Item: U) where U;

    /// Maps each element to an iterator and flattens the results.
    fn flat_map(self, f: fn(Self.Item): IntoIterator(Item: U)): Iterator(Item: U) where U;

    /// Flattens nested iterators.
    fn flatten(self): Iterator(Item: U) where Self.Item: IntoIterator(Item: U), U;

    /// Yields at most `n` elements.
    fn take(self, n: usize): Iterator(Item: Self.Item);

    /// Skips the first `n` elements.
    fn skip(self, n: usize): Iterator(Item: Self.Item);

    /// Yields elements while the predicate is true.
    fn take_while(self, predicate: fn(&Self.Item): bool): Iterator(Item: Self.Item);

    /// Skips elements while the predicate is true.
    fn skip_while(self, predicate: fn(&Self.Item): bool): Iterator(Item: Self.Item);

    /// Chains two iterators end-to-end.
    fn chain(self, other: IntoIterator(Item: Self.Item)): Iterator(Item: Self.Item);

    /// Zips two iterators into pairs. Stops when either is exhausted.
    fn zip(self, other: IntoIterator(Item: U)): Iterator(Item: (Self.Item, U)) where U;

    /// Yields `(index, element)` pairs starting from 0.
    fn enumerate(self): Iterator(Item: (usize, Self.Item));

    /// Wraps in a peekable iterator that supports `peek()`.
    fn peekable(self): Peekable(I: Self);

    /// Wraps to always return None after the first None.
    fn fuse(self): Fuse(I: Self);
}
```

### 9.3 Provided Consumer Methods

Consumers drive the iterator and produce a final value.

```spl
impl Iterator {
    /// Applies a function to each element for side effects.
    fn for_each(self, f: fn(Self.Item)): ();

    /// Left fold with initial accumulator.
    fn fold(self, init: B, f: fn(B, Self.Item): B): B where B;

    /// Reduces elements to a single value without initial accumulator.
    fn reduce(self, f: fn(Self.Item, Self.Item): Self.Item): Self.Item?;

    /// Collects into a target collection type.
    fn collect(self): C where C: FromIterator(Item: Self.Item);

    /// Counts the number of elements.
    fn count(self): usize;

    /// Returns the last element.
    fn last(self): Self.Item?;

    /// Returns the nth element (0-indexed).
    fn nth(self, n: usize): Self.Item?;

    /// Sums all elements.
    fn sum(self): S where S: Sum(Item: Self.Item);

    /// Multiplies all elements.
    fn product(self): P where P: Product(Item: Self.Item);

    /// Returns true if any element satisfies the predicate. Short-circuits.
    fn any(self, predicate: fn(Self.Item): bool): bool;

    /// Returns true if all elements satisfy the predicate. Short-circuits.
    fn all(self, predicate: fn(Self.Item): bool): bool;

    /// Returns the first element satisfying the predicate.
    fn find(self, predicate: fn(&Self.Item): bool): Self.Item?;

    /// Returns the position of the first element satisfying the predicate.
    fn position(self, predicate: fn(Self.Item): bool): usize?;

    /// Returns the maximum element.
    fn max(self): Self.Item? where Self.Item: Ord;

    /// Returns the minimum element.
    fn min(self): Self.Item? where Self.Item: Ord;
}
```

### 9.4 IntoIterator

The `IntoIterator` trait converts a type into an `Iterator`. This is the trait used by `for x in collection` (consuming iteration).

```spl
trait IntoIterator {
    type Item;
    type IntoIter: Iterator(Item: Self.Item);

    fn into_iter(self): Self.IntoIter;
}
```

**Blanket implementation:** Every `Iterator` automatically implements `IntoIterator` (returning itself).

### 9.5 FromIterator

```spl
trait FromIterator {
    type Item;

    /// Creates a collection from an iterator.
    fn from_iter(iter: I): Self where I: IntoIterator(Item: Self.Item);
}
```

### 9.6 Sum and Product

These traits power the `.sum()` and `.product()` consumer methods on iterators.

```spl
/// Trait for types that can be created by summing an iterator.
trait Sum {
    /// Sums the elements of an iterator, starting from the type's additive identity.
    fn sum(iter: I): Self where I: Iterator(Item: Self);
}

/// Trait for types that can be created by multiplying an iterator.
trait Product {
    /// Multiplies the elements of an iterator, starting from the type's multiplicative identity.
    fn product(iter: I): Self where I: Iterator(Item: Self);
}
```

**Built-in implementations:** All integer types, `f32`, `f64`, and `decimal` implement both `Sum` and `Product`.

### 9.7 Step

The `Step` trait defines types that can be iterated over in ranges (`a..b`, `a..=b`). See [iteration.md](iteration.md) section 3 for the full range implementation.

```spl
trait Step: Clone + PartialOrd {
    /// Returns the number of steps from `start` to `self`.
    fn steps_from(&self, start: &Self): usize;

    /// Returns the value `count` steps forward from `self`.
    fn forward(&self, count: usize): Self;

    /// Returns the value `count` steps backward from `self`.
    fn backward(&self, count: usize): Self;
}
```

**Built-in implementations:** All integer types and `char` implement `Step`.

---

## 10. Operator Traits

Operator traits are defined in `std.ops`. Each trait maps to one or more operators in the language. See [traits.md](traits.md) section 1.4 for the generic trait definition pattern.

### 10.0 Operator Desugaring Summary

Every operator in SPL desugars to a trait method call. The compiler rewrites operator expressions before type checking.

| Expression | Desugars To | Trait | Method |
|------------|-------------|-------|--------|
| `a + b` | `a.add(b)` | `Add(RHS)` | `add(self, rHS)` |
| `a - b` | `a.sub(b)` | `Sub(RHS)` | `sub(self, rhs)` |
| `a * b` | `a.mul(b)` | `Mul(RHS)` | `mul(self, rhs)` |
| `a / b` | `a.div(b)` | `Div(RHS)` | `div(self, rhs)` |
| `a % b` | `a.rem(b)` | `Rem(RHS)` | `rem(self, rhs)` |
| `-a` | `a.neg()` | `Neg` | `neg(self)` |
| `a ** b` | `a.pow(b)` | `Pow(RHS)` | `pow(self, rhs)` |
| `a & b` | `a.bitand(b)` | `BitAnd(RHS)` | `bitand(self, rhs)` |
| `a \| b` | `a.bitor(b)` | `BitOr(RHS)` | `bitor(self, rhs)` |
| `a ^ b` | `a.bitxor(b)` | `BitXor(RHS)` | `bitxor(self, rhs)` |
| `!a` | `a.not()` | `Not` | `not(self)` |
| `a << b` | `a.shl(b)` | `Shl(RHS)` | `shl(self, rhs)` |
| `a >> b` | `a.shr(b)` | `Shr(RHS)` | `shr(self, rhs)` |
| `a += b` | `a.add_assign(b)` | `AddAssign(RHS)` | `add_assign(&mut self, rhs)` |
| `a -= b` | `a.sub_assign(b)` | `SubAssign(RHS)` | `sub_assign(&mut self, rhs)` |
| `a *= b` | `a.mul_assign(b)` | `MulAssign(RHS)` | `mul_assign(&mut self, rhs)` |
| `a /= b` | `a.div_assign(b)` | `DivAssign(RHS)` | `div_assign(&mut self, rhs)` |
| `a %= b` | `a.rem_assign(b)` | `RemAssign(RHS)` | `rem_assign(&mut self, rhs)` |
| `a **= b` | `a.pow_assign(b)` | `PowAssign(RHS)` | `pow_assign(&mut self, rhs)` |
| `a &= b` | `a.bitand_assign(b)` | `BitAndAssign(RHS)` | `bitand_assign(&mut self, rhs)` |
| `a \|= b` | `a.bitor_assign(b)` | `BitOrAssign(RHS)` | `bitor_assign(&mut self, rhs)` |
| `a ^= b` | `a.bitxor_assign(b)` | `BitXorAssign(RHS)` | `bitxor_assign(&mut self, rhs)` |
| `a <<= b` | `a.shl_assign(b)` | `ShlAssign(RHS)` | `shl_assign(&mut self, rhs)` |
| `a >>= b` | `a.shr_assign(b)` | `ShrAssign(RHS)` | `shr_assign(&mut self, rhs)` |
| `a == b` | `a.eq(&b)` | `PartialEq(RHS)` | `eq(&self, other: &RHS)` |
| `a != b` | `a.ne(&b)` | `PartialEq(RHS)` | `ne(&self, other: &RHS)` |
| `a < b` | `a.lt(&b)` | `PartialOrd(RHS)` | `lt(&self, other: &RHS)` |
| `a > b` | `a.gt(&b)` | `PartialOrd(RHS)` | `gt(&self, other: &RHS)` |
| `a <= b` | `a.le(&b)` | `PartialOrd(RHS)` | `le(&self, other: &RHS)` |
| `a >= b` | `a.ge(&b)` | `PartialOrd(RHS)` | `ge(&self, other: &RHS)` |
| `collection[i]` | `*collection.index(i)` | `Index(Idx)` | `index(&self, idx)` |
| `collection[i] = v` | `*collection.index_mut(i) = v` | `IndexMut(Idx)` | `index_mut(&mut self, idx)` |

**Mixed-type arithmetic:** SPL does not perform implicit numeric type promotion. `i32 + i64` is a type error — use explicit conversion (e.g., `a.widen() + b`). The default `RHS = Self` on operator traits means binary operators expect both operands to have the same type unless a cross-type implementation is explicitly provided.

**Integer overflow:** All integer arithmetic operations trap (panic) on overflow by default. Use `wrapping_*`, `saturating_*`, or `checked_*` methods for alternative behavior. See [type-system.md](type-system.md) section 8 "Integer Overflow" for details.

### 10.1 Arithmetic Operators

```spl
/// `a + b` desugars to `a.add(b)`
trait Add where RHS = Self {
    type Output;
    fn add(self, rhs: RHS): Self.Output;
}

/// `a - b` desugars to `a.sub(b)`
trait Sub where RHS = Self {
    type Output;
    fn sub(self, rhs: RHS): Self.Output;
}

/// `a * b` desugars to `a.mul(b)`
trait Mul where RHS = Self {
    type Output;
    fn mul(self, rhs: RHS): Self.Output;
}

/// `a / b` desugars to `a.div(b)`
trait Div where RHS = Self {
    type Output;
    fn div(self, rhs: RHS): Self.Output;
}

/// `a % b` desugars to `a.rem(b)`
trait Rem where RHS = Self {
    type Output;
    fn rem(self, rhs: RHS): Self.Output;
}

/// `-a` desugars to `a.neg()`
trait Neg {
    type Output;
    fn neg(self): Self.Output;
}

/// `a ** b` desugars to `a.pow(b)`
trait Pow where RHS = Self {
    type Output;
    fn pow(self, rhs: RHS): Self.Output;
}
```

**Built-in implementations:** All integer types (`i8`–`i128`, `u8`–`u128`, `isize`, `usize`) implement `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Pow`. Signed types also implement `Neg`. Float types (`f32`, `f64`) implement all arithmetic traits. `decimal` implements all arithmetic traits.

```spl
// Example: implementing Add for a custom type
impl Add for Point {
    type Output = Point;
    fn add(self, rhs: Point): Point {
        return Point(x: self.x + rhs.x, y: self.y + rhs.y);
    }
}
```

### 10.2 Bitwise Operators

```spl
/// `a & b` desugars to `a.bitand(b)`
trait BitAnd where RHS = Self {
    type Output;
    fn bitand(self, rhs: RHS): Self.Output;
}

/// `a | b` desugars to `a.bitor(b)`
trait BitOr where RHS = Self {
    type Output;
    fn bitor(self, rhs: RHS): Self.Output;
}

/// `a ^ b` desugars to `a.bitxor(b)`
trait BitXor where RHS = Self {
    type Output;
    fn bitxor(self, rhs: RHS): Self.Output;
}

/// Prefix `!a` desugars to `a.not()` (logical NOT for `bool`, bitwise NOT for integers).
/// Note: prefix `!` (Not operator) is distinct from postfix `!` (try operator, see error-handling.md section 2).
trait Not {
    type Output;
    fn not(self): Self.Output;
}

/// `a << b` desugars to `a.shl(b)`
trait Shl where RHS = Self {
    type Output;
    fn shl(self, rhs: RHS): Self.Output;
}

/// `a >> b` desugars to `a.shr(b)`
trait Shr where RHS = Self {
    type Output;
    fn shr(self, rhs: RHS): Self.Output;
}
```

**Built-in implementations:** All integer types implement `BitAnd`, `BitOr`, `BitXor`, `Not`, `Shl`, `Shr`. `bool` implements `Not`, `BitAnd`, `BitOr`, `BitXor`.

### 10.3 Compound Assignment Operators

```spl
/// `a += b` desugars to `a.add_assign(b)`
trait AddAssign where RHS = Self {
    fn add_assign(&mut self, rhs: RHS): ();
}

/// `a -= b` desugars to `a.sub_assign(b)`
trait SubAssign where RHS = Self {
    fn sub_assign(&mut self, rhs: RHS): ();
}

/// `a *= b` desugars to `a.mul_assign(b)`
trait MulAssign where RHS = Self {
    fn mul_assign(&mut self, rhs: RHS): ();
}

/// `a /= b` desugars to `a.div_assign(b)`
trait DivAssign where RHS = Self {
    fn div_assign(&mut self, rhs: RHS): ();
}

/// `a %= b` desugars to `a.rem_assign(b)`
trait RemAssign where RHS = Self {
    fn rem_assign(&mut self, rhs: RHS): ();
}

/// `a **= b` desugars to `a.pow_assign(b)`
trait PowAssign where RHS = Self {
    fn pow_assign(&mut self, rhs: RHS): ();
}

/// `a &= b`, `a |= b`, `a ^= b`, `a <<= b`, `a >>= b`
trait BitAndAssign where RHS = Self { fn bitand_assign(&mut self, rhs: RHS): (); }
trait BitOrAssign where RHS = Self { fn bitor_assign(&mut self, rhs: RHS): (); }
trait BitXorAssign where RHS = Self { fn bitxor_assign(&mut self, rhs: RHS): (); }
trait ShlAssign where RHS = Self { fn shl_assign(&mut self, rhs: RHS): (); }
trait ShrAssign where RHS = Self { fn shr_assign(&mut self, rhs: RHS): (); }
```

**Built-in implementations:** All types implementing the corresponding binary operator also implement the compound assignment variant.

### 10.4 Index Operators

```spl
/// `collection[index]` desugars to `*collection.index(index)` (in value context)
/// `&collection[index]` desugars to `collection.index(index)`
trait Index where Idx {
    type Output;
    fn index(&self, index: Idx): &Self.Output;
}

/// `&mut collection[index]` desugars to `collection.index_mut(index)`
/// `collection[index] = v` desugars to `*collection.index_mut(index) = v`
trait IndexMut: Index(Idx) where Idx {
    fn index_mut(&mut self, index: Idx): &mut Self.Output;
}
```

**Built-in implementations:** `Vec(T)` implements `Index(Idx: usize)` and `IndexMut(Idx: usize)`. `[T; N]` and `[T]` implement `Index(Idx: usize)` and `IndexMut(Idx: usize)`. `HashMap(K, V)` implements `Index(Idx: K)`.

See [memory-model.md](memory-model.md) section 4 for the full desugaring of index expressions in different contexts.

---

## 11. Comparison Traits

Comparison traits are defined in `std.cmp`. They form a hierarchy: `PartialEq` ← `Eq`, `PartialOrd` ← `Ord`.

### 11.1 PartialEq

Provides equality comparison (`==` and `!=`).

```spl
/// `a == b` desugars to `a.eq(&b)`
/// `a != b` desugars to `a.ne(&b)`
trait PartialEq where RHS = Self {
    /// Required: returns true if self equals other.
    fn eq(&self, other: &RHS): bool;

    /// Provided: returns true if self does not equal other.
    fn ne(&self, other: &RHS): bool {
        return !self.eq(other);
    }
}
```

**Contract:**
- Symmetric: `a == b` implies `b == a`
- Transitive: `a == b` and `b == c` implies `a == c`

**Note:** `PartialEq` does NOT require reflexivity (`a == a`). This allows types like `f32` and `f64` where `NaN != NaN`.

**Built-in implementations:** All primitive types, `String`, `&str`, `bool`, `char`, tuples (element-wise), arrays (element-wise), `Option(T)` where `T: PartialEq`, `Result(T, E)` where `T: PartialEq, E: PartialEq`.

### 11.2 Eq

Marker trait extending `PartialEq` that additionally guarantees reflexivity (`a == a`).

```spl
trait Eq: PartialEq { }
```

**Contract:** In addition to `PartialEq`'s requirements:
- Reflexive: `a == a` is always true

**Note:** `f32` and `f64` implement `PartialEq` but NOT `Eq` (because `NaN != NaN`). `decimal` implements `Eq` (no NaN values).

### 11.3 PartialOrd

Provides ordering comparison (`<`, `>`, `<=`, `>=`).

```spl
trait PartialOrd: PartialEq(RHS) where RHS = Self {
    /// Required: returns the ordering between self and other, or None if incomparable.
    fn partial_cmp(&self, other: &RHS): Option(T: Ordering);

    /// Provided methods using partial_cmp:
    fn lt(&self, other: &RHS): bool {
        return self.partial_cmp(other) is Some(Ordering.Less);
    }
    fn le(&self, other: &RHS): bool {
        match self.partial_cmp(other) {
            Some(Ordering.Less) | Some(Ordering.Equal) => true,
            _ => false,
        }
    }
    fn gt(&self, other: &RHS): bool {
        return self.partial_cmp(other) is Some(Ordering.Greater);
    }
    fn ge(&self, other: &RHS): bool {
        match self.partial_cmp(other) {
            Some(Ordering.Greater) | Some(Ordering.Equal) => true,
            _ => false,
        }
    }
}
```

**Built-in implementations:** All numeric types, `char`, `bool`, `String`, `&str`.

### 11.4 Ord

Provides total ordering. Every pair of values has a defined order.

```spl
trait Ord: Eq + PartialOrd {
    /// Required: returns the ordering between self and other.
    fn cmp(&self, other: &Self): Ordering;

    /// Provided: returns the larger of two values.
    fn max(self, other: Self): Self {
        if self.cmp(&other) is Ordering.Less { other } else { self }
    }

    /// Provided: returns the smaller of two values.
    fn min(self, other: Self): Self {
        if self.cmp(&other) is Ordering.Greater { other } else { self }
    }

    /// Provided: restricts a value to a range [min, max].
    fn clamp(self, min: Self, max: Self): Self {
        if self.cmp(&min) is Ordering.Less { return min; }
        if self.cmp(&max) is Ordering.Greater { return max; }
        return self;
    }
}
```

**Note:** `f32` and `f64` implement `PartialOrd` but NOT `Ord` (because NaN has no total ordering). All integer types, `char`, `bool`, `String`, and `decimal` implement `Ord`.

### 11.5 Ordering

```spl
enum Ordering{
    Less,
    Equal,
    Greater,
}

impl Ordering {
    /// Returns true if this ordering is Less.
    fn is_lt(&self): bool { self is Ordering.Less }

    /// Returns true if this ordering is Equal.
    fn is_eq(&self): bool { self is Ordering.Equal }

    /// Returns true if this ordering is Greater.
    fn is_gt(&self): bool { self is Ordering.Greater }

    /// Reverses the ordering: Less ↔ Greater, Equal stays Equal.
    fn reverse(self): Ordering {
        match self {
            .Less => Ordering.Greater,
            .Equal => Ordering.Equal,
            .Greater => Ordering.Less,
        }
    }

    /// Chains two orderings: uses `other` if self is Equal.
    fn then(self, other: Ordering): Ordering {
        match self {
            .Equal => other,
            _ => self,
        }
    }
}
```

---

## 12. Conversion Traits

Conversion traits are defined in `std.convert`. They provide standard interfaces for converting between types.

### 12.1 From

Infallible conversion from one type to another. Also used by the `!` (try) operator for automatic error conversion — see [error-handling.md](error-handling.md) sections 6-7.

```spl
trait From where T {
    /// Converts from T to Self.
    fn from(value: T): Self;
}
```

**Contract:** The conversion must always succeed (no panics, no errors).

```spl
// Example: converting IoError to AppError
impl From(T: IoError) for AppError {
    fn from(e: IoError): Self {
        return AppError.Io(e);
    }
}

// Usage with ! operator: IoError automatically converts to AppError
fn read_data(path: &str): Data throws AppError {
    let content = fs.read_to_string(path)!;  // IoError → AppError via From
    return parse(content)!;
}
```

### 12.2 Into

The reciprocal of `From`. A blanket implementation provides `Into` for any type implementing `From`:

```spl
trait Into where T {
    /// Converts self into T.
    fn into(self): T;
}

// Blanket implementation (compiler-provided):
impl Into(T: U) for T where T, U, U: From(T: T) {
    fn into(self): U {
        return U.from(self);
    }
}
```

**Guideline:** Implement `From`, not `Into`. The blanket impl gives you `Into` for free.

### 12.3 TryFrom

Fallible conversion from one type to another.

```spl
trait TryFrom where T {
    type Error;

    /// Attempts to convert from T to Self, returning an error on failure.
    fn try_from(value: T): Result(T: Self, E: Self.Error);
}
```

```spl
// Example: converting i64 to u8
impl TryFrom(T: i64) for u8 {
    type Error = TryFromIntError;

    fn try_from(value: i64): Result(T: u8, E: TryFromIntError) {
        if value < 0 || value > 255 {
            return Err(TryFromIntError);
        }
        return Ok(value.truncate());
    }
}

// Usage
let small: u8 = 42i64.try_into()!;   // Ok(42)
let big: u8 = 1000i64.try_into()!;    // Err(TryFromIntError)
```

### 12.4 TryInto

The reciprocal of `TryFrom`. A blanket implementation provides `TryInto` for any type implementing `TryFrom`:

```spl
trait TryInto where T {
    type Error;

    /// Attempts to convert self into T.
    fn try_into(self): Result(T: T, E: Self.Error);
}

// Blanket implementation (compiler-provided):
impl TryInto(T: U) for T where T, U, U: TryFrom(T: T) {
    type Error = U.Error;

    fn try_into(self): Result(T: U, E: U.Error) {
        return U.try_from(self);
    }
}
```

### 12.5 AsRef and AsMut

Cheap reference-to-reference conversions.

```spl
trait AsRef where T {
    fn as_ref(&self): &T;
}

trait AsMut where T {
    fn as_mut(&mut self): &mut T;
}
```

**Built-in implementations:** `String` implements `AsRef(T: str)`, `Vec(T)` implements `AsRef(T: [T])`.

---

## 13. Deref Traits

The `Deref` and `DerefMut` traits enable transparent dereferencing, allowing a type to behave like another type through automatic coercion. They are defined in `std.ops`.

```spl
/// Enables `*value` to produce `&Self.Target`.
/// Also enables automatic deref coercions: `&T` coerces to `&T.Target`.
trait Deref {
    type Target;
    fn deref(&self): &Self.Target;
}

/// Enables mutable dereference: `*value = x` where value: &mut T.
/// Also enables automatic deref coercions: `&mut T` coerces to `&mut T.Target`.
trait DerefMut: Deref {
    fn deref_mut(&mut self): &mut Self.Target;
}
```

**Built-in implementations:**

| Type | Target | Effect |
|------|--------|--------|
| `Box(T)` | `T` | `Box(T)` transparently behaves like `T` |
| `String` | `str` | `&String` coerces to `&str` |
| `Vec(T)` | `[T]` | `&Vec(T)` coerces to `&[T]` |

Deref coercions are used by the method resolution algorithm (see [type-system.md](type-system.md) section 7) to automatically follow the deref chain when looking up methods.

---

## 14. Error Handling

```spl
// std.error

/// The `Error` trait provides a common interface for error types.
/// Used by untyped `throws` functions and for error introspection.
trait Error {
    /// A short description of the error.
    fn message(&self): &str;

    /// The underlying cause of this error, if any.
    fn source(&self): Option(T: &Error) { return None; }
}

/// Standard I/O error type
struct IoError(kind: IoErrorKind, message: String)

enum IoErrorKind{
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    Interrupted,
    UnexpectedEof,
    Other,
}

impl Error for IoError {
    fn message(&self): &str { return &self.message; }
}
```

---

## References

- [type-system.md](type-system.md) - Type system and traits
- [iteration.md](iteration.md) - Iterator design (IndexIterator, RefIterator)
- [error-handling.md](error-handling.md) - Result and Option usage, Try trait
- [concurrency.md](concurrency.md) - Task and channel types
- [memory-model.md](memory-model.md) - Ownership, references, index desugaring
- [closures.md](closures.md) - Closure capture semantics
