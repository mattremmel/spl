//! Item parser for SPL.
//!
//! Parses top-level items: functions, structs, type aliases, and impl blocks.

// Functions are called from tests; will be used by PARSE-8 module parser
#![allow(dead_code)]

use crate::parser::{CompletedMarker, Parser};
use crate::syntax::SyntaxKind;

use super::expr;
use super::stmt;

/// Parse a function definition: `[pub] fn name[<generics>](params) [-> Type] { body }`
pub(crate) fn function_def(
    p: &mut Parser<'_>,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional visibility
    p.eat(SyntaxKind::PUB_KW);

    // fn keyword
    p.expect(SyntaxKind::FN_KW)?;

    // Function name
    name(p)?;

    // Optional generic parameters
    if p.at(SyntaxKind::LT) {
        generic_params(p)?;
    }

    // Parameter list
    param_list(p)?;

    // Optional return type
    if p.eat(SyntaxKind::ARROW) {
        stmt::type_annotation(p)?;
    }

    // Function body
    expr::block(p)?;

    Ok(m.complete(p, SyntaxKind::FunctionDef))
}

/// Parse a name (identifier).
fn name(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    if !p.at(SyntaxKind::IDENT) {
        let err = p.error_at_current("expected identifier".to_string());
        m.abandon(p);
        return Err(err);
    }
    p.bump();
    Ok(m.complete(p, SyntaxKind::Name))
}

/// Parse a parameter list: `(self_param?, params...)`
fn param_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;

    // Check for self parameter first
    if is_self_param_start(p) {
        self_param(p)?;
        if !p.at(SyntaxKind::R_PAREN) {
            p.expect(SyntaxKind::COMMA)?;
        }
    }

    // Regular parameters
    while !p.at(SyntaxKind::R_PAREN) && p.current().is_some() {
        param(p)?;
        if !p.at(SyntaxKind::R_PAREN) {
            p.expect(SyntaxKind::COMMA)?;
        }
    }

    p.expect(SyntaxKind::R_PAREN)?;
    Ok(m.complete(p, SyntaxKind::ParamList))
}

/// Check if we're at the start of a self parameter.
fn is_self_param_start(p: &mut Parser<'_>) -> bool {
    p.at(SyntaxKind::SELF_VALUE_KW)
        || (p.at(SyntaxKind::AMP) && p.peek_at(1, SyntaxKind::SELF_VALUE_KW))
        || (p.at(SyntaxKind::AMP) && p.peek_at(1, SyntaxKind::MUT_KW))
}

/// Parse a self parameter: `self`, `&self`, or `&mut self`
fn self_param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional & and mut
    if p.eat(SyntaxKind::AMP) {
        p.eat(SyntaxKind::MUT_KW);
    }

    p.expect(SyntaxKind::SELF_VALUE_KW)?;
    Ok(m.complete(p, SyntaxKind::SelfParam))
}

/// Parse a regular parameter: `name: Type`
fn param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Parameter name
    name(p)?;

    // Type annotation
    p.expect(SyntaxKind::COLON)?;
    stmt::type_annotation(p)?;

    Ok(m.complete(p, SyntaxKind::Param))
}

/// Parse generic parameters: `<T, U, ...>`
fn generic_params(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::LT)?;

    while !p.at(SyntaxKind::GT) && p.current().is_some() {
        generic_param(p)?;
        if !p.at(SyntaxKind::GT) {
            p.expect(SyntaxKind::COMMA)?;
        }
    }

    p.expect(SyntaxKind::GT)?;
    Ok(m.complete(p, SyntaxKind::GenericParams))
}

/// Parse a single generic parameter: `T`
fn generic_param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    name(p)?;
    Ok(m.complete(p, SyntaxKind::GenericParam))
}

#[cfg(test)]
mod tests {
    use crate::parser::tests::check_item;
    use expect_test::expect;

