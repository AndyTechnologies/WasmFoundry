//! Toolchains: how a source becomes a module.
//!
//! The trait exists because there are two implementations, not before: `precompiled`
//! arrived in PHASE 5 as a direct path, and `rust` is its second consumer. What crosses
//! the boundary is a request and a result — no cargo flags, no file layout, no compiler
//! concepts. The rest of the system never builds a compiler command itself.

pub mod precompiled;
pub mod rust_toolchain;

use std::path::{Path, PathBuf};

use wf_core::{
    Artifact, ArtifactId, Diagnostic, DiagnosticCode, ModuleSpec, Severity, ToolchainId,
};

use crate::process::ProcessRunner;

pub use precompiled::PrecompiledToolchain;
pub use rust_toolchain::RustToolchain;

/// The guest platform this build targets.
///
/// One value for every toolchain today: PHASE 6 builds core WebAssembly for
/// `wasm32-unknown-unknown`. Guest and host are separate concepts, and a host triple
/// belongs to bundling (PHASE 14), not here.
const GUEST_TARGET: &str = "wasm32-unknown-unknown";

/// What a toolchain is asked to build.
pub struct CompileRequest<'a> {
    module: &'a ModuleSpec,
    root: &'a Path,
    runner: &'a dyn ProcessRunner,
}

impl<'a> CompileRequest<'a> {
    /// Builds a request for one declared module.
    pub fn new(module: &'a ModuleSpec, root: &'a Path, runner: &'a dyn ProcessRunner) -> Self {
        CompileRequest {
            module,
            root,
            runner,
        }
    }

    /// The module being built, as the manifest declared it.
    pub fn module(&self) -> &ModuleSpec {
        self.module
    }

    /// The project root: the directory holding `wasmfoundry.toml`.
    pub fn root(&self) -> &Path {
        self.root
    }

    /// The process runner this build must use.
    pub fn runner(&self) -> &dyn ProcessRunner {
        self.runner
    }
}

/// What a toolchain produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    artifact: Artifact,
    path: PathBuf,
    detail: String,
}

impl CompileResult {
    /// The logical artifact: what it is called and what kind of thing it is.
    pub fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    /// Where its bytes were published.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One line describing how it was built, for the build report.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Debug for CompileRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Hand-written because the runner behind `&dyn ProcessRunner` has no `Debug`
        // bound, and adding one would force every future runner to expose internals.
        f.debug_struct("CompileRequest")
            .field("module", &self.module)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// Whether a toolchain can handle a source, and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    supported: bool,
    reason: Option<String>,
}

impl Detection {
    /// The toolchain can handle this source.
    pub fn supported() -> Self {
        Detection {
            supported: true,
            reason: None,
        }
    }

    /// The toolchain cannot, and this is why.
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Detection {
            supported: false,
            reason: Some(reason.into()),
        }
    }

    /// Whether the source is handled.
    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// The reason, when the source is not handled.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// Turns a source into a published WebAssembly module.
pub trait Toolchain {
    /// The name this toolchain answers to, as a manifest writes it.
    ///
    /// Returns a name rather than a `ToolchainId`: the identity is fixed and non-empty
    /// by construction, so wrapping it would only produce an infallible `Result`.
    fn id(&self) -> &'static str;

    /// Whether this toolchain can handle the source, and if not, why.
    fn detect(&self, request: &CompileRequest<'_>) -> Detection;

    /// Builds the module, publishing it into the project's `target/` directory.
    fn compile(&self, request: &CompileRequest<'_>) -> Result<CompileResult, Diagnostic>;
}

/// The toolchains this version implements.
///
/// A `match` rather than a registry: two entries, both known at compile time, and a
/// table that has to be maintained by hand would be a second place for them to live.
/// Which ids exist is decided here and nowhere else, which is what makes the manifest's
/// `toolchain` field incapable of naming an arbitrary program (threat model, class B).
pub fn lookup(id: &ToolchainId) -> Option<&'static dyn Toolchain> {
    match id.as_str() {
        "precompiled" => Some(&PrecompiledToolchain),
        "rust" => Some(&RustToolchain),
        _ => None,
    }
}

/// Reports a toolchain the manifest names that this version does not implement.
pub fn unknown_toolchain(module: &ModuleSpec, id: &ToolchainId) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::MissingToolchain,
        Severity::Error,
        format!(
            "module `{}` asks for toolchain `{}`, which this version does not provide",
            module.name, id
        ),
    )
    .with_help("supported toolchains: precompiled, rust")
}

