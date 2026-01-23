//! Constant folding optimization pass.
//!
//! Folds constant binary operations at compile time.
//! For example: `_1 = Add(const 2, const 3)` becomes `_1 = const 5`.

use crate::mir::Body;
use crate::mir::operand::{BinOp, Constant, Operand, Rvalue};
use crate::mir::statement::StatementKind;
use crate::sema::types::TypeInterner;

use super::{OptimizationPass, PassResult};

/// ConstantFolding optimization pass.
///
/// Evaluates binary operations on constants at compile time.
pub struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &'static str {
        "ConstantFolding"
    }

    fn run(&self, body: &mut Body, _types: &TypeInterner) -> PassResult {
        let mut changed = false;

        for block in &mut body.basic_blocks {
            for stmt in &mut block.statements {
                if let StatementKind::Assign(_, rvalue) = &mut stmt.kind
                    && let Some(folded) = try_fold_rvalue(rvalue)
                {
                    *rvalue = folded;
                    changed = true;
                }
            }
        }

        PassResult { changed }
    }
}

/// Try to fold a binary operation on constants.
fn try_fold_rvalue(rvalue: &Rvalue) -> Option<Rvalue> {
    match rvalue {
        Rvalue::BinaryOp(op, lhs, rhs) => {
            let lhs_const = extract_constant(lhs)?;
            let rhs_const = extract_constant(rhs)?;
            let result = fold_binop(*op, lhs_const, rhs_const)?;
            Some(Rvalue::Use(Operand::Constant(result)))
        }
        _ => None,
    }
}

/// Extract a constant from an operand.
fn extract_constant(operand: &Operand) -> Option<&Constant> {
    match operand {
        Operand::Constant(c) => Some(c),
        _ => None,
    }
}

/// Fold a binary operation on two constants.
fn fold_binop(op: BinOp, lhs: &Constant, rhs: &Constant) -> Option<Constant> {
    match (lhs, rhs) {
        // Integer operations
        (Constant::Int(a), Constant::Int(b)) => fold_int_binop(op, *a, *b),

        // Boolean operations
        (Constant::Bool(a), Constant::Bool(b)) => fold_bool_binop(op, *a, *b),

        // Float operations
        (Constant::Float(a), Constant::Float(b)) => fold_float_binop(op, *a, *b),

        _ => None,
    }
}

/// Fold an integer binary operation.
fn fold_int_binop(op: BinOp, a: i128, b: i128) -> Option<Constant> {
    match op {
        BinOp::Add => Some(Constant::Int(a.wrapping_add(b))),
        BinOp::Sub => Some(Constant::Int(a.wrapping_sub(b))),
        BinOp::Mul => Some(Constant::Int(a.wrapping_mul(b))),
        BinOp::Div if b != 0 => Some(Constant::Int(a / b)),
        BinOp::Rem if b != 0 => Some(Constant::Int(a % b)),
        BinOp::BitAnd => Some(Constant::Int(a & b)),
        BinOp::BitOr => Some(Constant::Int(a | b)),
        BinOp::BitXor => Some(Constant::Int(a ^ b)),
        BinOp::Eq => Some(Constant::Bool(a == b)),
        BinOp::Ne => Some(Constant::Bool(a != b)),
        BinOp::Lt => Some(Constant::Bool(a < b)),
        BinOp::Le => Some(Constant::Bool(a <= b)),
        BinOp::Gt => Some(Constant::Bool(a > b)),
        BinOp::Ge => Some(Constant::Bool(a >= b)),
        BinOp::Shl => {
            let shift = b as u32;
            Some(Constant::Int(a.wrapping_shl(shift)))
        }
        BinOp::Shr => {
            let shift = b as u32;
            Some(Constant::Int(a.wrapping_shr(shift)))
        }
        _ => None,
    }
}

/// Fold a boolean binary operation.
fn fold_bool_binop(op: BinOp, a: bool, b: bool) -> Option<Constant> {
    match op {
        BinOp::BitAnd => Some(Constant::Bool(a && b)),
        BinOp::BitOr => Some(Constant::Bool(a || b)),
        BinOp::BitXor => Some(Constant::Bool(a ^ b)),
        BinOp::Eq => Some(Constant::Bool(a == b)),
        BinOp::Ne => Some(Constant::Bool(a != b)),
        _ => None,
    }
}

