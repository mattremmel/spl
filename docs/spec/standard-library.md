# Standard Library

This document provides an overview of SPL's standard library organization and core types.

> **Status:** Skeleton with TODOs. This document outlines the planned standard library structure. Detailed specifications for each module are pending.

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
├── sync            # Synchronization primitives
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
Option, Result, String, Vec, Box

// Enum variants
Some, None, Ok, Err

// Traits
Clone, Copy, Default, Debug, Display
PartialEq, Eq, PartialOrd, Ord
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

```spl
enum Option{ Some(T), None } where T

impl Option(T: T) where T {
    fn is_some(&self): bool;
    fn is_none(&self): bool;
    fn unwrap(self): T;
    fn unwrap_or(self, default: T): T;
    fn unwrap_or_else(self, f: fn(): T): T;
    fn map(self, f: fn(T): U): Option(T: U) where U;
    fn and_then(self, f: fn(T): Option(T: U)): Option(T: U) where U;
    fn or(self, other: Option(T: T)): Option(T: T);
    fn or_else(self, f: fn(): Option(T: T)): Option(T: T);
    fn ok_or(self, err: E): Result(T: T, E: E) where E;
    fn ok_or_else(self, f: fn(): E): Result(T: T, E: E) where E;
    fn as_ref(&self): Option(T: &T);
    fn as_mut(&mut self): Option(T: &mut T);
    fn take(&mut self): Option(T: T);
    fn replace(&mut self, value: T): Option(T: T);
    fn filter(self, predicate: fn(&T): bool): Option(T: T);
    fn flatten(self): Option(T: T) where T: Option(T: U), U;
    fn zip(self, other: Option(T: U)): Option(T: (T, U)) where U;
}
```

**TODO:** Document Option in detail with examples

### 2.2 Result

```spl
enum Result{ Ok(T), Err(E) } where T, E

impl Result(T: T, E: E) where T, E {
    fn is_ok(&self): bool;
    fn is_err(&self): bool;
    fn unwrap(self): T;
    fn unwrap_err(self): E;
    fn unwrap_or(self, default: T): T;
    fn unwrap_or_else(self, f: fn(E): T): T;
    fn map(self, f: fn(T): U): Result(T: U, E: E) where U;
    fn map_err(self, f: fn(E): F): Result(T: T, E: F) where F;
    fn and_then(self, f: fn(T): Result(T: U, E: E)): Result(T: U, E: E) where U;
    fn or(self, other: Result(T: T, E: F)): Result(T: T, E: F) where F;
    fn or_else(self, f: fn(E): Result(T: T, E: F)): Result(T: T, E: F) where F;
    fn ok(self): Option(T: T);
    fn err(self): Option(T: E);
    fn as_ref(&self): Result(T: &T, E: &E);
    fn as_mut(&mut self): Result(T: &mut T, E: &mut E);
}
```

**TODO:** Document Result in detail with examples

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
    fn chars(&self): impl Iterator(Item: char);
    fn bytes(&self): impl Iterator(Item: u8);
    fn lines(&self): impl Iterator(Item: &str);
    fn split(&self, pattern: &str): impl Iterator(Item: &str);
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

**TODO:** Document String in detail with examples

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
    fn iter(&self): impl Iterator(Item: &T);
    fn iter_mut(&mut self): impl Iterator(Item: &mut T);
    fn sort(&mut self): () where T: Ord;
    fn sort_by(&mut self, compare: fn(&T, &T): Ordering): ();
    fn reverse(&mut self): ();
    fn contains(&self, value: &T): bool where T: PartialEq;
    fn dedup(&mut self): () where T: PartialEq;
}
```

**TODO:** Document Vec in detail with examples

### 2.5 Box

```spl
struct Box(T) where T { /* ... */ }

impl Box(T: T) where T {
    fn new(value: T): Box(T: T);
    fn into_inner(self): T;
}
```

**TODO:** Document Box in detail with examples

---

## 3. Collections

**TODO:** Specify each collection type

### 3.1 HashMap

```spl
struct HashMap(K, V) where K: Hash + Eq, V { /* ... */ }
```

### 3.2 HashSet

```spl
struct HashSet(T) where T: Hash + Eq { /* ... */ }
```

### 3.3 BTreeMap

```spl
struct BTreeMap(K, V) where K: Ord, V { /* ... */ }
```

### 3.4 BTreeSet

```spl
struct BTreeSet(T) where T: Ord { /* ... */ }
```

### 3.5 VecDeque

```spl
struct VecDeque(T) where T { /* ... */ }
```

### 3.6 LinkedList

```spl
struct LinkedList(T) where T { /* ... */ }
```

### 3.7 BinaryHeap

```spl
struct BinaryHeap(T) where T: Ord { /* ... */ }
```

---

## 4. I/O

**TODO:** Specify I/O traits and types

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
    fn lines(&self): impl Iterator(Item: Result(T: String, E: IoError));
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

**TODO:** Specify filesystem types and functions

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

**TODO:** Specify networking types

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

**TODO:** Specify time types

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

**TODO:** Specify formatting traits and macros

```spl
// std.fmt

trait Debug {
    fn fmt(&self, f: &mut Formatter): Result(T: (), E: Error);
}

trait Display {
    fn fmt(&self, f: &mut Formatter): Result(T: (), E: Error);
}

// Macros
format!("{}", value)      // Returns String
print!("{}", value)       // Print to stdout
println!("{}", value)     // Print line to stdout
eprint!("{}", value)      // Print to stderr
eprintln!("{}", value)    // Print line to stderr
```

---

## 9. Iterator Utilities

**TODO:** Specify iterator adapters

```spl
// std.iter

trait Iterator {
    type Item;
    fn next(&mut self): Option(T: Self.Item);

