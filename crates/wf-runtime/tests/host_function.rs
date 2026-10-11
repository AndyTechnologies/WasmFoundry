// Tests for the host function mechanism.
//
// What is verified here is the plumbing, not the contract: a guest can call a function
// the host registered, the host can read the bytes the guest pointed at, and failures
// come back as a typed host error rather than a trap.

use wf_core::EntryPoint;
use wf_runtime::{HostError, HostFunction, Runtime, RuntimeError};

/// Records what a host function was given, so the test can look at it afterwards.
#[derive(Clone, Default)]
struct Seen(std::sync::Arc<std::sync::Mutex<Option<String>>>);

impl Seen {
    fn note(&self, text: &str) {
        *self.0.lock().expect("lock") = Some(text.to_owned());
    }

    fn text(&self) -> Option<String> {
        self.0.lock().expect("lock").clone()
    }
}

/// A host function that reads the range the guest pointed at and records it.
fn recorder(seen: Seen) -> HostFunction {
    HostFunction::new("test", "note", 2, move |guest, arguments| {
        let bytes = guest.read_memory(arguments[0], arguments[1])?;
        seen.note(&String::from_utf8_lossy(&bytes));
        Ok(None)
    })
}

/// A guest that puts "hello" in its data segment and passes a pointer to it.
const GUEST: &str = r#"
    (module
      (import "test" "note" (func $note (param i32 i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "hello")
      (func (export "_start")
        i32.const 0
        i32.const 5
        call $note))"#;

fn wasm(source: &str) -> Vec<u8> {
    wat::parse_str(source).expect("fixture must assemble")
}

#[test]
fn a_guest_call_reaches_the_host_and_the_host_reads_its_bytes() {
    let seen = Seen::default();
    let runtime = Runtime::with_host_functions(vec![recorder(seen.clone())]);

    let module = runtime.compile(&wasm(GUEST)).expect("compiles");
    runtime
        .run(&module, &EntryPoint::default())
        .expect("the call must succeed");

    assert_eq!(seen.text().as_deref(), Some("hello"));
}

#[test]
fn the_same_host_works_for_several_modules() {
    let seen = Seen::default();
    let runtime = Runtime::with_host_functions(vec![recorder(seen.clone())]);

    for _ in 0..3 {
        let module = runtime.compile(&wasm(GUEST)).expect("compiles");
        runtime.run(&module, &EntryPoint::default()).expect("runs");
    }

    assert_eq!(seen.text().as_deref(), Some("hello"));
}

/// Without the function registered, the import cannot be satisfied.
#[test]
fn a_host_function_is_only_reachable_when_it_was_registered() {
    let runtime = Runtime::default();
    let module = runtime.compile(&wasm(GUEST)).expect("compiles");

    let error = runtime
        .run(&module, &EntryPoint::default())
        .expect_err("the host offers nothing");

    assert!(
        matches!(error, RuntimeError::Link { .. }),
        "expected a link error, got {error:?}"
    );
}

/// The host must not be able to read memory the guest does not export.
#[test]
fn a_guest_without_memory_is_reported_rather_than_trapped() {
    let seen = Seen::default();
    let no_memory = r#"
        (module
          (import "test" "note" (func (param i32 i32)))
          (func (export "_start")
            i32.const 0
            i32.const 1
            call 0))"#;

    let runtime = Runtime::with_host_functions(vec![recorder(seen.clone())]);
    let module = runtime.compile(&wasm(no_memory)).expect("compiles");

    let error = runtime
        .run(&module, &EntryPoint::default())
        .expect_err("the host cannot read anything");

    match &error {
        RuntimeError::Host { message } => {
            assert!(message.contains("memory"), "{message}");
        }
        other => panic!("expected a host error, got {other:?}"),
    }
    assert_eq!(error.code(), wf_core::DiagnosticCode::HostFailure);
    assert!(seen.text().is_none(), "nothing could be read");
}

/// A pointer past the end of memory is refused, not read out of bounds.
#[test]
fn a_range_past_the_end_of_memory_is_refused() {
    let seen = Seen::default();
    let guest = r#"
        (module
          (import "test" "note" (func $note (param i32 i32)))
          (memory (export "memory") 1)
          (func (export "_start")
            i32.const 65500
            i32.const 100
            call $note))"#;

    let runtime = Runtime::with_host_functions(vec![recorder(seen.clone())]);
    let module = runtime.compile(&wasm(guest)).expect("compiles");

    let error = runtime
        .run(&module, &EntryPoint::default())
        .expect_err("the range runs past the end");

    match &error {
        RuntimeError::Host { message } => {
            assert!(message.contains("past the end"), "{message}");
        }
        other => panic!("expected a host error, got {other:?}"),
    }
    assert!(seen.text().is_none(), "nothing may be read out of bounds");
}

/// A negative pointer is refused before anything is read.
#[test]
fn a_negative_pointer_is_refused() {
    let seen = Seen::default();
    let guest = r#"
        (module
          (import "test" "note" (func $note (param i32 i32)))
          (memory (export "memory") 1)
          (func (export "_start")
            i32.const -4
            i32.const 8
            call $note))"#;

    let runtime = Runtime::with_host_functions(vec![recorder(seen.clone())]);
    let module = runtime.compile(&wasm(guest)).expect("compiles");

    let error = runtime
        .run(&module, &EntryPoint::default())
        .expect_err("a negative pointer is not a range");

    assert!(
        matches!(error, RuntimeError::Host { .. }),
        "expected a host error, got {error:?}"
    );
}

/// The errors name what was wrong, because the guest passed the numbers and the guest's
/// author is the one who has to fix them.
#[test]
fn host_errors_explain_themselves() {
    assert!(HostError::NoMemory.to_string().contains("memory"));
    assert!(
        HostError::InvalidPointer {
            pointer: -4,
            length: 8
        }
        .to_string()
        .contains("-4")
    );
    assert!(
        HostError::OutBounds {
            pointer: 65500,
            length: 100
        }
        .to_string()
        .contains("past the end")
    );
    assert!(
        HostError::Refused {
            reason: "nope".to_owned()
        }
        .to_string()
        .contains("nope")
    );
}

/// A host function listed under several namespaces must appear under each one.
#[test]
fn a_host_function_is_reachable_under_the_namespace_it_registered() {
    let seen = Seen::default();
    let runtime = Runtime::with_host_functions(vec![HostFunction::new("alpha", "note", 2, {
        let seen = seen.clone();
        move |guest, arguments| {
            let bytes = guest.read_memory(arguments[0], arguments[1])?;
            seen.note(&format!("alpha:{}", String::from_utf8_lossy(&bytes)));
            Ok(None)
        }
    })]);

    let guest = r#"
        (module
          (import "alpha" "note" (func $note (param i32 i32)))
          (memory (export "memory") 1)
          (data (i32.const 0) "hi")
          (func (export "_start")
            i32.const 0
            i32.const 2
            call $note))"#;

    let module = runtime.compile(&wasm(guest)).expect("compiles");
    runtime.run(&module, &EntryPoint::default()).expect("runs");

    assert_eq!(seen.text().as_deref(), Some("alpha:hi"));
}
