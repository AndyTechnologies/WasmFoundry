# Fixtures

Test inputs that must exist, be stable, and never change underneath a test.

## Rules

- **Versioned in git.** A fixture is part of the source tree, never downloaded during a
  test run. A test that requires network access is a broken test.
- **Immutable in practice.** Changing a byte of a fixture changes what tests mean. When a
  fixture must change, that is a reviewable decision, not a drive-by edit.
- **Small and explainable.** Every fixture states in this file, or in a sibling
  `README.md`, what it is for and which test owns it.

## How fixtures are produced

**Sources are versioned, binaries are generated.** The `.wat` files in this tree are
the record; the `.wasm` bytes are assembled from them at test time with a pinned
`wat` dev-dependency. A committed binary would be unreviewable — a reviewer sees bytes
and has to trust them — whereas the WAT is readable, and the generator is pinned in
`Cargo.lock`. Nothing is hand-edited at the byte level except where a test needs a
specific corruption, and those cases build the bytes in the test itself so the mutation
is visible.

## Current contents

| Fixture | Shows |
| --- | --- |
| `valid/empty-module.wat` | A module with nothing in it; empty sections must be reported, not omitted |
| `valid/full-sections.wat` | Imported function and memory, exported memory, table, global and function |
| `exports/entry-point.wat` | A dependency-free module exporting `_start`, the precompiled shape |
| `memory/unbounded.wat` | A memory with no declared maximum; the JSON must say `null` explicitly |
| `globals/mutable-and-immutable.wat` | Both global mutabilities in one module |
| `invalid/not-wasm.txt` | Input that is not WebAssembly at all; must fail with `WF001` |
| `entry/missing-start.wat` | Exports exist, but not `_start`; must fail with `WF005` |
| `entry/with-parameters.wat` | `_start` takes an argument; refused as `WF010` rather than guessed at |
| `entry/not-a-function.wat` | `_start` is exported as a memory; `WF010`, not "missing" |
| `runtime/trap.wat` | Faults immediately; the reason must survive, not just the backtrace header (`WF009`) |
| `runtime/unsatisfied-import.wat` | Imports nothing that exists yet; must fail with `WF002` |

## Layout

Planned from PHASE 3 onward:

```text
fixtures/
├── wasm/
│   ├── valid/      minimal well-formed modules
│   ├── invalid/    malformed binaries; must be rejected, never crash
│   ├── imports/    modules with each import class
│   ├── exports/    modules with each export class
│   ├── memory/     memory and table shapes, min/max pages
│   ├── globals/    global types and mutability
│   └── cycles/     modules forming a dependency cycle
├── projects/       complete sample projects per toolchain
│   ├── precompiled/
│   ├── rust/
│   ├── cpp/
│   └── assemblyscript/
└── bundles/        produced bundle artifacts and corruption cases
```

The `imports/` and `cycles/` subdirectories arrive with PHASE 7, when module matching
and the dependency graph need cases they can be tested against. The `entry/` and
`runtime/` subdirectories describe PHASE 4's failure modes; `wf run`'s end-to-end tests
assemble these same cases inline so the assertions and the fixtures stay in step.