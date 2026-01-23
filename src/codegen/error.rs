//! Code generation error types.
//!
//! This module defines error types for the Cranelift code generation backend.

use std::fmt;

// =============================================================================
// Trap codes for runtime errors
// =============================================================================

/// Trap code for unreachable code.
pub const TRAP_UNREACHABLE: u8 = 0;

/// Trap code for failed assertions.
pub const TRAP_ASSERT_FAILED: u8 = 1;

/// Trap code for unwinding resume.
pub const TRAP_RESUME: u8 = 2;

/// Errors that can occur during code generation.
#[derive(Debug)]
pub enum CodegenError {
    /// The target platform is not supported.
    UnsupportedTarget(String),

    /// Failed to configure the target ISA.
    IsaConfiguration(String),

    /// An error occurred in the Cranelift module.
    ModuleError(String),

    /// A type is not supported for code generation.
    UnsupportedType(String),

    /// An internal compiler error occurred.
    Internal(String),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::UnsupportedTarget(target) => {
                write!(f, "unsupported target: {}", target)
            }
            CodegenError::IsaConfiguration(msg) => {
                write!(f, "ISA configuration error: {}", msg)
            }
            CodegenError::ModuleError(msg) => {
                write!(f, "module error: {}", msg)
            }
            CodegenError::UnsupportedType(ty) => {
                write!(f, "unsupported type for code generation: {}", ty)
            }
            CodegenError::Internal(msg) => {
                write!(f, "internal codegen error: {}", msg)
            }
        }
    }
}

impl std::error::Error for CodegenError {}

// =============================================================================
// Runtime errors
// =============================================================================

/// Errors that can occur during runtime execution of compiled code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// The main function was not found in the compiled module.
    MainNotFound,

    /// A trap occurred during execution.
    Trap {
        /// The trap code (see TRAP_* constants).
        code: Option<u8>,
    },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::MainNotFound => write!(f, "main function not found"),
            RuntimeError::Trap { code: Some(c) } => write!(f, "trap occurred: code {}", c),
            RuntimeError::Trap { code: None } => write!(f, "trap occurred"),
        }
    }
}

impl std::error::Error for RuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_unsupported_target() {
        let err = CodegenError::UnsupportedTarget("arm32".to_string());
        assert_eq!(err.to_string(), "unsupported target: arm32");
    }

    #[test]
    fn error_display_isa_configuration() {
        let err = CodegenError::IsaConfiguration("invalid flags".to_string());
        assert_eq!(err.to_string(), "ISA configuration error: invalid flags");
    }

    #[test]
    fn error_display_module_error() {
        let err = CodegenError::ModuleError("link failed".to_string());
        assert_eq!(err.to_string(), "module error: link failed");
    }

    #[test]
    fn error_display_unsupported_type() {
        let err = CodegenError::UnsupportedType("i256".to_string());
        assert_eq!(
            err.to_string(),
            "unsupported type for code generation: i256"
        );
    }

    #[test]
    fn error_display_internal() {
        let err = CodegenError::Internal("unexpected state".to_string());
        assert_eq!(err.to_string(), "internal codegen error: unexpected state");
    }

    #[test]
    fn error_is_debug() {
        let err = CodegenError::Internal("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Internal"));
    }

    #[test]
    fn runtime_error_main_not_found() {
        let err = RuntimeError::MainNotFound;
        assert_eq!(err.to_string(), "main function not found");
    }

    #[test]
    fn runtime_error_trap_with_code() {
        let err = RuntimeError::Trap {
            code: Some(TRAP_ASSERT_FAILED),
        };
        assert_eq!(err.to_string(), "trap occurred: code 1");
    }

    #[test]
    fn runtime_error_trap_no_code() {
        let err = RuntimeError::Trap { code: None };
        assert_eq!(err.to_string(), "trap occurred");
    }

    #[test]
    fn runtime_error_is_eq() {
        let err1 = RuntimeError::MainNotFound;
        let err2 = RuntimeError::MainNotFound;
        assert_eq!(err1, err2);
    }

    #[test]
    fn trap_codes_are_distinct() {
        assert_ne!(TRAP_UNREACHABLE, TRAP_ASSERT_FAILED);
        assert_ne!(TRAP_ASSERT_FAILED, TRAP_RESUME);
        assert_ne!(TRAP_UNREACHABLE, TRAP_RESUME);
    }
}
