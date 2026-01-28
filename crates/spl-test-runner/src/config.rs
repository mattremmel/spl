//! Test configuration parsing.
//!
//! Defines the unified TOML configuration format for all test types.

use serde::Deserialize;

/// Test mode specifying what kind of validation to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TestMode {
    /// Package should load successfully (default for package tests).
    LoadPass,
    /// Package should fail to load.
    LoadFail,
    /// Code should compile successfully.
    CompilePass,
    /// Code should fail to compile.
    CompileFail,
    /// Code should compile and run successfully.
    #[default]
    RunPass,
    /// Code should compile but fail at runtime.
    RunFail,
}

/// Expectations for package loading.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PackageExpectations {
    /// Expected number of items.
    pub items: Option<usize>,
    /// Expected number of files.
    pub files: Option<usize>,
    /// Expected package name.
    pub name: Option<String>,
    /// Expected number of modules.
    pub modules: Option<usize>,
}

/// Expectations for compilation.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CompileExpectations {
    /// Expected error message pattern (for compile-fail).
    pub error: Option<String>,
    /// Multiple expected error patterns.
    #[serde(default)]
    pub errors: Vec<String>,
}

/// Expectations for program execution.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RunExpectations {
    /// Expected return value from `main()`.
    #[serde(rename = "return")]
    pub return_value: Option<i32>,
    /// Expected stdout content.
    pub stdout: Option<String>,
}

/// Source code specification.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Source {
    /// Inline source code.
    Inline { inline: String },
    /// Source from a file (relative to test.toml).
    File { file: String },
}

/// Unified test configuration.
///
/// Supports all test modes: load-pass, load-fail, compile-pass, compile-fail,
/// run-pass, and run-fail.
///
/// # Example
///
/// ```toml
/// mode = "run-pass"
///
/// [expect.run]
/// return = 42
///
/// [source]
/// inline = "fn main(): i32 { 42 }"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct TestConfig {
    /// Test mode (defaults to run-pass).
    #[serde(default)]
    pub mode: TestMode,

    /// Whether to ignore this test.
    #[serde(default)]
    pub ignore: bool,

    /// Expectations organized by phase.
    #[serde(default)]
    pub expect: Expectations,

    /// Source code for single-file tests.
    pub source: Option<Source>,
}

/// All expectations organized by phase.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Expectations {
    /// Package loading expectations.
    #[serde(default)]
    pub package: PackageExpectations,

    /// Compilation expectations.
    #[serde(default)]
    pub compile: CompileExpectations,

    /// Runtime expectations.
    #[serde(default)]
    pub run: RunExpectations,
}

impl TestConfig {
    /// Parse a test configuration from TOML.
    pub fn parse(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Get all expected compile error patterns.
    pub fn expected_compile_errors(&self) -> Vec<&str> {
        let mut errors: Vec<&str> = self
            .expect
            .compile
            .errors
            .iter()
            .map(String::as_str)
            .collect();
        if let Some(err) = &self.expect.compile.error {
            errors.push(err.as_str());
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_run_pass_inline() {
        let config = TestConfig::parse(
            r#"
            mode = "run-pass"
            [expect.run]
            return = 42
            [source]
            inline = "fn main(): i32 { 42 }"
        "#,
        )
        .unwrap();

        assert_eq!(config.mode, TestMode::RunPass);
        assert_eq!(config.expect.run.return_value, Some(42));
        assert!(matches!(config.source, Some(Source::Inline { .. })));
    }

    #[test]
    fn parse_compile_fail() {
        let config = TestConfig::parse(
            r#"
            mode = "compile-fail"
            [expect.compile]
            error = "cannot find"
        "#,
        )
        .unwrap();

        assert_eq!(config.mode, TestMode::CompileFail);
        assert_eq!(config.expect.compile.error.as_deref(), Some("cannot find"));
    }

    #[test]
    fn parse_load_pass() {
        let config = TestConfig::parse(
            r#"
            mode = "load-pass"
            [expect.package]
            items = 2
            files = 1
        "#,
        )
        .unwrap();

        assert_eq!(config.mode, TestMode::LoadPass);
        assert_eq!(config.expect.package.items, Some(2));
        assert_eq!(config.expect.package.files, Some(1));
    }

    #[test]
    fn parse_ignored() {
        let config = TestConfig::parse(
            r#"
            mode = "run-pass"
            ignore = true
        "#,
        )
        .unwrap();

        assert!(config.ignore);
    }

    #[test]
    fn default_mode_is_run_pass() {
        let config = TestConfig::parse(
            r#"
            [source]
            inline = "fn main() {}"
        "#,
        )
        .unwrap();

        assert_eq!(config.mode, TestMode::RunPass);
    }
}
