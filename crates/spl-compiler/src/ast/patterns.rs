//! Pattern AST nodes.

use crate::ast::{Name, NameRef, Path, ast_enum, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

ast_node!(IdentPat);
ast_node!(WildcardPat);
ast_node!(LiteralPat);
ast_node!(RangePat);
ast_node!(TuplePat);
ast_node!(SlicePat);
ast_node!(StructPat);
ast_node!(RefPat);
ast_node!(RestPat);
ast_node!(StructPatField);

ast_enum!(
    /// Pattern enum - all pattern variants.
    Pat {
        Ident(IdentPat),
        Wildcard(WildcardPat),
        Literal(LiteralPat),
        Range(RangePat),
        Tuple(TuplePat),
        Slice(SlicePat),
        Struct(StructPat),
        Ref(RefPat),
        Rest(RestPat),
    }
);

// === Typed accessors ===

impl IdentPat {
    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }

    pub fn name(&self) -> Option<Name> {
        child(&self.0)
    }
}

impl WildcardPat {
    // Wildcard is just `_`
}

impl LiteralPat {
    pub fn token(&self) -> Option<SyntaxToken> {
        // Skip whitespace/trivia tokens to get the actual literal token
        self.0
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .find(|it| !it.kind().is_trivia())
    }
}

impl RangePat {
    /// Get the start token of the range pattern (the literal before `..`).
    pub fn start(&self) -> Option<SyntaxToken> {
        self.0
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .find(|t| {
                matches!(
                    t.kind(),
                    SyntaxKind::INT_LITERAL | SyntaxKind::FLOAT_LITERAL | SyntaxKind::CHAR_LITERAL
                )
            })
    }

    /// Get the end token of the range pattern (the literal after `..`), if present.
    pub fn end(&self) -> Option<SyntaxToken> {
        let mut found_dot_dot = false;
        for child in self.0.children_with_tokens() {
            if let Some(token) = child.into_token() {
                if token.kind() == SyntaxKind::DOT_DOT {
                    found_dot_dot = true;
                } else if found_dot_dot
                    && matches!(
                        token.kind(),
                        SyntaxKind::INT_LITERAL
                            | SyntaxKind::FLOAT_LITERAL
                            | SyntaxKind::CHAR_LITERAL
                    )
                {
                    return Some(token);
                }
            }
        }
        None
    }
}

impl TuplePat {
    pub fn patterns(&self) -> impl Iterator<Item = Pat> {
        children(&self.0)
    }
}

impl SlicePat {
    pub fn patterns(&self) -> impl Iterator<Item = Pat> {
        children(&self.0)
    }
}

impl StructPat {
    /// Get the struct type as a path (always present, even for simple `Point { }`).
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }

    pub fn fields(&self) -> impl Iterator<Item = StructPatField> {
        children(&self.0)
    }

    pub fn rest(&self) -> Option<RestPat> {
        child(&self.0)
    }
}

impl StructPatField {
    /// Get the field name (always wrapped in `NameRef`).
    pub fn name(&self) -> Option<NameRef> {
        child(&self.0)
    }

    /// Get the nested pattern (for `field: pattern` syntax).
    /// Returns None for shorthand syntax like `{ x }`.
    pub fn pat(&self) -> Option<Pat> {
        child(&self.0)
    }
}

impl RefPat {
    pub fn amp(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::AMP)
    }

    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }

    pub fn pat(&self) -> Option<Pat> {
        child(&self.0)
    }
}

impl RestPat {
    // Rest is just `..`
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use rowan::ast::AstNode;

    /// Helper to parse source and find first pattern of a specific kind.
    fn parse_pat<P: AstNode<Language = crate::syntax::Lang>>(source: &str) -> P {
        let parsed = parse(source);
        assert!(
            parsed.errors().is_empty(),
            "parse errors: {:?}",
            parsed.errors()
        );
        let root = parsed.syntax();
        root.descendants()
            .find_map(P::cast)
            .expect("expected pattern not found")
    }

    // =========================================================================
    // IdentPat Tests
    // =========================================================================

    #[test]
    fn ident_pat_simple() {
        let pat: IdentPat = parse_pat("fn main() { let x = 1; }");
        assert!(pat.name().is_some());
        // In SPL, `mut` is on the LetStmt, not the IdentPat
        assert!(pat.mut_kw().is_none());
    }

