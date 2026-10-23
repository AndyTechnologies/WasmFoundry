//! Bundle format: `[launcher][payload][metadata][footer]`.
//!
//! The footer is a fixed 84-byte structure at the end of the file. A launcher
//! locates the payload by reading the footer from EOF — no ELF, Mach-O, or PE
//! parsing is required. The format is POC (proof of concept) and not frozen;
//! format v1 is frozen in PHASE 15.
//!
//! # Footer layout (all fields little-endian)
//!
//! | Offset | Size | Field            | Value                                          |
//! | ------ | ---- | ---------------- | ---------------------------------------------- |
//! | 0      | 8    | magic            | ASCII `"WFBUNDLE"`                             |
//! | 8      | 4    | format_version   | `0` (POC; v1 frozen in PHASE 15)              |
//! | 12     | 4    | header_size      | Size of the footer in bytes (= 84)             |
//! | 16     | 8    | payload_offset   | Absolute file offset of payload section         |
//! | 24     | 8    | payload_size     | Length of payload section in bytes              |
//! | 32     | 8    | metadata_offset  | Absolute file offset of metadata section        |
//! | 40     | 8    | metadata_size    | Length of metadata section in bytes             |
//! | 48     | 32   | checksum         | SHA-256(payload bytes ++ metadata bytes)       |
//! | 80     | 4    | flags            | `0` (reserved)                                 |
//!
//! **Total: 84 bytes** (`FOOTER_LEN`).
//!
//! # Layout invariant
//!
//! The sections tile `[0, file_length)` exactly:
//!
//! ```text
//! [0, payload_offset)          = launcher bytes
//! [payload_offset, ...)        = payload bytes  (length = payload_size)
//! [payload_offset + payload_size, ...) = metadata bytes (length = metadata_size)
//! [metadata_offset + metadata_size, file_length) = footer (exactly FOOTER_LEN)
//! ```
//!
//! `probe()` validates this invariant: offsets must form a contiguous tile with no
//! gaps and no overlaps, and the footer must begin where metadata ends.

use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

/// Fixed size of the footer structure in bytes.
///
/// 8 (magic) + 4 (version) + 4 (header_size) + 8*4 (four u64 offsets/sizes)
/// + 32 (checksum) + 4 (flags) = 84.
pub const FOOTER_LEN: usize = 84;

/// ASCII magic bytes identifying a WasmFoundry bundle.
///
/// Exactly 8 bytes, no null padding: `"WFBUNDLE"`.
const MAGIC: [u8; 8] = *b"WFBUNDLE";

/// Current POC format version.
///
/// Format v1 is frozen in PHASE 15; this POC is version 0 to make it
/// impossible to confuse a POC bundle with a frozen one.
const FORMAT_VERSION: u32 = 0;

/// Result of probing a bundle file.
///
/// Borrowed slices refer into the original byte buffer, so the caller
/// retains ownership of the bytes and decides how to store them.
#[derive(Debug)]
pub struct Probe<'a> {
    /// The payload section (typically a WebAssembly module).
    pub payload: &'a [u8],
    /// The metadata section (e.g., a manifest or JSON descriptor).
    pub metadata: &'a [u8],
    /// Size of the launcher section in bytes.
    pub launcher_len: u64,
    /// Format version from the footer.
    pub format_version: u32,
    /// Flags from the footer (reserved, always 0 for POC).
    pub flags: u32,
}

/// Errors that can occur when building or probing a bundle.
///
/// These are CLI-facing diagnostics — the Display message is what the user
/// reads when something is wrong with a bundle file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    /// The file is too short to contain a footer.
    TooShort,
    /// The magic bytes at the footer offset do not match `"WFBUNDLE"`.
    BadMagic,
    /// The format version in the footer is not recognised.
    UnknownVersion { found: u32 },
    /// The internal layout is inconsistent (gaps, overlaps, or truncated sections).
    Layout { source: String },
    /// The SHA-256 checksum in the footer does not match the payload+metadata.
    Checksum { expected: String, found: String },
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BundleError::TooShort => {
                write!(f, "file is too short to contain a bundle footer")
            }
            BundleError::BadMagic => {
                write!(f, "not a WasmFoundry bundle: magic bytes do not match")
            }
            BundleError::UnknownVersion { found } => {
                write!(
                    f,
                    "unrecognised bundle format version {found}; this launcher supports version {FORMAT_VERSION}"
                )
            }
            BundleError::Layout { source } => {
                write!(f, "bundle layout is inconsistent: {source}")
            }
            BundleError::Checksum { expected, found } => {
                write!(
                    f,
                    "bundle checksum mismatch: expected {expected}, got {found}"
                )
            }
        }
    }
}

