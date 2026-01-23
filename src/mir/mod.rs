//! Mid-level Intermediate Representation (MIR).
//!
//! MIR is a control flow graph representation used for:
//! - Borrow checking with non-lexical lifetimes (NLL)
//! - Pre-CLIF optimizations (constant folding, dead code elimination)
//! - Explicit control flow via basic blocks and terminators
//!
//! # Architecture
//!
//! ```text
//! HIR (typed, nested) → MIR (CFG, flat) → CLIF → Native
//!                            ↑
//!                     borrow checking + optimizations
//! ```
//!
//! # Key Design Decisions
//!
//! - **Flat statements**: No nested expressions; each operation is an assignment
//! - **Explicit places**: `Place { local, projection }` for tracking borrows
//! - **Move vs Copy**: `Operand::Move` vs `Operand::Copy` for ownership
//! - **Local(0) is return**: Return place is always local 0
//! - **BasicBlock(0) is entry**: Entry block is always index 0

pub mod body;
pub mod lower;
pub mod operand;
pub mod optimize;
pub mod pretty;
pub mod statement;
pub mod terminator;
pub mod types;
pub mod validate;

// Re-export main types for convenience
pub use body::{BasicBlockData, Body, LocalDecl};
pub use lower::{
    MirBuilder, MirLoweringContext, hir_binop_to_mir, hir_unop_to_mir, literal_to_operand,
    lower_hir_to_mir, lower_literal,
};
pub use operand::{AggregateKind, BinOp, BorrowKind, CastKind, Constant, Operand, Rvalue, UnOp};
pub use optimize::{OptimizationContext, OptimizationPass, PassResult, optimize_mir};
pub use pretty::{MirPrinter, pretty_print};
pub use statement::{Statement, StatementKind};
pub use terminator::{BasicBlock, SwitchTargets, Terminator, TerminatorKind};
pub use types::{FieldIdx, Local, Place, PlaceElem};
pub use validate::{ValidationContext, validate_mir};
