//! Test harness for running .spl test files.
//!
//! This uses `datatest-stable` to automatically discover and run all `.spl`
//! files in the `tests/stdlib/` directory.
//!
//! Each test file can contain directives in `//@ directive` comments to
//! control test behavior. See `spl::testing::TestDirectives` for details.

use datatest_stable::include_dir;
use spl::testing::run_spl_test;
use std::path::Path;

/// Run a single SPL test file.
///
/// This function receives the path and contents from `datatest-stable`
/// when using the `include_dir!` macro for embedding test files.
#[allow(clippy::needless_pass_by_value)] // Required by datatest-stable API
fn run_test(path: &Path, contents: String) -> datatest_stable::Result<()> {
    run_spl_test(path, &contents).map_err(std::convert::Into::into)
}

datatest_stable::harness! {
    { test = run_test, root = include_dir!("tests/stdlib"), pattern = r".*\.spl$" },
}
