//! Math intrinsic functions.
//!
//! Most math intrinsics are emitted as inline Cranelift IR for better performance.
//! Only `pow` requires a function call since Cranelift lacks a native instruction.

use cranelift_codegen::ir::{InstBuilder, Value, types};
use cranelift_frontend::FunctionBuilder;

use super::{Runtime, default_call_conv, make_signature};

/// Register math intrinsics that require function calls.
///
/// Most math intrinsics are inlined directly. Only `__pow` needs registration
/// since Cranelift doesn't have a native power instruction.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __pow: (F64, F64) -> F64 - needs function call (no native instruction)
    runtime.register(
        "__pow",
        __pow as *const u8,
        make_signature(call_conv, &[types::F64, types::F64], &[types::F64]),
    );
}

/// Power function: base^exp.
///
/// This is the only math intrinsic that requires a function call because
/// Cranelift doesn't have a native power instruction.
pub extern "C" fn __pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

// ============================================================================
// Inline IR emission functions
// ============================================================================
//
// These functions are public APIs for use by the codegen module when it
// encounters calls to math intrinsics. They emit inline Cranelift IR instead
// of function calls for better performance.

/// Emit inline IR for integer absolute value.
///
/// Uses Cranelift's native `iabs` instruction.
#[allow(dead_code)]
pub fn emit_abs_int(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().iabs(x)
}

/// Emit inline IR for float absolute value.
///
/// Uses Cranelift's native `fabs` instruction.
#[allow(dead_code)]
pub fn emit_abs_float(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().fabs(x)
}

/// Emit inline IR for signed integer minimum.
///
/// Uses Cranelift's native `smin` instruction.
#[allow(dead_code)]
pub fn emit_min_int(builder: &mut FunctionBuilder, a: Value, b: Value) -> Value {
    builder.ins().smin(a, b)
}

/// Emit inline IR for signed integer maximum.
///
/// Uses Cranelift's native `smax` instruction.
#[allow(dead_code)]
pub fn emit_max_int(builder: &mut FunctionBuilder, a: Value, b: Value) -> Value {
    builder.ins().smax(a, b)
}

/// Emit inline IR for floating-point square root.
///
/// Uses Cranelift's native `sqrt` instruction.
#[allow(dead_code)]
pub fn emit_sqrt(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().sqrt(x)
}

// ============================================================================
// Intrinsic lookup for codegen
// ============================================================================

/// Math intrinsics that can be emitted inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InlineMathIntrinsic {
    AbsInt,
    AbsFloat,
    MinInt,
    MaxInt,
    Sqrt,
}

