//! The `wf wit` command: validate WIT files using wasm-tools.

use std::path::Path;

use wf_toolchains::process::{CommandSpec, ProcessRunner, StdRunner};

use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// Runs `wf wit check` and returns the process exit code.
pub fn run(args: &crate::cli::WitCheckArgs) -> i32 {
    if !args.path.exists() {
        eprintln!(
            "wf: cannot read WIT file {}: No such file or directory",
            args.path.display()
        );
        return EXIT_USAGE;
    }

    let spec = CommandSpec::new("wasm-tools")
        .args(["wit", "validate", &args.path.display().to_string()])
        .current_dir(args.path.parent().unwrap_or(Path::new(".")));

    let output = match StdRunner.run(&spec) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("wf: failed to run wasm-tools: {e}");
            eprintln!("  help: install wasm-tools (wasm-tools wit validate)");
            return EXIT_DIAGNOSTIC;
        }
    };

    if output.success() {
        EXIT_OK
    } else {
        eprintln!("wf: WIT validation failed:");
        if !output.stdout().is_empty() {
            eprintln!("{}", output.stdout());
        }
        if !output.stderr().is_empty() {
            eprintln!("{}", output.stderr());
        }
        EXIT_DIAGNOSTIC
    }
}
