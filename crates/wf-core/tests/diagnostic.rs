// Tests for diagnostics, the way every layer reports a problem.
//
// A diagnostic is a contract: the code is stable, the severity says what happens
// next, and the message must be readable without knowing which crate produced it.

use wf_core::{Diagnostic, DiagnosticCode, Severity};

#[test]
fn every_code_renders_as_the_token_scripts_match() {
    // Codes are a published interface. A script branches on `WF005`, so the rendering
    // must be exactly that token.
    assert_eq!(DiagnosticCode::InvalidWasm.to_string(), "WF001");
    assert_eq!(DiagnosticCode::UnresolvedImport.to_string(), "WF002");
    assert_eq!(DiagnosticCode::MissingEntrypoint.to_string(), "WF005");
    assert_eq!(DiagnosticCode::RuntimeTrap.to_string(), "WF009");
    assert_eq!(DiagnosticCode::EntrypointUnsuitable.to_string(), "WF010");
}

#[test]
fn a_diagnostic_carries_its_code_severity_and_message() {
    let diagnostic = Diagnostic::new(
        DiagnosticCode::InvalidWasm,
        Severity::Error,
        "the input is not a WebAssembly module",
    );

    assert_eq!(diagnostic.code(), DiagnosticCode::InvalidWasm);
    assert_eq!(diagnostic.severity(), Severity::Error);
    assert_eq!(
        diagnostic.message(),
        "the input is not a WebAssembly module"
    );
}

#[test]
fn a_diagnostic_renders_code_then_message_on_one_line() {
    let diagnostic = Diagnostic::new(
        DiagnosticCode::MissingEntrypoint,
        Severity::Error,
        "entry point `_start` is not exported by this module",
    );

    assert_eq!(
        diagnostic.to_string(),
        "WF005: entry point `_start` is not exported by this module"
    );
}

#[test]
fn help_is_optional_and_only_rendered_when_present() {
    let plain = Diagnostic::new(
        DiagnosticCode::UnresolvedImport,
        Severity::Error,
        "no provider",
    );
    assert_eq!(plain.to_string(), "WF002: no provider");
    assert!(plain.help().is_none());

    let with_help = plain.with_help("add the host function, or run without the import");
    assert_eq!(
        with_help.to_string(),
        "WF002: no provider\n  help: add the host function, or run without the import"
    );
}

#[test]
fn help_is_advice_and_never_changes_the_message() {
    let diagnostic = Diagnostic::new(
        DiagnosticCode::RuntimeTrap,
        Severity::Error,
        "guest trapped",
    )
    .with_help("check the guest for an out-of-bounds access");

    assert_eq!(diagnostic.message(), "guest trapped");
    assert_eq!(
        diagnostic.help(),
        Some("check the guest for an out-of-bounds access")
    );
}

#[test]
fn severity_is_distinct_from_the_code() {
    // The same cause can be an error in one context and a warning in another; a code
    // alone must not decide the exit behaviour.
    let error = Diagnostic::new(DiagnosticCode::InvalidWasm, Severity::Error, "rejected");
    let warning = Diagnostic::new(DiagnosticCode::InvalidWasm, Severity::Warning, "suspicious");

    assert_ne!(error.severity(), warning.severity());
    assert_eq!(error.code(), warning.code());
}

#[test]
fn a_diagnostic_compares_by_content_not_by_identity() {
    let first = Diagnostic::new(DiagnosticCode::InvalidWasm, Severity::Error, "same reason");
    let second = Diagnostic::new(DiagnosticCode::InvalidWasm, Severity::Error, "same reason");

    assert_eq!(first, second);
}
