//! Item parser for SPL.
//!
//! Parses top-level items: functions, structs, type aliases, and impl blocks.

// Functions are called from tests; will be used by PARSE-8 module parser
#![allow(dead_code)]

use crate::{CompletedMarker, Parser};
use spl_syntax::SyntaxKind;

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
fn inner_attribute(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
    if p.at(SyntaxKind::R_BRACKET) {
        p.error(p.error_at_current("expected attribute name".to_string()));
    } else if let Err(err) = attr_content(p) {
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

    // Consume ] if present
    if p.at(SyntaxKind::R_BRACKET) {
        p.bump();
    }
    Ok(m.complete(p, SyntaxKind::InnerAttribute))
}

/// Check if we're at a token that could start an item (for recovery).
pub(crate) fn is_item_start(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::FN_KW
            | SyntaxKind::STRUCT_KW
            | SyntaxKind::ENUM_KW
            | SyntaxKind::TRAIT_KW
            | SyntaxKind::TYPE_KW
            | SyntaxKind::IMPL_KW
            | SyntaxKind::PUB_KW
            | SyntaxKind::USE_KW
            | SyntaxKind::EXTERN_KW
            | SyntaxKind::MODULE_KW
            | SyntaxKind::UNSAFE_KW
            | SyntaxKind::CONST_KW
            | SyntaxKind::STATIC_KW
            | SyntaxKind::GEN_KW
    )
}

/// Check if the current token starts a nested item inside a block.
///
/// This is used by the block parser to distinguish items from expressions.
/// Some keywords (`unsafe`, `const`) can start both items and expressions,
/// so we use lookahead to disambiguate:
/// - `unsafe { ... }` = expression, `unsafe fn/trait/impl` = item
/// - `const IDENT` = const def (item), `const fn` = item
pub(crate) fn is_nested_item_start(kind: SyntaxKind, p: &mut Parser<'_>) -> bool {
    match kind {
        // `unsafe` can be item (unsafe fn/trait/impl) or expression (unsafe { ... })
        SyntaxKind::UNSAFE_KW => matches!(
            p.peek(1),
            Some(SyntaxKind::FN_KW) | Some(SyntaxKind::TRAIT_KW) | Some(SyntaxKind::IMPL_KW)
        ),

        // `const` can be item (const X: T = ..., const fn ...) or could be const expr in future
        SyntaxKind::CONST_KW => {
            matches!(p.peek(1), Some(SyntaxKind::IDENT) | Some(SyntaxKind::FN_KW))
        }

        // `#[...]` attributes before items
        SyntaxKind::HASH => {
            if p.peek(1) == Some(SyntaxKind::L_BRACKET) {
                let offset = attribute_lookahead(p);
                let vis_offset = visibility_lookahead_at(p, offset);
                let check = offset + vis_offset;
                p.peek(check).is_some_and(is_item_start)
            } else {
                false
            }
        }

        // All other unambiguous item starters
        _ => is_item_start(kind),
    }
}

/// Parse a single outer attribute: `#[...]`
fn attribute(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
    if p.at(SyntaxKind::R_BRACKET) {
        p.error(p.error_at_current("expected attribute name".to_string()));
    } else if let Err(err) = attr_content(p) {
        p.error(err);
        // Skip to closing bracket or item-starting token
        while p.current().is_some()
            && !p.at(SyntaxKind::R_BRACKET)
            && !p.current().is_some_and(is_item_start)
        {
            p.bump();
        }
    }

    // Consume ] if present, otherwise just emit error
    if p.at(SyntaxKind::R_BRACKET) {
        p.bump();
    }
    Ok(m.complete(p, SyntaxKind::Attribute))
}

/// Parse attribute content: path with optional input
fn attr_content(p: &mut Parser<'_>) -> Result<(), crate::ParseError> {
    attr_path(p)?;
    if p.at(SyntaxKind::L_PAREN) {
        attr_input_paren(p)?;
    } else if p.at(SyntaxKind::EQ) {
        attr_input_eq(p)?;
    }
    Ok(())
}

