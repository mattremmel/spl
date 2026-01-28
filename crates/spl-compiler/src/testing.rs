//! Test utilities for the SPL compiler.
//!
//! This module provides ergonomic helpers for testing compilation results,
//! including compile result assertions and diagnostic matchers.
//!
//! # Example
//!
//! ```
//! use spl_compiler::testing::{compile_ok, compile_err, assert_has_error};
//!
//! // Test successful compilation
//! let bodies = compile_ok("fn main() {}");
//! assert_eq!(bodies.len(), 1);
//!
//! // Test compilation failure
//! let diags = compile_err("fn main() { x; }");
//! assert_has_error(&diags, "cannot find");
//! ```

use crate::ast::SourceFile;
use crate::lexer::Span;
use crate::mir::{BasicBlockData, Body, Constant, Operand, Rvalue, StatementKind, TerminatorKind};
use crate::parser::ParseError;
use crate::sema::{InferResult, ResolveResult};
use crate::session::CompileSession;
use crate::{Diagnostic, Severity, mir};
use rowan::ast::AstNode;

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
/// use spl_compiler::testing::compile_ok;
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
/// use spl_compiler::testing::compile_err;
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

/// Create a compile session for the given source code.
///
/// The session provides lazy, cached access to each compilation phase.
/// Use this when you need fine-grained control over the compilation pipeline
/// or want to access intermediate results.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::session;
///
/// let mut sess = session("fn main() { let x = 1; }");
///
/// // Access phases lazily
/// let ast = sess.ast().unwrap();
/// let infer = sess.infer().unwrap();
///
/// // Diagnostics accumulated across all accessed phases
/// assert!(!sess.has_errors());
/// ```
pub fn session(source: &str) -> CompileSession<'_> {
    CompileSession::new(source)
}

// ============================================================================
// Span Utilities
// ============================================================================

/// Extract the source text for a span.
///
/// Returns the substring of `source` covered by `span`.
///
/// # Panics
///
/// Panics if the span is out of bounds for the source string.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::span_to_source;
///
/// let source = "let x = 42;";
/// let span = 4..5; // 'x'
/// assert_eq!(span_to_source(source, &span), "x");
/// ```
pub fn span_to_source<'a>(source: &'a str, span: &Span) -> &'a str {
    &source[span.clone()]
}

/// Assert a span covers expected source text.
///
/// # Panics
///
/// Panics if the span doesn't cover the expected text.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::assert_span_text;
///
/// let source = "let x = 42;";
/// let span = 4..5;
/// assert_span_text(source, &span, "x");
/// ```
pub fn assert_span_text(source: &str, span: &Span, expected: &str) {
    let actual = span_to_source(source, span);
    assert_eq!(
        actual, expected,
        "Span {}..{} should be '{}' but was '{}'",
        span.start, span.end, expected, actual
    );
}

// ============================================================================
// Diagnostic Formatting
// ============================================================================

/// Format diagnostics for display in test output.
///
/// Returns a string with each diagnostic on its own line,
/// formatted as `[severity] message`.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::{compile_err, format_diagnostics};
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

/// Format parse errors for display in test output.
pub fn format_parse_errors(errors: &[ParseError]) -> String {
    errors
        .iter()
        .map(|e| format!("[parse error] {}", e.message))
        .collect::<Vec<_>>()
        .join("\n")
}

// ============================================================================
// Phase-Specific Helpers
// ============================================================================

/// Parse source code and return the AST, panicking on error.
///
/// Use this when testing code that should parse successfully.
///
/// # Panics
///
/// Panics if parsing fails, showing all parse errors.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::parse_ok;
///
/// let ast = parse_ok("fn main() {}");
/// assert!(ast.items().next().is_some());
/// ```
pub fn parse_ok(source: &str) -> SourceFile {
    let parse = crate::parser::parse(source);
    assert!(
        parse.ok(),
        "parse failed:\n{}",
        format_parse_errors(parse.errors())
    );
    SourceFile::cast(parse.syntax()).expect("cast to SourceFile")
}

/// Parse source code and return errors, panicking on success.
///
/// Use this when testing code that should produce parse errors.
///
/// # Panics
///
/// Panics if parsing succeeds (no errors).
///
/// # Example
///
/// ```
/// use spl_compiler::testing::parse_err;
///
/// let errors = parse_err("@@@ fn main() {}");
/// assert!(!errors.is_empty());
/// ```
pub fn parse_err(source: &str) -> Vec<ParseError> {
    let parse = crate::parser::parse(source);
    assert!(!parse.ok(), "expected parse errors but parsing succeeded");
    parse.errors().to_vec()
}

