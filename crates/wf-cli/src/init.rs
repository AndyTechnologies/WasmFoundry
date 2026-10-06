//! The `wf init` command.
//!
//! It creates the smallest project WasmFoundry can build: a manifest, a source directory
//! and one precompiled module. Nothing is compiled here — the precompiled toolchain
//! accepts an already-built module, which is what makes this the first vertical slice
//! that needs no guest toolchain installed.

use std::path::{Path, PathBuf};

use crate::cli::InitArgs;
use crate::manifest::MANIFEST_FILE;
use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// The module `wf init` scaffolds: an empty entry point with nothing to do.
///
/// Written as text rather than checked in as bytes, so that a reviewer reads what the
/// scaffold contains instead of trusting an opaque blob.
const SAMPLE_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "_start")))
"#;

/// Runs `wf init` and returns the process exit code.
pub fn run(args: &InitArgs) -> i32 {
    let root = PathBuf::from(&args.name);

    if root.exists() {
        eprintln!(
            "wf: {} already exists, refusing to overwrite it",
            root.display()
        );
        return EXIT_USAGE;
    }

    let manifest = render_manifest(&args.name, &args.name);
    let sample = match wat::parse_str(SAMPLE_WAT) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("wf: cannot assemble the sample module: {error}");
            return EXIT_DIAGNOSTIC;
        }
    };

    if let Err(error) = create_project(&root, &manifest, &sample, &args.name) {
        eprintln!("wf: cannot create project: {error}");
        return EXIT_DIAGNOSTIC;
    }

    println!("wf: created project `{}` in {}", args.name, root.display());
    println!("  {MANIFEST_FILE}");
    println!(
        "  src/{}.wasm  (sample module, replace it with yours)",
        args.name
    );
    println!();
    println!("Next: cd {} && wf build", args.name);
    EXIT_OK
}

/// Creates the project directory, its manifest and its source directory.
///
/// `package` is what the module file is named, and it is the same name the manifest
/// declares, so the scaffold and the manifest cannot drift apart.
fn create_project(
    root: &Path,
    manifest: &str,
    sample: &[u8],
    package: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join(MANIFEST_FILE), manifest)?;

    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir)?;

    // Named after the package so that `wf build` produces `target/<package>.wasm`,
    // which is the file the acceptance criteria inspect.
    std::fs::write(source_dir.join(format!("{package}.wasm")), sample)?;
    Ok(())
}

/// Renders the manifest a fresh project starts with.
///
/// The module is named after the package so that a one-module project builds to
/// `target/<package>.wasm` without extra configuration.
fn render_manifest(name: &str, package: &str) -> String {
    format!(
        r#"schema = 1

[package]
name = "{package}"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "{name}"
source = "src/{package}.wasm"
toolchain = "precompiled"
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered manifest must parse back into a valid domain manifest, otherwise
    /// `wf init` would create something `wf build` refuses.
    #[test]
    fn the_rendered_manifest_round_trips() {
        let rendered = render_manifest("hello", "hello");
        let manifest = crate::manifest::parse(&rendered).expect("init writes valid syntax");
        assert_eq!(manifest.schema, 1);
        assert_eq!(manifest.package.name, "hello");
        assert_eq!(manifest.modules.len(), 1);
        assert_eq!(manifest.modules[0].source, "src/hello.wasm");
        assert!(manifest.validate().is_empty(), "{:?}", manifest.validate());
    }

    /// The scaffold must not overwrite an existing directory.
    #[test]
    fn an_existing_directory_is_reported_before_anything_is_written() {
        let dir = std::env::temp_dir().join(format!("wf-init-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");

        let args = crate::cli::InitArgs {
            name: dir.to_string_lossy().to_string(),
        };
        let code = run(&args);
        assert_eq!(code, EXIT_USAGE);

        let manifest = dir.join(MANIFEST_FILE);
        assert!(
            !manifest.exists(),
            "nothing must be written when the target exists"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sample_module_is_a_real_module() {
        let bytes = wat::parse_str(SAMPLE_WAT).expect("sample must assemble");
        let analysis = wf_wasm::analyze(&bytes).expect("sample must be analysable");
        let core = analysis.core().expect("sample must be a core module");
        assert!(
            core.exports()
                .iter()
                .any(|export| export.name() == "_start"),
            "the scaffold must export an entry point"
        );
    }
}
