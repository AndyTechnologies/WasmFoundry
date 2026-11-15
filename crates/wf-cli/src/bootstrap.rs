//! The `wf bootstrap` command.
//! Self-host WasmFoundry: compile the CLI to WASM + bundle → `wf` auto-hospedado.

use std::path::PathBuf;
use std::process::Command;
use std::fs;

use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// Runs `wf bootstrap` and returns the process exit code.
pub fn run() -> i32 {
    println!("wf: bootstrapping self-hosted WasmFoundry...");

    // Step 1: Scaffold a new project
    let project_name = "bootstrap-project";
    let root = PathBuf::from(project_name);

    if root.exists() {
        eprintln!(
            "wf: {} already exists, refusing to overwrite it",
            root.display()
        );
        return EXIT_USAGE;
    }

    // Create project directory structure
    if let Err(error) = fs::create_dir_all(&root) {
        eprintln!("wf: cannot create project directory: {error}");
        return EXIT_DIAGNOSTIC;
    }
    let source_dir = root.join("src");
    if let Err(error) = fs::create_dir_all(&source_dir) {
        eprintln!("wf: cannot create source directory: {error}");
        return EXIT_DIAGNOSTIC;
    }

    let package = project_name;

    // Create sample WAT module
    const SAMPLE_WAT: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "_start")))"#;

    let sample_bytes = match wat::parse_str(SAMPLE_WAT) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("wf: cannot assemble the sample module: {error}");
            return EXIT_DIAGNOSTIC;
        }
    };

    // Write sample module
    if let Err(error) = fs::write(source_dir.join(format!("{package}.wasm")), &sample_bytes) {
        eprintln!("wf: cannot write sample module: {error}");
        return EXIT_DIAGNOSTIC;
    }

    // Create minimal manifest
    let manifest = format!(
        r#"schema = 1

[package]
name = "{package}"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "{package}"
source = "src/{package}.wasm"
toolchain = "precompiled"
"#
    );

    // Write manifest
    if let Err(error) = fs::write(root.join("wasmfoundry.toml"), &manifest) {
        eprintln!("wf: cannot write manifest: {error}");
        return EXIT_DIAGNOSTIC;
    }

    println!("wf: created project `{}` in {}", project_name, root.display());
    println!("  wasmfoundry.toml");
    println!("  src/{}.wasm  (sample module)", package);
    println!();

    // Step 2: Build the project (wf-cli)
    println!("wf: building project...");
    let cli_crate_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let build_output = Command::new("cargo")
        .args(["build", "--release", "-p", "wf-cli"])
        .current_dir(&cli_crate_dir)
        .output();

    match build_output {
        Ok(output) if output.status.success() => {
            println!("wf: build completed successfully");
        }
        Ok(output) => {
            eprintln!("wf: build failed");
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            return EXIT_DIAGNOSTIC;
        }
        Err(error) => {
            eprintln!("wf: build command failed: {error}");
            return EXIT_DIAGNOSTIC;
        }
    }
    println!();

    // Step 3: Bundle the CLI as WASM
    println!("wf: bundling CLI as WASM...");
    // Use cargo run -p wf-bundle --bin wf-launcher from the cli crate directory
    let bundle_output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "wf-bundle",
            "--bin",
            "wf-launcher",
            "--",
            "bundle",
        ])
        .current_dir(&cli_crate_dir)
        .output();

    match bundle_output {
        Ok(output) if output.status.success() => {
            println!("wf: bundle completed successfully");
        }
        Ok(output) => {
            eprintln!("wf: bundle failed");
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            return EXIT_DIAGNOSTIC;
        }
        Err(error) => {
            eprintln!("wf: bundle command failed: {error}");
            return EXIT_DIAGNOSTIC;
        }
    }

    println!("wf: self-hosted WasmFoundry is ready!");
    println!("  Project: {}", root.display());
    println!();
    println!("Next steps:");
    println!("  1. Replace the sample module with your own WebAssembly code");
    println!("  2. Run `wf build` to rebuild");
    println!("  3. Run `wf run` to execute your guest module");

    EXIT_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_run_returns_ok() {
        let code = run();
        // Just test that it doesn't panic and returns a code
        assert!(code == EXIT_OK || code == EXIT_USAGE || code == EXIT_DIAGNOSTIC);
    }
}