impl Error for BundleError {}

/// Assembles a bundle from its three sections and a footer.
///
/// The output layout is `[launcher][payload][metadata][footer]`. Offsets are
/// computed, the SHA-256 checksum covers `payload ++ metadata`, and the footer
/// is written last (at the end of the assembled file).
///
/// # Panics
///
/// Does not panic. All failure modes return an error.
pub fn build(launcher: &[u8], payload: &[u8], metadata: &[u8]) -> Result<Vec<u8>, BundleError> {
    let payload_offset = launcher.len() as u64;
    let metadata_offset = payload_offset + payload.len() as u64;

    // Compute SHA-256 over payload ++ metadata.
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hasher.update(metadata);
    let checksum: [u8; 32] = hasher.finalize().into();

    // Assemble the file.
    let total_len = launcher.len() + payload.len() + metadata.len() + FOOTER_LEN;
    let mut out = Vec::with_capacity(total_len);

    out.extend_from_slice(launcher);
    out.extend_from_slice(payload);
    out.extend_from_slice(metadata);

    // Write footer at the end.
    write_footer(
        &mut out,
        payload_offset,
        payload.len() as u64,
        metadata_offset,
        metadata.len() as u64,
        checksum,
    );

    debug_assert_eq!(out.len(), total_len);
    Ok(out)
}

/// Probes a bundle file by reading the footer from EOF.
///
/// Validates magic, version, layout consistency, and SHA-256 checksum.
/// Returns a [`Probe`] borrowing slices from the input buffer.
pub fn probe(file_bytes: &[u8]) -> Result<Probe<'_>, BundleError> {
    if file_bytes.len() < FOOTER_LEN {
        return Err(BundleError::TooShort);
    }

    let eof = file_bytes.len();
    let footer_start = eof - FOOTER_LEN;
    let footer = &file_bytes[footer_start..eof];

    // Parse footer fields.
    let (
        magic,
        format_version,
        header_size,
        payload_offset,
        payload_size,
        metadata_offset,
        metadata_size,
        checksum,
        flags,
    ) = parse_footer(footer)?;

    // Validate magic.
    if magic != MAGIC {
        return Err(BundleError::BadMagic);
    }

    // Validate version.
    if format_version != FORMAT_VERSION {
        return Err(BundleError::UnknownVersion {
            found: format_version,
        });
    }

    // Validate header_size matches our footer length.
    if header_size != FOOTER_LEN as u32 {
        return Err(BundleError::Layout {
            source: format!("footer declares header_size {header_size} but expected {FOOTER_LEN}"),
        });
    }

    // Validate layout tiles [0, eof) exactly.
    // launcher = [0, payload_offset)
    // payload  = [payload_offset, payload_offset + payload_size)
    // metadata = [metadata_offset, metadata_offset + metadata_size)
    // footer   = [footer_start, eof)
    //
    // Checks:
    // 1. payload_offset + payload_size == metadata_offset (no gap, no overlap)
    // 2. metadata_offset + metadata_size == footer_start (metadata ends where footer begins)
    // 3. payload_offset <= eof (offsets are within file bounds)

    let payload_end =
        payload_offset
            .checked_add(payload_size)
            .ok_or_else(|| BundleError::Layout {
                source: "payload offset + payload size overflowed".to_owned(),
            })?;

    if payload_end != metadata_offset {
        return Err(BundleError::Layout {
            source: format!(
                "payload ends at {payload_end} but metadata starts at {metadata_offset}"
            ),
        });
    }

    let metadata_end =
        metadata_offset
            .checked_add(metadata_size)
            .ok_or_else(|| BundleError::Layout {
                source: "metadata offset + metadata size overflowed".to_owned(),
            })?;

    if metadata_end != footer_start as u64 {
        return Err(BundleError::Layout {
            source: format!("metadata ends at {metadata_end} but footer starts at {footer_start}"),
        });
    }

    // Verify all sections are within file bounds.
    if payload_offset as usize > eof || footer_start > eof {
        return Err(BundleError::Layout {
            source: "offsets extend beyond file boundaries".to_owned(),
        });
    }

    // Extract sections.
    let payload = &file_bytes[payload_offset as usize..payload_end as usize];
    let metadata = &file_bytes[metadata_offset as usize..metadata_end as usize];

    // Verify SHA-256(payload ++ metadata) == checksum.
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hasher.update(metadata);
    let computed: [u8; 32] = hasher.finalize().into();

    if computed != checksum {
        return Err(BundleError::Checksum {
            expected: hex(&checksum),
            found: hex(&computed),
        });
    }

    Ok(Probe {
        payload,
        metadata,
        launcher_len: payload_offset,
        format_version,
        flags,
    })
}

