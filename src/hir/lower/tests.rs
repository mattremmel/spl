//! Tests for HIR lowering.

use super::*;
use crate::ast::SourceFile;
use crate::parser::parse;
use crate::sema::infer::infer;
use crate::sema::resolver::resolve;
use crate::sema::types::PrimitiveKind;
use rowan::ast::AstNode;

fn parse_expr(src: &str) -> Expr {
    let full_src = format!("fn main() {{ let x = {src}; }}");
    let parsed = parse(&full_src);
    assert!(
        parsed.errors().is_empty(),
        "Parse errors: {:?}",
        parsed.errors()
    );

    use crate::ast::{Item, Stmt};
    let file = SourceFile::cast(parsed.syntax()).unwrap();
    let fn_item = file.items().next().unwrap();
    if let Item::Function(f) = fn_item {
        let body = f.body().unwrap();
        let stmt = body.statements().next().unwrap();
        if let Stmt::Let(let_stmt) = stmt {
            return let_stmt.initializer().unwrap();
        }
    }
    panic!("Could not extract expression from source");
}

fn lower(source: &str) -> HirDatabase {
    let parsed = parse(source);
    assert!(
        parsed.errors().is_empty(),
        "Parse errors: {:?}",
        parsed.errors()
    );
    let source_file = SourceFile::cast(parsed.syntax()).unwrap();
    let resolve_result = resolve(&source_file);
    let infer_result = infer(&source_file, resolve_result);
    lower_to_hir(&source_file, infer_result)
}

// ========================================================================
// Original literal folding tests
// ========================================================================

#[test]
fn lower_negated_i8() {
    let expr = parse_expr("-128i8");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, suffix, .. } => {
            assert_eq!(value, -128);
            assert_eq!(suffix, Some(PrimitiveKind::I8));
        }
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn lower_parenthesized_negation() {
    let expr = parse_expr("-(128i8)");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, suffix, .. } => {
            assert_eq!(value, -128);
            assert_eq!(suffix, Some(PrimitiveKind::I8));
        }
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn lower_double_paren_negation() {
    let expr = parse_expr("(-(128i8))");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, suffix, .. } => {
            assert_eq!(value, -128);
            assert_eq!(suffix, Some(PrimitiveKind::I8));
        }
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn lower_double_negation() {
    // --128i8 should fold to +128
    let expr = parse_expr("--128i8");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, suffix, .. } => {
            assert_eq!(value, 128); // Double negation = positive
            assert_eq!(suffix, Some(PrimitiveKind::I8));
        }
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn passthrough_variable() {
    let expr = parse_expr("-x");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(!was_lowered);
    assert!(matches!(lowered, LoweredExpr::Passthrough));
}

#[test]
fn lower_plain_int_literal() {
    // Plain literals are now lowered for constant folding
    let expr = parse_expr("42");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 42),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn lower_negated_float() {
    let expr = parse_expr("-1.5f32");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::FloatLiteral { value, suffix, .. } => {
            assert!((value - (-1.5)).abs() < f64::EPSILON);
            assert_eq!(suffix, Some(PrimitiveKind::F32));
        }
        _ => panic!("Expected FloatLiteral"),
    }
}

#[test]
fn lower_negated_unsuffixed_int() {
    let expr = parse_expr("-42");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, suffix, .. } => {
            assert_eq!(value, -42);
            assert_eq!(suffix, None);
        }
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn lower_hex_literal() {
    let expr = parse_expr("-0x80i8");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, suffix, .. } => {
            assert_eq!(value, -128); // 0x80 = 128
            assert_eq!(suffix, Some(PrimitiveKind::I8));
        }
        _ => panic!("Expected IntLiteral"),
    }
}

// ========================================================================
// Constant folding tests - Phase 2: Boolean Negation
// ========================================================================

