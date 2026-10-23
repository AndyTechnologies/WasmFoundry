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
                  The command surface is being rebuilt incrementally: \
                  `wf inspect` in PHASE 3, `wf run` in PHASE 4."
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
    /// Execute a WebAssembly core module.
    Run(RunArgs),
    /// Create a new project.
    Init(InitArgs),
    /// Build the modules declared in the project manifest.
    Build(BuildArgs),
    /// Wrap the built project into a native launcher bundle (POC).
    Bundle(BundleArgs),
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

/// Arguments of `wf run`.
#[derive(Debug, clap::Args)]
pub struct RunArgs {
    /// Path to the WebAssembly module to execute. Omit it to run the project declared by
    /// `wasmfoundry.toml` in the current directory.
    #[arg(value_name = "MODULE")]
    pub path: Option<PathBuf>,

    /// Guest export to invoke. Defaults to `_start`.
    #[arg(long)]
    pub entry: Option<String>,
}

/// Arguments of `wf init`.
#[derive(Debug, clap::Args)]
pub struct InitArgs {
    /// Name of the project to create, and of its directory.
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// Arguments of `wf build`.
#[derive(Debug, clap::Args)]
pub struct BuildArgs {
    /// Watch for changes and rebuild automatically.
    #[arg(long)]
    pub watch: bool,
}

/// Arguments of `wf bundle`.
#[derive(Debug, clap::Args)]
pub struct BundleArgs {
    /// Path to a prebuilt launcher binary. PHASE 14: `wf` never builds a launcher
    /// itself; the bundle receives one that already exists.
    #[arg(long, value_name = "LAUNCHER")]
    pub launcher: std::path::PathBuf,
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
        Some(Command::Run(args)) => crate::run::run(&args),
        Some(Command::Init(args)) => crate::init::run(&args),
        Some(Command::Build(args)) => crate::build::run(&args),
        Some(Command::Bundle(args)) => crate::bundle::run(&args),
    }
}
