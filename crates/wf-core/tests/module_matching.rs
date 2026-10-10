// Tests for module matching: how an import's namespace finds a project module.
//
// The rule can be stated explicitly per module, in which case the file name and the
// declared name stop mattering for linking. Without a stated namespace, the
// `module_matching` mode decides which of the two counts — because a module declared as
// `engine` may well have its source at `src/cog.wasm`, and which one binds an import is a
// decision that has to be written down somewhere.

use wf_core::{ModuleMatching, ModuleSpec, ResolveError, namespace_of, resolve_module};

/// Builds a module declaration.
fn module(name: &str, source: &str) -> ModuleSpec {
    ModuleSpec {
        name: name.to_owned(),
        source: source.to_owned(),
        toolchain: "precompiled".to_owned(),
        namespace: None,
    }
}

/// The same declaration with a namespace stated in the manifest.
fn module_with_namespace(name: &str, source: &str, namespace: &str) -> ModuleSpec {
    ModuleSpec {
        namespace: Some(namespace.to_owned()),
        ..module(name, source)
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
    let modules = vec![module("engine", "src/cog.wasm")];
    let resolved = resolve_module(ModuleMatching::NameOnly, &modules, "engine").expect("exists");

    assert_eq!(resolved.source, "src/cog.wasm");
}

#[test]
fn the_two_modes_can_disagree_about_the_same_module() {
    let modules = vec![module("engine", "src/cog.wasm")];

    assert!(resolve_module(ModuleMatching::NameOnly, &modules, "engine").is_ok());
    assert!(resolve_module(ModuleMatching::FileName, &modules, "engine").is_err());

    assert!(resolve_module(ModuleMatching::FileName, &modules, "cog").is_ok());
    assert!(resolve_module(ModuleMatching::NameOnly, &modules, "cog").is_err());
}

#[test]
fn a_stated_namespace_beats_both_the_name_and_the_file() {
    // The manifest says what the module publishes as, so neither the declared name nor
    // the source file decides anymore.
    let modules = vec![module_with_namespace("engine", "src/cog.wasm", "cogwheel")];

    assert!(resolve_module(ModuleMatching::FileName, &modules, "cogwheel").is_ok());
    assert!(resolve_module(ModuleMatching::NameOnly, &modules, "cogwheel").is_ok());

    // Both modes now reject what the mode would otherwise have bound.
    assert!(resolve_module(ModuleMatching::FileName, &modules, "cog").is_err());
    assert!(resolve_module(ModuleMatching::NameOnly, &modules, "engine").is_err());
}

#[test]
fn a_stated_namespace_is_what_gets_published_under() {
    let module = module_with_namespace("engine", "src/cog.wasm", "cogwheel");
    assert_eq!(
        Some("cogwheel"),
        namespace_of(ModuleMatching::FileName, &module)
    );
    assert_eq!(
        Some("cogwheel"),
        namespace_of(ModuleMatching::NameOnly, &module)
    );
}

#[test]
fn an_empty_stated_namespace_is_treated_as_not_stated() {
    // An empty namespace cannot bind anything, so it is the same as not stating one,
    // rather than a module that publishes under no name at all.
    let module = ModuleSpec {
        namespace: Some(String::new()),
        ..module("engine", "src/cog.wasm")
    };
    assert_eq!(Some("cog"), namespace_of(ModuleMatching::FileName, &module));
}

#[test]
fn two_modules_stating_one_namespace_are_ambiguous() {
    // An explicit namespace does not make a collision less real.
    let modules = vec![
        module_with_namespace("a", "src/a.wasm", "shared"),
        module_with_namespace("b", "src/b.wasm", "shared"),
    ];

    let error =
        resolve_module(ModuleMatching::FileName, &modules, "shared").expect_err("collision");
    match &error {
        ResolveError::Ambiguous { candidates, .. } => {
            assert_eq!(candidates.len(), 2, "{candidates:?}");
        }
        other => panic!("expected ambiguity, got {other:?}"),
    }
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
fn every_matching_module_is_listed_for_a_message_that_can_name_them() {
    // A diagnostic that has to say which modules claim a namespace should not have to
    // re-implement the rule to find out.
    let modules = vec![
        module("app", "src/engine.wasm"),
        module("engine", "other/engine.wasm"),
        module("other", "src/app.wasm"),
    ];

    let claimants = wf_core::matching_modules(ModuleMatching::FileName, &modules, "engine");
    assert_eq!(claimants.len(), 2);
    assert_eq!(claimants[0].name, "app");
    assert_eq!(claimants[1].name, "engine");
}

#[test]
fn a_module_never_matches_itself_twice_by_accident() {
    let modules = vec![module("app", "src/app.wasm")];
    let resolved = resolve_module(ModuleMatching::FileName, &modules, "app").expect("matches");
    assert_eq!(resolved.name, "app");
}

#[test]
fn the_default_matches_the_convention_most_projects_would_write() {
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
    let module = module("engine", "src/cog.wasm");

    assert_eq!(Some("cog"), namespace_of(ModuleMatching::FileName, &module));
    assert_eq!(
        Some("engine"),
        namespace_of(ModuleMatching::NameOnly, &module)
    );
}

#[test]
fn a_source_without_a_file_name_publishes_nothing() {
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
