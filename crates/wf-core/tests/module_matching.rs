// Tests for module matching: how an import's namespace finds a project module.
//
// The plan is explicit that this must not be assumed trivial. Two modules can have the
// same file name in different directories, and a project can name a module after
// something other than its file, so the rule that picks a module has to be stated.

use wf_core::{ModuleMatching, ModuleSpec, ResolveError, resolve_module};

/// Builds a module declaration.
fn module(name: &str, source: &str) -> ModuleSpec {
    ModuleSpec {
        name: name.to_owned(),
        source: source.to_owned(),
        toolchain: "precompiled".to_owned(),
    }
}

/// One project: `src/app.wasm` and `src/engine.wasm`.
fn project() -> Vec<ModuleSpec> {
    vec![
        module("app", "src/app.wasm"),
        module("engine", "src/engine.wasm"),
    ]
}

#[test]
fn file_name_matching_compares_the_source_stem() {
    let modules = project();
    let resolved = resolve_module(ModuleMatching::FileName, &modules, "engine").expect("exists");

    assert_eq!(resolved.name, "engine");
    assert_eq!(resolved.source, "src/engine.wasm");
}

#[test]
fn name_only_matching_compares_the_declared_name() {
    // The same project resolves by the manifest's `name` field instead of the file.
    let modules = vec![module("engine", "src/cog.wasm")];
    let resolved = resolve_module(ModuleMatching::NameOnly, &modules, "engine").expect("exists");

    assert_eq!(resolved.source, "src/cog.wasm");
}

#[test]
fn the_two_modes_can_disagree_about_the_same_module() {
    // Declared name and file name differ; the mode decides which one counts.
    let modules = vec![module("engine", "src/cog.wasm")];

    assert!(resolve_module(ModuleMatching::NameOnly, &modules, "engine").is_ok());
    assert!(resolve_module(ModuleMatching::FileName, &modules, "engine").is_err());

    assert!(resolve_module(ModuleMatching::FileName, &modules, "cog").is_ok());
    assert!(resolve_module(ModuleMatching::NameOnly, &modules, "cog").is_err());
}

#[test]
fn an_unknown_namespace_reports_what_was_looked_for_and_under_which_rule() {
    let modules = project();
    let error =
        resolve_module(ModuleMatching::FileName, &modules, "database").expect_err("no match");

    match &error {
        ResolveError::NotFound { namespace, mode } => {
            assert_eq!(namespace, "database");
            assert_eq!(*mode, ModuleMatching::FileName);
        }
        other => panic!("expected not found, got {other:?}"),
    }
    assert!(error.to_string().contains("database"), "{error}");
}

#[test]
fn two_modules_sharing_a_file_name_are_reported_as_ambiguous() {
    // The realistic collision: two directories, same file name. Picking one silently
    // would bind an import to a module the project author may not have meant.
    let modules = vec![
        module("app", "src/engine.wasm"),
        module("engine", "other/engine.wasm"),
    ];

    let error =
        resolve_module(ModuleMatching::FileName, &modules, "engine").expect_err("ambiguous");
    match &error {
        ResolveError::Ambiguous {
            namespace,
            candidates,
        } => {
            assert_eq!(namespace, "engine");
            assert_eq!(candidates.len(), 2, "{candidates:?}");
            assert!(candidates.contains(&"app".to_owned()), "{candidates:?}");
            assert!(candidates.contains(&"engine".to_owned()), "{candidates:?}");
        }
        other => panic!("expected ambiguity, got {other:?}"),
    }
    assert!(error.to_string().contains("engine"), "{error}");
}

#[test]
fn a_module_never_matches_itself_twice_by_accident() {
    // A single module must resolve once, even when its name and its file name agree.
    let modules = vec![module("app", "src/app.wasm")];
    let resolved = resolve_module(ModuleMatching::FileName, &modules, "app").expect("matches");
    assert_eq!(resolved.name, "app");
}

#[test]
fn the_default_matches_the_convention_most_projects_would_write() {
    // A manifest that does not say gets the same answer as one that says file-name.
    assert_eq!(ModuleMatching::default(), ModuleMatching::FileName);
}

#[test]
fn the_modes_render_as_the_values_a_manifest_writes() {
    assert_eq!(ModuleMatching::FileName.to_string(), "file-name");
    assert_eq!(ModuleMatching::NameOnly.to_string(), "name-only");
    assert_eq!(
        "file-name".parse::<ModuleMatching>().unwrap(),
        ModuleMatching::FileName
    );
    assert_eq!(
        "name-only".parse::<ModuleMatching>().unwrap(),
        ModuleMatching::NameOnly
    );
}

#[test]
fn an_unrecognised_mode_is_rejected_with_the_ones_that_exist() {
    let error = "whatever"
        .parse::<ModuleMatching>()
        .expect_err("unknown mode");
    let message = error.to_string();
    assert!(message.contains("file-name"), "{message}");
    assert!(message.contains("name-only"), "{message}");
}

#[test]
fn the_namespace_a_module_publishes_under_follows_the_active_rule() {
    // The same rule that resolves an import decides what the module publishes as, or the
    // two sides of a link would never meet.
    let module = module("engine", "src/cog.wasm");

    assert_eq!(
        Some("cog"),
        wf_core::namespace_of(ModuleMatching::FileName, &module)
    );
    assert_eq!(
        Some("engine"),
        wf_core::namespace_of(ModuleMatching::NameOnly, &module)
    );
}

#[test]
fn a_source_without_a_file_name_publishes_nothing() {
    // A trailing slash does not remove the file name — `src/` is the directory `src`,
    // whose stem is `src`. What has no stem is a path that names no file at all.
    let directory = module("app", "src/");
    assert_eq!(
        wf_core::namespace_of(ModuleMatching::FileName, &directory),
        Some("src"),
        "a directory path still has a last component to name"
    );

    let no_file = module("app", ".");
    assert_eq!(
        wf_core::namespace_of(ModuleMatching::FileName, &no_file),
        None,
        "with no file name there is nothing to bind to"
    );
}
