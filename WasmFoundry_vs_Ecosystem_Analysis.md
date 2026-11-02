# WasmFoundry vs. Ecosistema WebAssembly (Oct 2026) — Análisis de Brechas y Plan de Acción

**Documento base:** `Estado_Wasm_Oct_2026_Version_Consolidada.md`
**Estado actual WasmFoundry:** PHASE 10–11 (multi-toolchain, multi-module, Host ABI v1, WASI, mounts, limits, cache, watch, bundle POC)
**Fecha de corte:** 24 octubre 2026

---

## 1. Matriz de Comparación: WasmFoundry vs. Estado del Ecosistema

| Área Ecosistema (Oct 2026) | Estado Ecosistema | WasmFoundry Actual | Brecha | Prioridad |
|---------------------------|-------------------|---------------------|--------|-----------|
| **Core Wasm 3.0** | Tier 1 (GC, memory64, EH, tail-call, SIMD, refs) | `wasmparser` 0.260 / Wasmtime 49: soporta todo | **Ninguna** — delega en Wasmtime/wasmparser | ✅ |
| **WASI 0.2** | Estable (Tier 1 Wasmtime) | Implementado: `wasmtime-wasi` preview1/preview2 | **Ninguna** — `RuntimePolicy::granted(wasi=true)` | ✅ |
| **WASI 0.3** | **Experimental solo Wasmtime** (v43+) | **NO implementado** — usa preview1 | **Crítica** — sin async nativo, streams, futures | 🔴 |
| **Component Model MVP** | Estable desde WASI 0.2 | **Parcial** — `ComponentRuntime` esqueleto, `wf component build` | **Alta** — WIT parsing, compose, world resolution | 🟠 |
| **Runtimes cross-platform** | Wasmtime Tier 1 x86_64; ARM64 Tier 2 | Embebe Wasmtime 49 (Tier 1 x86_64) | **Media** — ARM64/RISC-V no probados en CI | 🟡 |
| **Debugging/DWARF** | Tier 3 Wasmtime (sin maintainer) | **NO implementado** — `wf inspect` solo estático | **Alta** — sin simbolización traps, core dumps | 🟠 |
| **Observabilidad** | Fragmentada (Dylibso, Fermyon, Chrome) | **NO implementado** — sin OTel, métricas, traces | **Alta** — sin `wasi-telemetry`, host spans | 🟠 |
| **Package Management** | Warg archivado → wasm-pkg-tools/OCI (WIP) | **Propio** — `wf build` publica en `target/`, sin registry | **Crítica** — sin lockfile, SBOM, firmas, OCI | 🔴 |
| **SDK Bindings multi-lang** | `wit-bindgen` por lenguaje, fragmentado | **Parcial** — Rust SDK (ABI v1), C++/AS pendientes | **Alta** — sin `wit-sdk` unificado, registry WIT | 🟠 |
| **WASI Portability** | Subconjuntos por runtime, 3 generaciones WASI | **Parcial** — `host_namespaces` declara externos | **Media** — sin capability check pre-deploy, adapters | 🟡 |
| **Seguridad/Límites** | Wasmtime Tier 1 (fuel, epochs, guard regions) | **Implementado** — wall-clock, memory, denies por defecto | **Baja** — fuel pendiente, sin attestation | 🟢 |
| **Optimización** | wasm-opt/Binaryen maduro | **NO implementado** — `cargo -O` solo, sin wasm-opt | **Media** — sin pipeline por objetivo (edge/server) | 🟡 |
| **Benchmarks** | Solo libsodium 2026 (00f.net), ad-hoc | **NO implementado** | **Media** — sin `wf bench`, suite estándar | 🟡 |

---

## 2. Estado Detallado por Capacidad WasmFoundry (vs. Plan Original)

