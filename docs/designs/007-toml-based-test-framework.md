# ADR-007: TOML-Based Test Framework

**Status:** Accepted
**Date:** 2026-01-28

## Context

Compiler testing requires many test cases covering:
- Parse success/failure
- Type checking success/failure
- Runtime behavior (exit codes, output)
- Package loading

Traditional approaches:
- **Inline tests**: Limited, can't test compilation failures
- **Snapshot tests**: Good for output, awkward for expected failures
- **Custom DSL**: Learning curve, maintenance burden

We need a test framework that:
- Clearly expresses test intent
- Supports multiple test modes
- Is easy to read and write
- Can test both success and failure cases

## Decision

Use TOML configuration files for test cases:

```toml
mode = "run-pass"
ignore = false

[expect.run]
return = 42

[source]
inline = "fn main(): i32 { 42 }"
```

### Test Modes

| Mode | Description |
|------|-------------|
| `load-pass` | Package should load successfully |
| `load-fail` | Package should fail to load |
| `compile-pass` | Code should compile successfully |
| `compile-fail` | Code should fail to compile |
| `run-pass` | Code should compile and run successfully |
| `run-fail` | Code should compile but fail at runtime |

### Expectations

```toml
[expect.package]
items = 2         # Number of top-level items
files = 1         # Number of source files
name = "my_pkg"   # Package name
modules = 3       # Number of modules

[expect.compile]
error = "cannot find"  # Expected error substring

[expect.run]
return = 42            # Expected exit code
stdout = "hello"       # Expected stdout
```

## Rationale

### Why TOML?
- **Human-readable**: Easy to write and review
- **Standard format**: Well-known, good tooling
- **Typed values**: Numbers, strings, booleans
- **Hierarchical**: Natural for nested expectations
- **Comments**: Can document test intent

### Why Not Custom DSL?
- Learning curve for contributors
- Parser/lexer maintenance burden
- TOML tooling already exists

### Why Not Inline Attributes?
- Can't test compilation failures easily
- Mixes test config with source code
- Hard to test multi-file scenarios

### Why Separate Source Section?
- Clear separation of config and code
- Supports both inline and file-based sources
- Easy to add new source options (multiple files, etc.)

## Consequences

### Positive
- Clear, declarative test configuration
- Easy to add new test cases
- Self-documenting tests
- Supports all test scenarios
- Standard format with good tooling

### Negative
- Extra file per test (or shared config)
- Parsing overhead (minor)
- Must learn TOML syntax

## Implementation

- **Config parsing**: `spl-test-runner/src/config.rs`
- **Test execution**: `spl-test-runner/src/runner.rs`
- **Process execution**: `spl-test-runner/src/executor.rs`
- **Test cases**: `crates/spl-test-runner/cases/`

### Test Directory Structure (Feature-Based)

Tests are organized by language feature, not by test mode. Each feature directory contains all related tests (success and failure cases together):

```
cases/
├── arithmetic/
│   ├── basic.toml           # mode = "run-pass"
│   ├── overflow.toml        # mode = "run-fail"
│   └── type-mismatch.toml   # mode = "compile-fail"
├── functions/
│   ├── basic.toml           # mode = "run-pass"
│   ├── recursion.toml       # mode = "run-pass"
│   ├── missing-return.toml  # mode = "compile-fail"
│   └── arity-mismatch.toml  # mode = "compile-fail"
├── control-flow/
│   ├── if-else.toml
│   ├── while-loop.toml
│   └── unreachable.toml
├── types/
│   ├── inference.toml
│   ├── structs.toml
│   └── generics.toml
└── packages/
    ├── simple/
    │   ├── test.toml
    │   └── main.spl
    └── multi-file/
        ├── test.toml
        ├── main.spl
        └── lib.spl
```

### Why Feature-Based Organization?

- **Discoverability**: All tests for a feature are in one place
- **Coverage visibility**: Easy to see if a feature has both positive and negative tests
- **Parallel development**: Contributors can own feature directories
- **Natural grouping**: Matches how features are documented and discussed

### Example Test Files

**functions/recursion.toml**
```toml
mode = "run-pass"

[expect.run]
return = 55

[source]
inline = """
fn fib(n: i32): i32 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main(): i32 { fib(10) }
"""
```

**types/type-mismatch.toml**
```toml
mode = "compile-fail"

[expect.compile]
error = "type mismatch"

[source]
inline = "fn main() { let x: i32 = true; }"
```

**arithmetic/overflow.toml**
```toml
mode = "run-fail"

[expect.run]
# Program should panic/abort on overflow in debug mode

[source]
inline = """
fn main(): i32 {
    let x: i32 = 2147483647;
    x + 1  // overflow
}
"""
```

## References

- [TOML Specification](https://toml.io/)
- [Rust `toml` crate](https://docs.rs/toml/)
- [rustc ui tests](https://rustc-dev-guide.rust-lang.org/tests/ui.html)
