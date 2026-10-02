# End-to-end tests

End-to-end tests drive the real `wf` binary over a real project on the real filesystem.
They are the only tests that prove the product works as a product, rather than as a set
of components that happen to compile.

## Scope

Unit tests live inside each crate and test domain rules in isolation. Integration tests
live inside `wf-runtime` and exercise real Wasmtime. This directory holds the tests that
span everything:

```text
wf init
  -> wf build
    -> wf run
      -> wf inspect
        -> wf bundle
```

## Rules

- **Drive the binary, not the library.** Tests invoke the built `wf` executable so that
  argument parsing, output formatting and exit codes are all under test. A test that
  calls the library directly cannot catch a broken CLI.
- **Exit codes are asserted, not just output.** A failing command must fail with the
  documented code.
- **No network.** Fixtures are versioned under `fixtures/`, never downloaded.
- **Golden outputs for stable surfaces.** Human and JSON representations are compared
  against golden files once both are supported interfaces.
- **Isolated working directories.** Each test creates its own temporary project and never
  touches the repository tree.

## Structure

```text
tests/
├── e2e/           full pipeline tests, by slice
├── cli/           argument parsing, help, version, exit codes
└── golden/        expected human and JSON output
```

## Current contents

None. The first end-to-end test lands with PHASE 5, when `wf init`, `wf build` and
`wf run` exist.