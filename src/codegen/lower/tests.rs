//! Integration tests for MIR to CLIF lowering.
//!
//! These tests verify correct lowering by JIT compiling MIR and executing it.

use std::mem;

use crate::codegen::CodegenContext;
use crate::codegen::module::{FunctionDef, ModuleCompiler};
use crate::mir::body::{Body, LocalDecl};
use crate::mir::operand::{
    AggregateKind, BinOp, BorrowKind, CastKind, Constant, Operand, Rvalue, UnOp,
};
use crate::mir::statement::Statement;
use crate::mir::terminator::{SwitchTargets, Terminator, TerminatorKind};
use crate::mir::types::{FieldIdx, Local, Place, PlaceElem};
use crate::sema::symbol::DefId;
use crate::sema::types::{Mutability, PrimitiveKind, TypeInterner};

use super::FunctionLowerer;

/// Test runner that JIT compiles MIR and returns executable function pointers.
struct JitTestRunner {
    ctx: CodegenContext,
    types: TypeInterner,
}

impl JitTestRunner {
    fn new() -> Self {
        let ctx = CodegenContext::new_jit().expect("failed to create JIT context");
        let types = TypeInterner::new();
        JitTestRunner { ctx, types }
    }

    /// Compile a MIR body and return a function pointer.
    ///
    /// # Safety
    /// The caller must ensure the function signature matches the MIR body's signature.
    fn compile(&mut self, body: &Body, name: &str) -> *const u8 {
        FunctionLowerer::compile(&mut self.ctx, body, &self.types, name)
            .expect("compilation failed")
    }

    fn types(&self) -> &TypeInterner {
        &self.types
    }

    fn types_mut(&mut self) -> &mut TypeInterner {
        &mut self.types
    }
}

// =============================================================================
// Phase 1: Return Constant (Infrastructure)
// =============================================================================

#[test]
fn lower_return_int_constant() {
    // fn() -> i32 { 42 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // _0 = 42
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));

    // return
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_int_constant");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

#[test]
fn lower_return_bool_constant() {
    // fn() -> bool { true }
    let mut runner = JitTestRunner::new();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::new(bool_ty);
    let entry = body.alloc_block();

    // _0 = true
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Bool(true))),
        0..0,
    ));

    // return
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_bool_constant");
    let func: fn() -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 1);
}

#[test]
fn lower_return_unit() {
    // fn() -> () { }
    let mut runner = JitTestRunner::new();
    let unit_ty = runner.types().unit();

    let mut body = Body::new(unit_ty);
    let entry = body.alloc_block();

    // return (no assignment needed for unit)
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_unit");
    let func: fn() = unsafe { mem::transmute(ptr) };

    // Should compile and execute without panic
    func();
}

// =============================================================================
// Phase 2: Local Variables
// =============================================================================

#[test]
fn lower_local_variable_copy() {
    // fn() -> i32 { let _1 = 42; _0 = copy _1; return }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _local1 = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();

    // _1 = 42
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));

    // _0 = copy _1
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(Place::from_local(Local(1)))),
        0..0,
    ));

    // return
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "local_variable_copy");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

#[test]
fn lower_multiple_locals() {
    // fn() -> i32 { let _1 = 10; let _2 = 20; _0 = copy _2; return }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _local1 = body.alloc_local(LocalDecl::new(i32_ty, true));
    let _local2 = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();

    // _1 = 10
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Int(10))),
        0..0,
    ));

    // _2 = 20
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(2)),
        Rvalue::Use(Operand::Constant(Constant::Int(20))),
        0..0,
    ));

    // _0 = copy _2
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(Place::from_local(Local(2)))),
        0..0,
    ));

    // return
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "multiple_locals");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 20);
}

#[test]
fn lower_operand_move() {
    // fn() -> i32 { let _1 = 42; _0 = move _1; return }
    // For primitives, move and copy should behave the same
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _local1 = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();

    // _1 = 42
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));

    // _0 = move _1
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Move(Place::from_local(Local(1)))),
        0..0,
    ));

    // return
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "operand_move");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

// =============================================================================
// Phase 3: Function Arguments
// =============================================================================

#[test]
fn lower_identity_function() {
    // fn(x: i32) -> i32 { x }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();

    // _0 = copy _1 (the argument)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(Place::from_local(Local(1)))),
        0..0,
    ));

    // return
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "identity_function");
    let func: fn(i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(42), 42);
    assert_eq!(func(0), 0);
    assert_eq!(func(-100), -100);
}

#[test]
fn lower_two_arguments() {
    // fn(a: i32, b: i32) -> i32 { b }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    // _0 = copy _2 (second argument)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(Place::from_local(Local(2)))),
        0..0,
    ));

    // return
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "two_arguments");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(10, 20), 20);
    assert_eq!(func(100, 200), 200);
}

// =============================================================================
// Phase 4: Binary Operations
// =============================================================================

#[test]
fn lower_binop_add() {
    // fn(a: i32, b: i32) -> i32 { a + b }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    // _0 = _1 + _2
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_add");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(10, 32), 42);
    assert_eq!(func(-5, 10), 5);
}

#[test]
fn lower_binop_sub() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Sub,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_sub");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(50, 8), 42);
    assert_eq!(func(10, 15), -5);
}

#[test]
fn lower_binop_mul() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Mul,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_mul");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(6, 7), 42);
    assert_eq!(func(-3, 4), -12);
}

#[test]
fn lower_binop_div() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Div,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_div");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(84, 2), 42);
    assert_eq!(func(10, 3), 3);
}

#[test]
fn lower_binop_rem() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Rem,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_rem");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(10, 3), 1);
    assert_eq!(func(42, 5), 2);
}

#[test]
fn lower_binop_eq() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Eq,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_eq");
    let func: fn(i32, i32) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(5, 5), 1);
    assert_eq!(func(5, 6), 0);
}

#[test]
fn lower_binop_ne() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Ne,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_ne");
    let func: fn(i32, i32) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(5, 5), 0);
    assert_eq!(func(5, 6), 1);
}

#[test]
fn lower_binop_lt() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Lt,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_lt");
    let func: fn(i32, i32) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(3, 5), 1);
    assert_eq!(func(5, 5), 0);
    assert_eq!(func(7, 5), 0);
}

#[test]
fn lower_binop_le() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Le,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_le");
    let func: fn(i32, i32) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(3, 5), 1);
    assert_eq!(func(5, 5), 1);
    assert_eq!(func(7, 5), 0);
}

#[test]
fn lower_binop_gt() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Gt,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_gt");
    let func: fn(i32, i32) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(7, 5), 1);
    assert_eq!(func(5, 5), 0);
    assert_eq!(func(3, 5), 0);
}

#[test]
fn lower_binop_ge() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Ge,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_ge");
    let func: fn(i32, i32) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(7, 5), 1);
    assert_eq!(func(5, 5), 1);
    assert_eq!(func(3, 5), 0);
}

#[test]
fn lower_binop_bitand() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::BitAnd,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_bitand");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(0b1111, 0b1010), 0b1010);
    assert_eq!(func(0xFF, 0x0F), 0x0F);
}

#[test]
fn lower_binop_bitor() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::BitOr,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_bitor");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(0b1100, 0b0011), 0b1111);
    assert_eq!(func(0xF0, 0x0F), 0xFF);
}

#[test]
fn lower_binop_bitxor() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::BitXor,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_bitxor");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(0b1111, 0b1010), 0b0101);
    assert_eq!(func(0xFF, 0xFF), 0x00);
}

#[test]
fn lower_binop_shl() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Shl,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_shl");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(1, 4), 16);
    assert_eq!(func(0b0001, 3), 0b1000);
}

#[test]
fn lower_binop_shr() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Shr,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "binop_shr");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(16, 2), 4);
    assert_eq!(func(0b1000, 3), 0b0001);
    // Test arithmetic shift (sign extension)
    assert_eq!(func(-8, 2), -2);
}

// =============================================================================
// Phase 5: Unary Operations
// =============================================================================

#[test]
fn lower_unop_neg() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(Place::from_local(Local(1)))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "unop_neg");
    let func: fn(i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(42), -42);
    assert_eq!(func(-10), 10);
    assert_eq!(func(0), 0);
}

#[test]
fn lower_unop_not_bool() {
    let mut runner = JitTestRunner::new();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(bool_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::UnaryOp(UnOp::Not, Operand::Copy(Place::from_local(Local(1)))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "unop_not_bool");
    let func: fn(i8) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(1), 0);
    assert_eq!(func(0), 1);
}

