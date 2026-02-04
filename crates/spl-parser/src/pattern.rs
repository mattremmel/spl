//! Pattern parser for SPL.
//!
//! Parses patterns used in let bindings, match arms, and function parameters.

use crate::{CompletedMarker, ParseError, Parser};
use spl_syntax::SyntaxKind;

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
                Ok(wildcard_pat(p))
            } else {
                // Could be ident, path, or struct pattern
                // Lookahead to check if this is a struct pattern (path followed by {)
                ident_or_struct_pat(p)
            }
        },
        // Path-starting keywords that can begin qualified patterns (module.Type, super.Type, etc.)
        // Note: 'crate' keyword was removed - use '$' for package root
        MODULE_KW | SUPER_KW | SELF_VALUE_KW => {
            // These keywords can start paths in patterns (e.g., module.Point(x: x))
            path_or_struct_or_enum_pat(p)
        },
        // Enum shorthand pattern: .Variant or .Variant(patterns)
        DOT => {
            // Check if followed by identifier (enum shorthand)
            if p.peek(1) == Some(SyntaxKind::IDENT) {
                enum_shorthand_pat(p)
            } else {
                let err = p.error_at_current("expected pattern".to_string());
                Err(err)
            }
        },
        // Rest pattern: ..
        DOT_DOT => Ok(rest_pat(p)),
        // Literal patterns (may be range pattern if followed by ..)
        INT_LITERAL | FLOAT_LITERAL | STRING_LITERAL | CHAR_LITERAL | TRUE_KW | FALSE_KW => {
            Ok(literal_or_range_pat(p))
        },
        _ => {
            let err = p.error_at_current("expected pattern".to_string());
            Err(err)
        },
    })
}

/// Parse an enum shorthand pattern: `.Variant` or `.Variant(patterns)`
fn enum_shorthand_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();

    // Consume the leading dot
    p.expect(SyntaxKind::DOT)?;

    // Parse the variant name
    crate::item::name(p)?;

    // Optional pattern list: (pattern, ...)
    if p.at(SyntaxKind::L_PAREN) {
        p.expect(SyntaxKind::L_PAREN)?;
        p.parse_delimited(SyntaxKind::R_PAREN, |p| {
            pattern(p)?;
            Ok(())
        })?;
        p.expect(SyntaxKind::R_PAREN)?;
    }

    Ok(m.complete(p, SyntaxKind::EnumShorthandPat))
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
fn wildcard_pat(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // consume `_`
    m.complete(p, SyntaxKind::WildcardPat)
}

/// Parse an identifier pattern: `x`, `foo`
fn ident_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    crate::item::name(p)?; // Wrap in Name (binding site)
    Ok(m.complete(p, SyntaxKind::IdentPat))
}

/// Parse an identifier, path, struct pattern, or enum pattern.
/// Lookahead is needed to distinguish:
/// - `x` (ident)
/// - `Point(x: a)` (struct pattern with named field)
/// - `Point(x, y)` (struct pattern shorthand OR enum pattern)
/// - `Some(x)` (enum pattern)
fn ident_or_struct_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    // Check if this is a path (has . after identifier for qualified paths)
    let is_path = p.peek(1) == Some(SyntaxKind::DOT);
    // Check if this is a pattern with parentheses (struct or enum)
    let has_parens = p.peek(1) == Some(SyntaxKind::L_PAREN);
    // Check if it looks like a struct pattern: Path(IDENT = ...)
    let is_struct = has_parens && looks_like_struct_pat(p);

    if is_path {
        // This is a path - could be struct pattern or enum pattern with qualified name
        path_or_struct_or_enum_pat(p)
    } else if is_struct {
        // Struct pattern with named fields: Name(x: a, ...)
        struct_pat(p)
    } else if has_parens {
        // Enum pattern or struct shorthand: Some(x) or Point(x, y)
        // Parse as enum pattern; semantic analysis will handle struct shorthand
        enum_pat(p)
    } else {
        // Simple identifier pattern
        ident_pat(p)
    }
}

/// Check if the upcoming pattern looks like a struct pattern.
/// Returns true if we see `(IDENT :` indicating a named struct field.
fn looks_like_struct_pat(p: &mut Parser<'_>) -> bool {
    // peek(1) is L_PAREN, peek(2) is first token inside
    let first = p.peek(2);
    let second = p.peek(3);

    match (first, second) {
        // IDENT : ... (named field)
        (Some(SyntaxKind::IDENT), Some(SyntaxKind::COLON)) => true,
        // Otherwise (shorthand or enum pattern) - parse as enum
        _ => false,
    }
}

