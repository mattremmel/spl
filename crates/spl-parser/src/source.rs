//! Token source for the parser.

use spl_lexer::SpannedToken;
use spl_syntax::SyntaxKind;
use std::ops::Range;

/// Token with its `SyntaxKind`.
#[derive(Debug, Clone)]
pub struct Token<'src> {
    pub kind: SyntaxKind,
    pub text: &'src str,
    pub span: Range<usize>,
}

impl<'src> Token<'src> {
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    #[allow(dead_code)]
    pub fn text(&self) -> &'src str {
        self.text
    }
}

/// Source of tokens for the parser.
pub struct Source<'src> {
    tokens: Vec<Token<'src>>,
    pos: usize,
}

impl<'src> Source<'src> {
    /// Create a new token source from lexer tokens.
    pub fn new(spanned_tokens: Vec<SpannedToken<'src>>) -> Self {
        let mut tokens: Vec<Token<'src>> = spanned_tokens
            .into_iter()
            .map(|st| Token {
                kind: SyntaxKind::from(st.token),
                text: st.text,
                span: st.span,
            })
            .collect();

        Self::fixup_chained_field_access(&mut tokens);
        Self { tokens, pos: 0 }
    }

    /// Split `DOT FLOAT_LITERAL` into `DOT INT_LITERAL DOT INT_LITERAL` for chained
    /// positional field access (e.g., `pair.0.1`).
    fn fixup_chained_field_access(tokens: &mut Vec<Token<'src>>) {
        let mut i = 0;
        while i + 1 < tokens.len() {
            if tokens[i].kind == SyntaxKind::DOT && tokens[i + 1].kind == SyntaxKind::FLOAT_LITERAL
            {
                let text = tokens[i + 1].text;
                let span_start = tokens[i + 1].span.start;

                // Only split simple digit.digit patterns (no exponent, no suffix)
                if let Some(dot_pos) = text.find('.') {
                    let first = &text[..dot_pos];
                    let second = &text[dot_pos + 1..];
                    if !first.is_empty()
                        && !second.is_empty()
                        && first.bytes().all(|b| b.is_ascii_digit() || b == b'_')
                        && second.bytes().all(|b| b.is_ascii_digit() || b == b'_')
                    {
                        let int1 = Token {
                            kind: SyntaxKind::INT_LITERAL,
                            text: first,
                            span: span_start..span_start + dot_pos,
                        };
                        let dot = Token {
                            kind: SyntaxKind::DOT,
                            text: &text[dot_pos..=dot_pos],
                            span: span_start + dot_pos..span_start + dot_pos + 1,
                        };
                        let int2 = Token {
                            kind: SyntaxKind::INT_LITERAL,
                            text: second,
                            span: span_start + dot_pos + 1..span_start + text.len(),
                        };
                        tokens.splice(i + 1..i + 2, [int1, dot, int2]);
                        i += 4; // skip past DOT INT DOT INT
                        continue;
                    }
                }
            }
            i += 1;
        }
    }

    /// Get the current token kind, skipping trivia.
    pub fn current(&self) -> Option<SyntaxKind> {
        self.peek_non_trivia(0)
    }

    /// Peek at a token kind, skipping trivia.
    #[allow(dead_code)]
    pub fn peek(&self, n: usize) -> Option<SyntaxKind> {
        self.peek_non_trivia(n)
    }

    /// Get the current token's text range.
    pub fn current_range(&self) -> Range<usize> {
        self.current_token().map(|t| t.span.clone()).unwrap_or(0..0)
    }

    /// Get the current token's text.
    pub fn current_text(&self) -> Option<&'src str> {
        self.current_token().map(|t| t.text)
    }

    /// Bump the current token.
    pub fn bump(&mut self) -> Option<&Token<'src>> {
        // Skip trivia first
        self.skip_trivia();
        if self.pos < self.tokens.len() {
            let token = &self.tokens[self.pos];
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    /// Convert into the raw tokens for the sink.
    pub fn into_tokens(self) -> Vec<Token<'src>> {
        self.tokens
    }

    // === Internal helpers ===

    fn current_token(&self) -> Option<&Token<'src>> {
        let mut pos = self.pos;
        while pos < self.tokens.len() {
            let token = &self.tokens[pos];
            if !token.kind.is_trivia() {
                return Some(token);
            }
            pos += 1;
        }
        None
    }

    fn peek_non_trivia(&self, n: usize) -> Option<SyntaxKind> {
        let mut pos = self.pos;
        let mut count = 0;
        while pos < self.tokens.len() {
            let token = &self.tokens[pos];
            if !token.kind.is_trivia() {
                if count == n {
                    return Some(token.kind);
                }
                count += 1;
            }
            pos += 1;
        }
        None
    }

    fn skip_trivia(&mut self) {
        while self.pos < self.tokens.len() && self.tokens[self.pos].kind.is_trivia() {
            self.pos += 1;
        }
    }

    // ===== Contract Helpers =====

    /// Returns the current token position (index into tokens array).
    /// Used for contract assertions to verify parser advancement.
    pub fn token_position(&self) -> usize {
        self.pos
    }
}