#[test]
fn lower_unop_not_int() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::UnaryOp(UnOp::Not, Operand::Copy(Place::from_local(Local(1)))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "unop_not_int");
    let func: fn(i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(0), -1);
    assert_eq!(func(-1), 0);
}

// =============================================================================
// Phase 6: Goto (Unconditional Branch)
// =============================================================================

#[test]
fn lower_goto() {
    // bb0: goto bb1
    // bb1: _0 = 42; return
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let bb0 = body.alloc_block();
    let bb1 = body.alloc_block();

    // bb0: goto bb1
    body.block_mut(bb0)
        .set_terminator(Terminator::goto(bb1, 0..0));

    // bb1: _0 = 42; return
    body.block_mut(bb1).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));
    body.block_mut(bb1)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "goto");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

#[test]
fn lower_goto_chain() {
    // bb0 -> bb1 -> bb2 -> return
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let bb0 = body.alloc_block();
    let bb1 = body.alloc_block();
    let bb2 = body.alloc_block();

    // bb0: goto bb1
    body.block_mut(bb0)
        .set_terminator(Terminator::goto(bb1, 0..0));

    // bb1: goto bb2
    body.block_mut(bb1)
        .set_terminator(Terminator::goto(bb2, 0..0));

    // bb2: _0 = 42; return
    body.block_mut(bb2).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));
    body.block_mut(bb2)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "goto_chain");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

// =============================================================================
// Phase 7: SwitchInt (Conditional Branch)
// =============================================================================

#[test]
fn lower_switch_bool() {
    // if cond { 1 } else { 2 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(i32_ty, &[(bool_ty, false)]);
    let entry = body.alloc_block();
    let then_block = body.alloc_block();
    let else_block = body.alloc_block();

    // entry: switchInt(_1) -> [0: else_block, otherwise: then_block]
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::from_local(Local(1))),
            targets: SwitchTargets::new_bool(then_block, else_block),
        },
        0..0,
    ));

    // then_block: _0 = 1; return
    body.block_mut(then_block).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(1))),
        0..0,
    ));
    body.block_mut(then_block)
        .set_terminator(Terminator::return_(0..0));

    // else_block: _0 = 2; return
    body.block_mut(else_block).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(2))),
        0..0,
    ));
    body.block_mut(else_block)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "switch_bool");
    let func: fn(i8) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(1), 1); // true -> 1
    assert_eq!(func(0), 2); // false -> 2
}

#[test]
fn lower_switch_int() {
    // match x { 0 => 10, 1 => 20, _ => 30 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();
    let case_0 = body.alloc_block();
    let case_1 = body.alloc_block();
    let default = body.alloc_block();

    // entry: switchInt(_1) -> [0: case_0, 1: case_1, otherwise: default]
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::from_local(Local(1))),
            targets: SwitchTargets::new(vec![(0, case_0), (1, case_1)], default),
        },
        0..0,
    ));

    // case_0: _0 = 10; return
    body.block_mut(case_0).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(10))),
        0..0,
    ));
    body.block_mut(case_0)
        .set_terminator(Terminator::return_(0..0));

    // case_1: _0 = 20; return
    body.block_mut(case_1).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(20))),
        0..0,
    ));
    body.block_mut(case_1)
        .set_terminator(Terminator::return_(0..0));

    // default: _0 = 30; return
    body.block_mut(default).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(30))),
        0..0,
    ));
    body.block_mut(default)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "switch_int");
    let func: fn(i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(0), 10);
    assert_eq!(func(1), 20);
    assert_eq!(func(2), 30);
    assert_eq!(func(100), 30);
}

// =============================================================================
// Phase 8: Loops
// =============================================================================

#[test]
fn lower_loop_countdown() {
    // sum = 0; while n > 0 { sum += n; n -= 1; } return sum;
    // For n=5: 5+4+3+2+1 = 15
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    // Locals: _0 = return, _1 = arg n, _2 = sum, _3 = cond temp
    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let _sum = body.alloc_local(LocalDecl::new(i32_ty, true)); // _2
    let _cond = body.alloc_local(LocalDecl::new(bool_ty, true)); // _3

    let entry = body.alloc_block();
    let loop_header = body.alloc_block();
    let loop_body = body.alloc_block();
    let exit = body.alloc_block();

    // entry: _2 = 0; goto loop_header
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(2)),
        Rvalue::Use(Operand::Constant(Constant::Int(0))),
        0..0,
    ));
    body.block_mut(entry)
        .set_terminator(Terminator::goto(loop_header, 0..0));

    // loop_header: _3 = _1 > 0; switchInt(_3) -> [0: exit, otherwise: loop_body]
    body.block_mut(loop_header)
        .push_statement(Statement::assign(
            Place::from_local(Local(3)),
            Rvalue::BinaryOp(
                BinOp::Gt,
                Operand::Copy(Place::from_local(Local(1))),
                Operand::Constant(Constant::Int(0)),
            ),
            0..0,
        ));
    body.block_mut(loop_header).set_terminator(Terminator::new(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::from_local(Local(3))),
            targets: SwitchTargets::new_bool(loop_body, exit),
        },
        0..0,
    ));

    // loop_body: _2 = _2 + _1; _1 = _1 - 1; goto loop_header
    body.block_mut(loop_body).push_statement(Statement::assign(
        Place::from_local(Local(2)),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(2))),
            Operand::Copy(Place::from_local(Local(1))),
        ),
        0..0,
    ));
    body.block_mut(loop_body).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::BinaryOp(
            BinOp::Sub,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Constant(Constant::Int(1)),
        ),
        0..0,
    ));
    body.block_mut(loop_body)
        .set_terminator(Terminator::goto(loop_header, 0..0));

    // exit: _0 = _2; return
    body.block_mut(exit).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(Place::from_local(Local(2)))),
        0..0,
    ));
    body.block_mut(exit)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "loop_countdown");
    let func: fn(i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(5), 15);
    assert_eq!(func(0), 0);
    assert_eq!(func(10), 55);
}

// =============================================================================
// Phase 9: Type Casts
// =============================================================================

#[test]
fn lower_cast_i8_to_i32() {
    let mut runner = JitTestRunner::new();
    let i8_ty = runner.types_mut().primitive(PrimitiveKind::I8);
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i8_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::IntToInt,
            Operand::Copy(Place::from_local(Local(1))),
            i32_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "cast_i8_to_i32");
    let func: fn(i8) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(42), 42);
    assert_eq!(func(-1), -1); // Sign extension
}

#[test]
fn lower_cast_i32_to_i8() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let i8_ty = runner.types_mut().primitive(PrimitiveKind::I8);

    let mut body = Body::with_args(i8_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::IntToInt,
            Operand::Copy(Place::from_local(Local(1))),
            i8_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "cast_i32_to_i8");
    let func: fn(i32) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(42), 42);
    assert_eq!(func(256), 0); // Truncation
}

#[test]
fn lower_cast_i32_to_f64() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::IntToFloat,
            Operand::Copy(Place::from_local(Local(1))),
            f64_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "cast_i32_to_f64");
    let func: fn(i32) -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func(42) - 42.0).abs() < f64::EPSILON);
    assert!((func(-10) - (-10.0)).abs() < f64::EPSILON);
}

#[test]
fn lower_cast_f64_to_i32() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::FloatToInt,
            Operand::Copy(Place::from_local(Local(1))),
            i32_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "cast_f64_to_i32");
    let func: fn(f64) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(42.7), 42); // Truncates toward zero
    assert_eq!(func(-10.9), -10);
}

// =============================================================================
// Phase 10: Floating Point
// =============================================================================

#[test]
fn lower_float_add() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_add");
    let func: fn(f64, f64) -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func(1.5, 2.5) - 4.0).abs() < f64::EPSILON);
}

#[test]
fn lower_float_sub() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Sub,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_sub");
    let func: fn(f64, f64) -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func(5.0, 3.0) - 2.0).abs() < f64::EPSILON);
}

#[test]
fn lower_float_mul() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Mul,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_mul");
    let func: fn(f64, f64) -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func(3.0, 4.0) - 12.0).abs() < f64::EPSILON);
}

#[test]
fn lower_float_div() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Div,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_div");
    let func: fn(f64, f64) -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func(10.0, 4.0) - 2.5).abs() < f64::EPSILON);
}