| Capacidad | Plan Phase | Estado Real | Gap vs Ecosistema |
|-----------|------------|-------------|-------------------|
| `wf inspect` | 3 | ✅ **Done** — header + full validation + imports/exports/memories/tables/globals/functions | ✅ Core Wasm analysis completa |
| `wf run` (core) | 4 | ✅ **Done** — Engine/Module/Store/Linker/Instance, entry `_start` o `--entry` | ✅ Ejecución básica |
| `wf init` / `wf build` (precompiled) | 5 | ✅ **Done** — manifest `wasmfoundry.toml` schema=1, validación, publish a `target/` | ✅ Proyecto mínimo |
| Rust toolchain | 6 | ✅ **Done** — `cargo build --target wasm32-unknown-unknown`, fingerprinting, cache | ✅ Guest Rust |
| Multi-module + matching | 7 | ✅ **Done** — graph, topological order, cycle detection, `FileName`/`NameOnly` | ✅ Composición |
| Host ABI v1 | 8 | ✅ **Done** — `env.console.*` (UTF-8 ptr/len), versionado ABI | ✅ ABI portable |
| WASI + mounts + limits | 9 | ✅ **Done** — `RuntimePolicy`, `FilesystemAccess`, `Mount`, `ExecutionLimits` (wall-clock, memory) | ✅ Sandbox + capacidades |
| C++ toolchain | 10 | ✅ **Done** — `CppToolchain` (clang/wasi-sdk), detection, compile, cache key | ✅ Guest C++ |
| AssemblyScript toolchain | 11 | ✅ **Done** — `AssemblyScriptToolchain` (asc, runtime variants, optimize, shrink) | ✅ Guest AS |
| `wf-toolchains` crate | 10 | ✅ **Done** — trait `Toolchain`, 5 impls, `lookup` security boundary | ✅ Arquitectura |
| Cache (BLAKE3) | 12 | ✅ **Done** — fingerprint: manifest, toolchain, sources, deps; skip entry | ✅ Cache local |
| Watch | 13 | ✅ **Done** (GAP-0001) — debounced rebuild, ignora `target/`, editor noise | ✅ Dev loop |
| Bundle v1 + launcher | 14 | ✅ **POC Done** — `wf-bundle` crate, footer fijo, `wf-launcher` dev | ⚠️ Distribución launchers pendiente |
| Component Model | 17 | 🟡 **Parcial** — `ComponentRuntime` esqueleto, `wf component build`, `wit` command | 🔴 **Brecha mayor** |
| Supply-chain / CI | 19–21 | ❌ **Pendiente** — sin CI, sin SBOM, sin firmas, sin `cargo-audit` | 🔴 **Crítica para v0.1.0** |

---

## 3. Brechas Críticas vs. Ecosistema (Prioridad 🔴)

### 3.1 WASI 0.3 / Component Model Async — **BLOQUEANTE**

**Ecosistema:** WASI 0.3 ratificado jun 2026 con async nativo (`async func`, `stream<T>`, `future<T>`). Wasmtime v43+ tiene snapshot experimental; roadmap bimestral.

**WasmFoundry:** Solo `wasmtime-wasi` preview1 (`wasi_snapshot_preview1`). `ComponentRuntime` compila pero **no ejecuta WASI 0.3** — `wasi:cli/run` stub, sin `WasiView` implementation.

**Qué falta:**
- `wasmtime-wasi` preview2/preview3 (p2/p3 features en Wasmtime)
- `WasiView` implementation para `Store<HostState>` (requerido por `wasmtime-wasi` p3)
- `wasi:io`, `wasi:filesystem`, `wasi:sockets`, `wasi:http` async interfaces
- Canonical ABI async (component-model-async Tier 1 en Wasmtime)

**Plan de implementación:**
```rust
// 1. Actualizar Cargo.toml: wasmtime-wasi con features ["p2", "p3"]
wasmtime-wasi = { version = "49", features = ["p2", "p3"] }

// 2. Implementar WasiView para HostState (wf-runtime/src/policy.rs)
struct HostState {
    // ...existing
    wasi_ctx: wasmtime_wasi::p3::WasiCtx,  // o WasiView trait
}

// 3. Registrar WASI p3 en Linker
wasmtime_wasi::p3::add_to_linker(&mut linker)?;

// 4. ComponentRuntime: usar wasi:cli/run real (no stub)
// 5. Actualizar RuntimePolicy para declarar capacidades WASI 0.3 por módulo
```

**Archivos a tocar:**
- `Cargo.toml` (workspace deps)
- `crates/wf-runtime/src/policy.rs` — `WasiView` + engine config
- `crates/wf-runtime/src/lib.rs` — `ComponentRuntime::run` real
- `crates/wf-core/src/policy.rs` — capacidades WASI 0.3 en `RuntimePolicy`
- `crates/wf-cli/src/component.rs` — `wf component build` end-to-end

