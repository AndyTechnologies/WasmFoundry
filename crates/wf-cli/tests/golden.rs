// Golden tests for the `wf inspect` report.
//
// `wf inspect` has two supported interfaces, human and JSON, and both are contracted
// surfaces: a reader parses them and a tool consumes the JSON. Golden files pin the
// whole rendering so that a change to the format is a reviewed change to a file rather
// than a surprise for whoever is parsing it.
//
// Regenerate after an intentional format change with:
//
//     WF_UPDATE_GOLDEN=1 cargo test -p wf-cli --test golden

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Set this variable to rewrite the golden files.
const UPDATE_ENV: &str = "WF_UPDATE_GOLDEN";

/// The case under test: one fixture and the shapes it must render.
struct Case {
    name: &'static str,
    wat: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "full-sections",
        wat: r#"
            (module
              (import "env" "console_log" (func (param i32 i32)))
              (import "env" "host_memory" (memory 2 16))
              (memory (export "memory") 2 16)
              (table (export "table") 1 10 funcref)
              (global (export "answer") i32 (i32.const 42))
              (func (export "_start") (param i32) (result i32)
                local.get 0
                i32.const 1
                i32.add))"#,
    },
    Case {
        name: "empty-module",
        wat: "(module)",
    },
    Case {
        name: "entry-point",
        wat: r#"
            (module
              (memory (export "memory") 1)
              (func (export "_start")))"#,
    },
    Case {
        name: "component",
        wat: "(component)",
    },
    Case {
        name: "unbounded-memory",
        wat: "(module (memory (export \"memory\") 1))",
    },
];

/// Where a golden file lives.
fn golden_path(name: &str, extension: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("inspect")
        .join(format!("{name}.{extension}"))
}

/// Runs `wf inspect` over assembled bytes.
fn inspect(source: &str, format: &str, target: &Path) -> Output {
    let bytes = wat::parse_str(source).expect("fixture must assemble");
    std::fs::write(target, bytes).expect("fixture must be writable");
    Command::new(env!("CARGO_BIN_EXE_wf"))
        .args(["inspect", &target.display().to_string(), "--format", format])
        .output()
        .expect("the wf binary must be runnable")
}

/// Replaces the fixture path with a stable token.
///
/// The report names the file it read, and that file lives in a per-process temp
/// directory. A golden file compared against the raw path would fail on every machine
/// and every run, so the path is normalised rather than removed from the report: the
/// line keeps saying a module was read, it just does not claim a particular directory.
fn normalize(output: &str, target: &Path) -> String {
    output.replace(
        &format!("Module: {}", target.display()),
        "Module: <module.wasm>",
    )
}

/// Whether this run was asked to rewrite the golden files.
///
/// Read once by the callers rather than inside the assertion, so that a test about the
/// behaviour of a missing golden file cannot be affected by an environment variable set
/// for a different purpose.
fn updating_golden_files() -> bool {
    std::env::var(UPDATE_ENV).is_ok()
}

/// Compares a rendering against its golden file, or rewrites it when asked to.
fn assert_golden(name: &str, extension: &str, actual: &str) {
    assert_golden_inner(name, extension, actual, updating_golden_files())
}

/// The assertion itself, with the update mode passed in explicitly.
fn assert_golden_inner(name: &str, extension: &str, actual: &str, update: bool) {
    let path = golden_path(name, extension);

    if update {
        std::fs::create_dir_all(path.parent().expect("golden file has a directory"))
            .expect("golden directory must be creatable");
        std::fs::write(&path, actual).expect("golden file must be writable");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden file {}.\n\
             Regenerate with: WF_UPDATE_GOLDEN=1 cargo test -p wf-cli --test golden",
            path.display()
        )
    });

    if expected == actual {
        return;
    }

    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let first_difference = expected_lines
        .iter()
        .zip(actual_lines.iter())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| expected_lines.len().min(actual_lines.len()));

    panic!(
        "golden mismatch for `{name}` at line {}.\n\
         --- expected line {} ---\n{}\n\
         --- actual line {} ---\n{}\n\
         If this change to the report is intended, run:\n\
         WF_UPDATE_GOLDEN=1 cargo test -p wf-cli --test golden",
        first_difference + 1,
        first_difference + 1,
        expected_lines
            .get(first_difference)
            .unwrap_or(&"<end of file>"),
        first_difference + 1,
        actual_lines
            .get(first_difference)
            .unwrap_or(&"<end of file>"),
    );
}

