//! Parser for SPL source code.
//!
//! Produces a lossless concrete syntax tree using rowan.

mod event;
mod expr;
mod item;
mod pattern;
mod sink;
mod source;
mod stmt;

use crate::lexer::{Lexer, SpannedToken};
use crate::syntax::{SyntaxKind, SyntaxNode};
use event::Event;
use sink::Sink;
use source::Source;

pub use event::ParseError;

// === Recovery Sets ===
// These define synchronization points where the parser can resume after errors.

/// Tokens that can start a new top-level item.
const ITEM_RECOVERY_SET: &[SyntaxKind] = &[
    SyntaxKind::FN_KW,
    SyntaxKind::STRUCT_KW,
    SyntaxKind::TYPE_KW,
    SyntaxKind::IMPL_KW,
    SyntaxKind::PUB_KW,
];

/// Tokens that can start a new statement or end a block.
const STMT_RECOVERY_SET: &[SyntaxKind] = &[
    SyntaxKind::LET_KW,
    SyntaxKind::IF_KW,
    SyntaxKind::WHILE_KW,
    SyntaxKind::FOR_KW,
    SyntaxKind::LOOP_KW,
    SyntaxKind::RETURN_KW,
    SyntaxKind::BREAK_KW,
    SyntaxKind::CONTINUE_KW,
    SyntaxKind::L_BRACE,
    SyntaxKind::R_BRACE,
    SyntaxKind::SEMI,
];