---

### 3.2 Package Management / Supply Chain — **BLOQUEANTE PARA v0.1.0**

**Ecosistema:** Warg archivado jul 2025 → `wasm-pkg-tools` sobre OCI (WIP). Sin registry dominante con firmas/SBOM/provenance. `wapm` solo Wasmer. `wash` (wasmCloud) usa OCI.

**WasmFoundry:** `wf build` publica en `target/` local. **Sin:**
- Lockfile universal (`wasm-lock.json` con hashes, deps transitivas WIT)
- Publicación OCI (`oras`, `cosign`, `notation`)
- SBOM (SPDX/CycloneDX) generado en build
- Firmas/attestation (SLSA, cosign)
- Vulnerability scanning (Grype/OSV)
- Policy engine (admission: "solo firmados por org X", "sin WASI 0.1")

**Qué falta implementar (Mínimo para v0.1.0):**
1. **Lockfile** — `wf build --lock` genera `wasmfoundry.lock` con:
   ```json
   {
     "version": 1,
     "modules": [
       {"name": "app", "toolchain": "rust", "hash": "blake3...", "deps": []},
       {"name": "lib", "toolchain": "cpp", "hash": "blake3...", "deps": ["app"]}
     ],
     "wit_deps": [],
     "policy": {"wasi": true, "mounts": [...]}
   }
   ```
2. **SBOM** — `wf build --sbom` usa `wasm-tools metadata add --producer --sbom`
3. **Verificación** — `wf verify target/app.wasm` valida: hash, firma (si existe), SBOM, wasmparser validation
4. **Publicación OCI** — `wf publish --registry ghcr.io/user/project` (usa `wasm-pkg-tools` o `oras`)

**Archivos a tocar:**
- `crates/wf-core/src/manifest.rs` — `Lockfile` struct
- `crates/wf-toolchains/src/cache.rs` — extender `Entry` con deps WIT
- `crates/wf-cli/src/build.rs` — `--lock`, `--sbom` flags
- `crates/wf-cli/src/` — nuevo `verify.rs`, `publish.rs`
- `crates/wf-bundle/src/` — incluir lockfile/SBOM en bundle metadata

---

### 3.3 Debugging / Observabilidad — **ALTA (DX crítico)**

**Ecosistema:** DWARF Tier 3 Wasmtime ("no active maintainer"), Chrome DevTools sí, lldb/gdb Wasmtime sí. **Fragmentado.** Dylibso Observe SDK para Shopify, Fermyon OTel proposal. Retos: tamaño DWARF, unwinding no estándar, traces prod con símbolos.

**WasmFoundry:** `wf inspect` **solo estático** (header + wasmparser analysis). **Sin:**
- Simbolización traps (DWARF → file:line)
- Core dump analysis
- Host spans/métricas OTel
- `wasi:logging` / `wasi:telemetry` implementation
- Traces distribuidas host↔componente

**Plan mínimo (Fase 1):**
1. **`wf inspect --format json`** — ya existe, extender con DWARF info si presente
2. **`wf run --coredump-on-trap`** — capturar core dump Wasmtime, `wf coredump analyze`
3. **Host OTel spans** — `tracing` crate + `opentelemetry` en `wf-runtime` host functions
4. **`wasi:logging` implementation** — host function que exporta a OTel collector

**Archivos:**
- `crates/wf-runtime/src/lib.rs` — `Runtime::run` captura traps + coredump
- `crates/wf-runtime/src/host.rs` — host functions con `tracing` spans
- `crates/wf-cli/src/inspect.rs` — `--format json` + DWARF parsing (usar `wasm-tools addr2line` o `gimli`)
- `crates/wf-cli/` — nuevo `coredump.rs`, `trace.rs`

---

### 3.4 SDK Común / WIT Registry — **ALTA (Ecosistema)**

