# Threat model

What WasmFoundry protects, what it does not, and where the line sits. Written before
`wf build` learned to execute a compiler, so the boundary is documented before it is
crossed rather than after.

The model follows the plan's five trust classes and states each one's **current** status
and its status **at the end of this rewrite**. A class marked `not yet` is not a promise
that the code is safe; it is a statement that the feature does not exist.

---

## The one rule that governs everything else

**The Wasmtime sandbox protects `wf run`. It does not protect `wf build`.**

```text
wf run  malicious.wasm        Wasmtime sandbox, WASI capabilities, resource limits
                              the guest reaches only what the host hands it

wf build malicious-project    cargo, build.rs, clang, cmake, npm scripts
                              every process the project defines, running as you
```

These are two different trust boundaries and the product never conflates them. A user who
runs a `.wasm` from an untrusted source and a user who builds a project from an untrusted
source are not doing the same thing, and no amount of runtime isolation changes the
second.

Never document, never claim, never imply:

> "WasmFoundry sandboxes the whole project."

That statement is false. `wf build` runs the guest toolchain with the permissions of the
user who invoked it.

---

## Trust classes

### A. Untrusted WebAssembly

**Who controls it:** anyone who can hand `wf inspect`, `wf run` or `wf build` a `.wasm`
file.

| Surface | Controls | Protection | Status |
| --- | --- | --- | --- |
| `wf inspect` | every byte, every declared count and offset | the binary is validated in full before any section is read; nothing is executed | **in place** |
| `wf run` | every instruction, memory growth, loop bounds | Wasmtime: linear memory isolation, no host access, no imports resolved (there is no host ABI yet) | **in place, no limits** |
| `wf build` (precompiled) | every byte of a source module | validation before publishing; a source that is not a module is rejected | **in place** |

**Known gaps, stated rather than implied:**

- **No execution limits yet.** Until PHASE 9 `wf run` sets no wall-clock timeout and no
  memory cap. A guest may run forever or grow without bound. Treat an untrusted module
  as untrusted input and run it only where unbounded execution is acceptable to you.
- **No host ABI yet.** Today this is isolation by *absence*: every import fails to
  resolve because nothing is provided. When PHASE 8 adds the host ABI and PHASE 9 adds
  WASI, isolation becomes a matter of policy, and the policy defaults to deny.

**Residual risk after PHASE 9:** a Wasmtime engine bug. This is an upstream dependency
risk, tracked by `cargo-audit` in PHASE 19, not something WasmFoundry can fix.

---

### B. Untrusted manifest

**Who controls it:** anyone who can write `wasmfoundry.toml`.

The manifest decides which files are read, which toolchain runs, what is written to
`target/`, and which export is invoked. It is therefore attacker-reachable even when
every `.wasm` is trusted.

| Property | Status |
| --- | --- |
| Unknown `schema` is rejected before a single field is read | **in place** |
| Unknown table and field names are rejected, so a typo cannot silently do nothing | **in place** |
| Structural problems are reported all at once, each with advice | **in place** |
| The manifest is data only; it cannot carry executable content | **in place** |

**Known gaps:**

- **Source paths are resolved relative to the working directory.** A manifest may
  declare a source outside the project (`../../elsewhere.wasm`). That is currently
  permitted and only reads a file; it becomes a write concern when `target/` resolution
  gets harder. **PHASE 9 (mounts) revisits this**, and the default for guest-visible
  mounts is `read-only`.
- **Toolchain availability is checked, toolchain *identity* is not signed.** A manifest
  naming `toolchain = "rust"` makes the CLI run `cargo`. Trusting the manifest's
  toolchain field is equivalent to trusting the project.
- **`host_namespaces` declares what the build must not try to resolve.** It cannot name a
  program or a path — only a namespace string that imports are allowed to leave
  unsatisfied — so it widens what a manifest may *link*, never what it may *execute*.
- **`namespace` is a linking name, not a file.** A stated namespace is compared as a
  string against import names; it never reaches the filesystem or the process table.

**Resolved in PHASE 6:** the manifest's `toolchain` field resolves through one `match`
in `wf-cli/src/toolchains/mod.rs` to a toolchain this version implements. It never
resolves to a path, never to a shell string, and there is no way to write a manifest that
makes `wf build` start an arbitrary program. A name the match does not know is refused
with `WF004` listing what does exist. The manifest therefore decides *which known
toolchain* runs, not *what runs*.

---

### C. Untrusted source project

