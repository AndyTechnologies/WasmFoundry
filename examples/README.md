# Examples

Worked examples a user can copy, build and run. Each one demonstrates a complete, honest
path through the product — no example may depend on undocumented tooling or internal
flags.

An example lands only when the capability it demonstrates actually exists. An example
that cannot be run is documentation lying about the product, which the repository's hard
rules forbid.

## Planned examples

| Example | Toolchain | Demonstrates | Phase |
| --- | --- | --- | --- |
| `hello-precompiled` | precompiled | `wf init`, `wf build`, `wf run` on an existing `.wasm` | 5 |
| `hello-rust` | Rust | guest compiled to wasm, then run | 6 |
| `two-modules` | Rust + precompiled | module A imports module B, resolved through the dependency graph | 7 |
| `console-host` | Rust | Host ABI v1 console bindings | 8 |
| `wasi-file` | Rust | WASI with an explicit read-only mount | 9 |
| `cpp-hello` | C++ | C++ source to wasm, then run | 10 |
| `assemblyscript-hello` | AssemblyScript | AS source to wasm, then run | 11 |
| `bundle` | precompiled | `wf bundle` producing a standalone executable | 15 |

## Structure

Each example is self-contained:

```text
examples/<name>/
├── README.md       what it shows, and the exact commands that make it run
├── wasmfoundry.toml  project manifest
└── src/            guest sources
```

The `README.md` states the expected output. If the product's output changes, the example
is updated in the same commit as the behaviour.

## Current contents

None. The first example arrives with PHASE 5, when `wf init` and `wf build` exist for
precompiled wasm.