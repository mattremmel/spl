//! Math intrinsic functions.
//!
//! Most math intrinsics are emitted as inline Cranelift IR for better performance.
//! Trigonometric and logarithmic functions require function calls to libm.

use cranelift_codegen::ir::{InstBuilder, Value, types};
use cranelift_frontend::FunctionBuilder;

use super::{Runtime, default_call_conv, make_signature};

/// Register math intrinsics that require function calls.
///
/// Most math intrinsics are inlined directly. These need function calls
/// since Cranelift doesn't have native instructions for them.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // Power and exponential
    runtime.register(
        "__pow",
        __pow as *const u8,
        make_signature(call_conv, &[types::F64, types::F64], &[types::F64]),
    );
    runtime.register(
        "__exp",
        __exp as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__log",
        __log as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__log10",
        __log10 as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__log2",
        __log2 as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );

    // Trigonometric
    runtime.register(
        "__sin",
        __sin as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__cos",
        __cos as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__tan",
        __tan as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__asin",
        __asin as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__acos",
        __acos as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__atan",
        __atan as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );
    runtime.register(
        "__atan2",
        __atan2 as *const u8,
        make_signature(call_conv, &[types::F64, types::F64], &[types::F64]),
    );
}

// ============================================================================
// Function call intrinsics (no native Cranelift instruction)
// ============================================================================

/// Power function: base^exp.
pub extern "C" fn __pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

/// Exponential function: e^x.
pub extern "C" fn __exp(x: f64) -> f64 {
    x.exp()
}

/// Natural logarithm: ln(x).
pub extern "C" fn __log(x: f64) -> f64 {
    x.ln()
}

/// Base-10 logarithm: log10(x).
pub extern "C" fn __log10(x: f64) -> f64 {
    x.log10()
}

/// Base-2 logarithm: log2(x).
pub extern "C" fn __log2(x: f64) -> f64 {
    x.log2()
}

/// Sine function.
pub extern "C" fn __sin(x: f64) -> f64 {
    x.sin()
}

/// Cosine function.
pub extern "C" fn __cos(x: f64) -> f64 {
    x.cos()
}

/// Tangent function.
pub extern "C" fn __tan(x: f64) -> f64 {
    x.tan()
}

/// Arc sine function.
pub extern "C" fn __asin(x: f64) -> f64 {
    x.asin()
}

/// Arc cosine function.
pub extern "C" fn __acos(x: f64) -> f64 {
    x.acos()
}

/// Arc tangent function.
pub extern "C" fn __atan(x: f64) -> f64 {
    x.atan()
}

/// Two-argument arc tangent: atan2(y, x).
pub extern "C" fn __atan2(y: f64, x: f64) -> f64 {
    y.atan2(x)
}

// ============================================================================
// Inline IR emission functions
// ============================================================================
//
// These functions emit native Cranelift instructions for better performance.

// --- Absolute value ---

/// Emit inline IR for integer absolute value.
#[allow(dead_code)]
pub fn emit_abs_int(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().iabs(x)
}

/// Emit inline IR for float absolute value.
#[allow(dead_code)]
pub fn emit_abs_float(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().fabs(x)
}

// --- Min/max ---

/// Emit inline IR for signed integer minimum.
#[allow(dead_code)]
pub fn emit_min_int(builder: &mut FunctionBuilder, a: Value, b: Value) -> Value {
    builder.ins().smin(a, b)
}

/// Emit inline IR for signed integer maximum.
#[allow(dead_code)]
pub fn emit_max_int(builder: &mut FunctionBuilder, a: Value, b: Value) -> Value {
    builder.ins().smax(a, b)
}

/// Emit inline IR for floating-point minimum.
#[allow(dead_code)]
pub fn emit_min_float(builder: &mut FunctionBuilder, a: Value, b: Value) -> Value {
    builder.ins().fmin(a, b)
}

/// Emit inline IR for floating-point maximum.
#[allow(dead_code)]
pub fn emit_max_float(builder: &mut FunctionBuilder, a: Value, b: Value) -> Value {
    builder.ins().fmax(a, b)
}

// --- Square root ---

/// Emit inline IR for floating-point square root.
#[allow(dead_code)]
pub fn emit_sqrt(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().sqrt(x)
}

// --- Rounding ---

