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
//! | MIR Lowering | [`IceError`](mir::IceError) | Result propagation | Structured ICE reporting |
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
//! │  MIR Lowering   │──▶ IceError (structured internal compiler error)
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
//! - **MIR ICE errors**: MIR lowering assumes valid, well-typed HIR. Any invariant
//!   violation at this stage indicates a compiler bug, not user error. We return
//!   structured [`IceError`](mir::IceError) with context (spans, `DefId`s) to help
//!   diagnose bugs, converting them to user-friendly diagnostics.

pub mod ast;
pub mod diagnostic;
mod tracing_init;

/// Native code generation via Cranelift.
///
/// This module re-exports the spl-codegen crate for JIT and AOT compilation.
pub use spl_codegen as codegen;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod package;
pub mod parser;
pub mod sema;
pub mod session;
pub mod stdlib;
pub mod syntax;
pub mod testing;

pub use diagnostic::{Diagnostic, DiagnosticRenderer, Label, RenderConfig, Severity};
pub use lexer::{Lexer, Span, SpannedToken, Token};
pub use parser::{Parse, ParseError, parse};
pub use sema::{DefId, SemanticContext, Symbol, SymbolKind};
pub use session::CompileSession;
pub use syntax::{Lang, SyntaxKind, SyntaxNode, SyntaxToken};
pub use tracing_init::{init_tracing, init_tracing_with_options, init_tracing_with_timing};

// ============================================================================
// High-Level Compile API
// ============================================================================

use rowan::ast::AstNode;
use tracing::{info, info_span};

/// Maximum number of diagnostics to report before stopping.
/// This prevents overwhelming output on heavily broken code.
const MAX_DIAGNOSTICS: usize = 100;

/// Truncate diagnostics to `MAX_DIAGNOSTICS` if exceeded, adding an "error limit reached" note.
fn truncate_diagnostics_if_needed(diagnostics: &mut Vec<Diagnostic>) {
    if diagnostics.len() > MAX_DIAGNOSTICS {
        diagnostics.truncate(MAX_DIAGNOSTICS);
        diagnostics.push(Diagnostic::warning(format!(
            "error limit reached ({MAX_DIAGNOSTICS} errors), stopping"
        )));
    }
}

/// Result of compiling source code through the full pipeline.
///
/// Contains either MIR bodies (on success) or nothing (on error),
/// plus all diagnostics (errors and warnings) produced.
pub struct CompileResult {
    /// MIR bodies if compilation succeeded (no errors).
    pub bodies: Option<Vec<mir::Body>>,
    /// Type interner used during compilation (for codegen).
    pub types: Option<sema::types::TypeInterner>,
    /// All diagnostics (errors and warnings).
    pub diagnostics: Vec<Diagnostic>,
}

