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
pub mod operand;
pub mod statement;
pub mod terminator;
pub mod types;

// Re-export main types for convenience
pub use body::{BasicBlockData, Body, LocalDecl};
pub use operand::{AggregateKind, BinOp, BorrowKind, CastKind, Constant, Operand, Rvalue, UnOp};
pub use statement::{Statement, StatementKind};
pub use terminator::{BasicBlock, SwitchTargets, Terminator, TerminatorKind};
pub use types::{FieldIdx, Local, Place, PlaceElem};
