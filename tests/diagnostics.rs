//! Error message quality tests using snapshot testing.
//!
//! These tests verify that diagnostic messages are clear, accurate,
//! and provide useful information to users.

#[allow(unused_imports)]
use expect_test::{Expect, expect};
use spl::testing::{compile_err, format_diagnostics};

/// Check that the diagnostics for a source contain the expected patterns.
#[allow(dead_code)]
fn check_diagnostic(source: &str, expected: &Expect) {
    let diagnostics = compile_err(source);
    let formatted = format_diagnostics(&diagnostics);
    expected.assert_eq(&formatted);
}

/// Check that the diagnostics contain a specific substring.
fn check_contains(source: &str, pattern: &str) {
    let diagnostics = compile_err(source);
    let formatted = format_diagnostics(&diagnostics);
    assert!(
        formatted.contains(pattern),
        "Expected diagnostic to contain '{pattern}', got:\n{formatted}"
    );
}

// =============================================================================
// Type Mismatch Errors
// =============================================================================

#[test]
fn type_mismatch_bool_to_i32() {
    check_contains("fn main() { let x: i32 = true; }", "type mismatch");
}

#[test]
fn type_mismatch_i32_to_bool() {
    check_contains("fn main() { let x: bool = 42; }", "type mismatch");
}

