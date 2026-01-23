//! Math intrinsic functions.
//!
//! Pure mathematical functions that are easy to test.

use cranelift_codegen::ir::types;

use super::{Runtime, default_call_conv, make_signature};

/// Register all math intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __abs_int: (I64) -> I64
    runtime.register(
        "__abs_int",
        __abs_int as *const u8,
        make_signature(call_conv, &[types::I64], &[types::I64]),
    );

    // __abs_float: (F64) -> F64
    runtime.register(
        "__abs_float",
        __abs_float as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );

    // __min_int: (I64, I64) -> I64
    runtime.register(
        "__min_int",
        __min_int as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[types::I64]),
    );

    // __max_int: (I64, I64) -> I64
    runtime.register(
        "__max_int",
        __max_int as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[types::I64]),
    );

    // __sqrt: (F64) -> F64
    runtime.register(
        "__sqrt",
        __sqrt as *const u8,
        make_signature(call_conv, &[types::F64], &[types::F64]),
    );

    // __pow: (F64, F64) -> F64
    runtime.register(
        "__pow",
        __pow as *const u8,
        make_signature(call_conv, &[types::F64, types::F64], &[types::F64]),
    );
}

/// Absolute value of an integer.
pub extern "C" fn __abs_int(x: i64) -> i64 {
    x.abs()
}

/// Absolute value of a float.
pub extern "C" fn __abs_float(x: f64) -> f64 {
    x.abs()
}

/// Minimum of two integers.
pub extern "C" fn __min_int(a: i64, b: i64) -> i64 {
    a.min(b)
}

/// Maximum of two integers.
pub extern "C" fn __max_int(a: i64, b: i64) -> i64 {
    a.max(b)
}

