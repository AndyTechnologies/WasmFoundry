# WasmFoundry

## Plan maestro de reescritura desde cero

### Documento de implementación para agente de código

**Proyecto actual:** `AndyTechnologies/wasm-apps`
**Proyecto objetivo:** `WasmFoundry`
**Lenguaje objetivo:** Rust
**CLI objetivo:** `wf`
**Estado de la reescritura:** desde cero, sin migración incremental
**Compatibilidad con la implementación anterior:** no requerida
**Compatibilidad funcional:** perseguida mediante una matriz explícita de paridad
**Runtime:** Wasmtime
**Tooling WebAssembly:** `wasmparser`, `wit-parser`, `wit-component`, `wasm-encoder`
**Build system:** Cargo workspace
**CI/CD:** eliminar durante la reescritura y reconstruir al final

---

# 1. Objetivo de este documento

Este documento es el contrato operativo que deberá seguir el agente de código para transformar el repositorio actual en `WasmFoundry`.

El agente debe ejecutar la reescritura completa de principio a fin.

No debe interpretar el documento como una lista de ideas opcionales.

Las decisiones marcadas como **OBLIGATORIO** forman parte del resultado esperado.

Las decisiones marcadas como **DEFERIDO** no deben implementarse antes de que llegue el milestone correspondiente.

---

# 2. Principio fundamental

No hacer una traducción:

```text
TypeScript → Rust
```

Hacer una reimplementación:

```text
wasm-apps
    ↓
análisis funcional
    ↓
matriz de capacidades
    ↓
nuevo diseño
    ↓
WasmFoundry
```

El código existente sirve únicamente como:

```text
referencia funcional
referencia de comportamiento
fuente de casos límite
fuente de fixtures
fuente del ABI existente
```

No sirve como plantilla arquitectónica.

Está expresamente prohibido portar directamente:

```text
wasm-io.ts
linker.ts
codegen.ts
plugin-manager.ts
C++ templates
Nunjucks templates
cmake-js integration
Wasmtime C API integration
```

El parser WASM existente implementa manualmente parsing de header, sections, imports, exports, tipos y LEB128; esta implementación no debe migrarse a Rust. Debe sustituirse por `wasmparser`.

---

# 3. Resultado final esperado

La estructura final debe converger a:

```text
WasmFoundry/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── LICENSE
├── README.md
├── AGENTS.md
│
├── crates/
│   ├── wf-core/
│   ├── wf-wasm/
│   ├── wf-runtime/
│   ├── wf-toolchains/
│   ├── wf-bundle/
│   └── wf-cli/
│
├── docs/
│   ├── architecture.md
│   ├── parity-matrix.md
│   ├── product.md
│   ├── abi/
│   │   ├── host-abi-v1.md
│   │   └── guest-sdks.md
│   ├── security/
│   │   ├── threat-model.md
│   │   └── runtime-security.md
│   ├── bundling/
│   │   └── bundle-format-v1.md
│   ├── gaps/
│   └── adr/
│
├── examples/
├── fixtures/
└── tests/
```

Pero esta estructura **no debe aparecer toda en el primer commit**.

Los crates `wf-toolchains` y `wf-bundle` se incorporarán cuando el producto llegue a sus respectivos vertical slices.

---

# 4. Arquitectura inicial deliberadamente pequeña

No comenzar con 15 crates.

La arquitectura inicial será:

```text
wf-core
wf-wasm
wf-runtime
wf-cli
```

Después:

```text
+ wf-toolchains
```

y posteriormente:

```text
+ wf-bundle
```

No crear un crate únicamente porque "algún día podría ser útil".

La regla:

> Una frontera física nueva debe estar justificada por un segundo consumidor, una segunda implementación o una responsabilidad que necesite evolucionar independientemente.

---

# 5. Dependencias entre crates

La dirección de dependencias debe ser:

```text
                    wf-core
                      ▲
                      │
          ┌───────────┼────────────┐
          │           │            │
       wf-wasm    wf-runtime   wf-toolchains
          ▲           ▲            ▲
          │           │            │
          └───────────┼────────────┘
                      │
                    wf-cli
                      │
                      ▼
                  wf-bundle
```

Una versión más precisa para la fase inicial:

```text
wf-core
  ↑
wf-wasm
  ↑
wf-runtime
  ↑
wf-cli
```

`wf-cli` puede depender de los tres.

Cuando aparezca la compilación multi-toolchain:

```text
wf-toolchains → wf-core
wf-cli → wf-toolchains
```

Cuando aparezca packaging:

```text
wf-bundle → wf-core
wf-bundle → wf-runtime
wf-cli → wf-bundle
```

---

# 6. Reglas de dependencias

## `wf-core`

NO debe depender de:

```text
wasmtime
wasmparser
tokio
clap
serde
cargo
clang
assemblyscript
filesystem APIs específicas
```

Debe contener únicamente:

```text
tipos del dominio
reglas
validaciones
grafo
fingerprints
errores de dominio
```

---

## `wf-wasm`

Puede depender de:

```text
wasmparser
```

y posteriormente:

```text
wit-parser
wit-component
wasm-encoder
```

No debe exponer tipos de esas librerías en su API pública.

---

## `wf-runtime`

Puede depender de:

```text
wasmtime
wasmtime-wasi
```

y sus dependencias transitivas.

El resto del sistema no debe depender directamente de tipos concretos de Wasmtime cuando pueda evitarlo.

Wasmtime actualmente proporciona `Engine`, `Module`, `Store`, `Linker` y el modelo de ejecución necesario para este diseño. `Store` contiene las instancias y el estado del host; `Module` representa el código Wasm compilado; `Linker` resuelve imports.

---

# 7. No usar async como arquitectura de aplicación

El MVP debe ser:

```text
síncrono
+
std threads cuando sea necesario
```

No introducir:

```text
Tokio
async trait
async everywhere
```

únicamente por anticipación.

Excepción: `wasmtime-wasi` puede traer Tokio transitivamente; eso no significa que WasmFoundry deba diseñarse alrededor de Tokio. La implementación actual de `wasmtime-wasi` utiliza Tokio internamente.

---

# 8. Preparación Git: precondiciones absolutas

Antes de modificar cualquier archivo:

```bash
git status --short
git branch --show-current
git remote -v
git fetch origin --prune
```

El agente debe detener la operación si existe un worktree sucio:

```text
M M file
?? file
D file
```

No debe asumir que los cambios locales pertenecen al proyecto.

No debe hacer:

```bash
git reset --hard
git clean -fdx
```

antes de preservar la versión legacy y verificar el estado.

---

# 9. Preservación exacta del estado actual

El objetivo es que:

```text
legacy
```

represente exactamente el `main` actual antes de la reescritura.

## Paso 9.1

Actualizar `main`:

```bash
git switch main
git pull --ff-only origin main
```

## Paso 9.2

Capturar el commit exacto:

```bash
LEGACY_SHA="$(git rev-parse HEAD)"
echo "$LEGACY_SHA"
```

## Paso 9.3

Verificar que no existe ya una rama `legacy`.

```bash
git branch --list legacy
git ls-remote --heads origin legacy
```

Si `legacy` ya existe:

* verificar a qué commit apunta;
* no sobrescribirla silenciosamente;
* si no apunta a `LEGACY_SHA`, detener la operación destructiva.

## Paso 9.4

Crear la rama:

```bash
git branch legacy "$LEGACY_SHA"
```

## Paso 9.5

Crear además una etiqueta inmutable:

```bash
git tag -a legacy-v1 "$LEGACY_SHA" \
  -m "Último estado de wasm-apps antes de la reescritura a WasmFoundry"
```

## Paso 9.6

Publicar ambos:

```bash
git push origin legacy
git push origin legacy-v1
```

## Paso 9.7

Verificar:

```bash
git rev-parse legacy
git rev-parse origin/legacy
git rev-parse legacy-v1
```

Los tres deben corresponder al estado legacy esperado.

---

# 10. Uso de worktrees para separar físicamente ambas versiones

Como el nuevo proyecto debe convivir localmente en una carpeta diferente, utilizar Git worktrees.

Estructura recomendada:

```text
~/src/wasm-apps-legacy/
    ↓
    branch legacy

~/src/WasmFoundry/
    ↓
    branch main
```

Después de crear `legacy`:

