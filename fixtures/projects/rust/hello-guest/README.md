# `hello-guest`

A minimal Rust guest for PHASE 6.

It exists to prove three things at once: `wf build` invokes `cargo` with the guest target,
cargo's output is discovered without guessing, and the published module runs.

## Layout

- `Cargo.toml` — `crate-type = ["cdylib"]`, which is what makes cargo emit a `.wasm`.
- `src/lib.rs` — one exported `_start` returning `7`.

The value `7` is the whole observable behaviour: there is no host ABI until PHASE 8, so
the guest cannot print. `wf run` reporting `returned 7` is the end-to-end proof.

## Used by

`crates/wf-cli/tests/rust_project.rs`, which copies this directory into a temporary
project, adds a `wasmfoundry.toml`, runs `wf build` against the real `cargo`, and then
runs the published module.
