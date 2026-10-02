//! Smoke test for the `wf-core` package manifest.
//!
//! `wf-core` currently contains no behaviour, so there is nothing to assert
//! about behaviour. What is worth verifying is that the package itself is
//! wired into the workspace and reports a version, so a manifest mistake
//! surfaces here instead of at release time.

/// The crate version is inherited from `[workspace.package]`, so it must
/// always be present and non-empty.
#[test]
fn crate_version_is_present() {
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
}
