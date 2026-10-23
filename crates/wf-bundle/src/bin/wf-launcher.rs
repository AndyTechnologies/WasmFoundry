//! Dev launcher for WasmFoundry bundles.
//!
//! Reads the current executable's bytes, probes the bundle footer from EOF,
//! and runs the embedded payload as a WebAssembly module using wf-runtime.
//!
//! This is a development tool — a minimal binary that demonstrates the bundle
//! format. It uses no clap, no extra dependencies, and relies on std only
//! (plus wf-runtime for the execution step).

use std::process::exit;

use wf_bundle::{BundleError, probe};
use wf_core::{EntryPoint, RuntimePolicy};
use wf_runtime::{Runtime, RuntimeError};

fn main() {
    let exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("wf-launcher: cannot determine executable path: {e}");
            exit(1);
        }
    };

    let exe_bytes = match std::fs::read(&exe_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("wf-launcher: cannot read executable: {e}");
            exit(1);
        }
    };

    let probe_result = match probe(&exe_bytes) {
        Ok(p) => p,
        Err(BundleError::TooShort) => {
            eprintln!(
                "wf-launcher: not a bundle — file is too short ({:?} is not a bundled executable)",
                exe_path
            );
            exit(1);
        }
        Err(e) => {
            eprintln!("wf-launcher: {e}");
            exit(1);
        }
    };

    let runtime = Runtime::with_policy(wf_runtime::console(), RuntimePolicy::denied());

    let module = match runtime.compile(probe_result.payload) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("wf-launcher: {e}");
            exit(1);
        }
    };

    let entry = EntryPoint::default();

    match runtime.run(&module, &entry) {
        Ok(outcome) => {
            let name = "payload";
            println!(
                "wf-launcher: ran {name}, returned {}",
                outcome
                    .return_value()
                    .map_or(String::from("()"), |v| v.to_string())
            );
            exit(0);
        }
        Err(RuntimeError::Trap { message }) => {
            eprintln!("wf-launcher: guest trapped: {message}");
            exit(1);
        }
        Err(e) => {
            eprintln!("wf-launcher: {e}");
            exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    /// A plain un-bundled executable (e.g., a regular Rust binary) must fail
    /// cleanly with exit 1 when probed, not panic. This test covers the
    /// `probe()` path on non-bundle data.
    #[test]
    fn probing_non_bundle_bytes_returns_error() {
        // A regular binary starts with ELF magic on Linux, PE on Windows,
        // or Mach-O on macOS. None of those have the WFBUNDLE footer.
        let not_a_bundle = b"ELF\x7f\x01\x01\x01\x00"
            .iter()
            .chain(std::iter::repeat(&0u8))
            .take(1024)
            .copied()
            .collect::<Vec<u8>>();

        let result = wf_bundle::probe(&not_a_bundle);
        // A 1024-byte file has enough room for a footer, so it won't be TooShort.
        // The footer bytes are all zeros, so magic won't match.
        assert!(result.is_err());
        assert!(
            matches!(result, Err(wf_bundle::BundleError::BadMagic)),
            "expected BadMagic, got {result:?}"
        );
    }
}