/// Publishes validated bytes as a project artifact.
///
/// Shared by every toolchain, because what they disagree about is how the bytes were
/// produced, not what happens next: read, validate, then copy verbatim into `target/`.
/// A toolchain that publishes without validating would defer a bad input to the moment
/// someone runs it.
pub fn publish(
    root: &Path,
    name: &str,
    bytes: &[u8],
    detail: impl Into<String>,
) -> Result<CompileResult, Diagnostic> {
    let analysis = wf_wasm::analyze(bytes).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidWasm,
            Severity::Error,
            format!("module `{name}`: {error}"),
        )
        .with_help("the compiled output is not a valid WebAssembly module")
    })?;

    let kind = match analysis {
        wf_wasm::Analysis::CoreModule(_) => wf_core::ArtifactKind::CoreModule,
        wf_wasm::Analysis::Component(_) => wf_core::ArtifactKind::Component,
    };

    let id = ArtifactId::new(name).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("module `{name}`: {error}"),
        )
        .with_help("give every module a non-empty `name`")
    })?;

    let target_dir = root.join("target");
    std::fs::create_dir_all(&target_dir).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("cannot create {}: {error}", target_dir.display()),
        )
        .with_help("check that the project directory is writable")
    })?;

    let path = target_dir.join(format!("{id}.wasm"));
    std::fs::write(&path, bytes).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("cannot write {}: {error}", path.display()),
        )
        .with_help("check that the project directory is writable")
    })?;

    Ok(CompileResult {
        artifact: Artifact::new(id, kind),
        path,
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dispatch table is what makes a manifest unable to name a program, so every
    /// entry it accepts must actually be implemented, and nothing else may be.
    #[test]
    fn only_the_implemented_toolchains_resolve() {
        for name in ["precompiled", "rust"] {
            let id = ToolchainId::new(name).expect("valid name");
            let toolchain = lookup(&id).expect("implemented");
            assert_eq!(toolchain.id(), name, "id must match the name it answers to");
        }

        for name in ["cpp", "assemblyscript", "cargo", "/bin/sh", "rust"] {
            let id = ToolchainId::new(name).expect("valid name");
            if name == "rust" {
                assert!(lookup(&id).is_some());
                continue;
            }
            assert!(
                lookup(&id).is_none(),
                "`{name}` must not resolve: a manifest naming it must be refused"
            );
        }
    }

    #[test]
    fn an_unknown_toolchain_names_what_it_found_and_what_exists() {
        let module = ModuleSpec {
            name: "app".to_owned(),
            source: "src/app.wasm".to_owned(),
            toolchain: "cpp".to_owned(),
            namespace: None,
        };
        let id = ToolchainId::new(&module.toolchain).expect("valid name");
        let diagnostic = unknown_toolchain(&module, &id);

        assert_eq!(diagnostic.code(), DiagnosticCode::MissingToolchain);
        assert!(
            diagnostic.message().contains("cpp"),
            "{}",
            diagnostic.message()
        );
        assert!(
            diagnostic.message().contains("app"),
            "{}",
            diagnostic.message()
        );
        let help = diagnostic.help().expect("must say what is available");
        assert!(help.contains("precompiled"), "{help}");
        assert!(help.contains("rust"), "{help}");
    }

    /// Publishing must not let an invalid module through, whatever the toolchain.
    #[test]
    fn publishing_validates_before_writing() {
        let dir = std::env::temp_dir().join(format!("wf-publish-{}", std::process::id()));
        let error = publish(&dir, "bad", b"not wasm at all", "detail").expect_err("not a module");

        assert_eq!(error.code(), DiagnosticCode::InvalidWasm);
        assert!(
            !dir.join("target/bad.wasm").exists(),
            "nothing may be published from invalid bytes"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_detection_reports_a_reason_when_it_refuses() {
        let refusal = Detection::unsupported("no Cargo.toml");
        assert!(!refusal.is_supported());
        assert_eq!(refusal.reason(), Some("no Cargo.toml"));

        let acceptance = Detection::supported();
        assert!(acceptance.is_supported());
        assert!(acceptance.reason().is_none());
    }
}