impl CompileResult {
    /// Returns `true` if compilation succeeded (no errors).
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.bodies.is_some()
    }

    /// Returns `true` if compilation failed (has errors).
    #[must_use]
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
/// use spl_compiler::compile;
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
#[must_use]
pub fn compile(source: &str) -> CompileResult {
    let _span = info_span!("compile", source_bytes = source.len()).entered();
    let mut diagnostics = Vec::new();

    // Phase 1: Parse
    let parse = parser::parse(source);

    // Convert parse errors to diagnostics
    for error in parse.errors() {
        diagnostics.push(Diagnostic::error(&error.message).with_label(error.range.clone(), ""));
    }
    truncate_diagnostics_if_needed(&mut diagnostics);

    // If there are parse errors, we cannot continue
    if !parse.ok() {
        return CompileResult {
            bodies: None,
            types: None,
            diagnostics,
        };
    }

    // Phase 2: Convert to AST
    let Some(source_file) = ast::SourceFile::cast(parse.syntax()) else {
        diagnostics.push(Diagnostic::error("failed to parse source file"));
        return CompileResult {
            bodies: None,
            types: None,
            diagnostics,
        };
    };

    // Phase 3: Name resolution
    let mut resolve_result = sema::resolve(&source_file);
    // Convert spl_diagnostic::Diagnostic to crate::Diagnostic
    let mut resolve_diags: Vec<Diagnostic> = resolve_result
        .diagnostics
        .drain(..)
        .map(Diagnostic::from)
        .collect();
    diagnostics.append(&mut resolve_diags);
    truncate_diagnostics_if_needed(&mut diagnostics);

    // Phase 4: Type inference
    let mut infer_result = sema::infer(&source_file, &resolve_result);
    // Convert spl_diagnostic::Diagnostic to crate::Diagnostic
    let mut infer_diags: Vec<Diagnostic> = infer_result
        .diagnostics
        .drain(..)
        .map(Diagnostic::from)
        .collect();
    diagnostics.append(&mut infer_diags);
    truncate_diagnostics_if_needed(&mut diagnostics);

    // Check for errors before lowering
    let has_errors = diagnostics.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        return CompileResult {
            bodies: None,
            types: None,
            diagnostics,
        };
    }

    // Phase 5: HIR lowering
    let hir_db = hir::lower::lower_to_hir(&source_file, &infer_result);

    // Phase 6: MIR lowering
    let bodies = match mir::lower_hir_to_mir(&hir_db) {
        Ok(bodies) => bodies,
        Err(ice) => {
            diagnostics.push(Diagnostic::from(ice.to_diagnostic()));
            return CompileResult {
                bodies: None,
                types: None,
                diagnostics,
            };
        }
    };

    // Preserve the type interner for codegen
    let types = hir_db.types;

    let body_count = bodies.len();
    info!(body_count, "compilation complete");

    CompileResult {
        bodies: Some(bodies),
        types: Some(types),
        diagnostics,
    }
}

// ============================================================================
// JIT Execution API
// ============================================================================

use thiserror::Error;

