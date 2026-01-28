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

### Test Directory Structure

```
cases/
├── run-pass/
│   ├── arithmetic.toml
│   ├── control-flow.toml
│   └── ...
├── compile-fail/
│   ├── undefined-variable.toml
│   └── ...
└── packages/
    ├── simple/
    │   ├── test.toml
    │   └── main.spl
    └── ...
```

### Example Test Files

**run-pass/fibonacci.toml**
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

**compile-fail/type-mismatch.toml**
```toml
mode = "compile-fail"

[expect.compile]
error = "type mismatch"

[source]
inline = "fn main() { let x: i32 = true; }"
```

## References

- [TOML Specification](https://toml.io/)
- [Rust `toml` crate](https://docs.rs/toml/)
- [rustc ui tests](https://rustc-dev-guide.rust-lang.org/tests/ui.html)
