//! Re-exports from spl-diagnostic crate with compiler-internal extensions.
//!
//! This module re-exports all types from `spl_diagnostic` and adds
//! compiler-internal functionality like `FileId` tracking.

pub use spl_diagnostic::{
    DiagnosticRenderer, Label, RenderConfig, Severity, Span, render_diagnostic,
    render_diagnostic_plain,
};

use crate::package::FileId;
use std::path::PathBuf;

/// A diagnostic message with source location and annotations.
///
/// This is an extension of `spl_diagnostic::Diagnostic` that adds
/// compiler-internal `file_id` tracking for multi-file compilation.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The severity of the diagnostic.
    pub severity: Severity,
    /// The main error message.
    pub message: String,
    /// Labels pointing to specific spans in the source.
    pub labels: Vec<Label>,
    /// Additional notes providing context.
    pub notes: Vec<String>,
    /// Hints suggesting fixes.
    pub hints: Vec<String>,
    /// Optional file ID where the diagnostic originated.
    /// Used internally during compilation; converted to `file_path` for output.
    pub file_id: Option<FileId>,
    /// Optional file path where the diagnostic originated.
    /// Set for multi-file package compilation; `None` for single-file compilation.
    pub file_path: Option<PathBuf>,
}

impl Diagnostic {
    /// Create a new error diagnostic.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            hints: Vec::new(),
            file_id: None,
            file_path: None,
        }
    }

    /// Create a new warning diagnostic.
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            hints: Vec::new(),
            file_id: None,
            file_path: None,
        }
    }

    /// Set the file ID for this diagnostic (internal use during compilation).
    pub fn with_file_id(mut self, file_id: FileId) -> Self {
        self.file_id = Some(file_id);
        self
    }

    /// Set the file path for this diagnostic.
    pub fn with_file_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Add a primary label at the given span.
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::primary(span, message));
        self
    }

    /// Add a secondary label at the given span.
    pub fn with_secondary_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::secondary(span, message));
        self
    }

    /// Add a note to the diagnostic.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Add a hint to the diagnostic.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hints.push(hint.into());
        self
    }

    /// Convert to the base `spl_diagnostic::Diagnostic` for rendering.
    #[must_use]
    pub fn to_base(&self) -> spl_diagnostic::Diagnostic {
        let mut base = if self.severity == Severity::Error {
            spl_diagnostic::Diagnostic::error(&self.message)
        } else {
            spl_diagnostic::Diagnostic::warning(&self.message)
        };

        for label in &self.labels {
            if label.primary {
                base = base.with_label(label.span.clone(), &label.message);
            } else {
                base = base.with_secondary_label(label.span.clone(), &label.message);
            }
        }

        for note in &self.notes {
            base = base.with_note(note);
        }

        for hint in &self.hints {
            base = base.with_hint(hint);
        }

        if let Some(path) = &self.file_path {
            base = base.with_file_path(path.clone());
        }

        base
    }
}

impl From<spl_diagnostic::Diagnostic> for Diagnostic {
    fn from(base: spl_diagnostic::Diagnostic) -> Self {
        Self {
            severity: base.severity,
            message: base.message,
            labels: base.labels,
            notes: base.notes,
            hints: base.hints,
            file_id: None,
            file_path: base.file_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_builder_error() {
        let diag = Diagnostic::error("unexpected token")
            .with_label(5..10, "found here")
            .with_note("expected an identifier")
            .with_hint("try using a valid name");

        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "unexpected token");
        assert_eq!(diag.labels.len(), 1);
        assert!(diag.labels[0].primary);
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.hints.len(), 1);
    }

    #[test]
    fn diagnostic_builder_warning() {
        let diag = Diagnostic::warning("unused variable");
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn diagnostic_multiple_labels() {
        let diag = Diagnostic::error("test")
            .with_label(0..5, "primary")
            .with_secondary_label(10..15, "secondary");

        assert_eq!(diag.labels.len(), 2);
        assert!(diag.labels[0].primary);
        assert!(!diag.labels[1].primary);
    }

    #[test]
    fn diagnostic_to_base_conversion() {
        let diag = Diagnostic::error("test error")
            .with_label(0..5, "here")
            .with_note("a note")
            .with_hint("a hint")
            .with_file_path("/test/file.spl");

        let base = diag.to_base();
        assert_eq!(base.severity, Severity::Error);
        assert_eq!(base.message, "test error");
        assert_eq!(base.labels.len(), 1);
        assert_eq!(base.notes.len(), 1);
        assert_eq!(base.hints.len(), 1);
        assert!(base.file_path.is_some());
    }

    #[test]
    fn severity_as_str() {
        assert_eq!(Severity::Error.as_str(), "error");
        assert_eq!(Severity::Warning.as_str(), "warning");
    }

    #[test]
    fn label_primary() {
        let label = Label::primary(5..10, "test message");
        assert_eq!(label.span, 5..10);
        assert_eq!(label.message, "test message");
        assert!(label.primary);
    }

    #[test]
    fn label_secondary() {
        let label = Label::secondary(5..10, "test message");
        assert_eq!(label.span, 5..10);
        assert_eq!(label.message, "test message");
        assert!(!label.primary);
    }

    #[test]
    fn render_config_defaults() {
        let config = RenderConfig::default();
        assert!(config.colors);
        assert!(config.file_name.is_none());
    }

    #[test]
    fn render_config_builder() {
        let config = RenderConfig::new()
            .with_colors(false)
            .with_file_name("my_file.spl");
        assert!(!config.colors);
        assert_eq!(config.file_name, Some("my_file.spl".to_string()));
    }
}
