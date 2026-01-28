//! MIR terminators.
//!
//! Terminators end basic blocks and describe control flow transfers.
//! Every basic block must end with exactly one terminator.

use spl_lexer::Span;

use crate::operand::Operand;
use crate::types::Place;

/// A basic block identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BasicBlock(pub u32);

impl BasicBlock {
    /// The entry block is always block 0.
    pub const ENTRY: BasicBlock = BasicBlock(0);

    /// Create a new basic block ID.
    pub fn new(index: u32) -> Self {
        BasicBlock(index)
    }

    /// Get the index of this block.
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Switch targets for multi-way branches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchTargets {
    /// Mapping from discriminant values to target blocks.
    targets: Vec<(u128, BasicBlock)>,
    /// The default target if no value matches.
    otherwise: BasicBlock,
}

impl SwitchTargets {
    /// Create switch targets from a list of (value, target) pairs and a default.
    pub fn new(targets: Vec<(u128, BasicBlock)>, otherwise: BasicBlock) -> Self {
        SwitchTargets { targets, otherwise }
    }

    /// Create a boolean switch (condition ? then : else).
    ///
    /// - `true_target`: target when condition is true (non-zero)
    /// - `false_target`: target when condition is false (zero)
    pub fn new_bool(true_target: BasicBlock, false_target: BasicBlock) -> Self {
        // In boolean switch: 0 = false, non-zero = true
        // We map 0 -> false_target, otherwise -> true_target
        // But actually, we should map 1 -> true_target and otherwise (0) -> false_target
        SwitchTargets {
            targets: vec![(0, false_target)],
            otherwise: true_target,
        }
    }

    /// Get the target for a specific value, if it exists.
    pub fn target_for(&self, value: u128) -> Option<BasicBlock> {
        self.targets
            .iter()
            .find(|(v, _)| *v == value)
            .map(|(_, bb)| *bb)
    }

    /// Get the otherwise/default target.
    pub fn otherwise(&self) -> BasicBlock {
        self.otherwise
    }

    /// Iterate over all (value, target) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (u128, BasicBlock)> + '_ {
        self.targets.iter().copied()
    }

    /// Returns an iterator over all unique target blocks.
    pub fn all_targets(&self) -> impl Iterator<Item = BasicBlock> + '_ {
        let mut seen = std::collections::HashSet::new();
        self.targets
            .iter()
            .map(|(_, bb)| *bb)
            .chain(std::iter::once(self.otherwise))
            .filter(move |bb| seen.insert(*bb))
    }
}

/// The kind of terminator.
#[derive(Clone, Debug, PartialEq)]
pub enum TerminatorKind {
    /// Return from the function.
    Return,

    /// Unconditional branch to a block.
    Goto(BasicBlock),

    /// Multi-way branch based on a discriminant value.
    SwitchInt {
        /// The discriminant value to switch on.
        discr: Operand,
        /// The switch targets.
        targets: SwitchTargets,
    },

    /// Function call.
    Call {
        /// The function to call.
        func: Operand,
        /// Arguments to the function.
        args: Vec<Operand>,
        /// Where to store the return value.
        destination: Place,
        /// Block to continue at after the call (None for diverging calls).
        target: Option<BasicBlock>,
    },

    /// Drop a value.
    Drop {
        /// The place to drop.
        place: Place,
        /// Block to continue at after the drop.
        target: BasicBlock,
    },

    /// Assert that a condition is true, panic otherwise.
    Assert {
        /// The condition to check.
        cond: Operand,
        /// True if we expect the condition to be true.
        expected: bool,
        /// Block to continue at if assertion succeeds.
        target: BasicBlock,
    },

    /// Unreachable code (undefined behavior if reached).
    Unreachable,

    /// Resume unwinding (for panic propagation).
    Resume,
}

/// A terminator ends a basic block with a control flow transfer.
#[derive(Clone, Debug, PartialEq)]
pub struct Terminator {
    /// The kind of terminator.
    pub kind: TerminatorKind,
    /// Source span for diagnostics.
    pub span: Span,
}

impl Terminator {
    /// Create a new terminator.
    pub fn new(kind: TerminatorKind, span: Span) -> Self {
        Terminator { kind, span }
    }

    /// Create a return terminator.
    pub fn return_(span: Span) -> Self {
        Terminator {
            kind: TerminatorKind::Return,
            span,
        }
    }

