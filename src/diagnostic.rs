//! Diagnostic reporting for SPL compiler errors.
//!
//! Provides rich error messages with source code snippets using ariadne.
//!
//! # Example
//!
//! ```text
//! Error: expected ';' after statement
//!    ╭─[src/main.spl:3:15]
//!    │
//!  3 │     let x = 42
//!    │               ┬
//!    │               ╰── expected ';'
//! ───╯
//! ```

use crate::Span;
use ariadne::{Color, Config, Label as AriadneLabel, Report, ReportKind, Source};
use std::io;

/// Severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A fatal error that prevents compilation.
    Error,
    /// A warning that doesn't prevent compilation.
    Warning,
}

impl Severity {
    /// Returns the display name of the severity.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }

    fn to_report_kind(self) -> ReportKind<'static> {
        match self {
            Severity::Error => ReportKind::Error,
            Severity::Warning => ReportKind::Warning,
        }
    }
}

/// A labeled span within the source code.
#[derive(Debug, Clone)]
pub struct Label {
    /// The byte range in the source.
    pub span: Span,
    /// The message to display under the span.
    pub message: String,
    /// Whether this is the primary label or secondary.
    pub primary: bool,
}

impl Label {
    /// Create a new primary label.
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            primary: true,
        }
    }

    /// Create a new secondary label.
    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            primary: false,
        }
    }
}

/// A diagnostic message with source location and annotations.
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
        }
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

    /// Build an ariadne Report from this diagnostic.
    fn to_report<'a>(&self, file_name: &'a str, config: Config) -> Report<'a, (&'a str, Span)> {
        // Find the primary span for the report header
        let primary_span = self.labels.first().map(|l| l.span.clone()).unwrap_or(0..0);

        let mut builder = Report::build(self.severity.to_report_kind(), (file_name, primary_span))
            .with_config(config)
            .with_message(&self.message);

        // Add labels
        for label in &self.labels {
            let color = if label.primary {
                match self.severity {
                    Severity::Error => Color::Red,
                    Severity::Warning => Color::Yellow,
                }
            } else {
                Color::Blue
            };

            let ariadne_label =
                AriadneLabel::new((file_name, label.span.clone())).with_color(color);

            let ariadne_label = if !label.message.is_empty() {
                ariadne_label.with_message(&label.message)
            } else {
                ariadne_label
            };

            builder = builder.with_label(ariadne_label);
        }

        // Add notes
        for note in &self.notes {
            builder = builder.with_note(note);
        }

        // Add hints (ariadne calls these "help")
        for hint in &self.hints {
            builder = builder.with_help(hint);
        }

        builder.finish()
    }
}

/// Configuration for diagnostic rendering.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Whether to use colors in output.
    pub colors: bool,
    /// The file name to display in the header.
    pub file_name: Option<String>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            colors: true,
            file_name: None,
        }
    }
}

impl RenderConfig {
    /// Create a new render config with colors enabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to use colors.
    pub fn with_colors(mut self, colors: bool) -> Self {
        self.colors = colors;
        self
    }

    /// Set the file name to display.
    pub fn with_file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }
}

/// Renderer for diagnostics.
pub struct DiagnosticRenderer {
    source: String,
    config: RenderConfig,
}

impl DiagnosticRenderer {
    /// Create a new diagnostic renderer.
    pub fn new(source: &str, config: RenderConfig) -> Self {
        Self {
            source: source.to_string(),
            config,
        }
    }

    /// Render a diagnostic to a string.
    pub fn render(&self, diagnostic: &Diagnostic) -> String {
        let file_name = self.config.file_name.as_deref().unwrap_or("<input>");
        let ariadne_config = Config::default().with_color(self.config.colors);
        let report = diagnostic.to_report(file_name, ariadne_config);

        let mut output = Vec::new();
        report
            .write((file_name, Source::from(&self.source)), &mut output)
            .expect("failed to write diagnostic");

        String::from_utf8(output).expect("diagnostic output is not valid UTF-8")
    }

    /// Write the diagnostic to stderr.
    pub fn eprint(&self, diagnostic: &Diagnostic) -> io::Result<()> {
        let file_name = self.config.file_name.as_deref().unwrap_or("<input>");
        let ariadne_config = Config::default().with_color(self.config.colors);
        let report = diagnostic.to_report(file_name, ariadne_config);

        report
            .eprint((file_name, Source::from(&self.source)))
            .map_err(io::Error::other)
    }
}

/// Convenience function to render a diagnostic with default settings.
pub fn render_diagnostic(source: &str, diagnostic: &Diagnostic) -> String {
    let renderer = DiagnosticRenderer::new(source, RenderConfig::default());
    renderer.render(diagnostic)
}

