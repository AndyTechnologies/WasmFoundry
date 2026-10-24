//! The Component toolchain: compile a guest component with `cargo component`.
//!
//! Everything about *how* cargo component is invoked lives here: the flags, the
// WIT file validation, the output location. The rest of the system sees a
//! request and a published module, which is what stops cargo component's flags
//! from leaking into the build pipeline.

use std::path::{Path, PathBuf};

use wf_core::{Diagnostic, DiagnosticCode, Severity};

use crate::process::CommandSpec;

use super::{CompileRequest, CompileResult, Detection, Toolchain, publish};

/// The Component guest toolchain.
#[derive(Debug, Default, Clone, Copy)]
pub struct ComponentToolchain;

impl ComponentToolchain {
    /// The directory containing the component's WIT files and Cargo.toml.
    fn component_root(request: &CompileRequest<'_>) -> PathBuf {
        request.root().join(request.module().source.as_str())
    }

    /// The WIT file for this component (assumes a single .wit file in the root).
    fn wit_file(root: &Path) -> Result<PathBuf, Diagnostic> {
        let entries = std::fs::read_dir(root).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!("cannot read component root {}: {error}", root.display()),
            )
            .with_help("check that the component source directory exists")
        })?;

        for entry in entries {
            let entry = entry.map_err(|error| {
                Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    format!("cannot read directory entry: {error}"),
                )
            })?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wit") {
                return Ok(path);
            }
        }

        Err(Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("no .wit file found in {}", root.display()),
        )
        .with_help("place a .wit file in the component's source directory"))
    }

    /// The file cargo component is expected to produce for this module.
    fn expected_artifact(request: &CompileRequest<'_>) -> PathBuf {
        request
            .root()
            .join("target")
            .join("wasm32-wasip1")
            .join("release")
            .join(format!("{}.wasm", request.module().name))
    }
}

