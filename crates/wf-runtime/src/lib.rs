//! Wasmtime embedding for WasmFoundry.
//!
//! # Responsibility
//!
//! This crate turns an analysed WebAssembly binary into a moving program. It
//! owns the Engine, compiles modules, builds the store and the linker, resolves
//! the entry point and reports what the guest produced. It is the only crate
//! allowed to depend on `wasmtime`.
//!
//! # Ownership model
//!
//! The embedding model is fixed by Wasmtime and must be respected literally:
//!
//! ```text
//! Engine    compilation and resource-usage configuration, shared and long lived
//!   |
//! Module    a compiled binary; cheap to instantiate many times
//!   |
//! Store     mutable state for one logical execution (host data, memory, resources)
//!   |
//! Linker    name resolution for the imports a module declares
//!   |
//! Instance  a running shape of a module inside one store
//!   |
//! entry     an exported function called with concrete arguments
//! ```
//!
//! The lifecycle rule that follows from it: **no `Store` is global**. A
//! `Store` carries execution state, so a process-wide store would be global
//! mutable state shared between runs, which leaks one guest's memory into the
//! next invocation and makes concurrent runs impossible. An `Engine` may be
//! shared because it holds configuration, not execution state; everything from
//! `Store` downwards is created per invocation.
//!
//! # Current status
//!
//! PHASE 2 declares the dependency and the lifecycle contract only. Execution
//! arrives with `wf run` (PHASE 4), which assumes core wasm without WASI; WASI,
//! mounts and the capability policy arrive in PHASE 9. No execution API is
//! declared here yet, because a signature written before its implementation
//! would be a guess about error handling, host data and policy plumbing.

#![forbid(unsafe_code)]
