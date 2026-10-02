# Architecture

WasmFoundry is four crates in a strict dependency direction. The boundaries are not
decorative: each one exists so that a specific kind of change stays inside one crate.

## Crate map

```text
                 wf-cli
                   |
                wf-runtime
                   |
                 wf-wasm
                   |
                 wf-core
```

| Crate | Owns | Never owns |
| --- | --- | --- |
| `wf-core` | Domain vocabulary, pure rules, graph traversal, fingerprints, diagnostics | Bytes, files, processes, engines, CLI concerns |
| `wf-wasm` | Everything asked *about* a WebAssembly binary | Execution, build orchestration |
| `wf-runtime` | Engine, module, store, linker, instance, entry invocation | Parsing, compiling, CLI concerns |
| `wf-cli` | Argument parsing, terminal output, exit codes | Domain logic of any kind |

Two crates will be added later, when a real second use case justifies the boundary:
`wf-toolchains` (PHASE 10, when a second real toolchain exists) and `wf-bundle` (PHASE
14). Until then they do not exist, and no placeholder crate is reserved for them.

## Dependency rules

| Crate | May depend on | Forbidden |
| --- | --- | --- |
| `wf-core` | `std` only | `wasmtime`, `wasmparser`, `clap`, `serde`, `tokio`, filesystem, process, network, `unsafe` |
| `wf-wasm` | `wasmparser`, `wf-core` | Exposing `wasmparser` types in the public API, execution, CLI logic |
| `wf-runtime` | `wasmtime`, `wf-core`, `wf-wasm` | Source compilation, compiler invocation, CLI logic, filesystem build orchestration |
| `wf-cli` | `clap`, `wf-core`, `wf-wasm`, `wf-runtime`, `wf-wasm` (via analysis) | WebAssembly parsing, engine internals, business rules |

A new crate boundary is justified by exactly one of: a second consumer, a second
implementation, or a responsibility that must evolve independently. None of those is
true today for anything beyond the four crates above.

## The pure domain

`wf-core` is the only crate every other crate may depend on, and it depends on nothing.
Its discipline is simple: a pure domain function takes values and returns values. It does
not know where bytes live, how a guest is executed, or how a binary format is printed.

```rust
// allowed
pub fn topological_order(&self) -> Result<Vec<ModuleId>, CycleError>;
pub fn detect_cycle(&self) -> Option<Vec<ModuleId>>;
pub fn calculate_fingerprint(inputs: &FingerprintInputs) -> Fingerprint;

// forbidden: each of these belongs to another crate
pub fn compile_with_cargo(&self) -> ...;
pub fn execute_wasmtime(&self) -> ...;
pub fn read_file(&self, path: &Path) -> ...;
```

### Vocabulary roadmap

Types are introduced together with the feature that consumes them. Writing them earlier
freezes names and invariants that real call sites would have chosen differently.

| Type | Purpose | Introduced in |
| --- | --- | --- |
| `Diagnostic` | Machine-readable problem: code, severity, message, location | PHASE 3 |
| `EntryPoint` | The guest export an invocation starts at, not a bare string | PHASE 4 |
| `Artifact` | Logical result of a build step, independent of where bytes live | PHASE 5 |
| `ArtifactId` | Stable identity of an artifact inside a graph | PHASE 5 |
| `ArtifactKind` | What an artifact is, deciding who may consume it | PHASE 5 |
| `BuildProfile` | Knobs a build varies on, without naming a compiler flag | PHASE 5 |
| `ToolchainId` | Identity of a guest toolchain | PHASE 6 |
| `Target` | Normalised guest target triple, independent of the host | PHASE 6 |
| `ModuleId` | Identity of a module inside a project | PHASE 7 |
| `Dependency` | Directed edge between modules, carrying the reason | PHASE 7 |
| `DependencyGraph` | Pure graph plus roots, leaves, order, cycle detection | PHASE 7 |
| `RuntimePolicy` | Capabilities one invocation is allowed | PHASE 9 |
| `Fingerprint` | Content-derived identity of a build input | PHASE 12 |

`Artifact` is deliberately not `Vec<u8>`. The domain records that an artifact exists and
what it is; a later layer decides where its bytes live. There is no `ArtifactStore`
until something actually needs one.

### Guest target and host target

They are different concepts and never share a single `target` field:

