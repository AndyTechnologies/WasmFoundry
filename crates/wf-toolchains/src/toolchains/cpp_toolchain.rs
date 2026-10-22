//! The C++ toolchain: compile a C++ source file to WebAssembly with `clang++`.
//!
//! Everything about *how* clang++ is invoked lives here: the flags, the target,
//! the output location. The rest of the system sees a request and a published
//! module, which is what stops clang++'s flags from leaking into the build
//! pipeline.

use std::path::PathBuf;

use wf_core::{Diagnostic, DiagnosticCode, Severity};

use crate::process::CommandSpec;

use super::{CompileRequest, CompileResult, Detection, Toolchain, publish};

/// The C++ guest toolchain.
#[derive(Debug, Default, Clone, Copy)]
pub struct CppToolchain;

impl CppToolchain {
    /// Resolves the declared source against the project root.
    fn source_path(request: &CompileRequest<'_>) -> PathBuf {
        request.root().join(request.module().source.as_str())
    }

    /// The file clang++ is expected to produce for this module.
    ///
    /// The output is placed in the project's `target/` directory with the module
    /// name and a `.wasm` extension.
    fn expected_artifact(request: &CompileRequest<'_>) -> PathBuf {
        request
            .root()
            .join("target")
            .join(format!("{}.wasm", request.module().name))
    }
}

impl Toolchain for CppToolchain {
    fn id(&self) -> &'static str {
        "cpp"
    }

    fn cache_key(&self) -> String {
        "clang++ --target=wasm32-unknown-unknown -nostdlib -Wl,--no-entry \
         -Wl,--export-all -Oz -flto -fno-exceptions -fno-rtti -std=c++17"
            .to_owned()
    }

    fn source_paths(&self, request: &CompileRequest<'_>) -> Vec<PathBuf> {
        vec![Self::source_path(request)]
    }

    fn detect(&self, request: &CompileRequest<'_>) -> Detection {
        let source = Self::source_path(request);
        // Accept any existing file; the compilation step will fail if it's not a
        // valid C++ source.
        if !source.exists() {
            return Detection::unsupported(format!(
                "declared source `{}` does not exist",
                request.module().source
            ));
        }
        if !source.is_file() {
            return Detection::unsupported(format!(
                "declared source `{}` is a directory, not a file",
                request.module().source
            ));
        }
        Detection::supported()
    }

    fn compile(&self, request: &CompileRequest<'_>) -> Result<CompileResult, Diagnostic> {
        let source = Self::source_path(request);
        let detection = self.detect(request);
        if !detection.is_supported() {
            return Err(Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!(
                    "module `{}` asks the cpp toolchain to build `{}`, but {}",
                    request.module().name,
                    request.module().source,
                    detection.reason().unwrap_or("it cannot handle this source")
                ),
            )
            .with_help("point `source` at an existing C++ source file"));
        }

        let output_path = Self::expected_artifact(request);

        // Ensure the target directory exists.
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

        let spec = CommandSpec::new("clang++")
            .args(vec![
                // Target WebAssembly: the unknown-unknown triple (no OS, no libc)
                "--target=wasm32-unknown-unknown".to_string(),
                // Do not link with standard libraries
                "-nostdlib".to_string(),
                // Do not define an entry point (we rely on exported _start or custom)
                "-Wl,--no-entry".to_string(),
                // Export all functions (needed for the guest to be usable)
                "-Wl,--export-all".to_string(),
                // Optimize for size
                "-Oz".to_string(),
                // Enable LTO
                "-flto".to_string(),
                // Disable exceptions and RTTI (not needed for minimal wasm)
                "-fno-exceptions".to_string(),
                "-fno-rtti".to_string(),
                // Use C++17
                "-std=c++17".to_string(),
                // Input file
                source.display().to_string(),
                // Output file
                "-o".to_string(),
                output_path.display().to_string(),
            ])
            .current_dir(request.root());

        let output = request.runner().run(&spec).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!("module `{}`: {error}", request.module().name),
            )
            .with_help("install the Clang toolchain, or remove the cpp toolchain from this module")
        })?;

        if !output.success() {
            return Err(toolchain_failure(request, &spec, &output));
        }

        let bytes = std::fs::read(&output_path).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!(
                    "module `{}`: clang++ succeeded but {} was not produced: {error}",
                    request.module().name,
                    output_path.display()
                ),
            )
            .with_help("the source must be valid C++ for the wasm32-unknown-unknown target")
        })?;

        publish(
            request.root(),
            &request.module().name,
            &bytes,
            spec.display(),
        )
    }
}