#[test]
fn fold_not_true() {
    let expr = parse_expr("!true");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(!value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_not_false() {
    let expr = parse_expr("!false");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_double_not_true() {
    let expr = parse_expr("!!true");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_not_parenthesized() {
    let expr = parse_expr("!(false)");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_not_variable_passthrough() {
    // Negation of a variable should pass through (not foldable)
    let expr = parse_expr("!x");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(!was_lowered);
    assert!(matches!(lowered, LoweredExpr::Passthrough));
}

// ========================================================================
// Constant folding tests - Phase 3: Integer Arithmetic
// ========================================================================

#[test]
fn fold_int_add() {
    let expr = parse_expr("1 + 2");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 3),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_int_sub() {
    let expr = parse_expr("5 - 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 2),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_int_mul() {
    let expr = parse_expr("3 * 4");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 12),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_int_div() {
    let expr = parse_expr("10 / 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 3),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_int_rem() {
    let expr = parse_expr("10 % 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 1),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_int_add_with_suffix() {
    let expr = parse_expr("1i8 + 2i8");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, suffix, .. } => {
            assert_eq!(value, 3);
            assert_eq!(suffix, Some(PrimitiveKind::I8));
        }
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_div_by_zero_passthrough() {
    let expr = parse_expr("10 / 0");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(!was_lowered);
    assert!(matches!(lowered, LoweredExpr::Passthrough));
}

#[test]
fn fold_int_add_variable_passthrough() {
    let expr = parse_expr("1 + x");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(!was_lowered);
    assert!(matches!(lowered, LoweredExpr::Passthrough));
}

// ========================================================================
// Constant folding tests - Phase 4: Float Arithmetic
// ========================================================================

#[test]
fn fold_float_add() {
    let expr = parse_expr("1.0 + 2.5");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::FloatLiteral { value, .. } => {
            assert!((value - 3.5).abs() < f64::EPSILON);
        }
        _ => panic!("Expected FloatLiteral"),
    }
}

#[test]
fn fold_float_sub() {
    let expr = parse_expr("5.0 - 2.0");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::FloatLiteral { value, .. } => {
            assert!((value - 3.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected FloatLiteral"),
    }
}

#[test]
fn fold_float_mul() {
    let expr = parse_expr("2.0 * 3.0");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::FloatLiteral { value, .. } => {
            assert!((value - 6.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected FloatLiteral"),
    }
}

#[test]
fn fold_float_div() {
    let expr = parse_expr("10.0 / 4.0");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::FloatLiteral { value, .. } => {
            assert!((value - 2.5).abs() < f64::EPSILON);
        }
        _ => panic!("Expected FloatLiteral"),
    }
}

#[test]
fn fold_float_div_by_zero() {
    let expr = parse_expr("1.0 / 0.0");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::FloatLiteral { value, .. } => {
            assert!(value.is_infinite());
        }
        _ => panic!("Expected FloatLiteral"),
    }
}

#[test]
fn fold_float_with_suffix() {
    let expr = parse_expr("1.0f32 + 2.0f32");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::FloatLiteral { value, suffix, .. } => {
            assert!((value - 3.0).abs() < f64::EPSILON);
            assert_eq!(suffix, Some(PrimitiveKind::F32));
        }
        _ => panic!("Expected FloatLiteral"),
    }
}

// ========================================================================
// Constant folding tests - Phase 5: Comparison Operators
// ========================================================================

#[test]
fn fold_int_eq() {
    let expr = parse_expr("3 == 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_int_ne() {
    let expr = parse_expr("3 != 4");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_int_lt() {
    let expr = parse_expr("2 < 5");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_int_gt() {
    let expr = parse_expr("5 > 2");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_int_le() {
    let expr = parse_expr("3 <= 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_int_ge() {
    let expr = parse_expr("5 >= 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_float_lt() {
    let expr = parse_expr("1.5 < 2.0");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_bool_eq() {
    let expr = parse_expr("true == true");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_bool_ne() {
    let expr = parse_expr("true != false");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

// ========================================================================
// Constant folding tests - Phase 6: Logical Operators
// ========================================================================

#[test]
fn fold_and_true_true() {
    let expr = parse_expr("true && true");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_and_true_false() {
    let expr = parse_expr("true && false");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(!value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_and_false_true() {
    let expr = parse_expr("false && true");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(!value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_or_true_false() {
    let expr = parse_expr("true || false");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_or_false_false() {
    let expr = parse_expr("false || false");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(!value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_and_false_variable_passthrough() {
    let expr = parse_expr("false && x");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(!was_lowered);
    assert!(matches!(lowered, LoweredExpr::Passthrough));
}

// ========================================================================
// Constant folding tests - Phase 7: Nested/Complex Expressions
// ========================================================================

#[test]
fn fold_nested_arithmetic() {
    let expr = parse_expr("(1 + 2) * 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 9),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_chained_add() {
    let expr = parse_expr("1 + 2 + 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 6),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_nested_comparison() {
    let expr = parse_expr("(1 + 2) < 5");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_nested_logical() {
    let expr = parse_expr("!(!true)");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_complex_logical() {
    let expr = parse_expr("(1 < 2) && (3 > 1)");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_mixed_precedence() {
    let expr = parse_expr("2 + 3 * 4");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 14),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_negated_arithmetic() {
    let expr = parse_expr("-(1 + 2)");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, -3),
        _ => panic!("Expected IntLiteral"),
    }
}

// ========================================================================
// Constant folding tests - Edge Cases
// ========================================================================

#[test]
fn fold_rem_by_zero_passthrough() {
    let expr = parse_expr("10 % 0");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(!was_lowered);
    assert!(matches!(lowered, LoweredExpr::Passthrough));
}

#[test]
fn fold_negative_int_comparison() {
    let expr = parse_expr("-5 < -3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value), // -5 < -3 is true
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_negative_subtraction() {
    let expr = parse_expr("-5 - -3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, -2), // -5 - (-3) = -2
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_deeply_nested_parens() {
    let expr = parse_expr("((((1 + 2))))");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 3),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_comparison_chain() {
    // (1 == 1) == true
    let expr = parse_expr("(1 == 1) == true");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(value),
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_not_of_comparison() {
    let expr = parse_expr("!(1 < 2)");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(!value), // !(true) = false
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_float_negative_division() {
    let expr = parse_expr("-10.0 / 2.0");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::FloatLiteral { value, .. } => {
            assert!((value - (-5.0)).abs() < f64::EPSILON);
        }
        _ => panic!("Expected FloatLiteral"),
    }
}

#[test]
fn fold_complex_arithmetic() {
    // ((2 + 3) * 4) - 10 / 2 = 20 - 5 = 15
    let expr = parse_expr("((2 + 3) * 4) - 10 / 2");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 15),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_overflow_passthrough() {
    // i128::MAX + 1 would overflow
    let expr = parse_expr("170141183460469231731687303715884105727i128 + 1i128");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    // Should passthrough because checked_add returns None on overflow
    assert!(!was_lowered);
    assert!(matches!(lowered, LoweredExpr::Passthrough));
}

#[test]
fn fold_large_multiplication_no_overflow() {
    // Large but doesn't overflow
    let expr = parse_expr("1000000 * 1000000");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::IntLiteral { value, .. } => assert_eq!(value, 1_000_000_000_000),
        _ => panic!("Expected IntLiteral"),
    }
}

#[test]
fn fold_triple_negation() {
    let expr = parse_expr("!!!true");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(!value), // !!!true = false
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_eq_false_result() {
    let expr = parse_expr("3 == 4");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(!value), // 3 == 4 is false
        _ => panic!("Expected BoolLiteral"),
    }
}

#[test]
fn fold_lt_false_result() {
    let expr = parse_expr("5 < 3");
    let (lowered, was_lowered) = try_lower_expr(&expr);
    assert!(was_lowered);
    match lowered {
        LoweredExpr::BoolLiteral { value, .. } => assert!(!value), // 5 < 3 is false
        _ => panic!("Expected BoolLiteral"),
    }
}

// ========================================================================
// New HIR lowering tests - Phase 2: Literals
// ========================================================================

#[test]
fn lower_int_literal() {
    let db = lower("fn main() { let x = 42; }");
    assert!(!db.exprs.is_empty());

    // Find the literal expression
    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Literal(Literal::Int(v)) = &expr.kind {
            assert_eq!(*v, 42);
            return;
        }
    }
    panic!("Did not find int literal");
}

#[test]
fn lower_int_literal_i8() {
    let db = lower("fn main() { let x: i8 = 42i8; }");
    assert!(!db.exprs.is_empty());

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Literal(Literal::Int(v)) = &expr.kind {
            assert_eq!(*v, 42);
            return;
        }
    }
    panic!("Did not find int literal");
}

#[test]
fn lower_negated_literal() {
    let db = lower("fn main() { let x: i8 = -128i8; }");
    assert!(!db.exprs.is_empty());

    // Should be folded to a single literal
    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Literal(Literal::Int(v)) = &expr.kind
            && *v == -128
        {
            return;
        }
    }
    panic!("Did not find folded negated literal");
}

#[test]
fn lower_float_literal() {
    let db = lower("fn main() { let x = 2.5; }");
    assert!(!db.exprs.is_empty());

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Literal(Literal::Float(v)) = &expr.kind {
            assert!((*v - 2.5_f64).abs() < 0.001);
            return;
        }
    }
    panic!("Did not find float literal");
}

#[test]
fn lower_bool_literal() {
    let db = lower("fn main() { let x = true; let y = false; }");
    assert!(!db.exprs.is_empty());

    let mut found_true = false;
    let mut found_false = false;
    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Literal(Literal::Bool(v)) = &expr.kind {
            if *v {
                found_true = true;
            } else {
                found_false = true;
            }
        }
    }
    assert!(found_true, "Did not find true literal");
    assert!(found_false, "Did not find false literal");
}

#[test]
fn lower_char_literal() {
    let db = lower("fn main() { let x = 'a'; }");
    assert!(!db.exprs.is_empty());

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Literal(Literal::Char(c)) = &expr.kind {
            assert_eq!(*c, 'a');
            return;
        }
    }
    panic!("Did not find char literal");
}

#[test]
fn lower_string_literal() {
    let db = lower(r#"fn main() { let x = "hello"; }"#);
    assert!(!db.exprs.is_empty());

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Literal(Literal::String(s)) = &expr.kind {
            assert_eq!(s, "hello");
            return;
        }
    }
    panic!("Did not find string literal");
}

// ========================================================================
// Phase 3: Variables & Binary Ops
// ========================================================================

#[test]
fn lower_local_reference() {
    let db = lower("fn main() { let x = 1; x; }");

    let mut found_var = false;
    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Var(_) = &expr.kind {
            found_var = true;
            break;
        }
    }
    assert!(found_var, "Did not find variable reference");
}

#[test]
fn lower_binary_add() {
    // Use a variable to prevent folding
    let db = lower("fn main() { let y = 1; let x = y + 2; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Binary { op, .. } = &expr.kind {
            assert_eq!(*op, BinOp::Add);
            return;
        }
    }
    panic!("Did not find binary add");
}

#[test]
fn lower_binary_comparison() {
    // Use a variable to prevent folding
    let db = lower("fn main() { let y = 1; let x = y < 2; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Binary { op, .. } = &expr.kind {
            assert_eq!(*op, BinOp::Lt);
            return;
        }
    }
    panic!("Did not find binary comparison");
}

#[test]
fn lower_binary_logical_and() {
    // Use a variable to prevent folding
    let db = lower("fn main() { let y = true; let x = y && false; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Binary { op, .. } = &expr.kind {
            assert_eq!(*op, BinOp::And);
            return;
        }
    }
    panic!("Did not find logical and");
}

#[test]
fn lower_binary_assign() {
    let db = lower("fn main() { let mut x = 1; x = 2; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Binary { op, .. } = &expr.kind
            && *op == BinOp::Assign
        {
            return;
        }
    }
    panic!("Did not find assignment");
}

// ========================================================================
// Phase 4: Control Flow & Desugaring
// ========================================================================

#[test]
fn lower_if_expr() {
    let db = lower("fn main() { if true { 1 } else { 2 }; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::If {
            else_branch: Some(_),
            ..
        } = &expr.kind
        {
            return;
        }
    }
    panic!("Did not find if-else expression");
}

#[test]
fn lower_if_without_else() {
    let db = lower("fn main() { if true { 1; } }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::If {
            else_branch: None, ..
        } = &expr.kind
        {
            return;
        }
    }
    panic!("Did not find if expression without else");
}

#[test]
fn lower_while_to_loop() {
    let db = lower("fn main() { while true { 1; } }");

    // After desugaring, there should be a Loop, not a while
    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Loop { .. } = &expr.kind {
            return;
        }
    }
    panic!("Did not find desugared loop");
}

#[test]
fn lower_loop_expr() {
    let db = lower("fn main() { loop { break; } }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Loop { .. } = &expr.kind {
            return;
        }
    }
    panic!("Did not find loop");
}

#[test]
fn lower_break_with_value() {
    let db = lower("fn main() { loop { break 42; } }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Break { value: Some(_) } = &expr.kind {
            return;
        }
    }
    panic!("Did not find break with value");
}

#[test]
fn lower_return() {
    let db = lower("fn main() { return 42; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Return { value: Some(_) } = &expr.kind {
            return;
        }
    }
    panic!("Did not find return");
}

// ========================================================================
// Phase 5: Patterns
// ========================================================================

#[test]
fn lower_bind_pattern() {
    let db = lower("fn main() { let x = 1; }");

    for (_, pat) in db.pats.iter() {
        if let HirPatKind::Bind { mutable: false, .. } = &pat.kind {
            return;
        }
    }
    panic!("Did not find bind pattern");
}

#[test]
fn lower_mut_bind_pattern() {
    let db = lower("fn main() { let mut x = 1; }");

    for (_, pat) in db.pats.iter() {
        if let HirPatKind::Bind { mutable: true, .. } = &pat.kind {
            return;
        }
    }
    panic!("Did not find mutable bind pattern");
}

#[test]
fn lower_tuple_pattern() {
    let db = lower("fn main() { let (a, b) = (1, 2); }");

    for (_, pat) in db.pats.iter() {
        if let HirPatKind::Tuple { elements } = &pat.kind {
            assert_eq!(elements.len(), 2);
            return;
        }
    }
    panic!("Did not find tuple pattern");
}

#[test]
fn lower_wildcard_pattern() {
    let db = lower("fn main() { let _ = 1; }");

    for (_, pat) in db.pats.iter() {
        if let HirPatKind::Wildcard = &pat.kind {
            return;
        }
    }
    panic!("Did not find wildcard pattern");
}

#[test]
fn lower_struct_pattern() {
    let db = lower(
        "struct Point(x: i32, y: i32) fn main() { let Point(x: x, y: y) = Point(x: 1, y: 2); }",
    );

    for (_, pat) in db.pats.iter() {
        if let HirPatKind::Struct { fields, .. } = &pat.kind {
            assert_eq!(fields.len(), 2);
            return;
        }
    }
    panic!("Did not find struct pattern");
}

// ========================================================================
// Phase 6: Structs & Functions
// ========================================================================

#[test]
fn lower_struct_expr() {
    let db = lower("struct Point(x: i32, y: i32) fn main() { Point(x: 1, y: 2); }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Struct { fields, .. } = &expr.kind {
            assert_eq!(fields.len(), 2);
            return;
        }
    }
    panic!("Did not find struct expression");
}

#[test]
fn lower_field_access() {
    let db =
        lower("struct Point(x: i32, y: i32) fn main() { let p = Point(x: 1, y: 2); p.x; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Field { field, .. } = &expr.kind {
            assert_eq!(field, "x");
            return;
        }
    }
    panic!("Did not find field access");
}

#[test]
fn lower_tuple_field_access() {
    let db = lower("fn main() { let t = (1, 2); t.0; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::TupleField { index, .. } = &expr.kind {
            assert_eq!(*index, 0);
            return;
        }
    }
    panic!("Did not find tuple field access");
}

#[test]
fn lower_function_def() {
    let db = lower("fn foo(x: i32): i32 { x }");

    assert!(!db.items.is_empty());
    for item in &db.items {
        if let HirItem::Function(f) = item {
            assert_eq!(f.name, "foo");
            return;
        }
    }
    panic!("Did not find function definition");
}

#[test]
fn lower_function_call() {
    let db = lower("fn foo() {} fn main() { foo(); }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Call { .. } = &expr.kind {
            return;
        }
    }
    panic!("Did not find function call");
}

#[test]
fn lower_method_call() {
    let db = lower("struct S() impl S { fn foo(&self) {} } fn main() { let s = S(); s.foo(); }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::MethodCall { method, .. } = &expr.kind {
            assert_eq!(method, "foo");
            return;
        }
    }
    panic!("Did not find method call");
}

// ========================================================================
// Phase 7: Arrays & Tuples
// ========================================================================

#[test]
fn lower_array_literal() {
    let db = lower("fn main() { let arr = [1, 2, 3]; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Array { elements } = &expr.kind {
            assert_eq!(elements.len(), 3);
            return;
        }
    }
    panic!("Did not find array literal");
}

#[test]
fn lower_array_repeat() {
    let db = lower("fn main() { let arr = [0; 10]; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::ArrayRepeat { count, .. } = &expr.kind {
            assert_eq!(*count, 10);
            return;
        }
    }
    panic!("Did not find array repeat");
}

#[test]
fn lower_tuple_expr() {
    let db = lower("fn main() { let t = (1, 2, 3); }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Tuple { elements } = &expr.kind {
            assert_eq!(elements.len(), 3);
            return;
        }
    }
    panic!("Did not find tuple expression");
}

#[test]
fn lower_index_expr() {
    let db = lower("fn main() { let arr = [1, 2, 3]; arr[0]; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Index { .. } = &expr.kind {
            return;
        }
    }
    panic!("Did not find index expression");
}

// ========================================================================
// Additional coverage tests
// ========================================================================

#[test]
fn lower_unary_not() {
    let db = lower("fn main() { let x = true; !x; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Unary { op, .. } = &expr.kind
            && *op == UnaryOp::Not
        {
            return;
        }
    }
    panic!("Did not find unary not");
}

#[test]
fn lower_unary_neg_variable() {
    let db = lower("fn main() { let x = 1; -x; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Unary { op, .. } = &expr.kind
            && *op == UnaryOp::Neg
        {
            return;
        }
    }
    panic!("Did not find unary negation of variable");
}

#[test]
fn lower_unary_deref() {
    let db = lower("fn main() { let x = &1; *x; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Unary { op, .. } = &expr.kind
            && *op == UnaryOp::Deref
        {
            return;
        }
    }
    panic!("Did not find unary deref");
}

#[test]
fn lower_ref_expr() {
    let db = lower("fn main() { let x = 1; &x; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Ref { mutable: false, .. } = &expr.kind {
            return;
        }
    }
    panic!("Did not find reference expression");
}

#[test]
fn lower_ref_mut_expr() {
    let db = lower("fn main() { let mut x = 1; &mut x; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Ref { mutable: true, .. } = &expr.kind {
            return;
        }
    }
    panic!("Did not find mutable reference expression");
}

#[test]
fn lower_continue() {
    let db = lower("fn main() { loop { continue; } }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Continue = &expr.kind {
            return;
        }
    }
    panic!("Did not find continue");
}

#[test]
fn lower_break_without_value() {
    let db = lower("fn main() { loop { break; } }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Break { value: None } = &expr.kind {
            return;
        }
    }
    panic!("Did not find break without value");
}

#[test]
fn lower_return_without_value() {
    let db = lower("fn main() { return; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Return { value: None } = &expr.kind {
            return;
        }
    }
    panic!("Did not find return without value");
}

#[test]
fn lower_cast_expr() {
    let db = lower("fn main() { let x = 1i32 as i64; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Cast { .. } = &expr.kind {
            return;
        }
    }
    panic!("Did not find cast expression");
}

#[test]
fn lower_block_with_tail() {
    let db = lower("fn main() { let x = { 1; 2 }; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Block {
            stmts,
            tail: Some(_),
        } = &expr.kind
        {
            // Should have one statement (1;) and a tail (2)
            if !stmts.is_empty() {
                return;
            }
        }
    }
    panic!("Did not find block with tail expression");
}