/// Fold a float binary operation.
fn fold_float_binop(op: BinOp, a: f64, b: f64) -> Option<Constant> {
    match op {
        BinOp::Add => Some(Constant::Float(a + b)),
        BinOp::Sub => Some(Constant::Float(a - b)),
        BinOp::Mul => Some(Constant::Float(a * b)),
        BinOp::Div => Some(Constant::Float(a / b)),
        BinOp::Rem => Some(Constant::Float(a % b)),
        BinOp::Eq => Some(Constant::Bool(a == b)),
        BinOp::Ne => Some(Constant::Bool(a != b)),
        BinOp::Lt => Some(Constant::Bool(a < b)),
        BinOp::Le => Some(Constant::Bool(a <= b)),
        BinOp::Gt => Some(Constant::Bool(a > b)),
        BinOp::Ge => Some(Constant::Bool(a >= b)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::statement::Statement;
    use crate::mir::terminator::Terminator;
    use crate::mir::types::Place;
    use crate::mir::validate::test_helpers::MirTestBuilder;

    // =========================================================================
    // Phase 5: ConstantFolding Pass Tests
    // =========================================================================

    #[test]
    fn constant_folding_int_add() {
        // _1 = Add(const 2, const 3)  =>  _1 = const 5
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(BinOp::Add, Operand::const_int(2), Operand::const_int(3)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        // Check that the statement was folded
        let stmt = &body.block(bb).statements[0];
        match &stmt.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(5)))) => {}
            other => panic!("expected folded int constant, got {:?}", other),
        }
    }

    #[test]
    fn constant_folding_int_sub() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(BinOp::Sub, Operand::const_int(10), Operand::const_int(3)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        let stmt = &body.block(bb).statements[0];
        match &stmt.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(7)))) => {}
            other => panic!("expected folded int constant 7, got {:?}", other),
        }
    }

    #[test]
    fn constant_folding_int_mul() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(BinOp::Mul, Operand::const_int(4), Operand::const_int(5)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        let stmt = &body.block(bb).statements[0];
        match &stmt.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(20)))) => {}
            other => panic!("expected folded int constant 20, got {:?}", other),
        }
    }

    #[test]
    fn constant_folding_int_div() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(BinOp::Div, Operand::const_int(10), Operand::const_int(3)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        let stmt = &body.block(bb).statements[0];
        match &stmt.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(3)))) => {}
            other => panic!("expected folded int constant 3, got {:?}", other),
        }
    }

    #[test]
    fn constant_folding_div_by_zero_not_folded() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(BinOp::Div, Operand::const_int(10), Operand::const_int(0)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        // Division by zero should NOT be folded
        assert!(!result.changed);
    }

    #[test]
    fn constant_folding_bool_and() {
        // true && false => false
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let temp = builder.add_local(bool_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(
                    BinOp::BitAnd,
                    Operand::const_bool(true),
                    Operand::const_bool(false),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        let stmt = &body.block(bb).statements[0];
        match &stmt.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Bool(false)))) => {}
            other => panic!("expected folded bool constant false, got {:?}", other),
        }
    }

    #[test]
    fn constant_folding_bool_or() {
        // false || true => true
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let temp = builder.add_local(bool_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(
                    BinOp::BitOr,
                    Operand::const_bool(false),
                    Operand::const_bool(true),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        let stmt = &body.block(bb).statements[0];
        match &stmt.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Bool(true)))) => {}
            other => panic!("expected folded bool constant true, got {:?}", other),
        }
    }

    #[test]
    fn constant_folding_comparison() {
        // 5 < 10 => true
        let mut builder = MirTestBuilder::new();
        let bool_ty = builder.types.bool();
        let temp = builder.add_local(bool_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(BinOp::Lt, Operand::const_int(5), Operand::const_int(10)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        let stmt = &body.block(bb).statements[0];
        match &stmt.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Bool(true)))) => {}
            other => panic!("expected folded bool constant true, got {:?}", other),
        }
    }

    #[test]
    fn constant_folding_skips_non_const() {
        // _2 = Add(_1, const 3) - should NOT be folded
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp1 = builder.add_local(i32_ty, true);
        let temp2 = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp2),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::copy_local(temp1),
                    Operand::const_int(3),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
    }

    #[test]
    fn constant_folding_multiple_statements() {
        // Multiple statements, some foldable
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp1 = builder.add_local(i32_ty, true);
        let temp2 = builder.add_local(i32_ty, true);
        let temp3 = builder.add_local(i32_ty, true);

        let bb = builder.add_block();

        // _1 = Add(const 1, const 2) - foldable
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp1),
                Rvalue::BinaryOp(BinOp::Add, Operand::const_int(1), Operand::const_int(2)),
                0..0,
            ),
        );

        // _2 = Add(_1, const 3) - NOT foldable
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp2),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::copy_local(temp1),
                    Operand::const_int(3),
                ),
                0..0,
            ),
        );

        // _3 = Mul(const 4, const 5) - foldable
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp3),
                Rvalue::BinaryOp(BinOp::Mul, Operand::const_int(4), Operand::const_int(5)),
                0..0,
            ),
        );

        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        // Check first statement was folded
        match &body.block(bb).statements[0].kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(3)))) => {}
            other => panic!("expected folded constant 3, got {:?}", other),
        }

        // Check second statement was NOT folded
        match &body.block(bb).statements[1].kind {
            StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::Add, _, _)) => {}
            other => panic!("expected unfold binary op, got {:?}", other),
        }

        // Check third statement was folded
        match &body.block(bb).statements[2].kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(20)))) => {}
            other => panic!("expected folded constant 20, got {:?}", other),
        }
    }

    #[test]
    fn constant_folding_empty_body() {
        let builder = MirTestBuilder::new();
        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
    }

    #[test]
    fn constant_folding_use_operand_not_folded() {
        // _1 = const 42 - nothing to fold
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::Use(Operand::const_int(42)),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(!result.changed);
    }

    #[test]
    fn constant_folding_bitwise_ops() {
        let mut builder = MirTestBuilder::new();
        let i32_ty = builder.types.i32();
        let temp = builder.add_local(i32_ty, true);

        let bb = builder.add_block();
        // 0b1100 & 0b1010 = 0b1000 = 8
        builder.add_statement(
            bb,
            Statement::assign(
                Place::from_local(temp),
                Rvalue::BinaryOp(
                    BinOp::BitAnd,
                    Operand::const_int(0b1100),
                    Operand::const_int(0b1010),
                ),
                0..0,
            ),
        );
        builder.set_terminator(bb, Terminator::return_(0..0));

        let (mut body, types) = builder.build();

        let pass = ConstantFolding;
        let result = pass.run(&mut body, &types);

        assert!(result.changed);

        let stmt = &body.block(bb).statements[0];
        match &stmt.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(Constant::Int(8)))) => {}
            other => panic!("expected folded int constant 8, got {:?}", other),
        }
    }
}
