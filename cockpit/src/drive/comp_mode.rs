//! Lean / Comp Mode — ultra-high-speed execution posture for angelX.
//!
//! When Comp Mode is enabled:
//! - Background visual simulation (world_viz miniworld, terrain raycasting / 3D,
//!   hearth disk sync, Kitty graphics compose) is detached/tabled to eliminate UI
//!   latency and per-frame overhead.
//! - The TUI renders in lean full-text mode (~sub-millisecond frame budget).
//! - All tools (including visual inspection, view_file, show, scryglass) remain
//!   fully registered and discoverable by the model on demand.
//! - Turn wall-clock deadlines are unbounded (0 = off) by default.
//! - Toggled via `/comp on|off`, `/lean on|off`, `ANGEL_COMP_MODE=1`, or `angel --comp`.
//!
//! The race cadence is a second, separate switch: `ANGEL_COMPETITION_MODE`, read
//! each turn by the turn classifier, the schema picker and the caddy board.
//! `/turbo podrace` and `/turbo handoff` raise both switches ([`engage_competition`]);
//! `/turbo on|loop|flash` raise only lean mode. [`set`]`(false)` lowers both: lean
//! mode goes off and the race flag goes back to what the process launched with,
//! so a flag the operator exported at launch stays up and one `/turbo` raised
//! does not outlive `/turbo off`.

use std::ffi::OsString;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, PoisonError};

const COMP_UNCACHED: u8 = 0;
const COMP_OFF: u8 = 1;
const COMP_ON: u8 = 2;

static CACHED_COMP_MODE: AtomicU8 = AtomicU8::new(COMP_UNCACHED);

/// The flag the turn classifier, schema picker and caddy board read each turn.
const COMPETITION_ENV: &str = "ANGEL_COMPETITION_MODE";

/// `Some(launch)` while `/turbo` holds the race flag up, where `launch` is what
/// the process had in it before the first raise (`None` = unset). Kept from the
/// first raise only, so podrace then handoff still restores the launch value.
static RAISED_FROM: Mutex<Option<Option<OsString>>> = Mutex::new(None);

// The environment reading is shared with the realm host, which falls back on it
// when no live posture was lent (`angel_live::host::posture`).
pub(crate) use angel_live::host::posture::from_env_uncached;
#[cfg(test)]
pub(crate) use angel_live::host::posture::parse;

/// Whether Lean / Comp / Turbo Mode is currently active.
pub(crate) fn enabled() -> bool {
    #[cfg(test)]
    {
        from_env_uncached()
    }
    #[cfg(not(test))]
    {
        match CACHED_COMP_MODE.load(Ordering::Acquire) {
            COMP_ON => return true,
            COMP_OFF => return false,
            _ => {}
        }
        let active = from_env_uncached();
        store_mode(active);
        active
    }
}

/// Ambient Stage sims (loop loom, raytrace, MoA, hammertime cadence).
/// Comp / lean mode is additional: default cockpit still paints and ticks.
pub(crate) fn ambient_stage_sim_allowed() -> bool {
    !enabled()
}

/// Miniworld mirrors, journeys, lesson roll, world clock, and turn-end fireworks.
/// Hidden / backdrop-off skip because the Stage was not laid out last draw.
/// Comp / lean is additional: chrome stays, the sim tax does not.
pub(crate) fn stage_world_mirrors_allowed(world_pane_visible: bool) -> bool {
    world_pane_visible && ambient_stage_sim_allowed()
}

fn store_mode(active: bool) {
    let code = if active { COMP_ON } else { COMP_OFF };
    CACHED_COMP_MODE.store(code, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn invalidate_cache() {
    CACHED_COMP_MODE.store(COMP_UNCACHED, Ordering::Release);
}

/// Set the live Comp / Lean / Turbo mode posture. Off also lowers a race flag
/// that `/turbo` raised.
pub(crate) fn set(active: bool) {
    if active {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_COMP_MODE", "1") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TURBO", "1") };
        store_mode(true);
    } else {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_COMP_MODE") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_TURBO") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_TURBO_MODE") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_LEAN_MODE") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_LEAN") };
        store_mode(false);
        lower_competition();
    }
    crate::ui::surfaces::invalidate_backdrop_cache();
}

/// `/turbo podrace` and `/turbo handoff`: lean mode plus the race cadence.
pub(crate) fn engage_competition() {
    set(true);
    let mut raised_from = RAISED_FROM.lock().unwrap_or_else(PoisonError::into_inner);
    if raised_from.is_none() {
        *raised_from = Some(std::env::var_os(COMPETITION_ENV));
    }
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(COMPETITION_ENV, "1") };
}

/// Put the race flag back to what launch had, if `/turbo` raised it.
fn lower_competition() {
    let Some(launch) = RAISED_FROM
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
    else {
        return;
    };
    match launch {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var(COMPETITION_ENV, value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var(COMPETITION_ENV) },
    }
}

/// Human-facing status text describing the active mode.
pub(crate) fn status_text() -> String {
    if enabled() {
        "⚡ TURBO / COMP MODE ON — ultra-lean high-speed harness · live agent thinking trace & \
         controls visible in right panel · heavy ambient simulations detached for zero-latency execution · \
         all tools remain available on demand · /turbo off (or /comp off) to return to standard mode"
            .to_string()
    } else {
        "TURBO / COMP MODE OFF — visual backdrop and ambient miniworld active · /turbo on (or /comp on) for \
         ultra-lean high-speed execution"
            .to_string()
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/comp_mode__tests.rs"]
mod tests;
