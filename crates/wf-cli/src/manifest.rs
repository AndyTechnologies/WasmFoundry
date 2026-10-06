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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleFile {
    name: String,
    source: String,
    toolchain: String,
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

    Ok(Manifest {
        schema: file.schema,
        package: Package {
            name: file.package.name,
            version: file.package.version,
        },
        project: Project {
            source_dir: file.project.source_dir,
            entry: file.project.entry,
        },
        modules: file
            .module
            .into_iter()
            .map(|module| ModuleSpec {
                name: module.name,
                source: module.source,
                toolchain: module.toolchain,
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
}
