//! JSON representation of an analysis.
//!
//! The domain types in `wf-wasm` are deliberately not serialisable: they carry no
//! knowledge of any output format, and `serde` never enters the analysis crate. This
//! module is the translation layer — domain object to CLI DTO to `serde_json` — so the
//! JSON shape is an interface decision, not a property of the model.
//!
//! Naming is `kebab-case` and enums are lower-case strings, because this output is
//! consumed by other tools.

use serde::Serialize;
use wf_wasm::{
    Analysis, ExportKind, FuncSignature, Function, FunctionKind, Global, ImportType, Memory, Table,
    ValueType,
};

/// A function signature in wire form.
///
/// `FuncSignature` lives in the analysis crate, which has no `serde` dependency and no
/// business knowing one. The signature therefore becomes a CLI type on the way out,
/// exactly like every other field.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct SignatureEntry {
    params: Vec<String>,
    results: Vec<String>,
}

impl From<&FuncSignature> for SignatureEntry {
    fn from(signature: &FuncSignature) -> Self {
        SignatureEntry {
            params: signature.params().iter().map(render_value).collect(),
            results: signature.results().iter().map(render_value).collect(),
        }
    }
}

/// Renders a value type the way the binary spells it.
///
/// A string rather than an object: `i32` and `funcref` are names, not structures, and a
/// consumer that has to unwrap a variant to read `i32` is doing this product's work.
fn render_value(value: &ValueType) -> String {
    value.to_string()
}

/// Renders an analysis as pretty-printed JSON.
pub fn to_json(analysis: &Analysis) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&Report::from(analysis))
}

/// Top level document.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct Report<'a> {
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    module: Option<ModuleReport<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'a str>,
}

impl<'a> From<&'a Analysis> for Report<'a> {
    fn from(analysis: &'a Analysis) -> Self {
        match analysis {
            Analysis::CoreModule(module) => Report {
                kind: "core-module",
                module: Some(module.into()),
                note: None,
            },
            Analysis::Component(_) => Report {
                kind: "component",
                module: None,
                note: Some(
                    "components are recognised but not analysed; analysis arrives in PHASE 17",
                ),
            },
        }
    }
}

/// Everything the analysis found about a core module.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ModuleReport<'a> {
    imports: Vec<ImportEntry<'a>>,
    exports: Vec<ExportEntry<'a>>,
    memories: Vec<MemoryEntry>,
    tables: Vec<TableEntry>,
    globals: Vec<GlobalEntry>,
    functions: Vec<FunctionEntry<'a>>,
}

impl<'a> From<&'a wf_wasm::CoreModule> for ModuleReport<'a> {
    fn from(module: &'a wf_wasm::CoreModule) -> Self {
        ModuleReport {
            imports: module.imports().iter().map(ImportEntry::from).collect(),
            exports: module
                .exports()
                .iter()
                .map(|export| ExportEntry::new(export, module))
                .collect(),
            memories: module.memories().iter().map(MemoryEntry::from).collect(),
            tables: module.tables().iter().map(TableEntry::from).collect(),
            globals: module.globals().iter().map(GlobalEntry::from).collect(),
            functions: module.functions().iter().map(FunctionEntry::from).collect(),
        }
    }
}

/// One import, with its class kept explicit.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ImportEntry<'a> {
    module: &'a str,
    name: &'a str,
    #[serde(flatten)]
    ty: ImportTypeEntry<'a>,
}

/// The class and type of an import. Flattened so `type` sits next to `module`/`name`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ImportTypeEntry<'a> {
    #[serde(rename = "type")]
    class: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<SignatureEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory: Option<MemoryEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    table: Option<TableEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    global: Option<GlobalEntry>,
}

impl<'a> From<&'a wf_wasm::Import> for ImportEntry<'a> {
    fn from(import: &'a wf_wasm::Import) -> Self {
        let ty = match import.ty() {
            ImportType::Function(signature) => ImportTypeEntry {
                class: "function",
                signature: Some(signature.into()),
                memory: None,
                table: None,
                global: None,
            },
            ImportType::Table(table) => ImportTypeEntry {
                class: "table",
                signature: None,
                memory: None,
                table: Some(table.into()),
                global: None,
            },
            ImportType::Memory(memory) => ImportTypeEntry {
                class: "memory",
                signature: None,
                memory: Some(memory.into()),
                table: None,
                global: None,
            },
            ImportType::Global(global) => ImportTypeEntry {
                class: "global",
                signature: None,
                memory: None,
                table: None,
                global: Some(global.into()),
            },
        };
        ImportEntry {
            module: import.module(),
            name: import.name(),
            ty,
        }
    }
}

/// One export.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ExportEntry<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    class: ExportKindEntry,
    index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<SignatureEntry>,
}

impl<'a> ExportEntry<'a> {
    /// Builds the entry, resolving a function export's signature through the index the
    /// binary declares rather than through the position in the export list.
    fn new(export: &'a wf_wasm::Export, module: &'a wf_wasm::CoreModule) -> Self {
        let signature = (export.kind() == ExportKind::Function)
            .then(|| {
                module
                    .functions()
                    .iter()
                    .find(|function| function.index() == export.index())
            })
            .flatten()
            .map(Function::signature);

        ExportEntry {
            name: export.name(),
            class: ExportKindEntry::from(export.kind()),
            index: export.index(),
            signature: signature.map(SignatureEntry::from),
        }
    }
}

/// Lower-case rendering of an export class.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ExportKindEntry {
    Function,
    Table,
    Memory,
    Global,
}

impl From<ExportKind> for ExportKindEntry {
    fn from(kind: ExportKind) -> Self {
        match kind {
            ExportKind::Function => ExportKindEntry::Function,
            ExportKind::Table => ExportKindEntry::Table,
            ExportKind::Memory => ExportKindEntry::Memory,
            ExportKind::Global => ExportKindEntry::Global,
        }
    }
}

/// One memory in the index space.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct MemoryEntry {
    minimum_pages: u64,
    /// Explicit null when the memory is unbounded, so a consumer never has to
    /// distinguish "absent" from "no maximum".
    maximum_pages: Option<u64>,
    shared: bool,
    memory64: bool,
}

impl From<&Memory> for MemoryEntry {
    fn from(memory: &Memory) -> Self {
        MemoryEntry {
            minimum_pages: memory.minimum(),
            maximum_pages: memory.maximum(),
            shared: memory.shared(),
            memory64: memory.memory64(),
        }
    }
}

/// One table in the index space.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct TableEntry {
    element_type: String,
    minimum: u64,
    maximum: Option<u64>,
    shared: bool,
}

impl From<&Table> for TableEntry {
    fn from(table: &Table) -> Self {
        TableEntry {
            element_type: render_value(table.element_type()),
            minimum: table.minimum(),
            maximum: table.maximum(),
            shared: table.shared(),
        }
    }
}

/// One global in the index space.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct GlobalEntry {
    value_type: String,
    mutable: bool,
}

impl From<&Global> for GlobalEntry {
    fn from(global: &Global) -> Self {
        GlobalEntry {
            value_type: render_value(global.value_type()),
            mutable: global.mutable(),
        }
    }
}

/// One function in the index space.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct FunctionEntry<'a> {
    index: u32,
    origin: &'a str,
    signature: SignatureEntry,
}

impl<'a> From<&'a Function> for FunctionEntry<'a> {
    fn from(function: &'a Function) -> Self {
        let origin = match function.kind() {
            FunctionKind::Imported => "imported",
            FunctionKind::Defined { .. } => "defined",
        };
        FunctionEntry {
            index: function.index(),
            origin,
            signature: function.signature().into(),
        }
    }
}
