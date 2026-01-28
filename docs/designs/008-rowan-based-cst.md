# ADR-008: Rowan-Based CST

**Status:** Accepted
**Date:** 2026-01-28

## Context

The parser needs to produce a syntax tree that:
- Is lossless (preserves whitespace, comments)
- Supports incremental re-parsing
- Enables IDE features (go-to-definition, syntax highlighting)
- Can be traversed efficiently
- Works with error recovery

Traditional AST approaches:
- **Direct AST**: Lossy, can't round-trip source
- **Hand-rolled CST**: Significant implementation effort
- **Existing library**: Proven design, less code to maintain

## Decision

Use the `rowan` crate for concrete syntax tree infrastructure:

```rust
use rowan::{GreenNode, SyntaxNode};

pub enum Lang {}
impl rowan::Language for Lang {
    type Kind = SyntaxKind;
    // ...
}

pub type SyntaxNode = rowan::SyntaxNode<Lang>;
```

Key properties:
- **Lossless**: All source bytes represented in tree
- **Green/Red tree**: Immutable green tree, on-demand red tree
- **Typed nodes**: `SyntaxKind` enum for all tokens and nodes
- **Arc-based sharing**: Incremental reparsing via subtree sharing

## Rationale

### Why Rowan?
- **Proven**: Used by rust-analyzer (production-quality)
- **Lossless**: Essential for IDE features
- **Efficient**: Green tree sharing reduces memory
- **Ergonomic**: Nice API for traversal and mutation
- **Rust-native**: Good integration with ecosystem

### Why Lossless?
- **Formatting**: Can reprint source exactly
- **IDE features**: Syntax highlighting needs comments/whitespace
- **Refactoring**: Need to preserve formatting when editing
- **Error recovery**: Can show context around errors

### Why Green/Red Tree?
- **Green tree**: Immutable, shared, efficient storage
- **Red tree**: On-demand, with parent pointers for traversal
- **Incremental**: Changed subtrees get new green nodes, rest shared

### Why Not Lossy AST?
- Can't implement formatters
- Can't show good error context
- IDE features require full source info

## Consequences

### Positive
- Lossless: all IDE features possible
- Efficient: subtree sharing for large files
- Battle-tested: rust-analyzer proves the design
- Type-safe: `SyntaxKind` enum catches bugs at compile time

### Negative
- Learning curve for green/red tree concepts
- More complex than simple AST
- Slightly higher memory usage (storing trivia)

## Implementation

- **SyntaxKind enum**: `spl-syntax/src/lib.rs`
- **Language trait**: `spl-syntax/src/lib.rs`
- **Parser integration**: `spl-parser/src/sink.rs`

### SyntaxKind Design

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // Tokens
    LET_KW,
    IDENT,
    WHITESPACE,
    COMMENT,
    ERROR,
    // ...

    // Composite nodes
    SourceFile,
    FunctionDef,
    Block,
    LetStmt,
    // ...
}
```

### Tree Structure Example

```
SourceFile@0..23
  FunctionDef@0..12
    FN_KW@0..2 "fn"
    WHITESPACE@2..3 " "
    Name@3..7
      IDENT@3..7 "main"
    ParamList@7..9
      L_PAREN@7..8 "("
      R_PAREN@8..9 ")"
    Block@9..12
      WHITESPACE@9..10 " "
      L_BRACE@10..11 "{"
      R_BRACE@11..12 "}"
```

### Typed AST Views

CST provides untyped tree; typed views cast nodes:

```rust
pub struct FnDef(SyntaxNode);

impl FnDef {
    pub fn name(&self) -> Option<Name> {
        self.0.children().find_map(Name::cast)
    }

    pub fn body(&self) -> Option<Block> {
        self.0.children().find_map(Block::cast)
    }
}
```

## References

- [rowan crate](https://docs.rs/rowan/)
- [rust-analyzer syntax](https://github.com/rust-lang/rust-analyzer/tree/master/crates/syntax)
- [Red-green trees](https://ericlippert.com/2012/06/08/red-green-trees/)
