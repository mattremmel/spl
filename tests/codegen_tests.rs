//! Codegen correctness tests.
//!
//! These tests verify that the generated code executes correctly
//! by compiling and running full programs.

/// Compile source and execute it, returning the exit code.
fn execute(source: &str) -> i32 {
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let temp_dir = std::env::temp_dir();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let exe_name = format!("spl_codegen_test_{}_{}", std::process::id(), counter);
    let exe_path = temp_dir.join(exe_name);

    spl::compile_and_link(source, &exe_path).expect("compilation failed");

    let output = Command::new(&exe_path).output().expect("failed to execute");

    let _ = std::fs::remove_file(&exe_path);

    output.status.code().unwrap_or(-1)
}

/// Test that a program returns the expected value.
fn check_returns(source: &str, expected: i32) {
    let result = execute(source);
    assert_eq!(
        result, expected,
        "Expected return value {expected}, got {result}"
    );
}

// =============================================================================
// Comparison Operators
// =============================================================================

#[test]
fn comparison_less_than_i32() {
    check_returns("fn main(): i32 { if 1 < 2 { 1 } else { 0 } }", 1);
    check_returns("fn main(): i32 { if 2 < 1 { 1 } else { 0 } }", 0);
    check_returns("fn main(): i32 { if 1 < 1 { 1 } else { 0 } }", 0);
}

#[test]
fn comparison_less_equal_i32() {
    check_returns("fn main(): i32 { if 1 <= 2 { 1 } else { 0 } }", 1);
    check_returns("fn main(): i32 { if 2 <= 1 { 1 } else { 0 } }", 0);
    check_returns("fn main(): i32 { if 1 <= 1 { 1 } else { 0 } }", 1);
}

#[test]
fn comparison_greater_than_i32() {
    check_returns("fn main(): i32 { if 2 > 1 { 1 } else { 0 } }", 1);
    check_returns("fn main(): i32 { if 1 > 2 { 1 } else { 0 } }", 0);
    check_returns("fn main(): i32 { if 1 > 1 { 1 } else { 0 } }", 0);
}

#[test]
fn comparison_greater_equal_i32() {
    check_returns("fn main(): i32 { if 2 >= 1 { 1 } else { 0 } }", 1);
    check_returns("fn main(): i32 { if 1 >= 2 { 1 } else { 0 } }", 0);
    check_returns("fn main(): i32 { if 1 >= 1 { 1 } else { 0 } }", 1);
}

#[test]
fn comparison_equal_i32() {
    check_returns("fn main(): i32 { if 1 == 1 { 1 } else { 0 } }", 1);
    check_returns("fn main(): i32 { if 1 == 2 { 1 } else { 0 } }", 0);
}

#[test]
fn comparison_not_equal_i32() {
    check_returns("fn main(): i32 { if 1 != 2 { 1 } else { 0 } }", 1);
    check_returns("fn main(): i32 { if 1 != 1 { 1 } else { 0 } }", 0);
}

#[test]
fn comparison_i64() {
    check_returns(
        r#"
        fn main(): i32 {
            let a: i64 = 100;
            let b: i64 = 200;
            return if a < b { 1 } else { 0 };
        }
        "#,
        1,
    );
    check_returns(
        r#"
        fn main(): i32 {
            let a: i64 = 100;
            let b: i64 = 100;
            return if a == b { 1 } else { 0 };
        }
        "#,
        1,
    );
}

#[test]
fn comparison_bool() {
    check_returns("fn main(): i32 { if true == true { 1 } else { 0 } }", 1);
    check_returns("fn main(): i32 { if true == false { 1 } else { 0 } }", 0);
    check_returns("fn main(): i32 { if true != false { 1 } else { 0 } }", 1);
}

// =============================================================================
// Arithmetic Operations
// =============================================================================

#[test]
fn arithmetic_add() {
    check_returns("fn main(): i32 { 10 + 32 }", 42);
}

#[test]
fn arithmetic_subtract() {
    check_returns("fn main(): i32 { 50 - 8 }", 42);
}

#[test]
fn arithmetic_multiply() {
    check_returns("fn main(): i32 { 6 * 7 }", 42);
}

#[test]
fn arithmetic_divide() {
    check_returns("fn main(): i32 { 84 / 2 }", 42);
}

#[test]
fn arithmetic_modulo() {
    check_returns("fn main(): i32 { 47 % 5 }", 2);
}

