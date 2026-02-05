//! Concrete syntax tree infrastructure for SPL.

use spl_lexer::Token;

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
    GEN_KW,
    STRUCT_KW,
    ENUM_KW,
    TRAIT_KW,
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
    YIELD_KW,
    THROW_KW,
    THROWS_KW,
    AS_KW,
    TRUE_KW,
    FALSE_KW,
    PUB_KW,
    SELF_TYPE_KW,
    SELF_VALUE_KW,
    SUPER_KW,
    WHERE_KW,
    IS_KW,
    MATCH_KW,
    EXTERN_KW,
    CONST_KW,
    STATIC_KW,
    UNSAFE_KW,
    USE_KW,
    MODULE_KW,

    // Operators - Arithmetic
    PLUS,
    MINUS,
    STAR,
    STAR_STAR,
    SLASH,
    PERCENT,

    // Operators - Comparison
    EQ_EQ,
    NE,
    LT,
    GT,
    LE,
    GE,
    SHL,
    SHR,

    // Operators - Logical
    AND_AND,
    OR_OR,
    BANG,

    // Operators - Bitwise
    PIPE,
    CARET,
    TILDE,
    AMP,

    // Operators - Assignment
    EQ,
    PLUS_EQ,
    MINUS_EQ,
    STAR_EQ,
    STAR_STAR_EQ,
    SLASH_EQ,
    PERCENT_EQ,
    AMP_EQ,
    PIPE_EQ,
    CARET_EQ,
    SHL_EQ,
    SHR_EQ,

    // Operators - Other
    FAT_ARROW,
    DOT_DOT,
    DOT_DOT_EQ,
    ELLIPSIS,
    DOT,
    QUESTION,
    QUESTION_DOT,
    QUESTION_QUESTION,
    DOLLAR,
    HASH,
    AT,

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

    // Label marker
    TICK,

    // Literals & Identifiers
    INT_LITERAL,
    FLOAT_LITERAL,
    STRING_LITERAL,
    RAW_STRING_LITERAL,
    BYTE_STRING_LITERAL,
    RAW_BYTE_STRING_LITERAL,
    C_STRING_LITERAL,
    CHAR_LITERAL,
    BYTE_CHAR_LITERAL,
    IDENT,

    // Trivia & Error
    WHITESPACE,
    NEWLINE,
    COMMENT,
    ERROR,

    // === COMPOSITE NODES ===
    // Root
    SourceFile,

    // Items
    FunctionDef,
    StructDef,
    EnumDef,
    VariantList,
    Variant,
    TraitDef,
    TraitItem,
    AssociatedType,
    ImplBlock,
    TypeAlias,
    ExternBlock,
    ExternFn,
    UseDecl,
    UseTree,
    UseTreeList,
    ModuleDef,
    ConstDef,
    StaticDef,
    GeneratorDef,
    ParamList,
    Param,
    SelfParam,
    VariadicParam,
    GenericParam,
    GenericParams,
    GenericArgs,
    TypeArg,
    FieldList,
    FieldDef,
    WhereClause,
    ThrowsClause,
    TypeBound,
    LabelSpec,
    /// Label for labeled loops/blocks: 'name:
    Label,

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
    StructUpdateBase,
    CallExpr,
    CallArg,
    BinExpr,
    PrefixExpr,
    RefExpr,
    FieldExpr,
    IndexExpr,
    SliceExpr,
    IfExpr,
    WhileExpr,
    ForExpr,
    LoopExpr,
    BreakExpr,
    ContinueExpr,
    ReturnExpr,
    YieldExpr,
    BlockExpr,
    CastExpr,
    RangeExpr,
    IsExpr,
    MatchExpr,
    MatchArm,
    EnumShorthandExpr,
    TryExpr,
    OptionalFieldExpr,
    DollarExpr,
    ClosureExpr,
    CaptureList,
    Capture,
    ClosureParams,
    ClosureParam,
    UnsafeExpr,
    ThrowExpr,

    // Types
    RefType,
    ArrayType,
    SliceType,
    TupleType,
    TupleTypeElement,
    FnPtrType,
    PathType,
    NeverType,
    OptionalType,

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
    EnumShorthandPat,
    OrPat,
    GroupedPat,

    // Names & Paths
    Name,
    NameRef,
    Path,
    PathSegment,
    Lifetime,
    LifetimeParams,
    Visibility,

    // Attributes
    Attribute,
    InnerAttribute,
    AttrPath,
    AttrInput,
    AttrArg,

    #[doc(hidden)]
    __LAST,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(self, Self::WHITESPACE | Self::NEWLINE | Self::COMMENT)
    }
}

