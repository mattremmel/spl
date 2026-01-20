//! Diagnostic reporting for SPL compiler errors.
//!
//! Provides rich error messages with source code snippets, colored output,
//! and support for multiple notes and hints.
//!
//! # Example
//!
//! ```text
//! error: expected ';' after statement
//!   --> src/main.spl:3:15
//!    |
//!  3 |     let x = 42
//!    |               ^ expected ';'
//!    |
//!    = note: statements must end with a semicolon
//!    = hint: add ';' here
//! ```

use crate::Span;
use std::fmt;
use std::io::{self, Write};

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
}

/// A labeled span within the source code.
#[derive(Debug, Clone)]
pub struct Label {
    /// The byte range in the source.
    pub span: Span,
    /// The message to display under the span.
    pub message: String,
    /// Whether this is the primary label (shown with '^') or secondary ('~').
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
}

/// ANSI color codes for terminal output.
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const RED: &str = "\x1b[31m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const CYAN: &str = "\x1b[36m";
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

/// Source code information for rendering diagnostics.
pub struct SourceCode<'a> {
    /// The full source code.
    source: &'a str,
    /// Line start offsets (byte positions).
    line_starts: Vec<usize>,
}

impl<'a> SourceCode<'a> {
    /// Create a new source code wrapper.
    pub fn new(source: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (i, c) in source.char_indices() {
            if c == '\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            source,
            line_starts,
        }
    }

    /// Get the line number (1-indexed) for a byte offset.
    pub fn line_number(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(line) => line + 1,
            Err(line) => line,
        }
    }

    /// Get the column number (1-indexed) for a byte offset.
    pub fn column_number(&self, offset: usize) -> usize {
        let line = self.line_number(offset);
        let line_start = self.line_starts[line - 1];
        offset - line_start + 1
    }

    /// Get the content of a line (0-indexed internally, but takes 1-indexed line number).
    pub fn line_content(&self, line: usize) -> &'a str {
        if line == 0 || line > self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[line - 1];
        let end = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.source.len());
        // Trim trailing newline
        self.source[start..end].trim_end_matches('\n').trim_end_matches('\r')
    }

    /// Get the number of lines.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

/// Renderer for diagnostics.
pub struct DiagnosticRenderer<'a> {
    source: &'a SourceCode<'a>,
    config: RenderConfig,
}

impl<'a> DiagnosticRenderer<'a> {
    /// Create a new diagnostic renderer.
    pub fn new(source: &'a SourceCode<'a>, config: RenderConfig) -> Self {
        Self { source, config }
    }

    /// Render a diagnostic to a string.
    pub fn render(&self, diagnostic: &Diagnostic) -> String {
        let mut output = String::new();
        self.render_to(&mut output, diagnostic).unwrap();
        output
    }

    /// Render a diagnostic to any Write implementation.
    pub fn render_to<W: fmt::Write>(&self, w: &mut W, diagnostic: &Diagnostic) -> fmt::Result {
        // Render header: "error: message"
        self.render_header(w, diagnostic)?;

        // Render source location and snippets
        if !diagnostic.labels.is_empty() {
            self.render_source_snippet(w, diagnostic)?;
        }

        // Render notes
        for note in &diagnostic.notes {
            self.render_note(w, note)?;
        }

        // Render hints
        for hint in &diagnostic.hints {
            self.render_hint(w, hint)?;
        }

        Ok(())
    }

    /// Write the diagnostic to stderr.
    pub fn eprint(&self, diagnostic: &Diagnostic) -> io::Result<()> {
        let rendered = self.render(diagnostic);
        eprint!("{}", rendered);
        io::stderr().flush()
    }