// ---- Internal helpers ----

/// Parses the 84-byte footer into its fields.
#[allow(clippy::type_complexity)]
fn parse_footer(
    footer: &[u8],
) -> Result<([u8; 8], u32, u32, u64, u64, u64, u64, [u8; 32], u32), BundleError> {
    if footer.len() < FOOTER_LEN {
        return Err(BundleError::TooShort);
    }

    let mut magic = [0u8; 8];
    magic.copy_from_slice(&footer[0..8]);

    let format_version = u32::from_le_bytes(footer[8..12].try_into().unwrap());
    let header_size = u32::from_le_bytes(footer[12..16].try_into().unwrap());
    let payload_offset = u64::from_le_bytes(footer[16..24].try_into().unwrap());
    let payload_size = u64::from_le_bytes(footer[24..32].try_into().unwrap());
    let metadata_offset = u64::from_le_bytes(footer[32..40].try_into().unwrap());
    let metadata_size = u64::from_le_bytes(footer[40..48].try_into().unwrap());

    let mut checksum = [0u8; 32];
    checksum.copy_from_slice(&footer[48..80]);

    let flags = u32::from_le_bytes(footer[80..84].try_into().unwrap());

    Ok((
        magic,
        format_version,
        header_size,
        payload_offset,
        payload_size,
        metadata_offset,
        metadata_size,
        checksum,
        flags,
    ))
}

/// Writes a footer to the end of the output buffer.
fn write_footer(
    out: &mut Vec<u8>,
    payload_offset: u64,
    payload_size: u64,
    metadata_offset: u64,
    metadata_size: u64,
    checksum: [u8; 32],
) {
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&(FOOTER_LEN as u32).to_le_bytes());
    out.extend_from_slice(&payload_offset.to_le_bytes());
    out.extend_from_slice(&payload_size.to_le_bytes());
    out.extend_from_slice(&metadata_offset.to_le_bytes());
    out.extend_from_slice(&metadata_size.to_le_bytes());
    out.extend_from_slice(&checksum);
    out.extend_from_slice(&0u32.to_le_bytes()); // flags = 0
}

