//! How an import's namespace finds the project module that provides it.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use crate::ModuleSpec;

/// Which field of a module declaration an import's namespace is compared against.
///
/// The rule exists because the two answers differ: a module may be declared as `engine`
/// while its source lives at `src/cog.wasm`, and which one the namespace is matched
/// against decides whether an import binds at all. Silently picking one would make the
/// project's behaviour depend on a convention nobody wrote down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ModuleMatching {
    /// Match the file name of the module's source, without its extension.
    #[default]
    FileName,
    /// Match the `name` field of the module declaration.
    NameOnly,
}

impl fmt::Display for ModuleMatching {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ModuleMatching::FileName => "file-name",
            ModuleMatching::NameOnly => "name-only",
        })
    }
}

/// Why a `module_matching` value could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseModuleMatchingError {
    value: String,
}

impl fmt::Display for ParseModuleMatchingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown module matching mode `{}`; expected `file-name` or `name-only`",
            self.value
        )
    }
}

impl std::error::Error for ParseModuleMatchingError {}

impl FromStr for ModuleMatching {
    type Err = ParseModuleMatchingError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "file-name" => Ok(ModuleMatching::FileName),
            "name-only" => Ok(ModuleMatching::NameOnly),
            other => Err(ParseModuleMatchingError {
                value: other.to_owned(),
            }),
        }
    }
}

/// Why a namespace could not be bound to exactly one module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// No module matches under the active rule.
    NotFound {
        /// The namespace the import asked for.
        namespace: String,
        /// The rule that was applied.
        mode: ModuleMatching,
    },
    /// More than one module matches, and choosing one would be a guess.
    Ambiguous {
        /// The namespace the import asked for.
        namespace: String,
        /// Every module that matched, in declaration order.
        candidates: Vec<String>,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::NotFound { namespace, mode } => {
                write!(
                    f,
                    "no module matches the namespace `{namespace}` using `{mode}` matching",
                    namespace = namespace,
                    mode = mode
                )
            }
            ResolveError::Ambiguous {
                namespace,
                candidates,
            } => write!(
                f,
                "the namespace `{namespace}` matches {} modules: {}",
                candidates.len(),
                candidates.join(", ")
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Binds a namespace to the module that provides it.
///
/// Ambiguity is an error rather than a first-match rule: when two modules both claim a
/// namespace, the import binds to whichever one the reader happened to list first, and
/// that is a behaviour nobody chose.
pub fn resolve_module<'a>(
    mode: ModuleMatching,
    modules: &'a [ModuleSpec],
    namespace: &str,
) -> Result<&'a ModuleSpec, ResolveError> {
    let matches: Vec<&ModuleSpec> = modules
        .iter()
        .filter(|module| matches_namespace(mode, module, namespace))
        .collect();

    match matches.as_slice() {
        [] => Err(ResolveError::NotFound {
            namespace: namespace.to_owned(),
            mode,
        }),
        [only] => Ok(only),
        several => Err(ResolveError::Ambiguous {
            namespace: namespace.to_owned(),
            candidates: several.iter().map(|module| module.name.clone()).collect(),
        }),
    }
}

/// The namespace a module publishes its exports under.
///
/// The same rule that resolves an import decides what a module publishes as. If the two
/// sides were computed differently, an import could be bound to a module that never
/// publishes under that name.
///
/// `None` when the rule cannot produce a name — a file-name match over a source with no
/// file name has nothing to bind to.
pub fn namespace_of(mode: ModuleMatching, module: &ModuleSpec) -> Option<&str> {
    match mode {
        ModuleMatching::NameOnly => Some(module.name.as_str()),
        ModuleMatching::FileName => Path::new(&module.source).file_stem()?.to_str(),
    }
}

/// Applies the active rule to one module declaration.
fn matches_namespace(mode: ModuleMatching, module: &ModuleSpec, namespace: &str) -> bool {
    match mode {
        ModuleMatching::NameOnly => module.name == namespace,
        ModuleMatching::FileName => Path::new(&module.source)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem == namespace),
    }
}
