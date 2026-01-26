//! Expression AST nodes.

use crate::ast::{Block, NameRef, Pat, Type, ast_enum, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

ast_node!(LiteralExpr);
ast_node!(PathExpr);
ast_node!(ParenExpr);
ast_node!(TupleExpr);
ast_node!(ArrayExpr);
ast_node!(CallExpr);
ast_node!(CallArg);
ast_node!(BinExpr);
ast_node!(PrefixExpr);
ast_node!(RefExpr);
ast_node!(FieldExpr);
ast_node!(IndexExpr);
ast_node!(SliceExpr);
ast_node!(IfExpr);
ast_node!(WhileExpr);
ast_node!(ForExpr);
ast_node!(LoopExpr);
ast_node!(BreakExpr);
ast_node!(ContinueExpr);
ast_node!(ReturnExpr);
ast_node!(BlockExpr);
ast_node!(CastExpr);
ast_node!(RangeExpr);
ast_node!(IsExpr);
ast_node!(MatchExpr);
ast_node!(MatchArm);
ast_node!(Path);
ast_node!(PathSegment);

ast_enum!(
    /// Expression enum - all expression variants.
    Expr {
        Literal(LiteralExpr),
        Path(PathExpr),
        Paren(ParenExpr),
        Tuple(TupleExpr),
        Array(ArrayExpr),
        Call(CallExpr),
        Binary(BinExpr),
        Prefix(PrefixExpr),
        Ref(RefExpr),
        Field(FieldExpr),
        Index(IndexExpr),
        Slice(SliceExpr),
        If(IfExpr),
        While(WhileExpr),
        For(ForExpr),
        Loop(LoopExpr),
        Break(BreakExpr),
        Continue(ContinueExpr),
        Return(ReturnExpr),
        Block(BlockExpr),
        Cast(CastExpr),
        Range(RangeExpr),
        Is(IsExpr),
        Match(MatchExpr),
    }
);

// === Typed accessors ===

impl LiteralExpr {
    pub fn token(&self) -> Option<SyntaxToken> {
        // Skip whitespace/trivia tokens to get the actual literal token
        self.0
            .children_with_tokens()
            .filter_map(|it| it.into_token())
            .find(|it| !it.kind().is_trivia())
    }
}

impl PathExpr {
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }
}

impl ParenExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl TupleExpr {
    pub fn exprs(&self) -> impl Iterator<Item = Expr> {
        children(&self.0)
    }
}

impl ArrayExpr {
    pub fn exprs(&self) -> impl Iterator<Item = Expr> {
        children(&self.0)
    }

    /// Check if this is array repeat syntax [elem; count] vs array literal [a, b, c].
    pub fn is_repeat(&self) -> bool {
        token(&self.0, SyntaxKind::SEMI).is_some()
    }
}

ast_node!(StructUpdateBase);

impl StructUpdateBase {
    /// Get the base expression in `..base`
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl CallExpr {
    /// Get the callee expression (function path, method access, or arbitrary expression).
    pub fn callee(&self) -> Option<Expr> {
        child(&self.0)
    }

    /// Get all arguments to this call.
    pub fn args(&self) -> impl Iterator<Item = CallArg> {
        children(&self.0)
    }

    /// Get the struct update base if present: `..base`
    pub fn update_base(&self) -> Option<StructUpdateBase> {
        child(&self.0)
    }
}

impl CallArg {
    /// Get the argument name if this is a named argument (`name: value`).
    /// Returns `None` for positional arguments.
    pub fn name(&self) -> Option<NameRef> {
        child(&self.0)
    }

    /// Get the argument name token directly (for named args where
    /// the name is stored as a raw IDENT token, not wrapped in NameRef).
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
    }

