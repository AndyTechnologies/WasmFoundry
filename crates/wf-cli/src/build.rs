// The `wf build` command.
//
// It reads the manifest, resolves each module to a toolchain and reports what each one
// produced. No compiler knowledge lives here: which toolchain handles a manifest value,
// and how it is invoked, belong to [`wf_toolchains::toolchains`].
//
// When `--watch` is provided, the command watches the project directory for changes
// and rebuilds automatically.

use notify::Watcher;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use wf_core::{Diagnostic, DiagnosticCode, Manifest, Severity, ToolchainId};
use wf_toolchains::process::StdRunner;
use wf_toolchains::toolchains::{self, CompileRequest};

use crate::cli::BuildArgs;
use crate::manifest::MANIFEST_FILE;
use crate::project::{self, ImportSummary};
use crate::{EXIT_DIAGNOSTIC, EXIT_OK};

/// Runs `wf build` and returns the process exit code.
pub fn run(args: &BuildArgs) -> i32 {
    // Being outside a project is the same problem for `build` and for `run`: the caller
    // is in the wrong directory. It gets the usage exit code in both, rather than a
    // diagnostic that suggests the manifest itself is broken.
    if !Path::new(MANIFEST_FILE).exists() {
        eprintln!("wf: no {MANIFEST_FILE} in this directory");
        eprintln!("  help: run `wf init <name>` to create a project, then `wf build` inside it");
        return crate::EXIT_USAGE;
    }

    // Perform an initial build.
    if let Err(code) = do_build() {
        return code;
    }

    // If watch mode is not requested, we are done.
    if !args.watch {
        return EXIT_OK;
    }

    // Watch mode: set up a file system watcher and rebuild on changes.
    println!("wf: watching for changes (press Ctrl+C to stop)");

    let (tx, rx) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("wf: could not create watcher: {e}");
            return EXIT_DIAGNOSTIC;
        }
    };

    // Watch the current directory recursively.
    if let Err(e) = watcher.watch(Path::new("."), notify::RecursiveMode::Recursive) {
        eprintln!("wf: could not watch directory: {e}");
        return EXIT_DIAGNOSTIC;
    }

    // Rebuild only when relevant events arrived, and only after the event storm of one
    // save operation settled: one build per pause in the events, none otherwise.
    let debounce = Duration::from_millis(500);
    let mut pending_changes = false;
    let mut last_event = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => {
                // Filter out uninteresting events.
                if should_ignore_event(&event) {
                    continue;
                }
                pending_changes = true;
                last_event = Instant::now();
            }
            Ok(Err(e)) => {
                eprintln!("wf: watcher error: {e}");
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {
                if pending_changes && last_event.elapsed() >= debounce {
                    // A broken build must not kill the watch loop: the diagnostic is on
                    // screen, the user edits a fix, and the next change rebuilds.
                    pending_changes = false;
                    last_event = Instant::now();
                    if let Err(_code) = do_build() {
                        continue;
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    EXIT_OK
}

/// Perform a single build, returning an exit code (OK on success).
fn do_build() -> Result<(), i32> {
    let manifest = match crate::manifest::read(Path::new(MANIFEST_FILE)) {
        Ok(manifest) => manifest,
        Err(diagnostic) => return Err(report(&[diagnostic])),
    };

    let diagnostics = manifest.validate();
    if !diagnostics.is_empty() {
        return Err(report(&diagnostics));
    }

    let root = Path::new(".");
    let runner = StdRunner;
    let mut published: Vec<(String, PathBuf)> = Vec::new();

    for module in &manifest.modules {
        let id = match ToolchainId::new(&module.toolchain) {
            Ok(id) => id,
            Err(error) => {
                return Err(report(&[Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    format!("module `{}`: {error}", module.name),
                )]));
            }
        };

        let Some(toolchain) = toolchains::lookup(&id) else {
            return Err(report(&[toolchains::unknown_toolchain(module, &id)]));
        };

        let request = CompileRequest::new(module, root, &runner);

        let detection = toolchain.detect(&request);
        if !detection.is_supported() {
            return Err(report(&[Diagnostic::new(
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
            .with_help("check the `source` field of this [[module]]")]));
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
            Err(diagnostic) => return Err(report(&[diagnostic])),
        }
    }

    // Once every module exists, check the set: an import that names two project modules
    // cannot be bound, and a cycle cannot be instantiated. Both are manifest problems,
    // and saying so here is cheaper than saying it the first time somebody runs it.
    if let Err(diagnostic) = check_links(&manifest, &published) {
        return Err(report(&[diagnostic]));
    }

    println!(
        "wf: built {} module(s) into {}/",
        published.len(),
        Path::new("target").display()
    );
    Ok(())
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
        &crate::project::allowed_host_namespaces(&manifest.project.host_namespaces),
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

/// Returns true if the event should be ignored (e.g., changes in target directory,
/// temporary files, etc.).
fn should_ignore_event(event: &notify::Event) -> bool {
    // Ignore events that are not relevant to the build (e.g., changes in target/).
    for path in &event.paths {
        let path_str = path.to_string_lossy();
        if path_str.contains("/target/") {
            return true;
        }
        // Ignore common temporary files.
        if path_str.ends_with(".swp") || path_str.ends_with(".tmp") || path_str.ends_with(".bak") {
            return true;
        }
        // Ignore hidden directories like .git, .atl, .codegraph (optional).
        if path_str.contains("/.git/")
            || path_str.contains("/.atl/")
            || path_str.contains("/.codegraph/")
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(path: &str) -> notify::Event {
        notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Any,
        )))
        .add_path(PathBuf::from(path))
    }

    /// A write of the published artifact must never trigger a rebuild of itself.
    #[test]
    fn the_build_output_never_triggers_a_rebuild() {
        assert!(should_ignore_event(&event("./target/hello.wasm")));
    }

    /// A source change is exactly why the watch loop exists.
    #[test]
    fn a_source_change_is_relevant() {
        assert!(!should_ignore_event(&event("./src/hello.rs")));
    }

    /// Editor swap files, backups and VCS internals are never interesting.
    #[test]
    fn editor_and_metadata_noise_is_ignored() {
        for path in ["./src/hello.rs.swp", "./src/hello.rs.bak", "./.git/index"] {
            assert!(should_ignore_event(&event(path)), "{path}");
        }
    }
}
