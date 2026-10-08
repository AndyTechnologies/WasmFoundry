// Tests for the vocabulary that identifies how something is built.
//
// Two distinct types instead of one `target: &str` field: a guest target and a toolchain
// identity are different kinds of thing, and the plan's rule is that they must never be
// represented by the same field.

use wf_core::{GuestTarget, ToolchainId};

#[test]
fn a_toolchain_id_is_the_name_the_manifest_writes() {
    let id = ToolchainId::new("rust").expect("a known name is valid");
    assert_eq!(id.as_str(), "rust");
}

#[test]
fn an_empty_toolchain_id_is_rejected() {
    assert!(ToolchainId::new("").is_err());
}

#[test]
fn toolchain_ids_compare_by_name() {
    let rust = ToolchainId::new("rust").expect("valid");
    assert_eq!(rust, ToolchainId::new("rust").expect("valid"));
    assert_ne!(rust, ToolchainId::new("precompiled").expect("valid"));
}

#[test]
fn toolchain_ids_render_as_the_name_they_hold() {
    let id = ToolchainId::new("precompiled").expect("valid");
    assert_eq!(format!("{id}"), "precompiled");
}

#[test]
fn an_empty_toolchain_id_says_what_is_wrong() {
    let error = ToolchainId::new("").expect_err("empty is not an identity");
    assert!(error.to_string().contains("empty"), "{}", error.to_string());
}

#[test]
fn a_guest_target_is_a_triple() {
    let target = GuestTarget::new("wasm32-unknown-unknown").expect("a triple is valid");
    assert_eq!(target.as_str(), "wasm32-unknown-unknown");
}

#[test]
fn an_empty_guest_target_is_rejected() {
    assert!(GuestTarget::new("").is_err());
}

#[test]
fn guest_targets_compare_by_triple() {
    let wasm = GuestTarget::new("wasm32-unknown-unknown").expect("valid");
    assert_eq!(
        wasm,
        GuestTarget::new("wasm32-unknown-unknown").expect("valid")
    );
    assert_ne!(
        wasm,
        GuestTarget::new("wasm32-wasip1").expect("valid"),
        "different guest triples are different targets"
    );
}

#[test]
fn a_guest_target_is_not_a_toolchain() {
    // The whole point of two types: this must not compile if someone reaches for the
    // wrong one, and at runtime it must never be possible to mistake one for the other.
    let target = GuestTarget::new("wasm32-unknown-unknown").expect("valid");
    let toolchain = ToolchainId::new("rust").expect("valid");

    assert_eq!(target.as_str(), "wasm32-unknown-unknown");
    assert_eq!(toolchain.as_str(), "rust");
    assert_ne!(target.as_str(), toolchain.as_str());
}
