#![forbid(unsafe_code)]
//! WebAssembly analysis for WasmFoundry.
//!
//! # Responsibility
//!
//! This crate answers questions *about* a WebAssembly binary: what kind of
//! binary is it, which sections does it contain, what does it import and
//! export, does it validate. It is a thin, opinionated layer over
//! [`wasmparser`], which is the only implementation allowed to decode a
//! WebAssembly encoding. WasmFoundry does not ship a hand-written magic
//! reader, LEB128 decoder or validator; re-implementing them would be
//! strictly worse than using the maintained library. The single exception is
//! the fixed eight-byte header below: `wasmparser` exposes no header-only entry
//! point, it reports header problems only as a failed full parse, and a caller
//! that only wants to tell a module from a component should not pay for a full
//! parse. Everything past the header is delegated.
//!
//! # Public API rule
//!
//! No `wasmparser` type may appear in this crate's public surface. Callers
//! depend on `wf-wasm`, not on the parser it wraps, so the analysis dependency
//! can move, change version or be replaced without touching `wf-runtime` or
//! `wf-cli`. Errors are therefore translated into the [`Error`] type defined
//! here rather than returned as parser errors.
//!
//! # Current status
//!
//! PHASE 2 exposes exactly one honest operation: reading the eight-byte binary
//! header. It answers "is this a core module or a component?", and nothing
//! more. It does **not** parse sections, validate the body, or resolve types —
//! those operations arrive with `wf inspect` (PHASE 3), which is the first
//! feature that needs them.

use std::error::Error as StdError;
use std::fmt;

/// The four bytes every WebAssembly binary and component starts with:
/// `\0asm`.
const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];

/// Binary format version of a core module.
const VERSION_CORE: u16 = 1;

/// Binary format version of a component.
///
/// Components deliberately do not use version 1: the version field carries 13
/// (`0x0d`) so that a component is never mistaken for a core module by a
/// consumer that only reads the version.
const VERSION_COMPONENT: u16 = 13;

/// The binary format layer, taken from the second half of the header.
///
/// Core modules use layer 0; components use layer 1.
const LAYER_CORE: u16 = 0;
const LAYER_COMPONENT: u16 = 1;

/// Total size of the WebAssembly binary header: magic, version, layer.
const HEADER_LEN: usize = 8;

mod analysis;

pub use analysis::{
    Analysis, ComponentSummary, CoreModule, Export, ExportKind, FuncSignature, Function,
    FunctionKind, Global, Import, ImportType, Memory, Table, ValueType, analyze,
};

/// The flavour of a WebAssembly binary, as declared by its header layer.
///
/// This is a *format* distinction, not a behavioural one: a core module can be
/// a guest WasmFoundry is able to run, a component needs the component model
/// support that only arrives in a later phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleKind {
    /// A core WebAssembly module (binary format layer 0).
    CoreModule,
    /// A WebAssembly component (binary format layer 1).
    Component,
}

impl fmt::Display for ModuleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleKind::CoreModule => f.write_str("core module"),
            ModuleKind::Component => f.write_str("component"),
        }
    }
}

/// Why a binary could not be classified as a WebAssembly binary.
///
/// Each variant names a specific defect in the input, so callers can report *what*
/// is wrong with an input instead of a generic failure.
///
/// This type is not `Copy`: a parser message is owned text, not a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Fewer than eight bytes were available, so the header is incomplete.
    /// `available` is the number of bytes actually received.
    TruncatedHeader {
        /// Bytes present in the input, always less than [`HEADER_LEN`].
        available: usize,
    },
    /// The input does not start with the WebAssembly magic number.
    BadMagic([u8; 4]),
    /// The binary format version is not the one this implementation knows.
    UnsupportedVersion {
        /// Version field read from the header.
        version: u16,
    },
    /// The header declares a format layer that is neither core nor component.
    UnsupportedLayer {
        /// Layer field read from the header.
        layer: u16,
    },
    /// The header was fine but the body is not a valid module. This is the code path
    /// for malformed input: truncated sections, counts that overrun the binary, type
    /// indexes that were never defined.
    InvalidWasm {
        /// What the parser rejected, in its own words.
        message: String,
        /// Byte offset the parser blamed, when it named one.
        offset: Option<u64>,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TruncatedHeader { available } => write!(
                f,
                "truncated WebAssembly header: {available} bytes available, {HEADER_LEN} required"
            ),
            Error::BadMagic(bytes) => write!(
                f,
                "not a WebAssembly binary: expected magic {WASM_MAGIC:02x?}, found {bytes:02x?}"
            ),
            Error::UnsupportedVersion { version } => {
                write!(f, "unsupported WebAssembly binary version {version}")
            }
            Error::UnsupportedLayer { layer } => {
                write!(f, "unsupported WebAssembly binary layer {layer}")
            }
            Error::InvalidWasm { message, offset } => match offset {
                Some(offset) => write!(f, "invalid WebAssembly module at byte {offset}: {message}"),
                None => write!(f, "invalid WebAssembly module: {message}"),
            },
        }
    }
}