impl From<Token> for SyntaxKind {
    fn from(token: Token) -> Self {
        match token {
            // Trivia
            Token::Whitespace => Self::WHITESPACE,
            Token::Newline => Self::NEWLINE,
            Token::LineComment | Token::BlockComment => Self::COMMENT,
            // Keywords
            Token::Let => Self::LET_KW,
            Token::Mut => Self::MUT_KW,
            Token::Fn => Self::FN_KW,
            Token::Gen => Self::GEN_KW,
            Token::Struct => Self::STRUCT_KW,
            Token::Enum => Self::ENUM_KW,
            Token::Trait => Self::TRAIT_KW,
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
            Token::Yield => Self::YIELD_KW,
            Token::Throw => Self::THROW_KW,
            Token::Throws => Self::THROWS_KW,
            Token::As => Self::AS_KW,
            Token::True => Self::TRUE_KW,
            Token::False => Self::FALSE_KW,
            Token::Pub => Self::PUB_KW,
            Token::SelfType => Self::SELF_TYPE_KW,
            Token::SelfValue => Self::SELF_VALUE_KW,
            Token::Super => Self::SUPER_KW,
            Token::Where => Self::WHERE_KW,
            Token::Is => Self::IS_KW,
            Token::Match => Self::MATCH_KW,
            Token::Extern => Self::EXTERN_KW,
            Token::Const => Self::CONST_KW,
            Token::Static => Self::STATIC_KW,
            Token::Unsafe => Self::UNSAFE_KW,
            Token::Use => Self::USE_KW,
            Token::Module => Self::MODULE_KW,
            // Operators - Arithmetic
            Token::Plus => Self::PLUS,
            Token::Minus => Self::MINUS,
            Token::Star => Self::STAR,
            Token::StarStar => Self::STAR_STAR,
            Token::Slash => Self::SLASH,
            Token::Percent => Self::PERCENT,
            // Operators - Comparison
            Token::EqEq => Self::EQ_EQ,
            Token::Ne => Self::NE,
            Token::Lt => Self::LT,
            Token::Gt => Self::GT,
            Token::Le => Self::LE,
            Token::Ge => Self::GE,
            Token::Shl => Self::SHL,
            Token::Shr => Self::SHR,
            // Operators - Logical
            Token::AndAnd => Self::AND_AND,
            Token::OrOr => Self::OR_OR,
            Token::Bang => Self::BANG,
            // Operators - Bitwise
            Token::Pipe => Self::PIPE,
            Token::Caret => Self::CARET,
            Token::Tilde => Self::TILDE,
            Token::Amp => Self::AMP,
            // Operators - Assignment
            Token::Eq => Self::EQ,
            Token::PlusEq => Self::PLUS_EQ,
            Token::MinusEq => Self::MINUS_EQ,
            Token::StarEq => Self::STAR_EQ,
            Token::StarStarEq => Self::STAR_STAR_EQ,
            Token::SlashEq => Self::SLASH_EQ,
            Token::PercentEq => Self::PERCENT_EQ,
            Token::AmpEq => Self::AMP_EQ,
            Token::PipeEq => Self::PIPE_EQ,
            Token::CaretEq => Self::CARET_EQ,
            Token::ShlEq => Self::SHL_EQ,
            Token::ShrEq => Self::SHR_EQ,
            // Operators - Other
            Token::FatArrow => Self::FAT_ARROW,
            Token::DotDot => Self::DOT_DOT,
            Token::DotDotEq => Self::DOT_DOT_EQ,
            Token::Ellipsis => Self::ELLIPSIS,
            Token::Dot => Self::DOT,
            Token::Question => Self::QUESTION,
            Token::QuestionDot => Self::QUESTION_DOT,
            Token::QuestionQuestion => Self::QUESTION_QUESTION,
            Token::Dollar => Self::DOLLAR,
            Token::Hash => Self::HASH,
            Token::At => Self::AT,
            // Delimiters
            Token::LParen => Self::L_PAREN,
            Token::RParen => Self::R_PAREN,
            Token::LBrace => Self::L_BRACE,
            Token::RBrace => Self::R_BRACE,
            Token::LBracket => Self::L_BRACKET,
            Token::RBracket => Self::R_BRACKET,
            Token::Semi => Self::SEMI,
            Token::Colon => Self::COLON,
            Token::Comma => Self::COMMA,
            Token::Tick => Self::TICK,
            // Literals
            Token::Integer => Self::INT_LITERAL,
            Token::Float => Self::FLOAT_LITERAL,
            Token::String => Self::STRING_LITERAL,
            Token::RawString => Self::RAW_STRING_LITERAL,
            Token::ByteString => Self::BYTE_STRING_LITERAL,
            Token::RawByteString => Self::RAW_BYTE_STRING_LITERAL,
            Token::CString => Self::C_STRING_LITERAL,
            Token::Char => Self::CHAR_LITERAL,
            Token::ByteChar => Self::BYTE_CHAR_LITERAL,
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
        // SAFETY: This transmute is safe because:
        // 1. SyntaxKind is #[repr(u16)], guaranteeing the same memory layout as u16
        // 2. Rust guarantees repr(u16) enum variants are contiguous starting from 0
        //    when no explicit discriminants are assigned
        // 3. __LAST is the final sentinel variant, so any value < __LAST corresponds
        //    to a valid SyntaxKind variant
        // 4. The assert above ensures we never transmute an out-of-bounds value
        // 5. This is the standard pattern from rowan's official examples and is used
        //    by rust-analyzer: https://github.com/rust-analyzer/rowan/blob/master/examples/s_expressions.rs
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
        // Test token variants map correctly
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
