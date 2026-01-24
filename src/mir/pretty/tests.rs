use super::*;
use crate::mir::body::{BasicBlockData, Body, LocalDecl};
use crate::mir::operand::{
    AggregateKind, BinOp, BorrowKind, CastKind, Constant, Operand, Rvalue, UnOp,
};
use crate::mir::statement::Statement;
use crate::mir::terminator::{BasicBlock, SwitchTargets, Terminator, TerminatorKind};
use crate::mir::types::{FieldIdx, Local, Place, PlaceElem};
use crate::sema::symbol::DefId;
use crate::sema::types::{Mutability, TypeId};
use expect_test::{Expect, expect};

fn check(actual: &str, expected: &Expect) {
    expected.assert_eq(actual);
}

// === Phase 1: Primitives ===

mod primitives {
    use super::*;

    #[test]
    fn local_zero() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_local(Local(0)), "_0");
    }

    #[test]
    fn local_numbered() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_local(Local(5)), "_5");
    }

    #[test]
    fn basic_block_zero() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_basic_block(BasicBlock(0)), "bb0");
    }

    #[test]
    fn basic_block_numbered() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_basic_block(BasicBlock(3)), "bb3");
    }

    #[test]
    fn field_idx() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_field_idx(FieldIdx(2)), "2");
    }
}

// === Phase 2: Constants ===

mod constants {
    use super::*;

    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn const_int() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_constant(&Constant::Int(42, DUMMY_TY)), "const 42_ty0");
    }

    #[test]
    fn const_int_negative() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_constant(&Constant::Int(-1, DUMMY_TY)), "const -1_ty0");
    }

    #[test]
    fn const_float() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_constant(&Constant::Float(2.5, DUMMY_TY)), "const 2.5_ty0");
    }

    #[test]
    fn const_bool_true() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_constant(&Constant::Bool(true)), "const true");
    }

    #[test]
    fn const_bool_false() {
        let printer = MirPrinter::new();
        assert_eq!(
            printer.print_constant(&Constant::Bool(false)),
            "const false"
        );
    }

    #[test]
    fn const_char() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_constant(&Constant::Char('x')), "const 'x'");
    }

    #[test]
    fn const_string() {
        let printer = MirPrinter::new();
        assert_eq!(
            printer.print_constant(&Constant::String("hi".to_string())),
            r#"const "hi""#
        );
    }

    #[test]
    fn const_unit() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_constant(&Constant::Unit), "const ()");
    }

    #[test]
    fn const_fn_def() {
        let printer = MirPrinter::new();
        assert_eq!(
            printer.print_constant(&Constant::FnDef(DefId(5))),
            "const fn_5"
        );
    }

    #[test]
    fn const_zeroed() {
        let printer = MirPrinter::new();
        assert_eq!(
            printer.print_constant(&Constant::Zeroed(TypeId(3))),
            "const zeroed(ty3)"
        );
    }
}

// === Phase 3: Operands ===

mod operands {
    use super::*;

    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn operand_copy() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(1));
        assert_eq!(printer.print_operand(&Operand::Copy(place)), "copy _1");
    }

    #[test]
    fn operand_move() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(2));
        assert_eq!(printer.print_operand(&Operand::Move(place)), "move _2");
    }

    #[test]
    fn operand_constant() {
        let printer = MirPrinter::new();
        assert_eq!(
            printer.print_operand(&Operand::Constant(Constant::Int(5, DUMMY_TY))),
            "const 5_ty0"
        );
    }
}

// === Phase 4: Places & Projections ===

mod places {
    use super::*;

    #[test]
    fn place_local() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(1));
        assert_eq!(printer.print_place(&place), "_1");
    }

    #[test]
    fn place_deref() {
        let printer = MirPrinter::new();
        let place = Place::deref(Local(1));
        assert_eq!(printer.print_place(&place), "(*_1)");
    }

    #[test]
    fn place_field() {
        let printer = MirPrinter::new();
        let place = Place::field(Local(1), FieldIdx(0));
        assert_eq!(printer.print_place(&place), "_1.0");
    }

    #[test]
    fn place_index() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(1)).project(PlaceElem::Index(Local(2)));
        assert_eq!(printer.print_place(&place), "_1[_2]");
    }

    #[test]
    fn place_deref_field() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(1))
            .project(PlaceElem::Deref)
            .project(PlaceElem::Field(FieldIdx(1)));
        assert_eq!(printer.print_place(&place), "(*_1).1");
    }

    #[test]
    fn place_const_index() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(1)).project(PlaceElem::ConstantIndex {
            offset: 0,
            from_end: false,
        });
        assert_eq!(printer.print_place(&place), "_1[0]");
    }

    #[test]
    fn place_const_index_end() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(1)).project(PlaceElem::ConstantIndex {
            offset: 1,
            from_end: true,
        });
        assert_eq!(printer.print_place(&place), "_1[-1]");
    }

    #[test]
    fn place_subslice() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(1)).project(PlaceElem::Subslice { from: 1, to: 3 });
        assert_eq!(printer.print_place(&place), "_1[1..3]");
    }

    #[test]
    fn place_downcast() {
        let printer = MirPrinter::new();
        let place = Place::from_local(Local(1)).project(PlaceElem::Downcast(2));
        assert_eq!(printer.print_place(&place), "_1 as variant#2");
    }
}

