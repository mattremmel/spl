# ADR-004: Separate IR Phases (AST → HIR → MIR)

**Status:** Accepted
**Date:** 2026-01-28

## Context

Compilers transform source code through multiple representations. The choice of how many IRs to use and what each contains significantly impacts:
- Compiler complexity and maintainability
- Optimization opportunities
- Error message quality
- IDE support capabilities

A single IR throughout is simpler but limits what each phase can assume. Too many IRs adds complexity without proportional benefit.

## Decision

Use three intermediate representations:

```
Source → CST/AST → HIR → MIR → [Backend] → Native
                              ↓
                         Cranelift (default)
                         LLVM (future)
                         Interpreter (future)
                         WASM (future)
```

**MIR is the stable backend interface.** All backends lower from the same MIR representation, ensuring the frontend and backend are decoupled.

### CST/AST (Concrete Syntax Tree)
- Produced by parser
- Lossless (preserves whitespace, comments)
- Untyped, unresolved
- Good for IDE features (syntax highlighting, refactoring)

### HIR (High-level IR)
- Names resolved to `DefId`s
- Types attached to all expressions
- Desugared constructs (`while` → `loop`, etc.)
- Arena-allocated with stable IDs
- Good for type checking, high-level optimizations

### MIR (Mid-level IR)
- Control flow graph (basic blocks + terminators)
- Flat statements (no nested expressions)
- Explicit places for borrow checking
- Move vs Copy semantics explicit
- Good for borrow checking, codegen preparation

## Rationale

### Why CST (not direct AST)?
- IDE features require lossless trees (formatting, syntax highlighting)
- Error recovery is easier with concrete tokens
- Can derive typed AST views from CST

### Why Separate HIR from AST?
- AST has syntax concerns mixed with semantic
- HIR can assume names are resolved
- Desugaring simplifies later phases
- Type information available without re-inferring

### Why Separate MIR from HIR?
- HIR has nested expressions; MIR is flat
- MIR's CFG form is needed for borrow checking
- Explicit control flow simplifies codegen
- Move/Copy semantics must be explicit for ownership analysis

### Why Not More IRs?
- Three IRs (plus backend IR) is sufficient
- Each IR has clear responsibilities
- More IRs would add complexity without benefit

### Why Backend-Agnostic MIR?
- **Portability**: Same MIR works with any backend
- **Testing**: Can test MIR generation without codegen
- **Future flexibility**: Add LLVM for optimized builds, interpreter for debugging
- **Separation of concerns**: Frontend team doesn't need backend knowledge

## Consequences

### Positive
- Each IR is optimized for its purpose
- Clear phase boundaries
- IDE features work on CST/AST level
- Borrow checking works on MIR level
- Easier to test phases independently
- Backend-agnostic MIR enables multiple codegen targets

### Negative
- Multiple lowering passes to maintain
- Information must be preserved across lowerings
- More code than a single-IR design
- Must resist leaking backend concepts into MIR

## Implementation

- **CST/AST**: `spl-syntax/`, `spl-parser/`, `spl-ast/`
- **HIR**: `spl-hir/`
- **MIR**: `spl-mir/`
- **Lowering**: `spl-hir/src/lower.rs`, `spl-mir/src/lower.rs`

### IR Comparison

| Property | AST | HIR | MIR |
|----------|-----|-----|-----|
| Names | Text | `DefId` | `DefId` |
| Types | Optional | Mandatory | Mandatory |
| Nesting | Yes | Yes | No (flat) |
| Control Flow | Structured | Structured | CFG |
| Borrow Info | No | No | Yes |

### Phase Pipeline

```rust
// In spl-compiler/src/lib.rs
let parse = parser::parse(source);           // → CST
let ast = ast::SourceFile::cast(parse);      // → AST view
let resolve = sema::resolve(&ast);           // → Resolution
let infer = sema::infer(&ast, &resolve);     // → Types
let hir = hir::lower(&ast, &infer);          // → HIR
let mir = mir::lower(&hir)?;                 // → MIR (backend-agnostic)
let native = backend.compile(&mir)?;         // → Native (backend-specific)
```

### Backend Trait

```rust
pub trait Backend {
    type Output;
    type Error;

    fn compile(&self, mir: &MirProgram) -> Result<Self::Output, Self::Error>;
}

// Implementations
pub struct CraneliftBackend { /* JIT or AOT mode */ }
pub struct LlvmBackend { /* future */ }
pub struct InterpreterBackend { /* future */ }
```

MIR must not contain:
- Backend-specific type representations
- Calling convention details (abstracted via ABI enum)
- Register or stack slot references
- Backend-specific intrinsics

## References

- [Rust Compiler IRs](https://rustc-dev-guide.rust-lang.org/overview.html#intermediate-representations)
- [rust-analyzer Architecture](https://github.com/rust-lang/rust-analyzer/blob/master/docs/dev/architecture.md)
