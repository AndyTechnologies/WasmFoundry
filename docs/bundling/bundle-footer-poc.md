# Bundle Footer POC — Design Notes

> **Status:** POC (proof of concept). The format is **not frozen**.
> Format v1 will be frozen in **PHASE 15** after this POC validates the approach.

## Overview

The bundle format embeds a WebAssembly payload inside a native executable
(launcher) so that a single binary can be distributed and executed without
any external tooling. The launcher locates the payload by reading a fixed
footer from the end of the file — **no ELF, Mach-O, or PE section parsing**
is required.

## Layout

```
[ launcher bytes ][ payload bytes ][ metadata ][ footer ]
[0           ]    [A          ]    [B        ][C      ]
                 payload_offset    metadata_offset    EOF
```

| Section    | Purpose                                                     |
| ---------- | ----------------------------------------------------------- |
| Launcher   | Native executable (Rust binary) that bootstraps the payload |
| Payload    | WebAssembly module to be executed                           |
| Metadata   | Free-form bytes (manifest, descriptor, version info)        |
| Footer     | 84-byte fixed structure that indexes the file               |

## Footer Fields

All fields are **little-endian** byte order. Total size: **84 bytes**.

| Offset | Size | Type   | Field             | Description                                        |
| ------ | ---- | ------ | ----------------- | -------------------------------------------------- |
| 0      | 8    | `[u8;8]` | magic           | ASCII `"WFBUNDLE"` — no null padding               |
| 8      | 4    | u32    | format_version    | `0` for POC; v1 frozen in PHASE 15                |
| 12     | 4    | u32    | header_size       | Size of the footer in bytes (84)                   |
| 16     | 8    | u64    | payload_offset    | Absolute file offset where payload begins           |
| 24     | 8    | u64    | payload_size      | Length of the payload section in bytes              |
| 32     | 8    | u64    | metadata_offset   | Absolute file offset where metadata begins          |
| 40     | 8    | u64    | metadata_size     | Length of the metadata section in bytes             |
| 48     | 32   | `[u8;32]` | checksum        | SHA-256(payload_bytes ++ metadata_bytes)           |
| 80     | 4    | u32    | flags             | `0` (reserved for future use)                      |

### Field notes

- **magic** — Exactly 8 ASCII bytes `"WFBUNDLE"`. Chosen to be human-readable
  in a hex dump and distinct enough to avoid false positives in random binary data.
- **format_version** — Set to `0` during POC. When the format stabilises in
  PHASE 15, the version will increment to `1`. This makes it impossible to
  confuse a POC bundle with a production one.
- **header_size** — In the POC this equals `FOOTER_LEN` (84). The plan lists
  this field as "header size"; the name will be finalised in PHASE 15 when
  the format is frozen. For now it encodes the footer length for self-validation.
- **payload_offset / payload_size** — Together these define the payload region
  as an absolute file range `[payload_offset, payload_offset + payload_size)`.
- **metadata_offset / metadata_size** — Same pattern for metadata. The layout
  invariant requires `payload_offset + payload_size == metadata_offset` and
  `metadata_offset + metadata_size == file_length - FOOTER_LEN`.
- **checksum** — SHA-256 computed over `payload_bytes ++ metadata_bytes`
  (concatenation of the two sections). Verifies integrity of both the
  WebAssembly module and its descriptor before execution.
- **flags** — Reserved. Set to `0` in the POC. Future use may include
  compression flags, signing indicators, or version negotiation hints.

## Launcher behaviour

The dev launcher (`wf-launcher`) operates as follows:

1. **Self-read:** Reads its own executable bytes via `std::env::current_exe()`.
2. **Footer extraction:** Reads the last 84 bytes as the footer.
3. **Validation:** Checks magic, version, layout consistency, and checksum.
4. **Execution:** Runs the payload as a WebAssembly module using `wf-runtime`
   with `console()` host functions and `RuntimePolicy::denied()`.
5. **Reporting:** Prints `wf-launcher: ran <name>, returned <N>` on success;
   exits with code 1 and a diagnostic on any error.

If the executable is not a bundle (e.g., a regular Rust binary), `probe()`
returns an error and the launcher exits with code 1 — no panic.

## Build process

The bundle is assembled by concatenating the sections in order:

```
launcher_bytes ++ payload_bytes ++ metadata_bytes ++ footer_bytes
```

Offsets are computed from the lengths of preceding sections, the checksum is
computed over `payload ++ metadata`, and the footer is appended last.

## Edge cases

- **Empty payload:** Valid — the checksum covers an empty payload + metadata.
- **Empty metadata:** Valid — the checksum covers payload + empty metadata.
- **Empty launcher:** Valid — a 0-byte launcher means `payload_offset == 0`.
- **Truncated file:** Rejected as `BundleError::TooShort` if fewer than 84 bytes.
- **Single-byte corruption:** Any byte flip in payload, metadata, or checksum
  bytes is detected by the SHA-256 verification.

## References

- Plan sections: [§69](../../plan.md#69) (checksums), [§72–§77](../../plan.md)
  (bundle format), [§3](../../plan.md#3) (repository structure).
- Feature document: [`odd/tasks/bundle-poc.md`](../../odd/tasks/bundle-poc.md).
