use super::*;
use pretty_assertions::assert_eq;

/// Helper to lex a string and collect all tokens (without spans for simpler assertions)
fn lex(source: &str) -> Vec<(Token, &str)> {
    Lexer::new(source).map(|st| (st.token, st.text)).collect()
}

/// Helper to lex and filter out trivia (whitespace and comments)
fn lex_no_trivia(source: &str) -> Vec<(Token, &str)> {
    Lexer::new(source)
        .filter(|st| {
            !matches!(
                st.token,
                Token::Whitespace | Token::LineComment | Token::BlockComment
            )
        })
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

/// Helper to check single token
#[allow(clippy::needless_pass_by_value)] // Token is small and Copy-like
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

// Note: 'crate' was removed as a keyword - see removed_keyword_crate_is_now_ident test

#[test]
fn keyword_super() {
    check_single("super", Token::Super);
}

#[test]
fn keyword_where() {
    check_single("where", Token::Where);
}

#[test]
fn keyword_is() {
    check_single("is", Token::Is);
}

// Note: 'not' was removed as a keyword - see removed_keyword_not_is_now_ident test

#[test]
fn keyword_match() {
    check_single("match", Token::Match);
}

#[test]
fn keyword_yield() {
    check_single("yield", Token::Yield);
}

#[test]
fn keyword_use() {
    check_single("use", Token::Use);
}

#[test]
fn keyword_module() {
    check_single("module", Token::Module);
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

#[test]
fn float_suffix_f32() {
    check_single("3.14f32", Token::Float);
}

#[test]
fn float_suffix_f64() {
    check_single("3.14f64", Token::Float);
}

#[test]
fn float_exponent_suffix_f32() {
    check_single("1.5e10f32", Token::Float);
}

#[test]
fn float_exponent_only_suffix_f64() {
    check_single("1e10f64", Token::Float);
}

#[test]
fn float_suffix_with_underscores() {
    check_single("1_000.5_5f32", Token::Float);
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

// Note: @ is now a valid operator (capture list prefix) - see op_at test

#[test]
fn op_hash() {
    check_single("#", Token::Hash);
}

#[test]
fn attribute_tokens_sequence() {
    check(
        "#[test]",
        &[
            (Token::Hash, "#"),
            (Token::LBracket, "["),
            (Token::Ident, "test"),
            (Token::RBracket, "]"),
        ],
    );
}

#[test]
fn inner_attribute_tokens_sequence() {
    check(
        "#![feature]",
        &[
            (Token::Hash, "#"),
            (Token::Bang, "!"),
            (Token::LBracket, "["),
            (Token::Ident, "feature"),
            (Token::RBracket, "]"),
        ],
    );
}

// Note: ~ is now a valid operator (bitwise NOT) - see op_tilde test

#[test]
fn error_with_valid_tokens() {
    // Error token should not prevent lexing of valid tokens
    check(
        "let ` x",
        &[
            (Token::Let, "let"),
            (Token::Error, "`"),
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
fn tick_followed_by_ident() {
    // With the Tick token, 'a is now Tick + Ident (for label syntax)
    // rather than an unterminated char literal
    let tokens = lex_no_trivia("'a");
    assert_eq!(tokens, vec![(Token::Tick, "'"), (Token::Ident, "a")]);
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

// ============================================================
// Integer Literals with Suffixes
// ============================================================

#[test]
fn int_decimal_suffix_u8() {
    check_single("255u8", Token::Integer);
}

#[test]
fn int_decimal_suffix_i32() {
    check_single("42i32", Token::Integer);
}

#[test]
fn int_decimal_suffix_usize() {
    check_single("100usize", Token::Integer);
}

#[test]
fn int_hex_suffix_u8() {
    check_single("0xFFu8", Token::Integer);
}

#[test]
fn int_binary_suffix_i8() {
    check_single("0b1111i8", Token::Integer);
}

#[test]
fn int_octal_suffix_u32() {
    check_single("0o777u32", Token::Integer);
}

#[test]
fn int_suffix_with_underscores() {
    check_single("1_000_000u64", Token::Integer);
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
    // Block comments DO nest (like Rust) - /* outer /* inner */ needs matching */
    // "/* outer /* inner */ let" is now an unterminated comment
    let result = lex_all("/* outer /* inner */ let");
    assert!(result.has_errors());
    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e.kind, LexErrorKind::UnterminatedBlockComment))
    );
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
            "Unexpected error token for text: {text:?}"
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
    let result = lex_all("let ` x");
    assert!(result.has_errors());
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].kind, LexErrorKind::InvalidCharacter('`'));

    // Valid tokens are still produced
    let tokens: Vec<_> = result.tokens.iter().map(|t| &t.token).collect();
    assert!(tokens.contains(&&Token::Let));
    assert!(tokens.contains(&&Token::Ident));
}

#[test]
fn recovery_multiple_invalid_characters() {
    let result = lex_all("` \\ §");
    assert_eq!(result.errors.len(), 3);
    assert_eq!(result.errors[0].kind, LexErrorKind::InvalidCharacter('`'));
    assert_eq!(result.errors[1].kind, LexErrorKind::InvalidCharacter('\\'));
    assert_eq!(result.errors[2].kind, LexErrorKind::InvalidCharacter('§'));
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
    let result = lex_all("let ` x = 42;");
    assert!(result.has_errors());

    // Count valid non-trivia tokens (excluding Error and trivia)
    let valid_count = result
        .tokens
        .iter()
        .filter(|t| {
            !matches!(
                t.token,
                Token::Error | Token::Whitespace | Token::LineComment | Token::BlockComment
            )
        })
        .count();
    assert_eq!(valid_count, 5); // let, x, =, 42, ;
}

#[test]
fn recovery_error_spans_are_correct() {
    let result = lex_all("ab`cd");
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].span, 2..3);
}