/// Emit inline IR for floor (round toward negative infinity).
#[allow(dead_code)]
pub fn emit_floor(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().floor(x)
}

/// Emit inline IR for ceil (round toward positive infinity).
#[allow(dead_code)]
pub fn emit_ceil(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().ceil(x)
}

/// Emit inline IR for trunc (round toward zero).
#[allow(dead_code)]
pub fn emit_trunc(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().trunc(x)
}

/// Emit inline IR for nearest (round to nearest, ties to even).
#[allow(dead_code)]
pub fn emit_round(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().nearest(x)
}

// --- Floating-point manipulation ---

/// Emit inline IR for copysign (copy sign of second arg to first).
#[allow(dead_code)]
pub fn emit_copysign(builder: &mut FunctionBuilder, mag: Value, sign: Value) -> Value {
    builder.ins().fcopysign(mag, sign)
}

/// Emit inline IR for fused multiply-add: a * b + c.
#[allow(dead_code)]
pub fn emit_fma(builder: &mut FunctionBuilder, a: Value, b: Value, c: Value) -> Value {
    builder.ins().fma(a, b, c)
}

// --- Bit manipulation ---

/// Emit inline IR for count leading zeros.
#[allow(dead_code)]
pub fn emit_clz(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().clz(x)
}

/// Emit inline IR for count trailing zeros.
#[allow(dead_code)]
pub fn emit_ctz(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().ctz(x)
}

/// Emit inline IR for population count (count set bits).
#[allow(dead_code)]
pub fn emit_popcnt(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().popcnt(x)
}

/// Emit inline IR for bitwise reverse.
#[allow(dead_code)]
pub fn emit_bitrev(builder: &mut FunctionBuilder, x: Value) -> Value {
    builder.ins().bitrev(x)
}

/// Emit inline IR for rotate left.
#[allow(dead_code)]
pub fn emit_rotl(builder: &mut FunctionBuilder, x: Value, amount: Value) -> Value {
    builder.ins().rotl(x, amount)
}

/// Emit inline IR for rotate right.
#[allow(dead_code)]
pub fn emit_rotr(builder: &mut FunctionBuilder, x: Value, amount: Value) -> Value {
    builder.ins().rotr(x, amount)
}

// ============================================================================
// Intrinsic lookup for codegen
// ============================================================================