#[test]
fn arithmetic_negation() {
    check_returns(
        r#"
        fn main(): i32 {
            let x = 42;
            return -x + 84;
        }
        "#,
        42,
    );
}

#[test]
fn arithmetic_complex_expression() {
    check_returns("fn main(): i32 { (10 + 5) * 2 + 12 }", 42);
}

// =============================================================================
// Integer Type Boundaries
// =============================================================================

#[test]
fn i32_max_value() {
    check_returns(
        r#"
        fn main(): i32 {
            let x: i32 = 2147483647;
            return if x > 0 { 1 } else { 0 };
        }
        "#,
        1,
    );
}

#[test]
fn i32_min_value() {
    check_returns(
        r#"
        fn main(): i32 {
            let x: i32 = -2147483648;
            return if x < 0 { 1 } else { 0 };
        }
        "#,
        1,
    );
}

#[test]
fn i64_large_values() {
    // Use values that don't fit in i32
    check_returns(
        r#"
        fn main(): i32 {
            let big: i64 = 9000000000;
            return if big > 8000000000 { 1 } else { 0 };
        }
        "#,
        1,
    );
}

// =============================================================================
// Control Flow
// =============================================================================

#[test]
fn if_else_basic() {
    check_returns("fn main(): i32 { if true { 42 } else { 0 } }", 42);
    check_returns("fn main(): i32 { if false { 0 } else { 42 } }", 42);
}

#[test]
fn if_else_nested() {
    check_returns(
        r#"
        fn main(): i32 {
            if true {
                if false { 0 } else { 42 }
            } else {
                0
            }
        }
        "#,
        42,
    );
}

#[test]
fn while_loop_basic() {
    check_returns(
        r#"
        fn main(): i32 {
            let mut x = 0;
            while x < 42 {
                x = x + 1;
            }
            return x;
        }
        "#,
        42,
    );
}

#[test]
fn while_loop_break() {
    check_returns(
        r#"
        fn main(): i32 {
            let mut x = 0;
            while true {
                x = x + 1;
                if x == 42 { break; }
            }
            return x;
        }
        "#,
        42,
    );
}

#[test]
fn while_loop_continue() {
    check_returns(
        r#"
        fn main(): i32 {
            let mut x = 0;
            let mut sum = 0;
            while x < 10 {
                x = x + 1;
                if x % 2 == 0 { continue; }
                sum = sum + x;
            }
            return sum;
        }
        "#,
        25, // 1 + 3 + 5 + 7 + 9 = 25
    );
}

#[test]
fn loop_with_break_value() {
    check_returns(
        r#"
        fn main(): i32 {
            let result = loop {
                break 42;
            };
            return result;
        }
        "#,
        42,
    );
}

#[test]
fn for_loop_basic() {
    check_returns(
        r#"
        fn main(): i32 {
            let mut sum = 0;
            for i in 0..10 {
                sum = sum + i;
            }
            return sum;
        }
        "#,
        45, // 0 + 1 + 2 + ... + 9 = 45
    );
}

// =============================================================================
// Functions
// =============================================================================

#[test]
fn function_call_simple() {
    check_returns(
        r#"
        fn answer(): i32 { 42 }
        fn main(): i32 { answer() }
        "#,
        42,
    );
}

#[test]
fn function_call_with_args() {
    check_returns(
        r#"
        fn add(_ a: i32, _ b: i32): i32 { a + b }
        fn main(): i32 { add(40, 2) }
        "#,
        42,
    );
}

#[test]
fn function_recursive() {
    check_returns(
        r#"
        fn fib(_ n: i32): i32 {
            if n <= 1 { n }
            else { fib(n - 1) + fib(n - 2) }
        }
        fn main(): i32 { fib(10) }
        "#,
        55, // fib(10) = 55
    );
}

#[test]
fn function_mutually_recursive() {
    check_returns(
        r#"
        fn is_even(_ n: i32): i32 { if n == 0 { 1 } else { is_odd(n - 1) } }
        fn is_odd(_ n: i32): i32 { if n == 0 { 0 } else { is_even(n - 1) } }
        fn main(): i32 { is_even(42) }
        "#,
        1, // 42 is even
    );
}

// =============================================================================
// Arrays
// =============================================================================

#[test]
fn array_indexing() {
    check_returns(
        r#"
        fn main(): i32 {
            let arr = [10, 20, 30, 42, 50];
            return arr[3];
        }
        "#,
        42,
    );
}

