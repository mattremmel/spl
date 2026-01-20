//! Parser for SPL source code.
//!
//! Produces a lossless concrete syntax tree using rowan.

mod event;
mod expr;
mod sink;
mod source;
mod stmt;

use crate::lexer::{Lexer, SpannedToken};
use crate::syntax::{SyntaxKind, SyntaxNode};
use event::Event;
use sink::Sink;
use source::Source;

pub use event::ParseError;

/// Result of parsing, containing the syntax tree and any errors.
#[derive(Debug)]
pub struct Parse {
    green_node: rowan::GreenNode,
    errors: Vec<ParseError>,
}

impl Parse {
    /// Returns the root syntax node.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green_node.clone())
    }

    /// Returns the errors encountered during parsing.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Returns true if parsing succeeded without errors.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns a debug representation of the syntax tree.
    pub fn debug_tree(&self) -> String {
        let mut s = String::new();
        format_node(&mut s, &self.syntax(), 0);
        s
    }
}

fn format_node(out: &mut String, node: &SyntaxNode, indent: usize) {
    use std::fmt::Write;
    let kind = node.kind();
    let range = node.text_range();
    let _ = writeln!(out, "{:indent$}{kind:?}@{range:?}", "", indent = indent);

    for child in node.children_with_tokens() {
        match child {
            rowan::NodeOrToken::Node(n) => format_node(out, &n, indent + 2),
            rowan::NodeOrToken::Token(t) => {
                let _ = writeln!(
                    out,
                    "{:indent$}{:?}@{:?} {:?}",
                    "",
                    t.kind(),
                    t.text_range(),
                    t.text(),
                    indent = indent + 2
                );
            }
        }
    }
}

/// Parser for SPL source code.
pub(crate) struct Parser<'src> {
    source: Source<'src>,
    events: Vec<Event>,
}

impl<'src> Parser<'src> {
    /// Create a new parser for the given source.
    pub fn new(source: &'src str) -> Self {
        let tokens: Vec<SpannedToken<'src>> = Lexer::new(source).collect();
        Self {
            source: Source::new(tokens),
            events: Vec::new(),
        }
    }

    /// Parse an expression (entry point for PARSE-2).
    pub fn parse_expr(&mut self) -> Result<(), ParseError> {
        expr::expr(self)?;
        Ok(())
    }

    /// Finish parsing and produce the syntax tree.
    pub fn finish(self) -> Parse {
        let sink = Sink::new(self.source.into_tokens(), self.events);
        sink.finish()
    }

    // === Internal parser methods ===

    /// Start a new node.
    fn start(&mut self) -> Marker {
        let pos = self.events.len();
        self.events.push(Event::Placeholder);
        Marker::new(pos)
    }

    /// Get current token kind (skipping trivia).
    fn current(&mut self) -> Option<SyntaxKind> {
        self.source.current()
    }

    /// Check if current token matches the expected kind.
    fn at(&mut self, kind: SyntaxKind) -> bool {
        self.current() == Some(kind)
    }

    /// Check if current token matches any of the expected kinds.
    #[allow(dead_code)]
    fn at_any(&mut self, kinds: &[SyntaxKind]) -> bool {
        self.current().is_some_and(|k| kinds.contains(&k))
    }

    /// Consume the current token if it matches.
    fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Consume the current token unconditionally.
    fn bump(&mut self) {
        let token = self.source.bump().expect("bump called with no token");
        self.events
            .push(Event::Token { kind: token.kind(), n_raw_tokens: 1 });
    }

    /// Expect a specific token, emitting an error if not found.
    fn expect(&mut self, kind: SyntaxKind) -> Result<(), ParseError> {
        if self.eat(kind) {
            Ok(())
        } else {
            Err(self.error_at_current(format!("expected {:?}", kind)))
        }
    }

    /// Create an error at the current position.
    fn error_at_current(&self, message: String) -> ParseError {
        ParseError {
            message,
            range: self.source.current_range(),
        }
    }

    /// Emit an error event.
    fn error(&mut self, error: ParseError) {
        self.events.push(Event::Error(error));
    }
}

/// Marks the start of a syntax node.
pub(crate) struct Marker {
    pos: usize,
    completed: bool,
}

impl Marker {
    fn new(pos: usize) -> Self {
        Self {
            pos,
            completed: false,
        }
    }

    /// Complete this marker with the given syntax kind.
    pub fn complete(mut self, p: &mut Parser<'_>, kind: SyntaxKind) -> CompletedMarker {
        self.completed = true;
        let event_at_pos = &mut p.events[self.pos];
        assert!(matches!(event_at_pos, Event::Placeholder));
        *event_at_pos = Event::Start {
            kind,
            forward_parent: None,
        };
        p.events.push(Event::Finish);
        CompletedMarker { pos: self.pos }
    }

    /// Abandon this marker without creating a node.
    pub fn abandon(mut self, p: &mut Parser<'_>) {
        self.completed = true;
        if self.pos == p.events.len() - 1 {
            match p.events.pop() {
                Some(Event::Placeholder) => {}
                _ => unreachable!(),
            }
        }
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        if !self.completed {
            panic!("Marker dropped without being completed or abandoned");
        }
    }
}

/// A completed marker that can be used as a precede target.
#[derive(Clone, Copy)]
pub(crate) struct CompletedMarker {
    pos: usize,
}

impl CompletedMarker {
    /// Create a new node that wraps this completed node.
    /// Used for left-associative operators.
    pub fn precede(self, p: &mut Parser<'_>) -> Marker {
        let new_pos = p.events.len();
        p.events.push(Event::Placeholder);

        if let Event::Start { forward_parent, .. } = &mut p.events[self.pos] {
            *forward_parent = Some(new_pos - self.pos);
        } else {
            unreachable!();
        }

        Marker::new(new_pos)
    }
}

/// Parse source code and return the syntax tree.
pub fn parse(source: &str) -> Parse {
    let mut parser = Parser::new(source);

    // For now, parse a single expression (PARSE-2 scope)
    // Later phases will add statement/item parsing
    let m = parser.start();
    if parser.current().is_some() {
        let _ = parser.parse_expr();
    }
    m.complete(&mut parser, SyntaxKind::SourceFile);

    parser.finish()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use expect_test::Expect;

    /// Test helper that parses an expression and compares the tree.
    pub fn check_expr(input: &str, expected_tree: &Expect) {
        let mut parser = Parser::new(input);
        let _ = parser.parse_expr();
        let parse = parser.finish();
        expected_tree.assert_eq(&parse.debug_tree());
    }

    /// Test helper that parses and checks for no errors.
    pub fn check_expr_ok(input: &str) {
        let mut parser = Parser::new(input);
        let result = parser.parse_expr();
        let parse = parser.finish();
        assert!(result.is_ok(), "Parse error: {:?}", result);
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }
}
