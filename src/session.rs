//! Compile session providing lazy, cached phase execution.
//!
//! The [`CompileSession`] abstraction provides:
//! - Lazy computation: phases run only when accessed
//! - Caching: each phase runs at most once
//! - Early stopping: phases stop on errors from prior phases
//! - Diagnostic accumulation: all errors collected in one place
//!
//! # Example
//!
//! ```
//! use spl::session::CompileSession;
//!
//! let mut session = CompileSession::new("fn main() {}");
//!
//! // Access any phase - computed lazily, cached
//! let ast = session.ast();
//! let infer = session.infer();
//!
//! // Diagnostics accumulated across all phases
//! assert!(!session.has_errors());
//! ```

use crate::CompileResult;
use crate::ast::SourceFile;
use crate::diagnostic::{Diagnostic, Severity};
use crate::hir::HirDatabase;
use crate::mir::Body;
use crate::parser::Parse;
use crate::sema::infer::InferResult;
use crate::sema::resolver::ResolveResult;
use rowan::ast::AstNode;

/// Maximum number of diagnostics to report before stopping.
const MAX_DIAGNOSTICS: usize = 100;

/// Truncate diagnostics if they exceed the maximum, adding a warning.
fn truncate_diagnostics_if_needed(diagnostics: &mut Vec<Diagnostic>) {
    if diagnostics.len() > MAX_DIAGNOSTICS {
        diagnostics.truncate(MAX_DIAGNOSTICS);
        diagnostics.push(Diagnostic::warning(format!(
            "error limit reached ({MAX_DIAGNOSTICS} errors), stopping"
        )));
    }
}

/// Tracks which phases have been attempted (to avoid re-running failed phases).
#[derive(Debug, Clone, Copy, Default)]
struct PhaseAttempted {
    parse: bool,
    ast: bool,
    resolve: bool,
    infer: bool,
    hir: bool,
    mir: bool,
}

/// A compile session providing lazy, cached phase execution.
///
/// Each phase is computed only when accessed and cached for subsequent accesses.
/// If a prior phase fails, subsequent phases return `None`.
pub struct CompileSession<'src> {
    source: &'src str,
    diagnostics: Vec<Diagnostic>,
    // Cached phase results
    parse: Option<Parse>,
    ast: Option<SourceFile>,
    resolve: Option<ResolveResult>,
    infer: Option<InferResult>,
    hir: Option<HirDatabase>,
    mir: Option<Vec<Body>>,
    // Track which phases were attempted
    phase_attempted: PhaseAttempted,
}

