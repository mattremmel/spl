//! Item parser for SPL.
//!
//! Parses top-level items: functions, structs, type aliases, and impl blocks.

// Functions are called from tests; will be used by PARSE-8 module parser
#![allow(dead_code)]

use crate::parser::{CompletedMarker, Parser};
use crate::syntax::SyntaxKind;

use super::expr;
use super::stmt;

// === Attribute Parsing ===

/// Parse zero or more outer attributes: `#[name]`, `#[name(args)]`, `#[name = value]`
fn opt_attributes(p: &mut Parser<'_>) {
    while p.at(SyntaxKind::HASH) && !p.peek_at(1, SyntaxKind::BANG) {
        if attribute(p).is_err() {
            break;
        }
    }
}

/// Parse zero or more inner attributes: `#![name]`, etc.
fn opt_inner_attributes(p: &mut Parser<'_>) {
    while p.at(SyntaxKind::HASH) && p.peek_at(1, SyntaxKind::BANG) {
        if inner_attribute(p).is_err() {
            break;
        }
    }
}

/// Parse a single inner attribute: `#![...]`
fn inner_attribute(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    if let Err(err) = p.expect(SyntaxKind::HASH) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = p.expect(SyntaxKind::BANG) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = p.expect(SyntaxKind::L_BRACKET) {
        p.error(err.clone());
        return Ok(m.complete(p, SyntaxKind::InnerAttribute));
    }

    // Tolerate missing content but emit error
    if !p.at(SyntaxKind::R_BRACKET) {
        if let Err(err) = attr_content(p) {
            p.error(err);
            // Skip to closing bracket or item-starting token (with limit)
            let mut skip_count = 0;
            while p.current().is_some()
                && !p.at(SyntaxKind::R_BRACKET)
                && !p.current().is_some_and(is_item_start)
                && skip_count < 100
            {
                p.bump();
                skip_count += 1;
            }
        }
    } else {
        p.error(p.error_at_current("expected attribute name".to_string()));
    }

    // Consume ] if present
    if p.at(SyntaxKind::R_BRACKET) {
        p.bump();
    }
    Ok(m.complete(p, SyntaxKind::InnerAttribute))
}

/// Check if we're at a token that could start an item (for recovery).
fn is_item_start(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::FN_KW
            | SyntaxKind::STRUCT_KW
            | SyntaxKind::TYPE_KW
            | SyntaxKind::IMPL_KW
            | SyntaxKind::PUB_KW
            | SyntaxKind::USE_KW
            | SyntaxKind::EXTERN_KW
    )
}

/// Parse a single outer attribute: `#[...]`
fn attribute(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    if let Err(err) = p.expect(SyntaxKind::HASH) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = p.expect(SyntaxKind::L_BRACKET) {
        p.error(err.clone());
        return Ok(m.complete(p, SyntaxKind::Attribute));
    }

    // Tolerate missing content but emit error
    if !p.at(SyntaxKind::R_BRACKET) {
        if let Err(err) = attr_content(p) {
            p.error(err);
            // Skip to closing bracket or item-starting token
            while p.current().is_some()
                && !p.at(SyntaxKind::R_BRACKET)
                && !p.current().is_some_and(is_item_start)
            {
                p.bump();
            }
        }
    } else {
        p.error(p.error_at_current("expected attribute name".to_string()));
    }

    // Consume ] if present, otherwise just emit error
    if p.at(SyntaxKind::R_BRACKET) {
        p.bump();
    }
    Ok(m.complete(p, SyntaxKind::Attribute))
}

/// Parse attribute content: path with optional input
fn attr_content(p: &mut Parser<'_>) -> Result<(), crate::parser::ParseError> {
    attr_path(p)?;
    if p.at(SyntaxKind::L_PAREN) {
        attr_input_paren(p)?;
    } else if p.at(SyntaxKind::EQ) {
        attr_input_eq(p)?;
    }
    Ok(())
}

