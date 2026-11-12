//! The optimization sweep's measurement (2026-11-08): the host's own tick on a
//! four-knight party, scene by scene, timed the way the host runs it.
//!
//! ```text
//! ANGEL_REALM_PLAN=tabula ANGEL_BENCH_WORKER=<angel-delve-system> \
//!   cargo test -p angel-realm-host --lib --profile fast delve_optimize_bench \
//!   -- --ignored --nocapture
//! ```
//!
//! Each scene is a `DelveServer` on a `Shared`, stepped and published by hand
//! (`GameServer::step`, `publish`): the real tick, with the rules worker when
//! `ANGEL_BENCH_WORKER` names one (`hot`) and without it (`embedded`). Each
//! seat's share of the tree is then diffed and encoded as its writer does,
//! and one client reads the tick and draws a frame offscreen.
//!
//! One line a scene, mode and metric: `BENCH scene mode metric n=.. p50=..
//! p95=.. max=.. mean=..` in microseconds, `BYTES scene mode per_seat_tick=..`,
//! and `STATE scene mode hash=..` of the run after the last tick (the same
//! binary's hot and embedded runs agree, and so does every build's).
//! `ANGEL_BENCH_SCENES=a,b`, `ANGEL_BENCH_TICKS=n`, `ANGEL_BENCH_ROUNDS=n`
//! pick what runs; rounds alternate the modes so a busy machine slows both.
use super::*;
use crate::together_shooter::{Input, RoomKind, Run};
use angel_net::client::{Frame, GameClient, View};
use angel_net::proto::{PROTOCOL, Welcome};
use angel_serve::Replicate;
use std::hash::{Hash, Hasher};

const SEATS: u32 = 4;

fn party(mut run: Run) -> Run {
    for (id, name) in [(3, "Three"), (4, "Four")] {
        run.join_seat(id, name);
    }
    run
}

fn home_party() -> Run {
    party(Run::at_home(41, 1, Some("Two"), Default::default(), Default::default()))
}

fn kingdom() -> Run {
    let mut run = home_party();
    run.walk_kingdom(None);
    assert_eq!(run.room().kind, RoomKind::Kingdom);
    run
}

/// Floor 4's fight room with the most foes.
fn crowded_fight() -> Run {
    let mut run = party(Run::new(3, 9, Some("Two")));
    while run.dungeon.depth < 4 {
        run.descend_for_test();
    }
    let mut best = (0, 0);
    for index in 0..run.dungeon.rooms.len() {
        if run.dungeon.rooms[index].kind != RoomKind::Fight {
            continue;
        }
        let mut probe = run.clone();
        probe.enter_for_test(index);
        if probe.enemies.len() > best.0 {
            best = (probe.enemies.len(), index);
        }
    }
    run.enter_for_test(best.1);
    run
}

/// The ballot between floors, its clock counting down a tick at a time.
fn crossroads() -> Run {
    use angel_delve::rules::crossroads::{Game, Stage, State, VOTE_TIME};
    let mut run = crowded_fight();
    run.floor_plates.clear();
    run.crossroads = Some(State {
        depth: 4,
        pack: angel_delve::rules::Pack::Archive,
        choices: [Game::Racers, Game::Powder, Game::Shaft],
        seats: vec![1, 2, 3, 4],
        votes: Default::default(),
        stage: Stage::Voting,
        left: VOTE_TIME,
        chosen: None,
        order: Vec::new(),
    });
    run
}

fn boss() -> Run {
    let mut run = party(Run::new(3, 9, Some("Two")));
    run.lair_for_test();
    run
}

fn racers() -> Run {
    use angel_delve::rules::{knight_racers, roundtable};
    let mut run = home_party();
    run.walk_kingdom(Some(knight_racers::ENTRY));
    roundtable::enter(&mut run, &knight_racers::ADVENTURE);
    if let Some(race) = run.knight_racers.as_mut() {
        race.skip_menus();
    }
    assert!(run.knight_racers.is_some(), "the race is on");
    run
}

