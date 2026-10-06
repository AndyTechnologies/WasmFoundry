//! Artifacts: the logical result of a build step.

use std::fmt;

/// Identity of an artifact within a project.
///
/// A name rather than a path: the graph, a cache entry and a build report all refer to
/// an artifact by what it is called, and the moment this became a path the domain would
/// own storage decisions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtifactId {
    name: String,
}

impl ArtifactId {
    /// Builds an identity from a name.
    ///
    /// Rejects an empty name: an artifact nothing can refer to has no use, and finding
    /// that out here is cheaper than finding it out from a cache lookup.
    pub fn new(name: &str) -> Result<Self, ArtifactIdError> {
        if name.is_empty() {
            return Err(ArtifactIdError);
        }
        Ok(ArtifactId {
            name: name.to_owned(),
        })
    }

    /// The name this artifact is known by.
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Why an artifact identity could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactIdError;

impl fmt::Display for ArtifactIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the artifact name must not be empty")
    }
}

impl std::error::Error for ArtifactIdError {}

/// What an artifact is.
///
/// The kind decides who may consume it: a core module can be instantiated by the
/// runtime, and a component cannot until the component model lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    /// A compiled core WebAssembly module.
    CoreModule,
    /// A WebAssembly component.
    Component,
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ArtifactKind::CoreModule => "core module",
            ArtifactKind::Component => "component",
        })
    }
}

/// A produced artifact: what it is called, and what kind of thing it is.
///
/// Deliberately without a path. The bytes are wherever the layer that produced the
/// artifact put them, and asking this type where they are would make the domain own
/// storage — which is the layer below's job, and which arrives when a cache needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    id: ArtifactId,
    kind: ArtifactKind,
}

impl Artifact {
    /// Records an artifact.
    pub fn new(id: ArtifactId, kind: ArtifactKind) -> Self {
        Artifact { id, kind }
    }

    /// The identity this artifact is known by.
    pub fn id(&self) -> &ArtifactId {
        &self.id
    }

    /// What kind of thing was produced.
    pub fn kind(&self) -> ArtifactKind {
        self.kind
    }
}