#[test]
fn lower_float_lt() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Lt,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_lt");
    let func: fn(f64, f64) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(1.0, 2.0), 1);
    assert_eq!(func(2.0, 2.0), 0);
    assert_eq!(func(3.0, 2.0), 0);
}

// =============================================================================
// Phase 11: Storage Statements
// =============================================================================

#[test]
fn lower_storage_markers() {
    // StorageLive/StorageDead should be no-ops that don't break execution
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let local = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();

    // StorageLive(_1)
    body.block_mut(entry)
        .push_statement(Statement::storage_live(local, 0..0));

    // _1 = 42
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(local),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));

    // _0 = copy _1
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(Place::from_local(local))),
        0..0,
    ));

    // StorageDead(_1)
    body.block_mut(entry)
        .push_statement(Statement::storage_dead(local, 0..0));

    // return
    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "storage_markers");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

// =============================================================================
// Additional Coverage Tests
// =============================================================================

#[test]
fn lower_return_float_constant() {
    // fn() -> f64 { 3.14 }
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::new(f64_ty);
    let entry = body.alloc_block();

    // _0 = 1.234
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Float(1.234))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_float_constant");
    let func: fn() -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func() - 1.234).abs() < f64::EPSILON);
}

#[test]
fn lower_return_char_constant() {
    // fn() -> char { 'A' } (char is i32 internally)
    let mut runner = JitTestRunner::new();
    let char_ty = runner.types_mut().primitive(PrimitiveKind::Char);

    let mut body = Body::new(char_ty);
    let entry = body.alloc_block();

    // _0 = 'A' (65)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Char('A'))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_char_constant");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 65); // 'A' = 65
}

#[test]
fn lower_unop_neg_float() {
    // fn(x: f64) -> f64 { -x }
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(Place::from_local(Local(1)))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "unop_neg_float");
    let func: fn(f64) -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func(3.5) - (-3.5)).abs() < f64::EPSILON);
    assert!((func(-2.5) - 2.5).abs() < f64::EPSILON);
    assert!((func(0.0) - 0.0).abs() < f64::EPSILON);
}

#[test]
fn lower_float_eq() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Eq,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_eq");
    let func: fn(f64, f64) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(1.5, 1.5), 1);
    assert_eq!(func(1.5, 2.5), 0);
}

#[test]
fn lower_float_ne() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Ne,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_ne");
    let func: fn(f64, f64) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(1.5, 1.5), 0);
    assert_eq!(func(1.5, 2.5), 1);
}

#[test]
fn lower_float_le() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Le,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_le");
    let func: fn(f64, f64) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(1.0, 2.0), 1);
    assert_eq!(func(2.0, 2.0), 1);
    assert_eq!(func(3.0, 2.0), 0);
}

#[test]
fn lower_float_gt() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Gt,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_gt");
    let func: fn(f64, f64) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(3.0, 2.0), 1);
    assert_eq!(func(2.0, 2.0), 0);
    assert_eq!(func(1.0, 2.0), 0);
}

#[test]
fn lower_float_ge() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(bool_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Ge,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "float_ge");
    let func: fn(f64, f64) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(3.0, 2.0), 1);
    assert_eq!(func(2.0, 2.0), 1);
    assert_eq!(func(1.0, 2.0), 0);
}

#[test]
fn lower_cast_f32_to_f64() {
    let mut runner = JitTestRunner::new();
    let f32_ty = runner.types_mut().f32();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(f32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::FloatToFloat,
            Operand::Copy(Place::from_local(Local(1))),
            f64_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "cast_f32_to_f64");
    let func: fn(f32) -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func(3.5f32) - 3.5f64).abs() < f64::EPSILON);
}

#[test]
fn lower_cast_f64_to_f32() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();
    let f32_ty = runner.types_mut().f32();

    let mut body = Body::with_args(f32_ty, &[(f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::FloatToFloat,
            Operand::Copy(Place::from_local(Local(1))),
            f32_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "cast_f64_to_f32");
    let func: fn(f64) -> f32 = unsafe { mem::transmute(ptr) };

    assert!((func(3.5f64) - 3.5f32).abs() < f32::EPSILON);
}

#[test]
fn lower_f32_add() {
    let mut runner = JitTestRunner::new();
    let f32_ty = runner.types_mut().f32();

    let mut body = Body::with_args(f32_ty, &[(f32_ty, false), (f32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "f32_add");
    let func: fn(f32, f32) -> f32 = unsafe { mem::transmute(ptr) };

    assert!((func(1.5f32, 2.5f32) - 4.0f32).abs() < f32::EPSILON);
}

#[test]
fn lower_i64_add() {
    let mut runner = JitTestRunner::new();
    let i64_ty = runner.types_mut().i64();

    let mut body = Body::with_args(i64_ty, &[(i64_ty, false), (i64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "i64_add");
    let func: fn(i64, i64) -> i64 = unsafe { mem::transmute(ptr) };

    assert_eq!(
        func(10_000_000_000i64, 20_000_000_000i64),
        30_000_000_000i64
    );
}

#[test]
fn lower_i16_add() {
    let mut runner = JitTestRunner::new();
    let i16_ty = runner.types_mut().primitive(PrimitiveKind::I16);

    let mut body = Body::with_args(i16_ty, &[(i16_ty, false), (i16_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "i16_add");
    let func: fn(i16, i16) -> i16 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(100i16, 200i16), 300i16);
}

#[test]
fn lower_i8_add() {
    let mut runner = JitTestRunner::new();
    let i8_ty = runner.types_mut().primitive(PrimitiveKind::I8);

    let mut body = Body::with_args(i8_ty, &[(i8_ty, false), (i8_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "i8_add");
    let func: fn(i8, i8) -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(10i8, 20i8), 30i8);
}

#[test]
fn lower_nested_loops() {
    // outer_sum = 0
    // for i in 1..=2:
    //     for j in 1..=3:
    //         outer_sum += i * j
    // return outer_sum  // 1*1 + 1*2 + 1*3 + 2*1 + 2*2 + 2*3 = 6 + 12 = 18
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    // Locals: _0=return, _1=sum, _2=i, _3=j, _4=temp_cond, _5=temp_product
    let mut body = Body::new(i32_ty);
    let _sum = body.alloc_local(LocalDecl::new(i32_ty, true)); // _1
    let _i = body.alloc_local(LocalDecl::new(i32_ty, true)); // _2
    let _j = body.alloc_local(LocalDecl::new(i32_ty, true)); // _3
    let _cond = body.alloc_local(LocalDecl::new(bool_ty, true)); // _4
    let _prod = body.alloc_local(LocalDecl::new(i32_ty, true)); // _5

    let entry = body.alloc_block(); // bb0
    let outer_header = body.alloc_block(); // bb1
    let inner_header = body.alloc_block(); // bb2
    let inner_body = body.alloc_block(); // bb3
    let inner_exit = body.alloc_block(); // bb4: increment i, loop back to outer
    let outer_exit = body.alloc_block(); // bb5: return

    // bb0 (entry): sum=0, i=1, goto outer_header
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Int(0))),
        0..0,
    ));
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(2)),
        Rvalue::Use(Operand::Constant(Constant::Int(1))),
        0..0,
    ));
    body.block_mut(entry)
        .set_terminator(Terminator::goto(outer_header, 0..0));

    // bb1 (outer_header): if i <= 2 goto inner_init else outer_exit
    body.block_mut(outer_header)
        .push_statement(Statement::assign(
            Place::from_local(Local(4)),
            Rvalue::BinaryOp(
                BinOp::Le,
                Operand::Copy(Place::from_local(Local(2))),
                Operand::Constant(Constant::Int(2)),
            ),
            0..0,
        ));
    // Also reset j=1 for inner loop
    body.block_mut(outer_header)
        .push_statement(Statement::assign(
            Place::from_local(Local(3)),
            Rvalue::Use(Operand::Constant(Constant::Int(1))),
            0..0,
        ));
    body.block_mut(outer_header).set_terminator(Terminator::new(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::from_local(Local(4))),
            targets: SwitchTargets::new_bool(inner_header, outer_exit),
        },
        0..0,
    ));

    // bb2 (inner_header): if j <= 3 goto inner_body else inner_exit
    body.block_mut(inner_header)
        .push_statement(Statement::assign(
            Place::from_local(Local(4)),
            Rvalue::BinaryOp(
                BinOp::Le,
                Operand::Copy(Place::from_local(Local(3))),
                Operand::Constant(Constant::Int(3)),
            ),
            0..0,
        ));
    body.block_mut(inner_header).set_terminator(Terminator::new(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::from_local(Local(4))),
            targets: SwitchTargets::new_bool(inner_body, inner_exit),
        },
        0..0,
    ));

    // bb3 (inner_body): sum += i * j; j += 1; goto inner_header
    body.block_mut(inner_body).push_statement(Statement::assign(
        Place::from_local(Local(5)),
        Rvalue::BinaryOp(
            BinOp::Mul,
            Operand::Copy(Place::from_local(Local(2))),
            Operand::Copy(Place::from_local(Local(3))),
        ),
        0..0,
    ));
    body.block_mut(inner_body).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(5))),
        ),
        0..0,
    ));
    body.block_mut(inner_body).push_statement(Statement::assign(
        Place::from_local(Local(3)),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(3))),
            Operand::Constant(Constant::Int(1)),
        ),
        0..0,
    ));
    body.block_mut(inner_body)
        .set_terminator(Terminator::goto(inner_header, 0..0));

    // bb4 (inner_exit): i += 1; goto outer_header
    body.block_mut(inner_exit).push_statement(Statement::assign(
        Place::from_local(Local(2)),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(2))),
            Operand::Constant(Constant::Int(1)),
        ),
        0..0,
    ));
    body.block_mut(inner_exit)
        .set_terminator(Terminator::goto(outer_header, 0..0));

    // bb5 (outer_exit): _0 = sum; return
    body.block_mut(outer_exit).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(Place::from_local(Local(1)))),
        0..0,
    ));
    body.block_mut(outer_exit)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "nested_loops");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    // 1*1 + 1*2 + 1*3 + 2*1 + 2*2 + 2*3 = 1+2+3+2+4+6 = 18
    assert_eq!(func(), 18);
}

