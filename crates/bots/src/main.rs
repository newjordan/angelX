//! `angel-bots`: N real WebSocket clients against a muster.
//!
//! ```text
//! angel-bots --url ws://127.0.0.1:8790/ws --join KEY [--n 200] [--secs 60]
//!            [--slow 10] [--draw] [--warmup 5] [--stand] [--stray 600] [--delay 0]
//! ```
//!
//! Each bot drives the same runtime a browser does, walking about and
//! waving. `--slow K` makes K of them read their socket at 1 KB/s, to show a
//! slow client never slows the sim or anyone else. `--draw` renders every
//! frame at 60 fps, to measure the client's CPU natively. `--stray PX`
//! keeps the walkers within that many pixels of the muster point (a small
//! number packs the whole crowd into one view). `--delay S` waits before
//! the first bot joins, so a browser that joined first holds seat 0 in the
//! middle of the spawn spiral (the densest view, for the browser probe).
//! Measurement starts once everyone has joined and `--warmup` seconds have
//! passed.

use angel_bots::{BotConfig, Headless, Report, STRAY, pct, run};
use angel_muster::MusterClient;
use angel_muster::rules::{SNAPSHOT_HZ, SUB};
use std::thread;
use std::time::{Duration, Instant};

/// Bots join this far apart, as a room of people would, faster.
const JOIN_GAP: Duration = Duration::from_millis(10);

struct Args {
    url: String,
    join: String,
    n: u32,
    secs: u64,
    slow: u32,
    draw: bool,
    warmup: u64,
    stand: bool,
    stray: i32,
    delay: u64,
}

