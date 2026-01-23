//! Intrinsic functions for SPL's standard library.
//!
//! This module provides built-in functions that can be called from JIT-compiled code.
//! All intrinsic names follow the `__` (double underscore) prefix convention.
//!
//! # Intrinsic Categories
//!
//! Intrinsics fall into two categories based on their implementation:
//!
//! ## Inline IR Intrinsics
//!
//! These emit native Cranelift instructions directly, with no function call overhead:
//!
//! - `__abs_int`, `__abs_float` - absolute value (`iabs`, `fabs` instructions)
//! - `__min_int`, `__max_int` - signed min/max (`smin`, `smax` instructions)
//! - `__sqrt` - square root (`sqrt` instruction)
//!
//! See [`math::InlineMathIntrinsic`] for the codegen API.
//!
//! ## Runtime Function Intrinsics
//!
//! These require function calls to Rust code because they:
//! - Need OS interaction (I/O, process control)
//! - Require memory allocation (string operations)
//! - Have no native CPU instruction (`pow`)
//!
//! Current runtime intrinsics:
//! - I/O: `__print_int`, `__print_bool`, `__print_char`, `__print_str`, `__print_newline`
//! - Panic: `__abort`
//! - Math: `__pow` (no native instruction)
//! - Convert (stubs): `__int_to_string`, `__float_to_string`
//! - String (stubs): `__str_len`, `__str_concat`
//!
//! # Self-Hosting Considerations
//!
//! When self-hosting the SPL compiler (rewriting it in SPL), the runtime function
//! intrinsics will need alternative implementations since they currently call Rust.
//!
//! ## Option 1: libc / System Calls
//!
//! Call libc functions or raw syscalls instead of Rust:
//!
//! ```text
//! // __print_int becomes:
//! 1. Format integer to ASCII buffer (in SPL or inline IR)
//! 2. Call write(STDOUT_FILENO, buffer, len) via libc
//!    - Or emit raw syscall: syscall(SYS_write, 1, buffer, len)
//! ```
//!
//! ## Option 2: Minimal C Runtime
//!
//! Keep a small `libspl_runtime.a` with OS primitives:
//!
//! ```c
//! // runtime.c - compile and link with SPL programs
//! void __print_int(int64_t x) { printf("%lld", x); }
//! void __abort() { exit(1); }
//! void* __alloc(size_t n) { return malloc(n); }
//! ```
//!
//! This is the approach most languages take (Go, Rust, OCaml all have runtime
//! components not written in themselves).
//!
//! ## Option 3: Implement in SPL
//!
//! Once SPL has sufficient features, rewrite intrinsics in SPL itself:
//!
//! ```text
//! fn __print_int(x: Int) {
//!     let buffer = format_int(x);  // SPL function
//!     __write(1, buffer.ptr, buffer.len);  // thin syscall wrapper
//! }
//! ```
//!
//! ## Option 4: Inline Syscalls in Codegen
//!
//! For minimal runtime, emit syscalls directly as Cranelift IR:
//!
//! ```text
//! // Linux x86_64 write syscall:
//! mov rax, 1      // SYS_write
//! mov rdi, 1      // fd = stdout
//! mov rsi, buffer // buf
//! mov rdx, len    // count
//! syscall
//! ```
//!
//! ## Recommended Bootstrap Path
//!
//! 1. **Stage 0**: SPL compiler in Rust, intrinsics call Rust (current state)
//! 2. **Stage 1**: SPL compiler in SPL, compiled by Stage 0, links C runtime
//! 3. **Stage 2**: SPL compiler compiled by Stage 1 (proves self-hosting)
//! 4. **Optional**: Replace C runtime with SPL implementations over time

mod convert;
mod io;
mod math;
mod panic;
mod string;

use cranelift_codegen::ir::{AbiParam, Signature};
use cranelift_codegen::isa::CallConv;

use super::Runtime;

/// Register all intrinsic functions with the runtime.
pub fn register_all(runtime: &mut Runtime) {
    math::register(runtime);
    io::register(runtime);
    panic::register(runtime);
    convert::register(runtime);
    string::register(runtime);
}

/// Create a Cranelift signature with the given parameter and return types.
pub fn make_signature(
    call_conv: CallConv,
    params: &[cranelift_codegen::ir::Type],
    returns: &[cranelift_codegen::ir::Type],
) -> Signature {
    let mut sig = Signature::new(call_conv);
    for &ty in params {
        sig.params.push(AbiParam::new(ty));
    }
    for &ty in returns {
        sig.returns.push(AbiParam::new(ty));
    }
    sig
}

/// Get the default calling convention for intrinsics.
#[cfg(target_family = "unix")]
pub fn default_call_conv() -> CallConv {
    CallConv::SystemV
}

#[cfg(target_family = "windows")]
pub fn default_call_conv() -> CallConv {
    CallConv::WindowsFastcall
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    #[test]
    fn register_all_adds_intrinsics_to_runtime() {
        let mut runtime = Runtime::new();
        assert!(runtime.is_empty());

        register_all(&mut runtime);

        assert!(!runtime.is_empty());
        // Should have at least the core intrinsics
        assert!(runtime.len() >= 10);
    }

    #[test]
    fn all_intrinsics_have_valid_signatures() {
        let mut runtime = Runtime::new();
        register_all(&mut runtime);

        for func in runtime.iter() {
            // Name should not be empty
            assert!(!func.name.is_empty(), "Intrinsic has empty name");
            // Function pointer should not be null
            assert!(!func.ptr.is_null(), "Intrinsic {} has null pointer", func.name);
        }
    }

    #[test]
    fn intrinsic_names_follow_double_underscore_prefix_convention() {
        let mut runtime = Runtime::new();
        register_all(&mut runtime);

        for func in runtime.iter() {
            assert!(
                func.name.starts_with("__"),
                "Intrinsic '{}' does not follow __ prefix convention",
                func.name
            );
        }
    }

    #[test]
    fn make_signature_no_params_no_returns() {
        let sig = make_signature(CallConv::SystemV, &[], &[]);
        assert!(sig.params.is_empty());
        assert!(sig.returns.is_empty());
    }

    #[test]
    fn make_signature_with_params() {
        let sig = make_signature(CallConv::SystemV, &[types::I64, types::I64], &[types::I64]);
        assert_eq!(sig.params.len(), 2);
        assert_eq!(sig.params[0].value_type, types::I64);
        assert_eq!(sig.params[1].value_type, types::I64);
        assert_eq!(sig.returns.len(), 1);
        assert_eq!(sig.returns[0].value_type, types::I64);
    }

    #[test]
    fn make_signature_preserves_call_conv() {
        let sig = make_signature(CallConv::SystemV, &[], &[]);
        assert_eq!(sig.call_conv, CallConv::SystemV);
    }
}
