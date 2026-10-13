# Host ABI v1

The contract between a guest and the host that runs it. This document is written before
the implementation and is the reference the implementation follows; where they differ,
the implementation is wrong.

Status of each function is stated per function. "Lands in PHASE 9" means it is specified
here and does not exist yet.

---

## Why this document exists

A guest compiled to WebAssembly has no way to talk to anything outside its linear memory.
Every print, every clock read, every file access crosses a boundary that has to be
defined by someone: the names a guest may import, what each argument means, who owns the
bytes, and what happens when something goes wrong.

Without that, a guest and its host disagree silently — the guest writes a pointer, the
host reads whatever is at that offset, and both believe they are right.

---

## Evidence, and what was decided against it

The previous implementation of this project shipped a host surface. It was read for what
it was: evidence of a contract, and of the places where that contract was awkward.

What was found, verified in the source rather than assumed:

| Fact | Where it was verified |
| --- | --- |
| Console functions lived in the `env` namespace with dotted names (`env.console.log`) | `packages/linker/src/builtin-host-functions.ts` |
| Each took a single `i32` pointer, with no length argument | the generated host code reads `args[0].i32()` and nothing else |
| The string length sat **four bytes before** the pointer, right-shifted once | `_readAsString` in `_string-reader.c.njk`: `memcpy(&len, data + ptr - 4, ...); len >>= 1` |
| The payload was UTF-16, decoded to UTF-8 by the host | `uint16_t* chars = ...; for (...)` with a hand-written UTF-8 encoder |
| Decoding stopped at a NUL code unit, and astral characters were not handled | the encoder has no surrogate pair branch |
| Rust guests did not use `env.console.*` at all — they printed through WASI | the Rust guest crate is `#![no_std]`, imports `wasi_snapshot_preview1`, and `console::log` wraps `wasi::stdout_write` |
| AssemblyScript needed a second, much larger surface | `env.Math.*`, `env.Object.*`, `env.Reflect.*`, `env.String.fromCodePoint`, `env.Date.now`, `env.performance.now`, `env.crypto.getRandomValuesN` |

That last column is the argument for the split below. The previous "console" API was not
language-neutral: it was AssemblyScript's string layout exposed as a general API, and a
Rust or C++ guest could only call it by faking that layout.

**Binary compatibility with that surface is not a goal.** A guest compiled against the
previous ABI must be rebuilt. What is preserved is the *semantics* — what `console.warn`
means, where it goes — tracked in `docs/parity-matrix.md`.

---

## The two surfaces

```text
                    Host ABI v1
                         │
        ┌────────────────┼────────────────┐
        │                │                │
  Portable ABI     Compatibility ABI   WASI
  env.console.*    env.Math.* ...      (PHASE 9)
  language-neutral AssemblyScript's own
```

| Surface | Namespace | Audience |
| --- | --- | --- |
| Portable console ABI | `env` | every guest, in every language |
| AssemblyScript compatibility ABI | `env` | AssemblyScript runtime and stdlib only — PHASE 11 |
| WASI | `wasi_snapshot_preview1` | guests that opt into WASI — PHASE 9 |

The compatibility ABI exists so that AssemblyScript's needs never become the universal
contract. Its functions are named in [`guest-sdks.md`](guest-sdks.md); this document
defines the portable surface.

---

## Names and namespaces

Functions are imported by a **namespace** and a **name**, the two parts of a WebAssembly
import section entry. The namespace is `env` for the portable ABI.

Names use the dotted form `console.log`, matching the convention the platform has:
`console.debug`, `console.info`, `console.warn`, `console.error`, `console.assert`,
`console.time`, `console.timeLog`, `console.timeEnd`.

Naming a function `console.log` rather than `log` keeps every host function identifiable
in a report, in a trap message and in a manifest check, without the runtime having to
know which family it came from.

---

## Signatures

Every portable console function takes an `i32` pointer and an `i32` length, and returns
nothing.

```wat
(import "env" "console.log"  (func (param i32 i32)))
(import "env" "console.warn" (func (param i32 i32)))
(import "env" "console.error"(func (param i32 i32)))
(import "env" "console.assert" (func (param i32 i32 i32)))
```

`console.assert` takes three: a condition, a pointer and a length. It writes the message
only when the condition is zero, and it never traps.

Fixed-width integers and no results, so the same host function serves every guest
language without a language-specific branch.

---

## Strings

**A string crosses the boundary as a pointer and a length, and is UTF-8.**

```text
guest memory:  [ ... | h | e | l | l | o | ... ]
                        ^ pointer
                            ^ length counts these bytes
```

### Why pointer and length, and not a terminator

A NUL-terminated convention makes an embedded NUL unrepresentable, and makes the host
scan memory until it finds a byte the guest chose. A length says exactly which bytes
belong to the argument.

### Why UTF-8, and not UTF-16

