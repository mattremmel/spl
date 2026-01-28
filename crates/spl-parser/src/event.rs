//! Parse events emitted during parsing.

use spl_syntax::SyntaxKind;
use std::ops::Range;

/// Events emitted during parsing.
#[derive(Debug, Clone)]
pub enum Event {
    /// Start a new node with the given kind.
    Start {
        kind: SyntaxKind,
        /// If set, this node should become a child of the node
        /// at position `pos + forward_parent`.
        forward_parent: Option<usize>,
    },

    /// Finish the current node.
    Finish,

    /// Add a token to the current node.
    Token {
        #[allow(dead_code)] // Used for debugging/future features
        kind: SyntaxKind,
        n_raw_tokens: u8,
    },

    /// Synthetic token with specified text (not from source).
    SyntheticToken { kind: SyntaxKind, text: String },

    /// A parse error.
    Error(ParseError),

    /// Placeholder for a Start event that will be filled in later.
    Placeholder,
}

/// A parse error with location information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub range: Range<usize>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}..{}",
            self.message, self.range.start, self.range.end
        )
    }
}

impl std::error::Error for ParseError {}