#[test]
fn lower_drop_terminator() {
    // Drop is a no-op that jumps to target
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let local = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();
    let after_drop = body.alloc_block();

    // entry: _1 = 42; drop(_1) -> after_drop
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(local),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Drop {
            place: Place::from_local(local),
            target: after_drop,
        },
        0..0,
    ));

    // after_drop: _0 = 42; return
    body.block_mut(after_drop).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));
    body.block_mut(after_drop)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "drop_terminator");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

#[test]
fn lower_assert_terminator() {
    // Assert is currently a no-op that jumps to target
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::new(i32_ty);
    let _cond_local = body.alloc_local(LocalDecl::new(bool_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _1 = true; assert(_1, expected=true) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Bool(true))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: true,
            target: after_assert,
        },
        0..0,
    ));

    // after_assert: _0 = 99; return
    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(99))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_terminator");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 99);
}

#[test]
fn lower_nop_statement() {
    // Nop should do nothing
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // Several nops followed by actual code
    body.block_mut(entry).push_statement(Statement::nop(0..0));
    body.block_mut(entry).push_statement(Statement::nop(0..0));
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(77))),
        0..0,
    ));
    body.block_mut(entry).push_statement(Statement::nop(0..0));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "nop_statement");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 77);
}

#[test]
fn lower_switch_empty_targets() {
    // Switch with no specific targets, just otherwise
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();
    let default_block = body.alloc_block();

    // switch(_1) -> [otherwise: default_block]
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::from_local(Local(1))),
            targets: SwitchTargets::new(vec![], default_block),
        },
        0..0,
    ));

    body.block_mut(default_block)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(100))),
            0..0,
        ));
    body.block_mut(default_block)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "switch_empty_targets");
    let func: fn(i32) -> i32 = unsafe { mem::transmute(ptr) };

    // Any value should go to default
    assert_eq!(func(0), 100);
    assert_eq!(func(42), 100);
    assert_eq!(func(-1), 100);
}

#[test]
fn lower_bool_return_false() {
    // fn() -> bool { false }
    let mut runner = JitTestRunner::new();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::new(bool_ty);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Bool(false))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_bool_false");
    let func: fn() -> i8 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 0);
}

#[test]
fn lower_negative_int_constant() {
    // fn() -> i32 { -42 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Int(-42))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_negative_int");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), -42);
}

#[test]
fn lower_negative_float_constant() {
    // fn() -> f64 { -2.5 }
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::new(f64_ty);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Float(-2.5))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_negative_float");
    let func: fn() -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func() - (-2.5)).abs() < f64::EPSILON);
}

#[test]
fn lower_unicode_char_constant() {
    // fn() -> char { '中' } (Unicode char)
    let mut runner = JitTestRunner::new();
    let char_ty = runner.types_mut().primitive(PrimitiveKind::Char);

    let mut body = Body::new(char_ty);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Char('中'))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "return_unicode_char");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), '中' as i32); // 20013
}

#[test]
fn lower_comparison_to_i32() {
    // fn(a: i32, b: i32) -> i32 { (a < b) as i32 }
    // Tests comparison result (i8) being extended to i32
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let entry = body.alloc_block();

    // _0 = _1 < _2 (result is i8, but dest is i32, triggers uextend)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Lt,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "comparison_to_i32");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(3, 5), 1);
    assert_eq!(func(5, 3), 0);
}

#[test]
fn lower_cast_i32_to_i32() {
    // Same-size cast should be a no-op
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::IntToInt,
            Operand::Copy(Place::from_local(Local(1))),
            i32_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "cast_i32_to_i32");
    let func: fn(i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(42), 42);
    assert_eq!(func(-100), -100);
}

#[test]
fn lower_cast_f64_to_f64() {
    // Same-size float cast should be a no-op
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(f64_ty, false)]);
    let entry = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::FloatToFloat,
            Operand::Copy(Place::from_local(Local(1))),
            f64_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "cast_f64_to_f64");
    let func: fn(f64) -> f64 = unsafe { mem::transmute(ptr) };

    assert!((func(3.5) - 3.5).abs() < f64::EPSILON);
}

#[test]
fn lower_three_arguments() {
    // fn(a: i32, b: i32, c: i32) -> i32 { a + b + c }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false), (i32_ty, false)]);
    let temp = body.alloc_local(LocalDecl::new(i32_ty, true)); // _4
    let entry = body.alloc_block();

    // _4 = _1 + _2
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(temp),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    // _0 = _4 + _3
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(temp)),
            Operand::Copy(Place::from_local(Local(3))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "three_arguments");
    let func: fn(i32, i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(10, 20, 12), 42);
}

#[test]
fn lower_many_basic_blocks() {
    // Test with many sequential blocks to ensure block mapping works
    // bb0 -> bb1 -> bb2 -> bb3 -> bb4 -> return
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let temp = body.alloc_local(LocalDecl::new(i32_ty, true)); // _1

    let bb0 = body.alloc_block();
    let bb1 = body.alloc_block();
    let bb2 = body.alloc_block();
    let bb3 = body.alloc_block();
    let bb4 = body.alloc_block();

    // bb0: _1 = 1; goto bb1
    body.block_mut(bb0).push_statement(Statement::assign(
        Place::from_local(temp),
        Rvalue::Use(Operand::Constant(Constant::Int(1))),
        0..0,
    ));
    body.block_mut(bb0)
        .set_terminator(Terminator::goto(bb1, 0..0));

    // bb1: _1 = _1 + 2; goto bb2
    body.block_mut(bb1).push_statement(Statement::assign(
        Place::from_local(temp),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(temp)),
            Operand::Constant(Constant::Int(2)),
        ),
        0..0,
    ));
    body.block_mut(bb1)
        .set_terminator(Terminator::goto(bb2, 0..0));

    // bb2: _1 = _1 + 4; goto bb3
    body.block_mut(bb2).push_statement(Statement::assign(
        Place::from_local(temp),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(temp)),
            Operand::Constant(Constant::Int(4)),
        ),
        0..0,
    ));
    body.block_mut(bb2)
        .set_terminator(Terminator::goto(bb3, 0..0));

    // bb3: _1 = _1 + 8; goto bb4
    body.block_mut(bb3).push_statement(Statement::assign(
        Place::from_local(temp),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(temp)),
            Operand::Constant(Constant::Int(8)),
        ),
        0..0,
    ));
    body.block_mut(bb3)
        .set_terminator(Terminator::goto(bb4, 0..0));

    // bb4: _0 = _1 + 16; return
    body.block_mut(bb4).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(temp)),
            Operand::Constant(Constant::Int(16)),
        ),
        0..0,
    ));
    body.block_mut(bb4)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "many_basic_blocks");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    // 1 + 2 + 4 + 8 + 16 = 31
    assert_eq!(func(), 31);
}

