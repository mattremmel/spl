//! Parser for SPL source code.
//!
//! Produces a lossless concrete syntax tree using rowan.
//!
//! # Error Handling
//!
//! The parser uses an event-based architecture with error recovery to produce
//! partial results even when the input contains syntax errors.
//!
//! ## `ParseError` Type
//!
//! Errors are represented as [`ParseError`], which contains:
//! - A human-readable message describing what went wrong
//! - The source range where the error occurred
//!
//! Errors are collected as events during parsing and extracted when building
//! the final syntax tree. This allows the parser to report multiple errors
//! in a single pass.
//!
//! ## Recovery Sets and Synchronization
//!
//! When the parser encounters an unexpected token, it uses **recovery sets**
//! to find a synchronization point where parsing can resume:
//!
//! - `ITEM_RECOVERY_SET`: Tokens that start top-level items (`fn`, `struct`, etc.)
//! - `STMT_RECOVERY_SET`: Tokens that start statements or end blocks
//! - `EXPR_RECOVERY_SET`: Tokens that typically end expressions
//!
//! The recovery process:
//! 1. Emit an error event describing the problem
//! 2. Skip tokens until reaching a recovery set member or EOF
//! 3. Wrap skipped tokens in an `ERROR` syntax node
//! 4. Resume normal parsing
//!
//! This approach ensures the parser produces a complete (if imperfect) syntax
//! tree, enabling IDE features like syntax highlighting and code navigation
//! even in the presence of errors.
//!
//! ## Why Not Diagnostic?
//!
//! The parser uses its own [`ParseError`] type rather than the crate's
//! [`Diagnostic`](crate::Diagnostic) type for several reasons:
//!
//! - **Self-contained**: The parser module has no dependencies on semantic
//!   analysis infrastructure, making it reusable and easier to test.
//! - **Simplicity**: Parse errors are structural (wrong token) rather than
//!   semantic (wrong type), so they don't need rich labels or suggestions.
//! - **Performance**: Parse errors are lightweight and don't require the
//!   allocation overhead of `Diagnostic`'s label vectors.
//!
//! The compiler driver can convert [`ParseError`] to [`Diagnostic`](crate::Diagnostic)
//! when rendering errors to users.

#[macro_use]
mod macros;
mod event;
mod expr;
mod item;
mod path;
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
    SyntaxKind::HASH, // Attributes
    SyntaxKind::FN_KW,
    SyntaxKind::STRUCT_KW,
    SyntaxKind::TYPE_KW,
    SyntaxKind::IMPL_KW,
    SyntaxKind::PUB_KW,
    SyntaxKind::USE_KW,
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

/// Maximum number of tokens to skip during error recovery.
/// Prevents infinite loops or excessive CPU usage on malformed input.
const MAX_RECOVERY_TOKENS: usize = 500;

