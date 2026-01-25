# spl Compiler Code Review Prompt

You are performing a comprehensive code review of spl, a programming language compiler. Your goal is to evaluate and improve the codebase to production-quality standards comparable to Rust, Go, and Swift compilers.

## Review Philosophy

- **No backwards compatibility constraints**: Breaking changes are acceptable if they improve architecture
- **Production quality bar**: Code should meet the standards of major language implementations
- **Idiomatic Rust**: Follow Rust conventions, leverage the type system, embrace zero-cost abstractions
- **Compiler-specific patterns**: Apply established compiler engineering best practices

---

## Phase 1: Architecture Review

### 1.1 Overall Structure Analysis

Examine the project structure and answer:

- Is there clear separation between compiler phases (lexing, parsing, semantic analysis, IR, codegen)?
- Are phase boundaries well-defined with clean interfaces?
- Is there a clear data flow through the compilation pipeline?
- Are errors handled consistently across phases?

### 1.2 Data Structure Design

Evaluate core data structures:

```
For each major data structure (AST, IR, Symbol Table, Type System):
- Is it the right representation for its use cases?
- Does it enable efficient traversal patterns needed by later phases?
- Is mutability minimized and controlled?
- Are invariants encoded in the type system where possible?
```

**Red flags to identify:**
- Stringly-typed data that should be enums
- `Option<T>` where the None case represents an error (should be `Result`)
- Mutable state that could be immutable
- God objects that accumulate responsibilities
- Implicit dependencies between components

### 1.3 Error Handling Architecture

Evaluate the error strategy:

- Is there a unified error type or error hierarchy?
- Do errors carry sufficient context for good diagnostics?
- Are spans/source locations preserved through transformations?
- Is error recovery implemented for better UX?
- Are internal compiler errors (ICEs) distinguished from user errors?

**Recommended pattern:**
```rust
// Errors should be rich, structured, and enable great diagnostics
pub struct Diagnostic {
    severity: Severity,
    code: DiagnosticCode,
    message: String,
    primary_span: Span,
    labels: Vec<Label>,
    notes: Vec<String>,
    suggestions: Vec<Suggestion>,
}
```

### 1.4 Interning and Arenas

Check for appropriate use of interning:

- Are identifiers/symbols interned?
- Are types interned for fast equality checks?
- Is arena allocation used for AST/IR nodes?
- Are indices used instead of pointers where appropriate?

---

## Phase 2: Idiomatic Rust Review

### 2.1 Type System Usage

Look for opportunities to leverage Rust's type system:

```
[ ] Newtypes for domain concepts (e.g., `struct TypeId(u32)` not bare `u32`)
[ ] Enums for closed sets of variants (not stringly-typed)
[ ] NonZero types where zero is invalid
[ ] PhantomData for type-level state tracking
[ ] Typestate pattern for compile-time state machine validation
[ ] Builder pattern for complex construction
```

### 2.2 Error Handling Patterns

```
[ ] `?` operator used consistently
[ ] Custom error types with thiserror or manual impl
[ ] No unwrap() in library code (except with proof of safety via comment)
[ ] No expect() with generic messages
[ ] Error context added with .context() or .with_context()
[ ] Errors are actionable and specific
```

### 2.3 API Design

