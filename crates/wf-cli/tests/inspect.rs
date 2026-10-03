// End-to-end tests for `wf inspect`, driven through the real binary.
//
// These invoke the built executable rather than calling the library, because the parts
// most likely to break are the ones a library call skips: argument parsing, output
// formatting and exit codes. The `wat` dev-dependency assembles the fixtures so the
// inputs stay readable in this file instead of living as opaque bytes.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Writes WAT source into a temporary file and assembles it.
fn wasm_fixture(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "wf-inspect-test-{}-{name}.wasm",
        std::process::id()
    ));
    let bytes = wat::parse_str(source).expect("fixture must assemble");
    std::fs::write(&path, bytes).expect("fixture must be writable");
    path
}

/// Writes arbitrary bytes into a temporary file.
fn raw_fixture(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("wf-inspect-test-{}-{name}", std::process::id()));
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

/// Standard output of a run, as text.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Standard error of a run, as text.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// A module with one of every entity the report must show.
const FULL_MODULE: &str = r#"
    (module
      (import "env" "console_log" (func (param i32 i32)))
      (memory (export "memory") 2 16)
      (func (export "_start") (param i32) (result i32)
        local.get 0)
    )
"#;

#[test]
fn reports_a_core_module_with_every_section() {
    let path = wasm_fixture("full", FULL_MODULE);
    let output = wf(&["inspect", path.to_str().unwrap()]);

    assert!(
        output.status.success(),
        "inspect must succeed: {}",
        stderr(&output)
    );
    let report = stdout(&output);

    assert!(report.contains("Kind: core module"), "{report}");
    assert!(report.contains("env.console_log"), "{report}");
    assert!(report.contains("(i32, i32) -> ()"), "{report}");
    assert!(report.contains("_start"), "{report}");
    assert!(report.contains("min 2 pages, max 16 pages"), "{report}");

    std::fs::remove_file(&path).ok();
}

#[test]
fn empty_sections_are_stated_not_hidden() {
    let path = wasm_fixture("empty", "(module)");
    let output = wf(&["inspect", path.to_str().unwrap()]);

    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);

    // A section with no entries must say so. Silently omitting it would let a reader
    // assume there were no imports when the report simply never mentioned them.
    assert!(report.contains("Imports (0):"), "{report}");
    assert!(report.contains("Exports (0):"), "{report}");
    assert!(report.contains("(none)"), "{report}");

    std::fs::remove_file(&path).ok();
}

#[test]
fn json_output_is_valid_json_with_the_reported_entities() {
    let path = wasm_fixture("json", FULL_MODULE);
    let output = wf(&["inspect", path.to_str().unwrap(), "--format", "json"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let document: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("output must be valid JSON");

    assert_eq!(document["kind"], "core-module");
    assert_eq!(document["module"]["imports"][0]["module"], "env");
    assert_eq!(document["module"]["imports"][0]["name"], "console_log");
    assert_eq!(document["module"]["imports"][0]["type"], "function");
    assert_eq!(
        document["module"]["memories"][0]["maximum-pages"], 16,
        "an unbounded limit must be explicit null, not a missing key"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn json_and_human_agree_on_the_same_input() {
    let path = wasm_fixture("agree", FULL_MODULE);

    let human = wf(&["inspect", path.to_str().unwrap()]);
    let json = wf(&["inspect", path.to_str().unwrap(), "--format", "json"]);
    let document: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("valid JSON");

    let human_report = stdout(&human);
    assert_eq!(
        document["module"]["imports"].as_array().unwrap().len(),
        1,
        "the fixture imports only console_log; memory is declared, not imported"
    );
    assert!(
        human_report.contains("Imports (1):"),
        "both renderings must agree on the import count: {human_report}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_non_wasm_file_is_rejected_with_a_diagnostic_code() {
    let path = raw_fixture("notwasm", b"this is definitely not a wasm binary");
    let output = wf(&["inspect", path.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "must fail with the diagnostic exit code"
    );
    let message = stderr(&output);
    assert!(
        message.contains("WF001"),
        "the diagnostic code must be stable: {message}"
    );
    assert!(message.contains("WebAssembly"), "{message}");

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_truncated_module_is_rejected_rather_than_described() {
    let mut bytes = wat::parse_str(FULL_MODULE).expect("fixture assembles");
    bytes.truncate(bytes.len() - 10);
    let path = raw_fixture("truncated", &bytes);
    let output = wf(&["inspect", path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("WF001"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_missing_file_is_a_usage_error_not_a_diagnostic() {
    let output = wf(&["inspect", "/nonexistent/path/to/module.wasm"]);

    // Reading the file is the CLI's job; a bad path is the caller's mistake, not a
    // malformed module, and the two must not share an exit code.
    assert_eq!(output.status.code(), Some(2));
    assert!(!stderr(&output).contains("WF001"));
}

#[test]
fn a_component_is_recognised_and_not_mis_reported() {
    let path = wasm_fixture("component", "(component)");
    let output = wf(&["inspect", path.to_str().unwrap()]);

    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);
    assert!(report.contains("Kind: component"), "{report}");
    assert!(
        report.contains("PHASE 17"),
        "it must say what it does not do: {report}"
    );
    assert!(
        !report.contains("Imports"),
        "a component that was not analysed must not report an empty import section: {report}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn the_help_text_advertises_inspect() {
    let output = wf(&["--help"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("inspect"));
}

#[test]
fn an_unknown_flag_is_rejected() {
    let output = wf(&["inspect", "--not-a-flag"]);
    assert!(
        !output.status.success(),
        "an unknown flag must not be silently accepted"
    );
}

/// Guards the fixture helpers themselves: a test that cannot build its input must fail
/// loudly rather than assert against an empty report.
#[test]
fn fixtures_are_created_where_the_test_expects_them() {
    let path = wasm_fixture("selfcheck", "(module)");
    assert!(path.exists(), "fixture must exist at {}", path.display());
    assert!(Path::new(&path).extension().is_some());
    std::fs::remove_file(&path).ok();
}
