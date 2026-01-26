//! Multi-file package compilation.
//!
//! Compiles packages through the full pipeline: resolution, type inference,
//! HIR lowering, and MIR lowering.

use super::Package;
use crate::{CompileResult, Diagnostic, Severity, hir, mir, sema};

/// Maximum number of diagnostics to report before stopping.
const MAX_DIAGNOSTICS: usize = 100;

/// Truncate diagnostics to `MAX_DIAGNOSTICS` if exceeded.
fn truncate_diagnostics_if_needed(diagnostics: &mut Vec<Diagnostic>) {
    if diagnostics.len() > MAX_DIAGNOSTICS {
        diagnostics.truncate(MAX_DIAGNOSTICS);
        diagnostics.push(Diagnostic::warning(format!(
            "error limit reached ({MAX_DIAGNOSTICS} errors), stopping"
        )));
    }
}

/// Check if any diagnostic is an error.
fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// Compile a package through the full pipeline.
///
/// Runs resolution, type inference, HIR lowering, and MIR lowering
/// across all files in the package and its child modules.
///
/// # Example
///
/// ```ignore
/// use spl::package::{Package, compile_package};
///
/// let pkg = Package::load("path/to/package")?;
/// let result = compile_package(&pkg);
/// if result.is_ok() {
///     // Use result.bodies for code generation
/// }
/// ```
pub fn compile_package(package: &Package) -> CompileResult {
    let mut diagnostics = Vec::new();

    // Phase 1: Multi-file name resolution
    let mut resolve_result = sema::resolve_package(package);
    diagnostics.append(&mut resolve_result.diagnostics);
    truncate_diagnostics_if_needed(&mut diagnostics);

    // Check for errors before type inference
    if has_errors(&diagnostics) {
        return CompileResult {
            bodies: None,
            types: None,
            diagnostics,
        };
    }

    // Phase 2: Type inference across all files
    let mut infer_result = sema::infer_package(package, &resolve_result);
    diagnostics.append(&mut infer_result.diagnostics);
    truncate_diagnostics_if_needed(&mut diagnostics);

    // Check for errors before lowering
    if has_errors(&diagnostics) {
        return CompileResult {
            bodies: None,
            types: None,
            diagnostics,
        };
    }

    // Phase 3: HIR lowering
    let hir_db = hir::lower::lower_package_to_hir(package, &infer_result);

    // Phase 4: MIR lowering
    let bodies = mir::lower_hir_to_mir(&hir_db);

    // Preserve the type interner for codegen
    let types = hir_db.types;

    CompileResult {
        bodies: Some(bodies),
        types: Some(types),
        diagnostics,
    }
}