/// Math intrinsics that can be emitted inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InlineMathIntrinsic {
    // Absolute value
    AbsInt,
    AbsFloat,
    // Min/max
    MinInt,
    MaxInt,
    MinFloat,
    MaxFloat,
    // Square root
    Sqrt,
    // Rounding
    Floor,
    Ceil,
    Trunc,
    Round,
    // Float manipulation
    Copysign,
    Fma,
    // Bit manipulation
    Clz,
    Ctz,
    Popcnt,
    Bitrev,
    Rotl,
    Rotr,
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
            "__min_float" => Some(Self::MinFloat),
            "__max_float" => Some(Self::MaxFloat),
            "__sqrt" => Some(Self::Sqrt),
            "__floor" => Some(Self::Floor),
            "__ceil" => Some(Self::Ceil),
            "__trunc" => Some(Self::Trunc),
            "__round" => Some(Self::Round),
            "__copysign" => Some(Self::Copysign),
            "__fma" => Some(Self::Fma),
            "__clz" => Some(Self::Clz),
            "__ctz" => Some(Self::Ctz),
            "__popcnt" => Some(Self::Popcnt),
            "__bitrev" => Some(Self::Bitrev),
            "__rotl" => Some(Self::Rotl),
            "__rotr" => Some(Self::Rotr),
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
                assert_eq!(args.len(), 1);
                emit_abs_int(builder, args[0])
            }
            Self::AbsFloat => {
                assert_eq!(args.len(), 1);
                emit_abs_float(builder, args[0])
            }
            Self::MinInt => {
                assert_eq!(args.len(), 2);
                emit_min_int(builder, args[0], args[1])
            }
            Self::MaxInt => {
                assert_eq!(args.len(), 2);
                emit_max_int(builder, args[0], args[1])
            }
            Self::MinFloat => {
                assert_eq!(args.len(), 2);
                emit_min_float(builder, args[0], args[1])
            }
            Self::MaxFloat => {
                assert_eq!(args.len(), 2);
                emit_max_float(builder, args[0], args[1])
            }
            Self::Sqrt => {
                assert_eq!(args.len(), 1);
                emit_sqrt(builder, args[0])
            }
            Self::Floor => {
                assert_eq!(args.len(), 1);
                emit_floor(builder, args[0])
            }
            Self::Ceil => {
                assert_eq!(args.len(), 1);
                emit_ceil(builder, args[0])
            }
            Self::Trunc => {
                assert_eq!(args.len(), 1);
                emit_trunc(builder, args[0])
            }
            Self::Round => {
                assert_eq!(args.len(), 1);
                emit_round(builder, args[0])
            }
            Self::Copysign => {
                assert_eq!(args.len(), 2);
                emit_copysign(builder, args[0], args[1])
            }
            Self::Fma => {
                assert_eq!(args.len(), 3);
                emit_fma(builder, args[0], args[1], args[2])
            }
            Self::Clz => {
                assert_eq!(args.len(), 1);
                emit_clz(builder, args[0])
            }
            Self::Ctz => {
                assert_eq!(args.len(), 1);
                emit_ctz(builder, args[0])
            }
            Self::Popcnt => {
                assert_eq!(args.len(), 1);
                emit_popcnt(builder, args[0])
            }
            Self::Bitrev => {
                assert_eq!(args.len(), 1);
                emit_bitrev(builder, args[0])
            }
            Self::Rotl => {
                assert_eq!(args.len(), 2);
                emit_rotl(builder, args[0], args[1])
            }
            Self::Rotr => {
                assert_eq!(args.len(), 2);
                emit_rotr(builder, args[0], args[1])
            }
        }
    }

    /// Get the number of arguments this intrinsic expects.
    pub fn arg_count(self) -> usize {
        match self {
            Self::AbsInt | Self::AbsFloat | Self::Sqrt
            | Self::Floor | Self::Ceil | Self::Trunc | Self::Round
            | Self::Clz | Self::Ctz | Self::Popcnt | Self::Bitrev => 1,
            Self::MinInt | Self::MaxInt | Self::MinFloat | Self::MaxFloat
            | Self::Copysign | Self::Rotl | Self::Rotr => 2,
            Self::Fma => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::context::CodegenContext;
    use cranelift_codegen::ir::AbiParam;
    use std::f64::consts::PI;
    use std::mem;

    // ==================== Function call intrinsic tests ====================

    #[test]
    fn pow_works() {
        assert_eq!(__pow(2.0, 3.0), 8.0);
        assert_eq!(__pow(10.0, 2.0), 100.0);
        assert!(__pow(4.0, 0.5) - 2.0 < 1e-10);
    }

    #[test]
    fn exp_log_works() {
        assert!((__exp(1.0) - std::f64::consts::E).abs() < 1e-10);
        assert!((__log(std::f64::consts::E) - 1.0).abs() < 1e-10);
        assert!((__log10(100.0) - 2.0).abs() < 1e-10);
        assert!((__log2(8.0) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn trig_works() {
        assert!(__sin(0.0).abs() < 1e-10);
        assert!((__cos(0.0) - 1.0).abs() < 1e-10);
        assert!(__tan(0.0).abs() < 1e-10);
        assert!((__sin(PI / 2.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn inverse_trig_works() {
        assert!(__asin(0.0).abs() < 1e-10);
        assert!((__acos(1.0)).abs() < 1e-10);
        assert!(__atan(0.0).abs() < 1e-10);
        assert!((__atan2(1.0, 1.0) - PI / 4.0).abs() < 1e-10);
    }

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_function_call_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        // Function call intrinsics should be registered
        assert!(runtime.contains("__pow"));
        assert!(runtime.contains("__exp"));
        assert!(runtime.contains("__log"));
        assert!(runtime.contains("__sin"));
        assert!(runtime.contains("__cos"));
        assert!(runtime.contains("__tan"));
        assert!(runtime.contains("__atan2"));

        // Inline intrinsics should NOT be registered
        assert!(!runtime.contains("__abs_int"));
        assert!(!runtime.contains("__sqrt"));
        assert!(!runtime.contains("__floor"));
        assert!(!runtime.contains("__clz"));
    }

    // ==================== Intrinsic lookup tests ====================

    #[test]
    fn inline_intrinsic_from_name_comprehensive() {
        // All inline intrinsics should be found
        assert!(InlineMathIntrinsic::from_name("__abs_int").is_some());
        assert!(InlineMathIntrinsic::from_name("__abs_float").is_some());
        assert!(InlineMathIntrinsic::from_name("__min_int").is_some());
        assert!(InlineMathIntrinsic::from_name("__max_int").is_some());
        assert!(InlineMathIntrinsic::from_name("__min_float").is_some());
        assert!(InlineMathIntrinsic::from_name("__max_float").is_some());
        assert!(InlineMathIntrinsic::from_name("__sqrt").is_some());
        assert!(InlineMathIntrinsic::from_name("__floor").is_some());
        assert!(InlineMathIntrinsic::from_name("__ceil").is_some());
        assert!(InlineMathIntrinsic::from_name("__trunc").is_some());
        assert!(InlineMathIntrinsic::from_name("__round").is_some());
        assert!(InlineMathIntrinsic::from_name("__copysign").is_some());
        assert!(InlineMathIntrinsic::from_name("__fma").is_some());
        assert!(InlineMathIntrinsic::from_name("__clz").is_some());
        assert!(InlineMathIntrinsic::from_name("__ctz").is_some());
        assert!(InlineMathIntrinsic::from_name("__popcnt").is_some());
        assert!(InlineMathIntrinsic::from_name("__bitrev").is_some());
        assert!(InlineMathIntrinsic::from_name("__rotl").is_some());
        assert!(InlineMathIntrinsic::from_name("__rotr").is_some());

        // Function call intrinsics should NOT be found here
        assert!(InlineMathIntrinsic::from_name("__pow").is_none());
        assert!(InlineMathIntrinsic::from_name("__sin").is_none());
    }

    // ==================== JIT integration tests ====================

    #[test]
    fn jit_inline_abs_int() {
        let mut ctx = CodegenContext::new_jit().unwrap();
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
    fn jit_inline_floor_ceil() {
        let mut ctx = CodegenContext::new_jit().unwrap();
        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::F64));
        sig.returns.push(AbiParam::new(types::F64));
        let func_id = ctx.declare_function("test_floor", &sig).unwrap();
        ctx.compilation_context().func.signature = sig;

        {
            let (func, func_ctx) = ctx.builder_context();
            let mut builder = FunctionBuilder::new(func, func_ctx);
            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let x = builder.block_params(entry)[0];
            let result = emit_floor(&mut builder, x);
            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let floor: fn(f64) -> f64 = unsafe { mem::transmute(ptr) };

        assert_eq!(floor(2.7), 2.0);
        assert_eq!(floor(-2.7), -3.0);
        assert_eq!(floor(3.0), 3.0);
    }

    #[test]
    fn jit_inline_clz() {
        let mut ctx = CodegenContext::new_jit().unwrap();
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
            let result = emit_clz(&mut builder, x);
            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let clz: fn(i64) -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(clz(1), 63);  // 0b000...001
        assert_eq!(clz(2), 62);  // 0b000...010
        assert_eq!(clz(-1), 0);  // All bits set
    }

    #[test]
    fn jit_inline_popcnt() {
        let mut ctx = CodegenContext::new_jit().unwrap();
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
            let result = emit_popcnt(&mut builder, x);
            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let popcnt: fn(i64) -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(popcnt(0), 0);
        assert_eq!(popcnt(1), 1);
        assert_eq!(popcnt(0b1111), 4);
        assert_eq!(popcnt(-1), 64);  // All bits set
    }

    #[test]
    fn jit_inline_fma() {
        let mut ctx = CodegenContext::new_jit().unwrap();
        let mut sig = ctx.new_signature();
        sig.params.push(AbiParam::new(types::F64));
        sig.params.push(AbiParam::new(types::F64));
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

            let a = builder.block_params(entry)[0];
            let b = builder.block_params(entry)[1];
            let c = builder.block_params(entry)[2];
            let result = emit_fma(&mut builder, a, b, c);
            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(func_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(func_id);
        let fma: fn(f64, f64, f64) -> f64 = unsafe { mem::transmute(ptr) };

        // fma(a, b, c) = a * b + c
        assert_eq!(fma(2.0, 3.0, 4.0), 10.0);
        assert_eq!(fma(1.5, 2.0, 0.5), 3.5);
    }
}
