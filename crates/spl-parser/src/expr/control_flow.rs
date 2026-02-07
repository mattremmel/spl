//! Control flow expression parsing: if, while, for, loop, break, continue, return.

use crate::{CompletedMarker, Parser};
use spl_syntax::SyntaxKind;

use super::{expr, expr_no_struct};

/// Try to parse a label: 'name:
/// Returns true if a label was parsed.
fn try_parse_label(p: &mut Parser<'_>) -> bool {
    if !p.at(SyntaxKind::TICK) {
        return false;
    }

    let m = p.start();
    p.bump(); // consume '

    // Parse the label name
    let _ = crate::item::name(p);

    // Expect colon
    let _ = p.expect(SyntaxKind::COLON);

    m.complete(p, SyntaxKind::Label);
    true
}

/// Parse a labeled expression: 'label: loop/while/for/block
/// Called from primary.rs when we see a TICK token.
pub(super) fn labeled_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing labeled expression");
    // We already know we're at TICK, so start the outer marker for the loop/block
    let m = p.start();

    // Parse the label
    if !try_parse_label(p) {
        m.abandon(p);
        return Ok(None);
    }

    // Now dispatch to the appropriate loop/block based on keyword
    let kind = match p.current() {
        Some(SyntaxKind::LOOP_KW) => {
            p.bump(); // consume 'loop'
            if let Err(e) = block(p) {
                m.abandon(p);
                return Err(e);
            }
            SyntaxKind::LoopExpr
        }
        Some(SyntaxKind::WHILE_KW) => {
            p.bump(); // consume 'while'
            if let Err(e) = expr_no_struct(p) {
                m.abandon(p);
                return Err(e);
            }
            if let Err(e) = block(p) {
                m.abandon(p);
                return Err(e);
            }
            SyntaxKind::WhileExpr
        }
        Some(SyntaxKind::FOR_KW) => {
            p.bump(); // consume 'for'
            // Pattern
            if let Err(err) = crate::pattern::pattern(p) {
                m.abandon(p);
                return Err(err);
            }
            if let Err(err) = p.expect(SyntaxKind::IN_KW) {
                m.abandon(p);
                return Err(err);
            }
            if let Err(err) = expr_no_struct(p) {
                m.abandon(p);
                return Err(err);
            }
            if let Err(err) = block(p) {
                m.abandon(p);
                return Err(err);
            }
            SyntaxKind::ForExpr
        }
        Some(SyntaxKind::L_BRACE) => {
            // Labeled block
            if let Err(e) = block(p) {
                m.abandon(p);
                return Err(e);
            }
            SyntaxKind::BlockExpr
        }
        _ => {
            // Error: label must be followed by loop, while, for, or block
            // Just abandon and return None - the caller will handle it
            m.abandon(p);
            return Ok(None);
        }
    };

    Ok(Some(m.complete(p, kind)))
}

/// Parse a block expression.
pub(super) fn block_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing block expression");
    let m = p.start();
    if let Err(e) = block(p) {
        m.abandon(p);
        return Err(e);
    }
    Ok(Some(m.complete(p, SyntaxKind::BlockExpr)))
}

/// Parse a block with statements.
pub(crate) fn block(p: &mut Parser<'_>) -> Result<CompletedMarker, crate::ParseError> {
    crate::stmt::block(p)
}

/// Parse an if expression.
pub(super) fn if_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing if expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::IF_KW) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = expr_no_struct(p) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = block(p) {
        m.abandon(p);
        return Err(e);
    }

    if p.eat(SyntaxKind::ELSE_KW) {
        if p.at(SyntaxKind::IF_KW) {
            // else if
            if let Err(e) = if_expr(p) {
                m.abandon(p);
                return Err(e);
            }
        } else {
            // else block
            if let Err(e) = block(p) {
                m.abandon(p);
                return Err(e);
            }
        }
    }

    Ok(Some(m.complete(p, SyntaxKind::IfExpr)))
}

/// Parse a while expression.
pub(super) fn while_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing while expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::WHILE_KW) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = expr_no_struct(p) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = block(p) {
        m.abandon(p);
        return Err(e);
    }
    Ok(Some(m.complete(p, SyntaxKind::WhileExpr)))
}

/// Parse a for expression.
pub(super) fn for_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing for expression");
    let m = p.start();
    p.expect(SyntaxKind::FOR_KW)?;

    // Pattern (supports full pattern syntax including tuples)
    if let Err(err) = crate::pattern::pattern(p) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = p.expect(SyntaxKind::IN_KW) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = expr_no_struct(p) {
        m.abandon(p);
        return Err(err);
    }

    if let Err(err) = block(p) {
        m.abandon(p);
        return Err(err);
    }

    Ok(Some(m.complete(p, SyntaxKind::ForExpr)))
}

/// Parse a loop expression.
pub(super) fn loop_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing loop expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::LOOP_KW) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = block(p) {
        m.abandon(p);
        return Err(e);
    }
    Ok(Some(m.complete(p, SyntaxKind::LoopExpr)))
}