/// Resolve names in source code and return the result, panicking on error.
///
/// Use this when testing code that should resolve successfully.
///
/// # Panics
///
/// Panics if parsing fails or resolution produces errors.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::resolve_ok;
///
/// let result = resolve_ok("fn main() { let x = 1; x; }");
/// assert!(!result.resolutions.is_empty());
/// ```
pub fn resolve_ok(source: &str) -> ResolveResult {
    let ast = parse_ok(source);
    let result = crate::sema::resolve(&ast);
    assert!(
        result.diagnostics.is_empty(),
        "resolution failed:\n{}",
        format_diagnostics(&result.diagnostics)
    );
    result
}

/// Resolve names in source code and return diagnostics, panicking on success.
///
/// Use this when testing code that should produce resolution errors.
///
/// # Panics
///
/// Panics if parsing fails or resolution succeeds (no errors).
///
/// # Example
///
/// ```
/// use spl_compiler::testing::resolve_err;
///
/// let diags = resolve_err("fn main() { undefined; }");
/// assert!(!diags.is_empty());
/// ```
pub fn resolve_err(source: &str) -> Vec<Diagnostic> {
    let ast = parse_ok(source);
    let result = crate::sema::resolve(&ast);
    assert!(
        !result.diagnostics.is_empty(),
        "expected resolution errors but resolution succeeded"
    );
    result.diagnostics
}

/// Run type inference on source code and return the result, panicking on error.
///
/// Use this when testing code that should type-check successfully.
///
/// # Panics
///
/// Panics if parsing, resolution, or type inference fails.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::infer_ok;
///
/// let result = infer_ok("fn main() { let x = 1; }");
/// assert!(!result.binding_types.is_empty());
/// ```
pub fn infer_ok(source: &str) -> InferResult {
    let ast = parse_ok(source);
    let resolve_result = crate::sema::resolve(&ast);
    assert!(
        resolve_result.diagnostics.is_empty(),
        "resolution failed:\n{}",
        format_diagnostics(&resolve_result.diagnostics)
    );
    let infer_result = crate::sema::infer(&ast, &resolve_result);
    assert!(
        infer_result.diagnostics.is_empty(),
        "type inference failed:\n{}",
        format_diagnostics(&infer_result.diagnostics)
    );
    infer_result
}

/// Run type inference on source code and return diagnostics, panicking on success.
///
/// Use this when testing code that should produce type errors.
///
/// # Panics
///
/// Panics if parsing fails or type inference succeeds (no errors from
/// resolution or inference).
///
/// # Example
///
/// ```
/// use spl_compiler::testing::infer_err;
///
/// let diags = infer_err("fn main() { let x: bool = 1; }");
/// assert!(!diags.is_empty());
/// ```
pub fn infer_err(source: &str) -> Vec<Diagnostic> {
    let ast = parse_ok(source);
    let resolve_result = crate::sema::resolve(&ast);
    let infer_result = crate::sema::infer(&ast, &resolve_result);
    assert!(
        !infer_result.diagnostics.is_empty(),
        "expected type inference errors but inference succeeded"
    );
    infer_result.diagnostics
}

// ============================================================================
// MIR Inspection Helpers
// ============================================================================

/// Inspector for a collection of MIR bodies.
///
/// Provides convenient access to MIR structure for testing.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::{compile_ok, MirInspector};
///
/// let bodies = compile_ok("fn main() {} fn foo() {}");
/// let inspector = MirInspector::new(&bodies);
/// assert_eq!(inspector.function_count(), 2);
/// ```
pub struct MirInspector<'a> {
    bodies: &'a [Body],
}

impl<'a> MirInspector<'a> {
    /// Create a new MIR inspector.
    pub fn new(bodies: &'a [Body]) -> Self {
        MirInspector { bodies }
    }

    /// Get the number of function bodies.
    pub fn function_count(&self) -> usize {
        self.bodies.len()
    }

    /// Get an inspector for a specific body by index.
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn body(&self, index: usize) -> BodyInspector<'a> {
        BodyInspector::new(&self.bodies[index])
    }

    /// Get an iterator over all body inspectors.
    pub fn bodies(&self) -> impl Iterator<Item = BodyInspector<'a>> {
        self.bodies.iter().map(BodyInspector::new)
    }
}

/// Inspector for a single MIR function body.
///
/// Provides convenient access to body structure for testing.
pub struct BodyInspector<'a> {
    body: &'a Body,
}

