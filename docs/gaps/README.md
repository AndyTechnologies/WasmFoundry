# Gaps

A gap is something the rewrite discovers that the master plan does not cover: a missing
contract, an ambiguous behaviour, a platform difference, or a requirement that only
becomes visible while implementing.

Gaps are recorded rather than absorbed silently. A deviation that is never written down
becomes invisible, and invisible deviations are what turn a rewrite into a fork nobody
can reason about.

## When to open one

- **Non-blocking**: open a file here, choose the simplest defensible option, continue.
- **Blocking the current milestone**: investigate and resolve before continuing, then
  record the outcome here.
- **Changes a public architectural decision**: open a file here *and* an ADR in
  [`../adr/`](../adr/).

## Classification

Every gap is classified as exactly one of:

`legacy parity gap` · `architecture gap` · `platform gap` · `toolchain gap` ·
`runtime gap` · `security gap` · `packaging gap` · `documentation gap`

## Numbering

Files are named `GAP-XXXX.md` with a zero-padded sequence number. Numbers are assigned
in discovery order and are never reused or renumbered.

## Template

```markdown
# GAP-XXXX — <title>

## Context

## How it was discovered

## Impact

## Does it block the current milestone?

## Options

## Decision

## Consequences

## Affected tests

## Affected documentation
```

## Open gaps

| Number | Title | Classification | Status |
| --- | --- | --- | --- |
| [GAP-0001](GAP-0001.md) | `wf build --watch` lands before the cache | platform gap | Resolved |
| [GAP-0002](GAP-0002.md) | Compiler binary version not in build cache fingerprint (v1) | architecture gap | Deferred |
