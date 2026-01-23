//! Pattern parser for SPL.
//!
//! Parses patterns used in let bindings, match arms, and function parameters.

use crate::parser::{CompletedMarker, ParseError, Parser};
use crate::syntax::SyntaxKind;

/// Parse a pattern.
///
/// ```text
/// pattern = ref_pat | tuple_pat | slice_pat | wildcard_pat | literal_or_range_pat | rest_pat | ident_pat
/// ```
pub fn pattern(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    match_token!(p, {
        // Reference pattern: &x, &mut x
        AMP => ref_pat(p),
        // Tuple pattern: (a, b)
        L_PAREN => tuple_pat(p),
        // Slice pattern: [a, b]
        L_BRACKET => slice_pat(p),
        // Wildcard, identifier, or struct pattern
        IDENT => {
            if p.current_text() == Some("_") {
                wildcard_pat(p)
            } else {
                // Could be ident, path, or struct pattern
                // Lookahead to check if this is a struct pattern (path followed by {)
                ident_or_struct_pat(p)
            }
        },
        // Rest pattern: ..
        DOT_DOT => rest_pat(p),
        // Literal patterns (may be range pattern if followed by ..)
        INT_LITERAL | FLOAT_LITERAL | STRING_LITERAL | CHAR_LITERAL | TRUE_KW | FALSE_KW => {
            literal_or_range_pat(p)
        },
        _ => {
            let err = p.error_at_current("expected pattern".to_string());
            Err(err)
        },
    })
}

/// Parse a reference pattern: `&x`, `&mut x`
fn ref_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume `&`
    p.eat(SyntaxKind::MUT_KW); // optional `mut`
    pattern(p)?; // inner pattern (recursive)
    Ok(m.complete(p, SyntaxKind::RefPat))
}

/// Parse a wildcard pattern: `_`
fn wildcard_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume `_`
    Ok(m.complete(p, SyntaxKind::WildcardPat))
}

/// Parse an identifier pattern: `x`, `foo`
fn ident_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    crate::parser::item::name(p)?; // Wrap in Name (binding site)
    Ok(m.complete(p, SyntaxKind::IdentPat))
}

/// Parse an identifier, path, struct pattern, or enum pattern.
/// Lookahead is needed to distinguish:
/// - `x` (ident)
/// - `Point { x }` (struct pattern)
/// - `Some(x)` (enum pattern)
fn ident_or_struct_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    // Check if this is a path (has :: or . after identifier)
    let is_path = p.peek(1) == Some(SyntaxKind::COLON_COLON)
        || p.peek(1) == Some(SyntaxKind::DOT);
    // Check if this is a struct pattern (has { after identifier or path)
    let is_struct = p.peek(1) == Some(SyntaxKind::L_BRACE);
    // Check if this is an enum pattern (has ( after identifier)
    let is_enum = p.peek(1) == Some(SyntaxKind::L_PAREN);

    if is_path {
        // This is a path - could be struct pattern or enum pattern with qualified name
        path_or_struct_or_enum_pat(p)
    } else if is_struct {
        // Struct pattern: Name { ... }
        struct_pat(p)
    } else if is_enum {
        // Enum pattern: Some(x)
        enum_pat(p)
    } else {
        // Simple identifier pattern
        ident_pat(p)
    }
}

/// Parse a path and determine if it's followed by struct fields or enum args.
fn path_or_struct_or_enum_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in patterns)
    crate::parser::path::path_no_generics(p)?;

    // Check if followed by { for struct pattern
    if p.at(SyntaxKind::L_BRACE) {
        // Struct pattern with path
        parse_struct_fields(p)?;
        Ok(m.complete(p, SyntaxKind::StructPat))
    } else if p.at(SyntaxKind::L_PAREN) {
        // Enum pattern with path: Option::Some(x), Some(x)
        parse_enum_args(p)?;
        Ok(m.complete(p, SyntaxKind::TuplePat)) // Reuse TuplePat for enum patterns
    } else {
        // Just a path used as identifier pattern (for unit enum variants like None)
        Ok(m.complete(p, SyntaxKind::IdentPat))
    }
}

/// Parse an enum pattern: `Some(x)`, `Ok(value)`
fn enum_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    crate::parser::path::path_no_generics(p)?; // Parse the variant name
    parse_enum_args(p)?;
    Ok(m.complete(p, SyntaxKind::TuplePat)) // Reuse TuplePat for enum patterns
}

/// Parse enum pattern arguments: `(pattern, ...)`
fn parse_enum_args(p: &mut Parser<'_>) -> Result<(), ParseError> {
    p.expect(SyntaxKind::L_PAREN)?;

    if !p.at(SyntaxKind::R_PAREN) {
        pattern(p)?;
        while p.eat(SyntaxKind::COMMA) {
            if p.at(SyntaxKind::R_PAREN) {
                break;
            }
            pattern(p)?;
        }
    }

    p.expect(SyntaxKind::R_PAREN)?;
    Ok(())
}

/// Parse a struct pattern: `Point { x, y }`, `Point { x: a, y: b }`, `Point { x, .. }`
fn struct_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    crate::parser::path::path_no_generics(p)?; // Use Path for consistency (single-segment)
    parse_struct_fields(p)?;
    Ok(m.complete(p, SyntaxKind::StructPat))
}

/// Parse struct pattern fields: `{ x, y }`, `{ x: a, .. }`
fn parse_struct_fields(p: &mut Parser<'_>) -> Result<(), ParseError> {
    p.expect(SyntaxKind::L_BRACE)?;

    if !p.at(SyntaxKind::R_BRACE) {
        struct_pat_field(p)?;
        while p.eat(SyntaxKind::COMMA) {
            if p.at(SyntaxKind::R_BRACE) {
                break;
            }
            struct_pat_field(p)?;
        }
    }

    p.expect(SyntaxKind::R_BRACE)?;
    Ok(())
}