/// Parse attribute path: `name` or `name.path.segments`
fn attr_path(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
fn attr_input_paren(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
    if p.at(SyntaxKind::R_PAREN) {
        p.bump();
    } else {
        p.error(p.error_at_current("expected `)`".to_string()));
    }
    Ok(m.complete(p, SyntaxKind::AttrInput))
}

/// Parse `= value` attribute input
fn attr_input_eq(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
fn attr_arg(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
fn attr_value(p: &mut Parser<'_>) -> Result<(), crate::ParseError> {
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

/// Parse optional visibility: `pub`, `pub($)`, `pub($.path)`, `pub(super)`
fn opt_visibility(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if !p.at(SyntaxKind::PUB_KW) {
        return None;
    }

    let m = p.start();
    p.bump(); // pub

    if p.at(SyntaxKind::L_PAREN) {
        p.bump(); // (

        // Handle pub($), pub($.path), and pub(super)
        if p.at(SyntaxKind::SUPER_KW) {
            p.bump();
        } else if p.at(SyntaxKind::DOLLAR) {
            p.bump(); // $
            // Optional path after $: pub($.path)
            if p.at(SyntaxKind::DOT) {
                p.bump(); // .
                // Parse the path after $.
                let _ = crate::path::path_no_generics(p);
            }
        }

        p.expect(SyntaxKind::R_PAREN).ok();
    }

    Some(m.complete(p, SyntaxKind::Visibility))
}

/// Parse a function definition: `[attrs] [pub] [const] [unsafe] fn name(params) [: Type] [where ...] { body }`
///
/// Return type syntax: `fn foo(): i32 where T { ... }`
pub(crate) fn function_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // Optional const modifier
    p.eat(SyntaxKind::CONST_KW);

    // Optional unsafe modifier
    p.eat(SyntaxKind::UNSAFE_KW);

    // Optional extern modifier with optional ABI
    if p.eat(SyntaxKind::EXTERN_KW) {
        p.eat(SyntaxKind::STRING_LITERAL);
    }

    // fn keyword
    if let Err(e) = p.expect(SyntaxKind::FN_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Function name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Parameter list
    if let Err(e) = param_list(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional return type with `:` syntax
    if p.eat(SyntaxKind::COLON)
        && let Err(e) = stmt::type_annotation(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Optional throws clause
    if let Some(Err(e)) = opt_throws_clause(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional where clause (new syntax: `where T, U: Clone`)
    if p.at(SyntaxKind::WHERE_KW)
        && let Err(e) = where_clause(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Function body
    if let Err(e) = expr::block(p) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::FunctionDef))
}

/// Parse a generator function definition: `[attrs] [pub] gen fn name(params) [: Type] [throws] [where ...] { body }`
pub(crate) fn generator_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // gen keyword
    if let Err(e) = p.expect(SyntaxKind::GEN_KW) {
        m.abandon(p);
        return Err(e);
    }

    // fn keyword
    if let Err(e) = p.expect(SyntaxKind::FN_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Generator name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Parameter list
    if let Err(e) = param_list(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional return type with `:` syntax
    if p.eat(SyntaxKind::COLON)
        && let Err(e) = stmt::type_annotation(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Optional throws clause
    if let Some(Err(e)) = opt_throws_clause(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional where clause
    if p.at(SyntaxKind::WHERE_KW)
        && let Err(e) = where_clause(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Generator body
    if let Err(e) = expr::block(p) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::GeneratorDef))
}

/// Parse a where clause: `where T, U: Clone, ...`
fn where_clause(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::WHERE_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Parse comma-separated type parameters with optional bounds
    loop {
        if let Err(e) = where_type_param(p) {
            m.abandon(p);
            return Err(e);
        }

        if !p.eat(SyntaxKind::COMMA) {
            break;
        }

        // Allow trailing comma: break if next token can't start a type param
        if !p.at(SyntaxKind::IDENT) {
            break;
        }
    }

    Ok(m.complete(p, SyntaxKind::WhereClause))
}

/// Parse an optional throws clause: `throws [Type]`
fn opt_throws_clause(p: &mut Parser<'_>) -> Option<Result<CompletedMarker, crate::ParseError>> {
    if !p.at(SyntaxKind::THROWS_KW) {
        return None;
    }

    let m = p.start();
    p.bump(); // throws

    // Optional exception type (if not followed by where or block)
    if !p.at(SyntaxKind::WHERE_KW)
        && !p.at(SyntaxKind::L_BRACE)
        && let Err(e) = stmt::type_annotation(p)
    {
        m.abandon(p);
        return Some(Err(e));
    }

    Some(Ok(m.complete(p, SyntaxKind::ThrowsClause)))
}

/// Parse a type parameter in a where clause: `T` or `T: Bound + OtherBound`
fn where_type_param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Type parameter name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional bounds
    if p.eat(SyntaxKind::COLON) {
        // Parse first bound
        if let Err(e) = type_bound(p) {
            m.abandon(p);
            return Err(e);
        }

        // Parse additional bounds separated by +
        while p.eat(SyntaxKind::PLUS) {
            if let Err(e) = type_bound(p) {
                m.abandon(p);
                return Err(e);
            }
        }
    }

    // Optional default type: `= Type`
    if p.eat(SyntaxKind::EQ)
        && let Err(e) = stmt::type_annotation(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::GenericParam))
}

/// Parse a type bound: `Clone` or `Iterator<Item = T>`
fn type_bound(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Parse the path (trait name, possibly with generics)
    if let Err(e) = crate::path::path(p) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::TypeBound))
}

/// Parse a name (identifier).
pub(crate) fn name(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
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
fn param_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::L_PAREN) {
        m.abandon(p);
        return Err(e);
    }

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
        // Variadic: `...` (valid in extern fn declarations)
        if p.at(SyntaxKind::ELLIPSIS) {
            let vm = p.start();
            p.bump();
            vm.complete(p, SyntaxKind::VariadicParam);
            return Ok(());
        }
        param(p)?;
        Ok(())
    });

    if let Err(e) = p.expect(SyntaxKind::R_PAREN) {
        m.abandon(p);
        return Err(e);
    }
    Ok(m.complete(p, SyntaxKind::ParamList))
}

/// Check if we're at the start of a self parameter.
fn is_self_param_start(p: &mut Parser<'_>) -> bool {
    p.at(SyntaxKind::SELF_VALUE_KW)
        || (p.at(SyntaxKind::AMP) && p.peek_at(1, SyntaxKind::SELF_VALUE_KW))
        || (p.at(SyntaxKind::AMP) && p.peek_at(1, SyntaxKind::MUT_KW))
        || (p.at(SyntaxKind::MUT_KW) && p.peek_at(1, SyntaxKind::SELF_VALUE_KW))
}

/// Parse a self parameter: `self`, `mut self`, `&self`, or `&mut self`
fn self_param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional & (for references) and mut
    p.eat(SyntaxKind::AMP);
    p.eat(SyntaxKind::MUT_KW);

    if let Err(e) = p.expect(SyntaxKind::SELF_VALUE_KW) {
        m.abandon(p);
        return Err(e);
    }
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

/// Parse a regular parameter: `[LabelSpec] name: Type [= expr]`
fn param(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional label spec
    opt_label_spec(p);

    // Optional mut
    p.eat(SyntaxKind::MUT_KW);

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

    // Optional default value
    if p.eat(SyntaxKind::EQ)
        && let Err(err) = expr::expr(p)
    {
        p.error(err);
    }

    Ok(m.complete(p, SyntaxKind::Param))
}

/// Parse a struct definition.
///
/// Syntax: `[attrs] [pub] struct Name(fields) [where ...]` or `[attrs] [pub] struct Name;`
pub(crate) fn struct_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // struct keyword
    if let Err(e) = p.expect(SyntaxKind::STRUCT_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Struct name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Check what follows to determine struct type:
    // - `;` -> unit struct
    // - `(` -> parenthesized struct (new syntax)
    // - `where` -> where clause before body
    if p.eat(SyntaxKind::SEMI) {
        // Unit struct: struct S;
    } else if p.at(SyntaxKind::L_PAREN) {
        // Parenthesized struct: struct Point(x: i32, y: i32)
        if let Err(e) = paren_field_list(p) {
            m.abandon(p);
            return Err(e);
        }

        // Optional where clause
        if p.at(SyntaxKind::WHERE_KW)
            && let Err(e) = where_clause(p)
        {
            m.abandon(p);
            return Err(e);
        }

        // Optional trailing semicolon
        p.eat(SyntaxKind::SEMI);
    } else if p.at(SyntaxKind::WHERE_KW) {
        // Where clause before body (must have parens after)
        if let Err(e) = where_clause(p) {
            m.abandon(p);
            return Err(e);
        }
        // Expect field list
        if p.at(SyntaxKind::L_PAREN)
            && let Err(e) = paren_field_list(p)
        {
            m.abandon(p);
            return Err(e);
        }
    } else {
        // Error: expected ( or ;
        m.abandon(p);
        return Err(p.error_at_current("expected '(' or ';' after struct name".to_string()));
    }

    Ok(m.complete(p, SyntaxKind::StructDef))
}

/// Parse an enum definition.
///
/// Syntax: `[attrs] [pub] enum Name { Variants } [where ...]`
/// Note: Where clause comes AFTER closing brace per spec.
pub(crate) fn enum_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // enum keyword
    if let Err(e) = p.expect(SyntaxKind::ENUM_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Enum name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Variant list in braces
    if let Err(e) = p.expect(SyntaxKind::L_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    // Parse variants if not empty
    if !p.at(SyntaxKind::R_BRACE)
        && let Err(e) = variant_list(p)
    {
        p.error(e);
        // Try to recover to closing brace
        while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
            p.bump();
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    // Optional where clause AFTER closing brace
    if p.at(SyntaxKind::WHERE_KW)
        && let Err(e) = where_clause(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::EnumDef))
}

/// Parse a variant list: `Variant { "," Variant } [ "," ]`
fn variant_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Parse first variant
    if let Err(e) = variant(p) {
        m.abandon(p);
        return Err(e);
    }

    // Parse remaining variants
    while p.eat(SyntaxKind::COMMA) {
        // Allow trailing comma
        if p.at(SyntaxKind::R_BRACE) {
            break;
        }
        if let Err(e) = variant(p) {
            p.error(e);
            break;
        }
    }

    Ok(m.complete(p, SyntaxKind::VariantList))
}

/// Parse a single variant: `IDENT [ "(" VariantFields ")" ]`
fn variant(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Variant name (should be UPPER_IDENT per spec, but we just use IDENT and let sema check)
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional variant fields in parentheses
    if p.at(SyntaxKind::L_PAREN)
        && let Err(e) = variant_fields(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::Variant))
}