/// Parse attribute path: `name` or `name.path.segments`
fn attr_path(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    if let Err(err) = p.expect(SyntaxKind::IDENT) {
        m.abandon(p);
        return Err(err);
    }
    while p.eat(SyntaxKind::DOT) {
        if let Err(err) = p.expect(SyntaxKind::IDENT) {
            p.error(err);
            break;
        }
    }
    Ok(m.complete(p, SyntaxKind::AttrPath))
}

/// Parse parenthesized attribute input: `(args, ...)`
fn attr_input_paren(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;

    // Parse comma-separated args with error recovery
    // Use a maximum iteration count to prevent infinite loops
    const MAX_ARGS: usize = 1000;
    let mut arg_count = 0;

    while !p.at(SyntaxKind::R_PAREN)
        && !p.at(SyntaxKind::R_BRACKET)
        && p.current().is_some()
        && arg_count < MAX_ARGS
    {
        arg_count += 1;

        if let Err(err) = attr_arg(p) {
            p.error(err);
            // Skip one token to make progress
            if p.current().is_some()
                && !p.at(SyntaxKind::R_PAREN)
                && !p.at(SyntaxKind::COMMA)
                && !p.at(SyntaxKind::R_BRACKET)
            {
                p.bump();
            }
        }

        // Eat comma if present, otherwise we're done with args
        if !p.eat(SyntaxKind::COMMA) {
            break;
        }
    }

    // Don't fail if R_PAREN missing - just emit error and continue
    if !p.at(SyntaxKind::R_PAREN) {
        p.error(p.error_at_current("expected `)`".to_string()));
    } else {
        p.bump();
    }
    Ok(m.complete(p, SyntaxKind::AttrInput))
}

/// Parse `= value` attribute input
fn attr_input_eq(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::EQ)?;
    match p.current() {
        Some(
            SyntaxKind::STRING_LITERAL
            | SyntaxKind::INT_LITERAL
            | SyntaxKind::TRUE_KW
            | SyntaxKind::FALSE_KW,
        ) => p.bump(),
        _ => return Err(p.error_at_current("expected literal".to_string())),
    }
    Ok(m.complete(p, SyntaxKind::AttrInput))
}

/// Parse a single attribute argument: `value`, `key = value`, or nested `name(args)`
fn attr_arg(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    // Check for key = value pattern
    if p.at(SyntaxKind::IDENT) && p.peek_at(1, SyntaxKind::EQ) && !p.peek_at(2, SyntaxKind::EQ) {
        p.bump(); // key
        p.bump(); // =
        if let Err(err) = attr_value(p) {
            // Value failed but we consumed key and = so complete the arg
            p.error(err);
        }
        Ok(m.complete(p, SyntaxKind::AttrArg))
    } else {
        // Must successfully parse a value, otherwise fail
        match attr_value(p) {
            Ok(()) => Ok(m.complete(p, SyntaxKind::AttrArg)),
            Err(err) => {
                m.abandon(p);
                Err(err)
            }
        }
    }
}

/// Parse an attribute value: literal, identifier, or nested attribute content
fn attr_value(p: &mut Parser<'_>) -> Result<(), crate::parser::ParseError> {
    match p.current() {
        Some(
            SyntaxKind::STRING_LITERAL
            | SyntaxKind::INT_LITERAL
            | SyntaxKind::TRUE_KW
            | SyntaxKind::FALSE_KW,
        ) => {
            p.bump();
            Ok(())
        }
        Some(SyntaxKind::IDENT) => {
            // Could be simple ident or nested: name(args)
            attr_content(p)?;
            Ok(())
        }
        _ => Err(p.error_at_current("expected attribute value".to_string())),
    }
}

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

