# wasi-file

A WASI example with an explicit read-only mount. This demonstrates WasmFoundry's capability
policy — capabilities are denied by default and granted explicitly in the manifest.

## What it shows

- `[runtime] wasi = true` grants the WASI capability
- `[[mount]]` declares a host directory mounted at a guest path with explicit access mode
- `access = "read-only"` is the default (can also be `read-write`)
- The guest runs under Wasmtime's WASI preview1 implementation

## Commands

```bash
cd examples/wasi-file
wf build
wf run
```

## Expected output

```
wf: built main (core module) via precompiled -> ./target/main.wasm
     validated src/main.wasm
wf: built 1 module(s) into target/

hello wasi
wf: ran main via _start in 26.9ms
wf: entry point returned 0
```

The guest outputs "hello wasi" to stdout via WASI's `fd_write`. The mount at `/data` is
available to the guest (read-only), though this minimal guest only writes to stdout.

## Files

- `wasmfoundry.toml` — manifest with `wasi = true` and a read-only mount
- `src/main.wasm` — precompiled module using WASI `fd_write` to write "hello wasi"
- `data/hello.txt` — a file available at `/data/hello.txt` inside the guest (read-only)

## Key concepts

**Deny by default**: Without `wasi = true`, the guest's WASI imports would fail to resolve
with `WF002`. The capability must be explicitly granted.

**Explicit mounts**: Each mount declares:
- `host` — host filesystem path (relative to project root or absolute)
- `guest` — absolute path inside the guest's namespace
- `access` — `read-only` (default) or `read-write`

A relative `host` path is resolved relative to the project root (where `wasmfoundry.toml`
lives). The `guest` path must be absolute.