// Tests for the entry point vocabulary.
//
// `EntryPoint` is the first code in `wf-core`, so it carries the crate's discipline as
// much as it carries behaviour: it is a name for the export a guest invocation starts
// at, not a string the runtime happens to look for.

use wf_core::EntryPoint;

#[test]
fn defaults_to_start() {
    assert_eq!(EntryPoint::default().as_str(), "_start");
}

#[test]
fn an_explicit_entry_point_keeps_its_name() {
    let entry = EntryPoint::new("main").expect("a non-empty name is valid");
    assert_eq!(entry.as_str(), "main");
}

#[test]
fn entry_points_compare_by_name() {
    let main = EntryPoint::new("main").expect("valid");
    assert_eq!(main, EntryPoint::new("main").expect("valid"));
    assert_ne!(main, EntryPoint::default());
}

#[test]
fn entry_points_render_the_name_they_hold() {
    let entry = EntryPoint::new("main").expect("valid");
    assert_eq!(format!("{entry}"), "main");
    assert_eq!(format!("{}", EntryPoint::default()), "_start");
}

#[test]
fn an_empty_entry_point_is_rejected() {
    // An empty export name cannot resolve to anything. Rejecting it here means the
    // failure names the mistake instead of surfacing later as a missing export.
    let rejected = EntryPoint::new("");
    assert!(
        rejected.is_err(),
        "an empty entry point must not be constructible"
    );
}

#[test]
fn entry_points_round_trip_through_the_cli_conversions() {
    let entry: EntryPoint = "_start".parse().expect("a non-empty name is valid");
    assert_eq!(entry, EntryPoint::default());

    let rejected: Result<EntryPoint, _> = "".parse();
    assert!(rejected.is_err(), "an empty name must not parse");
}

#[test]
fn an_entry_point_accepts_the_names_guests_actually_export() {
    for name in ["_start", "main", "initialize", "run", "_initialize"] {
        assert_eq!(
            EntryPoint::new(name)
                .expect("all of these are legal export names")
                .as_str(),
            name
        );
    }
}
