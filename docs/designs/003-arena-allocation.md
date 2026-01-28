# ADR-003: Arena Allocation for HIR/MIR

**Status:** Accepted
**Date:** 2026-01-28

## Context

Intermediate representations (HIR, MIR) contain many small, interconnected nodes. Traditional heap allocation has problems:
- Many small allocations are slow
- Reference cycles require careful lifetime management
- Hard to drop the entire IR efficiently
- Cache locality is poor

The compiler needs an allocation strategy that is:
- Fast for many small allocations
- Easy to drop all at once
- Provides stable references during a phase
- Cache-friendly

## Decision

Use arena allocation via the `la_arena` crate for HIR and MIR nodes:

```rust
use la_arena::{Arena, Idx};

pub struct HirDatabase {
    pub exprs: Arena<HirExpr>,
    pub stmts: Arena<HirStmt>,
    pub pats: Arena<HirPat>,
    // ...
}

pub type ExprId = Idx<HirExpr>;
```

Key properties:
- Nodes allocated in typed arenas
- References via `Idx<T>` (lightweight handles, like `DefId`)
- Entire arena dropped at once
- No individual deallocation

## Rationale

### Why Arena Allocation?
- **Fast allocation**: Bump allocation is O(1)
- **Batch deallocation**: Drop entire IR in one operation
- **No reference counting**: `Idx<T>` is `Copy`, no `Rc`/`Arc` needed
- **Cache locality**: Nodes of same type are contiguous in memory
- **No lifetimes**: Indices are stable, no `&'arena` annotations

### Why la_arena?
- Used by rust-analyzer (proven in production)
- Typed indices (`Idx<T>`) prevent mixing up different arenas
- `#[cfg(debug_assertions)]` bounds checking
- Simple API, minimal dependencies

### Why Not Reference-Counted (Rc/Arc)?
- Overhead per node (reference count, weak count)
- Cycles require `Weak` references
- Can't drop entire graph efficiently
- More complex ownership model

### Why Not Raw Pointers?
- Unsafe, error-prone
- Hard to ensure validity
- No type safety between different arenas

## Consequences

### Positive
- Fast allocation and deallocation
- Simple ownership model (arena owns everything)
- Cache-friendly memory layout
- Indices are `Copy`, easy to pass around
- Entire IR dropped in one operation

### Negative
- Can't deallocate individual nodes (minor memory waste)
- Must pass arena alongside indices for lookups
- Indices are invalid after arena is dropped (use-after-free if misused)

## Implementation

- **HIR arena**: `spl-hir/src/lib.rs` - `HirDatabase`
- **MIR arena**: `spl-mir/src/body.rs` - `Body` (contains arenas)

### Usage Examples

```rust
// Allocate a new expression
let expr = HirExpr { kind: HirExprKind::Literal(Literal::Int(42)), ty, span };
let expr_id = db.alloc_expr(expr);

// Look up expression by ID
let expr = db.expr(expr_id);

// HIR nodes reference each other via IDs
HirExprKind::Binary { lhs: expr_id_1, rhs: expr_id_2, op }
```

### Memory Layout

```
Arena<HirExpr>:
┌────────┬────────┬────────┬────────┬───────────┐
│ Expr 0 │ Expr 1 │ Expr 2 │ Expr 3 │ (unused)  │
└────────┴────────┴────────┴────────┴───────────┘
          ↑
          Idx(1) points here (O(1) lookup)
```

## References

- [la_arena crate](https://docs.rs/la-arena/)
- [rust-analyzer HIR](https://github.com/rust-lang/rust-analyzer/tree/master/crates/hir)
- [Arena Allocation (Wikipedia)](https://en.wikipedia.org/wiki/Region-based_memory_management)
