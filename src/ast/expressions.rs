//! Expression AST nodes.

use crate::ast::{Block, NameRef, Pat, Type, ast_node, child, children, token};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;

ast_node!(LiteralExpr);
ast_node!(PathExpr);
ast_node!(ParenExpr);
ast_node!(TupleExpr);
ast_node!(ArrayExpr);
ast_node!(StructExpr);
ast_node!(StructExprField);
ast_node!(ApplyExpr);
ast_node!(ApplyArg);
ast_node!(BinExpr);
ast_node!(PrefixExpr);
ast_node!(RefExpr);
ast_node!(FieldExpr);
ast_node!(MethodCallExpr);
ast_node!(CallExpr);
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
ast_node!(ArgList);
ast_node!(Path);
ast_node!(PathSegment);

/// Expression enum - all expression variants.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Literal(LiteralExpr),
    Path(PathExpr),
    Paren(ParenExpr),
    Tuple(TupleExpr),
    Array(ArrayExpr),
    Struct(StructExpr),
    Apply(ApplyExpr),
    Binary(BinExpr),
    Prefix(PrefixExpr),
    Ref(RefExpr),
    Field(FieldExpr),
    MethodCall(MethodCallExpr),
    Call(CallExpr),
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

impl AstNode for Expr {
    type Language = crate::syntax::Lang;

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::LiteralExpr
                | SyntaxKind::PathExpr
                | SyntaxKind::ParenExpr
                | SyntaxKind::TupleExpr
                | SyntaxKind::ArrayExpr
                | SyntaxKind::StructExpr
                | SyntaxKind::ApplyExpr
                | SyntaxKind::BinExpr
                | SyntaxKind::PrefixExpr
                | SyntaxKind::RefExpr
                | SyntaxKind::FieldExpr
                | SyntaxKind::MethodCallExpr
                | SyntaxKind::CallExpr
                | SyntaxKind::IndexExpr
                | SyntaxKind::SliceExpr
                | SyntaxKind::IfExpr
                | SyntaxKind::WhileExpr
                | SyntaxKind::ForExpr
                | SyntaxKind::LoopExpr
                | SyntaxKind::BreakExpr
                | SyntaxKind::ContinueExpr
                | SyntaxKind::ReturnExpr
                | SyntaxKind::BlockExpr
                | SyntaxKind::CastExpr
                | SyntaxKind::RangeExpr
                | SyntaxKind::IsExpr
                | SyntaxKind::MatchExpr
        )
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::LiteralExpr => Some(Expr::Literal(LiteralExpr(node))),
            SyntaxKind::PathExpr => Some(Expr::Path(PathExpr(node))),
            SyntaxKind::ParenExpr => Some(Expr::Paren(ParenExpr(node))),
            SyntaxKind::TupleExpr => Some(Expr::Tuple(TupleExpr(node))),
            SyntaxKind::ArrayExpr => Some(Expr::Array(ArrayExpr(node))),
            SyntaxKind::StructExpr => Some(Expr::Struct(StructExpr(node))),
            SyntaxKind::ApplyExpr => Some(Expr::Apply(ApplyExpr(node))),
            SyntaxKind::BinExpr => Some(Expr::Binary(BinExpr(node))),
            SyntaxKind::PrefixExpr => Some(Expr::Prefix(PrefixExpr(node))),
            SyntaxKind::RefExpr => Some(Expr::Ref(RefExpr(node))),
            SyntaxKind::FieldExpr => Some(Expr::Field(FieldExpr(node))),
            SyntaxKind::MethodCallExpr => Some(Expr::MethodCall(MethodCallExpr(node))),
            SyntaxKind::CallExpr => Some(Expr::Call(CallExpr(node))),
            SyntaxKind::IndexExpr => Some(Expr::Index(IndexExpr(node))),
            SyntaxKind::SliceExpr => Some(Expr::Slice(SliceExpr(node))),
            SyntaxKind::IfExpr => Some(Expr::If(IfExpr(node))),
            SyntaxKind::WhileExpr => Some(Expr::While(WhileExpr(node))),
            SyntaxKind::ForExpr => Some(Expr::For(ForExpr(node))),
            SyntaxKind::LoopExpr => Some(Expr::Loop(LoopExpr(node))),
            SyntaxKind::BreakExpr => Some(Expr::Break(BreakExpr(node))),
            SyntaxKind::ContinueExpr => Some(Expr::Continue(ContinueExpr(node))),
            SyntaxKind::ReturnExpr => Some(Expr::Return(ReturnExpr(node))),
            SyntaxKind::BlockExpr => Some(Expr::Block(BlockExpr(node))),
            SyntaxKind::CastExpr => Some(Expr::Cast(CastExpr(node))),
            SyntaxKind::RangeExpr => Some(Expr::Range(RangeExpr(node))),
            SyntaxKind::IsExpr => Some(Expr::Is(IsExpr(node))),
            SyntaxKind::MatchExpr => Some(Expr::Match(MatchExpr(node))),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Expr::Literal(it) => it.syntax(),
            Expr::Path(it) => it.syntax(),
            Expr::Paren(it) => it.syntax(),
            Expr::Tuple(it) => it.syntax(),
            Expr::Array(it) => it.syntax(),
            Expr::Struct(it) => it.syntax(),
            Expr::Apply(it) => it.syntax(),
            Expr::Binary(it) => it.syntax(),
            Expr::Prefix(it) => it.syntax(),
            Expr::Ref(it) => it.syntax(),
            Expr::Field(it) => it.syntax(),
            Expr::MethodCall(it) => it.syntax(),
            Expr::Call(it) => it.syntax(),
            Expr::Index(it) => it.syntax(),
            Expr::Slice(it) => it.syntax(),
            Expr::If(it) => it.syntax(),
            Expr::While(it) => it.syntax(),
            Expr::For(it) => it.syntax(),
            Expr::Loop(it) => it.syntax(),
            Expr::Break(it) => it.syntax(),
            Expr::Continue(it) => it.syntax(),
            Expr::Return(it) => it.syntax(),
            Expr::Block(it) => it.syntax(),
            Expr::Cast(it) => it.syntax(),
            Expr::Range(it) => it.syntax(),
            Expr::Is(it) => it.syntax(),
            Expr::Match(it) => it.syntax(),
        }
    }
}

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

