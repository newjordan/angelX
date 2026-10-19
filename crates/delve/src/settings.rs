//! What the host sets once, before play: the cockpit from its environment
//! (`ANGEL_ART`, `ANGEL_DELVE_FEEL`, `ANGEL_DELVE_OPEN`), a browser from its welcome. Unset,
//! everything is off.
//!
//! Tests (and the cockpit's tests, through the `testing` feature) read the
//! same environment themselves when no host has set anything, so
//! `ANGEL_ART=v2 cargo test` draws with the second kit as it always has.

use std::sync::atomic::{AtomicBool, Ordering};

/// The host's choices for the whole process.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// Draw with the second art kit (`ANGEL_ART=v2`).
    pub art_v2: bool,
    /// Keep the feel ledger (`ANGEL_DELVE_FEEL=1`).
    pub feel_ledger: bool,
    /// Lay every delve floor open (`ANGEL_DELVE_OPEN=1`): one map to
    /// explore, no doors between fights (`rules::depths`). A run takes it
    /// when it starts (`Run::open_floors`).
    pub open_floors: bool,
}

static ART_V2: AtomicBool = AtomicBool::new(false);
static FEEL_LEDGER: AtomicBool = AtomicBool::new(false);
static OPEN_FLOORS: AtomicBool = AtomicBool::new(false);

/// Take the host's settings.
pub fn set(settings: Settings) {
    #[cfg(any(test, feature = "testing"))]
    from_env();
    ART_V2.store(settings.art_v2, Ordering::Relaxed);
    FEEL_LEDGER.store(settings.feel_ledger, Ordering::Relaxed);
    OPEN_FLOORS.store(settings.open_floors, Ordering::Relaxed);
}

/// The settings in force.
pub fn get() -> Settings {
    #[cfg(any(test, feature = "testing"))]
    from_env();
    Settings {
        art_v2: ART_V2.load(Ordering::Relaxed),
        feel_ledger: FEEL_LEDGER.load(Ordering::Relaxed),
        open_floors: OPEN_FLOORS.load(Ordering::Relaxed),
    }
}

/// Under test, the environment stands in for a host, once, before anything
/// a test sets.
#[cfg(any(test, feature = "testing"))]
fn from_env() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        ART_V2.store(
            std::env::var_os("ANGEL_ART").is_some_and(|v| v == "v2"),
            Ordering::Relaxed,
        );
        FEEL_LEDGER.store(
            std::env::var("ANGEL_DELVE_FEEL").is_ok_and(|v| v == "1"),
            Ordering::Relaxed,
        );
        OPEN_FLOORS.store(
            std::env::var("ANGEL_DELVE_OPEN").is_ok_and(|v| v == "1"),
            Ordering::Relaxed,
        );
    });
}
