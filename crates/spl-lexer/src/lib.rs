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
    /// Horizontal whitespace (spaces and tabs)
    #[regex(r"[ \t]+")]
    Whitespace,

    /// Newline (LF, CR, or CR-LF)
    #[regex(r"\r\n|\n|\r")]
    Newline,

    /// Line comment (// ...)
    #[regex(r"//[^\n]*")]
    LineComment,

    /// Block comment (/* ... */) - supports nesting
    #[token("/*", lex_block_comment)]
    BlockComment,

    // === Keywords ===
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("fn")]
    Fn,
    #[token("gen")]
    Gen,
    #[token("struct")]
    Struct,
    #[token("enum")]
    Enum,
    #[token("trait")]
    Trait,
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
    #[token("yield")]
    Yield,
    #[token("throw")]
    Throw,
    #[token("throws")]
    Throws,
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
    #[token("super")]
    Super,
    #[token("where")]
    Where,
    #[token("is")]
    Is,
    #[token("match")]
    Match,
    #[token("extern")]
    Extern,
    #[token("const")]
    Const,
    #[token("static")]
    Static,
    #[token("unsafe")]
    Unsafe,
    #[token("use")]
    Use,
    #[token("module")]
    Module,

    // === Operators ===
    // Arithmetic (multi-char first for longest match)
    #[token("**=")]
    StarStarEq,
    #[token("**")]
    StarStar,
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
    #[token("<<=")]
    ShlEq,
    #[token(">>=")]
    ShrEq,
    #[token("<<")]
    Shl,
    #[token(">>")]
    Shr,
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

    // Bitwise
    #[token("|=")]
    PipeEq,
    #[token("^=")]
    CaretEq,
    #[token("&=")]
    AmpEq,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("~")]
    Tilde,
    #[token("&")]
    Amp,

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
    #[token("=>")]
    FatArrow,
    #[token("::")]
    ColonColon,
    #[token("...")]
    Ellipsis,
    #[token("..=")]
    DotDotEq,
    #[token("..")]
    DotDot,
    #[token("?.")]
    QuestionDot,
    #[token("??")]
    QuestionQuestion,
    #[token("?")]
    Question,
    #[token(".")]
    Dot,
    #[token("$")]
    Dollar,
    #[token("#")]
    Hash,
    #[token("@")]
    At,

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
    // Supports both e and E for exponent, decimal suffix
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9][0-9_]*)?(f32|f64|decimal)?")]
    #[regex(r"[0-9][0-9_]*[eE][+-]?[0-9][0-9_]*(f32|f64|decimal)?")]
    Float,

    // Integer suffixes: i8|i16|i32|i64|i128|isize|u8|u16|u32|u64|u128|usize|bigint
    // Note: Underscore allowed before suffix (e.g., 42_i64, 0xFF_bigint)
    // For hex ending in 'b', use underscore to disambiguate: 0xFFb_bigint vs 0xFFbigint
    #[regex(r"0[xX][0-9a-fA-F][0-9a-fA-F_]*(_?([iu](8|16|32|64|128|size)|bigint))?")]
    #[regex(r"0[bB][01][01_]*(_?([iu](8|16|32|64|128|size)|bigint))?")]
    #[regex(r"0[oO][0-7][0-7_]*(_?([iu](8|16|32|64|128|size)|bigint))?")]
    #[regex(r"[0-9][0-9_]*(_?([iu](8|16|32|64|128|size)|bigint))?")]
    Integer,

    // String literals with full escape support including \xNN and \u{NNNNNN}
    #[regex(r#""([^"\\]|\\[nrt\\'\"0]|\\x[0-9a-fA-F]{2}|\\u\{[0-9a-fA-F]{1,6}\})*""#)]
    String,

    // Character literals with full escape support
    #[regex(r#"'([^'\\]|\\[nrt\\'\"0]|\\x[0-9a-fA-F]{2}|\\u\{[0-9a-fA-F]{1,6}\})?'"#)]
    Char,

    // Raw string literals: r"..." or r#"..."# etc.
    #[regex(r#"r#*""#, lex_raw_string)]
    RawString,

    // Byte string literals: b"..."
    #[regex(r#"b"([^"\\]|\\[nrt\\'\"0]|\\x[0-9a-fA-F]{2})*""#)]
    ByteString,

    // Raw byte string literals: br"..." or br#"..."#
    #[regex(r#"br#*""#, lex_raw_byte_string)]
    RawByteString,

    // C string literals: c"..."
    #[regex(r#"c"([^"\\]|\\[nrt\\'\"0]|\\x[0-9a-fA-F]{2}|\\u\{[0-9a-fA-F]{1,6}\})*""#)]
    CString,

    // Byte character literals: b'...'
    #[regex(r"b'([^'\\]|\\[nrt\\'0]|\\x[0-9a-fA-F]{2})'")]
    ByteChar,

    // Tick (single quote) for label syntax: 'label
    // Note: Logos uses longest-match, so 'a' matches Char (complete char literal)
    // while standalone ' matches Tick
    #[token("'")]
    Tick,

    // === Identifier ===
    // Unicode XID identifiers - ASCII pattern has higher priority for performance
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", priority = 3)]
    #[regex(r"[\p{XID_Start}][\p{XID_Continue}]*", priority = 2)]
    Ident,

    // === Error ===
    /// Represents invalid/unrecognized input
    Error,
}

