//! Path parsing: `segment (. segment)*`
//!
//! Produces structured Path nodes with PathSegment and NameRef children.

use crate::parser::{CompletedMarker, ParseError, Parser};
use crate::syntax::SyntaxKind;

/// Parse a path with optional generic arguments: `ident (. ident)* [(T, ...)]`
///
/// Used for type annotations where generic args are allowed.
pub fn path(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    path_segment(p, true)?;
    while p.at(SyntaxKind::DOT) && is_path_segment_start(p.peek(1)) {
        p.bump();
        path_segment(p, true)?;
    }
    Ok(m.complete(p, SyntaxKind::Path))
}

/// Parse a path without generic arguments: `ident (. ident)*`
///
/// Used for expressions and patterns where generics are handled separately.
pub fn path_no_generics(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    path_segment(p, false)?;
    while p.at(SyntaxKind::DOT) && is_path_segment_start(p.peek(1)) {
        p.bump();
        path_segment(p, false)?;
    }
    Ok(m.complete(p, SyntaxKind::Path))
}

/// Check if a token can start a path segment.
fn is_path_segment_start(token: Option<SyntaxKind>) -> bool {
    matches!(
        token,
        Some(SyntaxKind::IDENT)
            | Some(SyntaxKind::SELF_VALUE_KW)
            | Some(SyntaxKind::SELF_TYPE_KW)
            | Some(SyntaxKind::CRATE_KW)
            | Some(SyntaxKind::SUPER_KW)
            | Some(SyntaxKind::MODULE_KW)
    )
}

/// Parse a single path segment: `ident [(T, ...)]`
fn path_segment(p: &mut Parser<'_>, allow_generics: bool) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    name_ref(p)?;
    if allow_generics && p.at(SyntaxKind::L_PAREN) {
        generic_args_paren(p)?;
    }
    Ok(m.complete(p, SyntaxKind::PathSegment))
}

/// Parse generic arguments with parentheses: `(T, U, ...)`
fn generic_args_paren(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;
    p.parse_delimited(SyntaxKind::R_PAREN, |p| {
        super::stmt::type_annotation(p)?;
        Ok(())
    })?;
    p.expect(SyntaxKind::R_PAREN)?;
    Ok(m.complete(p, SyntaxKind::GenericArgs))
}

