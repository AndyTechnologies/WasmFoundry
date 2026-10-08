//! The one error every identity constructor shares.

use std::error::Error as StdError;
use std::fmt;

/// Why a name-based identity could not be constructed.
///
/// [`EntryPoint`](crate::EntryPoint), [`ArtifactId`](crate::ArtifactId),
/// [`ToolchainId`](crate::ToolchainId) and [`GuestTarget`](crate::GuestTarget) are all
/// references to something by name, and they all fail for the same reason: the name is
/// empty. One error type keeps that message consistent instead of four copies drifting
/// apart, and the `what` field is what tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameError {
    what: &'static str,
}

impl NameError {
    /// Builds the error for an identity of the given kind.
    pub fn new(what: &'static str) -> Self {
        NameError { what }
    }

    /// The explanation, for callers that want it without formatting.
    pub fn reason(&self) -> String {
        format!("the {} must not be empty", self.what)
    }
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason())
    }
}

impl StdError for NameError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendering is what a user reads from `--entry` or from a manifest error, so it
    /// must name the thing that was wrong rather than say "invalid".
    #[test]
    fn the_message_names_the_identity_that_was_empty() {
        let entry = NameError::new("entry point name").reason();
        assert!(entry.contains("entry point name"), "{entry}");
        assert!(entry.contains("empty"), "{entry}");
    }

    #[test]
    fn display_matches_the_reason_so_the_two_never_disagree() {
        let error = NameError::new("toolchain id");
        assert_eq!(error.to_string(), error.reason());
    }
}
