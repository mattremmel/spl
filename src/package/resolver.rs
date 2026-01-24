//! File inclusion resolution based on package directives.
//!
//! Resolves which files should be included in a package based on:
//! - Auto-include (default: all `.spl` files except `_package.spl`)
//! - Explicit includes via `#![include("file")]`
//! - Excludes via `#![exclude("file")]`
//! - Conditional includes/excludes via `#![include_if(cond, "file")]`

use super::PackageDirectives;
use std::collections::HashSet;
use std::fmt;

/// Errors that can occur during file resolution.
#[derive(Debug, Clone)]
pub enum ResolveError {
    /// An explicitly included file was not found.
    FileNotFound(String),
    /// An explicitly included package was not found.
    PackageNotFound(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::FileNotFound(file) => write!(f, "file not found: {}", file),
            ResolveError::PackageNotFound(pkg) => write!(f, "package not found: {}", pkg),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolve which files to include based on directives.
///
/// This is a non-failing version that ignores missing files.
/// Use [`try_resolve_includes`] for strict validation.
///
/// # Arguments
///
/// * `available` - List of available file names in the package directory
/// * `directives` - Parsed package directives
/// * `enabled_conditions` - List of enabled condition names (e.g., "debug", "test")
///
/// # Returns
///
/// Sorted list of file names to include.
pub fn resolve_includes<S1: AsRef<str>, S2: AsRef<str>>(
    available: &[S1],
    directives: &PackageDirectives,
    enabled_conditions: &[S2],
) -> Vec<String> {
    let available_set: HashSet<&str> = available.iter().map(|s| s.as_ref()).collect();
    let enabled_set: HashSet<&str> = enabled_conditions.iter().map(|s| s.as_ref()).collect();

    let mut result = if directives.no_auto_include {
        // Start with empty set, add only explicit includes
        HashSet::new()
    } else {
        // Start with all files except _package.spl
        available_set
            .iter()
            .filter(|&&f| f != "_package.spl")
            .copied()
            .collect::<HashSet<&str>>()
    };

    // Add explicit includes
    for inc in &directives.includes {
        if available_set.contains(inc.as_str()) {
            result.insert(inc.as_str());
        }
    }

    // Add conditional includes if condition is enabled
    for (condition, file) in &directives.conditional_includes {
        if enabled_set.contains(condition.as_str()) && available_set.contains(file.as_str()) {
            result.insert(file.as_str());
        }
    }

    // Remove excludes
    for exc in &directives.excludes {
        result.remove(exc.as_str());
    }

    // Remove conditional excludes if condition is enabled
    for (condition, file) in &directives.conditional_excludes {
        if enabled_set.contains(condition.as_str()) {
            result.remove(file.as_str());
        }
    }

    // Sort for deterministic ordering
    let mut sorted: Vec<String> = result.into_iter().map(String::from).collect();
    sorted.sort();
    sorted
}

/// Resolve includes with strict validation.
///
/// Returns an error if any explicitly included file is not found.
///
/// # Arguments
///
/// * `available` - List of available file names in the package directory
/// * `directives` - Parsed package directives
/// * `enabled_conditions` - List of enabled condition names
///
/// # Errors
///
/// Returns `ResolveError::FileNotFound` if an explicit include references a missing file.
pub fn try_resolve_includes<S1: AsRef<str>, S2: AsRef<str>>(
    available: &[S1],
    directives: &PackageDirectives,
    enabled_conditions: &[S2],
) -> Result<Vec<String>, ResolveError> {
    let available_set: HashSet<&str> = available.iter().map(|s| s.as_ref()).collect();
    let enabled_set: HashSet<&str> = enabled_conditions.iter().map(|s| s.as_ref()).collect();

    // Validate explicit includes exist
    for inc in &directives.includes {
        if !available_set.contains(inc.as_str()) {
            return Err(ResolveError::FileNotFound(inc.clone()));
        }
    }

    // Validate conditional includes exist (when condition is enabled)
    for (condition, file) in &directives.conditional_includes {
        if enabled_set.contains(condition.as_str()) && !available_set.contains(file.as_str()) {
            return Err(ResolveError::FileNotFound(file.clone()));
        }
    }

    Ok(resolve_includes(available, directives, enabled_conditions))
}

/// Resolve which subpackages to include based on directives.
///
/// This is a non-failing version that ignores missing packages.
/// Use [`try_resolve_packages`] for strict validation.
///
/// # Arguments
///
/// * `available` - List of available subpackage names (directory names)
/// * `directives` - Parsed package directives
/// * `enabled_conditions` - List of enabled condition names (e.g., "debug", "test")
///
/// # Returns
///
/// Sorted list of subpackage names to include.
pub fn resolve_packages<S1: AsRef<str>, S2: AsRef<str>>(
    available: &[S1],
    directives: &PackageDirectives,
    enabled_conditions: &[S2],
) -> Vec<String> {
    let available_set: HashSet<&str> = available.iter().map(|s| s.as_ref()).collect();
    let enabled_set: HashSet<&str> = enabled_conditions.iter().map(|s| s.as_ref()).collect();

    let mut result = if directives.no_auto_include_packages {
        // Start with empty set, add only explicit includes
        HashSet::new()
    } else {
        // Start with all available subpackages
        available_set.iter().copied().collect::<HashSet<&str>>()
    };

    // Add explicit package includes
    for inc in &directives.package_includes {
        if available_set.contains(inc.as_str()) {
            result.insert(inc.as_str());
        }
    }

    // Add conditional package includes if condition is enabled
    for (condition, pkg) in &directives.conditional_package_includes {
        if enabled_set.contains(condition.as_str()) && available_set.contains(pkg.as_str()) {
            result.insert(pkg.as_str());
        }
    }

    // Remove package excludes
    for exc in &directives.package_excludes {
        result.remove(exc.as_str());
    }

    // Remove conditional package excludes if condition is enabled
    for (condition, pkg) in &directives.conditional_package_excludes {
        if enabled_set.contains(condition.as_str()) {
            result.remove(pkg.as_str());
        }
    }

    // Sort for deterministic ordering
    let mut sorted: Vec<String> = result.into_iter().map(String::from).collect();
    sorted.sort();
    sorted
}

/// Resolve packages with strict validation.
///
/// Returns an error if any explicitly included package is not found.
///
/// # Arguments
///
/// * `available` - List of available subpackage names
/// * `directives` - Parsed package directives
/// * `enabled_conditions` - List of enabled condition names
///
/// # Errors
///
/// Returns `ResolveError::PackageNotFound` if an explicit include references a missing package.
pub fn try_resolve_packages<S1: AsRef<str>, S2: AsRef<str>>(
    available: &[S1],
    directives: &PackageDirectives,
    enabled_conditions: &[S2],
) -> Result<Vec<String>, ResolveError> {
    let available_set: HashSet<&str> = available.iter().map(|s| s.as_ref()).collect();
    let enabled_set: HashSet<&str> = enabled_conditions.iter().map(|s| s.as_ref()).collect();

    // Validate explicit package includes exist
    for inc in &directives.package_includes {
        if !available_set.contains(inc.as_str()) {
            return Err(ResolveError::PackageNotFound(inc.clone()));
        }
    }

    // Validate conditional package includes exist (when condition is enabled)
    for (condition, pkg) in &directives.conditional_package_includes {
        if enabled_set.contains(condition.as_str()) && !available_set.contains(pkg.as_str()) {
            return Err(ResolveError::PackageNotFound(pkg.clone()));
        }
    }

    Ok(resolve_packages(available, directives, enabled_conditions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_includes_all_spl_files() {
        let available = vec!["main.spl", "lib.spl", "utils.spl"];
        let directives = PackageDirectives::default();

        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["lib.spl", "main.spl", "utils.spl"]);
    }

    #[test]
    fn default_mode_excludes_package_spl() {
        let available = vec!["main.spl", "_package.spl"];
        let directives = PackageDirectives::default();

        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["main.spl"]);
        assert!(!result.contains(&"_package.spl".to_string()));
    }

    #[test]
    fn no_auto_include_requires_explicit() {
        let available = vec!["main.spl", "lib.spl", "unused.spl"];
        let directives = PackageDirectives {
            no_auto_include: true,
            includes: vec!["main.spl".to_string()],
            ..Default::default()
        };

        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["main.spl"]);
    }

    #[test]
    fn exclude_removes_from_auto_include() {
        let available = vec!["main.spl", "test.spl", "lib.spl"];
        let directives = PackageDirectives {
            excludes: vec!["test.spl".to_string()],
            ..Default::default()
        };

        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["lib.spl", "main.spl"]);
        assert!(!result.contains(&"test.spl".to_string()));
    }

