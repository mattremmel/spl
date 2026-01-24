//! Parsing of `_package.spl` directives.
//!
//! Package configuration is specified via inner attributes in a `_package.spl` file:
//!
//! ```text
//! // Package metadata
//! #![name("my_package")]
//!
//! // File inclusion directives
//! #![no_auto_include]
//! #![include("lib.spl")]
//! #![exclude("tests.spl")]
//! #![include_if(debug, "debug.spl")]
//! #![exclude_if(prod, "test_utils.spl")]
//!
//! // Subpackage inclusion directives
//! #![no_auto_include_packages]
//! #![include_package("utils")]
//! #![exclude_package("benchmarks")]
//! #![include_package_if(debug, "debug_tools")]
//! #![exclude_package_if(prod, "dev_utils")]
//! ```

use crate::ast::{InnerAttribute, SourceFile};
use crate::parser;
use rowan::ast::AstNode;
use std::fmt;

/// Errors that can occur when parsing package directives.
#[derive(Debug, Clone)]
pub enum DirectiveError {
    /// Parse error in the _package.spl file.
    ParseError(String),
    /// Malformed directive (e.g., missing argument).
    MalformedDirective { directive: String, reason: String },
}

impl fmt::Display for DirectiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DirectiveError::ParseError(msg) => write!(f, "parse error: {}", msg),
            DirectiveError::MalformedDirective { directive, reason } => {
                write!(f, "malformed directive '{}': {}", directive, reason)
            }
        }
    }
}

impl std::error::Error for DirectiveError {}

/// Parsed package directives from a `_package.spl` file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PackageDirectives {
    /// Package name override.
    pub name: Option<String>,
    /// If true, no files are auto-included; all must be explicitly listed.
    pub no_auto_include: bool,
    /// Files to explicitly include.
    pub includes: Vec<String>,
    /// Files to exclude from auto-include.
    pub excludes: Vec<String>,
    /// Conditional includes: (condition, file).
    pub conditional_includes: Vec<(String, String)>,
    /// Conditional excludes: (condition, file).
    pub conditional_excludes: Vec<(String, String)>,

    // --- Subpackage directives ---
    /// If true, no subpackages are auto-included; all must be explicitly listed.
    pub no_auto_include_packages: bool,
    /// Subpackages to explicitly include.
    pub package_includes: Vec<String>,
    /// Subpackages to exclude from auto-include.
    pub package_excludes: Vec<String>,
    /// Conditional subpackage includes: (condition, package).
    pub conditional_package_includes: Vec<(String, String)>,
    /// Conditional subpackage excludes: (condition, package).
    pub conditional_package_excludes: Vec<(String, String)>,
}

/// Parse package directives from source text.
///
/// Returns the directives and any parse errors. Unknown directives are ignored.
pub fn parse_package_directives(source: &str) -> Result<PackageDirectives, DirectiveError> {
    let parse = parser::parse(source);

    if !parse.ok() {
        let errors: Vec<_> = parse.errors().iter().map(|e| e.message.clone()).collect();
        return Err(DirectiveError::ParseError(errors.join("; ")));
    }

    let Some(source_file) = SourceFile::cast(parse.syntax()) else {
        return Err(DirectiveError::ParseError(
            "failed to parse source file".to_string(),
        ));
    };

    let mut directives = PackageDirectives::default();

    for attr in source_file.inner_attributes() {
        process_attribute(&attr, &mut directives)?;
    }

    Ok(directives)
}