#[test]
fn array_mutation() {
    check_returns(
        r#"
        fn main(): i32 {
            let mut arr = [0, 0, 0];
            arr[1] = 42;
            return arr[1];
        }
        "#,
        42,
    );
}

#[test]
fn array_in_loop() {
    check_returns(
        r#"
        fn main(): i32 {
            let arr = [1, 2, 3, 4, 5];
            let mut sum = 0;
            let mut i = 0;
            while i < 5 {
                sum = sum + arr[i];
                i = i + 1;
            }
            return sum;
        }
        "#,
        15, // 1 + 2 + 3 + 4 + 5 = 15
    );
}

// =============================================================================
// Structs
// =============================================================================

#[test]
fn struct_creation_and_field_access() {
    check_returns(
        r#"
        struct Point(x: i32, y: i32)
        fn main(): i32 {
            let p = Point(x: 10, y: 32);
            return p.x + p.y;
        }
        "#,
        42,
    );
}

#[test]
fn struct_field_mutation() {
    check_returns(
        r#"
        struct Counter(value: i32)
        fn main(): i32 {
            let mut c = Counter(value: 0);
            c.value = 42;
            return c.value;
        }
        "#,
        42,
    );
}

// =============================================================================
// Match Expressions
// =============================================================================

#[test]
fn match_basic() {
    check_returns(
        r#"
        fn main(): i32 {
            let x = 2;
            return match x {
                0 => 0,
                1 => 10,
                2 => 42,
                _ => 100,
            };
        }
        "#,
        42,
    );
}

#[test]
fn match_wildcard() {
    check_returns(
        r#"
        fn main(): i32 {
            let x = 999;
            return match x {
                0 => 0,
                1 => 1,
                _ => 42,
            };
        }
        "#,
        42,
    );
}

// =============================================================================
// Tuples
// =============================================================================

#[test]
fn tuple_creation_and_access() {
    check_returns(
        r#"
        fn main(): i32 {
            let pair = (10, 32);
            return pair.0 + pair.1;
        }
        "#,
        42,
    );
}

#[test]
fn tuple_destructuring() {
    check_returns(
        r#"
        fn main(): i32 {
            let pair = (40, 2);
            let (a, b) = pair;
            return a + b;
        }
        "#,
        42,
    );
}

// =============================================================================
// Type Casting
// =============================================================================

#[test]
fn cast_i32_to_i64() {
    check_returns(
        r#"
        fn takes_i64(_ x: i64): i32 { if x > 0 { 42 } else { 0 } }
        fn main(): i32 {
            let x: i32 = 100;
            return takes_i64(x as i64);
        }
        "#,
        42,
    );
}

#[test]
fn cast_i64_to_i32() {
    check_returns(
        r#"
        fn main(): i32 {
            let x: i64 = 42;
            return x as i32;
        }
        "#,
        42,
    );
}

// =============================================================================
// Logical Operations
// =============================================================================

#[test]
fn logical_and() {
    check_returns("fn main(): i32 { if true && true { 42 } else { 0 } }", 42);
    check_returns("fn main(): i32 { if true && false { 42 } else { 0 } }", 0);
    check_returns("fn main(): i32 { if false && true { 42 } else { 0 } }", 0);
}

#[test]
fn logical_or() {
    check_returns("fn main(): i32 { if false || true { 42 } else { 0 } }", 42);
    check_returns("fn main(): i32 { if true || false { 42 } else { 0 } }", 42);
    check_returns("fn main(): i32 { if false || false { 42 } else { 0 } }", 0);
}

#[test]
fn logical_not() {
    check_returns("fn main(): i32 { if !false { 42 } else { 0 } }", 42);
    check_returns("fn main(): i32 { if !true { 42 } else { 0 } }", 0);
}

#[test]
#[ignore = "block expressions in short-circuit not yet working"]
fn logical_short_circuit_and() {
    // The second expression should not be evaluated if first is false
    check_returns(
        r#"
        fn main(): i32 {
            let mut x = 0;
            if false && { x = 1; true } {
                return 0;
            } else {
                return x;
            }
        }
        "#,
        0, // x should still be 0 because second expr wasn't evaluated
    );
}

#[test]
#[ignore = "block expressions in short-circuit not yet working"]
fn logical_short_circuit_or() {
    // The second expression should not be evaluated if first is true
    check_returns(
        r#"
        fn main(): i32 {
            let mut x = 0;
            if true || { x = 1; false } {
                return x;
            } else {
                return 100;
            }
        }
        "#,
        0, // x should still be 0 because second expr wasn't evaluated
    );
}
