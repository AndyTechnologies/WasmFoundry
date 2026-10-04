//! Wasmtime embedding for WasmFoundry.
//!
//! # Responsibility
//!
//! This crate turns an analysed WebAssembly binary into a moving program. It owns the
//! Engine, compiles modules, builds the store and the linker, resolves the entry point
//! and reports what the guest produced. It is the only crate allowed to depend on
//! `wasmtime`.
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
//! The lifecycle rule that follows from it: **no `Store` is global**. A `Store` carries
//! execution state, so a process-wide store would be global mutable state shared between
//! runs, which leaks one guest's memory into the next invocation and makes concurrent
//! runs impossible. [`Runtime`] owns the `Engine`, because an engine holds configuration
//! rather than state and may be shared; everything from `Store` downwards is created
//! inside [`Runtime::run`] and dropped when it returns.
//!
//! # Boundary rule
//!
//! No `wasmtime` type appears in this crate's public API. Callers depend on
//! [`Runtime`] and [`RuntimeError`], not on the embedding underneath, so the engine
//! binding can change without touching `wf-cli`.
//!
//! # Current status
//!
//! Core modules only, no WASI and no host ABI: a module with imports fails as a
//! [`RuntimeError::Link`], because nothing satisfies imports until PHASE 8 defines the
//! Host ABI. There are **no execution limits yet** — the wall-clock timeout and memory
//! cap of PHASE 9 do not exist, and a guest is free to run forever or grow its memory
//! without bound. That is stated rather than implied; see `docs/product.md`.

#![forbid(unsafe_code)]

use std::error::Error as StdError;
use std::fmt;
use std::time::{Duration, Instant};

use wasmtime::{Error as WasmError, Extern, Linker, Module as WasmtimeModule, Store, ValType};

use wf_core::EntryPoint;

/// Executes WebAssembly binaries.
///
/// One instance per process invocation is enough: it owns the engine configuration and
/// the compiled modules hang off it as [`Module`] values.
#[derive(Debug, Default, Clone)]
pub struct Runtime {
    engine: wasmtime::Engine,
}

/// A compiled WebAssembly binary.
///
/// Opaque on purpose: the bytes were validated and compiled once, and a caller that
/// wants them again asks the runtime rather than carrying Wasmtime types of its own.
#[derive(Debug, Clone)]
pub struct Module {
    inner: WasmtimeModule,
}

/// What one invocation produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    entry: EntryPoint,
    return_value: Option<i32>,
    duration: Duration,
}

impl RunOutcome {
    /// The entry point that was invoked.
    pub fn entry(&self) -> &EntryPoint {
        &self.entry
    }

    /// The value the guest returned, when its entry point returns an `i32`.
    ///
    /// `None` for the conventional `() -> ()` entry, which reports nothing.
    pub fn return_value(&self) -> Option<i32> {
        self.return_value
    }

    /// Wall-clock time the invocation took, measured around the call only.
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

/// Why an invocation could not be completed.
///
/// One variant per cause, because the causes need different fixes and a caller that
/// only sees a string cannot tell them apart:
///
/// | Variant | What is wrong |
/// | --- | --- |
/// | [`InvalidWasm`](Self::InvalidWasm) | The bytes are not a compilable module |
/// | [`Link`](Self::Link) | An import has no provider |
/// | [`MissingExport`](Self::MissingExport) | The entry point does not exist |
/// | [`Trap`](Self::Trap) | The guest faulted while running |
/// | [`Configuration`](Self::Configuration) | The entry point exists but cannot be invoked as written |
///
/// Two causes listed in the plan's error taxonomy are deliberately absent, because
/// nothing can produce them yet and an unproduced variant would be dead code: a host
/// error needs a host function (PHASE 8) and an execution-limit error needs limits
/// (PHASE 9). Both arrive with their own cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// Wasmtime refused to compile the input.
    InvalidWasm {
        /// What the compiler rejected, in its own words.
        message: String,
    },
    /// An import has no definition in the linker.
    Link {
        /// The unsatisfied import, as the linker described it.
        message: String,
    },
    /// The guest exports no function under the requested name.
    MissingExport {
        /// The name that was looked up.
        name: String,
    },
    /// The guest faulted while executing.
    Trap {
        /// The fault Wasmtime reported, including the source location when it has one.
        message: String,
    },
    /// The entry point exists but is not something this runtime can invoke.
    Configuration {
        /// What the export actually is, and what the runtime supports.
        message: String,
    },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::InvalidWasm { message } => {
                write!(f, "invalid WebAssembly module: {message}")
            }
            RuntimeError::Link { message } => write!(f, "unresolved import: {message}"),
            RuntimeError::MissingExport { name } => {
                write!(f, "entry point `{name}` is not exported by this module")
            }
            RuntimeError::Trap { message } => write!(f, "guest trapped: {message}"),
            RuntimeError::Configuration { message } => {
                write!(f, "cannot invoke entry point: {message}")
            }
        }
    }
}

