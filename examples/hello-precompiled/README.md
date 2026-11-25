# hello-precompiled

A minimal precompiled WebAssembly example. This demonstrates the shortest path from
`wf init` to `wf run` using an existing `.wasm` file — no guest toolchain required.

## What it shows

- `wf init` scaffolds a project with a precompiled module
- `wf build` publishes the module into `target/`
- `wf run` executes the module and reports its return value

## Prerequisites

- WasmFoundry CLI installed (`cargo install --path crates/wf-cli`)

## Commands

```bash
cd examples/hello-precompiled
wf build
wf run
```

## Expected output

```
wf: built hello (core module) via precompiled -> ./target/hello.wasm
     validated src/hello.wasm
wf: built 1 module(s) into target/

wf: ran hello via _start in 1.2ms
wf: entry point returned 42
```

## Files

- `wasmfoundry.toml` — project manifest declaring a precompiled module
- `src/hello.wasm` — a minimal WebAssembly module exporting `_start` returning `42`

The module is a precompiled binary (`.wasm`), not WAT text. It was created with:

```bash
# Using the wat crate
echo '(module (func (export "_start") (result i32) (i32.const 42)))' | wat2wasm -o src/hello.wasm
```

Or programmatically via the `wat` crate (as done in this repository's fixtures).