//! Item parser for SPL.
//!
//! Parses top-level items: functions, structs, type aliases, and impl blocks.

// Functions are called from tests; will be used by PARSE-8 module parser
#![allow(dead_code)]

use crate::parser::{CompletedMarker, Parser};
use crate::syntax::SyntaxKind;

use super::expr;
use super::stmt;

/// Parse optional visibility: `pub`, `pub(crate)`, `pub(super)`, `pub(self)`, `pub(in path)`
fn opt_visibility(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if !p.at(SyntaxKind::PUB_KW) {
        return None;
    }

    let m = p.start();
    p.bump(); // pub

    if p.at(SyntaxKind::L_PAREN) {
        p.bump(); // (

        if p.at(SyntaxKind::CRATE_KW)
            || p.at(SyntaxKind::SUPER_KW)
            || p.at(SyntaxKind::SELF_VALUE_KW)
        {
            p.bump();
        } else if p.at(SyntaxKind::IN_KW) {
            p.bump(); // in
            // Parse path, ignoring errors (continue to closing paren for recovery)
            let _ = crate::parser::path::path_no_generics(p);
        }

        p.expect(SyntaxKind::R_PAREN).ok();
    }

    Some(m.complete(p, SyntaxKind::Visibility))
}

/// Parse a function definition: `[pub] fn name[<generics>](params) [-> Type] { body }`
pub(crate) fn function_def(
    p: &mut Parser<'_>,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional visibility
    opt_visibility(p);

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

/// Recovery set for parameter list (param start or list end).
const PARAM_RECOVERY_SET: &[SyntaxKind] = &[
    SyntaxKind::IDENT,
    SyntaxKind::SELF_VALUE_KW,
    SyntaxKind::AMP,
    SyntaxKind::COMMA,
    SyntaxKind::R_PAREN,
];

/// Parse a parameter list: `(self_param?, params...)`
fn param_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;

    // Check for self parameter first
    if is_self_param_start(p) {
        if let Err(err) = self_param(p) {
            p.recover_with_error(err, PARAM_RECOVERY_SET);
        }
        if !p.at(SyntaxKind::R_PAREN) && !p.eat(SyntaxKind::COMMA) {
            let err = p.error_at_current("expected ',' after self parameter".to_string());
            p.error(err);
        }
    }

    // Regular parameters
    while !p.at(SyntaxKind::R_PAREN) && p.current().is_some() {
        if let Err(err) = param(p) {
            p.recover_with_error(err, PARAM_RECOVERY_SET);
        }
        if !p.at(SyntaxKind::R_PAREN) && !p.eat(SyntaxKind::COMMA) {
            let err = p.error_at_current("expected ',' after parameter".to_string());
            p.error(err);
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

/// Parse a struct definition: `[pub] struct Name[<generics>] { fields }`
pub(crate) fn struct_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional visibility
    opt_visibility(p);

    // struct keyword
    p.expect(SyntaxKind::STRUCT_KW)?;

    // Struct name
    name(p)?;

    // Optional generic parameters
    if p.at(SyntaxKind::LT) {
        generic_params(p)?;
    }

    // Field list
    field_list(p)?;

    Ok(m.complete(p, SyntaxKind::StructDef))
}

/// Recovery set for field list (field start or list end).
const FIELD_RECOVERY_SET: &[SyntaxKind] = &[
    SyntaxKind::IDENT,
    SyntaxKind::PUB_KW,
    SyntaxKind::COMMA,
    SyntaxKind::R_BRACE,
];

/// Parse a field list: `{ [pub] name: Type, ... }`
fn field_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_BRACE)?;

    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        if let Err(err) = field_def(p) {
            // Recover to next field or end of list
            p.recover_with_error(err, FIELD_RECOVERY_SET);
        }
        // Allow trailing comma
        if !p.eat(SyntaxKind::COMMA) && !p.at(SyntaxKind::R_BRACE) {
            let err = p.error_at_current("expected ',' or '}'".to_string());
            p.error(err);
        }
    }

    p.expect(SyntaxKind::R_BRACE)?;
    Ok(m.complete(p, SyntaxKind::FieldList))
}

