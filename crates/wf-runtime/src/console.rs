//! The portable console ABI — Host ABI v1.
//!
//! The surface described in [`docs/abi/host-abi-v1.md`](../../docs/abi/host-abi-v1.md).
//! Every function here reads a range of guest memory and writes to a stream; none of them
//! allocates for the guest, writes into guest memory, or touches anything else.

use crate::host::HostFunction;

/// The namespace every portable host function lives in.
pub const NAMESPACE: &str = "env";

/// Namespaces this runtime provides on its own.
///
/// A build treats them as already satisfied: they are not project modules and never will
/// be, so making every guest declare `host_namespaces = ["env"]` before it can print
/// would be ceremony around a product decision the product already made.
pub const PROVIDED: &[&str] = &[NAMESPACE];

/// Which stream a message goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    /// Standard output: the guest's product.
    Output,
    /// Standard error: the guest's diagnostics.
    Diagnostic,
}

/// The functions this ABI provides, ready to hand to a runtime.
///
/// Built on every call rather than stored, so nothing about the host surface is shared
/// mutable state between runs.
pub fn console() -> Vec<HostFunction> {
    vec![
        console_function("console.log", Stream::Output),
        console_function("console.debug", Stream::Output),
        console_function("console.info", Stream::Output),
        console_function("console.warn", Stream::Diagnostic),
        console_function("console.error", Stream::Diagnostic),
        HostFunction::new(NAMESPACE, "console.assert", 3, |guest, arguments| {
            // A failed assertion reports; it does not stop the guest. The condition is
            // the guest's own truth, so the host only decides whether to write.
            if arguments[0] != 0 {
                return Ok(None);
            }
            let bytes = guest.read_memory(arguments[1], arguments[2])?;
            write(Stream::Diagnostic, "Assertion failed: ", &bytes);
            Ok(None)
        }),
    ]
}

/// One of the five message functions: the same shape at every level.
///
/// `debug` writes to the same stream as `log` and is kept separate so that a reader can
/// filter on the level; collapsing them would make filtering impossible.
fn console_function(name: &str, stream: Stream) -> HostFunction {
    HostFunction::new(NAMESPACE, name, 2, move |guest, arguments| {
        let bytes = guest.read_memory(arguments[0], arguments[1])?;
        write(stream, "", &bytes);
        Ok(None)
    })
}

/// Writes a message and a newline to its stream.
///
/// Invalid UTF-8 is replaced rather than rejected: a guest that passed malformed bytes
/// should still have its message visible, and stopping over encoding would hide the
/// thing the caller was trying to say.
fn write(stream: Stream, prefix: &str, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    let mut line = String::with_capacity(prefix.len() + text.len() + 1);
    line.push_str(prefix);
    line.push_str(&text);
    line.push('\n');

    match stream {
        Stream::Output => print!("{line}"),
        Stream::Diagnostic => eprint!("{line}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The levels that share the message shape.
    #[test]
    fn the_five_message_functions_exist_with_the_documented_arity() {
        let functions = console();
        let names: Vec<&str> = functions.iter().map(|function| function.name()).collect();

        for name in [
            "console.log",
            "console.debug",
            "console.info",
            "console.warn",
            "console.error",
        ] {
            assert!(names.contains(&name), "`{name}` must exist: {names:?}");
            let function = functions
                .iter()
                .find(|candidate| candidate.name() == name)
                .expect("found");
            assert_eq!(function.arity(), 2, "`{name}` takes a pointer and a length");
            assert_eq!(function.namespace(), NAMESPACE);
        }
    }

    #[test]
    fn assert_takes_a_condition_as_well_as_the_message() {
        let functions = console();
        let assert = functions
            .iter()
            .find(|function| function.name() == "console.assert")
            .expect("assert exists");

        assert_eq!(assert.arity(), 3, "condition, pointer, length");
        assert_eq!(assert.namespace(), NAMESPACE);
    }

    /// The namespace is part of the published contract: a guest imports by it, and
    /// changing it silently would break every guest built against v1.
    #[test]
    fn every_function_is_in_the_env_namespace() {
        for function in console() {
            assert_eq!(function.namespace(), NAMESPACE);
        }
    }

    #[test]
    fn function_names_are_the_documented_ones() {
        let functions = console();
        let mut names: Vec<&str> = functions.iter().map(|function| function.name()).collect();
        names.sort_unstable();

        assert_eq!(
            names,
            vec![
                "console.assert",
                "console.debug",
                "console.error",
                "console.info",
                "console.log",
                "console.warn",
            ]
        );
    }
}
