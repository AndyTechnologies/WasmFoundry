//! Section-level analysis of a WebAssembly binary.
//!
//! This module owns the translation from `wasmparser`'s reader types into the
//! domain-shaped report that `wf-cli` renders. Nothing from `wasmparser` crosses
//! into the public surface: every type below is owned here, and every parser
//! failure becomes an [`Error`].

use std::fmt;

use wasmparser::{
    BinaryReaderError, CompositeInnerType, ExternalKind, ImportSectionReader, Imports, Parser,
    Payload, TypeRef, ValType,
};

use crate::{Error, ModuleKind, classify};

/// A WebAssembly value type, as this crate names it.
///
/// Reference types are carried as their rendered form rather than being re-modelled
/// here: `funcref` and `externref` today, and future proposal types without this
/// crate having to change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueType {
    /// A 32-bit integer.
    I32,
    /// A 64-bit integer.
    I64,
    /// A 32-bit float.
    F32,
    /// A 64-bit float.
    F64,
    /// A 128-bit vector.
    V128,
    /// Any reference type, rendered as the binary spells it.
    Reference(String),
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::I32 => f.write_str("i32"),
            ValueType::I64 => f.write_str("i64"),
            ValueType::F32 => f.write_str("f32"),
            ValueType::F64 => f.write_str("f64"),
            ValueType::V128 => f.write_str("v128"),
            ValueType::Reference(name) => f.write_str(name),
        }
    }
}

/// The parameter and result types of a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncSignature {
    params: Vec<ValueType>,
    results: Vec<ValueType>,
}

impl FuncSignature {
    /// Types the function accepts, in order.
    pub fn params(&self) -> &[ValueType] {
        &self.params
    }

    /// Types the function returns, in order.
    pub fn results(&self) -> &[ValueType] {
        &self.results
    }
}

impl fmt::Display for FuncSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        for (i, param) in self.params.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{param}")?;
        }
        f.write_str(") -> (")?;
        for (i, result) in self.results.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{result}")?;
        }
        f.write_str(")")
    }
}

/// A table declared or imported by a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    element_type: ValueType,
    minimum: u64,
    maximum: Option<u64>,
    shared: bool,
}

impl Table {
    /// Element type held by the table.
    pub fn element_type(&self) -> &ValueType {
        &self.element_type
    }

    /// Minimum number of elements, as declared in the binary.
    pub fn minimum(&self) -> u64 {
        self.minimum
    }

    /// Maximum number of elements, or `None` when unbounded.
    pub fn maximum(&self) -> Option<u64> {
        self.maximum
    }

    /// Whether the table is shared between threads.
    pub fn shared(&self) -> bool {
        self.shared
    }
}

/// A linear memory declared or imported by a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    minimum: u64,
    maximum: Option<u64>,
    shared: bool,
    memory64: bool,
}

impl Memory {
    /// Minimum size in pages; one page is 64 KiB.
    pub fn minimum(&self) -> u64 {
        self.minimum
    }

    /// Maximum size in pages, or `None` when unbounded.
    pub fn maximum(&self) -> Option<u64> {
        self.maximum
    }

    /// Whether the memory is shared between threads.
    pub fn shared(&self) -> bool {
        self.shared
    }

    /// Whether the memory is indexed with 64-bit addresses.
    pub fn memory64(&self) -> bool {
        self.memory64
    }
}

/// A global variable declared or imported by a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    value_type: ValueType,
    mutable: bool,
    shared: bool,
}

impl Global {
    /// Type of the value the global holds.
    pub fn value_type(&self) -> &ValueType {
        &self.value_type
    }

    /// Whether the guest may write to the global.
    pub fn mutable(&self) -> bool {
        self.mutable
    }

    /// Whether the global is shared between threads.
    pub fn shared(&self) -> bool {
        self.shared
    }
}

/// The kind of entity an import brings into the module's index space.
///
/// The four classes are kept apart because they are genuinely different things: a
/// global is not a function, and multi-module resolution must not confuse them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportType {
    /// A function, with the signature it must satisfy.
    Function(FuncSignature),
    /// A table, with its element type and limits.
    Table(Table),
    /// A linear memory, with its limits.
    Memory(Memory),
    /// A global, with its type and mutability.
    Global(Global),
}

/// One entry of the import section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    module: String,
    name: String,
    ty: ImportType,
}

