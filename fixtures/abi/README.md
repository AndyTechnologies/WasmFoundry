# Host ABI fixtures

Guests that call the host ABI, written as WebAssembly text so the call itself is
readable. The behaviour is asserted end to end through the real binary.

## Why these are WAT and not a compiled guest

A compiled guest would show bytes; the WAT shows the contract — the import name, the
argument order, the pointer and the length. Those four things are what `docs/abi/host-abi-v1.md`
pins, and they are the four things a change would break.

| Fixture | Shows |
| --- | --- |
| `console/printing.wat` | `env.console.log(ptr, len)` writing to standard output |
| `console/assertion-fails.wat` | `console.assert(cond, ptr, len)` reporting without stopping the guest |

The tests assemble these at run time with the pinned `wat` dependency rather than
checking in the `.wasm`, so nothing opaque lands in the tree.
