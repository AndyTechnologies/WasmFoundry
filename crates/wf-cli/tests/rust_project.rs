// End-to-end test for the Rust toolchain: Gate 3.
//
// This one invokes the real `cargo` against a real guest crate, because a toolchain test
// that stubs the compiler proves nothing about a toolchain. Everything else in the suite
// checks the command line; this checks that the command line actually builds something.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The fixture guest, copied so the source tree is never written to.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/projects/rust/hello-guest"
);

/// The manifest for a project whose single module is built from that fixture.
const MANIFEST: &str = r#"schema = 1

[package]
name = "hello"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "hello"
source = "guest"
toolchain = "rust"
"#;

/// A temporary project holding a copy of the fixture guest.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("wf-rust-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        let destination = root.join("guest");
        copy_dir(Path::new(FIXTURE), &destination).expect("fixture must be copyable");
        std::fs::write(root.join("wasmfoundry.toml"), MANIFEST).expect("writable");

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

/// Copies a directory tree.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn a_rust_guest_is_compiled_and_then_run() {
    let project = Project::new("gate3");

    let built = project.wf(&["build"]);
    assert!(
        built.status.success(),
        "`wf build` must compile the guest. If this reports `rustup target add`, install \
         the guest target for the active toolchain: rustup target add wasm32-unknown-unknown\n{}",
        stderr(&built)
    );

    let report = stdout(&built);
    assert!(
        report.contains("via rust"),
        "the rust toolchain must be named: {report}"
    );
    assert!(report.contains("target/hello.wasm"), "{report}");

    let artifact = project.root.join("target/hello.wasm");
    assert!(artifact.exists(), "build must publish an artifact");

    // The published file must be a real module, not cargo's raw output renamed: `wf
    // inspect` reads it without executing anything.
    let inspected = project.wf(&["inspect", "target/hello.wasm"]);
    assert!(inspected.status.success(), "{}", stderr(&inspected));
    assert!(
        stdout(&inspected).contains("_start"),
        "{}",
        stdout(&inspected)
    );

    let ran = project.wf(&["run"]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(
        stdout(&ran).contains("returned 7"),
        "the guest's value is the only observable behaviour: {}",
        stdout(&ran)
    );
}

#[test]
fn rebuilding_a_rust_guest_publishes_the_same_bytes() {
    let project = Project::new("rebuild");

    let first = project.wf(&["build"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let first_bytes = std::fs::read(project.root.join("target/hello.wasm")).expect("artifact");

    let second = project.wf(&["build"]);
    assert!(second.status.success(), "{}", stderr(&second));
    let second_bytes = std::fs::read(project.root.join("target/hello.wasm")).expect("artifact");

    assert_eq!(
        first_bytes, second_bytes,
        "the same source must produce the same artifact"
    );
}

#[test]
fn a_source_without_a_crate_manifest_is_refused_before_cargo_runs() {
    let project = Project::new("nocargo");
    // Point the module at a directory that exists but holds no crate.
    let manifest = MANIFEST.replace("source = \"guest\"", "source = \"src\"");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF011"), "a manifest problem: {message}");
    assert!(message.contains("Cargo.toml"), "{message}");
    assert!(!project.root.join("target/hello.wasm").exists());
}

#[test]
fn a_toolchain_this_version_does_not_implement_is_refused_by_name() {
    let project = Project::new("unknown");
    let manifest = MANIFEST.replace("toolchain = \"rust\"", "toolchain = \"fortran\"");
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest).expect("writable");

    let built = project.wf(&["build"]);
    assert_eq!(built.status.code(), Some(1), "{}", stderr(&built));
    let message = stderr(&built);
    assert!(message.contains("WF004"), "{message}");
    assert!(message.contains("`fortran`"), "{message}");
    // The list of what does exist is what makes the message actionable.
    assert!(
        message.contains("precompiled")
            && message.contains("rust")
            && message.contains("cpp")
            && message.contains("assemblyscript"),
        "{message}"
    );
}

/// Building twice with unchanged source should skip recompilation via cache.
#[test]
fn a_second_build_is_cached() {
    let project = Project::new("cached-build");

    let first = project.wf(&["build"]);
    assert!(first.status.success(), "{}", stderr(&first));

    let second = project.wf(&["build"]);
    assert!(second.status.success(), "{}", stderr(&second));
    let output = stdout(&second);
    assert!(
        output.contains("wf: cached hello (unchanged)"),
        "second build should cache: {output}"
    );
    // The cached build should not run cargo again.
    assert!(
        !output.contains("cargo build"),
        "cached build should not invoke cargo: {output}"
    );
}

/// Editing a source file forces a rebuild; rebuilding after edits then caches again.
#[test]
fn editing_source_rebuilds_and_then_caches() {
    let project = Project::new("edit-rebuild");

    let first = project.wf(&["build"]);
    assert!(first.status.success(), "{}", stderr(&first));

    // Append a harmless line to the guest's lib.rs.
    let lib_rs = project.root.join("guest/src/lib.rs");
    let original = std::fs::read_to_string(&lib_rs).expect("lib.rs exists");
    std::fs::write(&lib_rs, format!("{}\n// edited\n", original)).expect("writable");

    let rebuild = project.wf(&["build"]);
    assert!(rebuild.status.success(), "{}", stderr(&rebuild));
    assert!(
        stdout(&rebuild).contains("cargo build"),
        "edit should trigger cargo: {}",
        stdout(&rebuild)
    );

    // Build again — should cache.
    let cached = project.wf(&["build"]);
    assert!(cached.status.success(), "{}", stderr(&cached));
    assert!(
        stdout(&cached).contains("wf: cached hello (unchanged)"),
        "third build should cache: {}",
        stdout(&cached)
    );
}

/// Touching mtime without changing content should not rebuild.
#[test]
fn mtime_only_change_does_not_rebuild() {
    let project = Project::new("mtime-only");

    let first = project.wf(&["build"]);
    assert!(first.status.success(), "{}", stderr(&first));

    // Bump the mtime of the guest's lib.rs without changing content.
    let lib_rs = project.root.join("guest/src/lib.rs");
    let file = std::fs::File::open(&lib_rs).expect("open lib.rs");
    let new_times = std::fs::FileTimes::new()
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600));
    file.set_times(new_times).expect("set times");

    // Small delay to ensure the mtime difference is observable.
    std::thread::sleep(std::time::Duration::from_millis(100));

    let rebuilt = project.wf(&["build"]);
    assert!(rebuilt.status.success(), "{}", stderr(&rebuilt));
    assert!(
        stdout(&rebuilt).contains("wf: cached hello (unchanged)"),
        "mtime-only change should not rebuild: {}",
        stdout(&rebuilt)
    );
}
