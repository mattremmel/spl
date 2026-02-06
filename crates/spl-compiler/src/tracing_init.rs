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

/// Output format for tracing logs.
#[derive(Debug, Clone, Copy, Default)]
pub enum LogFormat {
    /// Human-readable format (default).
    #[default]
    Human,
    /// JSON lines format for tooling consumption.
    Json,
}

/// Build an `EnvFilter` with correct precedence.
///
/// - `cli_filter = Some("debug")` — CLI flag was set, ignore env vars
/// - `cli_filter = None` — fall back to `SPL_LOG`, then `RUST_LOG`, then `"info"`
fn build_filter(cli_filter: Option<&str>) -> EnvFilter {
    if let Some(f) = cli_filter {
        EnvFilter::new(f)
    } else {
        EnvFilter::try_from_env("SPL_LOG")
            .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
            .unwrap_or_else(|_| EnvFilter::new("info"))
    }
}

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
    init_tracing_with_options(None, false, LogFormat::Human);
}

/// Initialize the global tracing subscriber with span timing enabled.
///
/// Like [`init_tracing`], but also prints how long each span was active.
/// This is useful for `--time-passes` style output.
///
/// Reads filter directives from `SPL_LOG` (preferred) or `RUST_LOG` (fallback).
/// Defaults to `info` level if neither is set.
pub fn init_tracing_with_timing() {
    init_tracing_with_options(None, true, LogFormat::Human);
}

/// Initialize the global tracing subscriber with custom filter, timing, and format options.
///
/// When `filter` is `Some(level)`, CLI flag takes precedence over env vars.
/// When `filter` is `None`, falls back to `SPL_LOG` or `RUST_LOG` env vars, then `"info"`.
///
/// When `timing` is `true`, span close events are printed, showing how long each
/// compilation pass took (equivalent to `--time-passes`).
///
/// `format` selects between human-readable and JSON output.
///
/// This should be called once, early in the program. Subsequent calls are no-ops.
pub fn init_tracing_with_options(filter: Option<&str>, timing: bool, format: LogFormat) {
    let filter = build_filter(filter);

    match format {
        LogFormat::Human => {
            let layer = fmt::layer()
                .with_target(true)
                .with_timer(fmt::time::uptime());

            if timing {
                let layer = layer.with_span_events(fmt::format::FmtSpan::CLOSE);
                let _ = tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .try_init();
            } else {
                let _ = tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .try_init();
            }
        }
        LogFormat::Json => {
            let layer = fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_target(true)
                .with_timer(fmt::time::uptime());

            if timing {
                let layer = layer.with_span_events(fmt::format::FmtSpan::CLOSE);
                let _ = tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .try_init();
            } else {
                let _ = tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .try_init();
            }
        }
    }
}
