// Tests for running a core module with Wasmtime.
//
// Each case asserts *why* something happened, not just that it failed: a trap, an
// unsatisfied import and a missing entry point look identical to a caller who only sees
// an error string, and telling them apart is the whole point of the typed error.
//
// Nothing in this file names a `wasmtime` type. The runtime owns its engine, so the
// engine — and every other Wasmtime type — stays behind this crate's boundary.

use wf_core::EntryPoint;
use wf_runtime::{Runtime, RuntimeError};

/// Assembles WAT into a runnable binary.
fn module(source: &str) -> Vec<u8> {
    wat::parse_str(source).expect("fixture must assemble")
}

/// Compiles and runs a module through the default entry point.
fn run_default(source: &str) -> Result<wf_runtime::RunOutcome, RuntimeError> {
    let runtime = Runtime::default();
    let compiled = runtime.compile(&module(source))?;
    runtime.run(&compiled, &EntryPoint::default())
}

#[test]
fn runs_a_module_that_exports_start() {
    let outcome = run_default("(module (func (export \"_start\")))").expect("must run");
    assert_eq!(outcome.entry().as_str(), "_start");
    assert_eq!(outcome.return_value(), None, "a void entry returns nothing");
}

#[test]
fn runs_a_module_that_returns_a_value() {
    let outcome = run_default("(module (func (export \"_start\") (result i32) (i32.const 7)))")
        .expect("must run");
    assert_eq!(outcome.return_value(), Some(7));
}

#[test]
fn runs_an_explicit_entry_point() {
    let runtime = Runtime::default();
    let compiled = runtime
        .compile(&module("(module (func (export \"main\")))"))
        .expect("compile");
    let outcome = runtime
        .run(&compiled, &EntryPoint::new("main").expect("valid"))
        .expect("must run");
    assert_eq!(outcome.entry().as_str(), "main");
}

#[test]
fn a_missing_entry_point_is_reported_as_missing() {
    let error = run_default("(module (func (export \"other\")))").expect_err("must fail");
    match &error {
        RuntimeError::MissingExport { name } => assert_eq!(name, "_start"),
        other => panic!("expected a missing export, got {other:?}"),
    }
    assert!(
        error.to_string().contains("_start"),
        "the message must name the export: {error}"
    );
}

#[test]
fn an_entry_point_that_is_not_a_function_is_reported_as_a_configuration_problem() {
    // The export exists, so "missing" would be wrong: the guest exported a memory under
    // that name, and saying so is the difference between a diagnosable module and a
    // confusing one.
    let error = run_default("(module (memory (export \"_start\") 1))").expect_err("must fail");
    match error {
        RuntimeError::Configuration { message } => {
            assert!(
                message.contains("_start"),
                "must name the export: {message}"
            );
            assert!(
                message.contains("memory"),
                "must say what it found: {message}"
            );
        }
        other => panic!("expected a configuration error, got {other:?}"),
    }
}

#[test]
fn an_entry_point_with_parameters_is_rejected_rather_than_guessed_at() {
    let error =
        run_default("(module (func (export \"_start\") (param i32)))").expect_err("must fail");
    match error {
        RuntimeError::Configuration { message } => {
            assert!(
                message.contains("(i32)"),
                "the message must show the signature: {message}"
            );
        }
        other => panic!("expected a configuration error, got {other:?}"),
    }
}

#[test]
fn an_unsatisfied_import_is_a_link_error() {
    // No host ABI exists yet, so any import is unsatisfiable. The cause must be named as
    // a link failure, not as a generic execution failure.
    let error = run_default(
        "(module
            (import \"env\" \"log\" (func))
            (func (export \"_start\")))",
    )
    .expect_err("must fail");
    match error {
        RuntimeError::Link { message } => {
            assert!(
                message.contains("env") || message.contains("log"),
                "{message}"
            );
        }
        other => panic!("expected a link error, got {other:?}"),
    }
}

#[test]
fn a_trap_during_execution_is_reported_as_a_trap() {
    let error =
        run_default("(module (func (export \"_start\") unreachable))").expect_err("must fail");
    match error {
        RuntimeError::Trap { message } => {
            assert!(!message.is_empty(), "a trap must say something");
        }
        other => panic!("expected a trap, got {other:?}"),
    }
}

#[test]
fn a_division_by_zero_traps_and_names_the_operation() {
    let error = run_default(
        "(module
            (func (export \"_start\") (result i32)
              i32.const 1
              i32.const 0
              i32.div_s))",
    )
    .expect_err("must fail");
    match error {
        RuntimeError::Trap { message } => assert!(message.contains("divide"), "{message}"),
        other => panic!("expected a trap, got {other:?}"),
    }
}

#[test]
fn garbage_bytes_are_reported_as_invalid_wasm() {
    let runtime = Runtime::default();
    let error = runtime
        .compile(b"this is not a module at all")
        .expect_err("must fail");
    match &error {
        // The outermost context says only "failed to parse"; the reason must survive.
        RuntimeError::InvalidWasm { message } => {
            assert!(
                message.contains("magic header"),
                "reason was lost: {message}"
            )
        }
        other => panic!("expected invalid wasm, got {other:?}"),
    }
}

#[test]
fn every_runtime_error_explains_itself() {
    // Nothing downstream should fall back to a bare "execution failed": the display of
    // each cause must name the cause.
    for error in [
        RuntimeError::InvalidWasm {
            message: "m".into(),
        },
        RuntimeError::Link {
            message: "m".into(),
        },
        RuntimeError::MissingExport {
            name: "_start".into(),
        },
        RuntimeError::Trap {
            message: "m".into(),
        },
        RuntimeError::Configuration {
            message: "m".into(),
        },
    ] {
        let text = error.to_string();
        assert!(!text.is_empty());
        assert_ne!(text, "execution failed");
    }
}

#[test]
fn two_threads_can_run_the_same_module_without_sharing_a_store() {
    // If a Store were shared, concurrent runs would race on the same execution state.
    // A fresh Store per invocation is what makes this hold. Each thread borrows the same
    // runtime and the same compiled module, so what is being shared is configuration —
    // never execution state.
    let runtime = Runtime::default();
    let compiled = runtime
        .compile(&module("(module (func (export \"_start\")))"))
        .expect("compile");
    let entry = EntryPoint::default();

    let mut failures = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let runtime = &runtime;
                let compiled = &compiled;
                let entry = &entry;
                scope.spawn(move || runtime.run(compiled, entry))
            })
            .collect();

        for handle in handles {
            match handle.join() {
                Ok(Ok(outcome)) => assert_eq!(outcome.entry(), &entry),
                Ok(Err(error)) => failures.push(error),
                Err(_) => panic!("thread panicked"),
            }
        }
    });

    assert!(
        failures.is_empty(),
        "concurrent runs must all succeed: {failures:?}"
    );
}

#[test]
fn running_the_same_module_twice_does_not_carry_state_over() {
    let runtime = Runtime::default();
    let compiled = runtime
        .compile(&module("(module (func (export \"_start\")))"))
        .expect("compile");
    let entry = EntryPoint::default();

    let first = runtime.run(&compiled, &entry).expect("first run");
    let second = runtime.run(&compiled, &entry).expect("second run");

    assert_eq!(first.return_value(), second.return_value());
    assert_eq!(first.entry(), second.entry());
}
