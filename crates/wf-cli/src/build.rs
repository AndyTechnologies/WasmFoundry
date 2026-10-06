//! The `wf build` command.
//!
//! PHASE 5 builds exactly one way: a module declared with `toolchain = "precompiled"` is
//! read, checked, and published into `target/`. Nothing is compiled, because there is no
//! compiler behind the precompiled toolchain — that is the point of this slice, it proves
//! the whole project pipeline without a guest toolchain being installed.

use std::path::{Path, PathBuf};

use wf_core::{Artifact, ArtifactId, ArtifactKind, Diagnostic, DiagnosticCode, Severity};
use wf_wasm::Analysis;

use crate::cli::BuildArgs;
use crate::manifest::MANIFEST_FILE;
use crate::{EXIT_DIAGNOSTIC, EXIT_OK};

/// The only toolchain this version implements.
///
/// PHASE 6 adds Rust, at which point this single comparison becomes the first real
/// `Toolchain` implementation and the comparison moves behind that trait. Writing the
/// trait now, with one implementation, would draw a boundary nothing has crossed yet.
const PRECOMPILED: &str = "precompiled";

/// Directory published artifacts go into.
const TARGET_DIR: &str = "target";

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

    let manifest_path = Path::new(MANIFEST_FILE);
    let manifest = match crate::manifest::read(manifest_path) {
        Ok(manifest) => manifest,
        Err(diagnostic) => return report(&[diagnostic]),
    };

    let diagnostics = manifest.validate();
    if !diagnostics.is_empty() {
        return report(&diagnostics);
    }

    let mut built = 0;
    for module in &manifest.modules {
        match build_module(module, Path::new(TARGET_DIR)) {
            Ok((artifact, target)) => {
                println!(
                    "wf: built {} ({}) -> {}",
                    artifact.id(),
                    artifact.kind(),
                    target.display()
                );
                built += 1;
            }
            Err(diagnostic) => return report(&[diagnostic]),
        }
    }

    println!("wf: built {built} module(s) into {TARGET_DIR}/");
    EXIT_OK
}

/// Builds one declared module.
///
/// Returns the logical artifact and where its bytes were published. The domain sees the
/// artifact; the path is this layer's business and is only ever printed.
fn build_module(
    module: &wf_core::ModuleSpec,
    target_dir: &Path,
) -> Result<(Artifact, PathBuf), Diagnostic> {
    if module.toolchain != PRECOMPILED {
        return Err(Diagnostic::new(
            DiagnosticCode::MissingToolchain,
            Severity::Error,
            format!(
                "module `{}` asks for toolchain `{}`, which this version does not provide",
                module.name, module.toolchain
            ),
        )
        .with_help(
            "set `toolchain = \"precompiled\"`; other toolchains arrive with later versions",
        ));
    }

    let source = Path::new(&module.source);
    let bytes = std::fs::read(source).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!(
                "module `{}` declares source `{}`, which cannot be read: {error}",
                module.name, module.source
            ),
        )
        .with_help("check the `source` path, or copy your compiled WebAssembly module there")
    })?;

    // The source must actually be a module before it is published. Publishing a file that
    // only pretends to be WebAssembly would defer the failure to the moment someone runs
    // it, which is the worst place to find out.
    let analysis = wf_wasm::analyze(&bytes).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidWasm,
            Severity::Error,
            format!("module `{}`: {error}", module.name),
        )
        .with_help("the file is not a valid WebAssembly module")
    })?;

    let kind = match analysis {
        Analysis::CoreModule(_) => ArtifactKind::CoreModule,
        Analysis::Component(_) => ArtifactKind::Component,
    };

    let id = ArtifactId::new(&module.name).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("module `{}`: {error}", module.name),
        )
        .with_help("give every module a non-empty `name`")
    })?;

    std::fs::create_dir_all(target_dir).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("cannot create {TARGET_DIR}/: {error}"),
        )
        .with_help("check that the working directory is writable")
    })?;

    let target = target_dir.join(format!("{id}.wasm"));
    std::fs::write(&target, &bytes).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("cannot write {}: {error}", target.display()),
        )
        .with_help("check that the working directory is writable")
    })?;

    Ok((Artifact::new(id, kind), target))
}