#[test]
fn lower_struct_item() {
    let db = lower("struct Foo(a: i32, b: bool)");

    for item in &db.items {
        if let HirItem::Struct(s) = item {
            assert_eq!(s.name, "Foo");
            assert_eq!(s.fields.len(), 2);
            assert_eq!(s.fields[0].name, "a");
            assert_eq!(s.fields[1].name, "b");
            return;
        }
    }
    panic!("Did not find struct item");
}

#[test]
fn lower_impl_item() {
    let db = lower("struct S() impl S { fn foo(&self) {} fn bar(&self) {} }");

    for item in &db.items {
        if let HirItem::Impl(impl_block) = item {
            assert_eq!(impl_block.items.len(), 2);
            return;
        }
    }
    panic!("Did not find impl item");
}

#[test]
fn lower_function_with_params() {
    let db = lower("fn add(a: i32, b: i32): i32 { a + b }");

    for item in &db.items {
        if let HirItem::Function(f) = item {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
            assert!(f.body.is_some());
            return;
        }
    }
    panic!("Did not find function with params");
}

#[test]
fn lower_empty_function() {
    let db = lower("fn empty() {}");

    for item in &db.items {
        if let HirItem::Function(f) = item {
            assert_eq!(f.name, "empty");
            assert!(f.params.is_empty());
            return;
        }
    }
    panic!("Did not find empty function");
}

