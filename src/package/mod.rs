//! Multi-file package loading for SPL.
//!
//! This module provides functionality for loading SPL packages from directories,
//! supporting multiple source files, `_package.spl` configuration, and subpackages.
//!
//! # Overview
//!
//! A package is a directory containing `.spl` source files. Packages can optionally
//! have a `_package.spl` configuration file that controls how files are included.
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
mod directive;
mod resolver;
mod scanner;
mod source_map;

pub use compilation_unit::CompilationUnit;
pub use directive::{parse_package_directives, DirectiveError, PackageDirectives};
pub use resolver::{
    resolve_includes, resolve_packages, try_resolve_includes, try_resolve_packages, ResolveError,
};
pub use scanner::{find_subpackages, has_package_config, scan_directory, ScanError};
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
    /// Error parsing _package.spl directives.
    DirectiveError(DirectiveError),
    /// Parse errors in source files.
    ParseErrors {
        file: PathBuf,
        errors: Vec<ParseError>,
    },
    /// Error resolving file inclusions.
    ResolveError(ResolveError),
    /// Error scanning directory.
    ScanError(ScanError),
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::Io(e) => write!(f, "I/O error: {}", e),
            PackageError::NotADirectory(p) => write!(f, "not a directory: {}", p.display()),
            PackageError::NoSourceFiles(p) => {
                write!(f, "no source files found in: {}", p.display())
            }
            PackageError::DirectiveError(e) => write!(f, "directive error: {}", e),
            PackageError::ParseErrors { file, errors } => {
                write!(
                    f,
                    "parse errors in {}: {}",
                    file.display(),
                    errors.len()
                )
            }
            PackageError::ResolveError(e) => write!(f, "resolve error: {}", e),
            PackageError::ScanError(e) => write!(f, "scan error: {}", e),
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

/// A loaded SPL package.
///
/// A package represents a directory of SPL source files that have been parsed
/// and combined into a compilation unit.
pub struct Package {
    /// Package name (from _package.spl or derived from directory name).
    name: String,
    /// Root directory of the package.
    root: PathBuf,
    /// The compilation unit containing all parsed files.
    compilation_unit: CompilationUnit,
    /// Subpackages (subdirectories with .spl files).
    subpackages: Vec<Package>,
}

impl Package {
    /// Load a package from a directory.
    ///
    /// Scans the directory for `.spl` files, parses the optional `_package.spl`
    /// configuration, and loads all source files.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path is not a directory
    /// - The directory contains no source files
    /// - The `_package.spl` file has syntax errors
    /// - Any source file has parse errors
    /// - An explicitly included file is not found
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PackageError> {
        Self::load_with_conditions(path, &[] as &[&str])
    }

    /// Load a package with enabled conditions.
    ///
    /// Conditions are used for conditional includes/excludes in `_package.spl`:
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

        // Parse _package.spl if it exists
        let directives = if has_package_config(&root) {
            let config_path = root.join("_package.spl");
            let config_content = fs::read_to_string(&config_path)?;
            parse_package_directives(&config_content)?
        } else {
            PackageDirectives::default()
        };

        // Determine package name
        let name = directives
            .name
            .clone()
            .unwrap_or_else(|| {
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

        // Check for parse errors
        if compilation_unit.has_errors() {
            // Report first file with errors
            if let Some((file_id, _)) = compilation_unit.errors().next() {
                let file_path = compilation_unit
                    .source_map()
                    .get_path(file_id)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| root.join("unknown"));

                let errors: Vec<_> = compilation_unit
                    .errors()
                    .filter(|(id, _)| *id == file_id)
                    .map(|(_, e)| e.clone())
                    .collect();

                return Err(PackageError::ParseErrors {
                    file: file_path,
                    errors,
                });
            }
        }

        // Find and load subpackages
        let subdirs = find_subpackages(&root)?;

        // Get subpackage names and filter to those with .spl files
        let available_packages: Vec<String> = subdirs
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

        // Resolve which packages to include
        let included_packages = try_resolve_packages(&available_packages, &directives, conditions)?;

        let mut subpackages = Vec::new();
        for pkg_name in &included_packages {
            let subdir = root.join(pkg_name);
            // Attempt to load, but don't fail the parent if subpackage fails
            if let Ok(subpkg) = Self::load_with_conditions(&subdir, conditions) {
                subpackages.push(subpkg);
            }
        }

        Ok(Package {
            name,
            root,
            compilation_unit,
            subpackages,
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

    /// Returns an iterator over subpackages.
    pub fn subpackages(&self) -> impl Iterator<Item = &Package> {
        self.subpackages.iter()
    }

    /// Returns the number of source files in this package.
    pub fn file_count(&self) -> usize {
        self.compilation_unit.file_count()
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
    fn package_discovers_subpackages() {
        let path = test_packages_path("nested");
        let pkg = Package::load(&path).unwrap();

        let subpackages: Vec<_> = pkg.subpackages().collect();
        assert_eq!(subpackages.len(), 1);
        assert_eq!(subpackages[0].name(), "child");
    }

    #[test]
    fn package_load_nonexistent_error() {
        let result = Package::load("/nonexistent/path");
        assert!(result.is_err());
        match result {
            Err(PackageError::NotADirectory(_)) => {}
            Err(e) => panic!("expected NotADirectory, got {:?}", e),
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
            Err(e) => panic!("expected NoSourceFiles, got {:?}", e),
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
    }
}
