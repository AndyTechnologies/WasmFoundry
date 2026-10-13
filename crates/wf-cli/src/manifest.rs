//! Reading `wasmfoundry.toml`.
//!
//! Parsing lives here rather than in `wf-core`: the domain crate has no dependencies and
//! no business knowing what a TOML table looks like. What this module produces is plain
//! data, and the rules about whether that data is *allowed* belong to
//! [`Manifest::validate`](wf_core::Manifest::validate).

use std::path::Path;

use serde::Deserialize;
use wf_core::{Diagnostic, DiagnosticCode, Manifest, ModuleSpec, Package, Project, Severity};

/// The file a project is described by.
pub const MANIFEST_FILE: &str = "wasmfoundry.toml";

/// What the file looks like, before it is a manifest.
///
/// Unknown fields are rejected rather than ignored: a typo in a table name would
/// otherwise silently do nothing, and a project that reads as configured but is not is
/// exactly the failure `wf build` must not produce.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    schema: u32,
    package: PackageFile,
    project: ProjectFile,
    #[serde(default)]
    module: Vec<ModuleFile>,
    /// Directories the guest may see. Denied unless a project asks for them.
    #[serde(default)]
    r#mount: Vec<MountFile>,
    #[serde(default)]
    runtime: RuntimeFile,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MountFile {
    host: String,
    guest: String,
    /// Parsed as text and interpreted by the domain.
    #[serde(default)]
    access: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RuntimeFile {
    #[serde(default)]
    wasi: Option<bool>,
    /// Seconds, as a float: half a second is a legitimate budget.
    #[serde(default)]
    wall_clock_seconds: Option<f64>,
    #[serde(default)]
    memory_pages: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageFile {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    source_dir: String,
    entry: String,
    /// Parsed as text and interpreted by the domain, so `wf-core` keeps no dependency
    /// on a serialisation format.
    #[serde(default)]
    module_matching: Option<String>,
    /// Namespaces this project expects to come from outside it.
    #[serde(default)]
    host_namespaces: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleFile {
    name: String,
    source: String,
    toolchain: String,
    /// The namespace this module publishes under, when the author wants to say so
    /// rather than have it derived from the file name or the declared name.
    #[serde(default)]
    namespace: Option<String>,
}

/// Reads and parses a manifest file.
///
/// A file that cannot be parsed is reported as an invalid manifest, never as an
/// unexplained failure: the reader has to tell syntax from structure, because the two are
/// fixed by different edits.
pub fn read(path: &Path) -> Result<Manifest, Diagnostic> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("cannot read {}: {error}", path.display()),
        )
        .with_help("run `wf init <name>` to create a project")
    })?;

    parse(&text)
}

/// Reads the `module_matching` value, rejecting an unrecognised one.
fn parse_module_matching(value: Option<&str>) -> Result<wf_core::ModuleMatching, Diagnostic> {
    match value {
        None => Ok(wf_core::ModuleMatching::default()),
        Some(value) => value.parse().map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!("{MANIFEST_FILE} is not a valid manifest: {error}"),
            )
            .with_help("set `module_matching = \"file-name\"` or `= \"name-only\"`")
        }),
    }
}