/// Parse a path and determine if it's followed by struct fields or enum args.
fn path_or_struct_or_enum_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in patterns)
    crate::path::path_no_generics(p)?;

    // Check if followed by ( for struct or enum pattern
    if p.at(SyntaxKind::L_PAREN) {
        // Look ahead to determine struct vs enum pattern
        // Struct pattern has named fields: (IDENT : ...)
        let first = p.peek(1);
        let second = p.peek(2);
        let is_struct = matches!(
            (first, second),
            (Some(SyntaxKind::IDENT), Some(SyntaxKind::COLON))
        );

        if is_struct {
            // Struct pattern with path: path::Point(x: a, ...)
            parse_struct_fields(p)?;
            Ok(m.complete(p, SyntaxKind::StructPat))
        } else {
            // Enum pattern with path: Option::Some(x), Some(x)
            parse_enum_args(p)?;
            Ok(m.complete(p, SyntaxKind::TuplePat)) // Reuse TuplePat for enum patterns
        }
    } else {
        // Just a path used as identifier pattern (for unit enum variants like None)
        Ok(m.complete(p, SyntaxKind::IdentPat))
    }
}

/// Parse an enum pattern: `Some(x)`, `Ok(value)`
fn enum_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    crate::path::path_no_generics(p)?; // Parse the variant name
    parse_enum_args(p)?;
    Ok(m.complete(p, SyntaxKind::TuplePat)) // Reuse TuplePat for enum patterns
}

/// Parse enum pattern arguments: `(pattern, ...)`
fn parse_enum_args(p: &mut Parser<'_>) -> Result<(), ParseError> {
    p.expect(SyntaxKind::L_PAREN)?;
    p.parse_delimited(SyntaxKind::R_PAREN, |p| {
        pattern(p)?;
        Ok(())
    })?;
    p.expect(SyntaxKind::R_PAREN)?;
    Ok(())
}

/// Parse a struct pattern: `Point(x: a, y: b)`, `Point(x, y)`, `Point(x, ..)`
fn struct_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    crate::path::path_no_generics(p)?; // Use Path for consistency (single-segment)
    parse_struct_fields(p)?;
    Ok(m.complete(p, SyntaxKind::StructPat))
}

/// Parse struct pattern fields: `(x: a, y: b)`, `(x, y, ..)`
fn parse_struct_fields(p: &mut Parser<'_>) -> Result<(), ParseError> {
    p.expect(SyntaxKind::L_PAREN)?;
    p.parse_delimited(SyntaxKind::R_PAREN, |p| {
        struct_pat_field(p)?;
        Ok(())
    })?;
    p.expect(SyntaxKind::R_PAREN)?;
    Ok(())
}

/// Parse a struct pattern field: `x`, `x: pattern`, or `..`
fn struct_pat_field(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    // Check for rest pattern `..`
    if p.at(SyntaxKind::DOT_DOT) {
        return Ok(rest_pat(p));
    }

    let m = p.start();
    crate::path::name_ref(p)?; // Wrap in NameRef (field reference)

    // Check for `: pattern`
    if p.eat(SyntaxKind::COLON) {
        pattern(p)?;
    }

    Ok(m.complete(p, SyntaxKind::StructPatField))
}

/// Parse a rest pattern: `..`
fn rest_pat(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // consume `..`
    m.complete(p, SyntaxKind::RestPat)
}

/// Parse a literal pattern, or a range pattern if followed by `..`
fn literal_or_range_pat(p: &mut Parser<'_>) -> CompletedMarker {
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

        m.complete(p, SyntaxKind::RangePat)
    } else {
        m.complete(p, SyntaxKind::LiteralPat)
    }
}

/// Parse a tuple pattern: `(a, b)`, `(a,)`, `()`
fn tuple_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume `(`
    p.parse_delimited(SyntaxKind::R_PAREN, |p| {
        pattern(p)?;
        Ok(())
    })?;
    p.expect(SyntaxKind::R_PAREN)?;
    Ok(m.complete(p, SyntaxKind::TuplePat))
}

/// Parse a slice pattern: `[a, b]`, `[first, .., last]`
fn slice_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume `[`
    p.parse_delimited(SyntaxKind::R_BRACKET, |p| {
        pattern(p)?;
        Ok(())
    })?;
    p.expect(SyntaxKind::R_BRACKET)?;
    Ok(m.complete(p, SyntaxKind::SlicePat))
}