    /// Get the argument value expression.
    pub fn value(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl BinExpr {
    pub fn lhs(&self) -> Option<Expr> {
        children::<Expr>(&self.0).next()
    }

    pub fn rhs(&self) -> Option<Expr> {
        children::<Expr>(&self.0).nth(1)
    }

    pub fn op_token(&self) -> Option<SyntaxToken> {
        self.0
            .children_with_tokens()
            .filter_map(|it| it.into_token())
            .find(|t| {
                matches!(
                    t.kind(),
                    SyntaxKind::PLUS
                        | SyntaxKind::MINUS
                        | SyntaxKind::STAR
                        | SyntaxKind::SLASH
                        | SyntaxKind::PERCENT
                        | SyntaxKind::EQ_EQ
                        | SyntaxKind::NE
                        | SyntaxKind::LT
                        | SyntaxKind::GT
                        | SyntaxKind::LE
                        | SyntaxKind::GE
                        | SyntaxKind::AND_AND
                        | SyntaxKind::OR_OR
                        | SyntaxKind::EQ
                        | SyntaxKind::PLUS_EQ
                        | SyntaxKind::MINUS_EQ
                        | SyntaxKind::STAR_EQ
                        | SyntaxKind::SLASH_EQ
                        | SyntaxKind::PERCENT_EQ
                )
            })
    }
}

impl PrefixExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn op_token(&self) -> Option<SyntaxToken> {
        self.0
            .children_with_tokens()
            .filter_map(|it| it.into_token())
            .find(|t| {
                matches!(
                    t.kind(),
                    SyntaxKind::BANG | SyntaxKind::MINUS | SyntaxKind::STAR
                )
            })
    }
}

impl RefExpr {
    pub fn amp(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::AMP)
    }

    pub fn mut_kw(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MUT_KW)
    }

    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl FieldExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn name(&self) -> Option<NameRef> {
        child(&self.0)
    }

    /// Get the field name token directly (for field expressions where
    /// the field name is stored as a raw IDENT token, not wrapped in NameRef).
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
    }

    /// Get the tuple index token for tuple field access (e.g., `t.0`, `t.1`).
    pub fn tuple_index_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::INT_LITERAL)
    }
}

impl IndexExpr {
    pub fn base(&self) -> Option<Expr> {
        children::<Expr>(&self.0).next()
    }

    pub fn index(&self) -> Option<Expr> {
        children::<Expr>(&self.0).nth(1)
    }
}

impl SliceExpr {
    pub fn base(&self) -> Option<Expr> {
        children::<Expr>(&self.0).next()
    }

    pub fn start(&self) -> Option<Expr> {
        children::<Expr>(&self.0).nth(1)
    }

    pub fn end(&self) -> Option<Expr> {
        children::<Expr>(&self.0).nth(2)
    }
}

impl IfExpr {
    pub fn condition(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn then_branch(&self) -> Option<Block> {
        child(&self.0)
    }

    pub fn else_branch(&self) -> Option<Expr> {
        // The else branch can be:
        // 1. A BlockExpr (when the parser wraps it)
        // 2. Another IfExpr (for else-if chains)
        // 3. A Block directly (when parser doesn't wrap it)
        // We try Expr children first, then look for a direct Block child
        if let Some(expr) = children::<Expr>(&self.0).nth(1) {
            return Some(expr);
        }
        // Check for a direct Block after the then branch
        // The first Block is the then branch, the second is the else branch
        let blocks: Vec<_> = self.0.children().filter_map(Block::cast).collect();
        if blocks.len() >= 2 {
            // Wrap the else block in a BlockExpr-like wrapper
            // Since we can't create a BlockExpr, we need a different approach
            // Actually, let's just convert the Block to an expression type
            return None; // For now, return None and fix this properly
        }
        None
    }

    /// Get the else branch as a Block (if it's a simple else block, not else-if).
    pub fn else_block(&self) -> Option<Block> {
        // The first Block is the then branch, the second is the else branch
        self.0.children().filter_map(Block::cast).nth(1)
    }
}

impl WhileExpr {
    pub fn condition(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn body(&self) -> Option<Block> {
        child(&self.0)
    }
}

impl ForExpr {
    pub fn pat(&self) -> Option<Pat> {
        child(&self.0)
    }

    pub fn iterable(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn body(&self) -> Option<Block> {
        child(&self.0)
    }
}

impl LoopExpr {
    pub fn body(&self) -> Option<Block> {
        child(&self.0)
    }
}

impl BreakExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl ContinueExpr {
    // continue has no value
}

impl ReturnExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl BlockExpr {
    pub fn block(&self) -> Option<Block> {
        child(&self.0)
    }
}

impl CastExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.0)
    }
}

