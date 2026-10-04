// End-to-end tests for `wf run`, driven through the real binary.
//
// These assert the two things a user depends on: the process exit code, and the
// diagnostic code printed for a failure. Both are documented as stable, so both are
// tested here rather than left to the human eye.

use std::path::PathBuf;
use std::process::{Command, Output};

/// Assembles WAT into a temporary binary module.
fn fixture(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("wf-run-test-{}-{name}.wasm", std::process::id()));
    let bytes = wat::parse_str(source).expect("fixture must assemble");
    std::fs::write(&path, bytes).expect("fixture must be writable");
    path
}

/// Runs `wf` with the given arguments.
fn wf(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(args)
        .output()
        .expect("the wf binary must be runnable")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn runs_a_module_and_exits_zero() {
    let path = fixture("ok", "(module (func (export \"_start\")))");
    let output = wf(&["run", path.to_str().unwrap()]);

    assert!(output.status.success(), "must succeed: {}", stderr(&output));
    let report = stdout(&output);
    assert!(report.contains("_start"), "{report}");
    assert!(report.contains("ran"), "{report}");

    std::fs::remove_file(&path).ok();
}

#[test]
fn reports_the_value_the_entry_point_returns() {
    let path = fixture(
        "ret",
        "(module (func (export \"_start\") (result i32) (i32.const 7)))",
    );
    let output = wf(&["run", path.to_str().unwrap()]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("returned 7"),
        "{}",
        stdout(&output)
    );
    // A guest return value is reported, not interpreted as this process's exit code:
    // that mapping belongs to the host ABI, which does not exist yet.
    assert_eq!(output.status.code(), Some(0));

    std::fs::remove_file(&path).ok();
}

#[test]
fn an_explicit_entry_point_is_used() {
    let path = fixture("main", "(module (func (export \"main\")))");
    let output = wf(&["run", path.to_str().unwrap(), "--entry", "main"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("via main"), "{}", stdout(&output));

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_missing_entry_point_names_itself_as_wf005() {
    let path = fixture("nostart", "(module (func (export \"other\")))");
    let output = wf(&["run", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("WF005"),
        "missing entry point is WF005: {message}"
    );
    assert!(message.contains("_start"), "{message}");

    std::fs::remove_file(&path).ok();
}

#[test]
fn an_unsatisfied_import_is_wf002() {
    let path = fixture(
        "import",
        "(module (import \"env\" \"log\" (func)) (func (export \"_start\")))",
    );
    let output = wf(&["run", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("WF002"),
        "unresolved import is WF002: {message}"
    );
    assert!(
        message.contains("log"),
        "the message must name the import: {message}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_trap_is_wf009_and_names_the_fault() {
    let path = fixture("trap", "(module (func (export \"_start\") unreachable))");
    let output = wf(&["run", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("WF009"),
        "runtime trap is WF009: {message}"
    );
    // The cause must survive: an outer context alone would read as a generic failure.
    assert!(
        message.contains("unreachable"),
        "the fault must be named: {message}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn an_entry_point_that_is_not_a_function_is_wf010() {
    let path = fixture("mementry", "(module (memory (export \"_start\") 1))");
    let output = wf(&["run", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.contains("WF010"), "{message}");
    assert!(
        message.contains("memory"),
        "it must say what it found: {message}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn an_entry_point_with_parameters_is_wf010() {
    let path = fixture("params", "(module (func (export \"_start\") (param i32)))");
    let output = wf(&["run", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.contains("WF010"), "{message}");
    assert!(
        message.contains("(i32)"),
        "the signature must be shown: {message}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn garbage_is_wf001_and_keeps_the_reason() {
    let path = std::env::temp_dir().join(format!("wf-run-test-{}-garbage", std::process::id()));
    std::fs::write(&path, b"definitely not wasm").expect("writable");
    let output = wf(&["run", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("WF001"),
        "invalid wasm is WF001: {message}"
    );
    assert!(
        message.contains("magic header"),
        "the reason must survive: {message}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_missing_file_is_a_usage_error() {
    let output = wf(&["run", "/nonexistent/path/to/module.wasm"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        !stderr(&output).contains("WF"),
        "a bad path is not a diagnostic"
    );
}

#[test]
fn an_empty_entry_point_is_rejected_before_running_anything() {
    let path = fixture("emptyentry", "(module (func (export \"_start\")))");
    let output = wf(&["run", path.to_str().unwrap(), "--entry", ""]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("empty"), "{}", stderr(&output));

    std::fs::remove_file(&path).ok();
}

#[test]
fn every_diagnostic_code_is_a_stable_wfnnn_token() {
    // The codes are a published interface. A future change to how they are printed must
    // not turn them into something a script cannot match.
    let cases = [
        ("(module (func (export \"other\")))", "WF005"),
        ("(module (func (export \"_start\") unreachable))", "WF009"),
        ("(module (memory (export \"_start\") 1))", "WF010"),
    ];

    for (source, expected) in cases {
        let path = fixture("codetoken", source);
        let output = wf(&["run", path.to_str().unwrap()]);
        assert!(
            stderr(&output).starts_with(expected),
            "code must lead the message: {}",
            stderr(&output)
        );
        std::fs::remove_file(&path).ok();
    }
}
