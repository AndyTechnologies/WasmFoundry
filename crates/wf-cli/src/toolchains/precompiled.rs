//! The precompiled toolchain: accept a module as it is.

use std::path::Path;

use wf_core::{Diagnostic, DiagnosticCode, Severity};

use super::{CompileRequest, CompileResult, Detection, Toolchain, publish};

/// Turns an already-compiled `.wasm` into a project artifact by validating it.
///
/// It compiles nothing, which is what makes it the smallest adapter in the system and
/// the one that proves the whole pipeline end to end without any compiler being
/// installed.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrecompiledToolchain;

impl PrecompiledToolchain {
    /// Resolves the declared source against the project root.
    fn source_path(request: &CompileRequest<'_>) -> std::path::PathBuf {
        request.root().join(request.module().source.as_str())
    }
}

impl Toolchain for PrecompiledToolchain {
    fn id(&self) -> &'static str {
        "precompiled"
    }

    fn detect(&self, request: &CompileRequest<'_>) -> Detection {
        let source = Self::source_path(request);
        if !source.exists() {
            return Detection::unsupported(format!(
                "declared source `{}` does not exist",
                request.module().source
            ));
        }
        if !source.is_file() {
            return Detection::unsupported(format!(
                "declared source `{}` is a directory, not a module file",
                request.module().source
            ));
        }
        Detection::supported()
    }

    fn compile(&self, request: &CompileRequest<'_>) -> Result<CompileResult, Diagnostic> {
        let source = Self::source_path(request);

        let bytes = std::fs::read(&source).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!(
                    "module `{}` declares source `{}`, which cannot be read: {error}",
                    request.module().name,
                    request.module().source
                ),
            )
            .with_help("check the `source` path, or copy your compiled WebAssembly module there")
        })?;

        publish(
            request.root(),
            &request.module().name,
            &bytes,
            format!("validated {}", display_short(request.root(), &source)),
        )
    }
}

/// Renders a path relative to the project root when it is inside it.
fn display_short(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => relative.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::StdRunner;
    use crate::toolchains::{CompileRequest, Toolchain};

    /// Builds a request over a temporary project root.
    fn request<'a>(module: &'a wf_core::ModuleSpec, root: &'a Path) -> CompileRequest<'a> {
        CompileRequest::new(module, root, &StdRunner)
    }

    fn module_for(source: &str) -> wf_core::ModuleSpec {
        wf_core::ModuleSpec {
            name: "app".to_owned(),
            source: source.to_owned(),
            toolchain: "precompiled".to_owned(),
        }
    }

    /// The detection reason must name the path, because that is the field to edit.
    #[test]
    fn an_absent_source_is_reported_with_the_path_it_looked_for() {
        let dir = std::env::temp_dir().join(format!("wf-pre-absent-{}", std::process::id()));
        let module = module_for("src/absent.wasm");
        let detection = PrecompiledToolchain.detect(&request(&module, &dir));

        assert!(!detection.is_supported());
        let reason = detection.reason().expect("a reason is the point");
        assert!(reason.contains("src/absent.wasm"), "{reason}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_is_not_a_module() {
        let dir = std::env::temp_dir().join(format!("wf-pre-dir-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).expect("temp dir");
        let module = module_for("src");
        let detection = PrecompiledToolchain.detect(&request(&module, &dir));

        assert!(!detection.is_supported());
        assert!(
            detection.reason().unwrap_or("").contains("directory"),
            "{:?}",
            detection.reason()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file that exists but is not WebAssembly must be caught before it is published.
    #[test]
    fn a_non_wasm_source_is_rejected_before_it_is_published() {
        let dir = std::env::temp_dir().join(format!("wf-pre-bad-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).expect("temp dir");
        std::fs::write(dir.join("src/app.wasm"), b"plain text").expect("writable");

        let module = module_for("src/app.wasm");
        let error = PrecompiledToolchain
            .compile(&request(&module, &dir))
            .expect_err("must not be a module");

        assert_eq!(error.code(), wf_core::DiagnosticCode::InvalidWasm);
        assert!(!dir.join("target/app.wasm").exists(), "must not publish");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Publishing is a copy: what lands in `target/` is what was read.
    #[test]
    fn the_published_artifact_is_byte_identical_to_the_source() {
        let dir = std::env::temp_dir().join(format!("wf-pre-copy-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).expect("temp dir");

        let sample = wat::parse_str("(module (func (export \"_start\")))").expect("assembles");
        std::fs::write(dir.join("src/app.wasm"), &sample).expect("writable");

        let module = module_for("src/app.wasm");
        let result = PrecompiledToolchain
            .compile(&request(&module, &dir))
            .expect("must build");

        assert_eq!(result.artifact().id().as_str(), "app");
        assert_eq!(result.artifact().kind(), wf_core::ArtifactKind::CoreModule);
        assert_eq!(result.path(), dir.join("target/app.wasm"));

        let published = std::fs::read(result.path()).expect("published");
        assert_eq!(published, sample, "publishing must not alter the bytes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The format detected must be the format published, not silently coerced.
    #[test]
    fn a_component_source_publishes_a_component_artifact() {
        let dir = std::env::temp_dir().join(format!("wf-pre-comp-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).expect("temp dir");

        let sample = wat::parse_str("(component)").expect("assembles");
        std::fs::write(dir.join("src/comp.wasm"), sample).expect("writable");

        let mut module = module_for("src/comp.wasm");
        module.name = "comp".to_owned();
        let result = PrecompiledToolchain
            .compile(&request(&module, &dir))
            .expect("must build");

        assert_eq!(result.artifact().kind(), wf_core::ArtifactKind::Component);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
