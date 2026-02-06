use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process;

use clap::{Parser, ValueEnum};
use spl_compiler::{
    AotError, Diagnostic, DiagnosticRenderer, JitError, RenderConfig, Severity,
    init_tracing_with_options,
};

/// The SPL compiler
#[derive(Parser)]
#[command(name = "spl", version, about = "SPL compiler and runtime")]
struct Cli {
    /// Source file to compile
    source: PathBuf,

    /// Output executable path (AOT compilation)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Execute via JIT instead of compiling
    #[arg(long)]
    jit: bool,

    /// Print timing for each compilation pass
    #[arg(long)]
    time_passes: bool,

    /// Set the log level
    #[arg(long, value_name = "LEVEL")]
    log_level: Option<LogLevel>,
}

#[derive(Clone, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

fn main() {
    let cli = Cli::parse();

    // Initialize tracing if requested
    if let Some(level) = &cli.log_level {
        init_tracing_with_options(level.as_str(), cli.time_passes);
    } else if cli.time_passes {
        init_tracing_with_options("info", true);
    }

    // Read source file
    let source = match std::fs::read_to_string(&cli.source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "error: could not read `{}`: {e}",
                cli.source.display()
            );
            process::exit(1);
        }
    };

    let file_name = cli.source.display().to_string();

    if cli.jit {
        run_jit(&source, &file_name);
    } else if let Some(output) = &cli.output {
        run_aot(&source, &file_name, output);
    } else {
        eprintln!("error: either --jit or -o <output> is required");
        process::exit(1);
    }
}

fn render_diagnostics(source: &str, file_name: &str, diagnostics: &[Diagnostic]) {
    let is_tty = std::io::stderr().is_terminal();
    let config = RenderConfig::new()
        .with_colors(is_tty)
        .with_file_name(file_name);
    let renderer = DiagnosticRenderer::new(source, config);

    for diag in diagnostics {
        let _ = renderer.eprint(&diag.to_base());
    }

    // Print summary
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();

    if errors > 0 || warnings > 0 {
        let mut parts = Vec::new();
        if errors > 0 {
            parts.push(format!(
                "{errors} error{}",
                if errors == 1 { "" } else { "s" }
            ));
        }
        if warnings > 0 {
            parts.push(format!(
                "{warnings} warning{}",
                if warnings == 1 { "" } else { "s" }
            ));
        }
        eprintln!("{} emitted", parts.join(", "));
    }
}

fn run_jit(source: &str, file_name: &str) {
    match spl_compiler::jit_execute(source) {
        Ok(exit_code) => process::exit(exit_code),
        Err(JitError::CompileError(diagnostics)) => {
            render_diagnostics(source, file_name, &diagnostics);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

fn run_aot(source: &str, file_name: &str, output: &Path) {
    match spl_compiler::compile_and_link(source, output) {
        Ok(()) => {}
        Err(AotError::CompileError(diagnostics)) => {
            render_diagnostics(source, file_name, &diagnostics);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}