/// Parse a function definition: `[attrs] [pub] fn name(params) [: Type] [where ...] { body }`
///
/// Return type syntax: `fn foo(): i32 where T { ... }`
pub(crate) fn function_def(
    p: &mut Parser<'_>,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // fn keyword
    p.expect(SyntaxKind::FN_KW)?;

    // Function name
    name(p)?;

    // Parameter list
    param_list(p)?;

    // Optional return type with `:` syntax
    if p.eat(SyntaxKind::COLON) {
        stmt::type_annotation(p)?;
    }

    // Optional where clause (new syntax: `where T, U: Clone`)
    if p.at(SyntaxKind::WHERE_KW) {
        where_clause(p)?;
    }

    // Function body
    expr::block(p)?;

    Ok(m.complete(p, SyntaxKind::FunctionDef))
}

/// Parse a where clause: `where T, U: Clone, ...`
fn where_clause(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::WHERE_KW)?;

    // Parse comma-separated type parameters with optional bounds
    loop {
        where_type_param(p)?;

        if !p.eat(SyntaxKind::COMMA) {
            break;
        }

        // Allow trailing comma before block
        if p.at(SyntaxKind::L_BRACE) {
            break;
        }
    }

    Ok(m.complete(p, SyntaxKind::WhereClause))
}

/// Parse a type parameter in a where clause: `T` or `T: Bound + OtherBound`
fn where_type_param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Type parameter name
    name(p)?;

    // Optional bounds
    if p.eat(SyntaxKind::COLON) {
        // Parse first bound
        type_bound(p)?;

        // Parse additional bounds separated by +
        while p.eat(SyntaxKind::PLUS) {
            type_bound(p)?;
        }
    }

    Ok(m.complete(p, SyntaxKind::GenericParam))
}

/// Parse a type bound: `Clone` or `Iterator<Item = T>`
fn type_bound(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Parse the path (trait name, possibly with generics)
    crate::parser::path::path(p)?;

    Ok(m.complete(p, SyntaxKind::TypeBound))
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
        // Eat comma after self param if present (or we're at close paren)
        if !p.at(SyntaxKind::R_PAREN) {
            p.eat(SyntaxKind::COMMA);
        }
    }

    // Regular parameters with recovery
    p.parse_delimited_with_recovery(SyntaxKind::L_PAREN, SyntaxKind::R_PAREN, |p| {
        param(p)?;
        Ok(())
    });

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

/// Parse optional label spec: `_` | IDENTIFIER (when followed by another IDENT before `:`)
/// This distinguishes `label param: Type` from `param: Type`
fn opt_label_spec(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    // A label spec exists if we have IDENT followed by IDENT (skipping whitespace)
    // i.e., `label param: Type` or `_ param: Type`
    if p.at(SyntaxKind::IDENT) && p.peek_at(1, SyntaxKind::IDENT) {
        let m = p.start();
        p.bump(); // consume label or `_`
        return Some(m.complete(p, SyntaxKind::LabelSpec));
    }
    None
}

/// Parse a regular parameter: `[LabelSpec] name: Type`
fn param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional label spec
    opt_label_spec(p);

    // Parameter name
    if let Err(err) = name(p) {
        m.abandon(p);
        return Err(err);
    }

    // Type annotation
    if let Err(err) = p.expect(SyntaxKind::COLON) {
        m.abandon(p);
        return Err(err);
    }
    if let Err(err) = stmt::type_annotation(p) {
        m.abandon(p);
        return Err(err);
    }

    Ok(m.complete(p, SyntaxKind::Param))
}

/// Parse a struct definition.
///
/// Syntax: `[attrs] [pub] struct Name(fields) [where ...]` or `[attrs] [pub] struct Name;`
pub(crate) fn struct_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // struct keyword
    p.expect(SyntaxKind::STRUCT_KW)?;

    // Struct name
    name(p)?;

    // Check what follows to determine struct type:
    // - `;` -> unit struct
    // - `(` -> parenthesized struct (new syntax)
    // - `where` -> where clause before body
    if p.eat(SyntaxKind::SEMI) {
        // Unit struct: struct S;
    } else if p.at(SyntaxKind::L_PAREN) {
        // Parenthesized struct: struct Point(x: i32, y: i32)
        paren_field_list(p)?;

        // Optional where clause
        if p.at(SyntaxKind::WHERE_KW) {
            where_clause(p)?;
        }

        // Optional trailing semicolon
        p.eat(SyntaxKind::SEMI);
    } else if p.at(SyntaxKind::WHERE_KW) {
        // Where clause before body (must have parens after)
        where_clause(p)?;
        // Expect field list
        if p.at(SyntaxKind::L_PAREN) {
            paren_field_list(p)?;
        }
    } else {
        // Error: expected ( or ;
        return Err(p.error_at_current("expected '(' or ';' after struct name".to_string()));
    }

    Ok(m.complete(p, SyntaxKind::StructDef))
}

