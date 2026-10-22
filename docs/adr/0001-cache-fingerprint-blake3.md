# ADR-0001: BLAKE3 for build cache fingerprints

## Status

Accepted. Implemented in PHASE 12 (cache).

## Decision

Use BLAKE3 (`blake3` crate) for content fingerprinting of build inputs. Each module's
fingerprint is a single BLAKE3 hash of length-prefixed input segments, rendered as a
64-character hex string.

## Why BLAKE3

- **Speed**: vectorised SIMD on modern CPUs; faster than SHA-256 without a security
  tradeoff for our use case (content identity, not cryptographic integrity).
- **Stable output**: deterministic 32-byte digest, same on every platform.
- **Single crate**: `blake3` is well-maintained with no C build step on common platforms
  (Rust fallback available).
- **No hand-written hashing**: length-prefixing each segment (`u64 LE + bytes`) prevents
  cross-boundary collisions without custom logic.

SHA-256 was considered but rejected: it is suited to distribution checksums, not the
fine-grained, per-module hashing this cache performs. The overhead is unnecessary when
BLAKE3 already provides collision resistance for content identity.

## What the fingerprint covers

Per `plan.md` #68, each module's fingerprint includes (in order):

1. Raw bytes of `wasmfoundry.toml` (whole manifest, not individual fields)
2. Host ABI version label (`abi-v1`)
3. WasmFoundry binary version (`CARGO_PKG_VERSION`)
4. Toolchain cache key (toolchain id + invocation contract)
5. Module name
6. Source file bytes, in the order returned by `toolchain.source_paths()`

The store lives at `target/wf/cache.toml` and records, per module: the fingerprint and
a list of (module-name, fingerprint) pairs representing its project dependencies.

## Limitations

Compiler binary versions are **not** fingerprinted (Rust v1). Upgrading clang/asc/rustc
silently reuses stale artifacts unless `target/` is cleared. This gap is recorded in
`docs/gaps/GAP-0002.md` and deferred to a future phase.
