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

/// Every module that publishes under a namespace, in declaration order.
///
/// Public because a diagnostic that has to name the modules claiming a namespace should
/// not have to re-implement the rule to find out which they are.
pub fn matching_modules<'a>(
    mode: ModuleMatching,
    modules: &'a [ModuleSpec],
    namespace: &str,
) -> Vec<&'a ModuleSpec> {
    modules
        .iter()
        .filter(|module| namespace_of(mode, module) == Some(namespace))
        .collect()
}

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
    match matching_modules(mode, modules, namespace).as_slice() {
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
/// The same function binds an import and publishes an export. If the two sides were
/// computed differently, an import could bind to a module that never publishes under
/// that name, which is a failure with no message at all.
///
/// A namespace stated in the manifest wins: it is the one place where the author has
/// written down what this module is called for linking purposes. Without one, the active
/// mode decides between the declared name and the source file's name — and an empty
/// stated namespace counts as not stated, because no import could ever bind to it.
///
/// `None` when nothing can produce a name.
pub fn namespace_of(mode: ModuleMatching, module: &ModuleSpec) -> Option<&str> {
    if let Some(stated) = module
        .namespace
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        return Some(stated);
    }

    match mode {
        ModuleMatching::NameOnly => Some(module.name.as_str()),
        ModuleMatching::FileName => Path::new(&module.source).file_stem()?.to_str(),
    }
}