/// Parse a parenthesized field list: `([pub] name: Type, ...)`
/// Supports both new named fields `(x: i32)` and old tuple fields `(i32)`
fn paren_field_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    use std::cell::Cell;

    let m = p.start();
    p.expect(SyntaxKind::L_PAREN)?;

    let index = Cell::new(0u32);
    p.parse_delimited_with_recovery(SyntaxKind::L_PAREN, SyntaxKind::R_PAREN, |p| {
        let i = index.get();
        paren_field_def(p, i)?;
        index.set(i + 1);
        Ok(())
    });

    p.expect(SyntaxKind::R_PAREN)?;
    Ok(m.complete(p, SyntaxKind::FieldList))
}

/// Parse a field in parenthesized struct: `[pub] name: Type` or `[pub] Type`
fn paren_field_def(
    p: &mut Parser<'_>,
    index: u32,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    opt_visibility(p);

    // Check if this is named field or tuple field
    // Named: `name: Type` - identifier followed by colon
    // Tuple: `Type` - just a type
    let is_named = p.at(SyntaxKind::IDENT)
        && p.peek_at(1, SyntaxKind::COLON)
        && !p.peek_at(2, SyntaxKind::COLON); // Avoid confusing `path::Type` with named field

    if is_named {
        // Named field: name: Type
        name(p)?;
        p.expect(SyntaxKind::COLON)?;
        stmt::type_annotation(p)?;
    } else {
        // Tuple field: just Type - use synthetic index name
        let name_m = p.start();
        p.emit_synthetic_token(SyntaxKind::INT_LITERAL, index.to_string());
        name_m.complete(p, SyntaxKind::Name);
        stmt::type_annotation(p)?;
    }

    Ok(m.complete(p, SyntaxKind::FieldDef))
}

/// Parse a type alias: `[attrs] [pub] type Name = Type [where ...];`
pub(crate) fn type_alias(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // type keyword
    p.expect(SyntaxKind::TYPE_KW)?;

    // Alias name
    name(p)?;

    // = Type
    p.expect(SyntaxKind::EQ)?;
    stmt::type_annotation(p)?;

    // Optional where clause (new syntax)
    if p.at(SyntaxKind::WHERE_KW) {
        where_clause(p)?;
    }

    // Semicolon
    p.expect(SyntaxKind::SEMI)?;

    Ok(m.complete(p, SyntaxKind::TypeAlias))
}

/// Recovery set for impl block contents (functions only).
const IMPL_ITEM_RECOVERY_SET: &[SyntaxKind] =
    &[SyntaxKind::FN_KW, SyntaxKind::PUB_KW, SyntaxKind::R_BRACE];

/// Parse an impl block.
///
/// Syntax: `[attrs] impl Type [where T, U] { items }`
pub(crate) fn impl_block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // impl keyword
    p.expect(SyntaxKind::IMPL_KW)?;

    // Self type
    stmt::type_annotation(p)?;

    // Optional where clause (new syntax)
    if p.at(SyntaxKind::WHERE_KW) {
        where_clause(p)?;
    }

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
    visibility_lookahead_at(p, 0)
}