/// Formats a byte slice as a lowercase hex string.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid WebAssembly module: empty module, no imports, no exports.
    /// Magic `\0asm` + version `0x01 0x00 0x00 0x00`.
    const MINIMAL_WASM: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

    #[test]
    fn footer_length_is_pinned() {
        assert_eq!(
            FOOTER_LEN,
            8 + 4 + 4 + 8 + 8 + 8 + 8 + 32 + 4,
            "FOOTER_LEN must be exactly 84 bytes"
        );
    }

    #[test]
    fn build_and_probe_round_trip_preserves_payload_and_metadata() {
        let launcher = b"LAUNCHER_BYTES";
        let payload = MINIMAL_WASM;
        let metadata = b"{'name':'test'}";

        let bundle = build(launcher, payload, metadata).expect("build succeeds");
        let probe = probe(&bundle).expect("probe succeeds");

        assert_eq!(probe.payload, payload);
        assert_eq!(probe.metadata, metadata);
        assert_eq!(probe.launcher_len, launcher.len() as u64);
        assert_eq!(probe.format_version, FORMAT_VERSION);
        assert_eq!(probe.flags, 0);
    }

    #[test]
    fn build_and_probe_with_empty_sections() {
        let payload = MINIMAL_WASM;
        let metadata = b"";

        let bundle = build(b"", payload, metadata).expect("build succeeds");
        let probe = probe(&bundle).expect("probe succeeds");

        assert_eq!(probe.payload, payload);
        assert_eq!(probe.metadata, metadata);
        assert_eq!(probe.launcher_len, 0);
    }

    #[test]
    fn wrong_magic_is_rejected() {
        let mut bundle = build(b"launch", MINIMAL_WASM, b"meta").expect("build");
        // Corrupt the magic at the footer (last 84 bytes).
        let footer_start = bundle.len() - FOOTER_LEN;
        bundle[footer_start] = b'X'; // change 'W' to 'X'

        let err = probe(&bundle).expect_err("should fail with bad magic");
        assert!(matches!(err, BundleError::BadMagic));
    }

    #[test]
    fn unknown_version_is_rejected() {
        let mut bundle = build(b"launch", MINIMAL_WASM, b"meta").expect("build");
        let footer_start = bundle.len() - FOOTER_LEN;
        // Write version 99 at offset 8..12 of footer.
        bundle[footer_start + 8..footer_start + 12].copy_from_slice(&99u32.to_le_bytes());

        let err = probe(&bundle).expect_err("should fail with unknown version");
        assert!(matches!(err, BundleError::UnknownVersion { found: 99 }));
    }

    #[test]
    fn truncated_file_is_rejected() {
        let bundle = build(b"launch", MINIMAL_WASM, b"meta").expect("build");
        // Truncate so the file is shorter than FOOTER_LEN.
        let truncated = &bundle[..FOOTER_LEN - 1];
        assert!(truncated.len() < FOOTER_LEN);

        let err = probe(truncated).expect_err("should fail with too short");
        assert!(matches!(err, BundleError::TooShort));
    }

    #[test]
    fn payload_metadata_size_inconsistency_is_rejected() {
        let mut bundle = build(b"launch", MINIMAL_WASM, b"meta").expect("build");
        let footer_start = bundle.len() - FOOTER_LEN;
        // Corrupt payload_size: set it to 999 (offset 24..32 in footer).
        bundle[footer_start + 24..footer_start + 32].copy_from_slice(&999u64.to_le_bytes());

        let err = probe(&bundle).expect_err("should fail with layout error");
        assert!(matches!(err, BundleError::Layout { .. }));
    }

    #[test]
    fn corrupting_one_payload_byte_causes_checksum_error() {
        let bundle = build(b"launch", MINIMAL_WASM, b"meta").expect("build");
        let mut corrupted = bundle.clone();
        // Flip a byte in the payload section (payload starts at launcher len).
        let payload_start = b"launch".len();
        corrupted[payload_start] ^= 0xFF;

        let err = probe(&corrupted).expect_err("should fail with checksum error");
        assert!(matches!(err, BundleError::Checksum { .. }));
    }

    #[test]
    fn corrupting_one_metadata_byte_causes_checksum_error() {
        let bundle = build(b"launch", MINIMAL_WASM, b"meta").expect("build");
        let mut corrupted = bundle.clone();
        // Metadata follows payload. Flip last byte of metadata.
        let footer_start = corrupted.len() - FOOTER_LEN;
        // Metadata is right before the footer.
        corrupted[footer_start - 1] ^= 0xFF;

        let err = probe(&corrupted).expect_err("should fail with checksum error");
        assert!(matches!(err, BundleError::Checksum { .. }));
    }

    #[test]
    fn corrupting_footer_checksum_byte_causes_checksum_error() {
        let bundle = build(b"launch", MINIMAL_WASM, b"meta").expect("build");
        let mut corrupted = bundle.clone();
        let footer_start = corrupted.len() - FOOTER_LEN;
        // Flip one byte in the checksum field (offset 48..80 in footer).
        corrupted[footer_start + 48] ^= 0xFF;

        let err = probe(&corrupted).expect_err("should fail with checksum error");
        match err {
            BundleError::Checksum { expected, found } => {
                assert_ne!(expected, found);
            }
            other => panic!("expected Checksum error, got {other:?}"),
        }
    }

    #[test]
    fn empty_file_is_rejected() {
        let err = probe(&[]).expect_err("empty file should fail");
        assert!(matches!(err, BundleError::TooShort));
    }

    #[test]
    fn file_shorter_than_footer_is_rejected() {
        let err = probe(&[0u8; FOOTER_LEN - 1]).expect_err("short file should fail");
        assert!(matches!(err, BundleError::TooShort));
    }

    #[test]
    fn exactly_footer_len_with_wrong_magic() {
        let bytes = vec![0u8; FOOTER_LEN];
        let err = probe(&bytes).expect_err("all-zero footer should fail");
        assert!(matches!(err, BundleError::BadMagic));
    }
}
