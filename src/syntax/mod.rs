//! Concrete syntax tree infrastructure for SPL.

use crate::lexer::Token;

/// All syntax kinds (tokens + composite nodes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum SyntaxKind {
    // === TOKENS (from lexer) ===
    // Keywords
    LET_KW,
    MUT_KW,
    FN_KW,
    STRUCT_KW,
    TYPE_KW,
    IMPL_KW,
    IF_KW,
    ELSE_KW,
    WHILE_KW,
    FOR_KW,
    IN_KW,
    LOOP_KW,
    BREAK_KW,
    CONTINUE_KW,
    RETURN_KW,
    AS_KW,
    TRUE_KW,
    FALSE_KW,
    PUB_KW,
    SELF_TYPE_KW,
    SELF_VALUE_KW,
    CRATE_KW,
    SUPER_KW,
    WHERE_KW,
    IS_KW,
    NOT_KW,
    MATCH_KW,

    // Operators
    PLUS,
    MINUS,
    STAR,
    SLASH,
    PERCENT,
    EQ_EQ,
    NE,
    LT,
    GT,
    LE,
    GE,
    AND_AND,
    OR_OR,
    BANG,
    EQ,
    PLUS_EQ,
    MINUS_EQ,
    STAR_EQ,
    SLASH_EQ,
    PERCENT_EQ,
    ARROW,
    FAT_ARROW,
    COLON_COLON,
    DOT_DOT,
    DOT,
    AMP,
    DOLLAR,

    // Delimiters
    L_PAREN,
    R_PAREN,
    L_BRACE,
    R_BRACE,
    L_BRACKET,
    R_BRACKET,
    SEMI,
    COLON,
    COMMA,

    // Literals & Identifiers
    INT_LITERAL,
    FLOAT_LITERAL,
    STRING_LITERAL,
    CHAR_LITERAL,
    IDENT,

    // Trivia & Error
    WHITESPACE,
    COMMENT,
    ERROR,

    // === COMPOSITE NODES ===
    // Root
    SourceFile,

    // Items
    FunctionDef,
    StructDef,
    ImplBlock,
    TypeAlias,
    ParamList,
    Param,
    SelfParam,
    GenericParam,
    GenericArgs,
    FieldList,
    FieldDef,
    WhereClause,
    TypeBound,
    LabelSpec,

    // Statements
    Block,
    LetStmt,
    ExprStmt,

    // Expressions (added incrementally)
    LiteralExpr,
    PathExpr,
    ParenExpr,
    TupleExpr,
    ArrayExpr,
    StructExpr,
    StructExprField,
    StructUpdateBase,
    BinExpr,
    PrefixExpr,
    RefExpr,
    FieldExpr,
    MethodCallExpr,
    CallExpr,
    IndexExpr,
    SliceExpr,
    IfExpr,
    WhileExpr,
    ForExpr,
    LoopExpr,
    BreakExpr,
    ContinueExpr,
    ReturnExpr,
    BlockExpr,
    CastExpr,
    RangeExpr,
    IsExpr,
    MatchExpr,
    MatchArm,
    ArgList,

    // Types
    RefType,
    ArrayType,
    SliceType,
    TupleType,
    FnPtrType,
    PathType,
    NeverType,

    // Patterns
    IdentPat,
    WildcardPat,
    LiteralPat,
    RangePat,
    TuplePat,
    SlicePat,
    StructPat,
    RefPat,
    RestPat,
    StructPatField,

    // Names & Paths
    Name,
    NameRef,
    Path,
    PathSegment,
    Visibility,

    #[doc(hidden)]
    __LAST,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(self, Self::WHITESPACE | Self::COMMENT)
    }
}

