//! Background availability prober for clubs in the bag.

use super::*;
use std::sync::atomic::AtomicU64;

/// Consecutive failed probes before a club is dropped from the bag. One dropped
/// packet shouldn't hide a working model, so a single miss is forgiven; a success
/// clears the count immediately (recovery is instant).
pub(crate) const PROBE_FAIL_THRESHOLD: u8 = 2;
/// Default seconds between full availability sweeps. Each sweep probes every club
/// concurrently, so wall-time per sweep is ~one probe timeout regardless of fleet
/// size. Override with `ANGEL_BAG_PROBE_SECS`; disable the prober entirely with
/// `ANGEL_BAG_PROBE=0` (availability then stays at the startup seed).
pub(crate) const PROBE_INTERVAL_SECS: u64 = 5;

/// Whether the background availability prober should run. On unless
/// `ANGEL_BAG_PROBE` is set to a falsy value (`0`/`off`/`false`/`no`).
pub(crate) fn prober_enabled() -> bool {
    match std::env::var("ANGEL_BAG_PROBE") {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no"
        ),
        Err(_) => true,
    }
}

/// Seconds between availability sweeps (`ANGEL_BAG_PROBE_SECS`, min 1s).
pub(crate) fn probe_interval() -> Duration {
    let secs = std::env::var("ANGEL_BAG_PROBE_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(PROBE_INTERVAL_SECS)
        .max(1);
    Duration::from_secs(secs)
}

/// One target's verdict for a prober sweep, returned by [`prober_step`].
pub(crate) struct ProbeVerdict {
    /// New availability bit to publish.
    pub(crate) available: bool,
    /// Has this target ever probed up (sticky once true).
    pub(crate) confirmed: bool,
    /// New consecutive-miss count (only meaningful once confirmed).
    pub(crate) misses: u8,
}

/// Pure prober decision for one target, so the hysteresis is testable without
/// threads. `available` is this probe's result; `confirmed` whether the target
/// has ever probed up; `misses` the consecutive misses so far. A success
/// publishes up and confirms. A failure on a target NEVER confirmed drops it at
/// once — its prior "up" was only the optimistic startup seed, never verified, so
/// a dead box can't linger in hand. A failure on a confirmed target holds it up
/// until [`PROBE_FAIL_THRESHOLD`] consecutive misses, so one dropped packet never
/// hides a working model (`saturating_add` keeps the u8 from overflowing).
pub(crate) fn prober_step(available: bool, confirmed: bool, misses: u8) -> ProbeVerdict {
    if available {
        ProbeVerdict {
            available: true,
            confirmed: true,
            misses: 0,
        }
    } else if !confirmed {
        ProbeVerdict {
            available: false,
            confirmed: false,
            misses: 0,
        }
    } else {
        let m = misses.saturating_add(1);
        ProbeVerdict {
            available: m < PROBE_FAIL_THRESHOLD,
            confirmed: true,
            misses: m,
        }
    }
}

/// One prober target with its hysteresis state.
///
/// `confirmed` = has this target EVER probed up. Until it has, its startup
/// availability is just the optimistic seed — unverified — so the first failed
/// probe corrects it immediately instead of granting the 2-miss grace.
/// Otherwise a dead box stays "in hand" for two whole sweeps (~5s), long
/// enough to send the first message to a corpse (the exact bug where a launch
/// landed on a down Spark and hard-failed). Once a box is confirmed up, the
/// consecutive-miss threshold applies, so one dropped packet never hides a
/// working model.
struct ProbeTarget {
    club: Arc<dyn Club>,
    slot: Arc<AtomicBool>,
    confirmed: AtomicBool,
    misses: AtomicU8,
}

/// The single sweeper's appendable target list. One thread serves the whole
/// process — startup used to spawn one immortal thread, and every agent the
/// background scanner discovered spawned another; they all fold in here now.
/// Late registrations wake the sweeper immediately, and a tighter requested
/// cadence takes effect without waiting through the original interval.
static PROBE_TARGETS: std::sync::OnceLock<Mutex<Vec<Arc<ProbeTarget>>>> =
    std::sync::OnceLock::new();
/// Fastest requested sweep cadence. Production callers all pass the same
/// configured value, while tests and dynamically attached subsystems may ask
/// for a faster cadence later. `fetch_min` makes that tightening monotonic.
static PROBE_INTERVAL_MS: AtomicU64 = AtomicU64::new(u64::MAX);
static PROBE_SWEEPER_THREAD: std::sync::OnceLock<std::thread::Thread> = std::sync::OnceLock::new();

/// Register clubs with the background prober: every interval the (single,
/// shared) sweeper probes each registered club concurrently and flips its
/// availability bit. Never touches the UI thread, so a dead box can be dropped
/// from the bag without ever blocking a draw. The sweeper starts on the first
/// call — its `interval` is fixed then (every caller passes `probe_interval()`,
/// so this is a distinction without a difference) — is detached, and exits with
/// the process.
pub(crate) fn spawn_prober(targets: Vec<(Arc<dyn Club>, Arc<AtomicBool>)>, interval: Duration) {
    if targets.is_empty() {
        return;
    }
    let list = PROBE_TARGETS.get_or_init(|| Mutex::new(Vec::new()));
    list.lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(targets.into_iter().map(|(club, slot)| {
            Arc::new(ProbeTarget {
                club,
                slot,
                confirmed: AtomicBool::new(false),
                misses: AtomicU8::new(0),
            })
        }));
    let interval_ms = u64::try_from(interval.as_millis())
        .unwrap_or(u64::MAX)
        .max(1);
    PROBE_INTERVAL_MS.fetch_min(interval_ms, Ordering::Relaxed);
    static SWEEPER: std::sync::Once = std::sync::Once::new();
    SWEEPER.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("club-prober".into())
            .spawn(move || {
                loop {
                    let _ = PROBE_SWEEPER_THREAD.set(std::thread::current());
                    // Snapshot, so a mid-sweep discovery never blocks on the lock
                    // longer than a Vec clone; fresh targets join the next sweep.
                    let sweep: Vec<Arc<ProbeTarget>> = PROBE_TARGETS
                        .get()
                        .map(|l| l.lock().unwrap_or_else(|e| e.into_inner()).clone())
                        .unwrap_or_default();
                    // Probe every mode concurrently — a sweep costs ~one probe
                    // timeout regardless of how many are dead.
                    std::thread::scope(|s| {
                        for t in &sweep {
                            s.spawn(move || {
                                // Single writer per target per sweep (sweeps are
                                // sequential), so plain load/store around the pure
                                // decision is race-free.
                                let (available, was_confirmed, prev_miss) = (
                                    t.club.is_available(),
                                    t.confirmed.load(Ordering::Relaxed),
                                    t.misses.load(Ordering::Relaxed),
                                );
                                let step = prober_step(available, was_confirmed, prev_miss);
                                t.slot.store(step.available, Ordering::Relaxed);
                                t.confirmed.store(step.confirmed, Ordering::Relaxed);
                                t.misses.store(step.misses, Ordering::Relaxed);
                            });
                        }
                    });
                    // `park_timeout` lets a newly registered target or a tighter
                    // interval wake the shared thread immediately. Unlike a bare
                    // sleep this cannot strand late-discovered clubs behind the
                    // original caller's whole interval; an early unpark token is
                    // retained until the next park, avoiding a lost-wakeup race.
                    let delay = Duration::from_millis(PROBE_INTERVAL_MS.load(Ordering::Relaxed));
                    std::thread::park_timeout(delay);
                }
            });
    });
    if let Some(thread) = PROBE_SWEEPER_THREAD.get() {
        thread.unpark();
    }
}
