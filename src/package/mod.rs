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
//! use spl::package::Package;
//!
//! let pkg = Package::load("path/to/package")?;
//! for item in pkg.items() {
//!     // Process AST items from all package files
//! }
//! ```

mod compilation_unit;
mod compile;
mod directive;
mod resolver;
mod scanner;
mod source_map;

pub use compilation_unit::CompilationUnit;
pub use compile::compile_package;
pub use directive::{DirectiveError, PackageDirectives, parse_package_directives};
pub use resolver::{
    ResolveError, resolve_includes, resolve_modules, try_resolve_includes, try_resolve_modules,
};
pub use scanner::{ScanError, find_modules, has_module_config, scan_directory};
pub use source_map::{FileId, SourceMap};

use crate::ast::Item;
use crate::parser::ParseError;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

/// Errors that can occur when loading a package.
#[derive(Debug)]
pub enum PackageError {
    /// An I/O error occurred.
    Io(io::Error),
    /// The path is not a directory.
    NotADirectory(PathBuf),
    /// The directory contains no source files.
    NoSourceFiles(PathBuf),
    /// Error parsing _module.spl directives.
    DirectiveError(DirectiveError),
    /// Parse errors in source files.
    ///
    /// Contains a list of (`file_path`, errors) for each file with parse errors.
    /// All files with errors are included, not just the first one.
    ParseErrors {
        errors: Vec<(PathBuf, Vec<ParseError>)>,
    },
    /// Error resolving file inclusions.
    ResolveError(ResolveError),
    /// Error scanning directory.
    ScanError(ScanError),
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::Io(e) => write!(f, "I/O error: {e}"),
            PackageError::NotADirectory(p) => write!(f, "not a directory: {}", p.display()),
            PackageError::NoSourceFiles(p) => {
                write!(f, "no source files found in: {}", p.display())
            }
            PackageError::DirectiveError(e) => write!(f, "directive error: {e}"),
            PackageError::ParseErrors { errors } => {
                let total_errors: usize = errors.iter().map(|(_, errs)| errs.len()).sum();
                let file_count = errors.len();
                write!(
                    f,
                    "parse errors in {file_count} file(s): {total_errors} total error(s)"
                )
            }
            PackageError::ResolveError(e) => write!(f, "resolve error: {e}"),
            PackageError::ScanError(e) => write!(f, "scan error: {e}"),
        }
    }
}

impl std::error::Error for PackageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PackageError::Io(e) => Some(e),
            PackageError::DirectiveError(e) => Some(e),
            PackageError::ResolveError(e) => Some(e),
            PackageError::ScanError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for PackageError {
    fn from(e: io::Error) -> Self {
        PackageError::Io(e)
    }
}

impl From<DirectiveError> for PackageError {
    fn from(e: DirectiveError) -> Self {
        PackageError::DirectiveError(e)
    }
}

impl From<ResolveError> for PackageError {
    fn from(e: ResolveError) -> Self {
        PackageError::ResolveError(e)
    }
}

impl From<ScanError> for PackageError {
    fn from(e: ScanError) -> Self {
        PackageError::ScanError(e)
    }
}

/// Warnings that can occur when loading a package.
///
/// Warnings indicate non-fatal issues that did not prevent package loading
/// but may require attention.
#[derive(Debug, Clone)]
pub enum PackageWarning {
    /// A child module failed to load.
    ModuleLoadFailed {
        /// Name of the failed module.
        name: String,
        /// Error message describing the failure.
        error: String,
    },
}

impl fmt::Display for PackageWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageWarning::ModuleLoadFailed { name, error } => {
                write!(f, "failed to load module '{name}': {error}")
            }
        }
    }
}

/// A loaded SPL package.
///
/// A package represents a directory of SPL source files that have been parsed
/// and combined into a compilation unit.
pub struct Package {
    /// Package name (from _module.spl or derived from directory name).
    name: String,
    /// Root directory of the package.
    root: PathBuf,
    /// The compilation unit containing all parsed files.
    compilation_unit: CompilationUnit,
    /// Child modules (subdirectories with .spl files).
    modules: Vec<Package>,
    /// Warnings encountered during loading.
    warnings: Vec<PackageWarning>,
}

impl Package {
    /// Load a package from a directory.
    ///
    /// Scans the directory for `.spl` files, parses the optional `_module.spl`
    /// configuration, and loads all source files.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path is not a directory
    /// - The directory contains no source files
    /// - The `_module.spl` file has syntax errors
    /// - Any source file has parse errors
    /// - An explicitly included file is not found
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PackageError> {
        Self::load_with_conditions(path, &[] as &[&str])
    }

