# Examples

Worked examples a user can copy, build and run. Each one demonstrates a complete, honest
path through the product — no example may depend on undocumented tooling or internal
flags.

An example lands only when the capability it demonstrates actually exists. An example
that cannot be run is documentation lying about the product, which the repository's hard
rules forbid.

## Available examples

| Example | Toolchain | Demonstrates | Quick run |
| --- | --- | --- | --- |
| [`hello-precompiled/`](hello-precompiled/) | precompiled | `wf init`, `wf build`, `wf run` on an existing `.wasm` | `cd hello-precompiled && wf build && wf run` |
| [`hello-rust/`](hello-rust/) | Rust | guest compiled to wasm, then run | `cd hello-rust && wf build && wf run` |
| [`two-modules/`](two-modules/) | precompiled | module A imports module B, resolved through the dependency graph | `cd two-modules && wf build && wf run` |
| [`wasi-file/`](wasi-file/) | precompiled | WASI with an explicit read-only mount | `cd wasi-file && wf build && wf run` |
| [`bundle/`](bundle/) | precompiled | `wf bundle` producing a standalone executable | `cd bundle && wf build && wf bundle --launcher ../../target/debug/wf-launcher && ./target/bundle-example.bundle` |

## Structure

Each example is self-contained:

```text
examples/<name>/
├── README.md       what it shows, and the exact commands that make it run
├── wasmfoundry.toml  project manifest
└── src/            guest sources (or guest/ for Rust toolchain)
```

The `README.md` states the expected output. If the product's output changes, the example
is updated in the same commit as the behaviour.

## Current contents

All 5 planned examples are implemented and runnable. Each one demonstrates a real
capability that exists in the current version.

## Planned future examples

| Example | Toolchain | Demonstrates | Phase |
| --- | --- | --- | --- |
| `console-host` | Rust | Host ABI v1 console bindings | 8 |
| `cpp-hello` | C++ | C++ source to wasm, then run | 10 |
| `assemblyscript-hello` | AssemblyScript | AS source to wasm, then run | 11 |