//! CFG (Control Flow Graph) validation for MIR.
//!
//! Validates that the MIR's control flow graph is well-formed:
//! - All blocks have terminators
//! - All successor references are valid

use crate::mir::Body;

/// Validate the CFG structure of a MIR body.
///
/// # Panics
///
/// Panics if:
/// - Any block is missing a terminator
/// - Any terminator references an invalid successor block
pub fn validate_cfg(body: &Body) {
    // Check all blocks have terminators
    for (idx, block) in body.basic_blocks.iter().enumerate() {
        assert!(
            block.terminator.is_some(),
            "CFG validation failed: BasicBlock({idx}) has no terminator"
        );
    }

    // Check all successors are valid block indices
    let num_blocks = body.num_blocks();
    for (idx, block) in body.basic_blocks.iter().enumerate() {
        for successor in block.successors() {
            assert!(
                (successor.index() as usize) < num_blocks,
                "CFG validation failed: BasicBlock({}) has invalid successor BasicBlock({}), \
                but only {} blocks exist",
                idx,
                successor.index(),
                num_blocks
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::MirTestBuilder;
    use super::*;
    use crate::mir::Operand;
    use crate::mir::terminator::{BasicBlock, SwitchTargets, Terminator, TerminatorKind};

    #[test]
    fn cfg_valid_single_return() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_cfg(&body); // Should not panic
    }

    #[test]
    fn cfg_valid_chain_of_gotos() {
        let mut builder = MirTestBuilder::new();
        let bb0 = builder.add_block();
        let bb1 = builder.add_block();
        let bb2 = builder.add_block();

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));
        builder.set_terminator(bb1, Terminator::goto(bb2, 0..0));
        builder.set_terminator(bb2, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_cfg(&body);
    }

    #[test]
    fn cfg_valid_loop_back_edge() {
        let mut builder = MirTestBuilder::new();
        let bb_entry = builder.add_block();
        let bb_header = builder.add_block();
        let bb_body = builder.add_block();
        let bb_exit = builder.add_block();

        builder.set_terminator(bb_entry, Terminator::goto(bb_header, 0..0));
        builder.set_terminator(
            bb_header,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::const_bool(true),
                    targets: SwitchTargets::new_bool(bb_body, bb_exit),
                },
                0..0,
            ),
        );
        builder.set_terminator(bb_body, Terminator::goto(bb_header, 0..0)); // back edge
        builder.set_terminator(bb_exit, Terminator::return_(0..0));

        let (body, _) = builder.build();
        validate_cfg(&body);
    }

    #[test]
    fn cfg_valid_empty_body() {
        // A body with no blocks is valid (unusual but allowed)
        let builder = MirTestBuilder::new();
        let (body, _) = builder.build();
        validate_cfg(&body);
    }

    #[test]
    fn cfg_valid_self_loop() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();
        builder.set_terminator(bb, Terminator::goto(bb, 0..0)); // self-loop

        let (body, _) = builder.build();
        validate_cfg(&body);
    }

    #[test]
    fn cfg_valid_unreachable_block() {
        let mut builder = MirTestBuilder::new();
        let bb_entry = builder.add_block();
        let bb_unreachable = builder.add_block();

        builder.set_terminator(bb_entry, Terminator::return_(0..0));
        builder.set_terminator(bb_unreachable, Terminator::unreachable(0..0));

        let (body, _) = builder.build();
        validate_cfg(&body);
    }

    #[test]
    #[should_panic(expected = "no terminator")]
    fn cfg_invalid_no_terminator() {
        let mut builder = MirTestBuilder::new();
        builder.add_block(); // No terminator set

        let (body, _) = builder.build();
        validate_cfg(&body);
    }

    #[test]
    #[should_panic(expected = "invalid successor")]
    fn cfg_invalid_successor() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();

        // Point to non-existent block
        builder.set_terminator(bb, Terminator::goto(BasicBlock(999), 0..0));

        let (body, _) = builder.build();
        validate_cfg(&body);
    }

    #[test]
    #[should_panic(expected = "no terminator")]
    fn cfg_invalid_second_block_no_terminator() {
        let mut builder = MirTestBuilder::new();
        let bb0 = builder.add_block();
        let bb1 = builder.add_block(); // No terminator

        builder.set_terminator(bb0, Terminator::goto(bb1, 0..0));

        let (body, _) = builder.build();
        validate_cfg(&body);
    }

    #[test]
    #[should_panic(expected = "invalid successor")]
    fn cfg_invalid_switch_target() {
        let mut builder = MirTestBuilder::new();
        let bb = builder.add_block();

        builder.set_terminator(
            bb,
            Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::const_bool(true),
                    // Both targets are invalid
                    targets: SwitchTargets::new_bool(BasicBlock(100), BasicBlock(200)),
                },
                0..0,
            ),
        );

        let (body, _) = builder.build();
        validate_cfg(&body);
    }
}
