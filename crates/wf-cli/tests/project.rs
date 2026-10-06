// End-to-end tests for the project lifecycle: init, build, run.
//
// Each test drives the real binary in a real temporary directory, because the behaviour
// that matters here is the one a user performs: creating files, finding a manifest in the
// working directory and publishing an artifact into `target/`. Library calls would skip
// all of that.

use std::path::PathBuf;
use std::process::{Command, Output};

/// A scratch directory for one test.
struct Project {
    root: PathBuf,
}

impl Project {
    /// Creates an empty directory for this test process.
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("wf-project-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir must be creatable");
        Project { root }
    }

    /// Runs `wf` with these arguments in this directory.
    fn wf(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_wf"))
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("the wf binary must be runnable")
    }

    /// The subdirectory this project was created in.
    fn in_project(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Reads a file relative to this directory.
    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.root.join(relative)).expect("file must exist")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// A project with one module, written directly so a test can vary a single field.
fn manifest(name: &str) -> String {
    format!(
        r#"schema = 1

[package]
name = "{name}"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "{name}"
source = "src/{name}.wasm"
toolchain = "precompiled"
"#
    )
}

/// Writes a module source from WAT text.
fn write_source(project: &Project, file: &str, wat_text: &str) {
    std::fs::create_dir_all(project.root.join("src")).expect("src dir");
    let bytes = wat::parse_str(wat_text).expect("fixture must assemble");
    std::fs::write(project.root.join("src").join(file), bytes).expect("writable");
}

#[test]
fn init_creates_a_project_that_builds_and_runs() {
    let project = Project::new("lifecycle");

    let created = project.wf(&["init", "hello"]);
    assert!(created.status.success(), "{}", stderr(&created));
    assert!(project.root.join("hello/wasmfoundry.toml").exists());
    assert!(project.root.join("hello/src/hello.wasm").exists());

    // Build from inside the project, the way a user does.
    let built = Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(["build"])
        .current_dir(project.in_project("hello"))
        .output()
        .expect("run build");
    assert!(built.status.success(), "{}", stderr(&built));
    assert!(
        stdout(&built).contains("target/hello.wasm"),
        "{}",
        stdout(&built)
    );

    let artifact = project.in_project("hello/target/hello.wasm");
    assert!(artifact.exists(), "build must publish an artifact");

    let ran = Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(["run"])
        .current_dir(project.in_project("hello"))
        .output()
        .expect("run the project");
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(stdout(&ran).contains("via _start"), "{}", stdout(&ran));

    let inspected = Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(["inspect", "target/hello.wasm"])
        .current_dir(project.in_project("hello"))
        .output()
        .expect("inspect the artifact");
    assert!(inspected.status.success(), "{}", stderr(&inspected));
    assert!(
        stdout(&inspected).contains("_start"),
        "{}",
        stdout(&inspected)
    );
}

#[test]
fn init_refuses_to_overwrite_an_existing_project() {
    let project = Project::new("overwrite");

    let first = project.wf(&["init", "hello"]);
    assert!(first.status.success());

    let manifest_before = project.read("hello/wasmfoundry.toml");
    let second = project.wf(&["init", "hello"]);

    assert_eq!(second.status.code(), Some(2), "{}", stderr(&second));
    assert!(
        stderr(&second).contains("already exists"),
        "{}",
        stderr(&second)
    );
    assert_eq!(
        project.read("hello/wasmfoundry.toml"),
        manifest_before,
        "an existing project must not be touched"
    );
}

#[test]
fn the_rendered_manifest_round_trips_through_build() {
    let project = Project::new("roundtrip");
    let created = project.wf(&["init", "roundtrip"]);
    assert!(created.status.success());

    let built = Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(["build"])
        .current_dir(project.in_project("roundtrip"))
        .output()
        .expect("build");
    assert!(
        built.status.success(),
        "the manifest `wf init` writes must be buildable: {}",
        stderr(&built)
    );
}

#[test]
fn build_reports_a_manifest_that_declares_an_absent_source() {
    let project = Project::new("absent");
    std::fs::create_dir_all(project.root.join("src")).expect("src dir");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest("absent")).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(
        message.contains("WF011"),
        "absent source is WF011: {message}"
    );
    assert!(message.contains("src/absent.wasm"), "{message}");
    assert!(
        message.contains("help:"),
        "it must say what to do: {message}"
    );
}