impl Toolchain for ComponentToolchain {
    fn id(&self) -> &'static str {
        "component"
    }

    fn detect(&self, request: &CompileRequest<'_>) -> Detection {
        let root = Self::component_root(request);
        if !root.exists() {
            return Detection::unsupported(format!(
                "declared source `{}` does not exist",
                request.module().source
            ));
        }
        if !root.is_dir() {
            return Detection::unsupported(format!(
                "declared source `{}` is not a directory",
                request.module().source
            ));
        }
        // Check for Cargo.toml
        let cargo_toml = root.join("Cargo.toml");
        if !cargo_toml.is_file() {
            return Detection::unsupported(format!("no Cargo.toml at {}", cargo_toml.display()));
        }
        // Check for at least one .wit file
        if Self::wit_file(&root).is_err() {
            return Detection::unsupported(format!("no .wit file found in {}", root.display()));
        }
        Detection::supported()
    }

    fn cache_key(&self) -> String {
        "cargo component build --release --target wasm32-wasip1".to_owned()
    }

    fn source_paths(&self, request: &CompileRequest<'_>) -> Vec<PathBuf> {
        let root = Self::component_root(request);
        let mut paths = vec![root.join("Cargo.toml")];
        if let Ok(wit) = Self::wit_file(&root) {
            paths.push(wit);
        }
        if let Ok(entries) = std::fs::read_dir(root.join("src")) {
            for entry in entries.flatten() {
                paths.push(entry.path());
            }
        }
        paths
    }

    fn compile(&self, request: &CompileRequest<'_>) -> Result<CompileResult, Diagnostic> {
        let root = Self::component_root(request);
        let detection = self.detect(request);
        if !detection.is_supported() {
            return Err(Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!(
                    "module `{}` asks the component toolchain to build `{}`, but {}",
                    request.module().name,
                    request.module().source,
                    detection.reason().unwrap_or("it cannot handle this source")
                ),
            )
            .with_help("point `source` at a directory containing Cargo.toml and a .wit file"));
        }

        let wit_path = Self::wit_file(&root)?;

        // Run WIT validation first
        let wit_check = CommandSpec::new("wasm-tools")
            .args(["wit", "validate", &wit_path.display().to_string()])
            .current_dir(&root);

        let output =
            request.runner().run(&wit_check).map_err(|e| {
                Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!("module `{}`: WIT validation failed: {e}", request.module().name),
            )
            .with_help(
                "install wasm-tools (wasm-tools wit validate), or check the WIT file for errors",
            )
            })?;

        if !output.success() {
            return Err(Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!("module `{}`: WIT validation failed", request.module().name),
            )
            .with_help("fix the WIT file and try again"));
        }

        // The artifact cargo component produces
        let output_path = Self::expected_artifact(request);

        // Ensure the target directory exists
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    format!("cannot create {}: {error}", parent.display()),
                )
                .with_help("check that the project directory is writable")
            })?;
        }

        // Compile with cargo component
        let spec = CommandSpec::new("cargo")
            .args([
                "component".to_string(),
                "build".to_string(),
                "--release".to_string(),
                "--target".to_string(),
                "wasm32-wasip1".to_string(),
            ])
            .current_dir(&root);

        let cargo_output = request.runner().run(&spec).map_err(|e| {
            Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!("module `{}`: cargo component failed: {e}", request.module().name),
            )
            .with_help(
                "install cargo component (cargo install cargo-component), or check the component for errors",
            )
        })?;

        if !cargo_output.success() {
            return Err(Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!("module `{}`: cargo component failed", request.module().name),
            )
            .with_help("check the component's Cargo.toml and WIT files for errors"));
        }

        // The component artifact is produced in the target directory
        let artifact_name = format!("{}.wasm", request.module().name);
        let actual_path = root
            .join("target")
            .join("wasm32-wasip1")
            .join("release")
            .join(&artifact_name);

        // If the expected path doesn't match, check the actual cargo component output
        let final_path = if actual_path.is_file() {
            actual_path
        } else if output_path.is_file() {
            output_path
        } else {
            return Err(Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!(
                    "module `{}`: cargo component succeeded but {} was not produced",
                    request.module().name,
                    actual_path.display()
                ),
            )
            .with_help("the component must be valid Rust for the wasm32-wasip1 target"));
        };

        // Use the cargo component output as the published artifact
        // We also need to serialize the component bytes for the module
        let module_bytes = std::fs::read(&final_path).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!(
                    "module `{}`: could not read artifact {}: {error}",
                    request.module().name,
                    final_path.display()
                ),
            )
            .with_help("the component must be valid Rust for the wasm32-wasip1 target")
        })?;

        let spec = CommandSpec::new("cargo")
            .args(vec![
                "component".to_string(),
                "build".to_string(),
                "--release".to_string(),
                "--target".to_string(),
                "wasm32-wasip1".to_string(),
            ])
            .current_dir(&root);

        publish(
            request.root(),
            &request.module().name,
            &module_bytes,
            spec.display(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{CommandOutput, CommandSpec, ProcessRunner};
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;

    fn project(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("wf-component-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("component")).expect("temp dir");
        root
    }

    fn module_for(source: &str) -> wf_core::ModuleSpec {
        wf_core::ModuleSpec {
            name: "hello".to_owned(),
            source: source.to_owned(),
            toolchain: "component".to_owned(),
            namespace: None,
        }
    }

    struct RecordingRunner {
        recorded: RefCell<Option<CommandSpec>>,
    }

    impl RecordingRunner {
        fn succeed() -> Self {
            RecordingRunner {
                recorded: RefCell::new(None),
            }
        }

        #[allow(dead_code)]
        fn fail(_output: impl Into<String>) -> Self {
            RecordingRunner {
                recorded: RefCell::new(Some(CommandSpec::new("cargo").args([
                    "component",
                    "build",
                    "--release",
                ]))),
            }
        }
    }

    impl ProcessRunner for RecordingRunner {
        fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, crate::process::ProcessError> {
            *self.recorded.borrow_mut() = Some(spec.clone());
            Ok(CommandOutput::new(Some(0), String::new(), String::new()))
        }
    }

    #[test]
    fn detect_needs_a_cargo_toml() {
        let root = project("detect");
        let module = module_for("component");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        let detection = ComponentToolchain.detect(&request);
        assert!(!detection.is_supported());
    }

    #[test]
    fn detect_needs_a_wit_file() {
        let root = project("detect-wit");
        fs::create_dir_all(root.join("component")).expect("dir");
        fs::write(
            root.join("component/Cargo.toml"),
            r#"
[package]
name = "hello"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
"#,
        )
        .expect("writable");
        let module = module_for("component");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        let detection = ComponentToolchain.detect(&request);
        assert!(!detection.is_supported());
    }

    #[test]
    fn detect_passes_with_cargo_and_wit() {
        let root = project("detect-ok");
        fs::create_dir_all(root.join("component")).expect("dir");
        fs::write(
            root.join("component/Cargo.toml"),
            r#"
[package]
name = "hello"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
"#,
        )
        .expect("writable");
        fs::write(
            root.join("component/hello.wit"),
            r#"
package example:hello;
world hello-world {}
"#,
        )
        .expect("writable");
        let module = module_for("component");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        let detection = ComponentToolchain.detect(&request);
        assert!(detection.is_supported());
    }
}
