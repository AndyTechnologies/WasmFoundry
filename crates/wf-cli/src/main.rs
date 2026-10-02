//! The `wf` command line interface.
//!
//! # Responsibility
//!
//! This binary is the only entry point a user is expected to run. It owns
//! argument parsing, terminal output and process exit codes. It owns no domain
//! logic: parsing a module belongs to `wf-wasm`, executing a guest belongs to
//! `wf-runtime`, and the vocabulary they speak belongs to `wf-core`. A CLI that
//! grows business logic becomes the god object the architecture forbids.
//!
//! # Current status
//!
//! PHASE 2 defines exactly two behaviours, both provided by the parser:
//! `wf --version` and `wf --help`. Running `wf` with no arguments reports the
//! product and version and states plainly that no command is implemented yet.
//! No command is stubbed: a command that prints "not implemented" is a lie
//! shaped like a feature. The first real command, `wf inspect`, arrives in
//! PHASE 3.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

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
                  `wf inspect` arrives in PHASE 3."
)]
struct Cli {
    /// Subcommand to execute. Optional because the product can also be
    /// invoked without one to report its version.
    #[command(subcommand)]
    command: Option<Subcommands>,
}

/// Placeholder for the `wf` subcommands.
///
/// The enum is deliberately empty: PHASE 3 fills it with `inspect`, PHASE 4
/// with `run`, PHASE 5 with `init` and `build`. It exists now so that the help
/// output, the derive wiring and the `Option<Subcommands>` shape are already
/// in place, instead of being retrofitted around a growing `main`.
#[derive(Debug, Subcommand)]
enum Subcommands {}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // Uninhabited: no subcommand exists yet, so this arm can never run.
        Some(subcommand) => match subcommand {},
        None => println!(
            "{PRODUCT} {} — WasmFoundry. No command is implemented yet; \
             use `{PRODUCT} --help`.",
            env!("CARGO_PKG_VERSION")
        ),
    }
}
