# ADR-015: Scoped Types and RefIterator

**Status:** Accepted
**Date:** 2026-01-30

## Context

SPL needs `for x in &hashmap` syntax for non-indexed collections (HashMap, LinkedList, Tree, etc.), but the second-class reference rule (refs cannot be stored in structs) prevents traditional external iterators.

### Current State

SPL's iteration model has three mechanisms:

1. **`Indexed` trait** - Works for random-access collections because `get(&self, i): &T` creates fresh borrows each call
2. **`Iterator` trait** - Works for generators and consuming iteration because it yields owned values
3. **Internal iteration** (`.each(fn(&T))`) - Works for any collection but doesn't provide `for` loop syntax

**The gap:** Non-indexed collections cannot use `for x in &collection` syntax because:
- `Indexed` requires O(1) random access (hashmaps, linked lists, trees don't have this)
- A traditional external iterator would need to store `&Collection` in a struct, violating second-class references

### User Requirements

- Enable `for x in &hashmap` syntax for non-indexed collections
- Full iterator chaining support: `.filter().take().map()` etc.
- Maintain SPL's simplicity (no full lifetime system)

## Decision

Introduce **scoped types**: a special category of types that can hold references but are compiler-enforced to never escape their scope. This enables external iterators over non-indexed collections while maintaining memory safety without lifetime annotations.

### Core Concept

Scoped types are analogous to [linear types](https://en.wikipedia.org/wiki/Substructural_type_system) from type theory - values that must be used in a restricted way.

```spl
#[scoped]  // Marks this type as non-escaping
struct HashMapIter(
    source: &HashMap(K, V),  // Allowed because struct is scoped
    position: BucketPos,
) where K, V
```

### Compiler Enforcement Rules

A `#[scoped]` type has these restrictions:

| Rule | Rationale |
|------|-----------|
| Cannot be stored in non-scoped structs | Prevents ref escape via embedding |
| Cannot be returned from non-scoped functions | Prevents ref escape via return |
| Cannot be sent to other threads | Thread-safety (no dangling refs) |
| Must be used within lexical scope of creation | Prevents ref outliving source |
| Can only be passed to functions expecting scoped types | Callee must respect scope |

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

**Key insight:** The `next(&mut self): Option(&Self.Item)` signature is legal under intersection semantics because the returned reference borrows from the input `&mut self`. Since scoped types cannot escape, the reference chain is always bounded.

### How Chaining Works

Adapter types are also scoped. Each combinator returns a new scoped type that wraps the previous one:

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

impl RefIterator for Take(I: I) where I: RefIterator {
    type Item = I.Item;

    fn next(&mut self): Option(&Self.Item) {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        return self.inner.next();
    }
}
```

### Chaining Example

```spl
let map: HashMap(K: String, V: i32) = /* ... */;

// This works because all intermediate types are scoped
let result: Vec(T: i32) = map.ref_iter()      // HashMapIter (scoped)
    .filter(|kv| kv.1 > 10)                    // Filter<HashMapIter> (scoped)
    .take(5)                                   // Take<Filter<...>> (scoped)
    .map(|kv| kv.1.clone())                    // Map yields owned values
    .collect();                                // Collects owned values

// The entire chain is valid because:
// 1. map.ref_iter() creates scoped HashMapIter borrowing &map
// 2. Each adapter wraps the previous, all scoped
// 3. .map() with clone converts refs to owned values
// 4. .collect() consumes owned values
```

### For-Loop Desugaring

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

### Scoped Functions

Functions that accept or return scoped types need special handling:

```spl
// This function can accept scoped types because it doesn't escape them
fn process_all(iter: I) where I: RefIterator {
    for item in iter {
        process(item);
    }
}

// NOT ALLOWED: returning a scoped type from a non-scoped function
// fn make_iter(map: &HashMap): HashMapIter { ... }

// ALLOWED: scoped return in scoped context
#[scoped]
fn filtered_iter(map: &HashMap): Filter(I: HashMapIter) {
    return map.ref_iter().filter(|x| x.1 > 0);
}
```

### Terminal Operations

Terminal operations consume the scoped iterator and produce owned values:

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

## Rationale

### Why Scoped Types?

| Alternative | Why Not Chosen |
|-------------|----------------|
| Compiler magic (for-loop special case) | Doesn't support chaining |
| Limited first-class refs (`Type(&self)`) | Too close to full lifetimes, complex |
| Generator-based refs | Complex resumption semantics with references |
| Keep internal iteration only | Doesn't provide `for` syntax |

Scoped types provide:
1. **Full chaining support** - Standard `.filter().map().take()` patterns work
2. **No lifetime annotations** - The scoped rule is simple: "don't escape"
3. **Consistency with closures** - Same mental model as non-escaping closures
4. **Zero runtime cost** - Purely compile-time enforcement

### Precedent: Non-Escaping Closures

This design mirrors how SPL already handles non-escaping closures:

| Closures | Scoped Structs |
|----------|----------------|
| Can borrow from scope | Can hold references |
| Cannot be stored/returned | Cannot escape scope |
| Compiler tracks escaping | Compiler tracks scope |
| Used by map/filter/each | Used by RefIterator chain |

From ADR-012 (Closures):
> "Non-escaping closures can temporarily borrow from the enclosing scope because the borrow doesn't outlive the function call"

The same reasoning applies to scoped structs.

### Theory: Substructural Types

This approach is related to [linear types](https://en.wikipedia.org/wiki/Substructural_type_system) from type theory:
- **Linear types**: Must be used exactly once
- **Affine types**: Must be used at most once (Rust's default)
- **Scoped types** (our concept): Must not escape their creation scope

[Austral](https://borretti.me/article/introducing-austral) uses linear types with regions for similar effect. SPL's scoped types are simpler - just "don't escape" rather than full linear tracking.

### Why RefIterator vs Extending Iterator?

We chose a separate `RefIterator` trait rather than modifying `Iterator` because:

1. **Clear semantics**: `Iterator` yields owned values, `RefIterator` yields references
2. **Type safety**: Prevents accidentally mixing owned and borrowed iteration
3. **Implementation clarity**: Collections implement whichever is appropriate
4. **Simpler bounds**: `where I: RefIterator` clearly signals scoped iteration

## Consequences

### Positive

- **Fills the gap**: Non-indexed collections can use `for x in &collection` syntax
- **Full chaining**: Standard iterator adapter patterns work naturally
- **No lifetime annotations**: The "scoped" concept is simpler than lifetimes
- **Consistent with existing design**: Mirrors non-escaping closure behavior
- **Zero runtime cost**: Purely compile-time enforcement
- **Gradual adoption**: Collections can add `RefIterator` support incrementally

### Negative

- **New concept**: Users must understand "scoped" vs regular types
- **Two iteration traits**: `Iterator` and `RefIterator` may cause confusion
- **Implementation complexity**: Compiler must track scoped-ness through generics
- **Limited patterns**: Some iterator patterns from other languages won't translate

### Migration and Adoption

Existing code continues to work:
- `Indexed` types keep using `for x in &vec` with current desugaring
- `Iterator` continues working for generators and consuming iteration
- `each()` continues working for internal iteration

New patterns enabled:
- `for x in &hashmap` with `RefIterator` implementation
- Chained adapters on reference iterators

## Implementation Considerations

### Borrow Checker Changes

- Track "scoped-ness" as a type property
- Propagate through generic bounds
- Error if scoped type escapes

### Type System

- New `#[scoped]` attribute
- Scoped types form a subset: `Scoped ⊂ All Types`
- Generics can be bounded: `where I: RefIterator` implies scoped

### Code Generation

- No runtime cost - purely compile-time
- Scoped types have same representation as non-scoped

### Error Messages

```spl
fn bad_escape(map: &HashMap): HashMapIter {  // ERROR
    return map.ref_iter();
}
// Error: cannot return scoped type `HashMapIter` from non-scoped function
// Help: scoped types can only be used within their creation scope

fn bad_store(map: &HashMap) {
    let iter = map.ref_iter();
    let s = SomeStruct(iter: iter);  // ERROR
}
// Error: cannot store scoped type `HashMapIter` in non-scoped struct `SomeStruct`
```

## Implementation Roadmap

### Phase 1: Core Infrastructure
1. Add `#[scoped]` attribute to type system
2. Implement escape analysis for scoped types
3. Add `RefIterator` trait to prelude

### Phase 2: Standard Library
1. Implement `RefIterator` for HashMap
2. Implement standard adapters (Filter, Map, Take, Skip, etc.)
3. Implement `RefIterator` for LinkedList, BTreeMap, etc.

### Phase 3: Ergonomics
1. For-loop desugaring for RefIterator types
2. Error message improvements
3. Documentation and examples

## Open Design Questions

### 1. Mutable Reference Iteration

Should RefIterator support `&mut` iteration?

```spl
#[scoped]
struct HashMapIterMut(
    source: &mut HashMap(K, V),
    position: BucketPos,
) where K, V

// Would enable:
for (k, v) in &mut hashmap {
    *v += 1;  // mutate values
}
```

**Recommendation:** Yes, add `RefIteratorMut` trait with `next(&mut self): Option(&mut Self.Item)`.

### 2. Interaction with Indexed Trait

How do `Indexed` and `RefIterator` relate?

**Decision:** Separate traits, separate desugaring:
- `for x in &vec` uses `Indexed.get(i)` (random-access optimization)
- `for x in &hashmap` uses `RefIterator.next()` (sequential access)

### 3. Generic Bounds Syntax

How do you write a generic function that accepts any RefIterator?

```spl
// RefIterator implementations are always scoped, so the bound is implied
fn process_all(iter: I) where I: RefIterator { ... }
```

**Decision:** `RefIterator` bound implies scoped - no additional annotation needed.

## References

- [ADR-011: Iteration and Generators](011-iteration-and-generators.md) - SPL iteration design
- [ADR-012: Closures and Capture Semantics](012-closures.md) - Non-escaping closures precedent
- [Substructural type systems (Wikipedia)](https://en.wikipedia.org/wiki/Substructural_type_system) - Linear/affine type theory
- [Austral language](https://borretti.me/article/introducing-austral) - Linear types with regions
- [Swift Ownership Manifesto](https://github.com/apple/swift/blob/main/docs/OwnershipManifesto.md) - Non-escaping values
