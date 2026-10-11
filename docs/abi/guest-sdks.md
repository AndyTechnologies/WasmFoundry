# Guest SDKs

The Host ABI is one contract. Each language implements it in its own way, and the SDK is
where that happens.

This document says what belongs to an SDK, so that a host function is never shaped to
make one language's binding convenient.

## The split

```text
                 Host ABI v1
                      │
           ┌──────────┼──────────┐
           ▼          ▼          ▼
       Rust SDK     C++ SDK    AS SDK
```

The ABI decides the names, the argument order and the string representation. An SDK
decides how its language reaches that contract, and owns everything the guest does before
the call and after it returns.

## What every SDK owns

| Responsibility | Why it is the SDK's |
| --- | --- |
| Encoding strings to UTF-8 | Language strings are not UTF-8: Rust's `&str` is already valid UTF-8, AssemblyScript's `String` is UTF-16, and C++ `std::string` is bytes that may not be text |
| Allocating the buffer it passes | The host never allocates for the guest, so the buffer has to come from somewhere the guest controls |
| Freeing that buffer | Whatever was allocated is released where it was allocated |
| Choosing a default encoding on failure | The SDK decides what to pass when its own value cannot be encoded |
| Naming and signature of each helper | `console::log(&str)` in Rust, `Console.log(string)` in AssemblyScript, `console_log(const char*)` in C++ |

## What an SDK never owns

- **The import names.** `env.console.log` is the ABI's, not the SDK's. An SDK that renames
  it has produced a guest that needs a different host.
- **The argument order.** Pointer then length, for every portable console function.
- **What the host does with the bytes.** Writing to stdout versus stderr is the ABI's
  contract, not the SDK's choice.
- **Permissions.** A capability is granted by the invocation, never by the SDK.

## Per-language notes

### Rust

The SDK is a `no_std`-compatible crate with a small API surface. Its `log` takes a
`&str`, which is already valid UTF-8, so encoding is a no-op and the buffer it passes is
the slice itself.

WASI is an alternative path rather than a second console: a guest may print through
WASI's `fd_write` or through the portable console. Both are supported, and which one a
guest uses is a choice of the guest's own build, not of this product.

### C++

The SDK takes `const char*` and a length, or a `std::string_view`. A caller that passes a
NUL-terminated C string passes `strlen` as the length; the SDK does not scan for the
terminator itself.

### AssemblyScript

AssemblyScript strings are UTF-16. The SDK encodes to UTF-8 into a temporary buffer,
passes pointer and length, and frees the buffer after the call.

This is the one language where the ABI costs a conversion. It is paid once, inside the
SDK helper, rather than in every host function — which is why the ABI chose UTF-8 over a
terminator, and why the AssemblyScript compatibility ABI exists separately for what
AssemblyScript's runtime needs beyond printing.

## Status

| SDK | Status |
| --- | --- |
| Rust | lands with the SDK work |
| C++ | lands in PHASE 10, with the C++ toolchain |
| AssemblyScript | lands in PHASE 11, with the AssemblyScript toolchain |

A guest can call the Host ABI without any SDK — the import and the pointer are all a
module needs. The SDKs exist so that a guest does not have to.