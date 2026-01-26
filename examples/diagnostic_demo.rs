//! Demo of the diagnostic system.
//!
//! Run with: cargo run --example `diagnostic_demo`

use spl::diagnostic::{Diagnostic, DiagnosticRenderer, RenderConfig};

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

    let config = RenderConfig::new().with_file_name("main.spl");
    let renderer = DiagnosticRenderer::new(source, config);

    // Example 1: Simple error with note and hint
    let diag1 = Diagnostic::error("expected ';' after expression")
        .with_label(24..26, "add ';' after this")
        .with_note("statements must end with a semicolon")
        .with_hint("add ';' after '42'");

    println!("{}", renderer.render(&diag1));

    // Example 2: Error with multiple labels
    let diag2 = Diagnostic::error("expected expression after '+'")
        .with_label(43..44, "unexpected ';'")
        .with_secondary_label(41..42, "binary operator here")
        .with_note("binary operators require operands on both sides")
        .with_hint("add an expression between '+' and ';'");

    println!("{}", renderer.render(&diag2));

    // Example 3: Warning
    let diag3 = Diagnostic::warning("unused variable")
        .with_label(35..36, "variable 'y' is never read")
        .with_hint("prefix with '_' to suppress: '_y'");

    println!("{}", renderer.render(&diag3));

    // Example 4: Multiple notes and hints
    let diag4 = Diagnostic::error("cannot find function 'println' in this scope")
        .with_label(49..56, "not found in this scope")
        .with_note("there is no function named 'println' in the current scope")
        .with_note("the standard library provides 'print' instead")
        .with_hint("use 'print' or 'print_line' from std::io");

    println!("{}", renderer.render(&diag4));

    // Show plain (no colors) version
    println!("=== Without colors ===\n");

    let config_plain = RenderConfig::new()
        .with_file_name("main.spl")
        .with_colors(false);
    let renderer_plain = DiagnosticRenderer::new(source, config_plain);
    println!("{}", renderer_plain.render(&diag1));
}