impl RangeExpr {
    /// Get the `..` token.
    pub fn op_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::DOT_DOT)
    }

    /// Get the start expression (before the `..` token), if any.
    pub fn start(&self) -> Option<Expr> {
        let dot_dot_offset = self.op_token()?.text_range().start();
        children::<Expr>(&self.0).find(|expr| expr.syntax().text_range().end() <= dot_dot_offset)
    }

    /// Get the end expression (after the `..` token), if any.
    pub fn end(&self) -> Option<Expr> {
        let dot_dot_offset = self.op_token()?.text_range().end();
        children::<Expr>(&self.0).find(|expr| expr.syntax().text_range().start() >= dot_dot_offset)
    }
}

impl Path {
    pub fn segments(&self) -> impl Iterator<Item = PathSegment> {
        children(&self.0)
    }
}

impl PathSegment {
    pub fn name(&self) -> Option<NameRef> {
        child(&self.0)
    }

    pub fn generic_args(&self) -> Option<crate::ast::GenericArgs> {
        child(&self.0)
    }
}

impl IsExpr {
    /// Get the left-hand side expression being matched.
    pub fn lhs(&self) -> Option<Expr> {
        children::<Expr>(&self.0).next()
    }

    /// Get the `is` keyword token.
    pub fn is_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IS_KW)
    }

    /// Check if this is an `is not` expression.
    pub fn is_negated(&self) -> bool {
        token(&self.0, SyntaxKind::NOT_KW).is_some()
    }

    /// Get the pattern being matched against.
    pub fn pattern(&self) -> Option<Pat> {
        child(&self.0)
    }
}

impl MatchExpr {
    /// Get the `match` keyword token.
    pub fn match_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::MATCH_KW)
    }

    /// Get the scrutinee expression (value being matched).
    pub fn scrutinee(&self) -> Option<Expr> {
        child(&self.0)
    }

    /// Get the match arms.
    pub fn arms(&self) -> impl Iterator<Item = MatchArm> {
        children(&self.0)
    }
}

impl MatchArm {
    /// Get the pattern for this arm.
    pub fn pattern(&self) -> Option<Pat> {
        child(&self.0)
    }

    /// Get the guard expression, if any (the `if condition` part).
    pub fn guard(&self) -> Option<Expr> {
        // Guard is the expression between `if` and `=>`
        // We need to find the expression that comes after `if` keyword
        let if_token = token(&self.0, SyntaxKind::IF_KW)?;
        let arrow_pos = token(&self.0, SyntaxKind::FAT_ARROW)?.text_range().start();
        children::<Expr>(&self.0).find(|expr| {
            let pos = expr.syntax().text_range().start();
            pos >= if_token.text_range().end() && pos < arrow_pos
        })
    }