impl StdError for Error {}

/// Determines whether a byte slice is a core module or a component by reading
/// only the eight-byte binary header.
///
/// The check is intentionally cheap and total: it inspects the magic number,
/// the format version and the format layer, and nothing else. Section bodies
/// are not read, so a successful result does **not** mean the binary is valid
/// or runnable — validation is a separate operation provided by `wasmparser`
/// and used once a feature needs it.
pub fn classify(bytes: &[u8]) -> Result<ModuleKind, Error> {
    if bytes.len() < HEADER_LEN {
        return Err(Error::TruncatedHeader {
            available: bytes.len(),
        });
    }

    let mut magic = [0u8; 4];
    magic.copy_from_slice(&bytes[..4]);
    if magic != WASM_MAGIC {
        return Err(Error::BadMagic(magic));
    }

    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    let layer = u16::from_le_bytes([bytes[6], bytes[7]]);

    // The layer decides which version is legitimate: a core module carries version 1
    // and a component carries version 13. A header that pairs a known layer with the
    // wrong version is a version problem, not a layer problem, and saying so keeps the
    // diagnosis accurate for whoever has to fix the file.
    match layer {
        LAYER_CORE if version == VERSION_CORE => Ok(ModuleKind::CoreModule),
        LAYER_COMPONENT if version == VERSION_COMPONENT => Ok(ModuleKind::Component),
        LAYER_CORE | LAYER_COMPONENT => Err(Error::UnsupportedVersion { version }),
        _ => Err(Error::UnsupportedLayer { layer }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real header of a core module: magic, version 1, layer 0.
    const CORE_MODULE_HEADER: [u8; 8] = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

    /// Real header of a component: magic, version 13 (0x0d), layer 1.
    const COMPONENT_HEADER: [u8; 8] = [0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00];

    #[test]
    fn classifies_core_module_header() {
        assert_eq!(classify(&CORE_MODULE_HEADER), Ok(ModuleKind::CoreModule));
    }

    #[test]
    fn classifies_component_header() {
        assert_eq!(classify(&COMPONENT_HEADER), Ok(ModuleKind::Component));
    }

    #[test]
    fn ignores_bytes_after_the_header() {
        // Classification reads 8 bytes; a body with unknown content must not
        // change the verdict, which is what "header only" means.
        let mut with_body = CORE_MODULE_HEADER.to_vec();
        with_body.extend_from_slice(&[0xff; 32]);
        assert_eq!(classify(&with_body), Ok(ModuleKind::CoreModule));
    }

    #[test]
    fn rejects_non_wasm_magic() {
        let mut wrong = CORE_MODULE_HEADER;
        wrong[0] = 0x7f; // ELF magic, a realistic mistake
        assert_eq!(
            classify(&wrong),
            Err(Error::BadMagic([0x7f, 0x61, 0x73, 0x6d]))
        );
    }

    #[test]
    fn rejects_unknown_version() {
        let mut wrong = CORE_MODULE_HEADER;
        wrong[4] = 0x02; // version 2
        assert_eq!(
            classify(&wrong),
            Err(Error::UnsupportedVersion { version: 2 })
        );
    }

    #[test]
    fn rejects_unknown_layer() {
        let mut wrong = CORE_MODULE_HEADER;
        wrong[6] = 0x02; // layer 2
        assert_eq!(classify(&wrong), Err(Error::UnsupportedLayer { layer: 2 }));
    }

    #[test]
    fn rejects_truncated_input() {
        assert_eq!(classify(&[]), Err(Error::TruncatedHeader { available: 0 }));
        assert_eq!(
            classify(&CORE_MODULE_HEADER[..7]),
            Err(Error::TruncatedHeader { available: 7 })
        );
    }

    #[test]
    fn error_messages_name_the_defect() {
        let empty = Error::TruncatedHeader { available: 3 };
        assert!(empty.to_string().contains("3 bytes"));
        let bad = Error::BadMagic([0x7f, 0x45, 0x4c, 0x46]);
        assert!(bad.to_string().contains("not a WebAssembly binary"));
        assert!(std::error::Error::source(&bad).is_none());
    }

    #[test]
    fn module_kind_renders_readable_labels() {
        assert_eq!(ModuleKind::CoreModule.to_string(), "core module");
        assert_eq!(ModuleKind::Component.to_string(), "component");
    }
}
