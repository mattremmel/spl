//! Demo of the diagnostic system.
//!
//! Run with: cargo run --example diagnostic_demo

use spl::diagnostic::{Diagnostic, DiagnosticRenderer, RenderConfig, SourceCode};

fn main() {
    // Source with intentional errors for demonstration
    // Line positions (0-indexed byte offsets):
    // Line 1: "fn main() {\n"           = 0..12
    // Line 2: "    let x = 42\n"        = 12..27  (42 at 24..26)
    // Line 3: "    let y = x + ;\n"     = 27..45  (; at 43..44, + at 41..42)
    // Line 4: "    println(y)\n"        = 45..60  (println at 49..56)
    // Line 5: "}"                       = 60..61
    let source = "fn main() {\n    let x = 42\n    let y = x + ;\n    println(y)\n}";

    println!("=== SPL Diagnostic System Demo ===\n");

    let source_code = SourceCode::new(source);

    // Example 1: Simple error with note and hint
    let diag1 = Diagnostic::error("expected ';' after expression")
        .with_label(24..26, "add ';' after this")
        .with_note("statements must end with a semicolon")
        .with_hint("add ';' after '42'");

    let config = RenderConfig::new().with_file_name("main.spl");
    let renderer = DiagnosticRenderer::new(&source_code, config);
    println!("{}", renderer.render(&diag1));

    // Example 2: Error with multiple labels
    let diag2 = Diagnostic::error("expected expression after '+'")
        .with_label(43..44, "unexpected ';'")
        .with_secondary_label(41..42, "binary operator here")
        .with_note("binary operators require operands on both sides")
        .with_hint("add an expression between '+' and ';'");

    let config2 = RenderConfig::new().with_file_name("main.spl");
    let renderer2 = DiagnosticRenderer::new(&source_code, config2);
    println!("{}", renderer2.render(&diag2));

    // Example 3: Warning
    let diag3 = Diagnostic::warning("unused variable")
        .with_label(35..36, "variable 'y' is never read")
        .with_hint("prefix with '_' to suppress: '_y'");

    let config3 = RenderConfig::new().with_file_name("main.spl");
    let renderer3 = DiagnosticRenderer::new(&source_code, config3);
    println!("{}", renderer3.render(&diag3));

    // Example 4: Multiple notes and hints
    let diag4 = Diagnostic::error("cannot find function 'println' in this scope")
        .with_label(49..56, "not found in this scope")
        .with_note("there is no function named 'println' in the current scope")
        .with_note("the standard library provides 'print' instead")
        .with_hint("use 'print' or 'print_line' from std::io");

    let config4 = RenderConfig::new().with_file_name("main.spl");
    let renderer4 = DiagnosticRenderer::new(&source_code, config4);
    println!("{}", renderer4.render(&diag4));

    // Show plain (no colors) version
    println!("=== Without colors ===\n");

    let config_plain = RenderConfig::new()
        .with_file_name("main.spl")
        .with_colors(false);
    let renderer_plain = DiagnosticRenderer::new(&source_code, config_plain);
    println!("{}", renderer_plain.render(&diag1));
}
