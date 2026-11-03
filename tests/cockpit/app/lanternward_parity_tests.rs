//! Lanternward's tick, Rust against Bend. The oracle is the rules as they
//! stood in `crates/delve/src/rules/lanternward/` before they moved
//! (`lanternward_oracle`); the Bend leg is the shipped `lanternward-rules`
//! generation, reached through `angel_delve::scripted::step` as a game tick
//! reaches it. Scenarios are seeded and driven from the oracle's session, so
//! both are stepped on the same inputs and compared whole, every tick.
use crate::lanternward_oracle::drive::{self, Driver, Scenario, Start};
use angel_delve::rules::lanternward::{Rules, Session};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What the oracle did, as the corpus the Node harness replays.
fn dump(sc: &mut Scenario, ticks: u32, dir: &std::path::Path) -> usize {
    let mut file = std::io::BufWriter::new(std::fs::File::create(dir.join(format!("{}.jsonl", sc.name))).unwrap());
    let start = drive::wire(&sc.session);
    writeln!(file, "{}", json!({"name": sc.name, "start": start})).unwrap();
    let mut n = 0;
    let mut last = start;
    drive::play(sc, ticks, |sc, t| {
        n += 1;
        let after = drive::wire(&t.after);
        let mut delta = serde_json::Map::new();
        for (k, v) in after.as_object().unwrap() {
            let was = if t.reset.is_some() { t.reset.as_ref().unwrap().get(k) } else { last.get(k) };
            if was != Some(v) {
                delta.insert(k.clone(), v.clone());
            }
        }
        let mut row = json!({
            "t": sc.tick,
            "ins": drive::ask(&t)["here"],
            "rules": drive::ask(&t)["rules"],
            "d": delta,
            "sounds": t.sounds.iter().map(|s| drive::SOUNDS.iter().position(|n| n == s).unwrap()).collect::<Vec<_>>(),
            "stop": t.stop.map(|s| format!("{s:?}")),
            "won": t.won,
        });
        if let Some(reset) = &t.reset {
            row["reset"] = reset.clone();
        }
        if sc.tick % 25 == 0 {
            let mut phases = serde_json::Map::new();
            for (name, s, sounds, stop) in &t.trace {
                phases.insert(
                    name.to_string(),
                    json!({"session": drive::wire(s), "sounds": sounds.iter().map(|s| drive::SOUNDS.iter().position(|n| n == s).unwrap()).collect::<Vec<_>>(), "stop": stop.map(|s| format!("{s:?}"))}),
                );
            }
            row["ph"] = Value::Object(phases);
            row["before"] = drive::wire(&t.before);
        }
        writeln!(file, "{row}").unwrap();
        last = after;
    });
    n
}

/// The scenarios the corpus holds: every room, one to four knights, the bot
/// and random keys, the table churning and the session thrown about.
fn matrix(scale: u32) -> Vec<(u64, u32, Start, Driver, bool, u32)> {
    let mut out = Vec::new();
    for (i, players) in [1u32, 2, 3, 4].into_iter().enumerate() {
        out.push((100 + i as u64, players, Start::Fresh, Driver::Bots, false, 9000 * scale));
    }
    for room in 0..5usize {
        for (j, players) in [1u32, 2, 4].into_iter().enumerate() {
            out.push((200 + room as u64 * 10 + j as u64, players, Start::Room(room), Driver::Bots, false, 2500 * scale));
        }
    }
    for k in 0..20u64 {
        let players = 1 + (k % 4) as u32;
        out.push((300 + k, players, Start::Random, Driver::Fuzz, k % 2 == 0, 2500 * scale));
        out.push((400 + k, players, Start::Random, Driver::Noisy, true, 2500 * scale));
    }
    out
}