#[test]
fn lower_diamond_control_flow() {
    // Diamond pattern: entry -> (then_block | else_block) -> merge -> return
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(i32_ty, &[(bool_ty, false)]);
    let result = body.alloc_local(LocalDecl::new(i32_ty, true)); // _2

    let entry = body.alloc_block();
    let then_block = body.alloc_block();
    let else_block = body.alloc_block();
    let merge = body.alloc_block();

    // entry: switchInt(_1) -> [0: else_block, otherwise: then_block]
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::from_local(Local(1))),
            targets: SwitchTargets::new_bool(then_block, else_block),
        },
        0..0,
    ));

    // then_block: _2 = 100; goto merge
    body.block_mut(then_block).push_statement(Statement::assign(
        Place::from_local(result),
        Rvalue::Use(Operand::Constant(Constant::Int(100))),
        0..0,
    ));
    body.block_mut(then_block)
        .set_terminator(Terminator::goto(merge, 0..0));

    // else_block: _2 = 200; goto merge
    body.block_mut(else_block).push_statement(Statement::assign(
        Place::from_local(result),
        Rvalue::Use(Operand::Constant(Constant::Int(200))),
        0..0,
    ));
    body.block_mut(else_block)
        .set_terminator(Terminator::goto(merge, 0..0));

    // merge: _0 = _2; return
    body.block_mut(merge).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(Place::from_local(result))),
        0..0,
    ));
    body.block_mut(merge)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "diamond_control_flow");
    let func: fn(i8) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(1), 100); // true
    assert_eq!(func(0), 200); // false
}

#[test]
fn lower_constant_in_binop() {
    // fn() -> i32 { 10 + 32 } - both operands are constants
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // _0 = 10 + 32
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Constant(Constant::Int(10)),
            Operand::Constant(Constant::Int(32)),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "constant_in_binop");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

#[test]
fn lower_zst_argument() {
    // fn(a: i32, _: (), b: i32) -> i32 { a + b }
    // Unit argument should be skipped in the ABI
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let unit_ty = runner.types().unit();

    let mut body = Body::with_args(
        i32_ty,
        &[(i32_ty, false), (unit_ty, false), (i32_ty, false)],
    );
    let entry = body.alloc_block();

    // _0 = _1 + _3 (skipping unit _2)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(3))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "zst_argument");
    // The ABI only has two i32 params since unit is ZST
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(10, 32), 42);
}

// =============================================================================
// Ignored Tests: Features Not Yet Implemented
// =============================================================================
// These tests document features that are not yet supported. They are marked
// #[ignore] and will fail with CodegenError until the features are implemented.

#[test]
fn lower_place_field_projection() {
    // Accessing a field of a tuple: place.field
    // fn(tuple: (i32, i32)) -> i32 { tuple.0 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let tuple_ty = runner.types_mut().mk_tuple(vec![i32_ty, i32_ty]);

    // Function takes a tuple as argument and returns its first field
    let mut body = Body::with_args(i32_ty, &[(tuple_ty, false)]);
    let entry = body.alloc_block();

    // _0 = (_1).0 - field projection
    let place_with_projection = Place::from_local(Local(1)).project(PlaceElem::Field(FieldIdx(0)));
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(place_with_projection)),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "place_field_projection");
    // Note: Can't easily call this function because tuples are passed by pointer on the ABI level
}

#[test]
fn lower_place_deref_projection() {
    // Dereferencing a pointer: *place
    // fn(ptr: &i32) -> i32 { *ptr }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let ref_ty = runner.types_mut().mk_ref(Mutability::Shared, i32_ty);

    // Function takes a pointer as argument and dereferences it
    let mut body = Body::with_args(i32_ty, &[(ref_ty, false)]);
    let entry = body.alloc_block();

    // _0 = *_1 - deref projection
    let place_with_deref = Place::deref(Local(1));
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Copy(place_with_deref)),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "place_deref_projection");

    // Test it: create an i32 on the stack and pass its address
    let value: i32 = 42;
    let func: fn(*const i32) -> i32 = unsafe { mem::transmute(ptr) };
    assert_eq!(func(&value as *const i32), 42);
}

#[test]
#[ignore = "string constants not yet supported"]
fn lower_string_constant() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // Use a string constant (should error)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::String("hello".to_string()))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "string_constant");
}

#[test]
#[ignore = "function references not yet supported"]
fn lower_fn_def_constant() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // Use a function definition constant (should error)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::FnDef(DefId(1)))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "fn_def_constant");
}

#[test]
fn lower_zeroed_constant() {
    // fn() -> i32 { zeroed::<i32>() } // returns 0
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // _0 = zeroed::<i32>()
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::Constant(Constant::Zeroed(i32_ty))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "zeroed_constant");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };
    assert_eq!(func(), 0);
}

// =============================================================================
// Phase: Function Calls
// =============================================================================

#[test]
fn lower_call_no_args() {
    let types = TypeInterner::new();
    let i32_ty = types.i32();

    // fn callee() -> i32 { 42 }
    let mut callee_body = Body::new(i32_ty);
    let callee_entry = callee_body.alloc_block();
    callee_body
        .block_mut(callee_entry)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(42)),
            0..0,
        ));
    callee_body
        .block_mut(callee_entry)
        .set_terminator(Terminator::return_(0..0));

    // fn caller() -> i32 { callee() }
    let mut caller_body = Body::new(i32_ty);
    let caller_entry = caller_body.alloc_block();
    let after_call = caller_body.alloc_block();

    caller_body
        .block_mut(caller_entry)
        .set_terminator(Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId(1))), // callee
                args: vec![],
                destination: Place::from_local(Local::RETURN_PLACE),
                target: Some(after_call),
            },
            0..0,
        ));

    caller_body
        .block_mut(after_call)
        .set_terminator(Terminator::return_(0..0));

    let functions = [
        FunctionDef::new(DefId(1), "callee", &callee_body),
        FunctionDef::new(DefId(2), "caller", &caller_body),
    ];
    let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

    let caller_ptr = module.get_function_ptr(DefId(2)).unwrap();
    let caller: fn() -> i32 = unsafe { mem::transmute(caller_ptr) };

    assert_eq!(caller(), 42);
}

#[test]
fn lower_call_with_args() {
    let types = TypeInterner::new();
    let i32_ty = types.i32();

    // fn add(a: i32, b: i32) -> i32 { a + b }
    let mut add_body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let add_entry = add_body.alloc_block();

    add_body
        .block_mut(add_entry)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::copy_local(Local(1)),
                Operand::copy_local(Local(2)),
            ),
            0..0,
        ));
    add_body
        .block_mut(add_entry)
        .set_terminator(Terminator::return_(0..0));

    // fn caller() -> i32 { add(10, 32) }
    let mut caller_body = Body::new(i32_ty);
    let caller_entry = caller_body.alloc_block();
    let after_call = caller_body.alloc_block();

    caller_body
        .block_mut(caller_entry)
        .set_terminator(Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId(1))), // add
                args: vec![Operand::const_int(10), Operand::const_int(32)],
                destination: Place::from_local(Local::RETURN_PLACE),
                target: Some(after_call),
            },
            0..0,
        ));

    caller_body
        .block_mut(after_call)
        .set_terminator(Terminator::return_(0..0));

    let functions = [
        FunctionDef::new(DefId(1), "add", &add_body),
        FunctionDef::new(DefId(2), "caller", &caller_body),
    ];
    let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

    let caller_ptr = module.get_function_ptr(DefId(2)).unwrap();
    let caller: fn() -> i32 = unsafe { mem::transmute(caller_ptr) };

    assert_eq!(caller(), 42);
}

