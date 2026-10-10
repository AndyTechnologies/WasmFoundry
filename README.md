# WasmFoundry

**Build, inspect and run multi-toolchain WebAssembly projects with a single, explicit
runtime contract.**

WasmFoundry is a Rust toolchain for WebAssembly that treats guest languages as adapters
instead of hard-coded branches, resolves imports between modules itself instead of
generating C++, and runs guests under an explicit capability policy instead of an
implicitly permissive host.

> **Status: PHASE 7.** Two guest toolchains — a precompiled module, and a Rust crate
> compiled by `cargo` — and a project can be made of several modules that import from
> each other: `wf build` resolves the graph, `wf run` instantiates them in dependency
> order. Everything else is still ahead — see [Status](#status).

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
# Create a project. It scaffolds a manifest, a source directory and a sample module.
wf init hello
cd hello

# Publish the declared module into target/. Sources are precompiled WebAssembly: nothing
# is compiled yet, so no guest toolchain is required.
wf build

# Execute the project's built module, or any path directly.
wf run
wf run target/hello.wasm
wf run target/hello.wasm --entry main

# Analyse any binary without executing it, for a human or as JSON for other tools.
wf inspect target/hello.wasm
wf inspect target/hello.wasm --format json
```

A Rust guest needs the guest target installed once:

```bash
rustup target add wasm32-unknown-unknown
```

and a module declared against it:

```toml
[[module]]
name = "hello"
source = "guest"          # the directory holding Cargo.toml
toolchain = "rust"
```

The crate must declare `[lib] crate-type = ["cdylib"]`, which is what makes `cargo` emit a
WebAssembly module. A project is otherwise one file, `wasmfoundry.toml`:

```toml
schema = 1

[package]
name = "hello"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "hello"
source = "src/hello.wasm"
toolchain = "precompiled"
```

A project may hold several modules. An import names a namespace, and the manifest's
`module_matching` decides which field of a module declaration that namespace is compared
against:

```toml
[project]
source_dir = "src"
entry = "_start"
module_matching = "file-name"   # or "name-only"
```

- `file-name` — the namespace matches the source file's name without its extension
- `name-only` — the namespace matches the module's `name`

`wf build` refuses a project whose imports cannot be bound: a namespace claimed by two
modules, or a cycle. An import that matches *no* project module is left alone — that is a
host or external import, and whether it can be satisfied is the runtime's question.

A manifest whose `schema` this version does not know is rejected outright: it is never
partially interpreted, because fields shared between two schemas may have changed meaning.
Unknown table and field names are also rejected, so a typo cannot silently do nothing.

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

`wf inspect` never executes the binary it reads. Exit codes are stable and a script may
branch on them:

| Code | Meaning |
| --- | --- |
| `0` | The command succeeded |
| `1` | The command ran and the input was rejected, with a `WFnnn` diagnostic |
| `2` | The command line or the input path was wrong |

Diagnostic codes, also stable:

| Code | Cause |
| --- | --- |
| `WF001` | The input is not a compilable WebAssembly module |
| `WF002` | An import has no provider, or cannot be bound to one module |
| `WF003` | The project's modules form a dependency cycle |
| `WF004` | The toolchain a module asks for is not available |
| `WF005` | The requested entry point is not exported |
| `WF009` | The guest faulted while executing |
| `WF010` | The entry point exists but cannot be invoked as written |
| `WF011` | The manifest does not mean what it appears to mean |
| `WF012` | A toolchain ran and did not produce a module |

A guest returning a value from its entry point is reported as output, not converted into
this process's exit code: that mapping is a host-ABI decision, and the host ABI arrives in
PHASE 8.

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
| 5 | `wf init`, `wf build` for precompiled wasm | ✅ done |
| 6 | Rust guest toolchain | ✅ done |
| 7 | Dependency graph and module matching | ✅ done |
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

Two trust boundaries exist in this product, and they are not the same thing.

**`wf run` executes a guest.** Wasmtime's sandbox applies: the guest has no filesystem,
network, environment or process access, because there is nothing to give it. There is no
host ABI yet, so every import fails to resolve rather than reaching the host.

**`wf run` has no execution limits yet.** Until PHASE 9 it sets no wall-clock timeout and
no memory cap, so a guest can run forever or grow its memory without bound. Run modules
you do not trust only where that is acceptable to you; this is stated rather than implied.

**`wf build` will not sandbox anything.** When it arrives in PHASE 5 it will run the guest
toolchain — `build.rs`, compilers, package managers, scripts — with the permissions of
your account. Wasmtime's sandbox does not protect against those processes, and the product
never claims it does.

`wf inspect` does not execute what it reads. See
[`docs/product.md`](docs/product.md#security-boundary); `docs/security/threat-model.md`
arrives with the capability policy in PHASE 9.

## License

MIT. See [LICENSE](LICENSE).