#[test]
#[ignore = "writes the Node harness's corpus: LANTERNWARD_CORPUS=dir cargo test ... write_lanternward_corpus -- --ignored"]
fn write_lanternward_corpus() {
    let dir = std::env::var_os("LANTERNWARD_CORPUS").map_or_else(|| std::env::temp_dir().join("lw/corpus"), std::path::PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    let only: Option<String> = std::env::var("LANTERNWARD_ONLY").ok();
    let mut total = 0;
    for (seed, players, start, driver, chaos, ticks) in matrix(1) {
        let mut sc = Scenario::new(seed, players, start, driver, chaos);
        if only.as_ref().is_some_and(|o| !sc.name.contains(o.as_str())) {
            continue;
        }
        total += dump(&mut sc, ticks, &dir);
    }
    eprintln!("lanternward corpus: {total} ticks in {}", dir.display());
}

/// The crate's session from the oracle's, through the shape both save as.
fn mine(oracle: &crate::lanternward_oracle::Session) -> Session {
    serde_json::from_value(serde_json::to_value(oracle).unwrap()).unwrap()
}

/// What the matrix has seen, so that "every branch" is a number and not a hope.
#[derive(Default)]
struct Seen {
    ticks: u64,
    sounds: BTreeSet<&'static str>,
    stops: BTreeSet<String>,
    wins: u32,
    wipes: u32,
    exits: u32,
    resets: u32,
    rooms: BTreeSet<u8>,
}

/// The first place two sessions differ, by path.
fn first_difference(a: &Value, b: &Value, path: &mut Vec<String>) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys()).collect::<BTreeSet<_>>() {
                path.push(key.clone());
                let found = match (x.get(key), y.get(key)) {
                    (Some(p), Some(q)) => first_difference(p, q, path),
                    (p, q) => Some(format!("{}: {p:?} against {q:?}", path.join("."))),
                };
                path.pop();
                if found.is_some() {
                    return found;
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                path.push(i.to_string());
                let found = first_difference(p, q, path);
                path.pop();
                if found.is_some() {
                    return found;
                }
            }
            None
        }
        _ if a == b => None,
        _ => Some(format!("{}: {a} against {b}", path.join("."))),
    }
}

/// One scenario on the oracle and on the shipped rules side by side, through
/// the adapter the game runs on, compared whole after every tick.
fn lockstep(mut sc: Scenario, ticks: u32, seen: &Mutex<Seen>) {
    let mut mine_now = mine(&sc.session);
    let name = sc.name.clone();
    let mut local = Seen::default();
    drive::play(&mut sc, ticks, |sc, t| {
        if t.reset.is_some() {
            local.resets += 1;
            mine_now = mine(&t.before);
        }
        let rules = Rules { lock: t.rules.lock, parry: t.rules.parry, combo: t.rules.combo, flame: t.rules.flame, tell: t.rules.tell };
        let events = mine_now.step(&t.inputs, &sc.heroes, &rules);
        let (want, got) = (serde_json::to_value(&t.after).unwrap(), serde_json::to_value(&mine_now).unwrap());
        if let Some(at) = first_difference(&want, &got, &mut Vec::new()) {
            panic!("{name} tick {}: the sessions part at {at}", sc.tick);
        }
        assert_eq!(events.sounds, t.sounds, "{name} tick {}: the sounds", sc.tick);
        assert_eq!(format!("{:?}", events.stop), format!("{:?}", t.stop), "{name} tick {}: the impact", sc.tick);
        assert_eq!(events.won, t.won, "{name} tick {}: the win", sc.tick);
        local.ticks += 1;
        local.sounds.extend(events.sounds.iter().copied());
        if let Some(stop) = events.stop {
            local.stops.insert(format!("{stop:?}"));
        }
        local.wins += u32::from(events.won);
        local.wipes += u32::from(t.after.wipe > 0 && t.before.wipe == 0);
        local.exits += u32::from(t.after.exiting > 0 && t.before.exiting == 0);
        local.rooms.insert(t.after.room);
    });
    let mut all = seen.lock().unwrap();
    all.ticks += local.ticks;
    all.sounds.extend(local.sounds);
    all.stops.extend(local.stops);
    all.wins += local.wins;
    all.wipes += local.wipes;
    all.exits += local.exits;
    all.resets += local.resets;
    all.rooms.extend(local.rooms);
}

