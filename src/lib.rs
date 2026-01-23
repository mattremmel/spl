//! SPL (Simple Programming Language) compiler.
//!
//! This crate provides a complete compiler pipeline from source code to MIR.
//!
//! # Error Handling Architecture
//!
//! The SPL compiler uses phase-specific error handling strategies, each tailored
//! to the needs of that compilation phase. This is intentional - different phases
//! have different goals and constraints.
//!
//! ## Phase Summary
//!
//! | Phase | Error Type | Strategy | Rationale |
//! |-------|------------|----------|-----------|
//! | Parser | [`ParseError`] | Recovery sets, event collection | IDE support, partial results |
//! | Sema | [`Diagnostic`] | Imperative collection, builder | Rich user-facing messages |
//! | HIR Lowering | `Missing` nodes | Fallback values | Continue despite earlier errors |
//! | MIR Lowering | `panic!()` | Invariant assertions | Input guaranteed valid |
//!
//! ## Error Flow
//!
//! ```text
//! Source Code
//!     │
//!     ▼
//! ┌─────────────────┐
//! │     Parser      │──▶ ParseError (recoverable, collected)
//! └────────┬────────┘
//!          │ CST (may contain ERROR nodes)
//!          ▼
//! ┌─────────────────┐
//! │   Resolution    │──▶ Diagnostic (user-facing, with spans/labels)
//! │   + Type Infer  │
//! └────────┬────────┘
//!          │ InferResult (types + diagnostics)
//!          ▼
//! ┌─────────────────┐
//! │  HIR Lowering   │──▶ Missing nodes (graceful degradation)
//! └────────┬────────┘
//!          │ HirDatabase (typed HIR)
//!          ▼
//! ┌─────────────────┐
//! │  MIR Lowering   │──▶ panic!() (invariant violations = compiler bugs)
//! └────────┬────────┘
//!          │ MIR Bodies
//!          ▼
//! ```
//!
//! ## Design Rationale
//!
//! - **Parser recovery**: The parser continues after errors to support IDE features
//!   and provide multiple error messages in a single pass. Uses [`ParseError`] rather
//!   than [`Diagnostic`] to keep the parser self-contained and reusable.
//!
//! - **Semantic diagnostics**: Name resolution and type inference produce [`Diagnostic`]
//!   with rich context (spans, labels, suggestions) for user-facing error messages.
//!   Errors are collected imperatively as analysis proceeds.
//!
//! - **HIR fallbacks**: When lowering encounters missing or malformed AST nodes
//!   (from earlier errors), it produces `HirExprKind::Missing` or error types rather
//!   than failing. This allows later phases to run for valid portions of code.
//!
//! - **MIR panics**: MIR lowering assumes valid, well-typed HIR. Any invariant
//!   violation at this stage indicates a compiler bug, not user error, so we panic
//!   rather than produce invalid MIR that would cause worse problems downstream.

pub mod ast;
pub mod codegen;
pub mod diagnostic;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod sema;
pub mod syntax;
pub mod testing;

pub use diagnostic::{Diagnostic, DiagnosticRenderer, Label, RenderConfig, Severity};
pub use lexer::{Lexer, Span, SpannedToken, Token};
pub use parser::{Parse, ParseError, parse};
pub use sema::{DefId, SemanticContext, Symbol, SymbolKind};
pub use syntax::{Lang, SyntaxKind, SyntaxNode, SyntaxToken};

// ============================================================================
// High-Level Compile API
// ============================================================================

use rowan::ast::AstNode;

/// Result of compiling source code through the full pipeline.
///
/// Contains either MIR bodies (on success) or nothing (on error),
/// plus all diagnostics (errors and warnings) produced.
pub struct CompileResult {
    /// MIR bodies if compilation succeeded (no errors).
    pub bodies: Option<Vec<mir::Body>>,
    /// All diagnostics (errors and warnings).
    pub diagnostics: Vec<Diagnostic>,
}

impl CompileResult {
    /// Returns `true` if compilation succeeded (no errors).
    pub fn is_ok(&self) -> bool {
        self.bodies.is_some()
    }

    /// Returns `true` if compilation failed (has errors).
    pub fn is_err(&self) -> bool {
        self.bodies.is_none()
    }

    /// Returns an iterator over error diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    /// Returns an iterator over warning diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
    }
}