fn process_attribute(
    attr: &InnerAttribute,
    directives: &mut PackageDirectives,
) -> Result<(), DirectiveError> {
    let Some(path) = attr.path() else {
        return Ok(()); // Ignore attributes without paths
    };

    let name = path.path_string();

    match name.as_str() {
        "name" => {
            let value = get_single_string_arg(attr, "name")?;
            directives.name = Some(value);
        }
        "no_auto_include" => {
            // No arguments expected
            directives.no_auto_include = true;
        }
        "include" => {
            let value = get_single_string_arg(attr, "include")?;
            directives.includes.push(value);
        }
        "exclude" => {
            let value = get_single_string_arg(attr, "exclude")?;
            directives.excludes.push(value);
        }
        "include_if" => {
            let (condition, file) = get_conditional_args(attr, "include_if")?;
            directives.conditional_includes.push((condition, file));
        }
        "exclude_if" => {
            let (condition, file) = get_conditional_args(attr, "exclude_if")?;
            directives.conditional_excludes.push((condition, file));
        }

        // Subpackage directives
        "no_auto_include_packages" => {
            directives.no_auto_include_packages = true;
        }
        "include_package" => {
            let value = get_single_string_arg(attr, "include_package")?;
            directives.package_includes.push(value);
        }
        "exclude_package" => {
            let value = get_single_string_arg(attr, "exclude_package")?;
            directives.package_excludes.push(value);
        }
        "include_package_if" => {
            let (condition, pkg) = get_conditional_args(attr, "include_package_if")?;
            directives.conditional_package_includes.push((condition, pkg));
        }
        "exclude_package_if" => {
            let (condition, pkg) = get_conditional_args(attr, "exclude_package_if")?;
            directives.conditional_package_excludes.push((condition, pkg));
        }

        _ => {
            // Unknown directives are silently ignored
        }
    }

    Ok(())
}

/// Extract a single string argument from an attribute like `#![name("value")]`.
fn get_single_string_arg(attr: &InnerAttribute, directive: &str) -> Result<String, DirectiveError> {
    let input = attr.input().ok_or_else(|| DirectiveError::MalformedDirective {
        directive: directive.to_string(),
        reason: "expected parenthesized argument".to_string(),
    })?;

    let mut args = input.args();
    let arg = args.next().ok_or_else(|| DirectiveError::MalformedDirective {
        directive: directive.to_string(),
        reason: "expected string argument".to_string(),
    })?;

    let value = arg.value().ok_or_else(|| DirectiveError::MalformedDirective {
        directive: directive.to_string(),
        reason: "expected string literal".to_string(),
    })?;

    let text = value.text();
    let unquoted = unquote_string(text);

    // Validate non-empty
    if unquoted.is_empty() {
        return Err(DirectiveError::MalformedDirective {
            directive: directive.to_string(),
            reason: "empty string not allowed".to_string(),
        });
    }

    Ok(unquoted)
}

/// Extract conditional arguments from `#![include_if(condition, "file")]`.
fn get_conditional_args(
    attr: &InnerAttribute,
    directive: &str,
) -> Result<(String, String), DirectiveError> {
    let input = attr.input().ok_or_else(|| DirectiveError::MalformedDirective {
        directive: directive.to_string(),
        reason: "expected parenthesized arguments".to_string(),
    })?;

    let args: Vec<_> = input.args().collect();

    if args.len() < 2 {
        return Err(DirectiveError::MalformedDirective {
            directive: directive.to_string(),
            reason: "expected (condition, \"file\") arguments".to_string(),
        });
    }

    // First arg is the condition (identifier via nested_path)
    let condition = args[0]
        .nested_path()
        .map(|p| p.path_string())
        .ok_or_else(|| DirectiveError::MalformedDirective {
            directive: directive.to_string(),
            reason: "expected condition identifier".to_string(),
        })?;

    // Second arg is the file (string literal)
    let file_value = args[1].value().ok_or_else(|| DirectiveError::MalformedDirective {
        directive: directive.to_string(),
        reason: "expected file string".to_string(),
    })?;

    let file = unquote_string(file_value.text());

    // Validate non-empty
    if file.is_empty() {
        return Err(DirectiveError::MalformedDirective {
            directive: directive.to_string(),
            reason: "empty string not allowed".to_string(),
        });
    }

    Ok((condition, file))
}

