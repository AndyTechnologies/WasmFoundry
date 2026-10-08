//! The export a guest invocation starts at.

use std::fmt;
use std::str::FromStr;

use crate::NameError;

/// The entry point a runtime invocation begins at.
///
/// This is a domain value, not a string: the default `_start`, the fact that an empty
/// name is not constructible, and the rendering are all here rather than spread across
/// the runtime and the CLI. Nothing downstream has to remember which export to look for.
///
/// A guest may export anything — `_start`, `_initialize`, `main`, `run` — so the value
/// is constructed explicitly rather than inferred from the module.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryPoint {
    name: String,
}

impl EntryPoint {
    /// Builds an entry point from an export name.
    ///
    /// Returns an error for an empty name: no module exports an empty name, so
    /// constructing one only delays the failure to a point where the message is worse.
    pub fn new(name: &str) -> Result<Self, NameError> {
        if name.is_empty() {
            return Err(NameError::new("entry point name"));
        }
        Ok(EntryPoint {
            name: name.to_owned(),
        })
    }

    /// The export name to look up in the guest.
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl Default for EntryPoint {
    /// `_start` — the convention followed by modules built for Wasmtime's command
    /// entry, and by the examples in this project.
    fn default() -> Self {
        EntryPoint {
            name: "_start".to_owned(),
        }
    }
}

impl fmt::Display for EntryPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

impl FromStr for EntryPoint {
    type Err = NameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        EntryPoint::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The name the runtime resolves when the caller does not say otherwise.
    #[test]
    fn start_is_the_convention_by_default() {
        assert_eq!(EntryPoint::default().as_str(), "_start");
    }

    #[test]
    fn rejecting_an_empty_name_reports_why() {
        let error = EntryPoint::new("").expect_err("an empty name is not an entry point");
        assert!(error.reason().contains("empty"), "{}", error.reason());
        assert_eq!(error.to_string(), error.reason());
    }
}
