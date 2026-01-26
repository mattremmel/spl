# SPL Architecture

This document provides a high-level overview of the SPL compiler architecture. For detailed information on specific topics, see the linked documentation.

## Compilation Pipeline

```
Source Code
    │
    ▼
┌─────────────────┐
│     Lexer       │  src/lexer/
│   (Tokenize)    │
└────────┬────────┘
         │ Token stream
         ▼
┌─────────────────┐
│     Parser      │  src/parser/
│  (Build CST)    │
└────────┬────────┘
         │ Concrete Syntax Tree (CST/AST)
         ▼
┌─────────────────┐
│   Resolution    │  src/sema/resolve.rs
│  (Name lookup)  │
└────────┬────────┘
         │ ResolveResult (DefId mappings)
         ▼
┌─────────────────┐
│ Type Inference  │  src/sema/infer.rs
│  (Hindley-M.)   │
└────────┬────────┘
         │ InferResult (types + diagnostics)
         ▼
┌─────────────────┐
│  HIR Lowering   │  src/hir/
│ (Typed, simpler)│
└────────┬────────┘
         │ HirDatabase
         ▼
┌─────────────────┐
│  MIR Lowering   │  src/mir/
│   (CFG form)    │
└────────┬────────┘
         │ MIR Bodies
         ▼
┌─────────────────┐
│    Codegen      │  src/codegen/
│  (Cranelift)    │
└─────────────────┘
```

**Phase descriptions:**

| Phase | Module | Output | Description |
|-------|--------|--------|-------------|
| Lexer | `lexer/` | `SpannedToken` stream | Tokenizes source into tokens with spans |
| Parser | `parser/` | CST (`SyntaxNode`) | Builds concrete syntax tree with error recovery |
| Resolution | `sema/resolve.rs` | `ResolveResult` | Maps names to `DefId`s, builds scope tree |
| Type Inference | `sema/infer.rs` | `InferResult` | Hindley-Milner inference, unification |
| HIR Lowering | `hir/` | `HirDatabase` | Typed high-level IR, desugared |
| MIR Lowering | `mir/` | `Vec<Body>` | Control-flow graph form |
| Codegen | `codegen/` | Machine code | Cranelift backend (JIT + AOT) |

## Error Handling by Phase

Each compilation phase uses an error strategy tailored to its goals:

| Phase | Error Type | Strategy | Rationale |
|-------|------------|----------|-----------|
| Parser | `ParseError` | Recovery sets, continue parsing | IDE support, report multiple errors |
| Resolution | `Diagnostic` | Imperative collection, rich labels | User-facing messages with context |
| Type Inference | `Diagnostic` | Same as resolution | Rich error messages with spans |
| HIR Lowering | `Missing` nodes | Fallback values | Continue despite earlier errors |
| MIR Lowering | `IceError` | Result propagation | Internal compiler errors (bugs) |

**Design principles:**

- **Parser recovery**: Continues after errors to support IDE features and report multiple errors per file
- **Semantic diagnostics**: Rich context with spans, labels, and suggestions for clear error messages
- **HIR fallbacks**: Produces `HirExprKind::Missing` for invalid AST, allowing later phases to run on valid code
- **MIR ICE**: Assumes well-typed HIR; any failure indicates a compiler bug, not user error

## Key Data Structures

### DefId (`sema/symbol.rs`)

Universal identifier for definitions (functions, structs, variables, parameters, fields).

```rust
pub struct DefId(u32);  // Simple index into symbol table
```

- Assigned once during resolution, never changes
- Used in resolution maps (span → DefId), type info, HIR nodes
- ID space partitioned: user definitions, builtins, invalid sentinel

### TypeId (`sema/types.rs`)

Interned type reference for efficient comparison.

```rust
pub struct TypeId(u32);  // Index into TypeInterner
```

- Types are interned so equality is pointer comparison
- `TypeInterner` owns all type data
- `TypeVar` represents inference variables during unification

### ScopeId (`sema/scope.rs`)

Node in the scope tree for name resolution.

```rust
pub struct ScopeId(u32);  // Index into scope tree
```

- Scopes form a tree (module → function → block → nested block)
- Name lookup traverses upward through ancestors
- Inner scopes can shadow outer definitions

### Arena Allocation

HIR and MIR use `la_arena` for arena allocation:
- Nodes allocated in typed arenas
- References via `Idx<T>` (lightweight handles)
- Entire arena dropped at once (no individual deallocation)

## Module Organization

```
src/
├── lib.rs           # Public API, compile() and jit_execute()
├── diagnostic.rs    # Diagnostic type and rendering
├── session.rs       # CompileSession (multi-file compilation)
├── testing.rs       # Test utilities
│
├── lexer/           # Tokenization
│   └── mod.rs       # Lexer, Token, Span types
│
├── syntax/          # Syntax infrastructure
│   └── mod.rs       # SyntaxKind, rowan integration
│
├── parser/          # Parsing with error recovery
│   ├── mod.rs       # Parse API
│   ├── grammar.rs   # Recursive descent grammar
│   └── event.rs     # Parse event collection
│
├── ast/             # Concrete syntax tree nodes
│   └── mod.rs       # Generated AST node types
│
├── sema/            # Semantic analysis
│   ├── mod.rs       # SemanticContext
│   ├── symbol.rs    # DefId, Symbol, SymbolKind
│   ├── scope.rs     # ScopeId, Scope, ScopeKind
│   ├── types.rs     # TypeId, TypeInterner, Type
│   ├── resolve.rs   # Name resolution pass
│   └── infer.rs     # Type inference pass
│
├── hir/             # High-level IR
│   ├── mod.rs       # HIR types
│   └── lower.rs     # AST → HIR lowering
│
├── mir/             # Mid-level IR (CFG)
│   ├── mod.rs       # MIR types, Body, BasicBlock
│   └── lower.rs     # HIR → MIR lowering
│
├── codegen/         # Code generation
│   ├── mod.rs       # Codegen API
│   ├── jit.rs       # JIT compilation (Cranelift)
│   └── aot.rs       # AOT compilation + linking
│
├── package/         # Package/module system
│   └── mod.rs       # Package resolution
│
└── stdlib/          # Standard library stubs
    └── mod.rs       # Builtin functions/types
```

## Testing Strategy

- **Unit tests**: Inline with modules (`#[cfg(test)]` blocks)
- **Integration tests**: `tests/` directory
- **Snapshot testing**: `expect-test` crate for golden file tests
- **Test utilities**: `src/testing.rs` provides helpers for compiler tests

Run tests:
```bash
cargo test                     # All tests
cargo clippy --all-targets -- -D warnings  # Lint
cargo +nightly fmt             # Format
```

## Further Reading

Detailed documentation in `docs/`:

- [syntax-grammar.md](docs/syntax-grammar.md) - Language grammar specification
- [lexical-grammar.md](docs/lexical-grammar.md) - Token definitions
- [type-system.md](docs/type-system.md) - Type system design
- [memory-model.md](docs/memory-model.md) - Ownership and borrowing
- [module-system.md](docs/module-system.md) - Package/module organization
- [compilation-unit-strategy.md](docs/compilation-unit-strategy.md) - Compilation strategy
- [rust-design-lessons.md](docs/rust-design-lessons.md) - Design rationale
- [cranelift-performance-strategy.md](docs/cranelift-performance-strategy.md) - Codegen performance