```bash
git switch legacy
git worktree add ../WasmFoundry main
```

Entrar al nuevo proyecto:

```bash
cd ../WasmFoundry
git branch --show-current
```

Debe mostrar:

```text
main
```

El worktree legacy debe conservar:

```text
legacy
```

---

# 11. Regla de oro Git durante la reescritura

Nunca hacer:

```bash
git push --force
```

durante esta reescritura.

Nunca borrar:

```text
legacy
legacy-v1
```

mientras el proyecto nuevo se está desarrollando.

No hacer rebase destructivo de `legacy`.

La rama legacy es el respaldo histórico oficial.

---

# 12. Inventario funcional previo a la eliminación

Antes de borrar `main`, el agente debe generar localmente un inventario temporal de la versión anterior.

No añadir ese inventario a `legacy`.

Utilizar:

```bash
git ls-tree -r --name-only legacy > /tmp/wasmfoundry-legacy-files.txt
```

e inspeccionar:

```bash
git ls-tree -r --name-only legacy
```

También debe inspeccionar:

```text
README.md
AGENTS.md
docs/
packages/
examples/
scripts/
.github/
package.json
pnpm-workspace.yaml
```

El estado actual contiene, entre otros:

```text
.codegraph
.engram
.github/workflows
.husky
data
docs
examples
packages
scripts
skills
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
```

y el paquete raíz aún usa pnpm/Node y scripts de release/test/lint del stack TypeScript.

---

# 13. Matriz de paridad obligatoria

Antes de implementar funcionalidades equivalentes, crear:

```text
docs/parity-matrix.md
```

Debe contener como mínimo:

| Funcionalidad legacy       | Estado WasmFoundry | Decisión               |
| -------------------------- | ------------------ | ---------------------- |
| AssemblyScript             | pendiente          | preservar              |
| C++                        | pendiente          | preservar              |
| Rust guest                 | pendiente          | preservar              |
| precompiled WASM           | pendiente          | preservar              |
| `moduleMatching=file-name` | pendiente          | preservar              |
| `moduleMatching=name-only` | pendiente          | preservar              |
| console                    | pendiente          | preservar              |
| fs                         | pendiente          | preservar/replantear   |
| WASI                       | pendiente          | preservar              |
| mounts                     | pendiente          | preservar/replantear   |
| optimization               | pendiente          | preservar              |
| sourcemaps                 | pendiente          | preservar              |
| cross compilation          | pendiente          | preservar              |
| plugins                    | diferido           | rediseñar              |
| custom templates           | descartado         | reemplazar             |
| Nunjucks                   | descartado         | eliminar               |
| C++ code generation        | descartado         | eliminar               |
| CMake linker               | descartado         | eliminar               |
| C-API Wasmtime             | descartado         | Rust API               |
| parser WASM propio         | descartado         | `wasmparser`           |
| cache                      | diferido           | implementar tras medir |

El `wapp.json` actual documenta explícitamente `moduleMatching`, `wasi`, `mounts`, configuraciones por toolchain, plugins y templates personalizados.

---

# 14. Regla de paridad

La reescritura no tiene que conservar:

```text
API
nombres
CLI antigua
formato wapp.json
estructura de carpetas
C++ generado
templates
```

Sí debe analizar cuidadosamente:

```text
semántica
comportamiento
capacidades
errores
casos límite
ABI
```

La pregunta correcta no es:

> "¿Cómo copio esta función?"

La pregunta correcta es:

> "¿Qué contrato funcional estaba implementando esta función?"

---

# 15. Eliminación completa de `main`

Una vez:

```text
legacy branch creada
legacy tag creada
legacy remote verificada
parity inventory creado
```

comenzar la destrucción controlada de `main`.

En el worktree nuevo:

```bash
git switch main
git status --short
```

Después eliminar todos los archivos legacy rastreados:

```bash
git rm -r -- .
```

Si quedan archivos no rastreados generados accidentalmente, inspeccionarlos primero.

No utilizar `git clean -fdx` ciegamente.

El objetivo es que `main` quede sin el código anterior.

---

# 16. Excepciones que pueden conservarse

El agente puede preservar:

```text
LICENSE
```

si es legalmente correcto y coincide con la intención del propietario.

Todo lo demás debe considerarse legacy hasta justificar lo contrario.

En particular, no preservar automáticamente:

```text
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
.npmrc
.pnpmfile.cjs
.prettierrc
.prettierignore
eslint.config.js
vitest.config.ts
.husky
.github/workflows
scripts/
packages/
```

El repositorio actual efectivamente contiene estos artefactos del stack anterior.

---

# 17. Archivos de agente

No conservar el `AGENTS.md` anterior.

Debe eliminarse y recrearse.

El `AGENTS.md` antiguo contiene instrucciones específicamente ligadas a:

```text
TypeScript/ESM
pnpm
Node
@wasm-apps/types
logger TypeScript
Nunjucks
CMake
```

por lo que mantenerlo contaminaría el proyecto nuevo.

Crear un nuevo `AGENTS.md` para Rust.

---

# 18. CI/CD legacy: eliminarlo completamente

Eliminar:

```text
.github/workflows/*
```

y cualquier otro archivo específico de automatización del pipeline anterior.

El repositorio actual tiene workflows bajo `.github/workflows`, por lo que el nuevo `main` no debe seguir ejecutándolos mientras se reconstruye el proyecto.

También eliminar del `main`:

```text
release scripts npm
publish scripts
package publishing configuration
GitHub action configuration
npm package metadata
```

No crear workflows nuevos todavía.

---

# 19. Importante: GitHub Actions histórico no desaparece

Eliminar los archivos YAML elimina el pipeline futuro del repositorio, pero no necesariamente elimina el historial de ejecuciones de GitHub.

Eso es aceptable.

No intentar manipular manualmente el historial de Actions durante esta fase.

---

# 20. Primer commit del nuevo `main`

Después de la limpieza, crear solamente:

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
LICENSE
README.md
AGENTS.md
docs/
crates/
fixtures/
examples/
tests/
.gitignore
```

No añadir CI.

No añadir release automation.

No añadir plugins.

No añadir cache.

No añadir bundle.

---

# 21. Nombre del proyecto

El nuevo proyecto debe utilizar:

```text
WasmFoundry
```

Nombre Cargo:

```text
wasmfoundry
```

CLI:

```text
wf
```

Crates:

```text
wf-core
wf-wasm
wf-runtime
wf-toolchains
wf-bundle
wf-cli
```

Antes de publicar en crates.io debe comprobarse la disponibilidad de esos nombres.

La renombración del repositorio GitHub es una operación administrativa separada; el agente no debe afirmar que la realizó si no tiene acceso al sistema de administración correspondiente.

---

# 22. Primer `Cargo.toml`

Utilizar un workspace.

Conceptualmente:

```toml
[workspace]
resolver = "3"
members = [
    "crates/wf-core",
    "crates/wf-wasm",
    "crates/wf-runtime",
    "crates/wf-cli",
]
```

No añadir desde el primer día:

```text
wf-toolchains
wf-bundle
```

---

# 23. Rust toolchain

Crear:

```text
rust-toolchain.toml
```

El agente debe:

1. determinar la versión mínima de Rust compatible con las dependencias seleccionadas;
2. verificarla realmente con `cargo check`;
3. fijar un toolchain reproducible;
4. no utilizar nightly salvo necesidad demostrada.

Wasmtime 49.0.1 es la release estable actual y debe ser el baseline a evaluar, no una versión imaginaria o futura.

No fijar una versión porque esté escrita en este documento si `cargo` demuestra otra restricción.

---

# 24. Primera dependencia de runtime

El runtime inicial debe utilizar:

```text
wasmtime
```

No utilizar:

```text
wasmtime-c-api
```

No instalar:

```text
CMake
cmake-js
bindgen para la C API
```

Wasmtime Rust ya expone el modelo de embedding necesario.

---

# 25. Primera dependencia de análisis

Utilizar:

```text
wasmparser
```

No crear:

```text
WasmReader
Leb128Reader
CustomWasmParser
```

si solamente están reproduciendo funcionalidades existentes de `wasmparser`.

`wasm-tools` recomienda utilizar sus librerías directamente cuando se integran en otro proyecto.

---

# 26. `wf-core`

Implementar inicialmente:

```text
ArtifactId
ArtifactKind
Artifact
ModuleId
ToolchainId
Target
EntryPoint
BuildProfile
Dependency
DependencyGraph
RuntimePolicy
Diagnostic
Fingerprint
```

No implementar todavía:

```text
ProjectAggregate
Repository<T>
DomainServiceFactory
PluginManager
UniversalCompiler
UniversalLinker
```

---

# 27. `wf-core` debe ser puro

Ejemplos válidos:

```rust
pub fn topological_order(...)
pub fn detect_cycle(...)
pub fn calculate_fingerprint(...)
pub fn resolve_module_name(...)
pub fn validate_runtime_policy(...)
```

Ejemplos prohibidos:

```rust
pub fn compile_with_cargo(...)
pub fn execute_wasmtime(...)
pub fn read_file(...)
```

---

# 28. `Artifact`

Debe representar el resultado lógico de una operación:

```text
source
    ↓
