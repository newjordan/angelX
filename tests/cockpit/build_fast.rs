//! The realm host's build script's working-build choices.
#[path = "../../crates/realm-host/build_fast.rs"]
mod build_fast;
#[path = "../../cockpit/native/build_parallel.rs"]
mod build_parallel;
#[path = "../../cockpit/build_fast.rs"]
mod cockpit_build_fast;

#[test]
fn build_script_selects_the_nested_profile() {
    for src in [
        include_str!("../../cockpit/web-bundle/build.rs"),
        include_str!("../../crates/realm-host/build.rs"),
    ] {
        assert!(src.contains("nested_web_profile"));
        assert!(
            !src.contains("--profile\", \"web\""),
            "the nested browser build must not hard-code profile web"
        );
    }
}

#[test]
fn delve_build_does_not_watch_a_missing_kit() {
    let src = include_str!("../../crates/delve/build.rs");
    assert!(
        src.contains("if kit_dir.exists()"),
        "a missing kit folder must not be watched"
    );
    assert!(
        src.contains("if !map_path.is_file()"),
        "a missing slot map must not be watched"
    );
}

// Shared encoder/decoder: the build script and lazy browser loader use this
// exact source, so these tests do not substitute a test-only packing format.
#[path = "../../cockpit/bundle_bytes.rs"]
mod bundle_bytes;

// The Delve build script shares this focused build-input policy gate.
#[path = "../../crates/delve/build_watch.rs"]
mod delve_build_watch;
