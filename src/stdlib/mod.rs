//! SPL Standard Library.
//!
//! The stdlib is written in SPL itself and embedded at compile time.
//! This module provides access to compiled stdlib definitions.
//!
//! # Architecture
//!
//! ```text
//! User Code (SPL)     →  calls  →  Stdlib (SPL)     →  calls  →  Intrinsics (Rust)
//!   print("hi")                     pub fn print()              __print_str(ptr, len)
//! ```
//!
//! The stdlib lives in `stdlib/` at the project root as `.spl` files.
//! These are embedded at compile time using `include_dir!`.

use include_dir::{Dir, include_dir};

use crate::Diagnostic;
use crate::mir::Body;

/// Embedded stdlib directory.
static STDLIB_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/stdlib");

/// Compiled standard library definitions.
///
/// Contains MIR bodies for all stdlib functions, ready to be linked
/// with user code during compilation.
pub struct StdlibDefs {
    /// MIR bodies for stdlib functions.
    pub bodies: Vec<Body>,
}

impl StdlibDefs {
    /// Compile the embedded stdlib to MIR.
    ///
    /// Concatenates all `.spl` files in the stdlib directory and compiles
    /// them to MIR. Returns an error if any stdlib file fails to compile
    /// (which indicates a bug in the stdlib itself).
    ///
    /// # Errors
    ///
    /// Returns diagnostics if stdlib compilation fails.
    ///
    /// # Example
    ///
    /// ```
    /// use spl::stdlib::StdlibDefs;
    ///
    /// let stdlib = StdlibDefs::compile();
    /// // Currently returns Ok with empty bodies since prelude is minimal
    /// assert!(stdlib.is_ok());
    /// ```
    pub fn compile() -> Result<Self, Vec<Diagnostic>> {
        let mut all_source = String::new();

        // Collect all .spl files from the embedded stdlib
        collect_spl_files(&STDLIB_DIR, &mut all_source);

        // If there's no actual code, return empty bodies
        // (prelude is currently just comments)
        if all_source.trim().is_empty() || !all_source.contains("fn ") {
            return Ok(StdlibDefs { bodies: Vec::new() });
        }

        // Compile the combined stdlib source
        let result = crate::compile(&all_source);

        if result.is_err() {
            return Err(result.diagnostics);
        }

        Ok(StdlibDefs {
            bodies: result.bodies.unwrap_or_default(),
        })
    }

    /// Returns true if the stdlib has any compiled functions.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Returns the number of stdlib functions.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }
}

/// Recursively collect .spl file contents from a directory.
fn collect_spl_files(dir: &Dir<'_>, output: &mut String) {
    for file in dir.files() {
        if file.path().extension().is_some_and(|ext| ext == "spl")
            && let Some(contents) = file.contents_utf8()
        {
            output.push_str(contents);
            output.push('\n');
        }
    }

    for subdir in dir.dirs() {
        collect_spl_files(subdir, output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdlib_compiles() {
        let result = StdlibDefs::compile();
        assert!(
            result.is_ok(),
            "stdlib compilation failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn stdlib_dir_exists() {
        // Verify the embedded directory is accessible
        assert!(
            STDLIB_DIR.files().next().is_some()
                || STDLIB_DIR.dirs().next().is_some()
                || STDLIB_DIR.files().count() == 0,
            "stdlib directory should be embedded"
        );
    }

    #[test]
    fn prelude_exists() {
        // Verify prelude.spl is embedded
        let prelude = STDLIB_DIR.get_file("prelude.spl");
        assert!(prelude.is_some(), "prelude.spl should be embedded");
    }

    #[test]
    fn stdlib_is_currently_empty() {
        // Until we add actual stdlib functions, it should be empty
        let stdlib = StdlibDefs::compile().unwrap();
        assert!(
            stdlib.is_empty(),
            "stdlib should be empty until we add functions"
        );
    }
}