artifact
```

El dominio no debe asumir todavía dónde están los bytes.

Conceptualmente:

```rust
struct ArtifactRef {
    id: ArtifactId,
    kind: ArtifactKind,
}
```

y el almacenamiento físico será responsabilidad de una capa posterior.

No implementar `ArtifactStore` hasta que exista la primera necesidad real.

---

# 29. `DependencyGraph`

Implementar como estructura pura.

Debe soportar:

```text
add node
add dependency
dependencies
dependents
roots
leaves
topological order
cycle detection
```

Debe producir errores deterministas.

Para:

```text
A → B → C → A
```

el error debe incluir:

```text
A → B → C → A
```

y no solamente:

```text
cycle detected
```

---

# 30. `moduleMatching`

La implementación actual soporta:

```text
file-name
name-only
```

y este comportamiento está documentado en el proyecto anterior.

La nueva arquitectura debe modelarlo explícitamente:

```rust
enum ModuleMatching {
    FileName,
    NameOnly,
}
```

No asumir que resolver imports a módulos es trivial.

---

# 31. Primer vertical slice: `wf inspect`

Crear:

```bash
wf inspect hello.wasm
```

Debe:

1. abrir el archivo;
2. validar que es WebAssembly;
3. analizarlo con `wasmparser`;
4. identificar formato;
5. mostrar imports;
6. mostrar exports;
7. mostrar memories;
8. mostrar tables;
9. mostrar globals;
10. mostrar funciones cuando sea relevante;
11. detectar si se trata de core module o component.

No necesita runtime.

---

# 32. Output humano inicial

Ejemplo aproximado:

```text
Module: hello.wasm
Kind: Core WebAssembly
Version: 1

Imports:
  env.console_log : (i32, i32) -> ()

Exports:
  _start : () -> ()
  memory : memory

Memory:
  min: 2 pages
  max: 16 pages
```

No congelar todavía el formato exacto.

Sí congelar:

```text
semántica
información mínima
códigos de error
```

---

# 33. `wf inspect --format json`

No añadir `serde` al dominio.

Crear DTOs de CLI:

```text
domain object
    ↓
CLI DTO
    ↓
serde_json
```

El JSON debe considerarse una representación de interfaz, no el modelo interno.

---

# 34. Primer vertical slice: `wf run`

Crear:

```bash
wf run hello.wasm
```

Implementar:

```text
Engine
Module
Store
Linker
Instance
entrypoint
```

El runtime debe asumir inicialmente:

```text
core wasm
sin WASI
```

---

# 35. Entry point

Por defecto:

```text
_start
```

pero no hardcodearlo en toda la arquitectura.

Modelarlo:

```rust
struct EntryPoint(String);
```

El CLI podrá añadir:

```bash
wf run app.wasm --entry main
```

posteriormente.

---

# 36. Runtime lifecycle

Utilizar conceptualmente:

```text
Engine
    ↓
Module
    ↓
Store<HostState>
    ↓
Linker
    ↓
Instance
    ↓
entry function
```

Un `Store` no debe mantenerse global para todas las ejecuciones. Wasmtime documenta el `Store` como contenedor del estado y las instancias de ejecución, pensado para una vida relativamente corta.

---

# 37. Runtime errors

Separar:

```text
invalid wasm
link error
trap
missing export
host error
runtime configuration error
execution limit
```

No devolver simplemente:

```text
execution failed
```

---

# 38. Segundo vertical slice: proyecto mínimo

Después de `wf run`, implementar:

```bash
wf init
wf build
```

pero usando inicialmente únicamente:

```text
precompiled WASM
```

Esto introduce el concepto de proyecto sin introducir todavía Rust/C++/AssemblyScript.

---

# 39. Configuración nueva

Crear:

```text
wasmfoundry.toml
```

Versión inicial:

```toml
schema = 1

[package]
name = "hello"
version = "0.1.0"

[project]
source_dir = "src"
entry = "_start"

[[module]]
name = "app"
source = "src/app.wasm"
toolchain = "precompiled"
```

No implementar todavía todo el esquema final.

---

# 40. Configuración debe crecer por necesidad

No añadir inicialmente:

```toml
[cache]
[watch]
[plugins]
[bundle]
[cross]
[components]
[optimization]
```

hasta que una fase lo necesite.

Cada nueva sección debe venir acompañada de:

```text
documentación
validación
tests
```

---

# 41. Primer contrato `Toolchain`

Cuando `precompiled` y `Rust` estén cerca, crear la primera abstracción real:

```rust
trait Toolchain {
    fn id(&self) -> ToolchainId;
    fn detect(&self, source: &Source) -> Detection;
    fn compile(&self, request: CompileRequest) -> Result<CompileResult>;
}
```

Mantenerla pequeña.

No incluir inicialmente:

```text
watch
cache
optimization
doctor
cross
```

como métodos.

---

# 42. `PrecompiledToolchain`

Responsabilidad:

```text
.wasm
    ↓
validate
    ↓
Artifact
```

No compilar nada.

Este adapter será además la implementación mínima para probar el pipeline completo.

---

# 43. `RustToolchain`

Después:

```text
.wasm.rs
    ↓
cargo
    ↓
wasm artifact
```

El toolchain debe encapsular:

```text
cargo invocation
target
release/debug
features
manifest
diagnostics
artifact discovery
```

El resto de WasmFoundry no debe construir comandos Cargo directamente.

---

# 44. Target guest vs target host

Deben ser conceptos separados.

Ejemplo:

```text
Guest:
wasm32-unknown-unknown

Host:
x86_64-unknown-linux-gnu
```

o:

```text
Guest:
wasm32-wasip1

Host:
aarch64-apple-darwin
```

Nunca utilizar un único `target` para representar ambos.

---

# 45. No migrar automáticamente a WASI

El proyecto actual utiliza Rust guest sobre `wasm32-unknown-unknown` y expone `_start`; además, su documentación indica que WASI es una opción separada.

Por tanto:

```text
Core WASM
```

y:

```text
WASI
```

son caminos explícitos.

No cambiar todos los ejemplos de `wasm32-unknown-unknown` a `wasm32-wasip1` únicamente por conveniencia arquitectónica.

---

# 46. Tercer vertical slice: multi-module

Implementar:

```text
module A
    imports B.foo

module B
    exports foo
```

El pipeline:

```text
sources
 ↓
compile
 ↓
analysis
 ↓
module resolution
 ↓
dependency graph
 ↓
runtime plan
 ↓
Wasmtime Linker
```

Wasmtime `Linker` ya proporciona resolución por nombre y permite instanciar módulos con imports satisfechos.

No volver a generar C++ para representar este proceso.

---

# 47. Imports y exports deben conservar el tipo

No modelar solamente:

```text
name
module
```

Debe preservarse:

```text
function
table
memory
global
```

y los tipos asociados.

El parser anterior ya distinguía estas cuatro clases manualmente.

La nueva implementación debe delegarlo en `wasmparser`.

---

# 48. Cuarto vertical slice: Host ABI

Antes de portar los bindings, crear:

```text
docs/abi/host-abi-v1.md
```

Debe describir:

```text
namespaces
imports
function signatures
argument representation
return representation
string representation
memory ownership
errors
capabilities
versioning
```

---

# 49. Separar Host ABI de SDKs

Arquitectura:

```text
                 Host ABI v1
                      │
           ┌──────────┼──────────┐
           │          │          │
           ▼          ▼          ▼
       Rust SDK     C++ SDK    AS SDK