#[test]
fn lower_recursive_call() {
    let types = TypeInterner::new();
    let i32_ty = types.i32();

    // fn factorial(n: i32) -> i32 {
    //     if n <= 1 { 1 } else { n * factorial(n - 1) }
    // }
    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);

    // Locals:
    // _0 = return place
    // _1 = n (argument)
    // _2 = temp for n <= 1
    // _3 = temp for n - 1
    // _4 = temp for factorial(n - 1)
    // _5 = temp for n * factorial(n - 1)
    let _temp_cond = body.alloc_local(LocalDecl::new(i32_ty, true)); // _2 - using i32 for bool
    let _temp_n_minus_1 = body.alloc_local(LocalDecl::new(i32_ty, true)); // _3
    let _temp_rec_result = body.alloc_local(LocalDecl::new(i32_ty, true)); // _4
    let _temp_mul = body.alloc_local(LocalDecl::new(i32_ty, true)); // _5

    let entry = body.alloc_block(); // bb0
    let then_block = body.alloc_block(); // bb1 - return 1
    let else_block = body.alloc_block(); // bb2 - compute n - 1
    let call_block = body.alloc_block(); // bb3 - call factorial
    let after_call = body.alloc_block(); // bb4 - multiply and return

    // bb0: entry - check if n <= 1
    // _2 = n <= 1
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(2)),
        Rvalue::BinaryOp(
            BinOp::Le,
            Operand::copy_local(Local(1)),
            Operand::const_int(1),
        ),
        0..0,
    ));
    // switch on _2
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::SwitchInt {
            discr: Operand::copy_local(Local(2)),
            targets: SwitchTargets::new_bool(then_block, else_block),
        },
        0..0,
    ));

    // bb1: then - return 1
    body.block_mut(then_block).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::const_int(1)),
        0..0,
    ));
    body.block_mut(then_block)
        .set_terminator(Terminator::return_(0..0));

    // bb2: else - compute n - 1
    body.block_mut(else_block).push_statement(Statement::assign(
        Place::from_local(Local(3)),
        Rvalue::BinaryOp(
            BinOp::Sub,
            Operand::copy_local(Local(1)),
            Operand::const_int(1),
        ),
        0..0,
    ));
    body.block_mut(else_block)
        .set_terminator(Terminator::goto(call_block, 0..0));

    // bb3: call factorial(n - 1)
    body.block_mut(call_block).set_terminator(Terminator::new(
        TerminatorKind::Call {
            func: Operand::Constant(Constant::FnDef(DefId(1))), // factorial (self)
            args: vec![Operand::copy_local(Local(3))],          // n - 1
            destination: Place::from_local(Local(4)),           // result
            target: Some(after_call),
        },
        0..0,
    ));

    // bb4: multiply n * result and return
    body.block_mut(after_call).push_statement(Statement::assign(
        Place::from_local(Local(5)),
        Rvalue::BinaryOp(
            BinOp::Mul,
            Operand::copy_local(Local(1)),
            Operand::copy_local(Local(4)),
        ),
        0..0,
    ));
    body.block_mut(after_call).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::copy_local(Local(5))),
        0..0,
    ));
    body.block_mut(after_call)
        .set_terminator(Terminator::return_(0..0));

    let functions = [FunctionDef::new(DefId(1), "factorial", &body)];
    let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

    let factorial_ptr = module.get_function_ptr(DefId(1)).unwrap();
    let factorial: fn(i32) -> i32 = unsafe { mem::transmute(factorial_ptr) };

    assert_eq!(factorial(0), 1);
    assert_eq!(factorial(1), 1);
    assert_eq!(factorial(5), 120);
    assert_eq!(factorial(10), 3628800);
}

#[test]
fn lower_mutual_recursion() {
    let types = TypeInterner::new();
    let i32_ty = types.i32();

    // fn is_even(n: i32) -> i32 {
    //     if n == 0 { 1 } else { is_odd(n - 1) }
    // }
    // fn is_odd(n: i32) -> i32 {
    //     if n == 0 { 0 } else { is_even(n - 1) }
    // }

    // Build is_even
    let mut even_body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let _even_temp_cond = even_body.alloc_local(LocalDecl::new(i32_ty, true)); // _2
    let _even_temp_n_minus_1 = even_body.alloc_local(LocalDecl::new(i32_ty, true)); // _3

    let even_entry = even_body.alloc_block(); // bb0
    let even_then = even_body.alloc_block(); // bb1
    let even_else = even_body.alloc_block(); // bb2
    let even_call = even_body.alloc_block(); // bb3
    let even_after = even_body.alloc_block(); // bb4

    // bb0: check n == 0
    even_body
        .block_mut(even_entry)
        .push_statement(Statement::assign(
            Place::from_local(Local(2)),
            Rvalue::BinaryOp(
                BinOp::Eq,
                Operand::copy_local(Local(1)),
                Operand::const_int(0),
            ),
            0..0,
        ));
    even_body
        .block_mut(even_entry)
        .set_terminator(Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::copy_local(Local(2)),
                targets: SwitchTargets::new_bool(even_then, even_else),
            },
            0..0,
        ));

    // bb1: return 1 (true)
    even_body
        .block_mut(even_then)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(1)),
            0..0,
        ));
    even_body
        .block_mut(even_then)
        .set_terminator(Terminator::return_(0..0));

    // bb2: compute n - 1
    even_body
        .block_mut(even_else)
        .push_statement(Statement::assign(
            Place::from_local(Local(3)),
            Rvalue::BinaryOp(
                BinOp::Sub,
                Operand::copy_local(Local(1)),
                Operand::const_int(1),
            ),
            0..0,
        ));
    even_body
        .block_mut(even_else)
        .set_terminator(Terminator::goto(even_call, 0..0));

    // bb3: call is_odd(n - 1)
    even_body
        .block_mut(even_call)
        .set_terminator(Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId(2))), // is_odd
                args: vec![Operand::copy_local(Local(3))],
                destination: Place::from_local(Local::RETURN_PLACE),
                target: Some(even_after),
            },
            0..0,
        ));

    // bb4: return
    even_body
        .block_mut(even_after)
        .set_terminator(Terminator::return_(0..0));

    // Build is_odd (similar structure)
    let mut odd_body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let _odd_temp_cond = odd_body.alloc_local(LocalDecl::new(i32_ty, true)); // _2
    let _odd_temp_n_minus_1 = odd_body.alloc_local(LocalDecl::new(i32_ty, true)); // _3

    let odd_entry = odd_body.alloc_block();
    let odd_then = odd_body.alloc_block();
    let odd_else = odd_body.alloc_block();
    let odd_call = odd_body.alloc_block();
    let odd_after = odd_body.alloc_block();

    // bb0: check n == 0
    odd_body
        .block_mut(odd_entry)
        .push_statement(Statement::assign(
            Place::from_local(Local(2)),
            Rvalue::BinaryOp(
                BinOp::Eq,
                Operand::copy_local(Local(1)),
                Operand::const_int(0),
            ),
            0..0,
        ));
    odd_body
        .block_mut(odd_entry)
        .set_terminator(Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::copy_local(Local(2)),
                targets: SwitchTargets::new_bool(odd_then, odd_else),
            },
            0..0,
        ));

    // bb1: return 0 (false)
    odd_body
        .block_mut(odd_then)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::const_int(0)),
            0..0,
        ));
    odd_body
        .block_mut(odd_then)
        .set_terminator(Terminator::return_(0..0));

    // bb2: compute n - 1
    odd_body
        .block_mut(odd_else)
        .push_statement(Statement::assign(
            Place::from_local(Local(3)),
            Rvalue::BinaryOp(
                BinOp::Sub,
                Operand::copy_local(Local(1)),
                Operand::const_int(1),
            ),
            0..0,
        ));
    odd_body
        .block_mut(odd_else)
        .set_terminator(Terminator::goto(odd_call, 0..0));

    // bb3: call is_even(n - 1)
    odd_body.block_mut(odd_call).set_terminator(Terminator::new(
        TerminatorKind::Call {
            func: Operand::Constant(Constant::FnDef(DefId(1))), // is_even
            args: vec![Operand::copy_local(Local(3))],
            destination: Place::from_local(Local::RETURN_PLACE),
            target: Some(odd_after),
        },
        0..0,
    ));

    // bb4: return
    odd_body
        .block_mut(odd_after)
        .set_terminator(Terminator::return_(0..0));

    let functions = [
        FunctionDef::new(DefId(1), "is_even", &even_body),
        FunctionDef::new(DefId(2), "is_odd", &odd_body),
    ];
    let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

    let is_even_ptr = module.get_function_ptr(DefId(1)).unwrap();
    let is_odd_ptr = module.get_function_ptr(DefId(2)).unwrap();

    let is_even: fn(i32) -> i32 = unsafe { mem::transmute(is_even_ptr) };
    let is_odd: fn(i32) -> i32 = unsafe { mem::transmute(is_odd_ptr) };

    // 1 = true, 0 = false
    assert_eq!(is_even(0), 1);
    assert_eq!(is_even(1), 0);
    assert_eq!(is_even(2), 1);
    assert_eq!(is_even(10), 1);
    assert_eq!(is_even(11), 0);

    assert_eq!(is_odd(0), 0);
    assert_eq!(is_odd(1), 1);
    assert_eq!(is_odd(2), 0);
    assert_eq!(is_odd(10), 0);
    assert_eq!(is_odd(11), 1);
}

