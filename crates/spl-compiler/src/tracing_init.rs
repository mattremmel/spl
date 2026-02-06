//! Tracing subscriber initialization for the SPL compiler.
//!
//! Provides convenience functions for setting up structured logging via `tracing`.
//! When no subscriber is installed (the default), all tracing macros are zero-cost no-ops.
//!
//! # Environment Variables
//!
//! - `SPL_LOG`: Filter directive for SPL crates (e.g. `debug`, `spl_sema=trace`)
//! - `RUST_LOG`: Standard filter directive (fallback if `SPL_LOG` is not set)
//!
//! # Usage
//!
//! ```ignore
//! use spl_compiler::init_tracing;
//!
//! // Install a subscriber that logs to stderr
//! init_tracing();
//!
//! // Now compile — spans and events will be printed
//! let result = spl_compiler::compile("fn main() {}");
//! ```

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// Initialize the global tracing subscriber with default settings.
///
/// Reads filter directives from `SPL_LOG` (preferred) or `RUST_LOG` (fallback).
/// Defaults to `info` level if neither is set.
///
/// Output goes to stderr in a human-readable format.
///
/// This should be called once, early in the program. Subsequent calls are no-ops
/// (the global subscriber can only be set once).
pub fn init_tracing() {
    let filter = EnvFilter::try_from_env("SPL_LOG")
        .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = fmt::layer()
        .with_target(true)
        .with_timer(fmt::time::uptime());

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(subscriber)
        .try_init();
}

/// Initialize the global tracing subscriber with span timing enabled.
///
/// Like [`init_tracing`], but also prints how long each span was active.
/// This is useful for `--time-passes` style output.
///
/// Reads filter directives from `SPL_LOG` (preferred) or `RUST_LOG` (fallback).
/// Defaults to `info` level if neither is set.
pub fn init_tracing_with_timing() {
    init_tracing_with_options("info", true);
}

/// Initialize the global tracing subscriber with custom filter and timing options.
///
/// The `filter` parameter sets the default log level (e.g. `"info"`, `"debug"`, `"trace"`).
/// It is overridden by the `SPL_LOG` or `RUST_LOG` environment variables if set.
///
/// When `timing` is `true`, span close events are printed, showing how long each
/// compilation pass took (equivalent to `--time-passes`).
///
/// This should be called once, early in the program. Subsequent calls are no-ops.
pub fn init_tracing_with_options(filter: &str, timing: bool) {
    let filter = EnvFilter::try_from_env("SPL_LOG")
        .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
        .unwrap_or_else(|_| EnvFilter::new(filter));

    let subscriber = fmt::layer()
        .with_target(true)
        .with_timer(fmt::time::uptime());

    // Conditionally add span events for timing output.
    // We need separate branches because `with_span_events` changes the layer type.
    if timing {
        let subscriber = subscriber.with_span_events(fmt::format::FmtSpan::CLOSE);
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(subscriber)
            .try_init();
    } else {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(subscriber)
            .try_init();
    }
}
