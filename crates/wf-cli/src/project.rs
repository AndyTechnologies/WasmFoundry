//! Loading order for a project's modules.
//!
//! Shared by `wf build` and `wf run` because both have to answer the same two questions
//! from the same evidence: which module does each import bind to, and in what order can
//! the set be instantiated. `wf build` uses it to refuse a project that cannot be loaded;
//! `wf run` uses the order it returns.

use wf_core::{
    Dependency, DependencyGraph, Diagnostic, DiagnosticCode, ModuleId, ModuleMatching, ModuleSpec,
    ResolveError, Severity, matching_modules, resolve_module,
};

/// The imports one module declares, after its source was analysed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    module: String,
    namespaces: Vec<String>,
}

impl ImportSummary {
    /// Records what `module` imports.
    pub fn new(module: impl Into<String>, namespaces: Vec<String>) -> Self {
        ImportSummary {
            module: module.into(),
            namespaces,
        }
    }

    /// The namespaces this module imports from.
    pub fn namespaces(&self) -> &[String] {
        &self.namespaces
    }
}

/// The order a project's modules must be loaded in, and who its entries are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectOrder {
    positions: Vec<usize>,
    roots: Vec<usize>,
}

impl ProjectOrder {
    /// Positions into the manifest's module list, dependencies first.
    ///
    /// A module appears after everything it needs, so loading in this order means an
    /// import is already available when the importing module is instantiated.
    pub fn positions(&self) -> &[usize] {
        &self.positions
    }

    /// Positions of modules nobody imports.
    ///
    /// More than one means the project has several independent entries, and which one
    /// to run is a question for the caller rather than a choice this module gets to
    /// make.
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }
}

