//! The `wf bundle` command: wrap a prebuilt native launcher and the project's built
//! module into one self-contained executable.
//!
//! PHASE 14 is the format proof, not the release system: the launcher arrives already
//! built (`plan.md` #73 — this command must never build a launcher), the project has
//! one module, and the POC format is not frozen yet (`plan.md` #77).

use std::os::unix::fs::PermissionsExt;
use std::path::Path; // the bundle must be ready to execute

use wf_bundle::build;

use crate::cli::BundleArgs;
use crate::manifest::MANIFEST_FILE;
use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// Runs `wf bundle` and returns the process exit code.
pub fn run(args: &BundleArgs) -> i32 {
    // Being outside a project is the same usage problem as build/run: no manifest here.
    if !Path::new(MANIFEST_FILE).exists() {
        eprintln!("wf: no {MANIFEST_FILE} in this directory");
        eprintln!("  help: run `wf init <name>` to create a project, then `wf build` first");
        return EXIT_USAGE;
    }

    // The launcher is given, never built here: bundling must not pull a compiler into
    // the pipeline (plan #73).
    let launcher_bytes = match std::fs::read(&args.launcher) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "wf: cannot read launcher {}: {error}",
                args.launcher.display()
            );
            eprintln!(
                "  help: point `--launcher` at a prebuilt launcher binary \
                 (`cargo build -p wf-bundle --bin wf-launcher` during development)"
            );
            return EXIT_USAGE;
        }
    };

    // The manifest is both the project description and the bundle's metadata section:
    // it lands in the bundle verbatim so the launcher (or an inspector) can read what
    // the launcher is about to run.
    let manifest = match crate::manifest::read(Path::new(MANIFEST_FILE)) {
        Ok(manifest) => manifest,
        Err(diagnostic) => {
            eprintln!("{diagnostic}");
            return EXIT_DIAGNOSTIC;
        }
    };
    let metadata_bytes = match std::fs::read(MANIFEST_FILE) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("wf: cannot read {MANIFEST_FILE}: {error}");
            return EXIT_DIAGNOSTIC;
        }
    };

    // The POC ships one module: a multi-module payload needs a container beyond the
    // footer format, and freezing that shape before the POC runs is what the roadmap
    // forbids (plan #77).
    if manifest.modules.len() != 1 {
        eprintln!(
            "wf: `wf bundle` packages one module in this POC, but the project declares {}",
            manifest.modules.len()
        );
        eprintln!("  help: keep a single [[module]] for now, or wait for PHASE 15");
        return EXIT_DIAGNOSTIC;
    }

    let module = &manifest.modules[0];
    let module_path = Path::new("target").join(format!("{}.wasm", module.name));
    let payload_bytes = match std::fs::read(&module_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("wf: cannot read {}: {error}", module_path.display());
            eprintln!("  help: run `wf build` first, then `wf bundle`");
            return EXIT_USAGE;
        }
    };

    let bundled = match build(&launcher_bytes, &payload_bytes, &metadata_bytes) {
        Ok(bundled) => bundled,
        Err(error) => {
            eprintln!("wf: bundling failed: {error}");
            return EXIT_DIAGNOSTIC;
        }
    };

    let output = Path::new("target").join(format!("{}.bundle", manifest.package.name));
    if let Err(error) = std::fs::write(&output, &bundled) {
        eprintln!("wf: cannot write {}: {error}", output.display());
        return EXIT_DIAGNOSTIC;
    }
    // A bundle is meant to be executed; feeding users a chmod recipe is a way of
    // saying "this tool stops half way". The launcher's own mode was copied in the
    // bytes; making the copy executable is part of making it.
    if let Err(error) = std::fs::set_permissions(&output, std::fs::Permissions::from_mode(0o755)) {
        eprintln!("wf: cannot make {} executable: {error}", output.display());
        return EXIT_DIAGNOSTIC;
    }

    println!(
        "wf: bundled {} -> {} (launcher {}, module `{}`)",
        manifest.package.name,
        output.display(),
        format_bytes(launcher_bytes.len()),
        module.name
    );

    EXIT_OK
}

/// Formats a byte count for the report line.
fn format_bytes(count: usize) -> String {
    if count < 1024 {
        format!("{count} B")
    } else {
        format!("{} KiB", count / 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_counts_render_human_repeatedly() {
        assert_eq!(format_bytes(950), "950 B");
        assert_eq!(format_bytes(2048), "2 KiB");
    }
}