    // Adapters (provided)
    fn map(self, f: fn(Self.Item): U): impl Iterator(Item: U) where U;
    fn filter(self, predicate: fn(&Self.Item): bool): impl Iterator(Item: Self.Item);
    fn filter_map(self, f: fn(Self.Item): Option(T: U)): impl Iterator(Item: U) where U;
    fn flat_map(self, f: fn(Self.Item): impl IntoIterator(Item: U)): impl Iterator(Item: U) where U;
    fn flatten(self): impl Iterator(Item: U) where Self.Item: IntoIterator(Item: U), U;
    fn take(self, n: usize): impl Iterator(Item: Self.Item);
    fn skip(self, n: usize): impl Iterator(Item: Self.Item);
    fn take_while(self, predicate: fn(&Self.Item): bool): impl Iterator(Item: Self.Item);
    fn skip_while(self, predicate: fn(&Self.Item): bool): impl Iterator(Item: Self.Item);
    fn chain(self, other: impl IntoIterator(Item: Self.Item)): impl Iterator(Item: Self.Item);
    fn zip(self, other: impl IntoIterator(Item: U)): impl Iterator(Item: (Self.Item, U)) where U;
    fn enumerate(self): impl Iterator(Item: (usize, Self.Item));
    fn peekable(self): Peekable(I: Self);
    fn fuse(self): Fuse(I: Self);

    // Consumers (provided)
    fn collect(self): C where C: FromIterator(Item: Self.Item);
    fn count(self): usize;
    fn last(self): Option(T: Self.Item);
    fn nth(self, n: usize): Option(T: Self.Item);
    fn fold(self, init: B, f: fn(B, Self.Item): B): B where B;
    fn reduce(self, f: fn(Self.Item, Self.Item): Self.Item): Option(T: Self.Item);
    fn all(self, predicate: fn(Self.Item): bool): bool;
    fn any(self, predicate: fn(Self.Item): bool): bool;
    fn find(self, predicate: fn(&Self.Item): bool): Option(T: Self.Item);
    fn position(self, predicate: fn(Self.Item): bool): Option(T: usize);
    fn max(self): Option(T: Self.Item) where Self.Item: Ord;
    fn min(self): Option(T: Self.Item) where Self.Item: Ord;
    fn sum(self): S where S: Sum(Item: Self.Item);
    fn product(self): P where P: Product(Item: Self.Item);
}
```

---

## 10. Operator Traits

**TODO:** Specify operator traits

```spl
// std.ops

trait Add(RHS) where RHS {
    type Output;
    fn add(self, rhs: RHS): Self.Output;
}

trait Sub(RHS) where RHS {
    type Output;
    fn sub(self, rhs: RHS): Self.Output;
}

trait Mul(RHS) where RHS {
    type Output;
    fn mul(self, rhs: RHS): Self.Output;
}

trait Div(RHS) where RHS {
    type Output;
    fn div(self, rhs: RHS): Self.Output;
}

trait Rem(RHS) where RHS {
    type Output;
    fn rem(self, rhs: RHS): Self.Output;
}

trait Neg {
    type Output;
    fn neg(self): Self.Output;
}

trait Not {
    type Output;
    fn not(self): Self.Output;
}

trait Index(Idx) where Idx {
    type Output;
    fn index(&self, index: Idx): &Self.Output;
}

trait IndexMut(Idx) where Idx: Index(Idx) {
    fn index_mut(&mut self, index: Idx): &mut Self.Output;
}
```

---

## 11. Comparison Traits

**TODO:** Specify comparison traits

```spl
// std.cmp

trait PartialEq(RHS) where RHS {
    fn eq(&self, other: &RHS): bool;
    fn ne(&self, other: &RHS): bool { return !self.eq(other); }
}

trait Eq: PartialEq(RHS: Self) { }

trait PartialOrd(RHS) where RHS: PartialEq(RHS) {
    fn partial_cmp(&self, other: &RHS): Option(T: Ordering);
    fn lt(&self, other: &RHS): bool;
    fn le(&self, other: &RHS): bool;
    fn gt(&self, other: &RHS): bool;
    fn ge(&self, other: &RHS): bool;
}

trait Ord: Eq + PartialOrd(RHS: Self) {
    fn cmp(&self, other: &Self): Ordering;
    fn max(self, other: Self): Self;
    fn min(self, other: Self): Self;
    fn clamp(self, min: Self, max: Self): Self;
}

enum Ordering {
    Less,
    Equal,
    Greater,
}
```

---

## 12. Conversion Traits

**TODO:** Specify conversion traits

```spl
// std.convert

trait From(T) where T {
    fn from(value: T): Self;
}

trait Into(T) where T {
    fn into(self): T;
}

trait TryFrom(T) where T {
    type Error;
    fn try_from(value: T): Result(T: Self, E: Self.Error);
}

trait TryInto(T) where T {
    type Error;
    fn try_into(self): Result(T: T, E: Self.Error);
}

trait AsRef(T) where T {
    fn as_ref(&self): &T;
}

trait AsMut(T) where T {
    fn as_mut(&mut self): &mut T;
}
```

---

## 13. Error Handling

**TODO:** Specify error types in detail

```spl
// std.error

/// The `Error` trait provides a common interface for error types.
/// Used by untyped `throws` functions and for error introspection.
trait Error {
    /// A short description of the error.
    fn message(&self): &str;

    /// The underlying cause of this error, if any.
    fn source(&self): Option(T: &dyn Error) { return None; }
}

/// Standard I/O error type
struct IoError {
    kind: IoErrorKind,
    message: String,
}

enum IoErrorKind {
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
- [iteration.md](iteration.md) - Iterator design
- [error-handling.md](error-handling.md) - Result and Option usage
- [concurrency.md](concurrency.md) - Task and channel types