impl Import {
    /// Namespace the import is resolved in, for example `env`.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Name the host must provide within the namespace.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What is being imported, with its type.
    pub fn ty(&self) -> &ImportType {
        &self.ty
    }
}

/// The class of an exported entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// An exported function.
    Function,
    /// An exported table.
    Table,
    /// An exported linear memory.
    Memory,
    /// An exported global.
    Global,
}

impl fmt::Display for ExportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ExportKind::Function => "func",
            ExportKind::Table => "table",
            ExportKind::Memory => "memory",
            ExportKind::Global => "global",
        })
    }
}

/// One entry of the export section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    name: String,
    kind: ExportKind,
    index: u32,
}

impl Export {
    /// Exported name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Class of the exported entity.
    pub fn kind(&self) -> ExportKind {
        self.kind
    }

    /// Position of the exported entity in the index space of its class.
    ///
    /// This is what the binary actually states, and it is not the same as the
    /// position of the export within the export section: a module may import
    /// functions, or export only some of them.
    pub fn index(&self) -> u32 {
        self.index
    }
}

/// Where a function in the index space comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    /// The function is imported and must be provided by the host.
    Imported,
    /// The function body lives in this module.
    Defined {
        /// Byte offset of the body within the binary, when the section was present.
        code_offset: Option<u64>,
    },
}

/// One function in the module's function index space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    index: u32,
    kind: FunctionKind,
    signature: FuncSignature,
}

impl Function {
    /// Position in the function index space.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Whether the function is imported or defined here.
    pub fn kind(&self) -> FunctionKind {
        self.kind
    }

    /// Signature the function has.
    pub fn signature(&self) -> &FuncSignature {
        &self.signature
    }
}

/// Everything this crate can say about a core module.
///
/// Imports are listed ahead of the entities they define, so `imports()` and
/// `memories()` overlap by design: an imported memory appears in both, once as a
/// requirement on the host and once as part of the module's index space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreModule {
    imports: Vec<Import>,
    exports: Vec<Export>,
    memories: Vec<Memory>,
    tables: Vec<Table>,
    globals: Vec<Global>,
    functions: Vec<Function>,
}

impl CoreModule {
    /// Entries the host must satisfy before this module can run.
    pub fn imports(&self) -> &[Import] {
        &self.imports
    }

    /// Entries this module makes available to the host.
    pub fn exports(&self) -> &[Export] {
        &self.exports
    }

    /// Memories in the module's index space, imported and declared.
    pub fn memories(&self) -> &[Memory] {
        &self.memories
    }

    /// Tables in the module's index space, imported and declared.
    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    /// Globals in the module's index space, imported and declared.
    pub fn globals(&self) -> &[Global] {
        &self.globals
    }

    /// Functions in the module's index space, imported and defined.
    pub fn functions(&self) -> &[Function] {
        &self.functions
    }
}

/// A recognised WebAssembly component.
///
/// Components are a different binary format with a different structure, and WasmFoundry
/// analyses them in PHASE 17. Reporting a component here means the format was correctly
/// identified and deliberately not mis-reported as a core module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSummary;

/// The result of analysing a WebAssembly binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Analysis {
    /// A core module, fully analysed.
    CoreModule(CoreModule),
    /// A component, recognised but not analysed.
    Component(ComponentSummary),
}

impl Analysis {
    /// Which binary format the input uses.
    pub fn kind(&self) -> ModuleKind {
        match self {
            Analysis::CoreModule(_) => ModuleKind::CoreModule,
            Analysis::Component(_) => ModuleKind::Component,
        }
    }

    /// The analysed core module, or `None` when the input is a component.
    pub fn core(&self) -> Option<&CoreModule> {
        match self {
            Analysis::CoreModule(module) => Some(module),
            Analysis::Component(_) => None,
        }
    }
}

/// Analyses a WebAssembly binary without executing any of it.
///
/// The binary is validated in full before any section is read, so a module that
/// declares sizes, counts or type indexes it does not have is rejected rather than
/// described. Header rejection happens first and cheaply.
pub fn analyze(bytes: &[u8]) -> Result<Analysis, Error> {
    match classify(bytes)? {
        ModuleKind::Component => Ok(Analysis::Component(ComponentSummary)),
        ModuleKind::CoreModule => {
            validate(bytes)?;
            Ok(Analysis::CoreModule(analyze_core(bytes)?))
        }
    }
}