impl<'a> BodyInspector<'a> {
    /// Create a new body inspector.
    pub fn new(body: &'a Body) -> Self {
        BodyInspector { body }
    }

    /// Get the underlying body reference.
    pub fn body(&self) -> &'a Body {
        self.body
    }

    /// Get the number of basic blocks.
    pub fn block_count(&self) -> usize {
        self.body.basic_blocks.len()
    }

    /// Get the number of local variables.
    pub fn local_count(&self) -> usize {
        self.body.locals.len()
    }

    /// Get an inspector for a specific block by index.
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn block(&self, index: usize) -> BlockInspector<'a> {
        BlockInspector::new(&self.body.basic_blocks[index])
    }

    /// Check if any block has an assignment with a specific integer constant.
    pub fn has_assignment_with_const(&self, value: i64) -> bool {
        for block in &self.body.basic_blocks {
            for stmt in &block.statements {
                if let StatementKind::Assign(_, rvalue) = &stmt.kind
                    && rvalue_has_const(rvalue, value)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Check if any terminator is a function call.
    pub fn has_call(&self) -> bool {
        for block in &self.body.basic_blocks {
            if let Some(term) = &block.terminator
                && matches!(term.kind, TerminatorKind::Call { .. })
            {
                return true;
            }
        }
        false
    }

    /// Get the total number of statements across all blocks.
    pub fn total_statements(&self) -> usize {
        self.body
            .basic_blocks
            .iter()
            .map(|b| b.statements.len())
            .sum()
    }
}

/// Helper to check if an rvalue contains a specific integer constant.
fn rvalue_has_const(rvalue: &Rvalue, value: i64) -> bool {
    match rvalue {
        Rvalue::Use(operand) | Rvalue::UnaryOp(_, operand) => operand_has_const(operand, value),
        Rvalue::BinaryOp(_, lhs, rhs) => {
            operand_has_const(lhs, value) || operand_has_const(rhs, value)
        }
        _ => false,
    }
}

/// Helper to check if an operand contains a specific integer constant.
fn operand_has_const(operand: &Operand, value: i64) -> bool {
    matches!(operand, Operand::Constant(Constant::Int(v, _)) if *v == value as i128)
}

/// Inspector for a single basic block.
///
/// Provides convenient access to block structure for testing.
pub struct BlockInspector<'a> {
    block: &'a BasicBlockData,
}

impl<'a> BlockInspector<'a> {
    /// Create a new block inspector.
    pub fn new(block: &'a BasicBlockData) -> Self {
        BlockInspector { block }
    }

    /// Get the underlying block reference.
    pub fn block(&self) -> &'a BasicBlockData {
        self.block
    }

    /// Get the number of statements in this block.
    pub fn statement_count(&self) -> usize {
        self.block.statements.len()
    }

    /// Check if the block terminates with a return.
    pub fn has_return(&self) -> bool {
        self.block
            .terminator
            .as_ref()
            .is_some_and(|t| matches!(t.kind, TerminatorKind::Return))
    }

    /// Check if the block terminates with a goto.
    pub fn has_goto(&self) -> bool {
        self.block
            .terminator
            .as_ref()
            .is_some_and(|t| matches!(t.kind, TerminatorKind::Goto(_)))
    }

    /// Check if the block terminates with a switch.
    pub fn has_switch(&self) -> bool {
        self.block
            .terminator
            .as_ref()
            .is_some_and(|t| matches!(t.kind, TerminatorKind::SwitchInt { .. }))
    }

    /// Check if the block terminates with a call.
    pub fn has_call(&self) -> bool {
        self.block
            .terminator
            .as_ref()
            .is_some_and(|t| matches!(t.kind, TerminatorKind::Call { .. }))
    }

    /// Check if the block has a terminator.
    pub fn is_terminated(&self) -> bool {
        self.block.terminator.is_some()
    }
}

/// Format a MIR body as a human-readable string.
///
/// # Example
///
/// ```
/// use spl_compiler::testing::{compile_ok, format_mir};
///
/// let bodies = compile_ok("fn main() {}");
/// let mir_text = format_mir(&bodies[0]);
/// assert!(mir_text.contains("return"));
/// ```
pub fn format_mir(body: &Body) -> String {
    mir::pretty_print(body, None)
}

// ============================================================================
// Package Loading Helpers
// ============================================================================

use crate::package::{Package, PackageError};