#[test]
fn recovery_lex_result_methods() {
    let ok_result = lex_all("let x = 1;");
    assert!(ok_result.is_ok());
    assert!(!ok_result.has_errors());

    let err_result = lex_all("let ` = 1;");
    assert!(!err_result.is_ok());
    assert!(err_result.has_errors());
}

#[test]
fn recovery_error_display() {
    let error = LexError::new(LexErrorKind::InvalidCharacter('@'), 5..6);
    let msg = format!("{error}");
    assert!(msg.contains("invalid character"));
    assert!(msg.contains('@'));
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
        .filter(|t| {
            !matches!(
                t.token,
                Token::Error | Token::Whitespace | Token::LineComment | Token::BlockComment
            )
        })
        .collect();
    assert_eq!(tokens.len(), 2);
}

#[test]
fn recovery_preserves_token_order() {
    // Now includes whitespace tokens
    // @ and ^ are now valid operators, so use invalid characters like ` and §
    let result = lex_all("a ` b § c");
    let tokens: Vec<_> = result.tokens.iter().map(|t| t.text).collect();
    assert_eq!(tokens, vec!["a", " ", "`", " ", "b", " ", "§", " ", "c"]);
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

// ============================================================
// Missing Keywords (from spec)
// ============================================================

#[test]
fn keyword_gen() {
    check_single("gen", Token::Gen);
}

#[test]
fn keyword_enum() {
    check_single("enum", Token::Enum);
}

#[test]
fn keyword_trait() {
    check_single("trait", Token::Trait);
}

#[test]
fn keyword_throw() {
    check_single("throw", Token::Throw);
}

#[test]
fn keyword_throws() {
    check_single("throws", Token::Throws);
}

#[test]
fn keyword_const() {
    check_single("const", Token::Const);
}

#[test]
fn keyword_static() {
    check_single("static", Token::Static);
}

#[test]
fn keyword_unsafe() {
    check_single("unsafe", Token::Unsafe);
}

#[test]
fn keyword_extern() {
    // Already exists, verify it works
    check_single("extern", Token::Extern);
}

// ============================================================
// Removed Keywords (should now be identifiers)
// ============================================================

#[test]
fn removed_keyword_crate_is_now_ident() {
    // 'crate' is no longer a keyword
    check_single("crate", Token::Ident);
}

#[test]
fn removed_keyword_not_is_now_ident() {
    // 'not' is no longer a keyword
    check_single("not", Token::Ident);
}

// ============================================================
// Tick Token (for labels)
// ============================================================

#[test]
fn tick_standalone() {
    check_single("'", Token::Tick);
}

#[test]
fn tick_before_ident() {
    check(
        "'outer:",
        &[
            (Token::Tick, "'"),
            (Token::Ident, "outer"),
            (Token::Colon, ":"),
        ],
    );
}

#[test]
fn char_literal_still_works() {
    // Ensure char literals with content still work after adding Tick
    check_single("'a'", Token::Char);
}

#[test]
fn tick_in_break_label() {
    check(
        "break 'outer",
        &[
            (Token::Break, "break"),
            (Token::Tick, "'"),
            (Token::Ident, "outer"),
        ],
    );
}

#[test]
fn tick_vs_char_disambiguation() {
    // 'a' is a char, 'a is tick + ident
    check("'a'", &[(Token::Char, "'a'")]);
    check(
        "'a:",
        &[(Token::Tick, "'"), (Token::Ident, "a"), (Token::Colon, ":")],
    );
}

// ============================================================
// Bitwise Operators
// ============================================================

#[test]
fn op_pipe() {
    check_single("|", Token::Pipe);
}

#[test]
fn op_caret() {
    check_single("^", Token::Caret);
}

#[test]
fn op_tilde() {
    check_single("~", Token::Tilde);
}

#[test]
fn op_shl() {
    check_single("<<", Token::Shl);
}

#[test]
fn op_shr() {
    check_single(">>", Token::Shr);
}

#[test]
fn op_amp_eq() {
    check_single("&=", Token::AmpEq);
}

#[test]
fn op_pipe_eq() {
    check_single("|=", Token::PipeEq);
}

#[test]
fn op_caret_eq() {
    check_single("^=", Token::CaretEq);
}

#[test]
fn op_shl_eq() {
    check_single("<<=", Token::ShlEq);
}

#[test]
fn op_shr_eq() {
    check_single(">>=", Token::ShrEq);
}

// Ambiguity: >> vs > > (generic closing)
#[test]
fn ambiguity_shr_vs_two_gt() {
    check_single(">>", Token::Shr);
    check(">> >>", &[(Token::Shr, ">>"), (Token::Shr, ">>")]);
}

// Ambiguity: << vs < <
#[test]
fn ambiguity_shl_vs_two_lt() {
    check_single("<<", Token::Shl);
}

// ============================================================
// Exponentiation Operators
// ============================================================

#[test]
fn op_star_star() {
    check_single("**", Token::StarStar);
}

#[test]
fn op_star_star_eq() {
    check_single("**=", Token::StarStarEq);
}

// Ambiguity: ** vs * * (multiply then dereference)
#[test]
fn ambiguity_starstar_vs_two_stars() {
    check_single("**", Token::StarStar);
    check("* *", &[(Token::Star, "*"), (Token::Star, "*")]);
}

// Combined: 2 ** 3 (exponentiation expression)
#[test]
fn combined_exponentiation() {
    check(
        "2 ** 3",
        &[
            (Token::Integer, "2"),
            (Token::StarStar, "**"),
            (Token::Integer, "3"),
        ],
    );
}

// ============================================================
// Range and Optional Operators
// ============================================================

#[test]
fn op_dot_dot_eq() {
    check_single("..=", Token::DotDotEq);
}

#[test]
fn op_question() {
    check_single("?", Token::Question);
}

#[test]
fn op_question_dot() {
    check_single("?.", Token::QuestionDot);
}

#[test]
fn op_question_question() {
    check_single("??", Token::QuestionQuestion);
}

#[test]
fn op_at() {
    check_single("@", Token::At);
}

// Ambiguity: ..= vs .. = (range then assign)
#[test]
fn ambiguity_inclusive_range() {
    check_single("..=", Token::DotDotEq);
    check(".. =", &[(Token::DotDot, ".."), (Token::Eq, "=")]);
}

// Combined: inclusive range in for loop
#[test]
fn combined_inclusive_range() {
    check(
        "for i in 0..=10",
        &[
            (Token::For, "for"),
            (Token::Ident, "i"),
            (Token::In, "in"),
            (Token::Integer, "0"),
            (Token::DotDotEq, "..="),
            (Token::Integer, "10"),
        ],
    );
}

// Combined: optional chaining
#[test]
fn combined_optional_chaining() {
    check(
        "x?.y?.z",
        &[
            (Token::Ident, "x"),
            (Token::QuestionDot, "?."),
            (Token::Ident, "y"),
            (Token::QuestionDot, "?."),
            (Token::Ident, "z"),
        ],
    );
}

// Combined: nullish coalescing
#[test]
fn combined_nullish_coalescing() {
    check(
        "x ?? y ?? z",
        &[
            (Token::Ident, "x"),
            (Token::QuestionQuestion, "??"),
            (Token::Ident, "y"),
            (Token::QuestionQuestion, "??"),
            (Token::Ident, "z"),
        ],
    );
}

// Combined: optional type
#[test]
fn combined_optional_type() {
    check(
        "let x: i32?",
        &[
            (Token::Let, "let"),
            (Token::Ident, "x"),
            (Token::Colon, ":"),
            (Token::Ident, "i32"),
            (Token::Question, "?"),
        ],
    );
}

// Combined: closure capture list
#[test]
fn combined_closure_capture() {
    check(
        "@[x, y]",
        &[
            (Token::At, "@"),
            (Token::LBracket, "["),
            (Token::Ident, "x"),
            (Token::Comma, ","),
            (Token::Ident, "y"),
            (Token::RBracket, "]"),
        ],
    );
}

// ============================================================
// Raw String Literals
// ============================================================

#[test]
fn raw_string_simple() {
    check_single(r#"r"hello""#, Token::RawString);
}

#[test]
fn raw_string_with_backslash() {
    check_single(r#"r"C:\Users\name""#, Token::RawString);
}

#[test]
fn raw_string_with_escape_sequences() {
    // Raw strings don't process escapes
    check_single(r#"r"hello\nworld""#, Token::RawString);
}

#[test]
fn raw_string_one_hash() {
    check_single(r##"r#"contains "quotes""#"##, Token::RawString);
}

#[test]
fn raw_string_two_hashes() {
    check_single(r###"r##"contains "# and quotes"##"###, Token::RawString);
}

#[test]
fn raw_string_three_hashes() {
    check_single(r####"r###"complex "##""###"####, Token::RawString);
}

#[test]
fn raw_string_empty() {
    check_single(r#"r"""#, Token::RawString);
}

#[test]
fn raw_string_empty_with_hash() {
    check_single(r##"r#""#"##, Token::RawString);
}

#[test]
fn raw_string_multiline() {
    check_single("r\"line1\nline2\"", Token::RawString);
}

// ============================================================
// Byte String Literals
// ============================================================

#[test]
fn byte_string_simple() {
    check_single(r#"b"hello""#, Token::ByteString);
}

#[test]
fn byte_string_with_escapes() {
    check_single(r#"b"data\x00\xFF""#, Token::ByteString);
}

#[test]
fn byte_string_empty() {
    check_single(r#"b"""#, Token::ByteString);
}

// ============================================================
// Raw Byte String Literals
// ============================================================

#[test]
fn raw_byte_string_simple() {
    check_single(r#"br"raw bytes""#, Token::RawByteString);
}

#[test]
fn raw_byte_string_with_hash() {
    check_single(r##"br#"with "quotes""#"##, Token::RawByteString);
}

#[test]
fn raw_byte_string_empty() {
    check_single(r#"br"""#, Token::RawByteString);
}

// ============================================================
// C String Literals
// ============================================================

#[test]
fn c_string_simple() {
    check_single(r#"c"Hello, C!""#, Token::CString);
}

#[test]
fn c_string_path() {
    check_single(r#"c"/path/to/file""#, Token::CString);
}

#[test]
fn c_string_empty() {
    check_single(r#"c"""#, Token::CString);
}

// ============================================================
// Byte Character Literals
// ============================================================

#[test]
fn byte_char_simple() {
    check_single("b'A'", Token::ByteChar);
}

#[test]
fn byte_char_escape_null() {
    check_single(r"b'\0'", Token::ByteChar);
}

#[test]
fn byte_char_escape_hex() {
    check_single(r"b'\xFF'", Token::ByteChar);
}

#[test]
fn byte_char_escape_newline() {
    check_single(r"b'\n'", Token::ByteChar);
}

// ============================================================
// Integer bigint Suffix
// ============================================================

#[test]
fn int_suffix_bigint() {
    check_single("999999999999999999999bigint", Token::Integer);
}

#[test]
fn int_suffix_bigint_with_underscore() {
    check_single("1_000_000bigint", Token::Integer);
}

#[test]
fn int_hex_suffix_bigint() {
    // Note: bigint suffix cannot be used directly with hex literals because 'b' is a hex digit.
    // The workaround is to use decimal: 18446744073709551615bigint
    // Or use binary: 0b1111111111111111111111111111111111111111111111111111111111111111bigint
    // For now, test decimal bigint works
    check_single("18446744073709551615bigint", Token::Integer);
}

// ============================================================
// Float decimal Suffix
// ============================================================

#[test]
fn float_suffix_decimal() {
    check_single("0.10decimal", Token::Float);
}

#[test]
fn float_suffix_decimal_precise() {
    check_single("19.99decimal", Token::Float);
}

#[test]
fn float_suffix_decimal_with_underscore() {
    check_single("1_000.00decimal", Token::Float);
}

// ============================================================
// Hex Escape Sequences
// ============================================================

#[test]
fn string_escape_hex() {
    check_single(r#""\x00\x7F\xFF""#, Token::String);
}

#[test]
fn char_escape_hex() {
    check_single(r"'\x7F'", Token::Char);
}

#[test]
fn char_escape_hex_lowercase() {
    check_single(r"'\xab'", Token::Char);
}

// ============================================================
// Unicode Escape Sequences
// ============================================================

#[test]
fn string_escape_unicode() {
    check_single(r#""\u{1F600}""#, Token::String);
}

#[test]
fn string_escape_unicode_short() {
    check_single(r#""\u{41}""#, Token::String); // 'A'
}

#[test]
fn string_escape_unicode_long() {
    check_single(r#""\u{10FFFF}""#, Token::String); // Max valid
}

#[test]
fn char_escape_unicode() {
    check_single(r"'\u{1F600}'", Token::Char);
}

#[test]
fn char_escape_unicode_short() {
    check_single(r"'\u{41}'", Token::Char);
}

// ============================================================
// Unicode Identifiers
// ============================================================

#[test]
fn ident_unicode_cafe() {
    check_single("café", Token::Ident);
}

#[test]
fn ident_unicode_japanese() {
    check_single("日本語", Token::Ident);
}

#[test]
fn ident_unicode_greek() {
    check_single("αβγ", Token::Ident);
}

#[test]
fn ident_unicode_mixed() {
    check_single("Größe", Token::Ident);
}

#[test]
fn ident_unicode_emoji_not_valid() {
    // Emojis are not valid XID characters
    let result = lex_all("🎉");
    assert!(result.has_errors());
}

#[test]
fn ident_unicode_with_combining() {
    // e followed by combining acute accent
    check_single("cafe\u{0301}", Token::Ident);
}

// ============================================================
// Nested Block Comments
// ============================================================

#[test]
fn comment_block_nested_simple() {
    check("/* outer /* inner */ outer */ let", &[(Token::Let, "let")]);
}

#[test]
fn comment_block_nested_deep() {
    check("/* 1 /* 2 /* 3 */ 2 */ 1 */ let", &[(Token::Let, "let")]);
}

#[test]
fn comment_block_nested_with_code() {
    check(
        "let /* comment /* nested */ end */ x",
        &[(Token::Let, "let"), (Token::Ident, "x")],
    );
}

#[test]
fn comment_block_unterminated_nested() {
    let result = lex_all("/* outer /* inner */");
    assert!(result.has_errors());
    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e.kind, LexErrorKind::UnterminatedBlockComment))
    );
}

// ============================================================
// Float Exponent Case (E and e)
// ============================================================

#[test]
fn float_exponent_uppercase() {
    check_single("1E10", Token::Float);
}

#[test]
fn float_exponent_uppercase_positive() {
    check_single("1E+10", Token::Float);
}

#[test]
fn float_exponent_uppercase_negative() {
    check_single("2E-3", Token::Float);
}

#[test]
fn float_full_uppercase_exponent() {
    check_single("2.5E-3", Token::Float);
}

// ============================================================
// Match Expression (FatArrow =>)
// ============================================================

#[test]
fn op_fat_arrow() {
    check_single("=>", Token::FatArrow);
}

#[test]
fn combined_match_expression() {
    check(
        "match x { 1 => true, _ => false }",
        &[
            (Token::Match, "match"),
            (Token::Ident, "x"),
            (Token::LBrace, "{"),
            (Token::Integer, "1"),
            (Token::FatArrow, "=>"),
            (Token::True, "true"),
            (Token::Comma, ","),
            (Token::Ident, "_"),
            (Token::FatArrow, "=>"),
            (Token::False, "false"),
            (Token::RBrace, "}"),
        ],
    );
}

// ============================================================
// Generator Functions
// ============================================================

#[test]
fn combined_generator_function() {
    check(
        "gen fn numbers() { yield 1; yield 2; }",
        &[
            (Token::Gen, "gen"),
            (Token::Fn, "fn"),
            (Token::Ident, "numbers"),
            (Token::LParen, "("),
            (Token::RParen, ")"),
            (Token::LBrace, "{"),
            (Token::Yield, "yield"),
            (Token::Integer, "1"),
            (Token::Semi, ";"),
            (Token::Yield, "yield"),
            (Token::Integer, "2"),
            (Token::Semi, ";"),
            (Token::RBrace, "}"),
        ],
    );
}

// ============================================================
// Error Handling (throw/throws)
// ============================================================

#[test]
fn combined_throws_function() {
    check(
        "fn parse(s: str) throws -> i32",
        &[
            (Token::Fn, "fn"),
            (Token::Ident, "parse"),
            (Token::LParen, "("),
            (Token::Ident, "s"),
            (Token::Colon, ":"),
            (Token::Ident, "str"),
            (Token::RParen, ")"),
            (Token::Throws, "throws"),
            (Token::Arrow, "->"),
            (Token::Ident, "i32"),
        ],
    );
}

#[test]
fn combined_throw_statement() {
    check(
        "throw ParseError",
        &[(Token::Throw, "throw"), (Token::Ident, "ParseError")],
    );
}

// ============================================================
// Additional Edge Cases
// ============================================================

#[test]
fn edge_shr_in_generics() {
    // Vec<Vec<i32>> - the >> should still be Shr token
    // Parser handles this case
    check(
        "Vec<Vec<i32>>",
        &[
            (Token::Ident, "Vec"),
            (Token::Lt, "<"),
            (Token::Ident, "Vec"),
            (Token::Lt, "<"),
            (Token::Ident, "i32"),
            (Token::Shr, ">>"),
        ],
    );
}

#[test]
fn edge_postfix_bang() {
    // result! for try/propagate
    check("result!", &[(Token::Ident, "result"), (Token::Bang, "!")]);
}

#[test]
fn edge_question_after_expr() {
    // For optional types like i32?
    check(
        "Option?",
        &[(Token::Ident, "Option"), (Token::Question, "?")],
    );
}

#[test]
fn edge_bitwise_expression() {
    check(
        "a | b & c ^ d",
        &[
            (Token::Ident, "a"),
            (Token::Pipe, "|"),
            (Token::Ident, "b"),
            (Token::Amp, "&"),
            (Token::Ident, "c"),
            (Token::Caret, "^"),
            (Token::Ident, "d"),
        ],
    );
}

#[test]
fn edge_shift_expression() {
    check(
        "a << 2 >> 1",
        &[
            (Token::Ident, "a"),
            (Token::Shl, "<<"),
            (Token::Integer, "2"),
            (Token::Shr, ">>"),
            (Token::Integer, "1"),
        ],
    );
}

#[test]
fn edge_bitwise_assignment() {
    check(
        "x &= y |= z ^= 1",
        &[
            (Token::Ident, "x"),
            (Token::AmpEq, "&="),
            (Token::Ident, "y"),
            (Token::PipeEq, "|="),
            (Token::Ident, "z"),
            (Token::CaretEq, "^="),
            (Token::Integer, "1"),
        ],
    );
}

#[test]
fn edge_enum_definition() {
    check(
        "enum Color { Red, Green, Blue }",
        &[
            (Token::Enum, "enum"),
            (Token::Ident, "Color"),
            (Token::LBrace, "{"),
            (Token::Ident, "Red"),
            (Token::Comma, ","),
            (Token::Ident, "Green"),
            (Token::Comma, ","),
            (Token::Ident, "Blue"),
            (Token::RBrace, "}"),
        ],
    );
}

#[test]
fn edge_trait_definition() {
    check(
        "trait Display { fn fmt(&self); }",
        &[
            (Token::Trait, "trait"),
            (Token::Ident, "Display"),
            (Token::LBrace, "{"),
            (Token::Fn, "fn"),
            (Token::Ident, "fmt"),
            (Token::LParen, "("),
            (Token::Amp, "&"),
            (Token::SelfValue, "self"),
            (Token::RParen, ")"),
            (Token::Semi, ";"),
            (Token::RBrace, "}"),
        ],
    );
}

#[test]
fn edge_const_static() {
    check(
        "const PI: f64 = 3.14; static mut COUNTER: i32 = 0;",
        &[
            (Token::Const, "const"),
            (Token::Ident, "PI"),
            (Token::Colon, ":"),
            (Token::Ident, "f64"),
            (Token::Eq, "="),
            (Token::Float, "3.14"),
            (Token::Semi, ";"),
            (Token::Static, "static"),
            (Token::Mut, "mut"),
            (Token::Ident, "COUNTER"),
            (Token::Colon, ":"),
            (Token::Ident, "i32"),
            (Token::Eq, "="),
            (Token::Integer, "0"),
            (Token::Semi, ";"),
        ],
    );
}

#[test]
fn edge_unsafe_block() {
    check(
        "unsafe { ptr.write(42) }",
        &[
            (Token::Unsafe, "unsafe"),
            (Token::LBrace, "{"),
            (Token::Ident, "ptr"),
            (Token::Dot, "."),
            (Token::Ident, "write"),
            (Token::LParen, "("),
            (Token::Integer, "42"),
            (Token::RParen, ")"),
            (Token::RBrace, "}"),
        ],
    );
}