/// Rejects a module the validator does not accept.
fn validate(bytes: &[u8]) -> Result<(), Error> {
    wasmparser::Validator::new()
        .validate_all(bytes)
        .map(|_types| ())
        .map_err(|error| Error::InvalidWasm {
            message: error.message().to_owned(),
            offset: Some(error.offset()),
        })
}

/// Walks the sections of an already validated core module.
fn analyze_core(bytes: &[u8]) -> Result<CoreModule, Error> {
    let mut types: Vec<Option<FuncSignature>> = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut memories = Vec::new();
    let mut tables = Vec::new();
    let mut globals = Vec::new();
    let mut functions = Vec::new();
    let mut declared_functions: Vec<u32> = Vec::new();
    let mut code_offsets: Vec<Option<u64>> = Vec::new();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(invalid)? {
            Payload::TypeSection(reader) => {
                for group in reader {
                    for sub_type in group.map_err(invalid)?.into_types() {
                        types.push(signature_of(&sub_type));
                    }
                }
            }
            Payload::ImportSection(reader) => {
                collect_imports(
                    reader,
                    &types,
                    &mut imports,
                    &mut memories,
                    &mut tables,
                    &mut globals,
                    &mut functions,
                )?;
            }
            Payload::FunctionSection(reader) => {
                for type_index in reader {
                    declared_functions.push(type_index.map_err(invalid)?);
                }
            }
            Payload::TableSection(reader) => {
                for table in reader {
                    let table = convert_table(&table.map_err(invalid)?.ty);
                    tables.push(table);
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    memories.push(convert_memory(&memory.map_err(invalid)?));
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = global.map_err(invalid)?;
                    globals.push(Global {
                        value_type: convert_value(&global.ty.content_type),
                        mutable: global.ty.mutable,
                        shared: global.ty.shared,
                    });
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(invalid)?;
                    exports.push(Export {
                        name: export.name.to_owned(),
                        kind: match export.kind {
                            ExternalKind::Func | ExternalKind::FuncExact => ExportKind::Function,
                            ExternalKind::Table => ExportKind::Table,
                            ExternalKind::Memory => ExportKind::Memory,
                            ExternalKind::Global => ExportKind::Global,
                            // Tags belong to the exception-handling proposal, which is
                            // not in the supported set; the validator would have
                            // rejected the module before reaching this point.
                            ExternalKind::Tag => ExportKind::Function,
                        },
                        index: export.index,
                    });
                }
            }
            Payload::CodeSectionStart { range, .. } => {
                code_offsets.push(Some(range.start));
            }
            _ => {}
        }
    }

    for (offset, type_index) in declared_functions.iter().enumerate() {
        let signature = lookup(&types, *type_index).ok_or_else(|| Error::InvalidWasm {
            message: format!("function declares type index {type_index}, which does not exist"),
            offset: None,
        })?;
        functions.push(Function {
            index: functions.len() as u32,
            kind: FunctionKind::Defined {
                code_offset: code_offsets.get(offset).copied().flatten(),
            },
            signature,
        });
    }

    Ok(CoreModule {
        imports,
        exports,
        memories,
        tables,
        globals,
        functions,
    })
}

/// Flattens the import section.
///
/// The binary format groups imports three ways, including a compact encoding that
/// shares one module name and one type across many names. All three expand to the same
/// entries a host has to satisfy, so they are flattened here rather than special-cased
/// by callers.
fn collect_imports(
    reader: ImportSectionReader<'_>,
    types: &[Option<FuncSignature>],
    imports: &mut Vec<Import>,
    memories: &mut Vec<Memory>,
    tables: &mut Vec<Table>,
    globals: &mut Vec<Global>,
    functions: &mut Vec<Function>,
) -> Result<(), Error> {
    for group in reader {
        match group.map_err(invalid)? {
            Imports::Single(_, import) => {
                record_import(
                    import.module,
                    import.name,
                    &import.ty,
                    types,
                    imports,
                    memories,
                    tables,
                    globals,
                    functions,
                );
            }
            Imports::Compact1 { module, items } => {
                for item in items {
                    let item = item.map_err(invalid)?;
                    record_import(
                        module, item.name, &item.ty, types, imports, memories, tables, globals,
                        functions,
                    );
                }
            }
            Imports::Compact2 { module, ty, names } => {
                for name in names {
                    let name = name.map_err(invalid)?;
                    record_import(
                        module, name, &ty, types, imports, memories, tables, globals, functions,
                    );
                }
            }
        }
    }
    Ok(())
}

