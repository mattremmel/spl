# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SPL (Simple Programming Language) is a Rust compiler for a custom programming language. It's a library crate using Rust 2024 edition with a multi-crate workspace architecture. The compiler features a complete pipeline from source to machine code via Cranelift.

For detailed architecture, see [ARCHITECTURE.md](ARCHITECTURE.md).

## Build & Test

Always use `just` commands instead of running `cargo` directly.

```bash
just build          # Build all crates
just test           # Run all tests (unit + spec, across all crates)
just spec-tests     # Run TOML spec tests only (spl-test-runner)
just check          # Lint + all tests (full CI check)
just lint           # Clippy with warnings as errors
just fmt            # Format (requires nightly rustfmt)
just fmt-check      # Check formatting without applying
just parser-tests   # Run parser crate tests only
just spec-file <name>  # Run a specific spec test file
just test-serial    # Single-threaded tests (for debugging panics)
just clean          # Clean build artifacts
```

## Architecture Overview

```
Source → Lexer → Parser → Resolution → Type Inference → HIR → MIR → Codegen
```

| Phase | Crate | Output |
|-------|-------|--------|
| Lexer | `spl-lexer` | Token stream with spans |
| Parser | `spl-parser` | CST with error recovery |
| Resolution | `spl-sema` | Name → DefId mappings |
| Type Inference | `spl-sema` | Hindley-Milner types |
| HIR Lowering | `spl-hir` | Typed high-level IR |
| MIR Lowering | `spl-mir` | Control-flow graph |
| Codegen | `spl-codegen` | Cranelift JIT/AOT |

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed phase descriptions and data structures.

## Crate Organization

### Current Crates

| Crate | Responsibility |
|-------|----------------|
| `spl-lexer` | Tokenization with Unicode identifier support |
| `spl-syntax` | Syntax infrastructure (rowan-based CST) |
| `spl-parser` | Recursive descent parser with error recovery |
| `spl-ast` | CST node type definitions |
| `spl-diagnostic` | Error types, spans, and rendering |
| `spl-sema` | Name resolution + Hindley-Milner type inference |
| `spl-hir` | High-level typed IR |
| `spl-mir` | Mid-level IR (CFG form) |
| `spl-codegen` | Cranelift backend (JIT + AOT) |
| `spl-package` | Package/module system |
| `spl-compiler` | Integration crate, public API |
| `spl-test-runner` | TOML-based test framework |

### Planned Crates

| Crate | Responsibility |
|-------|----------------|
| `spl-cli` | Command-line interface |
| `spl-ui-tests` | UI/diagnostic test framework |

## Testing

### Current Testing Infrastructure

**Unit tests**: Inline `#[cfg(test)]` modules throughout crates

**Integration tests**: `crates/spl-compiler/tests/`

**TOML test runner** (`spl-test-runner`): Declarative tests with modes:
- `run-pass` - Compiles and runs successfully
- `run-fail` - Compiles but fails at runtime
- `compile-pass` - Compiles without errors
- `compile-fail` - Fails to compile (with expected error)
- `load-pass` - Package loads successfully
- `load-fail` - Package fails to load

Example test file:
```toml
mode = "run-pass"
expected_output = "42"

[source]
main = """
fn main() -> i32 {
    42
}
"""
```

### Future Testing Goals

**TOML-based spec tests**: Link test cases to spec paragraphs for traceability
```toml
mode = "compile-fail"
spec_ref = "type-system.md#3.2.1"  # Links to spec paragraph
```

**UI/diagnostic tests**: Migrate inline diagnostic tests to standalone TOML format with expected diagnostic output snapshots

## Desired CLI

The `spl-cli` crate should provide:

```bash
# Compilation
spl source.spl -o output           # Compile to executable
spl --jit source.spl               # JIT execute

# Emit intermediate representations
spl --emit tokens source.spl       # Token stream
spl --emit ast source.spl          # Parse tree (CST)
spl --emit hir source.spl          # High-level IR
spl --emit mir source.spl          # Mid-level IR
spl --emit asm source.spl          # Assembly

# Debugging/profiling
spl --time-passes source.spl       # Per-pass timing
spl --log-level=debug source.spl   # Verbosity control
RUST_LOG=debug spl source.spl      # Environment variable
spl --log-format=json ...          # JSON output for tooling
```

## Structured Logging

SPL uses the `tracing` crate for structured logging, following the "wide events" philosophy. This means:

