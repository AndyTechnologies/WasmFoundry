# AGENTS.md

Operating instructions for coding agents working in this repository. Read this before
writing code.

## What this repository is

WasmFoundry is a Rust toolchain for building, inspecting and running WebAssembly
projects. It is a rewrite from scratch of a previous TypeScript implementation; that
implementation is **historical reference only** and lives in a separate, frozen
repository.

## Hard rules

Violating any of these is a defect, regardless of how convenient the alternative is.

1. **Never touch the legacy repository.** It is a separate repository on its own branch
   and tag, kept as the frozen historical record. Do not open it, do not write to it, do
   not "fix" it, do not copy code out of it. It is evidence, not a dependency.
2. **No hand-written WebAssembly parser.** All binary decoding goes through
   `wasmparser`. Do not add a custom reader, LEB128 decoder, section walker or
   validator. The single allowed exception is the fixed eight-byte binary header, because
   the library exposes no header-only entry point.
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
   repository. Document the idea in `docs/gaps/` instead and implement it with its first
   user.
7. **No CI.** There are deliberately no GitHub Actions workflows. Quality is enforced
   locally by the command block below. CI arrives in PHASE 20 with a real pipeline, not
   before.
8. **Do not claim unimplemented behaviour.** No stubbed commands, no fake outputs, no
   documentation describing a feature that does not exist. `v0.1.0` must not depend on
   undocumented internal tooling.

## Dependency direction

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

## Local quality gate

Every change must leave these green:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`rust-toolchain.toml` pins the toolchain, so `rustup` installs it automatically. Nightly
is not used and must not be introduced without a demonstrated need.

## Tests

Test behaviour, not implementation shape.

- **Unit** — domain rules in `wf-core`, analysis in `wf-wasm`. No real engine.
- **Integration** — `wf-runtime` against real Wasmtime; toolchains against the real
  compiler.
- **End-to-end** — `wf init` → `wf build` → `wf run` → `wf bundle`, driven from the root
  `tests/` directory.

Fixtures live in `fixtures/` and are versioned, never downloaded during a test run.

## Commits

Conventional Commits, message in the language of the surrounding conversation:

```text
type(scope): mensaje
```

Scopes used by this repository: `repo`, `core`, `wasm`, `runtime`, `cli`, `abi`,
`toolchains`, `bundle`, `test`, `docs`.

Keep commits to one coherent work unit. Tests and documentation land in the same commit
as the behaviour they describe.

## When you discover something the plan does not cover

Do not quietly deviate. Classify the gap, then either resolve it or record it as
`docs/gaps/GAP-XXXX.md` using the template in `docs/gaps/README.md`. If it changes a
public architectural decision, add an ADR in `docs/adr/`. If it blocks the current
milestone, stop and ask.

## Reference material

- `plan.md` — the master plan this rewrite follows, in full.
- `docs/parity-matrix.md` — what carries over from the legacy implementation, what
  changes, what is dropped and why.