/// Errors that can occur during JIT execution.
#[derive(Debug, Error)]
pub enum JitError {
    /// Compilation failed with diagnostics.
    #[error("compilation failed with {} error(s)", .0.len())]
    CompileError(Vec<Diagnostic>),
    /// Code generation failed.
    #[error("codegen error: {0}")]
    CodegenError(#[from] codegen::CodegenError),
    /// Runtime error during execution.
    #[error("runtime error: {0}")]
    RuntimeError(#[from] codegen::RuntimeError),
    /// No main function found in the program.
    #[error("no main function found")]
    NoMain,
    /// Internal compiler error during MIR lowering.
    #[error("internal compiler error: {0}")]
    Ice(#[from] mir::IceError),
}

/// Compile and execute SPL source code, returning the i32 result of `main()`.
///
/// This is a convenience function for JIT compilation and execution of SPL programs.
/// The program must have a `main` function that returns `i32`.
///
/// # Example
///
/// ```
/// use spl_compiler::jit_execute;
///
/// let result = jit_execute("fn main(): i32 { 42 }");
/// assert_eq!(result.unwrap(), 42);
/// ```
///
/// # Errors
///
/// Returns `JitError::CompileError` if the source has syntax or semantic errors.
/// Returns `JitError::NoMain` if no `main` function is defined.
/// Returns `JitError::CodegenError` if code generation fails.
/// Returns `JitError::RuntimeError` if execution fails (e.g., traps).
#[must_use = "JIT execution result should be used"]
pub fn jit_execute(source: &str) -> Result<i32, JitError> {
    let _span = info_span!("jit_execute").entered();
    // Compile source to MIR
    let result = compile(source);
    if result.is_err() {
        return Err(JitError::CompileError(result.diagnostics));
    }

    let bodies = result.bodies.unwrap();
    let types = result.types.unwrap();

    if bodies.is_empty() {
        return Err(JitError::NoMain);
    }

    // Build function definitions from bodies (using preserved metadata)
    let function_defs: Vec<_> = bodies
        .iter()
        .filter_map(|body| {
            let def_id = body.def_id?;
            let name = body.name.as_ref()?;
            Some((def_id, name.as_str(), body))
        })
        .collect();

    if function_defs.is_empty() {
        return Err(JitError::NoMain);
    }

    // Find main function
    let main_def_id = function_defs
        .iter()
        .find(|(_, name, _)| *name == "main")
        .map(|(def_id, _, _)| *def_id)
        .ok_or(JitError::NoMain)?;

    // Compile to native code
    let mut module = codegen::codegen_jit(&function_defs, &types)?;

    // Set main and run
    module.set_main(main_def_id);
    let result = module.run_main()?;

    Ok(result)
}

// ============================================================================
// AOT Compilation API
// ============================================================================

use std::path::Path;

/// Errors that can occur during AOT compilation.
#[derive(Debug, Error)]
pub enum AotError {
    /// Compilation failed with diagnostics.
    #[error("compilation failed with {} error(s)", .0.len())]
    CompileError(Vec<Diagnostic>),
    /// Code generation failed.
    #[error("codegen error: {0}")]
    CodegenError(#[from] codegen::CodegenError),
    /// Linking failed.
    #[error("link error: {0}")]
    LinkError(#[from] codegen::LinkError),
    /// No functions to compile.
    #[error("no functions to compile")]
    NoFunctions,
    /// IO error (writing files, etc.).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Internal compiler error during MIR lowering.
    #[error("internal compiler error: {0}")]
    Ice(#[from] mir::IceError),
}

/// Compile SPL source code to an object file.
///
/// Returns the raw object file bytes that can be written to disk or linked.
///
/// # Example
///
/// ```
/// use spl_compiler::compile_to_object;
///
/// let object_bytes = compile_to_object("fn main(): i32 { 42 }").unwrap();
/// // object_bytes can be written to a .o file or linked into an executable
/// ```
///
/// # Errors
///
/// Returns `AotError::CompileError` if the source has syntax or semantic errors.
/// Returns `AotError::NoFunctions` if no functions are defined.
/// Returns `AotError::CodegenError` if code generation fails.
#[must_use = "compiled object bytes should be used"]
pub fn compile_to_object(source: &str) -> Result<Vec<u8>, AotError> {
    let _span = info_span!("compile_to_object").entered();
    // Compile source to MIR
    let result = compile(source);
    if result.is_err() {
        return Err(AotError::CompileError(result.diagnostics));
    }

    let bodies = result.bodies.unwrap();
    let types = result.types.unwrap();

    if bodies.is_empty() {
        return Err(AotError::NoFunctions);
    }

    // Build function definitions from bodies
    let function_defs: Vec<_> = bodies
        .iter()
        .filter_map(|body| {
            let def_id = body.def_id?;
            let name = body.name.as_ref()?;
            Some(codegen::FunctionDef::new(def_id, name.as_str(), body))
        })
        .collect();

    if function_defs.is_empty() {
        return Err(AotError::NoFunctions);
    }

    // Compile to object file
    let obj = codegen::AotModuleCompiler::compile(&function_defs, &types)?;

    Ok(obj.into_bytes())
}

/// Compile SPL source code and link it into an executable.
///
/// This function:
/// 1. Compiles the source to MIR
/// 2. Generates an object file
/// 3. Links it into a standalone executable
///
/// # Example
///
/// ```ignore
/// use spl_compiler::compile_and_link;
/// use std::path::Path;
///
/// compile_and_link("fn main(): i32 { 42 }", Path::new("/tmp/my_program")).unwrap();
/// // /tmp/my_program is now an executable that returns 42
/// ```
///
/// # Errors
///
/// Returns `AotError::CompileError` if the source has syntax or semantic errors.
/// Returns `AotError::NoFunctions` if no functions are defined.
/// Returns `AotError::CodegenError` if code generation fails.
/// Returns `AotError::LinkError` if linking fails.
#[must_use = "compilation result should be checked for errors"]
pub fn compile_and_link(source: &str, output: &Path) -> Result<(), AotError> {
    let _span = info_span!("compile_and_link", output = %output.display()).entered();
    let object_bytes = compile_to_object(source)?;
    codegen::link_object_to_executable(&object_bytes, output, None)?;
    Ok(())
}

/// Compile SPL source code and link it with custom options.
///
/// This is like `compile_and_link` but allows specifying linker options
/// (libraries to link, search paths, etc.).
///
/// # Example
///
/// ```ignore
/// use spl_compiler::{compile_and_link_with_options, codegen::LinkOptions};
/// use std::path::Path;
///
/// let options = LinkOptions::new()
///     .library("m")           // Link against libm
///     .library_path("/usr/local/lib");
///
/// compile_and_link_with_options(
///     "fn main(): i32 { 42 }",
///     Path::new("/tmp/my_program"),
///     &options,
/// ).unwrap();
/// ```
#[must_use = "compilation result should be checked for errors"]
pub fn compile_and_link_with_options(
    source: &str,
    output: &Path,
    options: &codegen::LinkOptions,
) -> Result<(), AotError> {
    let object_bytes = compile_to_object(source)?;
    codegen::link_object_to_executable(&object_bytes, output, Some(options))?;
    Ok(())
}

// ============================================================================
// Project Compilation API
// ============================================================================

/// Errors that can occur when compiling a project.
#[derive(Debug, Error)]
pub enum ProjectError {
    /// Failed to load package from directory.
    #[error("failed to load package: {0}")]
    PackageLoad(#[from] package::PackageError),
}

/// Compile a project directory through the full pipeline.
///
/// This loads a package from the given directory path and compiles it through
/// the full pipeline: resolution, type inference, HIR lowering, and MIR lowering.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::compile_project;
/// use std::path::Path;
///
/// let result = compile_project(Path::new("path/to/project"));
/// match result {
///     Ok(compile_result) => {
///         if compile_result.is_ok() {
///             println!("Compilation succeeded!");
///         } else {
///             for diag in compile_result.errors() {
///                 println!("Error: {}", diag.message);
///             }
///         }
///     }
///     Err(e) => println!("Failed to load project: {e}"),
/// }
/// ```
///
/// # Errors
///
/// Returns `ProjectError::PackageLoad` if the directory cannot be loaded as a package
/// (e.g., path doesn't exist, not a directory, no source files, parse errors).
#[must_use = "compilation result should be checked"]
pub fn compile_project(path: &Path) -> Result<CompileResult, ProjectError> {
    let _span = info_span!("compile_project", path = %path.display()).entered();
    let pkg = package::Package::load(path)?;
    Ok(package::compile_package(&pkg))
}

#[cfg(test)]
mod jit_tests {
    use super::*;

    #[test]
    fn jit_execute_returns_42() {
        let result = jit_execute("fn main(): i32 { 42 }");
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn jit_execute_compile_error() {
        let result = jit_execute("fn main() { undefined; }");
        assert!(matches!(result, Err(JitError::CompileError(_))));
    }

    #[test]
    fn jit_execute_no_main() {
        let result = jit_execute("fn foo() {}");
        assert!(matches!(result, Err(JitError::NoMain)));
    }

    #[test]
    fn jit_execute_function_call() {
        let result = jit_execute(
            r#"
            fn add(_ a: i32, _ b: i32): i32 { a + b }
            fn main(): i32 { add(10, 32) }
        "#,
        );
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn jit_execute_arithmetic() {
        let result = jit_execute("fn main(): i32 { 1 + 2 * 3 }");
        assert_eq!(result.unwrap(), 7);
    }

    #[test]
    fn jit_execute_locals() {
        let result = jit_execute("fn main(): i32 { let x = 10; let y = 32; return x + y; }");
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn jit_execute_control_flow() {
        let result = jit_execute(
            r#"
            fn main(): i32 {
                let x = 5;
                return if x > 3 { 1 } else { 0 };
            }
        "#,
        );
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn jit_execute_loop() {
        let result = jit_execute(
            r#"
            fn main(): i32 {
                let mut sum = 0;
                let mut i = 1;
                while i <= 10 {
                    sum = sum + i;
                    i = i + 1;
                }
                return sum;
            }
        "#,
        );
        assert_eq!(result.unwrap(), 55);
    }

    #[test]
    fn jit_error_display_compile() {
        let err = JitError::CompileError(vec![Diagnostic::error("test error")]);
        assert!(err.to_string().contains("compilation failed"));
    }

    #[test]
    fn jit_error_display_no_main() {
        let err = JitError::NoMain;
        assert!(err.to_string().contains("no main"));
    }

    #[test]
    fn jit_error_display_ice() {
        let err = JitError::Ice(mir::IceError::FieldNotFound {
            field: "x".to_string(),
            struct_name: "Point".to_string(),
            struct_def_id: sema::symbol::DefId::INVALID,
            span: None,
        });
        assert!(err.to_string().contains("internal compiler error"));
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

    // === Backward compatibility: single-file compile ===

    #[test]
    fn compile_single_file_backward_compat() {
        // Verify that single-file compile() still works as expected
        let result = compile("fn main(): i32 { 42 }");
        assert!(result.is_ok(), "errors: {:?}", result.diagnostics);
    }

    #[test]
    fn compile_single_file_no_file_path() {
        // Single-file compilation should not have file paths in diagnostics
        let result = compile("fn main() { undefined; }");
        assert!(result.is_err());
        let has_file_path = result.diagnostics.iter().any(|d| d.file_path.is_some());
        assert!(
            !has_file_path,
            "single-file compile should not have file paths in diagnostics"
        );
    }

    // === compile_project tests ===

    fn test_packages_path(name: &str) -> std::path::PathBuf {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        std::path::Path::new(&manifest_dir)
            .join("../spl-test-runner/cases/packages")
            .join(name)
    }

    #[test]
    fn compile_project_simple_package() {
        let path = test_packages_path("simple");
        let result = compile_project(&path);
        assert!(result.is_ok(), "load error: {:?}", result.err());
        let result = result.unwrap();
        assert!(result.is_ok(), "compile errors: {:?}", result.diagnostics);
    }

    #[test]
    fn compile_project_missing_directory() {
        let result = compile_project(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn compile_project_cross_module_imports() {
        let path = test_packages_path("imports_simple");
        let result = compile_project(&path).unwrap();
        assert!(result.is_ok(), "errors: {:?}", result.diagnostics);
    }

    // === compile_project diagnostics with file paths ===

    #[test]
    fn compile_project_diagnostics_have_file_path() {
        let path = test_packages_path("error_in_submodule");
        let result = compile_project(&path).unwrap();
        assert!(result.is_err());

        let has_file_path = result.diagnostics.iter().any(|d| d.file_path.is_some());
        assert!(
            has_file_path,
            "expected diagnostics to include file paths, got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn compile_project_diagnostic_file_path_correct() {
        let path = test_packages_path("error_in_submodule");
        let result = compile_project(&path).unwrap();
        assert!(result.is_err());

        // Error is in utils/utils.spl
        let has_utils_error = result.diagnostics.iter().any(|d| {
            d.file_path
                .as_ref()
                .map(|p| p.to_string_lossy().contains("utils.spl"))
                .unwrap_or(false)
        });
        assert!(
            has_utils_error,
            "expected error in utils.spl, got paths: {:?}",
            result
                .diagnostics
                .iter()
                .map(|d| &d.file_path)
                .collect::<Vec<_>>()
        );
    }

    // === Verify existing visibility and module resolution tests ===

    #[test]
    fn compile_project_private_visibility_error() {
        let path = test_packages_path("import_private");
        let result = compile_project(&path).unwrap();
        assert!(result.is_err());
        assert!(
            result
                .errors()
                .any(|d| d.message.contains("private") || d.message.contains("not visible")),
            "expected private visibility error, got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn compile_project_missing_module_error() {
        let path = test_packages_path("import_missing");
        let result = compile_project(&path).unwrap();
        assert!(result.is_err());
        assert!(
            result
                .errors()
                .any(|d| d.message.contains("not found") || d.message.contains("cannot find")),
            "expected module not found error, got: {:?}",
            result.diagnostics
        );
    }
}

#[cfg(test)]
mod aot_tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn compile_to_object_simple() {
        let result = compile_to_object("fn main(): i32 { 42 }");
        assert!(result.is_ok(), "failed to compile: {:?}", result.err());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn compile_to_object_multiple_functions() {
        let result = compile_to_object(
            r#"
            fn add(_ a: i32, _ b: i32): i32 { a + b }
            fn main(): i32 { add(10, 32) }
        "#,
        );
        assert!(result.is_ok(), "failed to compile: {:?}", result.err());
    }

    #[test]
    fn compile_to_object_compile_error() {
        let result = compile_to_object("fn main() { undefined; }");
        assert!(matches!(result, Err(AotError::CompileError(_))));
    }

    #[test]
    fn compile_to_object_no_functions() {
        // Empty source has no functions
        let result = compile_to_object("");
        assert!(result.is_err());
    }

    #[test]
    fn aot_error_display() {
        let err = AotError::NoFunctions;
        assert!(err.to_string().contains("no functions"));

        let err = AotError::CompileError(vec![Diagnostic::error("test")]);
        assert!(err.to_string().contains("compilation failed"));
    }

    // Helper to create unique temp file paths for parallel tests
    fn unique_temp_exe(name: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let temp_dir = std::env::temp_dir();
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        temp_dir.join(format!("spl_test_{name}_{pid}_{unique_id}_{counter}"))
    }

    // Integration test: compile, link, and run a real executable
    // This test actually links and executes the binary
    #[test]
    fn compile_and_link_and_execute() {
        use std::fs;

        let exe_path = unique_temp_exe("exe");

        // Clean up any previous test file
        let _ = fs::remove_file(&exe_path);

        // Compile and link
        let result = compile_and_link("fn main(): i32 { 42 }", &exe_path);
        assert!(
            result.is_ok(),
            "failed to compile and link: {:?}",
            result.err()
        );

        // Verify the executable exists
        assert!(exe_path.exists(), "executable was not created");

        // Run the executable and check the exit code
        let output = Command::new(&exe_path)
            .output()
            .expect("failed to execute compiled binary");

        // On Unix, exit code is the return value from main (mod 256)
        assert_eq!(
            output.status.code(),
            Some(42),
            "unexpected exit code: {:?}",
            output.status
        );

        // Clean up
        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn compile_and_link_arithmetic() {
        use std::fs;

        let exe_path = unique_temp_exe("arith");

        let _ = fs::remove_file(&exe_path);

        let result = compile_and_link(
            r#"
            fn main(): i32 {
                let x = 10;
                let y = 3;
                return x * y + 2;
            }
        "#,
            &exe_path,
        );
        assert!(result.is_ok(), "failed: {:?}", result.err());

        let output = Command::new(&exe_path).output().expect("failed to execute");

        // 10 * 3 + 2 = 32
        assert_eq!(output.status.code(), Some(32));

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn compile_and_link_function_calls() {
        use std::fs;

        let exe_path = unique_temp_exe("calls");

        let _ = fs::remove_file(&exe_path);

        let result = compile_and_link(
            r#"
            fn double(_ x: i32): i32 { x * 2 }
            fn main(): i32 { double(21) }
        "#,
            &exe_path,
        );
        assert!(result.is_ok(), "failed: {:?}", result.err());

        let output = Command::new(&exe_path).output().expect("failed to execute");

        // double(21) = 42
        assert_eq!(output.status.code(), Some(42));

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn compile_and_link_conditionals() {
        use std::fs;

        let exe_path = unique_temp_exe("cond");

        let _ = fs::remove_file(&exe_path);

        let result = compile_and_link(
            r#"
            fn main(): i32 {
                let x = 5;
                return if x > 3 { 100 } else { 0 };
            }
        "#,
            &exe_path,
        );
        assert!(result.is_ok(), "failed: {:?}", result.err());

        let output = Command::new(&exe_path).output().expect("failed to execute");

        // x > 3, so returns 100
        assert_eq!(output.status.code(), Some(100));

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn aot_jit_output_equivalence() {
        // Verify that AOT and JIT produce the same results
        use std::fs;

        let source = r#"
            fn factorial(_ n: i32): i32 {
                if n <= 1 { 1 } else { n * factorial(n - 1) }
            }
            fn main(): i32 { factorial(5) }
        "#;

        // JIT result
        let jit_result = jit_execute(source).expect("JIT failed");

        // AOT result
        let exe_path = unique_temp_exe("equiv");
        let _ = fs::remove_file(&exe_path);

        compile_and_link(source, &exe_path).expect("AOT failed");

        let output = Command::new(&exe_path).output().expect("failed to execute");

        let aot_result = output.status.code().unwrap();

        // 5! = 120, but exit codes are mod 256, so both should be 120
        assert_eq!(
            jit_result, aot_result,
            "JIT and AOT produced different results"
        );
        assert_eq!(jit_result, 120);

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn compile_and_link_loop() {
        use std::fs;

        let exe_path = unique_temp_exe("loop");
        let _ = fs::remove_file(&exe_path);

        let result = compile_and_link(
            r#"
            fn main(): i32 {
                let mut sum = 0;
                let mut i = 1;
                while i <= 10 {
                    sum = sum + i;
                    i = i + 1;
                }
                return sum;
            }
        "#,
            &exe_path,
        );
        assert!(result.is_ok(), "failed: {:?}", result.err());

        let output = Command::new(&exe_path).output().expect("failed to execute");

        // Sum 1..10 = 55
        assert_eq!(output.status.code(), Some(55));

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn compile_and_link_negative_return() {
        use std::fs;

        let exe_path = unique_temp_exe("neg");
        let _ = fs::remove_file(&exe_path);

        let result = compile_and_link("fn main(): i32 { -1 }", &exe_path);
        assert!(result.is_ok(), "failed: {:?}", result.err());

        let output = Command::new(&exe_path).output().expect("failed to execute");

        // -1 as unsigned 8-bit is 255
        assert_eq!(output.status.code(), Some(255));

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn compile_and_link_zero_return() {
        use std::fs;

        let exe_path = unique_temp_exe("zero");
        let _ = fs::remove_file(&exe_path);

        let result = compile_and_link("fn main(): i32 { 0 }", &exe_path);
        assert!(result.is_ok(), "failed: {:?}", result.err());

        let output = Command::new(&exe_path).output().expect("failed to execute");

        assert_eq!(output.status.code(), Some(0));

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn compile_and_link_large_return_wraps() {
        use std::fs;

        let exe_path = unique_temp_exe("large");
        let _ = fs::remove_file(&exe_path);

        // 300 should wrap to 300 % 256 = 44
        let result = compile_and_link("fn main(): i32 { 300 }", &exe_path);
        assert!(result.is_ok(), "failed: {:?}", result.err());

        let output = Command::new(&exe_path).output().expect("failed to execute");

        assert_eq!(output.status.code(), Some(44)); // 300 % 256

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn compile_and_link_nested_calls() {
        use std::fs;

        let exe_path = unique_temp_exe("nested");
        let _ = fs::remove_file(&exe_path);

        let result = compile_and_link(
            r#"
            fn add(_ a: i32, _ b: i32): i32 { a + b }
            fn mul(_ a: i32, _ b: i32): i32 { a * b }
            fn main(): i32 { add(mul(3, 4), mul(2, 3)) }
        "#,
            &exe_path,
        );
        assert!(result.is_ok(), "failed: {:?}", result.err());

        let output = Command::new(&exe_path).output().expect("failed to execute");

        // 3*4 + 2*3 = 12 + 6 = 18
        assert_eq!(output.status.code(), Some(18));

        let _ = fs::remove_file(&exe_path);
    }

    #[test]
    fn aot_error_source_codegen() {
        use std::error::Error;
        let err = AotError::CodegenError(codegen::CodegenError::Internal("test".to_string()));
        assert!(err.source().is_some());
    }

    #[test]
    fn aot_error_source_io() {
        use std::error::Error;
        let err = AotError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(err.source().is_some());
    }

    #[test]
    fn aot_error_source_compile_error() {
        use std::error::Error;
        let err = AotError::CompileError(vec![]);
        assert!(err.source().is_none());
    }

    #[test]
    fn aot_error_display_all_variants() {
        let err1 =
            AotError::CompileError(vec![Diagnostic::error("test"), Diagnostic::error("test2")]);
        assert!(err1.to_string().contains("2 error"));

        let err2 = AotError::CodegenError(codegen::CodegenError::Internal("internal".to_string()));
        assert!(err2.to_string().contains("codegen error"));

        let err3 = AotError::LinkError(codegen::LinkError::LinkerFailed {
            status: Some(1),
            stdout: String::new(),
            stderr: "link failed".to_string(),
        });
        assert!(err3.to_string().contains("link error"));

        let err4 = AotError::NoFunctions;
        assert!(err4.to_string().contains("no functions"));

        let err5 = AotError::Io(std::io::Error::other("io"));
        assert!(err5.to_string().contains("IO error"));

        let err6 = AotError::Ice(mir::IceError::FieldNotFound {
            field: "x".to_string(),
            struct_name: "Point".to_string(),
            struct_def_id: sema::symbol::DefId::INVALID,
            span: None,
        });
        assert!(err6.to_string().contains("internal compiler error"));
    }
}
