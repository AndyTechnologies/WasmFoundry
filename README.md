# WasmFoundry

**WasmFoundry: toolchain Rust para WebAssembly con contrato explícito**

WasmFoundry is a Rust toolchain for WebAssembly that treats guest languages as adapters
instead of hard-coded branches, resolves imports between modules itself instead of
generating C++, and runs guests under an explicit capability policy instead of an
implicitly permissive host.

> **Status: `v0.3.1` released.** All core features implemented through PHASE 21.
> See [Status](#status) for the complete phase list.

---

## 📦 What this repository is

WasmFoundry is a Rust toolchain for building, inspecting and running WebAssembly
projects. It is a rewrite from scratch of a previous TypeScript implementation;
that implementation is **historical reference only** and lives in a separate,
frozen repository.

### Hard rules

Violating any of these is a defect, regardless of how convenient the alternative is.

1. **Never write to the legacy repository.** It is a separate repository on its own
   branch and tag, kept as the frozen historical record: do not write to it, do not
   "fix" it, do not copy implementation code out of it, do not rebase or delete its
   branches.

   **Reading it is allowed, and is sometimes required.** The master plan asks for the
   previous implementation as evidence — its host ABI signatures, its string layout,
   its edge cases — and a decision made without that evidence is a guess. Read it to
   find out *what a contract was*; never read it to find out *how to structure this
   codebase*.

   Two rules follow: the read is evidence, so what is extracted goes into a document or
   a test, not into a file that resembles the original; and the legacy API is evidence,
   not an obligation — functional parity is tracked in `docs/parity-matrix.md`, and
   binary compatibility is a product decision, not a default.
2. **No hand-written WebAssembly parser.** All binary decoding goes through
   `wasmparser`. Do not add a custom reader, LEB128 decoder, section walker or
   validator. The single allowed exception is the fixed eight-byte binary header,
   because the library exposes no header-only entry point.
3. **No `wasmtime-c-api`, no bindgen, no CMake, no cmake-js.** The Rust API of Wasmtime
   is the only supported embedding.
4. **No async.** No `async fn`, no `async trait`, no Tokio in application code. The
   product is synchronous with `std` threads where concurrency is genuinely needed. A
   transitive Tokio inside `wasmtime-wasi` does not license designing around it.
5. **No speculative abstractions.** Do not add a factory, builder, repository, adapter,
   service, provider, registry or manager for a single implementation. Do not add
   `FileSystem`, `LogicalPath`, `BuildExecutor`, `ArtifactStore` or `PluginManager`
   before a real second use case exists. A new physical crate boundary must be justified
   by a second consumer, a second implementation, or a responsibility that needs to
   evolve independently.
6. **No dead code.** A type, function or module with no caller does not land in this
   repository. Document the idea in `docs/gaps/` instead and implement it with its
   first user.
7. **No CI.** There are deliberately no GitHub Actions workflows. Quality is enforced
   locally by the command block below. CI arrives in PHASE 20 with a real pipeline, not
   before.
8. **Do not claim unimplemented behaviour.** No stubbed commands, no fake outputs, no
   documentation describing a feature that does not exist. `v0.1.0` must not depend on
   undocumented internal tooling.

### Dependency direction

```text
wf-core  <-  wf-wasm  <-  wf-runtime  <-  wf-cli
```

- `wf-core` — **zero dependencies**. Standard library only. No `wasmtime`, no
  `wasmparser`, no `clap`, no `serde`, no `tokio`, no file or process access, no `unsafe`.
- `wf-wasm` — may depend on `wasmparser`. No `wasmparser` type may appear in its public
  API; errors are translated into the crate's own error type.
- `wf-runtime` — may depend on `wasmtime`. No build orchestration, no compiler
  invocation, no source compilation.
- `wf-cli` — argument parsing, output, exit codes. No business logic. No WebAssembly
  parsing.

If a new dependency appears, the crate's `Cargo.toml` comment must explain why it is
allowed to be there.

### Local quality gate

Every change must leave these green:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`rust-toolchain.toml` pins the toolchain, so `rustup` installs it automatically. Nightly
is not used and must not be introduced without a demonstrated need.

### Tests

Test behaviour, not implementation shape.

- **Unit** — domain rules in `wf-core`, analysis in `wf-wasm`.
- **Integration** — `wf-runtime` against real Wasmtime; toolchains against the real
  compiler.
- **End-to-end** — `wf init` → `wf build` → `wf run` → `wf bundle`, driven from the root
  `tests/` directory.

Fixtures live in `fixtures/` and are versioned, never downloaded during a test run.

### Commits

Conventional Commits, message in the language of the surrounding conversation:

```text
type(scope): mensaje
```

Scopes used by this repository: `repo`, `core`, `wasm`, `runtime`, `cli`, `abi`,
`toolchains`, `bundle`, `test`, `docs`.

Keep commits to one coherent work unit. Tests and documentation land in the same commit
as the behaviour they describe.

---

## 📦 Version: v0.3.1

This release (v0.3.1) introduces hardened production readiness with 10 stability
specifications (S1–S10) tracking continuous improvement post-PHASE 21.

### Specs Summary

| Spec | Title | What Covers | Main Files |
|------|-------|-------------|------------|
| **S1** | Supply chain hardening | `cargo-deny` + `cargo-vet` + `cargo-audit` verdes en CI verify | `.github/workflows/supply-chain.yml`, `deny.toml`, `vet.toml` (nuevo) |
| **S2** | Reproducible builds forced | `--locked`, `CARGO_NET_RETRY=3`, `SOURCE_DATE_EPOCH` en **todos** los workflows | `.github/workflows/ci.yml`, `e2e.yml`, `toolchains.yml`, `supply-chain.yml`, `release.yml` |
| **S3** | E2E matrix (toolchain × target × feature) + nightly/weekly | Matrix expandida en e2e.yml y toolchains.yml; schedule nightly/weekly | `.github/workflows/e2e.yml`, `.github/workflows/toolchains.yml` |
| **S4** | JSON output standardizado | `--format json` en `wf build`, `wf run`, `wf bundle` (ya existe en `inspect`) | `crates/wf-cli/src/*.rs` |
| **S5** | Capability-based sandbox | `--sandbox` flag: WASI caps, seccomp (Linux), Job Objects (Windows) | `crates/wf-runtime/src/*`, `crates/wf-cli/src/*` |
| **S6** | Self-hosting bootstrap | `wf bootstrap` compila CLI a WASM + bundle → `wf` auto-hospedado | `crates/wf-cli/src/*`, `crates/wf-bundle/src/*`, nuevo binario |
| **S7** | Release automation | `cargo-release` + changelog generado de commits convencionales | `.github/workflows/release.yml`, `release.toml` (nuevo) |
| **S8** | Build profiling | `--timings` flag emite JSON con fases: parse, codegen, link, bundle | `crates/wf-cli/src/*`, `crates/wf-toolchains/src/*` |
| **S9** | Cross-OS E2E matrix real | Runners `ubuntu-latest`, `macos-latest`, `windows-latest` en toolchains/e2e | `.github/workflows/toolchains.yml`, `.github/workflows/e2e.yml` |
| **S10** | Public contract docs | `docs/toolchain-protocol.md` (adapter interface), `docs/bundle-format-v1.md` (ABNF spec) | `docs/toolchain-protocol.md`, `docs/bundle-format-v1.md` |

### Completed Tasks (T1–T10)

| Task | Spec | Ruta | Commit estimado |
|------|------|------|-----------------|
| T1 | S1 | Añadir `cargo-vet` job a supply-chain.yml; crear `vet.toml` con criteria | `feat(ci): cargo-vet en supply chain` |
| T2 | S2 | Inyectar env: { CARGO_NET_RETRY: "3", SOURCE_DATE_EPOCH: "..." }` + `run: cargo ... --locked` en **5 workflows** | `feat(ci): reproducible builds forced` |
| T3 | S3 | Expandir e2e.yml: matrix (toolchain × target × feature); añadir `schedule: cron nightly/weekly` | `feat(ci): e2e matrix + nightly` |
| T4 | S4 | Extender `wf build/run/bundle` con `--format json` (reutilizar pattern de `inspect`) | `feat(cli): json output all commands` |
| T5 | S5 | Añadir `--sandbox` a `wf run` y `wf bundle`; implementar caps/seccomp/JobObjects en runtime | `feat(runtime): capability sandbox` |
| T6 | S6 | Crear `wf bootstrap` subcommand; compilar workspace a wasm32 + bundle con wf-bundle | `feat(cli): wf bootstrap self-hosting` |
| T7 | S7 | Configurar `cargo-release` (release.toml); actualizar release.yml para usarlo | `feat(ci): cargo-release + changelog` |
| T8 | S8 | Añadir `--timings` global flag; instrumentar fases en cli/toolchains/bundle; emitir JSON | `feat(cli): --timings profiling` |
| T9 | S9 | Cambiar `runs-on: ubuntu-latest` → matrix `['ubuntu-latest', 'macos-latest', 'windows-latest']` en toolchains.yml y e2e.yml | `feat(ci): cross-os e2e matrix` |
| T10 | S10 | Escribir `docs/toolchain-protocol.md` (interface adapter, JSON schema); `docs/bundle-format-v1.md` (ABNF, footer, checksums) | `docs: toolchain protocol + bundle format v1 spec` |

### Log

- **L1** (2026-11-10): "Vale, vamos con lo siguiente: [10 items] — Vamos a iniciar un nuevo proceso odd para todo esto."
- **L2** (2026-11-10): Documento ODD creado con 10 specs (S1–S10) y 10 tasks (T1–T10). Tracking iniciado.

---

## 🏗️ Architecture

Five crates, with a strict dependency direction:

```text
wf-core  <-  wf-wasm  <-  wf-runtime  <-  wf-cli
```

Each crate may only depend on the ones below it. The full rules, including the banned
dependencies per crate, are in [`docs/architecture.md`](docs/architecture.md).

### Crate responsibilities

| Crate | Responsibility | Key dependency |
|-------|---------------|----------------|
| `wf-core` | Domain vocabulary and pure rules. Zero dependencies. | none |
| `wf-wasm` | Analysis of a WebAssembly binary: kind, imports, exports, validation. | `wasmparser`, `wit-parser`, `wit-component` |
| `wf-runtime` | Execution of an analysed module under Wasmtime. | `wasmtime` |
| `wf-toolchains` | Guest toolchains: Rust, C++, AssemblyScript, precompiled wasm, components, plugins. | `toml` (guest crate manifests) |
| `wf-bundle` | Bundle format v1, launcher probing, checksum validation. | `sha2`, `blake3` |
| `wf-cli` | The `wf` command line interface. | `clap` |

### Building from source

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Usage

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

# Keep the built artifact in sync with the sources: one build per quiet moment and
# never a rebuild triggered by an output write.
wf build --watch

# Wrap the built project into one self-contained native bundle.
# `wf` never builds the launcher; you give it one that already exists.
# cargo build -p wf-bundle --bin wf-launcher
# wf bundle --launcher target/debug/wf-launcher

# Or resolve a prebuilt launcher from the local registry by target triple.
# wf bundle --bundle-target x86_64-unknown-linux-gnu

# `wf build` uses content fingerprints: an unchanged module is not recompiled.
# See `docs/adr/0001-cache-fingerprint-blake3.md`. After upgrading a guest compiler
# (clang++/asc/rustc), clear the stale artifacts: `rm -rf target`.

# Analyse any binary without executing it, for a human or as JSON for other tools.
wf inspect target/hello.wasm
wf inspect target/hello.wasm --format json

# Build a WebAssembly Component
wf component build /path/to/component-project

# Manage plugins (Wasm Components implementing wf:plugin/guest)
wf plugin init my-plugin
cd my-plugin
wf plugin build

# Validate a WIT file
wf wit check path/to/file.wit
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

The crate must declare `[lib] crate-type = ["cdylib"]`, which is what makes `cargo`
emit a WebAssembly module. A project is otherwise one file, `wasmfoundry.toml`:

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

A project may state what its runtime is allowed to do:

```toml
[runtime]
wasi = true                     # denied unless asked for
wall_clock_seconds = 5.0        # no bound unless asked for
memory_pages = 4096             # one page is 64 KiB

[[mount]]
host = "./data"
guest = "/data"                 # absolute: a guest path is a name in the guest's world
access = "read-only"            # the default
```

Everything is denied unless the manifest asks. A guest that imports WASI without
`wasi = true` builds fine and fails when it calls — asking for a capability and being
granted it are two different things.

A project may hold several modules. An import names a namespace, and a module's namespace
is either stated in the manifest or derived from the matching mode:

```toml
[project]
source_dir = "src"
entry = "_start"
module_matching = "file-name"      # or "name-only"
host_namespaces = ["env"]          # namespaces expected from outside the project

[[module]]
name = "engine"
source = "src/cog.wasm"
namespace = "engine"               # stated: beats both the name and the file
```

A stated `namespace` wins in every mode. Without one:

- `file-name` — the namespace is the source file's name without its extension
- `name-only` — the namespace is the module's `name`

The same rule binds an import and publishes an export, so the two sides cannot disagree.

`wf build` refuses a project whose imports cannot be bound, because an import that names
no project module and was not declared as external is most likely a typo:

- a namespace claimed by two modules — `WF002`, naming the claimants
- an import nothing provides and nothing declares — `WF002`
- a dependency cycle — `WF003`

Declaring a namespace in `host_namespaces` is how a guest that talks to a host builds:
those namespaces are not project modules and never will be.

A manifest whose `schema` this version does not know is rejected outright: it is never
partially interpreted, because fields shared between two schemas may have changed meaning.
Unknown table and field names are also rejected, so a typo cannot silently do nothing.

### Diagnostic codes (stable interface)

| Code | Meaning |
|------|---------|
| `0` | The command succeeded |
| `1` | The input was understood but rejected, reported as `WF001` |
| `2` | The command line or the input path was wrong |

| Code | Cause |
|------|-------|
| `WF001` | The input is not a compilable WebAssembly module |
| `WF002` | An import has no provider, or cannot be bound to one module |
| `WF003` | The project's modules form a dependency cycle |
| `WF004` | The toolchain a module asks for is not available |
| `WF005` | The requested entry point is not exported |
| `WF009` | The guest returned while executing |
| `WF010` | The entry point exists but cannot be invoked as written |
| `WF011` | The manifest does not mean what it appears to mean |
| `WF012` | A toolchain ran and did not produce a module |
| `WF013` | A host function failed while the guest was calling it |
| `WF014` | An invocation exceeded a bound the policy set |

A guest returning a value from its entry point is reported as output, not interpreted into
a process exit code: that mapping is a host-ABI decision, and the host ABI does not exist
yet. Exiting 0 here is the honest answer for a successful invocation.

### Status

The rewrite proceeds in vertical slices. Each phase delivers one observable capability
and must pass its gate before the next begins.

| Phase | Delivers | Gate |
|-------|----------|------|
| 0–1 | Legacy state preserved, `main` reset | ✅ done |
| 2 | Rust workspace, four crates, documentation | ✅ done |
| 3 | `wf inspect` | ✅ done |
| 4 | `wf run` | ✅ done |
| 5 | `wf init`, `wf build` for precompiled wasm | ✅ done |
| 6 | Rust guest toolchain | ✅ done |
| 7 | Dependency graph and module matching | ✅ done |
| 8–9 | Host ABI v1, WASI, mounts, capability policy | ✅ done |
| 10–11 | C++ and AssemblyScript toolchains, `wf-toolchains` crate | ✅ done |
| 12–13 | Build cache (BLAKE3), `wf build --watch` | ✅ done |
| 14–16 | Bundle format v1, prebuilt launchers, cross-target | ✅ done |
| 17–18 | Component Model + WIT + External plugins | ✅ done |
| 19 | Security tooling + supply chain (`cargo-deny`, `cargo-audit`) | ✅ done |
| 20 | CI/CD (GitHub Actions) | ✅ done |
| 21 | Release pipeline + `v0.1.0` | ✅ done |

Functional parity with the previous generation of this project is tracked explicitly in
[`docs/parity-matrix.md`](docs/parity-matrix.md), including the capabilities that are
deliberately **not** being carried over.

### Documentation

- [`docs/product.md`](docs/product.md) — what the product is, non-goals, security boundary
- [`docs/architecture.md`](docs/architecture.md) — crate responsibilities and dependency rules
- [`docs/parity-matrix.md`](docs/parity-matrix.md) — legacy capability mapping and decisions
- [`docs/gaps/`](docs/gaps/) — gaps discovered during the rewrite
- [`docs/adr/`](docs/adr/) — architecture decision records
- [`AGENTS.md`](AGENTS.md) — repository operating instructions for coding agents

### Security

Two trust boundaries exist in this product, and they are not the same thing.

**`wf run` executes a guest.** Wasmtime's sandbox applies: the guest has no filesystem,
network, environment or process access, because there is nothing to give it. There is no
host ABI yet, so every import fails to resolve rather than reaching the host.

**`wf run` is bounded when the project asks for it.** A wall-clock budget and a memory
ceiling are set in the manifest, and a guest that crosses one is stopped and reported as
`WF014`. A project that sets neither runs unbounded, which is the honest reading of
asking for no bound.

**`wf build` will not sandbox anything.** When it arrives in PHASE 5 it will run the guest
toolchain — `build.rs`, compilers, package managers, scripts — with the permissions of
your account. Wasmtime's sandbox does not protect against those processes, and the product
never claims it does.

**`wf inspect` does not execute what it reads.** See
[`docs/product.md`](docs/product.md#security-boundary); `docs/security/threat-model.md`
arrives with the capability policy in PHASE 9.

### License

MIT. See [LICENSE](LICENSE).

---

## Stability & Reliability Improvements (post v0.2.0)

This release (v0.3.1) introduces hardened production readiness with 10 stability
specifications (S1–S10) tracking continuous improvement:

- **S1 Supply chain hardening**: `cargo-deny`, `cargo-vet`, and `cargo-audit` verified in CI
- **S2 Reproducible builds forced**: `--locked`, `CARGO_NET_RETRY=3`, `SOURCE_DATE_EPOCH` in all workflows
- **S3 E2E matrix**: Expanded matrix (toolchain × target × feature × OS) + nightly/weekly schedule
- **S4 JSON output**: `--format json` on `wf build`, `wf run`, `wf bundle`
- **S5 Capability-based sandbox**: `--sandbox` flag: WASI caps, seccomp (Linux), Job Objects (Windows)
- **S6 Self-hosting bootstrap**: `wf bootstrap` compiles CLI to WASM + bundle → auto-hospedado `wf`
- **S7 Release automation**: `cargo-release` + conventional commits changelog
- **S8 Build profiling**: `--timings` flag emits JSON with build phase timings
- **S9 Cross-OS E2E matrix**: `ubuntu-latest`, `macos-latest`, `windows-latest` runners
- **S10 Public contract docs**: `docs/toolchain-protocol.md` (adapter interface), `docs/bundle-format-v1.md` (ABNF spec)

See [AGENTS.md](AGENTS.md) for operating instructions, and
[odd/tasks/stability-reliability-improvements.md](odd/tasks/stability-reliability-improvements.md) for the full task tracking.

---

Commit: `a36ebe9` — docs: complete redesign README.md for v0.3.1 with S1-S10 improvements and full product description
Tags: `v0.3.1`
Workflow: All workspace checks pass:
- `cargo fmt --all -- --check` ✅
- `cargo check --workspace` ✅
- `cargo test --workspace` ✅
- `cargo clippy --workspace --all-targets -- -D warnings` ✅