```

El contrato ABI es común.

Cada lenguaje implementa su binding.

---

# 50. Portar el ABI desde el comportamiento, no desde el código

El repo legacy contiene bindings:

```text
console
fs
wasi
alloc
```

para Rust y otros mecanismos de integración. El crate Rust anterior es `no_std` y exporta módulos de `alloc`, `console`, `fs` y `wasi`.

El agente debe:

1. enumerar todas las funciones;
2. extraer sus nombres;
3. extraer firmas;
4. identificar representación de strings;
5. identificar quién asigna/libera memoria;
6. identificar ownership;
7. crear fixtures;
8. documentar;
9. implementar;
10. testear.

No asumir ninguna ABI de memoria basándose exclusivamente en memoria del modelo.

---

# 51. AssemblyScript string ABI

Antes de implementar funciones que reciban strings:

1. inspeccionar el AssemblyScript runtime usado en legacy;
2. inspeccionar los imports generados por módulos reales;
3. confirmar layout y encoding;
4. documentar el resultado en `host-abi-v1.md`;
5. construir tests de bytes reales.

No asumir:

```text
UTF-8
UTF-16
length prefix
pointer layout
```

sin verificarlo.

La ABI debe derivarse de artefactos reales.

---

# 52. Dividir ABI portable y ABI de compatibilidad

El diseño debe diferenciar:

## Portable WasmFoundry host ABI

Ejemplos conceptuales:

```text
console
time
random
process
filesystem
```

## AssemblyScript compatibility ABI

Funciones `env.*` requeridas por el runtime/stdlib de AssemblyScript.

Esto evita convertir necesidades específicas de AssemblyScript en el contrato universal de WasmFoundry.

---

# 53. Runtime extensions

No crear un framework enorme.

Primera extensión real:

```text
ConsoleExtension
```

Segunda:

```text
WasiExtension
```

Solo cuando exista la necesidad.

Cuando aparezca una segunda implementación real de la misma frontera, formalizar:

```rust
trait RuntimeExtension
```

No antes.

---

# 54. WASI

WASI debe configurarse desde:

```text
RuntimePolicy
```

hacia:

```text
wasmtime-wasi adapter
```

Nunca exponer:

```text
WasiCtx
WasiCtxBuilder
FsPerms
ResourceTable
```

al dominio.

La API actual de `wasmtime-wasi` separa actualmente `p1`, `p2` y `p3`, y `WasiView` conecta el `WasiCtx` al estado de `Store`.

---

# 55. Filesystem policy

El modelo del dominio debe utilizar algo estable:

```rust
enum FilesystemAccess {
    ReadOnly,
    ReadWrite,
}
```

El adapter traduce esto a la API actual de Wasmtime.

La documentación actual de `wasmtime-wasi` expone `FsPerms` y distintos caminos de WASI; no acoplar el dominio a ellos.

---

# 56. Mounts

Preservar el concepto legacy:

```toml
[[mount]]
host = "./data"
guest = "/data"
access = "read-only"
```

pero mejorarlo respecto a `wapp.json`.

El comportamiento legacy debe estudiarse porque anteriormente los mounts relativos se resolvían frente al cwd del build y se configuraban con permisos de lectura/escritura por defecto.

La nueva implementación debe cambiar el default a:

```text
read-only
```

salvo que exista una razón explícita para escritura.

---

# 57. Threat model

Crear:

```text
docs/security/threat-model.md
```

Separar explícitamente:

```text
A. WASM no confiable
B. manifest no confiable
C. source project no confiable
D. plugin no confiable
E. bundle final
```

---

# 58. Diferenciar runtime sandbox de build sandbox

Esto es crítico.

```text
wf run malicious.wasm
```

puede estar protegido por:

```text
Wasmtime sandbox
WASI capabilities
resource limits
```

Pero:

```text
wf build malicious-project
```

puede ejecutar:

```text
cargo
build.rs
npm
clang
cmake
scripts
```

con permisos del host.

La sandbox de Wasmtime no protege contra esos procesos.

No documentar nunca:

> "WasmFoundry sandboxea todo el proyecto."

Eso sería falso.

---

# 59. Políticas de seguridad iniciales

Por defecto:

```text
filesystem: denied
network: denied
environment: denied
host process execution: denied
```

Las capacidades se conceden explícitamente.

---

# 60. Ejecución limitada

Definir conceptualmente:

```text
ExecutionPolicy
    ├── wall_clock_timeout
    ├── fuel_budget
    └── memory_limit
```

MVP:

```text
wall-clock timeout
memory limit
```

Para timeout de pared, implementar primero mediante epoch interruption de Wasmtime.

Fuel quedará disponible para casos donde importe un presupuesto determinista de ejecución.

No implementar tres sistemas de timeout diferentes en paralelo.

---

# 61. Quinto vertical slice: C++

Crear `wf-toolchains`.

C++ debe seguir:

```text
C++ source
    ↓
toolchain adapter
    ↓
WASM
```

El resto del sistema no debe conocer:

```text
clang++
cmake
zig cc
wasi-sdk
sysroot
```

---

# 62. El primer backend C++ debe ser deliberadamente único

No soportar simultáneamente:

```text
GCC
Clang
Zig
MSVC
osxcross
```

como paths independientes.

Escoger un backend primario que cubra el caso MVP.

Después introducir capacidades:

```text
CompilerBackend
Target
Sysroot
Linker
```

solamente cuando exista una segunda implementación real.

---

# 63. Sexto vertical slice: AssemblyScript

Implementar:

```text
AssemblyScriptToolchain
```

El adapter debe encapsular:

```text
asc
runtime selection
optimization
shrink
source map
entry point
artifact discovery
```

El proyecto actual soporta varios modos de runtime AssemblyScript y opciones de optimización/sourcemaps; la matriz de paridad debe utilizar esto como comportamiento a evaluar.

---

# 64. No contaminar el runtime con AssemblyScript

No hacer:

```text
wasmfoundry runtime == AssemblyScript runtime
```

Debe existir:

```text
AssemblyScript toolchain
+
AssemblyScript compatibility host bindings
```

El runtime sigue siendo genérico.

---

# 65. Optimización

Separar:

```text
compiler optimization
```

de:

```text
post-build Wasm optimization
```

Por ejemplo:

```text
Rust -O
```

no es lo mismo que:

```text
wasm-opt
```

No añadir una capa de optimizer hasta que exista una herramienta real que implementar.

---

# 66. Sourcemaps

Preservar soporte debug.

El sistema debe poder decir:

```text
debug:
  keep names
  keep source maps
  keep debug metadata

release:
  optimize
  strip optional metadata
```

No eliminar metadata durante análisis.

---

# 67. Séptimo vertical slice: cache

Solo después de que existan al menos:

```text
Rust
C++
AssemblyScript
```

o dos toolchains reales.

Primero medir:

```text
cold build
warm build
compiler native cache
WasmFoundry rebuild
```

No implementar una cache global simplemente porque la arquitectura la contempla.

---

# 68. Cache fingerprint

Cuando llegue el momento, el fingerprint deberá incluir al menos:

```text
source hash
toolchain id
toolchain version
compiler options
guest target
manifest relevant fields
host ABI version
dependency fingerprints
WasmFoundry version
```

No utilizar únicamente:

```text
hash(source)
```

---

# 69. Hashing

Para fingerprints internos:

```text
BLAKE3
```

como candidato principal.

Para checksums de distribución:

```text
SHA-256
```

si se requiere compatibilidad amplia.

La decisión final debe quedar en un ADR pequeño cuando se implemente la cache.

---

# 70. Octavo vertical slice: watch

Solo después de:

```text
wf build
```

ser determinista.

Implementar:

```bash
wf build --watch
```

Inicialmente:

```text
rebuild completo
```

Después:

```text
graph invalidation
```

No implementar incrementalidad compleja antes de tener mediciones.

---

# 71. Build invalidation

Cuando se implemente:

```text
changed source
    ↓
affected artifact
    ↓