#[test]
fn build_rejects_a_toolchain_this_version_does_not_have() {
    let project = Project::new("rust");
    write_source(&project, "rust.wasm", "(module (func (export \"_start\")))");
    let manifest_text =
        manifest("rust").replace("toolchain = \"precompiled\"", "toolchain = \"rust\"");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest_text).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(
        message.contains("WF004"),
        "missing toolchain is WF004: {message}"
    );
    assert!(message.contains("`rust`"), "{message}");
    // It must not be confused with a missing file.
    assert!(!message.contains("cannot be read"), "{message}");
}

#[test]
fn build_rejects_a_source_that_is_not_webassembly() {
    let project = Project::new("garbage");
    std::fs::create_dir_all(project.root.join("src")).expect("src dir");
    std::fs::write(project.root.join("src/garbage.wasm"), b"not wasm at all").expect("writable");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest("garbage")).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF001"), "{message}");
    assert!(
        !project.root.join("target/garbage.wasm").exists(),
        "a file that is not a module must not be published"
    );
}

#[test]
fn an_unknown_schema_is_refused_before_anything_is_read_from_it() {
    let project = Project::new("schema");
    write_source(
        &project,
        "schema.wasm",
        "(module (func (export \"_start\")))",
    );
    let manifest_text = manifest("schema").replace("schema = 1", "schema = 2");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest_text).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF011"), "{message}");
    assert!(message.contains("schema 2"), "{message}");
    assert!(
        !project.root.join("target").exists(),
        "a manifest this version cannot read must produce nothing"
    );
}

#[test]
fn a_typo_in_a_table_name_is_reported_instead_of_ignored() {
    let project = Project::new("typo");
    write_source(&project, "typo.wasm", "(module (func (export \"_start\")))");
    let manifest_text = manifest("typo").replace("[project]", "[projct]");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest_text).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    assert!(stderr(&built).contains("WF011"), "{}", stderr(&built));
}

#[test]
fn run_without_a_build_tells_the_user_to_build() {
    let project = Project::new("nobuild");
    std::fs::create_dir_all(project.root.join("src")).expect("src dir");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest("nobuild")).expect("writable");

    let ran = project.wf(&["run"]);
    assert_eq!(ran.status.code(), Some(2), "{}", stderr(&ran));
    let message = stderr(&ran);
    assert!(
        message.contains("wf build"),
        "it must say what to run: {message}"
    );
}

#[test]
fn run_without_a_path_or_a_project_is_a_usage_error() {
    let project = Project::new("nothing");

    let ran = project.wf(&["run"]);
    assert_eq!(ran.status.code(), Some(2), "{}", stderr(&ran));
    assert!(
        stderr(&ran).contains("wasmfoundry.toml"),
        "{}",
        stderr(&ran)
    );
}

#[test]
fn build_then_run_reports_a_guest_return_value() {
    let project = Project::new("returns");
    write_source(
        &project,
        "returns.wasm",
        "(module (func (export \"_start\") (result i32) (i32.const 7)))",
    );
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest("returns")).expect("writable");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(stdout(&ran).contains("returned 7"), "{}", stdout(&ran));
}

/// The artifact published twice from the same source must be the same bytes, which is
/// what makes a build reproducible rather than merely repeatable.
#[test]
fn rebuilding_the_same_source_publishes_the_same_bytes() {
    let project = Project::new("reproducible");
    write_source(&project, "same.wasm", "(module (func (export \"_start\")))");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest("same")).expect("writable");

    let first = project.wf(&["build"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let first_bytes = std::fs::read(project.root.join("target/same.wasm")).expect("artifact");

    let second = project.wf(&["build"]);
    assert!(second.status.success(), "{}", stderr(&second));
    let second_bytes = std::fs::read(project.root.join("target/same.wasm")).expect("artifact");

    assert_eq!(
        first_bytes, second_bytes,
        "a rebuild must not change the artifact"
    );
}

/// The manifest lives in the working directory, so `wf build` outside a project must not
/// silently build something else.
#[test]
fn build_outside_a_project_reports_no_manifest() {
    let project = Project::new("elsewhere");
    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(2), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("wasmfoundry.toml"), "{message}");
    assert!(message.contains("help:"), "{message}");
}
