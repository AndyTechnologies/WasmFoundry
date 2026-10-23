// End-to-end tests for the bundle format v1 (plan.md #77), in the order the plan numbers
// them: build a launcher, copy it, add the payload, add the manifest, add the footer,
// execute the bundle, verify the payload is found, corrupt a byte, verify the
// checksum rejects it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use std::os::unix::fs::PermissionsExt;

/// A temporary project holding one precompiled module plus a copied launcher.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("wf-bundle-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("temp dir must be creatable");
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

    /// Runs an arbitrary executable in this directory.
    fn run(&self, path: &Path) -> Output {
        Command::new(path)
            .current_dir(&self.root)
            .output()
            .expect("the bundle must be runnable")
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

/// The dev launcher, built by the test itself if it is not already on disk: step 1 of
/// the plan's POC is "compile a launcher", and it is the TEST that compiles it —
/// `wf bundle` never does (plan #73).
fn launcher_path() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/wf-launcher");
    if !path.is_file() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let built = Command::new("cargo")
            .args(["build", "-p", "wf-bundle", "--bin", "wf-launcher"])
            .current_dir(&workspace)
            .output()
            .expect("cargo must be runnable to build the launcher");
        assert!(
            built.status.success(),
            "the launcher must build: {}",
            String::from_utf8_lossy(&built.stderr)
        );
    }
    path
}

/// Assembles the POC project: manifest + a module returning 7, built.
fn setup_project(project: &Project, name: &str) {
    let wat = r#"(module (func (export "_start") (result i32) (i32.const 7)))"#;
    std::fs::write(
        project.root.join("src/hello.wasm"),
        wat::parse_str(wat).expect("module assembles"),
    )
    .expect("writable");

    let manifest = format!(
        r#"schema = 1

[package]
name = "{name}"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "hello"
source = "src/hello.wasm"
toolchain = "precompiled"
"#
    );
    std::fs::write(project.root.join("wasmfoundry.toml"), manifest).expect("writable");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));
}

