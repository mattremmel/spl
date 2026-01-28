//! Macros for reducing parser boilerplate.

/// Match on the parser's current token with simplified syntax.
/// Removes the need for `Some(SyntaxKind::...)` wrappers.
///
/// # Example
///
/// ```ignore
/// match_token!(p, {
///     INT_LITERAL | FLOAT_LITERAL => literal_expr(p),
///     IDENT => ident_expr(p),
///     _ => Ok(None),
/// })
/// ```
macro_rules! match_token {
    ($p:expr, { $($($kind:ident)|+ => $body:expr),+ $(, _ => $default:expr)? $(,)? }) => {
        match $p.current() {
            $($(Some(crate::syntax::SyntaxKind::$kind))|+ => $body,)+
            $(_ => $default)?
        }
    };
}