// === Phase 5: Operators ===

mod operators {
    use super::*;

    #[test]
    fn binop_add() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_binop(BinOp::Add), "Add");
    }

    #[test]
    fn binop_sub() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_binop(BinOp::Sub), "Sub");
    }

    #[test]
    fn binop_eq() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_binop(BinOp::Eq), "Eq");
    }

    #[test]
    fn unop_neg() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_unop(UnOp::Neg), "Neg");
    }

    #[test]
    fn unop_not() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_unop(UnOp::Not), "Not");
    }

    #[test]
    fn all_binops() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_binop(BinOp::Mul), "Mul");
        assert_eq!(printer.print_binop(BinOp::Div), "Div");
        assert_eq!(printer.print_binop(BinOp::Rem), "Rem");
        assert_eq!(printer.print_binop(BinOp::BitAnd), "BitAnd");
        assert_eq!(printer.print_binop(BinOp::BitOr), "BitOr");
        assert_eq!(printer.print_binop(BinOp::BitXor), "BitXor");
        assert_eq!(printer.print_binop(BinOp::Shl), "Shl");
        assert_eq!(printer.print_binop(BinOp::Shr), "Shr");
        assert_eq!(printer.print_binop(BinOp::Ne), "Ne");
        assert_eq!(printer.print_binop(BinOp::Lt), "Lt");
        assert_eq!(printer.print_binop(BinOp::Le), "Le");
        assert_eq!(printer.print_binop(BinOp::Gt), "Gt");
        assert_eq!(printer.print_binop(BinOp::Ge), "Ge");
    }
}

// === Phase 6: Rvalues ===

mod rvalues {
    use super::*;

    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn rvalue_use() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Use(Operand::Copy(Place::from_local(Local(1))));
        assert_eq!(printer.print_rvalue(&rvalue), "copy _1");
    }

    #[test]
    fn rvalue_ref_shared() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Ref(BorrowKind::Shared, Place::from_local(Local(1)), DUMMY_TY);
        assert_eq!(printer.print_rvalue(&rvalue), "&_1");
    }

    #[test]
    fn rvalue_ref_mut() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Ref(BorrowKind::Mut, Place::from_local(Local(1)), DUMMY_TY);
        assert_eq!(printer.print_rvalue(&rvalue), "&mut _1");
    }

    #[test]
    fn rvalue_address_of() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::AddressOf(Mutability::Shared, Place::from_local(Local(1)), DUMMY_TY);
        assert_eq!(printer.print_rvalue(&rvalue), "&raw const _1");
    }

    #[test]
    fn rvalue_address_of_mut() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::AddressOf(Mutability::Mutable, Place::from_local(Local(1)), DUMMY_TY);
        assert_eq!(printer.print_rvalue(&rvalue), "&raw mut _1");
    }

    #[test]
    fn rvalue_binary() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        );
        assert_eq!(printer.print_rvalue(&rvalue), "Add(copy _1, copy _2)");
    }

    #[test]
    fn rvalue_unary() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(Place::from_local(Local(1))));
        assert_eq!(printer.print_rvalue(&rvalue), "Neg(copy _1)");
    }

    #[test]
    fn rvalue_cast() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Cast(
            CastKind::IntToFloat,
            Operand::Copy(Place::from_local(Local(1))),
            TypeId(5),
        );
        assert_eq!(printer.print_rvalue(&rvalue), "copy _1 as ty5 (IntToFloat)");
    }

    #[test]
    fn rvalue_len() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Len(Place::from_local(Local(1)));
        assert_eq!(printer.print_rvalue(&rvalue), "Len(_1)");
    }

    #[test]
    fn rvalue_discriminant() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Discriminant(Place::from_local(Local(1)));
        assert_eq!(printer.print_rvalue(&rvalue), "discriminant(_1)");
    }

    #[test]
    fn rvalue_repeat() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Repeat(Operand::Constant(Constant::Int(0, DUMMY_TY)), 5);
        assert_eq!(printer.print_rvalue(&rvalue), "[const 0_ty0; 5]");
    }

    #[test]
    fn rvalue_aggregate_tuple() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Aggregate(
            AggregateKind::Tuple,
            vec![
                Operand::Copy(Place::from_local(Local(1))),
                Operand::Copy(Place::from_local(Local(2))),
            ],
        );
        assert_eq!(printer.print_rvalue(&rvalue), "(copy _1, copy _2)");
    }

    #[test]
    fn rvalue_aggregate_array() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Aggregate(
            AggregateKind::Array,
            vec![
                Operand::Copy(Place::from_local(Local(1))),
                Operand::Copy(Place::from_local(Local(2))),
            ],
        );
        assert_eq!(printer.print_rvalue(&rvalue), "[copy _1, copy _2]");
    }

    #[test]
    fn rvalue_aggregate_adt() {
        let printer = MirPrinter::new();
        let rvalue = Rvalue::Aggregate(
            AggregateKind::Adt(DefId(10)),
            vec![
                Operand::Copy(Place::from_local(Local(1))),
                Operand::Copy(Place::from_local(Local(2))),
            ],
        );
        assert_eq!(printer.print_rvalue(&rvalue), "adt_10 { copy _1, copy _2 }");
    }
}