/// Parse a break expression.
pub(super) fn break_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing break expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::BREAK_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Optional label: 'label
    if p.at(SyntaxKind::TICK) {
        p.bump(); // consume '
        let _ = crate::path::name_ref(p);
    }

    // Optional value
    if p.current().is_some()
        && !p.at(SyntaxKind::SEMI)
        && !p.at(SyntaxKind::R_BRACE)
        && !p.at(SyntaxKind::R_PAREN)
        && let Err(e) = expr(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(Some(m.complete(p, SyntaxKind::BreakExpr)))
}

/// Parse a continue expression.
pub(super) fn continue_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing continue expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::CONTINUE_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Optional label: 'label
    if p.at(SyntaxKind::TICK) {
        p.bump(); // consume '
        let _ = crate::path::name_ref(p);
    }

    Ok(Some(m.complete(p, SyntaxKind::ContinueExpr)))
}

/// Parse a return expression.
pub(super) fn return_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing return expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::RETURN_KW) {
        m.abandon(p);
        return Err(e);
    }

    // Optional value
    if p.current().is_some()
        && !p.at(SyntaxKind::SEMI)
        && !p.at(SyntaxKind::R_BRACE)
        && !p.at(SyntaxKind::R_PAREN)
        && let Err(e) = expr(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(Some(m.complete(p, SyntaxKind::ReturnExpr)))
}

/// Parse a yield expression: `yield expr`
pub(super) fn yield_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing yield expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::YIELD_KW) {
        m.abandon(p);
        return Err(e);
    }

    // yield requires a value expression (unlike return which is optional)
    if p.current().is_some()
        && !p.at(SyntaxKind::SEMI)
        && !p.at(SyntaxKind::R_BRACE)
        && !p.at(SyntaxKind::R_PAREN)
        && let Err(e) = expr(p)
    {
        m.abandon(p);
        return Err(e);
    }

    Ok(Some(m.complete(p, SyntaxKind::YieldExpr)))
}

/// Parse an unsafe expression: `unsafe { ... }`
pub(super) fn unsafe_expr(
    p: &mut Parser<'_>,
) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing unsafe expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::UNSAFE_KW) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = block(p) {
        m.abandon(p);
        return Err(e);
    }
    Ok(Some(m.complete(p, SyntaxKind::UnsafeExpr)))
}

