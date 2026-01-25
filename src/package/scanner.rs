//! Directory scanning for package source files.
//!
//! Provides functions to scan a directory for `.spl` source files and detect
//! potential child modules.

use std::path::{Path, PathBuf};
use std::{fmt, io};

/// Errors that can occur during directory scanning.
#[derive(Debug)]
pub enum ScanError {
    /// An I/O error occurred.
    Io(io::Error),
    /// The path is not a directory.
    NotADirectory(PathBuf),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Io(e) => write!(f, "I/O error: {}", e),
            ScanError::NotADirectory(p) => write!(f, "not a directory: {}", p.display()),
        }
    }
}

impl std::error::Error for ScanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ScanError::Io(e) => Some(e),
            ScanError::NotADirectory(_) => None,
        }
    }
}

impl From<io::Error> for ScanError {
    fn from(e: io::Error) -> Self {
        ScanError::Io(e)
    }
}

/// Scans a directory for `.spl` source files (non-recursive).
///
/// Returns the paths to all `.spl` files in the directory, sorted alphabetically.
/// Does not recurse into subdirectories.
///
/// # Errors
///
/// Returns `ScanError::NotADirectory` if the path is not a directory.
/// Returns `ScanError::Io` for other I/O errors.
pub fn scan_directory(path: &Path) -> Result<Vec<PathBuf>, ScanError> {
    if !path.is_dir() {
        return Err(ScanError::NotADirectory(path.to_path_buf()));
    }

    let mut files = Vec::new();

    for entry in path.read_dir()? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_file() && entry_path.extension().is_some_and(|ext| ext == "spl") {
            files.push(entry_path);
        }
    }

    // Sort for deterministic ordering
    files.sort();
    Ok(files)
}

/// Finds subdirectories that could be child modules.
///
/// Returns paths to all immediate subdirectories, sorted alphabetically.
/// Does not check whether subdirectories contain `.spl` files.
///
/// # Errors
///
/// Returns `ScanError::NotADirectory` if the path is not a directory.
/// Returns `ScanError::Io` for other I/O errors.
pub fn find_modules(path: &Path) -> Result<Vec<PathBuf>, ScanError> {
    if !path.is_dir() {
        return Err(ScanError::NotADirectory(path.to_path_buf()));
    }

    let mut dirs = Vec::new();

    for entry in path.read_dir()? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            dirs.push(entry_path);
        }
    }

    // Sort for deterministic ordering
    dirs.sort();
    Ok(dirs)
}

/// Checks whether a directory has a `_module.spl` configuration file.
pub fn has_module_config(path: &Path) -> bool {
    path.join("_module.spl").is_file()
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
    fn scan_finds_spl_files() {
        let path = test_packages_path("simple");
        let files = scan_directory(&path).unwrap();

        let names: Vec<_> = files
            .iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .collect();

        assert!(names.contains(&"main.spl"));
        assert!(names.contains(&"helper.spl"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn scan_excludes_non_spl_files() {
        let path = test_packages_path("mixed");
        let files = scan_directory(&path).unwrap();

        let names: Vec<_> = files
            .iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .collect();

        assert!(names.contains(&"code.spl"));
        assert!(!names.contains(&"readme.txt"));
        assert!(!names.contains(&"data.json"));
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn scan_finds_module_spl() {
        let path = test_packages_path("configured");
        let files = scan_directory(&path).unwrap();

        let names: Vec<_> = files
            .iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .collect();

        assert!(names.contains(&"_module.spl"));
        assert!(names.contains(&"lib.spl"));
    }

    #[test]
    fn scan_detects_modules() {
        let path = test_packages_path("nested");
        let subdirs = find_modules(&path).unwrap();

        let names: Vec<_> = subdirs
            .iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .collect();

        assert!(names.contains(&"child"));
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn scan_nonexistent_directory_returns_error() {
        let path = PathBuf::from("/nonexistent/path/that/does/not/exist");
        let result = scan_directory(&path);

        assert!(result.is_err());
        match result.unwrap_err() {
            ScanError::NotADirectory(_) => {}
            e => panic!("expected NotADirectory, got {:?}", e),
        }
    }

    #[test]
    fn scan_file_as_directory_returns_error() {
        let path = test_packages_path("simple").join("main.spl");
        let result = scan_directory(&path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ScanError::NotADirectory(_)));
    }

    #[test]
    fn has_module_config_true() {
        let path = test_packages_path("configured");
        assert!(has_module_config(&path));
    }

    #[test]
    fn has_module_config_false() {
        let path = test_packages_path("simple");
        assert!(!has_module_config(&path));
    }

    #[test]
    fn scan_error_display() {
        let err = ScanError::NotADirectory(PathBuf::from("/some/path"));
        assert!(err.to_string().contains("/some/path"));

        let err = ScanError::Io(io::Error::new(io::ErrorKind::NotFound, "not found"));
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn scan_results_are_sorted() {
        let path = test_packages_path("simple");
        let files = scan_directory(&path).unwrap();

        let names: Vec<_> = files
            .iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .collect();

        // Should be alphabetically sorted
        assert_eq!(names, vec!["helper.spl", "main.spl"]);
    }
}