    /// Load a package with enabled conditions.
    ///
    /// Conditions are used for conditional includes/excludes in `_module.spl`:
    /// - `#![include_if(debug, "debug.spl")]` - include if "debug" condition is enabled
    /// - `#![exclude_if(release, "test.spl")]` - exclude if "release" condition is enabled
    pub fn load_with_conditions(
        path: impl AsRef<Path>,
        conditions: &[impl AsRef<str>],
    ) -> Result<Self, PackageError> {
        let root = path.as_ref().to_path_buf();

        if !root.is_dir() {
            return Err(PackageError::NotADirectory(root));
        }

        // Scan directory for .spl files
        let all_files = scan_directory(&root)?;

        // Parse _module.spl if it exists
        let directives = if has_module_config(&root) {
            let config_path = root.join("_module.spl");
            let config_content = fs::read_to_string(&config_path)?;
            parse_package_directives(&config_content)?
        } else {
            PackageDirectives::default()
        };

        // Determine package name
        let name = directives.name.clone().unwrap_or_else(|| {
            root.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string()
        });

        // Resolve which files to include
        let file_names: Vec<String> = all_files
            .iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .map(String::from)
            .collect();

        let included = try_resolve_includes(&file_names, &directives, conditions)?;

        if included.is_empty() {
            return Err(PackageError::NoSourceFiles(root));
        }

        // Build source map and load files
        let mut source_map = SourceMap::new();
        let mut file_ids = Vec::new();

        for file_name in &included {
            let file_path = root.join(file_name);
            let content = fs::read_to_string(&file_path)?;
            let id = source_map.add_file(&file_path, content);
            file_ids.push(id);
        }

        // Create compilation unit
        let compilation_unit = CompilationUnit::parse(source_map, &file_ids);

        // Check for parse errors - collect from all files
        if compilation_unit.has_errors() {
            // Group errors by file
            let mut errors_by_file: std::collections::HashMap<FileId, Vec<ParseError>> =
                std::collections::HashMap::new();

            for (file_id, error) in compilation_unit.errors() {
                errors_by_file
                    .entry(file_id)
                    .or_default()
                    .push(error.clone());
            }

            // Convert to Vec<(PathBuf, Vec<ParseError>)> with sorted file paths for determinism
            let mut all_errors: Vec<(PathBuf, Vec<ParseError>)> = errors_by_file
                .into_iter()
                .map(|(file_id, errs)| {
                    let path = compilation_unit
                        .source_map()
                        .get_path(file_id)
                        .map(PathBuf::from)
                        .unwrap_or_else(|| root.join("unknown"));
                    (path, errs)
                })
                .collect();

            // Sort by file path for deterministic output
            all_errors.sort_by(|(a, _), (b, _)| a.cmp(b));

            return Err(PackageError::ParseErrors { errors: all_errors });
        }

        // Find and load child modules
        let subdirs = find_modules(&root)?;

        // Get module names and filter to those with .spl files
        let available_modules: Vec<String> = subdirs
            .iter()
            .filter(|subdir| {
                scan_directory(subdir)
                    .map(|files| !files.is_empty())
                    .unwrap_or(false)
            })
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .map(String::from)
            .collect();

        // Resolve which modules to include
        let included_modules = try_resolve_modules(&available_modules, &directives, conditions)?;

        let mut modules = Vec::new();
        let mut warnings = Vec::new();

        for mod_name in &included_modules {
            let subdir = root.join(mod_name);
            // Attempt to load, but don't fail the parent if child module fails
            match Self::load_with_conditions(&subdir, conditions) {
                Ok(child_mod) => modules.push(child_mod),
                Err(e) => {
                    warnings.push(PackageWarning::ModuleLoadFailed {
                        name: mod_name.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(Package {
            name,
            root,
            compilation_unit,
            modules,
            warnings,
        })
    }

    /// Returns the package name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the compilation unit.
    pub fn compilation_unit(&self) -> &CompilationUnit {
        &self.compilation_unit
    }

    /// Returns an iterator over all items in the package.
    pub fn items(&self) -> impl Iterator<Item = (FileId, Item)> + '_ {
        self.compilation_unit.items()
    }

    /// Returns an iterator over child modules.
    pub fn modules(&self) -> impl Iterator<Item = &Package> {
        self.modules.iter()
    }

    /// Returns the number of source files in this package.
    pub fn file_count(&self) -> usize {
        self.compilation_unit.file_count()
    }

    /// Returns warnings encountered during loading.
    ///
    /// Warnings indicate non-fatal issues such as child modules that failed to load.
    pub fn warnings(&self) -> &[PackageWarning] {
        &self.warnings
    }

    /// Returns true if there are any warnings.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_packages_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("packages")
            .join(name)
    }

    #[test]
    fn package_load_simple() {
        let path = test_packages_path("simple");
        let pkg = Package::load(&path).unwrap();

        assert_eq!(pkg.name(), "simple");
        assert_eq!(pkg.file_count(), 2);
        assert_eq!(pkg.items().count(), 2); // main and helper functions
    }

    #[test]
    fn package_respects_name_directive() {
        let path = test_packages_path("configured");
        let pkg = Package::load(&path).unwrap();

        assert_eq!(pkg.name(), "custom");
    }

    #[test]
    fn package_discovers_modules() {
        let path = test_packages_path("nested");
        let pkg = Package::load(&path).unwrap();

        let modules: Vec<_> = pkg.modules().collect();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name(), "child");
    }

    #[test]
    fn package_load_nonexistent_error() {
        let result = Package::load("/nonexistent/path");
        assert!(result.is_err());
        match result {
            Err(PackageError::NotADirectory(_)) => {}
            Err(e) => panic!("expected NotADirectory, got {e:?}"),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn package_load_empty_directory_error() {
        let path = test_packages_path("empty");
        let result = Package::load(&path);

        assert!(result.is_err());
        match result {
            Err(PackageError::NoSourceFiles(_)) => {}
            Err(e) => panic!("expected NoSourceFiles, got {e:?}"),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn package_load_mixed_ignores_non_spl() {
        let path = test_packages_path("mixed");
        let pkg = Package::load(&path).unwrap();

        // Should only include code.spl, not readme.txt or data.json
        assert_eq!(pkg.file_count(), 1);
    }

    #[test]
    fn package_root_path() {
        let path = test_packages_path("simple");
        let pkg = Package::load(&path).unwrap();

        assert_eq!(pkg.root(), path);
    }

    #[test]
    fn package_error_display() {
        let err = PackageError::NotADirectory(PathBuf::from("/test"));
        assert!(err.to_string().contains("/test"));

        let err = PackageError::NoSourceFiles(PathBuf::from("/empty"));
        assert!(err.to_string().contains("/empty"));

        // Test ParseErrors display with multiple files
        let err = PackageError::ParseErrors {
            errors: vec![
                (PathBuf::from("file1.spl"), vec![]),
                (
                    PathBuf::from("file2.spl"),
                    vec![crate::parser::ParseError {
                        message: "test error".to_string(),
                        range: 0..1,
                    }],
                ),
            ],
        };
        let display = err.to_string();
        assert!(display.contains("2 file(s)"));
        assert!(display.contains("1 total error"));
    }

    #[test]
    fn module_respects_own_config() {
        let path = test_packages_path("subpackage_with_config");
        let pkg = Package::load(&path).unwrap();

        let child = pkg.modules().next().expect("expected a child module");
        // Child has its own _module.spl with #![name("custom_child")]
        assert_eq!(child.name(), "custom_child");
    }

    #[test]
    fn deep_module_nesting() {
        let path = test_packages_path("deep_nesting");
        let pkg = Package::load(&path).unwrap();

        let l1 = pkg.modules().next().expect("expected level1 module");
        assert_eq!(l1.name(), "level1");

        let l2 = l1.modules().next().expect("expected level2 module");
        assert_eq!(l2.name(), "level2");

        let l3 = l2.modules().next().expect("expected level3 module");
        assert_eq!(l3.name(), "level3");
    }

    #[test]
    fn module_load_failure_produces_warning() {
        let path = test_packages_path("subpackage_error");
        let pkg = Package::load(&path).unwrap();

        // Parent should load successfully
        assert_eq!(pkg.name(), "subpackage_error");
        assert_eq!(pkg.file_count(), 1);

        // No child modules loaded (broken_child failed)
        assert_eq!(pkg.modules().count(), 0);

        // But we should have a warning about the failure
        assert!(pkg.has_warnings());
        assert_eq!(pkg.warnings().len(), 1);

        match &pkg.warnings()[0] {
            PackageWarning::ModuleLoadFailed { name, error } => {
                assert_eq!(name, "broken_child");
                // Error is from missing include file
                assert!(error.contains("not found"));
            }
        }
    }

    #[test]
    fn package_warning_display() {
        let warning = PackageWarning::ModuleLoadFailed {
            name: "child".to_string(),
            error: "parse error".to_string(),
        };
        let display = warning.to_string();
        assert!(display.contains("child"));
        assert!(display.contains("parse error"));
    }
}
