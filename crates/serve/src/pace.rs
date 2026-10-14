//! How a writer paces its client: what it has written that the client has
//! not applied yet, and how much it lets be unapplied.
//!
//! The client says, in each `Cmds`, the newest snapshot tick it applied.
//! The writer times each snapshot from writing it to hearing it applied,
//! and keeps the least such delay of the last `DELAY_WINDOW`. A snapshot
//! applied within `QUEUE` of that least delay went through no queue: the
//! window of bytes the writer lets be unapplied grows by what it carried
//! (doubling each round trip), and after the client's first late snapshot
//! by `STEP` for each window's worth applied on time. One applied later
//! than that waited in a queue: the window halves, to no less than one full
//! snapshot. The writer holds back while more than the window is
//! unapplied, and then skips to the newest tick.
//!
//! So a fast client, near or far, grows its window to its round trip's
//! worth and is not held again; a link slower than the game's stream is
//! held to about one snapshot in flight, not seconds of stale state. The
//! window grows on evidence that nothing queued, never on a measured rate,
//! so holding back cannot talk a fast client's window down.

use crate::limits::{DELAY_WINDOW, MAX_IN_FLIGHT, MIN_IN_FLIGHT, QUEUE};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// After a late snapshot, the window grows by this many bytes for each
/// window's worth applied on time: slowly enough that a link slower than
/// the stream is held near the floor rather than overshot by the time the
/// next late snapshot is heard.
const STEP: usize = 150;

pub(crate) struct Pace {
    /// (tick, written at, bytes), oldest first.
    flight: VecDeque<(u32, Instant, usize)>,
    unapplied: usize,
    /// (applied at, delay), recent.
    delays: VecDeque<(Instant, Duration)>,
    window: usize,
    /// Where doubling stops and stepping starts: half the window at the
    /// last late snapshot.
    threshold: usize,
}

impl Default for Pace {
    fn default() -> Pace {
        Pace {
            flight: VecDeque::new(),
            unapplied: 0,
            delays: VecDeque::new(),
            window: MIN_IN_FLIGHT,
            threshold: MAX_IN_FLIGHT,
        }
    }
}

impl Pace {
    pub fn written(&mut self, tick: u32, at: Instant, bytes: usize) {
        self.flight.push_back((tick, at, bytes));
        self.unapplied += bytes;
    }

    /// The client applied everything through `tick`, as heard at `at`. A
    /// tick past anything written is not believed.
    pub fn applied(&mut self, tick: u32, at: Instant) {
        if self.flight.back().is_some_and(|&(newest, _, _)| tick > newest) {
            return;
        }
        let mut bytes = 0;
        let mut delay = None;
        while let Some(&(t, written, b)) = self.flight.front() {
            if t > tick {
                break;
            }
            self.flight.pop_front();
            self.unapplied -= b;
            bytes += b;
            if t == tick {
                delay = Some(at.saturating_duration_since(written));
            }
        }
        let Some(delay) = delay else { return };
        while self.delays.front().is_some_and(|(when, _)| at.saturating_duration_since(*when) > DELAY_WINDOW) {
            self.delays.pop_front();
        }
        self.delays.push_back((at, delay));
        let least = self.delays.iter().map(|(_, d)| *d).min().unwrap_or(delay);
        self.window = if delay > least + QUEUE {
            self.threshold = (self.window / 2).max(MIN_IN_FLIGHT);
            self.threshold
        } else if self.window < self.threshold {
            (self.window + bytes).min(self.threshold)
        } else {
            (self.window + (bytes * STEP).div_ceil(self.window)).min(MAX_IN_FLIGHT)
        };
    }

    /// Whether to hold back what would be written now. The first snapshot
    /// is never held, however big (a whole document).
    pub fn behind(&self) -> bool {
        !self.flight.is_empty() && self.unapplied > self.window
    }

    #[cfg(test)]
    fn unapplied(&self) -> usize {
        self.unapplied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: Duration = Duration::from_millis(50);

    /// What happened on a simulated link.
    struct Run {
        most_unapplied: usize,
        held: u32,
        held_after_two_seconds: u32,
    }

    /// A client behind a link of `rate` bytes a second and `latency` each
    /// way, sent a `size`-byte snapshot every 50 ms for `secs`.
    fn link(rate: f64, latency: Duration, size: usize, secs: u32) -> Run {
        let mut pace = Pace::default();
        let start = Instant::now();
        // When the link finishes carrying each written snapshot.
        let mut carried: VecDeque<(u32, Instant)> = VecDeque::new();
        let mut link_free = start;
        let mut run = Run { most_unapplied: 0, held: 0, held_after_two_seconds: 0 };
        for tick in 1..=(secs * 20) {
            let now = start + SNAPSHOT * tick;
            // Applied as it arrives, heard back a latency later.
            while carried.front().is_some_and(|(_, at)| *at + latency * 2 <= now) {
                let (t, at) = carried.pop_front().unwrap();
                pace.applied(t, at + latency * 2);
            }
            if pace.behind() {
                run.held += 1;
                if tick > 40 {
                    run.held_after_two_seconds += 1;
                }
                continue;
            }
            pace.written(tick, now, size);
            link_free = link_free.max(now) + Duration::from_secs_f64(size as f64 / rate);
            carried.push_back((tick, link_free));
            run.most_unapplied = run.most_unapplied.max(pace.unapplied());
        }
        run
    }

    #[test]
    fn a_fast_client_near_is_never_held() {
        assert_eq!(link(1_000_000.0, Duration::from_millis(1), 600, 20).held, 0);
    }

    #[test]
    fn a_fast_client_far_away_waits_only_while_its_window_opens() {
        // A phone 300 ms away, a packed crowd's 600-byte snapshots.
        let run = link(100_000.0, Duration::from_millis(150), 600, 30);
        assert_eq!(run.held_after_two_seconds, 0);
        assert!(run.held < 20, "held {}", run.held);
    }

    #[test]
    fn a_slow_link_holds_about_one_snapshot() {
        // 1 KB/s against a 12 KB/s stream.
        let run = link(1_000.0, Duration::from_millis(20), 600, 30);
        assert!(run.held > 400, "held {}", run.held);
        assert!(run.most_unapplied <= MIN_IN_FLIGHT + 2 * 600, "{} bytes unapplied", run.most_unapplied);
    }

    #[test]
    fn a_link_that_slows_down_is_followed() {
        let mut pace = Pace::default();
        let t0 = Instant::now();
        // Fast for a while: the window opens.
        for tick in 1..=100u32 {
            let at = t0 + SNAPSHOT * tick;
            pace.written(tick, at, 1_000);
            pace.applied(tick, at + Duration::from_millis(20));
        }
        assert!(pace.window > 50_000);
        // Then everything is late: a few halvings bring it to the floor.
        for tick in 101..=120u32 {
            let at = t0 + SNAPSHOT * tick;
            pace.written(tick, at, 1_000);
            pace.applied(tick, at + Duration::from_millis(800));
        }
        assert_eq!(pace.window, MIN_IN_FLIGHT);
    }

    #[test]
    fn the_first_snapshot_is_never_held_and_one_full_one_is_the_start() {
        let mut pace = Pace::default();
        let t0 = Instant::now();
        pace.written(1, t0, 200_000);
        assert!(pace.behind());
        pace.applied(1, t0 + Duration::from_millis(300));
        assert!(!pace.behind());
        let mut fresh = Pace::default();
        fresh.written(1, t0, MIN_IN_FLIGHT);
        assert!(!fresh.behind());
        fresh.written(2, t0, 1);
        assert!(fresh.behind());
    }
}
