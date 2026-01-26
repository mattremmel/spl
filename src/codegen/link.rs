//! Linker abstraction for AOT compilation.
//!
//! This module provides a trait-based linker abstraction that allows linking
//! object files into executables. The default implementation uses the system's
//! C compiler (`cc`).

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::error::CodegenError;

/// Options for the linker.
#[derive(Debug, Clone, Default)]
pub struct LinkOptions {
    /// Libraries to link against (-l flags).
    pub libraries: Vec<String>,
    /// Library search paths (-L flags).
    pub library_paths: Vec<PathBuf>,
    /// Extra arguments to pass through to the linker.
    pub extra_args: Vec<String>,
}

impl LinkOptions {
    /// Create new empty link options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a library to link against.
    pub fn library(mut self, name: impl Into<String>) -> Self {
        self.libraries.push(name.into());
        self
    }

    /// Add a library search path.
    pub fn library_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.library_paths.push(path.into());
        self
    }

    /// Add an extra argument to pass to the linker.
    pub fn extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }
}

/// Error that can occur during linking.
#[derive(Debug)]
pub enum LinkError {
    /// Failed to write object file to disk.
    WriteObjectFile(io::Error),
    /// Failed to spawn the linker process.
    SpawnLinker(io::Error),
    /// Linker exited with a non-zero status.
    LinkerFailed {
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    /// Failed to read the output binary.
    ReadBinary(io::Error),
    /// Generic IO error.
    Io(io::Error),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::WriteObjectFile(e) => write!(f, "failed to write object file: {e}"),
            LinkError::SpawnLinker(e) => write!(f, "failed to spawn linker: {e}"),
            LinkError::LinkerFailed { status, stderr, .. } => {
                if let Some(code) = status {
                    write!(f, "linker failed with exit code {code}: {stderr}")
                } else {
                    write!(f, "linker terminated by signal: {stderr}")
                }
            }
            LinkError::ReadBinary(e) => write!(f, "failed to read binary: {e}"),
            LinkError::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for LinkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LinkError::WriteObjectFile(e)
            | LinkError::SpawnLinker(e)
            | LinkError::ReadBinary(e)
            | LinkError::Io(e) => Some(e),
            LinkError::LinkerFailed { .. } => None,
        }
    }
}

impl From<io::Error> for LinkError {
    fn from(e: io::Error) -> Self {
        LinkError::Io(e)
    }
}

impl From<LinkError> for CodegenError {
    fn from(e: LinkError) -> Self {
        CodegenError::ModuleError(e.to_string())
    }
}

/// Trait for linkers that can produce executables from object files.
///
/// This abstraction allows for different linker implementations (cc, lld, mold, etc.)
/// while providing a consistent interface.
pub trait Linker {
    /// Link object files into an executable.
    ///
    /// # Arguments
    /// * `objects` - Slice of paths to object files
    /// * `output` - Path where the executable should be written
    /// * `options` - Linker options (libraries, search paths, etc.)
    fn link(
        &self,
        objects: &[&Path],
        output: &Path,
        options: &LinkOptions,
    ) -> Result<(), LinkError>;
}

/// Linker implementation that uses the system C compiler (`cc`).
///
/// This is the default and most portable linker implementation. It uses the
/// `CC` environment variable if set, otherwise falls back to `cc`.
///
/// On macOS, this uses Apple's clang. On Linux, this typically uses gcc or clang.
#[derive(Debug, Clone, Default)]
pub struct CcLinker {
    /// Override the compiler command (default: use CC env var or "cc").
    compiler: Option<String>,
}

impl CcLinker {
    /// Create a new CC linker with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a CC linker with a specific compiler command.
    pub fn with_compiler(compiler: impl Into<String>) -> Self {
        Self {
            compiler: Some(compiler.into()),
        }
    }

    /// Get the compiler command to use.
    fn compiler_command(&self) -> String {
        if let Some(ref cc) = self.compiler {
            cc.clone()
        } else {
            std::env::var("CC").unwrap_or_else(|_| "cc".to_string())
        }
    }

