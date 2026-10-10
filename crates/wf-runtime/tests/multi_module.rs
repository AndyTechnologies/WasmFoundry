// Tests for linking several modules into one execution.
//
// This is the Gate 4 path: module A imports something module B exports, and both run in
// one store with A's import satisfied by B's export — no generated C, no hand-written
// binding.

use wf_core::EntryPoint;
use wf_runtime::{Runtime, RuntimeError, Unit};

/// Assembles WAT into a runnable binary.
fn wasm(source: &str) -> Vec<u8> {
    wat::parse_str(source).expect("fixture must assemble")
}

/// One fixture: the namespace it publishes under, and what it contains.
struct Fixture {
    namespace: &'static str,
    source: &'static str,
}

impl Fixture {
    fn new(namespace: &'static str, source: &'static str) -> Self {
        Fixture { namespace, source }
    }
}

/// Compiles every fixture and runs `root`, in the order the fixtures are listed.
///
/// The order matters and is part of what these tests check, so it comes from the caller
/// rather than being sorted anywhere in the middle.
fn run_set(
    runtime: &Runtime,
    fixtures: &[Fixture],
    root: &str,
    entry: &EntryPoint,
) -> Result<wf_runtime::RunOutcome, RuntimeError> {
    let modules: Vec<_> = fixtures
        .iter()
        .map(|fixture| {
            runtime
                .compile(&wasm(fixture.source))
                .unwrap_or_else(|error| panic!("`{}` must compile: {error}", fixture.namespace))
        })
        .collect();

    let units: Vec<Unit> = fixtures
        .iter()
        .zip(modules.iter())
        .map(|(fixture, module)| Unit::new(fixture.namespace, module))
        .collect();

    runtime.run_units(&units, root, entry)
}

/// A provider exporting `hello`, and a consumer that imports it.
const PROVIDER: &str = r#"
    (module
      (func (export "hello") (result i32) (i32.const 21)))"#;

const CONSUMER: &str = r#"
    (module
      (import "provider" "hello" (func $hello (result i32)))
      (func (export "_start") (result i32)
        call $hello
        i32.const 2
        i32.mul))"#;

/// The pair every test starts from: provider first, because that is the linkable order.
fn pair() -> [Fixture; 2] {
    [
        Fixture::new("provider", PROVIDER),
        Fixture::new("app", CONSUMER),
    ]
}

#[test]
fn an_import_is_satisfied_by_another_module_export() {
    let runtime = Runtime::default();
    let outcome =
        run_set(&runtime, &pair(), "app", &EntryPoint::default()).expect("the import must resolve");

    assert_eq!(
        outcome.return_value(),
        Some(42),
        "the consumer must observe what the provider exported"
    );
}

#[test]
fn the_module_that_needs_another_is_the_one_that_runs() {
    let runtime = Runtime::default();
    let outcome = run_set(&runtime, &pair(), "app", &EntryPoint::default()).expect("runs");

    assert_eq!(outcome.entry().as_str(), "_start");
}

#[test]
fn an_import_without_a_provider_is_still_a_link_error() {
    let runtime = Runtime::default();
    let error = run_set(
        &runtime,
        &[Fixture::new("app", CONSUMER)],
        "app",
        &EntryPoint::default(),
    )
    .expect_err("nothing provides `provider`");

    match &error {
        RuntimeError::Link { message } => assert!(message.contains("provider"), "{message}"),
        other => panic!("expected a link error, got {other:?}"),
    }
    assert_eq!(error.code(), wf_core::DiagnosticCode::UnresolvedImport);
}

#[test]
fn a_provider_ordered_after_its_consumer_cannot_be_linked() {
    // The order is a contract the caller supplies. Supplying the wrong one must fail
    // loudly rather than appear to work for some inputs.
    let runtime = Runtime::default();
    let wrong_order = [
        Fixture::new("app", CONSUMER),
        Fixture::new("provider", PROVIDER),
    ];

    let error = run_set(&runtime, &wrong_order, "app", &EntryPoint::default())
        .expect_err("the provider is not published yet");

    assert!(
        matches!(error, RuntimeError::Link { .. }),
        "expected a link error, got {error:?}"
    );
}

#[test]
fn the_root_can_be_an_inner_module_and_only_that_one_runs() {
    // Running the provider directly must not pull the consumer in: who runs is chosen,
    // not inferred from what happens to be loaded.
    let runtime = Runtime::default();
    let outcome = run_set(
        &runtime,
        &pair(),
        "provider",
        &EntryPoint::new("hello").expect("valid"),
    )
    .expect("runs");

    assert_eq!(outcome.return_value(), Some(21));
}

#[test]
fn a_missing_root_module_is_reported_by_name() {
    let runtime = Runtime::default();
    let error = run_set(
        &runtime,
        &[Fixture::new("provider", PROVIDER)],
        "app",
        &EntryPoint::default(),
    )
    .expect_err("no unit publishes that namespace");

    match &error {
        RuntimeError::MissingExport { name } => assert_eq!(name, "app"),
        other => panic!("expected a missing root, got {other:?}"),
    }
    assert_eq!(error.code(), wf_core::DiagnosticCode::MissingEntrypoint);
}

#[test]
fn an_entry_point_missing_from_the_root_still_names_itself() {
    let runtime = Runtime::default();
    let fixtures = [
        Fixture::new("provider", PROVIDER),
        Fixture::new(
            "app",
            r#"(module
                (import "provider" "hello" (func $hello (result i32)))
                (func (export "main")))"#,
        ),
    ];

    let error = run_set(&runtime, &fixtures, "app", &EntryPoint::default())
        .expect_err("_start does not exist here");

    assert_eq!(error.code(), wf_core::DiagnosticCode::MissingEntrypoint);
    assert!(error.to_string().contains("_start"), "{error}");
}

/// Two units claiming one namespace must not silently override each other.
#[test]
fn publishing_two_modules_under_one_namespace_is_a_link_error() {
    let runtime = Runtime::default();
    let fixtures = [Fixture::new("dup", PROVIDER), Fixture::new("dup", PROVIDER)];

    let error = run_set(
        &runtime,
        &fixtures,
        "dup",
        &EntryPoint::new("hello").expect("valid"),
    )
    .expect_err("the second definition collides");

    assert!(
        matches!(error, RuntimeError::Link { .. }),
        "expected a link error, got {error:?}"
    );
}

/// Loading units must not share state between runs: each execution gets a fresh store.
#[test]
fn two_runs_over_the_same_units_do_not_share_state() {
    let runtime = Runtime::default();
    let first = run_set(&runtime, &pair(), "app", &EntryPoint::default()).expect("first run");
    let second = run_set(&runtime, &pair(), "app", &EntryPoint::default()).expect("second run");

    assert_eq!(first.return_value(), second.return_value());
}