/// Parse a field definition: `[pub] name: Type`
fn field_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional visibility
    opt_visibility(p);

    // Field name
    name(p)?;

    // Type annotation
    p.expect(SyntaxKind::COLON)?;
    stmt::type_annotation(p)?;

    Ok(m.complete(p, SyntaxKind::FieldDef))
}

/// Parse a type alias: `[pub] type Name = Type;`
pub(crate) fn type_alias(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional visibility
    opt_visibility(p);

    // type keyword
    p.expect(SyntaxKind::TYPE_KW)?;

    // Alias name
    name(p)?;

    // = Type
    p.expect(SyntaxKind::EQ)?;
    stmt::type_annotation(p)?;

    // Semicolon
    p.expect(SyntaxKind::SEMI)?;

    Ok(m.complete(p, SyntaxKind::TypeAlias))
}

/// Recovery set for impl block contents (functions only).
const IMPL_ITEM_RECOVERY_SET: &[SyntaxKind] =
    &[SyntaxKind::FN_KW, SyntaxKind::PUB_KW, SyntaxKind::R_BRACE];

/// Parse an impl block: `impl [<generics>] Type { items }`
pub(crate) fn impl_block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // impl keyword
    p.expect(SyntaxKind::IMPL_KW)?;

    // Optional generic parameters
    if p.at(SyntaxKind::LT) {
        generic_params(p)?;
    }

    // Self type
    stmt::type_annotation(p)?;

    // Items block
    p.expect(SyntaxKind::L_BRACE)?;

    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        if let Err(err) = item(p) {
            // Recover to next item in impl block
            p.recover_with_error(err, IMPL_ITEM_RECOVERY_SET);
        }
    }

    p.expect(SyntaxKind::R_BRACE)?;

    Ok(m.complete(p, SyntaxKind::ImplBlock))
}

/// Calculate lookahead to skip past visibility modifier.
/// Returns the offset after visibility where the item keyword should be.
fn visibility_lookahead(p: &mut Parser<'_>) -> usize {
    if !p.at(SyntaxKind::PUB_KW) {
        return 0;
    }
    // Check for pub(...)
    if p.peek_at(1, SyntaxKind::L_PAREN) {
        // Find the matching R_PAREN
        let mut depth = 1;
        let mut offset = 2;
        while depth > 0 {
            match p.peek(offset) {
                Some(SyntaxKind::L_PAREN) => depth += 1,
                Some(SyntaxKind::R_PAREN) => depth -= 1,
                None => break,
                _ => {}
            }
            offset += 1;
        }
        offset
    } else {
        // Just "pub"
        1
    }
}

/// Parse a top-level item (function, struct, type alias, or impl block).
pub(crate) fn item(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    // Check for visibility modifier and calculate lookahead
    let has_pub = p.at(SyntaxKind::PUB_KW);
    let lookahead = visibility_lookahead(p);

    match p.peek(lookahead) {
        Some(SyntaxKind::FN_KW) => function_def(p),
        Some(SyntaxKind::STRUCT_KW) => struct_def(p),
        Some(SyntaxKind::TYPE_KW) => type_alias(p),
        Some(SyntaxKind::IMPL_KW) if !has_pub => impl_block(p),
        _ => {
            let err = p.error_at_current("expected item (fn, struct, type, or impl)".to_string());
            Err(err)
        }
    }
}

