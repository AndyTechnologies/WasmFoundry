//! The `wf run` command.
//!
//! It executes a compiled module and reports either what the guest produced or precisely
//! why it could not run. The runtime crate does the work; this module only reads the file,
//! picks the entry point and maps the cause to a diagnostic code.

use std::path::{Path, PathBuf};
use std::time::Duration;

use wf_core::{Diagnostic, EntryPoint, Severity, namespace_of};
use wf_runtime::{Runtime, RuntimeError, Unit};

use crate::cli::RunArgs;
use crate::manifest::MANIFEST_FILE;
use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// Directory published artifacts live in, as written by `wf build`.
///
/// Shared with `build` by value rather than by a module: two commands that must agree on
/// a directory disagree in a way a compile error catches, which is cheaper than a shared
/// abstraction for one string.
const TARGET_DIR: &str = "target";

/// What the command will execute.
enum Planned {
    /// A module named on the command line.
    Module(PathBuf),
    /// The project's modules, ordered so that each one's imports are already published,
    /// plus the namespace of the one to invoke.
    Project {
        units: Vec<(String, PathBuf)>,
        root: String,
    },
}

/// Runs `wf run` and returns the process exit code.
pub fn run(args: &RunArgs) -> i32 {
    let planned = match &args.path {
        Some(path) => Planned::Module(path.clone()),
        None => match project_plan() {
            Ok(planned) => planned,
            Err(code) => return code,
        },
    };

    let entry = match entry_point(args) {
        Ok(entry) => entry,
        Err(reason) => {
            eprintln!("wf: {reason}");
            return EXIT_USAGE;
        }
    };

    let runtime = Runtime::default();

    match planned {
        Planned::Module(path) => {
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("wf: cannot read {}: {error}", path.display());
                    return EXIT_USAGE;
                }
            };

            let module = match runtime.compile(&bytes) {
                Ok(module) => module,
                Err(error) => return failure(&error),
            };

            match runtime.run(&module, &entry) {
                Ok(outcome) => report_outcome(&path.display().to_string(), &outcome),
                Err(error) => failure(&error),
            }
        }

        Planned::Project { units, root } => {
            // Every module is loaded, and the order is what makes the set linkable: a
            // provider is instantiated and published under its namespace before the
            // module that imports it.
            let mut compiled = Vec::with_capacity(units.len());
            for (namespace, path) in &units {
                let bytes = match std::fs::read(path) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        eprintln!("wf: cannot read {}: {error}", path.display());
                        return EXIT_USAGE;
                    }
                };
                match runtime.compile(&bytes) {
                    Ok(module) => compiled.push((namespace.clone(), module)),
                    Err(error) => return failure(&error),
                }
            }

            let runtime_units: Vec<Unit> = compiled
                .iter()
                .zip(units.iter())
                .map(|((_, module), (namespace, _))| Unit::new(namespace, module))
                .collect();

            match runtime.run_units(&runtime_units, &root, &entry) {
                Ok(outcome) => report_outcome(&root, &outcome),
                Err(error) => failure(&error),
            }
        }
    }
}

/// Prints the result of a successful invocation and returns the success code.
fn report_outcome(what: &str, outcome: &wf_runtime::RunOutcome) -> i32 {
    println!(
        "wf: ran {} via {} in {}",
        what,
        outcome.entry(),
        format_duration(outcome.duration())
    );
    // A guest returning an i32 is reported, not interpreted: mapping a guest's return
    // value onto a process exit code is a host-ABI decision, and the host ABI does not
    // exist yet. Exiting 0 here is the honest answer for a successful invocation.
    if let Some(value) = outcome.return_value() {
        println!("wf: entry point returned {value}");
    }
    EXIT_OK
}

