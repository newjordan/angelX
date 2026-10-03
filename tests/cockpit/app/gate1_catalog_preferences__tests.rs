use super::*;
use crate::agent::club::{Bag, Club};
use crate::agent::codex_catalog::{CatalogLoad, Source};

struct FixtureDir(PathBuf);
impl FixtureDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-gate1-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for FixtureDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

// A catalog-backed fixture Club avoids Codex's unrelated constructor env knobs
// and authentication setup; restore_from and CatalogLoad::resolve are real paths.
struct PinnedRoute {
    catalog: CatalogLoad,
    requested: Mutex<String>,
    levels: Vec<String>,
}
impl PinnedRoute {
    fn requested_effort(&self) -> Option<String> {
        Some(self.requested.lock().unwrap().clone())
    }
    fn capture_selection(
        &self,
        override_effort: Option<&str>,
    ) -> Result<crate::agent::codex_catalog::EffectiveSelection, String> {
        self.catalog.resolve(
            "gate1-model",
            override_effort.or(self.requested_effort().as_deref()),
            false,
        )
    }
}
impl Club for PinnedRoute {
    fn label(&self) -> &str {
        "openai"
    }
    fn live_model_name(&self) -> Option<String> {
        Some("gate1-model".into())
    }
    fn reasoning_levels(&self) -> &[String] {
        &self.levels
    }
    fn reasoning_effort(&self) -> Option<String> {
        self.capture_selection(None)
            .map(|s| s.wire)
            .unwrap_or_else(|_| self.requested_effort())
    }
    fn set_reasoning_effort(&self, effort: &str) -> Option<String> {
        let wire = self.capture_selection(Some(effort)).ok()?.wire?;
        *self.requested.lock().unwrap() = wire.clone();
        Some(wire)
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        panic!("fixture must never dispatch")
    }
}

fn pinned_route(effort: &str) -> Arc<PinnedRoute> {
    let spec = CodexModelInfo {
        slug: "gate1-model".into(),
        display_name: "Fixture".into(),
        context_window: Some(8192),
        supported_reasoning_levels: ["low", "high"]
            .into_iter()
            .map(|effort| CodexReasoningLevel {
                effort: effort.into(),
                description: String::new(),
            })
            .collect(),
        pi_thinking: [("minimal".into(), Some("low".into()))]
            .into_iter()
            .collect(),
        ..Default::default()
    };
    Arc::new(PinnedRoute {
        catalog: CatalogLoad::from_models(Source::PiStore, vec![spec]),
        requested: Mutex::new(effort.into()),
        levels: vec!["low".into(), "high".into()],
    })
}

#[test]
fn gate1_common_restore_preserves_applicable_provider_pin_after_route_restore() {
    // Explicit preference path + injected pin presence exercise the shared
    // production path without cfg!(test)'s wrapper bypass or ambient pin/HOME/auth reads.
    let root = FixtureDir::new("preferences");
    let path = root.0.join("brain-route.json");
    std::fs::write(&path, r#"{"v":1,"agent":"openai","driver":"openai","model":"gate1-model","reasoning_effort":"high"}"#).unwrap();
    for requested in ["pi:minimal", "unsupported"] {
        let club = pinned_route(requested);
        let mut bag = Bag::for_reasoning_render_test();
        bag.replace_in_hand_club_for_test(club.clone());
        assert!(bag.restore_route(1, 0)); // Before restoration: unrelated namespace.
        let result = crate::platform::route_preferences::restore_from(
            &mut bag,
            &path,
            false,
            false,
            |key| key == "ANGEL_OPENAI_REASONING_EFFORT",
        );
        assert!(result.route);
        assert!(!result.effort);
        assert_eq!(club.requested_effort().as_deref(), Some(requested));
        let effective = club.capture_selection(None);
        if requested == "pi:minimal" {
            assert_eq!(effective.unwrap().wire.as_deref(), Some("low"));
        } else {
            assert!(
                effective.is_err(),
                "saved THINK must not silently repair invalid intent"
            );
        }
        assert_eq!(bag.set_reasoning_effort("high").as_deref(), Some("high"));
        assert_eq!(
            club.capture_selection(None).unwrap().wire.as_deref(),
            Some("high")
        );
    }
    let club = pinned_route("pi:minimal");
    let mut bag = Bag::for_reasoning_render_test();
    bag.replace_in_hand_club_for_test(club.clone());
    let result =
        crate::platform::route_preferences::restore_from(&mut bag, &path, false, false, |key| {
            key == "ANGEL_OPENAI_API_REASONING_EFFORT"
        });
    assert!(
        result.effort,
        "an unrelated provider pin cannot block saved THINK"
    );
    assert_eq!(club.requested_effort().as_deref(), Some("high"));
}

fn legacy_json(window: Option<&str>) -> String {
    format!(
        r#"{{"models":[{{"slug":"gpt-5.6-luna","display_name":"Luna",{}"default_reasoning_level":"max","supported_reasoning_levels":[{{"effort":"max"}}]}}]}}"#,
        window
            .map(|v| format!("\"context_window\":{v},"))
            .unwrap_or_default()
    )
}

#[test]
fn gate1_legacy_json_malformed_window_drops_only_that_budget() {
    let root = FixtureDir::new("legacy-capacity");
    let path = root.0.join("models_cache.json");
    for invalid in ["18446744073709551616", "-1", "8192.5"] {
        let raw = legacy_json(Some(invalid));
        let models = checked_model_catalog_from_str(&raw).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].context_window, None);
        std::fs::write(&path, raw).unwrap();
        let load =
            CatalogLoad::load_checked(None, || checked_model_catalog_from_path(path.clone()));
        assert_eq!(load.source, Source::Legacy);
        // Codex's own cache shape is not ours to police: the route stays usable.
        assert!(
            load.resolve(OPENAI_LUNA_MODEL, Some(OPENAI_LUNA_EFFORT), true)
                .is_ok()
        );
    }
}

#[test]
fn gate1_checked_legacy_missing_malformed_and_unknown_context_keep_native_contract() {
    let root = FixtureDir::new("legacy-compat");
    let path = root.0.join("models_cache.json");
    let missing = CatalogLoad::load_checked(None, || checked_model_catalog_from_path(path.clone()));
    assert!(missing.models().is_empty());
    assert!(
        missing
            .resolve(OPENAI_LUNA_MODEL, Some(OPENAI_LUNA_EFFORT), true)
            .is_ok()
    );
    for raw in [
        "not JSON".to_string(),
        r#"{"models":"malformed"}"#.into(),
        r#"{"models":[{"slug":false,"context_window":-1}]}"#.into(),
        legacy_json(None),
        legacy_json(Some("null")),
    ] {
        std::fs::write(&path, raw).unwrap();
        let load =
            CatalogLoad::load_checked(None, || checked_model_catalog_from_path(path.clone()));
        assert!(load.checked.is_ok());
        assert!(
            load.resolve(OPENAI_LUNA_MODEL, Some(OPENAI_LUNA_EFFORT), true)
                .is_ok()
        );
        assert!(load.models().iter().all(|m| m.context_window.is_none()));
    }
    std::fs::write(&path, legacy_json(Some("8192"))).unwrap();
    let load = CatalogLoad::load_checked(None, || checked_model_catalog_from_path(path.clone()));
    assert_eq!(load.models()[0].context_window, Some(8192));
    assert!(
        load.resolve(OPENAI_LUNA_MODEL, Some(OPENAI_LUNA_EFFORT), false)
            .is_ok()
    );
}