/// Prints every diagnostic and returns the diagnostic exit code.
fn report(diagnostics: &[Diagnostic]) -> i32 {
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}");
    }
    EXIT_DIAGNOSTIC
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A toolchain this version does not provide must be named as such, and must not be
    /// confused with a missing file.
    #[test]
    fn an_unimplemented_toolchain_is_reported_as_missing() {
        let module = wf_core::ModuleSpec {
            name: "app".to_owned(),
            source: "src/app.wasm".to_owned(),
            toolchain: "rust".to_owned(),
        };
        let diagnostic =
            build_module(&module, Path::new("unused")).expect_err("rust is not implemented yet");
        assert_eq!(diagnostic.code(), DiagnosticCode::MissingToolchain);
        assert!(
            diagnostic.message().contains("rust"),
            "{}",
            diagnostic.message()
        );
    }

    /// The message must separate "not in this version" from "you forgot the file", since
    /// they are fixed by different actions.
    #[test]
    fn an_absent_source_names_the_path_it_could_not_find() {
        let module = wf_core::ModuleSpec {
            name: "app".to_owned(),
            source: "definitely/not/here.wasm".to_owned(),
            toolchain: PRECOMPILED.to_owned(),
        };
        let diagnostic =
            build_module(&module, Path::new("unused")).expect_err("the file is not there");
        assert_eq!(diagnostic.code(), DiagnosticCode::InvalidManifest);
        assert!(
            diagnostic.message().contains("definitely/not/here.wasm"),
            "{}",
            diagnostic.message()
        );
    }

    /// A file that exists but is not WebAssembly must be caught here, not at run time.
    #[test]
    fn a_non_wasm_source_is_rejected_before_it_is_published() {
        let dir = std::env::temp_dir().join(format!("wf-build-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let source = dir.join("notwasm.wasm");
        std::fs::write(&source, b"plain text pretending to be a module").expect("writable");

        let module = wf_core::ModuleSpec {
            name: "app".to_owned(),
            source: source.to_string_lossy().to_string(),
            toolchain: PRECOMPILED.to_owned(),
        };
        let diagnostic = build_module(&module, &dir).expect_err("must not be a module");
        assert_eq!(diagnostic.code(), DiagnosticCode::InvalidWasm);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Building is a copy of validated bytes: what is published must be what was read,
    /// so that a second build with the same input publishes the same artifact.
    #[test]
    fn the_published_artifact_is_byte_identical_to_the_source() {
        let dir = std::env::temp_dir().join(format!("wf-build-copy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");

        let sample = wat::parse_str("(module (func (export \"_start\")))").expect("assembles");
        let source = dir.join("sample.wasm");
        std::fs::write(&source, &sample).expect("writable");

        let module = wf_core::ModuleSpec {
            name: "sample".to_owned(),
            source: source.to_string_lossy().to_string(),
            toolchain: PRECOMPILED.to_owned(),
        };

        let published_dir = dir.join("target");
        let (artifact, target) = build_module(&module, &published_dir).expect("must build");
        assert_eq!(artifact.kind(), ArtifactKind::CoreModule);
        assert_eq!(artifact.id().as_str(), "sample");
        assert_eq!(target, published_dir.join("sample.wasm"));

        let published = std::fs::read(&target).expect("published artifact");
        assert_eq!(published, sample, "publishing must not alter the bytes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The analysis must not lose track of which format it published.
    #[test]
    fn a_component_source_publishes_a_component_artifact() {
        let dir = std::env::temp_dir().join(format!("wf-build-comp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");

        let sample = wat::parse_str("(component)").expect("assembles");
        let source = dir.join("comp.wasm");
        std::fs::write(&source, &sample).expect("writable");

        let module = wf_core::ModuleSpec {
            name: "comp".to_owned(),
            source: source.to_string_lossy().to_string(),
            toolchain: PRECOMPILED.to_owned(),
        };
        let (artifact, target) = build_module(&module, &dir.join("target")).expect("must build");
        assert_eq!(artifact.kind(), ArtifactKind::Component);
        assert!(
            target.starts_with(&dir),
            "the test must stay inside its temp dir"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
