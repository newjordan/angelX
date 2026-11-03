//! The bullet storm ramp: a public session's five knights at a full room
//! while the air is held at 100, 500, 1 000, 2 000, 5 000, 10 000, 20 000 and
//! 50 000 shots, past every cap (`Run::storm_for_test`), first a bit of
//! everything (mix 0), then the knights' homing arrows (mix 3). Each tick the
//! host steps, takes what the tick said, clones, publishes and sends each of
//! the five seats its tree; seat 0 reads it back and draws its stage, as a
//! page does.
//!
//! Each step prints, as it ends: the host's tick (p50, p95, the worst), the
//! step alone, seat 0's frame, the shots in the air, the step's peak memory
//! (`VmHWM`, started over each step), and a seat's wire (bytes a tick, the
//! first whole tree, the tree's values). Then a table of every step.
//!
//! It holds itself to a budget: before each step it reckons the step's time
//! from the last (scaled by the shots), holds fewer ticks when the budget
//! runs short, and stops, saying where it reached, when even the fewest
//! would pass it; mid-step it stops when the clock or the memory passes.
//!
//! `cargo test --release -p angel-delve bullet_storm_ramp -- --ignored
//! --nocapture --test-threads=1` (`STORM_RAMP_MIXES` "0,3",
//! `STORM_RAMP_STEPS` the shot counts, `STORM_RAMP_TICKS` a step (90),
//! `STORM_RAMP_SECS` a mix's budget (600), `STORM_RAMP_RSS_GB` (20),
//! `STORM_RAMP_PARTY` (5), `STORM_RAMP_DRAW=0` for the host alone,
//! `STORM_RAMP_3D=1` for rooms in 3D, `STORM_RAMP_PROPS=1` for shapes,
//! `STORM_RAMP_ALL_SEATS=1` for every seat drawing, `STORM_RAMP_HOST_DRAW=1`
//! for a watching host, `STORM_FOES` the monsters (12)). Each step also
//! prints the clone, publish, encode, watch, decode, draw and stage costs.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// A public session's knights.
const PARTY: u32 = 5;
/// The shots held in the air, step by step.
const STEPS: [usize; 8] = [100, 500, 1_000, 2_000, 5_000, 10_000, 20_000, 50_000];
/// Ticks a step is held and counted: three seconds at 30 a second.
const TICKS: u32 = 90;
/// Ticks first played and not counted: the first whole tree, the art warming.
const WARM: u32 = 3;
/// The fewest ticks a step is held when the budget runs short.
const FEWEST: u32 = 15;

/// What a step reached.
#[derive(Default)]
struct Held {
    mix: u8,
    target: usize,
    /// The seats sent to each tick.
    seats: u32,
    /// Ticks counted (warm-up aside).
    ticks: u32,
    /// The host's tick (step, what it said, clone, publish, five seats'
    /// patches), the step alone and seat 0's frame (read, draw, stage), ms.
    host: Vec<f64>,
    step: Vec<f64>,
    seat: Vec<f64>,
    /// Shots in the air after each counted step.
    shots: Vec<usize>,
    /// Bytes a seat was sent a tick, summed over seats and counted ticks,
    /// and the most one seat was sent in one.
    wire_sum: usize,
    wire_peak: usize,
    /// Seat 0's first message, the whole tree, bytes.
    whole: usize,
    /// The tree's values at the step's end.
    nodes: usize,
    /// Resident memory at most as sampled each tick, and the step's peak
    /// (`VmHWM`), KB.
    rss_kb: u64,
    hwm_kb: u64,
    /// Whether `VmHWM` was started over for this step.
    own_peak: bool,
    /// The step's wall time, warm-up and all, s.
    wall: f64,
    /// Why the step stopped short, if it did.
    cut: Option<String>,
    /// Per-layer costs and allocation counts from the original storm bench.
    layers: Tally,
}

#[derive(Clone, Copy)]
struct Drawing {
    seats: bool,
    all_seats: bool,
    host: bool,
    world3d: bool,
    props: bool,
}

