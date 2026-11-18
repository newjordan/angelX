//! Background availability prober for clubs in the bag.

use super::*;
use std::sync::atomic::AtomicU64;

pub(crate) const PROBE_FAIL_THRESHOLD: u8 = 2;
pub(crate) const PROBE_INTERVAL_SECS: u64 = 5;

pub(crate) fn prober_enabled() -> bool {
    match std::env::var("ANGEL_BAG_PROBE") {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no"
        ),
        Err(_) => true,
    }
}

pub(crate) fn probe_interval() -> Duration {
    let secs = std::env::var("ANGEL_BAG_PROBE_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(PROBE_INTERVAL_SECS)
        .max(1);
    Duration::from_secs(secs)
}

pub(crate) struct ProbeVerdict {
    pub(crate) available: bool,
    pub(crate) confirmed: bool,
    pub(crate) misses: u8,
}

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

struct ProbeTarget {
    club: Arc<dyn Club>,
    slot: Arc<AtomicBool>,
    confirmed: AtomicBool,
    misses: AtomicU8,
}

static PROBE_TARGETS: std::sync::OnceLock<Mutex<Vec<Arc<ProbeTarget>>>> =
    std::sync::OnceLock::new();
static PROBE_INTERVAL_MS: AtomicU64 = AtomicU64::new(u64::MAX);
static PROBE_SWEEPER_THREAD: std::sync::OnceLock<std::thread::Thread> = std::sync::OnceLock::new();

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
                    let sweep: Vec<Arc<ProbeTarget>> = PROBE_TARGETS
                        .get()
                        .map(|l| {
                            let mut list = l.lock().unwrap_or_else(|e| e.into_inner());
                            list.retain(|t| Arc::strong_count(&t.slot) > 1);
                            list.clone()
                        })
                        .unwrap_or_default();
                    std::thread::scope(|s| {
                        for t in &sweep {
                            s.spawn(move || {
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
                    let delay = Duration::from_millis(PROBE_INTERVAL_MS.load(Ordering::Relaxed));
                    std::thread::park_timeout(delay);
                }
            });
    });
    if let Some(thread) = PROBE_SWEEPER_THREAD.get() {
        thread.unpark();
    }
}
