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

Use lightweight index-based handles (`DefId`, `TypeId`, `ScopeId`) that are simple `u32` wrappers:

```rust
pub struct DefId(u32);
pub struct TypeId(u32);
pub struct ScopeId(u32);
```

These handles are:
- Assigned once during resolution, never change
- Used as keys in maps (`DefId` → type info, `DefId` → symbol info)
- Stored in HIR/MIR nodes instead of names or pointers
- `Copy` for ergonomic use

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

## Consequences

### Positive
- Efficient: single u32 copy, O(1) equality
- Stable: IDs don't change, safe to cache
- Simple: no lifetimes, no ownership issues
- Flexible: works as map keys, array indices, etc.

### Negative
- Indirection: must look up in tables to get actual data
- No type-level distinction between different ID spaces (user vs builtin)
- Invalid IDs can propagate if not checked

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

## References

- [Rust Compiler DefId](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_hir/def_id/struct.DefId.html)
- [la_arena crate](https://docs.rs/la-arena/) - Arena allocation with typed indices
