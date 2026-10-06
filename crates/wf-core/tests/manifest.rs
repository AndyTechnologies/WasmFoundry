// Tests for manifest validation.
//
// The manifest is data a project author wrote by hand, so every rule here is about
// refusing to interpret a file that does not mean what it appears to mean. The one rule
// that matters most is the schema: an unknown schema is rejected outright rather than
// read as far as it goes.

use wf_core::{DiagnosticCode, Manifest, ModuleSpec, Package, Project, Severity};

/// A manifest of the shape `wf init` writes.
fn valid_manifest() -> Manifest {
    Manifest {
        schema: 1,
        package: Package {
            name: "hello".to_owned(),
            version: "0.1.0".to_owned(),
        },
        project: Project {
            source_dir: "src".to_owned(),
            entry: "_start".to_owned(),
        },
        modules: vec![ModuleSpec {
            name: "hello".to_owned(),
            source: "src/hello.wasm".to_owned(),
            toolchain: "precompiled".to_owned(),
        }],
    }
}

#[test]
fn a_valid_manifest_produces_no_diagnostics() {
    assert!(valid_manifest().validate().is_empty());
}

#[test]
fn an_unsupported_schema_is_rejected_outright() {
    let mut manifest = valid_manifest();
    manifest.schema = 2;

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::InvalidManifest);
    assert!(
        diagnostics[0].message().contains("2"),
        "the message must name the schema it found: {}",
        diagnostics[0].message()
    );
    assert!(
        diagnostics[0].message().contains("1"),
        "the message must name the schema it supports: {}",
        diagnostics[0].message()
    );
}

#[test]
fn an_unknown_schema_is_not_partially_interpreted() {
    // With schema 2 the rest of the file uses a shape this version does not know. Reading
    // the fields it happens to share would produce a project that looks valid and is not.
    let manifest = Manifest {
        schema: 2,
        package: Package {
            name: String::new(),
            version: String::new(),
        },
        project: Project {
            source_dir: String::new(),
            entry: String::new(),
        },
        modules: Vec::new(),
    };

    let diagnostics = manifest.validate();
    assert_eq!(
        diagnostics.len(),
        1,
        "an unknown schema stops validation: {diagnostics:?}"
    );
    assert_eq!(diagnostics[0].code(), DiagnosticCode::InvalidManifest);
}

#[test]
fn an_empty_package_name_is_rejected() {
    let mut manifest = valid_manifest();
    manifest.package.name = String::new();

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("name"));
}

#[test]
fn an_empty_version_is_rejected() {
    let mut manifest = valid_manifest();
    manifest.package.version = String::new();

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("version"));
}

#[test]
fn an_empty_entry_point_is_rejected() {
    let mut manifest = valid_manifest();
    manifest.project.entry = String::new();

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("entry"));
}

#[test]
fn an_empty_source_dir_is_rejected() {
    let mut manifest = valid_manifest();
    manifest.project.source_dir = String::new();

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("source_dir"));
}

#[test]
fn a_manifest_without_modules_is_rejected() {
    let mut manifest = valid_manifest();
    manifest.modules.clear();

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("module"));
}

#[test]
fn a_module_without_a_source_is_rejected() {
    let mut manifest = valid_manifest();
    manifest.modules[0].source = String::new();

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("source"));
}

#[test]
fn a_module_without_a_toolchain_is_rejected() {
    let mut manifest = valid_manifest();
    manifest.modules[0].toolchain = String::new();

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message().contains("toolchain"));
}

#[test]
fn duplicate_module_names_are_rejected() {
    // Two modules with one name cannot both be built, and silently keeping one would
    // produce a project whose manifest says something other than what it does.
    let mut manifest = valid_manifest();
    manifest.modules.push(ModuleSpec {
        name: "hello".to_owned(),
        source: "src/other.wasm".to_owned(),
        toolchain: "precompiled".to_owned(),
    });

    let diagnostics = manifest.validate();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message().contains("hello"));
}

#[test]
fn every_structural_problem_reports_wf011_with_error_severity() {
    let mut manifest = valid_manifest();
    manifest.package.name = String::new();
    manifest.project.entry = String::new();
    manifest.modules.clear();

    let diagnostics = manifest.validate();
    assert!(!diagnostics.is_empty());
    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.code(), DiagnosticCode::InvalidManifest);
        assert_eq!(diagnostic.severity(), Severity::Error);
        assert!(
            diagnostic.help().is_some(),
            "each must say what to do: {diagnostic}"
        );
    }
}

#[test]
fn a_degenerate_manifest_never_panics() {
    // Schema 1 on purpose: an unknown schema stops validation immediately, so this case
    // is about the field-level rules and needs to reach them.
    let manifest = Manifest {
        schema: 1,
        package: Package {
            name: String::new(),
            version: String::new(),
        },
        project: Project {
            source_dir: String::new(),
            entry: String::new(),
        },
        modules: vec![ModuleSpec {
            name: String::new(),
            source: String::new(),
            toolchain: String::new(),
        }],
    };

    let diagnostics = manifest.validate();
    assert!(
        diagnostics.len() >= 5,
        "every empty field is reported: {diagnostics:?}"
    );
}
