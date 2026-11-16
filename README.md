# WasmFoundry

![WasmFoundry Logo](https://raw.githubusercontent.com/AndyTechnologies/WasmFoundry/main/assets/logo.svg)

**WasmFoundry: toolchain Rust para WebAssembly con contrato explícito**

*Build, inspección y ejecución de proyectos WebAssembly multi-toolchain con un solo contrato de tiempo de ejecución.*

WasmFoundry es un toolchain Rust para WebAssembly que trata a los lenguajes de invitados como adaptadores en lugar de ramas codificadas de forma rígida, resuelve las importaciones entre módulos en lugar de generar C++, y ejecuta invitados bajo una política de capacidades explícita en lugar de una permisividad host implicit.

---

## 📦 Versión actual: **v0.3.1**

La versión v0.3.1 introduce endurecimiento para producción:

- **S1 Suministro endurecido**: `cargo-deny`, `cargo-vet` y `cargo-audit` verificados en CI
- **S2 compilaciones reproducibles forzadas**: `--locked`, `CARGO_NET_RETRY=3`, `SOURCE_DATE_EPOCH` en todos los workflows
- **S3 matriz E2E**: matriz expandida (toolchain × target × feature × OS) + schedule nightly/weekly
- **S4 salida JSON**: `--format json` en `wf build`, `wf run`, `wf bundle`
- **S5 sandbox basado en capacidades**: flag `--sandbox`: caps WASI, seccomp (Linux), Job Objects (Windows)
- **S6 auto-alojamiento bootstrap**: `wf bootstrap` compila CLI a WASM + bundle → `wf` auto-hospedado
- **S7 automatización de release**: `cargo-release` + changelog de commits convencionales
- **S8 perfiles de compilación**: flag `--timings` emite JSON con fases de compilación
- **S9 matriz E2E cross-OS**: runners `ubuntu-latest`, `macos-latest`, `windows-latest`
- **S10 contratos públicos**: `docs/toolchain-protocol.md` (interfaz adaptador), `docs/bundle-format-v1.md` (espec ABNF)

---

## 🎯 Problema que resuelve

Desarrollar un proyecto WebAssembly que abarque más de un lenguaje de invitado, más de un módulo, o un binario nativo usualmente obliga a elegir entre herramientas que cada una cubre solo un segmento y pegamento que cubre ninguno:

- Los toolchains están alambrados por lenguaje, así que agregar un lenguaje requiere editar el core de build.
- La composición multi-módulo se hace generando C o C++ que re-declara toda importación, lo que arrastra una full C toolchain solo para enlazar dos `.wasm`.
- La superficie host ofrecida a los invitados es lo que los bindings generados acontecieron exportar, sin versión y sin capacidades declaradas.
- Distribuir un "ejecutable nativo" significa enganchar un launcher a un payload de una forma específica de un solo formato de ejecutable.

WasmFoundry toma la posición opuesta en cada caso:

| Problema | Posición |
| --- | --- |
| Muchos lenguajes de invitado | Cada uno es un adaptador `Toolchain` detrás de un trait; el core nunca se rama por nombre de lenguaje. |
| Vínculo multi-módulo | El grafo de dependencias se resuelve por nombre e instantiated a través de un linker. No C++, no CMake, no bindings generados. |
| Superficie host | Un ABI host documentado con versión, dividido entre un ABI portable y ABIs de compatibilidad lenguajes. |
| Distribución | Un binary nativo auto-contenido construido sobre un formato de bundle documentado con footer fijo y checksum, format-independent. |
| Permisos de invitado | Una política de capacidades por defecto denegada: no filesystem, no network, no environment, no process execution. |

---

## 📦 Paquetes

| Crate | Responsabilidad | Dependencia clave |
| --- | --- | --- |
| `wf-core` | Vocabulario de dominio y reglas puras. Sin dependencias. | none |
| `wf-wasm` | Análisis de un binary WebAssembly: tipo, imports, exports, validación. | `wasmparser`, `wit-parser`, `wit-component` |
| `wf-runtime` | Ejecución de un módulo analizado bajo Wasmtime. | `wasmtime` |
| `wf-toolchains` | Toolchains de invitado: Rust, C++, AssemblyScript, wasm precompilado, componentes, plugins. | `toml` (manifests de guest crates) |
| `wf-bundle` | Formato de bundle v1, probing de launcher, validación de checksum. | `sha2`, `blake3` |
| `wf-cli` | La interfaz de línea de comando `wf`. | `clap` |

---

## 📦 Requisitos

- **Cadena de herramientas Rust**. La versión exacta está fijada en `rust-toolchain.toml`, así que `rustup` la instala automáticamente.
- **Nada más, todavía**.

Las toolchains de invitado (Rust guest, C++, AssemblyScript) **no** se instalan por este repositorio y no son necesarias para compilarlo. Cada una se detecta bajo demanda y es reportada por `wf doctor` cuando ese comando existe.

### Compilar desde el fuente

```bash
cargo check --workspace
cargo test --workspace
```

### Uso

```bash
# Crear un proyecto. Esquematiza un manifest, un directorio de fuentes y un módulo de muestra.
wf init hello
cd hello

# Publicar el módulo declarado en target/. Las fuentes son wasm precompilado: nada se compila aún, así que no se requiere toolchain de invitado.
wf build

# Ejecutar el módulo built del proyecto, o cualquier camino directamente.
wf run
wf run target/hello.wasm
wf run target/hello.wasm --entry main

# Mantener el artifact built en sincronía con las fuentes: un build por momento de quietud y nunca un rebuild triggered por un write output.
wf build --watch

# Envuelve el proyecto built en un bundle nativo auto-contenido.
# `wf` nunca construye el launcher; tú le das uno que ya existe.
# cargo build -p wf-bundle --bin wf-launcher
# wf bundle --launcher target/debug/wf-launcher

# O resolver un launcher prebuilt del registry local por triple target.
# wf bundle --bundle-target x86_64-unknown-linux-gnu

# `wf build` usa fingerprints de contenido: un módulo sin cambios no se recompila.
# Vea `docs/adr/0001-cache-fingerprint-blake3.md`. Después de actualizar un compilador de invitado (clang++/asc/rustc), limpie los artifacts stale: `rm -rf target`.

# Analice cualquier binary sin ejecutarlo, para un humano o como JSON para otras herramientas.
wf inspect target/hello.wasm
wf inspect target/hello.wasm --format json

# Compilar un WebAssembly Component
wf component build /path/to/component-project
# Herramienta de component usa `cargo component` + validación WIT

# Gestionar plugins (Wasm Components implementando wf:plugin/guest)
wf plugin init my-plugin
cd my-plugin
wf plugin build
# Copiar target/my-plugin.wasm a el directorio plugins/ de tu proyecto

# Validar un archivo WIT
wf wit check path/to/file.wit
```

Un guest Rust necesita el target instalado una vez:

```bash
rustup target add wasm32-unknown-unknown
```

y un módulo declarado contra él:

```toml
[[module]]
name = "hello"
source = "guest"          # el directorio holding Cargo.toml
toolchain = "rust"
```

El crate debe declarar `[lib] crate-type = ["cdylib"]`, lo que hace que `cargo` emita un módulo WebAssembly. Un proyecto de lo contrario es un solo archivo, `wasmfoundry.toml`:

```toml
schema = 1

[package]
name = "hello"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "hello"
source = "src/hello.wasm"
toolchain = "precompiled"
```

Un proyecto puede.state qué su runtime está permitido hacer:

```toml
[runtime]
wasi = true                     # denied unless asked for
wall_clock_seconds = 5.0        # no bound unless asked for
memory_pages = 4096             # one page is 64 KiB

[[mount]]
host = "./data"
guest = "/data"                 # absolute: a guest path is a name in the guest's world
access = "read-only"            # the default
```

Todo está denegado a menos que el manifest lo pida. Un guest que importe WASI sin `wasi = true` compila bien y falla cuando llama — pedir un capability y ser granted son dos cosas diferentes.

Un proyecto puede tener varios módulos. Un import nombra un namespace, y el namespace de un módulo es ya sea stated en el manifest o derivado del matching mode:

```toml
[project]
source_dir = "src"
entry = "_start"
module_matching = "file-name"      # o "name-only"
host_namespaces = ["env"]          # namespaces expected from outside the project

[[module]]
name = "engine"
source = "src/cog.wasm"
namespace = "engine"               # stated: beats both the name and the file
```

Un `namespace` stated gana en todo modo. Sin uno:

- `file-name` — el namespace es el nombre del source file sin su extensión
- `name-only` — el namespace es el module `name`

La misma regla une un import y publica un export, así que los dos lados no pueden desacuerdarse.

`wf build` se niega a un proyecto cuyas imports no puedan binding, porque un import que nombra no módulo de proyecto y no fue declarado como external es probablemente un typo:

- un namespace claimed by two modules — `WF002`, naming the claimants
- un import nothing provides and nothing declares — `WF002`
- un dependency cycle — `WF003`

Declarar un `namespace` en `host_namespaces` es cómo un guest que habla con un host construye: esos namespaces no son módulos de proyecto y nunca lo serán.

Un manifest cuya `schema` esta versión no conoce es rechazado de inmediato: nunca se interpreta parcialmente, porque campos compartidos entre dos schemas pueden haber cambiado de significado. Nombres de tabla y campo desconocidos también son rechazados, así que un typo no pueda hacer nada en silencio.

```
$ wf inspect app.wasm
Module: app.wasm
Kind: core module
Version: 1

Imports (2):
  env.console_log  func  (i32, i32) -> ()
  env.memory  memory min 2 pages, max 16 pages

Exports (2):
  memory  memory
  _start  func  (i32) -> (i32)

Memories (2):
  #0  min 2 pages, max 16 pages
  #1  min 2 pages, max 16 pages

Tables (0):
  (none)

Globals (0):
  (none)

Functions (2):
  #0  imported  (i32, i32) -> ()
  #1  defined   (i32) -> (i32)
```

`wf inspect` nunca ejecuta el binary que lee. Los códigos de salida son estables y un script puede ramificarlos:

| Código | Significado |
| --- | --- |
| `0` | El comando tuvo éxito |
| `1` | El comando se ejecutó y la entrada fue rechazada, con un diagnostic `WFnnn` |
| `2` | El linea de comando o el path de entrada estaba mal |

Los códigos de diagnóstico, también estables:

| Código | Causa |
| --- | --- |
| `WF001` | La entrada no es un módulo WebAssembly compilable |
| `WF002` | Un import tiene no provider, o no puede binding a un módulo |
| `WF003` | Los módulos del proyecto forman un dependency cycle |
| `WF004` | El toolchain que un módulo pide no está disponible |
| `WF005` | El punto de entrada solicitado no está exportado |
| `WF009` | El guest faulted mientras ejecutaba |
| `WF010` | El punto de entrada existe pero no puede invocarse como está escrito |
| `WF011` | El manifest no significa lo que aparenta significar |
| `WF012` | Una toolchain se ejecutó y no produjo un módulo |
| `WF013` | Una host function falló mientras el guest la llamaba |
| `WF014` | Una invocación excedió un bound the policy set |

Un guest que retorna un valor desde su punto de entrada se reporta como output, no convertido en exit code de este proceso: ese mapeo es una decisión de host-ABI, y el host ABI llega en PHASE 8.

---

## 📋 Status

El rewrite procede en vertical slices. Cada phase entrega una capacidad observable y debe pasar su gate antes de que empiece la siguiente.

| Phase | Entrega | Gate |
| --- | --- | --- |
| 0–1 | Legacy state preserved, `main` reset | ✅ done |
| 2 | Rust workspace, four crates, documentation | ✅ done |
| 3 | `wf inspect` | ✅ done |
| 4 | `wf run` | ✅ done |
| 5 | `wf init`, `wf build` para wasm precompilado | ✅ done |
| 6 | Rust guest toolchain | ✅ done |
| 7 | Dependency graph and module matching | ✅ done |
| 8–9 | Host ABI v1, WASI, mounts, capability policy | ✅ done |
| 10–11 | C++ y AssemblyScript toolchains, `wf-toolchains` crate | ✅ done |
| 12–13 | Build cache (BLAKE3), `wf build --watch` | ✅ done |
| 14–16 | Bundle format v1, prebuilt launchers, cross-target | ✅ done |
| 17 | Component Model + WIT | ✅ done |
| 18 | External plugins (Wasm Components) | ✅ done |
| 19 | Security tooling + supply chain (`cargo-deny`, `cargo-audit`) | ✅ done |
| 20 | CI/CD (GitHub Actions) | ✅ done |
| 21 | Release pipeline + `v0.1.0` | ✅ done |

Paridad funcional con la generación anterior de este proyecto se rastrea explícitamente en
[`docs/parity-matrix.md`](docs/parity-matrix.md), incluyendo las capacidades que se están deliberadamente **no** carrying over.

### Documentación relevante

- [`docs/product.md`](docs/product.md) — qué es el producto, non-goals, security boundary
- [`docs/architecture.md`](docs/architecture.md) — responsabilidades de crate y reglas de dependencia
- [`docs/parity-matrix.md`](docs/parity-matrix.md) — mapping de capacidades legacy y decisiones
- [`docs/gaps/`](docs/gaps/) — gaps descubiertos durante el rewrite
- [`docs/adr/`](docs/adr/) — arquitectura decision records
- [`AGENTS.md`](AGENTS.md) — operating instructions for coding agents

### Seguridad

Dos boundaries de trust existen en este producto, y no son lo mismo.

**`wf run` ejecuta un invitado.** El sandbox de Wasmtime aplica: el invitado no tiene filesystem, network, environment o process access, porque no hay nada que darle. No hay host ABI todavía, así que todo import fails a resolver en lugar de alcanzar el host.

**`wf run` es bounded cuando el proyecto lo pide.** Un budget de wall-clock y un memory ceiling están set en el manifest, y un guest que cruza uno es stop y reportado como `WF014`. Un proyecto que set neither runs unbounded, which is the honest reading of asking for no bound.

**`wf build` no sandboxará nada.** Cuando llega en PHASE 5 correrá el guest toolchain — `build.rs`, compilers, package managers, scripts — con los permisos de tu cuenta. El sandbox de Wasmtime no protege contra esos procesos, y el producto nunca lo claims.

`wf inspect` no ejecuta lo que lee. Vea
[`docs/product.md`](docs/product.md#security-boundary); `docs/security/threat-model.md` llega con el capability policy en PHASE 9.

### Licencia

MIT. Vea [LICENSE](LICENSE).
