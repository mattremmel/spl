//! Test utilities for the SPL compiler.
//!
//! This module provides ergonomic helpers for testing compilation results,
//! including compile result assertions and diagnostic matchers.
//!
//! # Example
//!
//! ```
//! use spl::testing::{compile_ok, compile_err, assert_has_error};
//!
//! // Test successful compilation
//! let bodies = compile_ok("fn main() {}");
//! assert_eq!(bodies.len(), 1);
//!
//! // Test compilation failure
//! let diags = compile_err("fn main() { x; }");
//! assert_has_error(&diags, "cannot find");
//! ```

use crate::{mir, Diagnostic, Severity};

/// Compile source code and return MIR bodies, panicking on error.
///
/// Use this when testing code that should compile successfully.
///
/// # Panics
///
/// Panics if compilation fails, showing all diagnostics.
///
/// # Example
///
/// ```
/// use spl::testing::compile_ok;
///
/// let bodies = compile_ok("fn main() {}");
/// assert_eq!(bodies.len(), 1);
/// ```
pub fn compile_ok(source: &str) -> Vec<mir::Body> {
    let result = crate::compile(source);
    if let Some(bodies) = result.bodies {
        bodies
    } else {
        panic!(
            "compilation failed:\n{}",
            format_diagnostics(&result.diagnostics)
        );
    }
}

/// Compile source code and return diagnostics, panicking on success.
///
/// Use this when testing code that should produce errors.
///
/// # Panics
///
/// Panics if compilation succeeds (no errors).
///
/// # Example
///
/// ```
/// use spl::testing::compile_err;
///
/// let diags = compile_err("fn main() { x; }");
/// assert!(!diags.is_empty());
/// ```
pub fn compile_err(source: &str) -> Vec<Diagnostic> {
    let result = crate::compile(source);
    if result.is_err() {
        result.diagnostics
    } else {
        panic!("expected errors but compilation succeeded");
    }
}

/// Format diagnostics for display in test output.
///
/// Returns a string with each diagnostic on its own line,
/// formatted as `[severity] message`.
///
/// # Example
///
/// ```
/// use spl::testing::{compile_err, format_diagnostics};
///
/// let diags = compile_err("fn main() { x; }");
/// let formatted = format_diagnostics(&diags);
/// println!("{}", formatted);
/// ```
pub fn format_diagnostics(diags: &[Diagnostic]) -> String {
    diags
        .iter()
        .map(|d| format!("[{}] {}", d.severity.as_str(), d.message))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Assert that at least one diagnostic contains the given pattern.
///
/// # Panics
///
/// Panics if no diagnostic message contains the pattern.
///
/// # Example
///
/// ```
/// use spl::testing::{compile_err, assert_has_error};
///
/// let diags = compile_err("fn main() { x; }");
/// assert_has_error(&diags, "cannot find");
/// ```
pub fn assert_has_error(diags: &[Diagnostic], pattern: &str) {
    if !diags.iter().any(|d| d.message.contains(pattern)) {
        panic!(
            "no diagnostic matching '{}'\nActual diagnostics:\n{}",
            pattern,
            format_diagnostics(diags)
        );
    }
}

/// Assert that exactly `expected` error diagnostics exist.
///
/// Only counts diagnostics with `Severity::Error`.
///
/// # Panics
///
/// Panics if the error count doesn't match.
///
/// # Example
///
/// ```
/// use spl::testing::{compile_err, assert_error_count};
///
/// let diags = compile_err("fn main() { x; y; }");
/// assert_error_count(&diags, 2);
/// ```
pub fn assert_error_count(diags: &[Diagnostic], expected: usize) {
    let actual = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    if actual != expected {
        panic!(
            "expected {} errors, found {}\nDiagnostics:\n{}",
            expected,
            actual,
            format_diagnostics(diags)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === TDD Cycle 1: compile_ok() ===

    #[test]
    fn compile_ok_returns_bodies() {
        let bodies = compile_ok("fn main() {}");
        assert_eq!(bodies.len(), 1);
    }

    #[test]
    #[should_panic(expected = "compilation failed")]
    fn compile_ok_panics_on_error() {
        compile_ok("fn main() { undefined; }");
    }

    // === TDD Cycle 2: compile_err() ===

    #[test]
    fn compile_err_returns_diagnostics() {
        let diags = compile_err("fn main() { x; }");
        assert!(!diags.is_empty());
    }

    #[test]
    #[should_panic(expected = "expected errors")]
    fn compile_err_panics_on_success() {
        compile_err("fn main() {}");
    }

    // === TDD Cycle 3: format_diagnostics() ===

    #[test]
    fn format_diagnostics_shows_messages() {
        let diags = compile_err("fn main() { x; }");
        let formatted = format_diagnostics(&diags);
        assert!(formatted.contains("cannot find"));
    }

    #[test]
    fn format_diagnostics_includes_severity() {
        let diags = compile_err("fn main() { x; }");
        let formatted = format_diagnostics(&diags);
        assert!(formatted.contains("[error]"));
    }

    // === TDD Cycle 4: assert_has_error() ===

    #[test]
    fn assert_has_error_passes_on_match() {
        let diags = compile_err("fn main() { x; }");
        assert_has_error(&diags, "cannot find");
    }

    #[test]
    #[should_panic(expected = "no diagnostic matching")]
    fn assert_has_error_panics_on_no_match() {
        let diags = compile_err("fn main() { x; }");
        assert_has_error(&diags, "type mismatch xyz");
    }

    // === TDD Cycle 5: assert_error_count() ===

    #[test]
    fn assert_error_count_passes_on_match() {
        let diags = compile_err("fn main() { x; y; }");
        assert_error_count(&diags, 2);
    }

    #[test]
    #[should_panic(expected = "expected 1 errors, found 2")]
    fn assert_error_count_panics_on_mismatch() {
        let diags = compile_err("fn main() { x; y; }");
        assert_error_count(&diags, 1);
    }
}
