# Product

WasmFoundry is a WebAssembly toolchain that treats guest languages, module composition
and host capabilities as explicit, versioned, replaceable parts of one system.

It is **not** "another WebAssembly runtime" and **not** "another WebAssembly compiler".
Its value is the combination:

```text
multi-toolchain  +  multi-module composition  +  host ABI  +  capability runtime
                +  standalone native bundles
```

Any one of those exists elsewhere. Holding all five together, with the host surface
documented and the runtime permissions explicit, is the product.

## Who it is for

A developer with a WebAssembly project that has outgrown a single toolchain or a single
module, and who needs to reason about what the guest can actually do once it runs.

## The problems it takes seriously

**Toolchains leak into the build core.** Adding a guest language normally means touching
the orchestrator. Here, a language is an adapter behind one trait, and the orchestrator
never branches on a language name.

**Multi-module linking is done by generating C++.** The previous generation of this
project re-declared every import in generated C and compiled it. That made a C toolchain
a build requirement in order to link two `.wasm` files, and it hid the real resolution
problem behind a code generator. WasmFoundry resolves imports by name through a linker
and instantiates the modules directly.

**The host surface is whatever the bindings happened to export.** Guests receive an
undocumented surface with no version, no declared capabilities and no ownership rules.
WasmFoundry defines a Host ABI with a version, splits it into a portable ABI and
language-compatibility ABIs, and derives it from observed behaviour of real artifacts
rather than from assumptions about string encoding.

**"Sandboxed" is claimed too broadly.** A runtime sandbox says nothing about what
`wf build` does.

## Vertical slices

Each phase delivers one observable capability and must pass its gate before the next.

| Phase | Slice | Gate |
| --- | --- | --- |
| 0–1 | Legacy preservation, `main` reset | ✅ |
| 2 | Rust workspace, four crates, documentation | ✅ |
| 3 | `wf inspect` — analyse a binary without running it | ✅ imports, exports, memory, tables, globals, core vs component |
| 4 | `wf run` — execute a core module | ✅ Engine → Module → Store → Linker → entry, no WASI |
| 5 | `wf init`, `wf build` for precompiled wasm | ✅ project concept, `wasmfoundry.toml` |
| 6 | Rust guest toolchain | ✅ Rust source → wasm → run |
| 7 | Dependency graph, module matching | module A imports module B, both run |
| 8 | Host ABI v1 | documented namespaces, signatures, ownership |
| 9 | WASI, mounts, capability policy, execution limits | denied by default, granted explicitly, bounded execution |
| 10 | C++ toolchain | C++ source → wasm → run |
| 11 | AssemblyScript toolchain | AS source → wasm → run |
| 12 | Cache | cold miss, warm hit, correct invalidation |
| 13 | Watch | rebuild on change |
| 14–15 | Bundle POC, then Bundle Format v1 | checksum passes, corruption rejected |
| 16 | Prebuilt launchers, cross-target bundles | bundle for a host without that toolchain |
| 17 | Component model, WIT | component instantiation and run |
| 18 | External extensibility | only with a real case |
| 19 | Supply chain tooling | `cargo-audit`, `cargo-deny`, SBOM |
| 20–21 | CI, release pipeline, `v0.1.0` | reproducible, tested, releasable |

`v0.1.0` is reachable once `wf inspect`, `wf run`, `wf build`, the precompiled toolchain,
the Rust toolchain, a basic host ABI and coherent documentation exist. Cache, plugins,
components and full cross-compilation are explicitly **not** release blockers.

## Non-goals

- **Not a browser runtime.** No DOM, no JavaScript host, no embedding in a page.
- **Not a general plugin platform.** The first extensibility is internal: the `Toolchain`
  trait and runtime extensions. An external plugin protocol arrives only with a real
  consumer.
- **Not a WebAssembly code generator.** The product resolves modules; it does not emit
  host language source to represent them.
- **Not a performance-first engine.** Wasmtime is used because its model fits the
  capability requirements, not to compete on raw execution speed.
- **Not a drop-in replacement for the legacy CLI.** Compatibility is functional, tracked
  explicitly in `docs/parity-matrix.md`. Command names, config format and API are new.

## Security boundary

This is the distinction the product is most careful about, because conflating it produces
false promises.

**`wf run` is sandboxed, with a narrower boundary than it will have.** A guest runs under
Wasmtime and reaches nothing: there is no host ABI yet, so any import fails to resolve
and no filesystem, network, environment or process access is reachable at all. That is
true isolation by absence, not by policy.

What is missing is **execution limits**. PHASE 9 adds the wall-clock timeout and the
memory cap of §60. Until then a guest may run forever or grow without bound, and
`wf run` imposes neither. Until that exists, a module should be treated as untrusted
input to be run only where unbounded execution is acceptable.

When the capability policy lands, it stays deny-by-default: filesystem, network,
environment and host process execution are all granted explicitly, never inferred.

**`wf build` is not sandboxed.** Building a project runs the guest toolchain: `cargo`,
`build.rs`, compilers, package managers, and any script the project defines. Those
processes run with the permissions of the user who invoked `wf build`. The Wasmtime
sandbox does not protect against them, and the product never claims it does.

Since PHASE 6, `wf build` invokes `cargo`, so building a Rust project executes its
`build.rs` with the permissions of the user who ran `wf build`. Nothing about that is
sandboxed, and the runtime sandbox is not consulted during a build at all. What the
manifest *can* say is bounded: `toolchain` resolves to a toolchain this version knows,
never to a program or a path.

A project whose sources you do not trust is a different risk class from a `.wasm` file
you do not trust. `docs/security/threat-model.md` will classify each explicitly once it
exists (PHASE 19 carries the tooling; the threat model lands with the runtime security
work in PHASE 9).

## Versioning

Four versions move independently and a release of one does not imply a break in the
others:

```text
WasmFoundry version     the CLI and the crates
Host ABI version        the guest-visible host surface
Bundle format version   the self-contained executable format
Manifest schema version wasmfoundry.toml
```

Additive Host ABI changes are compatible. A changed signature, memory layout or ownership
rule requires Host ABI v2, and v1 is never modified in place. A launcher refuses an
unknown bundle format version instead of guessing. A manifest with an unknown schema
version is rejected, never partially interpreted.

## Documentation

- [`architecture.md`](architecture.md) — crate boundaries and dependency rules
- [`parity-matrix.md`](parity-matrix.md) — what carries over from the previous generation
- [`gaps/`](gaps/) — gaps found during the rewrite
- [`adr/`](adr/) — architecture decision records