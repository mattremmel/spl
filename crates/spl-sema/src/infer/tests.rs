//! Tests for bidirectional type inference.
//!
//! These tests are written first following TDD methodology.
//! The implementation should make all these tests pass.

use crate::infer::infer;
use crate::resolver::resolve;
use rowan::ast::AstNode;
use spl_ast::SourceFile;
use spl_parser::parse;

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

    let infer_result = infer(&source_file, &resolve_result);
    assert!(
        infer_result.diagnostics.is_empty(),
        "expected no inference errors, got: {:?}",
        infer_result
            .diagnostics
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );

    let actual = infer_result.display_first_binding(&resolve_result.ctx);
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
    let infer_result = infer(&source_file, &resolve_result);

    // Combine diagnostics from both resolution and inference phases
    let all_diagnostics: Vec<_> = resolve_result
        .diagnostics
        .iter()
        .chain(infer_result.diagnostics.iter())
        .collect();

    assert!(
        !all_diagnostics.is_empty(),
        "expected errors containing {expected:?}, got none"
    );

    for pattern in expected {
        let found = all_diagnostics.iter().any(|d| d.message.contains(pattern));
        assert!(
            found,
            "expected error containing '{}', got: {:?}",
            pattern,
            all_diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
    }
}

/// Parse source, run resolution, run inference, and verify warning messages.
/// Unlike `check_err`, this allows successful type checking while expecting warnings.
fn check_warn(source: &str, expected: &[&str]) {
    let parse_result = parse(source);
    assert!(
        parse_result.errors().is_empty(),
        "parse errors: {:?}",
        parse_result.errors()
    );
    let source_file = SourceFile::cast(parse_result.syntax()).expect("expected SourceFile");
    let resolve_result = resolve(&source_file);

    let infer_result = infer(&source_file, &resolve_result);

    // For warnings, we expect some diagnostics but type inference still succeeds
    assert!(
        !infer_result.diagnostics.is_empty(),
        "expected warnings containing {expected:?}, got none"
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
    check("fn main() { let x = \"hello\"; }", "str");
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
fn let_with_u16_annotation() {
    check("fn main() { let x: u16 = 42; }", "u16");
}

#[test]
fn let_with_u128_annotation() {
    check("fn main() { let x: u128 = 42; }", "u128");
}

#[test]
fn let_with_isize_annotation() {
    check("fn main() { let x: isize = 42; }", "isize");
}

#[test]
fn let_with_usize_annotation() {
    check("fn main() { let x: usize = 42; }", "usize");
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
    check("fn f(_ x: i64) {} fn main() { let a = 42; f(a); }", "i64");
}

#[test]
fn infer_int_from_fn_param_multiple() {
    // Both literals infer their types from the function parameters
    // The check function returns the last binding's type, so we check y's type (i64)
    check(
        "fn f(_ a: i32, _ b: i64) {} fn main() { let x = 1; let y = 2; f(x, y); }",
        "i64",
    );
}

#[test]
fn infer_float_from_fn_param() {
    check("fn f(_ x: f32) {} fn main() { let a = 3.14; f(a); }", "f32");
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
    check(
        "fn foo(_ x: i64) {} fn main() { let x = 42; foo(x); }",
        "i64",
    );
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
        "fn f(_ x: i64) {} fn main() { let a = 42; f(a); f(a); }",
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
    check("fn f(_ x: i32) {} fn main() { let y = f(42); }", "()");
}

#[test]
fn call_multiple_args() {
    check(
        "fn f(_ a: i32, _ b: bool) {} fn main() { let x = f(1, true); }",
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
        "fn f(): i32 { 1 } fn g(_ x: i32) {} fn main() { let y = g(f()); }",
        "()",
    );
}

#[test]
fn call_nested() {
    check(
        "fn f(_ x: i32): i32 { x } fn main() { let y = f(f(1)); }",
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
        "fn f(_ n: i32): i32 { if n == 0 { 0 } else { f(n - 1) } } fn main() { let x = f(5); }",
        "i32",
    );
}

#[test]
fn call_infers_literal_type() {
    check("fn f(_ x: i64) {} fn main() { let a = 42; f(a); }", "i64");
}

#[test]
fn call_infers_expr_type() {
    check(
        "fn f(_ x: i64) {} fn main() { let a = 1 + 2; f(a); }",
        "i64",
    );
}

#[test]
fn call_infers_var_type() {
    check("fn f(_ x: i64) {} fn main() { let a = 1; f(a); }", "i64");
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
        "fn f(_ x: &i32) {} fn main() { let a = 42; f(&a); let b = a; }",
        "i32",
    );
}

#[test]
fn ref_mut_in_function() {
    check(
        "fn f(_ x: &mut i32) {} fn main() { let mut a = 42; f(&mut a); let b = a; }",
        "i32",
    );
}

#[test]
fn ref_return() {
    check(
        "fn f(_ x: &i32): &i32 { x } fn main() { let a = 42; let b = f(&a); }",
        "&i32",
    );
}

#[test]
fn ref_coercion() {
    // Mutable reference can be coerced to shared reference
    check(
        "fn f(_ x: &i32) {} fn main() { let mut a = 42; f(&a); let b = a; }",
        "i32",
    );
}

#[test]
fn ref_infers_inner_type() {
    check("fn f(_ x: &i64) {} fn main() { let a = 42; f(&a); }", "i64");
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
        "struct S(a: i32) fn f(_ s: S) {} fn main() { let x = S(a: 1); f(x); }",
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
        "fn f(_ x: i32) {} fn main() { f(); }",
        &["expected 1 argument"],
    );
}

#[test]
fn error_too_many_args() {
    check_err(
        "fn f(_ x: i32) {} fn main() { f(1, 2); }",
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
        "fn main() { let x = if true { 1 } else { return; }; }",
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
        "fn f(_ x: i64): i64 { x } fn main() { let a = f(1 + 2); }",
        "i64",
    );
}