dependents
```

El grafo de dependencias debe servir como base de invalidación.

---

# 72. Octavo/noven vertical slice: Bundle

No compilar un launcher nuevo con Cargo en cada `wf bundle`.

El diseño final utilizará:

```text
prebuilt native launchers
```

publicados para targets soportados.

---

# 73. Desarrollo local del launcher

Aunque el release final utilice launchers precompilados, durante el desarrollo puede existir:

```text
crates/wf-bundle/
    src/
    src/bin/wf-launcher.rs
```

Ese launcher podrá compilarse manualmente durante el desarrollo.

La CLI `wf bundle` **no debe compilarlo automáticamente**.

Debe recibir un launcher ya construido.

---

# 74. Launcher source

Inicialmente permitir:

```bash
wf bundle --launcher /path/to/wf-launcher
```

Más adelante:

```text
downloaded release launcher
embedded packaged launcher
```

---

# 75. Bundle format v1

Crear:

```text
docs/bundling/bundle-format-v1.md
```

El formato debe tener:

```text
native launcher
    +
payload
    +
fixed footer
```

El footer deberá contener:

```text
magic
format version
header size
payload offset
payload size
metadata offset
metadata size
checksum
flags
```

La implementación debe demostrar primero que se puede:

```text
append payload
→ read footer
→ locate payload
→ validate checksum
→ execute
```

---

# 76. Bundle footer

No depender de:

```text
ELF internals
Mach-O load commands
PE sections
```

para localizar el payload.

El launcher debe localizar el footer desde EOF.

Ejemplo conceptual:

```text
[launcher bytes]
[payload bytes]
[metadata]
[footer]
```

Esto permite que el mismo formato lógico funcione sobre:

```text
ELF
Mach-O
PE
```

sin reescribir cada formato binario.

---

# 77. POC obligatorio del bundle

Antes de implementar el sistema completo:

1. compilar un launcher local;
2. copiarlo;
3. añadir `hello.wasm`;
4. añadir manifest;
5. añadir footer;
6. ejecutar el binario;
7. verificar que encuentra el payload;
8. alterar un byte;
9. verificar que checksum falla.

No congelar `Bundle Format v1` hasta que este POC funcione.

---

# 78. macOS signing

El ciclo de bundle debe ser:

```text
launcher
↓
embed payload
↓
final binary
↓
codesign
↓
verify signature
```

Nunca:

```text
codesign launcher
↓
append payload
```

El binario que se firma debe ser el artefacto final.

La notarización quedará para el sistema de release posterior.

---

# 79. Cross compilation

Separar:

```text
guest target
```

de:

```text
bundle target
```

La generación de un bundle de:

```text
aarch64-unknown-linux-gnu
```

no debe obligar al usuario a disponer del toolchain de Rust si existe un launcher precompilado correspondiente.

Esta es una de las razones principales para usar launchers precompilados.

---

# 80. Launcher registry futuro

Preparar conceptualmente:

```text
LauncherRegistry
```

pero no convertirlo todavía en una arquitectura de plugins.

Modelo futuro:

```text
target triple
    ↓
launcher artifact
    ↓
sha256
    ↓
release URL
```

---

# 81. Noveno vertical slice: Component Model

Solo cuando:

```text
Core Wasm
runtime
multi-module
WASI
ABI
```

estén estables.

Agregar:

```text
wit-parser
wit-component
```

y soporte:

```text
component
world
interface
```

`wasm-tools` ya proporciona `wit-parser` y `wit-component`, y su tooling permite construir Components desde core Wasm cuando existe metadata WIT apropiada.

No utilizar `wasm-tools compose` como base de la arquitectura; actualmente está marcado como deprecated.

---

# 82. Core Module y Component son diferentes

No crear:

```rust
UniversalWasmRuntime
```

como abstracción artificial.

Mantener:

```text
CoreModuleRuntime
ComponentRuntime
```

compartiendo solamente conceptos que realmente son comunes.

---

# 83. Component Model no debe reescribir Core Module

Debe existir:

```text
CoreModule support
```

como base estable.

El Component Model será una expansión del sistema, no una sustitución inmediata.

---

# 84. Sistema de plugins

No implementar plugins externos durante la primera mitad del proyecto.

No implementar:

```text
dynamic Rust libraries
ABI C manual
libloading-based plugin API
```

La primera extensibilidad será interna:

```text
Toolchain trait
Runtime extension
Bundler backend
```

y solo cuando aparezcan implementaciones suficientes.

---

# 85. Plugin protocol futuro

Cuando exista una necesidad real de plugins externos, preferir:

```text
Wasm Component + WIT
```

o:

```text
versioned subprocess protocol
```

antes que una ABI dinámica de Rust.

El plugin debe tener capacidades explícitas.

---

# 86. No portar custom templates

Los templates Nunjucks personalizados del proyecto anterior están deliberadamente descartados.

La configuración anterior permitía `templatePath` para modificar el código generado.

En WasmFoundry:

```text
custom template
    → DROP
```

La funcionalidad que realmente proporcionaban debe resolverse mediante:

```text
runtime
ABI
bundle format
toolchain adapters
```

no mediante generación C++.

---

# 87. No portar la C-API de Wasmtime

Eliminar definitivamente el concepto:

```text
wasmtime-c-api cache
wasmtime headers
wasmtime library download
cmake integration
```

El proyecto anterior descargaba la C-API en `~/.wasm-linker/`; WasmFoundry no debe tener un paso equivalente.

---

# 88. `wf doctor`

Después de estabilizar toolchains:

```bash
wf doctor
```

debe detectar:

```text
Rust
Cargo
Rust target
Clang
C++ toolchain
AssemblyScript
Node
required WASI target
bundle launcher
host architecture
```

Debe diferenciar:

```text
required
optional
unsupported
missing
misconfigured
```

---

# 89. `wf check`

Debe validar:

```text
manifest
sources
toolchains
Wasm validity
imports
exports
dependency graph
entry point
runtime policy
target compatibility
```

No ejecutar código guest.

---

# 90. `wf inspect`

Debe ser seguro frente a WASM malformado.

No confiar en:

```text
size declarations
counts
offsets
names
custom sections
```

El parser debe rechazar datos inválidos correctamente.

---

# 91. Fuzzing

Cuando `wf-wasm` exista:

usar:

```text
cargo fuzz
wasm-smith
wasm-mutate
```

como herramientas de testing.

`wasm-tools` proporciona `wasm-smith`, `wasm-mutate` y `wasm-shrink`, entre otras herramientas de generación y manipulación para testing de WebAssembly.

Objetivos prioritarios:

```text
module parser boundary
imports
exports
types
custom sections
WIT parsing
bundle footer parser
manifest parser
```

---

# 92. Testing por capas

## Unit

```text
wf-core
graph
matching
fingerprints
policies
diagnostics
```

No usar runtime real.

## Integration

```text
wf-wasm + wasmparser
wf-runtime + Wasmtime
toolchain + real compiler
```

## End-to-end

```bash
wf init
wf build
wf run
wf bundle
```

## Fuzz

```text
binary parsing
config parsing
bundle metadata
```

---

# 93. Tests de contrato de Toolchain

Cuando exista más de un toolchain real:

```text
Rust
C++
AssemblyScript
```

compartir tests comunes:

```text
detect
compile
invalid input
diagnostics
artifact produced
entrypoint
```

Esto será preferible a duplicar 3 suites casi idénticas.

---

# 94. Fixtures

Crear:

```text
fixtures/
├── wasm/
│   ├── valid/
│   ├── invalid/
│   ├── imports/
│   ├── exports/
│   ├── memory/
│   ├── globals/
│   └── cycles/
│
├── projects/
│   ├── precompiled/
│   ├── rust/
│   ├── cpp/
│   └── assemblyscript/
│
└── bundles/
```

No descargar fixtures de Internet automáticamente durante los tests.

---

# 95. Golden tests

Usar snapshots/golden files para:

```text
wf inspect
wf check
wf doctor
diagnostics
bundle metadata
```

Debe existir representación:

```text
human
json
```

cuando ambas sean interfaces soportadas.

---

# 96. Diagnostics

Diseñar errores estructurados:

```text
code
severity
message
help
source
location
related
```

Ejemplos:

```text
WF001 invalid wasm
WF002 unresolved import
WF003 dependency cycle
WF004 missing toolchain
WF005 invalid entrypoint
WF006 denied capability
WF007 bundle corruption
WF008 unsupported target
WF009 runtime trap
```

Los códigos deben ser estables una vez publicados.

---

# 97. Logging

Usar `tracing`.

No crear un logger global propio.

Spans mínimos:

```text
build
compile
analyze
resolve
run
bundle
cache
toolchain
```

---

# 98. Process execution

Cuando aparezcan toolchains externas, introducir una abstracción mínima:

```rust
trait ProcessRunner {
    fn run(&self, command: CommandSpec) -> Result<CommandOutput>;
}
```

No crear una jerarquía de procesos.

Debe soportar:

```text
program
args
cwd
environment
stdin
stdout
stderr
exit code
duration
timeout
```

---

# 99. No crear `FileSystem` inicialmente

Usar filesystem real.

Usar:

```text
tempfile
```

en tests.

Introducir `FileSystem` solamente si aparecen:

```text
in-memory filesystem
sandbox
remote filesystem
virtual filesystem
```

como necesidades reales.

---

# 100. No crear `LogicalPath`

Usar:

```text
Path
PathBuf
```

o:

```text
camino
```

si aporta valor para paths UTF-8.

Crear tipos propios únicamente cuando exista un invariante que deba protegerse.

---

# 101. No crear `BuildExecutor` inicialmente

Primero:

```text
wf build
```

debe ejecutar directamente el pipeline.

Cuando exista:

```text
parallel build
cache
remote build
dry-run
```

se podrá extraer:

```text
BuildPlan
BuildExecutor
```

La abstracción deberá salir del caso de uso real.

---

# 102. No crear un `ArtifactStore` inicialmente

Primero guardar artifacts en el filesystem normal.

Cuando aparezca la cache:

```text
ArtifactStore
CacheStore
```

podrán separarse.

---

# 103. Build Plan

Cuando el build sea suficientemente complejo:

```text
discover
↓
resolve
↓
compile
↓
analyze
↓
graph
↓
runtime
↓
bundle
```

deberá representarse como un plan.

Pero el agente no debe implementar el plan formal antes de que exista una segunda necesidad del pipeline.

---

# 104. Paridad de AssemblyScript

El agente debe comprobar explícitamente:

```text
runtime:
incremental
minimal
stub
full

