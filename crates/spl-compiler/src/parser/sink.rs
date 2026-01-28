//! Converts parse events into a rowan `GreenNode` tree.

use crate::parser::Parse;
use crate::parser::event::{Event, ParseError};
use crate::parser::source::Token;
use crate::syntax::Lang;
use rowan::{GreenNodeBuilder, Language};

/// Converts parse events into a syntax tree.
pub struct Sink<'src> {
    tokens: Vec<Token<'src>>,
    events: Vec<Event>,
    builder: GreenNodeBuilder<'static>,
    token_pos: usize,
    errors: Vec<ParseError>,
}

impl<'src> Sink<'src> {
    /// Create a new sink.
    pub fn new(tokens: Vec<Token<'src>>, events: Vec<Event>) -> Self {
        Self {
            tokens,
            events,
            builder: GreenNodeBuilder::new(),
            token_pos: 0,
            errors: Vec::new(),
        }
    }

    /// Process all events and build the syntax tree.
    pub fn finish(mut self) -> Parse {
        // First pass: find which events are forward-linked parents
        let forward_linked: Vec<bool> = self.compute_forward_linked();

        // Find the last Finish event index - this is the root node's finish.
        // We need to eat trailing trivia before finishing the root.
        let last_finish_idx = self.events.iter().rposition(|e| matches!(e, Event::Finish));

        // Second pass: process events
        // We need `i` for both indexing forward_linked and for event replacement
        #[allow(clippy::needless_range_loop)]
        for i in 0..self.events.len() {
            match std::mem::replace(&mut self.events[i], Event::Placeholder) {
                Event::Start {
                    kind,
                    forward_parent,
                } => {
                    // Collect parent chain for forward-linked nodes
                    // IMPORTANT: Check forward_linked FIRST - if this event was already
                    // emitted as part of an earlier forward chain, skip it entirely
                    let kinds = if forward_linked[i] {
                        // This node was already emitted as part of a forward chain
                        vec![]
                    } else if let Some(offset) = forward_parent {
                        self.collect_parent_chain(i, offset, kind)
                    } else {
                        vec![kind]
                    };

                    // Emit start nodes (outermost first)
                    for k in kinds {
                        self.builder.start_node(Lang::kind_to_raw(k));
                    }
                }
                Event::Finish => {
                    // Before finishing the root node (last Finish), eat any remaining trivia
                    // so it's included inside the root
                    if Some(i) == last_finish_idx {
                        self.eat_trivia();
                    }
                    self.builder.finish_node();
                }
                Event::Token { n_raw_tokens, .. } => {
                    self.eat_trivia();
                    self.eat_n_tokens(n_raw_tokens as usize);
                }
                Event::SyntheticToken { kind, text } => {
                    self.eat_trivia();
                    self.builder.token(Lang::kind_to_raw(kind), &text);
                }
                Event::Error(error) => {
                    self.errors.push(error);
                }
                Event::Placeholder => {}
            }
        }

        Parse {
            green_node: self.builder.finish(),
            errors: self.errors,
        }
    }

    /// Compute which events are targets of `forward_parent` links.
    fn compute_forward_linked(&self) -> Vec<bool> {
        let mut forward_linked = vec![false; self.events.len()];

        for (i, event) in self.events.iter().enumerate() {
            if let Event::Start {
                forward_parent: Some(offset),
                ..
            } = event
            {
                // Walk the entire chain and mark all targets
                let mut current = i + offset;
                let mut chain_len = 0;

                while current < self.events.len() {
                    chain_len += 1;
                    debug_assert!(
                        chain_len <= self.events.len(),
                        "invariant: forward-link chain length ({}) exceeds events count ({}), possible cycle",
                        chain_len,
                        self.events.len()
                    );

                    forward_linked[current] = true;
                    if let Event::Start {
                        forward_parent: Some(next_offset),
                        ..
                    } = &self.events[current]
                    {
                        current += next_offset;
                    } else {
                        break;
                    }
                }
            }
        }

        forward_linked
    }

    /// Collect the chain of parent kinds starting from a forward-linked node.
    fn collect_parent_chain(
        &self,
        start: usize,
        first_offset: usize,
        first_kind: crate::syntax::SyntaxKind,
    ) -> Vec<crate::syntax::SyntaxKind> {
        let mut kinds = vec![first_kind];
        let mut current = start + first_offset;
        let mut chain_len = 0;

        while current < self.events.len() {
            chain_len += 1;
            debug_assert!(
                chain_len <= self.events.len(),
                "invariant: parent chain length ({}) exceeds events count ({}), possible cycle",
                chain_len,
                self.events.len()
            );

            match &self.events[current] {
                Event::Start {
                    kind,
                    forward_parent: Some(offset),
                } => {
                    kinds.push(*kind);
                    current += offset;
                }
                Event::Start {
                    kind,
                    forward_parent: None,
                } => {
                    kinds.push(*kind);
                    break;
                }
                _ => break,
            }
        }

        // Return in reverse order (outermost parent first)
        kinds.reverse();
        kinds
    }

    fn eat_trivia(&mut self) {
        while self.token_pos < self.tokens.len() {
            let token = &self.tokens[self.token_pos];
            if !token.kind.is_trivia() {
                break;
            }
            self.builder
                .token(Lang::kind_to_raw(token.kind), token.text);
            self.token_pos += 1;
        }
    }

    fn eat_n_tokens(&mut self, n: usize) {
        for _ in 0..n {
            if self.token_pos >= self.tokens.len() {
                break;
            }
            let token = &self.tokens[self.token_pos];
            self.builder
                .token(Lang::kind_to_raw(token.kind), token.text);
            self.token_pos += 1;
        }
    }
}
