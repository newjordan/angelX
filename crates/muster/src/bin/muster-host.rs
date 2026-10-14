//! `muster-host`: the muster on its own, for bots and browsers.
//!
//! ```text
//! muster-host [--port 8790] [--bind 0.0.0.0] [--bundle DIR] [--key KEY]
//!             [--log SECS] [--sheep N] [--measure] [--local N] [--stray PX]
//!             [--dev] [--tune FILE]
//! ```
//!
//! Prints the link for each door and the LAN link's QR code, then serves
//! until stopped. `--bundle` serves a browser client (a directory with
//! `index.html` and its files). `--measure` waits for a crowd to gather,
//! measures while all of it is connected, prints the window's stats once
//! the first player leaves, and exits when the field is empty. `--local N`
//! seats N players in this process, as the host's own terminal and its
//! couch players are: each a `NativeLink` on an in-process connection,
//! drawing at 60 fps on a thread of its own and walking about within
//! `--stray` pixels of the muster point (as `angel-bots --stray`); with
//! `--measure`, what they saw over the window is reported too. `--dev`
//! lets any page set the server's knobs from its dev panel (F2), and
//! `--tune FILE` keeps the ones moved for the next start.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(why) = host::run() {
        eprintln!("muster-host: {why}");
        std::process::exit(1);
    }
}