The previous surface required UTF-16 because AssemblyScript stores strings that way,
which forced a hand-written UTF-16 to UTF-8 converter into the host and made the same
function unusable from a Rust or a C++ guest. UTF-8 is what Rust, C++, Go and every WASI
API speak natively.

An AssemblyScript guest pays one conversion, in the SDK helper that writes a string —
once per call site, not once per host function. That is the cost the split exists to
place deliberately instead of by accident.

### What the host guarantees

- The bytes it reads are exactly the range `[pointer, pointer + length)`.
- The length is counted in **bytes**, not in code points or code units.
- Invalid UTF-8 is replaced with U+FFFD rather than rejected, so a malformed guest string
  cannot stop output that should still be visible.

---

## Memory ownership

**The memory belongs to the guest. The host borrows.**

- The host reads the range the guest pointed at and copies what it needs. It keeps no
  reference into guest memory after the call returns.
- The host never writes into guest memory in the portable ABI v1. Every function in this
  document reads.
- The host never allocates on the guest's behalf. There is no host-allocated buffer the
  guest must free, because there is no such buffer.

A host function that reads a range the guest did not grant — past the end of memory, a
negative pointer — fails the call with a host error instead of reading anyway.

---

## Errors

Three things can go wrong, and they are not the same thing:

| Cause | Reported as | Code |
| --- | --- | --- |
| The guest passed a pointer or length that addresses nothing | host error | `WF013` |
| The guest has no exported linear memory | host error | `WF013` |
| The guest faulted somewhere else | trap | `WF009` |

Wasmtime surfaces both a failing host function and a guest fault as a trap, so the
runtime records the reason a host function failed and reports it as a host failure
instead. "The host could not read your pointer" and "your guest trapped" need different
fixes, and a report that calls both "execution failed" hides which one happened.

**A failing host function aborts the guest.** There is no partial output: a call that
fails is a trap from the guest's point of view, and the invocation ends.

---

## Capabilities

Printing is the one capability the portable console ABI needs: it reads a range of guest
memory and writes to the host's standard output or standard error. It touches no
filesystem, no network, no environment variable and no host process.

PHASE 9 adds the capability policy, and everything beyond `console` will require an
explicit grant. See [`../security/threat-model.md`](../security/threat-model.md).

---

## The functions

### `env.console.log` — implemented in PHASE 8

`(param i32 i32)` — pointer and length. Writes the message and a newline to standard
output.

### `env.console.debug` — implemented in PHASE 8

Identical to `log` in behaviour and output stream. Kept separate because the level is
what a reader filters on; collapsing them would make filtering impossible.

### `env.console.info` — implemented in PHASE 8

As `log`, standard output.

### `env.console.warn` — implemented in PHASE 8

As `log`, but to **standard error**, so a caller can separate diagnostics from output
with the shell.

### `env.console.error` — implemented in PHASE 8

As `warn`.

### `env.console.assert` — implemented in PHASE 8

`(param i32 i32 i32)` — condition, pointer, length. Writes `Assertion failed: <message>`
to standard error when the condition is zero; otherwise writes nothing. Never traps.

### `env.console.time`, `timeLog`, `timeEnd` — specified, not implemented

They need a clock, and a clock is a capability. PHASE 9 built the capability policy —
mounts, WASI, bounds — but did not add a clock to it, so these three functions do not
exist and a guest that imports them gets `WF002`.

They stay here as the specification they will be built against: `(param i32 i32)`, a
pointer and a length naming the label, writing to standard error. Adding them is one
capability and three host functions, and no change to this document.

### Not in this ABI

Functions that belong elsewhere: `process.exit` (WASI, PHASE 9), filesystem access
(WASI plus mounts, PHASE 9), random and clock access (PHASE 9), and everything in the
AssemblyScript compatibility ABI (PHASE 11).

---

## Versioning

The Host ABI has its own version, independent of the WasmFoundry release, the bundle
format and the manifest schema. This document is **Host ABI v1**.

| Change | Compatibility |
| --- | --- |
| A new function | compatible — a guest that does not import it is unaffected |
| An added parameter or a changed type | **not** compatible; requires Host ABI v2 |
| A changed memory layout or ownership rule | **not** compatible; requires Host ABI v2 |
| A changed meaning or output stream | **not** compatible; requires Host ABI v2 |

v1 is never edited in place to mean something else. A guest compiled against v1 keeps
running against v1, and a change of meaning is a new version, not a correction.

---

## How a guest uses this

A guest never calls a host function with a raw pointer by hand. The language SDK does:

```text
guest language          SDK helper              Host ABI
------------------      ------------------      ------------------
log("hello")     →      encode UTF-8            console.log(ptr, len)
                        pass pointer and length  →
```

What each SDK owns — encoding, allocation, the helper's own string type — is in
[`guest-sdks.md`](guest-sdks.md).