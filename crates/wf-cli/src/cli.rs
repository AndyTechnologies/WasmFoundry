//! Argument parsing for `wf`.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Product name shown in help and version output.
const PRODUCT: &str = "wf";

/// Top level command line surface of `wf`.
#[derive(Debug, Parser)]
#[command(
    name = "wf",
    version,
    about = "WasmFoundry: build, inspect and run WebAssembly projects",
    long_about = "WasmFoundry: build, inspect and run WebAssembly projects.\n\n\
                  The command surface is being rebuilt incrementally; \
                  `wf inspect` arrived in PHASE 3."
)]
pub struct Cli {
    /// Subcommand to execute. Optional because the product can also be
    /// invoked without one to report its version.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The commands `wf` can perform.
///
/// One variant per capability, and one capability per phase: a command exists here
/// only once it does something real.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Analyse a WebAssembly binary without running it.
    Inspect(InspectArgs),
}

/// Arguments of `wf inspect`.
#[derive(Debug, clap::Args)]
pub struct InspectArgs {
    /// Path to the WebAssembly binary to analyse.
    pub path: PathBuf,

    /// How to render the report.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Rendering of a command result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Aligned text for a terminal.
    Human,
    /// Machine-readable JSON.
    Json,
}

/// Parses arguments and dispatches, returning the process exit code.
pub fn run() -> i32 {
    let cli = Cli::parse();

    match cli.command {
        None => {
            println!(
                "{PRODUCT} {} — WasmFoundry. Use `{PRODUCT} --help`.",
                env!("CARGO_PKG_VERSION")
            );
            crate::EXIT_OK
        }
        Some(Command::Inspect(args)) => crate::inspect::run(&args),
    }
}