#[test]
fn human_report_matches_its_golden_file() {
    for case in CASES {
        let target = temp_file(case.name, "human");
        let output = inspect(case.wat, "human", &target);
        assert!(output.status.success(), "{}: {output:?}", case.name);
        let actual = normalize(&String::from_utf8_lossy(&output.stdout), &target);
        assert_golden(case.name, "txt", &actual);
        let _ = std::fs::remove_file(&target);
    }
}

#[test]
fn json_report_matches_its_golden_file() {
    for case in CASES {
        let target = temp_file(case.name, "json");
        let output = inspect(case.wat, "json", &target);
        assert!(output.status.success(), "{}: {output:?}", case.name);
        let actual = String::from_utf8_lossy(&output.stdout).to_string();

        // The golden file must be valid JSON: a format change that stops being
        // parseable should fail here rather than in a consumer.
        serde_json::from_str::<serde_json::Value>(&actual)
            .unwrap_or_else(|error| panic!("`{}` is no longer valid JSON: {error}", case.name));

        assert_golden(case.name, "json", &actual);
        let _ = std::fs::remove_file(&target);
    }
}

/// The two renderings must describe the same module, section by section.
#[test]
fn the_two_renderings_agree_on_section_counts() {
    for case in CASES {
        let target = temp_file(case.name, "agree");

        let human_output = inspect(case.wat, "human", &target);
        let json_output = inspect(case.wat, "json", &target);
        let human = String::from_utf8_lossy(&human_output.stdout).to_string();
        let json_text = String::from_utf8_lossy(&json_output.stdout).to_string();
        let document: serde_json::Value =
            serde_json::from_str(&json_text).expect("JSON output must parse");

        let is_component = document["kind"] == "component";
        assert_eq!(
            human.contains("Kind: component"),
            is_component,
            "`{}` disagrees about the format",
            case.name
        );

        if !is_component {
            for section in [
                "Imports",
                "Exports",
                "Memories",
                "Tables",
                "Globals",
                "Functions",
            ] {
                let key = section.to_lowercase();
                let reported: usize = document["module"][key.as_str()]
                    .as_array()
                    .map(Vec::len)
                    .unwrap_or(0);
                let heading = format!("{section} ({reported}):");
                assert!(
                    human.contains(&heading),
                    "`{}` reports {heading} in JSON but not in the human report:\n{human}",
                    case.name
                );
            }
        }

        let _ = std::fs::remove_file(&target);
    }
}

/// A missing golden file must fail with instructions, not as a silent pass.
#[test]
fn a_case_without_a_golden_file_fails_loudly() {
    let missing = golden_path("this-case-does-not-exist", "txt");
    if missing.exists() {
        let _ = std::fs::remove_file(&missing);
    }
    let result = std::panic::catch_unwind(|| {
        // `update` is passed explicitly: this test is about the comparison failing, and
        // must hold whether or not the run was asked to rewrite golden files.
        assert_golden_inner("this-case-does-not-exist", "txt", "x", false)
    });
    let message = result.expect_err("an absent golden file must fail");
    let text = message
        .downcast_ref::<String>()
        .map(String::as_str)
        .unwrap_or("non-string panic");
    assert!(text.contains("WF_UPDATE_GOLDEN=1"), "{text}");
}

/// Temporary path for one case.
fn temp_file(name: &str, shape: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "wf-golden-{}-{name}-{shape}.wasm",
        std::process::id()
    ))
}
