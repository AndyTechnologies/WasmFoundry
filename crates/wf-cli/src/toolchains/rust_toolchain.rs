//! The Rust toolchain: compile a guest crate to WebAssembly with `cargo`.
//!
//! Everything about *how* cargo is invoked lives here: the flags, the target, the
//! profile, the location of the output. The rest of the system sees a request and a
//! published module, which is what stops cargo's flags from leaking into the build
//! pipeline.

use std::path::{Path, PathBuf};

use wf_core::{Diagnostic, DiagnosticCode, Severity};

use crate::process::CommandSpec;

use super::{CompileRequest, CompileResult, Detection, GUEST_TARGET, Toolchain, publish};

/// The Rust guest toolchain.
#[derive(Debug, Default, Clone, Copy)]
pub struct RustToolchain;

impl RustToolchain {
    /// The crate manifest this toolchain reads.
    fn crate_manifest(request: &CompileRequest<'_>) -> PathBuf {
        request
            .root()
            .join(request.module().source.as_str())
            .join("Cargo.toml")
    }

    /// The file cargo is expected to produce for this crate.
    ///
    /// Cargo names the output after the *crate* name with `-` rewritten to `_`, and a
    /// `[lib] name` overrides the package name. Verified against a real build rather
    /// than assumed: package `my-guest` produces `my_guest.wasm`.
    fn expected_artifact(manifest_path: &Path) -> Result<PathBuf, Diagnostic> {
        let text = std::fs::read_to_string(manifest_path).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!("cannot read {}: {error}", manifest_path.display()),
            )
            .with_help("the module's `source` must point at a crate directory")
        })?;

        let document: toml::Value = toml::from_str(&text).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!("{} is not valid TOML: {error}", manifest_path.display()),
            )
            .with_help("check the crate's Cargo.toml")
        })?;

        let crate_name = document
            .get("lib")
            .and_then(|lib| lib.get("name"))
            .and_then(toml::Value::as_str)
            .or_else(|| {
                document
                    .get("package")
                    .and_then(|package| package.get("name"))
                    .and_then(toml::Value::as_str)
            })
            .ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    format!(
                        "{} declares neither [package] name nor [lib] name",
                        manifest_path.display()
                    ),
                )
                .with_help("every crate needs a package name")
            })?;

        let crate_dir = manifest_path.parent().unwrap_or(Path::new("."));
        Ok(crate_dir
            .join("target")
            .join(GUEST_TARGET)
            .join("release")
            .join(format!("{}.wasm", crate_name.replace('-', "_"))))
    }
}

impl Toolchain for RustToolchain {
    fn id(&self) -> &'static str {
        "rust"
    }

    fn detect(&self, request: &CompileRequest<'_>) -> Detection {
        let manifest = Self::crate_manifest(request);
        if manifest.is_file() {
            Detection::supported()
        } else {
            Detection::unsupported(format!("no Cargo.toml at {}", manifest.display()))
        }
    }

    fn compile(&self, request: &CompileRequest<'_>) -> Result<CompileResult, Diagnostic> {
        let manifest = Self::crate_manifest(request);
        let detection = self.detect(request);
        if !detection.is_supported() {
            return Err(Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!(
                    "module `{}` asks the rust toolchain to build `{}`, but {}",
                    request.module().name,
                    request.module().source,
                    detection.reason().unwrap_or("it cannot handle this source")
                ),
            )
            .with_help("point `source` at the directory containing the guest's Cargo.toml"));
        }

        // The artifact name is known before cargo runs, so a build that produces nothing
        // can be reported as "expected X and found nothing" rather than "build failed".
        let expected = Self::expected_artifact(&manifest)?;

        let spec = CommandSpec::new("cargo")
            .args([
                "build",
                "--release",
                "--target",
                GUEST_TARGET,
                "--manifest-path",
                &manifest.display().to_string(),
            ])
            .current_dir(request.root());

        let output = request.runner().run(&spec).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!("module `{}`: {error}", request.module().name),
            )
            .with_help("install the Rust toolchain, or remove the rust toolchain from this module")
        })?;

        if !output.success() {
            return Err(toolchain_failure(request, &spec, &output, &manifest));
        }

        let bytes = std::fs::read(&expected).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!(
                    "module `{}`: cargo succeeded but {} was not produced: {error}",
                    request.module().name,
                    expected.display()
                ),
            )
            .with_help(
                "the crate must declare `[lib] crate-type = [\"cdylib\"]` so cargo emits a \
                 WebAssembly module",
            )
        })?;

        publish(
            request.root(),
            &request.module().name,
            &bytes,
            spec.display(),
        )
    }
}

