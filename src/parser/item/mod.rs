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
pub(crate) fn name(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
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

    // Unit struct (semicolon), tuple struct (parens), or field list (braces)
    if p.eat(SyntaxKind::SEMI) {
        // Unit struct
    } else if p.at(SyntaxKind::L_PAREN) {
        tuple_field_list(p)?;
        p.expect(SyntaxKind::SEMI)?;
    } else {
        field_list(p)?;
    }

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

/// Recovery set for tuple field list (type start or list end).
const TUPLE_FIELD_RECOVERY_SET: &[SyntaxKind] = &[
    SyntaxKind::IDENT,
    SyntaxKind::PUB_KW,
    SyntaxKind::COMMA,
    SyntaxKind::R_PAREN,
    SyntaxKind::AMP,
    SyntaxKind::L_PAREN,
    SyntaxKind::L_BRACKET,
    SyntaxKind::FN_KW,
];

/// Parse a tuple field list: `([pub] Type, ...)`
fn tuple_field_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;

    let mut index = 0u32;
    while !p.at(SyntaxKind::R_PAREN) && p.current().is_some() {
        if let Err(err) = tuple_field_def(p, index) {
            p.recover_with_error(err, TUPLE_FIELD_RECOVERY_SET);
        }
        index += 1;
        if !p.eat(SyntaxKind::COMMA) && !p.at(SyntaxKind::R_PAREN) {
            let err = p.error_at_current("expected ',' or ')'".to_string());
            p.error(err);
        }
    }

    p.expect(SyntaxKind::R_PAREN)?;
    Ok(m.complete(p, SyntaxKind::FieldList))
}

/// Parse a tuple field definition: `[pub] Type`
fn tuple_field_def(
    p: &mut Parser<'_>,
    index: u32,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    opt_visibility(p);

    // Synthetic name node with index
    let name_m = p.start();
    p.emit_synthetic_token(SyntaxKind::INT_LITERAL, index.to_string());
    name_m.complete(p, SyntaxKind::Name);

    stmt::type_annotation(p)?;
    Ok(m.complete(p, SyntaxKind::FieldDef))
}

/// Parse a type alias: `[pub] type Name[<generics>] = Type;`
pub(crate) fn type_alias(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional visibility
    opt_visibility(p);

    // type keyword
    p.expect(SyntaxKind::TYPE_KW)?;

    // Alias name
    name(p)?;

    // Optional generic parameters
    if p.at(SyntaxKind::LT) {
        generic_params(p)?;
    }

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
mod tests;
