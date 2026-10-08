//! How something gets built: which toolchain, for which guest.

use std::fmt;

use crate::NameError;

/// Identity of the toolchain that turns a source into a module.
///
/// A name, not an enum: the set of toolchains grows with the product, and a closed set
/// would mean editing the domain every time one arrives. Which names are *available* is
/// decided by the layer that owns the implementations, and a manifest naming a toolchain
/// nobody implements is rejected there rather than by a missing variant here.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolchainId {
    name: String,
}

impl ToolchainId {
    /// Builds an identity from a toolchain name.
    pub fn new(name: &str) -> Result<Self, NameError> {
        if name.is_empty() {
            return Err(NameError::new("toolchain id"));
        }
        Ok(ToolchainId {
            name: name.to_owned(),
        })
    }

    /// The name the manifest writes and the registry looks up.
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for ToolchainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// A target triple for the guest: the platform the WebAssembly runs on.
///
/// Deliberately not `Target`, and deliberately not shared with a host triple. The plan
/// is explicit that a single `target` field for both is a mistake — bundling for
/// `aarch64-unknown-linux-gnu` has nothing to do with a guest targeting
/// `wasm32-unknown-unknown`. A separate host type arrives when bundling does (PHASE 14),
/// and until then there is only one kind of target, so nothing can conflate them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuestTarget {
    triple: String,
}

impl GuestTarget {
    /// Builds a guest target from a triple.
    pub fn new(triple: &str) -> Result<Self, NameError> {
        if triple.is_empty() {
            return Err(NameError::new("guest target"));
        }
        Ok(GuestTarget {
            triple: triple.to_owned(),
        })
    }

    /// The triple this target names.
    pub fn as_str(&self) -> &str {
        &self.triple
    }
}

impl fmt::Display for GuestTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.triple)
    }
}