/// The matrix, an edge or two besides: the clock at its limit, keys the wire
/// may not carry.
fn scenarios() -> Vec<(Scenario, u32)> {
    crate::agent::harness::live_policy::install_scripted_rules();
    let mut out: Vec<(Scenario, u32)> =
        matrix(1).into_iter().map(|(seed, players, start, driver, chaos, ticks)| (Scenario::new(seed, players, start, driver, chaos), ticks)).collect();
    for (k, players) in [1u32, 2, 4].into_iter().enumerate() {
        let mut sc = Scenario::new(500 + k as u64, players, Start::Fresh, Driver::Bots, false);
        sc.name += "-timeout";
        sc.session.elapsed = crate::lanternward_oracle::LIMIT - 40;
        sc.session.stage_t = crate::lanternward_oracle::LIMIT - 20;
        out.push((sc, 120));
    }
    for k in 0..6u64 {
        let players = 1 + (k % 4) as u32;
        out.push((Scenario::new(600 + k, players, if k < 3 { Start::Fresh } else { Start::Random }, Driver::Edge, k % 2 == 0), 2500));
    }
    out
}

#[test]
fn the_bend_rules_match_the_rust_rules_tick_by_tick_in_every_scenario() {
    let work = Mutex::new(scenarios().into_iter().map(Some).collect::<Vec<_>>());
    let next = AtomicUsize::new(0);
    let total = work.lock().unwrap().len();
    let seen = Mutex::new(Seen::default());
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        if i >= total {
                            break;
                        }
                        let (sc, ticks) = work.lock().unwrap()[i].take().unwrap();
                        lockstep(sc, ticks, &seen);
                    }
                })
            })
            .collect();
        for worker in workers {
            if let Err(panic) = worker.join() {
                std::panic::resume_unwind(panic);
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    eprintln!(
        "lanternward parity: {} ticks in {total} scenarios; {} sounds, impacts {:?}, {} wins, {} wipes, {} exits, {} hands on the session, rooms {:?}",
        seen.ticks,
        seen.sounds.len(),
        seen.stops,
        seen.wins,
        seen.wipes,
        seen.exits,
        seen.resets,
        seen.rooms
    );
    // The branches the matrix is there to reach.
    assert!(seen.wins >= 4, "the fresh games reach the Beacon: {}", seen.wins);
    assert!(seen.wipes >= 1, "a party is wiped and put back");
    assert!(seen.exits >= 1, "a room is left by its door");
    assert_eq!(seen.stops.len(), 4, "every weight of impact: {:?}", seen.stops);
    assert_eq!(seen.rooms.len(), 5, "every room is played");
    assert!(seen.sounds.len() >= 36, "the sounds the game makes: {:?}", seen.sounds);
}

#[test]
fn dead_warden_clock_stays_valid_after_the_drop() {
    crate::agent::harness::live_policy::install_scripted_rules();
    let mut sc = Scenario::new(42, 1, Start::Room(4), Driver::Bots, false);
    let boss = sc.session.boss.as_mut().expect("warden room");
    boss.st = crate::lanternward_oracle::boss::Ws::Dying;
    boss.hp = 0;
    boss.dead = 39;
    let mut session = mine(&sc.session);
    let rules = Rules::default();
    session.step(&Default::default(), &sc.heroes, &rules);
    assert_eq!(session.boss.as_ref().unwrap().dead, 40);
    assert_eq!(session.stage, 2);
    let drops = session.drops.len();
    session.boss.as_mut().unwrap().dead = 600;
    for _ in 0..3 {
        let elapsed = session.elapsed;
        session.step(&Default::default(), &sc.heroes, &rules);
        assert_eq!(session.boss.as_ref().unwrap().dead, 600);
        assert_eq!(session.drops.len(), drops);
        assert_eq!(session.problem(), None);
        assert_eq!(session.elapsed, elapsed + 1);
    }
}