/// Load a package from `tests/packages/` at the workspace root, panicking on error.
///
/// # Panics
///
/// Panics if the package cannot be loaded.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::testing::package_ok;
///
/// let pkg = package_ok("simple");
/// assert_eq!(pkg.file_count(), 2);
/// ```
pub fn package_ok(name: &str) -> Package {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    // Navigate from crate dir to workspace root
    let path = std::path::Path::new(&manifest_dir)
        .join("../../tests/packages")
        .join(name);
    Package::load(&path).unwrap_or_else(|e| panic!("failed to load package '{name}': {e:?}"))
}

/// Load a package from `tests/packages/` at the workspace root and expect it to fail.
///
/// # Panics
///
/// Panics if the package loads successfully.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::testing::package_err;
///
/// let err = package_err("empty");
/// // Error is returned for inspection
/// ```
pub fn package_err(name: &str) -> PackageError {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    // Navigate from crate dir to workspace root
    let path = std::path::Path::new(&manifest_dir)
        .join("../../tests/packages")
        .join(name);
    match Package::load(&path) {
        Ok(_) => panic!("expected package '{name}' to fail, but it succeeded"),
        Err(e) => e,
    }
}

/// Load a package from any path.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::testing::load_package;
///
/// let result = load_package("path/to/package");
/// assert!(result.is_ok());
/// ```
pub fn load_package(path: impl AsRef<std::path::Path>) -> Result<Package, PackageError> {
    Package::load(path)
}

/// Check a MIR snapshot using `expect_test`.
///
/// Compiles the source and compares the MIR output to the expected snapshot.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::testing::check_mir;
/// use expect_test::expect;
///
/// check_mir("fn main() {}", &expect![[r#"
///     fn fn(_0: ty0) -> ty0 {
///         bb0:
///             return
///     }
/// "#]]);
/// ```
#[cfg(test)]
pub fn check_mir(source: &str, expected: &expect_test::Expect) {
    let bodies = compile_ok(source);
    assert!(!bodies.is_empty(), "expected at least one MIR body");
    let mir_text = format_mir(&bodies[0]);
    expected.assert_eq(&mir_text);
}

// ============================================================================
// Fixture Loading
// ============================================================================

/// Load a fixture file from `tests/fixtures/`.
///
/// # Panics
///
/// Panics if the fixture file cannot be read.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::testing::load_fixture;
///
/// let source = load_fixture("simple_main.spl");
/// assert!(source.contains("fn main"));
/// ```
pub fn load_fixture(name: &str) -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let path = std::path::Path::new(&manifest_dir)
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to load fixture '{}': {}", path.display(), e))
}

/// Compile a fixture file and return MIR bodies, panicking on error.
///
/// # Panics
///
/// Panics if the fixture cannot be loaded or compilation fails.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::testing::compile_fixture_ok;
///
/// let bodies = compile_fixture_ok("simple_main.spl");
/// assert_eq!(bodies.len(), 1);
/// ```
pub fn compile_fixture_ok(name: &str) -> Vec<mir::Body> {
    let source = load_fixture(name);
    compile_ok(&source)
}

