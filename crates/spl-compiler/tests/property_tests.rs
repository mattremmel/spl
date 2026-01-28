//! Property-based tests using proptest.
//!
//! These tests verify invariants that should hold for all inputs,
//! using randomly generated test cases to find edge cases.

use proptest::prelude::*;
use spl_compiler::parser::parse;
#[allow(unused_imports)]
use spl_compiler::testing::{compile_ok, parse_ok};

// =============================================================================
// Generators for SPL Source Code
// =============================================================================

/// Reserved words that cannot be used as identifiers.
/// Includes keywords and built-in type names.
const RESERVED_WORDS: &[&str] = &[
    // Keywords
    "fn", "let", "mut", "if", "else", "while", "for", "in", "loop", "break", "continue", "return",
    "true", "false", "struct", "impl", "type", "match", "pub", "use", "mod", "as", "is", "where",
    "self", "Self", "super", "crate", "const", "static", "ref", "move", "yield",
    // Built-in types
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32",
    "f64", "bool", "char", "str", "String", "never",
];

/// Generate a valid SPL identifier.
fn valid_ident() -> impl Strategy<Value = String> {
    // Start with a letter, followed by letters/digits/underscores
    prop::string::string_regex("[a-z][a-z0-9_]{0,10}")
        .unwrap()
        .prop_filter("not a reserved word", |s| {
            !RESERVED_WORDS.contains(&s.as_str())
        })
}

/// Generate a simple type name.
fn simple_type() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("i32".to_string()),
        Just("i64".to_string()),
        Just("bool".to_string()),
        Just("f64".to_string()),
        Just("()".to_string()),
    ]
}

/// Generate a small integer literal.
fn int_literal() -> impl Strategy<Value = String> {
    (-1000i32..1000).prop_map(|n| n.to_string())
}

/// Generate a simple expression.
fn simple_expr() -> impl Strategy<Value = String> {
    prop_oneof![
        int_literal(),
        Just("true".to_string()),
        Just("false".to_string()),
    ]
}

/// Generate a binary operator.
fn binary_op() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("+".to_string()),
        Just("-".to_string()),
        Just("*".to_string()),
        Just("/".to_string()),
        Just("==".to_string()),
        Just("!=".to_string()),
        Just("<".to_string()),
        Just("<=".to_string()),
        Just(">".to_string()),
        Just(">=".to_string()),
    ]
}

/// Generate a binary expression with integer operands.
fn binary_int_expr() -> impl Strategy<Value = String> {
    (int_literal(), binary_op(), int_literal()).prop_map(|(l, op, r)| format!("{l} {op} {r}"))
}

/// Generate a simple let statement.
#[allow(dead_code)]
fn let_stmt() -> impl Strategy<Value = String> {
    (valid_ident(), simple_expr()).prop_map(|(name, expr)| format!("let {name} = {expr};"))
}

/// Generate a simple function with a body.
fn simple_function() -> impl Strategy<Value = String> {
    (valid_ident(), simple_expr()).prop_map(|(name, expr)| format!("fn {name}() {{ {expr}; }}"))
}

/// Generate a function with parameters.
fn function_with_params() -> impl Strategy<Value = String> {
    (valid_ident(), valid_ident(), simple_type())
        .prop_map(|(fname, pname, ptype)| format!("fn {fname}(_ {pname}: {ptype}) {{ }}"))
}

/// Generate a function with return type.
#[allow(dead_code)]
fn function_with_return() -> impl Strategy<Value = String> {
    (valid_ident(), simple_type(), int_literal()).prop_map(|(name, ret_type, value)| {
        // Only return int literals for i32/i64, otherwise use appropriate values
        let body = match ret_type.as_str() {
            "bool" => "true".to_string(),
            "f64" => "1.0".to_string(),
            "()" => String::new(),
            _ => value, // i32, i64, and fallback
        };
        format!("fn {name}(): {ret_type} {{ {body} }}")
    })
}

/// Generate a simple struct definition.
fn simple_struct() -> impl Strategy<Value = String> {
    (valid_ident(), valid_ident(), simple_type())
        .prop_map(|(sname, fname, ftype)| format!("struct {sname}({fname}: {ftype})"))
}

// =============================================================================
// Parse Stability Tests
// =============================================================================

proptest! {
    /// Parsing the same source twice should produce equivalent results.
    #[test]
    fn parse_is_deterministic(source in simple_function()) {
        let parse1 = parse(&source);
        let parse2 = parse(&source);

        // Both should have the same error status
        prop_assert_eq!(parse1.ok(), parse2.ok());

        // Both should have the same number of errors
        prop_assert_eq!(parse1.errors().len(), parse2.errors().len());

        // Both should produce the same debug tree
        prop_assert_eq!(parse1.debug_tree(), parse2.debug_tree());
    }

    /// Parse errors should not change between runs.
    #[test]
    fn parse_errors_stable(source in simple_function()) {
        let parse1 = parse(&source);
        let parse2 = parse(&source);

        // Error messages should be identical
        let errors1: Vec<_> = parse1.errors().iter().map(|e| &e.message).collect();
        let errors2: Vec<_> = parse2.errors().iter().map(|e| &e.message).collect();
        prop_assert_eq!(errors1, errors2);
    }
}

// =============================================================================
// Well-Formed Source Tests
// =============================================================================

