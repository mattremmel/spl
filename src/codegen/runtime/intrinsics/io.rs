//! I/O intrinsic functions.
//!
//! Functions for input/output to stdout, stderr, and stdin.
//!
//! # Current Implementation
//!
//! These intrinsics are implemented as Rust functions using `print!` macros,
//! which go through Rust's stdout buffering and formatting infrastructure.
//!
//! # Self-Hosting Alternatives
//!
//! When self-hosting, these will need to be reimplemented using one of:
//!
//! ## libc
//! ```c
//! // Link against libc and call directly
//! write(STDOUT_FILENO, buffer, len);  // for raw bytes
//! printf("%lld", value);               // for formatted output
//! read(STDIN_FILENO, buffer, len);     // for input
//! ```
//!
//! ## Raw syscalls (Linux x86_64)
//! ```text
//! // SYS_write = 1
//! mov rax, 1      // syscall number
//! mov rdi, 1      // fd = stdout (2 for stderr)
//! mov rsi, buf    // buffer pointer
//! mov rdx, len    // buffer length
//! syscall
//!
//! // SYS_read = 0
//! mov rax, 0      // syscall number
//! mov rdi, 0      // fd = stdin
//! mov rsi, buf    // buffer pointer
//! mov rdx, len    // buffer length
//! syscall
//! ```
//!
//! ## SPL implementation
//! ```text
//! fn __print_int(x: Int) {
//!     let buf = int_to_ascii(x);  // format to stack buffer
//!     __write(1, buf.ptr, buf.len);  // thin syscall wrapper
//! }
//! ```
//!
//! The integer-to-ASCII conversion would need to be implemented in SPL or
//! as inline IR (repeated division by 10, digit extraction).

use std::io::{BufRead, Read, Write};

use cranelift_codegen::ir::types;

use super::convert::StringResult;
use super::{Runtime, default_call_conv, make_signature};

/// Register all I/O intrinsics.
pub fn register(runtime: &mut Runtime) {
    let call_conv = default_call_conv();

    // ==================== stdout ====================

    // __print_int: (I64) -> ()
    runtime.register(
        "__print_int",
        __print_int as *const u8,
        make_signature(call_conv, &[types::I64], &[]),
    );

    // __print_float: (F64) -> ()
    runtime.register(
        "__print_float",
        __print_float as *const u8,
        make_signature(call_conv, &[types::F64], &[]),
    );

    // __print_bool: (I8) -> ()
    runtime.register(
        "__print_bool",
        __print_bool as *const u8,
        make_signature(call_conv, &[types::I8], &[]),
    );

    // __print_char: (I32) -> ()
    runtime.register(
        "__print_char",
        __print_char as *const u8,
        make_signature(call_conv, &[types::I32], &[]),
    );

    // __print_str: (*const u8, I64) -> ()
    // Using I64 for pointer on 64-bit platforms
    runtime.register(
        "__print_str",
        __print_str as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[]),
    );

    // __print_newline: () -> ()
    runtime.register(
        "__print_newline",
        __print_newline as *const u8,
        make_signature(call_conv, &[], &[]),
    );

    // ==================== stderr ====================

    // __eprint_int: (I64) -> ()
    runtime.register(
        "__eprint_int",
        __eprint_int as *const u8,
        make_signature(call_conv, &[types::I64], &[]),
    );

    // __eprint_str: (*const u8, I64) -> ()
    runtime.register(
        "__eprint_str",
        __eprint_str as *const u8,
        make_signature(call_conv, &[types::I64, types::I64], &[]),
    );

    // __eprint_newline: () -> ()
    runtime.register(
        "__eprint_newline",
        __eprint_newline as *const u8,
        make_signature(call_conv, &[], &[]),
    );

    // ==================== flush ====================

    // __flush: () -> ()
    runtime.register(
        "__flush",
        __flush as *const u8,
        make_signature(call_conv, &[], &[]),
    );

    // __flush_err: () -> ()
    runtime.register(
        "__flush_err",
        __flush_err as *const u8,
        make_signature(call_conv, &[], &[]),
    );

    // ==================== stdin ====================

    // __read_line: () -> (I64, I64) - returns (ptr, len) or (null, 0) on EOF
    runtime.register(
        "__read_line",
        __read_line as *const u8,
        make_signature(call_conv, &[], &[types::I64, types::I64]),
    );

    // __read_char: () -> I32 - returns char or -1 on EOF
    runtime.register(
        "__read_char",
        __read_char as *const u8,
        make_signature(call_conv, &[], &[types::I32]),
    );
}

