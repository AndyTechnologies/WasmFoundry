# WasmFoundry

**Build, inspect and run multi-toolchain WebAssembly projects with a single, explicit
runtime contract.**

WasmFoundry is a Rust toolchain for WebAssembly that treats guest languages as adapters
instead of hard-coded branches, resolves imports between modules itself instead of
generating C++, and runs guests under an explicit capability policy instead of an
implicitly permissive host.

> **Status: PHASE 3.** `wf inspect` works: it analyses a WebAssembly binary and reports
> its imports, exports, memories, tables, globals and functions without executing it.
> Everything else is still ahead — see [Status](#status).

---

## What problem this solves

Building a WebAssembly project that spans more than one guest language, more than one
module, or a native binary usually forces a choice between tools that each cover a
narrow slice and glue that covers none:

- Toolchains are wired per-language, so adding a language means editing the build core.
- Multi-module composition is done by generating C or C++ that re-declares every import,
  which drags a full C toolchain into the build just to link two `.wasm` files.
- The host surface offered to guests is whatever the generated bindings happened to
  export, with no version and no declared capabilities.
- Shipping a "native executable" means bolting a launcher onto a payload in a way that
  is specific to one executable format at a time.

WasmFoundry takes the opposite position on each:

| Problem | Position |
| --- | --- |
| Many guest languages | Each is a `Toolchain` adapter behind one trait; the build core never branches on a language name. |
| Multi-module linking | The dependency graph is resolved by name and instantiated through a linker. No C++, no CMake, no generated bindings. |
| Host surface | One documented Host ABI with a version, split between a portable ABI and language-compatibility ABIs. |
| Distribution | A self-contained native binary built from a documented bundle format with a fixed footer and a checksum, format-independent. |
| Guest permissions | A capability policy defaults to denied: no filesystem, no network, no environment, no process execution. |

---

## Architecture

Four crates, with a strict dependency direction:

```text
wf-core        pure domain: types, rules, graph traversal, fingerprints, diagnostics
   ^
wf-wasm        WebAssembly analysis, delegated to wasmparser
   ^
wf-runtime     execution, delegated to Wasmtime
   ^
wf-cli         the `wf` binary: argument parsing, output, exit codes
```

Each crate may only depend on the one below it. The full rules, including the banned
dependencies per crate, are in [`docs/architecture.md`](docs/architecture.md).

Two rules are worth stating here because they shape everything else:

1. **No hand-written WebAssembly parsing.** All binary decoding goes through
   `wasmparser`. The only bytes read directly are the fixed eight-byte binary header,
   because the library exposes no header-only entry point.
2. **No async architecture.** The product is synchronous, with `std` threads where
   concurrency is actually needed.

---

## Crates

| Crate | Responsibility | Key dependency |
| --- | --- | --- |
| `wf-core` | Domain vocabulary and pure rules. Zero dependencies. | none |
| `wf-wasm` | Analysis of a WebAssembly binary: kind, imports, exports, validation. | `wasmparser` |
| `wf-runtime` | Execution of an analysed module under Wasmtime. | `wasmtime` |
| `wf-cli` | The `wf` command line interface. | `clap` |

---

## Requirements

- A Rust toolchain. The exact version is pinned in `rust-toolchain.toml`, so `rustup`
  installs it automatically.
- Nothing else, yet.

Guest toolchains (Rust guest, C++, AssemblyScript) are **not** installed by this
repository and not required to build it. Each is detected on demand and reported by
`wf doctor` once that command exists.

## Building from source

```bash
cargo check --workspace
cargo test --workspace
```

## Usage

```bash
# Analyse a binary and print a report for a human.
wf inspect app.wasm

# The same analysis as JSON, for other tools to consume.
wf inspect app.wasm --format json
```

```
$ wf inspect app.wasm
Module: app.wasm
Kind: core module
Version: 1

Imports (2):
  env.console_log  func  (i32, i32) -> ()
  env.memory  memory min 2 pages, max 16 pages

Exports (2):
  memory  memory
  _start  func  (i32) -> (i32)

Memories (2):
  #0  min 2 pages, max 16 pages
  #1  min 2 pages, max 16 pages

Tables (0):
  (none)

Globals (0):
  (none)

Functions (2):
  #0  imported  (i32, i32) -> ()
  #1  defined   (i32) -> (i32)
```

`wf inspect` never executes the binary it reads. Exit codes are stable: `0` on success,
`1` when the input is rejected with a `WFnnn` diagnostic, `2` when the path or the
command line is wrong.

---

## Status

The rewrite proceeds in vertical slices. Each phase delivers one observable capability
and must pass its gate before the next begins.

| Phase | Delivers | Gate |
| --- | --- | --- |
| 0–1 | Legacy state preserved, `main` reset | ✅ done |
| 2 | Rust workspace, four crates, documentation | ✅ current |
| 3 | `wf inspect` | ✅ done |
| 4 | `wf run` | pending |
| 5 | `wf init`, `wf build` for precompiled wasm | pending |
| 6 | Rust guest toolchain | pending |
| 7 | Dependency graph and module matching | pending |
| 8–9 | Host ABI v1, WASI, mounts, capability policy | pending |
| 10–11 | C++ and AssemblyScript toolchains | pending |
| 12–13 | Cache, watch | pending |
| 14–16 | Bundle format v1 and prebuilt launchers | pending |
| 17 | Component model support | pending |
| 19–21 | Supply-chain tooling, CI, `v0.1.0` | pending |

Functional parity with the previous generation of this project is tracked explicitly in
[`docs/parity-matrix.md`](docs/parity-matrix.md), including the capabilities that are
deliberately **not** being carried over.

## Documentation

- [`docs/product.md`](docs/product.md) — what the product is, non-goals, security boundary
- [`docs/architecture.md`](docs/architecture.md) — crate responsibilities and dependency rules
- [`docs/parity-matrix.md`](docs/parity-matrix.md) — legacy capability mapping and decisions
- [`docs/gaps/`](docs/gaps/) — gaps discovered during the rewrite
- [`docs/adr/`](docs/adr/) — architecture decision records
- [`AGENTS.md`](AGENTS.md) — repository operating instructions for coding agents

## Security

`wf run` executes untrusted WebAssembly under Wasmtime with a deny-by-default
capability policy and resource limits. `wf build` does **not** sandbox anything: it runs
the guest toolchain — including `build.rs`, compilers and any script the project
defines — with the permissions of your user account. These are two different trust
boundaries and the product never conflates them. See
[`docs/product.md`](docs/product.md#security-boundary) and the threat model once it
exists.

## License

MIT. See [LICENSE](LICENSE).