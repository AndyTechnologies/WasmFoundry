//! The `wf build` command.
//!
//! It reads the manifest, resolves each module to a toolchain and reports what each one
//! produced. No compiler knowledge lives here: which toolchain handles a manifest value,
//! and how it is invoked, belong to [`crate::toolchains`].

use std::path::Path;

use wf_core::{Diagnostic, DiagnosticCode, Severity, ToolchainId};

use crate::cli::BuildArgs;
use crate::manifest::MANIFEST_FILE;
use crate::process::StdRunner;
use crate::toolchains::{self, CompileRequest};
use crate::{EXIT_DIAGNOSTIC, EXIT_OK};

/// Runs `wf build` and returns the process exit code.
pub fn run(_args: &BuildArgs) -> i32 {
    // Being outside a project is the same problem for `build` and for `run`: the caller
    // is in the wrong directory. It gets the usage exit code in both, rather than a
    // diagnostic that suggests the manifest itself is broken.
    if !Path::new(MANIFEST_FILE).exists() {
        eprintln!("wf: no {MANIFEST_FILE} in this directory");
        eprintln!("  help: run `wf init <name>` to create a project, then `wf build` inside it");
        return crate::EXIT_USAGE;
    }

    let manifest = match crate::manifest::read(Path::new(MANIFEST_FILE)) {
        Ok(manifest) => manifest,
        Err(diagnostic) => return report(&[diagnostic]),
    };

    let diagnostics = manifest.validate();
    if !diagnostics.is_empty() {
        return report(&diagnostics);
    }

    let root = Path::new(".");
    let runner = StdRunner;
    let mut built = 0;

    for module in &manifest.modules {
        let id = match ToolchainId::new(&module.toolchain) {
            Ok(id) => id,
            Err(error) => {
                return report(&[Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    format!("module `{}`: {error}", module.name),
                )]);
            }
        };

        let Some(toolchain) = toolchains::lookup(&id) else {
            return report(&[toolchains::unknown_toolchain(module, &id)]);
        };

        let request = CompileRequest::new(module, root, &runner);

        let detection = toolchain.detect(&request);
        if !detection.is_supported() {
            return report(&[Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!(
                    "module `{}`: {}",
                    module.name,
                    detection
                        .reason()
                        .unwrap_or("the source does not fit this toolchain")
                ),
            )
            .with_help("check the `source` field of this [[module]]")]);
        }

        match toolchain.compile(&request) {
            Ok(result) => {
                println!(
                    "wf: built {} ({}) via {} -> {}\n     {}",
                    result.artifact().id(),
                    result.artifact().kind(),
                    toolchain.id(),
                    result.path().display(),
                    result.detail()
                );
                built += 1;
            }
            Err(diagnostic) => return report(&[diagnostic]),
        }
    }

    println!(
        "wf: built {built} module(s) into {}/",
        Path::new("target").display()
    );
    EXIT_OK
}

/// Prints every diagnostic and returns the diagnostic exit code.
fn report(diagnostics: &[Diagnostic]) -> i32 {
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}");
    }
    EXIT_DIAGNOSTIC
}