    fn render_header<W: fmt::Write>(&self, w: &mut W, diagnostic: &Diagnostic) -> fmt::Result {
        if self.config.colors {
            let color = match diagnostic.severity {
                Severity::Error => colors::RED,
                Severity::Warning => colors::YELLOW,
            };
            writeln!(
                w,
                "{}{}{}{}: {}{}",
                colors::BOLD,
                color,
                diagnostic.severity.as_str(),
                colors::RESET,
                colors::BOLD,
                diagnostic.message,
            )?;
            write!(w, "{}", colors::RESET)?;
        } else {
            writeln!(w, "{}: {}", diagnostic.severity.as_str(), diagnostic.message)?;
        }
        Ok(())
    }

    fn render_source_snippet<W: fmt::Write>(
        &self,
        w: &mut W,
        diagnostic: &Diagnostic,
    ) -> fmt::Result {
        // Find the primary label for the location header
        let primary_label = diagnostic
            .labels
            .iter()
            .find(|l| l.primary)
            .or(diagnostic.labels.first());

        if let Some(label) = primary_label {
            let line = self.source.line_number(label.span.start);
            let col = self.source.column_number(label.span.start);

            // Location header: "  --> file:line:col"
            let file = self.config.file_name.as_deref().unwrap_or("<input>");
            if self.config.colors {
                writeln!(
                    w,
                    "  {}-->{}  {}:{}:{}",
                    colors::BLUE,
                    colors::RESET,
                    file,
                    line,
                    col
                )?;
            } else {
                writeln!(w, "  --> {}:{}:{}", file, line, col)?;
            }
        }

        // Group labels by line
        let mut labels_by_line: std::collections::BTreeMap<usize, Vec<&Label>> =
            std::collections::BTreeMap::new();
        for label in &diagnostic.labels {
            let line = self.source.line_number(label.span.start);
            labels_by_line.entry(line).or_default().push(label);
        }

        // Calculate the width needed for line numbers
        let max_line = labels_by_line.keys().max().copied().unwrap_or(1);
        let line_num_width = max_line.to_string().len();

        // Render each line with its labels
        for (line_num, labels) in &labels_by_line {
            let line_content = self.source.line_content(*line_num);

            // Empty line before snippet
            self.render_line_prefix(w, None, line_num_width)?;
            writeln!(w)?;

            // Source line: " 3 |     let x = 42"
            self.render_line_prefix(w, Some(*line_num), line_num_width)?;
            writeln!(w, "{}", line_content)?;

            // Underline and message
            self.render_underlines(w, line_num_width, *line_num, labels, line_content)?;
        }

        // Empty line after snippet
        self.render_line_prefix(w, None, line_num_width)?;
        writeln!(w)?;

        Ok(())
    }

    fn render_line_prefix<W: fmt::Write>(
        &self,
        w: &mut W,
        line_num: Option<usize>,
        width: usize,
    ) -> fmt::Result {
        if self.config.colors {
            write!(w, "{}", colors::BLUE)?;
        }

        if let Some(num) = line_num {
            write!(w, "{:>width$} | ", num, width = width)?;
        } else {
            write!(w, "{:>width$} | ", "", width = width)?;
        }

        if self.config.colors {
            write!(w, "{}", colors::RESET)?;
        }

        Ok(())
    }

    fn render_underlines<W: fmt::Write>(
        &self,
        w: &mut W,
        line_num_width: usize,
        line_num: usize,
        labels: &[&Label],
        line_content: &str,
    ) -> fmt::Result {
        let line_start = self.source.line_starts[line_num - 1];

        // Sort labels by start position
        let mut sorted_labels: Vec<_> = labels.iter().collect();
        sorted_labels.sort_by_key(|l| l.span.start);

        for label in sorted_labels {
            // Calculate column positions
            let start_col = label.span.start.saturating_sub(line_start);
            let end_col = label
                .span
                .end
                .saturating_sub(line_start)
                .min(line_content.len());
            let underline_len = end_col.saturating_sub(start_col).max(1);

            // Render underline line
            self.render_line_prefix(w, None, line_num_width)?;

            // Spaces before underline
            write!(w, "{:width$}", "", width = start_col)?;

            // Underline character
            let underline_char = if label.primary { '^' } else { '~' };
            let underline: String = std::iter::repeat_n(underline_char, underline_len).collect();

            if self.config.colors {
                let color = if label.primary {
                    colors::RED
                } else {
                    colors::BLUE
                };
                write!(w, "{}{}{}", color, underline, colors::RESET)?;

                if !label.message.is_empty() {
                    write!(w, " {}{}{}", color, label.message, colors::RESET)?;
                }
            } else {
                write!(w, "{}", underline)?;
                if !label.message.is_empty() {
                    write!(w, " {}", label.message)?;
                }
            }

            writeln!(w)?;
        }

        Ok(())
    }