/// Parse a throw expression: `throw expr`
pub(super) fn throw_expr(p: &mut Parser<'_>) -> Result<Option<CompletedMarker>, crate::ParseError> {
    tracing::trace!("parsing throw expression");
    let m = p.start();
    if let Err(e) = p.expect(SyntaxKind::THROW_KW) {
        m.abandon(p);
        return Err(e);
    }
    if let Err(e) = expr(p) {
        m.abandon(p);
        return Err(e);
    }
    Ok(Some(m.complete(p, SyntaxKind::ThrowExpr)))
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::check_expr;
    use expect_test::expect;

    #[test]
    fn if_expr_simple() {
        check_expr(
            "if x { 1 }",
            &expect![[r#"
                IfExpr@0..10
                  IF_KW@0..2 "if"
                  PathExpr@2..4
                    Path@2..4
                      PathSegment@2..4
                        NameRef@2..4
                          WHITESPACE@2..3 " "
                          IDENT@3..4 "x"
                  Block@4..10
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    LiteralExpr@6..8
                      WHITESPACE@6..7 " "
                      INT_LITERAL@7..8 "1"
                    WHITESPACE@8..9 " "
                    R_BRACE@9..10 "}"
            "#]],
        );
    }

    #[test]
    fn if_else_expr() {
        check_expr(
            "if true { 1 } else { 2 }",
            &expect![[r#"
                IfExpr@0..24
                  IF_KW@0..2 "if"
                  LiteralExpr@2..7
                    WHITESPACE@2..3 " "
                    TRUE_KW@3..7 "true"
                  Block@7..13
                    WHITESPACE@7..8 " "
                    L_BRACE@8..9 "{"
                    LiteralExpr@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "1"
                    WHITESPACE@11..12 " "
                    R_BRACE@12..13 "}"
                  WHITESPACE@13..14 " "
                  ELSE_KW@14..18 "else"
                  Block@18..24
                    WHITESPACE@18..19 " "
                    L_BRACE@19..20 "{"
                    LiteralExpr@20..22
                      WHITESPACE@20..21 " "
                      INT_LITERAL@21..22 "2"
                    WHITESPACE@22..23 " "
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn if_else_if_chain() {
        check_expr(
            "if true { 1 } else if false { 2 } else { 3 }",
            &expect![[r#"
                IfExpr@0..44
                  IF_KW@0..2 "if"
                  LiteralExpr@2..7
                    WHITESPACE@2..3 " "
                    TRUE_KW@3..7 "true"
                  Block@7..13
                    WHITESPACE@7..8 " "
                    L_BRACE@8..9 "{"
                    LiteralExpr@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "1"
                    WHITESPACE@11..12 " "
                    R_BRACE@12..13 "}"
                  WHITESPACE@13..14 " "
                  ELSE_KW@14..18 "else"
                  IfExpr@18..44
                    WHITESPACE@18..19 " "
                    IF_KW@19..21 "if"
                    LiteralExpr@21..27
                      WHITESPACE@21..22 " "
                      FALSE_KW@22..27 "false"
                    Block@27..33
                      WHITESPACE@27..28 " "
                      L_BRACE@28..29 "{"
                      LiteralExpr@29..31
                        WHITESPACE@29..30 " "
                        INT_LITERAL@30..31 "2"
                      WHITESPACE@31..32 " "
                      R_BRACE@32..33 "}"
                    WHITESPACE@33..34 " "
                    ELSE_KW@34..38 "else"
                    Block@38..44
                      WHITESPACE@38..39 " "
                      L_BRACE@39..40 "{"
                      LiteralExpr@40..42
                        WHITESPACE@40..41 " "
                        INT_LITERAL@41..42 "3"
                      WHITESPACE@42..43 " "
                      R_BRACE@43..44 "}"
            "#]],
        );
    }

    #[test]
    fn if_complex_condition() {
        check_expr(
            "if a > 0 && b < 10 { x }",
            &expect![[r#"
                IfExpr@0..24
                  IF_KW@0..2 "if"
                  BinExpr@2..18
                    BinExpr@2..8
                      PathExpr@2..4
                        Path@2..4
                          PathSegment@2..4
                            NameRef@2..4
                              WHITESPACE@2..3 " "
                              IDENT@3..4 "a"
                      WHITESPACE@4..5 " "
                      GT@5..6 ">"
                      LiteralExpr@6..8
                        WHITESPACE@6..7 " "
                        INT_LITERAL@7..8 "0"
                    WHITESPACE@8..9 " "
                    AND_AND@9..11 "&&"
                    BinExpr@11..18
                      PathExpr@11..13
                        Path@11..13
                          PathSegment@11..13
                            NameRef@11..13
                              WHITESPACE@11..12 " "
                              IDENT@12..13 "b"
                      WHITESPACE@13..14 " "
                      LT@14..15 "<"
                      LiteralExpr@15..18
                        WHITESPACE@15..16 " "
                        INT_LITERAL@16..18 "10"
                  Block@18..24
                    WHITESPACE@18..19 " "
                    L_BRACE@19..20 "{"
                    PathExpr@20..22
                      Path@20..22
                        PathSegment@20..22
                          NameRef@20..22
                            WHITESPACE@20..21 " "
                            IDENT@21..22 "x"
                    WHITESPACE@22..23 " "
                    R_BRACE@23..24 "}"
            "#]],
        );
    }

    #[test]
    fn if_nested() {
        check_expr(
            "if a { if b { x } }",
            &expect![[r#"
                IfExpr@0..19
                  IF_KW@0..2 "if"
                  PathExpr@2..4
                    Path@2..4
                      PathSegment@2..4
                        NameRef@2..4
                          WHITESPACE@2..3 " "
                          IDENT@3..4 "a"
                  Block@4..19
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    IfExpr@6..17
                      WHITESPACE@6..7 " "
                      IF_KW@7..9 "if"
                      PathExpr@9..11
                        Path@9..11
                          PathSegment@9..11
                            NameRef@9..11
                              WHITESPACE@9..10 " "
                              IDENT@10..11 "b"
                      Block@11..17
                        WHITESPACE@11..12 " "
                        L_BRACE@12..13 "{"
                        PathExpr@13..15
                          Path@13..15
                            PathSegment@13..15
                              NameRef@13..15
                                WHITESPACE@13..14 " "
                                IDENT@14..15 "x"
                        WHITESPACE@15..16 " "
                        R_BRACE@16..17 "}"
                    WHITESPACE@17..18 " "
                    R_BRACE@18..19 "}"
            "#]],
        );
    }

    #[test]
    fn while_expr_simple() {
        check_expr(
            "while cond { 1 }",
            &expect![[r#"
                WhileExpr@0..16
                  WHILE_KW@0..5 "while"
                  PathExpr@5..10
                    Path@5..10
                      PathSegment@5..10
                        NameRef@5..10
                          WHITESPACE@5..6 " "
                          IDENT@6..10 "cond"
                  Block@10..16
                    WHITESPACE@10..11 " "
                    L_BRACE@11..12 "{"
                    LiteralExpr@12..14
                      WHITESPACE@12..13 " "
                      INT_LITERAL@13..14 "1"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }

    #[test]
    fn for_expr_simple() {
        check_expr(
            "for i in items { x }",
            &expect![[r#"
                ForExpr@0..20
                  FOR_KW@0..3 "for"
                  IdentPat@3..5
                    Name@3..5
                      WHITESPACE@3..4 " "
                      IDENT@4..5 "i"
                  WHITESPACE@5..6 " "
                  IN_KW@6..8 "in"
                  PathExpr@8..14
                    Path@8..14
                      PathSegment@8..14
                        NameRef@8..14
                          WHITESPACE@8..9 " "
                          IDENT@9..14 "items"
                  Block@14..20
                    WHITESPACE@14..15 " "
                    L_BRACE@15..16 "{"
                    PathExpr@16..18
                      Path@16..18
                        PathSegment@16..18
                          NameRef@16..18
                            WHITESPACE@16..17 " "
                            IDENT@17..18 "x"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn for_with_range() {
        check_expr(
            "for i in 0..10 { x }",
            &expect![[r#"
                ForExpr@0..20
                  FOR_KW@0..3 "for"
                  IdentPat@3..5
                    Name@3..5
                      WHITESPACE@3..4 " "
                      IDENT@4..5 "i"
                  WHITESPACE@5..6 " "
                  IN_KW@6..8 "in"
                  RangeExpr@8..14
                    LiteralExpr@8..10
                      WHITESPACE@8..9 " "
                      INT_LITERAL@9..10 "0"
                    DOT_DOT@10..12 ".."
                    LiteralExpr@12..14
                      INT_LITERAL@12..14 "10"
                  Block@14..20
                    WHITESPACE@14..15 " "
                    L_BRACE@15..16 "{"
                    PathExpr@16..18
                      Path@16..18
                        PathSegment@16..18
                          NameRef@16..18
                            WHITESPACE@16..17 " "
                            IDENT@17..18 "x"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    #[test]
    fn loop_expr_simple() {
        check_expr(
            "loop { x }",
            &expect![[r#"
                LoopExpr@0..10
                  LOOP_KW@0..4 "loop"
                  Block@4..10
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    PathExpr@6..8
                      Path@6..8
                        PathSegment@6..8
                          NameRef@6..8
                            WHITESPACE@6..7 " "
                            IDENT@7..8 "x"
                    WHITESPACE@8..9 " "
                    R_BRACE@9..10 "}"
            "#]],
        );
    }

    #[test]
    fn loop_with_break_value() {
        check_expr(
            "loop { break 42; }",
            &expect![[r#"
                LoopExpr@0..18
                  LOOP_KW@0..4 "loop"
                  Block@4..18
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    ExprStmt@6..16
                      BreakExpr@6..15
                        WHITESPACE@6..7 " "
                        BREAK_KW@7..12 "break"
                        LiteralExpr@12..15
                          WHITESPACE@12..13 " "
                          INT_LITERAL@13..15 "42"
                      SEMI@15..16 ";"
                    WHITESPACE@16..17 " "
                    R_BRACE@17..18 "}"
            "#]],
        );
    }

    #[test]
    fn break_expr_no_value() {
        check_expr(
            "break",
            &expect![[r#"
                BreakExpr@0..5
                  BREAK_KW@0..5 "break"
            "#]],
        );
    }

    #[test]
    fn break_expr_with_value() {
        check_expr(
            "break 42",
            &expect![[r#"
                BreakExpr@0..8
                  BREAK_KW@0..5 "break"
                  LiteralExpr@5..8
                    WHITESPACE@5..6 " "
                    INT_LITERAL@6..8 "42"
            "#]],
        );
    }

    #[test]
    fn continue_expr() {
        check_expr(
            "continue",
            &expect![[r#"
                ContinueExpr@0..8
                  CONTINUE_KW@0..8 "continue"
            "#]],
        );
    }

    #[test]
    fn return_expr_no_value() {
        check_expr(
            "return",
            &expect![[r#"
                ReturnExpr@0..6
                  RETURN_KW@0..6 "return"
            "#]],
        );
    }

    #[test]
    fn return_expr_with_value() {
        check_expr(
            "return x + 1",
            &expect![[r#"
                ReturnExpr@0..12
                  RETURN_KW@0..6 "return"
                  BinExpr@6..12
                    PathExpr@6..8
                      Path@6..8
                        PathSegment@6..8
                          NameRef@6..8
                            WHITESPACE@6..7 " "
                            IDENT@7..8 "x"
                    WHITESPACE@8..9 " "
                    PLUS@9..10 "+"
                    LiteralExpr@10..12
                      WHITESPACE@10..11 " "
                      INT_LITERAL@11..12 "1"
            "#]],
        );
    }

    // === Control Flow Combination Tests ===

    #[test]
    fn nested_loops_mixed() {
        // for inside while inside loop
        check_expr(
            "loop { while cond { for i in items { x } } }",
            &expect![[r#"
                LoopExpr@0..44
                  LOOP_KW@0..4 "loop"
                  Block@4..44
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    WhileExpr@6..42
                      WHITESPACE@6..7 " "
                      WHILE_KW@7..12 "while"
                      PathExpr@12..17
                        Path@12..17
                          PathSegment@12..17
                            NameRef@12..17
                              WHITESPACE@12..13 " "
                              IDENT@13..17 "cond"
                      Block@17..42
                        WHITESPACE@17..18 " "
                        L_BRACE@18..19 "{"
                        ForExpr@19..40
                          WHITESPACE@19..20 " "
                          FOR_KW@20..23 "for"
                          IdentPat@23..25
                            Name@23..25
                              WHITESPACE@23..24 " "
                              IDENT@24..25 "i"
                          WHITESPACE@25..26 " "
                          IN_KW@26..28 "in"
                          PathExpr@28..34
                            Path@28..34
                              PathSegment@28..34
                                NameRef@28..34
                                  WHITESPACE@28..29 " "
                                  IDENT@29..34 "items"
                          Block@34..40
                            WHITESPACE@34..35 " "
                            L_BRACE@35..36 "{"
                            PathExpr@36..38
                              Path@36..38
                                PathSegment@36..38
                                  NameRef@36..38
                                    WHITESPACE@36..37 " "
                                    IDENT@37..38 "x"
                            WHITESPACE@38..39 " "
                            R_BRACE@39..40 "}"
                        WHITESPACE@40..41 " "
                        R_BRACE@41..42 "}"
                    WHITESPACE@42..43 " "
                    R_BRACE@43..44 "}"
            "#]],
        );
    }

    #[test]
    fn break_in_nested_context() {
        // break in nested if inside loop
        check_expr(
            "loop { if cond { break 42; } }",
            &expect![[r#"
                LoopExpr@0..30
                  LOOP_KW@0..4 "loop"
                  Block@4..30
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    IfExpr@6..28
                      WHITESPACE@6..7 " "
                      IF_KW@7..9 "if"
                      PathExpr@9..14
                        Path@9..14
                          PathSegment@9..14
                            NameRef@9..14
                              WHITESPACE@9..10 " "
                              IDENT@10..14 "cond"
                      Block@14..28
                        WHITESPACE@14..15 " "
                        L_BRACE@15..16 "{"
                        ExprStmt@16..26
                          BreakExpr@16..25
                            WHITESPACE@16..17 " "
                            BREAK_KW@17..22 "break"
                            LiteralExpr@22..25
                              WHITESPACE@22..23 " "
                              INT_LITERAL@23..25 "42"
                          SEMI@25..26 ";"
                        WHITESPACE@26..27 " "
                        R_BRACE@27..28 "}"
                    WHITESPACE@28..29 " "
                    R_BRACE@29..30 "}"
            "#]],
        );
    }

    #[test]
    fn continue_in_nested_context() {
        // continue in nested if inside for loop
        check_expr(
            "for i in items { if skip { continue; } }",
            &expect![[r#"
                ForExpr@0..40
                  FOR_KW@0..3 "for"
                  IdentPat@3..5
                    Name@3..5
                      WHITESPACE@3..4 " "
                      IDENT@4..5 "i"
                  WHITESPACE@5..6 " "
                  IN_KW@6..8 "in"
                  PathExpr@8..14
                    Path@8..14
                      PathSegment@8..14
                        NameRef@8..14
                          WHITESPACE@8..9 " "
                          IDENT@9..14 "items"
                  Block@14..40
                    WHITESPACE@14..15 " "
                    L_BRACE@15..16 "{"
                    IfExpr@16..38
                      WHITESPACE@16..17 " "
                      IF_KW@17..19 "if"
                      PathExpr@19..24
                        Path@19..24
                          PathSegment@19..24
                            NameRef@19..24
                              WHITESPACE@19..20 " "
                              IDENT@20..24 "skip"
                      Block@24..38
                        WHITESPACE@24..25 " "
                        L_BRACE@25..26 "{"
                        ExprStmt@26..36
                          ContinueExpr@26..35
                            WHITESPACE@26..27 " "
                            CONTINUE_KW@27..35 "continue"
                          SEMI@35..36 ";"
                        WHITESPACE@36..37 " "
                        R_BRACE@37..38 "}"
                    WHITESPACE@38..39 " "
                    R_BRACE@39..40 "}"
            "#]],
        );
    }

    #[test]
    fn if_in_loop_with_break_value() {
        // if inside loop where branches have break values
        check_expr(
            "loop { if done { break result; } else { continue; } }",
            &expect![[r#"
                LoopExpr@0..53
                  LOOP_KW@0..4 "loop"
                  Block@4..53
                    WHITESPACE@4..5 " "
                    L_BRACE@5..6 "{"
                    IfExpr@6..51
                      WHITESPACE@6..7 " "
                      IF_KW@7..9 "if"
                      PathExpr@9..14
                        Path@9..14
                          PathSegment@9..14
                            NameRef@9..14
                              WHITESPACE@9..10 " "
                              IDENT@10..14 "done"
                      Block@14..32
                        WHITESPACE@14..15 " "
                        L_BRACE@15..16 "{"
                        ExprStmt@16..30
                          BreakExpr@16..29
                            WHITESPACE@16..17 " "
                            BREAK_KW@17..22 "break"
                            PathExpr@22..29
                              Path@22..29
                                PathSegment@22..29
                                  NameRef@22..29
                                    WHITESPACE@22..23 " "
                                    IDENT@23..29 "result"
                          SEMI@29..30 ";"
                        WHITESPACE@30..31 " "
                        R_BRACE@31..32 "}"
                      WHITESPACE@32..33 " "
                      ELSE_KW@33..37 "else"
                      Block@37..51
                        WHITESPACE@37..38 " "
                        L_BRACE@38..39 "{"
                        ExprStmt@39..49
                          ContinueExpr@39..48
                            WHITESPACE@39..40 " "
                            CONTINUE_KW@40..48 "continue"
                          SEMI@48..49 ";"
                        WHITESPACE@49..50 " "
                        R_BRACE@50..51 "}"
                    WHITESPACE@51..52 " "
                    R_BRACE@52..53 "}"
            "#]],
        );
    }

    #[test]
    fn while_with_break() {
        check_expr(
            "while cond { if done { break; } }",
            &expect![[r#"
                WhileExpr@0..33
                  WHILE_KW@0..5 "while"
                  PathExpr@5..10
                    Path@5..10
                      PathSegment@5..10
                        NameRef@5..10
                          WHITESPACE@5..6 " "
                          IDENT@6..10 "cond"
                  Block@10..33
                    WHITESPACE@10..11 " "
                    L_BRACE@11..12 "{"
                    IfExpr@12..31
                      WHITESPACE@12..13 " "
                      IF_KW@13..15 "if"
                      PathExpr@15..20
                        Path@15..20
                          PathSegment@15..20
                            NameRef@15..20
                              WHITESPACE@15..16 " "
                              IDENT@16..20 "done"
                      Block@20..31
                        WHITESPACE@20..21 " "
                        L_BRACE@21..22 "{"
                        ExprStmt@22..29
                          BreakExpr@22..28
                            WHITESPACE@22..23 " "
                            BREAK_KW@23..28 "break"
                          SEMI@28..29 ";"
                        WHITESPACE@29..30 " "
                        R_BRACE@30..31 "}"
                    WHITESPACE@31..32 " "
                    R_BRACE@32..33 "}"
            "#]],
        );
    }

    #[test]
    fn yield_expr_with_value() {
        check_expr(
            "yield x + 1",
            &expect![[r#"
                YieldExpr@0..11
                  YIELD_KW@0..5 "yield"
                  BinExpr@5..11
                    PathExpr@5..7
                      Path@5..7
                        PathSegment@5..7
                          NameRef@5..7
                            WHITESPACE@5..6 " "
                            IDENT@6..7 "x"
                    WHITESPACE@7..8 " "
                    PLUS@8..9 "+"
                    LiteralExpr@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "1"
            "#]],
        );
    }

    #[test]
    fn yield_in_block() {
        check_expr(
            "{ yield 42; }",
            &expect![[r#"
                BlockExpr@0..13
                  Block@0..13
                    L_BRACE@0..1 "{"
                    ExprStmt@1..11
                      YieldExpr@1..10
                        WHITESPACE@1..2 " "
                        YIELD_KW@2..7 "yield"
                        LiteralExpr@7..10
                          WHITESPACE@7..8 " "
                          INT_LITERAL@8..10 "42"
                      SEMI@10..11 ";"
                    WHITESPACE@11..12 " "
                    R_BRACE@12..13 "}"
            "#]],
        );
    }

    #[test]
    fn return_in_if() {
        check_expr(
            "if cond { return x; }",
            &expect![[r#"
                IfExpr@0..21
                  IF_KW@0..2 "if"
                  PathExpr@2..7
                    Path@2..7
                      PathSegment@2..7
                        NameRef@2..7
                          WHITESPACE@2..3 " "
                          IDENT@3..7 "cond"
                  Block@7..21
                    WHITESPACE@7..8 " "
                    L_BRACE@8..9 "{"
                    ExprStmt@9..19
                      ReturnExpr@9..18
                        WHITESPACE@9..10 " "
                        RETURN_KW@10..16 "return"
                        PathExpr@16..18
                          Path@16..18
                            PathSegment@16..18
                              NameRef@16..18
                                WHITESPACE@16..17 " "
                                IDENT@17..18 "x"
                      SEMI@18..19 ";"
                    WHITESPACE@19..20 " "
                    R_BRACE@20..21 "}"
            "#]],
        );
    }

    // === Labeled Control Flow Tests ===

    #[test]
    fn break_with_label() {
        check_expr(
            "break 'outer",
            &expect![[r#"
                BreakExpr@0..12
                  BREAK_KW@0..5 "break"
                  WHITESPACE@5..6 " "
                  TICK@6..7 "'"
                  NameRef@7..12
                    IDENT@7..12 "outer"
            "#]],
        );
    }

    #[test]
    fn break_with_label_and_value() {
        check_expr(
            "break 'outer 42",
            &expect![[r#"
                BreakExpr@0..15
                  BREAK_KW@0..5 "break"
                  WHITESPACE@5..6 " "
                  TICK@6..7 "'"
                  NameRef@7..12
                    IDENT@7..12 "outer"
                  LiteralExpr@12..15
                    WHITESPACE@12..13 " "
                    INT_LITERAL@13..15 "42"
            "#]],
        );
    }

    #[test]
    fn continue_with_label() {
        check_expr(
            "continue 'outer",
            &expect![[r#"
                ContinueExpr@0..15
                  CONTINUE_KW@0..8 "continue"
                  WHITESPACE@8..9 " "
                  TICK@9..10 "'"
                  NameRef@10..15
                    IDENT@10..15 "outer"
            "#]],
        );
    }

    #[test]
    fn labeled_loop() {
        check_expr(
            "'outer: loop { }",
            &expect![[r#"
                LoopExpr@0..16
                  Label@0..7
                    TICK@0..1 "'"
                    Name@1..6
                      IDENT@1..6 "outer"
                    COLON@6..7 ":"
                  WHITESPACE@7..8 " "
                  LOOP_KW@8..12 "loop"
                  Block@12..16
                    WHITESPACE@12..13 " "
                    L_BRACE@13..14 "{"
                    WHITESPACE@14..15 " "
                    R_BRACE@15..16 "}"
            "#]],
        );
    }

    #[test]
    fn labeled_while() {
        check_expr(
            "'outer: while true { }",
            &expect![[r#"
                WhileExpr@0..22
                  Label@0..7
                    TICK@0..1 "'"
                    Name@1..6
                      IDENT@1..6 "outer"
                    COLON@6..7 ":"
                  WHITESPACE@7..8 " "
                  WHILE_KW@8..13 "while"
                  LiteralExpr@13..18
                    WHITESPACE@13..14 " "
                    TRUE_KW@14..18 "true"
                  Block@18..22
                    WHITESPACE@18..19 " "
                    L_BRACE@19..20 "{"
                    WHITESPACE@20..21 " "
                    R_BRACE@21..22 "}"
            "#]],
        );
    }

    #[test]
    fn labeled_for() {
        check_expr(
            "'outer: for i in items { }",
            &expect![[r#"
                ForExpr@0..26
                  Label@0..7
                    TICK@0..1 "'"
                    Name@1..6
                      IDENT@1..6 "outer"
                    COLON@6..7 ":"
                  WHITESPACE@7..8 " "
                  FOR_KW@8..11 "for"
                  IdentPat@11..13
                    Name@11..13
                      WHITESPACE@11..12 " "
                      IDENT@12..13 "i"
                  WHITESPACE@13..14 " "
                  IN_KW@14..16 "in"
                  PathExpr@16..22
                    Path@16..22
                      PathSegment@16..22
                        NameRef@16..22
                          WHITESPACE@16..17 " "
                          IDENT@17..22 "items"
                  Block@22..26
                    WHITESPACE@22..23 " "
                    L_BRACE@23..24 "{"
                    WHITESPACE@24..25 " "
                    R_BRACE@25..26 "}"
            "#]],
        );
    }

    #[test]
    fn labeled_block() {
        check_expr(
            "'outer: { 1 }",
            &expect![[r#"
                BlockExpr@0..13
                  Label@0..7
                    TICK@0..1 "'"
                    Name@1..6
                      IDENT@1..6 "outer"
                    COLON@6..7 ":"
                  Block@7..13
                    WHITESPACE@7..8 " "
                    L_BRACE@8..9 "{"
                    LiteralExpr@9..11
                      WHITESPACE@9..10 " "
                      INT_LITERAL@10..11 "1"
                    WHITESPACE@11..12 " "
                    R_BRACE@12..13 "}"
            "#]],
        );
    }

    #[test]
    fn nested_labeled_loops() {
        check_expr(
            "'outer: loop { 'inner: loop { break 'outer } }",
            &expect![[r#"
                LoopExpr@0..46
                  Label@0..7
                    TICK@0..1 "'"
                    Name@1..6
                      IDENT@1..6 "outer"
                    COLON@6..7 ":"
                  WHITESPACE@7..8 " "
                  LOOP_KW@8..12 "loop"
                  Block@12..46
                    WHITESPACE@12..13 " "
                    L_BRACE@13..14 "{"
                    LoopExpr@14..44
                      Label@14..22
                        WHITESPACE@14..15 " "
                        TICK@15..16 "'"
                        Name@16..21
                          IDENT@16..21 "inner"
                        COLON@21..22 ":"
                      WHITESPACE@22..23 " "
                      LOOP_KW@23..27 "loop"
                      Block@27..44
                        WHITESPACE@27..28 " "
                        L_BRACE@28..29 "{"
                        BreakExpr@29..42
                          WHITESPACE@29..30 " "
                          BREAK_KW@30..35 "break"
                          WHITESPACE@35..36 " "
                          TICK@36..37 "'"
                          NameRef@37..42
                            IDENT@37..42 "outer"
                        WHITESPACE@42..43 " "
                        R_BRACE@43..44 "}"
                    WHITESPACE@44..45 " "
                    R_BRACE@45..46 "}"
            "#]],
        );
    }

    #[test]
    fn char_literal_in_labeled_loop() {
        // Ensure 'a' (char literal) is not confused with label syntax
        check_expr(
            "'outer: loop { 'a' }",
            &expect![[r#"
                LoopExpr@0..20
                  Label@0..7
                    TICK@0..1 "'"
                    Name@1..6
                      IDENT@1..6 "outer"
                    COLON@6..7 ":"
                  WHITESPACE@7..8 " "
                  LOOP_KW@8..12 "loop"
                  Block@12..20
                    WHITESPACE@12..13 " "
                    L_BRACE@13..14 "{"
                    LiteralExpr@14..18
                      WHITESPACE@14..15 " "
                      CHAR_LITERAL@15..18 "'a'"
                    WHITESPACE@18..19 " "
                    R_BRACE@19..20 "}"
            "#]],
        );
    }

    // === Unsafe Expression Tests ===

    #[test]
    fn unsafe_expr_simple() {
        check_expr(
            "unsafe { 42 }",
            &expect![[r#"
            UnsafeExpr@0..13
              UNSAFE_KW@0..6 "unsafe"
              Block@6..13
                WHITESPACE@6..7 " "
                L_BRACE@7..8 "{"
                LiteralExpr@8..11
                  WHITESPACE@8..9 " "
                  INT_LITERAL@9..11 "42"
                WHITESPACE@11..12 " "
                R_BRACE@12..13 "}"
        "#]],
        );
    }

    #[test]
    fn unsafe_expr_with_statements() {
        check_expr(
            "unsafe { let x = 1; x }",
            &expect![[r#"
            UnsafeExpr@0..23
              UNSAFE_KW@0..6 "unsafe"
              Block@6..23
                WHITESPACE@6..7 " "
                L_BRACE@7..8 "{"
                LetStmt@8..19
                  WHITESPACE@8..9 " "
                  LET_KW@9..12 "let"
                  IdentPat@12..14
                    Name@12..14
                      WHITESPACE@12..13 " "
                      IDENT@13..14 "x"
                  WHITESPACE@14..15 " "
                  EQ@15..16 "="
                  LiteralExpr@16..18
                    WHITESPACE@16..17 " "
                    INT_LITERAL@17..18 "1"
                  SEMI@18..19 ";"
                PathExpr@19..21
                  Path@19..21
                    PathSegment@19..21
                      NameRef@19..21
                        WHITESPACE@19..20 " "
                        IDENT@20..21 "x"
                WHITESPACE@21..22 " "
                R_BRACE@22..23 "}"
        "#]],
        );
    }

    #[test]
    fn unsafe_expr_nested() {
        check_expr(
            "if true { unsafe { 1 } } else { 0 }",
            &expect![[r#"
            IfExpr@0..35
              IF_KW@0..2 "if"
              LiteralExpr@2..7
                WHITESPACE@2..3 " "
                TRUE_KW@3..7 "true"
              Block@7..24
                WHITESPACE@7..8 " "
                L_BRACE@8..9 "{"
                UnsafeExpr@9..22
                  WHITESPACE@9..10 " "
                  UNSAFE_KW@10..16 "unsafe"
                  Block@16..22
                    WHITESPACE@16..17 " "
                    L_BRACE@17..18 "{"
                    LiteralExpr@18..20
                      WHITESPACE@18..19 " "
                      INT_LITERAL@19..20 "1"
                    WHITESPACE@20..21 " "
                    R_BRACE@21..22 "}"
                WHITESPACE@22..23 " "
                R_BRACE@23..24 "}"
              WHITESPACE@24..25 " "
              ELSE_KW@25..29 "else"
              Block@29..35
                WHITESPACE@29..30 " "
                L_BRACE@30..31 "{"
                LiteralExpr@31..33
                  WHITESPACE@31..32 " "
                  INT_LITERAL@32..33 "0"
                WHITESPACE@33..34 " "
                R_BRACE@34..35 "}"
        "#]],
        );
    }

    // === Throw Expression Tests ===

    #[test]
    fn throw_expr_simple() {
        check_expr(
            "throw error",
            &expect![[r#"
            ThrowExpr@0..11
              THROW_KW@0..5 "throw"
              PathExpr@5..11
                Path@5..11
                  PathSegment@5..11
                    NameRef@5..11
                      WHITESPACE@5..6 " "
                      IDENT@6..11 "error"
        "#]],
        );
    }

    #[test]
    fn throw_expr_enum_shorthand() {
        check_expr(
            "throw .NotFound",
            &expect![[r#"
            ThrowExpr@0..15
              THROW_KW@0..5 "throw"
              EnumShorthandExpr@5..15
                WHITESPACE@5..6 " "
                DOT@6..7 "."
                Name@7..15
                  IDENT@7..15 "NotFound"
        "#]],
        );
    }

    #[test]
    fn throw_expr_with_call() {
        check_expr(
            "throw Error.new(msg)",
            &expect![[r#"
            ThrowExpr@0..20
              THROW_KW@0..5 "throw"
              CallExpr@5..20
                PathExpr@5..15
                  Path@5..15
                    PathSegment@5..11
                      NameRef@5..11
                        WHITESPACE@5..6 " "
                        IDENT@6..11 "Error"
                    DOT@11..12 "."
                    PathSegment@12..15
                      NameRef@12..15
                        IDENT@12..15 "new"
                L_PAREN@15..16 "("
                CallArg@16..19
                  PathExpr@16..19
                    Path@16..19
                      PathSegment@16..19
                        NameRef@16..19
                          IDENT@16..19 "msg"
                R_PAREN@19..20 ")"
        "#]],
        );
    }
}