/// Compile source code through the full pipeline.
///
/// Runs parsing, name resolution, type inference, HIR lowering, and MIR lowering.
/// Returns all diagnostics and, if successful, the MIR bodies.
///
/// # Example
///
/// ```
/// use spl::compile;
///
/// let result = compile("fn main() {}");
/// if result.is_ok() {
///     println!("Compilation succeeded!");
/// } else {
///     for diag in result.errors() {
///         println!("Error: {}", diag.message);
///     }
/// }
/// ```
pub fn compile(source: &str) -> CompileResult {
    let mut diagnostics = Vec::new();

    // Phase 1: Parse
    let parse = parser::parse(source);

    // Convert parse errors to diagnostics
    for error in parse.errors() {
        diagnostics.push(Diagnostic::error(&error.message).with_label(error.range.clone(), ""));
    }

    // If there are parse errors, we cannot continue
    if !parse.ok() {
        return CompileResult {
            bodies: None,
            diagnostics,
        };
    }

    // Phase 2: Convert to AST
    let Some(source_file) = ast::SourceFile::cast(parse.syntax()) else {
        diagnostics.push(Diagnostic::error("failed to parse source file"));
        return CompileResult {
            bodies: None,
            diagnostics,
        };
    };

    // Phase 3: Name resolution
    let mut resolve_result = sema::resolve(&source_file);
    diagnostics.append(&mut resolve_result.diagnostics);

    // Phase 4: Type inference
    let mut infer_result = sema::infer(&source_file, resolve_result);
    diagnostics.append(&mut infer_result.diagnostics);

    // Check for errors before lowering
    let has_errors = diagnostics.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        return CompileResult {
            bodies: None,
            diagnostics,
        };
    }

    // Phase 5: HIR lowering
    let hir_db = hir::lower::lower_to_hir(&source_file, infer_result);

    // Phase 6: MIR lowering
    let bodies = mir::lower_hir_to_mir(&hir_db);

    CompileResult {
        bodies: Some(bodies),
        diagnostics,
    }
}

#[cfg(test)]
mod compile_tests {
    use super::*;

    // === TDD Cycle 1: Valid program compiles ===

    #[test]
    fn compile_valid_empty_main() {
        let result = compile("fn main() {}");
        assert!(result.is_ok());
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.bodies.unwrap().len(), 1);
    }

    // === TDD Cycle 2: Parse errors reported ===

    #[test]
    fn compile_parse_error() {
        // Use an error the parser can recover from (invalid token between items)
        let result = compile("@@@ fn main() {}");
        assert!(result.is_err());
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn compile_parse_error_only_garbage() {
        let result = compile("!@#");
        assert!(result.is_err());
        assert!(!result.diagnostics.is_empty());
    }

    // === TDD Cycle 3: Undefined name error ===

    #[test]
    fn compile_undefined_name() {
        let result = compile("fn main() { x; }");
        assert!(result.is_err());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("cannot find"))
        );
    }

    // === TDD Cycle 4: Type error ===

    #[test]
    fn compile_type_error() {
        let result = compile("fn main() { let x: i32 = true; }");
        assert!(result.is_err());
    }

    // === TDD Cycle 5: Multiple errors collected ===

    #[test]
    fn compile_multiple_errors() {
        let result = compile("fn main() { x; y; }");
        assert!(result.is_err());
        assert!(result.diagnostics.len() >= 2);
    }

    // === Additional tests ===

    #[test]
    fn compile_valid_with_arithmetic() {
        let result = compile("fn main() { let x = 1 + 2; }");
        assert!(result.is_ok(), "errors: {:?}", result.diagnostics);
    }

    #[test]
    fn compile_valid_function_call() {
        let result = compile("fn foo() {} fn main() { foo(); }");
        assert!(result.is_ok(), "errors: {:?}", result.diagnostics);
        assert_eq!(result.bodies.unwrap().len(), 2);
    }

    #[test]
    fn compile_errors_iterator() {
        let result = compile("fn main() { x; }");
        let error_count = result.errors().count();
        assert!(error_count >= 1);
    }

    #[test]
    fn compile_warnings_iterator() {
        // Currently no warnings are generated, but test the iterator works
        let result = compile("fn main() {}");
        let warning_count = result.warnings().count();
        assert_eq!(warning_count, 0);
    }
}