// === Phase 7: Statements ===

mod statements {
    use super::*;

    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn stmt_assign() {
        let printer = MirPrinter::new();
        let stmt = Statement::assign(
            Place::from_local(Local(1)),
            Rvalue::Use(Operand::Constant(Constant::Int(42, DUMMY_TY))),
            0..0,
        );
        assert_eq!(printer.print_statement(&stmt), "_1 = const 42_ty0");
    }

    #[test]
    fn stmt_storage_live() {
        let printer = MirPrinter::new();
        let stmt = Statement::storage_live(Local(1), 0..0);
        assert_eq!(printer.print_statement(&stmt), "StorageLive(_1)");
    }

    #[test]
    fn stmt_storage_dead() {
        let printer = MirPrinter::new();
        let stmt = Statement::storage_dead(Local(1), 0..0);
        assert_eq!(printer.print_statement(&stmt), "StorageDead(_1)");
    }

    #[test]
    fn stmt_nop() {
        let printer = MirPrinter::new();
        let stmt = Statement::nop(0..0);
        assert_eq!(printer.print_statement(&stmt), "nop");
    }
}

// === Phase 8: Terminators ===

mod terminators {
    use super::*;

    #[test]
    fn term_return() {
        let printer = MirPrinter::new();
        let term = Terminator::return_(0..0);
        assert_eq!(printer.print_terminator(&term), "return");
    }

    #[test]
    fn term_goto() {
        let printer = MirPrinter::new();
        let term = Terminator::goto(BasicBlock(1), 0..0);
        assert_eq!(printer.print_terminator(&term), "goto -> bb1");
    }

    #[test]
    fn term_unreachable() {
        let printer = MirPrinter::new();
        let term = Terminator::unreachable(0..0);
        assert_eq!(printer.print_terminator(&term), "unreachable");
    }

    #[test]
    fn term_resume() {
        let printer = MirPrinter::new();
        let term = Terminator::new(TerminatorKind::Resume, 0..0);
        assert_eq!(printer.print_terminator(&term), "resume");
    }

    #[test]
    fn term_drop() {
        let printer = MirPrinter::new();
        let term = Terminator::new(
            TerminatorKind::Drop {
                place: Place::from_local(Local(1)),
                target: BasicBlock(1),
            },
            0..0,
        );
        assert_eq!(printer.print_terminator(&term), "drop(_1) -> bb1");
    }

    #[test]
    fn term_assert() {
        let printer = MirPrinter::new();
        let term = Terminator::new(
            TerminatorKind::Assert {
                cond: Operand::Copy(Place::from_local(Local(1))),
                expected: true,
                target: BasicBlock(2),
            },
            0..0,
        );
        assert_eq!(printer.print_terminator(&term), "assert(copy _1) -> bb2");
    }

