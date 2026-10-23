# Bundle Format v1 — Frozen

> **Status:** frozen as v1 (plan #75, #77). Supersedes
> [`bundle-footer-poc.md`](bundle-footer-poc.md), which remains as the POC record.
> The POC validated the approach; this document freezes the byte layout, reader
> checks, and payload semantics.

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
| Payload    | WebAssembly core module to be executed                      |
| Metadata   | The project's `wasmfoundry.toml` manifest, verbatim         |
| Footer     | 84-byte fixed structure that indexes the file               |

The sections tile `[0, file_length)` exactly with no gaps and no overlaps.

## Footer byte layout

All fields are **little-endian**. Total size: **84 bytes** (`FOOTER_LEN`).

| Offset | Size | Type      | Field             | Value/Description                                        |
| ------ | ---- | --------- | ----------------- | -------------------------------------------------------- |
| 0      | 8    | `[u8;8]`  | magic             | ASCII `"WFBUNDLE"` — no null padding                     |
| 8      | 4    | u32       | format_version    | `1` (frozen)                                             |
| 12     | 4    | u32       | header_size       | Size of the footer in bytes (`84`)                       |
| 16     | 8    | u64       | payload_offset    | Absolute file offset where payload begins                |
| 24     | 8    | u64       | payload_size      | Length of the payload section in bytes                   |
| 32     | 8    | u64       | metadata_offset   | Absolute file offset where metadata begins               |
| 40     | 8    | u64       | metadata_size     | Length of the metadata section in bytes                  |
| 48     | 32   | `[u8;32]` | checksum          | SHA-256(payload_bytes ++ metadata_bytes)                 |
| 80     | 4    | u32       | flags             | `0` (reserved)                                           |

Hex dump of field order (first 16 bytes shown):

```
0000 5746 4255 4e44 4c45  0100 0000  5400 0000  |WFBUNDLE......T...|
        ^^^^^^^^ magic     ^^ ver    ^^ header
0010  ...                          payload_offset (u64)
0018  ...                          payload_size (u64)
0020  ...                          metadata_offset (u64)
0028  ...                          metadata_size (u64)
0030  ...                          checksum [32 bytes]
0050  0000 0000                 flags
```

## Reader checks (in order)

A reader (`probe()`) must perform these checks in the order listed. Failing any
check must abort immediately with the corresponding error.

1. **File length:** the file must be at least 84 bytes. Reject as `TooShort`.
2. **Footer extraction:** read the last 84 bytes as the footer.
3. **Magic:** bytes `[0..8]` must equal `"WFBUNDLE"`. Reject as `BadMagic`.
4. **Version:** `format_version` at `[8..12]` must equal `1`. Reject as
   `UnknownVersion { found }` with the found value named in the error.
5. **Header size:** `header_size` at `[12..16]` must equal `84`. Reject as
   `Layout`.
6. **Layout tiling:** verify:
   - `payload_offset + payload_size == metadata_offset` (no gap, no overlap)
   - `metadata_offset + metadata_size == file_length - 84` (metadata ends where footer begins)
   Reject as `Layout` if either invariant fails.
7. **Checksum:** compute SHA-256 over `payload_bytes ++ metadata_bytes` and
   compare with the 32-byte checksum at `[48..80]`. Reject as `Checksum` on mismatch.
   (plan #69: distribution checksums use SHA-256.)

## Payload semantics for v1

- **Exactly one core module.** The payload is a single WebAssembly core module.
  Multi-module payloads are not supported in v1; they require a future format
  version with a container format (ADR required if the design changes).
- **Metadata:** the project's `wasmfoundry.toml` written verbatim. No transformation,
  no JSON conversion. A reader can reconstruct the project manifest from the bundle.

## Version and lifecycle policy

The format is frozen at version 1. Changes to the footer layout, field widths, or
reader check order require:

1. An **Architecture Decision Record** (ADR) in `docs/adr/`.
2. A **version bump** (the next version is 2).
3. The `probe()` function must reject the previous version cleanly.

A reader built for v1 accepts **only** version 1. Bundles at version 0 (the POC)
or version 2+ are rejected with `UnknownVersion`.

## Signing rule (plan #78)

The binary that is signed must be the **final artefact**, after the payload is
embedded. The signing cycle is:

```
launcher binary → embed payload + metadata + footer → final binary → codesign → verify signature
```

**Never codesign the launcher before the payload is appended.** Signing the launcher
first invalidates the signature when bytes are appended.

The codesign implementation lands with the release system (PHASES 19–21); this rule
is recorded in the frozen doc so that the format design accounts for it.

## Launcher registry (plan #79, #80)

### Local resolution order

`wf bundle --bundle-target <triple>` resolves the launcher without invoking `cargo`:

1. `WF_LAUNCHER_REGISTRY/<triple>/wf-launcher` (env var override, used by tests and CI)
2. `~/.wf/launchers/<triple>/wf-launcher` (default location)

If the resolved path does not exist, the command fails with a usage error naming
the searched path and suggesting `--launcher` as an alternative.

### Cross-compilation property

A prebuilt launcher for a target triple removes the need for the user to have the
Rust toolchain for that triple. Building `wf-launcher` for `aarch64-unknown-linux-gnu`
on an x86_64 host requires the cross-compiler; using a prebuilt launcher does not.

### Future: conceptual registry

The registry is currently a local directory tree. The conceptual design for a remote
registry is:

```
triple → launcher artifact → sha256 → release URL
```

No network access, no plugin system, and no download capability exists in v1.
The conceptual registry is documented here for the PHASE 21 release system to build
against. A maintainer who populates `~/.wf/launchers/` with prebuilt binaries
enables cross-target bundling for end users.
