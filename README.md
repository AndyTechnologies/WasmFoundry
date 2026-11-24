# WasmFoundry

**A WebAssembly toolchain where guests are adapters, module composition is explicit, and capabilities are deny-by-default.**

WasmFoundry builds, inspects, and runs WebAssembly projects. It treats guest languages (Rust, C++, AssemblyScript, precompiled WASM, components) as pluggable adapters — not hardcoded branches — resolves imports between modules through a dependency graph, and executes guests under an explicit capability policy instead of an implicitly permissive host.

---

## Quickstart (under 2 minutes)

```bash
git clone https://github.com/AndyTechnologies/WasmFoundry
cd WasmFoundry
cargo install --path crates/wf-cli
wf init hello && cd hello
wf build && wf run
```

You should see:
```
wf: built hello (core module) via precompiled -> ./target/hello.wasm
     validated src/hello.wasm
wf: built 1 module(s) into target/

wf: ran hello via _start in 1.2ms
wf: entry point returned 42
```

---

## Why WasmFoundry?

| Problem | Status quo | WasmFoundry |
| --- | --- | --- |
| **Adding a guest language** | Touch the build orchestrator | Drop in an adapter implementing one trait |
| **Multi-module linking** | Generate C++, compile it | Resolve imports by name through a linker |
| **Host surface** | Whatever bindings exported | Versioned Host ABI, declared capabilities |
| **"Sandboxed"** | Only runtime | Build runs your tools; run is bounded by manifest |
| **Distribution** | Ship WASM + runtime separately | `wf bundle` → single native executable |

**Killer use case**: Package a multi-language plugin system (Rust + C++ + AssemblyScript guests) with explicit capabilities (filesystem, network, wall-clock, memory) and distribute it as one native binary — no runtime install required on the target machine.

---

## Examples (copy, build, run)

| Example | What it demonstrates |
| --- | --- |
| [`examples/hello-precompiled/`](examples/hello-precompiled/) | Simplest path: existing `.wasm` → `wf build` → `wf run` |
| [`examples/hello-rust/`](examples/hello-rust/) | Rust source → `wasm32-unknown-unknown` → run |
| [`examples/two-modules/`](examples/two-modules/) | Module A imports Module B; dependency graph resolves it |
| [`examples/wasi-file/`](examples/wasi-file/) | WASI + explicit read-only mount in manifest |
| [`examples/bundle/`](examples/bundle/) | `wf bundle` → standalone native executable |

Each example has its own `README.md` with exact commands and expected output.

---

## Documentation

- **Getting Started** — [`docs/getting-started.md`](docs/getting-started.md) (detailed walkthrough)
- **Architecture** — [`docs/architecture.md`](docs/architecture.md) (crate boundaries, dependency rules)
- **Product Vision** — [`docs/product.md`](docs/product.md) (problems, non-goals, security boundary)
- **Toolchain Protocol** — [`docs/toolchain-protocol.md`](docs/toolchain-protocol.md) (adapter interface)
- **Bundle Format v1** — [`docs/bundle-format-v1.md`](docs/bundle-format-v1.md) (ABNF spec)
- **Parity Matrix** — [`docs/parity-matrix.md`](docs/parity-matrix.md) (legacy capability mapping)
- **Agent Rules** — [`AGENTS.md`](AGENTS.md) (operating instructions for contributors)

---

## Status

`v0.3.1` — All core features implemented through PHASE 21. Stability specs S1–S10 tracking continuous improvement.

| Phase | Delivers | Status |
|-------|----------|--------|
| 3 | `wf inspect` | ✅ |
| 4 | `wf run` | ✅ |
| 5 | `wf init`, `wf build` (precompiled) | ✅ |
| 6 | Rust guest toolchain | ✅ |
| 7 | Dependency graph, module matching | ✅ |
| 8–9 | Host ABI v1, WASI, mounts, capability policy, execution limits | ✅ |
| 10–11 | C++ & AssemblyScript toolchains | ✅ |
| 12–13 | Build cache (BLAKE3), `wf build --watch` | ✅ |
| 14–16 | Bundle format v1, prebuilt launchers, cross-target | ✅ |
| 17–18 | Component Model, WIT, External plugins | ✅ |
| 19–21 | Supply chain, CI, release pipeline | ✅ |

Functional parity with the previous generation tracked in [`docs/parity-matrix.md`](docs/parity-matrix.md).

---

## License

MIT. See [LICENSE](LICENSE).