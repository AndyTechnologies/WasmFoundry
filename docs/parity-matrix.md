# Parity matrix

What carries over from the previous generation of this project, what changes, and what is
dropped on purpose.

**Compatibility with the legacy implementation is not a goal.** Functional parity is: the
capability either exists in WasmFoundry with an equivalent contract, or it is explicitly
replaced, or it is dropped with a stated reason. Nothing is carried over silently.

State values:

| State | Meaning |
| --- | --- |
| `pending` | Must exist, not implemented yet |
| `deferred` | Deliberately later than `v0.1.0` |
| `dropped` | Will not exist; the capability is replaced or obsolete |
| `done` | Implemented and verified |

## Guest languages

| Capability | State | Decision | Phase |
| --- | --- | --- | --- |
| AssemblyScript | pending | preserve, behind a `Toolchain` adapter that owns `asc`, runtime selection, optimisation and source maps | 11 |
| C++ | pending | preserve, behind a toolchain adapter; one backend for MVP, not five | 10 |
| Rust guest | done | preserved for core wasm: `wf build` invokes `cargo` for `wasm32-unknown-unknown`; WASI stays a separate explicit path (PHASE 9) | 6 |
| Precompiled wasm | done | preserved: `wf init` scaffolds one, `wf build` validates and publishes it, `wf run` executes it | 5 |

## Configuration

| Capability | State | Decision | Phase |
| --- | --- | --- | --- |
| `moduleMatching=file-name` | done | preserved as `ModuleMatching::FileName`, the default; the manifest states it and validation rejects anything else | 7 |
| `moduleMatching=name-only` | done | preserved as `ModuleMatching::NameOnly`; an import claiming two modules is refused rather than bound to whichever listed first | 7 |
| Per-toolchain configuration | done | preserved, keyed by `ToolchainId` instead of toolchain-name conditionals | 6 |
| multi-module composition | done | the graph is resolved by name and instantiated through Wasmtime's linker; no C++ is generated to represent it | 7 |
| explicit per-module namespace | done | new, and the reason the derived rule is not the only answer: a module can be reported as one thing and published as another | 7 |
| Legacy `wapp.json` format | done | replaced by `wasmfoundry.toml` with an explicit `schema` version; an unknown schema is rejected, never partially read | 5 |

## Runtime and host

| Capability | State | Decision | Phase |
| --- | --- | --- | --- |
| console | pending | preserve in the portable Host ABI | 8 |
| fs | pending | replant; host ABI plus capability policy, defaults deny | 8–9 |
| WASI | pending | preserve; configured from `RuntimePolicy`, never from Wasmtime types in the domain | 9 |
| mounts | pending | preserve and improve; access defaults to read-only instead of read-write | 9 |
| Resource limits | pending | preserve; memory limit and wall-clock timeout in the MVP, fuel later | 9 |
| Host ABI versioning | pending | new; ABI v1 is never modified in place | 8 |

## Build behaviour

| Capability | State | Decision | Phase |
| --- | --- | --- | --- |
| optimisation | pending | preserve; compiler optimisation and post-build wasm optimisation stay separate concerns | 10–11 |
| sourcemaps | pending | preserve; debug metadata is never stripped during analysis | 10–11 |
| cross compilation | pending | preserve, split from guest target; bundles use prebuilt launchers so no host toolchain is required | 16 |
| cache | deferred | implement after measuring cold and warm builds with at least two real toolchains | 12 |
| watch | deferred | full rebuild first, graph invalidation later | 13 |

## Deliberately not carried over

| Legacy capability | State | Decision | Reason |
| --- | --- | --- | --- |
| plugins (JS, runtime plugins) | dropped | replaced by internal extension points: `Toolchain`, runtime extensions, bundler backends | External plugins before there are real internal implementations build a framework with no users |
| custom templates (Nunjucks) | dropped | removed entirely | The capability they provided is delivered by the runtime, the Host ABI and the bundle format instead of generated C++ |
| C++ code generation | dropped | removed entirely | Imports resolve by name in the linker; generating C++ makes a C toolchain a build requirement |
| CMake linker | dropped | removed entirely | Same as above, plus it makes linking depend on a system package that WasmFoundry does not control |
| Wasmtime C API integration | dropped | replaced by the Wasmtime Rust API | The C API required bindgen, CMake and a downloaded library; the Rust API is the supported embedding |
| custom WASM parser | dropped | replaced by `wasmparser` | A hand-written header, section, LEB128 and type reader is strictly worse than the maintained library. The eight-byte header is the only exception, because the library has no header-only entry point |
| npm release and publish pipeline | dropped | rebuilt in PHASE 20–21 | The legacy automation belongs to the previous stack; CI is reintroduced once the product is real |
| RmlUi DOM/UI host bindings | dropped | out of scope for `v0.1.0` | A UI framework binding is a product decision beyond this rewrite; the capability lives in the unmerged feature branches of the legacy repository, which remain frozen there |

## Status

`done` means implemented with tests. Rows still `pending` belong to phases the rewrite
has not reached.
The matrix is updated as each phase lands. No row may be marked `done` without a test
that exercises the capability.