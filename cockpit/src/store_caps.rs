//! Published store limits embedded with the binary, independent of process cwd.
use std::sync::OnceLock;

pub(crate) fn value(store: &str, key: &str) -> u64 {
    static CAPS: OnceLock<toml::Value> = OnceLock::new();
    let caps = CAPS.get_or_init(|| {
        include_str!("../../docs/telemetry/store-caps.toml")
            .parse()
            .expect("store-caps.toml must be valid TOML")
    });
    caps.get(store)
        .and_then(|s| s.get(key))
        .and_then(toml::Value::as_integer)
        .and_then(|n| u64::try_from(n).ok())
        .filter(|n| *n > 0)
        .expect("published store limit must be a positive integer")
}

#[cfg(test)]
mod tests {
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
}