/// Locates and orders the project's modules.
///
/// The order comes from the same resolution `wf build` validates: an import binds by the
/// manifest's matching rule, and a module is published under its exports before whatever
/// needs it is instantiated.
fn project_plan() -> Result<Planned, i32> {
    if !Path::new(MANIFEST_FILE).exists() {
        eprintln!("wf: no {MANIFEST_FILE} in this directory");
        eprintln!("  help: pass a path to a module, or run `wf init <name>` to create a project");
        return Err(EXIT_USAGE);
    }

    let manifest = match crate::manifest::read(Path::new(MANIFEST_FILE)) {
        Ok(manifest) => manifest,
        Err(diagnostic) => {
            eprintln!("{diagnostic}");
            return Err(EXIT_DIAGNOSTIC);
        }
    };

    let diagnostics = manifest.validate();
    if !diagnostics.is_empty() {
        for diagnostic in &diagnostics {
            eprintln!("{diagnostic}");
        }
        return Err(EXIT_DIAGNOSTIC);
    }

    // What each module imports is read from its published artifact, which is what will
    // be linked, rather than from its source.
    let mut imports = Vec::with_capacity(manifest.modules.len());
    for module in &manifest.modules {
        let path = artifact_of(&module.name);
        if !path.exists() {
            eprintln!("wf: {} has not been built yet", path.display());
            eprintln!("  help: run `wf build` first");
            return Err(EXIT_USAGE);
        }

        let bytes = std::fs::read(&path).map_err(|error| {
            eprintln!("wf: cannot read {}: {error}", path.display());
            EXIT_USAGE
        })?;
        let analysis = wf_wasm::analyze(&bytes).map_err(|error| {
            eprintln!("{error}");
            EXIT_DIAGNOSTIC
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
        imports.push(crate::project::ImportSummary::new(
            module.name.clone(),
            namespaces,
        ));
    }

    let order = crate::project::plan(
        &manifest.modules,
        &imports,
        manifest.project.module_matching,
        &manifest.project.host_namespaces,
    )
    .map_err(|diagnostic| {
        eprintln!("{diagnostic}");
        EXIT_DIAGNOSTIC
    })?;

    // A single-module project needs no choice; several entries do, because picking one
    // would decide which module the user meant.
    let root_index = match order.roots() {
        [only] => *only,
        many => {
            let names = many
                .iter()
                .map(|index| manifest.modules[*index].name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            eprintln!("wf: this project has {} entry modules: {names}", many.len());
            eprintln!("  help: wf run target/<module>.wasm");
            return Err(EXIT_USAGE);
        }
    };

    let units = order
        .positions()
        .iter()
        .filter_map(|index| {
            let module = &manifest.modules[*index];
            let namespace = namespace_of(manifest.project.module_matching, module)?;
            Some((namespace.to_owned(), artifact_of(&module.name)))
        })
        .collect();

    Ok(Planned::Project {
        units,
        root: namespace_of(
            manifest.project.module_matching,
            &manifest.modules[root_index],
        )
        .unwrap_or(&manifest.modules[root_index].name)
        .to_owned(),
    })
}

/// Where a module's published artifact lives.
fn artifact_of(name: &str) -> PathBuf {
    PathBuf::from(TARGET_DIR).join(format!("{name}.wasm"))
}

/// Resolves the entry point from the argument, or the default.
fn entry_point(args: &RunArgs) -> Result<EntryPoint, String> {
    match &args.entry {
        None => Ok(EntryPoint::default()),
        Some(name) => EntryPoint::new(name)
            .map_err(|error| format!("--entry {}: {error}", name.escape_default())),
    }
}

/// Prints a diagnostic and returns the diagnostic exit code.
fn failure(error: &RuntimeError) -> i32 {
    // The code comes from the runtime that produced the cause; building the diagnostic
    // here only decides severity and output.
    let diagnostic = Diagnostic::new(error.code(), Severity::Error, error.to_string());
    eprintln!("{diagnostic}");
    EXIT_DIAGNOSTIC
}

/// Formats a duration at the precision a human wants to read.
fn format_duration(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros < 1_000 {
        format!("{micros}us")
    } else if micros < 1_000_000 {
        format!("{:.1}ms", micros as f64 / 1_000.0)
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every duration the report can produce must be readable at its own scale.
    #[test]
    fn durations_render_at_the_scale_they_need() {
        assert_eq!(format_duration(Duration::from_micros(12)), "12us");
        assert_eq!(format_duration(Duration::from_micros(1_500)), "1.5ms");
        assert_eq!(format_duration(Duration::from_micros(1_500_000)), "1.50s");
    }

    /// The argument-level rejection must not reach the runtime.
    #[test]
    fn an_empty_entry_point_is_rejected_before_reading_the_file() {
        let error = EntryPoint::new("").expect_err("empty is invalid");
        assert!(error.reason().contains("empty"));
    }
}