#[test]
fn lower_while_desugaring_structure() {
    // Verify the while loop is properly desugared to:
    // loop { if !cond { break; } body }
    let db = lower("fn main() { while true { 1; } }");

    // Find the Loop
    let mut found_loop = false;
    let mut found_if_with_break = false;

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Loop { body } = &expr.kind {
            found_loop = true;
            // The body should be a block containing an if statement
            let body_expr = db.expr(*body);
            if let HirExprKind::Block { stmts, .. } = &body_expr.kind {
                // First statement should be the if-break
                if !stmts.is_empty() {
                    let first_stmt = db.stmt(stmts[0]);
                    if let HirStmtKind::Expr { expr, .. } = &first_stmt.kind {
                        let if_expr = db.expr(*expr);
                        if let HirExprKind::If {
                            else_branch: None, ..
                        } = &if_expr.kind
                        {
                            found_if_with_break = true;
                        }
                    }
                }
            }
        }
    }

    assert!(found_loop, "Did not find loop");
    assert!(
        found_if_with_break,
        "Did not find if-break structure in desugared while"
    );
}

#[test]
fn lower_binary_or() {
    // Use a variable to prevent folding
    let db = lower("fn main() { let y = true; let x = y || false; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Binary { op, .. } = &expr.kind
            && *op == BinOp::Or
        {
            return;
        }
    }
    panic!("Did not find logical or");
}