    #[test]
    fn ident_pat_in_let_mut() {
        // In SPL, `mut` is part of LetStmt, not IdentPat
        // The pattern itself is just an identifier without mut
        let pat: IdentPat = parse_pat("fn main() { let mut x = 1; }");
        assert!(pat.name().is_some());
        // mut is NOT on the pattern in SPL
        assert!(pat.mut_kw().is_none());
    }

    // =========================================================================
    // WildcardPat Tests
    // =========================================================================

    #[test]
    fn wildcard_pat() {
        let pat: WildcardPat = parse_pat("fn main() { let _ = 1; }");
        // Wildcard just exists, no special accessors
        assert!(pat.syntax().kind() == crate::syntax::SyntaxKind::WildcardPat);
    }

    // =========================================================================
    // TuplePat Tests
    // =========================================================================

    #[test]
    fn tuple_pat_empty() {
        let pat: TuplePat = parse_pat("fn main() { let () = unit(); }");
        assert_eq!(pat.patterns().count(), 0);
    }

    #[test]
    fn tuple_pat_multiple() {
        let pat: TuplePat = parse_pat("fn main() { let (a, b, c) = tuple; }");
        assert_eq!(pat.patterns().count(), 3);
    }

    // =========================================================================
    // StructPat Tests
    // =========================================================================

    #[test]
    fn struct_pat_with_fields() {
        // SPL uses parentheses syntax: Point(x: x, y: y)
        let pat: StructPat = parse_pat("fn main() { let Point(x: x, y: y) = p; }");
        assert!(pat.path().is_some());
        assert_eq!(pat.fields().count(), 2);
        assert!(pat.rest().is_none());
    }

    #[test]
    fn struct_pat_with_rest() {
        let pat: StructPat = parse_pat("fn main() { let Point(x: x, ..) = p; }");
        assert!(pat.path().is_some());
        assert_eq!(pat.fields().count(), 1);
        assert!(pat.rest().is_some());
    }

    #[test]
    fn struct_pat_field_with_rename() {
        // In SPL, struct patterns are always explicit: Point(x: a)
        let pat: StructPat = parse_pat("fn main() { let Point(x: a) = p; }");
        let field = pat.fields().next().expect("expected field");
        assert!(field.name().is_some());
        // Has a nested pattern (the binding `a`)
        assert!(field.pat().is_some());
    }

    // =========================================================================
    // RefPat Tests
    // =========================================================================

    #[test]
    fn ref_pat_immutable() {
        let pat: RefPat = parse_pat("fn main() { let &x = r; }");
        assert!(pat.amp().is_some());
        assert!(pat.mut_kw().is_none());
        assert!(pat.pat().is_some());
    }

    #[test]
    fn ref_pat_mutable() {
        let pat: RefPat = parse_pat("fn main() { let &mut x = r; }");
        assert!(pat.amp().is_some());
        assert!(pat.mut_kw().is_some());
    }

    // =========================================================================
    // SlicePat Tests
    // =========================================================================

    #[test]
    fn slice_pat_elements() {
        let pat: SlicePat = parse_pat("fn main() { let [a, b] = arr; }");
        assert_eq!(pat.patterns().count(), 2);
    }

    // =========================================================================
    // RangePat Tests
    // =========================================================================

    #[test]
    fn range_pat_full() {
        // Range patterns are parsed as literal tokens
        let pat: RangePat = parse_pat("fn main() { let 1..10 = x; }");
        let start = pat.start().expect("expected start token");
        assert_eq!(start.text(), "1");
        let end = pat.end().expect("expected end token");
        assert_eq!(end.text(), "10");
    }

    #[test]
    fn range_pat_open_end() {
        let pat: RangePat = parse_pat("fn main() { let 1.. = x; }");
        assert!(pat.start().is_some());
        assert!(pat.end().is_none()); // Open-ended range
    }

    // =========================================================================
    // LiteralPat Tests
    // =========================================================================

    #[test]
    fn literal_pat_int() {
        let pat: LiteralPat = parse_pat("fn main() { match x { 42 => {} } }");
        let tok = pat.token().expect("expected token");
        assert_eq!(tok.text(), "42");
    }

    #[test]
    fn literal_pat_bool() {
        let pat: LiteralPat = parse_pat("fn main() { match x { true => {} } }");
        let tok = pat.token().expect("expected token");
        assert_eq!(tok.text(), "true");
    }
}