/// Parse variant fields: `"(" (FieldList | TypeList) ")"`
/// Determine if it's named fields or tuple fields by checking for `:`.
fn variant_fields(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    // Re-use the parenthesized field list parser which handles both named and tuple fields
    paren_field_list(p)
}

/// Parse a trait definition (stub for now).
///
/// Syntax: `[unsafe] trait Name [: Bounds] [where ...] { items }`
pub(crate) fn trait_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // Optional unsafe
    p.eat(SyntaxKind::UNSAFE_KW);

    // trait keyword
    if let Err(e) = p.expect(SyntaxKind::TRAIT_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Trait name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional supertraits: `: PathType { "+" PathType }`
    if p.eat(SyntaxKind::COLON) {
        if let Err(e) = type_bound(p) {
            m.abandon(p);
            return Err(e);
        }
        while p.eat(SyntaxKind::PLUS) {
            if let Err(e) = type_bound(p) {
                m.abandon(p);
                return Err(e);
            }
        }
    }

    // Optional where clause BEFORE opening brace (unlike enum)
    if p.at(SyntaxKind::WHERE_KW)
        && let Err(e) = where_clause(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Trait body
    if let Err(e) = p.expect(SyntaxKind::L_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    // Parse trait items
    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        if let Err(err) = trait_item(p) {
            p.recover_with_error(err, TRAIT_ITEM_RECOVERY_SET);
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::TraitDef))
}

/// Recovery set for trait item contents.
const TRAIT_ITEM_RECOVERY_SET: &[SyntaxKind] = &[
    SyntaxKind::FN_KW,
    SyntaxKind::TYPE_KW,
    SyntaxKind::PUB_KW,
    SyntaxKind::CONST_KW,
    SyntaxKind::UNSAFE_KW,
    SyntaxKind::R_BRACE,
];

/// Parse a trait item: `[pub] (TraitMethod | AssociatedType)`
fn trait_item(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional visibility
    opt_visibility(p);

    // Determine if this is a method or associated type
    // Skip over optional modifiers to find the keyword
    let mut offset = 0;
    if p.peek(offset) == Some(SyntaxKind::CONST_KW) {
        offset += 1;
    }
    if p.peek(offset) == Some(SyntaxKind::UNSAFE_KW) {
        offset += 1;
    }

    match p.peek(offset) {
        Some(SyntaxKind::FN_KW) => {
            // Method
            p.eat(SyntaxKind::CONST_KW);
            p.eat(SyntaxKind::UNSAFE_KW);
            trait_method(p, m)
        }
        Some(SyntaxKind::TYPE_KW) => {
            // Associated type
            associated_type(p, m)
        }
        _ => {
            m.abandon(p);
            Err(p.error_at_current("expected trait item (fn or type)".to_string()))
        }
    }
}

/// Parse a trait method: `fn name(params) [: Type] [throws [Type]] [where ...] (Block | ";")`
fn trait_method(
    p: &mut Parser<'_>,
    m: crate::Marker,
) -> Result<CompletedMarker, crate::ParseError> {
    // fn keyword
    p.expect(SyntaxKind::FN_KW)?;

    // Method name
    name(p)?;

    // Parameter list
    param_list(p)?;

    // Optional return type
    if p.eat(SyntaxKind::COLON) {
        stmt::type_annotation(p)?;
    }

    // Optional throws clause
    if let Some(Err(e)) = opt_throws_clause(p) {
        return Err(e);
    }

    // Optional where clause
    if p.at(SyntaxKind::WHERE_KW) {
        where_clause(p)?;
    }

    // Body (block) or optional semicolon
    if p.at(SyntaxKind::L_BRACE) {
        expr::block(p)?;
    } else {
        stmt::eat_optional_semicolon(p);
    }

    Ok(m.complete(p, SyntaxKind::TraitItem))
}

/// Parse an associated type: `type Name [: Bounds] [";"]`
fn associated_type(
    p: &mut Parser<'_>,
    m: crate::Marker,
) -> Result<CompletedMarker, crate::ParseError> {
    // type keyword
    p.expect(SyntaxKind::TYPE_KW)?;

    // Type name
    name(p)?;

    // Optional bounds: `: PathType { "+" PathType }`
    if p.eat(SyntaxKind::COLON) {
        type_bound(p)?;
        while p.eat(SyntaxKind::PLUS) {
            type_bound(p)?;
        }
    }

    // Optional default: `= Type`
    if p.eat(SyntaxKind::EQ)
        && let Err(e) = stmt::type_annotation(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Optional semicolon
    stmt::eat_optional_semicolon(p);

    Ok(m.complete(p, SyntaxKind::AssociatedType))
}

/// Parse a parenthesized field list: `([pub] name: Type, ...)`
/// Supports both new named fields `(x: i32)` and old tuple fields `(i32)`
fn paren_field_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    use std::cell::Cell;

    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::L_PAREN) {
        m.abandon(p);
        return Err(e);
    }

    let index = Cell::new(0u32);
    p.parse_delimited_with_recovery(SyntaxKind::L_PAREN, SyntaxKind::R_PAREN, |p| {
        let i = index.get();
        paren_field_def(p, i)?;
        index.set(i + 1);
        Ok(())
    });

    if let Err(e) = p.expect(SyntaxKind::R_PAREN) {
        m.abandon(p);
        return Err(e);
    }
    Ok(m.complete(p, SyntaxKind::FieldList))
}