/// The run's limits: when it must be done by, and the memory it may hold.
struct Guard {
    start: Instant,
    budget: Duration,
    rss_kb: u64,
}

impl Guard {
    fn left(&self) -> f64 {
        self.budget.as_secs_f64() - self.start.elapsed().as_secs_f64()
    }
}

/// (resident now, its peak) in KB, from `/proc/self/status`.
fn rss_kb() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.split_whitespace().next()?.parse().ok())
            .unwrap_or(0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

/// Start the peak (`VmHWM`) over, so a step's is its own: Linux's
/// `clear_refs` 5. False where it cannot.
fn restart_peak() -> bool {
    std::fs::write("/proc/self/clear_refs", "5").is_ok()
}

fn at(sorted: &[f64], q: f64) -> f64 {
    sorted.get(((sorted.len().max(1) - 1) as f64 * q).round() as usize).copied().unwrap_or(0.0)
}

/// (p50, p95, worst) of `samples`.
fn spread(samples: &[f64]) -> (f64, f64, f64) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    (at(&sorted, 0.5), at(&sorted, 0.95), at(&sorted, 1.0))
}

/// Hold the air at `target` shots of `mix` for `ticks` (after `WARM`), the
/// party `party` strong, seat 0 drawing when `draws`.
fn hold(mix: u8, target: usize, ticks: u32, party: u32, drawing: Drawing, guard: &Guard) -> Held {
    let walled = Instant::now();
    let own_peak = restart_peak();
    crate::world3d::set_enabled(drawing.world3d);
    // The party's cover rule would thin the storm to a few dozen.
    crate::rules::cover::OFF.with(|off| off.set(true));
    let mut run = stacked_party(Weapon::Bow, BUILTIN, party, false);
    let mut publisher = wire::Publisher::default();
    let mut sent: Vec<Sent> = (0..party).map(|_| Sent { names: Writer::default(), last: None }).collect();
    let params = Params { world3d: drawing.world3d, ..Params::default() }.encode();
    let drawn_seats = if drawing.seats { if drawing.all_seats { party } else { 1 } } else { 0 };
    let mut clients: Vec<_> = (0..drawn_seats)
        .map(|seat| {
            let mut client = DelveClient::boot(&welcome(Some(seat), &params)).unwrap();
            angel_net::client::GameClient::stage_props(&mut client, drawing.props);
            client
        })
        .collect();
    let mut frame = Frame::default();
    let mut spawned = foes();
    let mut rng = 0x9E37_79B9_7F4A_7C15u64;
    let mut h = Held { mix, target, seats: party, own_peak, ..Held::default() };
    for tick in 1..=WARM + ticks {
        let counted = tick > WARM;
        let input = inputs(&run);
        run.storm_for_test(target, mix, &mut rng);
        let (_, a0, c0) = counting::now();
        let start = Instant::now();
        run.step(&input);
        let stepped = start.elapsed();
        keep(&mut run, &mut spawned);
        let at = Instant::now();
        let snapshot = run.clone();
        let cloned = at.elapsed();
        let at = Instant::now();
        let tree = publisher.publish(&snapshot).unwrap();
        let published = at.elapsed();
        drop(snapshot);
        let at = Instant::now();
        let mut bodies = Vec::with_capacity(sent.len());
        let mut sizes = Vec::with_capacity(sent.len());
        for (seat, s) in sent.iter_mut().enumerate() {
            let view = wire::seat_tree(&tree, Some(seat as u32)).into_owned();
            let mut out = Vec::new();
            let whole = match &s.last {
                None => {
                    s.names.value(&mut out, &view);
                    true
                }
                Some(old) => {
                    if let Some(patch) = diff(old, &view) {
                        if counted && seat == 0
                            && let angel_net::doc::delta::Patch::Struct { set, .. } = &patch
                            && let Some((_, list)) = set.iter().find(|(name, _)| name == "projectiles")
                        {
                            use angel_net::doc::delta::Patch;
                            match list {
                                Patch::KeyedEdit { gone, items } => {
                                    h.layers.list_edits += 1;
                                    h.layers.list_named += items.len();
                                    h.layers.list_gone += gone.len();
                                }
                                Patch::Keyed { order, items } => {
                                    h.layers.list_orders += 1;
                                    h.layers.list_named += items.len();
                                    h.layers.list_gone += order.len();
                                }
                                _ => {}
                            }
                        }
                        s.names.patch(&mut out, &patch);
                    }
                    false
                }
            };
            s.last = Some(view);
            sizes.push(out.len());
            bodies.push((whole, out));
        }
        let encoded = at.elapsed();
        let host = start.elapsed();
        if tick == 1 {
            h.whole = bodies[0].1.len();
        }
        let at = Instant::now();
        if drawing.host {
            let _ = arena::frame_for_live(&run, 384, 224, None);
        }
        let watched = at.elapsed();
        let (_, a1, c1) = counting::now();
        let mut seat_took = Duration::ZERO;
        for (seat, (client, (whole, body))) in clients.iter_mut().zip(&bodies).enumerate() {
            let began = Instant::now();
            client.snapshot(tick, *whole, body).expect("the snapshot reads");
            let decoded = began.elapsed();
            let now = f64::from(tick) * 1000.0 / f64::from(HZ);
            let at = Instant::now();
            client.draw(&view(client, now), &mut frame);
            let drawn = at.elapsed();
            let at = Instant::now();
            let mut out = Vec::new();
            client.stage(now, &mut out);
            let staged = at.elapsed();
            if seat == 0 {
                seat_took = began.elapsed();
            }
            if counted {
                h.layers.decode += decoded;
                h.layers.draw += drawn;
                h.layers.stage += staged;
                h.layers.stage_bytes += out.len();
            }
        }
        if tick == WARM + ticks {
            h.nodes = nodes(&tree);
        }
        drop(tree);
        let (rss, _) = rss_kb();
        h.rss_kb = h.rss_kb.max(rss);
        if counted {
            let (_, a2, c2) = counting::now();
            h.layers.clone += cloned;
            h.layers.publish += published;
            h.layers.encode += encoded;
            h.layers.watch += watched;
            h.layers.host_asked += a1 - a0;
            h.layers.host_allocs += c1 - c0;
            h.layers.seat_asked += a2 - a1;
            h.layers.seat_allocs += c2 - c1;
            h.ticks += 1;
            h.host.push(host.as_secs_f64() * 1000.0);
            h.step.push(stepped.as_secs_f64() * 1000.0);
            h.seat.push(seat_took.as_secs_f64() * 1000.0);
            h.shots.push(run.projectiles.len());
            h.wire_sum += sizes.iter().sum::<usize>();
            h.wire_peak = h.wire_peak.max(sizes.iter().copied().max().unwrap_or(0));
        }
        if rss > guard.rss_kb {
            h.cut = Some(format!("memory {} MB passed {} MB at tick {tick}", rss / 1024, guard.rss_kb / 1024));
            break;
        }
        if guard.left() <= 0.0 {
            h.cut = Some(format!("the budget ran out at tick {tick}"));
            break;
        }
    }
    h.hwm_kb = rss_kb().1;
    h.wall = walled.elapsed().as_secs_f64();
    crate::rules::cover::OFF.with(|off| off.set(false));
    crate::world3d::set_enabled(false);
    h
}

