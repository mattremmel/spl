//! Spec tests harness for spl-test-runner.
//!
//! Uses datatest-stable to discover and run all tests in the cases/ directory.

use spl_test_runner::{TestConfig, run_test};
use std::path::Path;

/// Run a single test from a test.toml file.
fn run_spec_test(path: &Path) -> datatest_stable::Result<()> {
    // Read and parse the config
    let contents = std::fs::read_to_string(path)?;
    let config: TestConfig = TestConfig::parse(&contents)
        .map_err(|e| format!("{}: failed to parse config: {}", path.display(), e))?;

    // Run the test
    run_test(path, &config).map_err(std::convert::Into::into)
}

datatest_stable::harness! {
    // Package tests
    { test = run_spec_test, root = "cases/packages", pattern = r".*/test\.toml$" },
    // Codegen tests
    { test = run_spec_test, root = "cases/codegen", pattern = r".*\.toml$" },
    // Stdlib tests
    { test = run_spec_test, root = "cases/stdlib", pattern = r".*\.toml$" },
}
