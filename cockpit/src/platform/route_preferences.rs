//! Durable operator choice for the interactive Brain Route controls.
//!
//! This is deliberately a preference, not a routing mandate: explicit
//! environment pins win, unavailable routes are never selected, malformed or
//! stale files are ignored, and the Bag's normal election remains the fallback.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::agent::club::{Bag, RouteChoice};

const SCHEMA_V: u8 = 2;
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RoutePreference {
    v: u8,
    #[serde(default)]
    route_id: Option<crate::agent::backplane::RouteId>,
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
        .unwrap_or_else(|| crate::platform::workspace_store::angel_dir().join("brain-route.json"))
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
    matches_with_backplane(choice, preference, crate::agent::backplane::active())
}

fn matches_with_backplane(
    choice: &RouteChoice,
    preference: &RoutePreference,
    use_stable_id: bool,
) -> bool {
    if use_stable_id && let Some(expected) = preference.route_id.as_ref() {
        return crate::agent::backplane::RouteId::chat(
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
        route_id: Some(crate::agent::backplane::RouteId::chat(
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
    crate::platform::workspace_store::write_private_atomic(path, &bytes)
        .map_err(std::io::Error::other)
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
#[path = "../../../tests/cockpit/app/route_preferences__tests.rs"]
mod tests;