    #[test]
    fn include_if_enabled_condition() {
        let available = vec!["main.spl", "debug.spl"];
        let directives = PackageDirectives {
            no_auto_include: true,
            includes: vec!["main.spl".to_string()],
            conditional_includes: vec![("debug".to_string(), "debug.spl".to_string())],
            ..Default::default()
        };

        let result = resolve_includes(&available, &directives, &["debug"]);

        assert_eq!(result, vec!["debug.spl", "main.spl"]);
    }

    #[test]
    fn include_if_disabled_condition() {
        let available = vec!["main.spl", "debug.spl"];
        let directives = PackageDirectives {
            no_auto_include: true,
            includes: vec!["main.spl".to_string()],
            conditional_includes: vec![("debug".to_string(), "debug.spl".to_string())],
            ..Default::default()
        };

        // Condition not enabled
        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["main.spl"]);
        assert!(!result.contains(&"debug.spl".to_string()));
    }

    #[test]
    fn exclude_if_enabled_condition() {
        let available = vec!["main.spl", "test.spl"];
        let directives = PackageDirectives {
            conditional_excludes: vec![("release".to_string(), "test.spl".to_string())],
            ..Default::default()
        };

        let result = resolve_includes(&available, &directives, &["release"]);

        assert_eq!(result, vec!["main.spl"]);
    }

    #[test]
    fn exclude_if_disabled_keeps_file() {
        let available = vec!["main.spl", "test.spl"];
        let directives = PackageDirectives {
            conditional_excludes: vec![("release".to_string(), "test.spl".to_string())],
            ..Default::default()
        };

        // release not enabled
        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["main.spl", "test.spl"]);
    }

    #[test]
    fn include_missing_file_returns_error() {
        let available = vec!["main.spl"];
        let directives = PackageDirectives {
            includes: vec!["missing.spl".to_string()],
            ..Default::default()
        };

        let result = try_resolve_includes(&available, &directives, &[] as &[&str]);

        assert!(result.is_err());
        match result.unwrap_err() {
            ResolveError::FileNotFound(f) => assert_eq!(f, "missing.spl"),
            e => panic!("expected FileNotFound, got {:?}", e),
        }
    }

    #[test]
    fn conditional_include_missing_when_enabled_returns_error() {
        let available = vec!["main.spl"];
        let directives = PackageDirectives {
            conditional_includes: vec![("debug".to_string(), "missing.spl".to_string())],
            ..Default::default()
        };

        let result = try_resolve_includes(&available, &directives, &["debug"]);

        assert!(result.is_err());
    }

    #[test]
    fn conditional_include_missing_when_disabled_ok() {
        let available = vec!["main.spl"];
        let directives = PackageDirectives {
            conditional_includes: vec![("debug".to_string(), "missing.spl".to_string())],
            ..Default::default()
        };

        // debug not enabled, so missing file is ok
        let result = try_resolve_includes(&available, &directives, &[] as &[&str]);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec!["main.spl"]);
    }

    #[test]
    fn results_are_sorted() {
        let available = vec!["z.spl", "a.spl", "m.spl"];
        let directives = PackageDirectives::default();

        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["a.spl", "m.spl", "z.spl"]);
    }

    #[test]
    fn resolve_error_display() {
        let err = ResolveError::FileNotFound("test.spl".to_string());
        assert!(err.to_string().contains("test.spl"));
        assert!(err.to_string().contains("not found"));

        let err = ResolveError::PackageNotFound("missing".to_string());
        assert!(err.to_string().contains("missing"));
        assert!(err.to_string().contains("package not found"));
    }

    // --- Package resolution tests ---

    #[test]
    fn default_mode_includes_all_packages() {
        let available = vec!["utils", "core", "tests"];
        let directives = PackageDirectives::default();

        let result = resolve_packages(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core", "tests", "utils"]);
    }

    #[test]
    fn no_auto_include_packages_requires_explicit() {
        let available = vec!["utils", "core", "tests"];
        let directives = PackageDirectives {
            no_auto_include_packages: true,
            package_includes: vec!["core".to_string()],
            ..Default::default()
        };

        let result = resolve_packages(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core"]);
    }

    #[test]
    fn exclude_package_removes_from_auto_include() {
        let available = vec!["utils", "core", "tests"];
        let directives = PackageDirectives {
            package_excludes: vec!["tests".to_string()],
            ..Default::default()
        };

        let result = resolve_packages(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core", "utils"]);
        assert!(!result.contains(&"tests".to_string()));
    }

    #[test]
    fn include_package_if_enabled_condition() {
        let available = vec!["core", "debug_tools"];
        let directives = PackageDirectives {
            no_auto_include_packages: true,
            package_includes: vec!["core".to_string()],
            conditional_package_includes: vec![("debug".to_string(), "debug_tools".to_string())],
            ..Default::default()
        };

        let result = resolve_packages(&available, &directives, &["debug"]);

        assert_eq!(result, vec!["core", "debug_tools"]);
    }

    #[test]
    fn include_package_if_disabled_condition() {
        let available = vec!["core", "debug_tools"];
        let directives = PackageDirectives {
            no_auto_include_packages: true,
            package_includes: vec!["core".to_string()],
            conditional_package_includes: vec![("debug".to_string(), "debug_tools".to_string())],
            ..Default::default()
        };

        // Condition not enabled
        let result = resolve_packages(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core"]);
        assert!(!result.contains(&"debug_tools".to_string()));
    }

    #[test]
    fn exclude_package_if_enabled_condition() {
        let available = vec!["core", "tests"];
        let directives = PackageDirectives {
            conditional_package_excludes: vec![("release".to_string(), "tests".to_string())],
            ..Default::default()
        };

        let result = resolve_packages(&available, &directives, &["release"]);

        assert_eq!(result, vec!["core"]);
    }

    #[test]
    fn exclude_package_if_disabled_keeps_package() {
        let available = vec!["core", "tests"];
        let directives = PackageDirectives {
            conditional_package_excludes: vec![("release".to_string(), "tests".to_string())],
            ..Default::default()
        };

        // release not enabled
        let result = resolve_packages(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core", "tests"]);
    }

    #[test]
    fn include_package_missing_returns_error() {
        let available = vec!["core"];
        let directives = PackageDirectives {
            package_includes: vec!["missing".to_string()],
            ..Default::default()
        };

        let result = try_resolve_packages(&available, &directives, &[] as &[&str]);

        assert!(result.is_err());
        match result.unwrap_err() {
            ResolveError::PackageNotFound(p) => assert_eq!(p, "missing"),
            e => panic!("expected PackageNotFound, got {:?}", e),
        }
    }

    #[test]
    fn conditional_include_package_missing_when_enabled_returns_error() {
        let available = vec!["core"];
        let directives = PackageDirectives {
            conditional_package_includes: vec![("debug".to_string(), "missing".to_string())],
            ..Default::default()
        };

        let result = try_resolve_packages(&available, &directives, &["debug"]);

        assert!(result.is_err());
    }

    #[test]
    fn conditional_include_package_missing_when_disabled_ok() {
        let available = vec!["core"];
        let directives = PackageDirectives {
            conditional_package_includes: vec![("debug".to_string(), "missing".to_string())],
            ..Default::default()
        };

        // debug not enabled, so missing package is ok
        let result = try_resolve_packages(&available, &directives, &[] as &[&str]);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec!["core"]);
    }

    #[test]
    fn package_results_are_sorted() {
        let available = vec!["zebra", "alpha", "middle"];
        let directives = PackageDirectives::default();

        let result = resolve_packages(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["alpha", "middle", "zebra"]);
    }
}
