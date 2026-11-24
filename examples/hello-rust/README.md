# hello-rust

A Rust guest compiled to WebAssembly. This demonstrates using the Rust toolchain to build
a guest module from source.

## What it shows

- `wf build` invokes `cargo` with the `wasm32-unknown-unknown` target
- The guest crate must declare `crate-type = ["cdylib"]` to emit a `.wasm`
- `wf run` executes the compiled module

## Prerequisites

- WasmFoundry CLI installed (`cargo install --path crates/wf-cli`)
- Rust toolchain with `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

## Commands

```bash
cd examples/hello-rust
wf build
wf run
```

## Expected output

```
wf: built hello (core module) via rust -> ./target/hello.wasm
     (in ./guest) cargo build --release --target wasm32-unknown-unknown
wf: built 1 module(s) into target/

wf: ran guest via _start in 1.4ms
wf: entry point returned 7
```

## Files

- `wasmfoundry.toml` — project manifest declaring a Rust module with `toolchain = "rust"` and `source = "guest"` (the directory containing `Cargo.toml`)
- `guest/Cargo.toml` — guest crate manifest with `crate-type = ["cdylib"]` and empty `[workspace]` table to opt out of the parent workspace
- `guest/src/lib.rs` — guest source with a `_start` function returning `7`