    /// Build the command-line arguments for linking.
    fn build_args(&self, objects: &[&Path], output: &Path, options: &LinkOptions) -> Vec<String> {
        let mut args = Vec::new();

        // Output file
        args.push("-o".to_string());
        args.push(output.to_string_lossy().into_owned());

        // Object files
        for obj in objects {
            args.push(obj.to_string_lossy().into_owned());
        }

        // Library search paths
        for path in &options.library_paths {
            args.push(format!("-L{}", path.display()));  // path.display() can't be inlined
        }

        // Libraries
        for lib in &options.libraries {
            args.push(format!("-l{lib}"));
        }

        // Extra arguments
        args.extend(options.extra_args.clone());

        args
    }
}

impl Linker for CcLinker {
    fn link(
        &self,
        objects: &[&Path],
        output: &Path,
        options: &LinkOptions,
    ) -> Result<(), LinkError> {
        let cc = self.compiler_command();
        let args = self.build_args(objects, output, options);

        let output_result = Command::new(&cc)
            .args(&args)
            .output()
            .map_err(LinkError::SpawnLinker)?;

        if !output_result.status.success() {
            return Err(LinkError::LinkerFailed {
                status: output_result.status.code(),
                stdout: String::from_utf8_lossy(&output_result.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output_result.stderr).into_owned(),
            });
        }

        Ok(())
    }
}

