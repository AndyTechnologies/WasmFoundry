// Tests for `wf-wasm::analyze`.
//
// Every binary here is assembled from WAT text by the pinned `wat` dev-dependency, so
// the input is readable and reviewable instead of an opaque committed blob. Malformed
// inputs are byte-level literals on purpose: a mutation has to produce exactly the
// header the parser is supposed to reject.

use wf_wasm::{Analysis, Error, FunctionKind, ImportType, ModuleKind, ValueType};

/// Assembles WAT source into a binary module.
fn wasm(source: &str) -> Vec<u8> {
    wat::parse_str(source).expect("test fixture must assemble")
}

/// A module exercising every import and export class the analyser must preserve.
const KITCHEN_SINK: &str = r#"
    (module
      (import "env" "console_log" (func (param i32 i32)))
      (import "env" "table" (table 1 10 funcref))
      (import "env" "memory" (memory 2 16))
      (import "env" "counter" (global (mut i32)))

      (memory (export "memory") 2 16)
      (table (export "table") 1 funcref)
      (global (export "answer") i32 (i32.const 42))
      (func (export "_start") (param i32) (result i32)
        local.get 0
        i32.const 1
        i32.add)
    )
"#;

/// Finds the import with the given module and name.
fn find_import<'a>(
    module: &'a wf_wasm::CoreModule,
    namespace: &str,
    name: &str,
) -> Option<&'a wf_wasm::Import> {
    module
        .imports()
        .iter()
        .find(|i| i.module() == namespace && i.name() == name)
}

/// Extracts the core module, failing loudly if the fixture stopped being a core module.
fn core(source: &str) -> wf_wasm::CoreModule {
    match analyze_source(source) {
        Analysis::CoreModule(module) => module,
        Analysis::Component(_) => panic!("fixture must be a core module"),
    }
}

/// Assembles and analyses, panicking with the analysis error.
fn analyze_source(source: &str) -> Analysis {
    wf_wasm::analyze(&wasm(source)).expect("fixture must be valid")
}

#[test]
fn classifies_a_core_module() {
    let analysis = analyze_source("(module)");
    assert_eq!(analysis.kind(), ModuleKind::CoreModule);
}

#[test]
fn classifies_a_component_without_pretending_to_analyse_it() {
    // A component is a real binary format; recognising it and declining to analyse it
    // is the honest answer until PHASE 17.
    let bytes = wat::parse_str("(component)").expect("component fixture");
    let analysis = wf_wasm::analyze(&bytes).expect("component is well formed");
    assert_eq!(analysis.kind(), ModuleKind::Component);
    assert!(
        analysis.core().is_none(),
        "a component must not produce a core module report"
    );
}

#[test]
fn preserves_the_class_of_every_import() {
    let module = core(KITCHEN_SINK);

    let func = find_import(&module, "env", "console_log").expect("function import");
    assert!(
        matches!(func.ty(), ImportType::Function(sig)
            if sig.params() == [ValueType::I32, ValueType::I32] && sig.results().is_empty()),
        "function import must keep its signature, got {:?}",
        func.ty()
    );

    let table = find_import(&module, "env", "table").expect("table import");
    assert!(
        matches!(table.ty(), ImportType::Table(t) if t.minimum() == 1 && t.maximum() == Some(10)),
        "table import must keep its limits, got {:?}",
        table.ty()
    );

    let memory = find_import(&module, "env", "memory").expect("memory import");
    assert!(
        matches!(memory.ty(), ImportType::Memory(m)
            if m.minimum() == 2 && m.maximum() == Some(16)),
        "memory import must keep its limits, got {:?}",
        memory.ty()
    );

    let global = find_import(&module, "env", "counter").expect("global import");
    assert!(
        matches!(global.ty(), ImportType::Global(g)
            if g.value_type() == &ValueType::I32 && g.mutable()),
        "global import must keep its type and mutability, got {:?}",
        global.ty()
    );
}

#[test]
fn reports_exports_with_their_class() {
    let module = core(KITCHEN_SINK);
    let exports = module.exports();

    let memory = exports
        .iter()
        .find(|e| e.name() == "memory")
        .expect("memory export");
    assert!(matches!(memory.kind(), wf_wasm::ExportKind::Memory));

    let table = exports
        .iter()
        .find(|e| e.name() == "table")
        .expect("table export");
    assert!(matches!(table.kind(), wf_wasm::ExportKind::Table));

    let answer = exports
        .iter()
        .find(|e| e.name() == "answer")
        .expect("global export");
    assert!(matches!(answer.kind(), wf_wasm::ExportKind::Global));

    let start = exports
        .iter()
        .find(|e| e.name() == "_start")
        .expect("function export");
    assert!(matches!(start.kind(), wf_wasm::ExportKind::Function));
}

