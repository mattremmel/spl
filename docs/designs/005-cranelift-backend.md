# ADR-005: Cranelift Backend (JIT + AOT)

**Status:** Accepted
**Date:** 2026-01-28

## Context

SPL needs a code generation backend that:
- Supports both JIT (for REPL, testing) and AOT (for executables)
- Generates reasonably fast code
- Has fast compile times
- Works on multiple platforms (x86_64, AArch64)
- Is well-maintained and documented

Alternatives considered:
- **LLVM**: Industry standard, best optimization, but slow compile times
- **QBE**: Simple, fast, but limited platform support
- **Custom backend**: Maximum control, but enormous effort

## Decision

Use Cranelift for all native code generation:

```rust
MIR Bodies → Cranelift IR → Machine Code
```

Cranelift provides:
- **JIT compilation** via `cranelift-jit` for immediate execution
- **AOT compilation** via `cranelift-object` for object files
- Same IR for both modes (code reuse)

## Rationale

### Why Cranelift?
- **Fast compile times**: Designed for JIT, ~10x faster than LLVM
- **Good code quality**: Not LLVM-level, but sufficient for most cases
- **Rust-native**: Written in Rust, good API ergonomics
- **Active development**: Backed by Bytecode Alliance, used by Wasmtime
- **JIT + AOT**: Same crate family supports both use cases

### Why Not LLVM?
- Slow compile times hurt development iteration
- Complex C++ API, harder to integrate
- Overkill for a learning/hobby compiler
- SPL can add LLVM later if needed for -O3 optimization

### Why Not QBE?
- Limited platform support (mainly x86_64 Linux)
- Less active development
- Would need separate JIT solution

### JIT vs AOT Mode Selection

| Use Case | Mode | Implementation |
|----------|------|----------------|
| REPL, tests | JIT | `cranelift-jit` with `JITModule` |
| Executables | AOT | `cranelift-object` + system linker |

## Consequences

### Positive
- Fast compile times for development
- Single codebase for JIT and AOT
- Good platform support (x86_64, AArch64)
- Straightforward Rust API
- Well-documented IR

### Negative
- Code quality ~80-90% of LLVM-optimized
- Fewer optimization passes
- Smaller community than LLVM
- May need LLVM backend eventually for production optimization

## Implementation

- **Core codegen**: `spl-codegen/src/lower.rs` - MIR → Cranelift IR
- **JIT module**: `spl-codegen/src/module.rs` - `ModuleCompiler`
- **AOT module**: `spl-codegen/src/aot.rs` - `AotModuleCompiler`
- **Type mapping**: `spl-codegen/src/types.rs` - SPL types → Cranelift types
- **Linker**: `spl-codegen/src/link.rs` - Object file → executable

### Compilation Pipeline

```rust
// JIT path
let module = codegen_jit(&bodies, &types)?;
let result = module.run_main()?;

// AOT path
let object = AotModuleCompiler::compile(&bodies, &types)?;
link_object_to_executable(&object.into_bytes(), &output_path, None)?;
```

### Key Cranelift Concepts

| Cranelift | SPL Equivalent |
|-----------|----------------|
| `FunctionBuilder` | MIR `Body` lowering context |
| `Block` | MIR `BasicBlock` |
| `Value` | MIR `Local` / `Operand` |
| `Inst` | MIR `Statement` |
| `Type` | SPL `TypeId` mapped to Cranelift type |

## References

- [Cranelift Documentation](https://cranelift.dev/)
- [Cranelift IR Reference](https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/ir.md)
- [Wasmtime](https://wasmtime.dev/) - Major Cranelift user
