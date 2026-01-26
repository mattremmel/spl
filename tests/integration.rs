//! Integration tests for the SPL compiler.
//!
//! These tests verify the full compilation pipeline from source to MIR.

use spl::testing::{
    assert_error_count, assert_has_error, compile_err, compile_ok, format_diagnostics,
};

// === Successful compilation ===

#[test]
fn empty_main_compiles() {
    let bodies = compile_ok("fn main() {}");
    assert_eq!(bodies.len(), 1);
}

#[test]
fn multiple_functions_compile() {
    let bodies = compile_ok("fn foo() {} fn bar() {} fn main() {}");
    assert_eq!(bodies.len(), 3);
}

#[test]
fn function_with_arithmetic() {
    let bodies = compile_ok("fn main() { let x = 1 + 2 * 3; }");
    assert_eq!(bodies.len(), 1);
}

// === Error cases ===

#[test]
fn undefined_variable_error() {
    let diags = compile_err("fn main() { x; }");
    assert_has_error(&diags, "cannot find");
}

#[test]
fn multiple_undefined_variables() {
    let diags = compile_err("fn main() { x; y; z; }");
    assert_error_count(&diags, 3);
}

#[test]
fn format_diagnostics_works() {
    let diags = compile_err("fn main() { undefined; }");
    let formatted = format_diagnostics(&diags);
    assert!(formatted.contains("[error]"));
    assert!(formatted.contains("cannot find"));
}

#[test]
fn range_type_inference_positive() {
    compile_ok(
        r#"
        fn main() {
            for i in 0..10 {
                let x: i32 = i;
            }
        }
    "#,
    );
}