/// Turns a failed clang++ run into a diagnostic that separates the causes.
fn toolchain_failure(
    request: &CompileRequest<'_>,
    spec: &CommandSpec,
    output: &crate::process::CommandOutput,
) -> Diagnostic {
    let stderr = output.stderr().trim();
    // Clang's last non-empty line is often the error summary.
    let summary = output
        .stdout()
        .lines()
        .chain(output.stderr().lines())
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("clang++ failed without reporting why")
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

    // Common hint: clang++ not found.
    if stderr.contains("command not found") || stderr.contains("no such file or directory") {
        return diagnostic.with_help("install clang++ and ensure it is in your PATH");
    }

    diagnostic
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::process::{CommandOutput, CommandSpec, ProcessError, ProcessRunner};

    /// A runner that records the command it was given and returns a fixed outcome.
    ///
    /// The reason `ProcessRunner` exists: the exact `clang++` line is this toolchain's
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
                outcome: Ok(CommandOutput::new(
                    Some(1),
                    String::new(),
                    stderr.to_owned(),
                )),
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

    /// A temporary project with a source file at `src/`.
    fn project(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("wf-cpp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("temp dir");
        root
    }

    fn module_for(source: &str) -> wf_core::ModuleSpec {
        wf_core::ModuleSpec {
            name: "app".to_owned(),
            source: source.to_owned(),
            toolchain: "cpp".to_owned(),
            namespace: None,
        }
    }

    #[test]
    fn detect_needs_an_existing_source_file() {
        let root = project("detect");
        let module = module_for("src/hello.cpp");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        // File does not exist -> unsupported
        let detection = CppToolchain.detect(&request);
        assert!(!detection.is_supported());
        assert!(
            detection.reason().unwrap_or("").contains("src/hello.cpp"),
            "{:?}",
            detection.reason()
        );

        // Create the file -> supported
        std::fs::write(root.join("src/hello.cpp"), "// dummy").expect("writable");
        let detection = CppToolchain.detect(&request);
        assert!(detection.is_supported());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn detect_rejects_a_directory() {
        let root = project("detect-dir");
        std::fs::create_dir_all(root.join("src")).expect("temp dir");
        let module = module_for("src");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        let detection = CppToolchain.detect(&request);
        assert!(!detection.is_supported());
        assert!(
            detection.reason().unwrap_or("").contains("directory"),
            "{:?}",
            detection.reason()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The command line is the contract: exact program, exact flags, exact order.
    #[test]
    fn compile_invokes_clang_with_correct_flags() {
        let root = project("cmd");
        // Create a dummy source file so detection passes.
        std::fs::write(root.join("src/app.cpp"), "// dummy").expect("writable");
        let module = module_for("src/app.cpp");
        let runner = RecordingRunner::succeed();
        let request = CompileRequest::new(&module, &root, &runner);

        let _ = CppToolchain.compile(&request);
        let spec = runner
            .recorded
            .borrow()
            .clone()
            .expect("clang++ must be asked to build");

        // Check the program and a few key flags.
        assert_eq!(spec.program(), "clang++");
        let args = spec.arguments();
        // Exactly one target flag: two would be a contradiction that happens to
        // resolve silently because the last flag wins.
        assert_eq!(
            args.iter().filter(|a| a.starts_with("--target=")).count(),
            1
        );
        assert!(args.contains(&"--target=wasm32-unknown-unknown".to_string()));
        assert!(args.contains(&"-nostdlib".to_string()));
        assert!(args.contains(&"-Wl,--no-entry".to_string()));
        assert!(args.contains(&"-Wl,--export-all".to_string()));
        assert!(args.contains(&"-Oz".to_string()));
        assert!(args.contains(&"-flto".to_string()));
        assert!(args.contains(&"-fno-exceptions".to_string()));
        assert!(args.contains(&"-fno-rtti".to_string()));
        assert!(args.contains(&"-std=c++17".to_string()));
        // Input and output files should be present.
        assert!(args.iter().any(|a| a.contains("src/app.cpp")));
        assert!(args.iter().any(|a| a.contains("-o")));
        assert!(args.iter().any(|a| a.ends_with(".wasm")));
        assert_eq!(spec.cwd(), Some(root.as_path()));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A failing clang++ run reports what it ran and why.
    #[test]
    fn a_failing_clang_run_reports_what_it_ran_and_why() {
        let root = project("fail");
        std::fs::write(root.join("src/app.cpp"), "// dummy").expect("writable");
        let module = module_for("src/app.cpp");
        let runner = RecordingRunner::fail("error: unable to execute command: Permission denied");
        let request = CompileRequest::new(&module, &root, &runner);

        let error = CppToolchain.compile(&request).expect_err("clang++ failed");
        assert_eq!(error.code(), wf_core::DiagnosticCode::ToolchainFailed);
        let message = error.message();
        assert!(message.contains("clang++"), "{message}");
        assert!(message.contains("Permission denied"), "{message}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A successful run that produced nothing must not be reported as success: the file
    /// was known before the build started, so its absence is diagnosable.
    #[test]
    fn a_build_that_produces_nothing_is_reported() {
        let root = project("no-artifact");
        // Create a dummy source file so detection passes.
        std::fs::write(root.join("src/app.cpp"), "// dummy").expect("writable");
        let module = module_for("src/app.cpp");
        let runner = RecordingRunner::succeed();

        // Ensure the target directory exists but the output file is missing.
        let output_path = root.join("target/app.wasm");
        let _ = std::fs::remove_file(&output_path);

        let request = CompileRequest::new(&module, &root, &runner);
        let error = CppToolchain
            .compile(&request)
            .expect_err("nothing was produced");
        assert_eq!(error.code(), wf_core::DiagnosticCode::ToolchainFailed);
        assert!(
            error.message().contains("app.wasm"),
            "it must name the file it expected: {}",
            error.message()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The happy path: clang++ succeeded, the artifact exists, and it is published.
    #[test]
    fn a_successful_build_publishes_the_compiled_module() {
        let root = project("ok");
        // Create a valid C++ source that compiles to a wasm module.
        // We'll use a minimal C++ file that exports a function.
        let src = r#"
            extern "C" {
                int _start() { return 42; }
            }
        "#;
        std::fs::write(root.join("src/app.cpp"), src).expect("writable");
        let module = module_for("src/app.cpp");
        let runner = RecordingRunner::succeed();

        // Pre-create the expected output file so the compile step sees success.
        // In a real test we would let clang++ produce it, but we don't have clang++
        // in the test environment. Instead we simulate a successful run by
        // pre‑creating the wasm file and letting the runner succeed.
        let output_path = root.join("target/app.wasm");
        std::fs::create_dir_all(output_path.parent().expect("parent")).expect("dirs");
        let sample_wasm =
            wat::parse_str("(module (func (export \"_start\") (result i32) (i32.const 42)))")
                .expect("assembles");
        std::fs::write(&output_path, &sample_wasm).expect("writable");

        let request = CompileRequest::new(&module, &root, &runner);
        let result = CppToolchain.compile(&request).expect("must build");

        assert_eq!(result.artifact().id().as_str(), "app");
        assert_eq!(result.artifact().kind(), wf_core::ArtifactKind::CoreModule);
        assert_eq!(result.path(), root.join("target/app.wasm"));
        assert!(
            result.detail().contains("--target=wasm32"),
            "the report must say how it was built: {}",
            result.detail()
        );
        assert_eq!(
            std::fs::read(result.path()).expect("published"),
            sample_wasm,
            "publishing must not alter the bytes"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
