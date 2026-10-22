// The `wf build` command.
//
// It reads the manifest, resolves each module to a toolchain and reports what each one
// produced. No compiler knowledge lives here: which toolchain handles a manifest value,
// and how it is invoked, belong to [`wf_toolchains::toolchains`].
//
// When `--watch` is provided, the command watches the project directory for changes
// and rebuilds automatically.

use notify::Watcher;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use wf_core::{Diagnostic, DiagnosticCode, ResolveError, Severity, ToolchainId, resolve_module};
use wf_toolchains::cache::{self, Cache, Entry};
use wf_toolchains::process::StdRunner;
use wf_toolchains::toolchains::{self, CompileRequest};

use crate::cli::BuildArgs;
use crate::manifest::MANIFEST_FILE;
use crate::project::{self, ImportSummary};
use crate::{EXIT_DIAGNOSTIC, EXIT_OK};

/// The Host ABI version label for fingerprinting (`plan.md` #68).
/// See `docs/abi/host-abi-v1.md` for the contract.
const HOST_ABI_LABEL: &str = "abi-v1";

/// Runs `wf build` and returns the process exit code.
pub fn run(args: &BuildArgs) -> i32 {
    // Being outside a project is the same problem for `build` and for `run`: the caller
    // is in the wrong directory. It gets the usage exit code in both, rather than a
    // diagnostic that suggests the manifest itself is broken.
    if !Path::new(MANIFEST_FILE).exists() {
        eprintln!("wf: no {MANIFEST_FILE} in this directory");
        eprintln!("  help: run `wf init <name>` to create a project, then `wf build` inside it");
        return crate::EXIT_USAGE;
    }

    // Perform an initial build.
    if let Err(code) = do_build() {
        return code;
    }

    // If watch mode is not requested, we are done.
    if !args.watch {
        return EXIT_OK;
    }

    // Watch mode: set up a file system watcher and rebuild on changes.
    println!("wf: watching for changes (press Ctrl+C to stop)");

    let (tx, rx) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("wf: could not create watcher: {e}");
            return EXIT_DIAGNOSTIC;
        }
    };

    // Watch the current directory recursively.
    if let Err(e) = watcher.watch(Path::new("."), notify::RecursiveMode::Recursive) {
        eprintln!("wf: could not watch directory: {e}");
        return EXIT_DIAGNOSTIC;
    }

    // Rebuild only when relevant events arrived, and only after the event storm of one
    // save operation settled: one build per pause in the events, none otherwise.
    let debounce = Duration::from_millis(500);
    let mut pending_changes = false;
    let mut last_event = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => {
                // Filter out uninteresting events.
                if should_ignore_event(&event) {
                    continue;
                }
                pending_changes = true;
                last_event = Instant::now();
            }
            Ok(Err(e)) => {
                eprintln!("wf: watcher error: {e}");
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {
                if pending_changes && last_event.elapsed() >= debounce {
                    // A broken build must not kill the watch loop: the diagnostic is on
                    // screen, the user edits a fix, and the next change rebuilds.
                    pending_changes = false;
                    last_event = Instant::now();
                    if let Err(_code) = do_build() {
                        continue;
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    EXIT_OK
}

/// Perform a single build, returning an exit code (OK on success).
fn do_build() -> Result<(), i32> {
    let manifest = match crate::manifest::read(Path::new(MANIFEST_FILE)) {
        Ok(manifest) => manifest,
        Err(diagnostic) => return Err(report(&[diagnostic])),
    };

    let diagnostics = manifest.validate();
    if !diagnostics.is_empty() {
        return Err(report(&diagnostics));
    }

    let root = Path::new(".");
    let runner = StdRunner;

    // Read the manifest file for fingerprinting. The normal error path above already
    // handles a truly unreadable file, so we treat a second read failure as empty bytes.
    let manifest_raw = std::fs::read(Path::new(MANIFEST_FILE)).unwrap_or_default();

    // Load the existing cache (corrupt = empty = rebuild everything).
    let cache = Cache::load(root);

    // Pre-loop fingerprints: the cache decisions of this run are made against the
    // project as it exists before any toolchain runs below.
    let own_fps = compute_own_fps(&manifest, root, &runner, &manifest_raw);

    let mut published: Vec<(String, PathBuf)> = Vec::new();
    let mut cache_updated = Cache::default();
    let mut anything_rebuilt = false;

    for module in &manifest.modules {
        let id = match ToolchainId::new(&module.toolchain) {
            Ok(id) => id,
            Err(error) => {
                return Err(report(&[Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    format!("module `{}`: {error}", module.name),
                )]));
            }
        };

        let Some(toolchain) = toolchains::lookup(&id) else {
            return Err(report(&[toolchains::unknown_toolchain(module, &id)]));
        };

        let request = CompileRequest::new(module, root, &runner);

        // Cache skip: entry present, fingerprint match, artifact exists, deps fresh.
        let artifact_path = root.join("target").join(format!("{}.wasm", module.name));
        if skip_entry(&cache, &module.name, &own_fps, &artifact_path) {
            println!(
                "wf: cached {} (unchanged) -> {}",
                module.name,
                artifact_path.display()
            );
            published.push((module.name.clone(), artifact_path));
            // Preserve the cached entry.
            if let Some(cached_entry) = cache.entries.get(&module.name) {
                cache_updated
                    .entries
                    .insert(module.name.clone(), cached_entry.clone());
            }
            continue;
        }

        let detection = toolchain.detect(&request);
        if !detection.is_supported() {
            return Err(report(&[Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!(
                    "module `{}`: {}",
                    module.name,
                    detection
                        .reason()
                        .unwrap_or("the source does not fit this toolchain")
                ),
            )
            .with_help("check the `source` field of this [[module]]")]));
        }

        match toolchain.compile(&request) {
            Ok(result) => {
                println!(
                    "wf: built {} ({}) via {} -> {}\n     {}",
                    result.artifact().id(),
                    result.artifact().kind(),
                    toolchain.id(),
                    result.path().display(),
                    result.detail()
                );
                published.push((module.name.clone(), result.path().to_path_buf()));
                anything_rebuilt = true;

                // Record dependencies from the published artifact.
                let own_fp = own_fps[&module.name].clone();
                let dep_list = record_deps(&manifest, &module.name, result.path(), &own_fps);

                cache_updated.entries.insert(
                    module.name.clone(),
                    Entry {
                        fingerprint: own_fp,
                        deps: dep_list,
                    },
                );
            }
            Err(diagnostic) => return Err(report(&[diagnostic])),
        }
    }

    // Once every module exists, check the set: an import that names two project modules
    // cannot be bound, and a cycle cannot be instantiated. Both are manifest problems,
    // and saying so here is cheaper than saying it the first time somebody runs it.
    if let Err(diagnostic) = check_links(&manifest, &published) {
        return Err(report(&[diagnostic]));
    }

    // Persist the cache only after a fully successful build. The entries this build
    // produced (with the toolchains' side effects, like a generated Cargo.lock) are
    // re-fingerprinted against the project as it exists NOW: a fingerprint recorded
    // before the toolchain ran would describe inputs that no longer exist and force a
    // spurious rebuild on the next run.
    if anything_rebuilt {
        let settled = compute_own_fps(&manifest, root, &runner, &manifest_raw);
        for (name, entry) in &mut cache_updated.entries {
            if let Some(fp) = settled.get(name) {
                entry.fingerprint = fp.clone();
                for (dep, dep_fp) in &mut entry.deps {
                    if let Some(fresh) = settled.get(dep) {
                        *dep_fp = fresh.clone();
                    }
                }
            }
        }
        cache_updated.save(root);
    }

    println!(
        "wf: built {} module(s) into {}/",
        published.len(),
        Path::new("target").display()
    );
    Ok(())
}

/// The fresh-state fingerprint of every module.
///
/// Inputs are the manifest bytes, the Host ABI label, the WasmFoundry version, the
/// toolchain's invocation contract, the module name and the bytes of every file the
/// toolchain's adapter claims as source. A module with an unreadable source gets an
/// empty fingerprint, which never matches a stored one (force rebuild).
fn compute_own_fps(
    manifest: &wf_core::Manifest,
    root: &Path,
    runner: &StdRunner,
    manifest_raw: &[u8],
) -> BTreeMap<String, String> {
    let mut own_fps: BTreeMap<String, String> = BTreeMap::new();
    for module in &manifest.modules {
        let id = match ToolchainId::new(&module.toolchain) {
            Ok(id) => id,
            Err(_) => {
                own_fps.insert(module.name.clone(), String::new());
                continue;
            }
        };

        let Some(toolchain) = toolchains::lookup(&id) else {
            own_fps.insert(module.name.clone(), String::new());
            continue;
        };

        let request = CompileRequest::new(module, root, runner);
        let source_paths = toolchain.source_paths(&request);

        let cache_key = toolchain.cache_key();
        let mut parts: Vec<&[u8]> = vec![
            manifest_raw,
            HOST_ABI_LABEL.as_bytes(),
            env!("CARGO_PKG_VERSION").as_bytes(),
            cache_key.as_bytes(),
            module.name.as_bytes(),
        ];

        let mut source_bytes_ok = true;
        let mut source_parts: Vec<Vec<u8>> = Vec::new();
        for src_path in &source_paths {
            match std::fs::read(src_path) {
                Ok(bytes) => source_parts.push(bytes),
                Err(_) => {
                    source_bytes_ok = false;
                    break;
                }
            }
        }

        if source_bytes_ok {
            for sb in &source_parts {
                parts.push(sb);
            }
            own_fps.insert(module.name.clone(), cache::hash_hex(&parts));
        } else {
            own_fps.insert(module.name.clone(), String::new());
        }
    }
    own_fps
}

/// Determines whether a module can be skipped because the cache already has a fresh copy.
///
/// Returns `true` when the cache entry exists, the fingerprint matches, the artifact
/// file is present, and all recorded dependencies are still at their recorded fingerprint.
/// Any of those conditions failing means the module must be rebuilt.
fn skip_entry(
    cache: &Cache,
    module_name: &str,
    own_fps: &BTreeMap<String, String>,
    artifact_path: &Path,
) -> bool {
    let Some(cached_entry) = cache.entries.get(module_name) else {
        return false;
    };
    let Some(own_fp) = own_fps.get(module_name) else {
        return false;
    };
    if cached_entry.fingerprint != *own_fp {
        return false;
    }
    if !artifact_path.is_file() {
        return false;
    }
    cached_entry.deps_are_fresh(own_fps)
}

/// Reads the published artifact and records which other project modules it depends on.
///
/// Only namespaces that resolve to another project module (via `resolve_module`) become
/// entries in the dependency list. Namespaces that fail resolution are left out —
/// `check_links` reports them as errors anyway.
fn record_deps(
    manifest: &wf_core::Manifest,
    module_name: &str,
    artifact_path: &Path,
    own_fps: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let Ok(bytes) = std::fs::read(artifact_path) else {
        return Vec::new();
    };
    let Ok(analysis) = wf_wasm::analyze(&bytes) else {
        return Vec::new();
    };

    let namespaces = analysis
        .core()
        .map(|core| {
            core.imports()
                .iter()
                .map(|import| import.module().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mode = manifest.project.module_matching;
    let specs = &manifest.modules;
    let mut deps = Vec::new();

    for namespace in namespaces {
        match resolve_module(mode, specs, &namespace) {
            Ok(target) => {
                // Only record dependencies on other modules (not self).
                if target.name != module_name
                    && let Some(target_fp) = own_fps.get(&target.name)
                {
                    deps.push((target.name.clone(), target_fp.clone()));
                }
            }
            Err(ResolveError::NotFound { .. }) => {
                // External namespace — not a project dependency.
            }
            Err(ResolveError::Ambiguous { .. }) => {
                // Ambiguous: check_links will report this; skip from deps.
            }
        }
    }

    deps
}

/// Resolves every published module's imports against the rest of the project.
///
/// What a module imports is read from the artifact rather than from its source: the
/// artifact is what will be linked, so it is the only evidence that matters.
fn check_links(
    manifest: &wf_core::Manifest,
    published: &[(String, PathBuf)],
) -> Result<(), Diagnostic> {
    let mut imports = Vec::with_capacity(published.len());

    for (name, path) in published {
        let bytes = std::fs::read(path).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ToolchainFailed,
                Severity::Error,
                format!("cannot read the published module `{name}`: {error}"),
            )
            .with_help("re-run `wf build`")
        })?;

        let analysis = wf_wasm::analyze(&bytes).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::InvalidWasm,
                Severity::Error,
                format!("module `{name}`: {error}"),
            )
            .with_help("the published module is not a valid WebAssembly module")
        })?;

        let namespaces = analysis
            .core()
            .map(|core| {
                core.imports()
                    .iter()
                    .map(|import| import.module().to_owned())
                    .collect()
            })
            .unwrap_or_default();

        imports.push(ImportSummary::new(name.clone(), namespaces));
    }

    project::plan(
        &manifest.modules,
        &imports,
        manifest.project.module_matching,
        &crate::project::allowed_host_namespaces(&manifest.project.host_namespaces),
    )
    .map(|_| ())
}