optimize:
0–3

shrink:
0–2

source maps
```

porque estas capacidades existen en la configuración legacy.

Cada una debe marcarse:

```text
preserve
replace
drop
```

con justificación.

---

# 105. Paridad de host API

El legacy expone funciones como:

```text
console.log
console.debug
console.info
console.warn
console.error
console.time
console.timeLog
console.timeEnd
console.assert
Math.*
Date.now
Performance.now
Process.exit
random seed
```

según la documentación actual.

El agente debe:

1. inventariarlas;
2. determinar cuáles son exclusivamente de AssemblyScript;
3. determinar cuáles pertenecen a una API genérica;
4. probarlas;
5. documentarlas;
6. evitar portar funciones innecesarias.

---

# 106. Paridad de Rust bindings

El crate Rust legacy es `no_std` y contiene:

```text
alloc
console
fs
wasi
```

con macro/infraestructura propia.

La nueva versión no debe simplemente copiar ese crate.

Primero determinar:

```text
qué parte es ABI
qué parte es SDK
qué parte es allocator
qué parte depende de wasm-apps
```

Después reconstruir solamente los contratos necesarios.

---

# 107. Build security

El sistema debe asumir que un proyecto fuente puede ejecutar código arbitrario mediante:

```text
build.rs
npm scripts
custom compiler scripts
CMake
shell commands
```

No prometer sandbox completo.

Documentar esta frontera claramente.

---

# 108. Runtime security

Sí debe existir:

```text
capability policy
WASI permissions
resource limits
timeouts
```

El runtime no debe habilitar automáticamente:

```text
network
filesystem
environment
processes
```

---

# 109. Supply chain

Antes de la primera release pública, introducir:

```text
cargo-audit
cargo-deny
```

y revisar:

```text
licenses
advisories
duplicate crates
sources
```

Después:

```text
SBOM
signed releases
checksums
```

No introducir esto durante el primer vertical slice.

---

# 110. Política de Wasmtime

El proyecto debe usar la versión estable seleccionada mediante Cargo y `Cargo.lock`.

Al 1 de octubre de 2026, la versión estable más reciente es:

```text
49.0.1
```

publicada el 24 de septiembre de 2026. Esa release incluye correcciones relacionadas con fuel, memoria host y `wasmtime-wasi`, entre otras.

La política será:

```text
Cargo.toml
    ↓
compatible version constraint
    ↓
Cargo.lock
    ↓
regression suite
```

No congelar manualmente el documento a una versión perpetua.

---

# 111. Política de upgrades

Actualizar Wasmtime cuando:

```text
security fix
bug relevante
feature necesaria
actualización periódica
```

pero siempre:

```text
upgrade
↓
cargo test
↓
runtime regression suite
↓
cross-platform validation
```

No depender de una periodicidad rígida si existe un advisory de seguridad urgente.

---

# 112. Gap management

Este es un requisito obligatorio del agente.

Cuando descubra algo que el documento no cubre:

## Paso 1

Clasificar:

```text
legacy parity gap
architecture gap
platform gap
toolchain gap
runtime gap
security gap
packaging gap
documentation gap
```

## Paso 2

Determinar:

```text
blocking current milestone?
```

## Paso 3

Si NO:

crear:

```text
docs/gaps/GAP-XXXX.md
```

## Paso 4

Si SÍ:

investigar y resolver antes de continuar.

## Paso 5

Si cambia una decisión arquitectónica:

crear ADR.

Nunca ocultar una desviación.

---

# 113. Formato de GAP

```markdown
# GAP-XXXX — <title>

## Contexto

## Cómo se descubrió

## Impacto

## ¿Bloquea el milestone?

## Opciones

## Decisión

## Consecuencias

## Tests afectados

## Documentación afectada
```

---

# 114. Formato de ADR

```markdown
# ADR-XXXX — <title>

## Estado
proposed | accepted | superseded

## Contexto

## Problema

## Alternativas

### A
### B
### C

## Decisión

## Consecuencias

## Reversibilidad

## Implementación
```

No crear ADRs para decisiones triviales.

---

# 115. Gaps especialmente esperados

El agente debe prestar especial atención a:

```text
1. ABI exacto de AssemblyScript
2. módulos Rust sin WASI
3. módulos Rust con WASI
4. firmas de imports
5. memoria compartida
6. globals
7. tables
8. ciclos de módulos
9. imports dinámicos
10. entrypoints no estándar
11. cross compilation
12. launcher prebuilt
13. firma macOS
14. Windows PE
15. toolchain C++
16. C++ WASI
17. AssemblyScript runtime variants
18. cache invalidation
19. component model
20. WIT versioning
```

---

# 116. Regla frente a incertidumbre técnica

Si una implementación tiene dos soluciones razonables:

```text
A
B
```

no inventar una tercera abstracción.

Elegir la solución más simple compatible con:

```text
seguridad
testabilidad
extensibilidad real
```

y registrar la decisión si afecta arquitectura pública.

---

# 117. Regla frente a sobreingeniería

El agente no debe introducir:

```text
factory
builder
repository
adapter
service
provider
registry
manager
```

solamente para seguir un patrón.

Cada abstracción debe responder:

```text
¿Qué responsabilidad separa?
¿Qué implementación alternativa permite?
¿Qué test facilita?
¿Por qué debe existir ahora?
```

---

# 118. Regla frente a subingeniería

Tampoco se permite:

```text
god object
global state
runtime singleton
CLI containing business logic
toolchain-specific conditionals everywhere
```

Si una responsabilidad empieza a expandirse demasiado, registrar un gap o ADR y extraerla.

---

# 119. Política de commits

Usar:

```text
tipo(scope): mensaje en español
```

Ejemplos:

```text
chore(repo): preservar estado legacy
chore(repo): reiniciar main para WasmFoundry

feat(core): implementar grafo de dependencias
feat(wasm): analizar módulos con wasmparser
feat(runtime): ejecutar módulos core con Wasmtime
feat(toolchains): añadir toolchain Rust
feat(abi): implementar console host ABI
feat(bundle): añadir formato de bundle v1

