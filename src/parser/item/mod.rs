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

/// Parse a function definition: `[pub] fn name(params) [: Type] [where ...] { body }`
///
/// Return type syntax: `fn foo(): i32 where T { ... }`
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
/// Syntax: `[pub] struct Name(fields) [where ...]` or `[pub] struct Name;`
pub(crate) fn struct_def(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

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

/// Parse a type alias: `[pub] type Name = Type [where ...];`
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
/// Syntax: `impl Type [where T, U] { items }`
pub(crate) fn impl_block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

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

/// Parse an extern function declaration: `[pub] fn name(params) [: Type];`
fn extern_fn(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

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

/// Parse a use declaration: `[pub] use path[.{tree}|.*|as name];`
///
/// Examples:
/// - `use std.vec.Vec;`
/// - `use std.collections.HashMap as Map;`
/// - `use std.prelude.*;`
/// - `use std.io.{Read, Write};`
/// - `use std.{vec.Vec, io.{Read, Write}};`
pub(crate) fn use_decl(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    let m = p.start();

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

/// Parse a top-level item (function, struct, type alias, impl block, extern block, or use decl).
pub(crate) fn item(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::parser::ParseError> {
    // Check for visibility modifier and calculate lookahead
    let has_pub = p.at(SyntaxKind::PUB_KW);
    let lookahead = visibility_lookahead(p);

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