- **Canonical log lines** - One rich, structured log per operation containing all debugging context
- **Structured format** - Key-value pairs instead of plain strings for queryability
- **High-cardinality data** - Include contextual data like function names, counts, sizes
- **Zero-cost when off** - Tracing has no overhead when no subscriber is active; can be compile-time disabled for production via feature flags

### Using the Logging

```bash
# Normal compilation (no logging by default)
spl source.spl -o output

# Show timing per pass
spl --time-passes source.spl -o output

# Enable debug logging
spl --log-level=debug source.spl -o output
RUST_LOG=debug spl source.spl -o output

# JSON format for tooling integration
spl --log-format=json --log-level=debug source.spl -o output

# Filter to specific module
RUST_LOG=spl_sema::resolver=trace spl source.spl -o output
```

### Adding Instrumentation

Each compilation pass should have a tracing span wrapping the work:

```rust
use tracing::{info_span, info};

pub fn my_pass(input: &Input) -> Result<Output> {
    // Create a span for the pass - includes timing automatically
    let _span = info_span!("my_pass").entered();

    // Do the work...
    let result = process(input)?;

    // Log completion with useful metrics
    info!(
        item_count = result.items.len(),
        "pass complete"
    );

    Ok(result)
}
```

### Logging Levels

| Level | Use for | Example |
|-------|---------|---------|
| `error` | Compilation failures, internal compiler errors | ICE, unrecoverable errors |
| `warn` | Suspicious patterns (surfaced via diagnostics) | Deprecated feature usage |
| `info` | Per-pass completion with summary metrics | "lexing complete", token counts |
| `debug` | Decision points, intermediate state | "resolving symbol X to Y" |
| `trace` | Detailed internal state, individual instructions | Instruction-by-instruction output |

### Good vs Bad Examples

**Good:** Wide event with context
```rust
let _span = info_span!(
    "codegen",
    arch = "x86_64",
    function_count = functions.len()
).entered();

// ... do code generation ...

info!(
    code_bytes = total_bytes,
    "code generation complete"
);
```

**Bad:** Scattered debug statements
```rust
println!("Starting codegen...");
for func in functions {
    println!("Generating function: {:?}", func.name);
}
println!("Done!");
```

**Good:** Structured key-value data
```rust
info!(
    token_count = tokens.len(),
    source_bytes = source.len(),
    "lexing complete"
);
```

**Bad:** String interpolation
```rust
println!("Lexed {} tokens from {} bytes", tokens.len(), source.len());
```

### Key Principles

- **Spans for timing**: Wrap passes in `info_span!()` — this enables `--time-passes`
- **Events for outcomes**: Use `info!()` after completing work with metrics
- **Context in spans**: Include high-level context (file, function count) in span fields
- **Metrics in events**: Include computed metrics (instruction counts, sizes) in events
- **Zero-cost when off**: Tracing has no overhead when no subscriber is active
- **Feature-gated**: Use `tracing` feature flag so tracing can be compiled out entirely for production builds if needed for performance

## Language Modification Checklist

When adding or modifying language features:

1. **Specification**: Update `docs/spec/` documents
2. **Lexer** (`spl-lexer`): Add new tokens if needed
3. **Parser** (`spl-parser`): Add grammar rules
4. **AST** (`spl-ast`): Add CST node types
5. **Resolution** (`spl-sema`): Update name resolution
6. **Type Inference** (`spl-sema`): Update type rules
7. **HIR** (`spl-hir`): Add HIR node variants
8. **MIR** (`spl-mir`): Add MIR lowering
9. **Codegen** (`spl-codegen`): Implement code generation
10. **Tests**: Add spec tests + diagnostic tests

## Issue Tracking

This project uses beads (`bd`) for issue tracking:

```bash
bd ready                           # Find available work
bd create --title="..." --type=task --priority=2
bd update <id> --status=in_progress
bd close <id>
bd sync                            # Sync with remote
```

## Further Reading

### Language Specification
- `docs/spec/` - Language specification (15 documents)
  - [syntax-grammar.md](docs/spec/syntax-grammar.md) - Grammar rules
  - [lexical-grammar.md](docs/spec/lexical-grammar.md) - Token definitions
  - [type-system.md](docs/spec/type-system.md) - Type system design
  - [memory-model.md](docs/spec/memory-model.md) - Ownership and borrowing

### Architecture & Design
- [ARCHITECTURE.md](ARCHITECTURE.md) - Detailed compiler architecture
- `docs/designs/` - Architecture Decision Records (15 ADRs)
- [docs/DECISIONS.md](docs/DECISIONS.md) - Design decisions summary
