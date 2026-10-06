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

`wf inspect` (PHASE 3) is the first consumer, and it is safe against malformed input by
construction: the header is checked, then the module is run through
`Validator::validate_all` in full, and only a module the validator accepts is described.
Declared sizes, counts, offsets and names are all attacker-controlled, so none of them is
trusted before that point.

What the analysis reports, and why each field earns its place:

- **imports keep their class** — function, table, memory or global, with the type that
  goes with it. A host that satisfies `env.console_log` with the wrong kind of entity has
  not satisfied anything.
- **exports carry the index they publish**, not their position in the export section.
  A module may import functions and export only some of them, so position and index
  diverge; resolving a function signature through position would be a guess.
- **unbounded limits are explicit**. A memory with no maximum is reported as `null` in
  JSON, not as a missing key, because absent and unbounded are different facts.

The `Error` type carries its own messages and offsets so that no `wasmparser` type reaches
the caller, and the diagnostic code `WF001` is what the CLI prints.

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

Wasmtime types do not escape into the domain. `wf-runtime` owns its engine, so neither
the engine nor the compiled module nor the store reaches `wf-cli`; the public surface is
`Runtime`, an opaque `Module`, `RunOutcome` and `RuntimeError`. That keeps the embedding
replaceable: a caller cannot depend on Wasmtime because it never sees Wasmtime.
`RuntimePolicy` (PHASE 9) will describe capabilities in domain terms
(`FilesystemAccess::ReadOnly`, and so on) and an adapter will translate them into whatever
`wasmtime-wasi` exposes.

The version is tracked as a constraint in `Cargo.toml` (`^49.0.1`) with the exact resolved
version in `Cargo.lock`, so a patch release lands by ordinary means rather than by a hand
edit. Nightly is not used, and the MSRV is whatever the heaviest dependency actually
requires — currently 1.96, imposed by `wasmtime` 49.

### Why the manifest is parsed outside the domain

```text
wasmfoundry.toml
    ↓  wf-cli reads and parses (toml, serde)
Manifest          plain data: schema, package, project, modules
    ↓  wf-core validates (pure rules)
Vec<Diagnostic>   every structural problem, with advice
    ↓  wf-cli resolves toolchains and files
artifacts in target/
```

The split keeps the domain free of a format dependency while still making the rules
shared. It also keeps the failure modes distinct: a syntax error and a structurally
unusable manifest are reported as the same code but with different messages, because they
are fixed by different edits.

### Why the error type has one variant per cause

A run can fail for five unrelated reasons, and each one is fixed by a different action:

```text
InvalidWasm     the bytes never compiled            fix the input
Link            an import has no provider           add a host function (PHASE 8)
MissingExport   the guest does not export it        fix --entry or the guest
Trap            the guest faulted while running     debug the guest
Configuration   the export cannot be invoked        fix the guest's signature
```

Two causes from the plan's taxonomy are absent on purpose: a host error needs a host
function, and an execution-limit error needs limits. Both arrive with their own cause
rather than as empty variants nobody can produce.

Every message is built by walking the error's cause chain. Wasmtime reports the context
first — `error while executing at wasm backtrace:` — and the reason one level down.
Taking only the top of the chain would print a backtrace header with no fault in it, which
is the generic failure the plan forbids with different words.

## Who owns the project pipeline

`wf init` and `wf build` parse the manifest, validate it and publish artifacts. All three
live in `wf-cli`, and that is a deliberate, reversible choice rather than an endorsement:

- **Parsing stays in the CLI** because `wf-core` has no dependencies and may not learn
  what a TOML table looks like. The CLI reads the file and hands the domain plain data.
- **Validation stays in the domain** because whether a manifest *means something usable*
  is a rule, and rules belong where every command can share them.
- **Orchestration sits in the CLI for now** because there is no second consumer of it, and
  §5 forbids a crate boundary without one. When `wf-toolchains` arrives (PHASE 10) or a
  cache needs to reason about build steps (PHASE 12), the pipeline moves out and `wf-cli`
  returns to argument parsing and output only.

The `Toolchain` trait is likewise not here yet: PHASE 5 has exactly one toolchain, and a
trait with one implementation draws a boundary nothing has crossed. It lands in PHASE 6 as
the second implementation arrives.

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