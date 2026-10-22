//! Guest toolchains: adapters that turn a declared source into a published,
//! validated WebAssembly module without leaking a compiler's flags into the build.
//!
//! The dependency direction is fixed: this crate sits above `wf-core` and `wf-wasm`
//! and below `wf-cli`, so the CLI never constructs a compiler command.

pub mod cache;
pub mod process;
pub mod toolchains;
