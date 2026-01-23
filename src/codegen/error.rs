//! Code generation error types.
//!
//! This module defines error types for the Cranelift code generation backend.

use std::fmt;

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
}
