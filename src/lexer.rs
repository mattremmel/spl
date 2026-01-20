//! SPL Lexer - Tokenizes SPL source code
//!
//! Uses the `logos` crate for efficient lexical analysis.
//!
//! # Error Recovery
//!
//! The lexer implements error recovery by continuing to produce tokens after
//! encountering invalid input. Errors are collected separately and can be
//! retrieved along with the token stream using [`lex_all`].

use logos::Logos;
use std::fmt;

/// All token types in the SPL language.
#[derive(Logos, Debug, Clone, PartialEq)]
pub enum Token {
    // === Trivia (whitespace and comments) ===
    /// Whitespace (spaces, tabs, newlines)
    #[regex(r"[ \t\n\r]+")]
    Whitespace,

    /// Line comment (// ...)
    #[regex(r"//[^\n]*")]
    LineComment,

    /// Block comment (/* ... */)
    #[regex(r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")]
    BlockComment,

    // === Keywords ===
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("fn")]
    Fn,
    #[token("struct")]
    Struct,
    #[token("type")]
    Type,
    #[token("impl")]
    Impl,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("loop")]
    Loop,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("return")]
    Return,
    #[token("as")]
    As,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("pub")]
    Pub,
    #[token("Self")]
    SelfType,
    #[token("self")]
    SelfValue,

    // === Operators ===
    // Arithmetic
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    // Comparison (multi-char first for longest match)
    #[token("==")]
    EqEq,
    #[token("!=")]
    Ne,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,

    // Logical
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("!")]
    Bang,

    // Assignment (multi-char first)
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("%=")]
    PercentEq,
    #[token("=")]
    Eq,

    // Other operators
    #[token("->")]
    Arrow,
    #[token("::")]
    ColonColon,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token("&")]
    Amp,
    #[token("$")]
    Dollar,

    // === Delimiters ===
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(";")]
    Semi,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,

    // === Literals ===
    // Float must come before Integer to handle 3.14 correctly
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*(e[+-]?[0-9][0-9_]*)?")]
    #[regex(r"[0-9][0-9_]*e[+-]?[0-9][0-9_]*")]
    Float,

    #[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*")]
    #[regex(r"0b[01][01_]*")]
    #[regex(r"0o[0-7][0-7_]*")]
    #[regex(r"[0-9][0-9_]*")]
    Integer,

    #[regex(r#""([^"\\]|\\.)*""#)]
    String,

    #[regex(r"'([^'\\]|\\.)?'")]
    Char,

    // === Identifier ===
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

    // === Error ===
    /// Represents invalid/unrecognized input
    Error,
}

/// A span representing the byte range of a token in the source code.
pub type Span = std::ops::Range<usize>;

/// Kinds of lexer errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexErrorKind {
    /// An unrecognized character was encountered.
    InvalidCharacter(char),
    /// A string literal was not terminated before end of input.
    UnterminatedString,
    /// A character literal was not terminated before end of input.
    UnterminatedChar,
    /// A block comment was not terminated before end of input.
    UnterminatedBlockComment,
    /// An empty character literal `''` was encountered.
    EmptyCharLiteral,
    /// A character literal contains multiple characters (not an escape).
    MultiCharacterLiteral,
    /// An invalid escape sequence was encountered.
    InvalidEscape(char),
    /// Generic error for unrecognized input.
    Unknown,
}

impl fmt::Display for LexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexErrorKind::InvalidCharacter(c) => {
                write!(f, "invalid character '{}'", c.escape_default())
            }
            LexErrorKind::UnterminatedString => write!(f, "unterminated string literal"),
            LexErrorKind::UnterminatedChar => write!(f, "unterminated character literal"),
            LexErrorKind::UnterminatedBlockComment => write!(f, "unterminated block comment"),
            LexErrorKind::EmptyCharLiteral => write!(f, "empty character literal"),
            LexErrorKind::MultiCharacterLiteral => {
                write!(f, "character literal contains multiple characters")
            }
            LexErrorKind::InvalidEscape(c) => {
                write!(f, "invalid escape sequence '\\{}'", c.escape_default())
            }
            LexErrorKind::Unknown => write!(f, "unrecognized input"),
        }
    }
}

/// A lexer error with location information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    /// The kind of error.
    pub kind: LexErrorKind,
    /// The byte range in the source where the error occurred.
    pub span: Span,
}

impl LexError {
    /// Create a new lexer error.
    pub fn new(kind: LexErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}..{}", self.kind, self.span.start, self.span.end)
    }
}

impl std::error::Error for LexError {}

/// A token with its source text and position information.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken<'a> {
    /// The token kind
    pub token: Token,
    /// The original source text for this token
    pub text: &'a str,
    /// The byte range in the source
    pub span: Span,
}

/// Lexer wrapper that iterates over tokens in source code.
pub struct Lexer<'a> {
    inner: logos::Lexer<'a, Token>,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source code.
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: Token::lexer(source),
        }
    }

    /// Returns the full source string being lexed.
    pub fn source(&self) -> &'a str {
        self.inner.source()
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = SpannedToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let kind = self.inner.next()?;
        let text = self.inner.slice();
        let span = self.inner.span();
        let token = match kind {
            Ok(token) => token,
            Err(()) => Token::Error,
        };
        Some(SpannedToken { token, text, span })
    }
}

/// Result of lexing source code, containing both tokens and any errors encountered.
#[derive(Debug, Clone, PartialEq)]
pub struct LexResult<'a> {
    /// All tokens produced, including error tokens for invalid input.
    pub tokens: Vec<SpannedToken<'a>>,
    /// Errors encountered during lexing, with detailed information.
    pub errors: Vec<LexError>,
}

