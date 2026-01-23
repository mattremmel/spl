//! Integration tests for MIR to CLIF lowering.
//!
//! These tests verify correct lowering by JIT compiling MIR and executing it.

use std::mem;

use crate::codegen::CodegenContext;
use crate::mir::body::{Body, LocalDecl};
use crate::mir::operand::{BinOp, CastKind, Constant, Operand, Rvalue, UnOp};
use crate::mir::statement::Statement;
use crate::mir::terminator::{SwitchTargets, Terminator, TerminatorKind};
use crate::mir::types::{Local, Place};
use crate::sema::types::PrimitiveKind;
use crate::sema::types::TypeInterner;

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
