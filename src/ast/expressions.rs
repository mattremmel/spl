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
}

impl StructExpr {
    pub fn path(&self) -> Option<Path> {
        child(&self.0)
    }

    pub fn fields(&self) -> impl Iterator<Item = StructExprField> {
        children(&self.0)
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
            .find(|t| matches!(t.kind(), SyntaxKind::BANG | SyntaxKind::MINUS | SyntaxKind::STAR))
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
}

impl MethodCallExpr {
    pub fn receiver(&self) -> Option<Expr> {
        child(&self.0)
    }

    pub fn name(&self) -> Option<NameRef> {
        child(&self.0)
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
    pub fn start(&self) -> Option<Expr> {
        children::<Expr>(&self.0).next()
    }

    pub fn end(&self) -> Option<Expr> {
        children::<Expr>(&self.0).nth(1)
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
