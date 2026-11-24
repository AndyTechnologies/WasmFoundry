# bundle

A standalone native executable bundle. This demonstrates `wf bundle` producing a
self-contained native binary that includes the Wasmtime runtime and the guest module.

## What it shows

- `wf bundle` creates a native executable from a built project
- The bundle includes the launcher (Wasmtime runtime) and the guest module payload
- The bundle runs without WasmFoundry installed — it is truly standalone
- Bundle format v1 with BLAKE3 checksum (corruption is detected)

## Prerequisites

- WasmFoundry CLI installed (`cargo install --path crates/wf-cli`)
- The dev launcher built: `cargo build -p wf-bundle --bin wf-launcher`

## Commands

```bash
cd examples/bundle
wf build
wf bundle --launcher ../../target/debug/wf-launcher
./target/bundle-example.bundle
```

## Expected output

```
wf: built hello (core module) via precompiled -> ./target/hello.wasm
     validated src/hello.wasm
wf: built 1 module(s) into target/

wf: bundled bundle-example -> target/bundle-example.bundle (launcher 342013 KiB, module `hello`)

wf-launcher: ran payload, returned 42
```

## Files

- `wasmfoundry.toml` — project manifest with a single precompiled module
- `src/hello.wasm` — precompiled module returning `42`
- `target/bundle-example.bundle` — the standalone native executable (created by `wf bundle`)

## How it works

The bundle format v1 concatenates:
1. The launcher binary (Wasmtime runtime)
2. The module payload (the `.wasm` file)
3. A manifest (JSON with module metadata)
4. A footer with BLAKE3 checksum of the payload + manifest

The launcher reads the footer, verifies the checksum, extracts the module, and executes it.
Any corruption of the bundle is detected at startup.

## Using prebuilt launchers

Instead of `--launcher`, you can use `--bundle-target` with a local launcher registry:

```bash
# Set up a registry with launchers per target triple
export WF_LAUNCHER_REGISTRY=/path/to/registry
wf bundle --bundle-target x86_64-unknown-linux-gnu
```

This allows bundling for a target without having that target's toolchain locally.