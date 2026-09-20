//! Durable operator choice for the interactive Brain Route controls.
//!
//! This is deliberately a preference, not a routing mandate: explicit
//! environment pins win, unavailable routes are never selected, malformed or
//! stale files are ignored, and the Bag's normal election remains the fallback.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::club::{Bag, RouteChoice};

const SCHEMA_V: u8 = 2;
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RoutePreference {
    v: u8,
    #[serde(default)]
    route_id: Option<crate::backplane::RouteId>,
    agent: String,
    driver: String,
    model: String,
    reasoning_effort: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RestoreResult {
    pub(crate) route: bool,
    pub(crate) effort: bool,
}

fn enabled() -> bool {
    std::env::var("ANGEL_ROUTE_MEMORY")
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            )
        })
        .unwrap_or(true)
}

pub(crate) fn preference_path() -> PathBuf {
    std::env::var_os("ANGEL_ROUTE_MEMORY_FILE")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::workspace_store::angel_dir().join("brain-route.json"))
}

fn route_pin_active() -> bool {
    std::env::var_os("ANGEL_DRIVER").is_some_and(|value| route_pin_is_concrete(&value))
}

/// The MoA wrapper is an engagement target, not an ordinary Brain Route. Older
/// cockpit setups exported `ANGEL_DRIVER=sota-moa` globally; allowing that
/// legacy value to claim the interactive route pin strands every normal turn on
/// the wrapper and prevents the operator's last concrete model from restoring.
/// Headless `--ask`/`--task` still honor the value in [`Bag::standard`]; only the
/// interactive route-memory layer treats it as non-concrete.
fn route_pin_is_concrete(value: &std::ffi::OsStr) -> bool {
    let value = value.to_string_lossy();
    let value = value.trim();
    !value.is_empty() && !value.eq_ignore_ascii_case("sota-moa")
}

fn effort_pin_active() -> bool {
    std::env::var_os("ANGEL_REASONING_EFFORT").is_some_and(|value| !value.is_empty())
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn matches(choice: &RouteChoice, preference: &RoutePreference) -> bool {
    matches_with_backplane(choice, preference, crate::backplane::active())
}

fn matches_with_backplane(
    choice: &RouteChoice,
    preference: &RoutePreference,
    use_stable_id: bool,
) -> bool {
    if use_stable_id && let Some(expected) = preference.route_id.as_ref() {
        return crate::backplane::RouteId::chat(
            &choice.agent,
            &choice.driver,
            preference.reasoning_effort.as_deref(),
        ) == *expected;
    }
    same(&choice.agent, &preference.agent)
        && same(&choice.driver, &preference.driver)
        && same(&choice.model, &preference.model)
}

fn snapshot(bag: &Bag) -> Option<RoutePreference> {
    let choice = bag
        .route_choices()
        .iter()
        .find(|choice| choice.selected)?
        .clone();
    Some(RoutePreference {
        v: SCHEMA_V,
        route_id: Some(crate::backplane::RouteId::chat(
            &choice.agent,
            &choice.driver,
            choice.reasoning_effort.as_deref(),
        )),
        agent: choice.agent,
        driver: choice.driver,
        model: choice.model,
        reasoning_effort: choice.reasoning_effort,
    })
}

fn load_from(path: &Path) -> Option<RoutePreference> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() == 0 || meta.len() > MAX_BYTES {
        return None;
    }
    let raw = std::fs::read(path).ok()?;
    let preference = serde_json::from_slice::<RoutePreference>(&raw).ok()?;
    matches!(preference.v, 1 | SCHEMA_V).then_some(preference)
}

fn save_to(path: &Path, preference: &RoutePreference) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(preference).map_err(std::io::Error::other)?;
    crate::workspace_store::write_private_atomic(path, &bytes).map_err(std::io::Error::other)
}

fn apply(
    bag: &mut Bag,
    preference: &RoutePreference,
    route_pinned: bool,
    effort_pinned: bool,
) -> RestoreResult {
    let mut restored = RestoreResult::default();
    if !route_pinned
        && let Some(choice) = bag
            .route_choices()
            .iter()
            .find(|choice| choice.available && matches(choice, preference))
    {
        restored.route = bag.select_route(choice.agent_index, choice.slot_index);
    }

    // Apply effort only when the currently-selected concrete route is the one
    // the preference describes. This prevents a stale model's level from
    // leaking onto a pinned or fallback route that happens to accept the word.
    let current_matches = bag
        .route_choices()
        .iter()
        .find(|choice| choice.selected && choice.available)
        .is_some_and(|choice| matches(choice, preference));
    if current_matches
        && !effort_pinned
        && let Some(effort) = preference.reasoning_effort.as_deref()
    {
        restored.effort = bag.set_reasoning_effort(effort).is_some();
    }
    restored
}

/// Restore the last explicit interactive choice. Test binaries never consult
/// the real home directory; pure file/apply helpers below carry the coverage.
pub(crate) fn restore(bag: &mut Bag) -> RestoreResult {
    if cfg!(test) || !enabled() {
        return RestoreResult::default();
    }
    let Some(preference) = load_from(&preference_path()) else {
        return RestoreResult::default();
    };
    apply(bag, &preference, route_pin_active(), effort_pin_active())
}