    #[test]
    fn function_minimal() {
        check_item(
            "fn foo() {}",
            &expect![[r#"
                FunctionDef@0..11
                  FN_KW@0..2 "fn"
                  Name@2..6
                    WHITESPACE@2..3 " "
                    IDENT@3..6 "foo"
                  ParamList@6..8
                    L_PAREN@6..7 "("
                    R_PAREN@7..8 ")"
                  Block@8..11
                    WHITESPACE@8..9 " "
                    L_BRACE@9..10 "{"
                    R_BRACE@10..11 "}"
            "#]],
        );
    }

    #[test]
    fn function_with_return_type() {
        check_item(
            "fn foo() -> i32 {}",
            &expect![[r#"
                FunctionDef@0..18
                  FN_KW@0..2 "fn"
                  Name@2..6
                    WHITESPACE@2..3 " "
                    IDENT@3..6 "foo"
                  ParamList@6..8
                    L_PAREN@6..7 "("
                    R_PAREN@7..8 ")"
                  WHITESPACE@8..9 " "
                  ARROW@9..11 "->"
                  PathType@11..15
                    WHITESPACE@11..12 " "
                    IDENT@12..15 "i32"
                  Block@15..18
                    WHITESPACE@15..16 " "
                    L_BRACE@16..17 "{"
                    R_BRACE@17..18 "}"
            "#]],
        );
    }

    #[test]
    fn function_with_params() {
        check_item(
            "fn add(x: i32, y: i32) -> i32 {}",
            &expect![[r#"
                FunctionDef@0..32
                  FN_KW@0..2 "fn"
                  Name@2..6
                    WHITESPACE@2..3 " "
                    IDENT@3..6 "add"
                  ParamList@6..22
                    L_PAREN@6..7 "("
                    Param@7..13
                      Name@7..8
                        IDENT@7..8 "x"
                      COLON@8..9 ":"
                      PathType@9..13
                        WHITESPACE@9..10 " "
                        IDENT@10..13 "i32"
                    COMMA@13..14 ","
                    Param@14..21
                      Name@14..16
                        WHITESPACE@14..15 " "
                        IDENT@15..16 "y"
                      COLON@16..17 ":"
                      PathType@17..21
                        WHITESPACE@17..18 " "
                        IDENT@18..21 "i32"
                    R_PAREN@21..22 ")"
                  WHITESPACE@22..23 " "
                  ARROW@23..25 "->"
                  PathType@25..29
                    WHITESPACE@25..26 " "
                    IDENT@26..29 "i32"
                  Block@29..32
                    WHITESPACE@29..30 " "
                    L_BRACE@30..31 "{"
                    R_BRACE@31..32 "}"
            "#]],
        );
    }

    #[test]
    fn function_with_body() {
        check_item(
            "fn answer() -> i32 { 42 }",
            &expect![[r#"
                FunctionDef@0..25
                  FN_KW@0..2 "fn"
                  Name@2..9
                    WHITESPACE@2..3 " "
                    IDENT@3..9 "answer"
                  ParamList@9..11
                    L_PAREN@9..10 "("
                    R_PAREN@10..11 ")"
                  WHITESPACE@11..12 " "
                  ARROW@12..14 "->"
                  PathType@14..18
                    WHITESPACE@14..15 " "
                    IDENT@15..18 "i32"
                  Block@18..25
                    WHITESPACE@18..19 " "
                    L_BRACE@19..20 "{"
                    LiteralExpr@20..23
                      WHITESPACE@20..21 " "
                      INT_LITERAL@21..23 "42"
                    WHITESPACE@23..24 " "
                    R_BRACE@24..25 "}"
            "#]],
        );
    }

    #[test]
    fn function_pub() {
        check_item(
            "pub fn foo() {}",
            &expect![[r#"
                FunctionDef@0..15
                  PUB_KW@0..3 "pub"
                  WHITESPACE@3..4 " "
                  FN_KW@4..6 "fn"
                  Name@6..10
                    WHITESPACE@6..7 " "
                    IDENT@7..10 "foo"
                  ParamList@10..12
                    L_PAREN@10..11 "("
                    R_PAREN@11..12 ")"
                  Block@12..15
                    WHITESPACE@12..13 " "
                    L_BRACE@13..14 "{"
                    R_BRACE@14..15 "}"
            "#]],
        );
    }

    #[test]
    fn function_with_self() {
        check_item(
            "fn method(&self) {}",
            &expect![[r#"
                FunctionDef@0..19
                  FN_KW@0..2 "fn"
                  Name@2..9
                    WHITESPACE@2..3 " "
                    IDENT@3..9 "method"
                  ParamList@9..16
                    L_PAREN@9..10 "("
                    SelfParam@10..15
                      AMP@10..11 "&"
                      SELF_VALUE_KW@11..15 "self"
                    R_PAREN@15..16 ")"
                  Block@16..19
                    WHITESPACE@16..17 " "
                    L_BRACE@17..18 "{"
                    R_BRACE@18..19 "}"
            "#]],
        );
    }

    #[test]
    fn function_with_mut_self() {
        check_item(
            "fn method(&mut self) {}",
            &expect![[r#"
                FunctionDef@0..23
                  FN_KW@0..2 "fn"
                  Name@2..9
                    WHITESPACE@2..3 " "
                    IDENT@3..9 "method"
                  ParamList@9..20
                    L_PAREN@9..10 "("
                    SelfParam@10..19
                      AMP@10..11 "&"
                      MUT_KW@11..14 "mut"
                      WHITESPACE@14..15 " "
                      SELF_VALUE_KW@15..19 "self"
                    R_PAREN@19..20 ")"
                  Block@20..23
                    WHITESPACE@20..21 " "
                    L_BRACE@21..22 "{"
                    R_BRACE@22..23 "}"
            "#]],
        );
    }

    #[test]
    fn function_with_self_and_params() {
        check_item(
            "fn method(&self, x: i32) {}",
            &expect![[r#"
                FunctionDef@0..27
                  FN_KW@0..2 "fn"
                  Name@2..9
                    WHITESPACE@2..3 " "
                    IDENT@3..9 "method"
                  ParamList@9..24
                    L_PAREN@9..10 "("
                    SelfParam@10..15
                      AMP@10..11 "&"
                      SELF_VALUE_KW@11..15 "self"
                    COMMA@15..16 ","
                    Param@16..23
                      Name@16..18
                        WHITESPACE@16..17 " "
                        IDENT@17..18 "x"
                      COLON@18..19 ":"
                      PathType@19..23
                        WHITESPACE@19..20 " "
                        IDENT@20..23 "i32"
                    R_PAREN@23..24 ")"
                  Block@24..27
                    WHITESPACE@24..25 " "
                    L_BRACE@25..26 "{"
                    R_BRACE@26..27 "}"
            "#]],
        );
    }

    #[test]
    fn function_with_generics() {
        check_item(
            "fn identity<T>(x: T) -> T {}",
            &expect![[r#"
                FunctionDef@0..28
                  FN_KW@0..2 "fn"
                  Name@2..11
                    WHITESPACE@2..3 " "
                    IDENT@3..11 "identity"
                  GenericParams@11..14
                    LT@11..12 "<"
                    GenericParam@12..13
                      Name@12..13
                        IDENT@12..13 "T"
                    GT@13..14 ">"
                  ParamList@14..20
                    L_PAREN@14..15 "("
                    Param@15..19
                      Name@15..16
                        IDENT@15..16 "x"
                      COLON@16..17 ":"
                      PathType@17..19
                        WHITESPACE@17..18 " "
                        IDENT@18..19 "T"
                    R_PAREN@19..20 ")"
                  WHITESPACE@20..21 " "
                  ARROW@21..23 "->"
                  PathType@23..25
                    WHITESPACE@23..24 " "
                    IDENT@24..25 "T"
                  Block@25..28
                    WHITESPACE@25..26 " "
                    L_BRACE@26..27 "{"
                    R_BRACE@27..28 "}"
            "#]],
        );
    }

    #[test]
    fn function_with_multiple_generics() {
        check_item(
            "fn pair<T, U>(a: T, b: U) {}",
            &expect![[r#"
                FunctionDef@0..28
                  FN_KW@0..2 "fn"
                  Name@2..7
                    WHITESPACE@2..3 " "
                    IDENT@3..7 "pair"
                  GenericParams@7..13
                    LT@7..8 "<"
                    GenericParam@8..9
                      Name@8..9
                        IDENT@8..9 "T"
                    COMMA@9..10 ","
                    GenericParam@10..12
                      Name@10..12
                        WHITESPACE@10..11 " "
                        IDENT@11..12 "U"
                    GT@12..13 ">"
                  ParamList@13..25
                    L_PAREN@13..14 "("
                    Param@14..18
                      Name@14..15
                        IDENT@14..15 "a"
                      COLON@15..16 ":"
                      PathType@16..18
                        WHITESPACE@16..17 " "
                        IDENT@17..18 "T"
                    COMMA@18..19 ","
                    Param@19..24
                      Name@19..21
                        WHITESPACE@19..20 " "
                        IDENT@20..21 "b"
                      COLON@21..22 ":"
                      PathType@22..24
                        WHITESPACE@22..23 " "
                        IDENT@23..24 "U"
                    R_PAREN@24..25 ")"
                  Block@25..28
                    WHITESPACE@25..26 " "
                    L_BRACE@26..27 "{"
                    R_BRACE@27..28 "}"
            "#]],
        );
    }

    #[test]
    fn function_owned_self() {
        check_item(
            "fn consume(self) {}",
            &expect![[r#"
                FunctionDef@0..19
                  FN_KW@0..2 "fn"
                  Name@2..10
                    WHITESPACE@2..3 " "
                    IDENT@3..10 "consume"
                  ParamList@10..16
                    L_PAREN@10..11 "("
                    SelfParam@11..15
                      SELF_VALUE_KW@11..15 "self"
                    R_PAREN@15..16 ")"
                  Block@16..19
                    WHITESPACE@16..17 " "
                    L_BRACE@17..18 "{"
                    R_BRACE@18..19 "}"
            "#]],
        );
    }
}
