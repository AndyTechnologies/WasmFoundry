// End-to-end tests for a multi-module project: Gate 4.
//
// Two modules, one importing the other, built and run through the real binary. This is
// what the plan asks for under "A → B": no generated C++, no hand-written binding — the
// dependency graph decides the order and the linker resolves the import by name.

use std::path::PathBuf;
use std::process::{Command, Output};

/// The consumer: imports `hello` from the module published under `engine`.
const APP: &str = r#"
    (module
      (import "engine" "hello" (func $hello (result i32)))
      (func (export "_start") (result i32)
        call $hello
        i32.const 2
        i32.mul))"#;

/// The provider: exports `hello`, importing nothing.
const ENGINE: &str = r#"
    (module
      (func (export "hello") (result i32) (i32.const 21)))"#;

/// A temporary project with `app` depending on `engine`.
struct Project {
    root: PathBuf,
}

impl Project {
    /// Writes a project from WAT fixtures.
    fn new(name: &str, modules: &[(&str, &str)], entry: &str) -> Self {
        let root = std::env::temp_dir().join(format!("wf-multi-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("temp dir");

        let mut manifest = format!(
            "schema = 1\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n\
             [project]\nsource_dir = \"src\"\nentry = \"{entry}\"\n\n"
        );
        for (module, source) in modules {
            let bytes = wat::parse_str(source).expect("fixture must assemble");
            std::fs::write(root.join("src").join(format!("{module}.wasm")), bytes)
                .expect("writable");
            manifest.push_str(&format!(
                "[[module]]\nname = \"{module}\"\nsource = \"src/{module}.wasm\"\n\
                 toolchain = \"precompiled\"\n\n"
            ));
        }

        std::fs::write(root.join("wasmfoundry.toml"), manifest).expect("writable");
        Project { root }
    }

    /// A project whose app imports engine, listed engine first to prove the order is
    /// computed rather than inherited from the manifest.
    fn app_with_engine(name: &str) -> Self {
        Self::new(name, &[("app", APP), ("engine", ENGINE)], "_start")
    }

    fn wf(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_wf"))
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("the wf binary must be runnable")
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

#[test]
fn a_module_importing_another_is_built_and_run() {
    let project = Project::app_with_engine("gate4");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));
    assert!(project.root.join("target/app.wasm").exists());
    assert!(project.root.join("target/engine.wasm").exists());

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(
        stdout(&ran).contains("returned 42"),
        "app must observe what engine exports: {}",
        stdout(&ran)
    );
}

#[test]
fn the_import_is_resolved_by_name_and_not_by_position() {
    // The provider is listed second in the manifest, so a build that relied on listing
    // order would publish app first and fail to link it.
    let project = Project::new("order", &[("app", APP), ("engine", ENGINE)], "_start");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(stdout(&ran).contains("returned 42"), "{}", stdout(&ran));
}

#[test]
fn a_cycle_is_refused_by_the_build() {
    let cycle_a = r#"(module
        (import "b" "ping" (func (result i32)))
        (func (export "pong") (result i32) (i32.const 1)))"#;
    let cycle_b = r#"(module
        (import "a" "pong" (func (result i32)))
        (func (export "ping") (result i32) (i32.const 2)))"#;

    let project = Project::new("cycle", &[("a", cycle_a), ("b", cycle_b)], "pong");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF003"), "a cycle is WF003: {message}");
    assert!(message.contains("a") && message.contains("b"), "{message}");
}

