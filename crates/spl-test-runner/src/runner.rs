//! Test runner for executing spec tests.

use crate::config::{Source, SpecTestFile, TestConfig, TestMode};
use crate::executor::{ExecuteError, execute_captured};
use spl_compiler::package::Package;
use spl_compiler::{Diagnostic, Severity};
use std::path::Path;

/// Run all cases in a grouped spec test file.
///
/// Each case either expects a clean parse (no `expect_error`) or expects parse failure
/// with an error matching the given pattern(s).
pub fn run_spec_test_file(path: &Path, file: &SpecTestFile) -> Result<(), String> {
    let mut failures = Vec::new();

    for case in &file.case {
        if case.ignore {
            continue;
        }

        let case_id = format!("{}/{}", file.section.id, case.name);
        let parse = spl_parser::parse(&case.source);

        // Collect all error patterns to check
        let mut expected_patterns: Vec<&str> = Vec::new();
        if let Some(pattern) = &case.expect_error {
            expected_patterns.push(pattern.as_str());
        }
        if let Some(patterns) = &case.expect_errors {
            expected_patterns.extend(patterns.iter().map(String::as_str));
        }

        if expected_patterns.is_empty() {
            // parse-pass: expect no errors
            if !parse.ok() {
                let errors: Vec<_> = parse
                    .errors()
                    .iter()
                    .map(|e| format!("  - {}", e.message))
                    .collect();
                failures.push(format!(
                    "[{}] expected parse to succeed, but got {} error(s):\n{}",
                    case_id,
                    parse.errors().len(),
                    errors.join("\n"),
                ));
            }
        } else {
            // parse-fail: expect errors
            if parse.ok() {
                failures.push(format!(
                    "[{case_id}] expected parse to fail, but it succeeded",
                ));
            } else {
                // Check each expected pattern is found in some error
                for pattern in &expected_patterns {
                    let found = parse
                        .errors()
                        .iter()
                        .any(|e| e.message.to_lowercase().contains(&pattern.to_lowercase()));
                    if !found {
                        let errors: Vec<_> = parse
                            .errors()
                            .iter()
                            .map(|e| format!("  - {}", e.message))
                            .collect();
                        failures.push(format!(
                            "[{}] expected error containing '{}', got:\n{}",
                            case_id,
                            pattern,
                            errors.join("\n"),
                        ));
                    }
                }
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{}: {} case(s) failed:\n{}",
            path.display(),
            failures.len(),
            failures.join("\n\n"),
        ))
    }
}

