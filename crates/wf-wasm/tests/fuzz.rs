// Deterministic input-generation tests for the analysis boundary.
//
// These are not libFuzzer and they do not need nightly: they generate structured
// WebAssembly with `wasm-smith` from a fixed, reproducible seed stream and check the
// invariants this crate must never break. A failure names its seed, so any case is
// reproducible byte for byte.
//
// The target is deliberately *this* crate rather than `wasmparser`: upstream is already
// fuzzed hard, and what is not covered upstream is the boundary WasmFoundry added —
// header classification, error translation, and the assembly of the analysis report.

use std::panic::{AssertUnwindSafe, catch_unwind};

use arbitrary::Unstructured;
use wasm_smith::{Config, Module};

/// Number of generated modules. Enough to exercise imports, exports, globals, tables,
/// memories and custom sections, small enough that the suite stays quick.
const CASES: u64 = 150;

/// Bytes handed to one generator.
const ENTROPY_BYTES: usize = 8 * 1024;

/// A tiny deterministic generator.
///
/// xorshift64* rather than a `rand` dependency: the stream must be identical on every
/// machine and every run, or a reported failure could not be reproduced from its seed.
struct Entropy(u64);

impl Entropy {
    fn new(seed: u64) -> Self {
        // A fixed non-zero offset, because xorshift's all-zero state never leaves it.
        Entropy(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Fills a buffer with the stream.
    fn fill(&self, seed: u64, length: usize) -> Vec<u8> {
        let mut state = Entropy::new(seed);
        let mut bytes = Vec::with_capacity(length);
        while bytes.len() < length {
            bytes.extend_from_slice(&state.next().to_le_bytes());
        }
        bytes.truncate(length);
        bytes
    }
}

/// Builds one module from a seed, or `None` when the generator declined.
fn generated(seed: u64) -> Option<Vec<u8>> {
    let data = Entropy::new(seed).fill(seed, ENTROPY_BYTES);
    let mut unstructured = Unstructured::new(&data);

    let config = Config {
        // Exports and custom sections are the parts the report has to resolve and the
        // parts a reader is most likely to mis-handle, so generate them deliberately
        // rather than only when the default happens to.
        export_everything: true,
        generate_custom_sections: true,
        ..Config::default()
    };

    let module = Module::new(config, &mut unstructured).ok()?;
    Some(module.to_bytes())
}

/// Every generated module must be analysable, deterministic, and never panic.
#[test]
fn generated_modules_are_analysed_without_panicking() {
    let mut generated_count = 0;
    let mut declined = 0;

    for seed in 0..CASES {
        let Some(wasm) = generated(seed) else {
            declined += 1;
            continue;
        };
        generated_count += 1;

        let first = catch_unwind(AssertUnwindSafe(|| wf_wasm::analyze(&wasm)))
            .unwrap_or_else(|_| panic!("analyze panicked on a generated module, seed {seed}"));

        let first = first.unwrap_or_else(|error| {
            panic!(
                "a generated module was rejected, seed {seed}: {error}. \
                 The message must name the defect, never be empty."
            )
        });

        // The same bytes must produce the same report: a report that drifts between runs
        // would make every downstream comparison meaningless.
        let second = catch_unwind(AssertUnwindSafe(|| wf_wasm::analyze(&wasm)))
            .unwrap_or_else(|_| panic!("analyze panicked on a generated module, seed {seed}"))
            .unwrap_or_else(|error| panic!("second analysis failed, seed {seed}: {error}"));

        assert_eq!(
            format!("{first:?}"),
            format!("{second:?}"),
            "the same bytes must produce the same report, seed {seed}"
        );
    }

    // Guards the test itself: a generator that silently stops producing modules would
    // turn this into a suite of no-ops that still passes.
    assert!(
        generated_count >= CASES / 2,
        "only {generated_count} modules generated and {declined} declined; \
         the test would be worthless at this rate"
    );
}

/// Truncated input must never crash the analysis or make it drift.
///
/// What it may do is either reject the input or describe whatever complete sections are
/// still there — eight bytes of header with no body *is* a valid empty module, so
/// "every truncation is rejected" would be a false claim. The invariants that do hold
/// are the ones asserted here.
#[test]
fn truncated_generated_modules_are_analysed_without_panicking() {
    for seed in 0..CASES {
        let Some(wasm) = generated(seed) else {
            continue;
        };
        if wasm.len() < 16 {
            continue;
        }

        for cut in [8, wasm.len() / 2, wasm.len() - 1] {
            if cut >= wasm.len() {
                continue;
            }
            let truncated = &wasm[..cut];

            let outcome = catch_unwind(AssertUnwindSafe(|| wf_wasm::analyze(truncated)))
                .unwrap_or_else(|_| {
                    panic!("analyze panicked on truncated input, seed {seed} cut {cut}")
                });

            match outcome {
                Ok(analysis) => {
                    // Whatever was accepted must be reproducible.
                    let again = wf_wasm::analyze(truncated).unwrap_or_else(|error| {
                        panic!("re-analysis disagreed, seed {seed}: {error}")
                    });
                    assert_eq!(
                        format!("{analysis:?}"),
                        format!("{again:?}"),
                        "truncated input must be reported deterministically, seed {seed} cut {cut}"
                    );
                }
                Err(error) => {
                    let rendered = error.to_string();
                    assert!(
                        !rendered.trim().is_empty(),
                        "a rejection must say something, seed {seed} cut {cut}"
                    );
                }
            }
        }
    }
}

/// A corrupted header must be rejected by name, with a message a reader can act on.
#[test]
fn corrupted_headers_name_the_defect() {
    for seed in 0..CASES {
        let Some(wasm) = generated(seed) else {
            continue;
        };
        if wasm.len() < 8 {
            continue;
        }

        // Replace the magic with something recognisably not WebAssembly.
        let mut bad_magic = wasm.clone();
        bad_magic[..4].copy_from_slice(b"ELF\x7f");

        let outcome = catch_unwind(AssertUnwindSafe(|| wf_wasm::analyze(&bad_magic)))
            .unwrap_or_else(|_| panic!("analyze panicked on bad magic, seed {seed}"));
        let error = match outcome {
            Ok(analysis) => panic!("bad magic was accepted, seed {seed}: {analysis:?}"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("WebAssembly"),
            "the message must say what was expected, seed {seed}: {error}"
        );
    }
}

/// Under-length input must reach the header check and stop there.
#[test]
fn short_input_is_reported_as_a_truncated_header() {
    for length in 0..8 {
        let bytes = vec![0x00; length];
        let error = wf_wasm::analyze(&bytes)
            .expect_err("fewer than eight bytes can never be a module")
            .to_string();
        assert!(error.contains("bytes"), "case {length}: {error}");
    }
}
