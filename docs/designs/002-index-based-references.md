# ADR-002: Index-Based References (DefId, TypeId)

**Status:** Accepted
**Date:** 2026-01-28

## Context

A compiler needs to reference definitions (functions, structs, variables) and types throughout the compilation pipeline. These references must be:
- Cheap to copy and compare
- Stable across compilation phases
- Safe to store in multiple data structures
- Suitable for use as map keys

Using pointers or references directly creates ownership and lifetime challenges. Using strings (names) is slow and doesn't handle shadowing or scopes.

## Decision

Use lightweight index-based handles (`DefId`, `TypeId`, `ScopeId`):

```rust
pub struct DefId {
    crate_id: CrateId,      // Which crate (for cross-crate support)
    local_id: LocalDefId,   // Index within crate
}

pub struct LocalDefId {
    index: u32,
    #[cfg(debug_assertions)]
    generation: u32,        // Catches use-after-free in debug builds
}

pub struct TypeId(u32);
pub struct ScopeId(u32);
pub struct CrateId(u32);
```

These handles are:
- Assigned once during resolution, never change
- Used as keys in maps (`DefId` → type info, `DefId` → symbol info)
- Stored in HIR/MIR nodes instead of names or pointers
- `Copy` for ergonomic use
- Debug-friendly (print with names, not just numbers)
- Cross-crate ready (CrateId + LocalDefId structure)

### ID Space Partitioning

`DefId` partitions its ID space:
- **User definitions**: `0` to `BUILTIN_START - 1`
- **Builtin definitions**: `BUILTIN_START` to `MAX - 1`
- **Invalid sentinel**: `u32::MAX` (marker for unresolved/error cases)

This ensures user and builtin IDs never collide without runtime checks.

## Rationale

### Why Not Pointers?
- Ownership becomes complex across phases
- Hard to store in multiple collections
- Lifetime annotations proliferate
- Can't easily serialize/deserialize

### Why Not String Names?
- Name comparison is O(n)
- Shadowing requires scope tracking anyway
- Qualified names are verbose and error-prone
- Interning strings gives similar benefits but with more complexity

### Why u32?
- 4 billion symbols is sufficient for any realistic program
- Smaller than `usize` on 64-bit (cache-friendly)
- Trivially `Copy`, `Eq`, `Hash`
- Matches arena index types

### Why Partition ID Space?
- No runtime checks needed to distinguish user vs builtin
- Can use same lookup tables for both
- Invalid sentinel simplifies error handling

### Why CrateId + LocalDefId?
- **Future-proof**: Separate compilation requires knowing which crate a def came from
- **Cheap to add now**: Just a u32 field, defaults to `CrateId(0)` for single-crate
- **Expensive to retrofit**: Changing DefId layout later breaks all serialization and caches

### Why Generation Counters?
- **Catch stale IDs**: If you hold a DefId after its arena is cleared, debug builds panic
- **Zero release cost**: `#[cfg(debug_assertions)]` removes the field entirely
- **Real bugs**: Compiler refactors often accidentally hold stale references

### Why Debug-Friendly Formatting?
- `DefId(42)` is useless for debugging
- `DefId(crate0::42 "process_request")` tells you exactly what you're looking at
- Requires context (name table), but worth the ergonomic improvement

## Consequences

### Positive
- Efficient: single u32 copy, O(1) equality
- Stable: IDs don't change, safe to cache
- Simple: no lifetimes, no ownership issues
- Flexible: works as map keys, array indices, etc.
- Debug-friendly: meaningful output when debugging
- Future-proof: cross-crate ready from day one
- Safe: generation counters catch stale ID bugs in debug builds

### Negative
- Indirection: must look up in tables to get actual data
- No type-level distinction between different ID spaces (user vs builtin)
- Invalid IDs can propagate if not checked
- Debug formatting requires name table access (thread-local or passed explicitly)
- Slightly larger struct in debug builds (generation counter)

## Implementation

- **DefId**: `spl-sema/src/symbol.rs`
- **TypeId**: `spl-sema/src/types.rs`
- **ScopeId**: `spl-sema/src/scope.rs`

### Usage Examples

```rust
// Symbol table maps DefId to Symbol info
let symbol = resolve_result.symbols.get(def_id);

// Type inference maps DefId to TypeId
let type_id = infer_result.binding_types.get(&def_id);

// HIR stores DefId, not names
HirExprKind::Var { def_id }
```

### Debug Formatting

```rust
impl DefId {
    /// Debug format with name lookup
    pub fn debug_with<'a>(&self, names: &'a NameTable) -> impl fmt::Debug + 'a {
        DefIdDebug { id: *self, names }
    }
}

// Usage in debug output:
// DefId(crate0::42 "process_request")

// Thread-local context for convenient Debug impl
impl fmt::Debug for DefId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(names) = NameTable::try_get_thread_local() {
            write!(f, "DefId({}::{})", self.crate_id.0, self.local_id.index)?;
            if let Some(name) = names.get(*self) {
                write!(f, " {:?}", name)?;
            }
            Ok(())
        } else {
            write!(f, "DefId({}::{})", self.crate_id.0, self.local_id.index)
        }
    }
}
```

### Generation Counter Validation

```rust
impl LocalDefId {
    #[cfg(debug_assertions)]
    pub fn validate(&self, arena: &DefArena) {
        assert_eq!(
            self.generation, arena.generation(),
            "stale DefId: arena has been reset since this ID was created"
        );
    }

    #[cfg(not(debug_assertions))]
    pub fn validate(&self, _arena: &DefArena) {
        // No-op in release builds
    }
}
```

## References

- [Rust Compiler DefId](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_hir/def_id/struct.DefId.html)
- [la_arena crate](https://docs.rs/la-arena/) - Arena allocation with typed indices