/// Turns a failed cargo run into a diagnostic that separates the causes cargo mixes
/// together in one wall of text.
fn toolchain_failure(
    request: &CompileRequest<'_>,
    spec: &CommandSpec,
    output: &crate::process::CommandOutput,
    manifest: &Path,
) -> Diagnostic {
    let stderr = output.stderr().trim();
    // Cargo's last non-empty line is the summary a human reads first. Walking from the
    // back instead of the front stops as soon as it is found.
    let summary = output
        .stdout()
        .lines()
        .chain(output.stderr().lines())
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("cargo failed without reporting why")
        .trim()
        .to_owned();

    let diagnostic = Diagnostic::new(
        DiagnosticCode::ToolchainFailed,
        Severity::Error,
        format!(
            "module `{}`: {} failed (exit {}): {summary}",
            request.module().name,
            spec.display(),
            output
                .status()
                .map_or("signal".to_owned(), |code| code.to_string()),
        ),
    )
    .with_help(format!("full output:\n{stderr}"));

    // The most common failure by far is the guest target never being installed, and
    // cargo reports it as a missing `core` rather than as a missing target.
    if stderr.contains("E0463") || stderr.contains("can't find crate for `core`") {
        return diagnostic.with_help(format!(
            "install the guest target: rustup target add {GUEST_TARGET}"
        ));
    }

    if !manifest.exists() {
        return diagnostic;
    }
    diagnostic
}
#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::process::{CommandOutput, CommandSpec, ProcessError, ProcessRunner};
    use crate::toolchains::{CompileRequest, Toolchain};

    /// A runner that records the command it was given and returns a fixed outcome.
    ///
    /// The reason `ProcessRunner` exists: the exact `cargo` line is this toolchain's
    /// contract, and it must be assertable without a compiler being installed.
    struct RecordingRunner {
        outcome: Result<CommandOutput, ProcessError>,
        recorded: RefCell<Option<CommandSpec>>,
    }

    impl RecordingRunner {
        fn succeed() -> Self {
            RecordingRunner {
                outcome: Ok(CommandOutput::new(Some(0), String::new(), String::new())),
                recorded: RefCell::new(None),
            }
        }

        fn fail(stderr: &str) -> Self {
            RecordingRunner {
                outcome: Ok(CommandOutput::new(Some(101), String::new(), stderr)),
                recorded: RefCell::new(None),
            }
        }
    }

    impl ProcessRunner for RecordingRunner {
        fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, ProcessError> {
            *self.recorded.borrow_mut() = Some(spec.clone());
            match &self.outcome {
                Ok(output) => Ok(output.clone()),
                Err(error) => Err(error.clone()),
            }
        }
    }

    /// A temporary project with a crate at `guest/`.
    fn project(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("wf-rust-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("guest/src")).expect("temp dir");
        std::fs::write(
            root.join("guest/Cargo.toml"),
            "[package]\nname = \"my-guest\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n",
        )
        .expect("writable");
        root
    }

    fn module_for(source: &str) -> wf_core::ModuleSpec {
        wf_core::ModuleSpec {
            name: "app".to_owned(),
            source: source.to_owned(),
            toolchain: "rust".to_owned(),
            namespace: None,
        }
    }

    #[test]
    fn detect_needs_a_crate_manifest() {
        let root = project("detect");
        let module = module_for("guest");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        assert!(RustToolchain.detect(&request).is_supported());

        let missing = module_for("nowhere");
        let request = CompileRequest::new(&missing, &root, &runner);
        let detection = RustToolchain.detect(&request);
        assert!(!detection.is_supported());
        assert!(
            detection.reason().unwrap_or("").contains("Cargo.toml"),
            "{:?}",
            detection.reason()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The command line is the contract: exact program, exact flags, exact order.
    #[test]
    fn compile_invokes_cargo_with_the_guest_target() {
        let root = project("cmd");
        let module = module_for("guest");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        let _ = RustToolchain.compile(&request);
        let spec = runner
            .recorded
            .borrow()
            .clone()
            .expect("cargo must be asked to build");

        assert_eq!(spec.program(), "cargo");
        assert_eq!(spec.arguments()[0], "build");
        assert_eq!(spec.arguments()[1], "--release");
        assert_eq!(spec.arguments()[2], "--target");
        assert_eq!(spec.arguments()[3], GUEST_TARGET);
        assert_eq!(spec.arguments()[4], "--manifest-path");
        assert!(
            spec.arguments()[5].ends_with("guest/Cargo.toml"),
            "{}",
            spec.arguments()[5]
        );
        assert_eq!(spec.cwd(), Some(root.as_path()));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The most common failure is a guest target that was never installed, and cargo
    /// reports it as a missing `core`. The help must translate that.
    #[test]
    fn a_missing_guest_target_is_translated_into_an_installable_hint() {
        let root = project("missing-target");
        let module = module_for("guest");
        let runner = RecordingRunner::fail(
            "error[E0463]: can't find crate for `core`\n  note: using the `rustc` driver",
        );
        let request = CompileRequest::new(&module, &root, &runner);

        let error = RustToolchain.compile(&request).expect_err("cargo failed");
        assert_eq!(error.code(), wf_core::DiagnosticCode::ToolchainFailed);
        let help = error.help().expect("must say how to fix it");
        assert!(
            help.contains("rustup target add wasm32-unknown-unknown"),
            "{help}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failing_cargo_run_reports_what_it_ran_and_why() {
        let root = project("fail");
        let module = module_for("guest");
        let runner = RecordingRunner::fail("error: no targets specified in the manifest");
        let request = CompileRequest::new(&module, &root, &runner);

        let error = RustToolchain.compile(&request).expect_err("cargo failed");
        assert_eq!(error.code(), wf_core::DiagnosticCode::ToolchainFailed);
        let message = error.message();
        assert!(message.contains("cargo build --release"), "{message}");
        assert!(message.contains("no targets specified"), "{message}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A successful run that produced nothing must not be reported as success: the file
    /// was known before the build started, so its absence is diagnosable.
    #[test]
    fn a_build_that_produces_nothing_is_reported() {
        let root = project("no-artifact");
        let module = module_for("guest");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        let error = RustToolchain
            .compile(&request)
            .expect_err("nothing was produced");
        assert_eq!(error.code(), wf_core::DiagnosticCode::ToolchainFailed);
        assert!(
            error.message().contains("my_guest.wasm"),
            "it must name the file it expected: {}",
            error.message()
        );
        let help = error.help().expect("must say how to fix it");
        assert!(help.contains("cdylib"), "{help}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The happy path: cargo succeeded, the artifact exists, and it is published.
    #[test]
    fn a_successful_build_publishes_the_compiled_module() {
        let root = project("ok");
        let module = module_for("guest");
        let runner = RecordingRunner::succeed();

        // The file cargo would have produced, at the path it would have produced it at.
        let artifact_path = root
            .join("guest/target")
            .join(GUEST_TARGET)
            .join("release/my_guest.wasm");
        std::fs::create_dir_all(artifact_path.parent().expect("parent")).expect("dirs");
        let sample = wat::parse_str("(module (func (export \"_start\")))").expect("assembles");
        std::fs::write(&artifact_path, &sample).expect("writable");

        let request = CompileRequest::new(&module, &root, &runner);
        let result = RustToolchain.compile(&request).expect("must build");

        assert_eq!(result.artifact().id().as_str(), "app");
        assert_eq!(result.artifact().kind(), wf_core::ArtifactKind::CoreModule);
        assert_eq!(result.path(), root.join("target/app.wasm"));
        assert!(
            result.detail().contains("--release"),
            "the report must say how it was built: {}",
            result.detail()
        );
        assert_eq!(
            std::fs::read(result.path()).expect("published"),
            sample,
            "publishing must not alter the bytes"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