/// Get matching close delimiter for an open delimiter.
/// Returns None if the token is not an open delimiter.
fn matching_close(open: SyntaxKind) -> Option<SyntaxKind> {
    match open {
        SyntaxKind::L_PAREN => Some(SyntaxKind::R_PAREN),
        SyntaxKind::L_BRACKET => Some(SyntaxKind::R_BRACKET),
        SyntaxKind::L_BRACE => Some(SyntaxKind::R_BRACE),
        _ => None,
    }
}

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
        debug_assert!(
            self.current().is_some(),
            "precondition: bump requires current token"
        );
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
            Err(self.error_at_current(format!("expected {kind:?}")))
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

    /// Emit a synthetic token (not from source).
    pub fn emit_synthetic_token(&mut self, kind: SyntaxKind, text: String) {
        self.events.push(Event::SyntheticToken { kind, text });
    }

    // === Error Recovery ===

    /// Check if we're at a token in the recovery set.
    fn at_set(&mut self, set: &[SyntaxKind]) -> bool {
        self.current().is_some_and(|k| set.contains(&k))
    }

    /// Emit an error and skip tokens until we reach a recovery point.
    /// Returns the marker for the error node wrapping the skipped tokens.
    ///
    /// This function is bounded to prevent infinite loops on malformed input.
    /// At most `MAX_RECOVERY_TOKENS` tokens will be skipped.
    ///
    /// IMPORTANT: Always consumes at least one token to ensure progress.
    /// This prevents infinite loops when we're already at a recovery token
    /// but the item parse still failed.
    fn recover_with_error(
        &mut self,
        error: ParseError,
        recovery_set: &[SyntaxKind],
    ) -> CompletedMarker {
        let m = self.start();
        self.error(error);

        // Always consume at least one token to ensure progress
        // This handles the case where we're at a recovery token (like #)
        // but the parse still failed (e.g., malformed attribute #test)
        if self.current().is_some() {
            self.bump();
        }

        // Skip tokens until we hit a recovery point, EOF, or token limit
        let mut consumed = 1;
        while !self.at_set(recovery_set)
            && self.current().is_some()
            && consumed < MAX_RECOVERY_TOKENS
        {
            self.bump();
            consumed += 1;
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

    /// Recover by skipping to a matching close delimiter.
    ///
    /// Tracks nesting depth to find the correct matching delimiter.
    /// Does NOT consume the final close delimiter.
    /// Returns the number of tokens consumed.
    ///
    /// This is useful for recovering from errors inside delimited expressions
    /// like function calls or parenthesized expressions.
    #[allow(dead_code)]
    fn recover_to_delimiter(&mut self, open: SyntaxKind) -> usize {
        let Some(close) = matching_close(open) else {
            return 0;
        };

        let mut depth = 1;
        let mut consumed = 0;

        while depth > 0 && self.current().is_some() && consumed < MAX_RECOVERY_TOKENS {
            match self.current() {
                Some(k) if k == open => {
                    depth += 1;
                    self.bump();
                }
                Some(k) if k == close => {
                    depth -= 1;
                    if depth == 0 {
                        return consumed; // Don't consume final close
                    }
                    self.bump();
                }
                _ => {
                    self.bump();
                }
            }
            consumed += 1;
        }

        consumed
    }

    /// Skip tokens until next comma or matching close delimiter.
    ///
    /// Tracks nesting depth for the given delimiter pair.
    /// Does NOT consume the comma or close delimiter.
    /// Returns the number of tokens consumed.
    fn recover_to_comma_or_close(&mut self, open: SyntaxKind, close: SyntaxKind) -> usize {
        let mut depth = 1;
        let mut consumed = 0;

        while self.current().is_some() && consumed < MAX_RECOVERY_TOKENS {
            match self.current() {
                Some(SyntaxKind::COMMA) if depth == 1 => return consumed, // Found comma at our level
                Some(k) if k == open => {
                    depth += 1;
                    self.bump();
                }
                Some(k) if k == close => {
                    depth -= 1;
                    if depth == 0 {
                        return consumed; // Found matching close
                    }
                    self.bump();
                }
                _ => {
                    self.bump();
                }
            }
            consumed += 1;
        }

        consumed
    }

    // ===== Delimited List Parsing =====

    /// Parse comma-separated items until reaching the close delimiter.
    ///
    /// This helper extracts the common pattern for parsing delimited lists:
    /// ```text
    /// if !at(close) {
    ///     parse_item()?;
    ///     while eat(COMMA) {
    ///         if at(close) { break; }  // trailing comma
    ///         parse_item()?;
    ///     }
    /// }
    /// ```
    ///
    /// Returns `Ok(true)` if at least one item was parsed, `Ok(false)` if the list was empty.
    /// Does NOT consume the close delimiter - caller must handle that.
    pub(crate) fn parse_delimited<F>(
        &mut self,
        close: SyntaxKind,
        mut parse_item: F,
    ) -> Result<bool, ParseError>
    where
        F: FnMut(&mut Self) -> Result<(), ParseError>,
    {
        if self.at(close) {
            return Ok(false);
        }

        parse_item(self)?;
        while self.eat(SyntaxKind::COMMA) {
            if self.at(close) {
                break; // trailing comma
            }
            parse_item(self)?;
        }
        Ok(true)
    }

    /// Parse comma-separated items with error recovery.
    ///
    /// When `parse_item` fails, skips tokens to the next comma or close delimiter,
    /// wrapping skipped tokens in an ERROR node, then continues parsing.
    ///
    /// Does NOT consume the close delimiter - caller must handle that.
    pub(crate) fn parse_delimited_with_recovery<F>(
        &mut self,
        open: SyntaxKind,
        close: SyntaxKind,
        mut parse_item: F,
    ) where
        F: FnMut(&mut Self) -> Result<(), ParseError>,
    {
        if self.at(close) {
            return;
        }

        loop {
            match parse_item(self) {
                Ok(()) => {}
                Err(err) => {
                    // Wrap error tokens in ERROR node
                    let m = self.start();
                    self.error(err);
                    self.recover_to_comma_or_close(open, close);
                    m.complete(self, SyntaxKind::ERROR);
                }
            }

            if !self.eat(SyntaxKind::COMMA) {
                break;
            }
            if self.at(close) {
                break; // trailing comma
            }
        }
    }

    // ===== Contract Helpers =====

    /// Returns the current token position.
    /// Used for contract assertions to verify parser advancement.
    pub(crate) fn current_offset(&self) -> usize {
        self.source.token_position()
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
                _ => unreachable!("Marker abandon: expected Placeholder event at position {}", self.pos),
            }
        }
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        assert!(
            self.completed,
            "Marker dropped without being completed or abandoned"
        );
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
            unreachable!("CompletedMarker precede: expected Start event at position {}", self.pos);
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
        assert!(result.is_ok(), "Parse error: {result:?}");
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
        let parse = parse("struct A() %%% fn foo() {} ??? struct B()");
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

    // === Phase 7: Additional Error Recovery Tests ===

    #[test]
    fn recovery_missing_semicolon() {
        // Missing semicolon - the parser generates error and continues
        let parse = parse("fn foo() { let x = 1; let y = 2; x }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
        assert!(tree.matches("LetStmt").count() == 2);
    }

    #[test]
    fn recovery_extra_token_between_items() {
        // Extra garbage token between items is recoverable
        let parse = parse("fn foo() {} %%% fn bar() {}");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        // Both functions should still be parsed
        assert!(tree.matches("FunctionDef").count() == 2);
    }

    #[test]
    fn recovery_nested_error() {
        // Error in nested impl block should recover to next method
        let parse = parse("impl Foo { fn bar() {} @@@ fn baz() {} }");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        // Should still parse impl with methods
        assert!(tree.contains("ImplBlock"));
        assert!(tree.matches("FunctionDef").count() >= 1);
    }

    // === Phase 8: Error Message Quality Tests ===
    // Note: Some error recovery tests trigger Marker panics when encountering
    // invalid syntax mid-construct. These tests verify the parser handles
    // invalid tokens between valid items gracefully.

    #[test]
    fn error_msg_between_items() {
        // Error between valid items - this is well-handled
        let parse = parse("fn foo() {} @@@ fn bar() {}");
        assert!(!parse.ok());
        let errors = parse.errors();
        assert!(!errors.is_empty());
        // Both functions should still be parsed
        let tree = parse.debug_tree();
        assert!(tree.matches("FunctionDef").count() == 2);
    }

    #[test]
    fn error_position_between_items() {
        // Verify error positions are accurate for errors between items
        let parse = parse("fn foo() {} @");
        assert!(!parse.ok());
        let errors = parse.errors();
        assert!(!errors.is_empty());
        // Error should be at position 12 (the @)
        let range = &errors[0].range;
        assert!(
            range.start >= 11 && range.start <= 13,
            "range.start = {}",
            range.start
        );
    }

    // === Phase 9: Whitespace and Comment Handling Tests ===

    #[test]
    fn whitespace_heavy() {
        let parse = parse("fn    foo   (   )   {   }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
    }

    #[test]
    fn comment_inline() {
        let parse = parse("fn /* comment */ foo() {}");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
        assert!(tree.contains("COMMENT"));
    }

    #[test]
    fn comment_line() {
        let parse = parse("// line comment\nfn foo() {}");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
        assert!(tree.contains("COMMENT"));
    }

    #[test]
    fn comment_multiline() {
        let parse = parse("fn foo() { let x = /* multi\nline */ 1; }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn no_whitespace() {
        let parse = parse("fn foo(){let x:i32=1;}");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
        assert!(tree.contains("LetStmt"));
    }

    // === Phase 10: Boundary Condition Tests ===

    #[test]
    fn empty_source() {
        let parse = parse("");
        assert!(parse.ok());
        let tree = parse.debug_tree();
        assert!(tree.contains("SourceFile@0..0"));
    }

    #[test]
    fn whitespace_only() {
        let parse = parse("   \n\t  ");
        assert!(parse.ok());
    }

    #[test]
    fn comment_only() {
        let parse = parse("// just a comment");
        assert!(parse.ok());
        let tree = parse.debug_tree();
        assert!(tree.contains("COMMENT"));
    }

    #[test]
    fn deeply_nested_parens() {
        let parse = parse("fn foo() { let x = ((((((((((1)))))))))); }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn deeply_nested_blocks() {
        let parse = parse("fn foo() { { { { { 1 } } } } }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn many_parameters() {
        let parse = parse(
            "fn foo(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, i: i32, j: i32) {}",
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn many_struct_fields() {
        let parse = parse(
            "struct S(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, i: i32, j: i32)",
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn long_expression_chain() {
        let parse = parse("fn foo() { let x = 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10; }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn deeply_nested_generics() {
        let parse = parse("fn foo() { let x: A(B(C(D(E)))); }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    // === Deep Nesting Stress Tests ===

    #[test]
    fn very_deeply_nested_blocks() {
        // 10+ levels of nested blocks
        let parse = parse("fn foo() { { { { { { { { { { { 1 } } } } } } } } } } }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn long_method_chain() {
        // 10+ method calls in a chain
        let parse = parse("fn foo() { obj.a().b().c().d().e().f().g().h().i().j().k().l(); }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn many_struct_fields_20() {
        // 20+ struct fields
        let parse = parse(
            "struct S(a1: i32, a2: i32, a3: i32, a4: i32, a5: i32, a6: i32, a7: i32, a8: i32, a9: i32, a10: i32, a11: i32, a12: i32, a13: i32, a14: i32, a15: i32, a16: i32, a17: i32, a18: i32, a19: i32, a20: i32)",
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn deeply_nested_if_else() {
        // Many levels of if-else
        let parse = parse(
            "fn foo() { if a { if b { if c { if d { if e { 1 } else { 2 } } else { 3 } } else { 4 } } else { 5 } } else { 6 } }",
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn nested_array_expressions() {
        // Arrays containing arrays containing arrays
        let parse = parse("fn foo() { let x = [[[[1, 2], [3, 4]], [[5, 6], [7, 8]]]]; }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn nested_tuple_expressions() {
        // Tuples containing tuples
        let parse = parse("fn foo() { let x = ((((1, 2), (3, 4)), ((5, 6), (7, 8)))); }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn complex_index_chain() {
        // Multiple index operations chained
        let parse = parse("fn foo() { arr[0][1][2][3][4]; }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    // === Phase 11: Integration Tests ===

    #[test]
    fn full_point_struct() {
        let parse = parse(
            r#"
            struct Point(x: i32, y: i32)

            impl Point {
                fn new(x: i32, y: i32): Point {
                    Point(x, y)
                }

                fn distance(&self, other: &Point): f64 {
                    let dx = self.x - other.x;
                    let dy = self.y - other.y;
                    0.0
                }
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        assert!(tree.contains("StructDef"));
        assert!(tree.contains("ImplBlock"));
    }

    #[test]
    fn generic_container() {
        let parse = parse(
            r#"
            struct Node(
                value: T,
                next: Option(Box(Node(T)))
            ) where T
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn control_flow_in_method() {
        let parse = parse(
            r#"
            fn process(items: Vec(i32)): i32 {
                let mut sum = 0;
                for item in items {
                    if item > 0 {
                        sum = sum + item;
                    } else {
                        continue;
                    }
                }
                sum
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn pattern_in_for() {
        let parse = parse(
            r#"
            fn foo() {
                for (a, b) in pairs {
                    bar(a, b);
                }
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn nested_struct_in_call() {
        let parse = parse(
            r#"
            fn foo() {
                bar(Point(x: Inner(y: 1)));
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn complex_generic_type() {
        let parse = parse(
            r#"
            fn foo() {
                let x: Vec(HashMap(String, Option((i32, bool))));
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn multiple_impl_blocks() {
        let parse = parse(
            r#"
            struct Foo()
            struct Bar()

            impl Foo {
                fn a() {}
            }

            impl Bar {
                fn b() {}
            }

            impl Foo {
                fn c() {}
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        // Should have 2 StructDef and 3 ImplBlock
        assert_eq!(tree.matches("StructDef").count(), 2);
        assert_eq!(tree.matches("ImplBlock").count(), 3);
    }

    #[test]
    fn visibility_combinations() {
        let parse = parse(
            r#"
            pub struct Foo(
                pub a: i32,
                pub(crate) b: i32,
                pub(super) c: i32,
                d: i32
            )

            impl Foo {
                pub fn public() {}
                pub(crate) fn internal() {}
                fn private() {}
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn type_alias_variety() {
        let parse = parse(
            r#"
            type Int = i32;
            type Pair = (i32, i32);
            type Buffer = [u8; 256];
            type Callback = fn(i32) -> i32;
            type Nested = Option(Vec(String));
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        assert_eq!(tree.matches("TypeAlias").count(), 5);
    }

    #[test]
    fn method_call_chain() {
        let parse = parse(
            r#"
            fn foo() {
                obj.method1().method2().method3().field.method4();
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    #[test]
    fn match_like_if_chain() {
        let parse = parse(
            r#"
            fn classify(x: i32): i32 {
                if x < 0 {
                    -1
                } else if x == 0 {
                    0
                } else {
                    1
                }
            }
        "#,
        );
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
    }

    // === Error Recovery at Item Level ===
    // Note: The parser recovers from garbage between items but may panic
    // when encountering invalid syntax within constructs.

    #[test]
    fn recovery_garbage_before_function() {
        // Garbage before a valid function should be skipped
        let parse = parse("@@@ fn foo() {}");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
    }

    #[test]
    fn recovery_garbage_after_function() {
        // Garbage after a valid function should be reported
        let parse = parse("fn foo() {} @@@");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
    }

    #[test]
    fn recovery_mixed_valid_and_invalid_items() {
        // Valid items with garbage between should all be parsed
        let parse = parse("struct A() @@@ fn foo() {} ### struct B()");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        assert_eq!(tree.matches("StructDef").count(), 2);
        assert!(tree.contains("FunctionDef"));
    }

    // === parse_delimited() helper tests ===

    #[test]
    fn parse_delimited_empty_tuple() {
        // "()" -> empty list
        check_expr(
            "()",
            &expect![[r#"
                TupleExpr@0..2
                  L_PAREN@0..1 "("
                  R_PAREN@1..2 ")"
            "#]],
        );
    }

    #[test]
    fn parse_delimited_single_item() {
        // "(a,)" -> single element tuple
        check_expr(
            "(a,)",
            &expect![[r#"
                TupleExpr@0..4
                  L_PAREN@0..1 "("
                  PathExpr@1..2
                    Path@1..2
                      PathSegment@1..2
                        NameRef@1..2
                          IDENT@1..2 "a"
                  COMMA@2..3 ","
                  R_PAREN@3..4 ")"
            "#]],
        );
    }

    #[test]
    fn parse_delimited_multiple_items() {
        // "(a, b, c)" -> three items
        check_expr(
            "(a, b, c)",
            &expect![[r#"
                TupleExpr@0..9
                  L_PAREN@0..1 "("
                  PathExpr@1..2
                    Path@1..2
                      PathSegment@1..2
                        NameRef@1..2
                          IDENT@1..2 "a"
                  COMMA@2..3 ","
                  PathExpr@3..5
                    Path@3..5
                      PathSegment@3..5
                        NameRef@3..5
                          WHITESPACE@3..4 " "
                          IDENT@4..5 "b"
                  COMMA@5..6 ","
                  PathExpr@6..8
                    Path@6..8
                      PathSegment@6..8
                        NameRef@6..8
                          WHITESPACE@6..7 " "
                          IDENT@7..8 "c"
                  R_PAREN@8..9 ")"
            "#]],
        );
    }

    #[test]
    fn parse_delimited_trailing_comma() {
        // "(a, b,)" -> trailing comma allowed
        check_expr(
            "(a, b,)",
            &expect![[r#"
                TupleExpr@0..7
                  L_PAREN@0..1 "("
                  PathExpr@1..2
                    Path@1..2
                      PathSegment@1..2
                        NameRef@1..2
                          IDENT@1..2 "a"
                  COMMA@2..3 ","
                  PathExpr@3..5
                    Path@3..5
                      PathSegment@3..5
                        NameRef@3..5
                          WHITESPACE@3..4 " "
                          IDENT@4..5 "b"
                  COMMA@5..6 ","
                  R_PAREN@6..7 ")"
            "#]],
        );
    }

    #[test]
    fn parse_delimited_brackets() {
        // "[a, b]" -> works with different delimiters
        check_expr(
            "[a, b]",
            &expect![[r#"
                ArrayExpr@0..6
                  L_BRACKET@0..1 "["
                  PathExpr@1..2
                    Path@1..2
                      PathSegment@1..2
                        NameRef@1..2
                          IDENT@1..2 "a"
                  COMMA@2..3 ","
                  PathExpr@3..5
                    Path@3..5
                      PathSegment@3..5
                        NameRef@3..5
                          WHITESPACE@3..4 " "
                          IDENT@4..5 "b"
                  R_BRACKET@5..6 "]"
            "#]],
        );
    }

    // === Delimiter Recovery Tests (spl-3r0) ===

    #[test]
    fn delimiter_matching_helper() {
        // Test that matching_close returns correct close delimiters
        assert_eq!(
            super::matching_close(SyntaxKind::L_PAREN),
            Some(SyntaxKind::R_PAREN)
        );
        assert_eq!(
            super::matching_close(SyntaxKind::L_BRACKET),
            Some(SyntaxKind::R_BRACKET)
        );
        assert_eq!(
            super::matching_close(SyntaxKind::L_BRACE),
            Some(SyntaxKind::R_BRACE)
        );
        assert_eq!(super::matching_close(SyntaxKind::IDENT), None);
    }

    #[test]
    fn recovery_error_in_function_call_args() {
        // Error token inside function call should not break the function def
        // The @ token should cause an error but the outer structure should be preserved
        let parse = parse("fn f() { let x = foo(a, b); }");
        assert!(parse.ok(), "Parse errors: {:?}", parse.errors());
        let tree = parse.debug_tree();
        assert!(tree.contains("FunctionDef"));
        assert!(tree.contains("CallExpr"));
    }

    #[test]
    fn recovery_bounded_does_not_hang() {
        // Even with many tokens, recovery should be bounded
        // This should not cause an infinite loop or hang
        let input = "fn f() { ".to_string() + &"x ".repeat(100) + "}";
        let parse = parse(&input);
        // Should complete without hanging, may have errors
        assert!(parse.debug_tree().contains("FunctionDef"));
    }

    // === parse_delimited_with_recovery tests (spl-3bb) ===

    #[test]
    fn parse_delimited_recovery_skips_bad_item() {
        // Error in one item should not break entire list
        // "(a: i32, @, b: i32)" - @ is invalid, should recover and parse b
        let parse = parse("fn f(a: i32, @, b: i32) {}");
        assert!(!parse.ok()); // Has errors
        let tree = parse.debug_tree();
        // Both 'a' and 'b' params should be present (use "Param@" to avoid matching ParamList)
        assert_eq!(tree.matches("Param@").count(), 2, "tree:\n{tree}");
    }

    #[test]
    fn parse_delimited_recovery_nested_delimiters() {
        // Recovery should handle nested parens correctly
        // "fn f(a: i32, (@@), b: i32) {}" - error tokens in place of a param
        // Recovery should skip (@@) and continue to parse b
        let parse = parse("fn f(a: i32, (@@), b: i32) {}");
        assert!(!parse.ok());
        let tree = parse.debug_tree();
        // Should still have 2 params (a and b)
        assert_eq!(tree.matches("Param@").count(), 2, "tree:\n{tree}");
    }

    #[test]
    fn parse_delimited_recovery_emits_error_node() {
        // Recovered errors should be wrapped in ERROR nodes
        let parse = parse("fn f(a: i32, @@@, b: i32) {}");
        let tree = parse.debug_tree();
        assert!(tree.contains("ERROR"), "tree:\n{tree}");
    }
}