    #[test]
    fn term_assert_not() {
        let printer = MirPrinter::new();
        let term = Terminator::new(
            TerminatorKind::Assert {
                cond: Operand::Copy(Place::from_local(Local(1))),
                expected: false,
                target: BasicBlock(2),
            },
            0..0,
        );
        assert_eq!(printer.print_terminator(&term), "assert(!copy _1) -> bb2");
    }

    #[test]
    fn term_switch() {
        let printer = MirPrinter::new();
        let targets =
            SwitchTargets::new(vec![(0, BasicBlock(1)), (1, BasicBlock(2))], BasicBlock(3));
        let term = Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::Copy(Place::from_local(Local(1))),
                targets,
            },
            0..0,
        );
        assert_eq!(
            printer.print_terminator(&term),
            "switchInt(copy _1) -> [0: bb1, 1: bb2, otherwise: bb3]"
        );
    }

    #[test]
    fn term_call() {
        let printer = MirPrinter::new();
        let term = Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId(5))),
                args: vec![
                    Operand::Copy(Place::from_local(Local(1))),
                    Operand::Copy(Place::from_local(Local(2))),
                ],
                destination: Place::from_local(Local(0)),
                target: Some(BasicBlock(1)),
            },
            0..0,
        );
        assert_eq!(
            printer.print_terminator(&term),
            "_0 = call const fn_5(copy _1, copy _2) -> bb1"
        );
    }

    #[test]
    fn term_call_no_return() {
        let printer = MirPrinter::new();
        let term = Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId(5))),
                args: vec![Operand::Copy(Place::from_local(Local(1)))],
                destination: Place::from_local(Local(0)),
                target: None,
            },
            0..0,
        );
        assert_eq!(
            printer.print_terminator(&term),
            "_0 = call const fn_5(copy _1) -> !"
        );
    }
}

// === Phase 9: Basic Blocks ===