#[cfg(test)]
mod tests {
    use crate::tests::check_expr;
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
    fn struct_pattern_simple() {
        check_expr(
            "{ let Point(x: x, y: y) = p; }",
            &expect![[r#"
                BlockExpr@0..30
                  Block@0..30
                    L_BRACE@0..1 "{"
                    LetStmt@1..28
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      StructPat@5..23
                        Path@5..11
                          PathSegment@5..11
                            NameRef@5..11
                              WHITESPACE@5..6 " "
                              IDENT@6..11 "Point"
                        L_PAREN@11..12 "("
                        StructPatField@12..16
                          NameRef@12..13
                            IDENT@12..13 "x"
                          COLON@13..14 ":"
                          IdentPat@14..16
                            Name@14..16
                              WHITESPACE@14..15 " "
                              IDENT@15..16 "x"
                        COMMA@16..17 ","
                        StructPatField@17..22
                          NameRef@17..19
                            WHITESPACE@17..18 " "
                            IDENT@18..19 "y"
                          COLON@19..20 ":"
                          IdentPat@20..22
                            Name@20..22
                              WHITESPACE@20..21 " "
                              IDENT@21..22 "y"
                        R_PAREN@22..23 ")"
                      WHITESPACE@23..24 " "
                      EQ@24..25 "="
                      PathExpr@25..27
                        Path@25..27
                          PathSegment@25..27
                            NameRef@25..27
                              WHITESPACE@25..26 " "
                              IDENT@26..27 "p"
                      SEMI@27..28 ";"
                    WHITESPACE@28..29 " "
                    R_BRACE@29..30 "}"
            "#]],
        );
    }

    // === Enum Shorthand Patterns ===

    #[test]
    fn enum_shorthand_pattern_unit() {
        check_expr(
            "{ let .None = x; }",
            &expect![[r#"
                BlockExpr@0..18
                  Block@0..18
                    L_BRACE@0..1 "{"
                    LetStmt@1..16
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      EnumShorthandPat@5..11
                        WHITESPACE@5..6 " "
                        DOT@6..7 "."
                        Name@7..11
                          IDENT@7..11 "None"
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

    #[test]
    fn enum_shorthand_pattern_binding() {
        check_expr(
            "{ let .Some(x) = opt; }",
            &expect![[r#"
                BlockExpr@0..23
                  Block@0..23
                    L_BRACE@0..1 "{"
                    LetStmt@1..21
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      EnumShorthandPat@5..14
                        WHITESPACE@5..6 " "
                        DOT@6..7 "."
                        Name@7..11
                          IDENT@7..11 "Some"
                        L_PAREN@11..12 "("
                        IdentPat@12..13
                          Name@12..13
                            IDENT@12..13 "x"
                        R_PAREN@13..14 ")"
                      WHITESPACE@14..15 " "
                      EQ@15..16 "="
                      PathExpr@16..20
                        Path@16..20
                          PathSegment@16..20
                            NameRef@16..20
                              WHITESPACE@16..17 " "
                              IDENT@17..20 "opt"
                      SEMI@20..21 ";"
                    WHITESPACE@21..22 " "
                    R_BRACE@22..23 "}"
            "#]],
        );
    }

    #[test]
    fn enum_shorthand_pattern_nested() {
        check_expr(
            "{ let .Some(.Inner(x)) = opt; }",
            &expect![[r#"
                BlockExpr@0..31
                  Block@0..31
                    L_BRACE@0..1 "{"
                    LetStmt@1..29
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      EnumShorthandPat@5..22
                        WHITESPACE@5..6 " "
                        DOT@6..7 "."
                        Name@7..11
                          IDENT@7..11 "Some"
                        L_PAREN@11..12 "("
                        EnumShorthandPat@12..21
                          DOT@12..13 "."
                          Name@13..18
                            IDENT@13..18 "Inner"
                          L_PAREN@18..19 "("
                          IdentPat@19..20
                            Name@19..20
                              IDENT@19..20 "x"
                          R_PAREN@20..21 ")"
                        R_PAREN@21..22 ")"
                      WHITESPACE@22..23 " "
                      EQ@23..24 "="
                      PathExpr@24..28
                        Path@24..28
                          PathSegment@24..28
                            NameRef@24..28
                              WHITESPACE@24..25 " "
                              IDENT@25..28 "opt"
                      SEMI@28..29 ";"
                    WHITESPACE@29..30 " "
                    R_BRACE@30..31 "}"
            "#]],
        );
    }

    #[test]
    fn enum_shorthand_pattern_in_match() {
        check_expr(
            "match x { .None => 0, .Some(v) => v }",
            &expect![[r#"
                MatchExpr@0..37
                  MATCH_KW@0..5 "match"
                  PathExpr@5..7
                    Path@5..7
                      PathSegment@5..7
                        NameRef@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                  WHITESPACE@7..8 " "
                  L_BRACE@8..9 "{"
                  MatchArm@9..21
                    EnumShorthandPat@9..15
                      WHITESPACE@9..10 " "
                      DOT@10..11 "."
                      Name@11..15
                        IDENT@11..15 "None"
                    WHITESPACE@15..16 " "
                    FAT_ARROW@16..18 "=>"
                    LiteralExpr@18..20
                      WHITESPACE@18..19 " "
                      INT_LITERAL@19..20 "0"
                    COMMA@20..21 ","
                  MatchArm@21..35
                    EnumShorthandPat@21..30
                      WHITESPACE@21..22 " "
                      DOT@22..23 "."
                      Name@23..27
                        IDENT@23..27 "Some"
                      L_PAREN@27..28 "("
                      IdentPat@28..29
                        Name@28..29
                          IDENT@28..29 "v"
                      R_PAREN@29..30 ")"
                    WHITESPACE@30..31 " "
                    FAT_ARROW@31..33 "=>"
                    PathExpr@33..35
                      Path@33..35
                        PathSegment@33..35
                          NameRef@33..35
                            WHITESPACE@33..34 " "
                            IDENT@34..35 "v"
                  WHITESPACE@35..36 " "
                  R_BRACE@36..37 "}"
            "#]],
        );
    }
}