#[test]
fn lower_call_multiple_args() {
    let types = TypeInterner::new();
    let i32_ty = types.i32();

    // fn sum4(a: i32, b: i32, c: i32, d: i32) -> i32 { a + b + c + d }
    let mut sum_body = Body::with_args(
        i32_ty,
        &[
            (i32_ty, false),
            (i32_ty, false),
            (i32_ty, false),
            (i32_ty, false),
        ],
    );
    let _temp1 = sum_body.alloc_local(LocalDecl::new(i32_ty, true)); // _5
    let _temp2 = sum_body.alloc_local(LocalDecl::new(i32_ty, true)); // _6
    let sum_entry = sum_body.alloc_block();

    // _5 = _1 + _2
    sum_body
        .block_mut(sum_entry)
        .push_statement(Statement::assign(
            Place::from_local(Local(5)),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::copy_local(Local(1)),
                Operand::copy_local(Local(2)),
            ),
            0..0,
        ));
    // _6 = _5 + _3
    sum_body
        .block_mut(sum_entry)
        .push_statement(Statement::assign(
            Place::from_local(Local(6)),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::copy_local(Local(5)),
                Operand::copy_local(Local(3)),
            ),
            0..0,
        ));
    // _0 = _6 + _4
    sum_body
        .block_mut(sum_entry)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::copy_local(Local(6)),
                Operand::copy_local(Local(4)),
            ),
            0..0,
        ));
    sum_body
        .block_mut(sum_entry)
        .set_terminator(Terminator::return_(0..0));

    // fn caller() -> i32 { sum4(1, 2, 3, 4) }
    let mut caller_body = Body::new(i32_ty);
    let caller_entry = caller_body.alloc_block();
    let after_call = caller_body.alloc_block();

    caller_body
        .block_mut(caller_entry)
        .set_terminator(Terminator::new(
            TerminatorKind::Call {
                func: Operand::Constant(Constant::FnDef(DefId(1))),
                args: vec![
                    Operand::const_int(1),
                    Operand::const_int(2),
                    Operand::const_int(3),
                    Operand::const_int(4),
                ],
                destination: Place::from_local(Local::RETURN_PLACE),
                target: Some(after_call),
            },
            0..0,
        ));

    caller_body
        .block_mut(after_call)
        .set_terminator(Terminator::return_(0..0));

    let functions = [
        FunctionDef::new(DefId(1), "sum4", &sum_body),
        FunctionDef::new(DefId(2), "caller", &caller_body),
    ];
    let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

    let caller_ptr = module.get_function_ptr(DefId(2)).unwrap();
    let caller: fn() -> i32 = unsafe { mem::transmute(caller_ptr) };

    assert_eq!(caller(), 10); // 1 + 2 + 3 + 4 = 10
}

#[test]
fn lower_call_chain() {
    let types = TypeInterner::new();
    let i32_ty = types.i32();

    // fn a() -> i32 { 1 }
    let mut a_body = Body::new(i32_ty);
    let a_entry = a_body.alloc_block();
    a_body.block_mut(a_entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Use(Operand::const_int(1)),
        0..0,
    ));
    a_body
        .block_mut(a_entry)
        .set_terminator(Terminator::return_(0..0));

    // fn b() -> i32 { a() + 10 }
    let mut b_body = Body::new(i32_ty);
    let _b_temp = b_body.alloc_local(LocalDecl::new(i32_ty, true)); // _1 for a() result
    let b_entry = b_body.alloc_block();
    let b_after_call = b_body.alloc_block();

    b_body.block_mut(b_entry).set_terminator(Terminator::new(
        TerminatorKind::Call {
            func: Operand::Constant(Constant::FnDef(DefId(1))), // a
            args: vec![],
            destination: Place::from_local(Local(1)),
            target: Some(b_after_call),
        },
        0..0,
    ));

    b_body
        .block_mut(b_after_call)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::copy_local(Local(1)),
                Operand::const_int(10),
            ),
            0..0,
        ));
    b_body
        .block_mut(b_after_call)
        .set_terminator(Terminator::return_(0..0));

    // fn c() -> i32 { b() + 100 }
    let mut c_body = Body::new(i32_ty);
    let _c_temp = c_body.alloc_local(LocalDecl::new(i32_ty, true)); // _1 for b() result
    let c_entry = c_body.alloc_block();
    let c_after_call = c_body.alloc_block();

    c_body.block_mut(c_entry).set_terminator(Terminator::new(
        TerminatorKind::Call {
            func: Operand::Constant(Constant::FnDef(DefId(2))), // b
            args: vec![],
            destination: Place::from_local(Local(1)),
            target: Some(c_after_call),
        },
        0..0,
    ));

    c_body
        .block_mut(c_after_call)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::copy_local(Local(1)),
                Operand::const_int(100),
            ),
            0..0,
        ));
    c_body
        .block_mut(c_after_call)
        .set_terminator(Terminator::return_(0..0));

    let functions = [
        FunctionDef::new(DefId(1), "a", &a_body),
        FunctionDef::new(DefId(2), "b", &b_body),
        FunctionDef::new(DefId(3), "c", &c_body),
    ];
    let module = ModuleCompiler::compile(&functions, &types).expect("compilation failed");

    let c_ptr = module.get_function_ptr(DefId(3)).unwrap();
    let c: fn() -> i32 = unsafe { mem::transmute(c_ptr) };

    assert_eq!(c(), 111); // 1 + 10 + 100 = 111
}

#[test]
#[ignore = "references not yet supported"]
fn lower_rvalue_ref() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _local = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();

    // _0 = &_1
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Ref(BorrowKind::Shared, Place::from_local(Local(1))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "rvalue_ref");
}

#[test]
#[ignore = "address_of not yet supported"]
fn lower_rvalue_address_of() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _local = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();

    // _0 = &raw const _1
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::AddressOf(Mutability::Shared, Place::from_local(Local(1))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "rvalue_address_of");
}

#[test]
fn lower_rvalue_len() {
    // fn() -> usize { let arr: [i32; 5]; len(arr) } // returns 5
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let i64_ty = runner.types_mut().i64(); // usize on 64-bit
    let array_ty = runner.types_mut().mk_array(i32_ty, 5);

    let mut body = Body::new(i64_ty); // Return usize
    let _array_local = body.alloc_local(LocalDecl::new(array_ty, true));
    let entry = body.alloc_block();

    // _0 = len(_1)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Len(Place::from_local(Local(1))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "rvalue_len");
    let func: fn() -> i64 = unsafe { mem::transmute(ptr) };
    assert_eq!(func(), 5);
}

#[test]
#[ignore = "aggregates not yet supported"]
fn lower_rvalue_aggregate_tuple() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // _0 = (1, 2) - tuple aggregate
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Aggregate(
            AggregateKind::Tuple,
            vec![
                Operand::Constant(Constant::Int(1)),
                Operand::Constant(Constant::Int(2)),
            ],
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "rvalue_aggregate_tuple");
}

#[test]
#[ignore = "aggregates not yet supported"]
fn lower_rvalue_aggregate_array() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // _0 = [1, 2, 3] - array aggregate
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Aggregate(
            AggregateKind::Array,
            vec![
                Operand::Constant(Constant::Int(1)),
                Operand::Constant(Constant::Int(2)),
                Operand::Constant(Constant::Int(3)),
            ],
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "rvalue_aggregate_array");
}

#[test]
#[ignore = "discriminant not yet supported"]
fn lower_rvalue_discriminant() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _enum_local = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();

    // _0 = discriminant(_1)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Discriminant(Place::from_local(Local(1))),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "rvalue_discriminant");
}

#[test]
#[ignore = "repeat not yet supported"]
fn lower_rvalue_repeat() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let entry = body.alloc_block();

    // _0 = [0; 10] - repeat expression
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Repeat(Operand::Constant(Constant::Int(0)), 10),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "rvalue_repeat");
}

#[test]
#[ignore = "unsize casts not yet supported"]
fn lower_cast_unsize() {
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let entry = body.alloc_block();

    // Unsize cast (e.g., [T; N] to [T])
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::Cast(
            CastKind::Unsize,
            Operand::Copy(Place::from_local(Local(1))),
            i32_ty,
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "cast_unsize");
}