fn pub_room() -> Run {
    let mut run = home_party();
    let tavern = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Alehouse)
        .expect("the home has the Sozzled Griffin");
    run.enter_for_test(tavern);
    run
}

/// The realm's standing wishes as a host with a built-up realm has them: each
/// one a name, its words and 32 by 32 rows of ink. A sensitivity probe, not a
/// count anyone measured on a real realm. `salt` changes the ids, so a list
/// that places the same lots is still a list the caches have not seen.
fn stand_wishes_salted(n: usize, salt: u32) {
    use angel_delve::realm::{Status, Wish};
    let wishes: Vec<Wish> = (0..n)
        .map(|i| Wish {
            id: format!("wish-{salt}-{i:04}"),
            name: format!("A built wish number {i}"),
            words: "a tower of pale stone with a banner and a small garden at its foot".into(),
            by: "Two".into(),
            status: Status::Built,
            price: Default::default(),
            art: (0..32).map(|r| (0..32).map(|c| ['.', '#', 'a', 'b', 'c'][(r * 7 + c * 3 + i) % 5]).collect()).collect(),
            near: String::new(),
            paid: Vec::new(),
            raised: i as u32,
            lot: None,
        })
        .collect();
    crate::realm_standing::publish(Arc::new(wishes));
}

fn stand_wishes(n: usize) {
    stand_wishes_salted(n, 0);
}

/// A new salt each time it is asked.
fn fresh_salt() -> u32 {
    static SALT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    SALT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn scene(name: &str) -> Run {
    if name.starts_with("wish_lands") {
        // Forty wishes stand; one more lands partway through (`run_scene`).
        angel_delve::world::wishes::SLOW_SEARCH.store(name.ends_with("slow"), std::sync::atomic::Ordering::Relaxed);
        stand_wishes_salted(40, fresh_salt());
        let mut run = kingdom();
        run.kingdom.wishes = crate::realm_standing::standing_wishes();
        return run;
    }
    if name == "kingdom_wishes" {
        stand_wishes(40);
        let mut run = kingdom();
        run.kingdom.wishes = crate::realm_standing::standing_wishes();
        return run;
    }
    stand_wishes(0);
    match name {
        "kingdom" => kingdom(),
        "fight4" => crowded_fight(),
        "boss" => boss(),
        "crossroads" => crossroads(),
        "racers" => racers(),
        "pub" => pub_room(),
        _ => panic!("no scene {name}"),
    }
}

/// What the four knights press on tick `i`: walk in turns, loose a shot every
/// third tick, and (on the road) hold the gas.
fn presses(name: &str, i: usize) -> [Input; 4] {
    let mut out = [Input::default(); 4];
    for (seat, input) in out.iter_mut().enumerate() {
        let turn = (i / 20 + seat) % 4;
        let (mx, my) = [(1, 0), (0, 1), (-1, 0), (0, -1)][turn];
        *input = if name == "racers" {
            Input { move_x: [0, 1, 0, -1][(i / 15 + seat) % 4], move_y: -1, ..Default::default() }
        } else {
            Input { move_x: mx, move_y: my, aim_x: 1, fire: i % 3 == 0, ..Default::default() }
        };
    }
    out
}

#[derive(Default)]
struct Samples(Vec<f64>);

impl Samples {
    fn add(&mut self, us: f64) {
        self.0.push(us);
    }
    fn say(&mut self, scene: &str, mode: &str, metric: &str) {
        self.0.sort_by(|a, b| a.total_cmp(b));
        let n = self.0.len();
        if n == 0 {
            return;
        }
        let at = |p: f64| self.0[((n as f64 * p) as usize).min(n - 1)];
        let mean = self.0.iter().sum::<f64>() / n as f64;
        println!(
            "BENCH {scene} {mode} {metric} n={n} p50={:.0} p95={:.0} max={:.0} mean={mean:.0}",
            at(0.5),
            at(0.95),
            self.0[n - 1]
        );
    }
}

/// `DelveClient::snapshot`'s reading of a tree, without the client round it.
#[derive(Default)]
struct Probe {
    names: angel_net::doc::wire::Reader,
    tree: Option<Value>,
    guest: angel_delve::wire::Guest,
}

impl Probe {
    fn read(&mut self, whole: bool, body: &[u8]) {
        use angel_net::codec::Bytes;
        let (mut fields, mut shots) = (None, angel_delve::wire::Shots::Rebuild);
        if whole {
            self.names = Default::default();
            let mut b = Bytes::new(body);
            self.tree = Some(self.names.value(&mut b).unwrap());
        } else if !body.is_empty() {
            let mut b = Bytes::new(body);
            let patch = self.names.patch(&mut b).unwrap();
            fields = angel_delve::wire::Guest::fields(&patch);
            shots = angel_delve::wire::Guest::shots_of(&patch);
            angel_net::doc::delta::apply(self.tree.as_mut().unwrap(), patch).unwrap();
        } else {
            return;
        }
        let tree = self.tree.as_ref().unwrap();
        self.guest.read(tree, fields.as_deref(), &shots).unwrap();
    }
}

fn tick_of(i: usize) -> u32 {
    i as u32 + 1
}

fn us(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e6
}

/// CPU time, in microseconds, of this thread (`THREAD`) or of the whole process
/// (`PROCESS`: the rules worker's own pipe thread counts here too). A busy machine
/// stretches the wall clock but not the CPU a piece of work takes.
fn cpu(clock: libc::clockid_t) -> f64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(clock, &mut t) };
    t.tv_sec as f64 * 1e6 + t.tv_nsec as f64 / 1e3
}
const THREAD: libc::clockid_t = libc::CLOCK_THREAD_CPUTIME_ID;
const PROCESS: libc::clockid_t = libc::CLOCK_PROCESS_CPUTIME_ID;

