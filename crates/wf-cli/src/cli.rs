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
    /// Validate a WIT file using wasm-tools.
    Wit(WitArgs),
    /// Build a WebAssembly component.
    Component(ComponentArgs),
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
    /// Path to a prebuilt launcher binary. Mutually exclusive with `--bundle-target`.
    #[arg(long, value_name = "LAUNCHER")]
    pub launcher: Option<std::path::PathBuf>,

    /// Prebuilt launcher resolved from the local registry by target triple.
    /// Mutually exclusive with `--launcher`.
    #[arg(long, value_name = "TRIPLE")]
    pub bundle_target: Option<String>,
}

/// Arguments of `wf wit`.
#[derive(Debug, clap::Args)]
pub struct WitArgs {
    #[command(subcommand)]
    pub command: WitCommand,
}

/// Subcommands of `wf wit`.
#[derive(Debug, Subcommand)]
pub enum WitCommand {
    /// Validate a WIT file using wasm-tools.
    Check(WitCheckArgs),
}

/// Arguments of `wf wit check`.
#[derive(Debug, clap::Args)]
pub struct WitCheckArgs {
    /// Path to the WIT file to validate.
    pub path: PathBuf,
}

/// Arguments of `wf component`.
#[derive(Debug, clap::Args)]
pub struct ComponentArgs {
    #[command(subcommand)]
    pub command: ComponentCommand,
}

/// Subcommands of `wf component`.
#[derive(Debug, Subcommand)]
pub enum ComponentCommand {
    /// Build a WebAssembly component.
    Build(ComponentBuildArgs),
}

/// Arguments of `wf component build`.
#[derive(Debug, clap::Args)]
pub struct ComponentBuildArgs {
    /// Path to the component project directory (containing Cargo.toml and .wit).
    #[arg(value_name = "DIR")]
    pub dir: PathBuf,
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
        Some(Command::Wit(args)) => match args.command {
            crate::cli::WitCommand::Check(args) => crate::wit::run(&args),
        },
        Some(Command::Component(args)) => match args.command {
            crate::cli::ComponentCommand::Build(args) => crate::component::run(&args),
        },
    }
}