/// Prints every diagnostic and returns the diagnostic exit code.
fn report(diagnostics: &[Diagnostic]) -> i32 {
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}");
    }
    EXIT_DIAGNOSTIC
}

/// Returns true if the event should be ignored (e.g., changes in target directory,
/// temporary files, etc.).
fn should_ignore_event(event: &notify::Event) -> bool {
    // Ignore events that are not relevant to the build (e.g., changes in target/).
    for path in &event.paths {
        let path_str = path.to_string_lossy();
        if path_str.contains("/target/") {
            return true;
        }
        // Ignore common temporary files.
        if path_str.ends_with(".swp") || path_str.ends_with(".tmp") || path_str.ends_with(".bak") {
            return true;
        }
        // Ignore hidden directories like .git, .atl, .codegraph (optional).
        if path_str.contains("/.git/")
            || path_str.contains("/.atl/")
            || path_str.contains("/.codegraph/")
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn event(path: &str) -> notify::Event {
        notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Any,
        )))
        .add_path(PathBuf::from(path))
    }

    /// A write of the published artifact must never trigger a rebuild of itself.
    #[test]
    fn the_build_output_never_triggers_a_rebuild() {
        assert!(should_ignore_event(&event("./target/hello.wasm")));
    }

    /// A source change is exactly why the watch loop exists.
    #[test]
    fn a_source_change_is_relevant() {
        assert!(!should_ignore_event(&event("./src/hello.rs")));
    }

    /// Editor swap files, backups and VCS internals are never interesting.
    #[test]
    fn editor_and_metadata_noise_is_ignored() {
        for path in ["./src/hello.rs.swp", "./src/hello.rs.bak", "./.git/index"] {
            assert!(should_ignore_event(&event(path)), "{path}");
        }
    }

    // ---- Cache skip_entry unit tests ----

    fn make_cache(entry_module: &str, entry_fp: &str) -> Cache {
        let mut c = Cache::default();
        c.entries.insert(
            entry_module.to_owned(),
            Entry {
                fingerprint: entry_fp.to_owned(),
                deps: Vec::new(),
            },
        );
        c
    }

    fn make_fps(module: &str, fp: &str) -> BTreeMap<String, String> {
        let mut fps = BTreeMap::new();
        fps.insert(module.to_owned(), fp.to_owned());
        fps
    }

    #[test]
    fn skip_entry_no_cache_entry_returns_false() {
        let dir = std::env::temp_dir().join(format!("wf-skip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("target")).unwrap();

        let cache = Cache::default();
        let fps = make_fps("hello", "abc");
        let artifact = dir.join("target/hello.wasm");
        assert!(!skip_entry(&cache, "hello", &fps, &artifact));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skip_entry_stale_fingerprint_returns_false() {
        let dir = std::env::temp_dir().join(format!("wf-skip-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("target")).unwrap();

        let cache = make_cache("hello", "old-fp");
        let fps = make_fps("hello", "new-fp");
        let artifact = dir.join("target/hello.wasm");
        std::fs::write(&artifact, b"\0").unwrap();

        assert!(!skip_entry(&cache, "hello", &fps, &artifact));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skip_entry_fresh_fingerprint_missing_artifact_returns_false() {
        let dir = std::env::temp_dir().join(format!("wf-skip-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("target")).unwrap();

        let cache = make_cache("hello", "same-fp");
        let fps = make_fps("hello", "same-fp");
        let artifact = dir.join("target/hello.wasm");
        // Artifact does not exist.

        assert!(!skip_entry(&cache, "hello", &fps, &artifact));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skip_entry_fresh_fingerprint_existing_artifact_fresh_deps_returns_true() {
        let dir = std::env::temp_dir().join(format!("wf-skip-fresh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("target")).unwrap();

        let cache = make_cache("hello", "same-fp");
        let fps = make_fps("hello", "same-fp");
        let artifact = dir.join("target/hello.wasm");
        std::fs::write(&artifact, b"\0").unwrap();

        assert!(skip_entry(&cache, "hello", &fps, &artifact));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skip_entry_dep_changed_returns_false() {
        let dir = std::env::temp_dir().join(format!("wf-skip-dep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("target")).unwrap();

        let mut cache = Cache::default();
        cache.entries.insert(
            "hello".to_owned(),
            Entry {
                fingerprint: "same-fp".to_owned(),
                deps: vec![("helper".to_owned(), "old-helper-fp".to_owned())],
            },
        );

        let mut fps = BTreeMap::new();
        fps.insert("hello".to_owned(), "same-fp".to_owned());
        fps.insert("helper".to_owned(), "new-helper-fp".to_owned());

        let artifact = dir.join("target/hello.wasm");
        std::fs::write(&artifact, b"\0").unwrap();

        assert!(!skip_entry(&cache, "hello", &fps, &artifact));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