/// A step's row of the table (seat 0's columns dashed when it does not draw).
fn line(h: &Held, draws: bool) -> String {
    let (hp50, hp95, hmax) = spread(&h.host);
    let (sp50, _, smax) = spread(&h.step);
    let (dp50, dp95, dmax) = spread(&h.seat);
    let shots = h.shots.iter().sum::<usize>() as f64 / h.shots.len().max(1) as f64;
    let seats = (h.ticks * h.seats).max(1);
    let wire = h.wire_sum as f64 / seats as f64;
    let seat = if draws { format!("{dp50:8.2} {dp95:8.2} {dmax:8.2}") } else { format!("{:>8} {:>8} {:>8}", "-", "-", "-") };
    format!(
        "{:>3} {:>6} {:>5} {:>6.0} | {hp50:8.2} {hp95:8.2} {hmax:8.2} | {sp50:8.2} {smax:8.2} | {seat} | {:>7} {:>7}{} | {:>9.0} {:>9.0} {:>8.1} {:>8} {:>9}{}",
        h.mix,
        h.target,
        h.ticks,
        shots,
        h.rss_kb / 1024,
        h.hwm_kb / 1024,
        if h.own_peak { " " } else { "*" },
        wire,
        wire * f64::from(HZ) / 1024.0,
        h.wire_peak as f64 / 1024.0,
        h.whole / 1024,
        h.nodes,
        h.cut.as_ref().map_or(String::new(), |c| format!("  CUT: {c}")),
    )
}

