//! Multi-file package compilation.
//!
//! Compiles packages through the full pipeline: resolution, type inference,
//! HIR lowering, and MIR lowering.

use super::{Package, SourceMap};
use crate::{CompileResult, Diagnostic, Severity, hir, mir, sema};
use tracing::{debug, info, info_span, warn};

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

/// Convert `file_id` to `file_path` in all diagnostics using the source map.
fn attach_file_paths(diagnostics: &mut [Diagnostic], source_map: &SourceMap) {
    for diag in diagnostics {
        if let Some(file_id) = diag.file_id
            && let Some(path) = source_map.get_path(file_id)
        {
            diag.file_path = Some(path.to_path_buf());
        }
    }
}

/// Compile a package through the full pipeline.
///
/// Runs resolution, type inference, HIR lowering, and MIR lowering
/// across all files in the package and its child modules.
///
/// # Example
///
/// ```ignore
/// use spl_compiler::package::{Package, compile_package};
///
/// let pkg = Package::load("path/to/package")?;
/// let result = compile_package(&pkg);
/// if result.is_ok() {
///     // Use result.bodies for code generation
/// }
/// ```
pub fn compile_package(package: &Package) -> CompileResult {
    let _span = info_span!("compile_package").entered();

    let mut diagnostics = Vec::new();
    let source_map = package.compilation_unit().source_map();

    // Phase 1: Multi-file name resolution
    let mut resolve_result = sema::resolve_package(package);
    // Convert spl_diagnostic::Diagnostic to crate::Diagnostic
    let mut resolve_diags: Vec<Diagnostic> = resolve_result
        .diagnostics
        .drain(..)
        .map(Diagnostic::from)
        .collect();
    let resolve_diagnostic_count = resolve_diags.len();
    diagnostics.append(&mut resolve_diags);
    truncate_diagnostics_if_needed(&mut diagnostics);
    debug!(diagnostic_count = resolve_diagnostic_count, "resolution complete");

    // Check for errors before type inference
    if has_errors(&diagnostics) {
        warn!(diagnostic_count = diagnostics.len(), "early stop after resolution due to errors");
        attach_file_paths(&mut diagnostics, source_map);
        return CompileResult {
            bodies: None,
            types: None,
            diagnostics,
        };
    }

    // Phase 2: Type inference across all files
    let mut infer_result = sema::infer_package(package, &resolve_result);
    // Convert spl_diagnostic::Diagnostic to crate::Diagnostic
    let mut infer_diags: Vec<Diagnostic> = infer_result
        .diagnostics
        .drain(..)
        .map(Diagnostic::from)
        .collect();
    let infer_diagnostic_count = infer_diags.len();
    diagnostics.append(&mut infer_diags);
    truncate_diagnostics_if_needed(&mut diagnostics);
    debug!(diagnostic_count = infer_diagnostic_count, "inference complete");

    // Check for errors before lowering
    if has_errors(&diagnostics) {
        warn!(diagnostic_count = diagnostics.len(), "early stop after inference due to errors");
        attach_file_paths(&mut diagnostics, source_map);
        return CompileResult {
            bodies: None,
            types: None,
            diagnostics,
        };
    }

    // Phase 3: HIR lowering
    let hir_db = hir::lower::lower_package_to_hir(package, &infer_result);
    debug!("HIR lowering complete");

    // Phase 4: MIR lowering
    let bodies = match mir::lower_hir_to_mir(&hir_db) {
        Ok(bodies) => {
            debug!(body_count = bodies.len(), "MIR lowering complete");
            bodies
        }
        Err(ice) => {
            warn!("MIR lowering failed with ICE");
            diagnostics.push(Diagnostic::from(ice.to_diagnostic()));
            attach_file_paths(&mut diagnostics, source_map);
            return CompileResult {
                bodies: None,
                types: None,
                diagnostics,
            };
        }
    };

    // Preserve the type interner for codegen
    let types = hir_db.types;

    // Attach file paths to any remaining diagnostics (warnings)
    attach_file_paths(&mut diagnostics, source_map);

    info!(body_count = bodies.len(), diagnostic_count = diagnostics.len(), "package compilation complete");

    CompileResult {
        bodies: Some(bodies),
        types: Some(types),
        diagnostics,
    }
}