Apply Rust API guidelines (https://rust-lang.github.io/api-guidelines/):

```
[ ] Functions take borrowed data when they don't need ownership
[ ] Return owned data unless lifetime extension is intentional
[ ] Use impl Trait for return types where appropriate
[ ] Iterators over collections, not collected Vecs
[ ] Into<T> for flexible input types
[ ] AsRef/AsMut for reference conversions
[ ] Default trait implemented where sensible
[ ] Debug trait on all public types
[ ] Display trait on user-facing types
```

### 2.4 Performance Patterns

```
[ ] Cow<str> for potentially-owned strings
[ ] SmallVec for small, bounded collections
[ ] IndexMap/IndexSet for deterministic iteration
[ ] Avoid allocation in hot paths
[ ] Consider &[T] over Vec<T> in function signatures
[ ] Use iterators instead of indexing where possible
```

### 2.5 Common Anti-patterns to Flag

```
- clone() to satisfy borrow checker (restructure instead)
- Rc<RefCell<T>> when ownership can be restructured  
- String when &str suffices
- Vec<T> when &[T] suffices
- Box<dyn Trait> when enum dispatch works
- Mutex when not needed for thread safety
- pub fields that should be private with accessors
```

---

## Phase 3: Compiler-Specific Patterns

### 3.1 Visitor Pattern Review

If using visitors:

- Is there a clean visitor trait?
- Are there both mutable and immutable visitors?
- Is traversal order well-defined and documented?
- Can visitors short-circuit?

**Alternative to consider:** 
```rust
// Query-based architecture (rustc-style)
trait Compiler {
    fn parse(&self, file: FileId) -> &Ast;
    fn resolve(&self, file: FileId) -> &ResolvedAst;
    fn typecheck(&self, file: FileId) -> &TypedAst;
    // Results are memoized, dependencies tracked
}
```

### 3.2 Symbol Resolution

- Is there a clear distinction between names and resolved symbols?
- Are scopes represented explicitly?
- Is shadowing handled correctly?
- Are forward references supported if needed?

### 3.3 Type System Implementation

- Is there a clear Type representation?
- How is type equality implemented (structural vs nominal)?
- Are recursive types handled (via indirection)?
- Is type inference implemented cleanly?
- Are generics/polymorphism implemented correctly?

### 3.4 IR Design

If there's an intermediate representation:

- Is it lower-level than AST but higher than target?
- Does it enable optimizations?
- Is it in SSA form (or similar)?
- Are basic blocks and control flow explicit?

---

## Phase 4: Test Coverage Analysis

### 4.1 Unit Test Coverage

For each module, verify:

```
[ ] Core logic has unit tests
[ ] Edge cases are tested
[ ] Error paths are tested
[ ] Tests are deterministic
[ ] Tests are fast
```

### 4.2 Integration Test Strategy

```
[ ] End-to-end compilation tests exist
[ ] Test programs cover language features
[ ] Error message tests (expect specific diagnostics)
[ ] Regression tests for fixed bugs
```

### 4.3 Property-Based Testing

Consider where applicable:

```rust
// Example: parsing/pretty-printing roundtrip
#[test]
fn parse_print_roundtrip() {
    proptest!(|(program in arbitrary_program())| {
        let ast = parse(&program);
        let printed = pretty_print(&ast);
        let reparsed = parse(&printed);
        assert_eq!(ast, reparsed);
    });
}
```

### 4.4 Test Infrastructure

```
[ ] Test fixtures are organized and documented
[ ] Golden/snapshot tests for complex outputs
[ ] Test utilities reduce boilerplate
[ ] CI runs all tests
[ ] Tests can be run individually
```

### 4.5 Missing Test Categories to Add

- Parser edge cases and error recovery
- Type system corner cases
- Name resolution with shadowing
- Error message quality tests
- Performance regression tests (optional but valuable)

---

## Phase 5: Documentation Review

### 5.1 Code Documentation

```
[ ] Public APIs have doc comments
[ ] Complex algorithms are explained
[ ] Module-level docs explain purpose
[ ] Examples in doc comments where helpful
[ ] Safety comments on unsafe blocks
```

### 5.2 Architecture Documentation

```
[ ] README explains project structure
[ ] ARCHITECTURE.md or similar exists
[ ] Design decisions are recorded (ADRs)
[ ] Build/development instructions are clear
```

---

## Output Format

Structure your review as:

### Executive Summary
- Overall code quality assessment (1-10)
- Top 3 architectural strengths
- Top 3 architectural concerns
- Estimated effort for improvements (low/medium/high)

### Critical Issues
Issues that should be fixed before production use:
1. [Issue]: [Why it matters] → [Recommended fix]

### Architectural Improvements
Breaking changes worth making:
1. [Current state] → [Proposed state] | Impact: [files/modules affected]

### Code Quality Findings
Organized by category:
- Type System Usage: [findings]
- Error Handling: [findings]
- API Design: [findings]
- Performance: [findings]

### Test Coverage Gaps
- [Area]: [What's missing] → [Recommended tests]

### Quick Wins
Low-effort improvements to make immediately:
1. [Change] | Files: [list] | Effort: [estimate]

### Detailed File-by-File Review
For each file with significant findings:
```
## path/to/file.rs

### Issues
- Line X: [issue] → [fix]

### Suggestions  
- [suggestion]

### Tests Needed
- [test case]
```

---

## Review Commands

Use these commands to structure the review:

```bash
# Start the review
/review start

# Review specific phase
/review architecture
/review rust-idioms
/review compiler-patterns
/review tests
/review docs

# Generate summary
/review summary

# Create issues/tasks from findings
/review create-issues
```

---

## Reference Standards

When in doubt, reference these exemplary codebases:

- **Rust compiler (rustc)**: Query-based architecture, diagnostics
- **Rust-analyzer**: Incremental compilation, IDE features
- **Cranelift**: IR design, codegen patterns
- **tree-sitter**: Parser architecture, error recovery
- **swc**: High-performance TypeScript/JavaScript compiler in Rust

---

## Final Checklist

Before concluding the review:

```
[ ] All phases reviewed
[ ] Findings prioritized by impact
[ ] Recommendations are actionable
[ ] Breaking changes identified and justified
[ ] Test coverage gaps documented
[ ] No false positives (verify issues are real)
[ ] Positive patterns also noted (not just problems)
```
