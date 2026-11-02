# Estado WebAssembly Octubre 2026 — Versión Consolidada

**Fecha de corte:** 24 de octubre de 2026
**Fuentes:** Investigación primaria (fetch GitHub, wasmtime.dev, wasi.dev, component-model.bytecodealliance.org, WebAssembly org) + Informe Perplexity Deep Research (oct 2026)
**Propósito:** Referencia única autoritativa para decisiones técnicas en WasmFoundry

---

## 1. Resumen Ejecutivo

WebAssembly completó en **2025–2026 su transición a estándar maduro en el núcleo**:

- **Wasm 3.0** — Estándar "vivo" desde **17 sep 2025** (GC, memory64, exceptions, tail-call, SIMD relajado, multi-memory, typed references, custom annotations, JS String builtins)
- **WASI 0.3** — Ratificado **11 jun 2026** con async nativo (`async func`, `stream<T>`, `future<T>`, nuevo Canonical ABI)
- **Wasmtime v50.0.0** — **20 oct 2026**, Core Project CNCF, runtime de referencia

> **Conclusión central:** La madurez está **distribuida de forma muy desigual** según capa y caso de uso. No es "maduro" o "inmaduro" globalmente.

---

## 2. Madurez por Capas (Semáforo)

### 🟢 **Verde — Producción Probada**

| Capa | Estado | Evidencia |
|------|--------|-----------|
| **Core Wasm 3.0** | Tier 1 | Pasa testsuite oficial; fuzzing 24/7 OSS-Fuzz; Wasmtime/Wasmer/WasmEdge/wazero/wasmi conformes |
| **WASI 0.2** (ene 2024) | Estable | `wasi:io`, `filesystem`, `clocks`, `random`, `sockets`, `http` — Wasmtime + jco superan suite oficial |
| **Component Model MVP** | Estable | WIT, Canonical ABI, linking, resources — base de WASI 0.2/0.3 |
| **Wasmtime x86_64** | Tier 1 | Linux/macOS/Windows; Cranelift + Winch; fuzzing continuo |
| **Edge/Serverless** | Producción | Cloudflare Workers, Fastly Compute, Akamai (post-Fermyon) |
| **Plugins sandbox** | Producción | Shopify Functions (Rust/Zig/TinyGo), Extism, Cosmonic Control |
| **Ports navegador** | Producción | Figma (3x carga), Google Sheets (2x cálculo WasmGC), Photoshop Web |
| **AI Agents / MCP** | Emergente 2025-26 | Wasm como sandbox para herramientas de agentes (Cosmonic, GetCoreTech) |
| **wasm-opt / Binaryen** | Maduro | Usado en prod por Emscripten, wasm-pack, toolchains Rust/Go/TinyGo |

### 🟡 **Amarillo — En Transición (Funcional con Fricción)**

| Capa | Estado Real | Gaps Críticos |
|------|-------------|---------------|
| **WASI 0.3** (jun 2026) | **Solo Wasmtime experimental** (v43+ snapshots); docs.rs: "unstable, incomplete" | Resto runtimes: case-by-case; 3 generaciones WASI conviven (0.1/0.2/0.3) |
| **Package Management** | **Warg archivado 28 jul 2025** → `wasm-pkg-tools` sobre OCI (WIP) | Sin registry dominante con firmas/SBOM/provenance tipo npm/cargo |
| **Debugging** | DWARF existe pero **Tier 3 Wasmtime** ("no active maintainer", known bugs) | Fragmentado: Chrome DevTools ✅, lldb/gdb Wasmtime ✅, unificación ❌ |
| **Observabilidad** | Piezas: Dylibso Observe SDK, Fermyon OTel, Chrome — **sin integración estándar** | Traces host↔componente, símbolos en prod, métricas internas requieren instrumentación |
| **ARM64 / RISC-V / s390x** | Tier 2 Wasmtime (sin fuzzing continuo); Winch solo x86_64/aarch64 core | No Tier 1 en arquitecturas no-x86 |
| **JVM Languages** | Tooling **pre-Component Model o beta** (Java/Kotlin/Scala) | Blockers para backends Spring/Quarkus |
| **wasmCloud** | CNCF Incubating v2.11 (oct 2026) | **LFX "Concerning": -25% contribuidores, -38% orgs último año** (métrica OSS, no prod) |
| **Docker + Wasm** | **Deprecated, beta, sin mantenimiento** (Docker Docs) | No GA; NGINX Unit archivado oct 2025 |