impl<'a> LexResult<'a> {
    /// Returns true if lexing completed without errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns true if there were any lexing errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Lex source code and collect all tokens and errors.
///
/// This function lexes the entire input, continuing after errors to report
/// as many problems as possible. Error tokens are included in the token stream
/// and detailed error information is collected separately.
///
/// # Example
///
/// ```
/// use spl::lexer::{lex_all, Token, LexErrorKind};
///
/// let result = lex_all("let x = @;");
/// assert!(result.has_errors());
/// assert_eq!(result.errors[0].kind, LexErrorKind::InvalidCharacter('@'));
///
/// // Valid tokens are still produced
/// assert!(result.tokens.iter().any(|t| t.token == Token::Let));
/// ```
pub fn lex_all(source: &str) -> LexResult<'_> {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    for spanned in Lexer::new(source) {
        if spanned.token == Token::Error {
            let error = classify_error(source, &spanned);
            errors.push(error);
        }
        tokens.push(spanned);
    }

    // Check for unterminated constructs at end of input
    if let Some(error) = check_unterminated(source) {
        errors.push(error);
    }

    LexResult { tokens, errors }
}

/// Classify an error token into a specific error kind.
fn classify_error(source: &str, token: &SpannedToken<'_>) -> LexError {
    let text = token.text;
    let span = token.span.clone();

    // Single character errors
    if let Some(c) = text.chars().next() {
        // Check for unterminated string starting with "
        if c == '"' {
            return LexError::new(LexErrorKind::UnterminatedString, span);
        }
        // Check for unterminated char starting with '
        if c == '\'' {
            if text == "''" {
                return LexError::new(LexErrorKind::EmptyCharLiteral, span);
            }
            return LexError::new(LexErrorKind::UnterminatedChar, span);
        }
        // Check for unterminated block comment
        if text.starts_with("/*") {
            return LexError::new(LexErrorKind::UnterminatedBlockComment, span);
        }
        // Single invalid character
        if text.len() == c.len_utf8() {
            return LexError::new(LexErrorKind::InvalidCharacter(c), span);
        }
    }

    // Check for invalid escape sequences in strings/chars
    if text.contains('\\') {
        // Try to find the invalid escape
        let bytes = text.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\\' && i + 1 < bytes.len() {
                let next = bytes[i + 1] as char;
                if !matches!(next, 'n' | 'r' | 't' | '\\' | '\'' | '"' | '0') {
                    return LexError::new(LexErrorKind::InvalidEscape(next), span);
                }
            }
        }
    }

    // Check the source around the error for context
    let start = span.start;
    if start < source.len() {
        let remaining = &source[start..];
        // Check if we're at the start of an unterminated string
        if remaining.starts_with('"') && !remaining[1..].contains('"') {
            return LexError::new(LexErrorKind::UnterminatedString, span);
        }
        // Check if we're at the start of an unterminated char
        if remaining.starts_with('\'') {
            return LexError::new(LexErrorKind::UnterminatedChar, span);
        }
    }

    LexError::new(LexErrorKind::Unknown, span)
}

