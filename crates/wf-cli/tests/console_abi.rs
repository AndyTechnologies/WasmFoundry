// End-to-end tests for the portable console ABI: a guest that prints.
//
// Everything about this path is otherwise unverifiable: the import name, the argument
// order, the encoding and the stream are only proven by a real module calling them and
// something coming out the other end.

use std::path::PathBuf;
use std::process::{Command, Output};

/// A guest that prints "hello from the guest" and then asserts on a false condition.
const PRINTING_GUEST: &str = r#"
    (module
      (import "env" "console.log" (func $log (param i32 i32)))
      (import "env" "console.warn" (func $warn (param i32 i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "hello from the guest")
      (data (i32.const 64) "something is wrong")
      (func (export "_start")
        i32.const 0
        i32.const 20
        call $log
        i32.const 64
        i32.const 19
        call $warn))"#;

/// A guest that asserts on a false condition and keeps running afterwards.
const ASSERTING_GUEST: &str = r#"
    (module
      (import "env" "console.assert" (func $assert (param i32 i32 i32)))
      (import "env" "console.log" (func $log (param i32 i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "the count was never five")
      (func (export "_start")
        i32.const 0
        i32.const 0
        i32.const 24
        call $assert
        i32.const 0
        i32.const 0
        call $log))"#;

/// A temporary project with one precompiled module.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str, wat: &str) -> Self {
        let root = std::env::temp_dir().join(format!("wf-console-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("temp dir");

        std::fs::write(
            root.join(format!("src/{name}.wasm")),
            wat::parse_str(wat).expect("fixture must assemble"),
        )
        .expect("writable");

        let manifest = format!(
            "schema = 1\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n\
             [project]\nsource_dir = \"src\"\nentry = \"_start\"\n\n\
             [[module]]\nname = \"{name}\"\nsource = \"src/{name}.wasm\"\n\
             toolchain = \"precompiled\"\n"
        );
        std::fs::write(root.join("wasmfoundry.toml"), manifest).expect("writable");
        Project { root }
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
fn a_guest_that_prints_reaches_standard_output() {
    let project = Project::new("printing", PRINTING_GUEST);

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(
        stdout(&ran).contains("hello from the guest"),
        "the message the guest passed must be the message that comes out: {}",
        stdout(&ran)
    );
}

#[test]
fn warn_goes_to_standard_error_and_log_to_standard_output() {
    // The two streams are the contract: a caller separates diagnostics from product with
    // the shell, and merging them would make that impossible.
    let project = Project::new("streams", PRINTING_GUEST);

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));

    let output = stdout(&ran);
    let error = stderr(&ran);

    assert!(output.contains("hello from the guest"), "{output}");
    assert!(
        !output.contains("something is wrong"),
        "warn must not be on stdout: {output}"
    );
    assert!(error.contains("something is wrong"), "{error}");
    assert!(
        !error.contains("hello from the guest"),
        "log must not be on stderr: {error}"
    );
}

#[test]
fn a_failed_assertion_reports_and_the_guest_keeps_running() {
    // An assertion is the guest's own statement about its state. It is reported, and it
    // does not stop the program: the log after it still happens.
    let project = Project::new("asserting", ASSERTING_GUEST);

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));

    let error = stderr(&ran);
    assert!(
        error.contains("Assertion failed: the count was never five"),
        "{error}"
    );
    assert!(
        stdout(&ran).is_empty() || !stdout(&ran).contains("Assertion failed"),
        "an assertion is a diagnostic, not product: {}",
        stdout(&ran)
    );
}

#[test]
fn the_import_name_is_the_published_contract() {
    // A guest importing anything other than the documented name cannot link, and the
    // failure names the import that was missing.
    let project = Project::new(
        "wrongname",
        r#"(module
            (import "env" "console.printf" (func $printf (param i32 i32)))
            (memory (export "memory") 1)
            (func (export "_start")
              i32.const 0
              i32.const 0
              call $printf))"#,
    );

    // `env` is the namespace the runtime provides, so the build accepts the import
    // without the manifest declaring it. What must not happen is the guest printing
    // nothing: the failure has to name the function that does not exist.
    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert_eq!(ran.status.code(), Some(1), "{}", stderr(&ran));
    let message = stderr(&ran);
    assert!(message.contains("console.printf"), "{message}");
    assert!(message.contains("WF002"), "{message}");
}
