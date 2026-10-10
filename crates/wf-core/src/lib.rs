#![forbid(unsafe_code)]
//! Pure domain vocabulary for WasmFoundry.
//!
//! # Responsibility
//!
//! This crate owns the *names and the rules* of the domain: what an artifact
//! is, which modules exist, what a build profile means, how a dependency graph
//! is traversed. It is the only crate that every other crate is allowed to
//! depend on, and it depends on nothing.
//!
//! # Hard dependency ban
//!
//! `wf-core` declares zero dependencies. `std` is the only code it may reach:
//!
//! - no `wasmtime`, no `wasmparser`, no `clap`, no `serde`, no `tokio`;
//! - no file system access, no process spawning, no network;
//! - no `unsafe`.
//!
//! A pure domain function takes values and returns values. It does not know
//! where bytes live, how a guest is executed, or how a binary format is
//! printed. Those responsibilities belong to `wf-wasm`, `wf-runtime` and
//! `wf-cli` respectively.
//!
//! # Current status
//!
//! This crate is intentionally **empty of code**. PHASE 2 establishes the
//! workspace; the first consumer of the domain vocabulary arrives in PHASE 3
//! (`wf inspect`). Type definitions written before a consumer exists are
//! speculative: they freeze names and invariants that real call sites would
//! have chosen differently. The vocabulary is therefore documented below as
//! text only, and is introduced type by type, with its first real user.
//!
//! # Roadmap
//!
//! Planned vocabulary, in the order it is expected to appear. The phase
//! column names the first feature that needs the type; a type lands with that
//! feature, not before.
//!
//! - `Diagnostic` — a machine-readable problem (code, severity, message,
//!   help) that every layer can produce and every layer can format.
//!   **Introduced in PHASE 4** ([`Diagnostic`]), so that the codes printed by
//!   the CLI stop being private constants of one command. Its shape stays at
//!   code, severity, message and help; `source`, `location` and `related` are
//!   added when a layer actually reports positions.
//! - `EntryPoint` — the guest-export contract a runtime invocation starts at,
//!   separating exported symbol names from a raw string.
//!   **Introduced in PHASE 4** ([`EntryPoint`]), the first type of this roadmap
//!   to exist.
//!
//! Four identities — [`EntryPoint`], [`ArtifactId`], [`ToolchainId`] and
//! [`GuestTarget`] — share [`NameError`] for their single failure mode, so the
//! message a user reads cannot drift between them.
//! - `Artifact` — the logical result of a build step, independent of where its
//!   bytes are stored. PHASE 5.
//! - `ArtifactId` — stable identity of an `Artifact` inside a graph, so
//!   artifacts can be referenced without embedding paths. PHASE 5.
//! - `ArtifactKind` — what an `Artifact` *is* (core module, component, source
//!   tree, launcher), which decides who is allowed to consume it. PHASE 5.
//! - `BuildProfile` — the knobs a build varies on (optimisation, debug info,
//!   source maps) without naming a concrete toolchain flag. PHASE 5.
//! - `ToolchainId` — identity of a guest toolchain, so per-toolchain
//!   configuration never becomes toolchain-specific conditionals in the
//!   domain. **Introduced in PHASE 6** ([`ToolchainId`]).
//! - `GuestTarget` — a guest target triple, kept out of the same type as a
//!   host triple so the two can never be one field. **Introduced in PHASE 6**
//!   ([`GuestTarget`]); a host target arrives with bundling in PHASE 14.
//! - `ModuleId` — identity of a wasm module inside a project, needed before
//!   module-name matching can be resolved.
//!   **Introduced in PHASE 7** ([`ModuleId`]).
//! - `Dependency` — a directed edge from one module to another, carrying the
//!   reason the edge exists.
//!   **Introduced in PHASE 7** ([`Dependency`]).
//! - `DependencyGraph` — the pure graph structure plus traversal rules:
//!   roots, leaves, topological order and cycle detection.
//!   **Introduced in PHASE 7** ([`DependencyGraph`]).
//! - `RuntimePolicy` — the set of capabilities a guest invocation is allowed
//!   (WASI, mounts, host ABI), validated independently of any engine. PHASE 9.
//! - `Fingerprint` — the content-derived identity of a build input, used by
//!   cache lookups once caching exists. PHASE 12.
//!
//! The roadmap is a plan, not a promise of shape. A type may arrive with
//! fewer fields than sketched here, or not at all, if the feature that needs
//! it turns out not to require it.

mod artifact;
mod diagnostic;
mod entry_point;
mod graph;
mod manifest;
mod matching;
mod name_error;
mod toolchain;

pub use artifact::{Artifact, ArtifactId, ArtifactKind};
pub use diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use entry_point::EntryPoint;
pub use graph::{Dependency, DependencyGraph, GraphError, ModuleId};
pub use manifest::{Manifest, ModuleSpec, Package, Project, SUPPORTED_SCHEMA};
pub use matching::{
    ModuleMatching, ParseModuleMatchingError, ResolveError, matching_modules, namespace_of,
    resolve_module,
};
pub use name_error::NameError;
pub use toolchain::{GuestTarget, ToolchainId};