mod basic_blocks {
    use super::*;

    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn block_empty() {
        let mut printer = MirPrinter::new();
        let mut block = BasicBlockData::new();
        block.set_terminator(Terminator::return_(0..0));

        printer.print_block(0, &block);

        check(
            &printer.finish(),
            &expect![[r#"
                bb0:
                    return
            "#]],
        );
    }

    #[test]
    fn block_with_statements() {
        let mut printer = MirPrinter::new();
        let mut block = BasicBlockData::new();
        block.push_statement(Statement::assign(
            Place::from_local(Local(1)),
            Rvalue::Use(Operand::Constant(Constant::Int(1, DUMMY_TY))),
            0..0,
        ));
        block.set_terminator(Terminator::return_(0..0));

        printer.print_block(0, &block);

        check(
            &printer.finish(),
            &expect![[r#"
                bb0:
                    _1 = const 1_ty0
                    return
            "#]],
        );
    }
}

// === Phase 10: Function Bodies ===

mod function_bodies {
    use super::*;

    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn body_simple() {
        let mut body = Body::new(TypeId(1)); // return type
        let bb = body.alloc_block();
        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(Local(0)),
            Rvalue::Use(Operand::Constant(Constant::Int(42, DUMMY_TY))),
            0..0,
        ));
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("simple"));
        check(
            &output,
            &expect![[r#"
                fn simple() -> ty1 {
                    bb0:
                        _0 = const 42_ty0
                        return
                }
            "#]],
        );
    }

    #[test]
    fn body_with_args() {
        let mut body = Body::with_args(TypeId(1), &[(TypeId(2), false), (TypeId(3), true)]);
        let bb = body.alloc_block();
        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(Local(0)),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::Copy(Place::from_local(Local(1))),
                Operand::Copy(Place::from_local(Local(2))),
            ),
            0..0,
        ));
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("add"));
        check(
            &output,
            &expect![[r#"
                fn add(_1: ty2, _2: ty3) -> ty1 {
                    bb0:
                        _0 = Add(copy _1, copy _2)
                        return
                }
            "#]],
        );
    }

    #[test]
    fn body_multi_block() {
        let mut body = Body::with_args(TypeId(1), &[(TypeId(2), false)]);
        let temp = body.alloc_local(LocalDecl::new(TypeId(1), true));

        let bb0 = body.alloc_block();
        let bb1 = body.alloc_block();

        // bb0: goto bb1
        body.block_mut(bb0).push_statement(Statement::assign(
            Place::from_local(temp),
            Rvalue::Use(Operand::Constant(Constant::Int(1, DUMMY_TY))),
            0..0,
        ));
        body.block_mut(bb0)
            .set_terminator(Terminator::goto(bb1, 0..0));

        // bb1: return
        body.block_mut(bb1).push_statement(Statement::assign(
            Place::from_local(Local(0)),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::Copy(Place::from_local(Local(1))),
                Operand::Copy(Place::from_local(temp)),
            ),
            0..0,
        ));
        body.block_mut(bb1)
            .set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("multi_block"));
        check(
            &output,
            &expect![[r#"
                fn multi_block(_1: ty2) -> ty1 {
                    let mut _2: ty1;

                    bb0:
                        _2 = const 1_ty0
                        goto -> bb1
                    bb1:
                        _0 = Add(copy _1, copy _2)
                        return
                }
            "#]],
        );
    }

    #[test]
    fn body_with_named_local() {
        let mut body = Body::new(TypeId(1));
        let _x = body.alloc_local(LocalDecl::with_name(TypeId(2), true, "x"));

        let bb = body.alloc_block();
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("named"));
        check(
            &output,
            &expect![[r#"
                fn named() -> ty1 {
                    let mut _1: ty2; // x

                    bb0:
                        return
                }
            "#]],
        );
    }

    #[test]
    fn body_with_control_flow() {
        // if cond { 1 } else { 2 }
        let mut body = Body::with_args(TypeId(1), &[(TypeId(2), false)]); // bool arg

        let entry = body.alloc_block();
        let then_bb = body.alloc_block();
        let else_bb = body.alloc_block();
        let join_bb = body.alloc_block();

        // entry: switch on arg
        let targets = SwitchTargets::new_bool(then_bb, else_bb);
        body.block_mut(entry).set_terminator(Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::Copy(Place::from_local(Local(1))),
                targets,
            },
            0..0,
        ));

        // then: _0 = 1; goto join
        body.block_mut(then_bb).push_statement(Statement::assign(
            Place::from_local(Local(0)),
            Rvalue::Use(Operand::Constant(Constant::Int(1, DUMMY_TY))),
            0..0,
        ));
        body.block_mut(then_bb)
            .set_terminator(Terminator::goto(join_bb, 0..0));

        // else: _0 = 2; goto join
        body.block_mut(else_bb).push_statement(Statement::assign(
            Place::from_local(Local(0)),
            Rvalue::Use(Operand::Constant(Constant::Int(2, DUMMY_TY))),
            0..0,
        ));
        body.block_mut(else_bb)
            .set_terminator(Terminator::goto(join_bb, 0..0));

        // join: return
        body.block_mut(join_bb)
            .set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("if_else"));
        check(
            &output,
            &expect![[r#"
                fn if_else(_1: ty2) -> ty1 {
                    bb0:
                        switchInt(copy _1) -> [0: bb2, otherwise: bb1]
                    bb1:
                        _0 = const 1_ty0
                        goto -> bb3
                    bb2:
                        _0 = const 2_ty0
                        goto -> bb3
                    bb3:
                        return
                }
            "#]],
        );
    }
}

// === Phase 11: Integration Tests ===

mod integration {
    use super::*;

    const DUMMY_TY: TypeId = TypeId(0);

