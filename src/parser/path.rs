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

/// Parse a single path segment: `ident [<T, ...>]`
fn path_segment(p: &mut Parser<'_>, allow_generics: bool) -> Result<CompletedMarker, ParseError> {
    let m = p.start();
    name_ref(p)?;
    if allow_generics && p.at(SyntaxKind::LT) {
        super::stmt::generic_args(p)?;
    }
    Ok(m.complete(p, SyntaxKind::PathSegment))
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