/// CPU time a child process has used, in microseconds (clock ticks of 10 ms).
fn child_cpu(pid: u32) -> Option<f64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = after.split_whitespace().collect();
    let (utime, stime): (f64, f64) = (fields.get(11)?.parse().ok()?, fields.get(12)?.parse().ok()?);
    Some((utime + stime) * 1e4)
}

fn hash_of(run: &Run) -> u64 {
    let state = angel_delve::rules::runtime_state::capture(run).expect("a state");
    let bytes = serde_json::to_vec(&state.into_value()).unwrap();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

/// The first `apply` takes the prepared worker up; until one has, the engine
/// answers that it is preparing. Paused calls (`step` false) change nothing.
fn wait_for_worker(shared: &Shared) {
    let began = Instant::now();
    loop {
        {
            let mut core = lock(&shared.core);
            step_system(&mut core, &BTreeMap::new(), false);
            if core.engine.as_ref().is_some_and(|engine| engine.status().active.is_some()) && core.engine_error.is_none() {
                return;
            }
        }
        assert!(began.elapsed() < Duration::from_secs(120), "the rules worker never came up");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// One scene in one mode, `ticks` long: every metric's samples go to `out`.
fn run_scene(name: &str, mode: &str, ticks: usize, out: &mut BTreeMap<&'static str, Samples>, bytes: &mut usize) {
    use angel_delve::wire;
    let hot = mode == "hot";
    let _guard = crate::tests::env_lock();
    let worker = std::env::var("ANGEL_BENCH_WORKER").ok();
    let _env = if hot {
        crate::tests::TestEnvGuard::set("ANGEL_DELVE_SYSTEM", worker.as_deref().expect("ANGEL_BENCH_WORKER"))
    } else {
        crate::tests::TestEnvGuard::unset("ANGEL_DELVE_SYSTEM")
    };
    let run = scene(name);
    let shared = Arc::new(Shared::new(run, None));
    lock(&shared.controls).active = true;
    if hot {
        wait_for_worker(&shared);
    }
    let mut server = DelveServer::new(Arc::clone(&shared), SEATS, false);
    let rep = angel_serve::DocRep(SeatView);
    let mut viewers: Vec<_> = (0..SEATS).map(|seat| rep.viewer(Some(seat))).collect();
    let params = wire::Params::of(angel_delve::settings::get()).encode();
    let welcome = Welcome {
        protocol: PROTOCOL,
        game: crate::together_seat::Client::GAME,
        seat: Some(1),
        seat_token: "",
        tick_hz: 30,
        snapshot_hz: 30,
        tick: 0,
        params: &params,
    };
    let mut client = crate::together_seat::Client::boot(&welcome).unwrap();
    // A second reader of seat 1's snapshots, as the client reads them (`DelveClient::snapshot`),
    // for what `wire::Guest` says of itself: ticks read by putting fields back, ticks read whole,
    // and the fields that made it read whole.
    let mut probe = Probe::default();
    let mut frame = Frame::default();
    // The first ticks lay the worker's state down and the caches warm: not counted.
    let warm = 12;
    let started_at = lock(&shared.core).run.tick;
    let worker_pid = lock(&shared.core).engine.as_ref().and_then(|e| e.status().worker_pid);
    let mut counted_from: Option<(f64, Option<f64>, Instant)> = None;
    let mut landed_at: Option<u32> = None;
    let mut worst_after_landing = (0.0f64, 0.0f64);
    for i in 0..ticks + warm {
        // The knights are kept on their feet, so the scene stays what it is.
        {
            let mut core = lock(&shared.core);
            for hero in core.run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
        }
        if name.starts_with("wish_lands") && i == warm + 60 {
            stand_wishes_salted(41, fresh_salt());
            landed_at = Some(tick_of(i));
        }
        let keys = presses(name, i);
        let cmds: Vec<Option<DelveCmd>> = keys
            .iter()
            .map(|input| Some(DelveCmd { held: *input, pressed: Input::default() }))
            .collect();
        let tick = i as u32 + 1;
        let counted = i >= warm;
        let mut events = angel_serve::Events::default();
        let at = Instant::now();
        let cpu_at = cpu(PROCESS);
        server.step(tick, &cmds, &mut events);
        let step = us(at);
        let step_cpu = cpu(PROCESS) - cpu_at;
        if i == 0 {
            // The first step of a run: caches cold (a rules worker's own are, whatever the host's).
            println!("FIRST_STEP {name} {mode} {:.1} ms", step / 1000.0);
        }
        if hot && let Some(error) = lock(&shared.core).engine_error.clone() {
            println!("ENGINE_ERROR {name} tick {tick}: {error}");
            break;
        }
        let at = Instant::now();
        let cpu_at = cpu(THREAD);
        let doc = server.publish(tick);
        let publish = us(at);
        let publish_cpu = cpu(THREAD) - cpu_at;
        let mut encode = 0.0;
        let mut encode_cpu = 0.0;
        let mut read = 0.0;
        let mut read_cpu = 0.0;
        for (seat, viewer) in viewers.iter_mut().enumerate() {
            let mut buf = Vec::new();
            let at = Instant::now();
            let cpu_at = cpu(THREAD);
            let whole = rep.snapshot(&doc, Some(seat as u32), viewer, &mut buf);
            encode += us(at);
            encode_cpu += cpu(THREAD) - cpu_at;
            if counted {
                *bytes += buf.len();
            }
            if seat == 1 {
                probe.read(whole, &buf);
                let at = Instant::now();
                let cpu_at = cpu(THREAD);
                client.snapshot(i as u32 + 1, whole, &buf).unwrap();
                read = us(at);
                read_cpu = cpu(THREAD) - cpu_at;
            }
        }
        let view = View {
            render_tick: i as f64 + 0.5,
            newest_tick: i as u32 + 1,
            ack: 0,
            pending: &[],
            partial: 0.5,
            hands: Default::default(),
            seat: Some(1),
            now_ms: i as f64 * 33.3,
            correct_ms: angel_net::clock::CORRECT_MS,
        };
        let at = Instant::now();
        let cpu_at = cpu(THREAD);
        client.draw(&view, &mut frame);
        let draw = us(at);
        let draw_cpu = cpu(THREAD) - cpu_at;
        if counted && counted_from.is_none() {
            counted_from = Some((cpu(PROCESS), worker_pid.and_then(child_cpu), Instant::now()));
        }
        if landed_at.is_some_and(|at| tick >= at) {
            let wall = step + publish + encode + read + draw;
            let on_cpu = step_cpu + publish_cpu + encode_cpu + read_cpu + draw_cpu;
            if wall > worst_after_landing.0 {
                worst_after_landing = (wall, on_cpu);
            }
        }
        if counted {
            out.entry("step").or_default().add(step);
            out.entry("publish").or_default().add(publish);
            out.entry("encode4").or_default().add(encode);
            out.entry("client_read").or_default().add(read);
            out.entry("client_draw").or_default().add(draw);
            out.entry("host_tick").or_default().add(step + publish + encode);
            out.entry("cpu_step").or_default().add(step_cpu);
            out.entry("cpu_publish").or_default().add(publish_cpu);
            out.entry("cpu_encode4").or_default().add(encode_cpu);
            out.entry("cpu_client_read").or_default().add(read_cpu);
            out.entry("cpu_client_draw").or_default().add(draw_cpu);
            out.entry("cpu_host_tick").or_default().add(step_cpu + publish_cpu + encode_cpu);
        }
        // The checkpoint's copy under the lock and its encode, as the service takes them.
        if counted && i % 30 == 0 {
            let at = Instant::now();
            let (run, commands) = {
                let core = lock(&shared.core);
                (core.run.clone(), core.commands.clone())
            };
            out.entry("ckpt_clone_locked").or_default().add(us(at));
            let at = Instant::now();
            let _ = encode_checkpoint(1, &run, None, false, Some(&commands));
            out.entry("ckpt_encode").or_default().add(us(at));
        }
    }
    if let Some((proc_at, worker_at, wall_at)) = counted_from {
        let n = ticks as f64;
        let host = (cpu(PROCESS) - proc_at) / n;
        let worker = worker_pid.zip(worker_at).and_then(|(pid, at)| child_cpu(pid).map(|now| (now - at) / n));
        // Everything the tick costs: the host process (the sim, the publish, the four writers' work,
        // one client reading and drawing) and the rules worker's process.
        println!("CPU {name} {mode} host_process_us_per_tick={host:.0} worker_process_us_per_tick={:.0} wall_us_per_tick={:.0}", worker.unwrap_or(0.0), wall_at.elapsed().as_secs_f64() * 1e6 / n);
    }
    {
        let (fast, full, miss) = probe.guest.stats();
        println!("GUEST {name} {mode} ticks_read_by_putting_fields_back={fast} ticks_read_whole={full} fields_that_forced_a_whole_read=[{miss}]");
    }
    if landed_at.is_some() {
        println!(
            "LANDING {name} {mode} worst_tick_after_the_wish_landed_wall_ms={:.1} cpu_ms={:.1}",
            worst_after_landing.0 / 1000.0,
            worst_after_landing.1 / 1000.0
        );
    }
    let core = lock(&shared.core);
    let alive = core.run.players.values().filter(|h| h.hp > 0).count();
    println!(
        "SCENE {name} {mode} ticks={} room={:?} foes={} shots={} knights_alive={alive} frame={}x{} engine_error={:?}",
        core.run.tick - started_at,
        core.run.room().kind,
        core.run.enemies.len(),
        core.run.projectiles.len(),
        frame.w,
        frame.h,
        core.engine_error,
    );
    println!("STATE {name} {mode} hash={:016x}", hash_of(&core.run));
}

#[test]
#[ignore]
fn delve_optimize_bench() {
    let want = std::env::var("ANGEL_BENCH_SCENES").unwrap_or_else(|_| "kingdom,fight4,boss,racers,pub".into());
    let ticks: usize = std::env::var("ANGEL_BENCH_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(150);
    let rounds: usize = std::env::var("ANGEL_BENCH_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    let wanted = std::env::var("ANGEL_BENCH_MODES").unwrap_or_default();
    let modes: Vec<&str> = if std::env::var("ANGEL_BENCH_WORKER").is_ok() { vec!["embedded", "hot"] } else { vec!["embedded"] }
        .into_iter()
        .filter(|m| wanted.is_empty() || wanted.split(',').any(|w| w == *m))
        .collect();
    for name in want.split(',') {
        let mut all: BTreeMap<(&str, &'static str), Samples> = BTreeMap::new();
        let mut bytes: BTreeMap<&str, usize> = BTreeMap::new();
        for _ in 0..rounds {
            for &mode in &modes {
                let mut out = BTreeMap::new();
                let mut sent = 0;
                run_scene(name, mode, ticks, &mut out, &mut sent);
                *bytes.entry(mode).or_default() += sent;
                for (metric, samples) in out {
                    all.entry((mode, metric)).or_default().0.extend(samples.0);
                }
            }
        }
        for ((mode, metric), samples) in &mut all {
            samples.say(name, mode, metric);
        }
        for (mode, total) in bytes {
            let per = total / (rounds * ticks * SEATS as usize);
            println!("BYTES {name} {mode} per_seat_tick={per} per_seat_second={}", per * 30);
        }
    }
}

/// `angel_live::delta::diff` as it was before whole subtrees were asked about first: every node
/// walked, a path built for each. Kept here to time the one against the other on the same states.
fn diff_every_node(old: &serde_json::Value, new: &serde_json::Value) -> Vec<serde_json::Value> {
    use serde_json::{Value, json};
    fn walk(old: &Value, new: &Value, path: &mut Vec<Value>, ops: &mut Vec<Value>) {
        match (old, new) {
            (Value::Object(a), Value::Object(b)) => {
                for (key, value) in b {
                    path.push(Value::String(key.clone()));
                    match a.get(key) {
                        Some(old) => walk(old, value, path, ops),
                        None => ops.push(json!(["s", Value::Array(path.clone()), value])),
                    }
                    path.pop();
                }
                for key in a.keys().filter(|key| !b.contains_key(*key)) {
                    path.push(Value::String(key.clone()));
                    ops.push(json!(["d", Value::Array(path.clone())]));
                    path.pop();
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                for (i, (old, new)) in a.iter().zip(b).enumerate() {
                    path.push(Value::from(i));
                    walk(old, new, path, ops);
                    path.pop();
                }
                if b.len() < a.len() {
                    ops.push(json!(["t", Value::Array(path.clone()), b.len()]));
                }
                for (i, value) in b.iter().enumerate().skip(a.len()) {
                    path.push(Value::from(i));
                    ops.push(json!(["s", Value::Array(path.clone()), value]));
                    path.pop();
                }
            }
            _ if !same(old, new) => ops.push(json!(["s", Value::Array(path.clone()), new])),
            _ => {}
        }
    }
    /// Equal, and bit for bit when a float: a worker must see `-0.0` become `0.0`.
    fn same(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => x == y && x.as_f64().map(f64::to_bits) == y.as_f64().map(f64::to_bits),
            _ => a == b,
        }
    }
    let mut ops = Vec::new();
    walk(old, new, &mut Vec::new(), &mut ops);
    ops
}

/// What one hot step is made of, piece by piece, on one scene stepped
/// embedded: the host's capture, diff, restore and clone, the worker's patch
/// (the host patches the same), and the bytes that cross. Then each top-level
/// field's size and how many ticks changed it.
#[test]
#[ignore]
fn delve_hot_breakdown() {
    use angel_delve::rules::runtime_state::{capture_for_step, restore_value};
    let want = std::env::var("ANGEL_BENCH_SCENES").unwrap_or_else(|_| "kingdom,fight4,boss,racers,pub".into());
    let ticks: usize = std::env::var("ANGEL_BENCH_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
    let _guard = crate::tests::env_lock();
    let _env = crate::tests::TestEnvGuard::unset("ANGEL_DELVE_SYSTEM");
    for name in want.split(',') {
        let mut run = scene(name);
        let mut parts: BTreeMap<&str, Samples> = BTreeMap::new();
        let mut held = capture_for_step(&run).unwrap().into_value();
        let mut publisher = angel_delve::wire::Publisher::default();
        let mut changed: BTreeMap<String, usize> = BTreeMap::new();
        let (mut ops_total, mut ops_bytes, mut state_bytes) = (0usize, 0usize, 0usize);
        for i in 0..ticks + 5 {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            let keys = presses(name, i);
            let inputs: BTreeMap<u32, Input> = keys.iter().enumerate().map(|(s, k)| (s as u32 + 1, *k)).collect();
            // The worker steps a run it has just restored, not the one it stepped last.
            let pre = capture_for_step(&run).unwrap().into_value();
            let mut cold = restore_value(&pre).unwrap();
            let at = Instant::now();
            cold.step(&inputs);
            let cold_step = us(at);
            let at = Instant::now();
            run.step(&inputs);
            let step = us(at);
            let at = Instant::now();
            let state = capture_for_step(&run).unwrap().into_value();
            let capture = us(at);
            let at = Instant::now();
            let old_ops = diff_every_node(&held, &state);
            let diff_old = us(at);
            let at = Instant::now();
            let ops = angel_live::delta::diff(&held, &state);
            let diff = us(at);
            // The two find the same changes, in the same order.
            assert_eq!(ops, old_ops);
            let at = Instant::now();
            let sent = serde_json::to_vec(&ops).unwrap();
            let encode = us(at);
            let at = Instant::now();
            let _: serde_json::Value = serde_json::from_slice(&sent).unwrap();
            let parse = us(at);
            let mut patched = held.clone();
            let at = Instant::now();
            angel_live::delta::patch(&mut patched, &ops).unwrap();
            let patch = us(at);
            let at = Instant::now();
            let _restored = restore_value(&patched).unwrap();
            let restore = us(at);
            let at = Instant::now();
            let _clone = patched.clone();
            let clone = us(at);
            let at = Instant::now();
            let _run_clone = run.clone();
            let run_clone = us(at);
            let at = Instant::now();
            let drawable = angel_delve::wire::drawable(&run);
            let drawable_us = us(at);
            assert!(drawable);
            let at = Instant::now();
            let _tree = publisher.publish(&run).unwrap();
            let tree = us(at);
            if i >= 5 {
                for (metric, v) in [("sim_step", step), ("cold_restored_step", cold_step), ("capture", capture), ("diff", diff), ("diff_every_node_old", diff_old), ("ops_to_json", encode), ("ops_from_json", parse), ("patch", patch), ("restore", restore), ("value_clone", clone), ("run_clone", run_clone), ("drawable", drawable_us), ("tree_publish", tree)] {
                    parts.entry(metric).or_default().add(v);
                }
                ops_total += ops.len();
                ops_bytes += sent.len();
                state_bytes = serde_json::to_vec(&state).unwrap().len();
                for op in &ops {
                    let field = op[1].get(1).and_then(|k| k.as_str()).unwrap_or("?").to_string();
                    *changed.entry(field).or_default() += 1;
                }
            }
            held = state;
        }
        for (metric, samples) in &mut parts {
            samples.say(name, "pieces", metric);
        }
        println!(
            "PIECES {name} ops_per_tick={} delta_bytes_per_tick={} state_bytes={state_bytes}",
            ops_total / ticks,
            ops_bytes / ticks
        );
        let mut sizes: Vec<(String, usize)> = held["run"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::to_vec(v).unwrap().len()))
            .collect();
        sizes.sort_by(|a, b| b.1.cmp(&a.1));
        for (field, bytes) in sizes.iter().take(14) {
            println!("FIELD {name} {field} bytes={bytes} ops_over_run={}", changed.get(field).copied().unwrap_or(0));
        }
    }
}

/// What the wish lots cost, old search against new in one process, taken in
/// turn; and the one-entry geography cache: a hit, and the price of two callers
/// that ask with different wish lists in turn.
#[test]
#[ignore]
fn delve_geography_cost() {
    use angel_delve::world::{kingdom::geography, wishes};
    use std::sync::atomic::Ordering::Relaxed;
    let _guard = crate::tests::env_lock();
    let none: Vec<angel_delve::realm::Wish> = Vec::new();
    let at = Instant::now();
    let _ = geography(&none);
    println!("GEO cold build, no wishes: {:.1} ms", us(at) / 1000.0);
    for n in [10, 40, 100] {
        let mut hashes = Vec::new();
        for round in 0..3 {
            for slow in [true, false] {
                wishes::SLOW_SEARCH.store(slow, Relaxed);
                stand_wishes_salted(n, fresh_salt());
                let list = crate::realm_standing::standing_wishes();
                let at = Instant::now();
                let lots = wishes::lots(&list);
                let took = us(at);
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                for lot in lots.iter() {
                    format!("{lot:?}").hash(&mut hash);
                }
                hashes.push(hash.finish());
                println!(
                    "GEO lots {n} wishes {} round {round}: {:.1} ms ({} placed)",
                    if slow { "old_search" } else { "new_search" },
                    took / 1000.0,
                    lots.iter().flatten().count()
                );
            }
        }
        println!("GEO lots {n} wishes: old and new place the same lots: {}", hashes.windows(2).all(|w| w[0] == w[1]));
    }
    wishes::SLOW_SEARCH.store(false, Relaxed);
    stand_wishes(40);
    let list = crate::realm_standing::standing_wishes();
    let _ = geography(&list);
    let mut hits = Samples::default();
    for _ in 0..200 {
        let at = Instant::now();
        let _ = geography(&list);
        hits.add(us(at));
    }
    hits.say("geo", "hit", "same_wishes");
    for slow in [true, false] {
        wishes::SLOW_SEARCH.store(slow, Relaxed);
        stand_wishes_salted(40, fresh_salt());
        let list = crate::realm_standing::standing_wishes();
        let mut flips = Samples::default();
        for i in 0..6 {
            let at = Instant::now();
            let _ = geography(if i % 2 == 0 { &none } else { &list });
            flips.add(us(at));
        }
        flips.say("geo", if slow { "old_search" } else { "new_search" }, "alternating_none_then_40_wishes");
    }
    wishes::SLOW_SEARCH.store(false, Relaxed);
    stand_wishes(0);
}

/// What a client does when the realm's wishes change, piece by piece: the lots
/// the 3D realm stands for them (`world3d::lots::of`, which the chunks'
/// sets are made from), then one frame drawn.
#[test]
#[ignore]
fn delve_wish_landing_cost() {
    use std::sync::atomic::Ordering::Relaxed;
    let _guard = crate::tests::env_lock();
    for slow in [true, false, true, false] {
        angel_delve::world::wishes::SLOW_SEARCH.store(slow, Relaxed);
        stand_wishes_salted(40, fresh_salt());
        let mut run = kingdom();
        run.kingdom.wishes = crate::realm_standing::standing_wishes();
        let at = Instant::now();
        let lots = angel_delve::world3d::lots::of(&run);
        println!(
            "LANDING_PARTS {} lots_of(40 wishes): {:.1} ms ({} chunk shares)",
            if slow { "old_search" } else { "new_search" },
            us(at) / 1000.0,
            lots.as_ref().map_or(0, |l| l.shares.len())
        );
        if let Some(lots) = lots {
            let (mut cut_us, mut bake_us) = (0.0, 0.0);
            for share in &lots.shares {
                let Some(chunk) = angel_delve::world3d::kingdom::chunks().chunk(share.index) else { continue };
                let at = Instant::now();
                let cut = lots.chunk(&chunk);
                cut_us += us(at);
                let at = Instant::now();
                let _ = cut.lit();
                bake_us += us(at);
            }
            println!("LANDING_PARTS   its chunks cut {:.1} ms, baked lit {:.1} ms", cut_us / 1000.0, bake_us / 1000.0);
        }
    }
    angel_delve::world::wishes::SLOW_SEARCH.store(false, Relaxed);
    stand_wishes(0);
}