/// Convenience function to render a diagnostic without colors.
pub fn render_diagnostic_plain(source: &str, diagnostic: &Diagnostic) -> String {
    let config = RenderConfig::new().with_colors(false);
    let renderer = DiagnosticRenderer::new(source, config);
    renderer.render(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::{Expect, expect};

    /// Helper to render a diagnostic and compare against expected output.
    fn check(source: &str, diagnostic: &Diagnostic, expected: &Expect) {
        let config = RenderConfig::new()
            .with_colors(false)
            .with_file_name("test.spl");
        let renderer = DiagnosticRenderer::new(source, config);
        let output = renderer.render(diagnostic);
        expected.assert_eq(&output);
    }

    // === Diagnostic builder tests ===

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

    // === Render output tests with expect-test ===

    #[test]
    fn render_simple_error() {
        let source = "let x = ;";
        let diag = Diagnostic::error("expected expression").with_label(8..9, "unexpected ';'");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: expected expression
                   ╭─[ test.spl:1:9 ]
                   │
                 1 │ let x = ;
                   │         ┬  
                   │         ╰── unexpected ';'
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_error_with_note() {
        let source = "let 123 = x;";
        let diag = Diagnostic::error("expected identifier")
            .with_label(4..7, "invalid identifier")
            .with_note("identifiers cannot start with a digit");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: expected identifier
                   ╭─[ test.spl:1:5 ]
                   │
                 1 │ let 123 = x;
                   │     ─┬─  
                   │      ╰─── invalid identifier
                   │ 
                   │ Note: identifiers cannot start with a digit
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_error_with_hint() {
        let source = "let x = 42";
        let diag = Diagnostic::error("missing semicolon")
            .with_label(8..10, "expected ';' after this")
            .with_hint("add ';' at the end of the statement");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: missing semicolon
                   ╭─[ test.spl:1:9 ]
                   │
                 1 │ let x = 42
                   │         ─┬  
                   │          ╰── expected ';' after this
                   │ 
                   │ Help: add ';' at the end of the statement
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_error_with_note_and_hint() {
        let source = "fn foo {}";
        let diag = Diagnostic::error("expected '(' after function name")
            .with_label(7..8, "unexpected '{'")
            .with_note("function definitions require a parameter list")
            .with_hint("add '()' before '{'");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: expected '(' after function name
                   ╭─[ test.spl:1:8 ]
                   │
                 1 │ fn foo {}
                   │        ┬  
                   │        ╰── unexpected '{'
                   │ 
                   │ Help: add '()' before '{'
                   │ 
                   │ Note: function definitions require a parameter list
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_warning() {
        let source = "let _unused = 42;";
        let diag = Diagnostic::warning("unused variable '_unused'")
            .with_label(4..11, "this variable is never used");

        check(
            source,
            &diag,
            &expect![[r#"
                Warning: unused variable '_unused'
                   ╭─[ test.spl:1:5 ]
                   │
                 1 │ let _unused = 42;
                   │     ───┬───  
                   │        ╰───── this variable is never used
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_multiline_source() {
        let source = "fn main() {\n    let x = ;\n}";
        // ';' is at position 24 (after "fn main() {\n    let x = ")
        let diag = Diagnostic::error("expected expression").with_label(24..25, "unexpected ';'");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: expected expression
                   ╭─[ test.spl:2:13 ]
                   │
                 2 │     let x = ;
                   │             ┬  
                   │             ╰── unexpected ';'
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_multiple_labels_same_line() {
        let source = "let x = y + ;";
        let diag = Diagnostic::error("expected expression after '+'")
            .with_label(12..13, "unexpected ';'")
            .with_secondary_label(10..11, "operator here");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: expected expression after '+'
                   ╭─[ test.spl:1:13 ]
                   │
                 1 │ let x = y + ;
                   │           ┬ ┬  
                   │           ╰──── operator here
                   │             │  
                   │             ╰── unexpected ';'
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_wide_underline() {
        let source = "let very_long_identifier = something_else;";
        let diag =
            Diagnostic::error("unknown identifier").with_label(27..41, "not found in this scope");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: unknown identifier
                   ╭─[ test.spl:1:28 ]
                   │
                 1 │ let very_long_identifier = something_else;
                   │                            ───────┬──────  
                   │                                   ╰──────── not found in this scope
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_label_without_message() {
        let source = "let x = ;";
        let diag = Diagnostic::error("expected expression").with_label(8..9, "");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: expected expression
                   ╭─[ test.spl:1:9 ]
                   │
                 1 │ let x = ;
                ───╯
            "#]],
        );
    }

    #[test]
    fn render_no_labels() {
        let source = "let x = 42;";
        let diag = Diagnostic::error("general error").with_note("this is a general note");

        check(
            source,
            &diag,
            &expect![[r#"
                Error: general error
            "#]],
        );
    }

    // === Test severity display ===

    #[test]
    fn severity_as_str() {
        assert_eq!(Severity::Error.as_str(), "error");
        assert_eq!(Severity::Warning.as_str(), "warning");
    }

    // === Test Label constructors ===

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

    // === Test RenderConfig ===

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
