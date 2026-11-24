# two-modules

A multi-module project where one module imports another. This demonstrates WasmFoundry's
dependency graph resolution — the build orders modules correctly and the linker resolves
imports by name, without generating C++.

## What it shows

- Module `app` imports function `hello` from namespace `engine`
- Module `engine` exports `hello` returning `21`
- `app` calls `hello`, multiplies the result by `2`, returns `42`
- `wf build` computes the dependency graph and builds in the correct order
- No generated C++, no hand-written bindings — imports are resolved by name

## Commands

```bash
cd examples/two-modules
wf build
wf run
```

## Expected output

```
wf: built app (core module) via precompiled -> ./target/app.wasm
     validated src/app.wasm
wf: built engine (core module) via precompiled -> ./target/engine.wasm
     validated src/engine.wasm
wf: built 2 module(s) into target/

wf: ran app via _start in 52.8ms
wf: entry point returned 42
```

## Files

- `wasmfoundry.toml` — project manifest declaring two precompiled modules
- `src/app.wasm` — consumer module importing `hello` from namespace `engine`
- `src/engine.wasm` — provider module exporting `hello` returning `21`

The namespace for imports/exports is derived from the module name by default
(`module_matching = "name-only"`), so `engine` provides namespace `engine`.