impl<'src> CompileSession<'src> {
    /// Create a new compile session for the given source code.
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            diagnostics: Vec::new(),
            parse: None,
            ast: None,
            resolve: None,
            infer: None,
            hir: None,
            mir: None,
            phase_attempted: PhaseAttempted::default(),
        }
    }

    // ========================================================================
    // Phase Accessors (lazy, cached)
    // ========================================================================

    /// Get the parse result, computing if necessary.
    ///
    /// Returns `None` if parsing failed (errors will be in `diagnostics()`).
    pub fn parse(&mut self) -> Option<&Parse> {
        if !self.phase_attempted.parse {
            self.phase_attempted.parse = true;
            self.run_parse();
        }
        self.parse.as_ref()
    }

    /// Get the AST, computing if necessary.
    ///
    /// Returns `None` if parsing failed.
    pub fn ast(&mut self) -> Option<&SourceFile> {
        if !self.phase_attempted.ast {
            self.phase_attempted.ast = true;
            self.run_ast();
        }
        self.ast.as_ref()
    }

    /// Get the resolve result, computing if necessary.
    ///
    /// Returns `None` if a prior phase failed.
    pub fn resolve(&mut self) -> Option<&ResolveResult> {
        if !self.phase_attempted.resolve {
            self.phase_attempted.resolve = true;
            self.run_resolve();
        }
        self.resolve.as_ref()
    }

    /// Get the inference result, computing if necessary.
    ///
    /// Returns `None` if a prior phase failed.
    pub fn infer(&mut self) -> Option<&InferResult> {
        if !self.phase_attempted.infer {
            self.phase_attempted.infer = true;
            self.run_infer();
        }
        self.infer.as_ref()
    }

    /// Get the HIR database, computing if necessary.
    ///
    /// Returns `None` if a prior phase had errors (HIR lowering requires error-free input).
    pub fn hir(&mut self) -> Option<&HirDatabase> {
        if !self.phase_attempted.hir {
            self.phase_attempted.hir = true;
            self.run_hir();
        }
        self.hir.as_ref()
    }

    /// Get the MIR bodies, computing if necessary.
    ///
    /// Returns `None` if a prior phase had errors.
    pub fn mir(&mut self) -> Option<&[Body]> {
        if !self.phase_attempted.mir {
            self.phase_attempted.mir = true;
            self.run_mir();
        }
        self.mir.as_deref()
    }

    // ========================================================================
    // Diagnostics
    // ========================================================================

    /// Get all diagnostics accumulated so far.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Check if any error diagnostics have been produced.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Convert this session into a `CompileResult`.
    ///
    /// This runs the full pipeline and returns the result.
    pub fn into_result(mut self) -> CompileResult {
        // Run full pipeline
        let _ = self.mir();

        // Extract types from HIR if available
        let types = self.hir.map(|db| db.types);

        CompileResult {
            bodies: self.mir,
            types,
            diagnostics: self.diagnostics,
        }
    }

    // ========================================================================
    // Phase Runners
    // ========================================================================

    fn run_parse(&mut self) {
        let parse = crate::parser::parse(self.source);

        // Convert parse errors to diagnostics
        for error in parse.errors() {
            self.diagnostics
                .push(Diagnostic::error(&error.message).with_label(error.range.clone(), ""));
        }
        truncate_diagnostics_if_needed(&mut self.diagnostics);

        // Only cache if parsing succeeded
        if parse.ok() {
            self.parse = Some(parse);
        }
    }

    fn run_ast(&mut self) {
        // Ensure parse ran
        if self.parse.is_none() && !self.phase_attempted.parse {
            self.phase_attempted.parse = true;
            self.run_parse();
        }

        // Check if parse succeeded
        let Some(parse) = &self.parse else {
            return;
        };

        // Cast to AST
        if let Some(source_file) = SourceFile::cast(parse.syntax()) {
            self.ast = Some(source_file);
        } else {
            self.diagnostics
                .push(Diagnostic::error("failed to parse source file"));
        }
    }

    fn run_resolve(&mut self) {
        // Ensure AST is available
        if self.ast.is_none() && !self.phase_attempted.ast {
            self.phase_attempted.ast = true;
            self.run_ast();
        }

        let Some(source_file) = &self.ast else {
            return;
        };

        let mut result = crate::sema::resolve(source_file);
        self.diagnostics.append(&mut result.diagnostics);
        truncate_diagnostics_if_needed(&mut self.diagnostics);
        self.resolve = Some(result);
    }

    fn run_infer(&mut self) {
        // Ensure resolve ran
        if self.resolve.is_none() && !self.phase_attempted.resolve {
            self.phase_attempted.resolve = true;
            self.run_resolve();
        }

        let Some(source_file) = &self.ast else {
            return;
        };
        let Some(resolve_result) = &self.resolve else {
            return;
        };

        let mut result = crate::sema::infer(source_file, resolve_result);
        self.diagnostics.append(&mut result.diagnostics);
        truncate_diagnostics_if_needed(&mut self.diagnostics);
        self.infer = Some(result);
    }

    fn run_hir(&mut self) {
        // Ensure infer ran
        if self.infer.is_none() && !self.phase_attempted.infer {
            self.phase_attempted.infer = true;
            self.run_infer();
        }

        // Check for errors - HIR lowering requires error-free input
        if self.has_errors() {
            return;
        }

        let Some(source_file) = &self.ast else {
            return;
        };
        let Some(infer_result) = &self.infer else {
            return;
        };

        let hir_db = crate::hir::lower::lower_to_hir(source_file, infer_result);
        self.hir = Some(hir_db);
    }

    fn run_mir(&mut self) {
        // Ensure HIR ran
        if self.hir.is_none() && !self.phase_attempted.hir {
            self.phase_attempted.hir = true;
            self.run_hir();
        }

        let Some(hir_db) = &self.hir else {
            return;
        };

        match crate::mir::lower_hir_to_mir(hir_db) {
            Ok(bodies) => self.mir = Some(bodies),
            Err(ice) => {
                self.diagnostics.push(ice.to_diagnostic());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_new_creates_empty_session() {
        let session = CompileSession::new("fn main() {}");
        assert!(session.diagnostics().is_empty());
        assert!(!session.has_errors());
    }

    #[test]
    fn session_parse_returns_result() {
        let mut session = CompileSession::new("fn main() {}");
        assert!(session.parse().is_some());
    }

    #[test]
    fn session_parse_caches_result() {
        let mut session = CompileSession::new("fn main() {}");
        let p1 = session.parse().unwrap() as *const _;
        let p2 = session.parse().unwrap() as *const _;
        assert_eq!(p1, p2);
    }

    #[test]
    fn session_ast_none_when_parse_fails() {
        let mut session = CompileSession::new("@@@ invalid");
        assert!(session.ast().is_none());
        assert!(session.has_errors());
    }

    #[test]
    fn session_infer_computes_prior_phases() {
        let mut session = CompileSession::new("fn main() { let x = 1; }");
        let infer = session.infer();
        assert!(infer.is_some());
        // Prior phases should be cached
        assert!(session.parse().is_some());
        assert!(session.resolve().is_some());
    }

    #[test]
    fn session_hir_none_when_errors() {
        let mut session = CompileSession::new("fn main() { undefined; }");
        assert!(session.hir().is_none());
        assert!(session.has_errors());
    }

    #[test]
    fn session_mir_full_pipeline() {
        let mut session = CompileSession::new("fn main() {}");
        let mir = session.mir();
        assert!(mir.is_some());
        assert_eq!(mir.unwrap().len(), 1);
    }

    #[test]
    fn session_into_result_preserves_diagnostics() {
        let mut session = CompileSession::new("fn main() { undefined; }");
        let _ = session.mir();
        let result = session.into_result();
        assert!(!result.diagnostics.is_empty());
    }
}
