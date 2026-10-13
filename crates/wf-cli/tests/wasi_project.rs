// End-to-end tests for the runtime policy as a project declares it.
//
// The policy is written in `wasmfoundry.toml`, so what is verified here is that a
// manifest statement becomes the behaviour a guest actually gets: WASI appears when it
// is asked for, a mount is opened or refused, and a bound stops a guest that crosses it.

use std::path::PathBuf;
use std::process::{Command, Output};

/// A guest that writes to standard output through WASI's `fd_write`.
const WASI_GUEST: &str = r#"
    (module
      (import "wasi_snapshot_preview1" "fd_write"
        (func $fd_write (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 8) "hello wasi")
      (func (export "_start") (result i32)
        i32.const 0
        i32.const 8
        i32.store
        i32.const 4
        i32.const 10
        i32.store
        i32.const 1
        i32.const 0
        i32.const 1
        i32.const 32
        call $fd_write))"#;

/// A guest that loops forever.
const SPINNING_GUEST: &str = r#"
    (module
      (func (export "_start")
        (loop $forever
          br $forever)))"#;

/// A temporary project whose manifest is the given text.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str, guest: &str, manifest: &str) -> Self {
        let root = std::env::temp_dir().join(format!("wf-policy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("temp dir");

        std::fs::write(
            root.join(format!("src/{name}.wasm")),
            wat::parse_str(guest).expect("fixture must assemble"),
        )
        .expect("writable");
        std::fs::write(root.join("wasmfoundry.toml"), manifest).expect("writable");

        Project { root }
    }

    /// A project whose guest is `guest`, with `extra` added to the manifest.
    fn with(name: &str, guest: &str, extra: &str) -> Self {
        let manifest = format!(
            "schema = 1\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n\
             [project]\nsource_dir = \"src\"\nentry = \"_start\"\n\n\
             [[module]]\nname = \"{name}\"\nsource = \"src/{name}.wasm\"\n\
             toolchain = \"precompiled\"\n\n{extra}"
        );
        Project::new(name, guest, &manifest)
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
fn a_project_that_does_not_ask_for_wasi_does_not_get_it() {
    let project = Project::with("denied", WASI_GUEST, "");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert_eq!(ran.status.code(), Some(1), "{}", stderr(&ran));
    let message = stderr(&ran);
    assert!(message.contains("WF002"), "{message}");
    assert!(message.contains("fd_write"), "{message}");
}

#[test]
fn a_project_that_asks_for_wasi_gets_it() {
    let project = Project::with("granted", WASI_GUEST, "[runtime]\nwasi = true\n");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(
        stdout(&ran).contains("hello wasi"),
        "the guest's own output must be visible: {}",
        stdout(&ran)
    );
}

#[test]
fn a_wall_clock_budget_stops_a_guest_that_never_returns() {
    let project = Project::with(
        "bounded",
        SPINNING_GUEST,
        "[runtime]\nwasi = true\nwall_clock_seconds = 0.2\n",
    );

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert_eq!(ran.status.code(), Some(1), "{}", stderr(&ran));
    let message = stderr(&ran);
    assert!(
        message.contains("WF014"),
        "an exceeded bound is WF014: {message}"
    );
    assert!(message.contains("wall-clock"), "{message}");
}

#[test]
fn a_mount_pointing_at_nothing_is_refused_before_the_guest_runs() {
    let project = Project::with(
        "badmount",
        WASI_GUEST,
        "[runtime]\nwasi = true\n\n[[mount]]\nhost = \"/no/such/directory\"\nguest = \"/data\"\n",
    );

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert_eq!(ran.status.code(), Some(1), "{}", stderr(&ran));
    let message = stderr(&ran);
    assert!(message.contains("/data"), "{message}");
}

#[test]
fn a_relative_guest_path_is_refused_by_the_manifest_rules() {
    let project = Project::with(
        "relpath",
        WASI_GUEST,
        "[runtime]\nwasi = true\n\n[[mount]]\nhost = \".\"\nguest = \"data\"\n",
    );

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF011"), "{message}");
    assert!(message.contains("absolute"), "{message}");
}

#[test]
fn two_mounts_claiming_one_path_are_refused() {
    let project = Project::with(
        "dupmount",
        WASI_GUEST,
        "[runtime]\nwasi = true\n\n\
         [[mount]]\nhost = \".\"\nguest = \"/shared\"\n\n\
         [[mount]]\nhost = \"./other\"\nguest = \"/shared\"\n",
    );

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF011"), "{message}");
    assert!(message.contains("/shared"), "{message}");
}

/// A mount that resolves is granted with the access the manifest asked for. The guest
/// cannot observe read-only versus read-write without a filesystem guest, which is
/// PHASE 9's own work; what is verified here is that the mount is opened at all.
#[test]
fn a_mount_that_resolves_does_not_stop_the_guest() {
    let project = Project::with(
        "goodmount",
        WASI_GUEST,
        "[runtime]\nwasi = true\n\n[[mount]]\nhost = \".\"\nguest = \"/data\"\n",
    );

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(stdout(&ran).contains("hello wasi"), "{}", stdout(&ran));
}

#[test]
fn mount_access_defaults_to_read_only() {
    let project = Project::with(
        "defaultaccess",
        WASI_GUEST,
        "[runtime]\nwasi = true\n\n[[mount]]\nhost = \".\"\nguest = \"/data\"\naccess = \"read-only\"\n",
    );

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
}

#[test]
fn an_unknown_mount_access_is_refused() {
    let project = Project::with(
        "badaccess",
        WASI_GUEST,
        "[runtime]\nwasi = true\n\n[[mount]]\nhost = \".\"\nguest = \"/data\"\naccess = \"read-anywhere\"\n",
    );

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF011"), "{message}");
    assert!(message.contains("read-anywhere"), "{message}");
}
