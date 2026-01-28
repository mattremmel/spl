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

Parsing produces events; a separate sink converts events to rowan green tree (see [ADR-008](008-rowan-based-cst.md)):

```
Source → Tokens → Parser → Events → Sink → GreenNode
```

Parse errors are lightweight during parsing, then enriched to full diagnostics with suggestions (see [ADR-001](001-phase-specific-error-handling.md)).

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

**Bounded**: Maximum tokens skipped (configurable)
- Default: `max_recovery_tokens = 500`
- Prevents infinite loops on malformed input
- Ensures termination
- Configurable for different use cases (IDE vs batch compilation)

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
- **Sink**: `spl-parser/src/sink.rs` (builds rowan green tree, see [ADR-008](008-rowan-based-cst.md))
- **Source**: `spl-parser/src/source.rs`
- **Recovery**: `spl-parser/src/recovery.rs`
- **Pratt parser**: `spl-parser/src/expr.rs` (expression parsing with precedence)
- **Config**: `spl-parser/src/config.rs` (recovery limits, etc.)

### Parser Configuration

```rust
pub struct ParserConfig {
    /// Maximum tokens to skip during error recovery.
    /// Default: 500. Set lower for stricter parsing, higher for more lenient.
    pub max_recovery_tokens: usize,

    /// Whether to attempt recovery at all.
    /// Disable for "fail fast" batch compilation.
    pub enable_recovery: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            max_recovery_tokens: 500,
            enable_recovery: true,
        }
    }
}
```

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

### Pratt Parsing for Expressions

Expressions use Pratt parsing (precedence climbing) for correct associativity and precedence:

```rust
fn expr(&mut self) -> CompletedMarker {
    self.expr_bp(0)  // Start with minimum binding power
}

fn expr_bp(&mut self, min_bp: u8) -> CompletedMarker {
    let mut lhs = self.expr_atom();  // Parse prefix/atom

    loop {
        let op = match self.current() {
            Some(op) if is_binary_op(op) => op,
            _ => break,
        };

        let (l_bp, r_bp) = infix_binding_power(op);

        // Stop if operator binds less tightly than our threshold
        if l_bp < min_bp {
            break;
        }

        let m = lhs.precede(self);  // Wrap LHS
        self.bump();                 // Consume operator
        self.expr_bp(r_bp);         // Parse RHS with right binding power
        lhs = m.complete(self, SyntaxKind::BinExpr);
    }

    lhs
}

/// Returns (left_binding_power, right_binding_power)
/// Left < Right for left-associative, Left > Right for right-associative
fn infix_binding_power(op: SyntaxKind) -> (u8, u8) {
    match op {
        // Assignment: right-associative (a = b = c parses as a = (b = c))
        EQ => (2, 1),

        // Logical or
        PIPE_PIPE => (3, 4),

        // Logical and
        AMP_AMP => (5, 6),

        // Comparison: non-associative (a < b < c is error)
        EQ_EQ | BANG_EQ | LT | GT | LT_EQ | GT_EQ => (7, 7),

        // Bitwise or
        PIPE => (9, 10),

        // Bitwise xor
        CARET => (11, 12),

        // Bitwise and
        AMP => (13, 14),

        // Shift
        LT_LT | GT_GT => (15, 16),

        // Additive: left-associative (a + b + c parses as (a + b) + c)
        PLUS | MINUS => (17, 18),

        // Multiplicative
        STAR | SLASH | PERCENT => (19, 20),

        _ => panic!("not a binary operator: {:?}", op),
    }
}
```

**Key concepts:**

| Concept | Explanation |
|---------|-------------|
| Binding power | Higher = binds tighter (`*` > `+`) |
| Left-associative | Left BP < Right BP (e.g., `17, 18` for `+`) |
| Right-associative | Left BP > Right BP (e.g., `2, 1` for `=`) |
| Non-associative | Left BP == Right BP (forces parentheses) |
| `precede()` | Wraps already-parsed LHS into new node |

**Prefix operators** (unary `-`, `!`, `&`, `*`) have their own binding powers:

```rust
fn prefix_binding_power(op: SyntaxKind) -> u8 {
    match op {
        MINUS | BANG => 21,      // Unary - and !
        AMP | STAR => 23,        // Reference and dereference
        _ => panic!("not a prefix operator"),
    }
}
```

**Postfix operators** (function calls, indexing, field access) bind tightest:

```rust
fn postfix_binding_power(op: SyntaxKind) -> Option<u8> {
    match op {
        L_PAREN => Some(25),     // Function call
        L_BRACKET => Some(25),  // Indexing
        DOT => Some(25),        // Field access
        _ => None,
    }
}
```

## References

- [rust-analyzer parser](https://github.com/rust-lang/rust-analyzer/tree/master/crates/parser)
- [Simple but Powerful Pratt Parsing](https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html)
- [Resilient LL Parsing](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html)
- [Pratt Parsers: Expression Parsing Made Easy](https://journal.stuffwithstuff.com/2011/03/19/pratt-parsers-expression-parsing-made-easy/)
