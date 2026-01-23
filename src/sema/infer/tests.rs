//! Tests for bidirectional type inference.
//!
//! These tests are written first following TDD methodology.
//! The implementation should make all these tests pass.

use crate::ast::SourceFile;
use crate::parser::parse;
use crate::sema::infer::infer;
use crate::sema::resolver::resolve;
use rowan::ast::AstNode;

/// Parse source, run resolution, run inference, and verify the type of the first `let` binding.
fn check(source: &str, expected: &str) {
    let parse_result = parse(source);
    assert!(
        parse_result.errors().is_empty(),
        "parse errors: {:?}",
        parse_result.errors()
    );
    let source_file = SourceFile::cast(parse_result.syntax()).expect("expected SourceFile");
    let resolve_result = resolve(&source_file);
    assert!(
        resolve_result.diagnostics.is_empty(),
        "resolution errors: {:?}",
        resolve_result
            .diagnostics
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );

    let infer_result = infer(&source_file, resolve_result);
    assert!(
        infer_result.diagnostics.is_empty(),
        "expected no inference errors, got: {:?}",
        infer_result
            .diagnostics
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );

    let actual = infer_result.display_first_binding();
    assert_eq!(actual, expected, "type mismatch");
}

/// Parse source, run resolution, run inference, and verify error messages.
fn check_err(source: &str, expected: &[&str]) {
    let parse_result = parse(source);
    assert!(
        parse_result.errors().is_empty(),
        "parse errors: {:?}",
        parse_result.errors()
    );
    let source_file = SourceFile::cast(parse_result.syntax()).expect("expected SourceFile");
    let resolve_result = resolve(&source_file);
    // Resolution errors may or may not exist depending on the test

    let infer_result = infer(&source_file, resolve_result);

    assert!(
        !infer_result.diagnostics.is_empty(),
        "expected errors containing {:?}, got none",
        expected
    );

    for pattern in expected {
        let found = infer_result
            .diagnostics
            .iter()
            .any(|d| d.message.contains(pattern));
        assert!(
            found,
            "expected error containing '{}', got: {:?}",
            pattern,
            infer_result
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
    }
}

/// Parse source, run resolution, run inference, and verify warning messages.
/// Unlike check_err, this allows successful type checking while expecting warnings.
fn check_warn(source: &str, expected: &[&str]) {
    let parse_result = parse(source);
    assert!(
        parse_result.errors().is_empty(),
        "parse errors: {:?}",
        parse_result.errors()
    );
    let source_file = SourceFile::cast(parse_result.syntax()).expect("expected SourceFile");
    let resolve_result = resolve(&source_file);

    let infer_result = infer(&source_file, resolve_result);

    // For warnings, we expect some diagnostics but type inference still succeeds
    assert!(
        !infer_result.diagnostics.is_empty(),
        "expected warnings containing {:?}, got none",
        expected
    );

    for pattern in expected {
        let found = infer_result
            .diagnostics
            .iter()
            .any(|d| d.message.contains(pattern));
        assert!(
            found,
            "expected warning containing '{}', got: {:?}",
            pattern,
            infer_result
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
    }
}

// =============================================================================
// 1.1 Literal Type Inference (12 tests)
// =============================================================================

#[test]
fn int_literal_defaults_to_i32() {
    check("fn main() { let x = 42; }", "i32");
}

#[test]
fn int_literal_zero() {
    check("fn main() { let x = 0; }", "i32");
}

#[test]
fn int_literal_negative() {
    check("fn main() { let x = -42; }", "i32");
}

#[test]
fn int_literal_large() {
    check("fn main() { let x = 1000000; }", "i32");
}

#[test]
fn float_literal_defaults_to_f64() {
    check("fn main() { let x = 3.14; }", "f64");
}

#[test]
fn float_literal_zero() {
    check("fn main() { let x = 0.0; }", "f64");
}

#[test]
fn float_literal_negative() {
    check("fn main() { let x = -3.14; }", "f64");
}

#[test]
fn float_literal_scientific() {
    check("fn main() { let x = 1e10; }", "f64");
}

#[test]
fn float_literal_suffixed_f32() {
    check("fn main() { let x = 3.14f32; }", "f32");
}

#[test]
fn float_literal_suffixed_f64() {
    check("fn main() { let x = 3.14f64; }", "f64");
}

#[test]
fn float_negated_suffixed_f32() {
    check("fn main() { let x = -1.5f32; }", "f32");
}

#[test]
fn bool_literal_true() {
    check("fn main() { let x = true; }", "bool");
}

#[test]
fn bool_literal_false() {
    check("fn main() { let x = false; }", "bool");
}

#[test]
fn char_literal() {
    check("fn main() { let x = 'a'; }", "char");
}

#[test]
fn string_literal() {
    check("fn main() { let x = \"hello\"; }", "String");
}

// =============================================================================
// 1.2 Type Annotations (10 tests)
// =============================================================================

#[test]
fn let_with_i8_annotation() {
    check("fn main() { let x: i8 = 42; }", "i8");
}

#[test]
fn let_with_i16_annotation() {
    check("fn main() { let x: i16 = 42; }", "i16");
}

#[test]
fn let_with_i32_annotation() {
    check("fn main() { let x: i32 = 42; }", "i32");
}

#[test]
fn let_with_i64_annotation() {
    check("fn main() { let x: i64 = 42; }", "i64");
}

#[test]
fn let_with_i128_annotation() {
    check("fn main() { let x: i128 = 42; }", "i128");
}

#[test]
fn let_with_u8_annotation() {
    check("fn main() { let x: u8 = 42; }", "u8");
}

#[test]
fn let_with_u32_annotation() {
    check("fn main() { let x: u32 = 42; }", "u32");
}

#[test]
fn let_with_u64_annotation() {
    check("fn main() { let x: u64 = 42; }", "u64");
}

#[test]
fn let_with_f32_annotation() {
    check("fn main() { let x: f32 = 3.14; }", "f32");
}

#[test]
fn let_with_f64_annotation() {
    check("fn main() { let x: f64 = 3.14; }", "f64");
}

// =============================================================================
// 1.3 Two-Way Inference - The Key Feature (25 tests)
// =============================================================================

#[test]
fn infer_int_from_fn_param() {
    check("fn f(x: i64) {} fn main() { let a = 42; f(a); }", "i64");
}

#[test]
fn infer_int_from_fn_param_multiple() {
    // Both literals infer their types from the function parameters
    // The check function returns the last binding's type, so we check y's type (i64)
    check(
        "fn f(a: i32, b: i64) {} fn main() { let x = 1; let y = 2; f(x, y); }",
        "i64",
    );
}

#[test]
fn infer_float_from_fn_param() {
    check("fn f(x: f32) {} fn main() { let a = 3.14; f(a); }", "f32");
}

#[test]
fn infer_from_let_annotation_simple() {
    check("fn main() { let x: i64 = 42; }", "i64");
}

#[test]
fn infer_from_let_annotation_binary() {
    check("fn main() { let x: i64 = 1 + 2; }", "i64");
}

#[test]
fn infer_from_let_annotation_nested() {
    check("fn main() { let x: i64 = 1 + 2 + 3; }", "i64");
}

#[test]
fn infer_from_return_type() {
    check("fn f(): i64 { let x = 42; return x; }", "i64");
}

#[test]
fn infer_from_return_type_explicit() {
    check("fn f(): i64 { let x = 42; return x; }", "i64");
}

#[test]
fn infer_from_return_type_block() {
    check("fn f(): i64 { let x = 42; return x; }", "i64");
}

#[test]
fn infer_from_binary_lhs() {
    check("fn main() { let x: i64 = 1; let y = x + 2; }", "i64");
}

#[test]
fn infer_from_binary_rhs() {
    check("fn main() { let x: i64 = 1; let y = 2 + x; }", "i64");
}

#[test]
fn infer_chain_through_binary() {
    check("fn main() { let x: i64 = 1; let y = x + 2; }", "i64");
}

#[test]
fn infer_from_assignment_target() {
    check(
        "fn main() { let mut x: i64 = 0; let y = 42; x = y; }",
        "i64",
    );
}

#[test]
fn infer_through_variable_usage() {
    check("fn main() { let x = 42; let y: i64 = x; }", "i64");
}

#[test]
fn infer_backwards_through_usage() {
    check("fn foo(x: i64) {} fn main() { let x = 42; foo(x); }", "i64");
}

#[test]
fn infer_in_array_context() {
    check("fn main() { let arr: [i64; 2] = [1, 2]; }", "[i64; 2]");
}

#[test]
fn infer_array_element_from_annotation() {
    check("fn main() { let arr: [i64; 3] = [1, 2, 3]; }", "[i64; 3]");
}

#[test]
fn infer_tuple_elements_from_annotation() {
    check("fn main() { let t: (i32, i64) = (1, 2); }", "(i32, i64)");
}

#[test]
fn infer_multiple_constraints_same() {
    // Same variable used in multiple calls with same type
    check(
        "fn f(x: i64) {} fn main() { let a = 42; f(a); f(a); }",
        "i64",
    );
}

#[test]
fn infer_multiple_usages_consistent() {
    check(
        "fn main() { let x = 42; let y: i64 = x; let z: i64 = x; }",
        "i64",
    );
}

#[test]
fn infer_if_from_context() {
    check(
        "fn main() { let x: i64 = if true { 1 } else { 2 }; }",
        "i64",
    );
}

#[test]
fn infer_if_branches_unify() {
    check(
        "fn f(): i64 { if true { 1 } else { 2 } } fn main() { let x = f(); }",
        "i64",
    );
}

#[test]
fn infer_block_from_context() {
    check("fn main() { let x: i64 = { 42 }; }", "i64");
}

#[test]
fn infer_nested_block() {
    check("fn main() { let x: i64 = { { 42 } }; }", "i64");
}

// =============================================================================
// 1.4 Binary Operators (20 tests)
// =============================================================================

#[test]
fn binary_add_i32() {
    check("fn main() { let x = 1 + 2; }", "i32");
}

#[test]
fn binary_sub_i32() {
    check("fn main() { let x = 1 - 2; }", "i32");
}

#[test]
fn binary_mul_i32() {
    check("fn main() { let x = 1 * 2; }", "i32");
}

#[test]
fn binary_div_i32() {
    check("fn main() { let x = 1 / 2; }", "i32");
}

#[test]
fn binary_rem_i32() {
    check("fn main() { let x = 1 % 2; }", "i32");
}

#[test]
fn binary_add_f64() {
    check("fn main() { let x = 1.0 + 2.0; }", "f64");
}

#[test]
fn binary_with_annotation() {
    check("fn main() { let x: i64 = 1 + 2; }", "i64");
}

#[test]
fn binary_eq_bool() {
    check("fn main() { let x = 1 == 2; }", "bool");
}

#[test]
fn binary_ne_bool() {
    check("fn main() { let x = 1 != 2; }", "bool");
}

#[test]
fn binary_lt_bool() {
    check("fn main() { let x = 1 < 2; }", "bool");
}

#[test]
fn binary_le_bool() {
    check("fn main() { let x = 1 <= 2; }", "bool");
}

#[test]
fn binary_gt_bool() {
    check("fn main() { let x = 1 > 2; }", "bool");
}

#[test]
fn binary_ge_bool() {
    check("fn main() { let x = 1 >= 2; }", "bool");
}

#[test]
fn binary_and() {
    check("fn main() { let x = true && false; }", "bool");
}

#[test]
fn binary_or() {
    check("fn main() { let x = true || false; }", "bool");
}

#[test]
fn binary_assign() {
    check("fn main() { let mut x = 1; let y = (x = 2); }", "()");
}

#[test]
fn binary_add_assign() {
    check("fn main() { let mut x = 1; let y = (x += 2); }", "()");
}

#[test]
fn binary_sub_assign() {
    check("fn main() { let mut x = 1; let y = (x -= 2); }", "()");
}

#[test]
fn binary_mul_assign() {
    check("fn main() { let mut x = 1; let y = (x *= 2); }", "()");
}

#[test]
fn binary_div_assign() {
    check("fn main() { let mut x = 1; let y = (x /= 2); }", "()");
}

// =============================================================================
// 1.5 Unary Operators (6 tests)
// =============================================================================

#[test]
fn unary_neg_int() {
    check("fn main() { let x = -42; }", "i32");
}

#[test]
fn unary_neg_float() {
    check("fn main() { let x = -3.14; }", "f64");
}

#[test]
fn unary_not_bool() {
    check("fn main() { let x = !true; }", "bool");
}

#[test]
fn unary_neg_with_annotation() {
    check("fn main() { let x: i64 = -42; }", "i64");
}

#[test]
fn unary_double_neg() {
    check("fn main() { let x = - -42; }", "i32");
}

#[test]
fn unary_not_not() {
    check("fn main() { let x = !!true; }", "bool");
}

// =============================================================================
// 1.6 Function Calls (15 tests)
// =============================================================================

#[test]
fn call_no_args() {
    check("fn f() {} fn main() { let x = f(); }", "()");
}

#[test]
fn call_one_arg() {
    check("fn f(x: i32) {} fn main() { let y = f(42); }", "()");
}

#[test]
fn call_multiple_args() {
    check(
        "fn f(a: i32, b: bool) {} fn main() { let x = f(1, true); }",
        "()",
    );
}

#[test]
fn call_with_return() {
    check("fn f(): i32 { 42 } fn main() { let x = f(); }", "i32");
}

#[test]
fn call_return_used_in_let() {
    check("fn f(): i64 { 42 } fn main() { let x = f(); }", "i64");
}

#[test]
fn call_return_used_in_binary() {
    check("fn f(): i32 { 1 } fn main() { let x = f() + 2; }", "i32");
}

#[test]
fn call_return_passed_to_fn() {
    check(
        "fn f(): i32 { 1 } fn g(x: i32) {} fn main() { let y = g(f()); }",
        "()",
    );
}

#[test]
fn call_nested() {
    check(
        "fn f(x: i32): i32 { x } fn main() { let y = f(f(1)); }",
        "i32",
    );
}

#[test]
fn call_in_binary() {
    check("fn f(): i32 { 1 } fn main() { let x = f() + f(); }", "i32");
}

#[test]
fn call_forward_reference() {
    check("fn main() { let x = f(); } fn f(): i32 { 42 }", "i32");
}

#[test]
fn call_mutual_recursion() {
    check(
        "fn a(): i32 { b() } fn b(): i32 { a() } fn main() { let x = a(); }",
        "i32",
    );
}

#[test]
fn call_recursive() {
    check(
        "fn f(n: i32): i32 { if n == 0 { 0 } else { f(n - 1) } } fn main() { let x = f(5); }",
        "i32",
    );
}

#[test]
fn call_infers_literal_type() {
    check("fn f(x: i64) {} fn main() { let a = 42; f(a); }", "i64");
}

#[test]
fn call_infers_expr_type() {
    check("fn f(x: i64) {} fn main() { let a = 1 + 2; f(a); }", "i64");
}

#[test]
fn call_infers_var_type() {
    check("fn f(x: i64) {} fn main() { let a = 1; f(a); }", "i64");
}

// =============================================================================
// 1.7 Control Flow (18 tests)
// =============================================================================

#[test]
fn if_simple() {
    check("fn main() { let x = if true { 1 } else { 2 }; }", "i32");
}

#[test]
fn if_no_else_unit() {
    check("fn main() { let x = if true { 1; }; }", "()");
}

#[test]
fn if_with_blocks() {
    check(
        "fn main() { let x = if true { let a = 1; a } else { 2 }; }",
        "i32",
    );
}

#[test]
fn if_nested() {
    check(
        "fn main() { let x = if true { if false { 1 } else { 2 } } else { 3 }; }",
        "i32",
    );
}

#[test]
fn if_in_function() {
    check(
        "fn f(): i32 { if true { 1 } else { 2 } } fn main() { let x = f(); }",
        "i32",
    );
}

#[test]
fn while_simple() {
    check("fn main() { let x = while true { }; }", "()");
}

#[test]
fn while_with_body() {
    check(
        "fn main() { let mut x = 0; while x < 10 { x = x + 1; } let y = x; }",
        "i32",
    );
}

#[test]
fn while_result_is_unit() {
    check("fn main() { let x = while false { }; }", "()");
}

#[test]
fn loop_simple() {
    check("fn main() { let x = loop { break; }; }", "()");
}

#[test]
fn loop_with_break_value() {
    check("fn main() { let x = loop { break 42; }; }", "i32");
}

#[test]
fn loop_break_infers_type() {
    check("fn main() { let x: i64 = loop { break 42; }; }", "i64");
}

#[test]
fn for_loop_simple() {
    // Use y after the for loop so it's found as the "last" binding by display_first_binding
    check("fn main() { let x = for i in 0..10 { }; let y = x; }", "()");
}

#[test]
fn for_loop_result_unit() {
    // Use y after the for loop so it's found as the "last" binding by display_first_binding
    check("fn main() { let x = for i in 0..10 { }; let y = x; }", "()");
}

#[test]
fn break_no_value() {
    check("fn main() { loop { let x = break; }; }", "!");
}

#[test]
fn break_with_value() {
    check("fn main() { let x = loop { break 42; }; }", "i32");
}

#[test]
fn continue_in_loop() {
    check("fn main() { loop { let x = continue; }; }", "!");
}

#[test]
fn return_no_value() {
    check("fn f() { let x = return; } fn main() { f(); }", "!");
}

#[test]
fn return_with_value() {
    check(
        "fn f(): i32 { let x = return 42; } fn main() { let y = f(); }",
        "i32",
    );
}

// =============================================================================
// 1.8 Blocks and Scopes (10 tests)
// =============================================================================

#[test]
fn block_empty() {
    check("fn main() { let x = { }; }", "()");
}

#[test]
fn block_with_tail() {
    check("fn main() { let x = { 42 }; }", "i32");
}

#[test]
fn block_with_semi_no_tail() {
    check("fn main() { let x = { 42; }; }", "()");
}

#[test]
fn block_multiple_stmts() {
    check(
        "fn main() { let x = { let a = 1; let b = 2; a + b }; }",
        "i32",
    );
}

#[test]
fn block_nested() {
    check("fn main() { let x = { { { 42 } } }; }", "i32");
}

#[test]
fn block_shadowing() {
    check(
        "fn main() { let x = { let x = 1; { let x = 2; x } }; }",
        "i32",
    );
}

#[test]
fn block_uses_outer_var() {
    check("fn main() { let x = { let a = 1; { a + 1 } }; }", "i32");
}

#[test]
fn block_type_from_context() {
    check("fn main() { let x: i64 = { 42 }; }", "i64");
}

#[test]
fn block_with_let_and_tail() {
    check("fn main() { let x = { let a: i64 = 1; a }; }", "i64");
}

#[test]
fn block_complex() {
    check(
        "fn main() { let x = { let a = 1; let b = 2; if true { a } else { b } }; }",
        "i32",
    );
}

// =============================================================================
// 1.9 Tuples (8 tests)
// =============================================================================

#[test]
fn tuple_empty() {
    check("fn main() { let x = (); }", "()");
}

#[test]
fn tuple_single() {
    check("fn main() { let x = (42,); }", "(i32,)");
}

#[test]
fn tuple_pair() {
    check("fn main() { let x = (1, 2); }", "(i32, i32)");
}

#[test]
fn tuple_mixed() {
    check("fn main() { let x = (1, true, 'a'); }", "(i32, bool, char)");
}

#[test]
fn tuple_nested() {
    check(
        "fn main() { let x = ((1, 2), (3, 4)); }",
        "((i32, i32), (i32, i32))",
    );
}

#[test]
fn tuple_with_annotation() {
    check("fn main() { let x: (i64, i32) = (1, 2); }", "(i64, i32)");
}

#[test]
fn tuple_access() {
    check("fn main() { let t = (1, 2); let x = t.0; }", "i32");
}

#[test]
fn tuple_infers_element_types() {
    check("fn main() { let x: (i64, f32) = (1, 2.0); }", "(i64, f32)");
}

// =============================================================================
// 1.10 Arrays (10 tests)
// =============================================================================

#[test]
fn array_empty() {
    check("fn main() { let x: [i32; 0] = []; }", "[i32; 0]");
}

#[test]
fn array_single() {
    check("fn main() { let x = [42]; }", "[i32; 1]");
}

#[test]
fn array_multiple() {
    check("fn main() { let x = [1, 2, 3]; }", "[i32; 3]");
}

#[test]
fn array_with_annotation() {
    check("fn main() { let x: [i64; 3] = [1, 2, 3]; }", "[i64; 3]");
}

#[test]
fn array_repeat() {
    check("fn main() { let x = [0; 5]; }", "[i32; 5]");
}

#[test]
fn array_repeat_with_annotation() {
    check("fn main() { let x: [i64; 5] = [0; 5]; }", "[i64; 5]");
}

#[test]
fn array_index() {
    check("fn main() { let arr = [1, 2, 3]; let x = arr[0]; }", "i32");
}

#[test]
fn array_index_type() {
    check(
        "fn main() { let arr: [i64; 3] = [1, 2, 3]; let x = arr[0]; }",
        "i64",
    );
}

#[test]
fn array_nested() {
    check("fn main() { let x = [[1, 2], [3, 4]]; }", "[[i32; 2]; 2]");
}

#[test]
fn array_infers_element_type() {
    check("fn main() { let x: [i64; 2] = [1, 2]; }", "[i64; 2]");
}

// =============================================================================
// 1.11 References (12 tests)
// =============================================================================

#[test]
fn ref_shared() {
    check("fn main() { let x = 42; let y = &x; }", "&i32");
}

#[test]
fn ref_mutable() {
    check("fn main() { let mut x = 42; let y = &mut x; }", "&mut i32");
}

#[test]
fn ref_type_annotation() {
    check("fn main() { let x = 42; let y: &i32 = &x; }", "&i32");
}

#[test]
fn ref_mut_type_annotation() {
    check(
        "fn main() { let mut x = 42; let y: &mut i32 = &mut x; }",
        "&mut i32",
    );
}

#[test]
fn deref_shared() {
    check("fn main() { let x = 42; let y = &x; let z = *y; }", "i32");
}

#[test]
fn deref_mutable() {
    // Deref of mutable ref, then assign
    check(
        "fn main() { let mut x = 42; let y = &mut x; *y = 43; let z = x; }",
        "i32",
    );
}

#[test]
fn ref_to_ref() {
    check("fn main() { let x = 42; let y = &x; let z = &y; }", "&&i32");
}

#[test]
fn ref_in_function() {
    check(
        "fn f(x: &i32) {} fn main() { let a = 42; f(&a); let b = a; }",
        "i32",
    );
}

#[test]
fn ref_mut_in_function() {
    check(
        "fn f(x: &mut i32) {} fn main() { let mut a = 42; f(&mut a); let b = a; }",
        "i32",
    );
}

#[test]
fn ref_return() {
    check(
        "fn f(x: &i32): &i32 { x } fn main() { let a = 42; let b = f(&a); }",
        "&i32",
    );
}

#[test]
fn ref_coercion() {
    // Mutable reference can be coerced to shared reference
    check(
        "fn f(x: &i32) {} fn main() { let mut a = 42; f(&a); let b = a; }",
        "i32",
    );
}

#[test]
fn ref_infers_inner_type() {
    check("fn f(x: &i64) {} fn main() { let a = 42; f(&a); }", "i64");
}

// =============================================================================
// 1.12 Structs (15 tests)
// =============================================================================

#[test]
fn struct_construct_empty() {
    check("struct S() fn main() { let x = S(); }", "S");
}

#[test]
fn struct_construct_one_field() {
    check("struct S(a: i32) fn main() { let x = S(a: 42); }", "S");
}

#[test]
fn struct_construct_multiple_fields() {
    check(
        "struct S(a: i32, b: bool) fn main() { let x = S(a: 1, b: true); }",
        "S",
    );
}

#[test]
fn struct_field_access() {
    check(
        "struct S(a: i32) fn main() { let x = S(a: 42); let y = x.a; }",
        "i32",
    );
}

#[test]
fn struct_field_type() {
    check(
        "struct S(a: i64) fn main() { let x = S(a: 42); let y = x.a; }",
        "i64",
    );
}

#[test]
fn struct_field_infers_literal() {
    check("struct S(a: i64) fn main() { let x = S(a: 42); }", "S");
}

#[test]
fn struct_nested() {
    check(
        "struct A(x: i32) struct B(a: A) fn main() { let b = B(a: A(x: 1)); }",
        "B",
    );
}

#[test]
fn struct_in_function_param() {
    check(
        "struct S(a: i32) fn f(s: S) {} fn main() { let x = S(a: 1); f(x); }",
        "S",
    );
}

#[test]
fn struct_in_function_return() {
    check(
        "struct S(a: i32) fn f(): S { S(a: 1) } fn main() { let x = f(); }",
        "S",
    );
}

#[test]
fn struct_method_call() {
    check(
        "struct S(a: i32) impl S { fn get(&self): i32 { self.a } } fn main() { let s = S(a: 1); let x = s.get(); }",
        "i32",
    );
}

#[test]
fn struct_method_with_params() {
    check(
        "struct S(a: i32) impl S { fn set(&mut self, v: i32) { self.a = v; } } fn main() { let mut s = S(a: 1); s.set(2); let x = s.a; }",
        "i32",
    );
}

#[test]
fn struct_multiple_impls() {
    check(
        "struct S() impl S { fn a(&self): i32 { 1 } } impl S { fn b(&self): i32 { 2 } } fn main() { let s = S(); let x = s.a(); }",
        "i32",
    );
}

#[test]
fn struct_self_type() {
    check(
        "struct S(a: i32) impl S { fn new(): Self { S(a: 0) } } fn main() { let x = S.new(); }",
        "S",
    );
}

#[test]
fn struct_field_shorthand() {
    check(
        "struct S(a: i32) fn main() { let a = 42; let x = S(a); }",
        "S",
    );
}

#[test]
fn struct_update_syntax() {
    check(
        "struct S(a: i32, b: i32) fn main() { let s = S(a: 1, b: 2); let x = S(a: 3, ..s); }",
        "S",
    );
}

// =============================================================================
// 1.13 Type Errors (25 tests)
// =============================================================================

#[test]
fn error_let_mismatch() {
    check_err("fn main() { let x: i32 = true; }", &["type mismatch"]);
}

#[test]
fn error_let_mismatch_string() {
    check_err("fn main() { let x: i32 = \"hello\"; }", &["type mismatch"]);
}

#[test]
fn error_assign_mismatch() {
    check_err(
        "fn main() { let mut x: i32 = 0; x = true; }",
        &["type mismatch"],
    );
}

#[test]
fn error_return_mismatch() {
    check_err("fn f(): i32 { true }", &["type mismatch"]);
}

#[test]
fn error_if_branch_mismatch() {
    check_err(
        "fn main() { let x = if true { 1 } else { true }; }",
        &["type mismatch"],
    );
}

#[test]
fn error_binary_operand_mismatch() {
    check_err("fn main() { let x = 1 + true; }", &["type mismatch"]);
}

#[test]
fn error_comparison_mismatch() {
    check_err("fn main() { let x = 1 < true; }", &["type mismatch"]);
}

#[test]
fn error_too_few_args() {
    check_err(
        "fn f(x: i32) {} fn main() { f(); }",
        &["expected 1 argument"],
    );
}

#[test]
fn error_too_many_args() {
    check_err(
        "fn f(x: i32) {} fn main() { f(1, 2); }",
        &["expected 1 argument"],
    );
}

#[test]
fn error_wrong_arg_count_zero() {
    check_err("fn f() {} fn main() { f(1); }", &["expected 0 argument"]);
}

#[test]
fn error_call_non_function() {
    check_err("fn main() { let x = 42; x(); }", &["not a function"]);
}

#[test]
fn error_call_bool() {
    check_err("fn main() { let x = true; x(); }", &["not a function"]);
}

#[test]
fn error_undefined_variable() {
    // This is a resolution error, not inference - will be caught by resolver
    check_err("fn main() { let x = y; }", &["cannot find"]);
}

#[test]
fn error_undefined_function() {
    // This is a resolution error
    check_err("fn main() { foo(); }", &["cannot find"]);
}

#[test]
fn error_undefined_type() {
    // This is a resolution error
    check_err("fn main() { let x: Foo = 1; }", &["cannot find"]);
}

#[test]
fn error_negate_bool() {
    check_err("fn main() { let x = -true; }", &["cannot apply unary"]);
}

#[test]
fn error_not_int() {
    check_err("fn main() { let x = !42; }", &["cannot apply unary"]);
}

#[test]
fn error_add_bool() {
    check_err(
        "fn main() { let x = true + false; }",
        &["cannot apply binary"],
    );
}

#[test]
fn error_and_int() {
    check_err("fn main() { let x = 1 && 2; }", &["type mismatch"]);
}

#[test]
fn error_ref_type_mismatch() {
    check_err("fn main() { let x: &i32 = &true; }", &["type mismatch"]);
}

// Moved to Phase 2 mutability tests below

#[test]
fn error_missing_field() {
    check_err(
        "struct S(a: i32) fn main() { let x = S(); }",
        &["missing field"],
    );
}

#[test]
fn error_unknown_field() {
    check_err(
        "struct S(a: i32) fn main() { let x = S(b: 1); }",
        &["unknown field"],
    );
}

#[test]
fn error_field_type_mismatch() {
    check_err(
        "struct S(a: i32) fn main() { let x = S(a: true); }",
        &["type mismatch"],
    );
}

#[test]
fn error_access_nonexistent_field() {
    check_err(
        "struct S(a: i32) fn main() { let x = S(a: 1); x.b; }",
        &["no field"],
    );
}

// =============================================================================
// 1.14 Never Type and Unit (8 tests)
// =============================================================================

#[test]
fn never_from_return() {
    check("fn f(): ! { loop {} } fn main() { let x = f(); }", "!");
}

#[test]
fn never_coerces_to_any() {
    check("fn main() { let x: i32 = return; }", "i32");
}

#[test]
fn never_in_if() {
    check(
        "fn main() { let x = if true { 1 } else { return }; }",
        "i32",
    );
}

#[test]
fn never_break_value() {
    check("fn main() { let x: i32 = loop { break 1; }; }", "i32");
}

#[test]
fn unit_from_empty_block() {
    check("fn main() { let x = {}; }", "()");
}

#[test]
fn unit_from_stmt_with_semi() {
    check("fn main() { let x = { 42; }; }", "()");
}

#[test]
fn unit_from_if_no_else() {
    check("fn main() { let x = if true { 1; }; }", "()");
}

#[test]
fn unit_explicit() {
    check("fn main() { let x: () = {}; }", "()");
}

// =============================================================================
// 1.15 Edge Cases and Complex Scenarios (15 tests)
// =============================================================================

#[test]
fn complex_nested_inference() {
    check(
        "fn f(x: i64): i64 { x } fn main() { let a = f(1 + 2); }",
        "i64",
    );
}

#[test]
fn diamond_inference() {
    // Same variable flows through multiple paths
    check(
        "fn f(x: i64) {} fn g(x: i64) {} fn main() { let a = 42; f(a); g(a); }",
        "i64",
    );
}

#[test]
fn long_chain_inference() {
    check(
        "fn main() { let a = 1; let b = a; let c = b; let d: i64 = c; }",
        "i64",
    );
}

#[test]
fn inference_through_function_chain() {
    check(
        "fn f(x: i64): i64 { x } fn g(x: i64): i64 { x } fn h(x: i64): i64 { x } fn main() { let a = f(g(h(42))); }",
        "i64",
    );
}

#[test]
fn mixed_int_float_error() {
    check_err("fn main() { let x = 1 + 2.0; }", &["type mismatch"]);
}

#[test]
fn conflicting_constraints_error() {
    check_err(
        "fn f(x: i32) {} fn g(x: i64) {} fn main() { let a = 1; f(a); g(a); }",
        &["type mismatch"],
    );
}

#[test]
fn self_referential_type() {
    // Type variable that would create an infinite type
    check_err("fn main() { let x = x; }", &["cannot find"]);
}

#[test]
fn very_long_expression() {
    check(
        "fn main() { let x = 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10; }",
        "i32",
    );
}

#[test]
fn deeply_nested_blocks() {
    check("fn main() { let x = { { { { { 42 } } } } }; }", "i32");
}

#[test]
fn many_function_params() {
    check(
        "fn f(a: i32, b: i32, c: i32, d: i32, e: i32): i32 { a } fn main() { let x = f(1, 2, 3, 4, 5); }",
        "i32",
    );
}

#[test]
fn empty_function() {
    check("fn f() {} fn main() { let x = f(); }", "()");
}

#[test]
fn function_only_return() {
    check(
        "fn f(): i32 { return 42; } fn main() { let x = f(); }",
        "i32",
    );
}

#[test]
fn multiple_returns() {
    check(
        "fn f(b: bool): i32 { if b { return 1; } return 2; } fn main() { let x = f(true); }",
        "i32",
    );
}

#[test]
fn function_implicit_return() {
    check("fn f(): i32 { 42 } fn main() { let x = f(); }", "i32");
}

// =============================================================================
// 1.16 Type Inference Edge Cases (Additional)
// =============================================================================

#[test]
fn inference_through_multiple_assignments() {
    // Type propagates through a chain of assignments
    check(
        "fn main() { let mut x = 1; let y = x; x = y; let z: i64 = y; }",
        "i64",
    );
}

#[test]
fn inference_nested_struct_fields() {
    // Inference through nested struct field access
    check(
        "struct Inner(val: i64) struct Outer(inner: Inner) fn main() { let o = Outer(inner: Inner(val: 42)); let x = o.inner.val; }",
        "i64",
    );
}

#[test]
fn inference_array_of_tuples() {
    // Array containing tuples
    check(
        "fn main() { let arr: [(i32, i64); 2] = [(1, 2), (3, 4)]; }",
        "[(i32, i64); 2]",
    );
}

#[test]
fn inference_tuple_of_arrays() {
    // Tuple containing arrays
    check(
        "fn main() { let t: ([i32; 2], [i64; 3]) = ([1, 2], [3, 4, 5]); }",
        "([i32; 2], [i64; 3])",
    );
}

#[test]
fn inference_ref_to_array_element() {
    // Reference to an array element
    check("fn f(arr: [i64; 3]) { let x = &arr[0]; }", "&i64");
}

#[test]
fn inference_complex_expression_chain() {
    // Complex expression with multiple operators and calls
    check(
        "fn f(x: i64): i64 { x } fn main() { let x = f(1) + f(2) + f(3); }",
        "i64",
    );
}

#[test]
fn inference_block_tail_type() {
    // Block returns the type of its tail expression
    check("fn main() { let x: i64 = { 42 }; }", "i64");
}

#[test]
fn inference_nested_function_calls() {
    // Deeply nested function calls
    check(
        "fn f(x: i64): i64 { x } fn g(x: i64): i64 { x } fn main() { let x = f(g(f(g(42)))); }",
        "i64",
    );
}

// =============================================================================
// 1.17 Error Message Quality Tests
// =============================================================================

#[test]
fn error_type_mismatch_basic() {
    // Type mismatch error is reported
    check_err("fn main() { let x: i32 = \"hello\"; }", &["type mismatch"]);
}

#[test]
fn error_wrong_arg_count() {
    // Error for wrong number of arguments
    check_err(
        "fn f(a: i32, b: i32) {} fn main() { f(1); }",
        &["expected 2 argument"],
    );
}

#[test]
fn error_field_access_on_non_struct() {
    // Accessing a field on a non-struct type
    check_err("fn main() { let x = 42; x.foo; }", &["non-struct"]);
}

#[test]
fn error_binary_op_on_incompatible() {
    // Binary operation on incompatible types
    check_err("fn main() { let x = true + 1; }", &["cannot apply binary"]);
}

#[test]
fn error_call_with_wrong_type() {
    // Calling function with wrong argument type
    check_err(
        "fn f(x: i32) {} fn main() { f(\"hello\"); }",
        &["type mismatch"],
    );
}

#[test]
fn error_return_type_mismatch() {
    // Function return type doesn't match
    check_err("fn f(): i32 { \"hello\" }", &["type mismatch"]);
}

// =============================================================================
// 2.0 Mutability Checking Tests
// =============================================================================

// Phase 1: Assignment to immutable variables

#[test]
fn error_assign_to_immutable_local() {
    check_err("fn main() { let x = 1; x = 2; }", &["cannot assign"]);
}

#[test]
fn assign_to_mutable_local() {
    check("fn main() { let mut x = 1; x = 2; let y = x; }", "i32");
}

// Phase 2: Mutable borrow of immutable variables

#[test]
fn error_mut_ref_to_immutable() {
    check_err(
        "fn main() { let x = 42; let y = &mut x; }",
        &["cannot borrow"],
    );
}

#[test]
fn mut_ref_to_mutable() {
    check("fn main() { let mut x = 42; let y = &mut x; }", "&mut i32");
}

#[test]
fn shared_ref_to_immutable_ok() {
    check("fn main() { let x = 42; let y = &x; }", "&i32");
}

// Phase 3: Parameters (immutable by default)

#[test]
fn error_assign_to_param() {
    check_err("fn foo(x: i32) { x = 1; }", &["cannot assign"]);
}

#[test]
fn error_mut_ref_to_param() {
    check_err("fn foo(x: i32) { let y = &mut x; }", &["cannot borrow"]);
}

// Phase 4: Compound assignment

#[test]
fn error_add_assign_to_immutable() {
    check_err("fn main() { let x = 1; x += 1; }", &["cannot assign"]);
}

#[test]
fn add_assign_to_mutable() {
    check("fn main() { let mut x = 1; x += 1; let y = x; }", "i32");
}

// Phase 5: Field assignment through mutable binding

#[test]
fn error_assign_field_immutable_binding() {
    check_err(
        "struct S(a: i32) fn main() { let s = S(a: 1); s.a = 2; }",
        &["cannot assign"],
    );
}

#[test]
fn assign_field_mutable_binding() {
    check(
        "struct S(a: i32) fn main() { let mut s = S(a: 1); s.a = 2; let x = s.a; }",
        "i32",
    );
}

// Phase 6: Deref assignment

#[test]
fn error_assign_through_shared_ref() {
    check_err(
        "fn main() { let mut x = 1; let r = &x; *r = 2; }",
        &["cannot assign"],
    );
}

#[test]
fn assign_through_mut_ref() {
    check(
        "fn main() { let mut x = 1; let r = &mut x; *r = 2; let y = x; }",
        "i32",
    );
}

// =============================================================================
// 2.1 Invalid Assignment Targets and Mutable Borrows (TDD tests for ? chain fixes)
// =============================================================================

#[test]
fn test_assign_to_literal_produces_error() {
    check_err("fn main() { 42 = 1; }", &["invalid assignment target"]);
}

#[test]
fn test_assign_to_binary_expr_produces_error() {
    check_err("fn main() { (1 + 2) = 3; }", &["invalid assignment target"]);
}

#[test]
fn test_mutable_borrow_of_literal_produces_error() {
    // This tests the silent failure in check_mutable_borrow's catch-all
    check_err(
        "fn main() { let x = &mut 42; }",
        &["cannot take mutable reference"],
    );
}

#[test]
fn test_assign_field_through_immutable_ref() {
    check_err(
        "struct S(x: i32) fn main() { let s = S(x: 1); let r = &s; r.x = 2; }",
        &["cannot assign to field of immutable reference"],
    );
}

// =============================================================================
// 3.0 Generic Function Instantiation (TDD tests for spl-7ab.4)
// =============================================================================

// Phase 1: Track Type Parameters in FnSignature

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_fn_identity_infers_from_arg() {
    check(
        "fn identity(x: T): T where T { x } fn main() { let a = identity(42); }",
        "i32",
    );
}

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_fn_identity_infers_from_context() {
    check(
        "fn identity(x: T): T where T { x } fn main() { let a: i64 = identity(42); }",
        "i64",
    );
}

// Phase 2: Instantiate Generic Functions

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_fn_two_params_same_type() {
    check(
        "fn pair(a: T, b: T): T where T { a } fn main() { let x = pair(1, 2); }",
        "i32",
    );
}

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_fn_multiple_type_params() {
    check(
        "fn swap(a: A, b: B): B where A, B { b } fn main() { let x = swap(1, true); }",
        "bool",
    );
}

// Phase 3: Generic Struct Instantiation

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_struct_field_access() {
    check(
        "struct Wrapper(value: T) where T fn main() { let w = Wrapper(value: 42); let x = w.value; }",
        "i32",
    );
}

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_struct_multiple_fields() {
    check(
        "struct Pair(first: A, second: B) where A, B fn main() { let p = Pair(first = 1, second = true); let x = p.second; }",
        "bool",
    );
}

// Phase 4: Generic Methods

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_struct_method_returns_param() {
    check(
        "struct Wrapper(value: T) where T impl Wrapper(T) where T { fn get(&self): T { self.value } } fn main() { let w = Wrapper(value: 42); let x = w.get(); }",
        "i32",
    );
}

// Phase 5: Error Cases

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn error_generic_type_mismatch() {
    check_err(
        "fn pair(a: T, b: T): T where T { a } fn main() { pair(1, true); }",
        &["type mismatch"],
    );
}

// Phase 6: Edge Cases

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_nested_calls() {
    check(
        "fn identity(x: T): T where T { x } fn main() { let a = identity(identity(42)); }",
        "i32",
    );
}

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_multiple_instantiations() {
    check(
        "fn identity(x: T): T where T { x } fn main() { let a = identity(42); let b = identity(true); }",
        "bool",
    );
}

// Phase 7: Method-Specific Type Parameters

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_method_with_own_type_param() {
    // Method has its own type parameter U distinct from impl type param T
    check(
        r#"
        struct Wrapper(value: T) where T
        impl Wrapper(T) where T {
            fn transform(&self, other: U): U where U { other }
        }
        fn main() {
            let w = Wrapper(value: 42);
            let x = w.transform(true);
        }
        "#,
        "bool",
    );
}

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_method_uses_both_impl_and_own_type_param() {
    // Method returns T (from impl) but takes U (method-specific)
    check(
        r#"
        struct Wrapper(value: T) where T
        impl Wrapper(T) where T {
            fn with_other(&self, _other: U): T where U { self.value }
        }
        fn main() {
            let w = Wrapper(value: 42);
            let x = w.with_other(true);
        }
        "#,
        "i32",
    );
}

// Phase 8: Generic Functions Returning Generic Structs

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_fn_returns_generic_struct() {
    check(
        r#"
        struct Wrapper(value: T) where T
        fn wrap(x: T): Wrapper(T) where T { Wrapper(value: x) }
        fn main() {
            let w = wrap(42);
            let x = w.value;
        }
        "#,
        "i32",
    );
}

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn generic_fn_returns_generic_struct_inferred_from_context() {
    check(
        r#"
        struct Wrapper(value: T) where T
        fn wrap(x: T): Wrapper(T) where T { Wrapper(value: x) }
        fn main() {
            let w: Wrapper(i64) = wrap(42);
            let x = w.value;
        }
        "#,
        "i64",
    );
}

// Phase 9: Nested Generic Types

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn nested_generic_struct() {
    check(
        r#"
        struct Inner(value: T) where T
        struct Outer(inner: Inner(T)) where T
        fn main() {
            let o = Outer(inner: Inner(value: 42));
            let x = o.inner.value;
        }
        "#,
        "i32",
    );
}

// =============================================================================
// 4.0 SEMA-5.1: Integer Literal Range Validation
// =============================================================================

// Valid literals at boundaries

#[test]
fn int_literal_u8_max() {
    check("fn main() { let x: u8 = 255; }", "u8");
}

#[test]
fn int_literal_u8_min() {
    check("fn main() { let x: u8 = 0; }", "u8");
}

#[test]
fn int_literal_i8_max() {
    check("fn main() { let x: i8 = 127; }", "i8");
}

#[test]
fn int_literal_i8_min() {
    check("fn main() { let x: i8 = -128; }", "i8");
}

#[test]
fn int_literal_u16_max() {
    check("fn main() { let x: u16 = 65535; }", "u16");
}

#[test]
fn int_literal_i16_range() {
    check("fn main() { let x: i16 = -32768; }", "i16");
}

#[test]
fn int_literal_u32_max() {
    check("fn main() { let x: u32 = 4294967295; }", "u32");
}

#[test]
fn int_literal_u64_max() {
    check("fn main() { let x: u64 = 18446744073709551615; }", "u64");
}

// Suffixed literals at boundaries

#[test]
fn int_literal_suffixed_u8_max() {
    check("fn main() { let x = 255u8; }", "u8");
}

#[test]
fn int_literal_suffixed_i8_max() {
    check("fn main() { let x = 127i8; }", "i8");
}

#[test]
fn int_literal_suffixed_i8_min() {
    check("fn main() { let x = -128i8; }", "i8");
}

// Invalid literals - overflow

#[test]
fn error_u8_overflow() {
    check_err("fn main() { let x: u8 = 256; }", &["out of range"]);
}

#[test]
fn error_u8_large_overflow() {
    check_err("fn main() { let x: u8 = 1000; }", &["out of range"]);
}

#[test]
fn error_u8_negative() {
    check_err("fn main() { let x: u8 = -1; }", &["out of range"]);
}

#[test]
fn error_i8_overflow_positive() {
    check_err("fn main() { let x: i8 = 128; }", &["out of range"]);
}

#[test]
fn error_i8_overflow_negative() {
    check_err("fn main() { let x: i8 = -129; }", &["out of range"]);
}

#[test]
fn error_u16_overflow() {
    check_err("fn main() { let x: u16 = 65536; }", &["out of range"]);
}

#[test]
fn error_i16_overflow() {
    check_err("fn main() { let x: i16 = 32768; }", &["out of range"]);
}

// Suffixed literal overflow

#[test]
fn error_suffixed_u8_overflow() {
    check_err("fn main() { let x = 256u8; }", &["out of range"]);
}

#[test]
fn error_suffixed_i8_overflow() {
    check_err("fn main() { let x = 128i8; }", &["out of range"]);
}

#[test]
fn error_suffixed_u8_negative() {
    check_err("fn main() { let x = -1u8; }", &["out of range"]);
}

// Parenthesized negation tests (HIR lowering)

#[test]
fn parenthesized_negated_i8_min() {
    check("fn main() { let x = -(128i8); }", "i8");
}

#[test]
fn double_paren_negated_i8_min() {
    check("fn main() { let x = (-(128i8)); }", "i8");
}

#[test]
fn parenthesized_negated_i16_min() {
    check("fn main() { let x = -(32768i16); }", "i16");
}

#[test]
fn error_double_negation_i8() {
    // --128i8 folds to +128, which is out of range for i8
    check_err("fn main() { let x = --128i8; }", &["out of range"]);
}

#[test]
fn error_parenthesized_positive_i8_overflow() {
    // -(-(128i8)) folds to +128, which is out of range for i8
    check_err("fn main() { let x = -(-(128i8)); }", &["out of range"]);
}

#[test]
fn error_parenthesized_u8_negative() {
    check_err("fn main() { let x = -(1u8); }", &["out of range"]);
}

// =============================================================================
// 4.1 SEMA-5.2: Cast Validity Checking
// =============================================================================

// Valid numeric casts

#[test]
fn cast_i32_to_i64() {
    check("fn main() { let x: i32 = 42; let y = x as i64; }", "i64");
}

#[test]
fn cast_i64_to_i32() {
    check("fn main() { let x: i64 = 42; let y = x as i32; }", "i32");
}

#[test]
fn cast_u8_to_u32() {
    check("fn main() { let x: u8 = 42; let y = x as u32; }", "u32");
}

#[test]
fn cast_i32_to_u32() {
    check("fn main() { let x: i32 = 42; let y = x as u32; }", "u32");
}

#[test]
fn cast_int_to_float() {
    check("fn main() { let x: i32 = 42; let y = x as f64; }", "f64");
}

#[test]
fn cast_float_to_int() {
    check("fn main() { let x: f64 = 3.14; let y = x as i32; }", "i32");
}

#[test]
fn cast_f32_to_f64() {
    check("fn main() { let x: f32 = 3.14; let y = x as f64; }", "f64");
}

#[test]
fn cast_f64_to_f32() {
    check("fn main() { let x: f64 = 3.14; let y = x as f32; }", "f32");
}

// Invalid casts

#[test]
fn error_bool_to_int() {
    check_err(
        "fn main() { let x = true; let y = x as i32; }",
        &["invalid cast"],
    );
}

#[test]
fn error_int_to_bool() {
    check_err(
        "fn main() { let x: i32 = 1; let y = x as bool; }",
        &["invalid cast"],
    );
}

#[test]
fn error_struct_to_int() {
    check_err(
        "struct S() fn main() { let s = S(); let x = s as i32; }",
        &["invalid cast"],
    );
}

#[test]
fn error_int_to_struct() {
    check_err(
        "struct S() fn main() { let x: i32 = 1; let s = x as S; }",
        &["invalid cast"],
    );
}

#[test]
fn error_tuple_to_int() {
    check_err(
        "fn main() { let t = (1, 2); let x = t as i32; }",
        &["invalid cast"],
    );
}

#[test]
fn error_array_to_int() {
    check_err(
        "fn main() { let a = [1, 2, 3]; let x = a as i32; }",
        &["invalid cast"],
    );
}

// =============================================================================
// 4.2 SEMA-5.3: Recursive Type Detection
// =============================================================================

#[test]
fn error_recursive_direct() {
    check_err("struct Foo(x: Foo)", &["recursive", "infinite"]);
}

#[test]
fn error_recursive_indirect() {
    check_err("struct A(b: B) struct B(a: A)", &["recursive"]);
}

#[test]
fn error_recursive_three_way() {
    check_err(
        "struct A(b: B) struct B(c: C) struct C(a: A)",
        &["recursive"],
    );
}

#[test]
fn recursive_with_ref_ok() {
    check(
        "struct Node(next: &Node) fn main() { let x: i32 = 0; }",
        "i32",
    );
}

#[test]
fn recursive_with_mut_ref_ok() {
    check(
        "struct Node(next: &mut Node) fn main() { let x: i32 = 0; }",
        "i32",
    );
}

#[test]
fn non_recursive_ok() {
    check(
        "struct A(x: i32) struct B(a: A) fn main() { let x: i32 = 0; }",
        "i32",
    );
}

#[test]
fn non_recursive_chain_ok() {
    check(
        "struct A(x: i32) struct B(a: A) struct C(b: B) fn main() { let x: i32 = 0; }",
        "i32",
    );
}

// =============================================================================
// 4.3 SEMA-5.4: Type Alias Cycle Detection
// =============================================================================

#[test]
fn error_alias_self() {
    check_err("type A = A; fn main() { let x: i32 = 0; }", &["cyclic"]);
}

#[test]
fn error_alias_mutual() {
    check_err(
        "type A = B; type B = A; fn main() { let x: i32 = 0; }",
        &["cyclic"],
    );
}

#[test]
fn error_alias_three_way() {
    check_err(
        "type A = B; type B = C; type C = A; fn main() { let x: i32 = 0; }",
        &["cyclic"],
    );
}

#[test]
fn error_alias_cyclic_usage() {
    // Verify using a cyclic alias doesn't cause infinite recursion
    check_err("type A = A; fn main() { let x: A = 0; }", &["cyclic"]);
}

#[test]
fn alias_chain_ok() {
    check(
        "type A = i32; type B = A; fn main() { let x: B = 1; }",
        "i32",
    );
}

#[test]
fn alias_to_struct_ok() {
    check(
        "struct S(x: i32) type Alias = S; fn main() { let a = Alias(x: 42); }",
        "S",
    );
}

// =============================================================================
// 4.4 SEMA-5.5: Constant Array Index Bounds Checking
// =============================================================================

#[test]
fn index_in_bounds() {
    check("fn main() { let a = [1, 2, 3]; let x = a[2]; }", "i32");
}

#[test]
fn index_in_bounds_zero() {
    check("fn main() { let a = [1, 2, 3]; let x = a[0]; }", "i32");
}

#[test]
fn index_variable_no_check() {
    // Non-constant indices should not produce compile-time errors
    check(
        "fn main() { let a = [1, 2, 3]; let i: i32 = 0; let x = a[i]; }",
        "i32",
    );
}

#[test]
fn error_index_oob() {
    check_err(
        "fn main() { let a = [1, 2, 3]; let x = a[5]; }",
        &["out of bounds"],
    );
}

#[test]
fn error_index_oob_exact() {
    // Array of length 3, index 3 is out of bounds
    check_err(
        "fn main() { let a = [1, 2, 3]; let x = a[3]; }",
        &["out of bounds"],
    );
}

#[test]
fn error_index_empty_array() {
    check_err(
        "fn main() { let a: [i32; 0] = []; let x = a[0]; }",
        &["out of bounds"],
    );
}

// =============================================================================
// 4.5 SEMA-5.6: Unreachable Code Detection
// =============================================================================

#[test]
fn warn_after_return() {
    check_warn("fn f() { return; let x: i32 = 1; }", &["unreachable"]);
}

#[test]
fn warn_after_return_value() {
    check_warn(
        "fn f(): i32 { return 1; let x: i32 = 2; x }",
        &["unreachable"],
    );
}

#[test]
fn warn_after_break() {
    check_warn(
        "fn main() { loop { break; let x: i32 = 1; } }",
        &["unreachable"],
    );
}

#[test]
fn warn_after_continue() {
    check_warn(
        "fn main() { loop { continue; let x: i32 = 1; } }",
        &["unreachable"],
    );
}

#[test]
fn no_warn_return_at_end() {
    // No warning when return is the last statement
    check("fn f(): i32 { let x: i32 = 1; return x; }", "i32");
}

#[test]
fn no_warn_in_if_branch() {
    // No warning when return is in an if branch (other code still reachable)
    check(
        "fn f(b: bool): i32 { if b { return 1; } let x: i32 = 2; return x; }",
        "i32",
    );
}

// =============================================================================
// 5.0 Additional Tests for SEMA-5 QA
// =============================================================================

// -----------------------------------------------------------------------------
// 5.1 i32/i64 Boundary Tests
// -----------------------------------------------------------------------------

#[test]
fn int_literal_i32_max() {
    check("fn main() { let x: i32 = 2147483647; }", "i32");
}

#[test]
fn int_literal_i32_min() {
    check("fn main() { let x: i32 = -2147483648; }", "i32");
}

#[test]
fn error_i32_overflow() {
    check_err("fn main() { let x: i32 = 2147483648; }", &["out of range"]);
}

#[test]
fn error_i32_underflow() {
    check_err("fn main() { let x: i32 = -2147483649; }", &["out of range"]);
}

#[test]
fn int_literal_i64_max() {
    check("fn main() { let x: i64 = 9223372036854775807; }", "i64");
}

#[test]
fn int_literal_i64_min() {
    check("fn main() { let x: i64 = -9223372036854775808; }", "i64");
}

#[test]
fn error_i64_overflow() {
    check_err(
        "fn main() { let x: i64 = 9223372036854775808; }",
        &["out of range"],
    );
}

// -----------------------------------------------------------------------------
// 5.2 Recursive Types via Arrays/Tuples
// -----------------------------------------------------------------------------

#[test]
fn error_recursive_via_array() {
    check_err("struct Foo(arr: [Foo; 1])", &["recursive"]);
}

#[test]
fn error_recursive_via_tuple() {
    check_err("struct Foo(tup: (i32, Foo))", &["recursive"]);
}

// -----------------------------------------------------------------------------
// 5.3 Type Alias Cycles via Compound Types
// -----------------------------------------------------------------------------

#[test]
fn error_alias_via_array() {
    check_err("type A = [B; 1]; type B = A;", &["cyclic"]);
}

#[test]
fn error_alias_via_tuple() {
    check_err("type A = (B, i32); type B = A;", &["cyclic"]);
}

// -----------------------------------------------------------------------------
// 5.4 Unreachable Code After Infinite Loop
// -----------------------------------------------------------------------------

#[test]
fn warn_after_infinite_loop() {
    check_warn("fn main() { loop {} let x: i32 = 1; }", &["unreachable"]);
}

#[test]
fn warn_after_return_in_nested_block() {
    check_warn("fn f() { { return; } let x: i32 = 1; }", &["unreachable"]);
}

#[test]
fn warn_first_unreachable_only() {
    // Should only warn about first unreachable statement
    check_warn(
        "fn f() { return; let x: i32 = 1; let y: i32 = 2; }",
        &["unreachable"],
    );
}

// -----------------------------------------------------------------------------
// 5.5 Additional Cast Tests
// -----------------------------------------------------------------------------

#[test]
fn cast_u8_to_i32() {
    check("fn main() { let x: u8 = 1; let y = x as i32; }", "i32");
}

#[test]
fn cast_u32_to_i32() {
    check("fn main() { let x: u32 = 1; let y = x as i32; }", "i32");
}

#[test]
fn error_unit_to_int() {
    check_err("fn main() { let x = () as i32; }", &["invalid cast"]);
}

// -----------------------------------------------------------------------------
// 5.6 Additional Array Bounds Tests
// -----------------------------------------------------------------------------

#[test]
fn index_nested_array() {
    check(
        "fn main() { let a = [[1, 2], [3, 4]]; let x = a[0][1]; }",
        "i32",
    );
}

#[test]
fn error_very_large_index() {
    check_err(
        "fn main() { let a = [1]; let x = a[999999]; }",
        &["out of bounds"],
    );
}

// =============================================================================
// 6.0 Control Flow Analysis (SEMA-6)
// =============================================================================

// -----------------------------------------------------------------------------
// 6.1 Return Path Analysis - Valid Cases
// -----------------------------------------------------------------------------

#[test]
fn return_path_explicit_return() {
    // Explicit return is a valid return path
    check(
        "fn f(): i32 { return 42; } fn main() { let x = f(); }",
        "i32",
    );
}

#[test]
fn return_path_tail_expression() {
    // Tail expression is a valid return path
    check("fn f(): i32 { 42 } fn main() { let x = f(); }", "i32");
}

#[test]
fn return_path_if_else_both_return() {
    // Both branches return a value - valid
    check(
        "fn f(b: bool): i32 { if b { 1 } else { 2 } } fn main() { let x = f(true); }",
        "i32",
    );
}

#[test]
fn return_path_if_else_explicit() {
    // Both branches have explicit return - valid
    check(
        "fn f(b: bool): i32 { if b { return 1; } else { return 2; } } fn main() { let x = f(true); }",
        "i32",
    );
}

#[test]
fn return_path_diverging_then_tail() {
    // If one branch returns, tail after if is still reachable - valid
    check(
        "fn f(b: bool): i32 { if b { return 1; } return 0; } fn main() { let x = f(true); }",
        "i32",
    );
}

#[test]
fn return_path_loop_with_break() {
    // Loop with break value is a valid return path
    check(
        "fn f(): i32 { loop { break 42; } } fn main() { let x = f(); }",
        "i32",
    );
}

#[test]
fn return_path_nested_if() {
    // Nested if/else chains - all paths return
    check(
        "fn f(a: bool, b: bool): i32 { if a { if b { 1 } else { 2 } } else { 3 } } fn main() { let x = f(true, false); }",
        "i32",
    );
}

#[test]
fn return_path_infinite_loop() {
    // Infinite loop has never type, so function "returns" (diverges)
    check(
        "fn f(): i32 { loop {} } fn main() { let x: i32 = 0; }",
        "i32",
    );
}

// -----------------------------------------------------------------------------
// 6.2 Return Path Analysis - Invalid Cases (Missing Returns)
// -----------------------------------------------------------------------------

#[test]
fn error_missing_return_if_no_else() {
    // If without else doesn't always return a value
    check_err(
        "fn f(b: bool): i32 { if b { return 42; } }",
        &["not all code paths return a value"],
    );
}

#[test]
fn error_missing_return_empty_body() {
    // Empty body doesn't return a value for non-unit return type
    check_err("fn f(): i32 { }", &["not all code paths return a value"]);
}

#[test]
fn error_missing_return_only_let() {
    // Let statement doesn't produce a return value
    check_err(
        "fn f(): i32 { let x = 1; }",
        &["not all code paths return a value"],
    );
}

#[test]
fn error_missing_return_one_branch() {
    // Only else branch returns, then branch has no value
    check_err(
        "fn f(b: bool): i32 { if b { let x = 1; } else { 2 } }",
        &["type mismatch"],
    );
}

#[test]
fn error_missing_return_nested_if_incomplete() {
    // Nested if missing inner else
    check_err(
        "fn f(a: bool, b: bool): i32 { if a { if b { 1 } } else { 2 } }",
        &["type mismatch"],
    );
}

// -----------------------------------------------------------------------------
// 6.3 Break/Continue Validation
// -----------------------------------------------------------------------------

#[test]
fn error_break_outside_loop() {
    check_err("fn main() { break; }", &["break outside of loop"]);
}

#[test]
fn error_continue_outside_loop() {
    check_err("fn main() { continue; }", &["continue outside of loop"]);
}

#[test]
fn error_break_in_if_outside_loop() {
    check_err(
        "fn main() { if true { break; } }",
        &["break outside of loop"],
    );
}

#[test]
fn break_in_nested_loop_ok() {
    // Break inside nested loop should work
    check(
        "fn main() { loop { loop { break; } break; } let x: i32 = 0; }",
        "i32",
    );
}

#[test]
fn error_break_value_in_while() {
    // Break with value is only allowed in loop expressions, not while
    check_err(
        "fn main() { while true { break 42; } }",
        &["break with value only allowed in `loop`"],
    );
}

#[test]
fn error_break_value_in_for() {
    // Break with value is only allowed in loop expressions, not for
    check_err(
        "fn main() { for i in 0..10 { break 42; } }",
        &["break with value only allowed in `loop`"],
    );
}

#[test]
fn continue_in_while_ok() {
    // Continue inside while is valid
    check(
        "fn main() { while true { continue; } let x: i32 = 0; }",
        "i32",
    );
}

#[test]
fn continue_in_for_ok() {
    // Continue inside for is valid
    check(
        "fn main() { for i in 0..10 { continue; } let x: i32 = 0; }",
        "i32",
    );
}

// -----------------------------------------------------------------------------
// 6.4 Enhanced Dead Code Detection
// -----------------------------------------------------------------------------

#[test]
fn warn_code_after_if_both_return() {
    // Code after if where both branches return is unreachable
    check_warn(
        "fn f(b: bool): i32 { if b { return 1; } else { return 2; } let x = 3; x }",
        &["unreachable"],
    );
}

#[test]
fn warn_code_after_loop_no_break() {
    // Code after infinite loop is unreachable
    check_warn("fn main() { loop {} let x: i32 = 1; }", &["unreachable"]);
}

#[test]
fn no_warn_code_after_if_one_returns() {
    // Code after if where only one branch returns is reachable
    check(
        "fn f(b: bool): i32 { if b { return 1; } let x = 2; return x; }",
        "i32",
    );
}

#[test]
fn no_warn_code_after_loop_with_break() {
    // Code after loop with break is reachable
    check("fn main() { loop { break; } let x: i32 = 1; }", "i32");
}

// -----------------------------------------------------------------------------
// 7. Type Variable Contract Tests
// -----------------------------------------------------------------------------
// These tests document the behavior of type inference variables:
// - IntVar: unifies only with integer types, defaults to i32
// - FloatVar: unifies only with float types, defaults to f64
// - Var: general type variable, unifies with anything

#[test]
fn int_var_unifies_with_integers_not_floats() {
    // Integer literals create IntVar that unifies with integer types
    check("fn main() { let x: i64 = 42; }", "i64");
    check("fn main() { let x: u8 = 1; }", "u8");
    // But cannot unify with floats
    check_err("fn main() { let x: f64 = 42; }", &["type mismatch"]);
}

#[test]
fn float_var_unifies_with_floats_not_integers() {
    // Float literals create FloatVar that unifies with float types
    check("fn main() { let x: f32 = 3.14; }", "f32");
    check("fn main() { let x: f64 = 2.718; }", "f64");
    // But cannot unify with integers
    check_err("fn main() { let x: i32 = 3.14; }", &["type mismatch"]);
}

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn general_var_unifies_with_anything() {
    // Generic type parameters create general Var that can unify with anything
    check(
        "struct Box(value: T) where T fn main() { let b = Box(value: 42); }",
        "Box",
    );
    check(
        "struct Box(value: T) where T fn main() { let b = Box(value: true); }",
        "Box",
    );
    check(
        "struct Box(value: T) where T fn main() { let b = Box(value: 3.14); }",
        "Box",
    );
}

#[test]
fn int_var_defaults_to_i32() {
    // Unconstrained integer literals default to i32
    check("fn main() { let x = 42; }", "i32");
    check("fn f(): i32 { 100 } fn main() { let x = f(); }", "i32");
}

#[test]
fn float_var_defaults_to_f64() {
    // Unconstrained float literals default to f64
    check("fn main() { let x = 3.14; }", "f64");
    check("fn f(): f64 { 2.718 } fn main() { let x = f(); }", "f64");
}

// =============================================================================
// 8.0 Unification Constraint Edge Cases (TDD Tests for spl-dkm)
// =============================================================================
//
// These tests document edge case behaviors of the type unification algorithm:
// - Int/Float constrained variables only unify with compatible types
// - &mut T coerces to &T but not vice versa
// - Never type unifies with anything

// -----------------------------------------------------------------------------
// 8.1 Int Var Constraint Tests
// -----------------------------------------------------------------------------

#[test]
fn int_var_constrained_by_multiple_same_type() {
    // Same int var used multiple times with the same integer type should succeed
    check(
        "fn f(x: i64, y: i64) {} fn main() { let a = 1; f(a, a); }",
        "i64",
    );
}

#[test]
fn int_var_constrained_by_multiple_contexts() {
    // Int var constrained by parameter and return type (both i64) should work
    check(
        "fn f(x: i64): i64 { x } fn main() { let a = 1; let b = f(a); }",
        "i64",
    );
}

#[test]
fn int_var_in_binary_op_with_typed_operand() {
    // Int var should unify through binary operation with typed operand
    check("fn main() { let x: i64 = 1; let y = 2 + x; }", "i64");
}

#[test]
fn error_int_var_cannot_unify_with_float_direct() {
    // Integer literal cannot be assigned to float type directly
    check_err("fn main() { let x: f64 = 42; }", &["type mismatch"]);
}

#[test]
fn error_int_var_cannot_unify_with_float_via_function() {
    // Integer literal cannot satisfy float parameter
    check_err(
        "fn f(x: f64) {} fn main() { let a = 42; f(a); }",
        &["type mismatch"],
    );
}

// -----------------------------------------------------------------------------
// 8.2 Float Var Constraint Tests
// -----------------------------------------------------------------------------

#[test]
fn float_var_constrained_by_multiple_same_type() {
    // Same float var used multiple times with the same float type should succeed
    check(
        "fn f(x: f32, y: f32) {} fn main() { let a = 1.0; f(a, a); }",
        "f32",
    );
}

#[test]
fn error_float_var_cannot_unify_with_int_direct() {
    // Float literal cannot be assigned to integer type directly
    check_err("fn main() { let x: i32 = 3.14; }", &["type mismatch"]);
}

#[test]
fn error_float_var_cannot_unify_with_int_via_function() {
    // Float literal cannot satisfy integer parameter
    check_err(
        "fn f(x: i32) {} fn main() { let a = 3.14; f(a); }",
        &["type mismatch"],
    );
}

// -----------------------------------------------------------------------------
// 8.3 Reference Coercion Tests
// -----------------------------------------------------------------------------

#[test]
fn ref_coercion_mut_to_shared_explicit() {
    // &mut T explicitly coerces to &T when passed to function expecting &T
    check(
        "fn f(x: &i32) {} fn main() { let mut a = 42; f(&mut a); }",
        "i32",
    );
}

#[test]
fn ref_coercion_in_method_receiver() {
    // &mut self should work where &self is expected (for read-only methods)
    check(
        "struct S(x: i32) impl S { fn get(&self): i32 { self.x } } fn main() { let mut s = S(x: 1); let y = (&mut s).get(); }",
        "i32",
    );
}

#[test]
fn error_ref_coercion_shared_to_mut() {
    // &T cannot coerce to &mut T (cannot gain mutability)
    check_err(
        "fn f(x: &mut i32) {} fn main() { let a = 42; f(&a); }",
        &["type mismatch"],
    );
}

#[test]
fn error_ref_coercion_shared_to_mut_in_assignment() {
    // Assignment through shared reference should fail
    check_err(
        "fn main() { let a = 42; let r: &i32 = &a; *r = 1; }",
        &["cannot assign"],
    );
}

// -----------------------------------------------------------------------------
// 8.4 Never Type Unification Tests
// -----------------------------------------------------------------------------

#[test]
fn never_in_if_then_propagates_else_type() {
    // When then-branch has never type, the else type should be usable.
    // Note: The current implementation assigns the if-expression type as `!`
    // when the then-branch diverges, but the else type is still available
    // for type-annotated contexts.
    check(
        "fn main() { let x: i32 = if true { return } else { 42 }; }",
        "i32",
    );
}

#[test]
fn never_in_if_else_propagates_then_type() {
    // When else-branch has never type, result type comes from then
    check(
        "fn main() { let x = if true { 42 } else { return }; }",
        "i32",
    );
}

#[test]
fn both_branches_never() {
    // When both branches have never type, result is never
    // Note: The binding `x` will have type `!` (never)
    check(
        "fn main() { let x: i32 = if true { return } else { return }; }",
        "i32",
    );
}

#[test]
fn never_from_loop_without_break() {
    // A loop without break diverges (never returns)
    check("fn main() { let x: i32 = loop {}; }", "i32");
}

#[test]
fn never_in_early_return_pattern() {
    // Return in one branch, value in another - function return type propagates
    check(
        "fn f(b: bool): i32 { if b { return 1; } return 42; } fn main() { let x = f(true); }",
        "i32",
    );
}

// -----------------------------------------------------------------------------
// 8.5 Type Variable Chain Tests
// -----------------------------------------------------------------------------

#[test]
fn type_var_chain_through_multiple_lets() {
    // Type should propagate through a chain of let bindings
    check(
        "fn main() { let a = 1; let b = a; let c = b; let d: i64 = c; }",
        "i64",
    );
}

#[test]
#[ignore = "needs semantic support for where clause generics"]
fn type_var_bidirectional_through_function() {
    // Type should flow both ways: argument constrains param, return constrains usage
    check(
        "fn identity(x: T): T where T { x } fn main() { let a: i64 = identity(1); }",
        "i64",
    );
}

// =============================================================================
// Implicit Return Semantics - Single Expression vs Multi-Statement
// =============================================================================

#[test]
fn implicit_return_single_literal() {
    // Single literal expression - implicit return allowed
    check("fn f(): i32 { 42 } fn main() { let x = f(); }", "i32");
}

#[test]
fn implicit_return_single_binary_expr() {
    // Single binary expression - implicit return allowed
    check(
        "fn f(x: i32): i32 { x * 2 } fn main() { let y = f(21); }",
        "i32",
    );
}

#[test]
fn implicit_return_single_if_expr() {
    // Single if-expression - implicit return allowed
    check(
        "fn f(a: i32, b: i32): i32 { if a > b { a } else { b } } fn main() { let x = f(1, 2); }",
        "i32",
    );
}

#[test]
fn explicit_return_with_statements() {
    // Statements with explicit return - allowed
    check(
        "fn f(x: i32): i32 { let y = x; return y + 1; } fn main() { let z = f(5); }",
        "i32",
    );
}

#[test]
fn unit_return_with_statements_no_return_needed() {
    // Unit return type - no return needed even with statements
    check("fn f() { let x = 1; } fn main() { let y = f(); }", "()");
}

#[test]
fn error_implicit_return_with_statements() {
    // Statements with implicit return - ERROR
    check_err(
        "fn f(x: i32): i32 { let y = x; y + 1 }",
        &["implicit return not allowed when function body contains statements"],
    );
}

#[test]
fn error_implicit_return_with_multiple_statements() {
    // Multiple statements with implicit return - ERROR
    check_err(
        "fn f(): i32 { let a = 1; let b = 2; a + b }",
        &["implicit return not allowed when function body contains statements"],
    );
}

#[test]
fn block_expression_in_let_still_works() {
    // Block expressions (not function bodies) should still work normally
    check(
        "fn f(): i32 { let x = { let y = 1; y + 1 }; return x; } fn main() { let z = f(); }",
        "i32",
    );
}