/// Resolves every import and orders the modules for loading.
///
/// An import that matches no project module is left alone rather than reported: it is
/// an external import — a host ABI namespace, or one that will be provided later — and
/// whether it can be satisfied is a runtime question, not a manifest one. An import that
/// matches *two* project modules is an error, because nothing in the project says which
/// one the author meant.
pub fn plan(
    specs: &[ModuleSpec],
    imports: &[ImportSummary],
    mode: ModuleMatching,
    host_namespaces: &[String],
) -> Result<ProjectOrder, Diagnostic> {
    let mut graph = DependencyGraph::new();
    for spec in specs {
        graph.add_node(ModuleId::new(&spec.name).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::InvalidManifest,
                Severity::Error,
                format!("module `{}`: {error}", spec.name),
            )
            .with_help("give every module a non-empty `name`")
        })?);
    }

    for spec in specs {
        let Some(summary) = imports.iter().find(|summary| summary.module == spec.name) else {
            continue;
        };

        for namespace in summary.namespaces() {
            match resolve_module(mode, specs, namespace) {
                Ok(target) => {
                    let from = ModuleId::new(&spec.name).expect("validated above");
                    let to = ModuleId::new(&target.name).expect("validated above");
                    graph
                        .add_dependency(Dependency::new(from, to, namespace.clone()))
                        .map_err(|error| {
                            Diagnostic::new(
                                DiagnosticCode::InvalidManifest,
                                Severity::Error,
                                format!("module `{}`: {error}", spec.name),
                            )
                            .with_help("give every module a non-empty `name`")
                        })?;
                }
                Err(ResolveError::NotFound { .. }) if host_namespaces.contains(namespace) => {
                    // Declared as coming from outside the project — the host ABI, or a
                    // binding the user supplies. Allowed because it was written down.
                }
                Err(ResolveError::NotFound { .. }) => {
                    // Nothing provides it and it was not declared, so the likely cause
                    // is a typo: refusing here is what turns a link failure at run time
                    // into a build failure at edit time.
                    return Err(Diagnostic::new(
                        DiagnosticCode::UnresolvedImport,
                        Severity::Error,
                        format!(
                            "module `{}` imports `{}`, which no module provides",
                            spec.name, namespace
                        ),
                    )
                    .with_help(
                        "add `namespace` to the module that should provide it, or declare \
                         it in [project] host_namespaces if the host provides it",
                    ));
                }
                Err(ResolveError::Ambiguous { .. }) => {
                    // Two modules claim this namespace, so binding it would be a guess.
                    // The candidates are named because the fix is to rename one of
                    // them, and a count alone leaves the reader to hunt.
                    let claimants = matching_modules(mode, specs, namespace)
                        .iter()
                        .map(|module| module.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(Diagnostic::new(
                        DiagnosticCode::UnresolvedImport,
                        Severity::Error,
                        format!(
                            "module `{}` imports `{}`, which {} modules claim: {claimants}",
                            spec.name,
                            namespace,
                            matching_modules(mode, specs, namespace).len()
                        ),
                    )
                    .with_help("rename one of the modules, or change [project] module_matching"));
                }
            }
        }
    }

    let order = graph.topological_order().map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::DependencyCycle,
            Severity::Error,
            error.to_string(),
        )
        .with_help("a cycle cannot be instantiated: a module cannot need itself, at any depth")
    })?;

    let positions = order
        .iter()
        .map(|id| {
            specs
                .iter()
                .position(|spec| spec.name == id.as_str())
                .expect("every node came from a spec")
        })
        .collect();

    let roots = graph
        .roots()
        .iter()
        .map(|id| {
            specs
                .iter()
                .position(|spec| spec.name == id.as_str())
                .expect("every node came from a spec")
        })
        .collect();

    Ok(ProjectOrder { positions, roots })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One declared module.
    fn spec(name: &str, source: &str) -> ModuleSpec {
        ModuleSpec {
            name: name.to_owned(),
            source: source.to_owned(),
            toolchain: "precompiled".to_owned(),
            namespace: None,
        }
    }

    /// Two modules: `app` imports `engine`, and `engine` imports nothing.
    fn pair() -> (Vec<ModuleSpec>, Vec<ImportSummary>) {
        (
            vec![
                spec("app", "src/app.wasm"),
                spec("engine", "src/engine.wasm"),
            ],
            vec![
                ImportSummary::new("app", vec!["engine".to_owned()]),
                ImportSummary::new("engine", Vec::new()),
            ],
        )
    }

    #[test]
    fn a_dependency_is_loaded_before_its_dependent() {
        let (specs, imports) = pair();
        let order = plan(&specs, &imports, ModuleMatching::FileName, &[]).expect("acyclic");

        assert_eq!(order.positions(), &[1, 0], "engine is at index 1, app at 0");
    }

    #[test]
    fn the_module_nothing_imports_is_the_entry() {
        let (specs, imports) = pair();
        let order = plan(&specs, &imports, ModuleMatching::FileName, &[]).expect("acyclic");

        assert_eq!(order.roots(), &[0], "app is the only entry");
    }

    #[test]
    fn a_declared_host_namespace_is_not_a_project_dependency() {
        // `env` is a host namespace, not a project module. It is allowed because the
        // manifest says so, which is what turns "external" from a guess into a
        // statement.
        let (specs, mut imports) = pair();
        imports[1].namespaces = vec!["env".to_owned()];

        let hosts = ["env".to_owned()];
        let order = plan(&specs, &imports, ModuleMatching::FileName, &hosts)
            .expect("a declared host namespace is fine");
        assert_eq!(order.positions().len(), 2, "both modules still load");
        assert_eq!(order.roots(), &[0], "app still imports engine");
    }

    #[test]
    fn an_undeclared_namespace_is_refused_because_it_is_most_likely_a_typo() {
        // The same `env` import, undeclared: nothing provides it, and saying so at build
        // time is what turns a link failure at run time into an error at edit time.
        let (specs, mut imports) = pair();
        imports[1].namespaces = vec!["env".to_owned()];

        let error = plan(&specs, &imports, ModuleMatching::FileName, &[])
            .expect_err("undeclared namespaces are refused");
        assert_eq!(error.code(), DiagnosticCode::UnresolvedImport);
        assert!(error.message().contains("env"), "{}", error.message());
        let help = error.help().expect("must say how to fix it");
        assert!(help.contains("host_namespaces"), "{help}");
    }

    #[test]
    fn two_modules_claiming_one_namespace_are_refused() {
        // Two files called engine.wasm: binding the import would pick whichever the
        // author happened to list first.
        let specs = vec![
            spec("app", "src/app.wasm"),
            spec("engine", "src/engine.wasm"),
            spec("backup", "other/engine.wasm"),
        ];
        let imports = vec![
            ImportSummary::new("app", vec!["engine".to_owned()]),
            ImportSummary::new("engine", Vec::new()),
            ImportSummary::new("backup", Vec::new()),
        ];

        let error =
            plan(&specs, &imports, ModuleMatching::FileName, &[]).expect_err("ambiguous namespace");
        assert_eq!(error.code(), DiagnosticCode::UnresolvedImport);
        assert!(error.message().contains("engine"), "{}", error.message());
        assert!(error.help().is_some(), "{error}");
    }

    #[test]
    fn a_cycle_is_refused_rather_than_ordered() {
        let specs = vec![spec("a", "src/a.wasm"), spec("b", "src/b.wasm")];
        let imports = vec![
            ImportSummary::new("a", vec!["b".to_owned()]),
            ImportSummary::new("b", vec!["a".to_owned()]),
        ];

        let error = plan(&specs, &imports, ModuleMatching::FileName, &[]).expect_err("a cycle");
        assert_eq!(error.code(), DiagnosticCode::DependencyCycle);
        let message = error.message();
        assert!(message.contains("a") && message.contains("b"), "{message}");
    }

    #[test]
    fn a_module_importing_itself_is_refused() {
        let specs = vec![spec("a", "src/a.wasm")];
        let imports = vec![ImportSummary::new("a", vec!["a".to_owned()])];

        let error = plan(&specs, &imports, ModuleMatching::FileName, &[]).expect_err("self import");
        assert_eq!(error.code(), DiagnosticCode::DependencyCycle);
    }

    #[test]
    fn a_chain_is_ordered_end_to_end() {
        let specs = vec![
            spec("app", "src/app.wasm"),
            spec("engine", "src/engine.wasm"),
            spec("math", "src/math.wasm"),
        ];
        let imports = vec![
            ImportSummary::new("app", vec!["engine".to_owned()]),
            ImportSummary::new("engine", vec!["math".to_owned()]),
            ImportSummary::new("math", Vec::new()),
        ];

        let order = plan(&specs, &imports, ModuleMatching::FileName, &[]).expect("acyclic");
        assert_eq!(order.positions(), &[2, 1, 0]);
        assert_eq!(order.roots(), &[0]);
    }

    #[test]
    fn matching_mode_decides_which_module_a_namespace_binds_to() {
        // Declared name and file name disagree, so the manifest's rule decides whether
        // `app` binds to `engine` at all — and, since an unbound import is refused,
        // whether the project builds.
        let specs = vec![spec("app", "src/app.wasm"), spec("engine", "src/cog.wasm")];
        let imports = vec![
            ImportSummary::new("app", vec!["cog".to_owned()]),
            ImportSummary::new("engine", Vec::new()),
        ];

        let by_file = plan(&specs, &imports, ModuleMatching::FileName, &[]).expect("binds `cog`");
        assert_eq!(by_file.roots(), &[0], "app is the only entry");

        // Under the other rule nothing provides `cog`, and nothing declares it as coming
        // from outside either, so the build refuses. The mode did not merely change the
        // order — it changed whether the project is buildable at all.
        let error = plan(&specs, &imports, ModuleMatching::NameOnly, &[])
            .expect_err("name-only matching has no `cog`");
        assert_eq!(error.code(), DiagnosticCode::UnresolvedImport);
        assert!(error.message().contains("cog"), "{}", error.message());
    }
}
