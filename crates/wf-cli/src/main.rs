//! The `wf` command line interface.
//!
//! # Responsibility
//!
//! This binary is the only entry point a user is expected to run. It owns
//! argument parsing, terminal output and process exit codes. It owns no domain
//! logic: parsing a module belongs to `wf-wasm`, executing a guest belongs to
//! `wf-runtime`, and the vocabulary they speak belongs to `wf-core`. A CLI that
//! grows business logic becomes the god object the architecture forbids.
//!
//! # Exit codes
//!
//! These numbers are a stable interface: a script may branch on them.
//!
//! | Code | Meaning |
//! | --- | --- |
//! | `0` | The command succeeded |
//! | `1` | The input was understood but rejected, reported as `WF001` |
//! | `2` | The command line or the input path was wrong |
//!
//! # Current status
//!
//! `wf inspect` is the only command (PHASE 3). It analyses a binary and never
//! executes one, so nothing here links the runtime. A command that prints
//! "not implemented" would be a lie shaped like a feature.

#![forbid(unsafe_code)]

mod cli;
mod inspect;

/// The command succeeded.
const EXIT_OK: i32 = 0;

/// The input was rejected with a diagnostic code.
const EXIT_DIAGNOSTIC: i32 = 1;

/// The command line or the input path was wrong.
const EXIT_USAGE: i32 = 2;

fn main() {
    std::process::exit(cli::run());
}
