//! MIR statements.
//!
//! Statements are the non-terminating operations in a basic block.
//! They execute sequentially and don't transfer control flow.

use crate::lexer::Span;

use super::operand::Rvalue;
use super::types::{Local, Place};

/// A statement kind in MIR.
#[derive(Clone, Debug, PartialEq)]
pub enum StatementKind {
    /// Assignment: `place = rvalue`.
    Assign(Place, Rvalue),
    /// Marks the start of a local's live range.
    StorageLive(Local),
    /// Marks the end of a local's live range.
    StorageDead(Local),
    /// No-op statement (placeholder, will be removed).
    Nop,
}

/// A statement in MIR.
#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    /// The kind of statement.
    pub kind: StatementKind,
    /// Source span for diagnostics.
    pub span: Span,
}

impl Statement {
    /// Create a new statement.
    pub fn new(kind: StatementKind, span: Span) -> Self {
        Statement { kind, span }
    }

    /// Create an assignment statement.
    pub fn assign(place: Place, rvalue: Rvalue, span: Span) -> Self {
        Statement {
            kind: StatementKind::Assign(place, rvalue),
            span,
        }
    }

    /// Create a StorageLive statement.
    pub fn storage_live(local: Local, span: Span) -> Self {
        Statement {
            kind: StatementKind::StorageLive(local),
            span,
        }
    }

    /// Create a StorageDead statement.
    pub fn storage_dead(local: Local, span: Span) -> Self {
        Statement {
            kind: StatementKind::StorageDead(local),
            span,
        }
    }

    /// Create a nop statement.
    pub fn nop(span: Span) -> Self {
        Statement {
            kind: StatementKind::Nop,
            span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::operand::Operand;

    #[test]
    fn statement_assign() {
        let place = Place::from_local(Local(1));
        let rvalue = Rvalue::Use(Operand::const_int(42));
        let stmt = Statement::assign(place.clone(), rvalue.clone(), 0..10);

        match stmt.kind {
            StatementKind::Assign(p, rv) => {
                assert_eq!(p, place);
                assert_eq!(rv, rvalue);
            }
            _ => panic!("expected Assign"),
        }
        assert_eq!(stmt.span, 0..10);
    }

    #[test]
    fn statement_storage_live() {
        let local = Local(5);
        let stmt = Statement::storage_live(local, 0..5);

        match stmt.kind {
            StatementKind::StorageLive(l) => assert_eq!(l, local),
            _ => panic!("expected StorageLive"),
        }
    }

    #[test]
    fn statement_storage_dead() {
        let local = Local(5);
        let stmt = Statement::storage_dead(local, 0..5);

        match stmt.kind {
            StatementKind::StorageDead(l) => assert_eq!(l, local),
            _ => panic!("expected StorageDead"),
        }
    }

    #[test]
    fn statement_nop() {
        let stmt = Statement::nop(0..0);
        assert_eq!(stmt.kind, StatementKind::Nop);
    }

    #[test]
    fn statement_new() {
        let kind = StatementKind::StorageLive(Local(1));
        let stmt = Statement::new(kind.clone(), 5..10);

        assert_eq!(stmt.kind, kind);
        assert_eq!(stmt.span, 5..10);
    }

    #[test]
    fn statement_kind_equality() {
        let assign1 = StatementKind::Assign(
            Place::from_local(Local(1)),
            Rvalue::Use(Operand::const_int(1)),
        );
        let assign2 = StatementKind::Assign(
            Place::from_local(Local(1)),
            Rvalue::Use(Operand::const_int(1)),
        );
        let assign3 = StatementKind::Assign(
            Place::from_local(Local(2)),
            Rvalue::Use(Operand::const_int(1)),
        );

        assert_eq!(assign1, assign2);
        assert_ne!(assign1, assign3);
    }
}