// The host is native only; the wasm build of this crate is its client half.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod host {
    use angel_muster::MusterServer;
    use angel_serve::link::qr;
    use angel_serve::{ClientBundle, ServeConfig, Server, Totals};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
    use std::thread;
    use std::time::{Duration, Instant};

    /// Players must stay this long at their peak before measuring starts.
    const SETTLE: Duration = Duration::from_secs(3);
    const POLL: Duration = Duration::from_secs(1);

    struct Args {
        port: u16,
        bind: IpAddr,
        bundle: Option<PathBuf>,
        key: Option<String>,
        log: Option<u64>,
        sheep: u32,
        measure: bool,
        local: u32,
        stray: i32,
        dev: bool,
        tune: Option<PathBuf>,
    }

    fn args() -> Result<Args, String> {
        let mut a = Args {
            port: 8790,
            bind: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            bundle: None,
            key: None,
            log: None,
            sheep: angel_muster::rules::SHEEP,
            measure: false,
            local: 0,
            stray: 600,
            dev: false,
            tune: None,
        };
        let mut it = std::env::args().skip(1);
        while let Some(flag) = it.next() {
            let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "--port" => a.port = value()?.parse().map_err(|_| "--port takes a number")?,
                "--bind" => a.bind = value()?.parse().map_err(|_| "--bind takes an IP address")?,
                "--bundle" => a.bundle = Some(PathBuf::from(value()?)),
                "--key" => a.key = Some(value()?),
                "--log" => a.log = Some(value()?.parse().map_err(|_| "--log takes seconds")?),
                "--sheep" => a.sheep = value()?.parse().map_err(|_| "--sheep takes a number")?,
                "--measure" => a.measure = true,
                "--local" => a.local = value()?.parse().map_err(|_| "--local takes a number")?,
                "--stray" => a.stray = value()?.parse().map_err(|_| "--stray takes pixels")?,
                "--dev" => a.dev = true,
                "--tune" => a.tune = Some(PathBuf::from(value()?)),
                "-h" | "--help" => {
                    println!(
                        "muster-host [--port 8790] [--bind 0.0.0.0] [--bundle DIR] [--key KEY] [--log SECS] [--sheep N] [--measure] [--local N] [--stray PX] [--dev] [--tune FILE]"
                    );
                    std::process::exit(0);
                }
                other => return Err(format!("unknown flag {other}")),
            }
        }
        Ok(a)
    }

    pub fn run() -> Result<(), String> {
        let a = args()?;
        let mut config = ServeConfig::new(SocketAddr::new(a.bind, a.port));
        if let Some(dir) = &a.bundle {
            config.bundle = Some(ClientBundle::from_dir(dir).map_err(|e| format!("bundle {}: {e}", dir.display()))?);
        }
        config.key = a.key.clone();
        config.log_every = a.log.map(Duration::from_secs);
        config.dev = a.dev;
        config.tune_file = a.tune.clone();
        let server = Server::start(MusterServer::new(a.sheep), config).map_err(|e| format!("cannot serve: {e}"))?;
        println!("the muster is open on {}", server.addr());
        println!("join key {}", server.key());
        let links = server.links();
        for link in &links {
            println!("  {link}");
        }
        if let Some(code) = links.first().and_then(|l| qr(l, true)) {
            println!("{code}");
        }
        let measuring = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let mut couch = Vec::new();
        for i in 0..a.local {
            let conn = server.connect_local().map_err(|e| format!("an in-process player: {e}"))?;
            let (measuring, stop, stray) = (measuring.clone(), stop.clone(), a.stray);
            let player = thread::Builder::new()
                .name(format!("couch-{i}"))
                .spawn(move || couch::play(conn, i, stray, &measuring, &stop))
                .map_err(|e| format!("an in-process player's thread: {e}"))?;
            couch.push(player);
        }
        if a.measure {
            measure(&server, &measuring);
            stop.store(true, SeqCst);
            for (i, player) in couch.into_iter().enumerate() {
                if let Ok(seen) = player.join() {
                    seen.report(i);
                }
            }
            start_over_when_empty(&server);
            server.stop();
            return Ok(());
        }
        loop {
            thread::park();
        }
    }

    /// Wait for a crowd, measure while all of it is in, report when it thins.
    fn measure(server: &Server, measuring: &AtomicBool) {
        let mut peak = 0;
        let mut peak_since = Instant::now();
        let mut start: Option<Totals> = None;
        let mut last: Option<Totals> = None;
        loop {
            thread::sleep(POLL);
            let now = server.totals();
            let players = now.players;
            if players > peak {
                (peak, peak_since, start, last) = (players, Instant::now(), None, None);
                measuring.store(false, SeqCst);
                continue;
            }
            if players == peak && peak > 0 {
                if start.is_none() && peak_since.elapsed() >= SETTLE {
                    start = Some(now);
                    measuring.store(true, SeqCst);
                } else if start.is_some() {
                    last = Some(now);
                }
                continue;
            }
            measuring.store(false, SeqCst);
            if let (Some(start), Some(last)) = (&start, &last) {
                report(&last.since(start), peak);
                return;
            }
            if players == 0 && peak > 0 {
                println!("the crowd left before it could be measured");
                return;
            }
        }
    }

    /// Every socket client gone (the in-process ones said goodbye already).
    fn start_over_when_empty(server: &Server) {
        while server.totals().players > 0 {
            thread::sleep(POLL);
        }
    }

    fn report(s: &angel_serve::Stats, peak: u32) {
        println!("measured {:.1} s with {peak} players ({} watching)", s.secs, s.spectators);
        println!("  ticks           {} ({:.1}/s)", s.ticks, s.ticks as f64 / s.secs);
        println!(
            "  tick (whole)    p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms  avg {:.3} ms",
            s.tick.p50, s.tick.p99, s.tick.max, s.tick.avg
        );
        println!("  step            p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms", s.step.p50, s.step.p99, s.step.max);
        println!(
            "  publish + wake  p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms",
            s.publish.p50, s.publish.p99, s.publish.max
        );
        println!(
            "  writer encode   p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms  ({} snapshots)",
            s.encode.p50, s.encode.p99, s.encode.max, s.encode.n
        );
        println!(
            "  per client      avg {:.0} B/s  p50 {:.0}  p99 {:.0}  max {:.0}  ({} clients)",
            s.client_bytes.avg, s.client_bytes.p50, s.client_bytes.p99, s.client_bytes.max, s.client_bytes.n
        );
        println!(
            "  snapshots/s     avg {:.2}  p50 {:.2}  ({} skipped, {} held back)",
            s.client_snapshots.avg, s.client_snapshots.p50, s.skipped, s.held
        );
        println!(
            "  connections     joined {} resumed {} rejected {} violations {} silent {} busy {} write errors {}",
            s.joined, s.resumed, s.rejected, s.violations, s.silent, s.busy, s.write_errors
        );
        println!("  {s}");
    }

    /// Players in this process: the host's own terminal and its couch
    /// players, each a `NativeLink` on an in-process connection.
    mod couch {
        use angel_muster::MusterClient;
        use angel_muster::rules::{FIELD, Knight, SNAPSHOT_HZ, SUB};
        use angel_net::input::button;
        use angel_net::proto::ToClient;
        use angel_serve::LocalConn;
        use angel_serve::client::{LinkOptions, NativeLink};
        use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
        use std::sync::{Arc, Mutex};
        use std::thread;
        use std::time::{Duration, Instant};

        /// A terminal's display frame, at 60 fps.
        const FRAME: Duration = Duration::from_micros(16_667);

        /// What one in-process player saw while measuring.
        #[derive(Default)]
        pub struct Seen {
            secs: f64,
            snapshots: u64,
            bytes: u64,
            /// Gaps between snapshot arrivals, ms.
            gaps: Vec<f32>,
            /// Ping round trips, ms.
            rtts: Vec<f32>,
            /// `NativeLink::frame` (taking what arrived, the commands, and
            /// prediction, interpolation and the 320x180 picture), ms.
            frames: Vec<f32>,
            last_snapshot: Option<f64>,
            reconnects: u32,
            seat: Option<u32>,
        }

        impl Seen {
            pub fn report(&self, i: usize) {
                let sorted = |v: &[f32]| {
                    let mut v = v.to_vec();
                    v.sort_by(f32::total_cmp);
                    v
                };
                let pct = |v: &[f32], p: f64| {
                    if v.is_empty() {
                        return f32::NAN;
                    }
                    v[((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1]
                };
                let nominal = 1000.0 / SNAPSHOT_HZ as f32;
                let (gaps, rtts, frames) = (sorted(&self.gaps), sorted(&self.rtts), sorted(&self.frames));
                let jitter = sorted(&gaps.iter().map(|g| (g - nominal).abs()).collect::<Vec<_>>());
                let secs = self.secs.max(1e-9);
                println!("in-process player {i} (seat {:?}), measured {:.1} s", self.seat, self.secs);
                println!(
                    "  snapshots/s      {:.2}   bytes/s {:.0}   reconnects {}",
                    self.snapshots as f64 / secs,
                    self.bytes as f64 / secs,
                    self.reconnects
                );
                println!(
                    "  snapshot gap     p50 {:.1} ms  p99 {:.1} ms  max {:.1} ms  (|gap - {nominal:.0}| p99 {:.1} ms)",
                    pct(&gaps, 0.5),
                    pct(&gaps, 0.99),
                    gaps.last().unwrap_or(&f32::NAN),
                    pct(&jitter, 0.99)
                );
                println!(
                    "  round trip       p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms",
                    pct(&rtts, 0.5),
                    pct(&rtts, 0.99),
                    rtts.last().unwrap_or(&f32::NAN)
                );
                println!(
                    "  frame            p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms  ({} frames)",
                    pct(&frames, 0.5),
                    pct(&frames, 0.99),
                    frames.last().unwrap_or(&f32::NAN),
                    frames.len()
                );
            }
        }

        /// Play until `stop`: draw every frame, walk about within `stray`
        /// pixels of the muster point, wave now and then.
        pub fn play(conn: LocalConn, i: u32, stray: i32, measuring: &Arc<AtomicBool>, stop: &AtomicBool) -> Seen {
            let stray = stray * SUB;
            let seen = Arc::new(Mutex::new(Seen::default()));
            let mut link = NativeLink::<MusterClient>::local(conn, &format!("Host {i}"), LinkOptions::default());
            let (tap_seen, tap_measuring) = (seen.clone(), measuring.clone());
            link.on_message(move |bytes, at| {
                if !tap_measuring.load(SeqCst) {
                    return;
                }
                let mut s = tap_seen.lock().expect("seen lock");
                s.bytes += bytes.len() as u64;
                match ToClient::decode(bytes) {
                    Ok(ToClient::Snapshot { .. }) => {
                        if let Some(last) = s.last_snapshot {
                            s.gaps.push((at - last) as f32);
                        }
                        s.last_snapshot = Some(at);
                        s.snapshots += 1;
                    }
                    Ok(ToClient::Pong { t, .. }) => s.rtts.push((at - t) as f32),
                    _ => {}
                }
            });
            let start = Instant::now();
            let ms = || start.elapsed().as_secs_f64() * 1000.0;
            let mut rng = u64::from(i) * 7919 + 17;
            let mut turn_at = 0.0;
            let mut next = Instant::now();
            let mut measured_from: Option<Instant> = None;
            while !stop.load(SeqCst) {
                let now = ms();
                if now >= turn_at {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    turn_at = now + 800.0 + (rng % 2200) as f64;
                    let own = link.runtime().game().and_then(|g| Knight::from_state(g.mirror().get(g.seat()?)?.latest()));
                    let stick = match own.map(|k| (FIELD / 2 - k.x, FIELD / 2 - k.y)) {
                        Some((dx, dy)) if dx.abs() > stray || dy.abs() > stray => {
                            let len = f64::from(dx).hypot(f64::from(dy));
                            ((f64::from(dx) / len * 127.0) as i8, (f64::from(dy) / len * 127.0) as i8)
                        }
                        _ if rng % 10 < 3 => (0, 0),
                        _ => {
                            let a = (rng % 360) as f64 * std::f64::consts::PI / 180.0;
                            ((a.cos() * 127.0) as i8, (a.sin() * 127.0) as i8)
                        }
                    };
                    let hands = link.hands();
                    hands.stick(Some(stick));
                    if rng % 4 == 0 {
                        hands.down(button::FIRE);
                        hands.up(button::FIRE);
                    }
                }
                let on = measuring.load(SeqCst);
                if on && measured_from.is_none() {
                    // A window opens: what an abandoned one saw is dropped.
                    *seen.lock().expect("seen lock") = Seen::default();
                    measured_from = Some(Instant::now());
                } else if !on && let Some(from) = measured_from.take() {
                    seen.lock().expect("seen lock").secs = from.elapsed().as_secs_f64();
                }
                let t0 = Instant::now();
                link.frame(now);
                let took = t0.elapsed();
                if on {
                    seen.lock().expect("seen lock").frames.push(took.as_secs_f32() * 1000.0);
                }
                next += FRAME;
                let now = Instant::now();
                if next > now {
                    thread::sleep(next - now);
                } else {
                    next = now;
                }
            }
            if let Some(from) = measured_from {
                seen.lock().expect("seen lock").secs = from.elapsed().as_secs_f64();
            }
            let (reconnects, seat) = (link.reconnects(), link.runtime().seat());
            link.bye();
            drop(link);
            let mut s = std::mem::take(&mut *seen.lock().expect("seen lock"));
            s.reconnects = reconnects;
            s.seat = seat;
            s
        }
    }
}
