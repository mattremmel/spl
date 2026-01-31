# SPL Language Specification

This directory contains the formal specification documents for the SPL (Simple Programming Language).

## Documents

| Document | Description |
|----------|-------------|
| [lexical-grammar.md](lexical-grammar.md) | Token definitions and lexical rules |
| [syntax-grammar.md](syntax-grammar.md) | Language grammar specification |
| [type-system.md](type-system.md) | Type system design: primitives, generics, inference |
| [traits.md](traits.md) | Trait definition, implementation, and object safety |
| [pattern-matching.md](pattern-matching.md) | Patterns, exhaustiveness, and binding modes |
| [memory-model.md](memory-model.md) | Ownership, borrowing, and lifetimes |
| [iteration.md](iteration.md) | Iteration, generators, and the IndexIter trait |
| [closures.md](closures.md) | Closure syntax and capture semantics |
| [error-handling.md](error-handling.md) | Error propagation, Try trait, and `!` operator |
| [concurrency.md](concurrency.md) | Tasks, channels, and synchronization primitives |
| [module-system.md](module-system.md) | Package and module organization |
| [attributes.md](attributes.md) | Attributes, derives, and conditional compilation |
| [unsafe.md](unsafe.md) | Unsafe operations and raw pointers |
| [ffi.md](ffi.md) | Foreign function interface (C interop) |
| [standard-library.md](standard-library.md) | Standard library overview (skeleton) |

## Related Documentation

- [Architecture Overview](../../ARCHITECTURE.md) - High-level compiler architecture
- [Architecture Decision Records](../designs/) - Design decisions and rationale
