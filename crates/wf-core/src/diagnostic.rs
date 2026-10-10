//! How any layer reports a problem.

use std::error::Error as StdError;
use std::fmt;

/// A published diagnostic code.
///
/// The code is the stable part of a diagnostic: a script matches `WF005`, while the
/// message may be reworded at any time. Codes are assigned once and never reused, and
/// this enum only ever contains codes something can actually produce — a code listed
/// before its cause exists would describe a product that has not been built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    /// `WF001` — the input is not a compilable WebAssembly module.
    InvalidWasm,
    /// `WF002` — an import has no provider.
    UnresolvedImport,
    /// `WF003` — the project's modules form a dependency cycle.
    DependencyCycle,
    /// `WF004` — the toolchain a module asks for is not available.
    MissingToolchain,
    /// `WF004` covers both the toolchain this version does not implement and the
    /// toolchain that is not installed; the message is what tells them apart.
    /// `WF005` — the requested entry point is not exported.
    MissingEntrypoint,
    /// `WF009` — the guest faulted while executing.
    RuntimeTrap,
    /// `WF010` — the entry point exists but cannot be invoked as written.
    EntrypointUnsuitable,
    /// `WF011` — the manifest does not mean what it appears to mean.
    InvalidManifest,
    /// `WF012` — a toolchain ran and did not produce a module.
    ToolchainFailed,
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DiagnosticCode::InvalidWasm => "WF001",
            DiagnosticCode::UnresolvedImport => "WF002",
            DiagnosticCode::DependencyCycle => "WF003",
            DiagnosticCode::MissingToolchain => "WF004",
            DiagnosticCode::MissingEntrypoint => "WF005",
            DiagnosticCode::RuntimeTrap => "WF009",
            DiagnosticCode::EntrypointUnsuitable => "WF010",
            DiagnosticCode::InvalidManifest => "WF011",
            DiagnosticCode::ToolchainFailed => "WF012",
        })
    }
}

/// How serious a diagnostic is.
///
/// Deliberately separate from the code: the same cause can be fatal in one command and
/// merely worth mentioning in another, and the decision belongs to the caller rather
/// than being baked into the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// The operation did not happen and must not continue.
    Error,
    /// The operation continued, but something is worth the reader's attention.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        })
    }
}

/// One reported problem: what failed, how badly, and what can be done about it.
///
/// Structured rather than a string so that a caller can decide what to do from the code
/// and the severity instead of parsing prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    severity: Severity,
    message: String,
    help: Option<String>,
}

impl Diagnostic {
    /// Builds a diagnostic with no advice attached.
    pub fn new(code: DiagnosticCode, severity: Severity, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            severity,
            message: message.into(),
            help: None,
        }
    }

    /// Attaches what the reader can do about it.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// The stable code identifying the cause.
    pub fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// How serious the problem is.
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// What went wrong, in full sentences.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// What to do about it, when there is something useful to say.
    pub fn help(&self) -> Option<&str> {
        self.help.as_deref()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let Some(help) = &self.help {
            write!(f, "\n  help: {help}")?;
        }
        Ok(())
    }
}

impl StdError for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The display is what a terminal shows and what a log ships; it must never omit the
    /// code, because the code is the part a reader can act on.
    #[test]
    fn the_code_leads_the_rendering() {
        let diagnostic = Diagnostic::new(
            DiagnosticCode::RuntimeTrap,
            Severity::Error,
            "guest trapped: unreachable",
        );
        assert!(diagnostic.to_string().starts_with("WF009:"));
    }
}