#[test]
#[ignore = "float remainder not supported"]
fn lower_float_rem() {
    let mut runner = JitTestRunner::new();
    let f64_ty = runner.types_mut().f64();

    let mut body = Body::with_args(f64_ty, &[(f64_ty, false), (f64_ty, false)]);
    let entry = body.alloc_block();

    // _0 = _1 % _2 (float remainder)
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local::RETURN_PLACE),
        Rvalue::BinaryOp(
            BinOp::Rem,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));

    body.block_mut(entry)
        .set_terminator(Terminator::return_(0..0));

    let _ptr = runner.compile(&body, "float_rem");
}

// =============================================================================
// Assert Terminator with Actual Branching and Trapping
// =============================================================================

#[test]
fn assert_succeeds_when_true() {
    // fn() -> i32 { assert(true, expected=true); 42 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::new(i32_ty);
    let _cond_local = body.alloc_local(LocalDecl::new(bool_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _1 = true; assert(_1, expected=true) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Bool(true))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: true,
            target: after_assert,
        },
        0..0,
    ));

    // after_assert: _0 = 42; return
    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(42))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_succeeds_when_true");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

#[test]
#[ignore = "trap tests require signal handling; tested via runtime module"]
fn assert_traps_when_condition_fails() {
    // fn() -> i32 { assert(false, expected=true); 42 }
    // Should trap before returning
    //
    // Note: This test is ignored because CPU traps (UD2 instruction) generate
    // SIGILL signals which can't be caught by std::panic::catch_unwind.
    // Trap handling is tested via the runtime module's run_main().
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::new(i32_ty);
    let _cond_local = body.alloc_local(LocalDecl::new(bool_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _1 = false; assert(_1, expected=true) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Bool(false))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: true,
            target: after_assert,
        },
        0..0,
    ));

    // after_assert: _0 = 42; return (should not reach here)
    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(42))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_traps_when_condition_fails");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    // This should trap - the test is ignored because catch_unwind can't catch signals
    let result = std::panic::catch_unwind(func);
    assert!(result.is_err(), "assert(false, expected=true) should trap");
}

#[test]
fn assert_expected_false_succeeds() {
    // fn() -> i32 { assert(false, expected=false); 42 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::new(i32_ty);
    let _cond_local = body.alloc_local(LocalDecl::new(bool_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _1 = false; assert(_1, expected=false) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Bool(false))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: false,
            target: after_assert,
        },
        0..0,
    ));

    // after_assert: _0 = 42; return
    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(42))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_expected_false_succeeds");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

#[test]
#[ignore = "trap tests require signal handling; tested via runtime module"]
fn assert_expected_false_traps() {
    // fn() -> i32 { assert(true, expected=false); 42 }
    // Should trap because condition is true but expected is false
    //
    // Note: This test is ignored because CPU traps (UD2 instruction) generate
    // SIGILL signals which can't be caught by std::panic::catch_unwind.
    // Trap handling is tested via the runtime module's run_main().
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::new(i32_ty);
    let _cond_local = body.alloc_local(LocalDecl::new(bool_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _1 = true; assert(_1, expected=false) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Bool(true))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: false,
            target: after_assert,
        },
        0..0,
    ));

    // after_assert: _0 = 42; return (should not reach here)
    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(42))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_expected_false_traps");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    // This should trap - the test is ignored because catch_unwind can't catch signals
    let result = std::panic::catch_unwind(func);
    assert!(result.is_err(), "assert(true, expected=false) should trap");
}

#[test]
fn assert_with_integer_condition_nonzero() {
    // fn() -> i32 { let x: i32 = 42; assert(x, expected=true); 100 }
    // Non-zero integer should be truthy
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _cond_local = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _1 = 42; assert(_1, expected=true) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Int(42))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: true,
            target: after_assert,
        },
        0..0,
    ));

    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(100))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_with_integer_condition_nonzero");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 100);
}

#[test]
fn assert_with_integer_condition_zero() {
    // fn() -> i32 { let x: i32 = 0; assert(x, expected=false); 100 }
    // Zero integer with expected=false should pass
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _cond_local = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _1 = 0; assert(_1, expected=false) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Int(0))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: false,
            target: after_assert,
        },
        0..0,
    ));

    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(100))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_with_integer_condition_zero");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 100);
}

#[test]
fn assert_with_negative_integer() {
    // fn() -> i32 { let x: i32 = -1; assert(x, expected=true); 100 }
    // Negative integers are non-zero, so truthy
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();

    let mut body = Body::new(i32_ty);
    let _cond_local = body.alloc_local(LocalDecl::new(i32_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Int(-1))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: true,
            target: after_assert,
        },
        0..0,
    ));

    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(100))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_with_negative_integer");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 100);
}

#[test]
fn multiple_asserts_in_sequence_all_pass() {
    // fn() -> i32 { assert(true, expected=true); assert(false, expected=false); 42 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::new(i32_ty);
    let _cond1 = body.alloc_local(LocalDecl::new(bool_ty, true));
    let _cond2 = body.alloc_local(LocalDecl::new(bool_ty, true));
    let entry = body.alloc_block();
    let after_first = body.alloc_block();
    let after_second = body.alloc_block();

    // entry: _1 = true; assert(_1, expected=true) -> after_first
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(1)),
        Rvalue::Use(Operand::Constant(Constant::Bool(true))),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(1))),
            expected: true,
            target: after_first,
        },
        0..0,
    ));

    // after_first: _2 = false; assert(_2, expected=false) -> after_second
    body.block_mut(after_first)
        .push_statement(Statement::assign(
            Place::from_local(Local(2)),
            Rvalue::Use(Operand::Constant(Constant::Bool(false))),
            0..0,
        ));
    body.block_mut(after_first).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(2))),
            expected: false,
            target: after_second,
        },
        0..0,
    ));

    // after_second: _0 = 42; return
    body.block_mut(after_second)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(42))),
            0..0,
        ));
    body.block_mut(after_second)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "multiple_asserts_in_sequence_all_pass");
    let func: fn() -> i32 = unsafe { mem::transmute(ptr) };

    assert_eq!(func(), 42);
}

#[test]
fn assert_with_computed_condition() {
    // fn(x: i32) -> i32 { assert(x > 0, expected=true); 42 }
    // Test with argument-derived condition
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false)]);
    let _cmp_result = body.alloc_local(LocalDecl::new(bool_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _2 = _1 > 0; assert(_2, expected=true) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(2)),
        Rvalue::BinaryOp(
            BinOp::Gt,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Constant(Constant::Int(0)),
        ),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(2))),
            expected: true,
            target: after_assert,
        },
        0..0,
    ));

    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(42))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_with_computed_condition");
    let func: fn(i32) -> i32 = unsafe { mem::transmute(ptr) };

    // Positive values should pass
    assert_eq!(func(1), 42);
    assert_eq!(func(100), 42);
}

#[test]
fn assert_with_bool_from_comparison() {
    // fn(a: i32, b: i32) -> i32 { assert(a == b, expected=true); 1 }
    let mut runner = JitTestRunner::new();
    let i32_ty = runner.types_mut().i32();
    let bool_ty = runner.types_mut().bool();

    let mut body = Body::with_args(i32_ty, &[(i32_ty, false), (i32_ty, false)]);
    let _cmp_result = body.alloc_local(LocalDecl::new(bool_ty, true));
    let entry = body.alloc_block();
    let after_assert = body.alloc_block();

    // entry: _3 = _1 == _2; assert(_3, expected=true) -> after_assert
    body.block_mut(entry).push_statement(Statement::assign(
        Place::from_local(Local(3)),
        Rvalue::BinaryOp(
            BinOp::Eq,
            Operand::Copy(Place::from_local(Local(1))),
            Operand::Copy(Place::from_local(Local(2))),
        ),
        0..0,
    ));
    body.block_mut(entry).set_terminator(Terminator::new(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::from_local(Local(3))),
            expected: true,
            target: after_assert,
        },
        0..0,
    ));

    body.block_mut(after_assert)
        .push_statement(Statement::assign(
            Place::from_local(Local::RETURN_PLACE),
            Rvalue::Use(Operand::Constant(Constant::Int(1))),
            0..0,
        ));
    body.block_mut(after_assert)
        .set_terminator(Terminator::return_(0..0));

    let ptr = runner.compile(&body, "assert_with_bool_from_comparison");
    let func: fn(i32, i32) -> i32 = unsafe { mem::transmute(ptr) };

    // Equal values should pass
    assert_eq!(func(5, 5), 1);
    assert_eq!(func(0, 0), 1);
    assert_eq!(func(-10, -10), 1);
}