test(runtime): cubrir traps y entrypoints
docs(architecture): documentar fronteras iniciales
```

Cada milestone debe producir commits pequeños y coherentes.

---

# 120. Gates obligatorios

No pasar de milestone si no se cumple su gate.

## Gate 0 — Git

```text
legacy creada
legacy-v1 creada
legacy remota verificada
main limpio
```

## Gate 1 — Runtime

```text
wf inspect
wf run
```

## Gate 2 — Project

```text
wf init
wf build
wf run
```

## Gate 3 — Rust

```text
Rust guest
→ wasm
→ run
```

## Gate 4 — Multi-module

```text
A → B
```

## Gate 5 — ABI

```text
console
WASI básico
```

## Gate 6 — C++

```text
C++ guest
→ wasm
→ run
```

## Gate 7 — AssemblyScript

```text
AS guest
→ wasm
→ run
```

## Gate 8 — Bundle

```text
wf bundle
./dist/app
```

## Gate 9 — Cache

```text
cold miss
warm hit
invalidación correcta
```

## Gate 10 — Components

```text
WIT
component
run
```

## Gate 11 — CI

```text
GitHub Actions
test matrix
build matrix
release dry run
```

---

# 121. CI/CD se reconstruye al final

Durante toda la reescritura:

```text
.github/workflows/
```

debe permanecer inexistente.

No añadir workflows de conveniencia.

La calidad se valida localmente mediante:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

hasta que CI sea reintroducida.

---

# 122. Nueva CI

Cuando el producto tenga:

```text
runtime
toolchains
bundle
tests
```

crear CI desde cero.

Primera fase:

```text
Linux x86_64
Rust
unit
integration
E2E
```

Luego:

```text
macOS
Windows
```

y finalmente:

```text
release matrix
```

---

# 123. CI mínima final

La nueva CI deberá verificar como mínimo:

```text
cargo fmt
cargo check
cargo test
cargo clippy
integration
E2E
```

sin:

```text
pnpm
Node
CMake
Nunjucks
Wasmtime C API
```

salvo que una toolchain concreta requiera una dependencia externa.

---

# 124. CI multi-toolchain

Cuando haya toolchains:

```text
Rust
C++
AssemblyScript
```

la matriz deberá instalar solamente las dependencias correspondientes a cada job.

No hacer que todos los jobs instalen todo.

---

# 125. Release pipeline

Solamente después de que:

```text
wf bundle
```

sea estable.

El release deberá producir:

```text
launcher Linux x64
launcher Linux ARM64
launcher macOS x64
launcher macOS ARM64
launcher Windows x64
```

más:

```text
checksums
SBOM
release metadata
```

---

# 126. El launcher se convierte en artefacto de release

Cada release deberá asociar:

```text
target triple
launcher version
Wasmtime version
bundle format version
checksum
```

La CLI podrá entonces seleccionar automáticamente:

```text
host target
    ↓
launcher artifact
```

---

# 127. Versiones independientes

Mantener conceptos separados:

```text
WasmFoundry version
Host ABI version
Bundle format version
Manifest schema version
```

Ejemplo:

```text
WasmFoundry 0.5
Host ABI v1
Bundle v1
Manifest schema v1
```

Una release del CLI no debe implicar automáticamente una ruptura de todos los formatos.

---

# 128. Host ABI compatibility

Regla:

```text
ABI v1 additions
```

pueden ser compatibles.

Pero:

```text
signature incompatible
memory layout incompatible
ownership incompatible
```

requiere:

```text
ABI v2
```

No modificar silenciosamente ABI v1.

---

# 129. Bundle compatibility

El launcher debe rechazar explícitamente:

```text
unsupported format version
```

No intentar interpretar un formato desconocido.

Debe mostrar:

```text
Bundle format version X is not supported by this launcher
```

---

# 130. Manifest compatibility

El manifest debe comenzar con:

```toml
schema = 1
```

Si aparece:

```text
schema = 2
```

y la versión no lo soporta:

```text
error claro
```

No interpretar parcialmente.

---

# 131. README final

El README nuevo no debe describir:

```text
wasm-apps
wapp
Nunjucks
CMake
TypeScript
```

como sistema vigente.

Debe explicar:

```text
qué es WasmFoundry
por qué existe
cómo se instala
wf init
wf build
wf run
wf inspect
wf bundle
arquitectura
toolchains
runtime
seguridad
estado del proyecto
```

---

# 132. Propuesta de valor que debe quedar clara

No presentar WasmFoundry como:

```text
"otro runtime WebAssembly"
```

ni como:

```text
"otro compilador WASM"
```

La propuesta debe ser:

```text
multi-toolchain
+
multi-module composition
+
host ABI
+
capability-based runtime
+
standalone native bundles
```

---

# 133. Documentación mínima final

Antes de `v0.1.0` deben existir:

```text
README.md
docs/product.md
docs/architecture.md
docs/parity-matrix.md
docs/abi/host-abi-v1.md
docs/security/threat-model.md
docs/bundling/bundle-format-v1.md
```

ADR solo para decisiones relevantes.

---

# 134. `v0.1.0`

No esperar a terminar:

```text
plugins
components
cross compilation completa
cache sofisticada
```

para publicar.

`v0.1.0` debe ser alcanzable cuando exista:

```text
wf inspect
wf run
wf build
precompiled
Rust
basic host ABI
```

y la documentación sea coherente.

---

# 135. Criterios mínimos de `v0.1.0`

Debe poder ejecutarse:

```bash
wf init hello
cd hello
wf build
wf run
```

y:

```bash
wf inspect target/hello.wasm
```

Además:

```text
Rust toolchain funcional
precompiled toolchain funcional
core module runtime
tests
documentación
```

---

# 136. No llamar "release" a una versión que dependa de herramientas internas no documentadas

Antes de publicar:

```text
wf
```

el usuario debe saber:

```text
qué necesita instalar
qué toolchains necesita
qué soporta
qué no soporta
```

No esconder dependencias.

---

# 137. Criterios de calidad antes de declarar la reescritura terminada

No aceptar el proyecto como terminado si existen:

```text
generated C++
Nunjucks
cmake-js
Wasmtime C API
custom WASM parser
global singleton
legacy package.json
pnpm lock
legacy CI
legacy AGENTS.md
undocumented ABI
undocumented bundle format
silent compatibility changes
```

---

# 138. Comprobación final de contaminación legacy

Ejecutar búsquedas:

```bash
git grep -n "wasm-apps"
git grep -n "wapp"
git grep -n "Nunjucks"
git grep -n "cmake-js"
git grep -n "wasmtime-c-api"
git grep -n "@wasm-apps"
git grep -n "pnpm"
```

Cada resultado debe clasificarse.

Los únicos resultados aceptables normalmente serán:

```text
docs/legacy references
parity matrix
migration history
legacy documentation
```

Nunca código activo.

---

# 139. Comprobación final de arquitectura

Verificar:

```text
wf-core
    no Wasmtime
    no wasmparser
    no CLI

wf-wasm
    no CLI logic

wf-runtime
    no filesystem build orchestration
    no Cargo invocation

wf-toolchains
    no runtime implementation

wf-bundle
    no source compilation

wf-cli
    no parser de WebAssembly
```

---

# 140. Comprobación final de extensibilidad

Debe ser posible añadir:

```text
nuevo toolchain
```

sin modificar:

```text
dependency graph
runtime internals
CLI core
existing toolchains
```

Debe ser posible añadir:

```text
nueva runtime capability
```

sin modificar:

```text
toolchain compilation logic
```

Debe ser posible añadir:

```text
nuevo bundle target
```

sin modificar:

```text
Wasm parser
toolchain compilation
```

---

# 141. Comprobación final de seguridad

Debe poder demostrarse:

```text
WASM sin filesystem → falla correctamente
WASM con filesystem permitido → funciona
WASM con mount inexistente → error claro
WASM con timeout → termina
WASM con trap → diagnóstico
bundle corrupto → rechazo
manifest inválido → rechazo
```

---

# 142. Comprobación final de reproducibilidad

Mismo proyecto:

```text
build 1
build 2
```

debe producir:

```text
mismos artifacts lógicos
```

cuando las entradas y toolchain sean las mismas.

Registrar diferencias si no ocurre.

No asumir reproducibilidad sin medirla.

---

# 143. Comprobación final de performance

Antes de release establecer baseline para:

```text
wf inspect startup
wf run startup
cold build
warm build
bundle creation
bundle startup
```

No fijar metas numéricas arbitrarias antes de medir.

Después documentar:

```text
baseline
hardware
OS
Rust version
Wasmtime version
artifact sizes
```

---

# 144. Comprobación final del bundle

El bundle debe:

```text
ser ejecutable
contener payload
validar checksum
funcionar sin wasm externo
rechazar payload corrupto
mostrar error ante formato desconocido
```

Y en macOS:

```text
payload final
↓
codesign
↓
verify
```

---

# 145. Comprobación final del repositorio

El `main` final debe contener:

```text
Rust
docs
fixtures
examples
tests
license
README
AGENTS
```

y no:

```text
TypeScript
pnpm
npm
Nunjucks
CMake integration legacy
C-API setup
legacy workflows
```

---

# 146. Comprobación final de Git

Debe quedar:

```text
main
    ↓