/// Compile a fixture file and return diagnostics, panicking on success.
///
/// # Panics
///
/// Panics if the fixture cannot be loaded or compilation succeeds.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::testing::compile_fixture_err;
///
/// let diags = compile_fixture_err("error_undefined.spl");
/// assert!(!diags.is_empty());
/// ```
pub fn compile_fixture_err(name: &str) -> Vec<Diagnostic> {
    let source = load_fixture(name);
    compile_err(&source)
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
/// use spl_compiler::testing::{compile_err, assert_has_error};
///
/// let diags = compile_err("fn main() { x; }");
/// assert_has_error(&diags, "cannot find");
/// ```
pub fn assert_has_error(diags: &[Diagnostic], pattern: &str) {
    assert!(
        diags.iter().any(|d| d.message.contains(pattern)),
        "no diagnostic matching '{}'\nActual diagnostics:\n{}",
        pattern,
        format_diagnostics(diags)
    );
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
/// use spl_compiler::testing::{compile_err, assert_error_count};
///
/// let diags = compile_err("fn main() { x; y; }");
/// assert_error_count(&diags, 2);
/// ```
pub fn assert_error_count(diags: &[Diagnostic], expected: usize) {
    let actual = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    assert!(
        actual == expected,
        "expected {expected} errors, found {actual}\nDiagnostics:\n{}",
        format_diagnostics(diags)
    );
}

// ============================================================================
// Test Directives for .spl Test Files
// ============================================================================

/// Directives parsed from `//@ directive` comments in test files.
///
/// These control how the test harness runs and validates tests.
///
/// # Example
///
/// ```text
/// //@ run-pass
/// //@ expect-return: 42
///
/// fn main(): i32 { 42 }
/// ```
#[derive(Debug, Default)]
pub struct TestDirectives {
    /// Test should compile and run successfully (default).
    pub run_pass: bool,
    /// Test should fail to compile.
    pub compile_fail: bool,
    /// Expected return value from `main()`.
    pub expect_return: Option<i32>,
    /// Expected substring in stdout.
    pub expect_stdout: Option<String>,
    /// Expected error message patterns (for compile-fail tests).
    pub expect_errors: Vec<String>,
    /// Skip this test.
    pub ignore: bool,
}

/// Parse test directives from source code comments.
///
/// Recognizes directives in the format `//@ directive` or `//@ directive: value`.
///
/// # Supported Directives
///
/// - `run-pass` - Expect successful compilation and execution (default)
/// - `compile-fail` - Expect compilation to fail
/// - `expect-return: N` - Assert `main()` returns N
/// - `expect-stdout: text` - Assert stdout contains text
/// - `expect-error: pattern` - Assert error message contains pattern
/// - `ignore` - Skip this test
///
/// # Example
///
/// ```
/// use spl_compiler::testing::parse_directives;
///
/// let source = r#"
/// //@ run-pass
/// //@ expect-return: 42
/// fn main(): i32 { 42 }
/// "#;
///
/// let directives = parse_directives(source);
/// assert!(directives.run_pass);
/// assert_eq!(directives.expect_return, Some(42));
/// ```
pub fn parse_directives(source: &str) -> TestDirectives {
    let mut directives = TestDirectives::default();
    let mut has_mode_directive = false;

    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("//@") {
            let directive = rest.trim();

            if directive == "run-pass" {
                directives.run_pass = true;
                has_mode_directive = true;
            } else if directive == "compile-fail" {
                directives.compile_fail = true;
                has_mode_directive = true;
            } else if directive == "ignore" {
                directives.ignore = true;
            } else if let Some(value) = directive.strip_prefix("expect-return:") {
                if let Ok(n) = value.trim().parse() {
                    directives.expect_return = Some(n);
                }
            } else if let Some(value) = directive.strip_prefix("expect-stdout:") {
                directives.expect_stdout = Some(value.trim().to_string());
            } else if let Some(value) = directive.strip_prefix("expect-error:") {
                directives.expect_errors.push(value.trim().to_string());
            }
        }
    }

    // Default to run-pass if no mode directive specified
    if !has_mode_directive {
        directives.run_pass = true;
    }

    directives
}

/// Result of executing an SPL program.
#[derive(Debug)]
pub struct ExecuteResult {
    /// The return value from `main()`.
    pub return_value: i32,
    /// Standard output from the program.
    pub stdout: String,
}

/// Run an SPL test given its path (for error messages) and source contents.
///
/// Parses directives from the source, compiles and optionally executes,
/// then validates the results against the directives.
///
/// # Arguments
///
/// * `path` - Path to the test file (for error messages)
/// * `source` - The SPL source code
///
/// # Returns
///
/// * `Ok(())` if the test passes
/// * `Err(message)` if the test fails
///
/// # Example
///
/// ```ignore
/// use spl_compiler::testing::run_spl_test;
/// use std::path::Path;
///
/// let source = r#"
/// //@ run-pass
/// //@ expect-return: 42
/// fn main(): i32 { 42 }
/// "#;
///
/// run_spl_test(Path::new("test.spl"), source).unwrap();
/// ```
pub fn run_spl_test(path: &std::path::Path, source: &str) -> Result<(), String> {
    let directives = parse_directives(source);

    // Handle ignored tests
    if directives.ignore {
        return Ok(());
    }

    // Compile the source
    let compile_result = crate::compile(source);

    if directives.compile_fail {
        // For compile-fail tests, we expect errors
        if compile_result.is_ok() {
            return Err(format!(
                "{}: expected compilation to fail, but it succeeded",
                path.display()
            ));
        }

        // Check expected error patterns
        for pattern in &directives.expect_errors {
            let found = compile_result
                .diagnostics
                .iter()
                .any(|d| d.message.contains(pattern));
            if !found {
                return Err(format!(
                    "{}: expected error containing '{}', but got:\n{}",
                    path.display(),
                    pattern,
                    format_diagnostics(&compile_result.diagnostics)
                ));
            }
        }

        return Ok(());
    }

    // For run-pass tests, compilation must succeed
    if compile_result.is_err() {
        return Err(format!(
            "{}: compilation failed:\n{}",
            path.display(),
            format_diagnostics(&compile_result.diagnostics)
        ));
    }

    // If we need to check return value or stdout, execute the program
    if directives.expect_return.is_some() || directives.expect_stdout.is_some() {
        let result = execute_captured(source)
            .map_err(|e| format!("{}: execution failed: {}", path.display(), e))?;

        // Check return value
        if let Some(expected) = directives.expect_return
            && result.return_value != expected
        {
            return Err(format!(
                "{}: expected return value {}, got {}",
                path.display(),
                expected,
                result.return_value
            ));
        }

        // Check stdout
        if let Some(expected) = &directives.expect_stdout
            && !result.stdout.contains(expected)
        {
            return Err(format!(
                "{}: expected stdout to contain '{}', got:\n{}",
                path.display(),
                expected,
                result.stdout
            ));
        }
    }

    Ok(())
}

/// Execute SPL source code and capture its output.
///
/// Compiles to an executable, runs it as a subprocess, and captures stdout/return value.
fn execute_captured(source: &str) -> Result<ExecuteResult, crate::AotError> {
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    // Create a unique temp file for the executable
    let temp_dir = std::env::temp_dir();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let exe_name = format!("spl_test_{}_{}", std::process::id(), counter);
    let exe_path = temp_dir.join(exe_name);

    // Compile and link
    crate::compile_and_link(source, &exe_path)?;

    // Execute and capture output
    let output = Command::new(&exe_path)
        .output()
        .map_err(crate::AotError::Io)?;

    // Clean up
    let _ = std::fs::remove_file(&exe_path);

    Ok(ExecuteResult {
        return_value: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
    })
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

    // === Phase-Specific Helpers: parse_ok/parse_err ===

    #[test]
    fn parse_ok_returns_source_file() {
        let ast = parse_ok("fn main() {}");
        assert!(ast.items().next().is_some());
    }

    #[test]
    #[should_panic(expected = "parse failed")]
    fn parse_ok_panics_on_error() {
        parse_ok("@@@ fn main() {}");
    }

    #[test]
    fn parse_err_returns_errors() {
        let errors = parse_err("@@@ fn main() {}");
        assert!(!errors.is_empty());
    }

    #[test]
    #[should_panic(expected = "expected parse errors")]
    fn parse_err_panics_on_success() {
        parse_err("fn main() {}");
    }

    // === Phase-Specific Helpers: resolve_ok/resolve_err ===

    #[test]
    fn resolve_ok_returns_result() {
        let result = resolve_ok("fn main() { let x = 1; x; }");
        assert!(!result.resolutions.is_empty());
    }

    #[test]
    #[should_panic(expected = "resolution failed")]
    fn resolve_ok_panics_on_undefined() {
        resolve_ok("fn main() { undefined; }");
    }

    #[test]
    fn resolve_err_returns_diagnostics() {
        let diags = resolve_err("fn main() { undefined; }");
        assert!(!diags.is_empty());
    }

    #[test]
    #[should_panic(expected = "expected resolution errors")]
    fn resolve_err_panics_on_success() {
        resolve_err("fn main() { let x = 1; x; }");
    }

    // === Phase-Specific Helpers: infer_ok/infer_err ===

    #[test]
    fn infer_ok_returns_result() {
        let result = infer_ok("fn main() { let x = 1; }");
        assert!(!result.binding_types.is_empty());
    }

    #[test]
    #[should_panic(expected = "type inference failed")]
    fn infer_ok_panics_on_type_error() {
        infer_ok("fn main() { let x: bool = 1; }");
    }

    #[test]
    fn infer_err_returns_diagnostics() {
        let diags = infer_err("fn main() { let x: bool = 1; }");
        assert!(!diags.is_empty());
    }

    #[test]
    #[should_panic(expected = "expected type inference errors")]
    fn infer_err_panics_on_success() {
        infer_err("fn main() { let x = 1; }");
    }

    // === format_parse_errors ===

    #[test]
    fn format_parse_errors_shows_messages() {
        let errors = parse_err("@@@ fn main() {}");
        let formatted = format_parse_errors(&errors);
        assert!(formatted.contains("[parse error]"));
    }

    // === MIR Inspection: MirInspector ===

    #[test]
    fn mir_inspector_finds_functions() {
        let bodies = compile_ok("fn main() {} fn foo() {}");
        let inspector = MirInspector::new(&bodies);
        assert_eq!(inspector.function_count(), 2);
    }

    #[test]
    fn mir_inspector_body_access() {
        let bodies = compile_ok("fn main() {}");
        let inspector = MirInspector::new(&bodies);
        let body = inspector.body(0);
        assert!(body.block_count() >= 1);
    }

    #[test]
    fn mir_inspector_bodies_iterator() {
        let bodies = compile_ok("fn a() {} fn b() {} fn c() {}");
        let inspector = MirInspector::new(&bodies);
        let count = inspector.bodies().count();
        assert_eq!(count, 3);
    }

    // === MIR Inspection: BodyInspector ===

    #[test]
    fn body_inspector_block_count() {
        let bodies = compile_ok("fn main() {}");
        let inspector = BodyInspector::new(&bodies[0]);
        assert!(inspector.block_count() >= 1);
    }

    #[test]
    fn body_inspector_local_count() {
        let bodies = compile_ok("fn main() { let x = 1; }");
        let inspector = BodyInspector::new(&bodies[0]);
        // At minimum: return place + x
        assert!(inspector.local_count() >= 2);
    }

    #[test]
    fn body_inspector_has_assignment_with_const() {
        let bodies = compile_ok("fn main() { let x = 42; }");
        let inspector = BodyInspector::new(&bodies[0]);
        assert!(inspector.has_assignment_with_const(42));
        assert!(!inspector.has_assignment_with_const(999));
    }

    #[test]
    fn body_inspector_has_call() {
        let bodies = compile_ok("fn foo() {} fn main() { foo(); }");
        // main is the second function
        let inspector = BodyInspector::new(&bodies[1]);
        assert!(inspector.has_call());
    }

    #[test]
    fn body_inspector_no_call() {
        let bodies = compile_ok("fn main() { let x = 1; }");
        let inspector = BodyInspector::new(&bodies[0]);
        assert!(!inspector.has_call());
    }

    #[test]
    fn body_inspector_total_statements() {
        let bodies = compile_ok("fn main() { let x = 1; let y = 2; }");
        let inspector = BodyInspector::new(&bodies[0]);
        // Should have at least 2 assignment statements
        assert!(inspector.total_statements() >= 2);
    }

    // === MIR Inspection: BlockInspector ===

    #[test]
    fn block_inspector_statement_count() {
        let bodies = compile_ok("fn main() { let x = 1; }");
        let inspector = BodyInspector::new(&bodies[0]);
        let block = inspector.block(0);
        // Entry block should have at least 1 statement
        assert!(block.statement_count() >= 1);
    }

    #[test]
    fn block_inspector_has_return() {
        let bodies = compile_ok("fn main() {}");
        let inspector = BodyInspector::new(&bodies[0]);
        // Find the block with return (may be entry or a later block)
        let has_return = (0..inspector.block_count()).any(|i| inspector.block(i).has_return());
        assert!(has_return);
    }

    #[test]
    fn block_inspector_is_terminated() {
        let bodies = compile_ok("fn main() {}");
        let inspector = BodyInspector::new(&bodies[0]);
        let block = inspector.block(0);
        assert!(block.is_terminated());
    }

    #[test]
    fn block_inspector_has_switch() {
        let bodies = compile_ok("fn main() { if true { 1; } else { 2; } }");
        let inspector = BodyInspector::new(&bodies[0]);
        // Should have a switch somewhere for the if condition
        let has_switch = (0..inspector.block_count()).any(|i| inspector.block(i).has_switch());
        assert!(has_switch);
    }

    // === MIR Snapshot: format_mir ===

    #[test]
    fn format_mir_contains_return() {
        let bodies = compile_ok("fn main() {}");
        let mir_text = format_mir(&bodies[0]);
        assert!(mir_text.contains("return"));
    }

    #[test]
    fn format_mir_shows_blocks() {
        let bodies = compile_ok("fn main() {}");
        let mir_text = format_mir(&bodies[0]);
        assert!(mir_text.contains("bb0:"));
    }

    // === MIR Snapshot: check_mir ===

    #[test]
    fn check_mir_basic() {
        use expect_test::expect;
        check_mir(
            "fn main() {}",
            &expect![[r#"
                fn fn() -> ty0 {
                    let _1: ty0;

                    bb0:
                        return
                }
            "#]],
        );
    }

    // === Fixture Loading ===

    #[test]
    fn load_fixture_reads_file() {
        let source = load_fixture("simple_main.spl");
        assert!(source.contains("fn main"));
    }

    #[test]
    fn compile_fixture_ok_returns_bodies() {
        let bodies = compile_fixture_ok("simple_main.spl");
        assert_eq!(bodies.len(), 1);
    }

    #[test]
    fn compile_fixture_ok_arithmetic() {
        let bodies = compile_fixture_ok("arithmetic.spl");
        assert_eq!(bodies.len(), 1);
        let inspector = BodyInspector::new(&bodies[0]);
        assert!(inspector.local_count() >= 4); // return + a + b + c
    }

    #[test]
    fn compile_fixture_err_returns_diagnostics() {
        let diags = compile_fixture_err("error_undefined.spl");
        assert!(!diags.is_empty());
        assert_has_error(&diags, "cannot find");
    }

    #[test]
    #[should_panic(expected = "failed to load fixture")]
    fn load_fixture_panics_on_missing_file() {
        load_fixture("nonexistent_file.spl");
    }

    // === Test Directives ===

    #[test]
    fn parse_directives_run_pass() {
        let source = "//@ run-pass\nfn main() {}";
        let directives = parse_directives(source);
        assert!(directives.run_pass);
        assert!(!directives.compile_fail);
    }

    #[test]
    fn parse_directives_compile_fail() {
        let source = "//@ compile-fail\nfn main() {}";
        let directives = parse_directives(source);
        assert!(directives.compile_fail);
        assert!(!directives.run_pass);
    }

    #[test]
    fn parse_directives_expect_return() {
        let source = "//@ expect-return: 42\nfn main(): i32 { 42 }";
        let directives = parse_directives(source);
        assert_eq!(directives.expect_return, Some(42));
    }

    #[test]
    fn parse_directives_expect_stdout() {
        let source = "//@ expect-stdout: hello\nfn main() {}";
        let directives = parse_directives(source);
        assert_eq!(directives.expect_stdout, Some("hello".to_string()));
    }

    #[test]
    fn parse_directives_expect_errors() {
        let source =
            "//@ compile-fail\n//@ expect-error: undefined\n//@ expect-error: type\nfn main() {}";
        let directives = parse_directives(source);
        assert_eq!(directives.expect_errors.len(), 2);
        assert!(directives.expect_errors.contains(&"undefined".to_string()));
        assert!(directives.expect_errors.contains(&"type".to_string()));
    }

    #[test]
    fn parse_directives_ignore() {
        let source = "//@ ignore\nfn main() {}";
        let directives = parse_directives(source);
        assert!(directives.ignore);
    }

    #[test]
    fn parse_directives_defaults_to_run_pass() {
        let source = "fn main() {}";
        let directives = parse_directives(source);
        assert!(directives.run_pass);
    }

    #[test]
    fn parse_directives_multiple() {
        let source = "//@ run-pass\n//@ expect-return: 0\nfn main(): i32 { 0 }";
        let directives = parse_directives(source);
        assert!(directives.run_pass);
        assert_eq!(directives.expect_return, Some(0));
    }

    #[test]
    fn run_spl_test_run_pass_succeeds() {
        use std::path::Path;
        let source = "//@ run-pass\nfn main() {}";
        let result = run_spl_test(Path::new("test.spl"), source);
        assert!(result.is_ok());
    }

    #[test]
    fn run_spl_test_compile_fail_succeeds() {
        use std::path::Path;
        let source = "//@ compile-fail\n//@ expect-error: cannot find\nfn main() { undefined; }";
        let result = run_spl_test(Path::new("test.spl"), source);
        assert!(result.is_ok());
    }

    #[test]
    fn run_spl_test_compile_fail_no_error() {
        use std::path::Path;
        let source = "//@ compile-fail\nfn main() {}";
        let result = run_spl_test(Path::new("test.spl"), source);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected compilation to fail"));
    }

    #[test]
    fn run_spl_test_expect_return() {
        use std::path::Path;
        let source = "//@ run-pass\n//@ expect-return: 42\nfn main(): i32 { 42 }";
        let result = run_spl_test(Path::new("test.spl"), source);
        assert!(result.is_ok(), "error: {result:?}");
    }

    #[test]
    fn run_spl_test_expect_return_wrong() {
        use std::path::Path;
        let source = "//@ run-pass\n//@ expect-return: 42\nfn main(): i32 { 0 }";
        let result = run_spl_test(Path::new("test.spl"), source);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected return value 42"));
    }

    #[test]
    fn run_spl_test_ignored() {
        use std::path::Path;
        let source = "//@ ignore\nfn main() { undefined; }";
        let result = run_spl_test(Path::new("test.spl"), source);
        assert!(result.is_ok()); // Ignored tests always pass
    }
}
