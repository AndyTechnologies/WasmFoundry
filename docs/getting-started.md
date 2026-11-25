# Getting Started with WasmFoundry

This guide walks you through installing WasmFoundry, creating your first project, and
understanding the core concepts. By the end, you'll have a working WebAssembly project
that you can build, run, and bundle into a standalone native executable.

## Prerequisites

- **Rust toolchain** (installed via [rustup](https://rustup.rs/))
- **wasm32-unknown-unknown target** for Rust guests:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

## Installation

### From Source (Recommended)

```bash
git clone https://github.com/AndyTechnologies/WasmFoundry
cd WasmFoundry
cargo install --path crates/wf-cli
```

This installs the `wf` binary to `~/.cargo/bin/` (ensure it's in your PATH).

### Verify Installation

```bash
wf --version
# v0.3.1
```

---

## Your First Project (2 minutes)

### 1. Initialize a Project

```bash
wf init hello
cd hello
```

This creates:
```
hello/
├── wasmfoundry.toml    # Project manifest
└── src/
    └── hello.wasm      # Precompiled WebAssembly module (returns 42)
```

### 2. Build the Project

```bash
wf build
```

Output:
```
wf: built hello (core module) via precompiled -> ./target/hello.wasm
     validated src/hello.wasm
wf: built 1 module(s) into target/
```

The module is published to `target/hello.wasm`. No compilation happened — the module was
precompiled and only validated.

### 3. Run the Project

```bash
wf run
```

Output:
```
wf: ran hello via _start in 1.2ms
wf: entry point returned 42
```

The guest's `_start` function returned `42`, which `wf run` reports.

---

## Core Concepts

### The Manifest: `wasmfoundry.toml`

Every project has a manifest that declares:
- **Package metadata** (name, version)
- **Project settings** (source directory, entry point)
- **Modules** (name, source, toolchain)
- **Runtime policy** (capabilities, limits, mounts)

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

### Toolchains

A **toolchain** is an adapter that produces a `.wasm` from source. WasmFoundry ships with:

| Toolchain | Source | Use Case |
|-----------|--------|----------|
| `precompiled` | Existing `.wasm` file | Fastest path, no guest compiler needed |
| `rust` | Rust crate (`Cargo.toml` with `cdylib`) | Rust guests |
| `cpp` | C++ source with `CMakeLists.txt` | C++ guests |
| `assemblyscript` | AssemblyScript project | TypeScript-like guests |
| `component` | WIT + component sources | WebAssembly Components |

### Modules and Imports

A project can have multiple modules. Imports between modules are resolved by **namespace**:

```toml
[[module]]
name = "app"
source = "src/app.wasm"
toolchain = "precompiled"

[[module]]
name = "engine"
source = "src/engine.wasm"
toolchain = "precompiled"
```

If `app` imports `hello` from namespace `engine`, the linker resolves it automatically.
The build computes the dependency graph and builds in the correct order — no generated C++.

### Capability Policy (Deny by Default)

Guests get **nothing** unless the manifest explicitly grants it:

```toml
[runtime]
wasi = true                    # Grant WASI (stdout, filesystem, etc.)
wall_clock_seconds = 5.0       # Stop guest after 5 seconds
memory_pages = 4096            # Limit to 256 MiB (1 page = 64 KiB)

[[mount]]
host = "./data"                # Host directory
guest = "/data"                # Guest path (must be absolute)
access = "read-only"           # Default; can be "read-write"
```

Without `wasi = true`, any WASI import fails at build time with `WF002`. With it, the
guest gets exactly the mounts you declared — nothing more.

---

## Working with Rust Guests

### 1. Create a Rust Guest Project

```bash
wf init hello-rust
cd hello-rust
```

### 2. Replace the Manifest

Edit `wasmfoundry.toml`:

```toml
schema = 1

[package]
name = "hello-rust"
version = "0.1.0"

[project]
source_dir = "guest"
entry = "_start"

[[module]]
name = "hello"
source = "guest"
toolchain = "rust"
```

### 3. Create the Guest Crate

```bash
mkdir -p guest/src
```

`guest/Cargo.toml`:
```toml
[package]
name = "hello-guest"
version = "0.1.0"
edition = "2021"
publish = false

[lib]
crate-type = ["cdylib"]

[workspace]
```

`guest/src/lib.rs`:
```rust
#[no_mangle]
pub extern "C" fn _start() -> i32 {
    7
}
```

**Key points:**
- `crate-type = ["cdylib"]` makes cargo emit a `.wasm`
- `[workspace]` opts out of the parent workspace (avoids "believes it's in a workspace" error)
- `_start` is the entry point (configurable in manifest)

### 4. Build and Run

```bash
wf build
wf run
```

Output:
```
wf: built hello (core module) via rust -> ./target/hello.wasm
     (in ./guest) cargo build --release --target wasm32-unknown-unknown
wf: built 1 module(s) into target/

wf: ran guest via _start in 1.4ms
wf: entry point returned 7
```

---

## Multi-Module Projects

Create a project where one module depends on another:

```bash
wf init two-modules
cd two-modules
```

Replace `wasmfoundry.toml`:

```toml
schema = 1

[package]
name = "two-modules"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "app"
source = "src/app.wasm"
toolchain = "precompiled"

[[module]]
name = "engine"
source = "src/engine.wasm"
toolchain = "precompiled"
```

Create two `.wasm` files (using `wat2wasm` or the `wat` crate):

**src/engine.wasm** — exports `hello` returning `21`:
```wat
(module (func (export "hello") (result i32) (i32.const 21)))
```

**src/app.wasm** — imports `hello` from `engine`, returns `42`:
```wat
(module
  (import "engine" "hello" (func $hello (result i32)))
  (func (export "_start") (result i32)
    call $hello
    i32.const 2
    i32.mul))
```

Build and run:
```bash
wf build
wf run
```

Output:
```
wf: built app (core module) via precompiled -> ./target/app.wasm
     validated src/app.wasm
wf: built engine (core module) via precompiled -> ./target/engine.wasm
     validated src/engine.wasm
wf: built 2 module(s) into target/

wf: ran app via _start in 52.8ms
wf: entry point returned 42
```

The build ordered them correctly (`engine` first) and the linker resolved the import.

---

## WASI and Mounts

Create a project that reads a file through WASI:

```bash
wf init wasi-file
cd wasi-file
```

Manifest with WASI and a mount:

```toml
schema = 1

[package]
name = "wasi-file"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "main"
source = "src/main.wasm"
toolchain = "precompiled"

[runtime]
wasi = true

[[mount]]
host = "./data"
guest = "/data"
access = "read-only"
```

Create `data/hello.txt`:
```
Hello from the mounted file!
```

The guest uses WASI `fd_write` to print "hello wasi" to stdout (see the
[wasi-file example](examples/wasi-file/) for the full module).

```bash
wf build
wf run
```

Output:
```
hello wasi
wf: ran main via _start in 26.9ms
wf: entry point returned 0
```

---

## Bundling: Standalone Native Executables

`wf bundle` wraps your built project into a single native binary that includes the
Wasmtime runtime. It runs without WasmFoundry installed.

### Prerequisites

Build the dev launcher once:

```bash
cd /path/to/WasmFoundry
cargo build -p wf-bundle --bin wf-launcher
```

### Create a Bundle

```bash
cd examples/bundle
wf build
wf bundle --launcher ../../target/debug/wf-launcher
./target/bundle-example.bundle
```

Output:
```
wf: bundled bundle-example -> target/bundle-example.bundle (launcher 342013 KiB, module `hello`)

wf-launcher: ran payload, returned 42
```

The bundle is a self-contained executable. Copy it to another machine (same architecture)
and it runs — no Rust, no WasmFoundry, no Wasmtime install needed.

### Cross-Target Bundling

Use `--bundle-target` with a launcher registry to bundle for a different target:

```bash
export WF_LAUNCHER_REGISTRY=/path/to/registry
wf bundle --bundle-target x86_64-unknown-linux-gnu
```

The registry should have launchers organized by triple:
```
registry/
└── x86_64-unknown-linux-gnu/
    └── wf-launcher
```

---

## Common Commands Reference

| Command | Purpose |
|---------|---------|
| `wf init <name>` | Scaffold a new project |
| `wf build` | Build all modules in the project |
| `wf build --watch` | Rebuild on source changes |
| `wf run` | Run the project's entry module |
| `wf run <path.wasm>` | Run any module directly |
| `wf inspect <path.wasm>` | Analyze a module (imports, exports, memory, etc.) |
| `wf inspect --format json` | Machine-readable output |
| `wf bundle --launcher <path>` | Create standalone native executable |
| `wf bundle --bundle-target <triple>` | Bundle using registry launcher |
| `wf component build <path>` | Build a WebAssembly Component |
| `wf plugin init <name>` | Scaffold a plugin project |
| `wf plugin build` | Build a plugin |
| `wf wit check <path.wit>` | Validate a WIT file |

---

## Troubleshooting

### `wf build` fails with "current package believes it's in a workspace"

Your guest crate is inside a Cargo workspace. Add `[workspace]` to the guest's
`Cargo.toml`:

```toml
[workspace]
```

### `wf build` fails with WF004 "toolchain not available"

The toolchain name in your manifest doesn't match a known toolchain. Run `wf build`
without a project to see the list of available toolchains in the error message.

### `wf run` fails with WF002 "import has no provider"

The guest imports something (e.g., WASI) that isn't granted. Add it to the manifest:

```toml
[runtime]
wasi = true
```

### `wf run` fails with WF014 "wall-clock budget exceeded"

The guest ran longer than `wall_clock_seconds`. Increase the limit or optimize the guest.

### `wf bundle` fails with "target/hello.wasm not found"

Run `wf build` first — the module must exist before bundling.

---

## Next Steps

- Browse the [examples/](examples/) — each has a README with exact commands
- Read [Architecture](architecture.md) for crate boundaries and dependency rules
- Read [Product Vision](product.md) for problems, non-goals, and security boundary
- Check [Toolchain Protocol](toolchain-protocol.md) if you want to write a custom toolchain
- See [Bundle Format v1](bundle-format-v1.md) for the bundle specification

---

## Getting Help

- **Bug reports**: [GitHub Issues](https://github.com/AndyTechnologies/WasmFoundry/issues/new?template=bug_report.yml)
- **Feature requests**: [GitHub Issues](https://github.com/AndyTechnologies/WasmFoundry/issues/new?template=feature_request.yml)
- **Questions**: [GitHub Discussions](https://github.com/AndyTechnologies/WasmFoundry/discussions)