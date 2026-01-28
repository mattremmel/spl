# ADR-009: Event-Based Parser with Recovery

**Status:** Accepted
**Date:** 2026-01-28

## Context

The parser must:
- Produce lossless CST (for IDE features)
- Report multiple errors per file
- Continue parsing after errors
- Be fast and memory-efficient
- Support IDE use cases (partial parses)

Traditional recursive descent:
- Directly builds tree nodes
- Hard to insert error recovery
- Tight coupling between parsing and tree building

## Decision

Use an event-based parser architecture:

```rust
enum Event {
    Start { kind: SyntaxKind, forward_parent: Option<usize> },
    Token { kind: SyntaxKind, n_raw_tokens: u8 },
    Finish,
    Error(ParseError),
    Placeholder,
}
```

Parsing produces events; a separate sink converts events to green tree:

```
Source → Tokens → Parser → Events → Sink → GreenNode
```

### Error Recovery

Recovery sets define synchronization points:

```rust
const ITEM_RECOVERY_SET: &[SyntaxKind] = &[
    FN_KW, STRUCT_KW, TYPE_KW, IMPL_KW, PUB_KW, USE_KW,
];

const STMT_RECOVERY_SET: &[SyntaxKind] = &[
    LET_KW, IF_KW, WHILE_KW, FOR_KW, RETURN_KW,
    L_BRACE, R_BRACE, SEMI,
];
```

On error:
1. Emit error event
2. Skip tokens until recovery set or EOF
3. Wrap skipped tokens in `ERROR` node
4. Resume normal parsing

## Rationale

### Why Event-Based?

**Decoupling**: Parser logic separate from tree construction
- Easier to test parser in isolation
- Sink can be swapped (different tree formats)
- Clear separation of concerns

**Recovery**: Error handling integrated naturally
- Events can include errors
- Skipped tokens become `ERROR` nodes
- No special tree node types for errors

**Efficiency**: Single pass, minimal allocation
- Events are small (enum variants)
- Tree built once from events
- No intermediate AST

### Why Recovery Sets?

**Predictable**: Parser knows where to resume
- Items start with keywords (`fn`, `struct`)
- Statements start with keywords (`let`, `if`)
- Clear synchronization points

**Bounded**: Maximum tokens skipped
- `MAX_RECOVERY_TOKENS = 500`
- Prevents infinite loops on malformed input
- Ensures termination

### Why Not Tree Rewriting?

Building tree then fixing errors is:
- More complex (two passes)
- Harder to get right
- Less efficient (allocate then mutate)

## Consequences

### Positive
- Multiple errors per file
- IDE features work with errors present
- Clean separation of parsing and tree building
- Predictable recovery behavior
- Well-tested pattern (rust-analyzer uses it)

### Negative
- More complex than simple recursive descent
- Forward parent tracking is subtle
- Recovery sets must be maintained

## Implementation

- **Events**: `spl-parser/src/event.rs`
- **Sink**: `spl-parser/src/sink.rs`
- **Source**: `spl-parser/src/source.rs`
- **Recovery**: `spl-parser/src/lib.rs` (recovery methods)

### Marker Pattern

```rust
fn expr(&mut self) -> Result<CompletedMarker, ParseError> {
    let m = self.start();  // Start marker

    // ... parse expression ...

    Ok(m.complete(self, SyntaxKind::BinExpr))
}
```

Markers track position in event stream:
- `start()` pushes `Placeholder`
- `complete()` replaces with `Start`, pushes `Finish`
- `abandon()` removes placeholder if last
- `precede()` enables left-associative operators

### Recovery Flow

```rust
fn recover_with_error(&mut self, error: ParseError, recovery_set: &[SyntaxKind]) {
    let m = self.start();
    self.error(error);

    // Always consume at least one token (ensures progress)
    if self.current().is_some() {
        self.bump();
    }

    // Skip until recovery point
    while !self.at_set(recovery_set) && self.current().is_some() {
        self.bump();
    }

    m.complete(self, SyntaxKind::ERROR)
}
```

### Example: Error in Parameter List

Input: `fn f(a: i32, @, b: i32) {}`

Events:
```
Start(FunctionDef)
  Token(FN_KW)
  Token(IDENT, "f")
  Start(ParamList)
    Token(L_PAREN)
    Start(Param) ... Finish
    Token(COMMA)
    Start(ERROR)
      Error("expected type")
      Token(ERROR, "@")
    Finish
    Token(COMMA)
    Start(Param) ... Finish
    Token(R_PAREN)
  Finish
  Start(Block) ... Finish
Finish
```

## References

- [rust-analyzer parser](https://github.com/rust-lang/rust-analyzer/tree/master/crates/parser)
- [Simple but Powerful Pratt Parsing](https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html)
- [Resilient LL Parsing](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html)
