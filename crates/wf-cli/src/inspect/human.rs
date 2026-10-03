//! Human-readable rendering of an analysis.
//!
//! The layout is not a stable interface: it is meant to be read. What is stable is
//! what it says — which imports exist and of what class, which exports exist, and the
//! shape of memories, tables, globals and functions.

use std::fmt::Write as _;
use std::path::Path;

use wf_wasm::{
    Analysis, CoreModule, Export, ExportKind, FuncSignature, Function, FunctionKind, Global,
    ImportType, Memory, Table,
};

/// Renders the analysis of one binary for a terminal.
pub fn render(path: &Path, analysis: &Analysis) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "Module: {}", path.display());
    match analysis {
        Analysis::CoreModule(module) => {
            let _ = writeln!(out, "Kind: core module");
            let _ = writeln!(out, "Version: 1");
            let _ = writeln!(out);
            render_core(&mut out, module);
        }
        Analysis::Component(_) => {
            let _ = writeln!(out, "Kind: component");
            let _ = writeln!(out, "Version: 13");
            let _ = writeln!(out);
            // Saying nothing would read as "this component has no imports", which is a
            // claim WasmFoundry has not verified. It says what is true instead.
            let _ = writeln!(
                out,
                "This binary is a WebAssembly component. WasmFoundry recognises the format\n\
                 but does not analyse components yet; that arrives in PHASE 17."
            );
        }
    }

    out
}

/// Renders the sections of a core module.
fn render_core(out: &mut String, module: &CoreModule) {
    render_imports(out, module);
    render_exports(out, module);
    render_memories(out, module);
    render_tables(out, module);
    render_globals(out, module);
    render_functions(out, module);
}

fn render_imports(out: &mut String, module: &CoreModule) {
    let _ = writeln!(out, "Imports ({}):", module.imports().len());
    if module.imports().is_empty() {
        let _ = writeln!(out, "  (none)");
        return;
    }
    for import in module.imports() {
        let detail = match import.ty() {
            ImportType::Function(signature) => format!("func  {signature}"),
            ImportType::Table(table) => format!("table {}", render_table(table)),
            ImportType::Memory(memory) => format!("memory {}", render_memory(memory)),
            ImportType::Global(global) => format!("global {}", render_global(global)),
        };
        let _ = writeln!(out, "  {}.{}  {detail}", import.module(), import.name());
    }
    let _ = writeln!(out);
}

fn render_exports(out: &mut String, module: &CoreModule) {
    let _ = writeln!(out, "Exports ({}):", module.exports().len());
    if module.exports().is_empty() {
        let _ = writeln!(out, "  (none)");
        return;
    }
    for export in module.exports() {
        // A function export also carries its signature, because that is what tells a
        // host what it must be able to call. The index comes from the export section
        // itself, so it stays correct when only some functions are exported.
        match exported_signature(module, export) {
            Some(signature) => {
                let _ = writeln!(out, "  {}  {}  {signature}", export.name(), export.kind());
            }
            None => {
                let _ = writeln!(out, "  {}  {}", export.name(), export.kind());
            }
        }
    }
    let _ = writeln!(out);
}

/// The signature of an exported function, looked up through the index the binary
/// declares. Returns `None` for every other export class.
fn exported_signature<'a>(module: &'a CoreModule, export: &Export) -> Option<&'a FuncSignature> {
    if export.kind() != ExportKind::Function {
        return None;
    }
    module
        .functions()
        .iter()
        .find(|function| function.index() == export.index())
        .map(Function::signature)
}

fn render_memories(out: &mut String, module: &CoreModule) {
    let _ = writeln!(out, "Memories ({}):", module.memories().len());
    if module.memories().is_empty() {
        let _ = writeln!(out, "  (none)");
        return;
    }
    for (index, memory) in module.memories().iter().enumerate() {
        let _ = writeln!(out, "  #{index}  {}", render_memory(memory));
    }
    let _ = writeln!(out);
}

fn render_tables(out: &mut String, module: &CoreModule) {
    let _ = writeln!(out, "Tables ({}):", module.tables().len());
    if module.tables().is_empty() {
        let _ = writeln!(out, "  (none)");
        return;
    }
    for (index, table) in module.tables().iter().enumerate() {
        let _ = writeln!(out, "  #{index}  {}", render_table(table));
    }
    let _ = writeln!(out);
}

fn render_globals(out: &mut String, module: &CoreModule) {
    let _ = writeln!(out, "Globals ({}):", module.globals().len());
    if module.globals().is_empty() {
        let _ = writeln!(out, "  (none)");
        return;
    }
    for (index, global) in module.globals().iter().enumerate() {
        let _ = writeln!(out, "  #{index}  {}", render_global(global));
    }
    let _ = writeln!(out);
}

fn render_functions(out: &mut String, module: &CoreModule) {
    let _ = writeln!(out, "Functions ({}):", module.functions().len());
    if module.functions().is_empty() {
        let _ = writeln!(out, "  (none)");
        return;
    }
    for function in module.functions() {
        let origin = match function.kind() {
            FunctionKind::Imported => "imported",
            FunctionKind::Defined { .. } => "defined ",
        };
        let _ = writeln!(
            out,
            "  #{}  {origin}  {}",
            function.index(),
            function.signature()
        );
    }
}

fn render_memory(memory: &Memory) -> String {
    let maximum = match memory.maximum() {
        Some(maximum) => format!("{maximum} pages"),
        None => "unbounded".to_owned(),
    };
    let mut text = format!("min {} pages, max {maximum}", memory.minimum());
    if memory.shared() {
        text.push_str(", shared");
    }
    if memory.memory64() {
        text.push_str(", 64-bit");
    }
    text
}

fn render_table(table: &Table) -> String {
    let maximum = match table.maximum() {
        Some(maximum) => format!("{maximum}"),
        None => "unbounded".to_owned(),
    };
    let mut text = format!(
        "{}, min {}, max {maximum}",
        table.element_type(),
        table.minimum()
    );
    if table.shared() {
        text.push_str(", shared");
    }
    text
}

fn render_global(global: &Global) -> String {
    if global.mutable() {
        format!("{} (mutable)", global.value_type())
    } else {
        format!("{} (immutable)", global.value_type())
    }
}