**Ecosistema:** `wit-bindgen` por lenguaje (Rust, Go, TS, Python, C#, Java, Zig, MoonBit). **Sin registry WIT versionado**, sin CLI unificado, sin type provider runtime.

**WasmFoundry:** `wf wit check` valida WIT con `wasm-tools`. **Sin:**
- `wf component add wasi:filesystem@0.2` — resuelve WIT, genera bindings, actualiza lockfile
- Registry WIT (OCI-native como `wit-registry`)
- Generación bindings multi-lang desde WIT (Rust ✅, C++ ❌, AS ❌, Python ❌)
- Harness interoperabilidad (mismo componente, tests en cada lang)

**Plan:**
1. **`wf wit fetch <pkg>@<ver>`** — descarga WIT de registry OCI (wasm-pkg-tools)
2. **`wf component new --wit <dir>`** — genera componente desde core wasm + WIT (usa `wit-component`)
3. **Bindings generators** — integrar `wit-bindgen` para Rust/C++/AS en `wf-toolchains`

---

## 4. Brechas Medias (Prioridad 🟡🟠)

### 4.1 Optimización Automática (`wasm-opt` pipeline)

**Ecosistema:** Binaryen/wasm-opt maduro, passes: DCE, inlining, SIMD, GC, memory packing. wasm-tools `mutate/shrink` para testing.

**WasmFoundry:** Solo `cargo -O` (compiler opt). **Sin** post-build wasm-opt.

**Implementar:**
```rust
// crates/wf-toolchains/src/toolchains/optimizer.rs
pub struct WasmOptimizer {
    profile: OptProfile,  // Size | Speed | Balanced | Edge
    target_runtime: TargetRuntime,  // Wasmtime | Wasmer | WasmEdge
}

impl WasmOptimizer {
    pub fn optimize(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        // wasm-opt con flags por profile/target
    }
}
```

**Integración:** `wf build --release --opt-profile edge --target-runtime wasmtime`

---

### 4.2 Benchmarks Reproducibles (`wf bench`)

**Ecosistema:** Solo libsodium 2026 (00f.net) con metodología rigurosa (freq fija, warmup, media geométrica, outlier exclusion). No suite estándar.

**WasmFoundry:** Nada.

**Implementar `wf bench`:**
```bash
wf bench --workload http-server --runtimes wasmtime@50,wasmer@7 --iterations 100 --warmup 10
```

**Workloads built-in (fixtures/benchmarks/):**
- `hello-world` — cold start, instantiation
- `http-server` — throughput, latency p50/p95/p99
- `json-parse` — compute bound
- `crypto-hash` — SIMD, memory
- `sqlite-query` — WASI filesystem + compute
- `ml-inference` — WASI-NN (si disponible)

**Output:** JSON + HTML report con violin plots, significancia estadística.

---

### 4.3 WASI Capability Check Pre-Deploy

**Ecosistema:** Runtimes implementan subconjuntos WASI distintos. `wasi-virt` pattern para virtualización.

**WasmFoundry:** `host_namespaces` declara externos, pero **sin chequeo previo** de capacidades vs host.

**Implementar:**
```bash
wf deploy --check-capabilities --target-runtime wasmtime@50
```

**Lógica:**
1. Leer imports del componente (`wf_wasm::analyze`)
2. Mapear a capacidades WASI (`wasi:filesystem`, `wasi:sockets`, etc.)
3. Consultar matriz `runtime_capabilities.toml` (versionada, actualizable)
4. Reportar: ✅ disponible, ⚠️ partial, ❌ missing + mitigation

---

### 4.4 Cross-runtime Compatibility Matrix

**Ecosistema:** Wasmtime Tier 1 x86_64; Wasmer/WasmEdge/wazero variables. WASI 0.3 solo Wasmtime experimental.

**WasmFoundry:** Hardcodeado a Wasmtime 49. **Sin:**
- `wf test --runtimes wasmtime@50,wasmer@7,wasmedge@0.17` (containers)
- Matriz de compatibilidad documentada (`docs/compatibility-matrix.md`)

---

## 5. Brechas Menores / Diferidas (Prioridad 🟢)

| Brecha | Estado | Acción |
|--------|--------|--------|
| Fuel budget (deterministic execution) | Pendiente | `RuntimePolicy.limits.fuel` + `store.limiter` |
| Attestation/Verification (`wasm-attest`) | No | Post-v0.1.0; requiere `cosign` + policy engine |
| `wit-registry` federado | No | Colaborar con Bytecode Alliance en `wasm-pkg-tools` |
| `wasi-telemetry` proposal | No | Seguimiento WASI subgroup; implementar cuando estable |
| Plugin system externo | Dropped | Plan dice: "internal extensions first" (Toolchain, RuntimeExtension) |

---

## 6. Plan de Acción Consolidado por Fases

### FASE A: Completar Component Model + WASI 0.3 (Bloqueante v0.1.0+)

| Semana | Tarea | Archivos | Criterio Done |
|--------|-------|----------|---------------|
| 1 | Actualizar `wasmtime-wasi` a features `p2`, `p3` | `Cargo.toml` | Compila |
| 2 | Implementar `WasiView` para `HostState` | `wf-runtime/src/policy.rs` | `wasmtime_wasi::p3::add_to_linker` compila |
| 3 | `ComponentRuntime::run` real con `wasi:cli/run` | `wf-runtime/src/lib.rs` | Componente "hello world" WASI 0.3 corre |
| 4 | `wf component build` end-to-end (Rust → component) | `wf-cli/src/component.rs`, `wf-toolchains/component_toolchain.rs` | `wf component build` produce `.wasm` válido |
| 5 | Tests Component Model: imports/exports/resources/async | `tests/component_*.rs` | Suite pasa |

### FASE B: Supply Chain Mínima (Bloqueante v0.1.0)

| Semana | Tarea | Archivos | Criterio Done |
|--------|-------|----------|---------------|
| 1 | Lockfile struct + serialización | `wf-core/src/manifest.rs`, `wf-toolchains/cache.rs` | `wf build --lock` genera `wasmfoundry.lock` |
| 2 | SBOM generation en build | `wf-cli/src/build.rs`, `wasm-tools metadata` | `wf build --sbom` produce `sbom.spdx.json` |
| 3 | `wf verify <artifact>` — hash, wasmparser, SBOM | `wf-cli/src/verify.rs` (nuevo) | Verifica artefacto local |
| 4 | Publicación OCI (`oras`/`wasm-pkg-tools`) | `wf-cli/src/publish.rs` (nuevo) | `wf publish` sube a ghcr.io |

### FASE C: Observabilidad + Debugging (DX Crítico)

| Semana | Tarea | Archivos | Criterio Done |
|--------|-------|----------|---------------|
| 1 | `wf run --coredump-on-trap` + `wf coredump analyze` | `wf-runtime`, `wf-cli/coredump.rs` | Trap produce coredump legible |
| 2 | Host OTel spans en `console` + host functions | `wf-runtime/src/host.rs` | Traces en Jaeger/OTel collector |
| 3 | `wasi:logging` host function → OTel | `wf-runtime/src/host.rs` + `wit` | Guest `log.info("hi")` aparece en collector |
| 4 | `wf inspect --format json` + DWARF (opcional) | `wf-cli/src/inspect.rs`, `gimli`/`wasm-tools addr2line` | JSON incluye source locations si DWARF presente |

### FASE D: Optimización + Benchmarks (Calidad)

| Semana | Tarea | Archivos | Criterio Done |
|--------|-------|----------|---------------|
| 1 | `WasmOptimizer` wrapper wasm-opt | `wf-toolchains/src/toolchains/optimizer.rs` | `wf build --release --opt-profile edge` reduce tamaño |
| 2 | `wf bench` harness + workloads built-in | `wf-cli/src/bench.rs` (nuevo), `fixtures/benchmarks/` | `wf bench --workload http-server` produce JSON |
| 3 | CI matrix benchmarks (GitHub Actions) | `.github/workflows/bench.yml` | Benchmarks corren en PR, comparan vs main |

### FASE E: Capability Check + Compatibility Matrix (Operabilidad)

| Semana | Tarea | Archivos | Criterio Done |
|--------|-------|----------|---------------|
| 1 | `runtime_capabilities.toml` (wasmtime/wasmer/wasmedge × WASI versions) | `docs/compatibility-matrix.md` + data file | Documentado |
| 2 | `wf deploy --check-capabilities` | `wf-cli/src/deploy.rs` (nuevo) | Pre-deploy reporta gaps |
| 3 | `wf test --runtimes ...` via containers | `wf-cli/src/test.rs` (nuevo), testcontainers | Matrix testing funcional |

---

## 7. Cambios Arquitectónicos Requeridos

### 7.1 `wf-runtime`: WASI 0.3 / Component Model

```diff
// Cargo.toml
-wasmtime-wasi = "49.0.2"
+wasmtime-wasi = { version = "49", features = ["p2", "p3"] }

// policy.rs
+use wasmtime_wasi::p3::{WasiCtx, WasiView, IoView, FsView, ...};
+
+struct HostState {
+    // ...
+    wasi_ctx: WasiCtx,
+}
+
+impl WasiView for HostState {
+    fn ctx(&mut self) -> &mut WasiCtx { &mut self.wasi_ctx }
+    fn table(&mut self) -> &mut ResourceTable { ... }
+}
+
+fn engine_for(policy: &RuntimePolicy) -> Engine {
+    let mut config = Config::new();
+    config.wasm_component_model_async(true);  // WASI 0.3 async
+    config.wasm_memory64(true);
+    // ...
+}
```

### 7.2 `wf-core`: Lockfile + SBOM Types

```rust
// manifest.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    pub modules: Vec<LockedModule>,
    pub wit_deps: Vec<WitDep>,
    pub policy_hash: String,  // hash of RuntimePolicy for repro
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedModule {
    pub name: String,
    pub toolchain: ToolchainId,
    pub hash: String,  // BLAKE3 of artifact
    pub deps: Vec<String>,  // module names
    pub wit_interface_hash: Option<String>,  // hash of .wit if component
}
```

### 7.3 `wf-toolchains`: Optimizer + WIT Fetch

```rust
// toolchains/optimizer.rs
pub enum OptProfile { Size, Speed, Balanced, Edge }
pub enum TargetRuntime { Wasmtime, Wasmer, WasmEdge }

pub struct WasmOptimizer {
    profile: OptProfile,
    target: TargetRuntime,
}

impl WasmOptimizer {
    pub fn optimize(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut args = vec!["-O"];
        match self.profile {
            OptProfile::Size => args.extend(["--optimize-size", "--strip-debug"]),
            OptProfile::Edge => args.extend(["--optimize-size", "--strip-debug", "--converge"]),
            // ...
        }
        // invoke wasm-opt via wasm_tools crate or command
    }
}
```

### 7.4 `wf-cli`: Nuevos Subcomandos

| Comando | Propósito | Fase |
|---------|-----------|------|
| `wf verify <artifact>` | Valida hash, wasmparser, SBOM, firma | B |
| `wf publish [--registry]` | Publica componente + lockfile + SBOM a OCI | B |
| `wf bench [--workload] [--runtimes]` | Benchmarks reproducibles | D |
| `wf deploy --check-capabilities` | Pre-deploy capability check | E |
| `wf test --runtimes wasmtime@50,wasmer@7` | Matrix testing cross-runtime | E |
| `wf coredump analyze <file>` | Análisis post-mortem | C |
| `wf component add <wit-pkg>@<ver>` | Fetch WIT + generate bindings | 3.4 |

---

## 8. Tests Requeridos (Nuevos + Extensiones)

| Test | Ubicación | Qué Valida |
|------|-----------|------------|
| Component Model: `wasi:cli/run` async | `tests/component_wasi_p3.rs` | WASI 0.3 funcional |
| Component Model: resources (borrow/own) | `tests/component_resources.rs` | Resource handling |
| Lockfile: reproducible build | `tests/lockfile_repro.rs` | Mismo lockfile → mismos artifacts |
| SBOM: contiene producer, deps, hashes | `tests/sbom_generation.rs` | SPDX válido |
| Verify: rechaza artifact corrupto | `tests/verify_corrupt.rs` | Exit code 1 + diagnostic |
| Optimizer: size reduction measurado | `tests/optimizer_size.rs` | `--opt-profile size` < original |
| Benchmarks: metodología libsodium | `tests/bench_methodology.rs` | Freq fija, warmup, geo mean |
| Capability check: WASI 0.2 vs 0.3 | `tests/capability_check.rs` | Reporta gaps correctamente |

---

## 9. Documentación a Actualizar/Crear

| Documento | Estado | Acción |
|-----------|--------|--------|
| `docs/compatibility-matrix.md` | ❌ No existe | **Crear** — matriz runtime × WASI × proposals |
| `docs/adr/0002-wasi-03-adoption.md` | ❌ No existe | **Crear** — decisión adopción WASI 0.3 |
| `docs/adr/0003-lockfile-format.md` | ❌ No existe | **Crear** — formato lockfile v1 |
| `docs/adr/0004-sbom-format.md` | ❌ No existe | **Crear** — SPDX vs CycloneDX decision |
| `docs/abi/host-abi-v2.md` | ❌ No existe | **Crear** — cuando ABI v1 cambie (WASI 0.3) |
| `docs/bundling/bundle-format-v1.md` | 🟡 Existe (plan) | **Completar** — footer spec, metadata schema |
| `docs/product.md` | ✅ Existe | **Actualizar** — reflejar Component Model, WASI 0.3 |
| `docs/parity-matrix.md` | ✅ Existe | **Actualizar** — marcar Component Model, WASI 0.3, Supply chain |
| `README.md` | ✅ Existe | **Actualizar** — status table, nuevos comandos |

---

## 10. Riesgos y Decisiones Pendientes

| Riesgo | Impacto | Mitigación |
|--------|---------|------------|
| **WASI 0.3 inestable en Wasmtime** | ComponentRuntime puede romper entre versiones Wasmtime | Pinnear Wasmtime version en `Cargo.lock`; test matrix con Wasmtime beta |
| **wasm-pkg-tools / OCI registry inmaduro** | Publicación OCI puede no funcionar reliably | Implementar lockfile/SBOM local primero; publicación OCI opcional tras v0.1.0 |
| **DWARF debugging Tier 3** | `wf inspect --dwarf` puede ser unreliable | Hacer opcional; documentar limitación; fallback a offsets |
| **wit-bindgen C++/AS no maduro** | Bindings multi-lang incompletos | Rust primero; C++/AS cuando `wit-bindgen` los soporte estable |
| **Benchmark suite sin estándar industria** | `wf bench` resultados no comparables externamente | Publicar metodología; contribuir a `wasm-bench` community effort |

---

## 11. Resumen Ejecutivo: Qué Hace Falta para `v0.1.0`

**Según `plan.md` #135, `v0.1.0` requiere:**
- ✅ `wf inspect` / `wf run` / `wf build` / `wf init`
- ✅ Rust toolchain funcional
- ✅ Precompiled toolchain funcional
- ✅ Core module runtime
- ✅ Tests + documentación coherente

**PERO el ecosistema 2026 exige MÁS para ser usable en producción:**

| Faltante Crítico | ¿Bloquea v0.1.0? | Acción |
|------------------|------------------|--------|
| **WASI 0.3 / Component Model** | Sí (ecosistema ya lo usa) | **FASE A** — 5 semanas |
| **Lockfile + SBOM + Verify** | Sí (supply chain básico) | **FASE B** — 4 semanas |
| **CI/CD** | Sí (plan #121 dice "al final") | **Paralelo** — 2 semanas |
| **Debugging básico (coredump)** | No (pero DX crítico) | **FASE C** — 3 semanas |

**Recomendación:** Extender definición de `v0.1.0` para incluir **Component Model + WASI 0.3 + Lockfile/SBOM**. Sin ellos, WasmFoundry nace obsoleto frente al ecosistema Oct 2026.

---

## 12. Próximos Pasos Inmediatos (Esta Semana)

1. **Actualizar dependencias Wasmtime/WASI** — `Cargo.toml` features `p2`, `p3`
2. **Crear `docs/compatibility-matrix.md`** — basado en tabla sección 4 de este doc
3. **ADR-0002: WASI 0.3 Adoption** — documentar decisión y migración preview1→p3
4. **Spike: `WasiView` implementation** — 1 día para validar factibilidad
5. **Crear issues GitHub** para cada fase (A–E) con checkboxes

---

*Análisis generado comparando `Estado_Wasm_Oct_2026_Version_Consolidada.md` contra código fuente WasmFoundry (crates wf-core, wf-wasm, wf-runtime, wf-toolchains, wf-cli, wf-bundle), `plan.md`, `docs/architecture.md`, `docs/parity-matrix.md`, `docs/gaps/`.*