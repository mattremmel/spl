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

Intern all types in a `TypeInterner`, reference via `TypeId`. This follows the same index-based pattern as `DefId` (see [ADR-002](002-index-based-references.md)):

```rust
pub struct TypeId {
    index: u32,
    #[cfg(debug_assertions)]
    generation: u32,  // Catches stale IDs (see ADR-002)
}

pub struct TypeInterner {
    types: Vec<Type>,
    type_to_id: FxHashMap<Type, TypeId>,  // Fast hasher (see below)

    // Pre-interned common types
    unit_id: TypeId,
    bool_id: TypeId,
    i32_id: TypeId,
    // ...

    #[cfg(debug_assertions)]
    generation: u32,
}
```

Key properties:
- Types stored once, referenced by ID
- Equality via ID comparison (O(1))
- Structural types interned by value
- Common types pre-interned for fast access
- Generation counters for stale ID detection (debug builds)
- Debug-friendly display shows type structure

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

### Structural Hash Collisions

Structural types (tuples, function pointers, generic instantiations) must be hashed for interning. Recommendation: **Use FxHash** instead of Rust's default SipHash.

| Hasher | Speed | HashDoS Safe | Use Case |
|--------|-------|--------------|----------|
| SipHash (default) | Slower | Yes | Untrusted input |
| FxHash | Fast | No | Compiler internals |
| AHash | Fast | Yes | General purpose |

**Why FxHash is safe here:**
- Compiler controls all inputs (no untrusted data)
- Type structures are bounded depth
- rustc uses FxHash for exactly this purpose
- ~2x faster than SipHash for small keys

```rust
use rustc_hash::FxHashMap;

pub struct TypeInterner {
    type_to_id: FxHashMap<Type, TypeId>,
    // ...
}
```

Hash quality for structural types is good because:
- `TypeId` is just `u32`, hashes trivially
- `Vec<TypeId>` hashes as sequence of u32s
- FxHash handles integer sequences well

## Consequences

### Positive
- O(1) type equality (just compare IDs)
- Memory efficient (no duplication)
- Simple API (`TypeId` is `Copy`)
- Fast access to common types
- Debug-friendly display
- Generation counters catch bugs (see [ADR-002](002-index-based-references.md))
- Fast hashing with FxHash

### Negative
- Must pass interner for type operations
- Extra indirection for type data
- IDs invalid after interner dropped (mitigated by generation counters)
- Substitution creates new interned types (memory growth)

## Implementation

- **TypeId**: `spl-sema/src/types.rs`
- **TypeInterner**: `spl-sema/src/types.rs`
- **Type enum**: `spl-sema/src/types.rs`
- **Substitution**: `spl-sema/src/subst.rs`
- **Display**: `spl-sema/src/types/display.rs`

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

### Debug-Friendly Display

Like `DefId` (see [ADR-002](002-index-based-references.md)), `TypeId` displays with meaningful context:

```rust
impl TypeId {
    pub fn display<'a>(&self, interner: &'a TypeInterner) -> impl fmt::Display + 'a {
        TypeIdDisplay { id: *self, interner }
    }
}

// Debug output examples:
// TypeId(0: "i32")
// TypeId(5: "(i32, bool)")
// TypeId(12: "fn(i32) -> bool")
// TypeId(18: "Vec<i32>")
// TypeId(23: "?T0")  // inference variable

impl fmt::Debug for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(interner) = TypeInterner::try_get_thread_local() {
            write!(f, "TypeId({}: {:?})", self.index, interner.display(*self))
        } else {
            write!(f, "TypeId({})", self.index)
        }
    }
}
```

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

### Type Substitution for Generics

When instantiating generic types, type parameters are substituted with concrete types:

```rust
// Generic definition: Vec<T>
// Instantiation: Vec<i32>

pub struct Substitution {
    /// Maps type parameter DefIds to concrete TypeIds
    map: FxHashMap<DefId, TypeId>,
}

impl TypeInterner {
    /// Apply substitution, returning new interned type
    pub fn subst(&mut self, ty: TypeId, subst: &Substitution) -> TypeId {
        match self.get(ty) {
            // Type parameter: look up in substitution
            Type::Param(def_id) => {
                subst.map.get(def_id).copied().unwrap_or(ty)
            }

            // Structural types: substitute recursively, intern result
            Type::Ref(mutability, inner) => {
                let inner = self.subst(inner, subst);
                self.mk_ref(mutability, inner)
            }

            Type::Array(elem, len) => {
                let elem = self.subst(elem, subst);
                self.mk_array(elem, len)
            }

            Type::Tuple(elems) => {
                let elems: Vec<_> = elems.iter()
                    .map(|&e| self.subst(e, subst))
                    .collect();
                self.mk_tuple(elems)
            }

            Type::Struct(def_id, args) => {
                let args: Vec<_> = args.iter()
                    .map(|&a| self.subst(a, subst))
                    .collect();
                self.mk_struct(def_id, args)
            }

            Type::FnPtr { params, ret } => {
                let params: Vec<_> = params.iter()
                    .map(|&p| self.subst(p, subst))
                    .collect();
                let ret = self.subst(ret, subst);
                self.mk_fn_ptr(params, ret)
            }

            // Primitives and other non-generic types: return as-is
            _ => ty,
        }
    }
}
```

**Substitution example:**

```rust
// Given: fn identity<T>(x: T) -> T
// Call:  identity(42)

// 1. Infer T = i32
// 2. Build substitution: { T -> i32 }
// 3. Substitute function type:
//    fn(T) -> T  becomes  fn(i32) -> i32

let subst = Substitution::new()
    .insert(t_param_def_id, interner.i32());

let instantiated = interner.subst(generic_fn_ty, &subst);
// instantiated is now interned as fn(i32) -> i32
```

**Memory consideration:** Each unique instantiation creates a new interned type. `Vec<i32>`, `Vec<bool>`, `Vec<String>` are three separate entries. This is expected and necessary for correct type equality.

## References

- [String interning](https://en.wikipedia.org/wiki/String_interning)
- [lasso crate](https://docs.rs/lasso/) - String interning
- [Rust Type Representation](https://rustc-dev-guide.rust-lang.org/ty.html)
- [rustc_hash (FxHash)](https://docs.rs/rustc-hash/) - Fast hasher used by rustc
- [ADR-002](002-index-based-references.md) - Index-based references pattern
