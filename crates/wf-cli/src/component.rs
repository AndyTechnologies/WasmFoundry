//! The `wf component` command: build a WebAssembly component.

use wf_toolchains::process::{CommandSpec, ProcessRunner, StdRunner};

use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// Runs `wf component build` and returns the process exit code.
pub fn run(args: &crate::cli::ComponentBuildArgs) -> i32 {
    if !args.dir.exists() {
        eprintln!(
            "wf: component directory {} does not exist",
            args.dir.display()
        );
        return EXIT_USAGE;
    }
    if !args.dir.is_dir() {
        eprintln!("wf: {} is not a directory", args.dir.display());
        return EXIT_USAGE;
    }

    let cargo_toml = args.dir.join("Cargo.toml");
    if !cargo_toml.is_file() {
        eprintln!("wf: no Cargo.toml in {}", args.dir.display());
        return EXIT_USAGE;
    }

    // Check for at least one .wit file
    let wit_file = std::fs::read_dir(&args.dir)
        .map_err(|_| {
            eprintln!("wf: cannot read directory {}", args.dir.display());
            EXIT_DIAGNOSTIC
        })
        .and_then(|entries| {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wit") {
                    return Ok(path);
                }
            }
            Err(EXIT_DIAGNOSTIC)
        });

    if wit_file.is_err() {
        eprintln!("wf: no .wit file found in {}", args.dir.display());
        return EXIT_USAGE;
    }

    // Run cargo component build
    let spec = CommandSpec::new("cargo")
        .args([
            "component".to_string(),
            "build".to_string(),
            "--release".to_string(),
            "--target".to_string(),
            "wasm32-wasip1".to_string(),
        ])
        .current_dir(&args.dir);

    let output = match StdRunner.run(&spec) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("wf: failed to run cargo component: {e}");
            eprintln!("  help: install cargo component (cargo install cargo-component)");
            return EXIT_DIAGNOSTIC;
        }
    };

    if output.success() {
        EXIT_OK
    } else {
        eprintln!("wf: cargo component build failed:");
        if !output.stdout().is_empty() {
            eprintln!("{}", output.stdout());
        }
        if !output.stderr().is_empty() {
            eprintln!("{}", output.stderr());
        }
        EXIT_DIAGNOSTIC
    }
}