// ==================== stdout ====================

/// Print an integer to stdout.
pub extern "C" fn __print_int(value: i64) {
    print!("{value}");
}

/// Print a float to stdout.
pub extern "C" fn __print_float(value: f64) {
    print!("{value}");
}

/// Print a boolean to stdout.
pub extern "C" fn __print_bool(value: i8) {
    if value != 0 {
        print!("true");
    } else {
        print!("false");
    }
}

/// Print a character to stdout.
pub extern "C" fn __print_char(value: i32) {
    if let Some(c) = char::from_u32(value as u32) {
        print!("{c}");
    }
}

/// Print a string to stdout.
///
/// # Safety
///
/// The pointer must be valid for `len` bytes if non-null.
pub extern "C" fn __print_str(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        return;
    }
    // SAFETY: Caller guarantees ptr is valid for len bytes
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    if let Ok(s) = std::str::from_utf8(slice) {
        print!("{s}");
    }
}

/// Print a newline to stdout.
pub extern "C" fn __print_newline() {
    println!();
}

// ==================== stderr ====================

/// Print an integer to stderr.
pub extern "C" fn __eprint_int(value: i64) {
    eprint!("{value}");
}

/// Print a string to stderr.
///
/// # Safety
///
/// The pointer must be valid for `len` bytes if non-null.
pub extern "C" fn __eprint_str(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        return;
    }
    // SAFETY: Caller guarantees ptr is valid for len bytes
    let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    if let Ok(s) = std::str::from_utf8(slice) {
        eprint!("{s}");
    }
}

/// Print a newline to stderr.
pub extern "C" fn __eprint_newline() {
    eprintln!();
}

// ==================== flush ====================

/// Flush stdout.
pub extern "C" fn __flush() {
    let _ = std::io::stdout().flush();
}

/// Flush stderr.
pub extern "C" fn __flush_err() {
    let _ = std::io::stderr().flush();
}

// ==================== stdin ====================

/// Read a line from stdin.
///
/// Returns (ptr, len) where ptr points to a newly allocated buffer containing
/// the line without the trailing newline, or (null, 0) on EOF or error.
///
/// # Safety
///
/// The caller is responsible for freeing the returned buffer with `__free`.
pub extern "C" fn __read_line() -> StringResult {
    let stdin = std::io::stdin();
    let mut line = String::new();

    match stdin.lock().read_line(&mut line) {
        Ok(0) => {
            // EOF
            StringResult {
                ptr: std::ptr::null(),
                len: 0,
            }
        }
        Ok(_) => {
            // Remove trailing newline
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }

            // Leak the string to return a stable pointer
            // Caller must free with __free
            let leaked = Box::leak(line.into_boxed_str());
            StringResult {
                ptr: leaked.as_ptr(),
                len: leaked.len() as i64,
            }
        }
        Err(_) => StringResult {
            ptr: std::ptr::null(),
            len: 0,
        },
    }
}