/// Best-effort atomic persistence after a successful operator route/effort
/// change. No prompt, response, endpoint, token, or credential is stored.
pub(crate) fn remember(bag: &Bag) {
    if cfg!(test) || !enabled() {
        return;
    }
    let Some(preference) = snapshot(bag) else {
        return;
    };
    let _ = save_to(&preference_path(), &preference);
}

fn clear_path(path: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn clear() -> std::io::Result<bool> {
    if cfg!(test) {
        return Ok(false);
    }
    clear_path(&preference_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "angel-route-memory-{label}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn preference(agent: &str, driver: &str, model: &str, effort: Option<&str>) -> RoutePreference {
        RoutePreference {
            v: SCHEMA_V,
            route_id: None,
            agent: agent.to_string(),
            driver: driver.to_string(),
            model: model.to_string(),
            reasoning_effort: effort.map(str::to_string),
        }
    }

    #[test]
    fn atomic_round_trip_is_versioned_and_bounded() {
        let path = temp_path("roundtrip");
        let expected = preference("openai", "openai", "gpt-5.6-sol", Some("ultra"));
        save_to(&path, &expected).unwrap();
        assert_eq!(load_from(&path), Some(expected));

        let legacy = serde_json::json!({
            "v": 1,
            "agent": "openai",
            "driver": "openai",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "high"
        });
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(
            load_from(&path).is_some(),
            "v1 legacy strings remain readable"
        );

        let stale = preference("openai", "openai", "old", None);
        let mut stale_json = serde_json::to_value(stale).unwrap();
        stale_json["v"] = serde_json::json!(99);
        std::fs::write(&path, serde_json::to_vec(&stale_json).unwrap()).unwrap();
        assert_eq!(load_from(&path), None);
        std::fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        assert_eq!(load_from(&path), None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn clear_is_idempotent() {
        let path = temp_path("clear");
        std::fs::write(&path, b"{}").unwrap();
        assert!(clear_path(&path).unwrap());
        assert!(!clear_path(&path).unwrap());
    }

    #[test]
    fn restore_selects_only_an_available_exact_route() {
        let mut bag = Bag::for_reasoning_render_test();
        let beta = preference("beta", "practice", "model-b", None);
        assert!(apply(&mut bag, &beta, false, false).route);
        assert_eq!(bag.in_hand_label(), "beta");

        let mut bag = Bag::for_render_test(&[
            ("alpha", &[("model-a", true)]),
            ("beta", &[("model-b", false)]),
        ]);
        let unavailable = preference("beta", "practice", "model-b", None);
        assert_eq!(
            apply(&mut bag, &unavailable, false, false),
            RestoreResult::default()
        );
        assert_eq!(bag.in_hand_label(), "alpha");
    }

    #[test]
    fn restore_respects_route_and_effort_pins() {
        let preferred = preference("openai", "gpt-5.6-sol", "gpt-5.6-sol", Some("high"));

        let mut bag = Bag::for_reasoning_render_test();
        let restored = apply(&mut bag, &preferred, false, false);
        assert!(restored.route);
        assert!(restored.effort);
        assert_eq!(bag.reasoning_effort().as_deref(), Some("high"));

        let mut pinned = Bag::for_reasoning_render_test();
        let restored = apply(&mut pinned, &preferred, true, true);
        assert_eq!(restored, RestoreResult::default());
        assert_eq!(pinned.reasoning_effort().as_deref(), Some("medium"));

        let mut differently_pinned = Bag::for_reasoning_render_test();
        let other_route = preference("beta", "practice", "model-b", Some("high"));
        let restored = apply(&mut differently_pinned, &other_route, true, false);
        assert_eq!(restored, RestoreResult::default());
        assert_eq!(
            differently_pinned.reasoning_effort().as_deref(),
            Some("medium"),
            "a stale route's effort must not leak onto the pinned model"
        );
    }

    #[test]
    fn legacy_sota_moa_pin_does_not_override_the_concrete_brain_route() {
        assert!(!route_pin_is_concrete(std::ffi::OsStr::new("sota-moa")));
        assert!(!route_pin_is_concrete(std::ffi::OsStr::new(" SOTA-MOA ")));
        assert!(route_pin_is_concrete(std::ffi::OsStr::new("openai")));
        assert!(route_pin_is_concrete(std::ffi::OsStr::new("longcat")));
    }

    #[test]
    fn snapshot_contains_only_selected_route_metadata() {
        let mut bag = Bag::for_reasoning_render_test();
        assert!(bag.select_route(0, 0));
        assert!(bag.set_reasoning_effort("high").is_some());
        let mut expected = preference("openai", "gpt-5.6-sol", "gpt-5.6-sol", Some("high"));
        expected.route_id = Some(crate::backplane::RouteId::chat(
            "openai",
            "gpt-5.6-sol",
            Some("high"),
        ));
        assert_eq!(snapshot(&bag), Some(expected));
    }

    #[test]
    fn active_restore_uses_stable_route_id_while_legacy_keeps_model_strings() {
        let original = Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
        let preference = snapshot(&original).unwrap();
        let refreshed = Bag::for_render_test(&[("alpha", &[("model-b", true)])]);
        let choice = refreshed.route_choices().iter().next().cloned().unwrap();
        assert!(matches_with_backplane(&choice, &preference, true));
        assert!(!matches_with_backplane(&choice, &preference, false));
    }
}