#[allow(dead_code)]
impl InlineMathIntrinsic {
    /// Try to look up an inline math intrinsic by name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "__abs_int" => Some(Self::AbsInt),
            "__abs_float" => Some(Self::AbsFloat),
            "__min_int" => Some(Self::MinInt),
            "__max_int" => Some(Self::MaxInt),
            "__sqrt" => Some(Self::Sqrt),
            _ => None,
        }
    }

    /// Emit the inline IR for this intrinsic.
    ///
    /// # Panics
    ///
    /// Panics if the wrong number of arguments is provided.
    pub fn emit(self, builder: &mut FunctionBuilder, args: &[Value]) -> Value {
        match self {
            Self::AbsInt => {
                assert_eq!(args.len(), 1, "__abs_int requires 1 argument");
                emit_abs_int(builder, args[0])
            }
            Self::AbsFloat => {
                assert_eq!(args.len(), 1, "__abs_float requires 1 argument");
                emit_abs_float(builder, args[0])
            }
            Self::MinInt => {
                assert_eq!(args.len(), 2, "__min_int requires 2 arguments");
                emit_min_int(builder, args[0], args[1])
            }
            Self::MaxInt => {
                assert_eq!(args.len(), 2, "__max_int requires 2 arguments");
                emit_max_int(builder, args[0], args[1])
            }
            Self::Sqrt => {
                assert_eq!(args.len(), 1, "__sqrt requires 1 argument");
                emit_sqrt(builder, args[0])
            }
        }
    }

    /// Get the number of arguments this intrinsic expects.
    pub fn arg_count(self) -> usize {
        match self {
            Self::AbsInt | Self::AbsFloat | Self::Sqrt => 1,
            Self::MinInt | Self::MaxInt => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::context::CodegenContext;
    use cranelift_codegen::ir::AbiParam;
    use std::mem;

    // ==================== Direct call tests (for __pow only) ====================

    #[test]
    fn pow_integer_exponent() {
        assert_eq!(__pow(2.0, 3.0), 8.0);
        assert_eq!(__pow(10.0, 2.0), 100.0);
    }

    #[test]
    fn pow_fractional_exponent() {
        let result = __pow(4.0, 0.5);
        assert!((result - 2.0).abs() < 1e-10);
    }

    #[test]
    fn pow_zero_exponent() {
        assert_eq!(__pow(5.0, 0.0), 1.0);
    }

    #[test]
    fn pow_one_exponent() {
        assert_eq!(__pow(5.0, 1.0), 5.0);
    }

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_pow_intrinsic() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        // Only __pow should be registered (others are inlined)
        assert!(runtime.contains("__pow"));
        assert!(!runtime.contains("__abs_int")); // Should NOT be registered
        assert!(!runtime.contains("__sqrt")); // Should NOT be registered
    }

    #[test]
    fn pow_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__pow").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::F64);
        assert_eq!(func.signature.params[1].value_type, types::F64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::F64);
    }

    // ==================== Intrinsic lookup tests ====================

    #[test]
    fn inline_intrinsic_from_name() {
        assert_eq!(InlineMathIntrinsic::from_name("__abs_int"), Some(InlineMathIntrinsic::AbsInt));
        assert_eq!(InlineMathIntrinsic::from_name("__abs_float"), Some(InlineMathIntrinsic::AbsFloat));
        assert_eq!(InlineMathIntrinsic::from_name("__min_int"), Some(InlineMathIntrinsic::MinInt));
        assert_eq!(InlineMathIntrinsic::from_name("__max_int"), Some(InlineMathIntrinsic::MaxInt));
        assert_eq!(InlineMathIntrinsic::from_name("__sqrt"), Some(InlineMathIntrinsic::Sqrt));
        assert_eq!(InlineMathIntrinsic::from_name("__pow"), None); // pow is NOT inline
        assert_eq!(InlineMathIntrinsic::from_name("unknown"), None);
    }

    #[test]
    fn inline_intrinsic_arg_count() {
        assert_eq!(InlineMathIntrinsic::AbsInt.arg_count(), 1);
        assert_eq!(InlineMathIntrinsic::AbsFloat.arg_count(), 1);
        assert_eq!(InlineMathIntrinsic::Sqrt.arg_count(), 1);
        assert_eq!(InlineMathIntrinsic::MinInt.arg_count(), 2);
        assert_eq!(InlineMathIntrinsic::MaxInt.arg_count(), 2);
    }

    // ==================== JIT integration tests ====================

    #[test]
    fn jit_inline_abs_int() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        // fn test(x: i64) -> i64 { abs(x) }
        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        let func_id = ctx.declare_function("test", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let x = builder.block_params(entry)[0];
            let result = emit_abs_int(&mut builder, x);

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let test: fn(i64) -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(42), 42);
        assert_eq!(test(-42), 42);
        assert_eq!(test(0), 0);
    }

    #[test]
    fn jit_inline_abs_float() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::F64));
        sig.returns.push(AbiParam::new(types::F64));
        let func_id = ctx.declare_function("test", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let x = builder.block_params(entry)[0];
            let result = emit_abs_float(&mut builder, x);

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let test: fn(f64) -> f64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(2.5), 2.5);
        assert_eq!(test(-2.5), 2.5);
    }

    #[test]
    fn jit_inline_min_int() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        let func_id = ctx.declare_function("test", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let a = builder.block_params(entry)[0];
            let b = builder.block_params(entry)[1];
            let result = emit_min_int(&mut builder, a, b);

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let test: fn(i64, i64) -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(10, 5), 5);
        assert_eq!(test(5, 10), 5);
        assert_eq!(test(7, 7), 7);
        assert_eq!(test(-10, -5), -10);
    }

    #[test]
    fn jit_inline_max_int() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        let func_id = ctx.declare_function("test", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let a = builder.block_params(entry)[0];
            let b = builder.block_params(entry)[1];
            let result = emit_max_int(&mut builder, a, b);

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let test: fn(i64, i64) -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(10, 5), 10);
        assert_eq!(test(5, 10), 10);
        assert_eq!(test(7, 7), 7);
        assert_eq!(test(-10, -5), -5);
    }

    #[test]
    fn jit_inline_sqrt() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::F64));
        sig.returns.push(AbiParam::new(types::F64));
        let func_id = ctx.declare_function("test", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let x = builder.block_params(entry)[0];
            let result = emit_sqrt(&mut builder, x);

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let test: fn(f64) -> f64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(4.0), 2.0);
        assert_eq!(test(9.0), 3.0);
        assert_eq!(test(16.0), 4.0);
    }

    #[test]
    fn jit_inline_via_enum() {
        let mut ctx = CodegenContext::new_jit().unwrap();

        // Test using the InlineMathIntrinsic enum API
        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        let func_id = ctx.declare_function("test", &sig).unwrap();

        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let x = builder.block_params(entry)[0];

            // Look up and emit via the enum API
            let intrinsic = InlineMathIntrinsic::from_name("__abs_int").unwrap();
            let result = intrinsic.emit(&mut builder, &[x]);

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let test: fn(i64) -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(-42), 42);
    }
}
