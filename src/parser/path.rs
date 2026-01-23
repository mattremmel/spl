//! Path parsing: `segment (:: segment)*`
//!
//! Produces structured Path nodes with PathSegment and NameRef children.

use crate::parser::{CompletedMarker, ParseError, Parser};
use crate::syntax::SyntaxKind;

/// Parse a path with optional generic arguments: `ident (:: ident)* [<T, ...>]`
///
/// Used for type annotations where generic args are allowed.
pub fn path(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    path_segment(p, true)?;
    while p.at(SyntaxKind::COLON_COLON) {
        p.bump();
        path_segment(p, true)?;
    }
    Ok(m.complete(p, SyntaxKind::Path))
}

/// Parse a path without generic arguments: `ident (:: ident)*`
///
/// Used for expressions and patterns where generics are handled separately.
pub fn path_no_generics(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    path_segment(p, false)?;
    while p.at(SyntaxKind::COLON_COLON) {
        p.bump();
        path_segment(p, false)?;
    }
    Ok(m.complete(p, SyntaxKind::Path))
}

/// Parse a single path segment: `ident [<T, ...>]` or `ident [(T, ...)]`
fn path_segment(p: &mut Parser<'_>, allow_generics: bool) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    name_ref(p)?;
    if allow_generics {
        if p.at(SyntaxKind::LT) {
            // Old syntax: <T, U>
            super::stmt::generic_args(p)?;
        } else if p.at(SyntaxKind::L_PAREN) {
            // New syntax: (T, U)
            generic_args_paren(p)?;
        }
    }
    Ok(m.complete(p, SyntaxKind::PathSegment))
}

/// Parse generic arguments with parentheses: `(T, U, ...)`
fn generic_args_paren(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;

    if !p.at(SyntaxKind::R_PAREN) {
        super::stmt::type_annotation(p)?;
        while p.eat(SyntaxKind::COMMA) {
            if p.at(SyntaxKind::R_PAREN) {
                break;
            }
            super::stmt::type_annotation(p)?;
        }
    }

    p.expect(SyntaxKind::R_PAREN)?;
    Ok(m.complete(p, SyntaxKind::GenericArgs))
}

/// Parse a name reference (identifier, self, Self, crate, or super).
pub(crate) fn name_ref(p: &mut Parser<'_>) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    if p.at(SyntaxKind::IDENT)
        || p.at(SyntaxKind::SELF_VALUE_KW)
        || p.at(SyntaxKind::SELF_TYPE_KW)
        || p.at(SyntaxKind::CRATE_KW)
        || p.at(SyntaxKind::SUPER_KW)
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
            "foo::bar::baz",
            &expect![[r#"
                PathExpr@0..13
                  Path@0..13
                    PathSegment@0..3
                      NameRef@0..3
                        IDENT@0..3 "foo"
                    COLON_COLON@3..5 "::"
                    PathSegment@5..8
                      NameRef@5..8
                        IDENT@5..8 "bar"
                    COLON_COLON@8..10 "::"
                    PathSegment@10..13
                      NameRef@10..13
                        IDENT@10..13 "baz"
            "#]],
        );
    }

    // Note: crate:: and super:: path prefixes are not currently supported
    // in expression context by this parser (only in type context).

    #[test]
    fn path_self_value() {
        check_expr(
            "self::item",
            &expect![[r#"
                PathExpr@0..10
                  Path@0..10
                    PathSegment@0..4
                      NameRef@0..4
                        SELF_VALUE_KW@0..4 "self"
                    COLON_COLON@4..6 "::"
                    PathSegment@6..10
                      NameRef@6..10
                        IDENT@6..10 "item"
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
            "fn foo(x: Vec<i32>) {}",
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
                              LT@13..14 "<"
                              PathType@14..17
                                Path@14..17
                                  PathSegment@14..17
                                    NameRef@14..17
                                      IDENT@14..17 "i32"
                              GT@17..18 ">"
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
            "fn foo(x: Result<Vec<i32>, Error>) {}",
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
                              LT@16..17 "<"
                              PathType@17..25
                                Path@17..25
                                  PathSegment@17..25
                                    NameRef@17..20
                                      IDENT@17..20 "Vec"
                                    GenericArgs@20..25
                                      LT@20..21 "<"
                                      PathType@21..24
                                        Path@21..24
                                          PathSegment@21..24
                                            NameRef@21..24
                                              IDENT@21..24 "i32"
                                      GT@24..25 ">"
                              COMMA@25..26 ","
                              PathType@26..32
                                Path@26..32
                                  PathSegment@26..32
                                    NameRef@26..32
                                      WHITESPACE@26..27 " "
                                      IDENT@27..32 "Error"
                              GT@32..33 ">"
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
        // Associated function call like Vec::new()
        check_expr(
            "Vec::new()",
            &expect![[r#"
                CallExpr@0..10
                  PathExpr@0..8
                    Path@0..8
                      PathSegment@0..3
                        NameRef@0..3
                          IDENT@0..3 "Vec"
                      COLON_COLON@3..5 "::"
                      PathSegment@5..8
                        NameRef@5..8
                          IDENT@5..8 "new"
                  ArgList@8..10
                    L_PAREN@8..9 "("
                    R_PAREN@9..10 ")"
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
            "a::b::c::d::e::f",
            &expect![[r#"
                PathExpr@0..16
                  Path@0..16
                    PathSegment@0..1
                      NameRef@0..1
                        IDENT@0..1 "a"
                    COLON_COLON@1..3 "::"
                    PathSegment@3..4
                      NameRef@3..4
                        IDENT@3..4 "b"
                    COLON_COLON@4..6 "::"
                    PathSegment@6..7
                      NameRef@6..7
                        IDENT@6..7 "c"
                    COLON_COLON@7..9 "::"
                    PathSegment@9..10
                      NameRef@9..10
                        IDENT@9..10 "d"
                    COLON_COLON@10..12 "::"
                    PathSegment@12..13
                      NameRef@12..13
                        IDENT@12..13 "e"
                    COLON_COLON@13..15 "::"
                    PathSegment@15..16
                      NameRef@15..16
                        IDENT@15..16 "f"
            "#]],
        );
    }
}