/// Parse a struct pattern field: `x`, `x: pattern`, or `..`
fn struct_pat_field(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    // Check for rest pattern `..`
    if p.at(SyntaxKind::DOT_DOT) {
        return rest_pat(p);
    }

    let m = p.start();
    crate::parser::path::name_ref(p)?; // Wrap in NameRef (field reference)

    // Check for `: pattern`
    if p.eat(SyntaxKind::COLON) {
        pattern(p)?;
    }

    Ok(m.complete(p, SyntaxKind::StructPatField))
}

/// Parse a rest pattern: `..`
fn rest_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume `..`
    Ok(m.complete(p, SyntaxKind::RestPat))
}

/// Parse a literal pattern, or a range pattern if followed by `..`
fn literal_or_range_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume the literal

    // Check if this is a range pattern: literal..literal or literal..
    if p.at(SyntaxKind::DOT_DOT) {
        p.bump(); // consume `..`

        // Optional end literal
        if matches!(
            p.current(),
            Some(SyntaxKind::INT_LITERAL)
                | Some(SyntaxKind::FLOAT_LITERAL)
                | Some(SyntaxKind::CHAR_LITERAL)
        ) {
            p.bump(); // consume end literal
        }

        Ok(m.complete(p, SyntaxKind::RangePat))
    } else {
        Ok(m.complete(p, SyntaxKind::LiteralPat))
    }
}

/// Parse a tuple pattern: `(a, b)`, `(a,)`, `()`
fn tuple_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume `(`

    if !p.at(SyntaxKind::R_PAREN) {
        pattern(p)?;
        while p.eat(SyntaxKind::COMMA) {
            if p.at(SyntaxKind::R_PAREN) {
                break;
            }
            pattern(p)?;
        }
    }

    p.expect(SyntaxKind::R_PAREN)?;
    Ok(m.complete(p, SyntaxKind::TuplePat))
}

/// Parse a slice pattern: `[a, b]`, `[first, .., last]`
fn slice_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume `[`

    if !p.at(SyntaxKind::R_BRACKET) {
        pattern(p)?;
        while p.eat(SyntaxKind::COMMA) {
            if p.at(SyntaxKind::R_BRACKET) {
                break;
            }
            pattern(p)?;
        }
    }

    p.expect(SyntaxKind::R_BRACKET)?;
    Ok(m.complete(p, SyntaxKind::SlicePat))
}

#[cfg(test)]
mod tests {
    use crate::parser::tests::check_expr;
    use expect_test::expect;