/// Records one import and the index-space entity it brings with it.
#[allow(clippy::too_many_arguments)]
fn record_import(
    namespace: &str,
    name: &str,
    type_ref: &TypeRef,
    types: &[Option<FuncSignature>],
    imports: &mut Vec<Import>,
    memories: &mut Vec<Memory>,
    tables: &mut Vec<Table>,
    globals: &mut Vec<Global>,
    functions: &mut Vec<Function>,
) {
    let ty = convert_import(type_ref, types);
    match &ty {
        ImportType::Table(table) => tables.push(table.clone()),
        ImportType::Memory(memory) => memories.push(memory.clone()),
        ImportType::Global(global) => globals.push(global.clone()),
        ImportType::Function(signature) => functions.push(Function {
            index: functions.len() as u32,
            kind: FunctionKind::Imported,
            signature: signature.clone(),
        }),
    }
    imports.push(Import {
        module: namespace.to_owned(),
        name: name.to_owned(),
        ty,
    });
}

/// Translates a parser failure into this crate's error type.
fn invalid(error: BinaryReaderError) -> Error {
    Error::InvalidWasm {
        message: error.message().to_owned(),
        offset: Some(error.offset()),
    }
}

/// Extracts the function signature from a type definition, if it is a function type.
fn signature_of(sub_type: &wasmparser::SubType) -> Option<FuncSignature> {
    match &sub_type.composite_type.inner {
        CompositeInnerType::Func(function) => Some(FuncSignature {
            params: function.params().iter().map(convert_value).collect(),
            results: function.results().iter().map(convert_value).collect(),
        }),
        // Structs, arrays and continuations are GC-proposal types. They occupy a type
        // index like any other, so they are recorded as "not a function" instead of
        // being skipped, which would shift every following index.
        _ => None,
    }
}

/// Resolves a type index to a function signature.
fn lookup(types: &[Option<FuncSignature>], index: u32) -> Option<FuncSignature> {
    types
        .get(index as usize)
        .and_then(|signature| signature.clone())
}

/// Builds the import entry for one import declaration.
fn convert_import(ty: &TypeRef, types: &[Option<FuncSignature>]) -> ImportType {
    match ty {
        TypeRef::Func(index) | TypeRef::FuncExact(index) => {
            ImportType::Function(lookup(types, *index).unwrap_or_else(FuncSignature::unknown))
        }
        TypeRef::Table(table) => ImportType::Table(convert_table(table)),
        TypeRef::Memory(memory) => ImportType::Memory(convert_memory(memory)),
        TypeRef::Global(global) => ImportType::Global(Global {
            value_type: convert_value(&global.content_type),
            mutable: global.mutable,
            shared: global.shared,
        }),
        // Tags are not part of the supported feature set and the validator rejects a
        // module that imports one before this point is reached.
        TypeRef::Tag(_) => ImportType::Function(FuncSignature::unknown()),
    }
}

fn convert_table(table: &wasmparser::TableType) -> Table {
    Table {
        element_type: ValueType::Reference(table.element_type.to_string()),
        minimum: table.initial,
        maximum: table.maximum,
        shared: table.shared,
    }
}

fn convert_memory(memory: &wasmparser::MemoryType) -> Memory {
    Memory {
        minimum: memory.initial,
        maximum: memory.maximum,
        shared: memory.shared,
        memory64: memory.memory64,
    }
}

fn convert_value(value: &ValType) -> ValueType {
    match value {
        ValType::I32 => ValueType::I32,
        ValType::I64 => ValueType::I64,
        ValType::F32 => ValueType::F32,
        ValType::F64 => ValueType::F64,
        ValType::V128 => ValueType::V128,
        ValType::Ref(reference) => ValueType::Reference(reference.to_string()),
    }
}

impl FuncSignature {
    /// Placeholder for a function whose type index cannot be resolved.
    ///
    /// The module was already validated, so this is only reachable for a type index
    /// that is not a function type — for example a struct from the GC proposal. The
    /// report says so instead of inventing a signature.
    fn unknown() -> FuncSignature {
        FuncSignature {
            params: Vec::new(),
            results: Vec::new(),
        }
    }
}
