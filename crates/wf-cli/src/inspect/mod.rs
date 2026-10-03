//! The `wf inspect` command.
//!
//! It answers "what is in this binary" without executing a single instruction of it:
//! the analysis is delegated to [`wf_wasm`], and this module only decides how the
//! result is presented.

pub mod dto;
pub mod human;

use crate::cli::{InspectArgs, OutputFormat};
use crate::{EXIT_DIAGNOSTIC, EXIT_OK};

/// Exit code for an input that is not a readable WebAssembly binary.
///
/// Stable once published: a script may branch on it.
const WF001: &str = "WF001";

/// Runs `wf inspect` and returns the process exit code.
pub fn run(args: &InspectArgs) -> i32 {
    let bytes = match std::fs::read(&args.path) {
        Ok(bytes) => bytes,
        Err(error) => {
            // Reading the file is the CLI's job, not the analyser's: a missing or
            // unreadable path is a usage problem, not a malformed module.
            eprintln!("wf: cannot read {}: {error}", args.path.display());
            return crate::EXIT_USAGE;
        }
    };

    let analysis = match wf_wasm::analyze(&bytes) {
        Ok(analysis) => analysis,
        Err(error) => {
            eprintln!("{WF001}: {error}");
            return EXIT_DIAGNOSTIC;
        }
    };

    match args.format {
        OutputFormat::Human => print!("{}", human::render(&args.path, &analysis)),
        OutputFormat::Json => match dto::to_json(&analysis) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("wf: cannot render JSON: {error}");
                return EXIT_DIAGNOSTIC;
            }
        },
    }

    EXIT_OK
}