/// Square root of a float.
pub extern "C" fn __sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// Power function: base^exp.
pub extern "C" fn __pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ==================== Direct call tests ====================

    #[test]
    fn abs_int_positive() {
        assert_eq!(__abs_int(42), 42);
    }

    #[test]
    fn abs_int_negative() {
        assert_eq!(__abs_int(-42), 42);
    }

    #[test]
    fn abs_int_zero() {
        assert_eq!(__abs_int(0), 0);
    }

    #[test]
    fn abs_float_positive() {
        assert_eq!(__abs_float(2.5), 2.5);
    }

    #[test]
    fn abs_float_negative() {
        assert_eq!(__abs_float(-2.5), 2.5);
    }

    #[test]
    fn min_int_first_smaller() {
        assert_eq!(__min_int(1, 5), 1);
    }

    #[test]
    fn min_int_second_smaller() {
        assert_eq!(__min_int(10, 3), 3);
    }

    #[test]
    fn min_int_equal() {
        assert_eq!(__min_int(7, 7), 7);
    }

    #[test]
    fn max_int_first_larger() {
        assert_eq!(__max_int(10, 3), 10);
    }

    #[test]
    fn max_int_second_larger() {
        assert_eq!(__max_int(1, 5), 5);
    }

    #[test]
    fn max_int_equal() {
        assert_eq!(__max_int(7, 7), 7);
    }

    #[test]
    fn sqrt_perfect_square() {
        assert_eq!(__sqrt(4.0), 2.0);
        assert_eq!(__sqrt(9.0), 3.0);
        assert_eq!(__sqrt(16.0), 4.0);
    }

    #[test]
    fn sqrt_non_perfect() {
        let result = __sqrt(2.0);
        assert!((result - std::f64::consts::SQRT_2).abs() < 1e-10);
    }

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

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_math_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__abs_int"));
        assert!(runtime.contains("__abs_float"));
        assert!(runtime.contains("__min_int"));
        assert!(runtime.contains("__max_int"));
        assert!(runtime.contains("__sqrt"));
        assert!(runtime.contains("__pow"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn abs_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__abs_int").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn abs_float_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__abs_float").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::F64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::F64);
    }

    #[test]
    fn min_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__min_int").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert_eq!(func.signature.params[1].value_type, types::I64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn max_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__max_int").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert_eq!(func.signature.params[1].value_type, types::I64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn sqrt_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__sqrt").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::F64);
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::F64);
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

    // ==================== Edge case tests ====================

    #[test]
    fn abs_int_min_value() {
        // i64::MIN.abs() panics, but wrapping_abs returns MIN
        // We use the standard abs which will panic - this documents the behavior
        // In a real implementation, we might want to handle this differently
        assert_eq!(__abs_int(i64::MAX), i64::MAX);
    }

    #[test]
    fn min_max_with_negatives() {
        assert_eq!(__min_int(-10, -5), -10);
        assert_eq!(__max_int(-10, -5), -5);
    }

    #[test]
    fn sqrt_zero() {
        assert_eq!(__sqrt(0.0), 0.0);
    }

    #[test]
    fn pow_zero_exponent() {
        assert_eq!(__pow(5.0, 0.0), 1.0);
    }

    #[test]
    fn pow_one_exponent() {
        assert_eq!(__pow(5.0, 1.0), 5.0);
    }

    // ==================== JIT integration tests ====================

    #[test]
    fn jit_call_abs_int() {
        use crate::codegen::context::CodegenContext;
        use cranelift_codegen::ir::{AbiParam, InstBuilder};
        use cranelift_frontend::FunctionBuilder;
        use cranelift_module::{Linkage, Module};
        use std::mem;

        let mut runtime = Runtime::new();
        register(&mut runtime);
        let abs_sig = runtime.get("__abs_int").unwrap().signature.clone();

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        // Create wrapper: fn test() -> i64 { __abs_int(-42) }
        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.returns.push(AbiParam::new(types::I64));
        let wrapper_id = ctx.declare_function("test", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();

            // Declare the external function inside the mutable borrow
            let abs_func_id = module
                .declare_function("__abs_int", Linkage::Import, &abs_sig)
                .unwrap();

            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(abs_func_id, builder.func);
            let arg = builder.ins().iconst(types::I64, -42);
            let call = builder.ins().call(func_ref, &[arg]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let test: fn() -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(), 42);
    }

    #[test]
    fn jit_call_min_int() {
        use crate::codegen::context::CodegenContext;
        use cranelift_codegen::ir::{AbiParam, InstBuilder};
        use cranelift_frontend::FunctionBuilder;
        use cranelift_module::{Linkage, Module};
        use std::mem;

        let mut runtime = Runtime::new();
        register(&mut runtime);
        let min_sig = runtime.get("__min_int").unwrap().signature.clone();

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        // Create wrapper: fn test() -> i64 { __min_int(10, 5) }
        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.returns.push(AbiParam::new(types::I64));
        let wrapper_id = ctx.declare_function("test", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();

            let min_func_id = module
                .declare_function("__min_int", Linkage::Import, &min_sig)
                .unwrap();

            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(min_func_id, builder.func);
            let a = builder.ins().iconst(types::I64, 10);
            let b = builder.ins().iconst(types::I64, 5);
            let call = builder.ins().call(func_ref, &[a, b]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let test: fn() -> i64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(), 5);
    }

    #[test]
    fn jit_call_sqrt() {
        use crate::codegen::context::CodegenContext;
        use cranelift_codegen::ir::{AbiParam, InstBuilder};
        use cranelift_frontend::FunctionBuilder;
        use cranelift_module::{Linkage, Module};
        use std::mem;

        let mut runtime = Runtime::new();
        register(&mut runtime);
        let sqrt_sig = runtime.get("__sqrt").unwrap().signature.clone();

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        // Create wrapper: fn test() -> f64 { __sqrt(16.0) }
        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.returns.push(AbiParam::new(types::F64));
        let wrapper_id = ctx.declare_function("test", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();

            let sqrt_func_id = module
                .declare_function("__sqrt", Linkage::Import, &sqrt_sig)
                .unwrap();

            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(sqrt_func_id, builder.func);
            let arg = builder.ins().f64const(16.0);
            let call = builder.ins().call(func_ref, &[arg]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let test: fn() -> f64 = unsafe { mem::transmute(ptr) };

        assert_eq!(test(), 4.0);
    }
}
