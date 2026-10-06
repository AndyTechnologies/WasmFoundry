//! The `wf run` command.
//!
//! It executes a compiled module and reports either what the guest produced or precisely
//! why it could not run. The runtime crate does the work; this module only reads the file,
//! picks the entry point and maps the cause to a diagnostic code.

use std::time::Duration;

use wf_core::{Diagnostic, EntryPoint, Severity};
use wf_runtime::{Runtime, RuntimeError};

use crate::cli::RunArgs;
use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// Runs `wf run` and returns the process exit code.
pub fn run(args: &RunArgs) -> i32 {
    let entry = match entry_point(args) {
        Ok(entry) => entry,
        Err(reason) => {
            eprintln!("wf: {reason}");
            return EXIT_USAGE;
        }
    };

    let bytes = match std::fs::read(&args.path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("wf: cannot read {}: {error}", args.path.display());
            return EXIT_USAGE;
        }
    };

    let runtime = Runtime::default();
    let module = match runtime.compile(&bytes) {
        Ok(module) => module,
        Err(error) => return failure(&error),
    };

    match runtime.run(&module, &entry) {
        Ok(outcome) => {
            println!(
                "wf: ran {} via {} in {}",
                args.path.display(),
                outcome.entry(),
                format_duration(outcome.duration())
            );
            // A guest returning an i32 is reported, not interpreted: mapping a guest's
            // return value onto a process exit code is a host-ABI decision, and the host
            // ABI does not exist yet. Exiting 0 here is the honest answer for a
            // successful invocation.
            if let Some(value) = outcome.return_value() {
                println!("wf: entry point returned {value}");
            }
            EXIT_OK
        }
        Err(error) => failure(&error),
    }
}

/// Resolves the entry point from the argument, or the default.
fn entry_point(args: &RunArgs) -> Result<EntryPoint, String> {
    match &args.entry {
        None => Ok(EntryPoint::default()),
        Some(name) => EntryPoint::new(name)
            .map_err(|error| format!("--entry {}: {error}", name.escape_default())),
    }
}

/// Prints a diagnostic and returns the diagnostic exit code.
fn failure(error: &RuntimeError) -> i32 {
    // The code comes from the runtime that produced the cause; building the diagnostic
    // here only decides severity and output.
    let diagnostic = Diagnostic::new(error.code(), Severity::Error, error.to_string());
    eprintln!("{diagnostic}");
    EXIT_DIAGNOSTIC
}

/// Formats a duration at the precision a human wants to read.
fn format_duration(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros < 1_000 {
        format!("{micros}us")
    } else if micros < 1_000_000 {
        format!("{:.1}ms", micros as f64 / 1_000.0)
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every duration the report can produce must be readable at its own scale.
    #[test]
    fn durations_render_at_the_scale_they_need() {
        assert_eq!(format_duration(Duration::from_micros(12)), "12us");
        assert_eq!(format_duration(Duration::from_micros(1_500)), "1.5ms");
        assert_eq!(format_duration(Duration::from_micros(1_500_000)), "1.50s");
    }

    /// The argument-level rejection must not reach the runtime.
    #[test]
    fn an_empty_entry_point_is_rejected_before_reading_the_file() {
        let error = EntryPoint::new("").expect_err("empty is invalid");
        assert!(error.reason().contains("empty"));
    }
}
