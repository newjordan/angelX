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

use std::sync::atomic::{AtomicU8, Ordering};

const COMP_UNCACHED: u8 = 0;
const COMP_OFF: u8 = 1;
const COMP_ON: u8 = 2;

static CACHED_COMP_MODE: AtomicU8 = AtomicU8::new(COMP_UNCACHED);

/// Parse boolean string representations for comp/lean mode flags.
pub(crate) fn parse(raw: Option<&str>) -> bool {
    match raw {
        Some(value) => {
            let value = value.trim().to_ascii_lowercase();
            !matches!(
                value.as_str(),
                "" | "0" | "false" | "no" | "off" | "disable" | "disabled"
            )
        }
        None => false,
    }
}

fn from_env_uncached() -> bool {
    parse(std::env::var("ANGEL_COMP_MODE").ok().as_deref())
        || parse(std::env::var("ANGEL_TURBO").ok().as_deref())
        || parse(std::env::var("ANGEL_TURBO_MODE").ok().as_deref())
        || parse(std::env::var("ANGEL_LEAN_MODE").ok().as_deref())
        || parse(std::env::var("ANGEL_LEAN").ok().as_deref())
}

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

/// Set the live Comp / Lean / Turbo mode posture.
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
    }
    crate::ui::surfaces::invalidate_backdrop_cache();
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
