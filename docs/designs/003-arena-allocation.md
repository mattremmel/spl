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
    pub types: Arena<HirType>,
    pub blocks: Arena<HirBlock>,
    // Separate arena per node type for cache locality
}

pub type ExprId = Idx<HirExpr>;
pub type StmtId = Idx<HirStmt>;
pub type PatId = Idx<HirPat>;
```

Key properties:
- Nodes allocated in typed arenas
- **Separate arena per node type** for cache-friendly traversals
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

### Why Separate Arenas Per Node Type?

Cache locality significantly impacts compile times:

```
Single arena (interleaved):
┌──────┬──────┬──────┬──────┬──────┬──────┬──────┐
│ Expr │ Stmt │ Pat  │ Expr │ Stmt │ Expr │ Pat  │
└──────┴──────┴──────┴──────┴──────┴──────┴──────┘
         ↑ Cache misses when traversing only exprs

Separate arenas (contiguous):
Exprs:  ┌──────┬──────┬──────┐
        │ Expr │ Expr │ Expr │  ← Cache-friendly traversal
        └──────┴──────┴──────┘
Stmts:  ┌──────┬──────┐
        │ Stmt │ Stmt │
        └──────┴──────┘
Pats:   ┌──────┬──────┐
        │ Pat  │ Pat  │
        └──────┴──────┘
```

Benefits:
- **Type checking**: Traverses expressions heavily → exprs are contiguous
- **Control flow analysis**: Traverses statements → stmts are contiguous
- **Pattern matching compilation**: Traverses patterns → pats are contiguous
- **Smaller working set**: Each pass loads only relevant data into cache

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
- Indices are invalid after arena is dropped (mitigated by generation counters, see [ADR-002](002-index-based-references.md))
- Multiple arenas means more fields to manage in database structs

## Implementation

- **HIR arena**: `spl-hir/src/lib.rs` - `HirDatabase`
- **MIR arena**: `spl-mir/src/body.rs` - `Body` (contains arenas)
- **Index safety**: See [ADR-002](002-index-based-references.md) for generation counters that catch stale index bugs

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

### HirDatabase Structure

```rust
pub struct HirDatabase {
    // Separate arenas for cache-friendly traversals
    pub exprs: Arena<HirExpr>,
    pub stmts: Arena<HirStmt>,
    pub pats: Arena<HirPat>,
    pub types: Arena<HirTypeRef>,
    pub blocks: Arena<HirBlock>,
    pub params: Arena<HirParam>,

    // Generation counter for stale index detection (debug only)
    #[cfg(debug_assertions)]
    generation: u32,
}

impl HirDatabase {
    pub fn alloc_expr(&mut self, expr: HirExpr) -> ExprId {
        self.exprs.alloc(expr)
    }

    pub fn expr(&self, id: ExprId) -> &HirExpr {
        #[cfg(debug_assertions)]
        id.validate_generation(self.generation);
        &self.exprs[id]
    }
}
```

## References

- [la_arena crate](https://docs.rs/la-arena/)
- [rust-analyzer HIR](https://github.com/rust-lang/rust-analyzer/tree/master/crates/hir)
- [Arena Allocation (Wikipedia)](https://en.wikipedia.org/wiki/Region-based_memory_management)
