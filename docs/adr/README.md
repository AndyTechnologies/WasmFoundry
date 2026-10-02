# Architecture decision records

An ADR records a decision that constrains future work, together with the alternatives
that were rejected and the conditions under which the decision should be revisited.

Not every choice deserves one. Naming a variable or picking a data structure is not an
architecture decision. An ADR is warranted when a reasonable engineer joining the project
later would otherwise ask "why is it done this way?" and find no answer in the code.

## When to write one

- A crate boundary, or the absence of one.
- A dependency that changes what the project may do at runtime.
- A wire format, an on-disk format, or a guest-visible ABI.
- A security posture, and especially any promise the product makes about isolation.
- A technology substitution with no obvious upgrade path.

Routine decisions inside an established boundary do not need one; they belong in
[`../gaps/`](../gaps/) if they turn out to be wrong.

## States

| State | Meaning |
| --- | --- |
| `proposed` | Written, not yet applied |
| `accepted` | In force |
| `superseded` | Replaced by a later ADR, which is named explicitly |

An accepted ADR is never edited to change its decision. It is superseded by a new file.

## Numbering

Files are named `ADR-XXXX.md`, zero-padded, assigned in decision order, never reused.

## Template

```markdown
# ADR-XXXX — <title>

## Status

accepted

## Context

## Problem

## Alternatives

### A

### B

### C

## Decision

## Consequences

## Reversibility

## Implementation
```

## Records

None yet. The decisions already taken during PHASE 0–2 — repository separation from the
legacy project, the minimal four-crate architecture, the pinned toolchain — are recorded
in `AGENTS.md` and `docs/architecture.md`, which are sufficient until a decision proves
hard to recover from the code.