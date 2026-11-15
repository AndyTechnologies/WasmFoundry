# WasmFoundry Bundle Format v1

## Overview

The WasmFoundry bundle format is a self-contained executable format that wraps a WebAssembly payload with a native launcher. It is designed for simplicity and portability across platforms.

**Format freeze**: PHASE 15. The format is frozen as v1 and must not change without a deliberate ADR process.

## File Layout

A bundle file has the layout `[launcher][payload][metadata][footer]`, where sections tile `[0, file_length)` exactly with no gaps and no overlaps.

```
+----------------------+---------------+--------+---------+
| launcher (variable)  | payload       | metadata | footer  |
|                      | (variable)    | (variable) | (fixed) |
+----------------------+---------------+--------+---------+
           0                      payload_offset     metadata_offset
                                       footer_start      file_length
```

## Footer Layout (all fields little-endian, 84 bytes total)

| Offset | Size | Field                | Description                              |
|--------|------|----------------------|------------------------------------------|
| 0      | 8    | magic                | ASCII `"WFBUNDLE"`                       |
| 8      | 4    | format_version       | `1` (frozen v1)                          |
| 12     | 4    | header_size          | Size of the footer in bytes (= 84)       |
| 16     | 8    | payload_offset       | Absolute file offset of payload section  |
| 24     | 8    | payload_size         | Length of payload section in bytes       |
| 32     | 8    | metadata_offset      | Absolute file offset of metadata section |
| 40     | 8    | metadata_size        | Length of metadata section in bytes      |
| 48     | 32   | checksum             | SHA-256(payload bytes ++ metadata bytes) |
| 80     | 4    | flags                | `0` (reserved)                           |

**Total**: 84 bytes (`FOOTER_LEN`)

## Layout Invariants

The sections tile `[0, file_length)` exactly:

```
[0, payload_offset)          = launcher bytes
[payload_offset, ...)        = payload bytes  (length = payload_size)
[payload_offset + payload_size, ...) = metadata bytes (length = metadata_size)
[metadata_offset + metadata_size, file_length) = footer (exactly FOOTER_LEN)
```

## Validation Rules

The `probe()` function validates the following invariants:

1. **Magic**: The first 8 bytes of the footer must be `"WFBUNDLE"`.
2. **Version**: The format version at offset 8..12 must be `1`.
3. **Header size**: The header_size at offset 12..16 must be 84.
4. **Payload consistency**: `payload_offset + payload_size == metadata_offset`.
5. **Metadata consistency**: `metadata_offset + metadata_size == footer_start`.
6. **Bounds**: All offsets must be within file bounds (`payload_offset <= file_length`, `footer_start <= file_length`).
7. **Checksum**: SHA-256(payload ++ metadata) must match the checksum at offset 48..80.

## Building a Bundle

```rust
use wf_bundle::build;

let launcher = include_bytes!("dev-launcher");  // native executable bytes
let payload = wasm_bytes;                        // WebAssembly module bytes
let metadata = r#"{ "name": "my-project" }"#;    // JSON or other metadata

let bundle = build(launcher, payload, metadata)
    .expect("bundle build succeeds");
```

The `build()` function returns `Ok(Vec<u8>)` with the assembled bundle `[launcher][payload][metadata][footer]`, or `Err(BundleError)` on failure.

## Error Types

### `BundleError::TooShort`

The file is shorter than the minimum footer size (84 bytes).

### `BundleError::BadMagic`

The magic bytes at the footer offset do not match `"WFBUNDLE"`.

### `BundleError::UnknownVersion { found }`

The format version in the footer is not recognized. Only version `1` is supported.

### `BundleError::Layout { source }`

The internal layout is inconsistent: gaps, overlaps, or truncated sections.

### `BundleError::Checksum { expected, found }`

The SHA-256 checksum in the footer does not match the computed hash over `payload ++ metadata`.

## Probing a Bundle

```rust
use wf_bundle::probe;

let file_bytes = std::fs::read("my-app")?;
let probe = probe(&file_bytes)?;

println!("Launcher length: {} bytes", probe.launcher_len);
println!("Payload: {} bytes", probe.payload.len());
println!("Metadata: {} bytes", probe.metadata.len());
println!("Format version: {}", probe.format_version);
println!("Flags: {}", probe.flags);
```

The `probe()` function returns a `Probe` struct with borrowed slices into the original buffer.

## Bundle Probing on Non-Bundle Data

A plain un-bundled executable (e.g., a regular Rust binary) must fail cleanly when probed, not panic. The probe path on non-bundle data should:

1. Attempt to read the footer from the last 84 bytes.
2. Check magic bytes — if they don't match `"WFBUNDLE"`, return `BundleError::BadMagic`.
3. If the file is too short (< 84 bytes), return `BundleError::TooShort`.

This ensures that tools like `file` or `binwalk` can safely probe any executable without crashing.

## Existing Test Coverage

The following test cases are covered in `crates/wf-bundle/src/lib.rs`:

- `footer_length_is_pinned`: Verifies `FOOTER_LEN == 84`.
- `build_and_probe_round_trip_preserves_payload_and_metadata`: Verifies build + probe round-trip.
- `build_and_probe_with_empty_sections`: Verifies building with empty payload/metadata.
- `wrong_magic_is_rejected`: Verifies corruption of magic bytes is rejected.
- `format_v1_is_accepted`: Verifies version `1` is accepted.
- `format_v0_poc_is_rejected`: Verifies version `0` (POC) is rejected.
- `format_v2_future_is_rejected`: Verifies version `2` (future) is rejected.
- `unknown_version_is_rejected`: Verifies version `99` is rejected.
- `truncated_file_is_rejected`: Verifies files shorter than footer are rejected.
- `payload_metadata_size_inconsistency_is_rejected`: Verifies layout inconsistencies are rejected.
- `corrupting_one_payload_byte_causes_checksum_error`: Verifies payload byte flip causes checksum error.
- `corrupting_one_metadata_byte_causes_checksum_error`: Verifies metadata byte flip causes checksum error.
- `corrupting_footer_checksum_byte_causes_checksum_error`: Verifies footer checksum byte flip causes checksum error.
- `empty_file_is_rejected`: Verifies empty file probe fails.
- `file_shorter_than_footer_is_rejected`: Verifies file < 84 bytes probe fails.
- `exactly_footer_len_with_wrong_magic`: Verifies all-zero file with footer length probe fails.

## Platform Notes

### Linux

- The dev-launcher uses `seccomp-bpf` for sandboxing if enabled in the manifest.
- No special OS considerations beyond standard WebAssembly execution.

### Windows

- The dev-launcher uses Job Objects for process isolation if enabled.
- No special OS considerations beyond standard WebAssembly execution.

### macOS

- The dev-launcher uses the native macOS execution path.
- No special OS considerations beyond standard WebAssembly execution.

## Migration

To migrate from an earlier format (POC v0):

1. Update the format version in the footer from `0` to `1`.
2. Ensure the footer layout matches the v1 specification.
3. Re-build all bundles using the new `wf bundle` command.
4. Update any tools that parse the footer to handle the new layout.

The format is intentionally frozen to avoid breaking changes. Any future format changes will require a new version number and a deliberate ADR process.