/// Parses manifest text.
pub fn parse(text: &str) -> Result<Manifest, Diagnostic> {
    let file: ManifestFile = toml::from_str(text).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::InvalidManifest,
            Severity::Error,
            format!("{MANIFEST_FILE} is not a valid manifest: {error}"),
        )
        .with_help("check the syntax; the schema is documented in docs/product.md")
    })?;

    let mut mounts = Vec::with_capacity(file.r#mount.len());
    for mount in file.r#mount {
        let access = match mount.access.as_deref() {
            None | Some("read-only") => wf_core::FilesystemAccess::ReadOnly,
            Some("read-write") => wf_core::FilesystemAccess::ReadWrite,
            Some(other) => {
                return Err(Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    format!(
                        "{MANIFEST_FILE} is not a valid manifest: unknown mount access `{other}`"
                    ),
                )
                .with_help("set `access = \"read-only\"` or `= \"read-write\"`"));
            }
        };
        mounts.push(wf_core::Mount::new(mount.host, mount.guest, access));
    }

    let limits = wf_core::ExecutionLimits::bounded(
        file.runtime
            .wall_clock_seconds
            .filter(|seconds| *seconds > 0.0)
            .map(std::time::Duration::from_secs_f64),
        file.runtime.memory_pages.filter(|pages| *pages > 0),
    );

    Ok(Manifest {
        schema: file.schema,
        mounts,
        runtime: wf_core::RuntimeSettings {
            wasi: file.runtime.wasi.unwrap_or(false),
            limits,
        },
        package: Package {
            name: file.package.name,
            version: file.package.version,
        },
        project: Project {
            source_dir: file.project.source_dir,
            entry: file.project.entry,
            module_matching: parse_module_matching(file.project.module_matching.as_deref())?,
            host_namespaces: file.project.host_namespaces,
        },
        modules: file
            .module
            .into_iter()
            .map(|module| ModuleSpec {
                name: module.name,
                source: module.source,
                toolchain: module.toolchain,
                namespace: module.namespace,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact shape `wf init` writes.
    const SAMPLE: &str = r#"
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
"#;

    #[test]
    fn a_well_formed_manifest_parses_into_the_domain() {
        let manifest = parse(SAMPLE).expect("must parse");
        assert_eq!(manifest.schema, 1);
        assert_eq!(manifest.package.name, "hello");
        assert_eq!(manifest.project.entry, "_start");
        assert_eq!(manifest.modules.len(), 1);
        assert_eq!(manifest.modules[0].toolchain, "precompiled");
        assert!(manifest.validate().is_empty());
    }

    #[test]
    fn syntax_and_structure_are_reported_as_the_same_diagnostic() {
        let error = parse("this is not toml").expect_err("must fail");
        assert_eq!(error.code(), DiagnosticCode::InvalidManifest);
        assert!(error.help().is_some());
    }

    #[test]
    fn a_missing_module_table_is_a_structural_problem_not_a_syntax_one() {
        // `[[module]]` may legitimately be absent from a file the author has not
        // finished, so it parses and `validate` reports it with advice. A missing
        // required table such as `[project]` cannot be read at all and fails as syntax.
        let no_modules = "schema = 1\n[package]\nname = \"x\"\nversion = \"1\"\n                          [project]\nsource_dir = \"s\"\nentry = \"e\"\n";
        let manifest = parse(no_modules).expect("must parse");
        let diagnostics = manifest.validate();
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message().contains("module"));

        let no_project = "schema = 1\n[package]\nname = \"x\"\nversion = \"1\"\n";
        let error = parse(no_project).expect_err("a required table must be present");
        assert_eq!(error.code(), DiagnosticCode::InvalidManifest);
    }

    #[test]
    fn an_unknown_field_is_rejected_so_a_typo_cannot_do_nothing() {
        let typo = SAMPLE.replace("toolchain = \"precompiled\"", "toolchn = \"precompiled\"");
        let error = parse(&typo).expect_err("a typo must not be ignored");
        assert!(
            error.message().contains("toolchn"),
            "the parser names the field it did not expect: {}",
            error.message()
        );
    }

    #[test]
    fn the_structural_rules_still_run_after_a_successful_parse() {
        let manifest = parse(&SAMPLE.replace("version = \"0.1.0\"", "version = \"\""))
            .expect("syntax is fine");
        let diagnostics = manifest.validate();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code(), DiagnosticCode::InvalidManifest);
    }

    /// xorshift64*: identical on every machine, so a failing seed reproduces the same
    /// bytes without a `rand` dependency.
    fn entropy(seed: u64) -> impl FnMut() -> u64 {
        let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
    }

    /// The characters most likely to confuse a TOML reader.
    const NOISE: &[u8] = b"abcdefghij0123456789{}[]\"'=\n\t .#-";

    /// Replaces `count` characters with noise.
    fn mutate(text: &str, seed: u64, count: usize) -> String {
        let mut bytes = text.as_bytes().to_vec();
        let mut next = entropy(seed);
        if bytes.is_empty() {
            return String::new();
        }
        for _ in 0..count.max(1) {
            let index = (next() as usize) % bytes.len();
            let pick = (next() as usize) % NOISE.len();
            bytes[index] = NOISE[pick];
        }
        String::from_utf8(bytes).unwrap_or_default()
    }

    /// A corrupted manifest must fail clearly: never panic, and never produce a half-read
    /// file that looks usable.
    #[test]
    fn corrupted_manifests_never_panic() {
        let mut parsed = 0;
        let mut rejected = 0;

        for seed in 0..200 {
            for count in [1, 4, 16] {
                let candidate = mutate(SAMPLE, seed, count);

                let outcome = std::panic::catch_unwind(|| parse(&candidate));
                let result = outcome.unwrap_or_else(|_| panic!("parse panicked, seed {seed}"));

                match result {
                    Ok(manifest) => {
                        parsed += 1;
                        // Whatever parsed must also survive validation, or a file could
                        // be accepted and then rejected with a panic.
                        let validation = std::panic::catch_unwind(|| manifest.validate());
                        let diagnostics = validation.unwrap_or_else(|_| {
                            panic!("validate panicked on a parsed manifest, seed {seed}")
                        });
                        for diagnostic in &diagnostics {
                            assert!(!diagnostic.message().is_empty(), "seed {seed}");
                        }
                    }
                    Err(diagnostic) => {
                        rejected += 1;
                        assert!(
                            !diagnostic.message().trim().is_empty(),
                            "a parse failure must say something, seed {seed}"
                        );
                        assert!(diagnostic.help().is_some(), "seed {seed}: {diagnostic}");
                    }
                }
            }
        }

        // If every mutation still parsed, the generator would be testing nothing.
        assert!(
            rejected > 0,
            "every corruption parsed cleanly ({parsed} accepted); mutations are too gentle"
        );
    }

    /// A truncated manifest must fail rather than read as a shorter, different project.
    #[test]
    fn truncated_manifests_never_panic() {
        for cut in 0..SAMPLE.len() {
            let truncated = &SAMPLE[..cut];
            let outcome = std::panic::catch_unwind(|| parse(truncated));
            let result = outcome.unwrap_or_else(|_| panic!("parse panicked at cut {cut}"));

            if let Ok(manifest) = result {
                // Not every truncation is an error — cutting the trailing newline leaves a
                // perfectly good file — so the assertion is the one that actually holds:
                // if a truncation validates clean, what it produced must be the whole
                // manifest. Anything else means a partial file was read as a complete one.
                if manifest.validate().is_empty() {
                    assert_eq!(
                        manifest,
                        parse(SAMPLE).expect("the reference manifest parses"),
                        "a manifest cut at byte {cut} validated clean but was not the full file"
                    );
                }
            }
        }
    }

    /// A budget that is not positive is not a bound.
    ///
    /// Reading `-3` as "three seconds" would be inventing a permission the author never
    /// granted, and reading it as "zero seconds" would stop every run. Not enforcing one
    /// is the only answer that does not guess.
    #[test]
    fn a_non_positive_budget_is_not_enforced() {
        for text in [
            "wall_clock_seconds = -3",
            "wall_clock_seconds = 0.0",
            "memory_pages = 0",
        ] {
            let manifest = parse(&SAMPLE.replace(
                "[package]",
                &format!("[runtime]\nwasi = true\n{text}\n\n[package]"),
            ))
            .expect("the manifest still parses");

            assert_eq!(
                manifest.runtime.limits.wall_clock(),
                None,
                "`{text}` must not become a bound"
            );
            assert_eq!(manifest.runtime.limits.memory_pages(), None);
        }
    }

    /// The runtime section states what a project asks for, and the policy says it back.
    #[test]
    fn the_runtime_section_becomes_the_policy() {
        let manifest = parse(&SAMPLE.replace(
            "[package]",
            "[runtime]\nwasi = true\nwall_clock_seconds = 2.5\nmemory_pages = 128\n\n[package]",
        ))
        .expect("parses");

        let policy = manifest.policy();
        assert!(policy.wasi_enabled());
        assert_eq!(
            policy.limits().wall_clock(),
            Some(std::time::Duration::from_secs_f64(2.5))
        );
        assert_eq!(policy.limits().memory_pages(), Some(128));
    }

    /// A mount is read into the manifest, with its access.
    #[test]
    fn a_mount_is_parsed_with_its_access() {
        let manifest = parse(&SAMPLE.replace(
            "[package]",
            "[[mount]]\nhost = \"./data\"\nguest = \"/data\"\naccess = \"read-write\"\n\n[package]",
        ))
        .expect("parses");

        assert_eq!(manifest.mounts.len(), 1);
        assert_eq!(manifest.mounts[0].host(), "./data");
        assert_eq!(manifest.mounts[0].guest(), "/data");
        assert_eq!(
            manifest.mounts[0].access(),
            wf_core::FilesystemAccess::ReadWrite
        );
    }

    /// A mount without an access is read-only, which is the answer a default should give.
    #[test]
    fn a_mount_without_an_access_is_read_only() {
        let manifest = parse(&SAMPLE.replace(
            "[package]",
            "[[mount]]\nhost = \"./data\"\nguest = \"/data\"\n\n[package]",
        ))
        .expect("parses");

        assert_eq!(
            manifest.mounts[0].access(),
            wf_core::FilesystemAccess::ReadOnly
        );
    }
}