**Who controls it:** anyone who can write a project's sources and its build scripts.

This is the class the plan calls out explicitly, because it is the one most easily
misunderstood: a project under `wf build` can run anything the host user can run.

```text
build.rs            arbitrary Rust, executed by cargo
npm scripts         arbitrary shell, executed by npm
custom compiler scripts, cmake, shell commands
```

| Status | |
| --- | --- |
| Not sandboxed | `wf build` runs `cargo` for a Rust guest, which executes `build.rs` as the invoking user |
| Never sandboxed by Wasmtime | the runtime sandbox is not consulted during a build at all |
| Reachable from a manifest | a `toolchain = "rust"` module makes `cargo` run; see class B for what that field may name |

**Active since PHASE 6.** Building a Rust project executes the crate's build script, and
a build script is arbitrary code. The boundary below is now a description of the shipped
product rather than a warning about a future one.

**The honest statement for users:**

> Building a project runs its toolchain with your permissions. Building a project you do
> not trust is equivalent to running code you do not trust.

**What is out of scope, deliberately:** process-level sandboxing of the build (seccomp,
landlock, containers, job objects). Introducing it is a product decision with a real
operating-system footprint, and it is not part of this rewrite. It is recorded as a
future option, not as a solved problem.

---

### D. Untrusted plugins

**Who would control it:** anyone who could install a plugin.

**Not applicable yet, and not soon.** External plugins are deferred to PHASE 18; the
plan's §84 forbids dynamic Rust libraries, a hand-written C ABI and a `libloading`
plugin API during the first half of the project. The first extensibility is internal
(the `Toolchain` trait, runtime extensions) and those have no third-party code path.

When this class activates, the requirements are: explicit capabilities, a versioned
protocol (Component Model + WIT, or a versioned subprocess protocol) and no ambient
authority. Until then there is nothing to sandbox.

---

### E. The final bundle

**Who controls it:** whoever supplies the launcher and the payload.

**Not applicable yet** — bundle work starts in PHASE 14. The contract to be enforced
then, already fixed by the plan:

| Requirement | Why |
| --- | --- |
| A fixed footer located from EOF | no reliance on ELF, Mach-O or PE internals, so one format logic covers all three |
| A checksum over the payload | corruption and tampering are detected before anything runs |
| An explicit unsupported-format rejection | a launcher must never guess at an unknown format version |
| `codesign` after payload assembly, never before | signing before appending would invalidate the signature |
| Prebuilt launchers, published with a checksum | the launcher is a release artifact, not something built per bundle |

---

## Capability defaults

PHASE 9 turns these from statements into enforced policy. Until then they describe what
the runtime *offers*, which today is nothing:

```text
filesystem          denied
network             denied
environment         denied
host process exec   denied
```

Capabilities are granted explicitly, per invocation, and never inferred from the project
being trusted. The domain expresses them (`FilesystemAccess::ReadOnly`, and so on) and an
adapter translates to whatever `wasmtime-wasi` exposes at the time; the domain never
adopts Wasmtime's types.

**Mount default:** `read-only`. The legacy implementation defaulted relative mounts to
read-write against the build working directory; that default is deliberately changed
because a default is a security decision, not a convenience.

---

## Supply chain

Outside the trust classes but relevant to all of them.

| When | What |
| --- | --- |
| PHASE 19 | `cargo-audit` and `cargo-deny`: licences, advisories, duplicate crates, sources |
| PHASE 19 | SBOM, signed releases, checksums |
| Not before first release | deliberately deferred: auditing a three-crate skeleton finds nothing worth reporting |

Upstream risk that WasmFoundry cannot mitigate: `wasmtime`, `wasmparser`, `wasm-smith`
and `toml` are all trusted to behave. They are pinned by `Cargo.lock` and reviewed by
`cargo-audit`.

---

## What this document does not claim

- That a build is sandboxed. It is not, and will not be.
- That an untrusted guest is bounded. It is bounded by Wasmtime's isolation, not by
  limits, until PHASE 9.
- That plugins or bundles are safe. Neither exists.
- That `wf inspect` is safe against a bug in `wasmparser`. Validation delegates to an
  upstream library; its fuzzing is upstream's job and WasmFoundry's is not to
  reintroduce a hand-written parser.

## Maintaining this document

Every phase that changes a trust boundary updates this file in the same commit. As of
PHASE 6: A has lost its "no limits" note only when PHASE 9 lands, B's decision is closed,
C is live, and E remains future work for PHASE 14–16.