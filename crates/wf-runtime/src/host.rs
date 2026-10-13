//! What the host offers to a guest.
//!
//! This is the mechanism, not the contract: a way for a command to say "when the guest
//! imports `namespace.name`, call this" without any Wasmtime type reaching the caller.
//! What each function actually does — the names, the arguments, what a string is — is the
//! Host ABI's business, documented separately in `docs/abi/host-abi-v1.md`.

use std::fmt;
use std::sync::Arc;

use wasmtime::{Caller, FuncType, Linker, Val, ValType};

use crate::policy::HostState;

/// What a host function's body is: the guest it is running against, the arguments the
/// guest passed, and the value it may return.
///
/// Arguments arrive as the `i32`s every function in the v1 console ABI takes. A
/// function needing another type will widen this; until then, a generic shape nobody can
/// fill in would be worse than an honest one.
type HostBody = dyn Fn(&mut Guest<'_>, &[i32]) -> Result<Option<i32>, HostError> + Send + Sync;

/// A host function the guest may import.
#[derive(Clone)]
pub struct HostFunction {
    namespace: String,
    name: String,
    /// How many `i32` arguments the guest must pass.
    arity: usize,
    body: Arc<HostBody>,
}

impl fmt::Debug for HostFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Hand-written because the body is a closure behind a trait object, and forcing
        // every host function to be `Debug` would be a burden for no reader.
        f.debug_struct("HostFunction")
            .field("namespace", &self.namespace)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl HostFunction {
    /// Registers `namespace.name` with a body written in Rust.
    ///
    /// `arity` is how many `i32` arguments the guest passes, and it is what the host
    /// declares: a guest calling with the wrong number of arguments is refused by the
    /// type check rather than by the body.
    pub fn new(
        namespace: impl Into<String>,
        name: impl Into<String>,
        arity: usize,
        body: impl Fn(&mut Guest<'_>, &[i32]) -> Result<Option<i32>, HostError> + Send + Sync + 'static,
    ) -> Self {
        HostFunction {
            namespace: namespace.into(),
            name: name.into(),
            arity,
            body: Arc::new(body),
        }
    }

    /// How many `i32` arguments this function takes.
    pub fn arity(&self) -> usize {
        self.arity
    }

    /// The namespace this function is published under.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The name within that namespace.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The guest a host function is running against.
///
/// A narrow window onto the guest's memory, deliberately: a host function can read the
/// range the guest pointed at, and nothing else. The bytes stay the guest's — the host
/// copies what it is given rather than holding on to them.
///
/// No engine type appears in this signature, so a host function cannot be written in a
/// way that depends on which engine is running it.
pub struct Guest<'a> {
    read: Box<dyn FnMut(i32, i32) -> Result<Vec<u8>, HostError> + 'a>,
}

impl Guest<'_> {
    /// Reads a range of the guest's linear memory.
    ///
    /// Returns raw bytes: whether they are text, and what encoding they use, is the Host
    /// ABI's decision and not this mechanism's.
    pub fn read_memory(&mut self, pointer: i32, length: i32) -> Result<Vec<u8>, HostError> {
        (self.read)(pointer, length)
    }
}

/// Reads a range out of a caller's exported memory.
fn read_range(
    caller: &mut Caller<'_, HostState>,
    pointer: i32,
    length: i32,
) -> Result<Vec<u8>, HostError> {
    if pointer < 0 || length < 0 {
        return Err(HostError::InvalidPointer { pointer, length });
    }

    let memory = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or(HostError::NoMemory)?;

    let start = pointer as usize;
    let end = (pointer as usize)
        .checked_add(length as usize)
        .ok_or(HostError::InvalidPointer { pointer, length })?;

    memory
        .data(&*caller)
        .get(start..end)
        .map(|slice| slice.to_vec())
        .ok_or(HostError::OutBounds { pointer, length })
}

/// Why a host function could not answer a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// The guest has no exported linear memory, so nothing could be read.
    NoMemory,
    /// A pointer or length the guest passed does not address anything readable.
    InvalidPointer {
        /// The pointer the guest passed.
        pointer: i32,
        /// The length the guest passed.
        length: i32,
    },
    /// The range ran past the end of memory.
    OutBounds {
        /// The pointer the guest passed.
        pointer: i32,
        /// The length the guest passed.
        length: i32,
    },
    /// The host function itself refused the call.
    Refused {
        /// What it objected to.
        reason: String,
    },
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostError::NoMemory => write!(f, "the guest exports no linear memory"),
            HostError::InvalidPointer { pointer, length } => write!(
                f,
                "the guest passed pointer {pointer} with length {length}, which is not a readable range"
            ),
            HostError::OutBounds { pointer, length } => write!(
                f,
                "the range at pointer {pointer} of length {length} runs past the end of memory"
            ),
            HostError::Refused { reason } => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for HostError {}

/// Defines every host function in a linker.
///
/// Called once per invocation, next to where the guest modules are instantiated, so the
/// host surface and the guest set are decided in the same run.
pub(crate) fn install(
    engine: &wasmtime::Engine,
    linker: &mut Linker<HostState>,
    functions: &[HostFunction],
) -> Result<(), crate::RuntimeError> {
    // Every host function in the v1 ABI takes the same two arguments — a pointer and a
    // length — and returns nothing. Stating that here is honest about the shape the
    // mechanism supports: a function needing something else will widen this, and until
    // then pretending to be generic would be a type nobody can fill in.
    let i32_type = ValType::I32;

    for function in functions {
        let body = Arc::clone(&function.body);
        let namespace = function.namespace().to_owned();
        let name = function.name().to_owned();
        // Two labels: one moves into the closure, one survives it for the failure
        // message below.
        let label = format!("{namespace}::{name}");
        let failure_label = format!("{namespace}::{name}");
        let ty = FuncType::new(
            engine,
            std::iter::repeat_n(i32_type.clone(), function.arity),
            [],
        );

        linker
            .func_new(&namespace, &name, ty, move |mut caller, params, _| {
                // Every argument is an i32 in this ABI, so anything else is a guest that
                // declared the wrong signature, and saying so is more useful than a
                // trap from deeper in.
                let mut arguments: Vec<i32> = Vec::with_capacity(params.len());
                for parameter in params {
                    match parameter {
                        Val::I32(value) => arguments.push(*value),
                        other => {
                            return Err(wasmtime::Error::msg(format!(
                                "the host expected i32 arguments for `{label}`, got {other:?}"
                            )));
                        }
                    }
                }

                let outcome = {
                    let mut guest = Guest {
                        read: Box::new(|pointer, length| read_range(&mut caller, pointer, length)),
                    };
                    body(&mut guest, &arguments)
                };

                if let Err(error) = outcome {
                    // A host function can only report a failure by failing the call, and
                    // Wasmtime surfaces that as a trap. Recording the reason is what lets
                    // the caller tell "the host refused" from "the guest faulted", which
                    // need different fixes.
                    caller.data_mut().host_error = Some(error.to_string());
                    return Err(wasmtime::Error::msg(error.to_string()));
                }

                // The mechanism supports one i32 result; today's ABI functions return
                // nothing, so `results` stays empty.
                Ok(())
            })
            .map_err(|error| crate::RuntimeError::Host {
                message: format!("cannot register host function `{failure_label}`: {error}"),
            })?;
    }
    Ok(())
}