    /// Create a goto terminator.
    pub fn goto(target: BasicBlock, span: Span) -> Self {
        Terminator {
            kind: TerminatorKind::Goto(target),
            span,
        }
    }

    /// Create an unreachable terminator.
    pub fn unreachable(span: Span) -> Self {
        Terminator {
            kind: TerminatorKind::Unreachable,
            span,
        }
    }

    /// Get all successor blocks of this terminator.
    pub fn successors(&self) -> Vec<BasicBlock> {
        match &self.kind {
            TerminatorKind::Return | TerminatorKind::Unreachable | TerminatorKind::Resume => vec![],
            TerminatorKind::Goto(bb) => vec![*bb],
            TerminatorKind::SwitchInt { targets, .. } => targets.all_targets().collect(),
            TerminatorKind::Call { target, .. } => target.iter().copied().collect(),
            TerminatorKind::Drop { target, .. } | TerminatorKind::Assert { target, .. } => {
                vec![*target]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operand::Constant;
    use crate::types::Local;
    use spl_sema::{DefId, TypeId};

    const DUMMY_TY: TypeId = TypeId::new(0);

    #[test]
    fn basic_block_id_is_copy_and_eq() {
        let bb1 = BasicBlock(1);
        let bb2 = bb1; // Copy
        let bb3 = BasicBlock(1);

        assert_eq!(bb1, bb2);
        assert_eq!(bb1, bb3);

        let bb4 = BasicBlock(2);
        assert_ne!(bb1, bb4);
    }

    #[test]
    fn basic_block_entry() {
        assert_eq!(BasicBlock::ENTRY, BasicBlock(0));
        assert_eq!(BasicBlock::ENTRY.index(), 0);
    }

    #[test]
    fn basic_block_new_and_index() {
        let bb = BasicBlock::new(42);
        assert_eq!(bb.index(), 42);
        assert_eq!(bb, BasicBlock(42));
    }

    #[test]
    fn terminator_return() {
        let term = Terminator::return_(0..5);
        assert_eq!(term.kind, TerminatorKind::Return);
        assert_eq!(term.span, 0..5);
    }

    #[test]
    fn terminator_goto() {
        let target = BasicBlock(5);
        let term = Terminator::goto(target, 0..5);

        match term.kind {
            TerminatorKind::Goto(bb) => assert_eq!(bb, target),
            _ => panic!("expected Goto"),
        }
    }

    #[test]
    fn terminator_switch_int_bool() {
        let true_block = BasicBlock(1);
        let false_block = BasicBlock(2);
        let targets = SwitchTargets::new_bool(true_block, false_block);
        let discr = Operand::copy_local(Local(1));

        let term = Terminator::new(
            TerminatorKind::SwitchInt {
                discr: discr.clone(),
                targets: targets.clone(),
            },
            0..10,
        );

        match term.kind {
            TerminatorKind::SwitchInt {
                discr: d,
                targets: t,
            } => {
                assert_eq!(d, discr);
                assert_eq!(t, targets);
            }
            _ => panic!("expected SwitchInt"),
        }
    }

    #[test]
    fn switch_targets_bool_true_target() {
        let true_block = BasicBlock(1);
        let false_block = BasicBlock(2);
        let targets = SwitchTargets::new_bool(true_block, false_block);

        // 0 should go to false_block
        assert_eq!(targets.target_for(0), Some(false_block));
        // otherwise (any non-zero) should go to true_block
        assert_eq!(targets.otherwise(), true_block);
    }

    #[test]
    fn switch_targets_custom() {
        let default = BasicBlock(0);
        let targets = SwitchTargets::new(
            vec![(1, BasicBlock(1)), (2, BasicBlock(2)), (5, BasicBlock(5))],
            default,
        );

        assert_eq!(targets.target_for(1), Some(BasicBlock(1)));
        assert_eq!(targets.target_for(2), Some(BasicBlock(2)));
        assert_eq!(targets.target_for(5), Some(BasicBlock(5)));
        assert_eq!(targets.target_for(3), None);
        assert_eq!(targets.otherwise(), default);
    }

    #[test]
    fn switch_targets_all_targets() {
        let targets =
            SwitchTargets::new(vec![(1, BasicBlock(1)), (2, BasicBlock(2))], BasicBlock(0));

        let all: Vec<_> = targets.all_targets().collect();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&BasicBlock(0)));
        assert!(all.contains(&BasicBlock(1)));
        assert!(all.contains(&BasicBlock(2)));
    }

    #[test]
    fn switch_targets_iter() {
        let targets = SwitchTargets::new(
            vec![(10, BasicBlock(1)), (20, BasicBlock(2))],
            BasicBlock(0),
        );

        let pairs: Vec<_> = targets.iter().collect();
        assert_eq!(pairs.len(), 2);
        assert!(pairs.contains(&(10, BasicBlock(1))));
        assert!(pairs.contains(&(20, BasicBlock(2))));
    }

    #[test]
    fn terminator_call() {
        let func = Operand::Constant(Constant::FnDef(DefId::new(1)));
        let args = vec![Operand::const_int(42, DUMMY_TY)];
        let destination = Place::from_local(Local(2));
        let target = Some(BasicBlock(3));

        let term = Terminator::new(
            TerminatorKind::Call {
                func: func.clone(),
                args: args.clone(),
                destination: destination.clone(),
                target,
            },
            0..20,
        );

        match term.kind {
            TerminatorKind::Call {
                func: f,
                args: a,
                destination: d,
                target: t,
            } => {
                assert_eq!(f, func);
                assert_eq!(a, args);
                assert_eq!(d, destination);
                assert_eq!(t, target);
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn terminator_call_diverging() {
        let func = Operand::Constant(Constant::FnDef(DefId::new(1)));
        let destination = Place::from_local(Local::RETURN_PLACE);

        let term = Terminator::new(
            TerminatorKind::Call {
                func,
                args: vec![],
                destination,
                target: None, // diverging
            },
            0..10,
        );

        match term.kind {
            TerminatorKind::Call { target, .. } => {
                assert!(target.is_none());
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn terminator_drop() {
        let place = Place::from_local(Local(1));
        let target = BasicBlock(2);

        let term = Terminator::new(
            TerminatorKind::Drop {
                place: place.clone(),
                target,
            },
            0..5,
        );

        match term.kind {
            TerminatorKind::Drop {
                place: p,
                target: t,
            } => {
                assert_eq!(p, place);
                assert_eq!(t, target);
            }
            _ => panic!("expected Drop"),
        }
    }

    #[test]
    fn terminator_assert() {
        let cond = Operand::const_bool(true);
        let target = BasicBlock(1);

        let term = Terminator::new(
            TerminatorKind::Assert {
                cond: cond.clone(),
                expected: true,
                target,
            },
            0..10,
        );

        match term.kind {
            TerminatorKind::Assert {
                cond: c,
                expected,
                target: t,
            } => {
                assert_eq!(c, cond);
                assert!(expected);
                assert_eq!(t, target);
            }
            _ => panic!("expected Assert"),
        }
    }

    #[test]
    fn terminator_unreachable() {
        let term = Terminator::unreachable(0..0);
        assert_eq!(term.kind, TerminatorKind::Unreachable);
    }

    #[test]
    fn terminator_resume() {
        let term = Terminator::new(TerminatorKind::Resume, 0..0);
        assert_eq!(term.kind, TerminatorKind::Resume);
    }

    #[test]
    fn terminator_successors_return() {
        let term = Terminator::return_(0..0);
        assert!(term.successors().is_empty());
    }

    #[test]
    fn terminator_successors_goto() {
        let term = Terminator::goto(BasicBlock(5), 0..0);
        assert_eq!(term.successors(), vec![BasicBlock(5)]);
    }

    #[test]
    fn terminator_successors_switch_int() {
        let targets =
            SwitchTargets::new(vec![(1, BasicBlock(1)), (2, BasicBlock(2))], BasicBlock(0));
        let term = Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::const_int(0, DUMMY_TY),
                targets,
            },
            0..0,
        );

        let successors = term.successors();
        assert_eq!(successors.len(), 3);
        assert!(successors.contains(&BasicBlock(0)));
        assert!(successors.contains(&BasicBlock(1)));
        assert!(successors.contains(&BasicBlock(2)));
    }

    #[test]
    fn terminator_successors_call() {
        let term = Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId::new(1))),
                args: vec![],
                destination: Place::from_local(Local(0)),
                target: Some(BasicBlock(3)),
            },
            0..0,
        );

        assert_eq!(term.successors(), vec![BasicBlock(3)]);
    }

    #[test]
    fn terminator_successors_call_diverging() {
        let term = Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId::new(1))),
                args: vec![],
                destination: Place::from_local(Local(0)),
                target: None,
            },
            0..0,
        );

        assert!(term.successors().is_empty());
    }

    #[test]
    fn terminator_successors_drop() {
        let term = Terminator::new(
            TerminatorKind::Drop {
                place: Place::from_local(Local(1)),
                target: BasicBlock(2),
            },
            0..0,
        );

        assert_eq!(term.successors(), vec![BasicBlock(2)]);
    }

    #[test]
    fn terminator_successors_assert() {
        let term = Terminator::new(
            TerminatorKind::Assert {
                cond: Operand::const_bool(true),
                expected: true,
                target: BasicBlock(1),
            },
            0..0,
        );

        assert_eq!(term.successors(), vec![BasicBlock(1)]);
    }

    #[test]
    fn terminator_successors_unreachable() {
        let term = Terminator::unreachable(0..0);
        assert!(term.successors().is_empty());
    }

    #[test]
    fn terminator_successors_resume() {
        let term = Terminator::new(TerminatorKind::Resume, 0..0);
        assert!(term.successors().is_empty());
    }

    // Additional coverage tests

    #[test]
    fn switch_targets_bool_nonzero_values() {
        let true_block = BasicBlock(1);
        let false_block = BasicBlock(2);
        let targets = SwitchTargets::new_bool(true_block, false_block);

        // 0 -> false_block
        assert_eq!(targets.target_for(0), Some(false_block));
        // 1 should go to otherwise (true_block)
        assert_eq!(targets.target_for(1), None);
        // Any non-zero goes to otherwise
        assert_eq!(targets.target_for(42), None);
        assert_eq!(targets.target_for(u128::MAX), None);
        // otherwise is true_block
        assert_eq!(targets.otherwise(), true_block);
    }

    #[test]
    fn switch_targets_empty() {
        // All values go to otherwise
        let otherwise = BasicBlock(5);
        let targets = SwitchTargets::new(vec![], otherwise);

        assert_eq!(targets.target_for(0), None);
        assert_eq!(targets.target_for(1), None);
        assert_eq!(targets.otherwise(), otherwise);

        let all: Vec<_> = targets.all_targets().collect();
        assert_eq!(all, vec![otherwise]);
    }

    #[test]
    fn switch_targets_duplicate_otherwise() {
        // Same block in both targets and otherwise
        let shared = BasicBlock(1);
        let targets = SwitchTargets::new(vec![(0, shared)], shared);

        // all_targets should deduplicate
        let all: Vec<_> = targets.all_targets().collect();
        assert_eq!(all.len(), 1); // Only one unique block
        assert_eq!(all[0], shared);
    }

    #[test]
    fn switch_targets_deduplicates_multiple() {
        let block_a = BasicBlock(1);
        let block_b = BasicBlock(2);
        // block_a appears twice in targets, plus as otherwise
        let targets = SwitchTargets::new(vec![(0, block_a), (1, block_b), (2, block_a)], block_a);

        let all: Vec<_> = targets.all_targets().collect();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&block_a));
        assert!(all.contains(&block_b));
    }

    #[test]
    fn terminator_assert_expected_false() {
        let cond = Operand::const_bool(false);
        let target = BasicBlock(1);

        let term = Terminator::new(
            TerminatorKind::Assert {
                cond: cond.clone(),
                expected: false,
                target,
            },
            0..10,
        );

        match term.kind {
            TerminatorKind::Assert {
                expected,
                target: t,
                ..
            } => {
                assert!(!expected);
                assert_eq!(t, target);
            }
            _ => panic!("expected Assert"),
        }
    }

    #[test]
    fn basic_block_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(BasicBlock(1));
        set.insert(BasicBlock(2));
        set.insert(BasicBlock(1)); // duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&BasicBlock(1)));
        assert!(set.contains(&BasicBlock(2)));
    }

    #[test]
    fn switch_targets_single_value() {
        let targets = SwitchTargets::new(vec![(42, BasicBlock(1))], BasicBlock(0));

        assert_eq!(targets.target_for(42), Some(BasicBlock(1)));
        assert_eq!(targets.target_for(0), None);
        assert_eq!(targets.otherwise(), BasicBlock(0));
    }
}
