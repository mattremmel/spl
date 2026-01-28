# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records documenting key design decisions in the SPL compiler.

## Index

| # | Title | Status | Summary |
|---|-------|--------|---------|
| [001](001-phase-specific-error-handling.md) | Phase-Specific Error Handling | Accepted | Different error strategies per compilation phase |
| [002](002-index-based-references.md) | Index-Based References | Accepted | `DefId` and `TypeId` as lightweight handles |
| [003](003-arena-allocation.md) | Arena Allocation | Accepted | `la_arena` for HIR/MIR nodes |
| [004](004-separate-ir-phases.md) | Separate IR Phases | Accepted | AST → HIR → MIR pipeline |
| [005](005-cranelift-backend.md) | Cranelift Backend | Accepted | JIT + AOT via Cranelift |
| [006](006-linker-abstraction.md) | Linker Abstraction | Accepted | Trait-based linker for AOT |
| [007](007-toml-based-test-framework.md) | TOML-Based Test Framework | Accepted | Declarative test configuration |
| [008](008-rowan-based-cst.md) | Rowan-Based CST | Accepted | Lossless syntax trees |
| [009](009-event-based-parser-recovery.md) | Event-Based Parser with Recovery | Accepted | Error recovery for IDE support |
| [010](010-type-interning.md) | Type Interning | Accepted | Efficient type equality via interning |

## ADR Template

When creating new ADRs, use this template:

```markdown
# ADR-XXX: [Title]

**Status:** Proposed | Accepted | Deprecated | Superseded
**Date:** YYYY-MM-DD

## Context

[Background, problem statement, constraints]

## Decision

[What was decided]

## Rationale

[Why this decision was made, alternatives considered, trade-offs]

## Consequences

### Positive
- [Benefits]

### Negative
- [Drawbacks]

## Implementation

[Key files and how the decision is reflected in code]

## References

- [Links to related docs, external resources]
```

## Related Documentation

- [Architecture Overview](../../ARCHITECTURE.md) - High-level compiler architecture
- [Language Specification](../spec/) - SPL language specification