#[test]
fn diamond_inference() {
    // Same variable flows through multiple paths
    check(
        "fn f(_ x: i64) {} fn g(_ x: i64) {} fn main() { let a = 42; f(a); g(a); }",
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
        "fn f(_ x: i64): i64 { x } fn g(_ x: i64): i64 { x } fn h(_ x: i64): i64 { x } fn main() { let a = f(g(h(42))); }",
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
        "fn f(_ x: i32) {} fn g(x: i64) {} fn main() { let a = 1; f(a); g(a); }",
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
        "fn f(_ a: i32, _ b: i32, _ c: i32, _ d: i32, _ e: i32): i32 { a } fn main() { let x = f(1, 2, 3, 4, 5); }",
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
        "fn f(_ b: bool): i32 { if b { return 1; } return 2; } fn main() { let x = f(true); }",
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
    check("fn f(_ arr: [i64; 3]) { let x = &arr[0]; }", "&i64");
}

#[test]
fn inference_complex_expression_chain() {
    // Complex expression with multiple operators and calls
    check(
        "fn f(_ x: i64): i64 { x } fn main() { let x = f(1) + f(2) + f(3); }",
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
        "fn f(_ x: i64): i64 { x } fn g(_ x: i64): i64 { x } fn main() { let x = f(g(f(g(42)))); }",
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
        "fn f(_ a: i32, _ b: i32) {} fn main() { f(1); }",
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
        "fn f(_ x: i32) {} fn main() { f(\"hello\"); }",
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
    check_err("fn foo(_ x: i32) { x = 1; }", &["cannot assign"]);
}

#[test]
fn error_mut_ref_to_param() {
    check_err("fn foo(_ x: i32) { let y = &mut x; }", &["cannot borrow"]);
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
fn generic_fn_identity_infers_from_arg() {
    check(
        "fn identity(_ x: T): T where T { x } fn main() { let a = identity(42); }",
        "i32",
    );
}

#[test]
fn generic_fn_identity_infers_from_context() {
    check(
        "fn identity(_ x: T): T where T { x } fn main() { let a: i64 = identity(42); }",
        "i64",
    );
}

// Phase 2: Instantiate Generic Functions

#[test]
fn generic_fn_two_params_same_type() {
    check(
        "fn pair(_ a: T, _ b: T): T where T { a } fn main() { let x = pair(1, 2); }",
        "i32",
    );
}

#[test]
fn generic_fn_multiple_type_params() {
    check(
        "fn swap(_ a: A, _ b: B): B where A, B { b } fn main() { let x = swap(1, true); }",
        "bool",
    );
}

// Phase 3: Generic Struct Instantiation

#[test]
fn generic_struct_field_access() {
    check(
        "struct Wrapper(value: T) where T fn main() { let w = Wrapper(value: 42); let x = w.value; }",
        "i32",
    );
}

#[test]
fn generic_struct_multiple_fields() {
    check(
        "struct Pair(first: A, second: B) where A, B fn main() { let p = Pair(first: 1, second: true); let x = p.second; }",
        "bool",
    );
}

// Phase 4: Generic Methods

#[test]
fn generic_struct_method_returns_param() {
    check(
        "struct Wrapper(value: T) where T impl Wrapper(T) where T { fn get(&self): T { self.value } } fn main() { let w = Wrapper(value: 42); let x = w.get(); }",
        "i32",
    );
}

// Phase 5: Error Cases

#[test]
fn error_generic_type_mismatch() {
    check_err(
        "fn pair(_ a: T, _ b: T): T where T { a } fn main() { pair(1, true); }",
        &["type mismatch"],
    );
}

// Phase 6: Edge Cases

#[test]
fn generic_nested_calls() {
    check(
        "fn identity(_ x: T): T where T { x } fn main() { let a = identity(identity(42)); }",
        "i32",
    );
}

#[test]
fn generic_multiple_instantiations() {
    check(
        "fn identity(_ x: T): T where T { x } fn main() { let a = identity(42); let b = identity(true); }",
        "bool",
    );
}

// Phase 7: Method-Specific Type Parameters

#[test]
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
fn generic_fn_returns_generic_struct() {
    check(
        r#"
        struct Wrapper(value: T) where T
        fn wrap(_ x: T): Wrapper(T) where T { Wrapper(value: x) }
        fn main() {
            let w = wrap(42);
            let x = w.value;
        }
        "#,
        "i32",
    );
}

#[test]
fn generic_fn_returns_generic_struct_inferred_from_context() {
    check(
        r#"
        struct Wrapper(value: T) where T
        fn wrap(_ x: T): Wrapper(T) where T { Wrapper(value: x) }
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
        "fn f(_ b: bool): i32 { if b { return 1; } let x: i32 = 2; return x; }",
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
        "fn f(_ b: bool): i32 { if b { 1 } else { 2 } } fn main() { let x = f(true); }",
        "i32",
    );
}

#[test]
fn return_path_if_else_explicit() {
    // Both branches have explicit return - valid
    check(
        "fn f(_ b: bool): i32 { if b { return 1; } else { return 2; } } fn main() { let x = f(true); }",
        "i32",
    );
}

#[test]
fn return_path_diverging_then_tail() {
    // If one branch returns, tail after if is still reachable - valid
    check(
        "fn f(_ b: bool): i32 { if b { return 1; } return 0; } fn main() { let x = f(true); }",
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
        "fn f(_ a: bool, _ b: bool): i32 { if a { if b { 1 } else { 2 } } else { 3 } } fn main() { let x = f(true, false); }",
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
        "fn f(_ b: bool): i32 { if b { return 42; } }",
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
        "fn f(_ b: bool): i32 { if b { let x = 1; } else { 2 } }",
        &["type mismatch"],
    );
}

#[test]
fn error_missing_return_nested_if_incomplete() {
    // Nested if missing inner else
    check_err(
        "fn f(_ a: bool, _ b: bool): i32 { if a { if b { 1 } } else { 2 } }",
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
        "fn f(_ b: bool): i32 { if b { return 1; } else { return 2; } let x = 3; x }",
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
        "fn f(_ b: bool): i32 { if b { return 1; } let x = 2; return x; }",
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
        "fn f(_ x: i64, _ y: i64) {} fn main() { let a = 1; f(a, a); }",
        "i64",
    );
}

#[test]
fn int_var_constrained_by_multiple_contexts() {
    // Int var constrained by parameter and return type (both i64) should work
    check(
        "fn f(_ x: i64): i64 { x } fn main() { let a = 1; let b = f(a); }",
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
        "fn f(_ x: f64) {} fn main() { let a = 42; f(a); }",
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
        "fn f(_ x: f32, _ y: f32) {} fn main() { let a = 1.0; f(a, a); }",
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
        "fn f(_ x: i32) {} fn main() { let a = 3.14; f(a); }",
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
        "fn f(_ x: &i32) {} fn main() { let mut a = 42; f(&mut a); }",
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
        "fn f(_ x: &mut i32) {} fn main() { let a = 42; f(&a); }",
        &["mutability mismatch"],
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
        "fn main() { let x: i32 = if true { return; } else { 42 }; }",
        "i32",
    );
}

#[test]
fn never_in_if_else_propagates_then_type() {
    // When else-branch has never type, result type comes from then
    check(
        "fn main() { let x = if true { 42 } else { return; }; }",
        "i32",
    );
}

#[test]
fn both_branches_never() {
    // When both branches have never type, result is never
    // Note: The binding `x` will have type `!` (never)
    check(
        "fn main() { let x: i32 = if true { return; } else { return; }; }",
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
        "fn f(_ b: bool): i32 { if b { return 1; } return 42; } fn main() { let x = f(true); }",
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
fn type_var_bidirectional_through_function() {
    // Type should flow both ways: argument constrains param, return constrains usage
    check(
        "fn identity(_ x: T): T where T { x } fn main() { let a: i64 = identity(1); }",
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
        "fn f(_ x: i32): i32 { x * 2 } fn main() { let y = f(21); }",
        "i32",
    );
}

#[test]
fn implicit_return_single_if_expr() {
    // Single if-expression - implicit return allowed
    check(
        "fn f(_ a: i32, _ b: i32): i32 { if a > b { a } else { b } } fn main() { let x = f(1, 2); }",
        "i32",
    );
}

#[test]
fn explicit_return_with_statements() {
    // Statements with explicit return - allowed
    check(
        "fn f(_ x: i32): i32 { let y = x; return y + 1; } fn main() { let z = f(5); }",
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
        "fn f(_ x: i32): i32 { let y = x; y + 1 }",
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

#[test]
fn implicit_return_single_match_expr() {
    // Single match expression - implicit return allowed
    check(
        "fn f(_ x: i32): i32 { match x { 0 => 1, _ => 2 } } fn main() { let y = f(0); }",
        "i32",
    );
}

#[test]
fn implicit_return_single_loop_with_break() {
    // Single loop expression with break value - implicit return allowed
    check(
        "fn f(): i32 { loop { break 42; } } fn main() { let x = f(); }",
        "i32",
    );
}

#[test]
fn implicit_return_single_block_expr() {
    // Single block expression wrapping a value - implicit return allowed
    check("fn f(): i32 { { 42 } } fn main() { let x = f(); }", "i32");
}

#[test]
fn implicit_return_single_path() {
    // Single path expression (parameter) - implicit return allowed
    check(
        "fn f(_ x: i32): i32 { x } fn main() { let y = f(42); }",
        "i32",
    );
}

#[test]
fn implicit_return_single_call() {
    // Single function call - implicit return allowed
    check(
        "fn g(): i32 { 42 } fn f(): i32 { g() } fn main() { let x = f(); }",
        "i32",
    );
}

#[test]
fn explicit_return_as_tail_expr() {
    // Return expression with semicolon - allowed
    check(
        "fn f(_ x: i32): i32 { let y = x + 1; return y; } fn main() { let z = f(5); }",
        "i32",
    );
}

#[test]
fn error_while_statement_plus_implicit_return() {
    // While statement followed by implicit return - ERROR
    check_err(
        "fn f(): i32 { while false {} 42 }",
        &["implicit return not allowed when function body contains statements"],
    );
}

#[test]
fn error_for_statement_plus_implicit_return() {
    // For statement followed by implicit return - ERROR
    check_err(
        "fn f(): i32 { for i in 0..1 {} 42 }",
        &["implicit return not allowed when function body contains statements"],
    );
}

#[test]
fn error_if_no_else_statement_plus_implicit_return() {
    // If without else as statement followed by implicit return - ERROR
    check_err(
        "fn f(_ b: bool): i32 { if b { let x = 1; } 42 }",
        &["implicit return not allowed when function body contains statements"],
    );
}

#[test]
fn error_assignment_plus_implicit_return() {
    // Assignment statement followed by implicit return - ERROR
    check_err(
        "fn f(_ x: i32): i32 { let mut y = x; y = y + 1; y }",
        &["implicit return not allowed when function body contains statements"],
    );
}

#[test]
fn error_expr_statement_plus_implicit_return() {
    // Expression statement (function call with semicolon) followed by implicit return - ERROR
    check_err(
        "fn g() {} fn f(): i32 { g(); 42 }",
        &["implicit return not allowed when function body contains statements"],
    );
}

#[test]
fn error_method_with_statements_implicit_return() {
    // Method with statements and implicit return - ERROR
    check_err(
        "struct S(x: i32) impl S { fn get(&self): i32 { let y = self.x; y } }",
        &["implicit return not allowed when function body contains statements"],
    );
}

#[test]
fn unit_return_while_statement_allowed() {
    // Unit return type with while statement - no error (unit doesn't need return)
    check("fn f() { while false {} } fn main() { let x = f(); }", "()");
}

#[test]
fn unit_return_with_trailing_semicolon() {
    // Unit return with expression statement at end - allowed
    check("fn f() { let x = 1; x; } fn main() { let y = f(); }", "()");
}

// =============================================================================
// Is Expression Tests
// =============================================================================

#[test]
fn is_expr_returns_bool() {
    check("fn main() { let x = 42 is i32; }", "bool");
}

#[test]
fn is_expr_with_literal_pattern() {
    check("fn main() { let x = 42 is 42; }", "bool");
}

#[test]
fn is_expr_with_wildcard() {
    check("fn main() { let x = 42 is _; }", "bool");
}

#[test]
fn is_not_expr_returns_bool() {
    check("fn main() { let x = 42 is not 0; }", "bool");
}

#[test]
fn is_expr_tuple_pattern() {
    check("fn main() { let t = (1, 2); let b = t is (_, _); }", "bool");
}

#[test]
fn is_expr_binding_pattern() {
    check("fn main() { let x = 42 is n; }", "bool");
}

// =============================================================================
// Match Expression Tests
// =============================================================================

#[test]
fn match_simple_literal() {
    check("fn main() { let x = match 42 { 0 => 1, _ => 2 }; }", "i32");
}

#[test]
fn match_returns_unified_arm_type() {
    check(
        "fn main() { let x = match true { true => 1, false => 2 }; }",
        "i32",
    );
}

#[test]
fn match_arm_type_mismatch_error() {
    check_err(
        "fn main() { let x = match 42 { 0 => 1, _ => true }; }",
        &["type mismatch"],
    );
}

#[test]
fn match_with_guard() {
    check(
        "fn main() { let y = 0; let x = match 42 { n if n > y => 1, _ => 0 }; }",
        "i32",
    );
}

#[test]
fn match_guard_must_be_bool() {
    check_err(
        "fn main() { match 42 { n if n => 1, _ => 0 }; }",
        &["expected bool"],
    );
}

#[test]
fn match_binding_pattern() {
    check("fn main() { let x = match 42 { n => n }; }", "i32");
}

#[test]
fn match_tuple_destructuring() {
    check(
        "fn main() { let x = match (1, 2) { (a, b) => a + b }; }",
        "i32",
    );
}

#[test]
fn match_wildcard_exhaustive() {
    // No warning expected - wildcard is exhaustive
    check("fn main() { let x = match 42 { 0 => 1, _ => 0 }; }", "i32");
}

// ===== Generic functions with trait bounds (non-regression) =====

#[test]
fn generic_with_clone_bound_works() {
    check(
        "fn identity(_ x: T): T where T: Clone { x } fn main() { let a = identity(42); }",
        "i32",
    );
}

#[test]
fn generic_with_multiple_bounds_works() {
    check(
        "fn foo(_ x: T): T where T: Clone + Debug { x } fn main() { let a = foo(42); }",
        "i32",
    );
}

// =============================================================================
// 15. Named Parameters (spl-g14.9)
// =============================================================================

#[test]
fn labeled_param_with_correct_label() {
    check(
        "fn greet(to person: i32) {} fn main() { let x = greet(to: 42); }",
        "()",
    );
}

#[test]
fn labeled_param_missing_label() {
    check_err(
        "fn greet(to person: i32) {} fn main() { greet(42); }",
        &["expected labeled argument `to`"],
    );
}

#[test]
fn positional_param_works() {
    check(
        "fn add(_ a: i32, _ b: i32): i32 { a + b } fn main() { let x = add(1, 2); }",
        "i32",
    );
}

#[test]
fn positional_param_rejects_label() {
    check_err(
        "fn add(_ a: i32): i32 { a } fn main() { add(a: 1); }",
        &["unexpected label"],
    );
}

#[test]
fn default_label_matches_name() {
    check("fn foo(x: i32) {} fn main() { let r = foo(x: 42); }", "()");
}

#[test]
fn default_label_wrong_name() {
    check_err(
        "fn foo(x: i32) {} fn main() { foo(y: 42); }",
        &["expected label `x`"],
    );
}

#[test]
fn mixed_positional_and_labeled() {
    check(
        "fn range(from start: i32, _ count: i32): i32 { start + count } fn main() { let x = range(from: 0, 5); }",
        "i32",
    );
}

// -----------------------------------------------------------------------------
// 15.1 Two-parameter permutations (positional, default, external)
// -----------------------------------------------------------------------------

// (positional, positional)
#[test]
fn param_positional_positional() {
    check(
        "fn f(_ a: i32, _ b: i32): i32 { a + b } fn main() { let x = f(1, 2); }",
        "i32",
    );
}

// (positional, default)
#[test]
fn param_positional_default() {
    check(
        "fn f(_ a: i32, b: i32): i32 { a + b } fn main() { let x = f(1, b: 2); }",
        "i32",
    );
}

// (positional, external)
#[test]
fn param_positional_external() {
    check(
        "fn f(_ a: i32, to b: i32): i32 { a + b } fn main() { let x = f(1, to: 2); }",
        "i32",
    );
}

// (default, positional)
#[test]
fn param_default_positional() {
    check(
        "fn f(a: i32, _ b: i32): i32 { a + b } fn main() { let x = f(a: 1, 2); }",
        "i32",
    );
}

// (default, default)
#[test]
fn param_default_default() {
    check(
        "fn f(a: i32, b: i32): i32 { a + b } fn main() { let x = f(a: 1, b: 2); }",
        "i32",
    );
}

// (default, external)
#[test]
fn param_default_external() {
    check(
        "fn f(a: i32, to b: i32): i32 { a + b } fn main() { let x = f(a: 1, to: 2); }",
        "i32",
    );
}

// (external, positional)
#[test]
fn param_external_positional() {
    check(
        "fn f(from a: i32, _ b: i32): i32 { a + b } fn main() { let x = f(from: 1, 2); }",
        "i32",
    );
}

// (external, default)
#[test]
fn param_external_default() {
    check(
        "fn f(from a: i32, b: i32): i32 { a + b } fn main() { let x = f(from: 1, b: 2); }",
        "i32",
    );
}

// (external, external)
#[test]
fn param_external_external() {
    check(
        "fn f(from a: i32, to b: i32): i32 { a + b } fn main() { let x = f(from: 1, to: 2); }",
        "i32",
    );
}

// -----------------------------------------------------------------------------
// 15.2 Error cases for label mismatches
// -----------------------------------------------------------------------------

// Positional rejects any label
#[test]
fn error_positional_with_wrong_label() {
    check_err(
        "fn f(_ a: i32, _ b: i32) {} fn main() { f(x: 1, 2); }",
        &["unexpected label `x`"],
    );
}

// Default requires matching label
#[test]
fn error_default_with_wrong_label() {
    check_err(
        "fn f(a: i32, b: i32) {} fn main() { f(x: 1, b: 2); }",
        &["expected label `a`, found `x`"],
    );
}

// Default requires label (not positional)
#[test]
fn error_default_missing_label() {
    check_err(
        "fn f(a: i32, b: i32) {} fn main() { f(1, b: 2); }",
        &["expected labeled argument `a`"],
    );
}

// External requires matching label
#[test]
fn error_external_with_wrong_label() {
    check_err(
        "fn f(from a: i32) {} fn main() { f(to: 1); }",
        &["expected label `from`, found `to`"],
    );
}

// External requires label (not positional)
#[test]
fn error_external_missing_label() {
    check_err(
        "fn f(from a: i32) {} fn main() { f(1); }",
        &["expected labeled argument `from`"],
    );
}

// Mixed: first positional, second requires label
#[test]
fn error_mixed_second_missing_label() {
    check_err(
        "fn f(_ a: i32, b: i32) {} fn main() { f(1, 2); }",
        &["expected labeled argument `b`"],
    );
}

// Mixed: first requires label, second positional given label
#[test]
fn error_mixed_second_unexpected_label() {
    check_err(
        "fn f(a: i32, _ b: i32) {} fn main() { f(a: 1, b: 2); }",
        &["unexpected label `b`"],
    );
}

// -----------------------------------------------------------------------------
// 15.3 Three-parameter permutations (P=positional, D=default, E=external)
// All 27 permutations: 3^3
// -----------------------------------------------------------------------------

// PPP
#[test]
fn param3_ppp() {
    check(
        "fn f(_ a: i32, _ b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(1, 2, 3); }",
        "i32",
    );
}

// PPD
#[test]
fn param3_ppd() {
    check(
        "fn f(_ a: i32, _ b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(1, 2, c: 3); }",
        "i32",
    );
}

// PPE
#[test]
fn param3_ppe() {
    check(
        "fn f(_ a: i32, _ b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(1, 2, to: 3); }",
        "i32",
    );
}

// PDP
#[test]
fn param3_pdp() {
    check(
        "fn f(_ a: i32, b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(1, b: 2, 3); }",
        "i32",
    );
}

// PDD
#[test]
fn param3_pdd() {
    check(
        "fn f(_ a: i32, b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(1, b: 2, c: 3); }",
        "i32",
    );
}

// PDE
#[test]
fn param3_pde() {
    check(
        "fn f(_ a: i32, b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(1, b: 2, to: 3); }",
        "i32",
    );
}

// PEP
#[test]
fn param3_pep() {
    check(
        "fn f(_ a: i32, at b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(1, at: 2, 3); }",
        "i32",
    );
}

// PED
#[test]
fn param3_ped() {
    check(
        "fn f(_ a: i32, at b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(1, at: 2, c: 3); }",
        "i32",
    );
}

// PEE
#[test]
fn param3_pee() {
    check(
        "fn f(_ a: i32, at b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(1, at: 2, to: 3); }",
        "i32",
    );
}

// DPP
#[test]
fn param3_dpp() {
    check(
        "fn f(a: i32, _ b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, 2, 3); }",
        "i32",
    );
}

// DPD
#[test]
fn param3_dpd() {
    check(
        "fn f(a: i32, _ b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, 2, c: 3); }",
        "i32",
    );
}

// DPE
#[test]
fn param3_dpe() {
    check(
        "fn f(a: i32, _ b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, 2, to: 3); }",
        "i32",
    );
}

// DDP
#[test]
fn param3_ddp() {
    check(
        "fn f(a: i32, b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, b: 2, 3); }",
        "i32",
    );
}

// DDD
#[test]
fn param3_ddd() {
    check(
        "fn f(a: i32, b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, b: 2, c: 3); }",
        "i32",
    );
}

// DDE
#[test]
fn param3_dde() {
    check(
        "fn f(a: i32, b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, b: 2, to: 3); }",
        "i32",
    );
}

// DEP
#[test]
fn param3_dep() {
    check(
        "fn f(a: i32, at b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, at: 2, 3); }",
        "i32",
    );
}

// DED
#[test]
fn param3_ded() {
    check(
        "fn f(a: i32, at b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, at: 2, c: 3); }",
        "i32",
    );
}

// DEE
#[test]
fn param3_dee() {
    check(
        "fn f(a: i32, at b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(a: 1, at: 2, to: 3); }",
        "i32",
    );
}

// EPP
#[test]
fn param3_epp() {
    check(
        "fn f(from a: i32, _ b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, 2, 3); }",
        "i32",
    );
}

// EPD
#[test]
fn param3_epd() {
    check(
        "fn f(from a: i32, _ b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, 2, c: 3); }",
        "i32",
    );
}

// EPE
#[test]
fn param3_epe() {
    check(
        "fn f(from a: i32, _ b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, 2, to: 3); }",
        "i32",
    );
}

// EDP
#[test]
fn param3_edp() {
    check(
        "fn f(from a: i32, b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, b: 2, 3); }",
        "i32",
    );
}

// EDD
#[test]
fn param3_edd() {
    check(
        "fn f(from a: i32, b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, b: 2, c: 3); }",
        "i32",
    );
}

// EDE
#[test]
fn param3_ede() {
    check(
        "fn f(from a: i32, b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, b: 2, to: 3); }",
        "i32",
    );
}

// EEP
#[test]
fn param3_eep() {
    check(
        "fn f(from a: i32, at b: i32, _ c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, at: 2, 3); }",
        "i32",
    );
}

// EED
#[test]
fn param3_eed() {
    check(
        "fn f(from a: i32, at b: i32, c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, at: 2, c: 3); }",
        "i32",
    );
}

// EEE
#[test]
fn param3_eee() {
    check(
        "fn f(from a: i32, at b: i32, to c: i32): i32 { a + b + c } fn main() { let x = f(from: 1, at: 2, to: 3); }",
        "i32",
    );
}

// =============================================================================
// 12.0 Opaque Primitive Types (StrRef methods)
// =============================================================================
//
// Tests for opaque primitive types like StrRef that have methods instead of
// direct field access. StrRef has .ptr() and .len() methods.

// -----------------------------------------------------------------------------
// 12.1 Block Field Access Tests
// -----------------------------------------------------------------------------

#[test]
fn strref_field_0_blocked() {
    check_err(
        r#"fn main() { let s = "hello"; let p = s.0; }"#,
        &["no field `0`"],
    );
}

#[test]
fn strref_field_1_blocked() {
    check_err(
        r#"fn main() { let s = "hello"; let n = s.1; }"#,
        &["no field `1`"],
    );
}

// -----------------------------------------------------------------------------
// 12.2 Method Resolution Tests
// -----------------------------------------------------------------------------

#[test]
fn strref_ptr_method() {
    // Returns *u8
    check(r#"fn main() { let s = "hello"; let p = s.ptr(); }"#, "*u8");
}

#[test]
fn strref_len_method() {
    // Returns usize - test with explicit type to ensure type is correct
    check(
        r#"fn main() { let s: str = "hello"; let n = s.len(); }"#,
        "usize",
    );
}

#[test]
fn strref_method_on_literal() {
    check(r#"fn main() { let n = "hello".len(); }"#, "usize");
}

#[test]
fn strref_unknown_method() {
    check_err(
        r#"fn main() { "hello".foo(); }"#,
        &["method `foo` not found"],
    );
}

#[test]
fn strref_unknown_method_on_variable() {
    // This should give the same error as strref_unknown_method
    check_err(
        r#"fn main() { let s = "hello"; s.foo(); }"#,
        &["method `foo` not found"],
    );
}

#[test]
fn strref_method_wrong_args() {
    check_err(
        r#"fn main() { "hello".len(42); }"#,
        &["expected 0 argument"],
    );
}

// -----------------------------------------------------------------------------
// 12.3 Type Compatibility Tests
// -----------------------------------------------------------------------------

#[test]
fn strref_through_variable() {
    // Verify that a StrRef variable preserves its type
    check(r#"fn main() { let s = "hello"; let t = s; }"#, "str");
}

#[test]
fn strref_variable_in_function_call() {
    // Pass a string variable to a function
    check(
        r#"fn f(_ s: str) {} fn main() { let s = "hello"; f(s); let x = 1; }"#,
        "i32", // last binding is x
    );
}

#[test]
fn strref_len_is_usize() {
    // Verify .len() returns usize by passing it to a function expecting usize
    check(
        r#"fn f(_ n: usize) {} fn main() { f("hi".len()); let x = 1; }"#,
        "i32", // last binding is x; if len() didn't return usize, there'd be an error
    );
}

// -----------------------------------------------------------------------------
// Phase 3: Occurs Check Tests
// -----------------------------------------------------------------------------
// Note: Source-level self-referential tests like `let x = (x,);` are caught by
// the resolver as "cannot find `x`" before inference runs. The occurs check is
// tested at the unit level in unify.rs with tests like unify_err_occurs_check_tuple.

// -----------------------------------------------------------------------------
// Phase 5: SelfType Error Diagnostic Tests
// -----------------------------------------------------------------------------

#[test]
fn self_type_outside_impl_return_error() {
    // Using Self in a free function return type should error with a helpful message
    check_err(
        r#"fn foo(): Self {}"#,
        &["`Self` is only valid inside impl blocks"],
    );
}

#[test]
fn self_type_outside_impl_param_error() {
    // Using Self in a free function parameter should error
    check_err(
        r#"fn foo(x: Self) {}"#,
        &["`Self` is only valid inside impl blocks"],
    );
}

#[test]
fn self_type_outside_impl_let_error() {
    // Using Self in a let binding type should error
    check_err(
        r#"fn main() { let x: Self = 1; }"#,
        &["`Self` is only valid inside impl blocks"],
    );
}

#[test]
fn self_type_inside_impl_ok() {
    // Using Self inside impl block should work
    // This checks that Self resolves to the impl'd type (Foo)
    check(
        r#"
        struct Foo(value: i32)
        impl Foo {
            fn new(v: i32): Self { Foo(value: v) }
        }
        fn main() {
            let x = Foo.new(v: 42);
        }
        "#,
        "Foo",
    );
}

// ===== Inline Module Tests =====

#[test]
fn infer_inline_module_internal_function() {
    // Functions inside a module can call each other and infer types correctly
    check(
        r#"
        module m {
            fn helper(): i32 { 42 }
            pub fn f(): i32 { helper() }
        }
        fn main() {
            let x = 1;
        }
        "#,
        "i32",
    );
}

#[test]
fn infer_inline_module_internal_struct() {
    // Structs defined in modules can be used within the module
    check(
        r#"
        module types {
            pub struct Point(pub x: i32, pub y: i32)
            pub fn origin(): Point { Point(x: 0, y: 0) }
        }
        fn main() {
            let x = 1;
        }
        "#,
        "i32",
    );
}

#[test]
fn infer_inline_module_internal_method_call() {
    // Methods on structs in modules work within the module
    check(
        r#"
        module m {
            pub struct S(pub value: i32)
            impl S {
                pub fn get(&self): i32 { self.value }
            }
            pub fn test(): i32 {
                S(value: 42).get()
            }
        }
        fn main() {
            let x = 1;
        }
        "#,
        "i32",
    );
}

#[test]
fn infer_inline_module_nested() {
    // Nested modules work correctly
    check(
        r#"
        module outer {
            pub module inner {
                pub fn deep(): i32 { 42 }
            }
        }
        fn main() {
            let x = 1;
        }
        "#,
        "i32",
    );
}

// ===== Qualified Module Access Tests =====

#[test]
fn module_qualified_function_call() {
    // Basic module.function() access
    check(
        r#"
        module m { pub fn f(): i32 { 42 } }
        fn main() { let x = m.f(); }
        "#,
        "i32",
    );
}

#[test]
fn module_qualified_function_with_args() {
    check(
        r#"
        module math { pub fn add(a: i32, b: i32): i32 { a + b } }
        fn main() { let x = math.add(a: 1, b: 2); }
        "#,
        "i32",
    );
}

#[test]
fn module_qualified_struct_construction() {
    check(
        r#"
        module types { pub struct Point(pub x: i32, pub y: i32) }
        fn main() { let p = types.Point(x: 1, y: 2); }
        "#,
        "Point",
    );
}

#[test]
fn module_nested_access() {
    check(
        r#"
        module outer { pub module inner { pub fn deep(): i32 { 42 } } }
        fn main() { let x = outer.inner.deep(); }
        "#,
        "i32",
    );
}

#[test]
fn module_struct_method_chain() {
    check(
        r#"
        module m {
            pub struct S()
            impl S { pub fn value(&self): i32 { 42 } }
        }
        fn main() { let x = m.S().value(); }
        "#,
        "i32",
    );
}

#[test]
fn module_private_item_error() {
    // Private functions in modules should not be accessible externally
    check_err(
        r#"
        module m { fn private(): i32 { 42 } }
        fn main() { m.private() }
        "#,
        &["private"],
    );
}

#[test]
fn module_item_not_found_error() {
    check_err(
        r#"
        module m { pub fn a(): i32 { 1 } }
        fn main() { m.nonexistent() }
        "#,
        &["cannot find"],
    );
}

// ===== Visibility Tests for Qualified Access =====

#[test]
fn visibility_qualified_access_public_ok() {
    // Public functions in modules should be accessible via qualified access
    check(
        r#"
        module m { pub fn public_fn(): i32 { 42 } }
        fn main() { let x = m.public_fn(); }
        "#,
        "i32",
    );
}

#[test]
fn visibility_qualified_access_private_error() {
    // Private functions in modules should NOT be accessible via qualified access
    check_err(
        r#"
        module m { fn private_fn(): i32 { 42 } }
        fn main() { m.private_fn() }
        "#,
        &["private"],
    );
}

#[test]
fn visibility_nested_module_public_access() {
    // Public items in nested modules accessible via qualified path
    check(
        r#"
        module outer {
            pub module inner {
                pub fn nested_fn(): i32 { 42 }
            }
        }
        fn main() { let x = outer.inner.nested_fn(); }
        "#,
        "i32",
    );
}

#[test]
fn visibility_nested_module_private_error() {
    // Private items in nested modules NOT accessible
    check_err(
        r#"
        module outer {
            pub module inner {
                fn private_fn(): i32 { 42 }
            }
        }
        fn main() { outer.inner.private_fn() }
        "#,
        &["private"],
    );
}

#[test]
fn visibility_child_can_call_parent_private() {
    // Child module can access parent's private functions through scope
    check(
        r#"
        fn private_helper(): i32 { 42 }
        module child {
            pub fn call_parent(): i32 { private_helper() }
        }
        fn main() { let x = child.call_parent(); }
        "#,
        "i32",
    );
}

#[test]
fn visibility_grandchild_can_call_grandparent_private() {
    // Grandchild can access grandparent's private functions through scope chain
    check(
        r#"
        fn private_grandparent(): i32 { 100 }
        module parent {
            pub module child {
                pub fn call_ancestor(): i32 { private_grandparent() }
            }
        }
        fn main() { let x = parent.child.call_ancestor(); }
        "#,
        "i32",
    );
}

#[test]
fn visibility_sibling_cannot_call_private() {
    // One module cannot access sibling module's private items
    check_err(
        r#"
        module a { fn private_fn(): i32 { 1 } }
        module b {
            pub fn try_call(): i32 { a.private_fn() }
        }
        fn main() { b.try_call() }
        "#,
        &["private"],
    );
}

#[test]
fn visibility_sibling_can_call_public() {
    // One module CAN access sibling module's public items
    check(
        r#"
        module a { pub fn public_fn(): i32 { 1 } }
        module b {
            pub fn call_sibling(): i32 { a.public_fn() }
        }
        fn main() { let x = b.call_sibling(); }
        "#,
        "i32",
    );
}

#[test]
fn visibility_pub_struct_private_field_error() {
    // TODO(visibility): This test should pass once field visibility checking is implemented
    // Public struct with private field: field not accessible outside module
    check_err(
        r#"
        module m {
            pub struct S(x: i32)
        }
        fn main() {
            let s = m.S(x: 42);
            let _ = s.x;
        }
        "#,
        &["private"],
    );
}

#[test]
fn visibility_pub_struct_pub_field_ok() {
    // Public struct with public field: field accessible
    check(
        r#"
        module m {
            pub struct S(pub x: i32)
        }
        fn main() {
            let s = m.S(x: 42);
            let y = s.x;
        }
        "#,
        "i32",
    );
}

#[test]
fn visibility_private_method_same_module_ok() {
    // Private method accessible within same module
    check(
        r#"
        struct S()
        impl S {
            fn private_method(&self): i32 { 42 }
            pub fn public_method(&self): i32 { self.private_method() }
        }
        fn main() {
            let s = S();
            let x = s.public_method();
        }
        "#,
        "i32",
    );
}

#[test]
fn visibility_private_method_other_module_error() {
    // Private method NOT accessible from other module
    check_err(
        r#"
        module m {
            pub struct S()
            impl S {
                fn private_method(&self): i32 { 42 }
            }
        }
        fn main() {
            let s = m.S();
            s.private_method();
        }
        "#,
        &["private"],
    );
}

#[test]
fn visibility_pub_method_other_module_ok() {
    // Public method accessible from other module
    check(
        r#"
        module m {
            pub struct S()
            impl S {
                pub fn public_method(&self): i32 { 42 }
            }
        }
        fn main() {
            let s = m.S();
            let x = s.public_method();
        }
        "#,
        "i32",
    );
}

// =============================================================================
// Complex Nested Expressions
// =============================================================================

#[test]
fn complex_nested_binary_ops() {
    // Complex nested arithmetic with different operator precedence
    check("fn main() { let x = 1 + 2 * 3 - 4 / 2; }", "i32");
}

#[test]
fn complex_nested_comparisons() {
    // Chained comparisons (logical)
    check("fn main() { let x = 1 < 2 && 3 > 2 || 4 == 4; }", "bool");
}

#[test]
fn complex_nested_call_and_binary() {
    // SPL requires labeled arguments for named params
    check(
        r#"
        fn add(a: i32, b: i32): i32 { a + b }
        fn main() { let x = add(a: 1, b: 2) + add(a: 3, b: 4) * 2; }
        "#,
        "i32",
    );
}

#[test]
fn complex_nested_field_access_and_call() {
    check(
        r#"
        struct Point(x: i32, y: i32)
        fn get_x(p: Point): i32 { p.x }
        fn main() {
            let p = Point(x: 10, y: 20);
            let result = get_x(p: p) + p.y;
        }
        "#,
        "i32",
    );
}

#[test]
fn complex_nested_if_in_binary() {
    check(
        "fn main() { let x = 1 + if true { 2 } else { 3 } + 4; }",
        "i32",
    );
}

#[test]
fn complex_deeply_nested_parens() {
    check("fn main() { let x = (((1 + 2) * 3) - 4) / 2; }", "i32");
}

#[test]
fn complex_nested_cast_in_binary() {
    check("fn main() { let x = (1 as i64) + (2 as i64); }", "i64");
}

#[test]
fn complex_nested_ref_and_deref() {
    check(
        r#"
        fn main() {
            let x = 42;
            let r = &x;
            let y = *r + 1;
        }
        "#,
        "i32",
    );
}

// =============================================================================
// Method Call Chains
// =============================================================================

#[test]
fn method_call_simple() {
    check(
        r#"
        struct Counter(value: i32)
        impl Counter {
            fn get(&self): i32 { self.value }
        }
        fn main() {
            let c = Counter(value: 42);
            let x = c.get();
        }
        "#,
        "i32",
    );
}

#[test]
fn method_chain_two_calls() {
    check(
        r#"
        struct Builder(value: i32)
        impl Builder {
            fn add(&mut self, n: i32): &mut Builder {
                self.value = self.value + n;
                return self;
            }
            fn build(&self): i32 { self.value }
        }
        fn main() {
            let mut b = Builder(value: 0);
            let x = b.add(n: 5).build();
        }
        "#,
        "i32",
    );
}

#[test]
fn method_chain_three_calls() {
    check(
        r#"
        struct Builder(value: i32)
        impl Builder {
            fn add(&mut self, n: i32): &mut Builder {
                self.value = self.value + n;
                return self;
            }
            fn mul(&mut self, n: i32): &mut Builder {
                self.value = self.value * n;
                return self;
            }
            fn build(&self): i32 { self.value }
        }
        fn main() {
            let mut b = Builder(value: 1);
            let x = b.add(n: 2).mul(n: 3).build();
        }
        "#,
        "i32",
    );
}

#[test]
fn method_chain_with_field_access() {
    check(
        r#"
        struct Container(inner: Inner)
        struct Inner(value: i32)
        impl Inner {
            fn get(&self): i32 { self.value }
        }
        fn main() {
            let c = Container(inner: Inner(value: 42));
            let x = c.inner.get();
        }
        "#,
        "i32",
    );
}

// =============================================================================
// Edge Cases in Casting
// =============================================================================

#[test]
fn cast_chain() {
    // Cast from one type to another and then to a third
    check("fn main() { let x = 42 as i8 as i64; }", "i64");
}

#[test]
fn cast_in_function_call() {
    check(
        r#"
        fn take_i64(n: i64): i64 { n }
        fn main() { let x = take_i64(n: 42 as i64); }
        "#,
        "i64",
    );
}

#[test]
fn cast_float_to_different_sizes() {
    check("fn main() { let x = 3.14f64 as f32; }", "f32");
}

#[test]
fn cast_in_comparison() {
    check("fn main() { let x = (1 as i64) == (2 as i64); }", "bool");
}

#[test]
fn cast_in_array_index() {
    check(
        r#"
        fn main() {
            let arr = [1, 2, 3];
            let idx: i64 = 1;
            let x = arr[idx as i32];
        }
        "#,
        "i32",
    );
}

// =============================================================================
// Generic Functions with Multiple Type Parameters
// =============================================================================

#[test]
fn generic_two_type_params() {
    // SPL uses `where T, U` syntax for generics
    check(
        r#"
        fn pair(_ a: T, _ b: U): T where T, U { a }
        fn main() { let x = pair(1, true); }
        "#,
        "i32",
    );
}

#[test]
fn generic_nested_instantiation() {
    check(
        r#"
        fn identity(_ x: T): T where T { x }
        fn main() { let x = identity(identity(42)); }
        "#,
        "i32",
    );
}

#[test]
fn generic_with_struct_constraint() {
    check(
        r#"
        struct Wrapper(value: T) where T
        fn unwrap(_ w: Wrapper(T)): T where T { w.value }
        fn main() {
            let w = Wrapper(value: 42);
            let x = unwrap(w);
        }
        "#,
        "i32",
    );
}

#[test]
fn generic_function_returning_tuple() {
    check(
        r#"
        fn make_pair(_ a: T, _ b: T): (T, T) where T { (a, b) }
        fn main() { let x = make_pair(1, 2); }
        "#,
        "(i32, i32)",
    );
}

// =============================================================================
// Complex Type Alias Scenarios
// =============================================================================

#[test]
fn type_alias_in_function_param() {
    check(
        r#"
        type MyInt = i32;
        fn take_int(n: MyInt): MyInt { n }
        fn main() { let x = take_int(n: 42); }
        "#,
        "i32",
    );
}

#[test]
fn type_alias_in_struct_field() {
    check(
        r#"
        type MyInt = i32;
        struct S(value: MyInt)
        fn main() {
            let s = S(value: 42);
            let x = s.value;
        }
        "#,
        "i32",
    );
}

#[test]
#[ignore = "generic type alias instantiation syntax Pair(i32) causes parser issues"]
fn type_alias_generic() {
    // SPL uses `where T` syntax for generic type aliases
    check(
        r#"
        type Pair = (T, T) where T;
        fn make_pair(): Pair(i32) { (1, 2) }
        fn main() { let x = make_pair(); }
        "#,
        "(i32, i32)",
    );
}

// =============================================================================
// Edge Cases in Type Inference
// =============================================================================

#[test]
fn infer_array_from_literal_elements() {
    check("fn main() { let x = [1, 2, 3]; }", "[i32; 3]");
}

#[test]
fn infer_tuple_heterogeneous() {
    check("fn main() { let x = (1, true, 3.14); }", "(i32, bool, f64)");
}

#[test]
fn infer_empty_tuple() {
    check("fn main() { let x = (); }", "()");
}

#[test]
fn infer_unit_return_implicit() {
    check(
        r#"
        fn no_return() { let _ = 1; }
        fn main() { let x = no_return(); }
        "#,
        "()",
    );
}

#[test]
fn infer_block_with_semicolon() {
    check("fn main() { let x = { 42; }; }", "()");
}

#[test]
fn infer_block_without_semicolon() {
    check("fn main() { let x = { 42 }; }", "i32");
}

#[test]
fn infer_nested_blocks() {
    check("fn main() { let x = { { { 42 } } }; }", "i32");
}

#[test]
fn infer_if_both_branches_same_type() {
    check("fn main() { let x = if true { 1 } else { 2 }; }", "i32");
}

#[test]
fn infer_match_all_arms_same_type() {
    check(
        r#"
        fn main() {
            let n = 1;
            let x = match n {
                0 => 10,
                1 => 20,
                _ => 30,
            };
        }
        "#,
        "i32",
    );
}

#[test]
fn infer_loop_break_value() {
    check(
        r#"
        fn main() {
            let x = loop {
                break 42;
            };
        }
        "#,
        "i32",
    );
}

// =============================================================================
// Complex Struct Patterns
// =============================================================================

#[test]
fn struct_pattern_nested() {
    // SPL uses parentheses syntax for struct patterns
    // Note: check() tests the first binding, so we need a helper function
    check(
        r#"
        struct Inner(value: i32)
        struct Outer(inner: Inner)
        fn make(): Outer { Outer(inner: Inner(value: 42)) }
        fn main() {
            let Outer(inner: Inner(value: x)) = make();
        }
        "#,
        "i32",
    );
}

#[test]
fn struct_pattern_with_rest() {
    check(
        r#"
        struct Point(x: i32, y: i32, z: i32)
        fn make(): Point { Point(x: 1, y: 2, z: 3) }
        fn main() {
            let Point(x: x, ..) = make();
        }
        "#,
        "i32",
    );
}

#[test]
fn tuple_pattern_in_let() {
    check(
        r#"
        fn main() {
            let pair = (1, true);
            let (a, b) = pair;
        }
        "#,
        "bool",
    );
}

// =============================================================================
// Type Inference Edge Cases (spl-69ov)
// =============================================================================

// -----------------------------------------------------------------------------
// Mutually Recursive Functions
// -----------------------------------------------------------------------------

#[test]
fn mutually_recursive_functions_simple() {
    // Two functions that call each other
    check(
        r#"
        fn f(_ n: i32): i32 { if n == 0 { 0 } else { g(n - 1) } }
        fn g(_ n: i32): i32 { if n == 0 { 1 } else { f(n - 1) } }
        fn main() { let x = f(10); }
        "#,
        "i32",
    );
}

#[test]
fn mutually_recursive_functions_three_way() {
    // Three functions that form a cycle
    check(
        r#"
        fn a(_ n: i32): i32 { if n == 0 { 0 } else { b(n - 1) } }
        fn b(_ n: i32): i32 { if n == 0 { 1 } else { c(n - 1) } }
        fn c(_ n: i32): i32 { if n == 0 { 2 } else { a(n - 1) } }
        fn main() { let x = a(5); }
        "#,
        "i32",
    );
}

#[test]
fn mutually_recursive_with_different_return_types() {
    // Mutually recursive with bool/i32 return types
    check(
        r#"
        fn is_even(_ n: i32): bool { if n == 0 { true } else { is_odd(n - 1) } }
        fn is_odd(_ n: i32): bool { if n == 0 { false } else { is_even(n - 1) } }
        fn main() { let x = is_even(4); }
        "#,
        "bool",
    );
}

// -----------------------------------------------------------------------------
// Nested Generic Types
// -----------------------------------------------------------------------------

#[test]
fn nested_array_in_array() {
    check(
        "fn main() { let x: [[i32; 2]; 3] = [[1, 2], [3, 4], [5, 6]]; }",
        "[[i32; 2]; 3]",
    );
}

#[test]
fn nested_tuple_in_tuple() {
    check(
        "fn main() { let x: ((i32, i64), (bool, f64)) = ((1, 2), (true, 3.14)); }",
        "((i32, i64), (bool, f64))",
    );
}

#[test]
fn deeply_nested_array() {
    check(
        "fn main() { let x: [[[i32; 1]; 1]; 1] = [[[42]]]; }",
        "[[[i32; 1]; 1]; 1]",
    );
}

#[test]
fn mixed_nested_types() {
    // Tuple containing array
    check(
        "fn main() { let x: ([i32; 2], bool) = ([1, 2], true); }",
        "([i32; 2], bool)",
    );
}

#[test]
fn array_of_tuples() {
    check(
        "fn main() { let x: [(i32, bool); 2] = [(1, true), (2, false)]; }",
        "[(i32, bool); 2]",
    );
}

// -----------------------------------------------------------------------------
// Complex Control Flow Type Inference
// -----------------------------------------------------------------------------

#[test]
fn if_else_with_loop_break() {
    // If-else where one branch is a loop with break
    check(
        r#"
        fn main() {
            let x: i32 = if true { 1 } else { loop { break 2; } };
        }
        "#,
        "i32",
    );
}

#[test]
fn nested_if_inference() {
    // Deeply nested if-else
    check(
        r#"
        fn f(_ a: bool, _ b: bool, _ c: bool): i32 {
            if a {
                if b { 1 } else { 2 }
            } else {
                if c { 3 } else { 4 }
            }
        }
        fn main() { let x = f(true, false, true); }
        "#,
        "i32",
    );
}

#[test]
fn while_with_break_value_inference() {
    // While loop doesn't produce values, test that types flow through
    check(
        r#"
        fn main() {
            let mut x: i32 = 0;
            while x < 10 {
                x = x + 1;
            }
            let y = x;
        }
        "#,
        "i32",
    );
}

#[test]
fn match_all_branches_same_type() {
    // Match expression type inference
    check(
        r#"
        fn main() {
            let x: i32 = 5;
            let y = match x {
                0 => 100,
                1 => 200,
                _ => 300,
            };
        }
        "#,
        "i32",
    );
}

#[test]
fn match_with_complex_patterns() {
    check(
        r#"
        fn main() {
            let pair = (1, 2);
            let x = match pair {
                (0, y) => y,
                (x, 0) => x,
                (a, b) => a + b,
            };
        }
        "#,
        "i32",
    );
}

// -----------------------------------------------------------------------------
// Edge Cases with Type Unification
// -----------------------------------------------------------------------------

#[test]
fn unify_through_function_call_chain() {
    // Type flows through multiple function calls
    check(
        r#"
        fn identity(_ x: i64): i64 { x }
        fn double(_ x: i64): i64 { x * 2 }
        fn main() {
            let a = 5;
            let b = identity(a);
            let c = double(b);
        }
        "#,
        "i64",
    );
}

#[test]
fn infer_from_multiple_constraints() {
    // Same variable used with multiple type constraints (should be consistent)
    check(
        r#"
        fn take_i64(_ x: i64) {}
        fn main() {
            let x = 42;
            take_i64(x);
            let y: i64 = x;
        }
        "#,
        "i64",
    );
}

#[test]
fn bidirectional_array_element_inference() {
    // Infer array element type from usage
    check(
        r#"
        fn take_i64(_ x: i64) {}
        fn main() {
            let arr = [1, 2, 3];
            take_i64(arr[0]);
        }
        "#,
        "[i64; 3]",
    );
}

#[test]
fn struct_field_type_inference() {
    // Infer type from struct field access
    check(
        r#"
        struct Point(x: i64, y: i64)
        fn main() {
            let p = Point(x: 1, y: 2);
            let x = p.x;
        }
        "#,
        "i64",
    );
}

// =============================================================================
// Missing Test Coverage (spl-qyu, spl-4fz, spl-5pe, spl-6ou)
// =============================================================================

// -----------------------------------------------------------------------------
// Comparison Chains (spl-qyu)
// -----------------------------------------------------------------------------

#[test]
fn comparison_chain_less_than() {
    // a < b && b < c - typical range check pattern
    check(
        r#"
        fn main() {
            let a = 1;
            let b = 2;
            let c = 3;
            let in_range = a < b && b < c;
        }
        "#,
        "bool",
    );
}

#[test]
fn comparison_chain_mixed_operators() {
    // a <= b && b < c - mixed comparison operators
    check(
        r#"
        fn main() {
            let x = 5;
            let result = 0 <= x && x < 10;
        }
        "#,
        "bool",
    );
}

#[test]
fn comparison_chain_three_comparisons() {
    // a < b && b < c && c < d - three-way chain
    check(
        r#"
        fn main() {
            let result = 1 < 2 && 2 < 3 && 3 < 4;
        }
        "#,
        "bool",
    );
}

#[test]
fn comparison_chain_with_function_calls() {
    // Comparison chain using function return values
    check(
        r#"
        fn min(): i32 { 0 }
        fn max(): i32 { 100 }
        fn value(): i32 { 50 }
        fn main() {
            let in_bounds = min() <= value() && value() <= max();
        }
        "#,
        "bool",
    );
}

// -----------------------------------------------------------------------------
// Rest Patterns in Nested Contexts (spl-4fz)
// -----------------------------------------------------------------------------

#[test]
fn rest_pattern_in_nested_tuple() {
    // Rest pattern inside a nested tuple
    check(
        r#"
        fn main() {
            let outer = ((1, 2, 3), true);
            let ((a, ..), flag) = outer;
        }
        "#,
        "bool",
    );
}

#[test]
fn rest_pattern_in_nested_struct() {
    // Rest pattern inside nested struct pattern
    check(
        r#"
        struct Inner(a: i32, b: i32, c: i32)
        struct Outer(inner: Inner, flag: bool)
        fn make(): Outer { Outer(inner: Inner(a: 1, b: 2, c: 3), flag: true) }
        fn main() {
            let Outer(inner: Inner(a: x, ..), flag: f) = make();
        }
        "#,
        "bool",
    );
}

#[test]
fn rest_pattern_multiple_levels() {
    // Rest patterns at multiple nesting levels
    check(
        r#"
        struct Point(x: i32, y: i32, z: i32)
        fn main() {
            let points = (Point(x: 1, y: 2, z: 3), Point(x: 4, y: 5, z: 6));
            let (Point(x: x1, ..), ..) = points;
        }
        "#,
        "i32",
    );
}

// -----------------------------------------------------------------------------
// Deeply Nested Patterns (spl-5pe)
// -----------------------------------------------------------------------------

#[test]
fn deeply_nested_tuple_pattern() {
    // Three levels of tuple nesting
    check(
        r#"
        fn main() {
            let nested = (((1, 2), 3), 4);
            let (((a, b), c), d) = nested;
        }
        "#,
        "i32",
    );
}

#[test]
fn deeply_nested_struct_pattern() {
    // Three levels of struct nesting
    check(
        r#"
        struct A(value: i32)
        struct B(a: A)
        struct C(b: B)
        fn make(): C { C(b: B(a: A(value: 42))) }
        fn main() {
            let C(b: B(a: A(value: x))) = make();
        }
        "#,
        "i32",
    );
}

#[test]
fn deeply_nested_mixed_pattern() {
    // Mixed struct and tuple nesting
    check(
        r#"
        struct Pair(first: i32, second: i32)
        fn main() {
            let data = ((Pair(first: 1, second: 2), true), false);
            let ((Pair(first: x, second: y), inner_flag), outer_flag) = data;
        }
        "#,
        "bool",
    );
}

// -----------------------------------------------------------------------------
// Mixed Struct/Tuple Patterns (spl-6ou)
// -----------------------------------------------------------------------------

#[test]
fn tuple_containing_struct() {
    // Tuple with struct elements
    check(
        r#"
        struct Point(x: i32, y: i32)
        fn main() {
            let pair = (Point(x: 1, y: 2), Point(x: 3, y: 4));
            let (Point(x: x1, y: y1), Point(x: x2, y: y2)) = pair;
        }
        "#,
        "i32",
    );
}

#[test]
fn struct_containing_tuple() {
    // Struct with tuple field
    check(
        r#"
        struct Container(pair: (i32, bool))
        fn make(): Container { Container(pair: (42, true)) }
        fn main() {
            let Container(pair: (num, flag)) = make();
        }
        "#,
        "bool",
    );
}

#[test]
fn alternating_struct_tuple_pattern() {
    // Alternating struct and tuple nesting
    check(
        r#"
        struct Wrapper(inner: (i32, i32))
        fn main() {
            let data = (Wrapper(inner: (1, 2)), Wrapper(inner: (3, 4)));
            let (Wrapper(inner: (a, b)), Wrapper(inner: (c, d))) = data;
        }
        "#,
        "i32",
    );
}

#[test]
fn range_int_float_mismatch() {
    check_err("fn main() { let r = 0..10.0; }", &["type mismatch"]);
}

// =============================================================================
// Function Pointer Type Inference
// =============================================================================

#[test]
fn fn_ptr_type_in_param() {
    // Function pointer types in parameters parse and type-check correctly
    check(
        "fn apply(_ f: fn(i32) -> i32, _ x: i32): i32 { 0 } fn main() { let x = 0; }",
        "i32",
    );
}

#[test]
fn fn_ptr_type_in_let() {
    // Function pointer types in let bindings parse correctly
    // (even if coercion from fn def isn't implemented yet)
    check_err(
        "fn add(_ a: i32, _ b: i32): i32 { a + b } fn main() { let f: fn(i32, i32) -> i32 = add; }",
        &["type mismatch"], // fn def to fn ptr coercion not yet implemented
    );
}

#[test]
fn fn_ptr_param_mismatch() {
    check_err(
        "fn add(_ a: i32, _ b: i32): i32 { a + b } fn main() { let f: fn(i64, i32) -> i32 = add; }",
        &["type mismatch"],
    );
}

#[test]
fn fn_ptr_return_mismatch() {
    check_err(
        "fn add(_ a: i32, _ b: i32): i32 { a + b } fn main() { let f: fn(i32, i32) -> i64 = add; }",
        &["type mismatch"],
    );
}

// =============================================================================
// Slice/Array Pattern Type Inference
// =============================================================================

#[test]
#[ignore = "slice pattern destructuring not yet implemented"]
fn slice_pattern_basic() {
    // Basic slice pattern with 2 elements - check type of extracted element
    check(
        r#"
        fn main() {
            let arr = [1, 2];
            let [a, b] = arr;
            let x = a;
        }
        "#,
        "i32",
    );
}

#[test]
#[ignore = "slice pattern destructuring not yet implemented"]
fn slice_pattern_with_annotation() {
    // Slice pattern with type annotation - check type of extracted element
    check(
        r#"
        fn main() {
            let arr: [i64; 3] = [1, 2, 3];
            let [a, b, c] = arr;
            let x = a;
        }
        "#,
        "i64",
    );
}

#[test]
#[ignore = "slice pattern destructuring not yet implemented"]
fn slice_pattern_with_rest() {
    // Slice pattern with rest to capture remaining elements
    check(
        r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5];
            let [first, .., last] = arr;
            let x = first;
        }
        "#,
        "i32",
    );
}

#[test]
fn slice_pattern_wrong_element_type() {
    // Pattern elements must match array element type
    check_err(
        r#"
        fn main() {
            let arr = [1, 2];
            let [a, b]: [bool; 2] = arr;
        }
        "#,
        &["type mismatch"],
    );
}

// =============================================================================
// Range Type Inference
// =============================================================================

#[test]
fn range_inference_basic() {
    // Range infers element type from bounds
    check("fn main() { let r = 0..10; }", "i32");
}

#[test]
fn range_inference_i64() {
    // Range with i64 bounds infers i64 element type
    check("fn main() { let x: i64 = 0; let r = x..10; }", "i64");
}

#[test]
fn range_from_inference() {
    // Range from (start..) infers type from start
    check("fn main() { let r = 5..; }", "i32");
}

#[test]
fn range_to_inference() {
    // Range to (..end) infers type from end
    check("fn main() { let r = ..10; }", "i32");
}

#[test]
fn range_incompatible_bounds() {
    // Start and end bounds must have compatible types
    check_err("fn main() { let r = 0i32..10i64; }", &["type mismatch"]);
}

// =============================================================================
// Mutable Borrows in Patterns
// =============================================================================

#[test]
fn mut_borrow_in_let_pattern() {
    // Basic mutable borrow in let binding
    check(
        r#"
        fn main() {
            let mut x = 42;
            let y = &mut x;
        }
        "#,
        "&mut i32",
    );
}

#[test]
fn mut_borrow_in_tuple_pattern() {
    // Mutable borrow within tuple destructuring
    check(
        r#"
        fn main() {
            let mut a = 1;
            let mut b = 2;
            let (x, y) = (&mut a, &mut b);
        }
        "#,
        "&mut i32",
    );
}

#[test]
fn mut_borrow_in_struct_pattern() {
    // Mutable borrow in struct field pattern
    check(
        r#"
        struct Pair(first: i32, second: i32)
        fn main() {
            let mut p = Pair(first: 1, second: 2);
            let r = &mut p.first;
        }
        "#,
        "&mut i32",
    );
}

#[test]
fn mut_borrow_error_immutable_source() {
    // Cannot mutably borrow immutable variable
    check_err(
        "fn main() { let x = 42; let y = &mut x; }",
        &["cannot borrow"],
    );
}

#[test]
fn mut_borrow_error_through_shared_ref() {
    // Cannot mutably borrow through shared reference
    check_err(
        r#"
        fn main() {
            let mut x = 42;
            let r = &x;
            let m = &mut *r;
        }
        "#,
        &["cannot borrow"],
    );
}

#[test]
fn mut_borrow_reborrow_allowed() {
    // Reborrowing a mutable reference is allowed
    check(
        r#"
        fn main() {
            let mut x = 42;
            let r1 = &mut x;
            let r2 = &mut *r1;
        }
        "#,
        "&mut i32",
    );
}

#[test]
fn mut_borrow_fn_param_immutable() {
    // Function parameter is immutable by default - cannot borrow mutably
    check_err("fn foo(_ x: i32) { let y = &mut x; }", &["cannot borrow"]);
}

#[test]
fn mut_borrow_nested_struct() {
    // Mutable borrow through nested struct access
    check(
        r#"
        struct Inner(value: i32)
        struct Outer(inner: Inner)
        fn main() {
            let mut o = Outer(inner: Inner(value: 42));
            let r = &mut o.inner.value;
        }
        "#,
        "&mut i32",
    );
}

// =============================================================================
// Built-in Types Visibility
// =============================================================================

#[test]
fn builtin_i32_accessible() {
    // i32 is accessible without imports
    check("fn main() { let x: i32 = 42; }", "i32");
}

#[test]
fn builtin_types_in_function_params() {
    // Built-in types work in function parameters
    check(
        "fn foo(_ a: i32, _ b: bool, _ c: f64): i64 { 0 } fn main() { let x = foo(1, true, 1.0); }",
        "i64",
    );
}

#[test]
fn builtin_types_in_nested_module() {
    // Built-in types accessible within nested modules
    check(
        r#"
        module inner {
            pub fn get_value(): i32 { 42 }
        }
        fn main() { let x = inner.get_value(); }
        "#,
        "i32",
    );
}

#[test]
fn builtin_unit_type() {
    // Unit type () is a built-in
    check("fn main() { let x: () = (); }", "()");
}

#[test]
fn builtin_bool_type() {
    // Bool type works correctly
    check("fn main() { let x: bool = true && false; }", "bool");
}

#[test]
fn builtin_all_integer_types() {
    // All integer types are accessible
    check(
        "fn main() { let a: i8 = 1; let b: i16 = 2; let c: i32 = 3; let d: i64 = 4; let e: i128 = 5; let f: u8 = 6; let g: u16 = 7; let h: u32 = 8; let i: u64 = 9; let j: u128 = 10; }",
        "u128",
    );
}

// =============================================================================
// Generic Syntax Tests
// =============================================================================

#[test]
fn generic_in_type_annotation() {
    // Generic types in let annotations work (type display shows struct name without args)
    check(
        r#"
        struct Box(value: T) where T
        fn main() {
            let b: Box(i32) = Box(value: 42);
        }
        "#,
        "Box",
    );
}

#[test]
fn generic_in_function_param() {
    // Generic types in function parameters - returns element type
    check(
        r#"
        struct Wrapper(value: T) where T
        fn unwrap(_ w: Wrapper(T)): T where T { w.value }
        fn main() {
            let w = Wrapper(value: 42);
            let x = unwrap(w);
        }
        "#,
        "i32",
    );
}

#[test]
fn generic_in_function_return() {
    // Generic types in function return type
    check(
        r#"
        struct Pair(a: T, b: T) where T
        fn make_pair(_ x: T, _ y: T): Pair(T) where T {
            Pair(a: x, b: y)
        }
        fn main() {
            let p = make_pair(1, 2);
        }
        "#,
        "Pair",
    );
}

#[test]
fn generic_nested_types() {
    // Nested generic types
    check(
        r#"
        struct Outer(inner: T) where T
        struct Inner(value: U) where U
        fn main() {
            let x: Outer(Inner(i32)) = Outer(inner: Inner(value: 42));
        }
        "#,
        "Outer",
    );
}

#[test]
fn generic_multiple_params() {
    // Multiple generic parameters
    check(
        r#"
        struct Pair(first: A, second: B) where A, B
        fn main() {
            let p: Pair(i32, bool) = Pair(first: 42, second: true);
        }
        "#,
        "Pair",
    );
}

#[test]
fn generic_inferred_from_usage() {
    // Generic type inferred from context
    check(
        r#"
        struct Box(value: T) where T
        fn main() {
            let b = Box(value: 42);
        }
        "#,
        "Box",
    );
}

// =============================================================================
// Yield Expressions
// =============================================================================

// --- Basic Yield (4 tests) ---

#[test]
fn yield_simple() {
    check("fn main() { let x = { yield 42; }; }", "i32");
}

#[test]
fn yield_infers_from_context() {
    check("fn main() { let x: i64 = { yield 42; }; }", "i64");
}

#[test]
fn yield_no_value() {
    check("fn main() { let x = { yield; }; }", "()");
}

#[test]
fn yield_expression_type_is_never() {
    check("fn main() { { let x = yield 42; }; }", "!");
}

// --- Yield with Tail Expression (3 tests) ---

#[test]
fn yield_before_tail() {
    check("fn main() { let x = { if true { yield 1; } 2 }; }", "i32");
}

#[test]
fn yield_multiple_paths() {
    check(
        "fn main() { let x = { if true { yield 1; } else { yield 2; } }; }",
        "i32",
    );
}

#[test]
fn yield_with_tail_unifies() {
    check(
        "fn main() { let x: i64 = { if true { yield 1; } 2 }; }",
        "i64",
    );
}

// --- Validation Errors (4 tests) ---

#[test]
fn error_yield_outside_block() {
    check_err(
        "fn main() { yield 42; }",
        &["yield outside of block expression"],
    );
}

#[test]
fn error_yield_in_function_body() {
    check_err(
        "fn f() { let x = 1; yield x; }",
        &["yield outside of block expression"],
    );
}

#[test]
fn error_yield_type_mismatch() {
    check_err(
        r#"fn main() { let x: i32 = { yield "hello"; }; }"#,
        &["type mismatch"],
    );
}

#[test]
fn error_yield_mismatch_with_tail() {
    check_err(
        r#"fn main() { let x = { if true { yield "hello"; } 42 }; }"#,
        &["type mismatch"],
    );
}

// --- Nested Block Interactions (3 tests) ---

#[test]
fn yield_in_nested_block() {
    check(
        "fn main() { let outer = { let inner = { yield 42; }; inner + 1 }; }",
        "i32",
    );
}

#[test]
fn yield_does_not_exit_loop() {
    check(
        "fn main() { let x = loop { let y = { yield 1; }; break y; }; }",
        "i32",
    );
}

#[test]
fn yield_and_break_different_scopes() {
    check(
        "fn main() { let x = loop { let y = { yield 10; }; break y + 1; }; }",
        "i32",
    );
}
