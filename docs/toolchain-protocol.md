# WasmFoundry Toolchain Protocol (v1)

## Overview

This document defines the public contract between `wf` CLI, `wf-core`, `wf-wasm`, and `wf-runtime` crates. It specifies the interface that adapter implementations must follow to integrate with WasmFoundry.

The protocol is designed to be stable across PHASE 21+ and supports both precompiled and custom toolchains.

## Schema Versions

- **Schema 1**: Initial version. Supports precompiled toolchains and basic manifest parsing.
- **Schema 2**: Added support for custom toolchains, WIT integration, and component-model features.

## Adapter Interface

Every toolchain adapter must implement the following traits from `wf-toolchains`:

### `ToolchainId`

```rust
pub trait ToolchainId: Debug + Clone + PartialEq + Eq {
    /// Create a new ToolchainId from a toolchain name string.
    fn new(name: &str) -> Result<Self, Diagnostic>;

    /// Return the toolchain name for display purposes.
    fn name(&self) -> String;

    /// Check if this is the "precompiled" toolchain.
    fn is_precompiled(&self) -> bool;
}
```

### `CompileRequest`

```rust
pub struct CompileRequest<'a> {
    /// The module spec from the manifest.
    pub module: &'a ModuleSpec,
    /// Root path of the project.
    pub root: &'a Path,
    /// Runner for executing toolchain commands.
    pub runner: &'a StdRunner,
}
```

### `compile` function

```rust
pub fn compile(request: &CompileRequest, runner: &StdRunner) -> Result<Vec<u8>, Diagnostic>;
```

Builds a single module and returns the raw Wasm bytes.

### `lookup` function

```rust
pub fn lookup(id: &ToolchainId) -> Option<Box<dyn Toolchain>>;
```

Returns a toolchain implementation for the given ID, or `None` if unknown.

## Module Specification

A `ModuleSpec` describes one module to build:

```rust
pub struct ModuleSpec {
    /// Name of the module (used for output filename).
    pub name: String,
    /// Toolchain to use (e.g., "rust", "cpp", "precompiled").
    pub toolchain: String,
    /// Source path relative to project root.
    pub source: PathBuf,
    /// Entry point function name.
    pub entry: String,
    /// Additional flags or configuration.
    pub flags: Vec<String>,
}
```

## Build Pipeline

The `wf build` command orchestrates the following pipeline for each module:

1. **Parse** the manifest and validate syntax.
2. **Resolve** the `ToolchainId` for the module's toolchain field.
3. **Lookup** the toolchain implementation via `wf-toolchains::lookup()`.
4. **Compile** the module using `toolchain.compile()`.
5. **Fingerprint** the resulting artifact using SHA-256.
6. **Cache** the result for incremental builds.
7. **Emit** build reports (JSON if `--format json` is specified).

## JSON Output Format

When `--format json` is specified, the CLI emits one of the following:

### Build Report (`wf build`)

```json
{
  "modules": [
    {
      "name": "hello",
      "toolchain": "rust",
      "artifact": "target/wasm32-unknown-unknown/release/hello.wasm",
      "kind": "executable",
      "fingerprint": "abc123...",
      "cached": false,
      "dependencies": [
        {"name": "std", "fingerprint": "def456..."}
      ],
      "detail": "compiled with rustc 1.70+"
    }
  ],
  "cached": 0,
  "total": 1
}
```

### Bundle Report (`wf bundle`)

```json
{
  "name": "my-project",
  "output": "dist/wf-launcher-x86_64-unknown-linux-gnu",
  "launcher_size_bytes": 12345,
  "module": "hello",
  "module_size_bytes": 4567
}
```

### Run Report (`wf run`)

```json
{
  "module": "hello",
  "entry": "_start",
  "duration_micros": 42,
  "return_value": 42
}
```

## Sandbox Configuration

Projects can opt-in to capability-based sandboxing via the manifest:

```toml
[runtime]
sandbox = { enabled = true, extra_syscalls = "network" }
```

When enabled, the runtime applies platform-specific restrictions:

- **Linux**: seccomp-bpf filter restricting syscalls to the minimal set plus any allowed extras.
- **Windows**: Job Objects with limited capabilities (no child processes, etc.).
- **Other platforms**: sandboxing not available (build succeeds but no isolation).

The minimal syscall set always includes: `read`, `write`, `openat`, `close`, `exit_group`, `rt_sigreturn`.

Additional syscalls can be allowed via the `extra_syscalls` field:
- `none`: No additional syscalls.
- `network`: Allow network syscalls (socket, connect, sendto, recvfrom).
- `process`: Allow process spawning (clone, fork, execve, wait4).
- `all`: Allow all additional syscalls (not recommended for production).

## Versioning & Compatibility

- The protocol version is baked into the Wasm binary header as `HOST_ABI_LABEL = "abi-v1"`.
- Adapters must target the Rust API of Wasmtime only — no `wasmtime-c-api`, no bindgen, no CMake.
- No speculative abstractions: `FileSystem`, `LogicalPath`, `BuildExecutor`, `ArtifactStore`, or `PluginManager` must not exist before a second consumer exists.
- Dead code elimination: types, functions, or modules with no caller do not land in the repository.

## Adapter Guidelines

New adapters must follow these rules:

1. **No hand-written WASM parser**: All binary decoding goes through `wasmparser`.
2. **No `wasmtime-c-api`, no bindgen, no CMake, no cmake-js**: Only the Rust API of Wasmtime is supported.
3. **No async**: The product is synchronous with `std` threads where concurrency is genuinely needed.
4. **No speculative abstractions**: No factory, builder, repository, adapter, service, provider, or registry for a single implementation.
5. **No dead code**: All types and functions must have callers.
6. **Conventional Commits**: All commit messages must follow the format `type(scope): mensaje`.

## Migration Guide

### From Precompiled to Custom Toolchain

1. Add a new `ToolchainId` implementation (e.g., `CppToolchainId`).
2. Implement the `Toolchain` trait with `compile()` and `lookup()`.
3. Register the toolchain in `wf-toolchains` crate's toolchain map.
4. Update `wf.toml` to use the new toolchain name in module specs.
5. Add any required system dependencies (e.g., `clang`, `lld` for C++).
6. Test with `wf build --target wasm32-unknown-unknown`.

## See Also

- `docs/bundle-format-v1.md` — Detailed bundle format specification with footer layout.
- `crates/wf-toolchains` — Toolchain implementation library.
- `crates/wf-core` — Core domain types and validation.
- `crates/wf-runtime` — Runtime execution with sandbox support.