#[test]
fn type_mismatch_string_to_i32() {
    check_contains(r#"fn main() { let x: i32 = "hello"; }"#, "type mismatch");
}

#[test]
fn type_mismatch_in_return() {
    check_contains("fn foo(): i32 { return true; }", "type mismatch");
}

#[test]
fn break_type_mismatch_shows_types() {
    // Multiple breaks with conflicting types should show expected/actual
    check_contains(
        "fn main() { let x = loop { if true { break 42; } break true; }; }",
        "expected `i32`, found `bool`",
    );
}

#[test]
fn return_type_mismatch_shows_types() {
    check_contains(
        "fn foo(): i32 { return true; }",
        "expected `i32`, found `bool`",
    );
}

#[test]
fn type_mismatch_function_arg() {
    check_contains(
        "fn foo(_ x: i32) {} fn main() { foo(true); }",
        "type mismatch",
    );
}

// =============================================================================
// Undefined Variable Errors
// =============================================================================

#[test]
fn undefined_variable_simple() {
    check_contains("fn main() { x; }", "cannot find");
}

#[test]
fn undefined_variable_in_expression() {
    check_contains("fn main() { let y = x + 1; }", "cannot find");
}

#[test]
fn undefined_function() {
    check_contains("fn main() { foo(); }", "cannot find");
}

#[test]
fn undefined_type() {
    check_contains("fn main() { let x: Foo = 1; }", "cannot find");
}

#[test]
fn undefined_struct() {
    check_contains("fn main() { let x = Foo(a: 1); }", "cannot find");
}

// =============================================================================
// Multiple Errors
// =============================================================================

#[test]
fn multiple_undefined_variables() {
    let diagnostics = compile_err("fn main() { a; b; c; }");
    assert_eq!(
        diagnostics.len(),
        3,
        "Expected 3 errors, got: {}",
        format_diagnostics(&diagnostics)
    );
}

#[test]
fn multiple_type_errors() {
    let diagnostics = compile_err(
        r#"
        fn take_i32(_ x: i32) {}
        fn take_bool(_ x: bool) {}
        fn main() {
            take_i32(true);
            take_bool(42);
        }
        "#,
    );
    assert_eq!(
        diagnostics.len(),
        2,
        "Expected 2 errors, got: {}",
        format_diagnostics(&diagnostics)
    );
}

// =============================================================================
// Invalid Operations
// =============================================================================

#[test]
fn invalid_binary_op_types() {
    check_contains(r#"fn main() { let x = "hello" + 1; }"#, "cannot apply");
}

#[test]
fn invalid_unary_negation() {
    check_contains("fn main() { let x = -true; }", "cannot apply");
}

#[test]
fn invalid_comparison() {
    check_contains(r#"fn main() { let x = "hello" < 1; }"#, "type mismatch");
}

// =============================================================================
// Invalid Casts
// =============================================================================

#[test]
fn invalid_cast_unit_to_int() {
    check_contains("fn main() { let x = () as i32; }", "invalid cast");
}

#[test]
fn invalid_cast_bool_to_float() {
    check_contains("fn main() { let x = true as f64; }", "invalid cast");
}

// =============================================================================
// Array Errors
// =============================================================================

#[test]
fn array_index_out_of_bounds() {
    check_contains(
        "fn main() { let a = [1, 2, 3]; let x = a[10]; }",
        "out of bounds",
    );
}

#[test]
fn array_mismatched_element_types() {
    check_contains(
        "fn main() { let a: [i32; 2] = [1, true]; }",
        "type mismatch",
    );
}

// =============================================================================
// Integer Range Errors
// =============================================================================

#[test]
fn integer_overflow_i32() {
    check_contains("fn main() { let x: i32 = 2147483648; }", "out of range");
}

#[test]
fn integer_underflow_i32() {
    check_contains("fn main() { let x: i32 = -2147483649; }", "out of range");
}

// =============================================================================
// Struct Errors
// =============================================================================

#[test]
fn struct_missing_field() {
    check_contains(
        "struct Point(x: i32, y: i32) fn main() { let p = Point(x: 1); }",
        "missing",
    );
}

#[test]
fn struct_unknown_field() {
    check_contains(
        "struct Point(x: i32, y: i32) fn main() { let p = Point(x: 1, y: 2, z: 3); }",
        "unknown field",
    );
}

#[test]
fn struct_duplicate_field() {
    check_contains(
        "struct Point(x: i32, y: i32) fn main() { let p = Point(x: 1, x: 2, y: 3); }",
        "duplicate",
    );
}

// =============================================================================
// Mutability Errors
// =============================================================================

#[test]
fn assign_to_immutable() {
    check_contains("fn main() { let x = 1; x = 2; }", "immutable");
}

// =============================================================================
// Return Type Errors
// =============================================================================

#[test]
fn missing_return_value() {
    check_contains("fn foo(): i32 { }", "not all code paths return");
}

#[test]
fn return_in_non_returning_function() {
    // Returning a value in a void function
    check_contains("fn foo() { return 42; }", "type mismatch");
}

// =============================================================================
// Cyclic and Recursive Type Errors
// =============================================================================

#[test]
fn recursive_struct_error() {
    check_contains("struct Node(next: Node)", "recursive");
}

#[test]
fn cyclic_type_alias_error() {
    check_contains("type A = B; type B = A;", "cyclic");
}

// =============================================================================
// Range Expression Type Inference
// =============================================================================

#[test]
fn range_type_mismatch() {
    check_contains("fn main() { let r = 0..true; }", "type mismatch");
}

#[test]
fn for_loop_variable_type_from_range() {
    // Loop var should be i32 from range, not compatible with bool
    check_contains(
        r#"fn main() {
            for i in 0..10 {
                let x: bool = i;
            }
        }"#,
        "type mismatch",
    );
}

#[test]
fn range_from_type_inference() {
    // Range from (1..) should infer element type from start
    check_contains(
        r#"fn main() {
            for i in (1..) {
                let x: bool = i;
                break;
            }
        }"#,
        "type mismatch",
    );
}

#[test]
fn range_to_type_inference() {
    // Range to (..10) should infer element type from end
    check_contains(
        r#"fn main() {
            for i in ..10 {
                let x: bool = i;
                break;
            }
        }"#,
        "type mismatch",
    );
}

#[test]
fn open_range_needs_context() {
    // Open range (..) has no type info - should error or infer from context
    check_contains("fn main() { let r = ..; }", "cannot infer");
}
