//! Telemetry: how long ticks and writers take, what each client is sent,
//! and who was turned away. Counters only grow; a `Totals` is a reading of
//! them, and two readings give the `Stats` of the time between.

use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

/// Exact below this many nanoseconds, then 16 buckets an octave (about 6%).
const LINEAR: u64 = 32;
const PER_OCTAVE: usize = 16;
const BUCKETS: usize = LINEAR as usize + (64 - 5) * PER_OCTAVE;

fn bucket(ns: u64) -> usize {
    if ns < LINEAR {
        return ns as usize;
    }
    let msb = 63 - ns.leading_zeros() as usize;
    let mantissa = ((ns >> (msb - 4)) & 15) as usize;
    LINEAR as usize + (msb - 5) * PER_OCTAVE + mantissa
}

/// The top of a bucket, in nanoseconds: percentiles read high, never low.
fn bucket_top(i: usize) -> u64 {
    if i < LINEAR as usize {
        return i as u64;
    }
    let msb = (i - LINEAR as usize) / PER_OCTAVE + 5;
    let mantissa = ((i - LINEAR as usize) % PER_OCTAVE) as u128;
    (((16 + mantissa + 1) << (msb - 4)) - 1).min(u128::from(u64::MAX)) as u64
}

/// Durations, counted into buckets without a lock.
pub struct Histo {
    counts: Box<[AtomicU64]>,
    sum: AtomicU64,
}

impl Default for Histo {
    fn default() -> Histo {
        Histo { counts: (0..BUCKETS).map(|_| AtomicU64::new(0)).collect(), sum: AtomicU64::new(0) }
    }
}

impl Histo {
    pub fn record(&self, d: Duration) {
        let ns = d.as_nanos().min(u128::from(u64::MAX)) as u64;
        self.counts[bucket(ns)].fetch_add(1, Relaxed);
        self.sum.fetch_add(ns, Relaxed);
    }

    fn read(&self) -> Counts {
        Counts { counts: self.counts.iter().map(|c| c.load(Relaxed)).collect(), sum: self.sum.load(Relaxed) }
    }
}

/// A histogram's reading.
#[derive(Clone, Debug, Default)]
pub struct Counts {
    counts: Vec<u64>,
    sum: u64,
}

impl Counts {
    fn since(&self, earlier: &Counts) -> Spread {
        let counts: Vec<u64> = if earlier.counts.is_empty() {
            self.counts.clone()
        } else {
            self.counts.iter().zip(&earlier.counts).map(|(a, b)| a - b).collect()
        };
        let n: u64 = counts.iter().sum();
        if n == 0 {
            return Spread::default();
        }
        let at = |p: f64| -> f64 {
            let rank = ((p * n as f64).ceil() as u64).max(1);
            let mut seen = 0;
            for (i, c) in counts.iter().enumerate() {
                seen += c;
                if seen >= rank {
                    return bucket_top(i) as f64 / 1e6;
                }
            }
            0.0
        };
        Spread { n, avg: (self.sum - earlier.sum) as f64 / n as f64 / 1e6, p50: at(0.50), p99: at(0.99), max: at(1.0) }
    }
}

/// A distribution: in milliseconds for times, bytes a second for rates.
/// Times are read from buckets about 6% wide, rounded up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spread {
    pub n: u64,
    pub avg: f64,
    pub p50: f64,
    pub p99: f64,
    pub max: f64,
}

impl Spread {
    fn of(mut values: Vec<f64>) -> Spread {
        if values.is_empty() {
            return Spread::default();
        }
        values.sort_by(f64::total_cmp);
        let n = values.len();
        let at = |p: f64| values[(((p * n as f64).ceil() as usize).max(1) - 1).min(n - 1)];
        Spread { n: n as u64, avg: values.iter().sum::<f64>() / n as f64, p50: at(0.5), p99: at(0.99), max: values[n - 1] }
    }
}

/// One connection's counters, kept by its writer.
#[derive(Default)]
pub struct ConnCounters {
    pub bytes: AtomicU64,
    pub snapshots: AtomicU64,
    /// Snapshot ticks it never got because its writer was busy writing.
    pub skipped: AtomicU64,
    /// Snapshot ticks held back because its client had not applied enough.
    pub held: AtomicU64,
}