```text
guest: wasm32-unknown-unknown | wasm32-wasip1
host:  x86_64-unknown-linux-gnu | aarch64-apple-darwin
```

Cross-compiling a bundle for another host triple must never require the user's Rust
toolchain for that host. That is exactly why bundles use prebuilt launchers (PHASE 14).

## WebAssembly analysis

`wf-wasm` is a thin opinionated layer over `wasmparser`, and the rule is that **no
`wasmparser` type appears in its public API**. Callers depend on `wf-wasm`, not on the
parser underneath, so the analysis dependency can change without touching `wf-runtime`
or `wf-cli`. Parser errors are translated into `wf-wasm`'s own error type.

Binary decoding, validation and type inspection are delegated entirely. The one place
bytes are read directly is the fixed eight-byte header — magic, version, format layer —
because the library exposes no header-only entry point and a caller that only needs to
tell a core module from a component should not pay for a full parse.

Imports and exports keep their type. The four classes — function, table, memory, global
— are distinguished by the parser and preserved, because multi-module resolution depends
on knowing that a global is not a function.

`wf inspect` (PHASE 3) is the first consumer. It must be safe against malformed input:
declared sizes, counts, offsets and names are all attacker-controlled.

## Runtime

`wf-runtime` embeds Wasmtime through its Rust API. There is no C API, no bindgen, no
CMake, no downloaded library.

```text
Engine    compilation and resource configuration; shared, long lived
  |
Module    a compiled binary; cheap to instantiate repeatedly
  |
Store     mutable state for one logical execution
  |
Linker    name resolution for the imports a module declares
  |
Instance  a running shape of a module inside one store
  |
entry     the exported function that starts the guest
```

**No `Store` is global.** A store carries execution state; a process-wide store would
leak one guest's memory into the next invocation and make concurrent runs impossible.
The `Engine` may be shared because it holds configuration, not state. Everything from
`Store` downwards is created per invocation.

Wasmtime types do not escape into the domain. `RuntimePolicy` describes capabilities in
domain terms (`FilesystemAccess::ReadOnly`, and so on); an adapter translates that into
whatever `wasmtime-wasi` currently exposes. WasmFoundry tracks a stable version
constraint in `Cargo.toml` and an exact version in `Cargo.lock`, rather than pinning a
version permanently by hand.

## Toolchains

The first real `Toolchain` trait arrives when `precompiled` and Rust are both close
(PHASE 6). It stays small:

```rust
trait Toolchain {
    fn id(&self) -> ToolchainId;
    fn detect(&self, source: &Source) -> Detection;
    fn compile(&self, request: CompileRequest) -> Result<CompileResult>;
}
```

No `watch`, no `cache`, no `optimization`, no `doctor`, no `cross` in the first version.
The rest of the system never constructs a compiler command directly; the adapter owns
invocation, target selection, features, diagnostics and artifact discovery.

A backend is not abstracted until a second implementation exists. C++ (PHASE 10) ships
with exactly one backend that covers the MVP case; `CompilerBackend`, `Sysroot` and
`Linker` become concepts when a second one shows up, not before.

## Multi-module resolution

```text
sources
  -> compile
  -> analysis
  -> module resolution
  -> dependency graph
  -> runtime plan
  -> Wasmtime Linker
```

Module matching is an explicit domain concept, not an implicit convention:

```rust
enum ModuleMatching {
    FileName,
    NameOnly,
}
```

The linker resolves imports by name; no C++ is generated to represent the process.

## Deliberate non-choices

These are not pending work. Each was rejected for a reason:

| Not built | Why |
| --- | --- |
| Code generation for linking | Imports resolve by name in the linker; generating C++ drags a C toolchain into every build |
| Custom templates | The capability templates provided is replaced by the runtime, the host ABI and the bundle format |
| A plugin system | The first extensibility is internal: the `Toolchain` trait and runtime extensions. An external plugin protocol waits for a real case |
| A cache before measuring | PHASE 12 requires at least two real toolchains and cold/warm measurements first |
| Incremental watch | PHASE 13 starts with a full rebuild; graph invalidation comes after |
| A build planner | The pipeline runs directly; a formal plan is extracted only when a second need appears |
| A filesystem abstraction | Real filesystem and `tempfile` in tests. An in-memory or sandboxed filesystem needs a real caller first |
| Component model in core support | PHASE 17 expands the system; core module support stays the stable base |