/// Read a single character from stdin.
///
/// Returns the character as a Unicode code point, or -1 on EOF or error.
pub extern "C" fn __read_char() -> i32 {
    let stdin = std::io::stdin();
    let mut buf = [0u8; 4]; // Max UTF-8 bytes for a char

    // Read one byte first
    match stdin.lock().read(&mut buf[0..1]) {
        Ok(0) => return -1, // EOF
        Ok(_) => {}
        Err(_) => return -1,
    }

    // Determine UTF-8 sequence length
    let seq_len = match buf[0] {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => return -1, // Invalid UTF-8 start byte
    };

    // Read remaining bytes if needed
    if seq_len > 1 {
        match stdin.lock().read_exact(&mut buf[1..seq_len]) {
            Ok(_) => {}
            Err(_) => return -1,
        }
    }

    // Parse as UTF-8
    match std::str::from_utf8(&buf[..seq_len]) {
        Ok(s) => s.chars().next().map_or(-1, |c| c as i32),
        Err(_) => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;
    use std::sync::atomic::{AtomicI32, Ordering};

    // ==================== Direct call tests ====================

    #[test]
    fn print_int_does_not_panic() {
        // These shouldn't panic
        __print_int(42);
        __print_int(-42);
        __print_int(0);
        __print_int(i64::MAX);
        __print_int(i64::MIN);
    }

    #[test]
    fn print_float_does_not_panic() {
        __print_float(2.5);
        __print_float(-2.5);
        __print_float(0.0);
        __print_float(f64::INFINITY);
        __print_float(f64::NAN);
    }

    #[test]
    fn print_bool_does_not_panic() {
        __print_bool(0);  // false
        __print_bool(1);  // true
        __print_bool(-1); // true (non-zero)
    }

    #[test]
    fn print_char_does_not_panic() {
        __print_char('A' as i32);
        __print_char('λ' as i32);
        __print_char(0); // null char
    }

    #[test]
    fn print_str_does_not_panic() {
        let s = "Hello, World!";
        __print_str(s.as_ptr(), s.len() as i64);
    }

    #[test]
    fn print_str_null_ptr_does_not_panic() {
        __print_str(std::ptr::null(), 10);
    }

    #[test]
    fn print_str_zero_len_does_not_panic() {
        let s = "Hello";
        __print_str(s.as_ptr(), 0);
    }

    #[test]
    fn print_str_negative_len_does_not_panic() {
        let s = "Hello";
        __print_str(s.as_ptr(), -5);
    }

    #[test]
    fn print_newline_does_not_panic() {
        __print_newline();
    }

    #[test]
    fn eprint_int_does_not_panic() {
        __eprint_int(42);
        __eprint_int(-1);
    }

    #[test]
    fn eprint_str_does_not_panic() {
        let s = "error message";
        __eprint_str(s.as_ptr(), s.len() as i64);
    }

    #[test]
    fn eprint_str_null_does_not_panic() {
        __eprint_str(std::ptr::null(), 10);
    }

    #[test]
    fn eprint_newline_does_not_panic() {
        __eprint_newline();
    }

    #[test]
    fn flush_does_not_panic() {
        __flush();
    }

    #[test]
    fn flush_err_does_not_panic() {
        __flush_err();
    }

    // Note: __read_line and __read_char require interactive input, so we
    // only test that they're registered and have correct signatures.

    // ==================== Registration tests ====================

    #[test]
    fn register_adds_all_io_intrinsics() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        // stdout
        assert!(runtime.contains("__print_int"));
        assert!(runtime.contains("__print_float"));
        assert!(runtime.contains("__print_bool"));
        assert!(runtime.contains("__print_char"));
        assert!(runtime.contains("__print_str"));
        assert!(runtime.contains("__print_newline"));

        // stderr
        assert!(runtime.contains("__eprint_int"));
        assert!(runtime.contains("__eprint_str"));
        assert!(runtime.contains("__eprint_newline"));

        // flush
        assert!(runtime.contains("__flush"));
        assert!(runtime.contains("__flush_err"));

        // stdin
        assert!(runtime.contains("__read_line"));
        assert!(runtime.contains("__read_char"));
    }

    // ==================== Signature tests ====================

    #[test]
    fn print_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__print_int").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn print_bool_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__print_bool").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I8);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn print_char_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__print_char").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I32);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn print_str_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__print_str").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn print_newline_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__print_newline").unwrap();
        assert!(func.signature.params.is_empty());
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn print_float_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__print_float").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::F64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn eprint_int_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__eprint_int").unwrap();
        assert_eq!(func.signature.params.len(), 1);
        assert_eq!(func.signature.params[0].value_type, types::I64);
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn eprint_str_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__eprint_str").unwrap();
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.params[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.params[1].value_type, types::I64); // len
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn flush_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__flush").unwrap();
        assert!(func.signature.params.is_empty());
        assert!(func.signature.returns.is_empty());
    }

    #[test]
    fn read_line_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__read_line").unwrap();
        assert!(func.signature.params.is_empty());
        assert_eq!(func.signature.returns.len(), 2);
        assert_eq!(func.signature.returns[0].value_type, types::I64); // ptr
        assert_eq!(func.signature.returns[1].value_type, types::I64); // len
    }

    #[test]
    fn read_char_signature() {
        let mut runtime = Runtime::new();
        register(&mut runtime);

        let func = runtime.get("__read_char").unwrap();
        assert!(func.signature.params.is_empty());
        assert_eq!(func.signature.returns.len(), 1);
        assert_eq!(func.signature.returns[0].value_type, types::I32);
    }

    // ==================== JIT integration tests ====================

    // For I/O functions, we use atomic counters to verify the JIT actually calls them

    #[test]
    fn jit_call_print_newline() {
        use crate::codegen::context::CodegenContext;
        use cranelift_codegen::ir::InstBuilder;
        use cranelift_frontend::FunctionBuilder;
        use cranelift_module::{Linkage, Module};
        use std::mem;

        static CALL_COUNT: AtomicI32 = AtomicI32::new(0);

        extern "C" fn mock_print_newline() {
            CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        // Reset counter
        CALL_COUNT.store(0, Ordering::SeqCst);

        let mut runtime = Runtime::new();
        let call_conv = default_call_conv();
        let print_sig = make_signature(call_conv, &[], &[]);
        runtime.register(
            "__print_newline",
            mock_print_newline as *const u8,
            print_sig.clone(),
        );

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        // Create wrapper: fn test() { __print_newline() }
        let wrapper_sig = ctx.new_signature();
        let wrapper_id = ctx.declare_function("test", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();

            let print_func_id = module
                .declare_function("__print_newline", Linkage::Import, &print_sig)
                .unwrap();

            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(print_func_id, builder.func);
            builder.ins().call(func_ref, &[]);

            builder.ins().return_(&[]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let test: fn() = unsafe { mem::transmute(ptr) };

        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 0);
        test();
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
        test();
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn jit_call_print_int() {
        use crate::codegen::context::CodegenContext;
        use cranelift_codegen::ir::{AbiParam, InstBuilder};
        use cranelift_frontend::FunctionBuilder;
        use cranelift_module::{Linkage, Module};
        use std::mem;
        use std::sync::atomic::AtomicI64;

        static LAST_VALUE: AtomicI64 = AtomicI64::new(0);

        extern "C" fn mock_print_int(value: i64) {
            LAST_VALUE.store(value, Ordering::SeqCst);
        }

        let mut runtime = Runtime::new();
        let call_conv = default_call_conv();
        let print_sig = make_signature(call_conv, &[types::I64], &[]);
        runtime.register(
            "__print_int",
            mock_print_int as *const u8,
            print_sig.clone(),
        );

        let mut ctx = CodegenContext::new_jit_with_runtime(&runtime).unwrap();

        // Create wrapper: fn test(x: i64) { __print_int(x) }
        let mut wrapper_sig = ctx.new_signature();
        wrapper_sig.params.push(AbiParam::new(types::I64));
        let wrapper_id = ctx.declare_function("test", &wrapper_sig).unwrap();

        ctx.compilation_context().func.signature = wrapper_sig;

        {
            let (func, func_ctx, module) = ctx.builder_context_with_module();

            let print_func_id = module
                .declare_function("__print_int", Linkage::Import, &print_sig)
                .unwrap();

            let mut builder = FunctionBuilder::new(func, func_ctx);

            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let func_ref = module.declare_func_in_func(print_func_id, builder.func);
            let arg = builder.block_params(entry)[0];
            builder.ins().call(func_ref, &[arg]);

            builder.ins().return_(&[]);
            builder.finalize();
        }

        ctx.define_function(wrapper_id).unwrap();
        ctx.finalize();

        let ptr = ctx.get_function_ptr(wrapper_id);
        let test: fn(i64) = unsafe { mem::transmute(ptr) };

        test(42);
        assert_eq!(LAST_VALUE.load(Ordering::SeqCst), 42);

        test(-100);
        assert_eq!(LAST_VALUE.load(Ordering::SeqCst), -100);
    }
}
