//! Multi-file package loading for SPL.
//!
//! This module provides functionality for loading SPL packages from directories,
//! supporting multiple source files, `_module.spl` configuration, and child modules.
//!
//! # Overview
//!
//! A package is a directory containing `.spl` source files. Packages can optionally
//! have a `_module.spl` configuration file that controls how files are included.
//!
//! # Basic Usage
//!
//! ```ignore
//! use spl_compiler::package::Package;
//!
//! let pkg = Package::load("path/to/package")?;
//! for item in pkg.items() {
//!     // Process AST items from all package files
//! }
//! ```

// Re-export everything from spl_package
pub use spl_package::*;

// Local compilation module (depends on crate internals)
mod compile;
pub use compile::compile_package;