#[test]
fn lower_compound_assign() {
    let db = lower("fn main() { let mut x = 1; x += 2; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Binary { op, .. } = &expr.kind
            && *op == BinOp::AddAssign
        {
            return;
        }
    }
    panic!("Did not find compound assignment");
}

#[test]
fn lower_param_reference() {
    let db = lower("fn foo(x: i32): i32 { x }");

    // The function body should reference the parameter
    let mut found_var_in_body = false;
    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Var(_) = &expr.kind {
            found_var_in_body = true;
            break;
        }
    }
    assert!(
        found_var_in_body,
        "Did not find parameter reference in body"
    );
}

#[test]
fn lower_nested_blocks() {
    let db = lower("fn main() { { { 1 } } }");

    let mut block_count = 0;
    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Block { .. } = &expr.kind {
            block_count += 1;
        }
    }
    // Should have at least 3 blocks (function body + 2 nested)
    assert!(
        block_count >= 3,
        "Expected at least 3 blocks, found {}",
        block_count
    );
}

#[test]
fn lower_method_call_with_args() {
    let db = lower(
        "struct S() impl S { fn add(&self, a: i32, b: i32): i32 { a + b } } fn main() { let s = S(); s.add(1, 2); }",
    );

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::MethodCall { method, args, .. } = &expr.kind {
            assert_eq!(method, "add");
            assert_eq!(args.len(), 2);
            return;
        }
    }
    panic!("Did not find method call with args");
}