const HEADER: &str = "mix  shots ticks   live |  host p50      p95    worst |  step p50    worst |  seat p50      p95    worst |  RSS MB  peak MB | B/seat/tick KB/s/seat peak KB whole KB    values";

/// The ramp, as the lost run had it: the mixed storm then the homing one,
/// 100 to 50 000 shots, each mix inside 600 s and 20 GB.
#[test]
#[ignore = "a bench: run it in release with --nocapture, guarded"]
fn bullet_storm_ramp() {
    let env = |name: &str| std::env::var(name).ok().filter(|s| !s.is_empty());
    let list = |name: &str, or: &[usize]| {
        env(name).map_or_else(|| or.to_vec(), |s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect())
    };
    let mixes: Vec<u8> = list("STORM_RAMP_MIXES", &[0, 3]).into_iter().map(|m| m as u8).collect();
    let steps = list("STORM_RAMP_STEPS", &STEPS);
    let ticks: u32 = env("STORM_RAMP_TICKS").and_then(|s| s.parse().ok()).unwrap_or(TICKS);
    let budget: u64 = env("STORM_RAMP_SECS").and_then(|s| s.parse().ok()).unwrap_or(600);
    let rss_gb: u64 = env("STORM_RAMP_RSS_GB").and_then(|s| s.parse().ok()).unwrap_or(20);
    let party: u32 = env("STORM_RAMP_PARTY").and_then(|s| s.parse().ok()).unwrap_or(PARTY);
    let draws = env("STORM_RAMP_DRAW").is_none_or(|v| v != "0");
    let enabled = |name: &str| env(name).is_some_and(|v| v == "1");
    let drawing = Drawing {
        seats: draws,
        all_seats: enabled("STORM_RAMP_ALL_SEATS"),
        host: enabled("STORM_RAMP_HOST_DRAW"),
        world3d: enabled("STORM_RAMP_3D"),
        props: enabled("STORM_RAMP_PROPS"),
    };
    let tick_ms = 1000.0 / f64::from(HZ);
    eprintln!(
        "== bullet storm ramp: party {party}, {} monsters, {HZ} Hz ({tick_ms:.1} ms a tick), {ticks} ticks a step (+{WARM} warm), seat 0 {}, budget {budget} s and {rss_gb} GB a mix",
        foes(),
        if draws { "draws" } else { "does not draw" },
    );
    let mut table: Vec<(Held, bool)> = Vec::new();
    let mut panics = Vec::new();
    let mut stops = Vec::new();
    for &mix in &mixes {
        let guard = Guard { start: Instant::now(), budget: Duration::from_secs(budget), rss_kb: rss_gb << 20 };
        let mut last: Option<(f64, usize, u32)> = None;
        for &target in &steps {
            // Reckon the step from the last: its wall a tick, scaled by the shots.
            let mut hold_for = ticks;
            if let Some((wall, was, held)) = last {
                let per_tick = wall / f64::from(held + WARM) * target as f64 / was.max(1) as f64;
                let fits = (guard.left() / per_tick).floor() as i64 - i64::from(WARM);
                if fits < i64::from(FEWEST) {
                    let why = format!(
                        "mix {mix}: stopped before {target} shots: {FEWEST} ticks would take ~{:.0} s, {:.0} s of {budget} left",
                        per_tick * f64::from(FEWEST + WARM),
                        guard.left().max(0.0)
                    );
                    eprintln!("STOP {why}");
                    stops.push(why);
                    break;
                }
                hold_for = hold_for.min(fits as u32);
            }
            eprintln!("-- mix {mix}, {target} shots, {hold_for} ticks ({:.0} s of {budget} left)", guard.left());
            let held = catch_unwind(AssertUnwindSafe(|| hold(mix, target, hold_for, party, drawing, &guard)));
            let held = match held {
                Ok(held) => held,
                Err(e) => {
                    let what = e
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_default();
                    let why = format!("mix {mix}: PANIC at {target} shots: {what}");
                    eprintln!("STOP {why}");
                    crate::rules::cover::OFF.with(|off| off.set(false));
                    crate::world3d::set_enabled(false);
                    panics.push(why);
                    break;
                }
            };
            eprintln!("STEP {}", line(&held, draws));
            let t = &held.layers;
            let frames = held.ticks * if drawing.all_seats { party } else { 1 };
            eprintln!(
                "  layers ms/tick: clone {:.3}, publish {:.3}, encode {:.3}, watch {:.3}; ms/frame: decode {:.3}, draw {:.3}, stage {:.3}; stage B/frame {:.0}; host KB/tick {:.0} in {:.0} asks; seat KB/frame {:.0} in {:.0} asks; shot edits {} orders {} named {} gone {}",
                ms(t.clone, held.ticks), ms(t.publish, held.ticks), ms(t.encode, held.ticks), ms(t.watch, held.ticks),
                ms(t.decode, frames), ms(t.draw, frames), ms(t.stage, frames),
                t.stage_bytes as f64 / f64::from(frames.max(1)),
                t.host_asked as f64 / 1024.0 / f64::from(held.ticks.max(1)),
                t.host_allocs as f64 / f64::from(held.ticks.max(1)),
                t.seat_asked as f64 / 1024.0 / f64::from(frames.max(1)),
                t.seat_allocs as f64 / f64::from(frames.max(1)),
                t.list_edits, t.list_orders, t.list_named, t.list_gone,
            );
            let cut = held.cut.clone();
            last = Some((held.wall, target, held.ticks));
            table.push((held, draws));
            if let Some(cut) = cut {
                let why = format!("mix {mix}: stopped in {target} shots: {cut}");
                eprintln!("STOP {why}");
                stops.push(why);
                break;
            }
        }
        eprintln!("== mix {mix} done in {:.0} s, peak resident {} MB", guard.start.elapsed().as_secs_f64(), rss_kb().1 / 1024);
    }
    eprintln!("\n== the ramp (ms; host = step + what it said + clone + publish + {party} seats' patches; seat = seat 0 reads, draws, stages; {tick_ms:.1} ms a tick at {HZ} Hz)");
    eprintln!("{HEADER}");
    for (held, draws) in &table {
        eprintln!("{}", line(held, *draws));
    }
    if table.iter().any(|(h, _)| !h.own_peak) {
        eprintln!("(* peak MB is the process's so far: VmHWM could not be started over)");
    }
    for mix in &mixes {
        let real: Vec<_> = table.iter().filter(|(h, _)| h.mix == *mix).collect();
        let last_real = real.iter().filter(|(h, _)| spread(&h.host).1 <= tick_ms).map(|(h, _)| h.target).max();
        let first_over = real.iter().filter(|(h, _)| spread(&h.host).1 > tick_ms).map(|(h, _)| h.target).min();
        eprintln!(
            "REAL-TIME mix {mix}: host p95 inside {tick_ms:.1} ms up to {} shots; first over at {}",
            last_real.map_or("none".into(), |n| n.to_string()),
            first_over.map_or("none".into(), |n| n.to_string()),
        );
    }
    for stop in &stops {
        eprintln!("STOPPED {stop}");
    }
    assert!(panics.is_empty(), "the storm panicked: {panics:?}");
}
