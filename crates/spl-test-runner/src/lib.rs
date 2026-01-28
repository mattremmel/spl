//! SPL Test Runner
//!
//! A unified test framework for SPL compiler tests. Supports multiple test modes:
//!
//! - **load-pass**: Package should load successfully
//! - **load-fail**: Package should fail to load
//! - **compile-pass**: Code should compile successfully
//! - **compile-fail**: Code should fail to compile
//! - **run-pass**: Code should compile and run successfully
//! - **run-fail**: Code should compile but fail at runtime
//!
//! # Test Configuration
//!
//! Tests are configured via TOML files:
//!
//! ```toml
//! mode = "run-pass"
//! ignore = false
//!
//! [expect.package]
//! items = 2
//! files = 1
//! name = "my_pkg"
//! modules = 3
//!
//! [expect.compile]
//! error = "cannot find"
//!
//! [expect.run]
//! return = 42
//! stdout = "hello"
//!
//! [source]
//! inline = "fn main(): i32 { 42 }"
//! # Or: file = "main.spl"
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use spl_test_runner::{TestConfig, run_test};
//! use std::path::Path;
//!
//! let config = TestConfig::parse(r#"
//!     mode = "run-pass"
//!     [expect.run]
//!     return = 42
//!     [source]
//!     inline = "fn main(): i32 { 42 }"
//! "#).unwrap();
//!
//! run_test(Path::new("test.toml"), &config).unwrap();
//! ```

pub mod config;
pub mod executor;
pub mod runner;

pub use config::{
    CompileExpectations, Expectations, PackageExpectations, RunExpectations, Source, TestConfig,
    TestMode,
};
pub use executor::{ExecuteError, ExecuteResult, execute_captured};
pub use runner::{format_diagnostics, run_package_test, run_source_test, run_test};