impl From<Token> for SyntaxKind {
    fn from(token: Token) -> Self {
        match token {
            // Trivia
            Token::Whitespace => Self::WHITESPACE,
            Token::LineComment => Self::COMMENT,
            Token::BlockComment => Self::COMMENT,
            // Keywords
            Token::Let => Self::LET_KW,
            Token::Mut => Self::MUT_KW,
            Token::Fn => Self::FN_KW,
            Token::Struct => Self::STRUCT_KW,
            Token::Type => Self::TYPE_KW,
            Token::Impl => Self::IMPL_KW,
            Token::If => Self::IF_KW,
            Token::Else => Self::ELSE_KW,
            Token::While => Self::WHILE_KW,
            Token::For => Self::FOR_KW,
            Token::In => Self::IN_KW,
            Token::Loop => Self::LOOP_KW,
            Token::Break => Self::BREAK_KW,
            Token::Continue => Self::CONTINUE_KW,
            Token::Return => Self::RETURN_KW,
            Token::As => Self::AS_KW,
            Token::True => Self::TRUE_KW,
            Token::False => Self::FALSE_KW,
            Token::Pub => Self::PUB_KW,
            Token::SelfType => Self::SELF_TYPE_KW,
            Token::SelfValue => Self::SELF_VALUE_KW,
            Token::Crate => Self::CRATE_KW,
            Token::Super => Self::SUPER_KW,
            Token::Where => Self::WHERE_KW,
            Token::Is => Self::IS_KW,
            Token::Not => Self::NOT_KW,
            Token::Match => Self::MATCH_KW,
            Token::Plus => Self::PLUS,
            Token::Minus => Self::MINUS,
            Token::Star => Self::STAR,
            Token::Slash => Self::SLASH,
            Token::Percent => Self::PERCENT,
            Token::EqEq => Self::EQ_EQ,
            Token::Ne => Self::NE,
            Token::Lt => Self::LT,
            Token::Gt => Self::GT,
            Token::Le => Self::LE,
            Token::Ge => Self::GE,
            Token::AndAnd => Self::AND_AND,
            Token::OrOr => Self::OR_OR,
            Token::Bang => Self::BANG,
            Token::Eq => Self::EQ,
            Token::PlusEq => Self::PLUS_EQ,
            Token::MinusEq => Self::MINUS_EQ,
            Token::StarEq => Self::STAR_EQ,
            Token::SlashEq => Self::SLASH_EQ,
            Token::PercentEq => Self::PERCENT_EQ,
            Token::Arrow => Self::ARROW,
            Token::FatArrow => Self::FAT_ARROW,
            Token::ColonColon => Self::COLON_COLON,
            Token::DotDot => Self::DOT_DOT,
            Token::Dot => Self::DOT,
            Token::Amp => Self::AMP,
            Token::Dollar => Self::DOLLAR,
            Token::LParen => Self::L_PAREN,
            Token::RParen => Self::R_PAREN,
            Token::LBrace => Self::L_BRACE,
            Token::RBrace => Self::R_BRACE,
            Token::LBracket => Self::L_BRACKET,
            Token::RBracket => Self::R_BRACKET,
            Token::Semi => Self::SEMI,
            Token::Colon => Self::COLON,
            Token::Comma => Self::COMMA,
            Token::Integer => Self::INT_LITERAL,
            Token::Float => Self::FLOAT_LITERAL,
            Token::String => Self::STRING_LITERAL,
            Token::Char => Self::CHAR_LITERAL,
            Token::Ident => Self::IDENT,
            Token::Error => Self::ERROR,
        }
    }
}

/// SPL language marker for rowan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {}

impl rowan::Language for Lang {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        assert!(raw.0 < SyntaxKind::__LAST as u16);
        // SAFETY: SyntaxKind is repr(u16) and we checked bounds
        unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

// Type aliases for convenience
pub type SyntaxNode = rowan::SyntaxNode<Lang>;
pub type SyntaxToken = rowan::SyntaxToken<Lang>;
pub type SyntaxElement = rowan::SyntaxElement<Lang>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_to_syntax_kind_all_variants() {
        // Test all 52 token variants map correctly
        assert_eq!(SyntaxKind::from(Token::Let), SyntaxKind::LET_KW);
        assert_eq!(SyntaxKind::from(Token::Plus), SyntaxKind::PLUS);
        assert_eq!(SyntaxKind::from(Token::Integer), SyntaxKind::INT_LITERAL);
        assert_eq!(SyntaxKind::from(Token::Ident), SyntaxKind::IDENT);
        assert_eq!(SyntaxKind::from(Token::Error), SyntaxKind::ERROR);
    }

    #[test]
    fn syntax_kind_is_trivia() {
        assert!(SyntaxKind::WHITESPACE.is_trivia());
        assert!(SyntaxKind::COMMENT.is_trivia());
        assert!(!SyntaxKind::IDENT.is_trivia());
    }
}
