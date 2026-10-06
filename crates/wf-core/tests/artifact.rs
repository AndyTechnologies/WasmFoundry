// Tests for artifacts: the logical result of a build step.
//
// An artifact records *that* something was produced and *what kind of thing* it is.
// Where its bytes live is deliberately not part of it — that is a later layer's concern,
// and baking a path in now would make the domain know about storage it has no business
// knowing about.

use wf_core::{Artifact, ArtifactId, ArtifactKind};

#[test]
fn an_artifact_id_is_the_name_it_was_built_from() {
    let id = ArtifactId::new("hello").expect("a non-empty name is valid");
    assert_eq!(id.as_str(), "hello");
}

#[test]
fn an_empty_artifact_id_is_rejected() {
    // An artifact with no identity cannot be referenced by a graph or by a cache entry.
    assert!(ArtifactId::new("").is_err());
}

#[test]
fn an_artifact_carries_its_identity_and_kind() {
    let artifact = Artifact::new(
        ArtifactId::new("app").expect("valid"),
        ArtifactKind::CoreModule,
    );

    assert_eq!(artifact.id().as_str(), "app");
    assert_eq!(artifact.kind(), ArtifactKind::CoreModule);
}

#[test]
fn the_two_known_kinds_are_distinguishable() {
    let module = Artifact::new(
        ArtifactId::new("app").expect("valid"),
        ArtifactKind::CoreModule,
    );
    let component = Artifact::new(
        ArtifactId::new("app").expect("valid"),
        ArtifactKind::Component,
    );

    assert_ne!(module.kind(), component.kind());
}

#[test]
fn kinds_render_as_names_a_human_would_use() {
    assert_eq!(ArtifactKind::CoreModule.to_string(), "core module");
    assert_eq!(ArtifactKind::Component.to_string(), "component");
}

#[test]
fn artifacts_compare_by_identity_and_kind() {
    let first = Artifact::new(
        ArtifactId::new("app").expect("valid"),
        ArtifactKind::CoreModule,
    );
    let second = Artifact::new(
        ArtifactId::new("app").expect("valid"),
        ArtifactKind::CoreModule,
    );

    assert_eq!(first, second);
}
