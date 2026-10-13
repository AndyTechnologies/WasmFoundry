// Tests for the runtime policy: WASI, bounds, and what each failure is called.
//
// The policy is the product's security statement, so these assert what a guest can reach
// and what happens when it exceeds a bound — not only that the policy type compiles.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use wf_core::{EntryPoint, ExecutionLimits, FilesystemAccess, Mount, RuntimePolicy};
use wf_runtime::{Limit, Runtime, RuntimeError};

fn wasm(source: &str) -> Vec<u8> {
    wat::parse_str(source).expect("fixture must assemble")
}

/// A guest that writes "hello wasi" to standard output through WASI.
const WASI_GUEST: &str = r#"
    (module
      (import "wasi_snapshot_preview1" "fd_write"
        (func $fd_write (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 8) "hello wasi")
      (func (export "_start") (result i32)
        ;; an iovec at 0: pointer to the text, then its length
        i32.const 0
        i32.const 8
        i32.store
        i32.const 4
        i32.const 10
        i32.store
        ;; fd 1, one iovec, and somewhere for the count to land
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

/// A guest that keeps asking for more memory.
const GROWING_GUEST: &str = r#"
    (module
      (memory (export "memory") 1)
      (func (export "_start")
        (loop $forever
          i32.const 1
          memory.grow
          drop
          br $forever)))"#;

#[test]
fn wasi_is_absent_unless_the_policy_grants_it() {
    // The default grants nothing, so the guest's WASI import has no provider at all.
    let runtime = Runtime::default();
    let module = runtime.compile(&wasm(WASI_GUEST)).expect("compiles");

    let error = runtime
        .run(&module, &EntryPoint::default())
        .expect_err("nothing provides WASI");

    assert!(
        matches!(error, RuntimeError::Link { .. }),
        "expected a link error, got {error:?}"
    );
    assert!(error.to_string().contains("fd_write"), "{error}");
}

#[test]
fn a_granted_wasi_guest_reaches_the_host_standard_output() {
    let policy = RuntimePolicy::wasi(Vec::new(), ExecutionLimits::unlimited());
    let runtime = Runtime::with_policy(Vec::new(), policy);

    let module = runtime.compile(&wasm(WASI_GUEST)).expect("compiles");
    let outcome = runtime
        .run(&module, &EntryPoint::default())
        .expect("WASI is granted, so fd_write resolves");

    // `fd_write` returns an errno; zero is success.
    assert_eq!(outcome.return_value(), Some(0));
}

#[test]
fn a_spinning_guest_is_stopped_by_its_wall_clock_budget() {
    let policy = RuntimePolicy::wasi(
        Vec::new(),
        ExecutionLimits::bounded(Some(Duration::from_millis(120)), None),
    );
    let runtime = Runtime::with_policy(Vec::new(), policy);

    let module = runtime.compile(&wasm(SPINNING_GUEST)).expect("compiles");
    let error = runtime
        .run(&module, &EntryPoint::default())
        .expect_err("the guest never yields");

    match &error {
        RuntimeError::Limit { limit, message } => {
            assert_eq!(
                *limit,
                Limit::WallClock,
                "a spinning guest is out of time: {message}"
            );
            assert!(message.contains("0.12"), "{message}");
        }
        other => panic!("expected a wall-clock limit, got {other:?}"),
    }
    assert_eq!(error.code(), wf_core::DiagnosticCode::LimitExceeded);
}

#[test]
fn a_guest_without_a_wall_clock_budget_is_not_interrupted() {
    // The absence of a bound is itself part of the contract, so it is asserted rather
    // than assumed. The guest spins; the test notices that it is still spinning.
    let runtime = Runtime::default();
    let module = runtime.compile(&wasm(SPINNING_GUEST)).expect("compiles");

    let running = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&running);
    let handle = std::thread::spawn(move || {
        let outcome = runtime.run(&module, &EntryPoint::default());
        observed.store(true, Ordering::SeqCst);
        outcome
    });

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        !running.load(Ordering::SeqCst),
        "with no budget set, nothing may stop the guest"
    );

    // The thread keeps spinning and the test binary ends it. Growing the guest's memory
    // without a ceiling instead would risk the machine's memory, which is not a risk a
    // test should be taking.
    drop(handle);
}

#[test]
fn a_memory_ceiling_stops_a_guest_that_asks_for_too_much() {
    let policy = RuntimePolicy::wasi(Vec::new(), ExecutionLimits::bounded(None, Some(2)));
    let runtime = Runtime::with_policy(Vec::new(), policy);

    let module = runtime.compile(&wasm(GROWING_GUEST)).expect("compiles");
    let error = runtime
        .run(&module, &EntryPoint::default())
        .expect_err("the guest grows past two pages");

    match &error {
        RuntimeError::Limit { limit, message } => {
            assert_eq!(*limit, Limit::Memory, "{message}");
            assert!(message.contains("memory"), "{message}");
        }
        other => panic!("expected a memory limit, got {other:?}"),
    }
    assert_eq!(error.code(), wf_core::DiagnosticCode::LimitExceeded);
}

/// A mount pointing at a directory that does not exist fails before the guest runs, and
/// says which path could not be opened.
#[test]
fn a_mount_that_cannot_be_opened_says_which_path() {
    let policy = RuntimePolicy::wasi(
        vec![Mount::new(
            "/no/such/directory/for/wasmfoundry/tests",
            "/data",
            FilesystemAccess::ReadOnly,
        )],
        ExecutionLimits::unlimited(),
    );
    let runtime = Runtime::with_policy(Vec::new(), policy);

    let module = runtime.compile(&wasm(WASI_GUEST)).expect("compiles");
    let error = runtime
        .run(&module, &EntryPoint::default())
        .expect_err("the directory is not there");

    match &error {
        RuntimeError::Host { message } => {
            assert!(message.contains("/data"), "{message}");
        }
        other => panic!("expected a host error, got {other:?}"),
    }
}

/// The policy is readable, so a command can report what it granted.
#[test]
fn the_policy_a_runtime_carries_is_the_one_it_applies() {
    let policy = RuntimePolicy::wasi(
        vec![Mount::new("./data", "/data", FilesystemAccess::ReadOnly)],
        ExecutionLimits::bounded(Some(Duration::from_secs(3)), Some(64)),
    );
    let runtime = Runtime::with_policy(Vec::new(), policy.clone());

    assert_eq!(runtime.policy(), &policy);
    assert_eq!(runtime.policy().mounts().len(), 1);
    assert_eq!(runtime.policy().limits().memory_pages(), Some(64));
}

/// A runtime built the ordinary way is the denied one.
#[test]
fn a_runtime_without_a_policy_grants_nothing() {
    let runtime = Runtime::default();
    assert!(!runtime.policy().wasi_enabled());
    assert!(runtime.policy().mounts().is_empty());
}
