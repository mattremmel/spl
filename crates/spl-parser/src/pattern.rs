//! Pattern parser for SPL.
//!
//! Parses patterns used in let bindings, match arms, and function parameters.

use crate::{CompletedMarker, ParseError, Parser};
use spl_syntax::SyntaxKind;

/// Parse a pattern (possibly an or-pattern).
///
/// ```text
/// pattern = single_pattern { "|" single_pattern }
/// ```
pub fn pattern(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let first = single_pattern(p)?;

    // Check for or-pattern continuation
    if !p.at(SyntaxKind::PIPE) {
        return Ok(first);
    }

    // Wrap in OrPat and parse remaining alternatives
    let m = first.precede(p);
    while p.eat(SyntaxKind::PIPE) {
        single_pattern(p)?;
    }
    Ok(m.complete(p, SyntaxKind::OrPat))
}

/// Parse a single pattern (no or-alternation at this level).
///
/// ```text
/// single_pattern = ref_pat | tuple_or_grouped_pat | slice_pat | wildcard_pat | literal_or_range_pat | rest_pat | ident_pat
/// ```
fn single_pattern(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    match_token!(p, {
        // Reference pattern: &x, &mut x
        AMP => ref_pat(p),
        // Tuple or grouped pattern: (a, b) or (a)
        L_PAREN => tuple_or_grouped_pat(p),
        // Slice pattern: [a, b]
        L_BRACKET => slice_pat(p),
        // Mutable binding pattern: `mut x`
        MUT_KW => {
            let m = p.start();
            p.bump(); // consume `mut`
            if let Err(e) = crate::item::name(p) {
                m.abandon(p);
                return Err(e);
            }
            Ok(m.complete(p, SyntaxKind::IdentPat))
        },
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
        MODULE_KW | SUPER_KW | SELF_VALUE_KW | SELF_TYPE_KW | DOLLAR => {
            // These keywords/tokens can start paths in patterns (e.g., module.Point(x: x), $.constants.MAX, Self(a, b))
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
        // Negative literal patterns: -1, -3.14
        MINUS => {
            if matches!(p.peek(1), Some(SyntaxKind::INT_LITERAL) | Some(SyntaxKind::FLOAT_LITERAL)) {
                Ok(negative_literal_or_range_pat(p))
            } else {
                let err = p.error_at_current("expected pattern".to_string());
                Err(err)
            }
        },
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
    if let Err(e) = p.expect(SyntaxKind::DOT) {
        m.abandon(p);
        return Err(e);
    }

    // Parse the variant name
    if let Err(e) = crate::item::name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional field/pattern list: (name: pat, ...) or (pat, ...)
    if p.at(SyntaxKind::L_PAREN)
        && let Err(e) = parse_enum_variant_fields(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::EnumShorthandPat))
}

/// Parse enum variant fields: `(pat, ...)` or `(name: pat, ...)`
///
/// Each item can be a named field (`x: pattern`) or a positional pattern.
/// Named fields are detected by IDENT followed by COLON.
fn parse_enum_variant_fields(p: &mut Parser<'_>) -> Result<(), ParseError> {
    p.expect(SyntaxKind::L_PAREN)?;
    p.parse_delimited(SyntaxKind::R_PAREN, |p| {
        // Check for rest pattern `..`
        if p.at(SyntaxKind::DOT_DOT) {
            rest_pat(p);
            return Ok(());
        }
        // Check for named field: IDENT `:` pattern
        if p.at(SyntaxKind::IDENT) && p.peek(1) == Some(SyntaxKind::COLON) {
            struct_pat_field(p)?;
            return Ok(());
        }
        // Otherwise parse as positional pattern
        pattern(p)?;
        Ok(())
    })?;
    p.expect(SyntaxKind::R_PAREN)?;
    Ok(())
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

    // Check if this is a range pattern with an ident start bound: `MAX..`, `MIN..MAX`
    let is_range = !is_path
        && !has_parens
        && matches!(
            p.peek(1),
            Some(SyntaxKind::DOT_DOT) | Some(SyntaxKind::DOT_DOT_EQ)
        );

    if is_range {
        // Path range pattern: ident as start bound, followed by `..` or `..=`
        let m = p.start();
        crate::path::path_no_generics(p)?;
        Ok(maybe_range_tail(p, m, SyntaxKind::IdentPat))
    } else if is_path {
        // This is a path - could be struct pattern, enum pattern, or range pattern
        path_or_struct_or_enum_range_pat(p)
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

/// Parse a path and determine if it's followed by struct fields, enum args, or range operator.
/// Used when we already know the path starts with a dot-qualified name.
fn path_or_struct_or_enum_range_pat(
    p: &mut Parser<'_>,
) -> Result<CompletedMarker, ParseError> {
    let m = p.start();

    // Use structured path parsing (no generics in patterns)
    crate::path::path_no_generics(p)?;

    // Check if followed by range operator (path as range start bound)
    if p.at(SyntaxKind::DOT_DOT) || p.at(SyntaxKind::DOT_DOT_EQ) {
        return Ok(maybe_range_tail(p, m, SyntaxKind::IdentPat));
    }

    // Check if followed by ( for struct or enum pattern
    if p.at(SyntaxKind::L_PAREN) {
        let first = p.peek(1);
        let second = p.peek(2);
        let is_struct = matches!(
            (first, second),
            (Some(SyntaxKind::IDENT), Some(SyntaxKind::COLON))
        );

        if is_struct {
            parse_struct_fields(p)?;
            Ok(m.complete(p, SyntaxKind::StructPat))
        } else {
            parse_enum_args(p)?;
            Ok(m.complete(p, SyntaxKind::TuplePat))
        }
    } else {
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

/// Parse a rest pattern: `..` or `..ident`
fn rest_pat(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // consume `..`
    // Optional binding identifier: `..rest`
    if p.at(SyntaxKind::IDENT)
        && p.current_text() != Some("_")
        && let Err(e) = crate::item::name(p)
    {
        p.error(e);
    }
    m.complete(p, SyntaxKind::RestPat)
}

/// Try to parse a range tail (`..` or `..=` plus optional end bound).
/// If present, completes `m` as `RangePat`; otherwise completes as the given `fallback` kind.
fn maybe_range_tail(
    p: &mut Parser<'_>,
    m: crate::Marker,
    fallback: SyntaxKind,
) -> CompletedMarker {
    if p.at(SyntaxKind::DOT_DOT) || p.at(SyntaxKind::DOT_DOT_EQ) {
        p.bump(); // consume `..` or `..=`

        // Optional end bound: negative literal, literal, or path
        if p.at(SyntaxKind::MINUS) {
            p.bump(); // consume `-`
        }
        if matches!(
            p.current(),
            Some(SyntaxKind::INT_LITERAL)
                | Some(SyntaxKind::FLOAT_LITERAL)
                | Some(SyntaxKind::CHAR_LITERAL)
        ) {
            p.bump(); // consume end literal
        } else if is_range_bound_path_start(p) {
            // Path as range end bound: e.g., `0..MAX`, `0..config.MAX`
            // Errors here are non-fatal — just means half-open range
            if let Err(e) = crate::path::path_no_generics(p) {
                p.error(e);
            }
        }

        m.complete(p, SyntaxKind::RangePat)
    } else {
        m.complete(p, fallback)
    }
}

/// Check if the current token can start a path used as a range bound.
fn is_range_bound_path_start(p: &mut Parser<'_>) -> bool {
    matches!(
        p.current(),
        Some(SyntaxKind::IDENT)
            | Some(SyntaxKind::DOLLAR)
            | Some(SyntaxKind::SELF_VALUE_KW)
            | Some(SyntaxKind::SUPER_KW)
            | Some(SyntaxKind::MODULE_KW)
    )
}

/// Parse a negative literal pattern: `-1`, `-3.14`, or a range starting with a negative literal.
fn negative_literal_or_range_pat(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // consume `-`
    p.bump(); // consume the literal
    maybe_range_tail(p, m, SyntaxKind::LiteralPat)
}

/// Parse a literal pattern, or a range pattern if followed by `..` or `..=`
fn literal_or_range_pat(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // consume the literal
    maybe_range_tail(p, m, SyntaxKind::LiteralPat)
}

/// Parse a tuple or grouped pattern.
///
/// Distinguishes:
/// - `()` = empty tuple
/// - `(a,)` = single-element tuple
/// - `(a, b)` = multi-element tuple
/// - `(a)` = grouped pattern (single pattern, no comma)
fn tuple_or_grouped_pat(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.bump(); // consume `(`

    if p.at(SyntaxKind::R_PAREN) {
        // Empty tuple: ()
        p.bump();
        return Ok(m.complete(p, SyntaxKind::TuplePat));
    }

    // Parse first pattern
    pattern(p)?;

    if p.at(SyntaxKind::R_PAREN) {
        // Single pattern without comma = grouped pattern
        p.bump();
        return Ok(m.complete(p, SyntaxKind::GroupedPat));
    }

    // Has comma = tuple pattern
    while p.eat(SyntaxKind::COMMA) {
        if p.at(SyntaxKind::R_PAREN) {
            break; // trailing comma
        }
        pattern(p)?;
    }
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

    // === Or-Pattern Tests ===

    #[test]
    fn or_pattern_simple() {
        // A | B in let binding
        check_expr(
            "{ let A | B = x; }",
            &expect![[r#"
                BlockExpr@0..18
                  Block@0..18
                    L_BRACE@0..1 "{"
                    LetStmt@1..16
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      OrPat@5..11
                        IdentPat@5..7
                          Name@5..7
                            WHITESPACE@5..6 " "
                            IDENT@6..7 "A"
                        WHITESPACE@7..8 " "
                        PIPE@8..9 "|"
                        IdentPat@9..11
                          Name@9..11
                            WHITESPACE@9..10 " "
                            IDENT@10..11 "B"
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
    fn or_pattern_triple() {
        // A | B | C (multiple alternatives)
        check_expr(
            "{ let A | B | C = x; }",
            &expect![[r#"
                BlockExpr@0..22
                  Block@0..22
                    L_BRACE@0..1 "{"
                    LetStmt@1..20
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      OrPat@5..15
                        IdentPat@5..7
                          Name@5..7
                            WHITESPACE@5..6 " "
                            IDENT@6..7 "A"
                        WHITESPACE@7..8 " "
                        PIPE@8..9 "|"
                        IdentPat@9..11
                          Name@9..11
                            WHITESPACE@9..10 " "
                            IDENT@10..11 "B"
                        WHITESPACE@11..12 " "
                        PIPE@12..13 "|"
                        IdentPat@13..15
                          Name@13..15
                            WHITESPACE@13..14 " "
                            IDENT@14..15 "C"
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
    fn or_pattern_in_match() {
        // Or-pattern in match arm
        check_expr(
            "match x { A | B => 1 }",
            &expect![[r#"
                MatchExpr@0..22
                  MATCH_KW@0..5 "match"
                  PathExpr@5..7
                    Path@5..7
                      PathSegment@5..7
                        NameRef@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                  WHITESPACE@7..8 " "
                  L_BRACE@8..9 "{"
                  MatchArm@9..20
                    OrPat@9..15
                      IdentPat@9..11
                        Name@9..11
                          WHITESPACE@9..10 " "
                          IDENT@10..11 "A"
                      WHITESPACE@11..12 " "
                      PIPE@12..13 "|"
                      IdentPat@13..15
                        Name@13..15
                          WHITESPACE@13..14 " "
                          IDENT@14..15 "B"
                    WHITESPACE@15..16 " "
                    FAT_ARROW@16..18 "=>"
                    LiteralExpr@18..20
                      WHITESPACE@18..19 " "
                      INT_LITERAL@19..20 "1"
                  WHITESPACE@20..21 " "
                  R_BRACE@21..22 "}"
            "#]],
        );
    }

    #[test]
    fn or_pattern_enum_shorthand() {
        // .None | .Some(_)
        check_expr(
            "match x { .None | .Some(_) => 1 }",
            &expect![[r#"
                MatchExpr@0..33
                  MATCH_KW@0..5 "match"
                  PathExpr@5..7
                    Path@5..7
                      PathSegment@5..7
                        NameRef@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "x"
                  WHITESPACE@7..8 " "
                  L_BRACE@8..9 "{"
                  MatchArm@9..31
                    OrPat@9..26
                      EnumShorthandPat@9..15
                        WHITESPACE@9..10 " "
                        DOT@10..11 "."
                        Name@11..15
                          IDENT@11..15 "None"
                      WHITESPACE@15..16 " "
                      PIPE@16..17 "|"
                      EnumShorthandPat@17..26
                        WHITESPACE@17..18 " "
                        DOT@18..19 "."
                        Name@19..23
                          IDENT@19..23 "Some"
                        L_PAREN@23..24 "("
                        WildcardPat@24..25
                          IDENT@24..25 "_"
                        R_PAREN@25..26 ")"
                    WHITESPACE@26..27 " "
                    FAT_ARROW@27..29 "=>"
                    LiteralExpr@29..31
                      WHITESPACE@29..30 " "
                      INT_LITERAL@30..31 "1"
                  WHITESPACE@31..32 " "
                  R_BRACE@32..33 "}"
            "#]],
        );
    }

    #[test]
    fn or_pattern_nested() {
        // (A | B, C | D) - or-patterns inside tuple
        check_expr(
            "{ let (A | B, C | D) = x; }",
            &expect![[r#"
                BlockExpr@0..27
                  Block@0..27
                    L_BRACE@0..1 "{"
                    LetStmt@1..25
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      TuplePat@5..20
                        WHITESPACE@5..6 " "
                        L_PAREN@6..7 "("
                        OrPat@7..12
                          IdentPat@7..8
                            Name@7..8
                              IDENT@7..8 "A"
                          WHITESPACE@8..9 " "
                          PIPE@9..10 "|"
                          IdentPat@10..12
                            Name@10..12
                              WHITESPACE@10..11 " "
                              IDENT@11..12 "B"
                        COMMA@12..13 ","
                        OrPat@13..19
                          IdentPat@13..15
                            Name@13..15
                              WHITESPACE@13..14 " "
                              IDENT@14..15 "C"
                          WHITESPACE@15..16 " "
                          PIPE@16..17 "|"
                          IdentPat@17..19
                            Name@17..19
                              WHITESPACE@17..18 " "
                              IDENT@18..19 "D"
                        R_PAREN@19..20 ")"
                      WHITESPACE@20..21 " "
                      EQ@21..22 "="
                      PathExpr@22..24
                        Path@22..24
                          PathSegment@22..24
                            NameRef@22..24
                              WHITESPACE@22..23 " "
                              IDENT@23..24 "x"
                      SEMI@24..25 ";"
                    WHITESPACE@25..26 " "
                    R_BRACE@26..27 "}"
            "#]],
        );
    }

    #[test]
    fn grouped_pattern() {
        // (pattern) for explicit grouping
        check_expr(
            "{ let (x) = y; }",
            &expect![[r#"
                BlockExpr@0..16
                  Block@0..16
                    L_BRACE@0..1 "{"
                    LetStmt@1..14
                      WHITESPACE@1..2 " "
                      LET_KW@2..5 "let"
                      GroupedPat@5..9
                        WHITESPACE@5..6 " "
                        L_PAREN@6..7 "("
                        IdentPat@7..8
                          Name@7..8
                            IDENT@7..8 "x"
                        R_PAREN@8..9 ")"
                      WHITESPACE@9..10 " "
                      EQ@10..11 "="
                      PathExpr@11..13
                        Path@11..13
                          PathSegment@11..13
                            NameRef@11..13
                              WHITESPACE@11..12 " "
                              IDENT@12..13 "y"
                      SEMI@13..14 ";"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }
}