#[test]
fn the_bundle_poc_runs_and_rejects_corruption() {
    let launcher = launcher_path();
    if !launcher.is_file() {
        panic!(
            "the POC needs the dev launcher; build it with `cargo build -p wf-bundle --bin wf-launcher`"
        );
    }

    let project = Project::new("poc");
    setup_project(&project, "hello");

    // Steps 1–2 of the plan: compile a launcher (built above), copy it.
    let copied = project.root.join("launcher");
    std::fs::copy(&launcher, &copied).expect("copyable");

    // Steps 3–5: add the payload, the manifest, the footer.
    let bundle = project.wf(&["bundle", "--launcher", &copied.to_string_lossy()]);
    assert!(bundle.status.success(), "{}", stderr(&bundle));
    let bundled_path = project.root.join("target/hello.bundle");
    assert!(bundled_path.is_file(), "{}", stdout(&bundle));
    assert_eq!(
        std::fs::metadata(&bundled_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0o111,
        "the bundle must be executable out of the box"
    );

    // Step 6–7: execute the binario and verify it finds the payload.
    let ran = project.run(&bundled_path);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(
        stdout(&ran).contains("returned 7"),
        "the bundle must run its payload: {}",
        stdout(&ran)
    );

    // Step 8: alter one payload byte.
    let bytes = std::fs::read(&bundled_path).expect("readable");
    let mut corrupt = bytes.clone();
    // The footer is the last 84 bytes; any byte before it is launcher or payload.
    let cut = corrupt.len() - 84 - 1;
    corrupt[cut] ^= 0x01;
    let broken_path = project.root.join("target/hello-corrupt.bundle");
    std::fs::write(&broken_path, &corrupt).expect("writable");
    std::fs::set_permissions(&broken_path, std::fs::Permissions::from_mode(0o755))
        .expect("permissions");

    // Step 9: the checksum fails.
    let failed = project.run(&broken_path);
    assert_ne!(
        failed.status.code(),
        Some(0),
        "corrupted bundle must not run"
    );
    assert!(
        stderr(&failed).contains("checksum"),
        "the failure must name the checksum: {}",
        stderr(&failed)
    );
}

/// The CLI refuses the preconditions instead of leaving them to the format: no
/// manifest, a missing launcher and an unbuilt module are all usage errors.
#[test]
fn bundling_without_its_prerequisites_is_refused() {
    let project = Project::new("prereq");

    // Outside a project the usage error names the manifest first: that is what build
    // and run report too, and the fix (`wf init`) is the same for all three.
    let no_launcher = project.wf(&["bundle", "--launcher", "definitely/missing"]);
    assert!(!no_launcher.status.success());
    assert!(
        stderr(&no_launcher).contains("no wasmfoundry.toml"),
        "{}",
        stderr(&no_launcher)
    );

    let launcher = launcher_path();
    // Now the manifest precondition is met and the launcher check is reached.
    std::fs::write(
        project.root.join("wasmfoundry.toml"),
        r#"schema = 1

[package]
name = "prereq"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "hello"
source = "src/hello.wasm"
toolchain = "precompiled"
"#,
    )
    .expect("writable");

    let missing_launcher = project.wf(&["bundle", "--launcher", "definitely/missing"]);
    assert!(!missing_launcher.status.success());
    assert!(
        stderr(&missing_launcher).contains("cannot read launcher"),
        "{}",
        stderr(&missing_launcher)
    );
    let unbuilt = project.wf(&["bundle", "--launcher", &launcher.to_string_lossy()]);
    assert!(!unbuilt.status.success());
    assert!(
        stderr(&unbuilt).contains("target/hello.wasm"),
        "{}",
        stderr(&unbuilt)
    );
    assert!(
        stderr(&unbuilt).contains("wf build"),
        "{}",
        stderr(&unbuilt)
    );
}

/// A multi-module project is refused with a diagnostic, never silently mis-packaged.
#[test]
fn multi_module_projects_are_refused_in_the_poc() {
    let launcher = launcher_path();
    let project = Project::new("multi");
    std::fs::write(
        project.root.join("src/hello.wasm"),
        wat::parse_str(r#"(module (func (export "_start") (result i32) (i32.const 7)))"#)
            .expect("assembles"),
    )
    .expect("writable");
    std::fs::write(
        project.root.join("wasmfoundry.toml"),
        r#"schema = 1

[package]
name = "multi"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "hello"
source = "src/hello.wasm"
toolchain = "precompiled"

[[module]]
name = "again"
source = "src/hello.wasm"
toolchain = "precompiled"
"#,
    )
    .expect("writable");

    let built = project.wf(&["build"]);
    assert!(built.status.success(), "{}", stderr(&built));

    let refused = project.wf(&["bundle", "--launcher", &launcher.to_string_lossy()]);
    assert!(!refused.status.success());
    assert!(
        stderr(&refused).contains("one module in v1"),
        "{}",
        stderr(&refused)
    );
}

/// --bundle-target resolves a launcher from the local registry (WF_LAUNCHER_REGISTRY env
/// override). The produced bundle executes and returns 7, mirroring the POC test.
#[test]
fn bundle_target_resolves_from_registry() {
    let launcher = launcher_path();
    if !launcher.is_file() {
        panic!(
            "needs the dev launcher; build it with `cargo build -p wf-bundle --bin wf-launcher`"
        );
    }

    let project = Project::new("btarget");
    setup_project(&project, "hello");

    // Set up a fake registry with the dev launcher as the "prebuilt" for our triple.
    let registry_root = project.root.join("fake-registry");
    let triple_dir = registry_root.join("x86_64-unknown-linux-gnu");
    std::fs::create_dir_all(&triple_dir).expect("dir");
    std::fs::copy(&launcher, triple_dir.join("wf-launcher")).expect("copy");

    let _bundle = project.wf(&["bundle", "--bundle-target", "x86_64-unknown-linux-gnu"]);
    let output = std::env::var("WF_LAUNCHER_REGISTRY").ok();

    // Re-run with the env override (spawn a fresh process with the env).
    let bundle = Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(["bundle", "--bundle-target", "x86_64-unknown-linux-gnu"])
        .current_dir(&project.root)
        .env("WF_LAUNCHER_REGISTRY", &registry_root)
        .output()
        .expect("wf must be runnable");

    assert!(
        bundle.status.success(),
        "{}",
        String::from_utf8_lossy(&bundle.stderr)
    );
    let bundled_path = project.root.join("target/hello.bundle");
    assert!(
        bundled_path.is_file(),
        "{}",
        String::from_utf8_lossy(&bundle.stdout)
    );

    // Execute and verify it runs the payload.
    let ran = project.run(&bundled_path);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert!(
        stdout(&ran).contains("returned 7"),
        "the bundle must run its payload: {}",
        stdout(&ran)
    );

    // Restore env if it was set before (we didn't modify it, only the child process env).
    drop(output);
}

/// When --bundle-target names a triple that does not exist in the registry, the CLI
/// refuses with a usage error that names the searched path.
#[test]
fn bundle_target_missing_triple_names_searched_path() {
    let project = Project::new("missing-triple");
    setup_project(&project, "hello");

    let registry_root = project.root.join("empty-registry");
    std::fs::create_dir_all(&registry_root).expect("dir");

    let result = Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(["bundle", "--bundle-target", "aarch64-unknown-linux-gnu"])
        .current_dir(&project.root)
        .env("WF_LAUNCHER_REGISTRY", &registry_root)
        .output()
        .expect("wf must be runnable");

    assert!(!result.status.success());
    let err = stderr(&result);
    assert!(
        err.contains("no launcher found for target"),
        "must name the missing target: {}",
        err
    );
    assert!(
        err.contains("aarch64-unknown-linux-gnu"),
        "error must name the searched path: {}",
        err
    );
}

/// --launcher and --bundle-target given together is refused with a diagnostic.
#[test]
fn launcher_and_bundle_target_mutually_exclusive() {
    let project = Project::new("mutual");
    setup_project(&project, "hello");

    let launcher = launcher_path();

    let refused = Command::new(env!("CARGO_BIN_EXE_wf"))
        .args([
            "bundle",
            "--launcher",
            &launcher.to_string_lossy(),
            "--bundle-target",
            "x86_64-unknown-linux-gnu",
        ])
        .current_dir(&project.root)
        .env("WF_LAUNCHER_REGISTRY", "/tmp/unused")
        .output()
        .expect("wf must be runnable");

    assert!(!refused.status.success());
    assert!(
        stderr(&refused).contains("mutually exclusive"),
        "{}",
        stderr(&refused)
    );
}

/// Neither --launcher nor --bundle-target given is refused.
#[test]
fn neither_launcher_nor_bundle_target_is_refused() {
    let project = Project::new("neither");
    setup_project(&project, "hello");

    let result = project.wf(&["bundle"]);
    assert!(!result.status.success());
    assert!(
        stderr(&result).contains("a launcher source is required"),
        "{}",
        stderr(&result)
    );
}

/// An unsafe triple like "../evil" is refused with a diagnostic.
#[test]
fn unsafe_triple_is_refused() {
    let project = Project::new("unsafe-triple");
    setup_project(&project, "hello");

    let result = Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(["bundle", "--bundle-target", "../evil"])
        .current_dir(&project.root)
        .output()
        .expect("wf must be runnable");

    assert!(!result.status.success());
    assert!(
        stderr(&result).contains("characters unsafe for a path component"),
        "{}",
        stderr(&result)
    );
}