/// Helper function to link object bytes to an executable.
///
/// This is a convenience function that:
/// 1. Writes the object bytes to a temporary file
/// 2. Links using the CC linker
/// 3. Returns success/failure
///
/// # Arguments
/// * `object_bytes` - The raw object file bytes
/// * `output` - Path where the executable should be written
/// * `options` - Optional linker options
pub fn link_object_to_executable(
    object_bytes: &[u8],
    output: &Path,
    options: Option<&LinkOptions>,
) -> Result<(), LinkError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    // Create a unique temporary file for the object using process ID, timestamp, and counter
    let temp_dir = std::env::temp_dir();
    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let obj_path = temp_dir.join(format!(
        "spl_temp_{}_{}_{}.o",
        std::process::id(),
        unique_id,
        counter
    ));

    // Write object bytes to temp file
    let mut file = std::fs::File::create(&obj_path).map_err(LinkError::WriteObjectFile)?;
    file.write_all(object_bytes)
        .map_err(LinkError::WriteObjectFile)?;
    drop(file);

    // Link using CC linker
    let linker = CcLinker::new();
    let default_options = LinkOptions::new();
    let opts = options.unwrap_or(&default_options);

    let result = linker.link(&[obj_path.as_path()], output, opts);

    // Clean up temp file (ignore errors)
    let _ = std::fs::remove_file(&obj_path);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_options_builder() {
        let opts = LinkOptions::new()
            .library("m")
            .library("pthread")
            .library_path("/usr/local/lib")
            .extra_arg("-static");

        assert_eq!(opts.libraries, vec!["m", "pthread"]);
        assert_eq!(opts.library_paths, vec![PathBuf::from("/usr/local/lib")]);
        assert_eq!(opts.extra_args, vec!["-static"]);
    }

    #[test]
    fn test_cc_linker_default() {
        let linker = CcLinker::new();
        // Should not panic
        let _ = linker.compiler_command();
    }

    #[test]
    fn test_cc_linker_with_compiler() {
        let linker = CcLinker::with_compiler("clang");
        assert_eq!(linker.compiler_command(), "clang");
    }

    #[test]
    fn test_cc_linker_build_args() {
        let linker = CcLinker::new();
        let obj1 = Path::new("/tmp/a.o");
        let obj2 = Path::new("/tmp/b.o");
        let output = Path::new("/tmp/out");

        let opts = LinkOptions::new().library("m").library_path("/usr/lib");

        let args = linker.build_args(&[obj1, obj2], output, &opts);

        assert!(args.contains(&"-o".to_string()));
        assert!(args.contains(&"/tmp/out".to_string()));
        assert!(args.contains(&"/tmp/a.o".to_string()));
        assert!(args.contains(&"/tmp/b.o".to_string()));
        assert!(args.contains(&"-L/usr/lib".to_string()));
        assert!(args.contains(&"-lm".to_string()));
    }

    #[test]
    fn test_link_error_display() {
        let err = LinkError::LinkerFailed {
            status: Some(1),
            stdout: String::new(),
            stderr: "undefined reference to main".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("linker failed"));
        assert!(msg.contains("undefined reference"));
    }

    #[test]
    fn test_link_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let link_err: LinkError = io_err.into();
        assert!(matches!(link_err, LinkError::Io(_)));
    }

    #[test]
    fn test_link_error_source() {
        use std::error::Error;
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let link_err = LinkError::WriteObjectFile(io_err);

        // Verify source() returns the underlying error
        assert!(link_err.source().is_some());
    }

    #[test]
    fn test_link_error_linker_failed_no_source() {
        use std::error::Error;
        let link_err = LinkError::LinkerFailed {
            status: Some(1),
            stdout: String::new(),
            stderr: "error".to_string(),
        };

        // LinkerFailed has no source error
        assert!(link_err.source().is_none());
    }

    #[test]
    fn test_link_error_display_all_variants() {
        // Test all error variant display messages
        let err1 = LinkError::WriteObjectFile(io::Error::other("write"));
        assert!(err1.to_string().contains("write object file"));

        let err2 = LinkError::SpawnLinker(io::Error::new(io::ErrorKind::NotFound, "not found"));
        assert!(err2.to_string().contains("spawn linker"));

        let err3 = LinkError::LinkerFailed {
            status: None,
            stdout: String::new(),
            stderr: "killed".to_string(),
        };
        assert!(err3.to_string().contains("terminated by signal"));

        let err4 = LinkError::ReadBinary(io::Error::other("read"));
        assert!(err4.to_string().contains("read binary"));

        let err5 = LinkError::Io(io::Error::other("generic"));
        assert!(err5.to_string().contains("IO error"));
    }

    #[test]
    fn test_link_options_empty() {
        let opts = LinkOptions::new();
        assert!(opts.libraries.is_empty());
        assert!(opts.library_paths.is_empty());
        assert!(opts.extra_args.is_empty());
    }

    #[test]
    fn test_link_options_default() {
        let opts = LinkOptions::default();
        assert!(opts.libraries.is_empty());
    }

    #[test]
    fn test_cc_linker_respects_cc_env() {
        // Save current CC value
        let old_cc = std::env::var("CC").ok();

        // SAFETY: This test is single-threaded and we restore the old value
        unsafe {
            std::env::set_var("CC", "custom-cc");
        }

        let linker = CcLinker::new();
        assert_eq!(linker.compiler_command(), "custom-cc");

        // Restore old CC
        // SAFETY: Restoring previous environment state
        unsafe {
            match old_cc {
                Some(val) => std::env::set_var("CC", val),
                None => std::env::remove_var("CC"),
            }
        }
    }

    #[test]
    fn test_link_error_to_codegen_error() {
        use crate::codegen::CodegenError;

        let link_err = LinkError::LinkerFailed {
            status: Some(1),
            stdout: String::new(),
            stderr: "undefined symbol".to_string(),
        };

        let codegen_err: CodegenError = link_err.into();
        assert!(matches!(codegen_err, CodegenError::ModuleError(_)));
        assert!(codegen_err.to_string().contains("undefined symbol"));
    }

    #[test]
    fn test_cc_linker_clone_and_debug() {
        let linker = CcLinker::with_compiler("gcc");
        let cloned = linker.clone();
        assert_eq!(cloned.compiler_command(), "gcc");

        // Test Debug impl
        let debug_str = format!("{:?}", linker);
        assert!(debug_str.contains("CcLinker"));
    }

    #[test]
    fn test_link_options_clone_and_debug() {
        let opts = LinkOptions::new().library("m");
        let cloned = opts.clone();
        assert_eq!(cloned.libraries, vec!["m"]);

        // Test Debug impl
        let debug_str = format!("{:?}", opts);
        assert!(debug_str.contains("LinkOptions"));
    }
}
