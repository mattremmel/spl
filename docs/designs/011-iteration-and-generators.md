# ADR-011: Iteration and Generators

**Status:** Accepted
**Date:** 2026-01-28

## Context

SPL needs an iteration model. The two primary approaches are:

1. **Exterior iteration** (Rust/Java/C++): Iterator objects that hold references to collections
2. **Interior iteration** (early Rust, coroutines): Collections control traversal via callbacks/generators

Rust's exterior iteration requires first-class references (iterators store `&Collection`), which brings lifetime complexity. SPL has chosen second-class references (see [DECISIONS.md](../DECISIONS.md) §4.1), making exterior iteration over borrowed data impossible.

Additionally, users expect ergonomic method chaining:
```spl
numbers.iter()
    .filter(|n| n > 0)
    .map(|n| n * 2)
    .take(5)
    .for_each(|n| println(n))
```

## Decision

### Phase 1: Generator Methods (Initial Release)

Use **interior iteration via generators** with method syntax for ergonomic chaining.

#### Generator Functions

```spl
gen fn naturals(): u64 {
    let n = 0u64;
    loop {
        yield n;
        n = n + 1;
    }
}

// Consuming generators
for n in naturals() {
    if n > 100 { break; }
    println(n);
}
```

#### Generator Methods on Collections

Collections provide generator methods that consume `self`:

```spl
impl Vec(T) where T {
    // Consuming iterator - takes ownership
    gen fn iter(self): T {
        for i in 0..self.len() {
            yield self.get(i);
        }
    }

    // Generator transformers
    gen fn filter(self, pred: fn(&T): bool): T {
        for item in self {
            if pred(&item) {
                yield item;
            }
        }
    }

    gen fn map(self, f: fn(T): U): U where U {
        for item in self {
            yield f(item);
        }
    }

    gen fn take(self, n: usize): T {
        let count = 0usize;
        for item in self {
            if count >= n { break; }
            yield item;
            count = count + 1;
        }
    }

    fn for_each(self, f: fn(T): ()): () {
        for item in self {
            f(item);
        }
    }
}
```

#### Method Chaining

Generator methods enable natural chaining:

```spl
// Each method consumes previous generator, produces new generator
let result = vec.iter()
    .filter(|n| n > 0)
    .map(|n| n * 2)
    .take(5)
    .collect();
```

Under the hood, these are generator transformers, not iterator objects holding references.

### Phase 2: First-Class References for `&self` (Future)

Later, SPL may add a **limited form of first-class references** specifically for methods returning references tied to `self`:

```spl
impl Vec(T) where T {
    // Future: borrow from self, return reference tied to self's lifetime
    fn iter(&self): Iter(&self, T) { ... }
}
```

This enables zero-copy iteration without the full complexity of Rust's lifetime system because:
- The lifetime relationship is always obvious (result borrows from `self`)
- No multi-parameter lifetime inference needed
- Single annotation syntax: `&self` in return type

This follows the approach outlined in [Swift's Ownership Manifesto](https://github.com/apple/swift/blob/main/docs/OwnershipManifesto.md): start strict, add escape hatches.

## Rationale

### Why Interior Iteration?

Graydon Hoare (Rust's original designer) on exterior vs interior iteration:

> "Iteration used to be by stack / non-escaping coroutines, which we also called 'interior' iteration... Such coroutines are now finally supported by LLVM and are actually a fairly old and reliable mechanism for a linking-friendly, not-having-to-inline-tons-of-library-code abstraction for iteration."

Benefits:
- **No lifetime complexity**: References don't escape function boundaries
- **No iterator invalidation**: Collection controls traversal
- **Simpler borrow checker**: No need to track borrowed iterators
- **Natural async integration**: Generators compose with async/await

### Why Generator Methods?

Composition syntax matters for usability. Compare:

```spl
// Function composition (awkward)
for x in map(filter(numbers.iter(), |n| n > 0), |n| n * 2) {
    process(x);
}

// Method chaining (natural)
numbers.iter()
    .filter(|n| n > 0)
    .map(|n| n * 2)
    .for_each(|x| process(x));
```

Generator methods provide the ergonomic chaining syntax while maintaining the interior iteration model.

### Why Phased Approach?

1. **Phase 1 works now**: Consuming generators handle most use cases
2. **Phase 2 is additive**: Adding `&self` returns doesn't break existing code
3. **Constraints inform design**: Building Phase 1 reveals which patterns actually need Phase 2
4. **Swift precedent**: Adding ownership features to a simpler base works (see Ownership Manifesto)

### What About Infinite/Stateful Iterators?

Second-class references don't prevent all iterators—only those that borrow from collections:

| Pattern | Works in Phase 1? |
|---------|-------------------|
| Infinite generators (`naturals()`) | Yes - yields owned values |
| Consuming iteration (`vec.iter()`) | Yes - moves ownership |
| Stateful generators (counters, RNGs) | Yes - state is owned |
| Borrowing iteration (`&vec.iter()` → `&T`) | Phase 2 |

## Consequences

### Positive

- No lifetime annotations for iteration
- Method chaining syntax from day one
- Simpler mental model (generators, not iterator traits)
- Clear upgrade path to zero-copy iteration
- Works with second-class references

### Negative

- Phase 1 consuming iteration may clone/copy more than necessary
- Some Rust iterator patterns don't translate directly
- Two iteration styles (consuming now, borrowing later) may cause ecosystem churn
- Generator implementation complexity in compiler

### Migration Considerations

Code written for Phase 1 will continue to work in Phase 2. Users wanting zero-copy iteration can migrate:

```spl
// Phase 1: consuming (works forever)
let doubled: Vec(i32) = vec.clone().iter().map(|n| n * 2).collect();

// Phase 2: borrowing (future, more efficient)
let doubled: Vec(i32) = vec.iter().map(|n| n * 2).collect();
```

## Implementation

### Generator Type

Generators are a distinct type category:

```spl
// gen fn declares a generator function
gen fn count_to(n: i32): i32 {
    for i in 0..n {
        yield i;
    }
}

// The type of a generator producing T
type CounterGen = gen i32;
```

### For Loop Desugaring

`for` loops desugar to generator consumption:

```spl
// Source
for x in generator {
    body(x);
}

// Desugars to (conceptually)
loop {
    match generator.next() {
        Some(x) => body(x),
        None => break,
    }
}
```

### Compiler Components

- **Parser**: `gen fn` syntax, `yield` expression
- **HIR**: `GeneratorDef`, `YieldExpr` nodes
- **Type checker**: Generator type inference, yield type unification
- **MIR**: Generator state machine transformation
- **Codegen**: LLVM coroutine intrinsics or state machine lowering

## References

- [Rust Design Lessons](../rust-design-lessons.md) - Interior vs exterior iteration
- [Swift Ownership Manifesto](https://github.com/apple/swift/blob/main/docs/OwnershipManifesto.md) - Phased ownership adoption
- [LLVM Coroutines](https://llvm.org/docs/Coroutines.html) - Implementation target
- [Kotlin Sequences](https://kotlinlang.org/docs/sequences.html) - Similar lazy evaluation model
- [Python Generators](https://docs.python.org/3/howto/functional.html#generators) - Interior iteration precedent