/// Parse a name reference (identifier, self, Self, crate, super, or module).
pub(crate) fn name_ref(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    if p.at(SyntaxKind::IDENT)
        || p.at(SyntaxKind::SELF_VALUE_KW)
        || p.at(SyntaxKind::SELF_TYPE_KW)
        || p.at(SyntaxKind::CRATE_KW)
        || p.at(SyntaxKind::SUPER_KW)
        || p.at(SyntaxKind::MODULE_KW)
    {
        p.bump();
        Ok(m.complete(p, SyntaxKind::NameRef))
    } else {
        m.abandon(p);
        Err(p.error_at_current("expected identifier".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::tests::{check_expr, check_item};
    use expect_test::expect;

    // === Simple Paths ===

    #[test]
    fn path_simple() {
        check_expr(
            "foo",
            &expect![[r#"
                PathExpr@0..3
                  Path@0..3
                    PathSegment@0..3
                      NameRef@0..3
                        IDENT@0..3 "foo"
            "#]],
        );
    }

    #[test]
    fn path_qualified() {
        check_expr(
            "foo.bar.baz",
            &expect![[r#"
                PathExpr@0..11
                  Path@0..11
                    PathSegment@0..3
                      NameRef@0..3
                        IDENT@0..3 "foo"
                    DOT@3..4 "."
                    PathSegment@4..7
                      NameRef@4..7
                        IDENT@4..7 "bar"
                    DOT@7..8 "."
                    PathSegment@8..11
                      NameRef@8..11
                        IDENT@8..11 "baz"
            "#]],
        );
    }

    // Note: crate:: and super:: path prefixes are not currently supported
    // in expression context by this parser (only in type context).

    #[test]
    fn path_self_value() {
        check_expr(
            "self.item",
            &expect![[r#"
                PathExpr@0..9
                  Path@0..9
                    PathSegment@0..4
                      NameRef@0..4
                        SELF_VALUE_KW@0..4 "self"
                    DOT@4..5 "."
                    PathSegment@5..9
                      NameRef@5..9
                        IDENT@5..9 "item"
            "#]],
        );
    }

    #[test]
    fn path_self_type_colon() {
        // Self as a type in type position
        check_item(
            "fn foo(): Self {}",
            &expect![[r#"
                FunctionDef@0..17
                  FN_KW@0..2 "fn"
                  Name@2..6
                    WHITESPACE@2..3 " "
                    IDENT@3..6 "foo"
                  ParamList@6..8
                    L_PAREN@6..7 "("
                    R_PAREN@7..8 ")"
                  COLON@8..9 ":"
                  PathType@9..14
                    Path@9..14
                      PathSegment@9..14
                        NameRef@9..14
                          WHITESPACE@9..10 " "
                          SELF_TYPE_KW@10..14 "Self"
                  Block@14..17
                    WHITESPACE@14..15 " "
                    L_BRACE@15..16 "{"
                    R_BRACE@16..17 "}"
            "#]],
        );
    }

    // === Paths with Generics ===

    #[test]
    fn path_with_generics() {
        check_item(
            "fn foo(x: Vec(i32)) {}",
            &expect![[r#"
                FunctionDef@0..22
                  FN_KW@0..2 "fn"
                  Name@2..6
                    WHITESPACE@2..3 " "
                    IDENT@3..6 "foo"
                  ParamList@6..19
                    L_PAREN@6..7 "("
                    Param@7..18
                      Name@7..8
                        IDENT@7..8 "x"
                      COLON@8..9 ":"
                      PathType@9..18
                        Path@9..18
                          PathSegment@9..18
                            NameRef@9..13
                              WHITESPACE@9..10 " "
                              IDENT@10..13 "Vec"
                            GenericArgs@13..18
                              L_PAREN@13..14 "("
                              PathType@14..17
                                Path@14..17
                                  PathSegment@14..17
                                    NameRef@14..17
                                      IDENT@14..17 "i32"
                              R_PAREN@17..18 ")"
                    R_PAREN@18..19 ")"
                  Block@19..22
                    WHITESPACE@19..20 " "
                    L_BRACE@20..21 "{"
                    R_BRACE@21..22 "}"
            "#]],
        );
    }

    #[test]
    fn path_nested_generics() {
        check_item(
            "fn foo(x: Result(Vec(i32), Error)) {}",
            &expect![[r#"
                FunctionDef@0..37
                  FN_KW@0..2 "fn"
                  Name@2..6
                    WHITESPACE@2..3 " "
                    IDENT@3..6 "foo"
                  ParamList@6..34
                    L_PAREN@6..7 "("
                    Param@7..33
                      Name@7..8
                        IDENT@7..8 "x"
                      COLON@8..9 ":"
                      PathType@9..33
                        Path@9..33
                          PathSegment@9..33
                            NameRef@9..16
                              WHITESPACE@9..10 " "
                              IDENT@10..16 "Result"
                            GenericArgs@16..33
                              L_PAREN@16..17 "("
                              PathType@17..25
                                Path@17..25
                                  PathSegment@17..25
                                    NameRef@17..20
                                      IDENT@17..20 "Vec"
                                    GenericArgs@20..25
                                      L_PAREN@20..21 "("
                                      PathType@21..24
                                        Path@21..24
                                          PathSegment@21..24
                                            NameRef@21..24
                                              IDENT@21..24 "i32"
                                      R_PAREN@24..25 ")"
                              COMMA@25..26 ","
                              PathType@26..32
                                Path@26..32
                                  PathSegment@26..32
                                    NameRef@26..32
                                      WHITESPACE@26..27 " "
                                      IDENT@27..32 "Error"
                              R_PAREN@32..33 ")"
                    R_PAREN@33..34 ")"
                  Block@34..37
                    WHITESPACE@34..35 " "
                    L_BRACE@35..36 "{"
                    R_BRACE@36..37 "}"
            "#]],
        );
    }

    #[test]
    fn path_associated_function() {
        // Associated function call like Vec.new()
        check_expr(
            "Vec.new()",
            &expect![[r#"
                ApplyExpr@0..9
                  Path@0..7
                    PathSegment@0..3
                      NameRef@0..3
                        IDENT@0..3 "Vec"
                    DOT@3..4 "."
                    PathSegment@4..7
                      NameRef@4..7
                        IDENT@4..7 "new"
                  L_PAREN@7..8 "("
                  R_PAREN@8..9 ")"
            "#]],
        );
    }

    // === Name Reference Tests ===

    #[test]
    fn name_ref_simple() {
        check_expr(
            "identifier",
            &expect![[r#"
                PathExpr@0..10
                  Path@0..10
                    PathSegment@0..10
                      NameRef@0..10
                        IDENT@0..10 "identifier"
            "#]],
        );
    }

    #[test]
    fn name_ref_underscore_prefix() {
        check_expr(
            "_unused",
            &expect![[r#"
                PathExpr@0..7
                  Path@0..7
                    PathSegment@0..7
                      NameRef@0..7
                        IDENT@0..7 "_unused"
            "#]],
        );
    }

    #[test]
    fn name_ref_with_numbers() {
        check_expr(
            "var123",
            &expect![[r#"
                PathExpr@0..6
                  Path@0..6
                    PathSegment@0..6
                      NameRef@0..6
                        IDENT@0..6 "var123"
            "#]],
        );
    }

    // === Deep Path Tests ===

    #[test]
    fn path_deeply_qualified() {
        check_expr(
            "a.b.c.d.e.f",
            &expect![[r#"
                PathExpr@0..11
                  Path@0..11
                    PathSegment@0..1
                      NameRef@0..1
                        IDENT@0..1 "a"
                    DOT@1..2 "."
                    PathSegment@2..3
                      NameRef@2..3
                        IDENT@2..3 "b"
                    DOT@3..4 "."
                    PathSegment@4..5
                      NameRef@4..5
                        IDENT@4..5 "c"
                    DOT@5..6 "."
                    PathSegment@6..7
                      NameRef@6..7
                        IDENT@6..7 "d"
                    DOT@7..8 "."
                    PathSegment@8..9
                      NameRef@8..9
                        IDENT@8..9 "e"
                    DOT@9..10 "."
                    PathSegment@10..11
                      NameRef@10..11
                        IDENT@10..11 "f"
            "#]],
        );
    }
}
