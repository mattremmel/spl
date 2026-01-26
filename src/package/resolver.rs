//! File inclusion resolution based on module directives.
//!
//! Resolves which files should be included in a module based on:
//! - Auto-include (default: all `.spl` files except `_module.spl`)
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
    /// An explicitly included module was not found.
    ModuleNotFound(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::FileNotFound(file) => write!(f, "file not found: {file}"),
            ResolveError::ModuleNotFound(mod_name) => write!(f, "module not found: {mod_name}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Validates that all explicit includes exist in the available set.
fn validate_explicit_includes<E>(
    includes: &[String],
    available_set: &HashSet<&str>,
    make_error: E,
) -> Result<(), ResolveError>
where
    E: Fn(&String) -> ResolveError,
{
    for inc in includes {
        if !available_set.contains(inc.as_str()) {
            return Err(make_error(inc));
        }
    }
    Ok(())
}

/// Validates that all conditional includes exist when their condition is enabled.
fn validate_conditional_includes<E>(
    conditional_includes: &[(String, String)],
    available_set: &HashSet<&str>,
    enabled_set: &HashSet<&str>,
    make_error: E,
) -> Result<(), ResolveError>
where
    E: Fn(&String) -> ResolveError,
{
    for (condition, item) in conditional_includes {
        if enabled_set.contains(condition.as_str()) && !available_set.contains(item.as_str()) {
            return Err(make_error(item));
        }
    }
    Ok(())
}

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
    let available_set: HashSet<&str> = available.iter().map(AsRef::as_ref).collect();
    let enabled_set: HashSet<&str> = enabled_conditions.iter().map(AsRef::as_ref).collect();

    let mut result = if directives.no_auto_include {
        // Start with empty set, add only explicit includes
        HashSet::new()
    } else {
        // Start with all files except _module.spl
        available_set
            .iter()
            .filter(|&&f| f != "_module.spl")
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
    let available_set: HashSet<&str> = available.iter().map(AsRef::as_ref).collect();
    let enabled_set: HashSet<&str> = enabled_conditions.iter().map(AsRef::as_ref).collect();

    validate_explicit_includes(&directives.includes, &available_set, |f| {
        ResolveError::FileNotFound(f.clone())
    })?;

    validate_conditional_includes(
        &directives.conditional_includes,
        &available_set,
        &enabled_set,
        |f| ResolveError::FileNotFound(f.clone()),
    )?;

    Ok(resolve_includes(available, directives, enabled_conditions))
}

/// Resolve which child modules to include based on directives.
///
/// This is a non-failing version that ignores missing modules.
/// Use [`try_resolve_modules`] for strict validation.
///
/// # Arguments
///
/// * `available` - List of available child module names (directory names)
/// * `directives` - Parsed module directives
/// * `enabled_conditions` - List of enabled condition names (e.g., "debug", "test")
///
/// # Returns
///
/// Sorted list of child module names to include.
pub fn resolve_modules<S1: AsRef<str>, S2: AsRef<str>>(
    available: &[S1],
    directives: &PackageDirectives,
    enabled_conditions: &[S2],
) -> Vec<String> {
    let available_set: HashSet<&str> = available.iter().map(AsRef::as_ref).collect();
    let enabled_set: HashSet<&str> = enabled_conditions.iter().map(AsRef::as_ref).collect();

    let mut result = if directives.no_auto_include_modules {
        // Start with empty set, add only explicit includes
        HashSet::new()
    } else {
        // Start with all available child modules
        available_set.iter().copied().collect::<HashSet<&str>>()
    };

    // Add explicit module includes
    for inc in &directives.module_includes {
        if available_set.contains(inc.as_str()) {
            result.insert(inc.as_str());
        }
    }

    // Add conditional module includes if condition is enabled
    for (condition, mod_name) in &directives.conditional_module_includes {
        if enabled_set.contains(condition.as_str()) && available_set.contains(mod_name.as_str()) {
            result.insert(mod_name.as_str());
        }
    }

    // Remove module excludes
    for exc in &directives.module_excludes {
        result.remove(exc.as_str());
    }

    // Remove conditional module excludes if condition is enabled
    for (condition, mod_name) in &directives.conditional_module_excludes {
        if enabled_set.contains(condition.as_str()) {
            result.remove(mod_name.as_str());
        }
    }

    // Sort for deterministic ordering
    let mut sorted: Vec<String> = result.into_iter().map(String::from).collect();
    sorted.sort();
    sorted
}

/// Resolve modules with strict validation.
///
/// Returns an error if any explicitly included module is not found.
///
/// # Arguments
///
/// * `available` - List of available child module names
/// * `directives` - Parsed module directives
/// * `enabled_conditions` - List of enabled condition names
///
/// # Errors
///
/// Returns `ResolveError::ModuleNotFound` if an explicit include references a missing module.
pub fn try_resolve_modules<S1: AsRef<str>, S2: AsRef<str>>(
    available: &[S1],
    directives: &PackageDirectives,
    enabled_conditions: &[S2],
) -> Result<Vec<String>, ResolveError> {
    let available_set: HashSet<&str> = available.iter().map(AsRef::as_ref).collect();
    let enabled_set: HashSet<&str> = enabled_conditions.iter().map(AsRef::as_ref).collect();

    validate_explicit_includes(&directives.module_includes, &available_set, |m| {
        ResolveError::ModuleNotFound(m.clone())
    })?;

    validate_conditional_includes(
        &directives.conditional_module_includes,
        &available_set,
        &enabled_set,
        |m| ResolveError::ModuleNotFound(m.clone()),
    )?;

    Ok(resolve_modules(available, directives, enabled_conditions))
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
    fn default_mode_excludes_module_spl() {
        let available = vec!["main.spl", "_module.spl"];
        let directives = PackageDirectives::default();

        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["main.spl"]);
        assert!(!result.contains(&"_module.spl".to_string()));
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

        let err = ResolveError::ModuleNotFound("missing".to_string());
        assert!(err.to_string().contains("missing"));
        assert!(err.to_string().contains("module not found"));
    }

    // --- Module resolution tests ---

    #[test]
    fn default_mode_includes_all_modules() {
        let available = vec!["utils", "core", "tests"];
        let directives = PackageDirectives::default();

        let result = resolve_modules(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core", "tests", "utils"]);
    }

    #[test]
    fn no_auto_include_modules_requires_explicit() {
        let available = vec!["utils", "core", "tests"];
        let directives = PackageDirectives {
            no_auto_include_modules: true,
            module_includes: vec!["core".to_string()],
            ..Default::default()
        };

        let result = resolve_modules(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core"]);
    }

    #[test]
    fn exclude_module_removes_from_auto_include() {
        let available = vec!["utils", "core", "tests"];
        let directives = PackageDirectives {
            module_excludes: vec!["tests".to_string()],
            ..Default::default()
        };

        let result = resolve_modules(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core", "utils"]);
        assert!(!result.contains(&"tests".to_string()));
    }

    #[test]
    fn include_module_if_enabled_condition() {
        let available = vec!["core", "debug_tools"];
        let directives = PackageDirectives {
            no_auto_include_modules: true,
            module_includes: vec!["core".to_string()],
            conditional_module_includes: vec![("debug".to_string(), "debug_tools".to_string())],
            ..Default::default()
        };

        let result = resolve_modules(&available, &directives, &["debug"]);

        assert_eq!(result, vec!["core", "debug_tools"]);
    }

    #[test]
    fn include_module_if_disabled_condition() {
        let available = vec!["core", "debug_tools"];
        let directives = PackageDirectives {
            no_auto_include_modules: true,
            module_includes: vec!["core".to_string()],
            conditional_module_includes: vec![("debug".to_string(), "debug_tools".to_string())],
            ..Default::default()
        };

        // Condition not enabled
        let result = resolve_modules(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core"]);
        assert!(!result.contains(&"debug_tools".to_string()));
    }

    #[test]
    fn exclude_module_if_enabled_condition() {
        let available = vec!["core", "tests"];
        let directives = PackageDirectives {
            conditional_module_excludes: vec![("release".to_string(), "tests".to_string())],
            ..Default::default()
        };

        let result = resolve_modules(&available, &directives, &["release"]);

        assert_eq!(result, vec!["core"]);
    }

    #[test]
    fn exclude_module_if_disabled_keeps_module() {
        let available = vec!["core", "tests"];
        let directives = PackageDirectives {
            conditional_module_excludes: vec![("release".to_string(), "tests".to_string())],
            ..Default::default()
        };

        // release not enabled
        let result = resolve_modules(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["core", "tests"]);
    }

    #[test]
    fn include_module_missing_returns_error() {
        let available = vec!["core"];
        let directives = PackageDirectives {
            module_includes: vec!["missing".to_string()],
            ..Default::default()
        };

        let result = try_resolve_modules(&available, &directives, &[] as &[&str]);

        assert!(result.is_err());
        match result.unwrap_err() {
            ResolveError::ModuleNotFound(m) => assert_eq!(m, "missing"),
            e => panic!("expected ModuleNotFound, got {:?}", e),
        }
    }

    #[test]
    fn conditional_include_module_missing_when_enabled_returns_error() {
        let available = vec!["core"];
        let directives = PackageDirectives {
            conditional_module_includes: vec![("debug".to_string(), "missing".to_string())],
            ..Default::default()
        };

        let result = try_resolve_modules(&available, &directives, &["debug"]);

        assert!(result.is_err());
    }

    #[test]
    fn conditional_include_module_missing_when_disabled_ok() {
        let available = vec!["core"];
        let directives = PackageDirectives {
            conditional_module_includes: vec![("debug".to_string(), "missing".to_string())],
            ..Default::default()
        };

        // debug not enabled, so missing module is ok
        let result = try_resolve_modules(&available, &directives, &[] as &[&str]);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec!["core"]);
    }

    #[test]
    fn module_results_are_sorted() {
        let available = vec!["zebra", "alpha", "middle"];
        let directives = PackageDirectives::default();

        let result = resolve_modules(&available, &directives, &[] as &[&str]);

        assert_eq!(result, vec!["alpha", "middle", "zebra"]);
    }

    // --- Conflict resolution tests (exclude wins over include) ---

    #[test]
    fn include_and_exclude_same_file_exclude_wins() {
        let available = vec!["main.spl", "conflict.spl"];
        let directives = PackageDirectives {
            includes: vec!["conflict.spl".to_string()],
            excludes: vec!["conflict.spl".to_string()],
            ..Default::default()
        };

        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        // Exclude should win - conflict.spl NOT included
        assert!(!result.contains(&"conflict.spl".to_string()));
        assert!(result.contains(&"main.spl".to_string()));
    }

    #[test]
    fn include_and_exclude_same_module_exclude_wins() {
        let available = vec!["core", "conflict"];
        let directives = PackageDirectives {
            module_includes: vec!["conflict".to_string()],
            module_excludes: vec!["conflict".to_string()],
            ..Default::default()
        };

        let result = resolve_modules(&available, &directives, &[] as &[&str]);

        // Exclude should win - conflict NOT included
        assert!(!result.contains(&"conflict".to_string()));
        assert!(result.contains(&"core".to_string()));
    }

    #[test]
    fn conditional_include_and_exclude_same_file_exclude_wins() {
        let available = vec!["main.spl", "conflict.spl"];
        let directives = PackageDirectives {
            conditional_includes: vec![("test".to_string(), "conflict.spl".to_string())],
            conditional_excludes: vec![("test".to_string(), "conflict.spl".to_string())],
            no_auto_include: true,
            includes: vec!["main.spl".to_string()],
            ..Default::default()
        };

        // Both conditions enabled - exclude should win
        let result = resolve_includes(&available, &directives, &["test"]);

        assert!(!result.contains(&"conflict.spl".to_string()));
        assert!(result.contains(&"main.spl".to_string()));
    }

    #[test]
    fn multiple_conditions_interact_correctly() {
        let available = vec!["main.spl", "debug.spl", "test.spl", "prod.spl"];
        let directives = PackageDirectives {
            conditional_includes: vec![
                ("debug".to_string(), "debug.spl".to_string()),
                ("test".to_string(), "test.spl".to_string()),
            ],
            conditional_excludes: vec![("prod".to_string(), "test.spl".to_string())],
            no_auto_include: true,
            includes: vec!["main.spl".to_string()],
            ..Default::default()
        };

        // debug + test + prod all enabled
        let result = resolve_includes(&available, &directives, &["debug", "test", "prod"]);

        assert!(result.contains(&"main.spl".to_string()));
        assert!(result.contains(&"debug.spl".to_string()));
        // test.spl included by test condition, but excluded by prod condition - exclude wins
        assert!(!result.contains(&"test.spl".to_string()));
    }

    #[test]
    fn exclude_all_files_returns_empty() {
        let available = vec!["main.spl", "lib.spl"];
        let directives = PackageDirectives {
            excludes: vec!["main.spl".to_string(), "lib.spl".to_string()],
            ..Default::default()
        };

        let result = resolve_includes(&available, &directives, &[] as &[&str]);

        assert!(result.is_empty());
    }
}