    #[test]
    fn wildcard_pattern() {
        check_expr(
            "{ let _ = x; }",
            &expect![[r#"
                BlockExpr@0..14
                  Block@0..14
                    L_BRACE@0..1 "{"
                    LetStmt@1..12
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      WildcardPat@5..7
                        WHITESPACE@5..6 " "
                        IDENT@6..7 "_"
                      WHITESPACE@7..8 " "
                      EQ@8..9 "="
                      PathExpr@9..11
                        Path@9..11
                          PathSegment@9..11
                            NameRef@9..11
                              WHITESPACE@9..10 " "
                              IDENT@10..11 "x"
                      SEMI@11..12 ";"
                    WHITESPACE@12..13 " "
                    R_BRACE@13..14 "}"
            "#]],
        );
    }

    #[test]
    fn ident_pattern() {
        check_expr(
            "{ let x = 1; }",
            &expect![[r#"
                BlockExpr@0..14
                  Block@0..14
                    L_BRACE@0..1 "{"
                    LetStmt@1..12
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      IdentPat@5..7
                        Name@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                      WHITESPACE@7..8 " "
                      EQ@8..9 "="
                      LiteralExpr@9..11
                        WHITESPACE@9..10 " "
                        INT_LITERAL@10..11 "1"
                      SEMI@11..12 ";"
                    WHITESPACE@12..13 " "
                    R_BRACE@13..14 "}"
            "#]],
        );
    }

    #[test]
    fn literal_pattern_int() {
        check_expr(
            "{ let 5 = x; }",
            &expect![[r#"
                BlockExpr@0..14
                  Block@0..14
                    L_BRACE@0..1 "{"
                    LetStmt@1..12
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      LiteralPat@5..7
                        WHITESPACE@5..6 " "
                        INT_LITERAL@6..7 "5"
                      WHITESPACE@7..8 " "
                      EQ@8..9 "="
                      PathExpr@9..11
                        Path@9..11
                          PathSegment@9..11
                            NameRef@9..11
                              WHITESPACE@9..10 " "
                              IDENT@10..11 "x"
                      SEMI@11..12 ";"
                    WHITESPACE@12..13 " "
                    R_BRACE@13..14 "}"
            "#]],
        );
    }

    #[test]
    fn literal_pattern_string() {
        check_expr(
            r#"{ let "hello" = x; }"#,
            &expect![[r#"
                BlockExpr@0..20
                  Block@0..20
                    L_BRACE@0..1 "{"
                    LetStmt@1..18
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      LiteralPat@5..13
                        WHITESPACE@5..6 " "
                        STRING_LITERAL@6..13 "\"hello\""
                      WHITESPACE@13..14 " "
                      EQ@14..15 "="
                      PathExpr@15..17
                        Path@15..17
                          PathSegment@15..17
                            NameRef@15..17
                              WHITESPACE@15..16 " "
                              IDENT@16..17 "x"
                      SEMI@17..18 ";"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn literal_pattern_bool() {
        check_expr(
            "{ let true = x; }",
            &expect![[r#"
                BlockExpr@0..17
                  Block@0..17
                    L_BRACE@0..1 "{"
                    LetStmt@1..15
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      LiteralPat@5..10
                        WHITESPACE@5..6 " "
                        TRUE_KW@6..10 "true"
                      WHITESPACE@10..11 " "
                      EQ@11..12 "="
                      PathExpr@12..14
                        Path@12..14
                          PathSegment@12..14
                            NameRef@12..14
                              WHITESPACE@12..13 " "
                              IDENT@13..14 "x"
                      SEMI@14..15 ";"
                    WHITESPACE@15..16 " "
                    R_BRACE@16..17 "}"
            "#]],
        );
    }

    #[test]
    fn literal_pattern_char() {
        check_expr(
            "{ let 'a' = x; }",
            &expect![[r#"
                BlockExpr@0..16
                  Block@0..16
                    L_BRACE@0..1 "{"
                    LetStmt@1..14
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      LiteralPat@5..9
                        WHITESPACE@5..6 " "
                        CHAR_LITERAL@6..9 "'a'"
                      WHITESPACE@9..10 " "
                      EQ@10..11 "="
                      PathExpr@11..13
                        Path@11..13
                          PathSegment@11..13
                            NameRef@11..13
                              WHITESPACE@11..12 " "
                              IDENT@12..13 "x"
                      SEMI@13..14 ";"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }

    #[test]
    fn rest_pattern() {
        check_expr(
            "{ let .. = x; }",
            &expect![[r#"
                BlockExpr@0..15
                  Block@0..15
                    L_BRACE@0..1 "{"
                    LetStmt@1..13
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      RestPat@5..8
                        WHITESPACE@5..6 " "
                        DOT_DOT@6..8 ".."
                      WHITESPACE@8..9 " "
                      EQ@9..10 "="
                      PathExpr@10..12
                        Path@10..12
                          PathSegment@10..12
                            NameRef@10..12
                              WHITESPACE@10..11 " "
                              IDENT@11..12 "x"
                      SEMI@12..13 ";"
                    WHITESPACE@13..14 " "
                    R_BRACE@14..15 "}"
            "#]],
        );
    }

    #[test]
    fn ref_pattern() {
        check_expr(
            "{ let &x = r; }",
            &expect![[r#"
                BlockExpr@0..15
                  Block@0..15
                    L_BRACE@0..1 "{"
                    LetStmt@1..13
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      RefPat@5..8
                        WHITESPACE@5..6 " "
                        AMP@6..7 "&"
                        IdentPat@7..8
                          Name@7..8
                            IDENT@7..8 "x"
                      WHITESPACE@8..9 " "
                      EQ@9..10 "="
                      PathExpr@10..12
                        Path@10..12
                          PathSegment@10..12
                            NameRef@10..12
                              WHITESPACE@10..11 " "
                              IDENT@11..12 "r"
                      SEMI@12..13 ";"
                    WHITESPACE@13..14 " "
                    R_BRACE@14..15 "}"
            "#]],
        );
    }

    #[test]
    fn ref_mut_pattern() {
        check_expr(
            "{ let &mut x = r; }",
            &expect![[r#"
                BlockExpr@0..19
                  Block@0..19
                    L_BRACE@0..1 "{"
                    LetStmt@1..17
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      RefPat@5..12
                        WHITESPACE@5..6 " "
                        AMP@6..7 "&"
                        MUT_KW@7..10 "mut"
                        IdentPat@10..12
                          Name@10..12
                            WHITESPACE@10..11 " "
                            IDENT@11..12 "x"
                      WHITESPACE@12..13 " "
                      EQ@13..14 "="
                      PathExpr@14..16
                        Path@14..16
                          PathSegment@14..16
                            NameRef@14..16
                              WHITESPACE@14..15 " "
                              IDENT@15..16 "r"
                      SEMI@16..17 ";"
                    WHITESPACE@17..18 " "
                    R_BRACE@18..19 "}"
            "#]],
        );
    }

    #[test]
    fn nested_ref_pattern() {
        // Note: using space between & to avoid && being lexed as AND_AND
        check_expr(
            "{ let & &x = r; }",
            &expect![[r#"
                BlockExpr@0..17
                  Block@0..17
                    L_BRACE@0..1 "{"
                    LetStmt@1..15
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      RefPat@5..10
                        WHITESPACE@5..6 " "
                        AMP@6..7 "&"
                        RefPat@7..10
                          WHITESPACE@7..8 " "
                          AMP@8..9 "&"
                          IdentPat@9..10
                            Name@9..10
                              IDENT@9..10 "x"
                      WHITESPACE@10..11 " "
                      EQ@11..12 "="
                      PathExpr@12..14
                        Path@12..14
                          PathSegment@12..14
                            NameRef@12..14
                              WHITESPACE@12..13 " "
                              IDENT@13..14 "r"
                      SEMI@14..15 ";"
                    WHITESPACE@15..16 " "
                    R_BRACE@16..17 "}"
            "#]],
        );
    }

    #[test]
    fn ref_wildcard_pattern() {
        check_expr(
            "{ let &_ = r; }",
            &expect![[r#"
                BlockExpr@0..15
                  Block@0..15
                    L_BRACE@0..1 "{"
                    LetStmt@1..13
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      RefPat@5..8
                        WHITESPACE@5..6 " "
                        AMP@6..7 "&"
                        WildcardPat@7..8
                          IDENT@7..8 "_"
                      WHITESPACE@8..9 " "
                      EQ@9..10 "="
                      PathExpr@10..12
                        Path@10..12
                          PathSegment@10..12
                            NameRef@10..12
                              WHITESPACE@10..11 " "
                              IDENT@11..12 "r"
                      SEMI@12..13 ";"
                    WHITESPACE@13..14 " "
                    R_BRACE@14..15 "}"
            "#]],
        );
    }

    #[test]
    fn tuple_pattern() {
        check_expr(
            "{ let (a, b) = t; }",
            &expect![[r#"
                BlockExpr@0..19
                  Block@0..19
                    L_BRACE@0..1 "{"
                    LetStmt@1..17
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      TuplePat@5..12
                        WHITESPACE@5..6 " "
                        L_PAREN@6..7 "("
                        IdentPat@7..8
                          Name@7..8
                            IDENT@7..8 "a"
                        COMMA@8..9 ","
                        IdentPat@9..11
                          Name@9..11
                            WHITESPACE@9..10 " "
                            IDENT@10..11 "b"
                        R_PAREN@11..12 ")"
                      WHITESPACE@12..13 " "
                      EQ@13..14 "="
                      PathExpr@14..16
                        Path@14..16
                          PathSegment@14..16
                            NameRef@14..16
                              WHITESPACE@14..15 " "
                              IDENT@15..16 "t"
                      SEMI@16..17 ";"
                    WHITESPACE@17..18 " "
                    R_BRACE@18..19 "}"
            "#]],
        );
    }

    #[test]
    fn tuple_pattern_single() {
        check_expr(
            "{ let (a,) = t; }",
            &expect![[r#"
                BlockExpr@0..17
                  Block@0..17
                    L_BRACE@0..1 "{"
                    LetStmt@1..15
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      TuplePat@5..10
                        WHITESPACE@5..6 " "
                        L_PAREN@6..7 "("
                        IdentPat@7..8
                          Name@7..8
                            IDENT@7..8 "a"
                        COMMA@8..9 ","
                        R_PAREN@9..10 ")"
                      WHITESPACE@10..11 " "
                      EQ@11..12 "="
                      PathExpr@12..14
                        Path@12..14
                          PathSegment@12..14
                            NameRef@12..14
                              WHITESPACE@12..13 " "
                              IDENT@13..14 "t"
                      SEMI@14..15 ";"
                    WHITESPACE@15..16 " "
                    R_BRACE@16..17 "}"
            "#]],
        );
    }

    #[test]
    fn tuple_pattern_empty() {
        check_expr(
            "{ let () = t; }",
            &expect![[r#"
                BlockExpr@0..15
                  Block@0..15
                    L_BRACE@0..1 "{"
                    LetStmt@1..13
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      TuplePat@5..8
                        WHITESPACE@5..6 " "
                        L_PAREN@6..7 "("
                        R_PAREN@7..8 ")"
                      WHITESPACE@8..9 " "
                      EQ@9..10 "="
                      PathExpr@10..12
                        Path@10..12
                          PathSegment@10..12
                            NameRef@10..12
                              WHITESPACE@10..11 " "
                              IDENT@11..12 "t"
                      SEMI@12..13 ";"
                    WHITESPACE@13..14 " "
                    R_BRACE@14..15 "}"
            "#]],
        );
    }

    #[test]
    fn slice_pattern() {
        check_expr(
            "{ let [a, b, c] = arr; }",
            &expect![[r#"
                BlockExpr@0..24
                  Block@0..24
                    L_BRACE@0..1 "{"
                    LetStmt@1..22
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      SlicePat@5..15
                        WHITESPACE@5..6 " "
                        L_BRACKET@6..7 "["
                        IdentPat@7..8
                          Name@7..8
                            IDENT@7..8 "a"
                        COMMA@8..9 ","
                        IdentPat@9..11
                          Name@9..11
                            WHITESPACE@9..10 " "
                            IDENT@10..11 "b"
                        COMMA@11..12 ","
                        IdentPat@12..14
                          Name@12..14
                            WHITESPACE@12..13 " "
                            IDENT@13..14 "c"
                        R_BRACKET@14..15 "]"
                      WHITESPACE@15..16 " "
                      EQ@16..17 "="
                      PathExpr@17..21
                        Path@17..21
                          PathSegment@17..21
                            NameRef@17..21
                              WHITESPACE@17..18 " "
                              IDENT@18..21 "arr"
                      SEMI@21..22 ";"
                    WHITESPACE@22..23 " "
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn slice_pattern_with_rest() {
        check_expr(
            "{ let [first, .., last] = arr; }",
            &expect![[r#"
                BlockExpr@0..32
                  Block@0..32
                    L_BRACE@0..1 "{"
                    LetStmt@1..30
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      SlicePat@5..23
                        WHITESPACE@5..6 " "
                        L_BRACKET@6..7 "["
                        IdentPat@7..12
                          Name@7..12
                            IDENT@7..12 "first"
                        COMMA@12..13 ","
                        RestPat@13..16
                          WHITESPACE@13..14 " "
                          DOT_DOT@14..16 ".."
                        COMMA@16..17 ","
                        IdentPat@17..22
                          Name@17..22
                            WHITESPACE@17..18 " "
                            IDENT@18..22 "last"
                        R_BRACKET@22..23 "]"
                      WHITESPACE@23..24 " "
                      EQ@24..25 "="
                      PathExpr@25..29
                        Path@25..29
                          PathSegment@25..29
                            NameRef@25..29
                              WHITESPACE@25..26 " "
                              IDENT@26..29 "arr"
                      SEMI@29..30 ";"
                    WHITESPACE@30..31 " "
                    R_BRACE@31..32 "}"
            "#]],
        );
    }

    #[test]
    fn range_pattern_full() {
        check_expr(
            "{ let 1..5 = x; }",
            &expect![[r#"
                BlockExpr@0..17
                  Block@0..17
                    L_BRACE@0..1 "{"
                    LetStmt@1..15
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      RangePat@5..10
                        WHITESPACE@5..6 " "
                        INT_LITERAL@6..7 "1"
                        DOT_DOT@7..9 ".."
                        INT_LITERAL@9..10 "5"
                      WHITESPACE@10..11 " "
                      EQ@11..12 "="
                      PathExpr@12..14
                        Path@12..14
                          PathSegment@12..14
                            NameRef@12..14
                              WHITESPACE@12..13 " "
                              IDENT@13..14 "x"
                      SEMI@14..15 ";"
                    WHITESPACE@15..16 " "
                    R_BRACE@16..17 "}"
            "#]],
        );
    }

    #[test]
    fn range_pattern_open_end() {
        check_expr(
            "{ let 1.. = x; }",
            &expect![[r#"
                BlockExpr@0..16
                  Block@0..16
                    L_BRACE@0..1 "{"
                    LetStmt@1..14
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      RangePat@5..9
                        WHITESPACE@5..6 " "
                        INT_LITERAL@6..7 "1"
                        DOT_DOT@7..9 ".."
                      WHITESPACE@9..10 " "
                      EQ@10..11 "="
                      PathExpr@11..13
                        Path@11..13
                          PathSegment@11..13
                            NameRef@11..13
                              WHITESPACE@11..12 " "
                              IDENT@12..13 "x"
                      SEMI@13..14 ";"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }

    #[test]
    fn range_pattern_char() {
        check_expr(
            "{ let 'a'..'z' = x; }",
            &expect![[r#"
                BlockExpr@0..21
                  Block@0..21
                    L_BRACE@0..1 "{"
                    LetStmt@1..19
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      RangePat@5..14
                        WHITESPACE@5..6 " "
                        CHAR_LITERAL@6..9 "'a'"
                        DOT_DOT@9..11 ".."
                        CHAR_LITERAL@11..14 "'z'"
                      WHITESPACE@14..15 " "
                      EQ@15..16 "="
                      PathExpr@16..18
                        Path@16..18
                          PathSegment@16..18
                            NameRef@16..18
                              WHITESPACE@16..17 " "
                              IDENT@17..18 "x"
                      SEMI@18..19 ";"
                    WHITESPACE@19..20 " "
                    R_BRACE@20..21 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pattern_simple() {
        check_expr(
            "{ let Point { x, y } = p; }",
            &expect![[r#"
                BlockExpr@0..27
                  Block@0..27
                    L_BRACE@0..1 "{"
                    LetStmt@1..25
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..20
                        Path@5..11
                          PathSegment@5..11
                            NameRef@5..11
                              WHITESPACE@5..6 " "
                              IDENT@6..11 "Point"
                        WHITESPACE@11..12 " "
                        L_BRACE@12..13 "{"
                        StructPatField@13..15
                          NameRef@13..15
                            WHITESPACE@13..14 " "
                            IDENT@14..15 "x"
                        COMMA@15..16 ","
                        StructPatField@16..18
                          NameRef@16..18
                            WHITESPACE@16..17 " "
                            IDENT@17..18 "y"
                        WHITESPACE@18..19 " "
                        R_BRACE@19..20 "}"
                      WHITESPACE@20..21 " "
                      EQ@21..22 "="
                      PathExpr@22..24
                        Path@22..24
                          PathSegment@22..24
                            NameRef@22..24
                              WHITESPACE@22..23 " "
                              IDENT@23..24 "p"
                      SEMI@24..25 ";"
                    WHITESPACE@25..26 " "
                    R_BRACE@26..27 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pattern_with_rename() {
        check_expr(
            "{ let Point { x: a, y: b } = p; }",
            &expect![[r#"
                BlockExpr@0..33
                  Block@0..33
                    L_BRACE@0..1 "{"
                    LetStmt@1..31
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..26
                        Path@5..11
                          PathSegment@5..11
                            NameRef@5..11
                              WHITESPACE@5..6 " "
                              IDENT@6..11 "Point"
                        WHITESPACE@11..12 " "
                        L_BRACE@12..13 "{"
                        StructPatField@13..18
                          NameRef@13..15
                            WHITESPACE@13..14 " "
                            IDENT@14..15 "x"
                          COLON@15..16 ":"
                          IdentPat@16..18
                            Name@16..18
                              WHITESPACE@16..17 " "
                              IDENT@17..18 "a"
                        COMMA@18..19 ","
                        StructPatField@19..24
                          NameRef@19..21
                            WHITESPACE@19..20 " "
                            IDENT@20..21 "y"
                          COLON@21..22 ":"
                          IdentPat@22..24
                            Name@22..24
                              WHITESPACE@22..23 " "
                              IDENT@23..24 "b"
                        WHITESPACE@24..25 " "
                        R_BRACE@25..26 "}"
                      WHITESPACE@26..27 " "
                      EQ@27..28 "="
                      PathExpr@28..30
                        Path@28..30
                          PathSegment@28..30
                            NameRef@28..30
                              WHITESPACE@28..29 " "
                              IDENT@29..30 "p"
                      SEMI@30..31 ";"
                    WHITESPACE@31..32 " "
                    R_BRACE@32..33 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pattern_with_rest() {
        check_expr(
            "{ let Point { x, .. } = p; }",
            &expect![[r#"
                BlockExpr@0..28
                  Block@0..28
                    L_BRACE@0..1 "{"
                    LetStmt@1..26
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..21
                        Path@5..11
                          PathSegment@5..11
                            NameRef@5..11
                              WHITESPACE@5..6 " "
                              IDENT@6..11 "Point"
                        WHITESPACE@11..12 " "
                        L_BRACE@12..13 "{"
                        StructPatField@13..15
                          NameRef@13..15
                            WHITESPACE@13..14 " "
                            IDENT@14..15 "x"
                        COMMA@15..16 ","
                        RestPat@16..19
                          WHITESPACE@16..17 " "
                          DOT_DOT@17..19 ".."
                        WHITESPACE@19..20 " "
                        R_BRACE@20..21 "}"
                      WHITESPACE@21..22 " "
                      EQ@22..23 "="
                      PathExpr@23..25
                        Path@23..25
                          PathSegment@23..25
                            NameRef@23..25
                              WHITESPACE@23..24 " "
                              IDENT@24..25 "p"
                      SEMI@25..26 ";"
                    WHITESPACE@26..27 " "
                    R_BRACE@27..28 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pattern_qualified_path() {
        check_expr(
            "{ let module::Point { x } = p; }",
            &expect![[r#"
                BlockExpr@0..32
                  Block@0..32
                    L_BRACE@0..1 "{"
                    LetStmt@1..30
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..25
                        Path@5..19
                          PathSegment@5..12
                            NameRef@5..12
                              WHITESPACE@5..6 " "
                              IDENT@6..12 "module"
                          COLON_COLON@12..14 "::"
                          PathSegment@14..19
                            NameRef@14..19
                              IDENT@14..19 "Point"
                        WHITESPACE@19..20 " "
                        L_BRACE@20..21 "{"
                        StructPatField@21..23
                          NameRef@21..23
                            WHITESPACE@21..22 " "
                            IDENT@22..23 "x"
                        WHITESPACE@23..24 " "
                        R_BRACE@24..25 "}"
                      WHITESPACE@25..26 " "
                      EQ@26..27 "="
                      PathExpr@27..29
                        Path@27..29
                          PathSegment@27..29
                            NameRef@27..29
                              WHITESPACE@27..28 " "
                              IDENT@28..29 "p"
                      SEMI@29..30 ";"
                    WHITESPACE@30..31 " "
                    R_BRACE@31..32 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pattern_empty() {
        check_expr(
            "{ let Unit {} = u; }",
            &expect![[r#"
                BlockExpr@0..20
                  Block@0..20
                    L_BRACE@0..1 "{"
                    LetStmt@1..18
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..13
                        Path@5..10
                          PathSegment@5..10
                            NameRef@5..10
                              WHITESPACE@5..6 " "
                              IDENT@6..10 "Unit"
                        WHITESPACE@10..11 " "
                        L_BRACE@11..12 "{"
                        R_BRACE@12..13 "}"
                      WHITESPACE@13..14 " "
                      EQ@14..15 "="
                      PathExpr@15..17
                        Path@15..17
                          PathSegment@15..17
                            NameRef@15..17
                              WHITESPACE@15..16 " "
                              IDENT@16..17 "u"
                      SEMI@17..18 ";"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pattern_nested() {
        check_expr(
            "{ let Outer { inner: Inner { x } } = o; }",
            &expect![[r#"
                BlockExpr@0..41
                  Block@0..41
                    L_BRACE@0..1 "{"
                    LetStmt@1..39
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..34
                        Path@5..11
                          PathSegment@5..11
                            NameRef@5..11
                              WHITESPACE@5..6 " "
                              IDENT@6..11 "Outer"
                        WHITESPACE@11..12 " "
                        L_BRACE@12..13 "{"
                        StructPatField@13..32
                          NameRef@13..19
                            WHITESPACE@13..14 " "
                            IDENT@14..19 "inner"
                          COLON@19..20 ":"
                          StructPat@20..32
                            Path@20..26
                              PathSegment@20..26
                                NameRef@20..26
                                  WHITESPACE@20..21 " "
                                  IDENT@21..26 "Inner"
                            WHITESPACE@26..27 " "
                            L_BRACE@27..28 "{"
                            StructPatField@28..30
                              NameRef@28..30
                                WHITESPACE@28..29 " "
                                IDENT@29..30 "x"
                            WHITESPACE@30..31 " "
                            R_BRACE@31..32 "}"
                        WHITESPACE@32..33 " "
                        R_BRACE@33..34 "}"
                      WHITESPACE@34..35 " "
                      EQ@35..36 "="
                      PathExpr@36..38
                        Path@36..38
                          PathSegment@36..38
                            NameRef@36..38
                              WHITESPACE@36..37 " "
                              IDENT@37..38 "o"
                      SEMI@38..39 ";"
                    WHITESPACE@39..40 " "
                    R_BRACE@40..41 "}"
            "#]],
        );
    }

    // === Phase 4: Pattern Edge Cases ===

    #[test]
    fn tuple_pattern_nested() {
        check_expr(
            "{ let ((a, b), (c, d)) = x; }",
            &expect![[r#"
                BlockExpr@0..29
                  Block@0..29
                    L_BRACE@0..1 "{"
                    LetStmt@1..27
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      TuplePat@5..22
                        WHITESPACE@5..6 " "
                        L_PAREN@6..7 "("
                        TuplePat@7..13
                          L_PAREN@7..8 "("
                          IdentPat@8..9
                            Name@8..9
                              IDENT@8..9 "a"
                          COMMA@9..10 ","
                          IdentPat@10..12
                            Name@10..12
                              WHITESPACE@10..11 " "
                              IDENT@11..12 "b"
                          R_PAREN@12..13 ")"
                        COMMA@13..14 ","
                        TuplePat@14..21
                          WHITESPACE@14..15 " "
                          L_PAREN@15..16 "("
                          IdentPat@16..17
                            Name@16..17
                              IDENT@16..17 "c"
                          COMMA@17..18 ","
                          IdentPat@18..20
                            Name@18..20
                              WHITESPACE@18..19 " "
                              IDENT@19..20 "d"
                          R_PAREN@20..21 ")"
                        R_PAREN@21..22 ")"
                      WHITESPACE@22..23 " "
                      EQ@23..24 "="
                      PathExpr@24..26
                        Path@24..26
                          PathSegment@24..26
                            NameRef@24..26
                              WHITESPACE@24..25 " "
                              IDENT@25..26 "x"
                      SEMI@26..27 ";"
                    WHITESPACE@27..28 " "
                    R_BRACE@28..29 "}"
            "#]],
        );
    }

    #[test]
    fn tuple_pattern_with_wildcard() {
        check_expr(
            "{ let (a, _, c) = x; }",
            &expect![[r#"
                BlockExpr@0..22
                  Block@0..22
                    L_BRACE@0..1 "{"
                    LetStmt@1..20
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      TuplePat@5..15
                        WHITESPACE@5..6 " "
                        L_PAREN@6..7 "("
                        IdentPat@7..8
                          Name@7..8
                            IDENT@7..8 "a"
                        COMMA@8..9 ","
                        WildcardPat@9..11
                          WHITESPACE@9..10 " "
                          IDENT@10..11 "_"
                        COMMA@11..12 ","
                        IdentPat@12..14
                          Name@12..14
                            WHITESPACE@12..13 " "
                            IDENT@13..14 "c"
                        R_PAREN@14..15 ")"
                      WHITESPACE@15..16 " "
                      EQ@16..17 "="
                      PathExpr@17..19
                        Path@17..19
                          PathSegment@17..19
                            NameRef@17..19
                              WHITESPACE@17..18 " "
                              IDENT@18..19 "x"
                      SEMI@19..20 ";"
                    WHITESPACE@20..21 " "
                    R_BRACE@21..22 "}"
            "#]],
        );
    }

    #[test]
    fn slice_pattern_empty() {
        check_expr(
            "{ let [] = x; }",
            &expect![[r#"
                BlockExpr@0..15
                  Block@0..15
                    L_BRACE@0..1 "{"
                    LetStmt@1..13
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      SlicePat@5..8
                        WHITESPACE@5..6 " "
                        L_BRACKET@6..7 "["
                        R_BRACKET@7..8 "]"
                      WHITESPACE@8..9 " "
                      EQ@9..10 "="
                      PathExpr@10..12
                        Path@10..12
                          PathSegment@10..12
                            NameRef@10..12
                              WHITESPACE@10..11 " "
                              IDENT@11..12 "x"
                      SEMI@12..13 ";"
                    WHITESPACE@13..14 " "
                    R_BRACE@14..15 "}"
            "#]],
        );
    }

    #[test]
    fn slice_pattern_single() {
        check_expr(
            "{ let [a] = x; }",
            &expect![[r#"
                BlockExpr@0..16
                  Block@0..16
                    L_BRACE@0..1 "{"
                    LetStmt@1..14
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      SlicePat@5..9
                        WHITESPACE@5..6 " "
                        L_BRACKET@6..7 "["
                        IdentPat@7..8
                          Name@7..8
                            IDENT@7..8 "a"
                        R_BRACKET@8..9 "]"
                      WHITESPACE@9..10 " "
                      EQ@10..11 "="
                      PathExpr@11..13
                        Path@11..13
                          PathSegment@11..13
                            NameRef@11..13
                              WHITESPACE@11..12 " "
                              IDENT@12..13 "x"
                      SEMI@13..14 ";"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }

    #[test]
    fn slice_pattern_rest_at_start() {
        check_expr(
            "{ let [.., last] = x; }",
            &expect![[r#"
                BlockExpr@0..23
                  Block@0..23
                    L_BRACE@0..1 "{"
                    LetStmt@1..21
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      SlicePat@5..16
                        WHITESPACE@5..6 " "
                        L_BRACKET@6..7 "["
                        RestPat@7..9
                          DOT_DOT@7..9 ".."
                        COMMA@9..10 ","
                        IdentPat@10..15
                          Name@10..15
                            WHITESPACE@10..11 " "
                            IDENT@11..15 "last"
                        R_BRACKET@15..16 "]"
                      WHITESPACE@16..17 " "
                      EQ@17..18 "="
                      PathExpr@18..20
                        Path@18..20
                          PathSegment@18..20
                            NameRef@18..20
                              WHITESPACE@18..19 " "
                              IDENT@19..20 "x"
                      SEMI@20..21 ";"
                    WHITESPACE@21..22 " "
                    R_BRACE@22..23 "}"
            "#]],
        );
    }

    #[test]
    fn slice_pattern_rest_at_end() {
        check_expr(
            "{ let [first, ..] = x; }",
            &expect![[r#"
                BlockExpr@0..24
                  Block@0..24
                    L_BRACE@0..1 "{"
                    LetStmt@1..22
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      SlicePat@5..17
                        WHITESPACE@5..6 " "
                        L_BRACKET@6..7 "["
                        IdentPat@7..12
                          Name@7..12
                            IDENT@7..12 "first"
                        COMMA@12..13 ","
                        RestPat@13..16
                          WHITESPACE@13..14 " "
                          DOT_DOT@14..16 ".."
                        R_BRACKET@16..17 "]"
                      WHITESPACE@17..18 " "
                      EQ@18..19 "="
                      PathExpr@19..21
                        Path@19..21
                          PathSegment@19..21
                            NameRef@19..21
                              WHITESPACE@19..20 " "
                              IDENT@20..21 "x"
                      SEMI@21..22 ";"
                    WHITESPACE@22..23 " "
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pattern_all_rest() {
        check_expr(
            "{ let Point { .. } = p; }",
            &expect![[r#"
                BlockExpr@0..25
                  Block@0..25
                    L_BRACE@0..1 "{"
                    LetStmt@1..23
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..18
                        Path@5..11
                          PathSegment@5..11
                            NameRef@5..11
                              WHITESPACE@5..6 " "
                              IDENT@6..11 "Point"
                        WHITESPACE@11..12 " "
                        L_BRACE@12..13 "{"
                        RestPat@13..16
                          WHITESPACE@13..14 " "
                          DOT_DOT@14..16 ".."
                        WHITESPACE@16..17 " "
                        R_BRACE@17..18 "}"
                      WHITESPACE@18..19 " "
                      EQ@19..20 "="
                      PathExpr@20..22
                        Path@20..22
                          PathSegment@20..22
                            NameRef@20..22
                              WHITESPACE@20..21 " "
                              IDENT@21..22 "p"
                      SEMI@22..23 ";"
                    WHITESPACE@23..24 " "
                    R_BRACE@24..25 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pattern_deeply_nested() {
        check_expr(
            "{ let A { b: B { c: C { x } } } = a; }",
            &expect![[r#"
                BlockExpr@0..38
                  Block@0..38
                    L_BRACE@0..1 "{"
                    LetStmt@1..36
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..31
                        Path@5..7
                          PathSegment@5..7
                            NameRef@5..7
                              WHITESPACE@5..6 " "
                              IDENT@6..7 "A"
                        WHITESPACE@7..8 " "
                        L_BRACE@8..9 "{"
                        StructPatField@9..29
                          NameRef@9..11
                            WHITESPACE@9..10 " "
                            IDENT@10..11 "b"
                          COLON@11..12 ":"
                          StructPat@12..29
                            Path@12..14
                              PathSegment@12..14
                                NameRef@12..14
                                  WHITESPACE@12..13 " "
                                  IDENT@13..14 "B"
                            WHITESPACE@14..15 " "
                            L_BRACE@15..16 "{"
                            StructPatField@16..27
                              NameRef@16..18
                                WHITESPACE@16..17 " "
                                IDENT@17..18 "c"
                              COLON@18..19 ":"
                              StructPat@19..27
                                Path@19..21
                                  PathSegment@19..21
                                    NameRef@19..21
                                      WHITESPACE@19..20 " "
                                      IDENT@20..21 "C"
                                WHITESPACE@21..22 " "
                                L_BRACE@22..23 "{"
                                StructPatField@23..25
                                  NameRef@23..25
                                    WHITESPACE@23..24 " "
                                    IDENT@24..25 "x"
                                WHITESPACE@25..26 " "
                                R_BRACE@26..27 "}"
                            WHITESPACE@27..28 " "
                            R_BRACE@28..29 "}"
                        WHITESPACE@29..30 " "
                        R_BRACE@30..31 "}"
                      WHITESPACE@31..32 " "
                      EQ@32..33 "="
                      PathExpr@33..35
                        Path@33..35
                          PathSegment@33..35
                            NameRef@33..35
                              WHITESPACE@33..34 " "
                              IDENT@34..35 "a"
                      SEMI@35..36 ";"
                    WHITESPACE@36..37 " "
                    R_BRACE@37..38 "}"
            "#]],
        );
    }

    #[test]
    fn tuple_pattern_with_ref() {
        check_expr(
            "{ let (&a, &mut b) = x; }",
            &expect![[r#"
                BlockExpr@0..25
                  Block@0..25
                    L_BRACE@0..1 "{"
                    LetStmt@1..23
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      TuplePat@5..18
                        WHITESPACE@5..6 " "
                        L_PAREN@6..7 "("
                        RefPat@7..9
                          AMP@7..8 "&"
                          IdentPat@8..9
                            Name@8..9
                              IDENT@8..9 "a"
                        COMMA@9..10 ","
                        RefPat@10..17
                          WHITESPACE@10..11 " "
                          AMP@11..12 "&"
                          MUT_KW@12..15 "mut"
                          IdentPat@15..17
                            Name@15..17
                              WHITESPACE@15..16 " "
                              IDENT@16..17 "b"
                        R_PAREN@17..18 ")"
                      WHITESPACE@18..19 " "
                      EQ@19..20 "="
                      PathExpr@20..22
                        Path@20..22
                          PathSegment@20..22
                            NameRef@20..22
                              WHITESPACE@20..21 " "
                              IDENT@21..22 "x"
                      SEMI@22..23 ";"
                    WHITESPACE@23..24 " "
                    R_BRACE@24..25 "}"
            "#]],
        );
    }

    #[test]
    fn literal_pattern_false() {
        check_expr(
            "{ let false = x; }",
            &expect![[r#"
                BlockExpr@0..18
                  Block@0..18
                    L_BRACE@0..1 "{"
                    LetStmt@1..16
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      LiteralPat@5..11
                        WHITESPACE@5..6 " "
                        FALSE_KW@6..11 "false"
                      WHITESPACE@11..12 " "
                      EQ@12..13 "="
                      PathExpr@13..15
                        Path@13..15
                          PathSegment@13..15
                            NameRef@13..15
                              WHITESPACE@13..14 " "
                              IDENT@14..15 "x"
                      SEMI@15..16 ";"
                    WHITESPACE@16..17 " "
                    R_BRACE@17..18 "}"
            "#]],
        );
    }
}
