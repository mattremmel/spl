//! Program execution for run-pass and run-fail tests.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

/// Result of executing an SPL program.
#[derive(Debug)]
pub struct ExecuteResult {
    /// The return value from `main()`.
    pub return_value: i32,
    /// Standard output from the program.
    pub stdout: String,
    /// Standard error from the program.
    pub stderr: String,
}

/// Errors that can occur during program execution.
#[derive(Debug, Error)]
pub enum ExecuteError {
    /// Compilation failed.
    #[error("compilation failed: {0}")]
    CompileFailed(spl_compiler::AotError),

    /// Failed to execute the compiled program.
    #[error("execution failed: {0}")]
    ExecutionFailed(std::io::Error),
}

impl From<spl_compiler::AotError> for ExecuteError {
    fn from(err: spl_compiler::AotError) -> Self {
        ExecuteError::CompileFailed(err)
    }
}

/// Execute SPL source code and capture its output.
///
/// Compiles to an executable, runs it as a subprocess, and captures stdout/return value.
pub fn execute_captured(source: &str) -> Result<ExecuteResult, ExecuteError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    // Create a unique temp file for the executable
    let temp_dir = std::env::temp_dir();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let exe_name = format!("spl_test_{}_{}", std::process::id(), counter);
    let exe_path = temp_dir.join(exe_name);

    // Compile and link
    spl_compiler::compile_and_link(source, &exe_path)?;

    // Execute and capture output
    let output = Command::new(&exe_path)
        .output()
        .map_err(ExecuteError::ExecutionFailed)?;

    // Clean up
    let _ = std::fs::remove_file(&exe_path);

    Ok(ExecuteResult {
        return_value: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_simple_return() {
        let result = execute_captured("fn main(): i32 { 42 }").unwrap();
        assert_eq!(result.return_value, 42);
    }

    #[test]
    fn execute_zero_return() {
        let result = execute_captured("fn main(): i32 { 0 }").unwrap();
        assert_eq!(result.return_value, 0);
    }
}
