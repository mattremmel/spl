//! Internal Compiler Error (ICE) types for MIR lowering.
//!
//! This module provides structured error types for invariant violations
//! during MIR lowering. These errors indicate compiler bugs, not user errors.

use crate::diagnostic::Diagnostic;
use crate::lexer::Span;
use crate::sema::symbol::DefId;
use crate::sema::types::TypeId;
use thiserror::Error;

/// Internal Compiler Error - indicates a bug in the compiler, not user error.
///
/// ICE errors occur when MIR lowering encounters invariant violations that
/// should have been caught by earlier compiler phases (parsing, name resolution,
/// type inference). They provide structured context for debugging.
#[derive(Debug, Error)]
pub enum IceError {
    /// Field not found in struct during lowering.
    #[error("ICE: field '{field}' not found in struct '{struct_name}'")]
    FieldNotFound {
        /// The field name being accessed.
        field: String,
        /// The struct name.
        struct_name: String,
        /// The struct's `DefId`.
        struct_def_id: DefId,
        /// Source span where the error occurred.
        span: Option<Span>,
    },

    /// Struct definition not found for a given `DefId`.
    #[error("ICE: struct definition not found for DefId {def_id:?}")]
    StructNotFound {
        /// The `DefId` that wasn't found.
        def_id: DefId,
        /// The field being accessed when the error occurred.
        field_being_accessed: String,
        /// Source span where the error occurred.
        span: Option<Span>,
    },

    /// Field access attempted on a non-struct type.
    #[error("ICE: field access on non-struct type {type_description}")]
    FieldAccessOnNonStruct {
        /// Description of the actual type.
        type_description: String,
        /// The `TypeId` of the non-struct type.
        type_id: TypeId,
        /// The field name being accessed.
        field_name: String,
        /// Source span where the error occurred.
        span: Option<Span>,
    },

    /// Invalid `DefId` encountered in a specific context.
    #[error("ICE: invalid DefId in {context}")]
    InvalidDefId {
        /// Description of where the invalid `DefId` was found.
        context: &'static str,
        /// Source span where the error occurred.
        span: Option<Span>,
    },

    /// Control flow statement (break/continue) found outside of a loop.
    #[error("ICE: {keyword} outside of loop")]
    ControlFlowOutsideLoop {
        /// The control flow keyword ("break" or "continue").
        keyword: &'static str,
        /// Source span where the error occurred.
        span: Option<Span>,
    },
}

impl IceError {
    /// Convert this ICE error to a user-facing diagnostic.
    ///
    /// The diagnostic explains that this is a compiler bug and provides
    /// source location context when available.
    pub fn to_diagnostic(&self) -> Diagnostic {
        let mut diag = Diagnostic::error(format!("internal compiler error: {self}"));

        // Add source label if we have a span
        if let Some(span) = self.span() {
            diag = diag.with_label(span.clone(), "error occurred here");
        }

        // Add the "this is a bug" note
        diag = diag.with_note(
            "This is a bug in the SPL compiler. Please report it at: \
             https://github.com/yourusername/spl/issues",
        );

        diag
    }

    /// Get the source span associated with this error, if any.
    fn span(&self) -> Option<&Span> {
        match self {
            Self::FieldNotFound { span, .. }
            | Self::StructNotFound { span, .. }
            | Self::FieldAccessOnNonStruct { span, .. }
            | Self::InvalidDefId { span, .. }
            | Self::ControlFlowOutsideLoop { span, .. } => span.as_ref(),
        }
    }
}

/// Result type for MIR lowering operations.
pub type IceResult<T> = Result<T, IceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_not_found_display() {
        let err = IceError::FieldNotFound {
            field: "x".to_string(),
            struct_name: "Point".to_string(),
            struct_def_id: DefId::INVALID,
            span: None,
        };
        assert!(err.to_string().contains("field 'x'"));
        assert!(err.to_string().contains("struct 'Point'"));
    }

    #[test]
    fn struct_not_found_display() {
        let err = IceError::StructNotFound {
            def_id: DefId::INVALID,
            field_being_accessed: "x".to_string(),
            span: None,
        };
        assert!(err.to_string().contains("struct definition not found"));
    }

    #[test]
    fn field_access_on_non_struct_display() {
        let err = IceError::FieldAccessOnNonStruct {
            type_description: "i32".to_string(),
            type_id: TypeId::new(0),
            field_name: "x".to_string(),
            span: None,
        };
        assert!(err.to_string().contains("non-struct type"));
        assert!(err.to_string().contains("i32"));
    }

    #[test]
    fn to_diagnostic_includes_note() {
        let err = IceError::FieldNotFound {
            field: "x".to_string(),
            struct_name: "Point".to_string(),
            struct_def_id: DefId::INVALID,
            span: None,
        };
        let diag = err.to_diagnostic();
        assert!(diag.message.contains("internal compiler error"));
        assert!(diag.notes.iter().any(|n| n.contains("bug")));
    }

    #[test]
    fn to_diagnostic_with_span() {
        let err = IceError::FieldNotFound {
            field: "x".to_string(),
            struct_name: "Point".to_string(),
            struct_def_id: DefId::INVALID,
            span: Some(10..20),
        };
        let diag = err.to_diagnostic();
        assert!(!diag.labels.is_empty());
        assert_eq!(diag.labels[0].span, 10..20);
    }
}