### 🔴 **Rojo — Inmaduro / Sin Resolver**

| Capa | Problema |
|------|----------|
| **Threads WASI** | **No existe API portable de hilos**; WASI 0.2 solo streaming HTTP concurrente |
| **Benchmarks reproducibles** | Solo estudios ad-hoc (libsodium 2026 metodológico en 00f.net); no suite estándar |
| **Certificación cross-runtime** | No existe; matrices declaradas por vendors, no empíricas |
| **SDK unificado multi-lang** | `wit-bindgen` por lenguaje; sin CLI único, sin registry WIT versionado |
| **Backend transaccional general** | Sin evidencia producción a escala; idoneidad depende de APIs/concurrencia/herramientas |
| **Web general** | **0.35% sitios desktop** (HTTP Archive 2025, estable 2 años); 2% top 1k; DOM via JS |

---

## 3. Proyectos Verificados en Producción (Oct 2026)

| Proyecto | Uso Wasm | Runtime | Evidencia |
|----------|----------|---------|-----------|
| **Google Sheets** | Motor cálculo Java→WasmGC; ~2x vs JS final; **regex 100x más lento nativo** | WasmGC (Chrome) | [web.dev case study](https://web.dev/case-studies/google-sheets-wasmgc) |
| **Figma** | Render C++→Wasm; migración histórica 3x carga | Wasm (navegador) | [Figma Blog](https://www.figma.com/blog/webassembly-cut-figmas-load-time-by-3x/) |
| **Adobe Photoshop Web** | Render complejo navegador | Wasm | DevNewsletter 2026 |
| **Google Maps/Earth/Meet/Photos** | Cómputo pesado | Wasm | DevNewsletter 2026 |
| **Cloudflare Workers** | Módulos Wasm (Rust/C++) 330+ ubicaciones; Workers AI (Llama-3-8b) | V8 + Wasm | Cloudflare Docs |
| **Fastly Compute** | Cliente→Wasm edge; instanciación μs; Viceroy local | Wasmtime (Lucet legacy) | Fastly Docs |
| **Shopify Functions** | Terceros (Rust/Zig/TinyGo) checkout crítico; límites tamaño/instrucciones; **Scripts deprecated 30 jun 2026** | Wasmtime | Shopify Dev, Dylibso blog |
| **Akamai** | Adquirió **Fermyon 1 dic 2025** → pivot **Cosmonic Control** (plano control enterprise sandboxes Wasm) | Spin/Wasmtime | Akamai Press, Cosmonic Blog |
| **wasmCloud** | Plataforma políglota; CNCF Incubating 8 nov 2024; v2.11 límites memoria/sockets por defecto | Wasmtime + wash-runtime | CNCF, GitHub Releases |
| **Extism (Dylibso)** | Framework plugins universal: cualquier lang → Wasm plugin | Wasmtime | Extism.org |
| **WasmEdge** | WASI-NN + GGML/llama.cpp para LLM inference (delega backends nativos) | WasmEdge | WasmEdge Docs |
| **Cosmonic** | Pivot a "Cosmonic Control" — enterprise control plane dense Wasm sandboxes | wasmCloud | Cosmonic Blog |
| **PHP-WASM / WordPress** | PHP runtime → Wasm para serverless | Wasmer/Wasmtime | DevClass 2026 |

**Patrones de adopción emergentes:**
1. **Cómputo pesado → navegador** (C++/Java reutilizado, híbrido Wasm + APIs nativas)
2. **Código terceros sin confianza** (sandbox capability-based decisivo)
3. **Edge/Serverless** (cold start ~0.5ms claim Fermyon SUSECON 2025 — *afirmación vendor*)
4. **Agentes IA 2025-26** — Wasm sandbox para MCP tools / código generado

---

## 4. Runtimes: Matriz de Soporte (Oct 2026)

### Wasmtime v50.0.0 (Referencia)
- **Tier 1:** x86_64 Linux/macOS/Win; Cranelift + Winch; fuzzing continuo
- **Tier 2:** aarch64 Linux/macOS, s390x, x86_64 Windows GNU (falta fuzzing continuo)
- **Tier 3:** iOS, Android, Windows ARM64, RISC-V, PowerPC, FreeBSD, musl, no_std
- **Propuestas Tier 1:** GC, memory64, exceptions, tail-call, SIMD, component-model, component-model-async, WASI 0.2/0.3
- **Experimental:** WASI 0.3 (v43+), stack-switching (x86_64 Linux only), compact imports (opt-in)
- **No implementado:** flexible-vectors, memory-control, shared-everything-threads

### Wasmer 7.1.0
- Enfoque: sandboxes ligeros, registry WAPM, SDK multi-lang (JS/TS, Python, Swift, Rust)
- Component Model: en progreso; WASI 0.2: parcial
- Usado: Vercel Edge, Wasmer Cloud

### WasmEdge 0.17.0
- Enfoque: edge, Kubernetes (Docker+Wasm), AI (WASI-NN), automotive
- WASI 0.2: en desarrollo; Component Model: experimental
- CNCF Sandbox

### wazero 1.12.0
- Pure Go, zero deps; WASI 0.1 only; sin Component Model
- Embeddable en Go apps

### WAMR 2.4.4
- Intel; enfoque IoT/embedded/edge; AOT + JIT; WASI 0.1/0.2 variable

### wasmi
- Rust puro; core Wasm + WASI 0.1 básico; embedded

### Navegadores
- **Chrome:** más completo (GC, EH, JS String builtins, memory64 16GB limit)
- **Safari:** más rezagado; añadió 2025: nuevas exceptions, JS String builtins, intérprete Wasm sin JIT
- **Firefox:** intermedio

---

## 5. Benchmarks de Referencia (Junio 2026 — 00f.net)

**Metodología:** libsodium (carga criptográfica), x86-64 freq fijada, media geométrica slowdown vs nativo, excluye outliers

| Runtime | Modo | Slowdown vs Nativo | Notas |
|---------|------|-------------------|-------|
| **Wasmtime 46** | JIT | **2.41x** | Mejora año a año 2024→2026 |
| **WAMR** | AOT | **1.57x** | Mejor AOT |
| **Node 26.3.1** | JIT | 7.95x | V8 Wasm |
| **Bun 1.3.14** | JIT | ~8x | Similar Node |
| **Wasmer 7.1** | JIT | ~3-4x | Estimado |
| **WasmEdge 0.17** | AOT/JIT | ~2-3x | Variable |

> **Lección:** Variabilidad enorme entre runtimes y modos (JIT vs AOT) para la **misma carga**. No hay "ranking universal" — resultados válidos solo para carga/arch/modo/config probados.

---

## 6. Las 8 Brechas: Estado Real + Especificación Funcional

### 6.1 Compatibilidad y Certificación Automática

**Estado:** 3 generaciones WASI conviven; propuestas activadas distinto por runtime/versión (ej. compact imports opt-in Wasmtime 50); suites oficiales WASI 0.2 pasadas por Wasmtime/jco; WAMR/WasmEdge variables.

**Funcionalidades Requeridas:**
- Analizador estático: imports/exports, contratos WIT, mundo WASI objetivo, propuestas requeridas
- Matriz compatibilidad empírica: runtime × versión × plataforma × arquitectura (Wasmtime, Wasmer, WasmEdge, WAMR, wazero, Node, Deno, Bun, navegadores)
- Suite conformidad 2 niveles: (a) estructural/tipos desde WIT (auto-generada); (b) semántica/negocio (casos propietario)
- Detección diferencias semánticas: códigos error, traps, límites default, encoding strings, orden init
- Certificados vinculados al hash del artefacto (firmados, máquina-legibles)
- Gate CI: fallo si runtime objetivo no soporta perfil requerido
- Detección regresiones: re-ejecución programada en nuevas versiones runtimes

---

### 6.2 SDK Común de Componentes y Bindings

**Estado:** cargo-component (Rust), jco (JS/TS), componentize-py (Python), wit-bindgen (C/C++/Go/Zig/Java/C#/MoonBit) — **fragmentados**; JVM pre-Component Model/beta.

**Funcionalidades Requeridas:**
- Contrato único: WIT como fuente verdad + versionado semántico interfaces + deprecaciones
- Generación bindings multi-lang: un CLI (`wit-sdk generate --lang rust|go|ts|py|cs|zig|java|kotlin`)
- Gestión errores/recursos estandarizada: mapeo `result`/errores WIT → exceptions/errors idiomáticos
- Composición asistida: resolución deps, cableado imports/exports, adaptadores WASI 0.1→0.2→0.3
- Harness pruebas interoperabilidad: instanciación + validación contrato en cada lenguaje
- Diagnósticos incompatibilidad: mensajes actionable (qué interfaz/versión no coincide, cómo corregir)

---

### 6.3 Depuración y Observabilidad Integrada

**Estado:** Fragmentado. Chrome DevTools (DWARF C/C++ ✅), Wasmtime (lldb/gdb + core dumps ✅), **Tier 3 Wasmtime** (sin maintainer activo, bugs conocidos). Retos estructurales: tamaño DWARF, unwinding no estandarizado, traces producción con símbolos difícil.

**Funcionalidades Requeridas:**
- Registro artefactos debug: extracción DWARF a archivos separados + Build/Debug IDs + "debug info registry" (estilo Sentry) + descarte rutas compilación sensibles
- Simbolización traps: traps → trazas función/archivo/línea (cuando símbolos + soporte runtime); salida degradada (offsets + módulo) cuando faltan; core dumps Wasmtime como evidencia
- Puente debug interactivo: lldb/gdb (runtimes) + Chrome DevTools (navegador) bajo mismo flujo; documentar diferencias JIT/AOT/navegador
- Instrumentación host: spans/métricas OTel + límites/fuel/recursos observables — limitado a lo que host mide sin tocar módulo
- Traces distribuidas cross-frontera: correlación host↔componente, atribución CPU/mem por componente donde runtime permita
- Perfiles reproducibles: heatmaps instrucciones (patrón Shopify `--profile`), grabación entradas para reproducir fallos
- Verificación rendimiento prod: comparación métricas entre despliegues para detectar regresiones

---

### 6.4 Gestor de Paquetes y Dependencias

**Estado:** **Warg archivado 28 jul 2025** → `wasm-pkg-tools` sobre OCI (WIP). wash (wasmCloud) usa OCI registry (ghcr.io). cargo para Rust. npm para JS. **Sin ecosistema unificado** con firmas/provenance/SBOM/avisos.

**Funcionalidades Requeridas:**
- Resolución deps componentes: rangos versiones, resolución determinista, lockfiles con hashes (`wasm-lock.json`)
- Publicación OCI vía wasm-pkg-tools: registros privados, caché local, mirror CI air-gapped
- Integridad/confianza: hashes, firmas (cosign), provenance (SLSA), SBOM desde **proceso build** (no solo binario)
- Seguridad supply chain: avisos vulnerabilidades (OSV/Grype), escaneo lockfiles, políticas licencia
- Gestión ciclo vida: promoción dev/staging/prod, reversión, desprecación
- Distinción artefactos: fuente, módulo core, componente — paquetes diferenciados con relaciones explícitas

---

### 6.5 APIs WASI Portables entre Runtimes

**Estado:** Subconjuntos distintos por runtime. WASI 0.2: Wasmtime/jco ✅; WAMR/WasmEdge variables. WASI 0.3: **experimental Wasmtime only**. `wasi:http`, `keyvalue`, `blobstore`, `config`, `logging`, `nn`, `crypto`, `threads` — no estables / solo wasmCloud (host plugins).

**Funcionalidades Requeridas:**
- Perfiles declarativos capacidades: descripción formal necesidades componente (http, cli, sockets, fs, clocks, random) + oferta host
- Chequeo previo despliegue: análisis componente vs host objetivo → informe carencias **antes de ejecutar**
- Adaptadores/virtualización: shims wasip1 legacy; virtualización interfaces para hosts sin soporte completo (patrón `wasi-virt`)
- Pruebas semánticas APIs: validación comportamiento (no solo tipos) cada interfaz WASI en cada runtime → divergencias reales
- Errores explicativos: qué falta, en qué host, mitigación posible
- Concesión declarativa: config host define capacidades concedidas por componente

---

### 6.6 Seguridad y Límites de Recursos

**Estado:** Sandbox sólido por diseño (deny-by-default). Wasmtime: guard regions 2GB, zeroing memory, Spectre mitigations (bounds checks, br_table), CFI (PAuth/BTI ARM), fuel/epochs, limiter (mem, instancias, tables, stack). wasmCloud v2.11: políticas OPA + límites default. **CVEs existen** (ej. GHSA-vqjp-4c8c-hfgg escape sandbox fs Wasmtime).

**Funcionalidades Requeridas:**
- Modelo amenazas explícito por despliegue: plantillas (qué se ejecuta, quién publica, contra qué protege)
- Políticas permisos por componente: concesión mínima capacidades (red, fs, env, clock) aplicada en despliegue (estilo OPA wasmCloud)
- Límites recursos integrales: fuel/instrucciones, epochs, memoria max, CPU time, concurrencia — **aplicados a módulo + trabajo host en nombre del módulo**
- Cancelación segura: interrupción timeout → trap limpio + liberación recursos; presupuesto/tolerancia definidos por despliegue
- Aislamiento entre cargas: memoria a cero entre instancias, instancia por petición, separación stores
- Auditoría/respuesta: registro violaciones política, alertas CVEs runtimes con ventana actualización, battery pruebas adversarias (fuzzing módulos hostiles con presupuesto recursos)
- Actualización runtime: flujo gestionado upgrades + verificación compatibilidad (conexión módulo 6.1)

---

### 6.7 Optimización Automática

**Estado:** Binaryen/wasm-opt (infra estándar), wasmtime compile (AOT .cwasm), wasmer compile, WasmEdge AOT. **Gaps:** sin optimizador cross-runtime unificado, sin PGO, streaming instantiation no estándar, module splitting tooling limitado, sin size budgets CI estándar.

**Funcionalidades Requeridas:**
- Análisis artefacto: desglose tamaño por sección/función, árbol deps componentes, detección dead code + DWARF embebido
- Pipeline transformación por objetivo: perfiles **browser** (min download), **edge** (min cold start), **server** (max throughput), **embedded** (min mem); wasm-opt flags adecuados + extracción DWARF + tree-shaking interfaces no usadas + virtualización WASI
- Carga diferida: particionado módulo (patrón wasm-split) para cargar solo lo necesario al inicio
- Precompilación + caché: AOT + caché artefactos nativos por runtime/versión/plataforma
- Validación antes/después: ejecución automática suite tests componente pre/post cada transformación; veredicto "sin regresiones detectadas por suite"
- Presupuestos CI: gates declarables ("size < X MB, cold start < Y ms") → fallan build si incumplen; detección regresiones rendimiento entre despliegues

---

### 6.8 Benchmarks Reproducibles

**Estado:** No suite estándar. Estudios: libsodium 2026 (00f.net, metodología rigurosa), académicos (opencloudification.com). Resultados no comparables.

**Funcionalidades Requeridas:**
- Manifiesto entorno: versiones exactas runtimes, OS, CPU (freq + throttling documentado), flags compilación — todo en archivo versionado
- Suite cargas representativas: cómputo puro, HTTP in/out, cold start, composición componentes, presión GC, E/S archivos
- Protocolo estadístico: warmup, repeticiones, mediana + media geométrica, corrección resultados, exclusión documentada fallos/tiempos cero (lección libsodium)
- Separación métricas: compile time, cold start, execution, peak mem, instrucciones ejecutadas — medidas/reportadas por separado
- Resultados brutos exportables: formato abierto para meta-análisis + manifiestos entorno
- Runner reproducible: contenedores versiones fijadas, hardware documentado, comparación histórica mismo runtime entre versiones (patrón libsodium 2024-2026)

---

## 7. Arquitectura Común: 3 Artefactos Centrales

Los 8 módulos comparten tres artefactos que son la **columna vertebral**:

```
┌─────────────────────────────────────────────────────────────────┐
│  1. IDENTIDAD DEL ARTEFACTO (hash binario/componente)           │
│     → Ancla: certificaciones (6.1), símbolos (6.3), paquetes    │
│        (6.4), benchmarks (6.8)                                   │
│     → Toda transformación cambia hash → renueva evidencias      │
├─────────────────────────────────────────────────────────────────┤
│  2. CONTRATO WIT (fuente de verdad)                             │
│     → Bindings (6.2), compatibilidad (6.1), perfiles            │
│        capacidades (6.5)                                         │
├─────────────────────────────────────────────────────────────────┤
│  3. MANIFIESTO DE DESPLIEGUE                                    │
│     → Declara: objetivos, límites, permisos, presupuestos       │
│     → Consumen: seguridad (6.6), optimización (6.7)             │
└─────────────────────────────────────────────────────────────────┘
```

### Flujo Extremo a Extremo (Orden Crítico)

```
CONSTRUCCIÓN (6.2) 
    → OPTIMIZACIÓN (6.7)           [transforma artefacto → cambia hash]
    → VALIDACIÓN + CERTIFICACIÓN (6.1, 6.5)   [sobre artefacto FINAL]
    → GENERACIÓN SÍMBOLOS + METADATOS (6.3, 6.4)
    → FIRMA + PUBLICACIÓN (6.4)
    → DESPLIEGUE CON POLÍTICAS + LÍMITES (6.6)
    → OPERACIÓN: OBSERVABILIDAD (6.3) + BENCHMARKS CONTINUOS (6.8)
```

> **Regla de oro:** Si el artefacto se transforma después de la certificación, **todas las evidencias vinculadas al hash deben regenerarse**.

---

## 8. Priorización MVP (Fases)

| Fase | Módulos | Rationale | Piezas Reutilizables |
|------|---------|-----------|---------------------|
| **Fase 1** (Valor inmediato) | **6.1 Certificación** + **6.4 Packaging OCI** | Fricción máxima hoy; piezas más maduras | Suites WASI oficiales, wasm-pkg-tools, OCI registry |
| **Fase 2** | **6.5 Capabilities check** + **6.6 Security policies** | Comparten manifiesto despliegue; base operativa | WIT worlds, wasi-virt, OPA wasmCloud |
| **Fase 3** | **6.2 SDK Unificado** + **6.3 Observabilidad** | Mayor inversión; coordinación toolchains lang | wit-bindgen, Dylibso/Fermyon/Chrome pieces |
| **Fase 4** | **6.7 Optimización** + **6.8 Benchmarks** | Se benefician artefactos/métricas fases anteriores | wasm-opt, Binaryen, libsodium methodology |

---

## 9. Proyectos Faltantes en la Industria (Síntesis)

### Prioridad Crítica (Bloquean adopción enterprise)

| Proyecto | Descripción |
|----------|-------------|
| **`wit-registry`** | Registry federado OCI-native para interfaces WIT versionadas + `wit-sdk` CLI unificado |
| **`wasm-pkg` / `wasm-registry`** | Package manager universal: lockfile, SBOM, signing, policy engine, security scanning, OCI |
| **`wasi-conformance`** | Suite oficial cross-runtime conformance WASI (functional + behavioral) |
| **`wasm-conformance`** | Suite conformance cross-runtime core Wasm + Component Model (behavioral) |

### Prioridad Alta (DX + Operabilidad)

| Proyecto | Descripción |
|----------|-------------|
| **`wasm-debug-adapter`** | DAP implementation cross-runtime + VS Code extension |
| **`wasm-profiler`** | Profiler unificado (perf/speedscope/pprof) + source mapping DWARF |
| **`wasi-telemetry`** | Propuesta WASI + impl. referencia logs/metrics/traces OTLP |
| **`wasm-bench`** | Suite estándar + harness + reporter + CI integration (cross-runtime, cross-arch) |

### Prioridad Media (Eficiencia + Seguridad)

| Proyecto | Descripción |
|----------|-------------|
| **`wasm-opt-unified`** | Frontend único wasm-opt/binaryen/wasm-tools con perfiles por target runtime |
| **`wasm-pgo`** | Infrastructure Profile-Guided Optimization Wasm (instrumentación runtime-agnóstica) |
| **`wasm-hardener`** | Post-procesador hardening automático binarios Wasm |
| **`wasi-capability-broker`** | Host plugin genérico capacidades WASI emergentes sobre backends configurables |
| **`wasm-attest`** | Attestation/verificación con políticas Rego/Cel |
| **`wasm-size-analyzer`** | Análisis tamaño actionable (tipo cargo-bloat/twiggy cross-language) |

### Prioridad Baja (Nice to Have)

| Proyecto | Descripción |
|----------|-------------|
| **`wit-lsp`** | Language Server Protocol para WIT |
| **`wasm-policy-engine`** | Admission policies componentes (Rego/Cel) |
| **`wasm-runtime-cert`** | Programa certificación runtimes (tipo FIPS para sandboxing) |
| **`wasm-inspect`** | Introspección en vivo instancias running |

---

## 10. Acelerador Pendiente de la Especificación

> **Threads / Multihilo en WASI** — Condiciona buena parte del segmento backends transaccionales. Sin API portable, JVM languages bloqueados, pools de conexiones y paralelismo interno no portable. [GetCoreTech 2026](https://www.getcoretech.net/articles/webassembly-2026-is-it-living-up-to-the-hype)

---

## 11. Incertidumbres y Validaciones Pendientes

| Tema | Incertidumbre | Validación Necesaria |
|------|---------------|---------------------|
| **Soporte real WASI 0.3** | Ratificación precede a soporte | Verificar runtime x runtime qué interfaces implementadas |
| **Encuestas State of Wasm** | Edición 2023 (303 users), sesgo vendors (55%) | No generalizar; usar solo como antecedente histórico |
| **LFX wasmCloud** | Mide actividad OSS, no adopción prod | Separar salud comunidad vs adopción producción |
| **Claims vendors** | 0.5ms cold start (Fermyon), 42k workloads/core (wasmCloud) | Verificación independiente requerida |
| **Evolución 2026-27** | Roadmap WASI bimestral 0.3.x + trabajo 0.4; decisiones Akamai/Spin | Monitorear wasi.dev/roadmap + Akamai blog |

---

## 12. Implicaciones Directas para WasmFoundry

### Arquitectura Objetivo (wf-core → wf-wasm → wf-runtime → wf-cli)

| Componente | Decisión | Justificación |
|------------|----------|---------------|
| **wf-wasm** | Basar en `wasm-tools` + `wasmparser` + `wit-parser` | No reinventar parsing/validación/optimización; Bytecode Alliance es referencia |
| **wf-runtime** | Embedding `wasmtime` (Tier 1, API Rust nativa) | Wasmtime = runtime referencia; Cranelift + Winch; fuzzing continuo |
| **wf-cli** | Orquestador: `wasm-tools` + `wasmtime` + `wit-bindgen` + `wasm-pkg-tools` | CLI único que compone herramientas existentes |
| **wf-core** | Zero-deps; tipos dominio, errores, WIT AST simplificado | Base para todos; sin wasmtime/wasmparser |

### Features Concretas por Implementar

| Feature | Comando | Implementación |
|---------|---------|----------------|
| **Build optimizado** | `wf build --release --target-runtime wasmtime\|wasmer\|wasmedge` | `wasm-opt` con perfiles por target |
| **Component deps** | `wf component add wasi:filesystem@0.2` | Client `wasm-pkg-tools`/OCI + lockfile WIT (`wasm-lock.json`) |
| **Matrix testing** | `wf test --runtimes wasmtime@50,wasmer@7,wasmedge@0.17` | Contenedores con runtimes fijados; comparar stdout/stderr/exit/mem/time |
| **Benchmarks** | `wf bench --workload http-server` | Metodología libsodium: freq fija, warmup, media geométrica, outlier exclusion |
| **SBOM + Provenance** | `wf build --sbom --sign` | `wasm-tools metadata add --producer "wf/x.y.z" --sbom` + cosign |
| **Capability check** | `wf deploy --check-capabilities` | Análisis WIT imports vs host profile (wasi-virt pattern) |
| **Security policy** | `wf run --policy policy.rego` | OPA/Rego policies: memory, fuel, net, fs, caps |

### Documentación a Mantener

- `docs/compatibility-matrix.md` — Matriz runtime × versión × proposal × WASI (basada en datos reales `wasm-conformance` o tests propios)
- `docs/gaps/` — Registrar gaps descubiertos (template `docs/gaps/README.md`)
- `docs/adr/` — ADRs para decisiones arquitectónicas (ej. "Usar wasmtime como runtime embebido")

---

## 13. Referencias Primarias (para verificación)

| Fuente | URL | Qué Validar |
|--------|-----|-------------|
| Wasm 3.0 Announcement | https://webassembly.org/news/2025-09-17-wasm-3.0/ | Estándar vivo sep 2025 |
| WASI 0.3 Release | https://wasi.dev/releases/wasi-p3 | Async nativo jun 2026 |
| WASI 0.2 Release | https://wasi.dev/releases/wasi-p2 | Estable ene 2024 |
| WASI Roadmap | https://wasi.dev/roadmap | Parches bimestrales 0.3.x |
| Wasmtime v50 Release | https://github.com/bytecodealliance/wasmtime/releases/tag/v50.0.0 | Versión oct 2026, compact imports opt-in |
| Wasmtime Stability Tiers | https://github.com/bytecodealliance/wasmtime/blob/main/docs/stability-tiers.md | Tier 1/2/3 definiciones |
| Wasmtime WASI API | https://docs.wasmtime.dev/api/wasmtime_wasi/index.html | "Experimental, unstable, incomplete" para 0.3 |
| Component Model Docs | https://component-model.bytecodealliance.org/language-support.html | Toolchains por lenguaje |
| Web Almanac 2025 Wasm | https://almanac.httparchive.org/en/2025/webassembly | 0.35% sitios, 2% top 1k |
| libsodium Benchmark 2026 | https://00f.net/2026/06/23/webassembly-runtimes-2026/ | Metodología + resultados |
| Google Sheets WasmGC | https://web.dev/case-studies/google-sheets-wasmgc | 2x speedup, regex 100x slower |
| Docker Wasm Deprecated | https://docs.docker.com/desktop/features/wasm/ | No GA, beta, sin mantenimiento |
| NGINX Unit Archived | https://unit.nginx.org/news/2024/wasm-component-model-part-1/ | Archivado oct 2025 |
| Warg Archived | https://github.com/bytecodealliance/registry | Archivado 28 jul 2025 |
| wasm-pkg-tools | https://github.com/bytecodealliance/wasm-pkg-tools | Sucesor OCI en construcción |
| wasmCloud LFX | https://www.cncf.io/projects/wasmcloud/ | "Concerning", -25% contrib, -38% orgs |
| Akamai acquires Fermyon | https://www.akamai.com/newsroom/press-release/akamai-announces-acquisition-of-function-as-a-service-company-fermyon | Dic 2025 |
| State of Wasm 2023 | https://thenewstack.io/whos-leading-webassembly-adoption-so-far-vendors/ | 303 users, 41% prod, 55% vendors |
| Debugging Pain | https://thenewstack.io/the-pain-of-debugging-webassembly/ | Retos estructurales DWARF |
| Dylibso Observe | https://dylibso.com/blog/dx-spotlight-shopify-function-runner-observability/ | Observabilidad Shopify |
| GetCoreTech 2026 | https://www.getcoretech.net/articles/webassembly-2026-is-it-living-up-to-the-hype/ | Análisis crítico adopción |

---

## 14. Changelog del Documento

| Fecha | Versión | Cambios |
|-------|---------|---------|
| 2026-10-24 | 1.0 | Versión consolidada inicial (fusiona investigación primaria + Perplexity Deep Research) |

---

*Documento generado automáticamente a partir de investigación multi-fuente. Verificar fuentes primarias antes de decisiones críticas. Actualizar cada 3-6 meses o ante hitos mayores (WASI 0.4, Wasmtime v51, Wasm 3.1, etc.).*