proptest! {
    /// Simple functions should parse without errors.
    #[test]
    fn simple_functions_parse(source in simple_function()) {
        let parse_result = parse(&source);
        prop_assert!(
            parse_result.ok(),
            "Failed to parse: {}\nErrors: {:?}",
            source,
            parse_result.errors()
        );
    }

    /// Functions with parameters should parse without errors.
    #[test]
    fn param_functions_parse(source in function_with_params()) {
        let parse_result = parse(&source);
        prop_assert!(
            parse_result.ok(),
            "Failed to parse: {}\nErrors: {:?}",
            source,
            parse_result.errors()
        );
    }

    /// Simple structs should parse without errors.
    #[test]
    fn simple_structs_parse(source in simple_struct()) {
        let parse_result = parse(&source);
        prop_assert!(
            parse_result.ok(),
            "Failed to parse: {}\nErrors: {:?}",
            source,
            parse_result.errors()
        );
    }

    /// Integer literals in range should parse correctly.
    #[test]
    fn int_literals_parse(n in -1_000_000i32..1_000_000) {
        let source = format!("fn main() {{ let x = {n}; }}");
        let parse_result = parse(&source);
        prop_assert!(
            parse_result.ok(),
            "Failed to parse int literal {}: {:?}",
            n,
            parse_result.errors()
        );
    }

    /// Binary expressions should parse without errors.
    #[test]
    fn binary_exprs_parse(expr in binary_int_expr()) {
        let source = format!("fn main() {{ let x = {expr}; }}");
        let parse_result = parse(&source);
        prop_assert!(
            parse_result.ok(),
            "Failed to parse binary expr: {}\nErrors: {:?}",
            expr,
            parse_result.errors()
        );
    }
}

// =============================================================================
// Type Inference Stability Tests
// =============================================================================

proptest! {
    /// Type inference should be deterministic.
    #[test]
    fn type_inference_deterministic(n in 0i32..1000) {
        let source = format!("fn main() {{ let x: i32 = {n}; }}");

        // Run type inference twice
        let result1 = spl_compiler::compile(&source);
        let result2 = spl_compiler::compile(&source);

        // Both should have same success/failure status
        prop_assert_eq!(result1.is_ok(), result2.is_ok());

        // Both should have same number of diagnostics
        prop_assert_eq!(result1.diagnostics.len(), result2.diagnostics.len());
    }
}

// =============================================================================
// AST Structure Tests
// =============================================================================

proptest! {
    /// Parser should always produce a SourceFile root.
    #[test]
    fn parser_produces_source_file(source in simple_function()) {
        let parse_result = parse(&source);
        let tree = parse_result.debug_tree();
        prop_assert!(
            tree.starts_with("SourceFile"),
            "Tree should start with SourceFile, got: {}",
            tree.lines().next().unwrap_or("")
        );
    }

    /// Functions should produce FunctionDef nodes.
    #[test]
    fn functions_produce_function_def(source in simple_function()) {
        let parse_result = parse(&source);
        let tree = parse_result.debug_tree();
        prop_assert!(
            tree.contains("FunctionDef"),
            "Tree should contain FunctionDef: {}",
            tree
        );
    }

    /// Structs should produce StructDef nodes.
    #[test]
    fn structs_produce_struct_def(source in simple_struct()) {
        let parse_result = parse(&source);
        let tree = parse_result.debug_tree();
        prop_assert!(
            tree.contains("StructDef"),
            "Tree should contain StructDef: {}",
            tree
        );
    }
}

// =============================================================================
// Compilation Stability Tests
// =============================================================================

proptest! {
    /// Compiling the same source twice should produce same results.
    #[test]
    fn compilation_is_deterministic(n in 0i32..100) {
        let source = format!("fn main(): i32 {{ {n} }}");

        let result1 = spl_compiler::compile(&source);
        let result2 = spl_compiler::compile(&source);

        // Both should succeed or both should fail
        prop_assert_eq!(result1.is_ok(), result2.is_ok());

        // If both succeed, both should have bodies
        if result1.is_ok() && result2.is_ok() {
            prop_assert!(result1.bodies.is_some());
            prop_assert!(result2.bodies.is_some());
        }
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

proptest! {
    /// Empty function bodies should compile.
    #[test]
    fn empty_functions_compile(name in valid_ident()) {
        let source = format!("fn {name}() {{}}");
        let result = spl_compiler::compile(&source);
        prop_assert!(
            result.is_ok(),
            "Empty function failed to compile: {:?}",
            result.diagnostics
        );
    }

    /// Deeply nested expressions should parse.
    #[test]
    fn nested_parens_parse(depth in 1usize..20) {
        let opens = "(".repeat(depth);
        let closes = ")".repeat(depth);
        let source = format!("fn main() {{ let x = {opens}42{closes}; }}");
        let parse_result = parse(&source);
        prop_assert!(
            parse_result.ok(),
            "Failed to parse nested parens (depth {}): {:?}",
            depth,
            parse_result.errors()
        );
    }

    /// Chained binary operations should parse.
    #[test]
    fn chained_ops_parse(count in 1usize..20) {
        let expr = "1 +".repeat(count) + " 1";
        let source = format!("fn main() {{ let x = {expr}; }}");
        let parse_result = parse(&source);
        prop_assert!(
            parse_result.ok(),
            "Failed to parse chained ops (count {}): {:?}",
            count,
            parse_result.errors()
        );
    }
}