/// Format diagnostics for display in test output.
pub fn format_diagnostics(diags: &[Diagnostic]) -> String {
    diags
        .iter()
        .map(|d| format!("[{}] {}", d.severity.as_str(), d.message))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run a single-file test with inline or file source.
pub fn run_source_test(path: &Path, config: &TestConfig, source: &str) -> Result<(), String> {
    if config.ignore {
        return Ok(());
    }

    // Compile the source
    let compile_result = spl_compiler::compile(source);

    match config.mode {
        TestMode::CompileFail => {
            if compile_result.is_ok() {
                return Err(format!(
                    "{}: expected compilation to fail, but it succeeded",
                    path.display()
                ));
            }

            // Check expected error patterns
            for pattern in config.expected_compile_errors() {
                let found = compile_result
                    .diagnostics
                    .iter()
                    .any(|d| d.message.to_lowercase().contains(&pattern.to_lowercase()));
                if !found {
                    return Err(format!(
                        "{}: expected error containing '{}', but got:\n{}",
                        path.display(),
                        pattern,
                        format_diagnostics(&compile_result.diagnostics)
                    ));
                }
            }

            Ok(())
        }

        TestMode::CompilePass => {
            if compile_result.is_err() {
                return Err(format!(
                    "{}: compilation failed:\n{}",
                    path.display(),
                    format_diagnostics(&compile_result.diagnostics)
                ));
            }
            Ok(())
        }

        TestMode::RunPass | TestMode::RunFail => {
            // For run tests, compilation must succeed first
            if compile_result.is_err() {
                return Err(format!(
                    "{}: compilation failed:\n{}",
                    path.display(),
                    format_diagnostics(&compile_result.diagnostics)
                ));
            }

            // Execute the program
            let result = match execute_captured(source) {
                Ok(r) => r,
                Err(ExecuteError::CompileFailed(e)) => {
                    return Err(format!("{}: compilation failed: {}", path.display(), e));
                }
                Err(ExecuteError::ExecutionFailed(e)) => {
                    return Err(format!("{}: execution failed: {}", path.display(), e));
                }
            };

            // For run-fail, we expect non-zero exit
            if config.mode == TestMode::RunFail {
                if result.return_value == 0 {
                    return Err(format!(
                        "{}: expected runtime failure, but program returned 0",
                        path.display()
                    ));
                }
                return Ok(());
            }

            // For run-pass, check expectations
            if let Some(expected) = config.expect.run.return_value
                && result.return_value != expected
            {
                return Err(format!(
                    "{}: expected return value {}, got {}",
                    path.display(),
                    expected,
                    result.return_value
                ));
            }

            if let Some(expected) = &config.expect.run.stdout
                && !result.stdout.contains(expected)
            {
                return Err(format!(
                    "{}: expected stdout to contain '{}', got:\n{}",
                    path.display(),
                    expected,
                    result.stdout
                ));
            }

            Ok(())
        }

        TestMode::LoadPass | TestMode::LoadFail => Err(format!(
            "{}: load-pass/load-fail modes require a package directory, not inline source",
            path.display()
        )),
    }
}

/// Run a package test from a directory.
pub fn run_package_test(path: &Path, config: &TestConfig) -> Result<(), String> {
    if config.ignore {
        return Ok(());
    }

    // path is test.toml, use parent as package dir
    let pkg_dir = path
        .parent()
        .ok_or_else(|| format!("{}: test.toml should have parent dir", path.display()))?;

    // Load the package
    let result = Package::load(pkg_dir);

    match config.mode {
        TestMode::LoadPass => {
            let pkg = result.map_err(|e| format!("{}: load failed: {:?}", path.display(), e))?;

            // Check expected item count
            if let Some(expected) = config.expect.package.items {
                let actual = pkg.items().count();
                if actual != expected {
                    return Err(format!(
                        "{}: expected {} items, got {}",
                        path.display(),
                        expected,
                        actual
                    ));
                }
            }

            // Check expected file count
            if let Some(expected) = config.expect.package.files {
                let actual = pkg.file_count();
                if actual != expected {
                    return Err(format!(
                        "{}: expected {} files, got {}",
                        path.display(),
                        expected,
                        actual
                    ));
                }
            }

            // Check expected name
            if let Some(expected) = &config.expect.package.name {
                let actual = pkg.name();
                if actual != expected {
                    return Err(format!(
                        "{}: expected name '{}', got '{}'",
                        path.display(),
                        expected,
                        actual
                    ));
                }
            }

            // Check expected module count
            if let Some(expected) = config.expect.package.modules {
                let actual = pkg.modules().count();
                if actual != expected {
                    return Err(format!(
                        "{}: expected {} modules, got {}",
                        path.display(),
                        expected,
                        actual
                    ));
                }
            }

            Ok(())
        }

        TestMode::LoadFail => {
            match result {
                Ok(_) => Err(format!(
                    "{}: expected load to fail, but it succeeded",
                    path.display()
                )),
                Err(err) => {
                    // Check error pattern if specified
                    for pattern in config.expected_compile_errors() {
                        let err_str = format!("{err:?}");
                        if !err_str.to_lowercase().contains(&pattern.to_lowercase()) {
                            return Err(format!(
                                "{}: expected error containing '{}', got: {}",
                                path.display(),
                                pattern,
                                err_str
                            ));
                        }
                    }
                    Ok(())
                }
            }
        }

        TestMode::CompilePass => {
            let pkg = result.map_err(|e| format!("{}: load failed: {:?}", path.display(), e))?;
            let compile_result = spl_compiler::package::compile_package(&pkg);

            if compile_result.is_err() {
                let errors: Vec<_> = compile_result.errors().map(|d| d.message.clone()).collect();
                return Err(format!(
                    "{}: compilation failed:\n{}",
                    path.display(),
                    errors.join("\n")
                ));
            }

            Ok(())
        }

        TestMode::CompileFail => {
            let pkg = result.map_err(|e| format!("{}: load failed: {:?}", path.display(), e))?;
            let compile_result = spl_compiler::package::compile_package(&pkg);

            // Check that compilation actually failed
            let has_errors = compile_result
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error);

            if !has_errors {
                return Err(format!("{}: expected compilation to fail", path.display()));
            }

            // Check error patterns
            for pattern in config.expected_compile_errors() {
                let has_match = compile_result
                    .errors()
                    .any(|d| d.message.to_lowercase().contains(&pattern.to_lowercase()));
                if !has_match {
                    let errors: Vec<_> =
                        compile_result.errors().map(|d| d.message.clone()).collect();
                    return Err(format!(
                        "{}: expected error containing '{}', got:\n{}",
                        path.display(),
                        pattern,
                        errors.join("\n")
                    ));
                }
            }

            Ok(())
        }

        TestMode::RunPass | TestMode::RunFail => Err(format!(
            "{}: run-pass/run-fail modes not yet supported for package tests",
            path.display()
        )),
    }
}

/// Run a test based on the path and config.
///
/// Determines whether this is a package test (directory with test.toml)
/// or a single-file test (with inline or file source).
pub fn run_test(path: &Path, config: &TestConfig) -> Result<(), String> {
    // If we have inline source, run as source test
    if let Some(Source::Inline { inline }) = &config.source {
        return run_source_test(path, config, inline);
    }

    // If we have file source, load and run
    if let Some(Source::File { file }) = &config.source {
        let base_dir = path.parent().unwrap_or(Path::new("."));
        let source_path = base_dir.join(file);
        let source = std::fs::read_to_string(&source_path).map_err(|e| {
            format!(
                "{}: failed to read source file '{}': {}",
                path.display(),
                file,
                e
            )
        })?;
        return run_source_test(path, config, &source);
    }

    // Otherwise, treat as package test
    run_package_test(path, config)
}
