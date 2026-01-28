# ADR-001: Phase-Specific Error Handling

**Status:** Accepted
**Date:** 2026-01-28

## Context

Compilers need robust error handling that serves multiple goals:
- Report multiple errors in a single pass (better UX)
- Continue compilation despite errors (IDE support)
- Distinguish user errors from compiler bugs
- Provide rich diagnostic context (spans, labels, suggestions)

A one-size-fits-all error strategy doesn't work well because different compilation phases have different needs and constraints.

## Decision

Each compilation phase uses an error handling strategy tailored to its specific goals:

| Phase | Error Type | Strategy | Rationale |
|-------|------------|----------|-----------|
| Parser | `ParseError` | Recovery sets, event collection | IDE support, partial results |
| Sema | `Diagnostic` | Imperative collection, builder | Rich user-facing messages |
| HIR Lowering | `Missing` nodes | Fallback values | Continue despite earlier errors |
| MIR Lowering | `IceError` | Result propagation | Structured ICE reporting |

## Rationale

### Parser Recovery
The parser continues after errors to support IDE features (syntax highlighting, code navigation) and to report multiple errors per file. It uses recovery sets to find synchronization points where parsing can resume. Parse errors are lightweight (`ParseError`) rather than rich diagnostics because:
- Parser is self-contained and reusable
- Parse errors are structural, not semantic
- No allocation overhead for label vectors

### Semantic Diagnostics
Name resolution and type inference produce `Diagnostic` with rich context (spans, labels, suggestions). Errors are collected imperatively as analysis proceeds, allowing the compiler to report all errors found rather than stopping at the first.

### HIR Fallbacks
When lowering encounters missing or malformed AST nodes (from earlier errors), it produces `HirExprKind::Missing` or error types rather than failing. This allows MIR lowering and codegen to proceed on valid portions of code.

### MIR ICE Errors
MIR lowering assumes valid, well-typed HIR. Any invariant violation indicates a compiler bug, not user error. `IceError` provides structured context (spans, `DefId`s) for debugging, and includes instructions for reporting the bug.

## Consequences

### Positive
- Each phase is optimized for its specific needs
- IDE features work even with syntax errors
- Users see all errors, not just the first
- Compiler bugs are clearly distinguished from user errors
- Rich diagnostic context aids debugging

### Negative
- Multiple error types to maintain
- Conversion needed between error types at phase boundaries
- Complexity in understanding which error type to use where

## Implementation

- **Parser errors**: `spl-parser/src/event.rs` - `ParseError`
- **Semantic diagnostics**: `spl-diagnostic/src/lib.rs` - `Diagnostic`
- **HIR fallbacks**: `spl-hir/src/expr.rs` - `HirExprKind::Missing`
- **MIR ICE errors**: `spl-mir/src/error.rs` - `IceError`
- **Error flow documentation**: `spl-compiler/src/lib.rs` (module-level docs)

## References

- [Rust Compiler Error Handling](https://rustc-dev-guide.rust-lang.org/diagnostics.html)
- [Error Recovery in Language Servers](https://microsoft.github.io/language-server-protocol/)