    /// Get the body expression (the result if this arm matches).
    pub fn body(&self) -> Option<Expr> {
        // Body is the expression after `=>`
        let arrow_pos = token(&self.0, SyntaxKind::FAT_ARROW)?.text_range().end();
        children::<Expr>(&self.0).find(|expr| expr.syntax().text_range().start() >= arrow_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SourceFile;
    use crate::parser::parse;
    use rowan::ast::AstNode;

    /// Helper to parse source and find first expression of a specific kind.
    fn parse_expr<E: AstNode<Language = crate::syntax::Lang>>(source: &str) -> E {
        let parsed = parse(source);
        assert!(
            parsed.errors().is_empty(),
            "parse errors: {:?}",
            parsed.errors()
        );
        let root = parsed.syntax();
        root.descendants()
            .find_map(E::cast)
            .expect("expected expression not found")
    }

    /// Helper to parse and get first expression in function body.
    #[allow(dead_code)]
    fn parse_first_expr(source: &str) -> Expr {
        let parsed = parse(source);
        assert!(
            parsed.errors().is_empty(),
            "parse errors: {:?}",
            parsed.errors()
        );
        let source_file = SourceFile::cast(parsed.syntax()).expect("expected SourceFile");
        source_file
            .items()
            .filter_map(|item| match item {
                crate::ast::Item::Function(f) => f.body(),
                _ => None,
            })
            .next()
            .and_then(|block| block.tail_expr())
            .expect("expected expression in function body")
    }

    // =========================================================================
    // LiteralExpr Tests
    // =========================================================================

    #[test]
    fn literal_expr_int() {
        let lit: LiteralExpr = parse_expr("fn main() { 42 }");
        let tok = lit.token().expect("expected token");
        assert_eq!(tok.text(), "42");
    }

    #[test]
    fn literal_expr_float() {
        let lit: LiteralExpr = parse_expr("fn main() { 3.14 }");
        let tok = lit.token().expect("expected token");
        assert_eq!(tok.text(), "3.14");
    }

    #[test]
    fn literal_expr_bool() {
        let lit: LiteralExpr = parse_expr("fn main() { true }");
        let tok = lit.token().expect("expected token");
        assert_eq!(tok.text(), "true");
    }

    #[test]
    fn literal_expr_string() {
        let lit: LiteralExpr = parse_expr("fn main() { \"hello\" }");
        let tok = lit.token().expect("expected token");
        assert_eq!(tok.text(), "\"hello\"");
    }

    // =========================================================================
    // TupleExpr Tests
    // =========================================================================

    #[test]
    fn tuple_expr_empty() {
        let tuple: TupleExpr = parse_expr("fn main() { () }");
        assert_eq!(tuple.exprs().count(), 0);
    }

    #[test]
    fn tuple_expr_multiple() {
        let tuple: TupleExpr = parse_expr("fn main() { (1, 2, 3) }");
        let exprs: Vec<_> = tuple.exprs().collect();
        assert_eq!(exprs.len(), 3);
    }

    // =========================================================================
    // ArrayExpr Tests
    // =========================================================================

    #[test]
    fn array_expr_literal() {
        let arr: ArrayExpr = parse_expr("fn main() { [1, 2, 3] }");
        assert!(!arr.is_repeat());
        assert_eq!(arr.exprs().count(), 3);
    }

    #[test]
    fn array_expr_repeat() {
        let arr: ArrayExpr = parse_expr("fn main() { [0; 10] }");
        assert!(arr.is_repeat());
        // In repeat syntax, we have element and count as children
        assert_eq!(arr.exprs().count(), 2);
    }

    // =========================================================================
    // BinExpr Tests
    // =========================================================================

    #[test]
    fn bin_expr_add() {
        let bin: BinExpr = parse_expr("fn main() { 1 + 2 }");
        assert!(bin.lhs().is_some());
        assert!(bin.rhs().is_some());
        let op = bin.op_token().expect("expected operator");
        assert_eq!(op.kind(), SyntaxKind::PLUS);
    }

    #[test]
    fn bin_expr_comparison() {
        let bin: BinExpr = parse_expr("fn main() { x == y }");
        let op = bin.op_token().expect("expected operator");
        assert_eq!(op.kind(), SyntaxKind::EQ_EQ);
    }

    #[test]
    fn bin_expr_logical() {
        let bin: BinExpr = parse_expr("fn main() { a && b }");
        let op = bin.op_token().expect("expected operator");
        assert_eq!(op.kind(), SyntaxKind::AND_AND);
    }

    #[test]
    fn bin_expr_assignment() {
        let bin: BinExpr = parse_expr("fn main() { x = 5 }");
        let op = bin.op_token().expect("expected operator");
        assert_eq!(op.kind(), SyntaxKind::EQ);
    }

    #[test]
    fn bin_expr_compound_assignment() {
        let bin: BinExpr = parse_expr("fn main() { x += 1 }");
        let op = bin.op_token().expect("expected operator");
        assert_eq!(op.kind(), SyntaxKind::PLUS_EQ);
    }

    // =========================================================================
    // PrefixExpr Tests
    // =========================================================================

    #[test]
    fn prefix_expr_negation() {
        let prefix: PrefixExpr = parse_expr("fn main() { -x }");
        let op = prefix.op_token().expect("expected operator");
        assert_eq!(op.kind(), SyntaxKind::MINUS);
        assert!(prefix.expr().is_some());
    }

    #[test]
    fn prefix_expr_not() {
        let prefix: PrefixExpr = parse_expr("fn main() { !flag }");
        let op = prefix.op_token().expect("expected operator");
        assert_eq!(op.kind(), SyntaxKind::BANG);
    }

    #[test]
    fn prefix_expr_deref() {
        let prefix: PrefixExpr = parse_expr("fn main() { *ptr }");
        let op = prefix.op_token().expect("expected operator");
        assert_eq!(op.kind(), SyntaxKind::STAR);
    }

    // =========================================================================
    // RefExpr Tests
    // =========================================================================

    #[test]
    fn ref_expr_immutable() {
        let ref_expr: RefExpr = parse_expr("fn main() { &x }");
        assert!(ref_expr.amp().is_some());
        assert!(ref_expr.mut_kw().is_none());
        assert!(ref_expr.expr().is_some());
    }

    #[test]
    fn ref_expr_mutable() {
        let ref_expr: RefExpr = parse_expr("fn main() { &mut x }");
        assert!(ref_expr.amp().is_some());
        assert!(ref_expr.mut_kw().is_some());
    }

    // =========================================================================
    // IfExpr Tests
    // =========================================================================

    #[test]
    fn if_expr_no_else() {
        let if_expr: IfExpr = parse_expr("fn main() { if true { 1 } }");
        assert!(if_expr.condition().is_some());
        assert!(if_expr.then_branch().is_some());
        assert!(if_expr.else_branch().is_none());
        assert!(if_expr.else_block().is_none());
    }

    #[test]
    fn if_expr_with_else() {
        let if_expr: IfExpr = parse_expr("fn main() { if true { 1 } else { 2 } }");
        assert!(if_expr.condition().is_some());
        assert!(if_expr.then_branch().is_some());
        assert!(if_expr.else_block().is_some());
    }

    // =========================================================================
    // CallExpr Tests (function calls, struct instantiation, method calls)
    // =========================================================================

    #[test]
    fn call_expr_no_args() {
        let call: CallExpr = parse_expr("fn main() { foo() }");
        assert!(call.callee().is_some());
        assert_eq!(call.args().count(), 0);
    }

    #[test]
    fn call_expr_with_args() {
        let call: CallExpr = parse_expr("fn main() { foo(1, 2) }");
        assert!(call.callee().is_some());
        assert_eq!(call.args().count(), 2);
    }

    // =========================================================================
    // FieldExpr Tests
    // =========================================================================

    #[test]
    fn field_expr_on_call_result() {
        // In SPL, point.x is a Path, not a FieldExpr
        // FieldExpr is for accessing fields on expression results
        let field: FieldExpr =
            parse_expr("fn foo(): Point { Point(x: 1, y: 2) } fn main() { foo().x }");
        assert!(field.expr().is_some());
        assert!(field.name().is_some() || field.name_token().is_some());
    }

    #[test]
    fn field_expr_tuple_index() {
        // Tuple field access on an expression result
        let field: FieldExpr = parse_expr("fn foo(): (i32, i32) { (1, 2) } fn main() { foo().0 }");
        assert!(field.expr().is_some());
        assert!(field.tuple_index_token().is_some());
    }

    // =========================================================================
    // IndexExpr Tests
    // =========================================================================

    #[test]
    fn index_expr() {
        let index: IndexExpr = parse_expr("fn main() { arr[0] }");
        assert!(index.base().is_some());
        assert!(index.index().is_some());
    }

    // =========================================================================
    // RangeExpr Tests
    // =========================================================================

    #[test]
    fn range_expr_full() {
        let range: RangeExpr = parse_expr("fn main() { 1..10 }");
        assert!(range.op_token().is_some());
        assert!(range.start().is_some());
        assert!(range.end().is_some());
    }

    #[test]
    fn range_expr_no_start() {
        let range: RangeExpr = parse_expr("fn main() { ..10 }");
        assert!(range.op_token().is_some());
        assert!(range.start().is_none());
        assert!(range.end().is_some());
    }

    #[test]
    fn range_expr_no_end() {
        let range: RangeExpr = parse_expr("fn main() { 1.. }");
        assert!(range.op_token().is_some());
        assert!(range.start().is_some());
        assert!(range.end().is_none());
    }

    #[test]
    fn range_expr_unbounded() {
        let range: RangeExpr = parse_expr("fn main() { .. }");
        assert!(range.op_token().is_some());
        assert!(range.start().is_none());
        assert!(range.end().is_none());
    }

    // =========================================================================
    // CastExpr Tests
    // =========================================================================

    #[test]
    fn cast_expr() {
        let cast: CastExpr = parse_expr("fn main() { x as i64 }");
        assert!(cast.expr().is_some());
        assert!(cast.ty().is_some());
    }

    // =========================================================================
    // Loop/Control Flow Tests
    // =========================================================================

    #[test]
    fn loop_expr() {
        let loop_expr: LoopExpr = parse_expr("fn main() { loop { break } }");
        assert!(loop_expr.body().is_some());
    }

    #[test]
    fn while_expr() {
        let while_expr: WhileExpr = parse_expr("fn main() { while true { x } }");
        assert!(while_expr.condition().is_some());
        assert!(while_expr.body().is_some());
    }

    #[test]
    fn break_expr_no_value() {
        let brk: BreakExpr = parse_expr("fn main() { loop { break } }");
        assert!(brk.expr().is_none());
    }

    #[test]
    fn break_expr_with_value() {
        let brk: BreakExpr = parse_expr("fn main() { loop { break 42 } }");
        assert!(brk.expr().is_some());
    }

    #[test]
    fn return_expr_no_value() {
        let ret: ReturnExpr = parse_expr("fn main() { return }");
        assert!(ret.expr().is_none());
    }

    #[test]
    fn return_expr_with_value() {
        let ret: ReturnExpr = parse_expr("fn main() { return 42 }");
        assert!(ret.expr().is_some());
    }

    // =========================================================================
    // Path Tests
    // =========================================================================

    #[test]
    fn path_single_segment() {
        let path: Path = parse_expr("fn main() { foo }");
        assert_eq!(path.segments().count(), 1);
    }

    #[test]
    fn path_multiple_segments() {
        // SPL uses `.` for module paths, not `::`
        let path: Path = parse_expr("fn main() { std.io.Result }");
        assert_eq!(path.segments().count(), 3);
    }

    // =========================================================================
    // MatchExpr Tests
    // =========================================================================

    #[test]
    fn match_expr_basic() {
        let match_expr: MatchExpr = parse_expr("fn main() { match x { 1 => true, _ => false } }");
        assert!(match_expr.match_token().is_some());
        assert!(match_expr.scrutinee().is_some());
        assert_eq!(match_expr.arms().count(), 2);
    }

    #[test]
    fn match_arm_pattern_and_body() {
        let match_expr: MatchExpr = parse_expr("fn main() { match x { 1 => true } }");
        let arm = match_expr.arms().next().expect("expected arm");
        assert!(arm.pattern().is_some());
        assert!(arm.body().is_some());
        assert!(arm.guard().is_none());
    }

    // =========================================================================
    // IsExpr Tests
    // =========================================================================

    #[test]
    fn is_expr_positive() {
        let is_expr: IsExpr = parse_expr("fn main() { x is Some(y) }");
        assert!(is_expr.lhs().is_some());
        assert!(is_expr.is_token().is_some());
        assert!(!is_expr.is_negated());
        assert!(is_expr.pattern().is_some());
    }

    #[test]
    fn is_expr_negated() {
        let is_expr: IsExpr = parse_expr("fn main() { x is not None }");
        assert!(is_expr.is_negated());
    }

    // =========================================================================
    // Method Call Tests (unified under CallExpr)
    // =========================================================================

    #[test]
    fn method_call_chained() {
        // Method calls on identifiers are parsed as CallExpr with a PathExpr callee (multi-segment path)
        // obj.method() is CallExpr { callee: PathExpr { Path { obj, method } } }
        // Only method calls on expressions (like get().method()) produce FieldExpr callees
        let call: CallExpr = parse_expr("fn main() { obj.method() }");
        assert!(call.callee().is_some());
        // The callee should be a PathExpr with a multi-segment path
        if let Some(Expr::Path(path_expr)) = call.callee() {
            let path = path_expr.path().expect("expected path");
            assert_eq!(path.segments().count(), 2); // obj.method
        } else {
            panic!("expected callee to be a PathExpr");
        }
        assert_eq!(call.args().count(), 0);
    }

    #[test]
    fn method_call_on_expr() {
        // Method calls on expressions (not identifiers) produce FieldExpr callees
        // (1 + 2).method() is CallExpr { callee: FieldExpr { expr: ParenExpr(1 + 2), field: method } }
        let call: CallExpr = parse_expr("fn main() { (1 + 2).method() }");
        assert!(call.callee().is_some());
        if let Some(Expr::Field(field_expr)) = call.callee() {
            assert!(field_expr.expr().is_some());
            // Field name is stored as raw IDENT token, not wrapped in NameRef
            assert!(field_expr.name_token().is_some());
            assert_eq!(field_expr.name_token().unwrap().text(), "method");
        } else {
            panic!("expected callee to be a FieldExpr");
        }
        assert_eq!(call.args().count(), 0);
    }

    #[test]
    fn method_call_with_args_chained() {
        let call: CallExpr = parse_expr("fn main() { obj.method(1, 2) }");
        assert!(call.callee().is_some());
        assert_eq!(call.args().count(), 2);
    }
}
