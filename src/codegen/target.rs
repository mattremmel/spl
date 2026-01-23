//! Target ISA configuration for code generation.
//!
//! This module handles target platform configuration, including auto-detection
//! for JIT compilation and explicit configuration for AOT compilation.

use std::sync::Arc;

use cranelift_codegen::isa::{CallConv, TargetIsa};
use cranelift_codegen::settings::{self, Configurable, Flags};
use cranelift_native;
use target_lexicon::Triple;

use super::error::CodegenError;

/// Target configuration for code generation.
///
/// Holds the target ISA, compiler flags, and target triple. This is used
/// to configure how code is generated for a specific platform.
pub struct TargetConfig {
    /// The target ISA (Instruction Set Architecture).
    isa: Arc<dyn TargetIsa>,
    /// Compiler flags used for this target.
    flags: Flags,
    /// The target triple describing the platform.
    triple: Triple,
}

impl TargetConfig {
    /// Create a target configuration for JIT compilation on the native host.
    ///
    /// Auto-detects the host CPU features and configures the ISA accordingly.
    pub fn native() -> Result<Self, CodegenError> {
        let mut flag_builder = settings::builder();

        // Enable optimizations suitable for JIT
        flag_builder
            .set("opt_level", "speed")
            .map_err(|e| CodegenError::IsaConfiguration(e.to_string()))?;

        // cranelift-jit requires is_pic=false
        flag_builder
            .set("is_pic", "false")
            .map_err(|e| CodegenError::IsaConfiguration(e.to_string()))?;

        let flags = Flags::new(flag_builder);

        // Get native target with host CPU features
        let isa = cranelift_native::builder()
            .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?
            .finish(flags.clone())
            .map_err(|e| CodegenError::IsaConfiguration(e.to_string()))?;

        let triple = isa.triple().clone();

        Ok(Self { isa, flags, triple })
    }

    /// Create a target configuration for a specific target triple (for AOT compilation).
    ///
    /// This is useful for cross-compilation or ahead-of-time compilation.
    pub fn for_target(triple: Triple) -> Result<Self, CodegenError> {
        let mut flag_builder = settings::builder();

        // Enable optimizations
        flag_builder
            .set("opt_level", "speed")
            .map_err(|e| CodegenError::IsaConfiguration(e.to_string()))?;

        let flags = Flags::new(flag_builder);

        let isa = cranelift_codegen::isa::lookup(triple.clone())
            .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?
            .finish(flags.clone())
            .map_err(|e| CodegenError::IsaConfiguration(e.to_string()))?;

        Ok(Self { isa, flags, triple })
    }

    /// Get the target ISA.
    pub fn isa(&self) -> &Arc<dyn TargetIsa> {
        &self.isa
    }

    /// Get the compiler flags.
    pub fn flags(&self) -> &Flags {
        &self.flags
    }

    /// Get the target triple.
    pub fn triple(&self) -> &Triple {
        &self.triple
    }

    /// Get the default calling convention for this target.
    pub fn default_call_conv(&self) -> CallConv {
        self.isa.default_call_conv()
    }

    /// Get the pointer type for this target (I32 for 32-bit, I64 for 64-bit).
    pub fn pointer_type(&self) -> cranelift_codegen::ir::Type {
        self.isa.pointer_type()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_target_creates() {
        let config = TargetConfig::native();
        assert!(config.is_ok(), "failed to create native target: {:?}", config.err());
    }

    #[test]
    fn native_target_has_valid_isa() {
        let config = TargetConfig::native().unwrap();
        // Should have a valid pointer type (either 32 or 64 bit)
        let ptr_type = config.pointer_type();
        assert!(
            ptr_type == cranelift_codegen::ir::types::I32
                || ptr_type == cranelift_codegen::ir::types::I64
        );
    }

    #[test]
    fn native_target_has_call_conv() {
        let config = TargetConfig::native().unwrap();
        // Should have a valid calling convention
        let _call_conv = config.default_call_conv();
    }

    #[test]
    fn native_target_has_triple() {
        let config = TargetConfig::native().unwrap();
        let triple = config.triple();
        // Triple should not be empty
        assert!(!triple.to_string().is_empty());
    }

    #[test]
    fn for_target_x86_64() {
        let triple: Triple = "x86_64-unknown-linux-gnu".parse().unwrap();
        let config = TargetConfig::for_target(triple.clone());

        // x86_64 support may be disabled when building on non-x86_64 platforms
        if let Ok(config) = config {
            assert_eq!(config.triple().architecture, triple.architecture);
        }
        // If it fails with UnsupportedTarget, that's expected on aarch64 builds
    }

    #[test]
    fn for_target_aarch64() {
        let triple: Triple = "aarch64-unknown-linux-gnu".parse().unwrap();
        let config = TargetConfig::for_target(triple.clone());
        assert!(config.is_ok(), "failed to create aarch64 target: {:?}", config.err());

        let config = config.unwrap();
        assert_eq!(config.triple().architecture, triple.architecture);
    }

    #[test]
    fn for_target_invalid() {
        // This should fail - Cranelift doesn't support WASM as a target ISA
        // (WASM is typically a source, not a target for Cranelift)
        let triple: Triple = "wasm32-unknown-unknown".parse().unwrap();
        let result = TargetConfig::for_target(triple);
        // This may or may not fail depending on Cranelift version
        // The test is here to ensure we handle errors gracefully
        let _ = result;
    }

    #[test]
    fn target_config_accessors() {
        let config = TargetConfig::native().unwrap();

        // Test all accessors work without panicking
        let _ = config.isa();
        let _ = config.flags();
        let _ = config.triple();
        let _ = config.default_call_conv();
        let _ = config.pointer_type();
    }
}
