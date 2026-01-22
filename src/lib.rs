//! SPL (Simple Programming Language) compiler.
//!
//! This crate provides a complete compiler pipeline from source code to MIR.
//!
//! # Error Handling Architecture
//!
//! The SPL compiler uses phase-specific error handling strategies, each tailored
//! to the needs of that compilation phase. This is intentional - different phases
//! have different goals and constraints.
//!
//! ## Phase Summary
//!
//! | Phase | Error Type | Strategy | Rationale |
//! |-------|------------|----------|-----------|
//! | Parser | [`ParseError`] | Recovery sets, event collection | IDE support, partial results |
//! | Sema | [`Diagnostic`] | Imperative collection, builder | Rich user-facing messages |
//! | HIR Lowering | `Missing` nodes | Fallback values | Continue despite earlier errors |
//! | MIR Lowering | `panic!()` | Invariant assertions | Input guaranteed valid |
//!
//! ## Error Flow
//!
//! ```text
//! Source Code
//!     │
//!     ▼
//! ┌─────────────────┐
//! │     Parser      │──▶ ParseError (recoverable, collected)
//! └────────┬────────┘
//!          │ CST (may contain ERROR nodes)
//!          ▼
//! ┌─────────────────┐
//! │   Resolution    │──▶ Diagnostic (user-facing, with spans/labels)
//! │   + Type Infer  │
//! └────────┬────────┘
//!          │ InferResult (types + diagnostics)
//!          ▼
//! ┌─────────────────┐
//! │  HIR Lowering   │──▶ Missing nodes (graceful degradation)
//! └────────┬────────┘
//!          │ HirDatabase (typed HIR)
//!          ▼
//! ┌─────────────────┐
//! │  MIR Lowering   │──▶ panic!() (invariant violations = compiler bugs)
//! └────────┬────────┘
//!          │ MIR Bodies
//!          ▼
//! ```
//!
//! ## Design Rationale
//!
//! - **Parser recovery**: The parser continues after errors to support IDE features
//!   and provide multiple error messages in a single pass. Uses [`ParseError`] rather
//!   than [`Diagnostic`] to keep the parser self-contained and reusable.
//!
//! - **Semantic diagnostics**: Name resolution and type inference produce [`Diagnostic`]
//!   with rich context (spans, labels, suggestions) for user-facing error messages.
//!   Errors are collected imperatively as analysis proceeds.
//!
//! - **HIR fallbacks**: When lowering encounters missing or malformed AST nodes
//!   (from earlier errors), it produces `HirExprKind::Missing` or error types rather
//!   than failing. This allows later phases to run for valid portions of code.
//!
//! - **MIR panics**: MIR lowering assumes valid, well-typed HIR. Any invariant
//!   violation at this stage indicates a compiler bug, not user error, so we panic
//!   rather than produce invalid MIR that would cause worse problems downstream.

pub mod ast;
pub mod diagnostic;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod sema;
pub mod syntax;

pub use diagnostic::{Diagnostic, DiagnosticRenderer, Label, RenderConfig, Severity};
pub use lexer::{Lexer, Span, SpannedToken, Token};
pub use parser::{Parse, ParseError, parse};
pub use sema::{DefId, SemanticContext, Symbol, SymbolKind};
pub use syntax::{Lang, SyntaxKind, SyntaxNode, SyntaxToken};