    fn render_note<W: fmt::Write>(&self, w: &mut W, note: &str) -> fmt::Result {
        if self.config.colors {
            writeln!(
                w,
                "  {} = {}note: {}{}",
                colors::BLUE,
                colors::RESET,
                note,
                colors::RESET
            )
        } else {
            writeln!(w, "   = note: {}", note)
        }
    }

    fn render_hint<W: fmt::Write>(&self, w: &mut W, hint: &str) -> fmt::Result {
        if self.config.colors {
            writeln!(
                w,
                "  {} = {}{}hint{}: {}",
                colors::BLUE,
                colors::RESET,
                colors::CYAN,
                colors::RESET,
                hint
            )
        } else {
            writeln!(w, "   = hint: {}", hint)
        }
    }
}

/// Convenience function to render a diagnostic with default settings.
pub fn render_diagnostic(source: &str, diagnostic: &Diagnostic) -> String {
    let source_code = SourceCode::new(source);
    let renderer = DiagnosticRenderer::new(&source_code, RenderConfig::default());
    renderer.render(diagnostic)
}

/// Convenience function to render a diagnostic without colors.
pub fn render_diagnostic_plain(source: &str, diagnostic: &Diagnostic) -> String {
    let source_code = SourceCode::new(source);
    let config = RenderConfig::new().with_colors(false);
    let renderer = DiagnosticRenderer::new(&source_code, config);
    renderer.render(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::{expect, Expect};

    /// Helper to render a diagnostic and compare against expected output.
    fn check(source: &str, diagnostic: &Diagnostic, expected: &Expect) {
        let source_code = SourceCode::new(source);
        let config = RenderConfig::new()
            .with_colors(false)
            .with_file_name("test.spl");
        let renderer = DiagnosticRenderer::new(&source_code, config);
        let output = renderer.render(diagnostic);
        expected.assert_eq(&output);
    }

    // === SourceCode unit tests ===

    #[test]
    fn source_code_line_numbers() {
        let source = "line 1\nline 2\nline 3";
        let sc = SourceCode::new(source);

        assert_eq!(sc.line_count(), 3);
        assert_eq!(sc.line_number(0), 1); // 'l' of line 1
        assert_eq!(sc.line_number(6), 1); // '\n' after line 1
        assert_eq!(sc.line_number(7), 2); // 'l' of line 2
        assert_eq!(sc.line_number(14), 3); // 'l' of line 3
    }

    #[test]
    fn source_code_column_numbers() {
        let source = "let x = 42;";
        let sc = SourceCode::new(source);

        assert_eq!(sc.column_number(0), 1); // 'l'
        assert_eq!(sc.column_number(4), 5); // 'x'
        assert_eq!(sc.column_number(8), 9); // '4'
    }

    #[test]
    fn source_code_line_content() {
        let source = "fn main() {\n    let x = 42;\n}";
        let sc = SourceCode::new(source);

        assert_eq!(sc.line_content(1), "fn main() {");
        assert_eq!(sc.line_content(2), "    let x = 42;");
        assert_eq!(sc.line_content(3), "}");
    }

    #[test]
    fn source_code_empty_lines() {
        let source = "line 1\n\nline 3";
        let sc = SourceCode::new(source);

        assert_eq!(sc.line_count(), 3);
        assert_eq!(sc.line_content(1), "line 1");
        assert_eq!(sc.line_content(2), "");
        assert_eq!(sc.line_content(3), "line 3");
    }

    #[test]
    fn source_code_single_line() {
        let source = "single line";
        let sc = SourceCode::new(source);

        assert_eq!(sc.line_count(), 1);
        assert_eq!(sc.line_number(0), 1);
        assert_eq!(sc.line_number(5), 1);
        assert_eq!(sc.line_content(1), "single line");
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
        let diag = Diagnostic::error("expected expression")
            .with_label(8..9, "unexpected ';'");

        check(
            source,
            &diag,
            &expect![[r#"
                error: expected expression
                  --> test.spl:1:9
                  | 
                1 | let x = ;
                  |         ^ unexpected ';'
                  | 
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
                error: expected identifier
                  --> test.spl:1:5
                  | 
                1 | let 123 = x;
                  |     ^^^ invalid identifier
                  | 
                   = note: identifiers cannot start with a digit
            "#]],
        );
    }

    #[test]
    fn render_error_with_hint() {
        let source = "let x = 42";
        let diag = Diagnostic::error("missing semicolon")
            .with_label(10..10, "expected ';'")
            .with_hint("add ';' at the end of the statement");

        check(
            source,
            &diag,
            &expect![[r#"
                error: missing semicolon
                  --> test.spl:1:11
                  | 
                1 | let x = 42
                  |           ^ expected ';'
                  | 
                   = hint: add ';' at the end of the statement
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
                error: expected '(' after function name
                  --> test.spl:1:8
                  | 
                1 | fn foo {}
                  |        ^ unexpected '{'
                  | 
                   = note: function definitions require a parameter list
                   = hint: add '()' before '{'
            "#]],
        );
    }

    #[test]
    fn render_error_with_multiple_notes() {
        let source = "println(x);";
        let diag = Diagnostic::error("cannot find function 'println'")
            .with_label(0..7, "not found in this scope")
            .with_note("there is no function named 'println'")
            .with_note("the standard library provides 'print' instead")
            .with_hint("use 'print' from std::io");

        check(
            source,
            &diag,
            &expect![[r#"
                error: cannot find function 'println'
                  --> test.spl:1:1
                  | 
                1 | println(x);
                  | ^^^^^^^ not found in this scope
                  | 
                   = note: there is no function named 'println'
                   = note: the standard library provides 'print' instead
                   = hint: use 'print' from std::io
            "#]],
        );
    }

    #[test]
    fn render_error_with_multiple_hints() {
        let source = "let mut = 5;";
        let diag = Diagnostic::error("expected identifier after 'mut'")
            .with_label(8..9, "unexpected '='")
            .with_hint("add a variable name: 'let mut x = 5;'")
            .with_hint("or remove 'mut' if not needed: 'let x = 5;'");

        check(
            source,
            &diag,
            &expect![[r#"
                error: expected identifier after 'mut'
                  --> test.spl:1:9
                  | 
                1 | let mut = 5;
                  |         ^ unexpected '='
                  | 
                   = hint: add a variable name: 'let mut x = 5;'
                   = hint: or remove 'mut' if not needed: 'let x = 5;'
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
                warning: unused variable '_unused'
                  --> test.spl:1:5
                  | 
                1 | let _unused = 42;
                  |     ^^^^^^^ this variable is never used
                  | 
            "#]],
        );
    }

    #[test]
    fn render_multiline_source() {
        let source = "fn main() {\n    let x = ;\n}";
        // ';' is at position 24 (after "fn main() {\n    let x = ")
        let diag = Diagnostic::error("expected expression")
            .with_label(24..25, "unexpected ';'");

        check(
            source,
            &diag,
            &expect![[r#"
                error: expected expression
                  --> test.spl:2:13
                  | 
                2 |     let x = ;
                  |             ^ unexpected ';'
                  | 
            "#]],
        );
    }

    #[test]
    fn render_error_on_line_three() {
        let source = "fn main() {\n    let x = 1;\n    let y = ;\n}";
        // ';' on line 3 is at position 40
        let diag = Diagnostic::error("expected expression")
            .with_label(40..41, "unexpected ';'");

        check(
            source,
            &diag,
            &expect![[r#"
                error: expected expression
                  --> test.spl:3:14
                  | 
                3 |     let y = ;
                  |              ^ unexpected ';'
                  | 
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
                error: expected expression after '+'
                  --> test.spl:1:13
                  | 
                1 | let x = y + ;
                  |           ~ operator here
                  |             ^ unexpected ';'
                  | 
            "#]],
        );
    }

    #[test]
    fn render_wide_underline() {
        let source = "let very_long_identifier = something_else;";
        let diag = Diagnostic::error("unknown identifier")
            .with_label(27..41, "not found in this scope");

        check(
            source,
            &diag,
            &expect![[r#"
                error: unknown identifier
                  --> test.spl:1:28
                  | 
                1 | let very_long_identifier = something_else;
                  |                            ^^^^^^^^^^^^^^ not found in this scope
                  | 
            "#]],
        );
    }

    #[test]
    fn render_label_without_message() {
        let source = "let x = ;";
        let diag = Diagnostic::error("expected expression")
            .with_label(8..9, "");

        check(
            source,
            &diag,
            &expect![[r#"
                error: expected expression
                  --> test.spl:1:9
                  | 
                1 | let x = ;
                  |         ^
                  | 
            "#]],
        );
    }

    #[test]
    fn render_error_at_start_of_line() {
        let source = "fn main() {\n@invalid\n}";
        let diag = Diagnostic::error("unexpected character '@'")
            .with_label(12..13, "invalid character");

        check(
            source,
            &diag,
            &expect![[r#"
                error: unexpected character '@'
                  --> test.spl:2:1
                  | 
                2 | @invalid
                  | ^ invalid character
                  | 
            "#]],
        );
    }

    #[test]
    fn render_error_at_end_of_line() {
        let source = "let x = 42";
        let diag = Diagnostic::error("missing semicolon")
            .with_label(9..10, "expected ';' after this");

        check(
            source,
            &diag,
            &expect![[r#"
                error: missing semicolon
                  --> test.spl:1:10
                  | 
                1 | let x = 42
                  |          ^ expected ';' after this
                  | 
            "#]],
        );
    }

    #[test]
    fn render_double_digit_line_numbers() {
        let source = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12";
        let diag = Diagnostic::error("error on line 12")
            .with_label(21..23, "here");

        check(
            source,
            &diag,
            &expect![[r#"
                error: error on line 12
                  --> test.spl:11:1
                   | 
                11 | 11
                   | ^^ here
                   | 
            "#]],
        );
    }

    #[test]
    fn render_no_labels() {
        let source = "let x = 42;";
        let diag = Diagnostic::error("general error")
            .with_note("this is a general note");

        check(
            source,
            &diag,
            &expect![[r#"
                error: general error
                   = note: this is a general note
            "#]],
        );
    }

    #[test]
    fn render_empty_source() {
        let source = "";
        let diag = Diagnostic::error("unexpected end of file")
            .with_note("expected a function or type definition");

        check(
            source,
            &diag,
            &expect![[r#"
                error: unexpected end of file
                   = note: expected a function or type definition
            "#]],
        );
    }

    #[test]
    fn render_indented_code() {
        let source = "fn main() {\n        let deeply_indented = 42;\n}";
        let diag = Diagnostic::warning("unused variable")
            .with_label(20..36, "never used");

        check(
            source,
            &diag,
            &expect![[r#"
                warning: unused variable
                  --> test.spl:2:9
                  | 
                2 |         let deeply_indented = 42;
                  |         ^^^^^^^^^^^^^^^^ never used
                  | 
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