/// Lex a nested block comment starting after "/*"
fn lex_block_comment(lex: &mut logos::Lexer<'_, Token>) -> bool {
    let remainder = lex.remainder();
    let bytes = remainder.as_bytes();
    let mut depth = 1;
    let mut i = 0;

    while i < bytes.len() && depth > 0 {
        if i + 1 < bytes.len() {
            if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                depth += 1;
                i += 2;
                continue;
            }
            if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                depth -= 1;
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    if depth == 0 {
        lex.bump(i);
        true
    } else {
        // Consume all remaining input for unterminated comment error
        lex.bump(bytes.len());
        false
    }
}

/// Lex a raw string literal starting at r#*"
fn lex_raw_string(lex: &mut logos::Lexer<'_, Token>) -> bool {
    let slice = lex.slice();
    // Count # characters (slice is "r#*\"")
    let hash_count = slice.len() - 2; // subtract 'r' and '"'

    lex_raw_string_body(lex, hash_count)
}

/// Lex a raw byte string literal starting at br#*"
fn lex_raw_byte_string(lex: &mut logos::Lexer<'_, Token>) -> bool {
    let slice = lex.slice();
    // Count # characters (slice is "br#*\"")
    let hash_count = slice.len() - 3; // subtract 'br' and '"'

    lex_raw_string_body(lex, hash_count)
}

/// Common logic for lexing raw string body
fn lex_raw_string_body(lex: &mut logos::Lexer<'_, Token>, hash_count: usize) -> bool {
    let remainder = lex.remainder();
    let bytes = remainder.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'"' {
            // Check for matching hashes
            let mut hashes = 0;
            while i + 1 + hashes < bytes.len() && bytes[i + 1 + hashes] == b'#' {
                hashes += 1;
            }
            if hashes >= hash_count {
                // Found closing delimiter
                lex.bump(i + 1 + hash_count);
                return true;
            }
        }
        i += 1;
    }

    // No closing delimiter found - consume all for error
    lex.bump(bytes.len());
    false
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
    /// A raw string literal was not terminated before end of input.
    UnterminatedRawString,
    /// A byte string literal was not terminated before end of input.
    UnterminatedByteString,
    /// A C string literal was not terminated before end of input.
    UnterminatedCString,
    /// A character literal was not terminated before end of input.
    UnterminatedChar,
    /// A byte character literal was not terminated before end of input.
    UnterminatedByteChar,
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
            LexErrorKind::UnterminatedRawString => write!(f, "unterminated raw string literal"),
            LexErrorKind::UnterminatedByteString => write!(f, "unterminated byte string literal"),
            LexErrorKind::UnterminatedCString => write!(f, "unterminated C string literal"),
            LexErrorKind::UnterminatedChar => write!(f, "unterminated character literal"),
            LexErrorKind::UnterminatedByteChar => {
                write!(f, "unterminated byte character literal")
            }
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
/// use spl_lexer::{lex_all, Token, LexErrorKind};
///
/// let result = lex_all("let x = `;");
/// assert!(result.has_errors());
/// assert_eq!(result.errors[0].kind, LexErrorKind::InvalidCharacter('`'));
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

    // Check for unterminated raw string
    if text.starts_with("r#") || text.starts_with("r\"") {
        return LexError::new(LexErrorKind::UnterminatedRawString, span);
    }

    // Check for unterminated raw byte string
    if text.starts_with("br#") || text.starts_with("br\"") {
        return LexError::new(LexErrorKind::UnterminatedRawString, span);
    }

    // Check for unterminated byte string
    if text.starts_with("b\"") {
        return LexError::new(LexErrorKind::UnterminatedByteString, span);
    }

    // Check for unterminated C string
    if text.starts_with("c\"") {
        return LexError::new(LexErrorKind::UnterminatedCString, span);
    }

    // Check for unterminated byte char
    if text.starts_with("b'") {
        return LexError::new(LexErrorKind::UnterminatedByteChar, span);
    }

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
                // Valid escapes: n, r, t, \, ', ", 0, x (for \xNN), u (for \u{})
                if !matches!(next, 'n' | 'r' | 't' | '\\' | '\'' | '"' | '0' | 'x' | 'u') {
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
    let mut block_comment_depth = 0;
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

        if block_comment_depth > 0 {
            if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                block_comment_depth += 1;
                i += 2;
                continue;
            }
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                block_comment_depth -= 1;
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
                block_comment_depth = 1;
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
    if block_comment_depth > 0 {
        return Some(LexError::new(
            LexErrorKind::UnterminatedBlockComment,
            comment_start..source.len(),
        ));
    }

    None
}

#[cfg(test)]
mod tests;