/// Why connections ended or were turned away, as counted.
#[derive(Default)]
pub struct Counters {
    pub http: AtomicU64,
    pub upgrades: AtomicU64,
    pub joined: AtomicU64,
    pub resumed: AtomicU64,
    pub rejected: AtomicU64,
    /// Frames that broke the protocol or a limit.
    pub violations: AtomicU64,
    /// Closed after `SILENT_CLOSE` without a frame.
    pub silent: AtomicU64,
    /// Turned away at the door: too many connections not yet in.
    pub busy: AtomicU64,
    /// Ticks whose events aged out before a held-back writer sent them.
    pub events_lost: AtomicU64,
    pub write_errors: AtomicU64,
}

/// Everything the server counts.
#[derive(Default)]
pub struct Telemetry {
    pub tick: Histo,
    pub step: Histo,
    pub publish: Histo,
    pub encode: Histo,
    pub ticks: AtomicU64,
    pub counters: Counters,
    /// What the seats' command queues did (`angel_net::cmd`).
    pub inputs: Mutex<angel_net::cmd::Counts>,
    /// (id, counters, connected at) of every live connection.
    pub conns: Mutex<Vec<(u64, std::sync::Arc<ConnCounters>, Instant)>>,
}

/// A reading of every counter, at one moment.
#[derive(Clone, Debug)]
pub struct Totals {
    at: Instant,
    tick: Counts,
    step: Counts,
    publish: Counts,
    encode: Counts,
    ticks: u64,
    counters: [u64; 10],
    inputs: angel_net::cmd::Counts,
    /// (id, bytes, snapshots, skipped, held, connected at)
    conns: Vec<(u64, u64, u64, u64, u64, Instant)>,
    pub players: u32,
    pub spectators: u32,
}

impl Telemetry {
    pub fn read(&self, players: u32, spectators: u32) -> Totals {
        let c = &self.counters;
        let conns = self
            .conns
            .lock()
            .expect("telemetry lock")
            .iter()
            .map(|(id, k, since)| {
                (*id, k.bytes.load(Relaxed), k.snapshots.load(Relaxed), k.skipped.load(Relaxed), k.held.load(Relaxed), *since)
            })
            .collect();
        Totals {
            at: Instant::now(),
            tick: self.tick.read(),
            step: self.step.read(),
            publish: self.publish.read(),
            encode: self.encode.read(),
            ticks: self.ticks.load(Relaxed),
            counters: [
                &c.http,
                &c.upgrades,
                &c.joined,
                &c.resumed,
                &c.rejected,
                &c.violations,
                &c.silent,
                &c.busy,
                &c.events_lost,
                &c.write_errors,
            ]
            .map(|a| a.load(Relaxed)),
            inputs: *self.inputs.lock().expect("telemetry lock"),
            conns,
            players,
            spectators,
        }
    }
}

impl Totals {
    /// What happened between `earlier` and this reading. Per-client rates
    /// count the connections alive at this reading, over the part of the
    /// span each was connected.
    pub fn since(&self, earlier: &Totals) -> Stats {
        let secs = self.at.duration_since(earlier.at).as_secs_f64();
        let mut bytes = Vec::new();
        let mut snapshots = Vec::new();
        let (mut skipped, mut held) = (0, 0);
        for &(id, b, s, sk, h, since) in &self.conns {
            let (b0, s0, sk0, h0, from) = match earlier.conns.iter().find(|c| c.0 == id) {
                Some(&(_, b0, s0, sk0, h0, _)) => (b0, s0, sk0, h0, earlier.at),
                None => (0, 0, 0, 0, since.max(earlier.at)),
            };
            let span = self.at.duration_since(from).as_secs_f64();
            if span > 0.0 {
                bytes.push((b - b0) as f64 / span);
                snapshots.push((s - s0) as f64 / span);
            }
            skipped += sk - sk0;
            held += h - h0;
        }
        let d = |i: usize| self.counters[i] - earlier.counters[i];
        Stats {
            secs,
            ticks: self.ticks - earlier.ticks,
            tick: self.tick.since(&earlier.tick),
            step: self.step.since(&earlier.step),
            publish: self.publish.since(&earlier.publish),
            encode: self.encode.since(&earlier.encode),
            players: self.players,
            spectators: self.spectators,
            client_bytes: Spread::of(bytes),
            client_snapshots: Spread::of(snapshots),
            skipped,
            held,
            http: d(0),
            upgrades: d(1),
            joined: d(2),
            resumed: d(3),
            rejected: d(4),
            violations: d(5),
            silent: d(6),
            busy: d(7),
            events_lost: d(8),
            write_errors: d(9),
            inputs: {
                let (now, then) = (&self.inputs, &earlier.inputs);
                angel_net::cmd::Counts {
                    taken: now.taken - then.taken,
                    guessed: now.guessed - then.guessed,
                    wrong: now.wrong - then.wrong,
                    late: now.late - then.late,
                    waited: now.waited - then.waited,
                    drained: now.drained - then.drained,
                    folded: now.folded - then.folded,
                }
            },
        }
    }
}