/// Parse a source file (sequence of items).
pub(crate) fn source_file(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();

    while p.current().is_some() {
        if let Err(err) = item(p) {
            // Recover to next item boundary, wrapping error in ERROR node
            p.recover_to_item(err);
        }
    }

    m.complete(p, SyntaxKind::SourceFile)
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
                    Path@11..15
                      PathSegment@11..15
                        NameRef@11..15
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
                        Path@9..13
                          PathSegment@9..13
                            NameRef@9..13
                              WHITESPACE@9..10 " "
                              IDENT@10..13 "i32"
                    COMMA@13..14 ","
                    Param@14..21
                      Name@14..16
                        WHITESPACE@14..15 " "
                        IDENT@15..16 "y"
                      COLON@16..17 ":"
                      PathType@17..21
                        Path@17..21
                          PathSegment@17..21
                            NameRef@17..21
                              WHITESPACE@17..18 " "
                              IDENT@18..21 "i32"
                    R_PAREN@21..22 ")"
                  WHITESPACE@22..23 " "
                  ARROW@23..25 "->"
                  PathType@25..29
                    Path@25..29
                      PathSegment@25..29
                        NameRef@25..29
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
                    Path@14..18
                      PathSegment@14..18
                        NameRef@14..18
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
                  Visibility@0..3
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
    fn visibility_pub_crate() {
        check_item(
            "pub(crate) fn foo() {}",
            &expect![[r#"
                FunctionDef@0..22
                  Visibility@0..10
                    PUB_KW@0..3 "pub"
                    L_PAREN@3..4 "("
                    CRATE_KW@4..9 "crate"
                    R_PAREN@9..10 ")"
                  WHITESPACE@10..11 " "
                  FN_KW@11..13 "fn"
                  Name@13..17
                    WHITESPACE@13..14 " "
                    IDENT@14..17 "foo"
                  ParamList@17..19
                    L_PAREN@17..18 "("
                    R_PAREN@18..19 ")"
                  Block@19..22
                    WHITESPACE@19..20 " "
                    L_BRACE@20..21 "{"
                    R_BRACE@21..22 "}"
            "#]],
        );
    }

    #[test]
    fn visibility_pub_super() {
        check_item(
            "pub(super) fn foo() {}",
            &expect![[r#"
                FunctionDef@0..22
                  Visibility@0..10
                    PUB_KW@0..3 "pub"
                    L_PAREN@3..4 "("
                    SUPER_KW@4..9 "super"
                    R_PAREN@9..10 ")"
                  WHITESPACE@10..11 " "
                  FN_KW@11..13 "fn"
                  Name@13..17
                    WHITESPACE@13..14 " "
                    IDENT@14..17 "foo"
                  ParamList@17..19
                    L_PAREN@17..18 "("
                    R_PAREN@18..19 ")"
                  Block@19..22
                    WHITESPACE@19..20 " "
                    L_BRACE@20..21 "{"
                    R_BRACE@21..22 "}"
            "#]],
        );
    }

    #[test]
    fn visibility_pub_self() {
        check_item(
            "pub(self) fn foo() {}",
            &expect![[r#"
                FunctionDef@0..21
                  Visibility@0..9
                    PUB_KW@0..3 "pub"
                    L_PAREN@3..4 "("
                    SELF_VALUE_KW@4..8 "self"
                    R_PAREN@8..9 ")"
                  WHITESPACE@9..10 " "
                  FN_KW@10..12 "fn"
                  Name@12..16
                    WHITESPACE@12..13 " "
                    IDENT@13..16 "foo"
                  ParamList@16..18
                    L_PAREN@16..17 "("
                    R_PAREN@17..18 ")"
                  Block@18..21
                    WHITESPACE@18..19 " "
                    L_BRACE@19..20 "{"
                    R_BRACE@20..21 "}"
            "#]],
        );
    }

    #[test]
    fn visibility_pub_in_path() {
        check_item(
            "pub(in crate::foo) fn bar() {}",
            &expect![[r#"
                FunctionDef@0..30
                  Visibility@0..18
                    PUB_KW@0..3 "pub"
                    L_PAREN@3..4 "("
                    IN_KW@4..6 "in"
                    Path@6..17
                      PathSegment@6..12
                        NameRef@6..12
                          WHITESPACE@6..7 " "
                          CRATE_KW@7..12 "crate"
                      COLON_COLON@12..14 "::"
                      PathSegment@14..17
                        NameRef@14..17
                          IDENT@14..17 "foo"
                    R_PAREN@17..18 ")"
                  WHITESPACE@18..19 " "
                  FN_KW@19..21 "fn"
                  Name@21..25
                    WHITESPACE@21..22 " "
                    IDENT@22..25 "bar"
                  ParamList@25..27
                    L_PAREN@25..26 "("
                    R_PAREN@26..27 ")"
                  Block@27..30
                    WHITESPACE@27..28 " "
                    L_BRACE@28..29 "{"
                    R_BRACE@29..30 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pub_crate() {
        check_item(
            "pub(crate) struct Foo {}",
            &expect![[r#"
                StructDef@0..24
                  Visibility@0..10
                    PUB_KW@0..3 "pub"
                    L_PAREN@3..4 "("
                    CRATE_KW@4..9 "crate"
                    R_PAREN@9..10 ")"
                  WHITESPACE@10..11 " "
                  STRUCT_KW@11..17 "struct"
                  Name@17..21
                    WHITESPACE@17..18 " "
                    IDENT@18..21 "Foo"
                  FieldList@21..24
                    WHITESPACE@21..22 " "
                    L_BRACE@22..23 "{"
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn field_pub_crate() {
        check_item(
            "struct Foo { pub(crate) x: i32 }",
            &expect![[r#"
                StructDef@0..32
                  STRUCT_KW@0..6 "struct"
                  Name@6..10
                    WHITESPACE@6..7 " "
                    IDENT@7..10 "Foo"
                  FieldList@10..32
                    WHITESPACE@10..11 " "
                    L_BRACE@11..12 "{"
                    FieldDef@12..30
                      Visibility@12..23
                        WHITESPACE@12..13 " "
                        PUB_KW@13..16 "pub"
                        L_PAREN@16..17 "("
                        CRATE_KW@17..22 "crate"
                        R_PAREN@22..23 ")"
                      Name@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "x"
                      COLON@25..26 ":"
                      PathType@26..30
                        Path@26..30
                          PathSegment@26..30
                            NameRef@26..30
                              WHITESPACE@26..27 " "
                              IDENT@27..30 "i32"
                    WHITESPACE@30..31 " "
                    R_BRACE@31..32 "}"
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
                        Path@19..23
                          PathSegment@19..23
                            NameRef@19..23
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
                        Path@17..19
                          PathSegment@17..19
                            NameRef@17..19
                              WHITESPACE@17..18 " "
                              IDENT@18..19 "T"
                    R_PAREN@19..20 ")"
                  WHITESPACE@20..21 " "
                  ARROW@21..23 "->"
                  PathType@23..25
                    Path@23..25
                      PathSegment@23..25
                        NameRef@23..25
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
                        Path@16..18
                          PathSegment@16..18
                            NameRef@16..18
                              WHITESPACE@16..17 " "
                              IDENT@17..18 "T"
                    COMMA@18..19 ","
                    Param@19..24
                      Name@19..21
                        WHITESPACE@19..20 " "
                        IDENT@20..21 "b"
                      COLON@21..22 ":"
                      PathType@22..24
                        Path@22..24
                          PathSegment@22..24
                            NameRef@22..24
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

    // === Struct tests ===

    #[test]
    fn struct_empty() {
        check_item(
            "struct Point {}",
            &expect![[r#"
                StructDef@0..15
                  STRUCT_KW@0..6 "struct"
                  Name@6..12
                    WHITESPACE@6..7 " "
                    IDENT@7..12 "Point"
                  FieldList@12..15
                    WHITESPACE@12..13 " "
                    L_BRACE@13..14 "{"
                    R_BRACE@14..15 "}"
            "#]],
        );
    }

    #[test]
    fn struct_with_fields() {
        check_item(
            "struct Point { x: i32, y: i32 }",
            &expect![[r#"
                StructDef@0..31
                  STRUCT_KW@0..6 "struct"
                  Name@6..12
                    WHITESPACE@6..7 " "
                    IDENT@7..12 "Point"
                  FieldList@12..31
                    WHITESPACE@12..13 " "
                    L_BRACE@13..14 "{"
                    FieldDef@14..21
                      Name@14..16
                        WHITESPACE@14..15 " "
                        IDENT@15..16 "x"
                      COLON@16..17 ":"
                      PathType@17..21
                        Path@17..21
                          PathSegment@17..21
                            NameRef@17..21
                              WHITESPACE@17..18 " "
                              IDENT@18..21 "i32"
                    COMMA@21..22 ","
                    FieldDef@22..29
                      Name@22..24
                        WHITESPACE@22..23 " "
                        IDENT@23..24 "y"
                      COLON@24..25 ":"
                      PathType@25..29
                        Path@25..29
                          PathSegment@25..29
                            NameRef@25..29
                              WHITESPACE@25..26 " "
                              IDENT@26..29 "i32"
                    WHITESPACE@29..30 " "
                    R_BRACE@30..31 "}"
            "#]],
        );
    }

    #[test]
    fn struct_pub() {
        check_item(
            "pub struct Foo {}",
            &expect![[r#"
                StructDef@0..17
                  Visibility@0..3
                    PUB_KW@0..3 "pub"
                  WHITESPACE@3..4 " "
                  STRUCT_KW@4..10 "struct"
                  Name@10..14
                    WHITESPACE@10..11 " "
                    IDENT@11..14 "Foo"
                  FieldList@14..17
                    WHITESPACE@14..15 " "
                    L_BRACE@15..16 "{"
                    R_BRACE@16..17 "}"
            "#]],
        );
    }

    #[test]
    fn struct_with_pub_field() {
        check_item(
            "struct Foo { pub x: i32 }",
            &expect![[r#"
                StructDef@0..25
                  STRUCT_KW@0..6 "struct"
                  Name@6..10
                    WHITESPACE@6..7 " "
                    IDENT@7..10 "Foo"
                  FieldList@10..25
                    WHITESPACE@10..11 " "
                    L_BRACE@11..12 "{"
                    FieldDef@12..23
                      Visibility@12..16
                        WHITESPACE@12..13 " "
                        PUB_KW@13..16 "pub"
                      Name@16..18
                        WHITESPACE@16..17 " "
                        IDENT@17..18 "x"
                      COLON@18..19 ":"
                      PathType@19..23
                        Path@19..23
                          PathSegment@19..23
                            NameRef@19..23
                              WHITESPACE@19..20 " "
                              IDENT@20..23 "i32"
                    WHITESPACE@23..24 " "
                    R_BRACE@24..25 "}"
            "#]],
        );
    }

    #[test]
    fn struct_with_generics() {
        check_item(
            "struct Pair<T, U> { first: T, second: U }",
            &expect![[r#"
                StructDef@0..41
                  STRUCT_KW@0..6 "struct"
                  Name@6..11
                    WHITESPACE@6..7 " "
                    IDENT@7..11 "Pair"
                  GenericParams@11..17
                    LT@11..12 "<"
                    GenericParam@12..13
                      Name@12..13
                        IDENT@12..13 "T"
                    COMMA@13..14 ","
                    GenericParam@14..16
                      Name@14..16
                        WHITESPACE@14..15 " "
                        IDENT@15..16 "U"
                    GT@16..17 ">"
                  FieldList@17..41
                    WHITESPACE@17..18 " "
                    L_BRACE@18..19 "{"
                    FieldDef@19..28
                      Name@19..25
                        WHITESPACE@19..20 " "
                        IDENT@20..25 "first"
                      COLON@25..26 ":"
                      PathType@26..28
                        Path@26..28
                          PathSegment@26..28
                            NameRef@26..28
                              WHITESPACE@26..27 " "
                              IDENT@27..28 "T"
                    COMMA@28..29 ","
                    FieldDef@29..39
                      Name@29..36
                        WHITESPACE@29..30 " "
                        IDENT@30..36 "second"
                      COLON@36..37 ":"
                      PathType@37..39
                        Path@37..39
                          PathSegment@37..39
                            NameRef@37..39
                              WHITESPACE@37..38 " "
                              IDENT@38..39 "U"
                    WHITESPACE@39..40 " "
                    R_BRACE@40..41 "}"
            "#]],
        );
    }

    // === Type alias tests ===

    #[test]
    fn type_alias_simple() {
        check_item(
            "type Int = i32;",
            &expect![[r#"
                TypeAlias@0..15
                  TYPE_KW@0..4 "type"
                  Name@4..8
                    WHITESPACE@4..5 " "
                    IDENT@5..8 "Int"
                  WHITESPACE@8..9 " "
                  EQ@9..10 "="
                  PathType@10..14
                    Path@10..14
                      PathSegment@10..14
                        NameRef@10..14
                          WHITESPACE@10..11 " "
                          IDENT@11..14 "i32"
                  SEMI@14..15 ";"
            "#]],
        );
    }

    #[test]
    fn type_alias_pub() {
        check_item(
            "pub type Callback = fn(i32) -> bool;",
            &expect![[r#"
                TypeAlias@0..36
                  Visibility@0..3
                    PUB_KW@0..3 "pub"
                  WHITESPACE@3..4 " "
                  TYPE_KW@4..8 "type"
                  Name@8..17
                    WHITESPACE@8..9 " "
                    IDENT@9..17 "Callback"
                  WHITESPACE@17..18 " "
                  EQ@18..19 "="
                  FnPtrType@19..35
                    WHITESPACE@19..20 " "
                    FN_KW@20..22 "fn"
                    L_PAREN@22..23 "("
                    PathType@23..26
                      Path@23..26
                        PathSegment@23..26
                          NameRef@23..26
                            IDENT@23..26 "i32"
                    R_PAREN@26..27 ")"
                    WHITESPACE@27..28 " "
                    ARROW@28..30 "->"
                    PathType@30..35
                      Path@30..35
                        PathSegment@30..35
                          NameRef@30..35
                            WHITESPACE@30..31 " "
                            IDENT@31..35 "bool"
                  SEMI@35..36 ";"
            "#]],
        );
    }

    // === Impl block tests ===

    #[test]
    fn impl_empty() {
        check_item(
            "impl Point {}",
            &expect![[r#"
                ImplBlock@0..13
                  IMPL_KW@0..4 "impl"
                  PathType@4..10
                    Path@4..10
                      PathSegment@4..10
                        NameRef@4..10
                          WHITESPACE@4..5 " "
                          IDENT@5..10 "Point"
                  WHITESPACE@10..11 " "
                  L_BRACE@11..12 "{"
                  R_BRACE@12..13 "}"
            "#]],
        );
    }

    #[test]
    fn impl_with_method() {
        check_item(
            "impl Point { fn new() -> Point {} }",
            &expect![[r#"
                ImplBlock@0..35
                  IMPL_KW@0..4 "impl"
                  PathType@4..10
                    Path@4..10
                      PathSegment@4..10
                        NameRef@4..10
                          WHITESPACE@4..5 " "
                          IDENT@5..10 "Point"
                  WHITESPACE@10..11 " "
                  L_BRACE@11..12 "{"
                  FunctionDef@12..33
                    WHITESPACE@12..13 " "
                    FN_KW@13..15 "fn"
                    Name@15..19
                      WHITESPACE@15..16 " "
                      IDENT@16..19 "new"
                    ParamList@19..21
                      L_PAREN@19..20 "("
                      R_PAREN@20..21 ")"
                    WHITESPACE@21..22 " "
                    ARROW@22..24 "->"
                    PathType@24..30
                      Path@24..30
                        PathSegment@24..30
                          NameRef@24..30
                            WHITESPACE@24..25 " "
                            IDENT@25..30 "Point"
                    Block@30..33
                      WHITESPACE@30..31 " "
                      L_BRACE@31..32 "{"
                      R_BRACE@32..33 "}"
                  WHITESPACE@33..34 " "
                  R_BRACE@34..35 "}"
            "#]],
        );
    }

    #[test]
    fn impl_with_generics() {
        check_item(
            "impl<T> Vec<T> {}",
            &expect![[r#"
                ImplBlock@0..17
                  IMPL_KW@0..4 "impl"
                  GenericParams@4..7
                    LT@4..5 "<"
                    GenericParam@5..6
                      Name@5..6
                        IDENT@5..6 "T"
                    GT@6..7 ">"
                  PathType@7..14
                    Path@7..14
                      PathSegment@7..14
                        NameRef@7..11
                          WHITESPACE@7..8 " "
                          IDENT@8..11 "Vec"
                        GenericArgs@11..14
                          LT@11..12 "<"
                          PathType@12..13
                            Path@12..13
                              PathSegment@12..13
                                NameRef@12..13
                                  IDENT@12..13 "T"
                          GT@13..14 ">"
                  WHITESPACE@14..15 " "
                  L_BRACE@15..16 "{"
                  R_BRACE@16..17 "}"
            "#]],
        );
    }

    // === Source file tests ===

    #[test]
    fn source_file_empty() {
        use crate::parser::tests::check_source_file;
        check_source_file(
            "",
            &expect![[r#"
                SourceFile@0..0
            "#]],
        );
    }

    #[test]
    fn source_file_single_function() {
        use crate::parser::tests::check_source_file;
        check_source_file(
            "fn main() {}",
            &expect![[r#"
                SourceFile@0..12
                  FunctionDef@0..12
                    FN_KW@0..2 "fn"
                    Name@2..7
                      WHITESPACE@2..3 " "
                      IDENT@3..7 "main"
                    ParamList@7..9
                      L_PAREN@7..8 "("
                      R_PAREN@8..9 ")"
                    Block@9..12
                      WHITESPACE@9..10 " "
                      L_BRACE@10..11 "{"
                      R_BRACE@11..12 "}"
            "#]],
        );
    }

    #[test]
    fn source_file_multiple_items() {
        use crate::parser::tests::check_source_file;
        check_source_file(
            "struct Point { x: i32 }\nfn main() {}",
            &expect![[r#"
                SourceFile@0..36
                  StructDef@0..23
                    STRUCT_KW@0..6 "struct"
                    Name@6..12
                      WHITESPACE@6..7 " "
                      IDENT@7..12 "Point"
                    FieldList@12..23
                      WHITESPACE@12..13 " "
                      L_BRACE@13..14 "{"
                      FieldDef@14..21
                        Name@14..16
                          WHITESPACE@14..15 " "
                          IDENT@15..16 "x"
                        COLON@16..17 ":"
                        PathType@17..21
                          Path@17..21
                            PathSegment@17..21
                              NameRef@17..21
                                WHITESPACE@17..18 " "
                                IDENT@18..21 "i32"
                      WHITESPACE@21..22 " "
                      R_BRACE@22..23 "}"
                  FunctionDef@23..36
                    WHITESPACE@23..24 "\n"
                    FN_KW@24..26 "fn"
                    Name@26..31
                      WHITESPACE@26..27 " "
                      IDENT@27..31 "main"
                    ParamList@31..33
                      L_PAREN@31..32 "("
                      R_PAREN@32..33 ")"
                    Block@33..36
                      WHITESPACE@33..34 " "
                      L_BRACE@34..35 "{"
                      R_BRACE@35..36 "}"
            "#]],
        );
    }

    #[test]
    fn source_file_with_impl() {
        use crate::parser::tests::check_source_file;
        check_source_file(
            "struct Foo {}\nimpl Foo { fn bar(&self) {} }",
            &expect![[r#"
                SourceFile@0..43
                  StructDef@0..13
                    STRUCT_KW@0..6 "struct"
                    Name@6..10
                      WHITESPACE@6..7 " "
                      IDENT@7..10 "Foo"
                    FieldList@10..13
                      WHITESPACE@10..11 " "
                      L_BRACE@11..12 "{"
                      R_BRACE@12..13 "}"
                  ImplBlock@13..43
                    WHITESPACE@13..14 "\n"
                    IMPL_KW@14..18 "impl"
                    PathType@18..22
                      Path@18..22
                        PathSegment@18..22
                          NameRef@18..22
                            WHITESPACE@18..19 " "
                            IDENT@19..22 "Foo"
                    WHITESPACE@22..23 " "
                    L_BRACE@23..24 "{"
                    FunctionDef@24..41
                      WHITESPACE@24..25 " "
                      FN_KW@25..27 "fn"
                      Name@27..31
                        WHITESPACE@27..28 " "
                        IDENT@28..31 "bar"
                      ParamList@31..38
                        L_PAREN@31..32 "("
                        SelfParam@32..37
                          AMP@32..33 "&"
                          SELF_VALUE_KW@33..37 "self"
                        R_PAREN@37..38 ")"
                      Block@38..41
                        WHITESPACE@38..39 " "
                        L_BRACE@39..40 "{"
                        R_BRACE@40..41 "}"
                    WHITESPACE@41..42 " "
                    R_BRACE@42..43 "}"
            "#]],
        );
    }
}