fn args() -> Result<Args, String> {
    let mut a = Args { url: String::new(), join: String::new(), n: 200, secs: 60, slow: 0, draw: false, warmup: 5, stand: false, stray: STRAY, delay: 0 };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--url" => a.url = value()?,
            "--join" => a.join = value()?,
            "--n" => a.n = value()?.parse().map_err(|_| "--n takes a number")?,
            "--secs" => a.secs = value()?.parse().map_err(|_| "--secs takes seconds")?,
            "--slow" => a.slow = value()?.parse().map_err(|_| "--slow takes a number")?,
            "--warmup" => a.warmup = value()?.parse().map_err(|_| "--warmup takes seconds")?,
            "--draw" => a.draw = true,
            "--stand" => a.stand = true,
            "--stray" => a.stray = value()?.parse::<i32>().map_err(|_| "--stray takes pixels")? * SUB,
            "--delay" => a.delay = value()?.parse().map_err(|_| "--delay takes seconds")?,
            "-h" | "--help" => {
                println!("angel-bots --url ws://HOST:PORT/ws --join KEY [--n 200] [--secs 60] [--slow K] [--draw] [--warmup 5] [--stand] [--stray 600] [--delay 0]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if a.url.is_empty() || a.join.is_empty() {
        return Err("--url and --join are needed".into());
    }
    Ok(a)
}

fn main() {
    let a = match args() {
        Ok(a) => a,
        Err(why) => {
            eprintln!("angel-bots: {why}");
            std::process::exit(2);
        }
    };
    thread::sleep(Duration::from_secs(a.delay));
    let start = Instant::now();
    let joined_by = start + JOIN_GAP * a.n;
    let measure_from = joined_by + Duration::from_secs(a.warmup);
    let until = measure_from + Duration::from_secs(a.secs);
    let mut bots = Vec::new();
    for i in 0..a.n {
        thread::sleep((start + JOIN_GAP * i).saturating_duration_since(Instant::now()));
        let cfg = BotConfig {
            url: a.url.clone(),
            join: a.join.clone(),
            name: format!("Bot {i}"),
            seed: u64::from(i) * 7919 + 17,
            wander: !a.stand,
            // The slow ones are spread through the crowd, not bunched.
            slow: a.slow > 0 && i % (a.n / a.slow).max(1) == 0 && i / (a.n / a.slow).max(1) < a.slow,
            fps: if a.draw { 60 } else { 30 },
            warmup: measure_from.saturating_duration_since(Instant::now()),
            stray: a.stray,
        };
        let draw = a.draw;
        let slow = cfg.slow;
        let handle = thread::Builder::new()
            .name(format!("bot-{i}"))
            .spawn(move || {
                let report = if draw { run::<MusterClient>(&cfg, until).0 } else { run::<Headless>(&cfg, until).0 };
                (slow, report)
            })
            .expect("a bot thread starts");
        bots.push(handle);
    }
    let reports: Vec<(bool, Report)> = bots.into_iter().map(|b| b.join().expect("a bot finishes")).collect();
    summarize(&a, &reports);
}

fn sorted(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(f32::total_cmp);
    v
}

fn summarize(a: &Args, reports: &[(bool, Report)]) {
    let (slow, normal): (Vec<&Report>, Vec<&Report>) = {
        let slow = reports.iter().filter(|(s, _)| *s).map(|(_, r)| r).collect();
        let normal = reports.iter().filter(|(s, _)| !*s).map(|(_, r)| r).collect();
        (slow, normal)
    };
    let nominal = 1000.0 / SNAPSHOT_HZ as f32;
    println!(
        "{} bots ({} slow), {} s measured after {} s warmup, {}, {}",
        a.n,
        slow.len(),
        a.secs,
        a.warmup,
        if a.draw { "drawing at 60 fps" } else { "headless" },
        if a.stand { "standing".to_string() } else { format!("wandering within {} px", a.stray / SUB) }
    );
    for (label, group) in [("normal", &normal), ("slow", &slow)] {
        if group.is_empty() {
            continue;
        }
        let measured: Vec<&&Report> = group.iter().filter(|r| r.secs > 0.0).collect();
        let rate = |f: &dyn Fn(&Report) -> f64| -> Vec<f32> { measured.iter().map(|r| f(r) as f32).collect() };
        let snaps = sorted(rate(&|r| r.snapshots as f64 / r.secs));
        let bytes = sorted(rate(&|r| r.bytes as f64 / r.secs));
        let intervals = sorted(measured.iter().flat_map(|r| r.intervals.iter().copied()).collect());
        let jitter = sorted(intervals.iter().map(|i| (i - nominal).abs()).collect());
        let rtts = sorted(measured.iter().flat_map(|r| r.rtts.iter().copied()).collect());
        let held = sorted(measured.iter().flat_map(|r| r.held.iter().map(|&h| h as f32)).collect());
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
        println!("{label} clients: {} measured of {}", measured.len(), group.len());
        println!(
            "  snapshots/s      avg {:.2}  min {:.2}  max {:.2}",
            mean(&snaps),
            snaps.first().unwrap_or(&f32::NAN),
            snaps.last().unwrap_or(&f32::NAN)
        );
        println!(
            "  bytes/s          avg {:.0}  p50 {:.0}  p99 {:.0}  max {:.0}",
            mean(&bytes),
            pct(&bytes, 0.5),
            pct(&bytes, 0.99),
            bytes.last().unwrap_or(&f32::NAN)
        );
        println!(
            "  snapshot gap     p50 {:.1} ms  p99 {:.1} ms  max {:.1} ms  (|gap - {nominal:.0}| p99 {:.1} ms)",
            pct(&intervals, 0.5),
            pct(&intervals, 0.99),
            intervals.last().unwrap_or(&f32::NAN),
            pct(&jitter, 0.99)
        );
        println!("  round trip       p50 {:.2} ms  p99 {:.2} ms  max {:.2} ms", pct(&rtts, 0.5), pct(&rtts, 0.99), rtts.last().unwrap_or(&f32::NAN));
        println!("  things in view   avg {:.1}  p50 {:.0}  max {:.0}", mean(&held), pct(&held, 0.5), held.last().unwrap_or(&f32::NAN));
        if a.draw {
            let frames = sorted(measured.iter().flat_map(|r| r.frames.iter().copied()).collect());
            println!(
                "  frame (native)   p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms  ({} frames)",
                pct(&frames, 0.5),
                pct(&frames, 0.99),
                frames.last().unwrap_or(&f32::NAN),
                frames.len()
            );
        }
    }
    let errors: Vec<&String> = reports.iter().filter_map(|(_, r)| r.error.as_ref()).collect();
    let rejects: Vec<&String> = reports.iter().filter_map(|(_, r)| r.rejected.as_ref()).collect();
    println!("disconnects {}  rejects {}", errors.len(), rejects.len());
    for e in errors.iter().chain(&rejects).take(10) {
        println!("  {e}");
    }
}