impl StdError for RuntimeError {}

impl Runtime {
    /// Compiles bytes into a reusable [`Module`].
    ///
    /// Validation happens here and only here: everything after a successful compile
    /// treats the module as well formed.
    pub fn compile(&self, bytes: &[u8]) -> Result<Module, RuntimeError> {
        WasmtimeModule::new(&self.engine, bytes)
            .map(|inner| Module { inner })
            .map_err(|error| RuntimeError::InvalidWasm {
                message: describe(&error),
            })
    }

    /// Invokes `entry` on a compiled module.
    ///
    /// The store and the linker are created here and dropped on return, so two runs —
    /// including two running in parallel threads — share no state at all.
    pub fn run(&self, module: &Module, entry: &EntryPoint) -> Result<RunOutcome, RuntimeError> {
        let started = Instant::now();

        let mut store = Store::new(&self.engine, ());
        let linker = Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &module.inner)
            .map_err(|error| RuntimeError::Link {
                message: describe(&error),
            })?;

        let export = instance
            .get_export(&mut store, entry.as_str())
            .ok_or_else(|| RuntimeError::MissingExport {
                name: entry.as_str().to_owned(),
            })?;

        let function = match export {
            Extern::Func(function) => function,
            other => {
                return Err(RuntimeError::Configuration {
                    message: format!(
                        "`{entry}` is not a function; the module exports a {}",
                        extern_kind(&other)
                    ),
                });
            }
        };

        let signature = function.ty(&store);
        let params: Vec<ValType> = signature.params().collect();
        let results: Vec<ValType> = signature.results().collect();

        let return_value =
            if params.is_empty() && results.is_empty() {
                let typed = function.typed::<(), ()>(&store).map_err(|error| {
                    RuntimeError::Configuration {
                        message: error.to_string(),
                    }
                })?;
                typed
                    .call(&mut store, ())
                    .map_err(|error| RuntimeError::Trap {
                        message: describe(&error),
                    })?;
                None
            } else if params.is_empty() && matches!(results.as_slice(), [ValType::I32]) {
                let typed = function.typed::<(), i32>(&store).map_err(|error| {
                    RuntimeError::Configuration {
                        message: error.to_string(),
                    }
                })?;
                Some(
                    typed
                        .call(&mut store, ())
                        .map_err(|error| RuntimeError::Trap {
                            message: describe(&error),
                        })?,
                )
            } else {
                // Guessing at a signature would be worse than refusing: passing the wrong
                // values to a guest is a way to corrupt state rather than to report an
                // incompatibility.
                return Err(RuntimeError::Configuration {
                    message: format!(
                        "`{entry}` has signature `({}) -> ({})`; WasmFoundry invokes \
                     `() -> ()` or `() -> i32`",
                        join_types(&params),
                        join_types(&results)
                    ),
                });
            };

        Ok(RunOutcome {
            entry: entry.clone(),
            return_value,
            duration: started.elapsed(),
        })
    }
}

/// Flattens a Wasmtime error into one message that names the actual cause.
///
/// `Error::to_string()` reports only the outermost context — for a trap that is
/// `error while executing at wasm backtrace:` and nothing else, and for a bad module it
/// is `failed to parse WebAssembly module`. The reason lives one level down. A report
/// that drops it is the `execution failed` the plan forbids with different words, so the
/// chain is walked and the reason comes first.
fn describe(error: &WasmError) -> String {
    let context = collapse(&error.to_string());
    let cause = error
        .chain()
        .last()
        .map(|cause| collapse(&cause.to_string()))
        .unwrap_or_default();

    if cause.is_empty() || cause == context {
        return context;
    }
    if context.is_empty() {
        return cause;
    }
    format!("{context}: {cause}")
}

/// Collapses runs of whitespace so a multi-line Wasmtime message stays one line.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Names an exported entity for an error message.
fn extern_kind(export: &Extern) -> &'static str {
    match export {
        Extern::Func(_) => "function",
        Extern::Global(_) => "global",
        Extern::Memory(_) | Extern::SharedMemory(_) => "memory",
        Extern::Table(_) => "table",
        Extern::Tag(_) => "tag",
    }
}

/// Joins value types for display, for example `i32, i32`.
fn join_types(types: &[ValType]) -> String {
    types
        .iter()
        .map(ValType::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
