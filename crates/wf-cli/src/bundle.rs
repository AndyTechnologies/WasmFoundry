//! The `wf bundle` command: wrap a prebuilt native launcher and the project's built
//! module into one self-contained executable.
//!
//! The launcher arrives prebuilt — this command never compiles one (plan #73). Two
//! ways to supply the launcher:
//!
//! 1. `--launcher <path>` — point at a local binary directly.
//! 2. `--bundle-target <triple>` — resolve from the local launcher registry. The
//!    registry is a directory tree; an override env var `WF_LAUNCHER_REGISTRY`
//!    points to the root, otherwise `~/.wf/launchers` is used.

use std::os::unix::fs::PermissionsExt;
use std::path::Path; // the bundle must be ready to execute
use std::path::PathBuf;

use wf_bundle::build;

use crate::cli::BundleArgs;
use crate::manifest::MANIFEST_FILE;
use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// Runs `wf bundle` and returns the process exit code.
pub fn run(args: &BundleArgs) -> i32 {
    // Exactly one of --launcher or --bundle-target must be given.
    let launcher_path: PathBuf = match (&args.launcher, &args.bundle_target) {
        (Some(path), None) => path.clone(),
        (None, Some(triple)) => match resolve_launcher(triple) {
            Ok(path) => path,
            Err(code) => return code,
        },
        (Some(_), Some(_)) => {
            eprintln!("wf: `--launcher` and `--bundle-target` are mutually exclusive");
            eprintln!(
                "  help: provide exactly one of `--launcher <path>` or `--bundle-target <triple>`"
            );
            return EXIT_USAGE;
        }
        (None, None) => {
            eprintln!("wf: a launcher source is required for `wf bundle`");
            eprintln!(
                "  help: use `--launcher <path>` to point at a local binary, \
                 or `--bundle-target <triple>` to resolve from the local registry"
            );
            return EXIT_USAGE;
        }
    };

    // Being outside a project is the same usage problem as build/run: no manifest here.
    if !Path::new(MANIFEST_FILE).exists() {
        eprintln!("wf: no {MANIFEST_FILE} in this directory");
        eprintln!("  help: run `wf init <name>` to create a project, then `wf build` first");
        return EXIT_USAGE;
    }

    let launcher_bytes = match std::fs::read(&launcher_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "wf: cannot read launcher {}: {error}",
                launcher_path.display()
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

    // v1 format packages one module: a multi-module payload needs a container beyond
    // the footer format, and freezing that shape before a real container exists is
    // what the roadmap forbids (plan #77).
    if manifest.modules.len() != 1 {
        eprintln!(
            "wf: `wf bundle` packages one module in v1, but the project declares {}",
            manifest.modules.len()
        );
        eprintln!("  help: keep a single [[module]] for now, or wait for a future format version");
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

/// Resolve a launcher from the local registry by target triple.
///
/// Resolution order:
/// 1. `WF_LAUNCHER_REGISTRY` env var (if set) + `/<triple>/wf-launcher`
/// 2. `~/.wf/launchers/<triple>/wf-launcher`
fn resolve_launcher(triple: &str) -> Result<PathBuf, i32> {
    if !is_safe_path_component(triple) {
        eprintln!("wf: target triple contains characters unsafe for a path component");
        eprintln!("  help: use a standard Rust target triple (e.g. x86_64-unknown-linux-gnu)");
        return Err(EXIT_USAGE);
    }

    let registry_root = std::env::var("WF_LAUNCHER_REGISTRY")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("~"));
            home.join(".wf").join("launchers")
        });

    let launcher_path = registry_root.join(triple).join("wf-launcher");

    if !launcher_path.is_file() {
        eprintln!(
            "wf: no launcher found for target `{triple}` at {}",
            launcher_path.display()
        );
        eprintln!(
            "  help: point `--launcher` at a local binary, or build one for this triple: \
             `cargo build --target {triple} -p wf-bundle --bin wf-launcher`"
        );
        return Err(EXIT_USAGE);
    }

    Ok(launcher_path)
}

/// Check that a target triple contains only characters safe for a filesystem path component.
///
/// Allowed: `[A-Za-z0-9_-.]` — the character set of standard Rust target triples.
fn is_safe_path_component(triple: &str) -> bool {
    !triple.is_empty()
        && triple
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
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

    #[test]
    fn safe_triples_are_accepted() {
        assert!(is_safe_path_component("x86_64-unknown-linux-gnu"));
        assert!(is_safe_path_component("aarch64-apple-darwin"));
        assert!(is_safe_path_component("wasm32-unknown-unknown"));
        assert!(is_safe_path_component("x86_64-pc-windows-msvc"));
        assert!(is_safe_path_component("armv7-linux-androideabi"));
    }

    #[test]
    fn unsafe_triples_are_rejected() {
        assert!(!is_safe_path_component("../evil"));
        assert!(!is_safe_path_component("/etc/passwd"));
        assert!(!is_safe_path_component("a;b"));
        assert!(!is_safe_path_component(""));
    }
}