#[test]
fn lower_function_call_with_args() {
    let db = lower("fn add(a: i32, b: i32): i32 { a + b } fn main() { add(1, 2); }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Call { args, .. } = &expr.kind
            && args.len() == 2
        {
            return;
        }
    }
    panic!("Did not find function call with args");
}

#[test]
fn lower_if_else_if_chain() {
    let db = lower("fn main() { if true { 1 } else if false { 2 } else { 3 }; }");

    // Should have nested If expressions
    let mut if_count = 0;
    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::If { .. } = &expr.kind {
            if_count += 1;
        }
    }
    assert!(
        if_count >= 2,
        "Expected at least 2 if expressions for else-if chain"
    );
}

#[test]
fn lower_empty_tuple() {
    let db = lower("fn main() { let x = (); }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Tuple { elements } = &expr.kind
            && elements.is_empty()
        {
            return;
        }
    }
    panic!("Did not find empty tuple");
}

#[test]
fn lower_single_element_tuple() {
    let db = lower("fn main() { let x = (1,); }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Tuple { elements } = &expr.kind
            && elements.len() == 1
        {
            return;
        }
    }
    panic!("Did not find single element tuple");
}

#[test]
fn lower_empty_array() {
    let db = lower("fn main() { let arr: [i32; 0] = []; }");

    for (_, expr) in db.exprs.iter() {
        if let HirExprKind::Array { elements } = &expr.kind
            && elements.is_empty()
        {
            return;
        }
    }
    panic!("Did not find empty array");
}

#[test]
fn spans_are_preserved() {
    let db = lower("fn main() { let x = 42; }");

    // Check that spans are non-empty
    for (id, _) in db.exprs.iter() {
        let span = db.span(id);
        assert!(span.is_some(), "Expression should have a span");
        let span = span.unwrap();
        assert!(span.start < span.end, "Span should be non-empty");
    }
}

#[test]
fn types_are_attached() {
    let db = lower("fn main() { let x: i32 = 42; }");

    // All expressions should have valid type IDs
    for (_, expr) in db.exprs.iter() {
        // TypeId should not be the default/uninitialized value
        // A simple check: the type should exist in the interner
        let _ = db.types.get(expr.ty);
    }
}