impl StructExpr {
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }

    pub fn fields(&self) -> impl Iterator<Item = StructExprField> {
        children(&self.0)
    }

    /// Get the struct update base expression: `..base` in `S { field: value, ..base }`
    pub fn update_base(&self) -> Option<StructUpdateBase> {
        child(&self.0)
    }
}

ast_node!(StructUpdateBase);

impl StructUpdateBase {
    /// Get the base expression in `..base`
    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl StructExprField {
    pub fn name(&self) -> Option<NameRef> {
        child(&self.0)
    }

    /// Get the field name token directly (for struct expression fields where
    /// the field name is stored as a raw IDENT token, not wrapped in NameRef).
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
    }

    pub fn expr(&self) -> Option<Expr> {
        child(&self.0)
    }
}

impl ApplyExpr {
    /// Get the path being applied (the callee - could be a function or struct).
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }

    /// Get all arguments to this application.
    pub fn args(&self) -> impl Iterator<Item = ApplyArg> {
        children(&self.0)
    }

    /// Get the struct update base if present: `..base`
    pub fn update_base(&self) -> Option<StructUpdateBase> {
        child(&self.0)
    }
}

impl ApplyArg {
    /// Get the argument name if this is a named argument (`name = value`).
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

impl MethodCallExpr {
    pub fn receiver(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn name(&self) -> Option<NameRef> {
        child(&self.0)
    }

    /// Get the method name token directly (for method calls where
    /// the method name is stored as a raw IDENT token, not wrapped in NameRef).
    pub fn name_token(&self) -> Option<SyntaxToken> {
        token(&self.0, SyntaxKind::IDENT)
    }

    pub fn arg_list(&self) -> Option<ArgList> {
        child(&self.0)
    }
}

impl CallExpr {
    pub fn callee(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn arg_list(&self) -> Option<ArgList> {
        child(&self.0)
    }
}

impl ArgList {
    pub fn args(&self) -> impl Iterator<Item = Expr> {
        children(&self.0)
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
            pos > if_token.text_range().end() && pos < arrow_pos
        })
    }

    /// Get the body expression (the result if this arm matches).
    pub fn body(&self) -> Option<Expr> {
        // Body is the expression after `=>`
        let arrow_pos = token(&self.0, SyntaxKind::FAT_ARROW)?.text_range().end();
        children::<Expr>(&self.0).find(|expr| expr.syntax().text_range().start() >= arrow_pos)
    }
}