/// Tokens that typically end expressions.
#[allow(dead_code)]
const EXPR_RECOVERY_SET: &[SyntaxKind] = &[
    SyntaxKind::SEMI,
    SyntaxKind::R_PAREN,
    SyntaxKind::R_BRACKET,
    SyntaxKind::R_BRACE,
    SyntaxKind::COMMA,
];

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
    #[allow(dead_code)]
    pub fn parse_expr(&mut self) -> Result<(), ParseError> {
        expr::expr(self)?;
        Ok(())
    }

    /// Parse a function definition (entry point for PARSE-5).
    #[allow(dead_code)]
    pub fn parse_function(&mut self) -> Result<(), ParseError> {
        item::function_def(self)?;
        Ok(())
    }

    /// Parse a top-level item (function, struct, type alias, impl).
    #[allow(dead_code)]
    pub fn parse_item(&mut self) -> Result<(), ParseError> {
        item::item(self)?;
        Ok(())
    }

    /// Parse a source file (sequence of items).
    pub fn parse_source_file(&mut self) -> CompletedMarker {
        item::source_file(self)
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

    /// Get the current token's text.
    fn current_text(&mut self) -> Option<&str> {
        self.source.current_text()
    }

    /// Check if current token matches any of the expected kinds.
    #[allow(dead_code)]
    fn at_any(&mut self, kinds: &[SyntaxKind]) -> bool {
        self.current().is_some_and(|k| kinds.contains(&k))
    }

    /// Check if the nth lookahead token (0 = current) matches the expected kind.
    #[allow(dead_code)]
    fn peek_at(&mut self, n: usize, kind: SyntaxKind) -> bool {
        self.source.peek(n) == Some(kind)
    }

    /// Get the nth lookahead token kind (0 = current).
    fn peek(&mut self, n: usize) -> Option<SyntaxKind> {
        self.source.peek(n)
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
        self.events.push(Event::Token {
            kind: token.kind(),
            n_raw_tokens: 1,
        });
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

    // === Error Recovery ===

    /// Check if we're at a token in the recovery set.
    fn at_set(&mut self, set: &[SyntaxKind]) -> bool {
        self.current().is_some_and(|k| set.contains(&k))
    }

    /// Emit an error and skip tokens until we reach a recovery point.
    /// Returns the marker for the error node wrapping the skipped tokens.
    fn recover_with_error(
        &mut self,
        error: ParseError,
        recovery_set: &[SyntaxKind],
    ) -> CompletedMarker {
        let m = self.start();
        self.error(error);

        // Skip tokens until we hit a recovery point or EOF
        while !self.at_set(recovery_set) && self.current().is_some() {
            self.bump();
        }

        m.complete(self, SyntaxKind::ERROR)
    }

    /// Try to recover to the nearest item boundary.
    /// Skips tokens and wraps them in an ERROR node.
    fn recover_to_item(&mut self, error: ParseError) -> CompletedMarker {
        self.recover_with_error(error, ITEM_RECOVERY_SET)
    }

    /// Try to recover to the nearest statement boundary.
    /// Skips tokens and wraps them in an ERROR node.
    fn recover_to_stmt(&mut self, error: ParseError) -> CompletedMarker {
        self.recover_with_error(error, STMT_RECOVERY_SET)
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
    parser.parse_source_file();
    parser.finish()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use expect_test::{Expect, expect};

    /// Test helper that parses an expression and compares the tree.
    pub fn check_expr(input: &str, expected_tree: &Expect) {
        let mut parser = Parser::new(input);
        let _ = parser.parse_expr();
        let parse = parser.finish();
        expected_tree.assert_eq(&parse.debug_tree());
    }

    /// Test helper that parses and checks for no errors.
    #[allow(dead_code)]
    pub fn check_expr_ok(input: &str) {
        let mut parser = Parser::new(input);
        let result = parser.parse_expr();
        let parse = parser.finish();
        assert!(result.is_ok(), "Parse error: {:?}", result);
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    /// Test helper that parses an item and compares the tree.
    pub fn check_item(input: &str, expected_tree: &Expect) {
        let mut parser = Parser::new(input);
        let _ = parser.parse_item();
        let parse = parser.finish();
        expected_tree.assert_eq(&parse.debug_tree());
    }

    /// Test helper that parses a source file and compares the tree.
    pub fn check_source_file(input: &str, expected_tree: &Expect) {
        let parse = super::parse(input);
        expected_tree.assert_eq(&parse.debug_tree());
    }

    // === Error Recovery Tests ===

    #[test]
    fn recovery_unknown_token_between_items() {
        // Unknown token between valid items should be wrapped in ERROR
        let parse = parse("fn foo() {} @ fn bar() {}");
        assert!(!parse.ok());
        assert_eq!(parse.errors().len(), 1);
        // Both functions should still be parsed
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
    }

    #[test]
    fn recovery_invalid_item_start() {
        // Invalid token at start should be skipped, valid items parsed
        let parse = parse("!!! fn foo() {}");
        assert!(!parse.ok());
        // The function should still be parsed
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
    }

    #[test]
    fn recovery_multiple_valid_items_with_garbage() {
        // Multiple valid items with garbage between them
        let parse = parse("struct A {} %%% fn foo() {} ??? struct B {}");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        // All three items should be parsed
        assert!(tree.contains("StructDef"));
        assert!(tree.contains("FunctionDef"));
    }

    #[test]
    fn recovery_preserves_syntax_tree_structure() {
        // Error recovery should produce a well-formed tree
        // Note: whitespace before recovery point attaches to next item
        check_source_file(
            "fn a() {} @@@ fn b() {}",
            &expect![[r#"
                SourceFile@0..23
                  FunctionDef@0..9
                    FN_KW@0..2 "fn"
                    Name@2..4
                      WHITESPACE@2..3 " "
                      IDENT@3..4 "a"
                    ParamList@4..6
                      L_PAREN@4..5 "("
                      R_PAREN@5..6 ")"
                    Block@6..9
                      WHITESPACE@6..7 " "
                      L_BRACE@7..8 "{"
                      R_BRACE@8..9 "}"
                  ERROR@9..13
                    WHITESPACE@9..10 " "
                    ERROR@10..11 "@"
                    ERROR@11..12 "@"
                    ERROR@12..13 "@"
                  FunctionDef@13..23
                    WHITESPACE@13..14 " "
                    FN_KW@14..16 "fn"
                    Name@16..18
                      WHITESPACE@16..17 " "
                      IDENT@17..18 "b"
                    ParamList@18..20
                      L_PAREN@18..19 "("
                      R_PAREN@19..20 ")"
                    Block@20..23
                      WHITESPACE@20..21 " "
                      L_BRACE@21..22 "{"
                      R_BRACE@22..23 "}"
            "#]],
        );
    }

    #[test]
    fn recovery_in_impl_block() {
        // Error in impl block should recover to next method
        let parse = parse("impl Foo { !! fn bar() {} }");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        assert!(tree.contains("ImplBlock"));
        assert!(tree.contains("FunctionDef"));
    }

    #[test]
    fn recovery_reports_all_errors() {
        // Multiple errors should all be reported
        let parse = parse("@@ fn a() {} ## fn b() {}");
        assert!(!parse.ok());
        // Should have at least 2 errors (one for each garbage section)
        assert!(parse.errors().len() >= 2);
    }

    #[test]
    fn recovery_empty_with_garbage() {
        // Only garbage should result in error nodes
        let parse = parse("!@#");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        assert!(tree.contains("ERROR"));
    }
}