/// Calculate lookahead to skip past visibility modifier, starting at a given offset.
/// Returns the number of tokens to skip (relative to start_offset).
fn visibility_lookahead_at(p: &mut Parser<'_>, start_offset: usize) -> usize {
    if p.peek(start_offset) != Some(SyntaxKind::PUB_KW) {
        return 0;
    }
    // Check for pub(...)
    if p.peek(start_offset + 1) == Some(SyntaxKind::L_PAREN) {
        // Find the matching R_PAREN
        let mut depth = 1;
        let mut offset = 2;
        while depth > 0 {
            match p.peek(start_offset + offset) {
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

/// Parse an extern block: `extern "ABI" { fn name(...); ... }`
pub(crate) fn extern_block(
    p: &mut Parser<'_>,
) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // extern keyword
    p.expect(SyntaxKind::EXTERN_KW)?;

    // Optional ABI string (e.g., "C")
    p.eat(SyntaxKind::STRING_LITERAL);

    // Items block
    p.expect(SyntaxKind::L_BRACE)?;

    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        if let Err(err) = extern_fn(p) {
            p.recover_with_error(err, EXTERN_ITEM_RECOVERY_SET);
        }
    }

    p.expect(SyntaxKind::R_BRACE)?;

    Ok(m.complete(p, SyntaxKind::ExternBlock))
}

/// Recovery set for extern block contents.
const EXTERN_ITEM_RECOVERY_SET: &[SyntaxKind] =
    &[SyntaxKind::FN_KW, SyntaxKind::PUB_KW, SyntaxKind::R_BRACE];

/// Parse an extern function declaration: `[attrs] [pub] fn name(params) [: Type];`
fn extern_fn(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // fn keyword
    p.expect(SyntaxKind::FN_KW)?;

    // Function name
    name(p)?;

    // Parameter list
    param_list(p)?;

    // Optional return type
    if p.eat(SyntaxKind::COLON) {
        stmt::type_annotation(p)?;
    }

    // Semicolon (no body)
    p.expect(SyntaxKind::SEMI)?;

    Ok(m.complete(p, SyntaxKind::ExternFn))
}

/// Parse a use declaration: `[attrs] [pub] use path[.{tree}|.*|as name];`
///
/// Examples:
/// - `use std.vec.Vec;`
/// - `use std.collections.HashMap as Map;`
/// - `use std.prelude.*;`
/// - `use std.io.{Read, Write};`
/// - `use std.{vec.Vec, io.{Read, Write}};`
pub(crate) fn use_decl(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // use keyword
    p.expect(SyntaxKind::USE_KW)?;

    // Parse the use tree
    use_tree(p)?;

    // Semicolon
    p.expect(SyntaxKind::SEMI)?;

    Ok(m.complete(p, SyntaxKind::UseDecl))
}

/// Parse a use tree: path segments with optional glob, rename, or grouping.
///
/// UseTree = path ["as" IDENT]
///         | path "." "*"
///         | path "." "{" UseTreeList "}"
///         | "{" UseTreeList "}"
fn use_tree(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

    // Check for leading group: `{...}`
    if p.at(SyntaxKind::L_BRACE) {
        use_tree_list(p)?;
        return Ok(m.complete(p, SyntaxKind::UseTree));
    }

    // Parse path segments separated by dots
    // First segment is required
    if !is_use_path_segment_start(p.current()) {
        m.abandon(p);
        return Err(p.error_at_current("expected path in use declaration".to_string()));
    }

    // Parse first segment
    use_path_segment(p)?;

    // Continue parsing `.segment` until we hit a terminator
    loop {
        if !p.at(SyntaxKind::DOT) {
            break;
        }

        // Look at what follows the dot
        match p.peek(1) {
            // Glob: `.*`
            Some(SyntaxKind::STAR) => {
                p.bump(); // .
                p.bump(); // *
                return Ok(m.complete(p, SyntaxKind::UseTree));
            }
            // Group: `.{...}`
            Some(SyntaxKind::L_BRACE) => {
                p.bump(); // .
                use_tree_list(p)?;
                return Ok(m.complete(p, SyntaxKind::UseTree));
            }
            // Another path segment
            Some(k) if is_use_path_segment_kind(k) => {
                p.bump(); // .
                use_path_segment(p)?;
            }
            // End of path
            _ => break,
        }
    }

    // Check for rename: `as name`
    if p.at(SyntaxKind::AS_KW) {
        p.bump(); // as
        name(p)?;
    }

    Ok(m.complete(p, SyntaxKind::UseTree))
}

/// Check if token can start a use path segment.
fn is_use_path_segment_start(token: Option<SyntaxKind>) -> bool {
    matches!(
        token,
        Some(k) if is_use_path_segment_kind(k)
    )
}

/// Check if a SyntaxKind can be a use path segment.
fn is_use_path_segment_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::IDENT
            | SyntaxKind::SELF_VALUE_KW
            | SyntaxKind::SUPER_KW
            | SyntaxKind::MODULE_KW
            | SyntaxKind::CRATE_KW
    )
}

