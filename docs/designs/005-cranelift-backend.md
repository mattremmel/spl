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

Use Cranelift as the initial backend implementation (see [ADR-004](004-separate-ir-phases.md) for the `Backend` trait):

```rust
impl Backend for CraneliftBackend {
    type Output = CompiledModule;  // JIT or object file
    type Error = CodegenError;

    fn compile(&self, mir: &MirProgram) -> Result<Self::Output, Self::Error>;
}
```

```
MIR Bodies → Cranelift IR → Machine Code
```

Cranelift provides:
- **JIT compilation** via `cranelift-jit` for immediate execution
- **AOT compilation** via `cranelift-object` for object files
- **Debug info** via DWARF generation
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

### Platform Support

SPL supports whatever platforms Cranelift supports. As of writing:
- x86_64 (Linux, macOS, Windows)
- AArch64 (Linux, macOS)
- RISC-V (Linux)
- s390x (Linux)

No explicit target restrictions—if Cranelift can generate code for it, SPL can target it.

### Debug Info

DWARF debug info enables source-level debugging with GDB/LLDB:
- Source locations mapped to machine code
- Variable names and scopes
- Type information for structured data
- Stack unwinding for backtraces

Cranelift's `cranelift-object` crate supports DWARF emission. Wire-up requires:
- Tracking source spans through MIR to codegen
- Emitting DIEs (Debug Info Entries) for functions, variables, types
- Line number tables mapping instructions to source

**Windows note**: Native Windows debugging (Visual Studio) uses PDB format. DWARF still works on Windows with GDB/LLDB, which is sufficient for initial support. PDB could be added later if needed.

## Consequences

### Positive
- Fast compile times for development
- Single codebase for JIT and AOT
- Good platform support (defers to Cranelift)
- Straightforward Rust API
- Well-documented IR
- DWARF support for debugging
- Implements Backend trait for clean separation (see [ADR-004](004-separate-ir-phases.md))

### Negative
- Code quality ~80-90% of LLVM-optimized
- Fewer optimization passes
- Smaller community than LLVM
- May need LLVM backend eventually for production optimization
- No native Windows debug info (PDB) initially

## Implementation

- **Backend trait impl**: `spl-codegen/src/cranelift/mod.rs` - `CraneliftBackend`
- **Core codegen**: `spl-codegen/src/cranelift/lower.rs` - MIR → Cranelift IR
- **JIT module**: `spl-codegen/src/cranelift/jit.rs` - `JitCompiler`
- **AOT module**: `spl-codegen/src/cranelift/aot.rs` - `AotCompiler`
- **Type mapping**: `spl-codegen/src/cranelift/types.rs` - SPL types → Cranelift types
- **Debug info**: `spl-codegen/src/cranelift/dwarf.rs` - DWARF generation
- **Linker**: `spl-codegen/src/link.rs` - Object file → executable (see [ADR-006](006-linker-abstraction.md))

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