/// Remove surrounding quotes from a string literal.
fn unquote_string(s: &str) -> String {
    s.trim_matches('"').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_name_directive() {
        let source = r#"#![name("my_package")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.name, Some("my_package".to_string()));
    }

    #[test]
    fn parse_no_auto_include_directive() {
        let source = r#"#![no_auto_include]"#;
        let directives = parse_package_directives(source).unwrap();

        assert!(directives.no_auto_include);
    }

    #[test]
    fn parse_include_directive() {
        let source = r#"#![include("file.spl")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.includes, vec!["file.spl"]);
    }

    #[test]
    fn parse_multiple_includes() {
        let source = r#"
            #![include("a.spl")]
            #![include("b.spl")]
            #![include("c.spl")]
        "#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.includes, vec!["a.spl", "b.spl", "c.spl"]);
    }

    #[test]
    fn parse_exclude_directive() {
        let source = r#"#![exclude("test.spl")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.excludes, vec!["test.spl"]);
    }

    #[test]
    fn parse_include_if_directive() {
        let source = r#"#![include_if(debug, "debug.spl")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(
            directives.conditional_includes,
            vec![("debug".to_string(), "debug.spl".to_string())]
        );
    }

    #[test]
    fn parse_exclude_if_directive() {
        let source = r#"#![exclude_if(prod, "test.spl")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(
            directives.conditional_excludes,
            vec![("prod".to_string(), "test.spl".to_string())]
        );
    }

    #[test]
    fn parse_empty_package_file() {
        let source = "";
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives, PackageDirectives::default());
    }

    #[test]
    fn parse_unknown_directive_ignored() {
        let source = r#"
            #![unknown_directive("value")]
            #![name("test")]
        "#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.name, Some("test".to_string()));
    }

    #[test]
    fn parse_malformed_directive_returns_error() {
        // name directive without argument
        let source = r#"#![name]"#;
        let result = parse_package_directives(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            DirectiveError::MalformedDirective { directive, .. } => {
                assert_eq!(directive, "name");
            }
            e => panic!("expected MalformedDirective, got {:?}", e),
        }
    }

    #[test]
    fn parse_combined_directives() {
        let source = r#"
            #![name("mypackage")]
            #![no_auto_include]
            #![include("main.spl")]
            #![include("lib.spl")]
            #![exclude("test_utils.spl")]
            #![include_if(debug, "debug.spl")]
            #![exclude_if(release, "dev.spl")]
        "#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.name, Some("mypackage".to_string()));
        assert!(directives.no_auto_include);
        assert_eq!(directives.includes, vec!["main.spl", "lib.spl"]);
        assert_eq!(directives.excludes, vec!["test_utils.spl"]);
        assert_eq!(
            directives.conditional_includes,
            vec![("debug".to_string(), "debug.spl".to_string())]
        );
        assert_eq!(
            directives.conditional_excludes,
            vec![("release".to_string(), "dev.spl".to_string())]
        );
    }

    #[test]
    fn directive_error_display() {
        let err = DirectiveError::ParseError("test".to_string());
        assert!(err.to_string().contains("parse error"));

        let err = DirectiveError::MalformedDirective {
            directive: "name".to_string(),
            reason: "missing arg".to_string(),
        };
        assert!(err.to_string().contains("name"));
        assert!(err.to_string().contains("missing arg"));
    }

    // --- Subpackage directive tests ---

    #[test]
    fn parse_no_auto_include_packages_directive() {
        let source = r#"#![no_auto_include_packages]"#;
        let directives = parse_package_directives(source).unwrap();

        assert!(directives.no_auto_include_packages);
    }

    #[test]
    fn parse_include_package_directive() {
        let source = r#"#![include_package("child")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.package_includes, vec!["child"]);
    }

    #[test]
    fn parse_multiple_include_packages() {
        let source = r#"
            #![include_package("utils")]
            #![include_package("core")]
            #![include_package("tests")]
        "#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(
            directives.package_includes,
            vec!["utils", "core", "tests"]
        );
    }

    #[test]
    fn parse_exclude_package_directive() {
        let source = r#"#![exclude_package("tests")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.package_excludes, vec!["tests"]);
    }

    #[test]
    fn parse_include_package_if_directive() {
        let source = r#"#![include_package_if(debug, "debug_tools")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(
            directives.conditional_package_includes,
            vec![("debug".to_string(), "debug_tools".to_string())]
        );
    }

    #[test]
    fn parse_exclude_package_if_directive() {
        let source = r#"#![exclude_package_if(prod, "dev_utils")]"#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(
            directives.conditional_package_excludes,
            vec![("prod".to_string(), "dev_utils".to_string())]
        );
    }

    #[test]
    fn parse_combined_file_and_package_directives() {
        let source = r#"
            #![name("mypackage")]
            #![include("main.spl")]
            #![exclude("test.spl")]
            #![include_package("utils")]
            #![exclude_package("benchmarks")]
            #![no_auto_include_packages]
        "#;
        let directives = parse_package_directives(source).unwrap();

        assert_eq!(directives.name, Some("mypackage".to_string()));
        assert_eq!(directives.includes, vec!["main.spl"]);
        assert_eq!(directives.excludes, vec!["test.spl"]);
        assert_eq!(directives.package_includes, vec!["utils"]);
        assert_eq!(directives.package_excludes, vec!["benchmarks"]);
        assert!(directives.no_auto_include_packages);
    }

    // --- Empty string validation tests ---

    #[test]
    fn parse_include_empty_string_errors() {
        let source = r#"#![include("")]"#;
        let result = parse_package_directives(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            DirectiveError::MalformedDirective { directive, reason } => {
                assert_eq!(directive, "include");
                assert!(reason.contains("empty"));
            }
            e => panic!("expected MalformedDirective, got {:?}", e),
        }
    }

    #[test]
    fn parse_exclude_empty_string_errors() {
        let source = r#"#![exclude("")]"#;
        let result = parse_package_directives(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            DirectiveError::MalformedDirective { directive, reason } => {
                assert_eq!(directive, "exclude");
                assert!(reason.contains("empty"));
            }
            e => panic!("expected MalformedDirective, got {:?}", e),
        }
    }

    #[test]
    fn parse_name_empty_string_errors() {
        let source = r#"#![name("")]"#;
        let result = parse_package_directives(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            DirectiveError::MalformedDirective { directive, reason } => {
                assert_eq!(directive, "name");
                assert!(reason.contains("empty"));
            }
            e => panic!("expected MalformedDirective, got {:?}", e),
        }
    }

    #[test]
    fn parse_include_package_empty_string_errors() {
        let source = r#"#![include_package("")]"#;
        let result = parse_package_directives(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            DirectiveError::MalformedDirective { directive, reason } => {
                assert_eq!(directive, "include_package");
                assert!(reason.contains("empty"));
            }
            e => panic!("expected MalformedDirective, got {:?}", e),
        }
    }

    #[test]
    fn parse_exclude_package_empty_string_errors() {
        let source = r#"#![exclude_package("")]"#;
        let result = parse_package_directives(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            DirectiveError::MalformedDirective { directive, reason } => {
                assert_eq!(directive, "exclude_package");
                assert!(reason.contains("empty"));
            }
            e => panic!("expected MalformedDirective, got {:?}", e),
        }
    }

    #[test]
    fn parse_include_if_empty_file_errors() {
        let source = r#"#![include_if(debug, "")]"#;
        let result = parse_package_directives(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            DirectiveError::MalformedDirective { directive, reason } => {
                assert_eq!(directive, "include_if");
                assert!(reason.contains("empty"));
            }
            e => panic!("expected MalformedDirective, got {:?}", e),
        }
    }
}
