#[test]
fn p06c_published_store_caps_cover_every_store() {
    for store in [
        "experience",
        "atlas_snapshot",
        "atlas_lessons",
        "caddy_recipes",
        "caddy_hazards",
        "dossier_artifact",
        "cut",
        "trajectories",
    ] {
        assert!(super::value(store, "max_bytes") > 0);
    }
    assert!(super::value("trajectories", "max_age_days") > 0);
}