/// Parse a field in parenthesized struct: `[pub] name: Type` or `[pub] Type`
fn paren_field_def(p: &mut Parser<'_>, index: u32) -> Result<CompletedMarker, crate::ParseError> {
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
        if let Err(e) = name(p) {
            m.abandon(p);
            return Err(e);
        }
        if let Err(e) = p.expect(SyntaxKind::COLON) {
            m.abandon(p);
            return Err(e);
        }
        if let Err(e) = stmt::type_annotation(p) {
            m.abandon(p);
            return Err(e);
        }
    } else {
        // Tuple field: just Type - use synthetic index name
        let name_m = p.start();
        p.emit_synthetic_token(SyntaxKind::INT_LITERAL, index.to_string());
        name_m.complete(p, SyntaxKind::Name);
        if let Err(e) = stmt::type_annotation(p) {
            m.abandon(p);
            return Err(e);
        }
    }

    Ok(m.complete(p, SyntaxKind::FieldDef))
}

/// Parse a static definition: `[attrs] [pub] static [mut] Name: Type = Expr [;]`
pub(crate) fn static_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // static keyword
    if let Err(e) = p.expect(SyntaxKind::STATIC_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Optional mut
    p.eat(SyntaxKind::MUT_KW);

    // Static name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // : Type
    if let Err(e) = p.expect(SyntaxKind::COLON) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = stmt::type_annotation(p) {
        m.abandon(p);
        return Err(e);
    }

    // = Expr
    if let Err(e) = p.expect(SyntaxKind::EQ) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = expr::expr(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional semicolon
    stmt::eat_optional_semicolon(p);

    Ok(m.complete(p, SyntaxKind::StaticDef))
}

/// Parse a const definition: `[attrs] [pub] const Name: Type = Expr [;]`
pub(crate) fn const_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // const keyword
    if let Err(e) = p.expect(SyntaxKind::CONST_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Const name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // : Type
    if let Err(e) = p.expect(SyntaxKind::COLON) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = stmt::type_annotation(p) {
        m.abandon(p);
        return Err(e);
    }

    // = Expr
    if let Err(e) = p.expect(SyntaxKind::EQ) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = expr::expr(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional semicolon
    stmt::eat_optional_semicolon(p);

    Ok(m.complete(p, SyntaxKind::ConstDef))
}

/// Parse optional generic parameters: `(T, U, ...)`
///
/// Used for type alias generic params: `type Pair(T) = (T, T)`
fn opt_generic_params(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if !p.at(SyntaxKind::L_PAREN) {
        return None;
    }

    let m = p.start();
    p.bump(); // (

    // Parse comma-delimited Name list
    if !p.at(SyntaxKind::R_PAREN) {
        if let Err(e) = name(p) {
            p.error(e);
        }

        while p.eat(SyntaxKind::COMMA) {
            // Allow trailing comma
            if p.at(SyntaxKind::R_PAREN) {
                break;
            }
            if let Err(e) = name(p) {
                p.error(e);
                break;
            }
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_PAREN) {
        p.error(e);
    }

    Some(m.complete(p, SyntaxKind::GenericParams))
}

/// Parse a type alias: `[attrs] [pub] type Name [(params)] = Type [where ...];`
pub(crate) fn type_alias(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // type keyword
    if let Err(e) = p.expect(SyntaxKind::TYPE_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Alias name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional generic params: type Pair(T) = ...
    opt_generic_params(p);

    // = Type
    if let Err(e) = p.expect(SyntaxKind::EQ) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = stmt::type_annotation(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional where clause (new syntax)
    if p.at(SyntaxKind::WHERE_KW)
        && let Err(e) = where_clause(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Optional semicolon
    stmt::eat_optional_semicolon(p);

    Ok(m.complete(p, SyntaxKind::TypeAlias))
}

/// Recovery set for impl block contents.
const IMPL_ITEM_RECOVERY_SET: &[SyntaxKind] = &[
    SyntaxKind::FN_KW,
    SyntaxKind::TYPE_KW,
    SyntaxKind::PUB_KW,
    SyntaxKind::R_BRACE,
];

/// Parse an impl block.
///
/// Syntax: `[attrs] ["unsafe"] "impl" [TraitType "for"] Type [where T, U] { items }`
pub(crate) fn impl_block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional unsafe
    p.eat(SyntaxKind::UNSAFE_KW);

    // impl keyword
    if let Err(e) = p.expect(SyntaxKind::IMPL_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Parse first type (could be trait type or self type)
    if let Err(e) = stmt::type_annotation(p) {
        m.abandon(p);
        return Err(e);
    }

    // Check for `for` keyword — if present, previous type was the trait
    // and we need to parse the self type
    if p.eat(SyntaxKind::FOR_KW)
        && let Err(e) = stmt::type_annotation(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Optional where clause (new syntax)
    if p.at(SyntaxKind::WHERE_KW)
        && let Err(e) = where_clause(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Items block
    if let Err(e) = p.expect(SyntaxKind::L_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        if let Err(err) = item(p) {
            // Recover to next item in impl block
            p.recover_with_error(err, IMPL_ITEM_RECOVERY_SET);
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::ImplBlock))
}

/// Calculate lookahead to skip past visibility modifier.
/// Returns the offset after visibility where the item keyword should be.
fn visibility_lookahead(p: &mut Parser<'_>) -> usize {
    visibility_lookahead_at(p, 0)
}

/// Calculate lookahead to skip past visibility modifier, starting at a given offset.
/// Returns the number of tokens to skip (relative to `start_offset`).
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
pub(crate) fn extern_block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // extern keyword
    if let Err(e) = p.expect(SyntaxKind::EXTERN_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Optional ABI string (e.g., "C")
    p.eat(SyntaxKind::STRING_LITERAL);

    // Items block
    if let Err(e) = p.expect(SyntaxKind::L_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
        if let Err(err) = extern_fn(p) {
            p.recover_with_error(err, EXTERN_ITEM_RECOVERY_SET);
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    Ok(m.complete(p, SyntaxKind::ExternBlock))
}

/// Recovery set for extern block contents.
const EXTERN_ITEM_RECOVERY_SET: &[SyntaxKind] =
    &[SyntaxKind::FN_KW, SyntaxKind::PUB_KW, SyntaxKind::R_BRACE];

/// Parse an extern function declaration: `[attrs] [pub] fn name(params) [: Type];`
fn extern_fn(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // fn keyword
    if let Err(e) = p.expect(SyntaxKind::FN_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Function name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Parameter list
    if let Err(e) = param_list(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional return type
    if p.eat(SyntaxKind::COLON)
        && let Err(e) = stmt::type_annotation(p)
    {
        m.abandon(p);
        return Err(e);
    }

    // Optional semicolon (no body)
    stmt::eat_optional_semicolon(p);

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
pub(crate) fn use_decl(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // use keyword
    if let Err(e) = p.expect(SyntaxKind::USE_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Parse the use tree
    if let Err(e) = use_tree(p) {
        m.abandon(p);
        return Err(e);
    }

    // Optional semicolon
    stmt::eat_optional_semicolon(p);

    Ok(m.complete(p, SyntaxKind::UseDecl))
}

/// Parse a use tree: path segments with optional glob, rename, or grouping.
///
/// `UseTree` = path ["as" IDENT]
///         | path "." "*"
///         | path "." "{" `UseTreeList` "}"
///         | "{" `UseTreeList` "}"
fn use_tree(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Check for leading group: `{...}`
    if p.at(SyntaxKind::L_BRACE) {
        if let Err(e) = use_tree_list(p) {
            m.abandon(p);
            return Err(e);
        }
        return Ok(m.complete(p, SyntaxKind::UseTree));
    }

    // Check for standalone glob: `*` (inside a group like `{Read, *}`)
    if p.at(SyntaxKind::STAR) {
        p.bump();
        return Ok(m.complete(p, SyntaxKind::UseTree));
    }

    // Parse path segments separated by dots
    // First segment is required
    if !is_use_path_segment_start(p.current()) {
        m.abandon(p);
        return Err(p.error_at_current("expected path in use declaration".to_string()));
    }

    // Parse first segment
    if let Err(e) = use_path_segment(p) {
        m.abandon(p);
        return Err(e);
    }

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
                if let Err(e) = use_tree_list(p) {
                    m.abandon(p);
                    return Err(e);
                }
                return Ok(m.complete(p, SyntaxKind::UseTree));
            }
            // Another path segment
            Some(k) if is_use_path_segment_kind(k) => {
                p.bump(); // .
                if let Err(e) = use_path_segment(p) {
                    m.abandon(p);
                    return Err(e);
                }
            }
            // End of path
            _ => break,
        }
    }

    // Check for rename: `as name`
    if p.at(SyntaxKind::AS_KW) {
        p.bump(); // as
        if let Err(e) = name(p) {
            m.abandon(p);
            return Err(e);
        }
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

/// Check if a `SyntaxKind` can be a use path segment.
fn is_use_path_segment_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::IDENT
            | SyntaxKind::SELF_VALUE_KW
            | SyntaxKind::SUPER_KW
            | SyntaxKind::MODULE_KW
            | SyntaxKind::DOLLAR
    )
}

/// Parse a single path segment (identifier or keyword like self/super/module).
fn use_path_segment(p: &mut Parser<'_>) -> Result<(), crate::ParseError> {
    if is_use_path_segment_kind(p.current().unwrap_or(SyntaxKind::ERROR)) {
        p.bump();
        Ok(())
    } else {
        Err(p.error_at_current("expected path segment".to_string()))
    }
}

/// Parse a use tree list: `{item1, item2, ...}`
fn use_tree_list(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::L_BRACE) {
        m.abandon(p);
        return Err(e);
    }

    // Parse comma-separated use trees
    if !p.at(SyntaxKind::R_BRACE) {
        if let Err(e) = use_tree(p) {
            m.abandon(p);
            return Err(e);
        }

        while p.eat(SyntaxKind::COMMA) {
            // Allow trailing comma
            if p.at(SyntaxKind::R_BRACE) {
                break;
            }
            if let Err(e) = use_tree(p) {
                m.abandon(p);
                return Err(e);
            }
        }
    }

    if let Err(e) = p.expect(SyntaxKind::R_BRACE) {
        m.abandon(p);
        return Err(e);
    }
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
pub(crate) fn item(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    // Skip over attributes and visibility to find the item keyword
    let attr_offset = attribute_lookahead(p);
    let vis_offset = visibility_lookahead_at(p, attr_offset);
    let mut lookahead = attr_offset + vis_offset;
    let has_pub = p.peek(attr_offset) == Some(SyntaxKind::PUB_KW);

    // Skip optional `const` modifier
    if p.peek(lookahead) == Some(SyntaxKind::CONST_KW) {
        // Check if this is `const fn` (modifier) or `const NAME` (const definition)
        // If followed by fn or unsafe, it's a function modifier
        let next = p.peek(lookahead + 1);
        if next == Some(SyntaxKind::FN_KW) || next == Some(SyntaxKind::UNSAFE_KW) {
            lookahead += 1;
        }
    }

    // Skip optional `unsafe` modifier
    if p.peek(lookahead) == Some(SyntaxKind::UNSAFE_KW) {
        lookahead += 1;
    }

    match p.peek(lookahead) {
        Some(SyntaxKind::GEN_KW) => generator_def(p),
        Some(SyntaxKind::FN_KW) => function_def(p),
        Some(SyntaxKind::STRUCT_KW) => struct_def(p),
        Some(SyntaxKind::ENUM_KW) => enum_def(p),
        Some(SyntaxKind::TRAIT_KW) => trait_def(p),
        Some(SyntaxKind::TYPE_KW) => type_alias(p),
        Some(SyntaxKind::IMPL_KW) if !has_pub => impl_block(p),
        Some(SyntaxKind::EXTERN_KW) => {
            // Disambiguate: extern [ABI] { ... } vs extern [ABI] fn ...
            let mut ext = lookahead + 1;
            if p.peek(ext) == Some(SyntaxKind::STRING_LITERAL) {
                ext += 1;
            }
            if p.peek(ext) == Some(SyntaxKind::L_BRACE) && !has_pub {
                extern_block(p)
            } else {
                function_def(p)
            }
        }
        Some(SyntaxKind::USE_KW) => use_decl(p),
        Some(SyntaxKind::MODULE_KW) => module_def(p),
        Some(SyntaxKind::CONST_KW) => const_def(p),
        Some(SyntaxKind::STATIC_KW) => static_def(p),
        _ => {
            let err = p.error_at_current(
                "expected item (fn, gen, struct, enum, trait, type, impl, extern, use, module, const, or static)"
                    .to_string(),
            );
            Err(err)
        }
    }
}

/// Parse a module definition: `[attrs] [pub] module name { items }` or `module name;`
pub(crate) fn module_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    let m = p.start();

    // Optional attributes
    opt_attributes(p);

    // Optional visibility
    opt_visibility(p);

    // module keyword
    if let Err(e) = p.expect(SyntaxKind::MODULE_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Module name
    if let Err(e) = name(p) {
        m.abandon(p);
        return Err(e);
    }

    // Either a block body or a semicolon (module reference)
    if p.at(SyntaxKind::L_BRACE) {
        // Items block
        p.bump(); // {

        while !p.at(SyntaxKind::R_BRACE) && p.current().is_some() {
            if let Err(err) = item(p) {
                // Recover to next item in module
                p.recover_with_error(
                    err,
                    &[SyntaxKind::FN_KW, SyntaxKind::PUB_KW, SyntaxKind::R_BRACE],
                );
            }
        }

        if let Err(e) = p.expect(SyntaxKind::R_BRACE) {
            m.abandon(p);
            return Err(e);
        }
    } else {
        // Module reference: `module name;`
        stmt::eat_optional_semicolon(p);
    }

    Ok(m.complete(p, SyntaxKind::ModuleDef))
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
