//! String intrinsic functions (stubs).
//!
//! Functions for string operations. Currently implemented as stubs.

use cranelift_codegen::ir::types;

use super::convert::StringResult;
use super::{Runtime, default_call_conv, make_signature};

/// Register all string intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // __str_len: (*const u8, I64) -> I64
    runtime.register(
        "__str_len",
        __str_len as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[types::I64]),
    );

    // __str_concat: (*const u8, I64, *const u8, I64) -> (*const u8, I64)
    runtime.register(
        "__str_concat",
        __str_concat as *const u8,
        make_signature(
            call_conv,
            &[types::I64, types::I64, types::I64, types::I64],
            &[types::I64, types::I64],
        ),
    );
}

/// Get the length of a string.
///
/// Returns the provided length, or 0 if the pointer is null.
pub extern "C" fn __str_len(ptr: *const u8, len: i64) -> i64 {
    if ptr.is_null() {
        0
    } else {
        len
    }
}

/// Concatenate two strings (stub).
///
/// Returns (null, 0) as this is not yet implemented.
pub extern "C" fn __str_concat(
    _ptr1: *const u8,
    _len1: i64,
    _ptr2: *const u8,
    _len2: i64,
) -> StringResult {
    // Stub: return null pointer and zero length
    StringResult {
        ptr: std::ptr::null(),
        len: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ==================== Direct call tests ====================

    #[test]
    fn str_len_returns_length() {
        let s = "Hello";
        let len = __str_len(s.as_ptr(), s.len() as i64);
        assert_eq!(len, 5);
    }

    #[test]
    fn str_len_null_returns_zero() {
        let len = __str_len(std::ptr::null(), 10);
        assert_eq!(len, 0);
    }

    #[test]
    fn str_len_empty_string() {
        let s = "";
        let len = __str_len(s.as_ptr(), 0);
        assert_eq!(len, 0);
    }

    #[test]
    fn str_concat_returns_null() {
        let s1 = "Hello";
        let s2 = "World";
        let result = __str_concat(
            s1.as_ptr(),
            s1.len() as i64,
            s2.as_ptr(),
            s2.len() as i64,
        );
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_string_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        assert!(runtime.contains("__str_len"));
        assert!(runtime.contains("__str_concat"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn str_len_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__str_len").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I64);
    }

    #[test]
    fn str_concat_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__str_concat").unwrap();
        assert_eq!(func.signature.params.len(), 4);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr1
        assert_eq!(func.signature.params[1].value_type, types::I64); // len1
        assert_eq!(func.signature.params[2].value_type, types::I64); // ptr2
        assert_eq!(func.signature.params[3].value_type, types::I64); // len2
        assert_eq!(func.signature.returns.len(), 2);
        assert_eq!(func.signature.returns[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // len
    }

    // ==================== Edge case tests ====================

    #[test]
    fn str_len_with_negative_length() {
        let s = "Hello";
        // Even with negative length, we just return it
        let len = __str_len(s.as_ptr(), -5);
        assert_eq!(len, -5);
    }

    #[test]
    fn str_concat_with_nulls() {
        let result = __str_concat(std::ptr::null(), 0, std::ptr::null(), 0);
        assert!(result.ptr.is_null());
        assert_eq!(result.len, 0);
    }

    // ==================== JIT integration tests ====================

    #[test]
    fn jit_call_str_len() {
        use crate::codegen::context::CodegenContext;
        use cranelift_codegen::ir::{AbiParam, InstBuilder};
        use cranelift_frontend::FunctionBuilder;
        use cranelift_module::{Linkage, Module};
        use std::mem;

        let mut runtime = Runtime::new();
        register(&mut runtime);
        let len_sig = runtime.get("__str_len").unwrap().signature.clone();

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        // Create wrapper: fn test(ptr: i64, len: i64) -> i64 { __str_len(ptr, len) }
        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.params.push(AbiParam::new(types::I64));
        wrapper_sig.params.push(AbiParam::new(types::I64));
        wrapper_sig.returns.push(AbiParam::new(types::I64));
        let wrapper_id = ctx.declare_function("test", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();

            let len_func_id = module
                .declare_function("__str_len", Linkage::Import, &len_sig)
                .unwrap();

            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(len_func_id, builder.func);
            let ptr = builder.block_params(entry)[0];
            let len = builder.block_params(entry)[1];
            let call = builder.ins().call(func_ref, &[ptr, len]);
            let result = builder.inst_results(call)[0];

            builder.ins().return_(&[result]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let test: fn(i64, i64) -> i64 = unsafe { mem::transmute(ptr) };

        let s = "Hello";
        let result = test(s.as_ptr() as i64, s.len() as i64);
        assert_eq!(result, 5);

        // Test with null
        let result = test(0, 10);
        assert_eq!(result, 0);
    }
}