#[test]
fn separates_declared_from_imported_functions() {
    let module = core(KITCHEN_SINK);

    // One imported function plus the one defined in the module body.
    assert_eq!(module.functions().len(), 2);

    let imported = module
        .functions()
        .iter()
        .find(|f| matches!(f.kind(), FunctionKind::Imported))
        .expect("the imported function must be listed");
    assert!(matches!(
        imported.signature().params(),
        [ValueType::I32, ValueType::I32]
    ));

    let defined = module
        .functions()
        .iter()
        .find(|f| matches!(f.kind(), FunctionKind::Defined { .. }))
        .expect("the defined function must be listed");
    assert!(matches!(defined.signature().params(), [ValueType::I32]));
    assert!(matches!(defined.signature().results(), [ValueType::I32]));
}

#[test]
fn reports_memories_and_globals_declared_by_the_module() {
    let module = core(KITCHEN_SINK);

    // Two memories: one imported, one declared. Both must be visible.
    assert_eq!(module.memories().len(), 2);
    assert!(
        module
            .memories()
            .iter()
            .all(|m| m.minimum() == 2 && m.maximum() == Some(16))
    );

    // Two globals: one imported (mutable) and one exported (immutable).
    assert_eq!(module.globals().len(), 2);
    assert!(
        module
            .globals()
            .iter()
            .any(|g| g.mutable() && g.value_type() == &ValueType::I32)
    );
    assert!(
        module
            .globals()
            .iter()
            .any(|g| !g.mutable() && g.value_type() == &ValueType::I32)
    );
}

#[test]
fn reports_a_module_with_no_imports_as_empty_rather_than_inventing_one() {
    let module = core("(module (func (export \"_start\")))");
    assert!(module.imports().is_empty());
    assert!(module.tables().is_empty());
    assert!(module.globals().is_empty());
}

#[test]
fn rejects_input_that_is_not_webassembly() {
    let error = wf_wasm::analyze(b"this is plain text, not a wasm binary at all").unwrap_err();
    assert!(
        matches!(error, Error::BadMagic(_)),
        "expected a magic failure, got {error:?}"
    );
}

#[test]
fn rejects_a_truncated_header() {
    let error = wf_wasm::analyze(&[0x00, 0x61, 0x73, 0x6d]).unwrap_err();
    assert!(
        matches!(error, Error::TruncatedHeader { available: 4 }),
        "expected a truncated header failure, got {error:?}"
    );
}

#[test]
fn rejects_an_unknown_binary_version() {
    let mut bytes = wasm("(module)");
    bytes[4] = 0x02; // little-endian version 2, which does not exist
    let error = wf_wasm::analyze(&bytes).unwrap_err();
    assert!(
        matches!(error, Error::UnsupportedVersion { version: 2 }),
        "expected an unsupported version failure, got {error:?}"
    );
}

#[test]
fn rejects_a_corrupt_section_rather_than_reporting_its_contents() {
    // Claim a type section exists, then cut the binary inside it. A parser that trusted
    // the declared count or size would panic or invent results here.
    let mut bytes = wasm(KITCHEN_SINK);
    let len = bytes.len();
    bytes.truncate(len - 12);
    let error = wf_wasm::analyze(&bytes).unwrap_err();
    assert!(
        matches!(error, Error::InvalidWasm { .. }),
        "a truncated body must be reported as invalid wasm, got {error:?}"
    );
}

#[test]
fn rejects_a_module_whose_declared_type_index_does_not_exist() {
    // Valid header, valid type section, but an import pointing at a type that was never
    // defined. Validation must catch it before any signature is rendered.
    let bytes = wat::parse_str(
        r#"
        (module
          (type (func))
          (import "env" "missing" (func (type 7)))
        )
        "#,
    )
    .expect("wat assembles; the failure must come from validation");
    let error = wf_wasm::analyze(&bytes).unwrap_err();
    assert!(
        matches!(error, Error::InvalidWasm { .. }),
        "an out-of-range type index must be rejected, got {error:?}"
    );
}

#[test]
fn error_messages_name_the_defect() {
    let error = wf_wasm::analyze(b"nope").unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("WebAssembly"),
        "message must say what failed: {message}"
    );
}

#[test]
fn analysis_of_the_same_bytes_is_deterministic() {
    let first = analyze_source(KITCHEN_SINK);
    let second = analyze_source(KITCHEN_SINK);
    assert_eq!(
        format!("{first:?}"),
        format!("{second:?}"),
        "the same input must produce the same report"
    );
}
