//! The project manifest: data a human writes, rules that refuse to misread it.

use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};

/// A parsed project manifest.
///
/// Plain data with public fields. Parsing belongs to the layer that owns the file; this
/// crate owns whether the parsed result is *allowed to be used*, which is the part that
/// must stay independent of any format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Schema version of the file. Must be one this version understands.
    pub schema: u32,
    /// Identity of the package the manifest describes.
    pub package: Package,
    /// Project-wide settings.
    pub project: Project,
    /// The modules this project produces, in manifest order.
    pub modules: Vec<ModuleSpec>,
}

/// Package identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// Name of the package.
    pub name: String,
    /// Version of the package, as written in the manifest.
    pub version: String,
}

/// Project-wide settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// Directory sources live under, relative to the manifest.
    pub source_dir: String,
    /// Export the project runs by default.
    pub entry: String,
}

/// One module the project builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSpec {
    /// Name used for the module in reports, and for its built artifact.
    pub name: String,
    /// Path to the module's source, relative to the manifest.
    pub source: String,
    /// Toolchain that turns `source` into a module.
    pub toolchain: String,
}

/// The schema version this version of WasmFoundry reads.
///
/// A manifest with any other schema is rejected before a single field is looked at:
/// fields shared by two schemas may have changed meaning, and half-reading a file the
/// version does not know is how a project gets built with the wrong settings.
pub const SUPPORTED_SCHEMA: u32 = 1;

impl Manifest {
    /// Checks that this manifest means what it appears to mean.
    ///
    /// Returns every structural problem found, so a reader sees all of them at once
    /// rather than fixing the file one run at a time. Structural problems are code
    /// [`DiagnosticCode::InvalidManifest`]; whether a named toolchain is actually
    /// *available* is not a property of the manifest and is checked by the layer that
    /// knows what is implemented.
    pub fn validate(&self) -> Vec<Diagnostic> {
        if self.schema != SUPPORTED_SCHEMA {
            return vec![Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!(
                    "unsupported manifest schema {}; this version reads schema {}",
                    self.schema, SUPPORTED_SCHEMA
                ),
            )
            .with_help(
                "set `schema = 1`, or use a WasmFoundry release that reads the schema this file declares",
            )];
        }

        let mut diagnostics = Vec::new();

        if self.package.name.is_empty() {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    "[package] must declare a `name`",
                )
                .with_help("add `name = \"hello\"` under [package]"),
            );
        }

        if self.package.version.is_empty() {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    "[package] must declare a `version`",
                )
                .with_help("add `version = \"0.1.0\"` under [package]"),
            );
        }

        if self.project.source_dir.is_empty() {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    "[project] must declare a `source_dir`",
                )
                .with_help("add `source_dir = \"src\"` under [project]"),
            );
        }

        if self.project.entry.is_empty() {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    "[project] must declare an `entry`",
                )
                .with_help("add `entry = \"_start\"` under [project]"),
            );
        }

        if self.modules.is_empty() {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::InvalidManifest,
                    Severity::Error,
                    "the manifest declares no modules",
                )
                .with_help("add a `[[module]]` table with a name, a source and a toolchain"),
            );
        }

        for module in &self.modules {
            let label = if module.name.is_empty() {
                "a [[module]] entry".to_owned()
            } else {
                format!("module `{}`", module.name)
            };

            if module.name.is_empty() {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::InvalidManifest,
                        Severity::Error,
                        "[[module]] entries must declare a `name`",
                    )
                    .with_help("add `name = \"app\"` to every [[module]] table"),
                );
            }

            if module.source.is_empty() {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::InvalidManifest,
                        Severity::Error,
                        format!("{label} must declare a `source`"),
                    )
                    .with_help("add `source = \"src/app.wasm\"` to the module"),
                );
            }

            if module.toolchain.is_empty() {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::InvalidManifest,
                        Severity::Error,
                        format!("{label} must declare a `toolchain`"),
                    )
                    .with_help("add `toolchain = \"precompiled\"` to the module"),
                );
            }
        }

        // Duplicate names are reported after the per-field checks so that a module with
        // no name at all does not duplicate itself twice.
        let mut seen: Vec<&str> = Vec::new();
        for module in &self.modules {
            if module.name.is_empty() {
                continue;
            }
            if seen.contains(&module.name.as_str()) {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::InvalidManifest,
                        Severity::Error,
                        format!("module `{}` is declared twice", module.name),
                    )
                    .with_help("rename one of them; every module must have a unique name"),
                );
            } else {
                seen.push(&module.name);
            }
        }

        diagnostics
    }
}
