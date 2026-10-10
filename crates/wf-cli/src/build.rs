//! The `wf build` command.
//!
//! It reads the manifest, resolves each module to a toolchain and reports what each one
//! produced. No compiler knowledge lives here: which toolchain handles a manifest value,
//! and how it is invoked, belong to [`crate::toolchains`].

use std::path::{Path, PathBuf};

use wf_core::{Diagnostic, DiagnosticCode, Manifest, Severity, ToolchainId};

use crate::project::{self, ImportSummary};

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
    let mut published: Vec<(String, PathBuf)> = Vec::new();

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
                published.push((module.name.clone(), result.path().to_path_buf()));
            }
            Err(diagnostic) => return report(&[diagnostic]),
        }
    }

    // Once every module exists, check the set: an import that names two project modules
    // cannot be bound, and a cycle cannot be instantiated. Both are manifest problems,
    // and saying so here is cheaper than saying it the first time somebody runs it.
    if let Err(diagnostic) = check_links(&manifest, &published) {
        return report(&[diagnostic]);
    }

    println!(
        "wf: built {} module(s) into {}/",
        published.len(),
        Path::new("target").display()
    );
    EXIT_OK
}

/// Resolves every published module's imports against the rest of the project.
///
/// What a module imports is read from the artifact rather than from its source: the
/// artifact is what will be linked, so it is the only evidence that matters.
fn check_links(manifest: &Manifest, published: &[(String, PathBuf)]) -> Result<(), Diagnostic> {
    let mut imports = Vec::with_capacity(published.len());

    for (name, path) in published {
        let bytes = std::fs::read(path).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!("cannot read the published module `{name}`: {error}"),
            )
            .with_help("re-run `wf build`")
        })?;

        let analysis = wf_wasm::analyze(&bytes).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::InvalidWasm,
                Severity::Error,
                format!("module `{name}`: {error}"),
            )
            .with_help("the published module is not a valid WebAssembly module")
        })?;

        let namespaces = analysis
            .core()
            .map(|core| {
                core.imports()
                    .iter()
                    .map(|import| import.module().to_owned())
                    .collect()
            })
            .unwrap_or_default();

        imports.push(ImportSummary::new(name.clone(), namespaces));
    }

    project::plan(
        &manifest.modules,
        &imports,
        manifest.project.module_matching,
    )
    .map(|_| ())
}

/// Prints every diagnostic and returns the diagnostic exit code.
fn report(diagnostics: &[Diagnostic]) -> i32 {
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}");
    }
    EXIT_DIAGNOSTIC
}
