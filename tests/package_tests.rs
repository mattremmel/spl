//! File-based integration tests for the package module.
//!
//! Uses datatest-stable to run tests from `tests/packages/` directories.
//! Each test package has a `_test.toml` file that specifies test expectations.

use serde::Deserialize;
use spl::Severity;
use spl::package::Package;
use std::path::Path;

/// Test configuration from `_test.toml`.
#[derive(Debug, Deserialize)]
struct TestConfig {
    /// Test mode: "load-pass", "load-fail", "compile-pass", or "compile-fail"
    mode: String,
    /// Expected item count (for load-pass)
    expect_items: Option<usize>,
    /// Expected file count (for load-pass)
    expect_files: Option<usize>,
    /// Expected package name (for load-pass)
    expect_name: Option<String>,
    /// Expected module count (for load-pass)
    expect_modules: Option<usize>,
    /// Expected error pattern (for load-fail)
    expect_error: Option<String>,
    /// Expected compile error pattern (for compile-fail)
    expect_compile_error: Option<String>,
}

fn run_package_test(path: &Path) -> datatest_stable::Result<()> {
    // path is _test.toml, use parent as package dir
    let pkg_dir = path.parent().expect("_test.toml should have parent dir");

    // Read config from filesystem
    let contents = std::fs::read_to_string(path)?;
    let config: TestConfig = toml::from_str(&contents)?;

    // Load the package
    let result = Package::load(pkg_dir);

    match config.mode.as_str() {
        "load-pass" => {
            let pkg = result.map_err(|e| format!("{}: load failed: {:?}", path.display(), e))?;

            // Check expected item count
            if let Some(expected) = config.expect_items {
                let actual = pkg.items().count();
                if actual != expected {
                    return Err(format!(
                        "{}: expected {} items, got {}",
                        path.display(),
                        expected,
                        actual
                    )
                    .into());
                }
            }

            // Check expected file count
            if let Some(expected) = config.expect_files {
                let actual = pkg.file_count();
                if actual != expected {
                    return Err(format!(
                        "{}: expected {} files, got {}",
                        path.display(),
                        expected,
                        actual
                    )
                    .into());
                }
            }

            // Check expected name
            if let Some(expected) = &config.expect_name {
                let actual = pkg.name();
                if actual != expected {
                    return Err(format!(
                        "{}: expected name '{}', got '{}'",
                        path.display(),
                        expected,
                        actual
                    )
                    .into());
                }
            }

            // Check expected module count
            if let Some(expected) = config.expect_modules {
                let actual = pkg.modules().count();
                if actual != expected {
                    return Err(format!(
                        "{}: expected {} modules, got {}",
                        path.display(),
                        expected,
                        actual
                    )
                    .into());
                }
            }
        }
        "load-fail" => {
            match result {
                Ok(_) => {
                    return Err(format!(
                        "{}: expected load to fail, but it succeeded",
                        path.display()
                    )
                    .into());
                }
                Err(err) => {
                    // Check error pattern if specified
                    if let Some(pattern) = &config.expect_error {
                        let err_str = format!("{err:?}");
                        if !err_str.to_lowercase().contains(&pattern.to_lowercase()) {
                            return Err(format!(
                                "{}: expected error containing '{}', got: {}",
                                path.display(),
                                pattern,
                                err_str
                            )
                            .into());
                        }
                    }
                }
            }
        }
        "compile-pass" => {
            let pkg = result.map_err(|e| format!("{}: load failed: {:?}", path.display(), e))?;
            let compile_result = spl::package::compile_package(&pkg);

            if compile_result.is_err() {
                let errors: Vec<_> = compile_result.errors().map(|d| d.message.clone()).collect();
                return Err(format!(
                    "{}: compilation failed:\n{}",
                    path.display(),
                    errors.join("\n")
                )
                .into());
            }
        }
        "compile-fail" => {
            let pkg = result.map_err(|e| format!("{}: load failed: {:?}", path.display(), e))?;
            let compile_result = spl::package::compile_package(&pkg);

            // Check that compilation actually failed
            let has_errors = compile_result
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error);

            if !has_errors {
                return Err(format!("{}: expected compilation to fail", path.display()).into());
            }

            // Check error pattern if specified
            if let Some(pattern) = &config.expect_compile_error {
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
                    )
                    .into());
                }
            }
        }
        other => {
            return Err(format!("{}: unknown mode: {}", path.display(), other).into());
        }
    }

    Ok(())
}

datatest_stable::harness! {
    { test = run_package_test, root = "tests/packages", pattern = r".*/_test\.toml$" },
}