#[test]
fn an_import_that_matches_two_modules_is_refused_by_the_build() {
    // Two sources sharing a file stem, in different directories: both claim the
    // namespace `engine`. Binding the import would pick whichever the manifest listed
    // first, so the build refuses instead.
    let project = Project::new("ambiguous", &[("app", APP)], "_start");

    std::fs::create_dir_all(project.root.join("other")).expect("temp dir");
    std::fs::write(
        project.root.join("src/engine.wasm"),
        wat::parse_str(ENGINE).expect("fixture must assemble"),
    )
    .expect("writable");
    std::fs::write(
        project.root.join("other/engine.wasm"),
        wat::parse_str(ENGINE).expect("fixture must assemble"),
    )
    .expect("writable");

    let manifest = "schema = 1\n\n[package]\nname = \"ambiguous\"\nversion = \"0.1.0\"\n\n\
         [project]\nsource_dir = \"src\"\nentry = \"_start\"\n\n\
         [[module]]\nname = \"app\"\nsource = \"src/app.wasm\"\ntoolchain = \"precompiled\"\n\n\
         [[module]]\nname = \"engine\"\nsource = \"src/engine.wasm\"\ntoolchain = \"precompiled\"\n\n\
         [[module]]\nname = \"spare\"\nsource = \"other/engine.wasm\"\ntoolchain = \"precompiled\"\n";
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(
        message.contains("WF002"),
        "an ambiguous import is WF002: {message}"
    );
    assert!(message.contains("engine"), "{message}");
    assert!(
        message.contains("spare") && message.contains("engine"),
        "{message}"
    );
    assert!(
        message.contains("help:"),
        "it must say how to fix it: {message}"
    );
}

#[test]
fn an_import_with_no_provider_fails_at_run_time_not_at_build() {
    // An import that matches nothing in the project is external — a host ABI namespace,
    // or one supplied later. Building it must work; running it must say what is missing.
    let orphan = r#"(module
        (import "host" "give" (func (result i32)))
        (func (export "_start") (result i32)
          call 0))"#;

    let project = Project::new("orphan", &[("app", orphan)], "_start");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert_eq!(ran.status.code(), Some(1), "{}", stderr(&ran));
    let message = stderr(&ran);
    assert!(
        message.contains("WF002"),
        "an unsatisfied import is WF002: {message}"
    );
    assert!(message.contains("host"), "{message}");
}

#[test]
fn a_project_with_two_entries_asks_which_module_to_run() {
    let lonely_a = r#"(module (func (export "_start") (result i32) (i32.const 1)))"#;
    let lonely_b = r#"(module (func (export "_start") (result i32) (i32.const 2)))"#;

    let project = Project::new(
        "two-roots",
        &[("alpha", lonely_a), ("beta", lonely_b)],
        "_start",
    );

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert_eq!(ran.status.code(), Some(2), "{}", stderr(&ran));
    let message = stderr(&ran);
    assert!(
        message.contains("alpha") && message.contains("beta"),
        "{message}"
    );
    assert!(
        message.contains("wf run"),
        "it must say how to choose: {message}"
    );
}

#[test]
fn a_single_module_project_still_runs_without_any_choice() {
    let lonely = r#"(module (func (export "_start") (result i32) (i32.const 9)))"#;
    let project = Project::new("single", &[("only", lonely)], "_start");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(stdout(&ran).contains("returned 9"), "{}", stdout(&ran));
}

/// The manifest can state how namespaces are matched, and an unrecognised value is
/// refused rather than ignored.
#[test]
fn module_matching_is_configurable_and_validated() {
    let project = Project::app_with_engine("matching");

    let manifest =
        std::fs::read_to_string(project.root.join("wasmfoundry.toml")).expect("readable");
    let configured = manifest.replace(
        "entry = \"_start\"",
        "entry = \"_start\"\nmodule_matching = \"file-name\"",
    );
    std::fs::write(project.root.join("wasmfoundry.toml"), configured).expect("writable");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let manifest =
        std::fs::read_to_string(project.root.join("wasmfoundry.toml")).expect("readable");
    let broken = manifest.replace(
        "module_matching = \"file-name\"",
        "module_matching = \"whatever\"",
    );
    std::fs::write(project.root.join("wasmfoundry.toml"), broken).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF011"), "{message}");
    assert!(message.contains("file-name"), "{message}");
}
