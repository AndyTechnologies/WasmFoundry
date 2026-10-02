# Fixtures

Test inputs that must exist, be stable, and never change underneath a test.

## Rules

- **Versioned in git.** A fixture is part of the source tree, never downloaded during a
  test run. A test that requires network access is a broken test.
- **Immutable in practice.** Changing a byte of a fixture changes what tests mean. When a
  fixture must change, that is a reviewable decision, not a drive-by edit.
- **Small and explainable.** Every fixture states in this file, or in a sibling
  `README.md`, what it is for and which test owns it.

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

## How fixtures are produced

Binary fixtures are generated, not hand-typed: a `.wat` source is assembled by a pinned
tool version and the resulting bytes are committed. Hand-editing bytes produces fixtures
that cannot be explained in review. The generator script arrives with the first fixture
that needs it; until then the committed bytes are the record.

## Current contents

None. PHASE 3 (`wf inspect`) adds the first real fixtures, together with the golden
outputs that pin the human and JSON representations.