    #[test]
    fn integration_simple_return() {
        // fn simple() -> i32 { 42 }
        let mut body = Body::new(TypeId(1));
        let bb = body.alloc_block();

        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(Local(0)),
            Rvalue::Use(Operand::Constant(Constant::Int(42, DUMMY_TY))),
            0..0,
        ));
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("simple"));
        check(
            &output,
            &expect![[r#"
                fn simple() -> ty1 {
                    bb0:
                        _0 = const 42_ty0
                        return
                }
            "#]],
        );
    }

    #[test]
    fn integration_arithmetic() {
        // fn add(a: i32, b: i32) -> i32 { a + b }
        let mut body = Body::with_args(TypeId(1), &[(TypeId(1), false), (TypeId(1), false)]);
        let bb = body.alloc_block();

        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(Local(0)),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::Copy(Place::from_local(Local(1))),
                Operand::Copy(Place::from_local(Local(2))),
            ),
            0..0,
        ));
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("add"));
        check(
            &output,
            &expect![[r#"
                fn add(_1: ty1, _2: ty1) -> ty1 {
                    bb0:
                        _0 = Add(copy _1, copy _2)
                        return
                }
            "#]],
        );
    }

    #[test]
    fn integration_function_call() {
        // fn caller() -> i32 { callee(1, 2) }
        let mut body = Body::new(TypeId(1));
        let bb = body.alloc_block();
        let ret_bb = body.alloc_block();

        body.block_mut(bb).set_terminator(Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId(10))),
                args: vec![
                    Operand::Constant(Constant::Int(1, DUMMY_TY)),
                    Operand::Constant(Constant::Int(2, DUMMY_TY)),
                ],
                destination: Place::from_local(Local(0)),
                target: Some(ret_bb),
            },
            0..0,
        ));

        body.block_mut(ret_bb)
            .set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("caller"));
        check(
            &output,
            &expect![[r#"
                fn caller() -> ty1 {
                    bb0:
                        _0 = call const fn_10(const 1_ty0, const 2_ty0) -> bb1
                    bb1:
                        return
                }
            "#]],
        );
    }

    #[test]
    fn integration_storage_markers() {
        let mut body = Body::new(TypeId(0));
        let temp = body.alloc_local(LocalDecl::new(TypeId(1), true));
        let bb = body.alloc_block();

        body.block_mut(bb)
            .push_statement(Statement::storage_live(temp, 0..0));
        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(temp),
            Rvalue::Use(Operand::Constant(Constant::Int(42, DUMMY_TY))),
            0..0,
        ));
        body.block_mut(bb)
            .push_statement(Statement::storage_dead(temp, 0..0));
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("storage"));
        check(
            &output,
            &expect![[r#"
                fn storage() -> ty0 {
                    let mut _1: ty1;

                    bb0:
                        StorageLive(_1)
                        _1 = const 42_ty0
                        StorageDead(_1)
                        return
                }
            "#]],
        );
    }

    #[test]
    fn integration_ref_and_deref() {
        let mut body = Body::with_args(TypeId(1), &[(TypeId(2), false)]);
        let ref_local = body.alloc_local(LocalDecl::new(TypeId(3), false)); // &i32

        let bb = body.alloc_block();

        // _2 = &_1
        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(ref_local),
            Rvalue::Ref(BorrowKind::Shared, Place::from_local(Local(1)), DUMMY_TY),
            0..0,
        ));

        // _0 = *_2
        body.block_mut(bb).push_statement(Statement::assign(
            Place::from_local(Local(0)),
            Rvalue::Use(Operand::Copy(Place::deref(ref_local))),
            0..0,
        ));

        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, Some("ref_deref"));
        check(
            &output,
            &expect![[r#"
                fn ref_deref(_1: ty2) -> ty1 {
                    let _2: ty3;

                    bb0:
                        _2 = &_1
                        _0 = copy (*_2)
                        return
                }
            "#]],
        );
    }
}

// === Additional Coverage ===

mod coverage {
    use super::*;

    #[test]
    fn default_trait() {
        let printer = MirPrinter::default();
        assert_eq!(printer.print_local(Local(0)), "_0");
    }

    #[test]
    fn cast_kinds() {
        let printer = MirPrinter::new();
        assert_eq!(printer.print_cast_kind(CastKind::IntToInt), "IntToInt");
        assert_eq!(printer.print_cast_kind(CastKind::FloatToInt), "FloatToInt");
        assert_eq!(
            printer.print_cast_kind(CastKind::FloatToFloat),
            "FloatToFloat"
        );
        assert_eq!(printer.print_cast_kind(CastKind::PtrToPtr), "PtrToPtr");
        assert_eq!(printer.print_cast_kind(CastKind::Unsize), "Unsize");
    }

    #[test]
    fn complex_place_projection() {
        let printer = MirPrinter::new();
        // (*(*_1.0).2)[_3]
        let place = Place::from_local(Local(1))
            .project(PlaceElem::Field(FieldIdx(0)))
            .project(PlaceElem::Deref)
            .project(PlaceElem::Field(FieldIdx(2)))
            .project(PlaceElem::Deref)
            .project(PlaceElem::Index(Local(3)));
        assert_eq!(printer.print_place(&place), "(*(*_1.0).2)[_3]");
    }

    #[test]
    fn empty_body_no_blocks() {
        let body = Body::new(TypeId(1));
        let output = pretty_print(&body, Some("empty"));
        check(
            &output,
            &expect![[r#"
                fn empty() -> ty1 {
                }
            "#]],
        );
    }

    #[test]
    fn body_default_name() {
        let mut body = Body::new(TypeId(1));
        let bb = body.alloc_block();
        body.block_mut(bb).set_terminator(Terminator::return_(0..0));

        let output = pretty_print(&body, None);
        check(
            &output,
            &expect![[r#"
                fn fn() -> ty1 {
                    bb0:
                        return
                }
            "#]],
        );
    }
}