/// Check for unterminated constructs at end of input.
fn check_unterminated(source: &str) -> Option<LexError> {
    let mut in_string = false;
    let mut in_char = false;
    let mut in_block_comment = false;
    let mut string_start = 0;
    let mut char_start = 0;
    let mut comment_start = 0;
    let mut escape_next = false;

    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        if escape_next {
            escape_next = false;
            i += 1;
            continue;
        }

        if in_string {
            if b == b'\\' {
                escape_next = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if in_char {
            if b == b'\\' {
                escape_next = true;
            } else if b == b'\'' {
                in_char = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        // Check for start of constructs
        if b == b'"' {
            in_string = true;
            string_start = i;
        } else if b == b'\'' {
            in_char = true;
            char_start = i;
        } else if b == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'*' {
                in_block_comment = true;
                comment_start = i;
                i += 2;
                continue;
            } else if bytes[i + 1] == b'/' {
                // Line comment - skip to end of line
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
        }
        i += 1;
    }

    // Check for unterminated constructs
    if in_string {
        return Some(LexError::new(
            LexErrorKind::UnterminatedString,
            string_start..source.len(),
        ));
    }
    if in_char {
        return Some(LexError::new(
            LexErrorKind::UnterminatedChar,
            char_start..source.len(),
        ));
    }
    if in_block_comment {
        return Some(LexError::new(
            LexErrorKind::UnterminatedBlockComment,
            comment_start..source.len(),
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Helper to lex a string and collect all tokens (without spans for simpler assertions)
    fn lex(source: &str) -> Vec<(Token, &str)> {
        Lexer::new(source).map(|st| (st.token, st.text)).collect()
    }

    /// Helper to lex and filter out trivia (whitespace and comments)
    fn lex_no_trivia(source: &str) -> Vec<(Token, &str)> {
        Lexer::new(source)
            .filter(|st| !matches!(st.token, Token::Whitespace | Token::LineComment | Token::BlockComment))
            .map(|st| (st.token, st.text))
            .collect()
    }

    /// Helper to lex with full span information
    fn lex_spanned(source: &str) -> Vec<SpannedToken<'_>> {
        Lexer::new(source).collect()
    }

    /// Helper to check that source lexes to expected tokens (ignoring trivia)
    fn check(source: &str, expected: &[(Token, &str)]) {
        assert_eq!(lex_no_trivia(source), expected);
    }

    /// Helper to check all tokens including trivia
    fn check_with_trivia(source: &str, expected: &[(Token, &str)]) {
        assert_eq!(lex(source), expected);
    }

    /// Helper to check single token
    fn check_single(source: &str, expected_token: Token) {
        let tokens = lex(source);
        assert_eq!(tokens.len(), 1, "Expected single token, got: {:?}", tokens);
        assert_eq!(tokens[0].0, expected_token);
        assert_eq!(tokens[0].1, source);
    }

    // ============================================================
    // Keywords
    // ============================================================

    #[test]
    fn keyword_let() {
        check_single("let", Token::Let);
    }

    #[test]
    fn keyword_mut() {
        check_single("mut", Token::Mut);
    }

    #[test]
    fn keyword_fn() {
        check_single("fn", Token::Fn);
    }

    #[test]
    fn keyword_struct() {
        check_single("struct", Token::Struct);
    }

    #[test]
    fn keyword_type() {
        check_single("type", Token::Type);
    }

    #[test]
    fn keyword_impl() {
        check_single("impl", Token::Impl);
    }

    #[test]
    fn keyword_if() {
        check_single("if", Token::If);
    }

    #[test]
    fn keyword_else() {
        check_single("else", Token::Else);
    }

    #[test]
    fn keyword_while() {
        check_single("while", Token::While);
    }

    #[test]
    fn keyword_for() {
        check_single("for", Token::For);
    }

    #[test]
    fn keyword_in() {
        check_single("in", Token::In);
    }

    #[test]
    fn keyword_loop() {
        check_single("loop", Token::Loop);
    }

    #[test]
    fn keyword_break() {
        check_single("break", Token::Break);
    }

    #[test]
    fn keyword_continue() {
        check_single("continue", Token::Continue);
    }

    #[test]
    fn keyword_return() {
        check_single("return", Token::Return);
    }

    #[test]
    fn keyword_as() {
        check_single("as", Token::As);
    }

    #[test]
    fn keyword_true() {
        check_single("true", Token::True);
    }

    #[test]
    fn keyword_false() {
        check_single("false", Token::False);
    }

    #[test]
    fn keyword_pub() {
        check_single("pub", Token::Pub);
    }

    #[test]
    fn keyword_self_type() {
        check_single("Self", Token::SelfType);
    }

    #[test]
    fn keyword_self_value() {
        check_single("self", Token::SelfValue);
    }

    // ============================================================
    // Operators - Arithmetic
    // ============================================================

    #[test]
    fn op_plus() {
        check_single("+", Token::Plus);
    }

    #[test]
    fn op_minus() {
        check_single("-", Token::Minus);
    }

    #[test]
    fn op_star() {
        check_single("*", Token::Star);
    }

    #[test]
    fn op_slash() {
        check_single("/", Token::Slash);
    }

    #[test]
    fn op_percent() {
        check_single("%", Token::Percent);
    }

    // ============================================================
    // Operators - Comparison
    // ============================================================

    #[test]
    fn op_eq_eq() {
        check_single("==", Token::EqEq);
    }

    #[test]
    fn op_ne() {
        check_single("!=", Token::Ne);
    }

    #[test]
    fn op_lt() {
        check_single("<", Token::Lt);
    }

    #[test]
    fn op_gt() {
        check_single(">", Token::Gt);
    }

    #[test]
    fn op_le() {
        check_single("<=", Token::Le);
    }

    #[test]
    fn op_ge() {
        check_single(">=", Token::Ge);
    }

    // ============================================================
    // Operators - Logical
    // ============================================================

    #[test]
    fn op_and_and() {
        check_single("&&", Token::AndAnd);
    }

    #[test]
    fn op_or_or() {
        check_single("||", Token::OrOr);
    }

    #[test]
    fn op_bang() {
        check_single("!", Token::Bang);
    }

    // ============================================================
    // Operators - Assignment
    // ============================================================

    #[test]
    fn op_eq() {
        check_single("=", Token::Eq);
    }

    #[test]
    fn op_plus_eq() {
        check_single("+=", Token::PlusEq);
    }

    #[test]
    fn op_minus_eq() {
        check_single("-=", Token::MinusEq);
    }

    #[test]
    fn op_star_eq() {
        check_single("*=", Token::StarEq);
    }

    #[test]
    fn op_slash_eq() {
        check_single("/=", Token::SlashEq);
    }

    #[test]
    fn op_percent_eq() {
        check_single("%=", Token::PercentEq);
    }

    // ============================================================
    // Operators - Other
    // ============================================================

    #[test]
    fn op_arrow() {
        check_single("->", Token::Arrow);
    }

    #[test]
    fn op_dot() {
        check_single(".", Token::Dot);
    }

    #[test]
    fn op_colon_colon() {
        check_single("::", Token::ColonColon);
    }

    #[test]
    fn op_amp() {
        check_single("&", Token::Amp);
    }

    #[test]
    fn op_dot_dot() {
        check_single("..", Token::DotDot);
    }

    #[test]
    fn op_dollar() {
        check_single("$", Token::Dollar);
    }

    // ============================================================
    // Delimiters
    // ============================================================

    #[test]
    fn delim_lparen() {
        check_single("(", Token::LParen);
    }

    #[test]
    fn delim_rparen() {
        check_single(")", Token::RParen);
    }

    #[test]
    fn delim_lbrace() {
        check_single("{", Token::LBrace);
    }

    #[test]
    fn delim_rbrace() {
        check_single("}", Token::RBrace);
    }

    #[test]
    fn delim_lbracket() {
        check_single("[", Token::LBracket);
    }

    #[test]
    fn delim_rbracket() {
        check_single("]", Token::RBracket);
    }

    #[test]
    fn delim_semi() {
        check_single(";", Token::Semi);
    }

    #[test]
    fn delim_colon() {
        check_single(":", Token::Colon);
    }

    #[test]
    fn delim_comma() {
        check_single(",", Token::Comma);
    }

    // ============================================================
    // Integer Literals
    // ============================================================

    #[test]
    fn int_decimal() {
        check_single("42", Token::Integer);
    }

    #[test]
    fn int_decimal_with_underscores() {
        check_single("1_000_000", Token::Integer);
    }

    #[test]
    fn int_decimal_leading_zero() {
        check_single("007", Token::Integer);
    }

    #[test]
    fn int_hex() {
        check_single("0xFF", Token::Integer);
    }

    #[test]
    fn int_hex_lowercase() {
        check_single("0x2a", Token::Integer);
    }

    #[test]
    fn int_hex_with_underscores() {
        check_single("0xFF_FF", Token::Integer);
    }

    #[test]
    fn int_binary() {
        check_single("0b101010", Token::Integer);
    }

    #[test]
    fn int_binary_with_underscores() {
        check_single("0b1010_1010", Token::Integer);
    }

    #[test]
    fn int_octal() {
        check_single("0o52", Token::Integer);
    }

    #[test]
    fn int_octal_with_underscores() {
        check_single("0o7_7_7", Token::Integer);
    }

    // ============================================================
    // Float Literals
    // ============================================================

    #[test]
    fn float_basic() {
        check_single("3.14", Token::Float);
    }

    #[test]
    fn float_with_underscores() {
        check_single("1_000.000_001", Token::Float);
    }

    #[test]
    fn float_exponent() {
        check_single("1e10", Token::Float);
    }

    #[test]
    fn float_exponent_positive() {
        check_single("1e+10", Token::Float);
    }

    #[test]
    fn float_exponent_negative() {
        check_single("2e-3", Token::Float);
    }

    #[test]
    fn float_full() {
        check_single("2.5e-3", Token::Float);
    }

    #[test]
    fn float_full_with_underscores() {
        check_single("1_0.2_5e1_0", Token::Float);
    }

    // ============================================================
    // String Literals
    // ============================================================

    #[test]
    fn string_simple() {
        check_single(r#""hello""#, Token::String);
    }

    #[test]
    fn string_empty() {
        check_single(r#""""#, Token::String);
    }

    #[test]
    fn string_with_spaces() {
        check_single(r#""hello world""#, Token::String);
    }

    #[test]
    fn string_escape_newline() {
        check_single(r#""hello\nworld""#, Token::String);
    }

    #[test]
    fn string_escape_tab() {
        check_single(r#""hello\tworld""#, Token::String);
    }

    #[test]
    fn string_escape_carriage_return() {
        check_single(r#""hello\rworld""#, Token::String);
    }

    #[test]
    fn string_escape_backslash() {
        check_single(r#""path\\to\\file""#, Token::String);
    }

    #[test]
    fn string_escape_quote() {
        check_single(r#""say \"hi\"""#, Token::String);
    }

    #[test]
    fn string_escape_null() {
        check_single(r#""null\0char""#, Token::String);
    }

    // ============================================================
    // Character Literals
    // ============================================================

    #[test]
    fn char_simple() {
        check_single("'a'", Token::Char);
    }

    #[test]
    fn char_digit() {
        check_single("'0'", Token::Char);
    }

    #[test]
    fn char_escape_newline() {
        check_single(r"'\n'", Token::Char);
    }

    #[test]
    fn char_escape_tab() {
        check_single(r"'\t'", Token::Char);
    }

    #[test]
    fn char_escape_carriage_return() {
        check_single(r"'\r'", Token::Char);
    }

    #[test]
    fn char_escape_backslash() {
        check_single(r"'\\'", Token::Char);
    }

    #[test]
    fn char_escape_single_quote() {
        check_single(r"'\''", Token::Char);
    }

    #[test]
    fn char_escape_null() {
        check_single(r"'\0'", Token::Char);
    }

    // ============================================================
    // Identifiers
    // ============================================================

    #[test]
    fn ident_simple() {
        check_single("foo", Token::Ident);
    }

    #[test]
    fn ident_with_underscore() {
        check_single("foo_bar", Token::Ident);
    }

    #[test]
    fn ident_starting_underscore() {
        check_single("_private", Token::Ident);
    }

    #[test]
    fn ident_double_underscore() {
        check_single("__internal", Token::Ident);
    }

    #[test]
    fn ident_with_digits() {
        check_single("Point2D", Token::Ident);
    }

    #[test]
    fn ident_uppercase() {
        check_single("FOO", Token::Ident);
    }

    #[test]
    fn ident_mixed_case() {
        check_single("FooBar", Token::Ident);
    }

    // ============================================================
    // Comments (should be skipped)
    // ============================================================

    #[test]
    fn comment_line_only() {
        check("// this is a comment", &[]);
    }

    #[test]
    fn comment_line_with_token_after() {
        check("// comment\nlet", &[(Token::Let, "let")]);
    }

    #[test]
    fn comment_line_inline() {
        check("let // comment", &[(Token::Let, "let")]);
    }

    #[test]
    fn comment_block_only() {
        check("/* block comment */", &[]);
    }

    #[test]
    fn comment_block_multiline() {
        check("/* multi\nline\ncomment */", &[]);
    }

    #[test]
    fn comment_block_with_tokens() {
        check(
            "let /* comment */ mut",
            &[(Token::Let, "let"), (Token::Mut, "mut")],
        );
    }

    // ============================================================
    // Whitespace (should be skipped)
    // ============================================================

    #[test]
    fn whitespace_spaces() {
        check("let   mut", &[(Token::Let, "let"), (Token::Mut, "mut")]);
    }

    #[test]
    fn whitespace_tabs() {
        check("let\t\tmut", &[(Token::Let, "let"), (Token::Mut, "mut")]);
    }

    #[test]
    fn whitespace_newlines() {
        check("let\n\nmut", &[(Token::Let, "let"), (Token::Mut, "mut")]);
    }

    #[test]
    fn whitespace_carriage_return() {
        check("let\r\nmut", &[(Token::Let, "let"), (Token::Mut, "mut")]);
    }

    #[test]
    fn whitespace_mixed() {
        check(
            "let \t\n  \r\n  mut",
            &[(Token::Let, "let"), (Token::Mut, "mut")],
        );
    }

    // ============================================================
    // Ambiguity Resolution
    // ============================================================

    #[test]
    fn ambiguity_range_not_float() {
        // 1..2 should be Integer, DotDot, Integer (not malformed float)
        check(
            "1..2",
            &[
                (Token::Integer, "1"),
                (Token::DotDot, ".."),
                (Token::Integer, "2"),
            ],
        );
    }

    #[test]
    fn ambiguity_keyword_vs_ident() {
        // "letter" should be identifier, not "let" + "ter"
        check_single("letter", Token::Ident);
    }

    #[test]
    fn ambiguity_longest_match_eq() {
        // "==" should be EqEq, not two Eq tokens
        check_single("==", Token::EqEq);
    }

    #[test]
    fn ambiguity_eq_followed_by_eq() {
        // "= =" with space should be two Eq tokens
        check("= =", &[(Token::Eq, "="), (Token::Eq, "=")]);
    }

    #[test]
    fn ambiguity_arrow_not_minus_gt() {
        check_single("->", Token::Arrow);
    }

    #[test]
    fn ambiguity_colon_colon_not_two_colons() {
        check_single("::", Token::ColonColon);
    }

    // ============================================================
    // Combined/Integration Tests
    // ============================================================

    #[test]
    fn combined_let_statement() {
        check(
            "let x = 42;",
            &[
                (Token::Let, "let"),
                (Token::Ident, "x"),
                (Token::Eq, "="),
                (Token::Integer, "42"),
                (Token::Semi, ";"),
            ],
        );
    }

    #[test]
    fn combined_function_signature() {
        check(
            "fn add(a: i32, b: i32) -> i32",
            &[
                (Token::Fn, "fn"),
                (Token::Ident, "add"),
                (Token::LParen, "("),
                (Token::Ident, "a"),
                (Token::Colon, ":"),
                (Token::Ident, "i32"),
                (Token::Comma, ","),
                (Token::Ident, "b"),
                (Token::Colon, ":"),
                (Token::Ident, "i32"),
                (Token::RParen, ")"),
                (Token::Arrow, "->"),
                (Token::Ident, "i32"),
            ],
        );
    }

    #[test]
    fn combined_struct_definition() {
        check(
            "pub struct Point { x: f64, y: f64 }",
            &[
                (Token::Pub, "pub"),
                (Token::Struct, "struct"),
                (Token::Ident, "Point"),
                (Token::LBrace, "{"),
                (Token::Ident, "x"),
                (Token::Colon, ":"),
                (Token::Ident, "f64"),
                (Token::Comma, ","),
                (Token::Ident, "y"),
                (Token::Colon, ":"),
                (Token::Ident, "f64"),
                (Token::RBrace, "}"),
            ],
        );
    }

    #[test]
    fn combined_method_call() {
        check(
            "point.distance(&other)",
            &[
                (Token::Ident, "point"),
                (Token::Dot, "."),
                (Token::Ident, "distance"),
                (Token::LParen, "("),
                (Token::Amp, "&"),
                (Token::Ident, "other"),
                (Token::RParen, ")"),
            ],
        );
    }

    #[test]
    fn combined_if_else() {
        check(
            "if x > 0 { true } else { false }",
            &[
                (Token::If, "if"),
                (Token::Ident, "x"),
                (Token::Gt, ">"),
                (Token::Integer, "0"),
                (Token::LBrace, "{"),
                (Token::True, "true"),
                (Token::RBrace, "}"),
                (Token::Else, "else"),
                (Token::LBrace, "{"),
                (Token::False, "false"),
                (Token::RBrace, "}"),
            ],
        );
    }

    #[test]
    fn combined_for_loop_range() {
        check(
            "for i in 0..10 { }",
            &[
                (Token::For, "for"),
                (Token::Ident, "i"),
                (Token::In, "in"),
                (Token::Integer, "0"),
                (Token::DotDot, ".."),
                (Token::Integer, "10"),
                (Token::LBrace, "{"),
                (Token::RBrace, "}"),
            ],
        );
    }

    #[test]
    fn combined_compound_assignment() {
        check(
            "x += 1.5e1;",
            &[
                (Token::Ident, "x"),
                (Token::PlusEq, "+="),
                (Token::Float, "1.5e1"),
                (Token::Semi, ";"),
            ],
        );
    }

    #[test]
    fn combined_type_cast() {
        check(
            "0x0A as f64",
            &[
                (Token::Integer, "0x0A"),
                (Token::As, "as"),
                (Token::Ident, "f64"),
            ],
        );
    }

    #[test]
    fn combined_path_expression() {
        check(
            "Point::new(0.0, 0.0)",
            &[
                (Token::Ident, "Point"),
                (Token::ColonColon, "::"),
                (Token::Ident, "new"),
                (Token::LParen, "("),
                (Token::Float, "0.0"),
                (Token::Comma, ","),
                (Token::Float, "0.0"),
                (Token::RParen, ")"),
            ],
        );
    }

    #[test]
    fn combined_logical_expression() {
        check(
            "flag && dist > 0.0 || !done",
            &[
                (Token::Ident, "flag"),
                (Token::AndAnd, "&&"),
                (Token::Ident, "dist"),
                (Token::Gt, ">"),
                (Token::Float, "0.0"),
                (Token::OrOr, "||"),
                (Token::Bang, "!"),
                (Token::Ident, "done"),
            ],
        );
    }

    #[test]
    fn combined_range_index() {
        // Note: This is range indexing (0..$), not slice syntax (0:$)
        // Slice syntax uses colon, e.g., arr[0:$]
        check(
            "arr[0..$]",
            &[
                (Token::Ident, "arr"),
                (Token::LBracket, "["),
                (Token::Integer, "0"),
                (Token::DotDot, ".."),
                (Token::Dollar, "$"),
                (Token::RBracket, "]"),
            ],
        );
    }

    #[test]
    fn combined_slice_syntax() {
        // Slice syntax uses colon as delimiter
        check(
            "arr[1:3]",
            &[
                (Token::Ident, "arr"),
                (Token::LBracket, "["),
                (Token::Integer, "1"),
                (Token::Colon, ":"),
                (Token::Integer, "3"),
                (Token::RBracket, "]"),
            ],
        );
    }

    #[test]
    fn combined_slice_to_end() {
        check(
            "arr[1:$]",
            &[
                (Token::Ident, "arr"),
                (Token::LBracket, "["),
                (Token::Integer, "1"),
                (Token::Colon, ":"),
                (Token::Dollar, "$"),
                (Token::RBracket, "]"),
            ],
        );
    }

    #[test]
    fn combined_slice_from_start() {
        check(
            "arr[:3]",
            &[
                (Token::Ident, "arr"),
                (Token::LBracket, "["),
                (Token::Colon, ":"),
                (Token::Integer, "3"),
                (Token::RBracket, "]"),
            ],
        );
    }

    #[test]
    fn combined_slice_full() {
        check(
            "arr[:]",
            &[
                (Token::Ident, "arr"),
                (Token::LBracket, "["),
                (Token::Colon, ":"),
                (Token::RBracket, "]"),
            ],
        );
    }

    #[test]
    fn combined_impl_self() {
        check(
            "impl Point { fn new() -> Self { } }",
            &[
                (Token::Impl, "impl"),
                (Token::Ident, "Point"),
                (Token::LBrace, "{"),
                (Token::Fn, "fn"),
                (Token::Ident, "new"),
                (Token::LParen, "("),
                (Token::RParen, ")"),
                (Token::Arrow, "->"),
                (Token::SelfType, "Self"),
                (Token::LBrace, "{"),
                (Token::RBrace, "}"),
                (Token::RBrace, "}"),
            ],
        );
    }

    #[test]
    fn combined_with_comments() {
        check(
            "// comment\nlet x = 42; /* inline */ let y = 0;",
            &[
                (Token::Let, "let"),
                (Token::Ident, "x"),
                (Token::Eq, "="),
                (Token::Integer, "42"),
                (Token::Semi, ";"),
                (Token::Let, "let"),
                (Token::Ident, "y"),
                (Token::Eq, "="),
                (Token::Integer, "0"),
                (Token::Semi, ";"),
            ],
        );
    }

    #[test]
    fn combined_method_with_self() {
        check(
            "fn distance(&self, other: &Point) -> f64",
            &[
                (Token::Fn, "fn"),
                (Token::Ident, "distance"),
                (Token::LParen, "("),
                (Token::Amp, "&"),
                (Token::SelfValue, "self"),
                (Token::Comma, ","),
                (Token::Ident, "other"),
                (Token::Colon, ":"),
                (Token::Amp, "&"),
                (Token::Ident, "Point"),
                (Token::RParen, ")"),
                (Token::Arrow, "->"),
                (Token::Ident, "f64"),
            ],
        );
    }

    #[test]
    fn combined_self_field_access() {
        check(
            "self.x + self.y",
            &[
                (Token::SelfValue, "self"),
                (Token::Dot, "."),
                (Token::Ident, "x"),
                (Token::Plus, "+"),
                (Token::SelfValue, "self"),
                (Token::Dot, "."),
                (Token::Ident, "y"),
            ],
        );
    }

    #[test]
    fn combined_mut_self() {
        check(
            "&mut self",
            &[
                (Token::Amp, "&"),
                (Token::Mut, "mut"),
                (Token::SelfValue, "self"),
            ],
        );
    }

    // ============================================================
    // Error Token Tests
    // ============================================================

    #[test]
    fn error_invalid_character() {
        // Backtick is not a valid token in SPL
        check("`", &[(Token::Error, "`")]);
    }

    #[test]
    fn error_at_sign() {
        check("@", &[(Token::Error, "@")]);
    }

    #[test]
    fn error_hash() {
        check("#", &[(Token::Error, "#")]);
    }

    #[test]
    fn error_tilde() {
        check("~", &[(Token::Error, "~")]);
    }

    #[test]
    fn error_with_valid_tokens() {
        // Error token should not prevent lexing of valid tokens
        check(
            "let @ x",
            &[
                (Token::Let, "let"),
                (Token::Error, "@"),
                (Token::Ident, "x"),
            ],
        );
    }

    #[test]
    fn error_incomplete_hex() {
        // "0x" without digits is an error followed by 'x' as ident
        let tokens = lex("0x");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].0, Token::Integer); // "0" is valid decimal
        assert_eq!(tokens[1].0, Token::Ident); // "x" is an ident
    }

    #[test]
    fn error_incomplete_binary() {
        // "0b" without digits
        let tokens = lex("0b");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].0, Token::Integer); // "0"
        assert_eq!(tokens[1].0, Token::Ident); // "b"
    }

    #[test]
    fn error_incomplete_octal() {
        // "0o" without digits
        let tokens = lex("0o");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].0, Token::Integer); // "0"
        assert_eq!(tokens[1].0, Token::Ident); // "o"
    }

    #[test]
    fn error_unterminated_string() {
        // Unterminated string should produce error
        let tokens = lex("\"hello");
        assert!(tokens.iter().any(|(t, _)| *t == Token::Error));
    }

    #[test]
    fn error_unterminated_char() {
        let tokens = lex("'a");
        assert!(tokens.iter().any(|(t, _)| *t == Token::Error));
    }

    // ============================================================
    // Span Tests
    // ============================================================

    #[test]
    fn span_single_token() {
        let tokens = lex_spanned("let");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Let);
        assert_eq!(tokens[0].span, 0..3);
    }

    #[test]
    fn span_multiple_tokens() {
        // Now includes whitespace tokens
        let tokens = lex_spanned("let x = 42;");
        assert_eq!(tokens.len(), 8); // let, ws, x, ws, =, ws, 42, ;
        assert_eq!(tokens[0].span, 0..3); // "let"
        assert_eq!(tokens[0].token, Token::Let);
        assert_eq!(tokens[1].span, 3..4); // " "
        assert_eq!(tokens[1].token, Token::Whitespace);
        assert_eq!(tokens[2].span, 4..5); // "x"
        assert_eq!(tokens[2].token, Token::Ident);
        assert_eq!(tokens[7].span, 10..11); // ";"
    }

    #[test]
    fn span_with_comments() {
        // Now comment is included as a token
        let tokens = lex_spanned("/* comment */let");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].token, Token::BlockComment);
        assert_eq!(tokens[0].span, 0..13);
        assert_eq!(tokens[1].token, Token::Let);
        assert_eq!(tokens[1].span, 13..16);
    }

    #[test]
    fn span_multiline() {
        // Now includes whitespace token
        let source = "let\n  x";
        let tokens = lex_spanned(source);
        assert_eq!(tokens.len(), 3); // let, whitespace, x
        assert_eq!(tokens[0].span, 0..3); // "let"
        assert_eq!(tokens[1].span, 3..6); // "\n  "
        assert_eq!(tokens[1].token, Token::Whitespace);
        assert_eq!(tokens[2].span, 6..7); // "x"
    }

    // ============================================================
    // Edge Case Tests
    // ============================================================

    #[test]
    fn edge_single_underscore() {
        // Single underscore is a valid identifier (wildcard pattern)
        check_single("_", Token::Ident);
    }

    #[test]
    fn edge_hex_with_all_letters() {
        check_single("0xABCDEF", Token::Integer);
    }

    #[test]
    fn edge_hex_lowercase() {
        check_single("0xabcdef", Token::Integer);
    }

    #[test]
    fn edge_binary_all_ones() {
        check_single("0b11111111", Token::Integer);
    }

    #[test]
    fn edge_binary_all_zeros() {
        check_single("0b00000000", Token::Integer);
    }

    #[test]
    fn edge_octal_max_digits() {
        check_single("0o77777777", Token::Integer);
    }

    #[test]
    fn edge_float_zero() {
        check_single("0.0", Token::Float);
    }

    #[test]
    fn edge_float_many_decimals() {
        check_single("3.141592653589793", Token::Float);
    }

    #[test]
    fn edge_string_with_all_escapes() {
        check_single(
            r#""line\nreturn\rtab\tbackslash\\quote\"null\0""#,
            Token::String,
        );
    }

    #[test]
    fn edge_char_space() {
        check_single("' '", Token::Char);
    }

    #[test]
    fn edge_empty_block_comment() {
        check("/**/let", &[(Token::Let, "let")]);
    }

    #[test]
    fn edge_nested_looking_comment() {
        // Block comments don't nest - /* inner */ closes at first */
        check("/* outer /* inner */ let", &[(Token::Let, "let")]);
    }

    #[test]
    fn edge_consecutive_operators() {
        check(
            "++--",
            &[
                (Token::Plus, "+"),
                (Token::Plus, "+"),
                (Token::Minus, "-"),
                (Token::Minus, "-"),
            ],
        );
    }

    #[test]
    fn edge_dot_vs_dotdot() {
        check(
            "a.b..c",
            &[
                (Token::Ident, "a"),
                (Token::Dot, "."),
                (Token::Ident, "b"),
                (Token::DotDot, ".."),
                (Token::Ident, "c"),
            ],
        );
    }

    #[test]
    fn edge_colon_vs_coloncolon() {
        check(
            "a:b::c",
            &[
                (Token::Ident, "a"),
                (Token::Colon, ":"),
                (Token::Ident, "b"),
                (Token::ColonColon, "::"),
                (Token::Ident, "c"),
            ],
        );
    }

    #[test]
    fn edge_amp_vs_andand() {
        check("&&&", &[(Token::AndAnd, "&&"), (Token::Amp, "&")]);
    }

    #[test]
    fn edge_all_comparison_operators() {
        check(
            "< > <= >= == !=",
            &[
                (Token::Lt, "<"),
                (Token::Gt, ">"),
                (Token::Le, "<="),
                (Token::Ge, ">="),
                (Token::EqEq, "=="),
                (Token::Ne, "!="),
            ],
        );
    }

    #[test]
    fn edge_all_assignment_operators() {
        check(
            "= += -= *= /= %=",
            &[
                (Token::Eq, "="),
                (Token::PlusEq, "+="),
                (Token::MinusEq, "-="),
                (Token::StarEq, "*="),
                (Token::SlashEq, "/="),
                (Token::PercentEq, "%="),
            ],
        );
    }

    // ============================================================
    // Full Example Program Test
    // ============================================================

    #[test]
    fn example_program_from_docs() {
        // This is the example program from lexical-grammar.md
        let source = r#"
// Point struct with public fields
pub struct Point {
    x: f64,
    y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Point {
        Point { x: x, y: y }
    }

    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

fn main() {
    let mut p1 = Point::new(0.0, 0.0);
    let p2 = Point::new(3.0, 4.0);

    // Calculate distance
    let dist = p1.distance(&p2);

    /* Update p1 position
       using compound assignment */
    p1.x += 1.5e1;
    p1.y += 0x0A as f64;

    // Loop with range
    for i in 0..10 {
        if i % 2 == 0 {
            continue;
        }
        // Process odd numbers
    }

    // Boolean and character literals
    let flag: bool = true;
    let ch: char = '\n';
    let msg: str = "Hello, SPL!\n";

    // Control flow
    while flag && dist > 0.0 {
        if dist <= 5.0 {
            break;
        }
    }

    loop {
        return;
    }
}
"#;

        let tokens = lex(source);

        // Verify no error tokens
        for (token, text) in &tokens {
            assert_ne!(
                *token,
                Token::Error,
                "Unexpected error token for text: {:?}",
                text
            );
        }

        // Verify we got a reasonable number of tokens
        assert!(
            tokens.len() > 100,
            "Expected >100 tokens, got {}",
            tokens.len()
        );

        // Verify key tokens are present
        let token_types: Vec<Token> = tokens.iter().map(|(t, _)| t.clone()).collect();

        // Check for key tokens that appear in the example program
        assert!(token_types.contains(&Token::Pub));
        assert!(token_types.contains(&Token::Struct));
        assert!(token_types.contains(&Token::Impl));
        assert!(token_types.contains(&Token::Fn));
        assert!(token_types.contains(&Token::Let));
        assert!(token_types.contains(&Token::Mut));
        assert!(token_types.contains(&Token::If));
        assert!(token_types.contains(&Token::While));
        assert!(token_types.contains(&Token::For));
        assert!(token_types.contains(&Token::In));
        assert!(token_types.contains(&Token::Loop));
        assert!(token_types.contains(&Token::Break));
        assert!(token_types.contains(&Token::Continue));
        assert!(token_types.contains(&Token::Return));
        assert!(token_types.contains(&Token::As));
        assert!(token_types.contains(&Token::True));
        assert!(token_types.contains(&Token::SelfValue));
        assert!(token_types.contains(&Token::Arrow));
        assert!(token_types.contains(&Token::DotDot));
        assert!(token_types.contains(&Token::Float));
        assert!(token_types.contains(&Token::Integer));
        assert!(token_types.contains(&Token::String));
        assert!(token_types.contains(&Token::Char));
        assert!(token_types.contains(&Token::ColonColon));
        assert!(token_types.contains(&Token::Amp));
        assert!(token_types.contains(&Token::PlusEq));
        assert!(token_types.contains(&Token::Percent));
        assert!(token_types.contains(&Token::EqEq));
        assert!(token_types.contains(&Token::Le));
        assert!(token_types.contains(&Token::AndAnd));
        assert!(token_types.contains(&Token::Gt));
    }

    // ============================================================
    // Error Recovery Tests (lex_all)
    // ============================================================

    #[test]
    fn recovery_invalid_character() {
        let result = lex_all("let @ x");
        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].kind, LexErrorKind::InvalidCharacter('@'));

        // Valid tokens are still produced
        let tokens: Vec<_> = result.tokens.iter().map(|t| &t.token).collect();
        assert!(tokens.contains(&&Token::Let));
        assert!(tokens.contains(&&Token::Ident));
    }

    #[test]
    fn recovery_multiple_invalid_characters() {
        let result = lex_all("@ # ~");
        assert_eq!(result.errors.len(), 3);
        assert_eq!(result.errors[0].kind, LexErrorKind::InvalidCharacter('@'));
        assert_eq!(result.errors[1].kind, LexErrorKind::InvalidCharacter('#'));
        assert_eq!(result.errors[2].kind, LexErrorKind::InvalidCharacter('~'));
    }

    #[test]
    fn recovery_unterminated_string() {
        let result = lex_all(r#"let x = "hello"#);
        assert!(result.has_errors());
        // Should detect unterminated string
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e.kind, LexErrorKind::UnterminatedString))
        );
    }

    #[test]
    fn recovery_unterminated_char() {
        let result = lex_all("let x = 'a");
        assert!(result.has_errors());
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e.kind, LexErrorKind::UnterminatedChar))
        );
    }

    #[test]
    fn recovery_unterminated_block_comment() {
        let result = lex_all("let x = /* comment");
        assert!(result.has_errors());
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e.kind, LexErrorKind::UnterminatedBlockComment))
        );
    }

    #[test]
    fn char_empty_literal() {
        // Empty char '' is now a valid token - parser will handle the semantic error
        check_single("''", Token::Char);
    }

    #[test]
    fn char_empty_in_context() {
        // Empty char in a let statement should lex successfully
        let result = lex_all("let x = '';");
        assert!(result.is_ok());
        assert!(
            result
                .tokens
                .iter()
                .any(|t| t.token == Token::Char && t.text == "''")
        );
    }

    #[test]
    fn recovery_valid_code_no_errors() {
        let result = lex_all("let x = 42;");
        assert!(result.is_ok());
        assert!(result.errors.is_empty());
        // Now includes whitespace tokens: let, ws, x, ws, =, ws, 42, ;
        assert_eq!(result.tokens.len(), 8);
    }

    #[test]
    fn recovery_continues_after_error() {
        // Ensure lexer continues producing tokens after an error
        let result = lex_all("let @ x = 42;");
        assert!(result.has_errors());

        // Count valid non-trivia tokens (excluding Error and trivia)
        let valid_count = result
            .tokens
            .iter()
            .filter(|t| !matches!(t.token, Token::Error | Token::Whitespace | Token::LineComment | Token::BlockComment))
            .count();
        assert_eq!(valid_count, 5); // let, x, =, 42, ;
    }

    #[test]
    fn recovery_error_spans_are_correct() {
        let result = lex_all("ab@cd");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].span, 2..3);
    }

    #[test]
    fn recovery_lex_result_methods() {
        let ok_result = lex_all("let x = 1;");
        assert!(ok_result.is_ok());
        assert!(!ok_result.has_errors());

        let err_result = lex_all("let @ = 1;");
        assert!(!err_result.is_ok());
        assert!(err_result.has_errors());
    }

    #[test]
    fn recovery_error_display() {
        let error = LexError::new(LexErrorKind::InvalidCharacter('@'), 5..6);
        let msg = format!("{}", error);
        assert!(msg.contains("invalid character"));
        assert!(msg.contains("@"));
        assert!(msg.contains("5..6"));
    }

    #[test]
    fn recovery_error_kind_display() {
        assert_eq!(
            format!("{}", LexErrorKind::UnterminatedString),
            "unterminated string literal"
        );
        assert_eq!(
            format!("{}", LexErrorKind::UnterminatedChar),
            "unterminated character literal"
        );
        assert_eq!(
            format!("{}", LexErrorKind::InvalidCharacter('@')),
            "invalid character '@'"
        );
    }

    #[test]
    fn recovery_unicode_invalid_char() {
        let result = lex_all("let \u{1F600} x"); // emoji
        assert!(result.has_errors());
        // Should still parse let and x (filtering out trivia and errors)
        let tokens: Vec<_> = result
            .tokens
            .iter()
            .filter(|t| !matches!(t.token, Token::Error | Token::Whitespace | Token::LineComment | Token::BlockComment))
            .collect();
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn recovery_preserves_token_order() {
        // Now includes whitespace tokens
        let result = lex_all("a @ b # c");
        let tokens: Vec<_> = result.tokens.iter().map(|t| t.text).collect();
        assert_eq!(tokens, vec!["a", " ", "@", " ", "b", " ", "#", " ", "c"]);
    }

    #[test]
    fn recovery_string_with_escaped_quote() {
        // This is valid - should have no errors
        let result = lex_all(r#""hello\"world""#);
        assert!(result.is_ok());
    }

    #[test]
    fn recovery_char_with_escaped_quote() {
        // This is valid - should have no errors
        let result = lex_all(r"'\''");
        assert!(result.is_ok());
    }
}
