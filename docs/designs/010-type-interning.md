# ADR-010: Type Interning

**Status:** Accepted
**Date:** 2026-01-28

## Context

The type system needs to:
- Compare types for equality frequently
- Store types in many places (HIR nodes, symbol table, etc.)
- Handle structural types (tuples, arrays, function pointers)
- Support type variables for inference
- Be memory-efficient

Naive approaches:
- **Clone types everywhere**: Expensive for structural types
- **Reference-counted types**: Overhead, complex ownership
- **String representation**: Slow comparison, parsing overhead

## Decision

Intern all types in a `TypeInterner`, reference via `TypeId`:

```rust
pub struct TypeId(u32);

pub struct TypeInterner {
    types: Vec<Type>,
    type_to_id: HashMap<Type, TypeId>,
    // Pre-interned common types
    unit_id: TypeId,
    bool_id: TypeId,
    i32_id: TypeId,
    // ...
}
```

Key properties:
- Types stored once, referenced by ID
- Equality via ID comparison (O(1))
- Structural types interned by value
- Common types pre-interned for fast access

## Rationale

### Why Interning?
- **Fast equality**: `type1 == type2` is just `id1 == id2`
- **Memory efficient**: Each unique type stored once
- **Simple references**: `TypeId` is `Copy`, no lifetimes
- **Deterministic**: Same type always gets same ID

### Why Pre-intern Primitives?
- Primitives are used constantly
- Avoid hash lookups for common types
- `interner.i32()` is O(1)

### Why Separate TypeVar?
- Type variables are ephemeral (inference only)
- Need unique identity even with same constraints
- Different lifetime than interned types

### Why HashMap for Dedup?
- Need to check if type already exists
- Structural types need deep comparison
- HashMap gives O(1) average lookup

## Consequences

### Positive
- O(1) type equality (just compare IDs)
- Memory efficient (no duplication)
- Simple API (`TypeId` is `Copy`)
- Fast access to common types

### Negative
- Must pass interner for type operations
- Extra indirection for type data
- IDs invalid after interner dropped

## Implementation

- **TypeId**: `spl-sema/src/types.rs`
- **TypeInterner**: `spl-sema/src/types.rs`
- **Type enum**: `spl-sema/src/types.rs`

### Type Representation

```rust
pub enum Type {
    Primitive(PrimitiveKind),
    Infer(TypeVar, InferKind),  // Inference variable
    Ref(Mutability, TypeId),
    RawPtr(Mutability, TypeId),
    Array(TypeId, u64),
    Slice(TypeId),
    Tuple(Vec<TypeId>),
    Struct(DefId, Vec<TypeId>),  // With type args
    Alias(DefId, Vec<TypeId>),
    FnPtr { params: Vec<TypeId>, ret: TypeId },
    Param(DefId),  // Generic type parameter
    SelfType,
    StrRef,
    Module(DefId),
    Error,
}
```

### Interner API

```rust
impl TypeInterner {
    // Intern a type (returns existing ID if present)
    pub fn intern(&mut self, ty: Type) -> TypeId;

    // Get type data by ID
    pub fn get(&self, id: TypeId) -> &Type;

    // Pre-interned accessors
    pub fn unit(&self) -> TypeId;
    pub fn bool(&self) -> TypeId;
    pub fn i32(&self) -> TypeId;
    // ...

    // Type construction helpers
    pub fn mk_ref(&mut self, mutability: Mutability, inner: TypeId) -> TypeId;
    pub fn mk_array(&mut self, elem: TypeId, len: u64) -> TypeId;
    pub fn mk_tuple(&mut self, elems: Vec<TypeId>) -> TypeId;
    // ...

    // Fresh type variables
    pub fn fresh_type_var(&mut self) -> TypeId;
    pub fn fresh_int_var(&mut self) -> TypeId;
    pub fn fresh_float_var(&mut self) -> TypeId;
}
```

### Inference Variable Kinds

```rust
pub enum InferKind {
    General,  // Unifies with anything
    Int,      // Only integers, defaults to i32
    Float,    // Only floats, defaults to f64
}
```

This enables:
- `let x = 42;` → `x: ?Int` → defaults to `i32`
- `let y = 3.14;` → `y: ?Float` → defaults to `f64`

### Usage Example

```rust
let mut interner = TypeInterner::new();

// Pre-interned primitives
let i32_ty = interner.i32();
let bool_ty = interner.bool();

// Structural types
let arr_ty = interner.mk_array(i32_ty, 10);
let tuple_ty = interner.mk_tuple(vec![i32_ty, bool_ty]);

// Same structure = same ID
let arr_ty2 = interner.mk_array(i32_ty, 10);
assert_eq!(arr_ty, arr_ty2);  // O(1) comparison
```

## References

- [String interning](https://en.wikipedia.org/wiki/String_interning)
- [lasso crate](https://docs.rs/lasso/) - String interning
- [Rust Type Representation](https://rustc-dev-guide.rust-lang.org/ty.html)