/// What the server did over a span, for the cockpit and the log.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub secs: f64,
    pub ticks: u64,
    /// The whole tick: commands taken, the step, publishing, waking the
    /// writers. Milliseconds.
    pub tick: Spread,
    pub step: Spread,
    pub publish: Spread,
    /// One client's snapshot, built and framed by its writer. Milliseconds.
    pub encode: Spread,
    pub players: u32,
    pub spectators: u32,
    /// Bytes a second each connected client was sent.
    pub client_bytes: Spread,
    pub client_snapshots: Spread,
    pub skipped: u64,
    pub held: u64,
    pub http: u64,
    pub upgrades: u64,
    pub joined: u64,
    pub resumed: u64,
    pub rejected: u64,
    pub violations: u64,
    pub silent: u64,
    pub busy: u64,
    pub events_lost: u64,
    pub write_errors: u64,
    /// The seats' command queues: ticks taken, guessed (and of those wrong),
    /// commands late, ticks waited, commands drained and folded.
    pub inputs: angel_net::cmd::Counts,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.0}s {} ticks | tick p50 {:.2} p99 {:.2} max {:.2} ms (step p99 {:.2}, publish p99 {:.2}) | \
             encode p50 {:.3} p99 {:.3} ms | {} players {} watching | per client {:.0} B/s avg, p99 {:.0}, \
             {:.1} snapshots/s | skipped {} held {} | joined {} resumed {} rejected {} violations {} silent {} busy {} | \
             commands taken {} guessed {} wrong {} late {} waited {} drained {} folded {}",
            self.secs,
            self.ticks,
            self.tick.p50,
            self.tick.p99,
            self.tick.max,
            self.step.p99,
            self.publish.p99,
            self.encode.p50,
            self.encode.p99,
            self.players,
            self.spectators,
            self.client_bytes.avg,
            self.client_bytes.p99,
            self.client_snapshots.avg,
            self.skipped,
            self.held,
            self.joined,
            self.resumed,
            self.rejected,
            self.violations,
            self.silent,
            self.busy,
            self.inputs.taken,
            self.inputs.guessed,
            self.inputs.wrong,
            self.inputs.late,
            self.inputs.waited,
            self.inputs.drained,
            self.inputs.folded,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_cover_every_value_and_read_high() {
        let mut last = 0;
        for ns in [0u64, 1, 31, 32, 33, 47, 48, 100, 1_000, 999_999, 10_000_000, u64::MAX / 2, u64::MAX] {
            let b = bucket(ns);
            assert!(b < BUCKETS);
            assert!(b >= last);
            last = b;
            assert!(bucket_top(b) >= ns, "{ns}");
            assert!(bucket_top(b) as f64 <= ns as f64 * 1.07 + 1.0, "{ns}");
        }
    }

    #[test]
    fn percentiles_come_from_the_window() {
        let h = Histo::default();
        let before = h.read();
        for ms in 1..=100 {
            h.record(Duration::from_millis(ms));
        }
        let s = h.read().since(&before);
        assert_eq!(s.n, 100);
        assert!((s.p50 - 50.0).abs() < 4.0, "{s:?}");
        assert!((s.p99 - 99.0).abs() < 7.0, "{s:?}");
        assert!((s.avg - 50.5).abs() < 0.01);
        let after = h.read();
        h.record(Duration::from_millis(500));
        let s = h.read().since(&after);
        assert_eq!(s.n, 1);
        assert!(s.max >= 500.0);
    }
}
