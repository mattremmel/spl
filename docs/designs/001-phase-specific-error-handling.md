# ADR-001: Phase-Specific Error Handling

**Status:** Accepted
**Date:** 2026-01-28

## Context

Compilers need robust error handling that serves multiple goals:
- Report multiple errors in a single pass (better UX)
- Continue compilation despite errors (IDE support)
- Distinguish user errors from compiler bugs
- Provide rich diagnostic context (spans, labels, suggestions)
- Offer state-of-the-art error messages with actionable fixes
- Detect common mistakes from other languages (C, Python, JavaScript, etc.)

A one-size-fits-all error strategy doesn't work well because different compilation phases have different needs and constraints.

## Decision

Each compilation phase uses an error handling strategy tailored to its specific goals:

| Phase | Error Type | Strategy | Rationale |
|-------|------------|----------|-----------|
| Parser | `ParseError` | Recovery sets, event collection | IDE support, partial results |
| Diagnostic Enrichment | `Diagnostic` | ParseError → rich Diagnostic | Add suggestions, detect other-language syntax |
| Sema | `Diagnostic` | Imperative collection, builder | Rich user-facing messages |
| HIR Lowering | `Missing` nodes | Fallback values | Continue despite earlier errors |
| MIR Lowering | `IceError` | Result propagation | Structured ICE reporting |

### Error Codes

All user-facing errors have stable error codes with category prefixes:

```
P0001: unexpected token
P0002: unclosed delimiter
P0003: invalid character in identifier

R0001: undefined variable
R0002: undefined function
R0003: ambiguous import

T0001: type mismatch
T0002: cannot infer type
T0003: missing struct field

B0001: use of moved value
B0002: cannot borrow as mutable
B0003: lifetime may not live long enough

W0001: unused variable
W0002: unreachable code
```

Error codes are:
- **Prefixed by category**: P=Parse, R=Resolution, T=Type, B=Borrow, W=Warning
- **Stable**: Once assigned, never reused or renumbered
- **Documented**: Each code has a dedicated explanation page
- **Searchable**: Users can search "SPL T0001" for help
- **Self-documenting**: Prefix tells you the error category at a glance

## Rationale

### Parser Recovery
The parser continues after errors to support IDE features (syntax highlighting, code navigation) and to report multiple errors per file. It uses recovery sets to find synchronization points where parsing can resume. Parse errors are lightweight (`ParseError`) during parsing:
- Parser is self-contained and reusable
- Parse errors capture location and error kind
- Rich diagnostics added in enrichment phase

### Diagnostic Enrichment
A post-parse phase converts `ParseError` into rich `Diagnostic` with:
- **Suggestions**: "did you mean `;`?", "add missing `}`"
- **Other-language detection**: Recognize syntax from C, Python, JavaScript, Go, etc.
- **Context-aware hints**: Based on surrounding valid syntax

Example other-language detections:
```
// C-style for loop
for (int i = 0; i < 10; i++) { }
     ^^^ SPL uses `for i in 0..10 { }` syntax

// Python-style def
def foo():
^^^ SPL uses `fn` for function definitions

// JavaScript arrow function
let f = (x) => x + 1;
            ^^ SPL uses `|x| x + 1` for closures

// Go-style error handling
if err != nil { }
       ^^ SPL uses `?` operator or `match` for error handling
```

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
- State-of-the-art error messages with suggestions
- Helps users migrating from other languages
- Stable error codes enable documentation and search

### Negative
- Multiple error types to maintain
- Conversion needed between error types at phase boundaries
- Complexity in understanding which error type to use where
- Enrichment phase adds complexity but enables rich UX
- Must maintain other-language syntax detection patterns

## Implementation

- **Parser errors**: `spl-parser/src/event.rs` - `ParseError`
- **Diagnostic enrichment**: `spl-diagnostic/src/enrich.rs` - `enrich_parse_errors()`
- **Other-language patterns**: `spl-diagnostic/src/other_lang.rs` - syntax detection
- **Semantic diagnostics**: `spl-diagnostic/src/lib.rs` - `Diagnostic`
- **Error codes**: `spl-diagnostic/src/codes.rs` - `ErrorCode` enum
- **HIR fallbacks**: `spl-hir/src/expr.rs` - `HirExprKind::Missing`
- **MIR ICE errors**: `spl-mir/src/error.rs` - `IceError`
- **Error flow documentation**: `spl-compiler/src/lib.rs` (module-level docs)

### Diagnostic Structure

```rust
pub struct Diagnostic {
    pub code: ErrorCode,           // E0001, E0101, etc.
    pub severity: Severity,        // Error, Warning, Note
    pub message: String,           // Primary message
    pub span: Span,                // Source location
    pub labels: Vec<Label>,        // Secondary spans with messages
    pub suggestions: Vec<Suggestion>, // Actionable fixes
    pub notes: Vec<String>,        // Additional context
}

pub struct Suggestion {
    pub message: String,           // "try using `fn` instead"
    pub span: Span,                // What to replace
    pub replacement: String,       // The fix
    pub applicability: Applicability, // MachineApplicable, MaybeIncorrect, etc.
}
```

### Error Code Prefixes

| Prefix | Category | Example |
|--------|----------|---------|
| P | Parse errors | P0001: unexpected token |
| R | Name resolution | R0001: undefined variable |
| T | Type checking | T0001: type mismatch |
| B | Borrow checking | B0001: use of moved value |
| C | Code generation | C0001: unsupported target |
| W | Warnings | W0001: unused variable |

```rust
pub enum ErrorCode {
    // Parse errors
    P0001, // unexpected token
    P0002, // unclosed delimiter
    // ...

    // Resolution errors
    R0001, // undefined variable
    R0002, // undefined function
    // ...

    // Type errors
    T0001, // type mismatch
    // ...
}
```

## References

- [Rust Compiler Error Handling](https://rustc-dev-guide.rust-lang.org/diagnostics.html)
- [Rust Error Index](https://doc.rust-lang.org/error_codes/error-index.html) - Stable error code documentation
- [Error Recovery in Language Servers](https://microsoft.github.io/language-server-protocol/)
- [Elm Error Messages](https://elm-lang.org/news/compiler-errors-for-humans) - Inspiration for friendly errors