WasmFoundry nuevo

legacy
    ↓
último wasm-apps original

legacy-v1
    ↓
tag exacto del estado original
```

Verificar:

```bash
git log --oneline --decorate --all --graph
```

---

# 147. Estado esperado de ramas

Conceptualmente:

```text
* main
  |
  |--- WasmFoundry commits
  |
  o--- legacy
       |
       |--- último wasm-apps TypeScript
```

No mergear `legacy` de vuelta a `main`.

La relación histórica ya existe porque la rama comparte el mismo repository.

---

# 148. Regla sobre el código legacy

No borrar la rama `legacy`.

No renombrarla.

No compactar su historial.

No migrar commits selectivamente.

No copiar implementation code salvo que una especificación necesite reproducirse y, aun entonces, reimplementarlo en Rust.

---

# 149. Secuencia exacta de implementación

El agente debe seguir este orden:

```text
PHASE 0
Git preservation

PHASE 1
Main reset + CI/CD removal

PHASE 2
Rust workspace

PHASE 3
wf inspect

PHASE 4
wf run

PHASE 5
wf init/build con precompiled WASM

PHASE 6
Rust toolchain

PHASE 7
Dependency graph + module matching

PHASE 8
Host ABI v1

PHASE 9
WASI + mounts + capability policy

PHASE 10
C++ toolchain

PHASE 11
AssemblyScript toolchain

PHASE 12
Cache

PHASE 13
Watch

PHASE 14
Bundle POC

PHASE 15
Bundle format v1 + prebuilt launcher

PHASE 16
Cross-target launchers

PHASE 17
Component Model + WIT

PHASE 18
External extensibility

PHASE 19
Security tooling + supply chain

PHASE 20
CI/CD

PHASE 21
Release v0.1.0
```

No saltarse fases que contengan un gate pendiente.

---

# 150. Regla de implementación vertical

En cada fase:

```text
1. definir comportamiento
2. implementar mínima estructura
3. implementar happy path
4. implementar errores
5. añadir tests
6. validar manualmente
7. actualizar docs
8. registrar gaps
9. commit
```

No implementar diez capas antes del primer comportamiento observable.

---

# 151. Regla de no bloqueo

El agente no debe detenerse porque haya una decisión secundaria no resuelta.

Si encuentra una decisión:

```text
no bloqueante
```

debe:

```text
documentarla
elegir la alternativa más simple
continuar
```

Solo debe detener el milestone si:

```text
seguridad
integridad de datos
destrucción de Git
ABI irreversible
bundle format irreversible
```

requieren una decisión que no pueda inferirse de la evidencia.

---

# 152. Regla de evidencia

Cuando algo ya existe en `legacy`:

```text
legacy code
legacy tests
legacy examples
legacy docs
```

usarlo como evidencia.

Cuando una decisión depende de una librería externa:

```text
documentation
source
tests
```

usarla como evidencia.

No inferir comportamiento crítico por nombres.

---

# 153. Estado inicial final esperado

Después de las fases Git + reset:

```text
legacy branch
    = old project

main
    = almost-empty Rust skeleton
```

Después del primer slice:

```text
main
    = wf inspect
    = wf run
```

Después de build:

```text
main
    = wf init
    = wf build
    = wf run
```

Después de toolchains:

```text
main
    = Rust
    = C++
    = AssemblyScript
```

Después de bundle:

```text
main
    = standalone executable
```

Después de CI:

```text
main
    = reproducible project
    = tested
    = releasable
```

---

# 154. Definición final de éxito

La reescritura se considera terminada cuando:

```text
[ ] main ya no contiene implementación TypeScript legacy
[ ] legacy conserva el estado original
[ ] legacy-v1 apunta al commit original
[ ] no existen workflows legacy
[ ] no existen scripts de release npm legacy
[ ] Cargo workspace funciona
[ ] wf inspect funciona
[ ] wf run funciona
[ ] wf build funciona
[ ] precompiled toolchain funciona
[ ] Rust toolchain funciona
[ ] dependency graph funciona
[ ] moduleMatching está definido
[ ] Host ABI v1 está documentado
[ ] Guest SDKs relevantes están documentados
[ ] WASI funciona
[ ] mounts funcionan
[ ] runtime policy funciona
[ ] C++ funciona
[ ] AssemblyScript funciona
[ ] custom templates fueron descartados explícitamente
[ ] parser legacy fue eliminado
[ ] Nunjucks fue eliminado
[ ] CMake linker legacy fue eliminado
[ ] Wasmtime C API fue eliminada
[ ] bundle format v1 está documentado
[ ] launcher prebuilt funciona
[ ] bundle checksum funciona
[ ] cache funciona o está conscientemente diferida
[ ] watch funciona o está conscientemente diferido
[ ] Component Model está integrado o explícitamente fuera de v0.1
[ ] threat model documentado
[ ] runtime security testeada
[ ] fuzzing inicial realizado
[ ] cargo-audit/cargo-deny configurados antes de release
[ ] CI nueva funciona
[ ] release pipeline nuevo funciona
[ ] documentación no contradice la implementación
```

---

# 155. Principio final para el agente

La prioridad de implementación es:

```text
CORRECCIÓN
    >
CLARIDAD
    >
SEPARACIÓN DE RESPONSABILIDADES
    >
EXTENSIBILIDAD REAL
    >
PERFORMANCE
    >
MICRO-OPTIMIZACIÓN
```

No construir un framework de plugins antes del primer toolchain.

No construir un sistema de cache antes de medir.

No construir un build planner complejo antes de tener builds reales.

No construir Component Model antes de estabilizar Core Wasm.

No construir CI/CD hasta que el flujo local sea reproducible.

No reconstruir el sistema anterior.

Construir **WasmFoundry**.

---

# 156. Resumen operativo de una sola página

El agente debe ejecutar esencialmente esto:

```text
1. Verificar worktree limpio.
2. Actualizar main.
3. Crear branch `legacy`.
4. Crear tag `legacy-v1`.
5. Push de ambos.
6. Cambiar checkout original a `legacy`.
7. Crear worktree separado para `main`.
8. Eliminar todo el contenido legacy de `main`.
9. Eliminar CI/CD y toolchain Node.
10. Crear Cargo workspace mínimo.
11. Crear wf-core.
12. Crear wf-wasm.
13. Crear wf-runtime.
14. Crear wf-cli.
15. Implementar `wf inspect`.
16. Implementar `wf run`.
17. Implementar `wf init`.
18. Implementar `wf build` para precompiled.
19. Añadir `Toolchain`.
20. Añadir Rust.
21. Añadir dependency graph.
22. Añadir module matching.
23. Diseñar Host ABI v1.
24. Implementar capacidades runtime.
25. Añadir WASI.
26. Añadir mounts.
27. Añadir C++.
28. Añadir AssemblyScript.
29. Añadir tests de contrato.
30. Añadir cache solamente después de medir.
31. Añadir watch.
32. Prototipar bundle.
33. Congelar Bundle Format v1.
34. Añadir launchers prebuilt.
35. Añadir Component Model.
36. Añadir extensibilidad externa cuando exista un caso real.
37. Añadir supply-chain tooling.
38. Crear CI nueva.
39. Crear release pipeline.
40. Publicar `v0.1.0`.
```

Este orden no es negociable salvo que un gap documentado demuestre técnicamente que una fase debe dividirse o reordenarse.