/// Parse a single path segment (identifier or keyword like self/super/module).
fn use_path_segment(p: &mut Parser<'_>) -> Result<(), crate::parser::ParseError> {
    if is_use_path_segment_kind(p.current().unwrap_or(SyntaxKind::ERROR)) {
        p.bump();
        Ok(())
    } else {
        Err(p.error_at_current("expected path segment".to_string()))
    }
}

/// Parse a use tree list: `{item1, item2, ...}`
fn use_tree_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();
    p.expect(SyntaxKind::L_BRACE)?;

    // Parse comma-separated use trees
    if !p.at(SyntaxKind::R_BRACE) {
        use_tree(p)?;

        while p.eat(SyntaxKind::COMMA) {
            // Allow trailing comma
            if p.at(SyntaxKind::R_BRACE) {
                break;
            }
            use_tree(p)?;
        }
    }

    p.expect(SyntaxKind::R_BRACE)?;
    Ok(m.complete(p, SyntaxKind::UseTreeList))
}

/// Calculate lookahead to skip past attributes.
/// Returns the offset after all outer attributes where the visibility/item keyword should be.
fn attribute_lookahead(p: &mut Parser<'_>) -> usize {
    let mut offset = 0;
    while p.peek(offset) == Some(SyntaxKind::HASH)
        && p.peek(offset + 1) != Some(SyntaxKind::BANG)
        && p.peek(offset + 1) == Some(SyntaxKind::L_BRACKET)
    {
        // Skip #[...]
        offset += 2; // Skip # and [
        let mut depth = 1;
        while depth > 0 {
            match p.peek(offset) {
                Some(SyntaxKind::L_BRACKET) => depth += 1,
                Some(SyntaxKind::R_BRACKET) => depth -= 1,
                None => break,
                _ => {}
            }
            offset += 1;
        }
    }
    offset
}

/// Parse a top-level item (function, struct, type alias, impl block, extern block, or use decl).
pub(crate) fn item(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    // Skip over attributes and visibility to find the item keyword
    let attr_offset = attribute_lookahead(p);
    let vis_offset = visibility_lookahead_at(p, attr_offset);
    let lookahead = attr_offset + vis_offset;
    let has_pub = p.peek(attr_offset) == Some(SyntaxKind::PUB_KW);

    match p.peek(lookahead) {
        Some(SyntaxKind::FN_KW) => function_def(p),
        Some(SyntaxKind::STRUCT_KW) => struct_def(p),
        Some(SyntaxKind::TYPE_KW) => type_alias(p),
        Some(SyntaxKind::IMPL_KW) if !has_pub => impl_block(p),
        Some(SyntaxKind::EXTERN_KW) if !has_pub => extern_block(p),
        Some(SyntaxKind::USE_KW) => use_decl(p),
        _ => {
            let err = p.error_at_current(
                "expected item (fn, struct, type, impl, extern, or use)".to_string(),
            );
            Err(err)
        }
    }
}

/// Parse a source file (sequence of items).
pub(crate) fn source_file(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();

    // Parse inner attributes at the start of the file
    opt_inner_attributes(p);

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
