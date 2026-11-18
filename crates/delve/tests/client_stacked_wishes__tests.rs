//! Stacked wishes (the play-test of 2026-10-18): one knight holding the top
//! tier of every wish the live server had learned (homing, bouncing twice,
//! bigger, rockets, forks, boomerangs, five at once, flame, ricochet, the
//! drains and the swarms), firing with two friends at a full room. The host
//! steps it, publishes it, and sends each of three seats its tree; each
//! seat reads it back and draws its stage, as a browser does.
//!
//! The bench (`stacked_wishes_bench`, ignored) prints where the time and the
//! memory go; the budget test holds the counts every stacked wish must stay
//! under, whatever is held.

use super::*;
use crate::rules::{
    EnemyKind, KNIGHT_SHOTS, MAX_PLAYERS, MAX_PROJECTILES, RoomKind, Trap, Weapon, cards, in_flight, phrasebook,
};
use angel_net::doc::delta::diff;
use angel_net::doc::wire::Writer;
use angel_net::proto::PROTOCOL;
use std::time::{Duration, Instant};

const LEARNED: &str = include_str!("fixtures/stacked-wishes.txt");
const SEATS: u32 = 3;
const HEAVY: u32 = 2;

mod counting {
    pub use crate::test_alloc::now;
}

struct Sent {
    names: Writer,
    last: Option<Value>,
}

fn welcome(seat: Option<u32>, params: &[u8]) -> Welcome<'_> {
    Welcome { protocol: PROTOCOL, game: wire::GAME, seat, seat_token: "tok", tick_hz: HZ, snapshot_hz: HZ, tick: 0, params }
}

fn view(c: &DelveClient, now_ms: f64) -> View<'static, DelveCmd> {
    let newest = c.newest_tick().unwrap_or(0);
    View {
        render_tick: f64::from(newest),
        newest_tick: newest,
        ack: 0,
        pending: &[],
        partial: 0.0,
        hands: angel_net::input::Hands::default(),
        seat: c.seat(),
        now_ms,
        correct_ms: angel_net::clock::CORRECT_MS,
    }
}

#[derive(Clone, Copy)]
struct Wishes {
    book: &'static str,
    skip: &'static str,
}

const ALL_LEARNED: Wishes = Wishes { book: LEARNED, skip: "" };
const LEARNED_NO_BOOMERANG: Wishes = Wishes { book: LEARNED, skip: "boomerang-bullets" };
const BUILTIN: Wishes = Wishes { book: phrasebook::TEXT, skip: "" };

fn hold_everything(run: &mut Run, id: u32, weapon: Weapon, wishes: Wishes) {
    let book = phrasebook::read(wishes.book);
    assert!(book.len() >= 15, "the book's wishes read: {}", book.len());
    for wish in book.iter().filter(|w| !wishes.skip.split(',').any(|s| s == w.id)) {
        let card = wish.card(wish.tiers.len() - 1).expect("a checked tier");
        let (card_id, kind) = (card.id.clone(), card.kind);
        assert!(run.book.insert(card), "the book has room");
        let hero = run.players.get_mut(&id).unwrap();
        hero.deck.push(card_id.clone());
        if kind == cards::Kind::Spell
            && let Some(slot) = hero.spells.iter().position(Option::is_none)
        {
            hero.spells[slot] = Some(card_id);
        }
    }
    let hero = run.players.get_mut(&id).unwrap();
    hero.weapon = weapon;
    hero.temper = 3;
    hero.rebonus(&run.book);
}

fn stacked(weapon: Weapon, wishes: Wishes) -> Run {
    stacked_party(weapon, wishes, SEATS, false)
}

fn stacked_party(weapon: Weapon, wishes: Wishes, party: u32, all: bool) -> Run {
    let mut run = Run::new(4, 1, None);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.open_room_for_test(room);
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.window = None;
    for id in 2..=party {
        run.join_seat(id, if id == HEAVY { "Heavy" } else { "Friend" });
    }
    if std::env::var("STACKED_OPEN").is_ok_and(|v| v == "1") {
        run.open_floor_for_test(3);
        run.projectiles.clear();
        run.window = None;
    }
    let (cx, cy) = centre(&run);
    for (n, (id, hero)) in run.players.iter_mut().enumerate() {
        (hero.x, hero.y) = (cx - 2.0 * (party - 1) as f32 + 4.0 * n as f32, cy);
        hero.invulnerable = 1_000_000;
        hero.weapon = if *id == 3 { Weapon::Handgonne } else { Weapon::Bow };
    }
    for n in 0..foes() {
        spawn(&mut run, n);
    }
    for id in 1..=party {
        if id == HEAVY || all {
            hold_everything(&mut run, id, weapon, wishes);
        }
    }
    run
}

const FOES: usize = 12;

fn foes() -> usize {
    static FOES_NOW: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *FOES_NOW.get_or_init(|| std::env::var("STORM_FOES").ok().and_then(|s| s.parse().ok()).unwrap_or(FOES))
}
const FOE_HP: u32 = 900;

fn centre(run: &Run) -> (f32, f32) {
    run.dungeon.open.as_ref().filter(|_| run.open_map()).map_or((24.0, 14.0), |o| o.arrival)
}

fn spawn(run: &mut Run, n: usize) {
    let angle = n as f32 * std::f32::consts::TAU / foes() as f32 + (n / foes()) as f32 * 0.4;
    let (cx, cy) = centre(run);
    run.spawn_at_for_test(EnemyKind::Skeleton, cx + 15.0 * angle.cos(), cy + 9.0 * angle.sin());
    let e = run.enemies.last_mut().unwrap();
    (e.hp, e.max_hp) = (FOE_HP, FOE_HP);
}

fn inputs(run: &Run) -> BTreeMap<u32, Input> {
    let foes: Vec<_> = run.enemies.iter().filter(|e| e.hp > 0).collect();
    run.players
        .iter()
        .map(|(&id, h)| {
            let (fx, fy) = foes
                .get((run.tick as usize / 15 + id as usize * 5) % foes.len().max(1))
                .map_or((24.0, 6.0), |e| (e.x, e.y));
            let (dx, dy) = (fx - h.x, fy - h.y);
            let d = dx.hypot(dy).max(1e-3);
            let cast = if run.players[&id].spells[0].is_some() && run.tick % 45 == 0 { 1 + (run.tick / 45 % 2) as u8 } else { 0 };
            let aim = |v: f32| (v / d).round() as i8;
            (id, Input { aim_x: aim(dx), aim_y: aim(dy), fire: true, cast, ..Input::default() })
        })
        .collect()
}

fn keep(run: &mut Run, n: &mut usize) -> usize {
    run.enemies.retain(|e| e.hp > 0);
    while run.enemies.iter().filter(|e| !e.dormant).count() < foes() {
        spawn(run, *n);
        *n += 1;
    }
    for h in run.players.values_mut() {
        h.hp = h.max_hp;
        h.invulnerable = 1_000_000;
    }
    let sounds = wire::take_sounds(run).len();
    let _ = wire::take_notices(run);
    run.cues.clear();
    run.feel.lines.clear();
    sounds
}

#[derive(Default, Debug)]
struct Tally {
    seats: u32,
    ticks: u32,
    step: Duration,
    step_worst: Duration,
    clone: Duration,
    publish: Duration,
    encode: Duration,
    watch: Duration,
    watch_worst: Duration,
    decode: Duration,
    draw: Duration,
    draw_worst: Duration,
    stage: Duration,
    shots_peak: usize,
    shots_sum: usize,
    heavy_peak: usize,
    heavy_sum: usize,
    owners: BTreeMap<u32, Vec<usize>>,
    sounds_peak: usize,
    sounds_sum: usize,
    nodes_peak: usize,
    shot_nodes_peak: usize,
    wire_bytes: usize,
    wire_peak: usize,
    stage_bytes: usize,
    stage_peak: usize,
    host_asked: u64,
    host_allocs: u64,
    seat_asked: u64,
    seat_allocs: u64,
    pictures: BTreeMap<&'static str, usize>,
    shot_looks_peak: usize,
    shot_sprites_peak: usize,
    list_edits: u32,
    list_orders: u32,
    list_named: usize,
    list_gone: usize,
    whole_bytes: usize,
}

fn ms(d: Duration, n: u32) -> f64 {
    d.as_secs_f64() * 1000.0 / f64::from(n.max(1))
}

fn nodes(v: &Value) -> usize {
    1 + match v {
        Value::Some(v) | Value::Variant(_, v) => nodes(v),
        Value::Shared(v) => nodes(v) - 1,
        Value::Seq(items) => items.iter().map(nodes).sum(),
        Value::Map(pairs) => pairs.iter().map(|(k, v)| nodes(k) + nodes(v)).sum(),
        Value::Struct(fields) => fields.iter().map(|(_, v)| nodes(v)).sum(),
        _ => 0,
    }
}

#[derive(Clone, Copy)]
struct Setup {
    weapon: Weapon,
    party: u32,
    all: bool,
    wishes: Wishes,
    seats_draw: bool,
    host_draws: bool,
    world3d: bool,
    storm: usize,
    mix: u8,
    props: bool,
}

fn play(setup: Setup, ticks: u32, mut each: impl FnMut(u32, &mut Run, i64)) -> Tally {
    crate::world3d::set_enabled(setup.world3d);
    let mut run = stacked_party(setup.weapon, setup.wishes, setup.party, setup.all);
    let seats = setup.party;
    let mut publisher = wire::Publisher::default();
    let mut sent: Vec<Sent> = (0..seats).map(|_| Sent { names: Writer::default(), last: None }).collect();
    let params = Params { world3d: setup.world3d, ..Params::default() }.encode();
    let mut clients: Vec<DelveClient> =
        (0..seats).map(|seat| DelveClient::boot(&welcome(Some(seat), &params)).unwrap()).collect();
    let mut frame = Frame::default();
    for client in &mut clients {
        angel_net::client::GameClient::stage_props(client, setup.props);
    }
    let mut t = Tally::default();
    let mut live_was: std::collections::BTreeMap<&'static str, i64> = std::collections::BTreeMap::new();
    let mut spawned = foes();
    let mut diag = arena::stage_frame::StageMaker::for_wire();
    let mut storm_rng = 0x9E37_79B9_7F4A_7C15u64;
    if setup.storm > 0 {
        crate::rules::cover::OFF.with(|off| off.set(true));
    }
    for tick in 1..=ticks {
        fn live_probe(tick: u32, phase: &'static str, was: &mut std::collections::BTreeMap<&'static str, i64>) {
            if std::env::var("STACKED_LIVE").is_ok_and(|v| v == "1") && tick % 50 == 0 {
                let live = counting::now().0;
                let was = was.entry(phase).or_insert(live);
                eprintln!("live {phase} tick {tick}: {} KB total, +{} KB in 50 ticks", live / 1024, (live - *was) / 1024);
                *was = live;
            }
        }
        let census_on = std::env::var("STACKED_CENSUS").is_ok_and(|v| v == "1");
        if census_on {
            crate::test_alloc::reset_census();
        }
        live_probe(tick, "pre", &mut live_was);
        let input = inputs(&run);
        if setup.storm > 0 {
            run.storm_for_test(setup.storm, setup.mix, &mut storm_rng);
        }
        let (_, a0, c0) = counting::now();
        if census_on {
            crate::test_alloc::census(true);
        }
        let at = Instant::now();
        run.step(&input);
        let took = at.elapsed();
        t.step += took;
        t.step_worst = t.step_worst.max(took);
        let sounds = keep(&mut run, &mut spawned);
        t.sounds_peak = t.sounds_peak.max(sounds);
        t.sounds_sum += sounds;
        live_probe(tick, "step", &mut live_was);
        if census_on {
            crate::test_alloc::census(false);
            if tick % 50 == 0 {
                let (buckets, large) = crate::test_alloc::census_report();
                for (lo, count, asked) in buckets {
                    eprintln!("  census tick {tick} bucket >= {lo:>8}  {count:>7} asks  {asked:>10} B");
                }
                for (size, count, asked) in large.iter().take(12) {
                    eprintln!("  census tick {tick} exact {size:>8}  {count:>7} asks  {asked:>10} B");
                }
            }
            crate::test_alloc::reset_census();
        }
        let at = Instant::now();
        let tree = if setup.seats_draw || setup.host_draws {
            let snapshot = run.clone();
            t.clone += at.elapsed();
            let at = Instant::now();
            let tree = publisher.publish(&snapshot).unwrap();
            t.publish += at.elapsed();
            live_probe(tick, "publish", &mut live_was);
            drop(snapshot);
            tree
        } else {
            t.clone += at.elapsed();
            let at = Instant::now();
            let tree = publisher.publish(&run).unwrap();
            t.publish += at.elapsed();
            tree
        };
        if setup.seats_draw || setup.host_draws {
            t.nodes_peak = t.nodes_peak.max(nodes(&tree));
        }
        t.shot_nodes_peak = t.shot_nodes_peak.max(tree.field("projectiles").map_or(0, nodes));
        let mut bodies = Vec::new();
        let at = Instant::now();
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
                        if let (0, angel_net::doc::delta::Patch::Struct { set, .. }) = (seat, &patch)
                            && let Some((_, list)) = set.iter().find(|(n, _)| n == "projectiles")
                        {
                            use angel_net::doc::delta::Patch;
                            match list {
                                Patch::KeyedEdit { gone, items } => {
                                    t.list_edits += 1;
                                    t.list_named += items.len();
                                    t.list_gone += gone.len();
                                }
                                Patch::Keyed { order, items } => {
                                    t.list_orders += 1;
                                    t.list_named += items.len();
                                    t.list_gone += order.len();
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
            if tick == 1 && seat == 0 {
                t.whole_bytes = out.len();
            }
            if tick > 1 {
                t.wire_bytes += out.len();
                t.wire_peak = t.wire_peak.max(out.len());
            }
            bodies.push((whole, out));
        }
        t.encode += at.elapsed();
        live_probe(tick, "encode", &mut live_was);
        if setup.host_draws {
            let at = Instant::now();
            let img = arena::frame_for_live(&run, 384, 224, None);
            let took = at.elapsed();
            t.watch += took;
            t.watch_worst = t.watch_worst.max(took);
            drop(img);
            live_probe(tick, "watch", &mut live_was);
        }
        let (_, a1, c1) = counting::now();
        t.host_asked += a1 - a0;
        t.host_allocs += c1 - c0;
        if setup.seats_draw {
            for (client, (whole, body)) in clients.iter_mut().zip(&bodies) {
                let at = Instant::now();
                client.snapshot(tick, *whole, body).expect("the snapshot reads");
                t.decode += at.elapsed();
                let now = f64::from(tick) * 1000.0 / f64::from(HZ);
                let at = Instant::now();
                client.draw(&view(client, now), &mut frame);
                let took = at.elapsed();
                t.draw += took;
                t.draw_worst = t.draw_worst.max(took);
                let at = Instant::now();
                let mut out = Vec::new();
                client.stage(now, &mut out);
                t.stage += at.elapsed();
                if tick > 2 {
                    t.stage_bytes += out.len();
                    t.stage_peak = t.stage_peak.max(out.len());
                }
            }
            if let Some(scene) = clients[0].scene.clone() {
                use arena::stage_frame::{FIGURES, GROUND, OVER, SCREEN, SHOTS, UNDER};
                let made = diag.make(&scene, |_| None);
                for p in &made.pictures {
                    let name = match p.id {
                        GROUND => "ground",
                        UNDER => "under",
                        SCREEN => "screen",
                        FIGURES => "figures",
                        OVER => "over",
                        SHOTS => "shots",
                        _ => "other",
                    };
                    if tick > 2 {
                        *t.pictures.entry(name).or_default() += p.rgba.len();
                    }
                }
                let mut looks: Vec<[u32; 4]> = made
                    .layers
                    .iter()
                    .filter_map(|l| match l.kind {
                        angel_stage::LayerKind::PicturePart(SHOTS, rect) => Some(rect.map(f32::to_bits)),
                        _ => None,
                    })
                    .collect();
                t.shot_sprites_peak = t.shot_sprites_peak.max(looks.len());
                looks.sort_unstable();
                looks.dedup();
                t.shot_looks_peak = t.shot_looks_peak.max(looks.len());
            }
        }
        let (live, a2, c2) = counting::now();
        live_probe(tick, "seats", &mut live_was);
        t.seat_asked += a2 - a1;
        t.seat_allocs += c2 - c1;
        let shots = run.projectiles.len();
        t.shots_peak = t.shots_peak.max(shots);
        t.shots_sum += shots;
        let heavy = run.projectiles.iter().filter(|p| !p.hostile && u32::from(p.traits.owner) == HEAVY).count();
        t.heavy_peak = t.heavy_peak.max(heavy);
        t.heavy_sum += heavy;
        for owner in 0..=seats {
            let n = run
                .projectiles
                .iter()
                .filter(|p| if owner == 0 { p.hostile } else { !p.hostile && u32::from(p.traits.owner) == owner })
                .count();
            t.owners.entry(owner).or_default().push(n);
        }
        t.ticks = tick;
        t.seats = seats;
        each(tick, &mut run, live);
    }
    crate::world3d::set_enabled(false);
    crate::rules::cover::OFF.with(|off| off.set(false));
    t
}

fn report(name: &str, t: &Tally) {
    let n = t.ticks;
    let seats = n * t.seats;
    eprintln!("== {name}: {n} ticks, {} seats", t.seats);
    eprintln!(
        "  shots: peak {} mean {:.0}; heavy knight's peak {} mean {:.0}; sounds/tick peak {} mean {:.1}",
        t.shots_peak,
        t.shots_sum as f64 / f64::from(n),
        t.heavy_peak,
        t.heavy_sum as f64 / f64::from(n),
        t.sounds_peak,
        t.sounds_sum as f64 / f64::from(n),
    );
    eprintln!("  tree nodes: peak {} (shots {})", t.nodes_peak, t.shot_nodes_peak);
    for (owner, samples) in &t.owners {
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
        eprintln!(
            "  live shots of {}: mean {:.1}, p50 {}, p95 {}, p99 {}, peak {}",
            if *owner == 0 { "monsters".to_string() } else { format!("knight {owner}") },
            sorted.iter().sum::<usize>() as f64 / sorted.len() as f64,
            at(0.5),
            at(0.95),
            at(0.99),
            at(1.0)
        );
    }
    eprintln!(
        "  host/tick ms: step {:.3} (worst {:.3}), clone {:.3}, publish {:.3}, encode x{} {:.3}, watch draw {:.3} (worst {:.3})",
        ms(t.step, n),
        ms(t.step_worst, 1),
        ms(t.clone, n),
        ms(t.publish, n),
        t.seats,
        ms(t.encode, n),
        ms(t.watch, n),
        ms(t.watch_worst, 1),
    );
    eprintln!(
        "  seat/frame ms: decode {:.3}, draw {:.3} (worst {:.3}), stage {:.3}",
        ms(t.decode, seats),
        ms(t.draw, seats),
        ms(t.draw_worst, 1),
        ms(t.stage, seats)
    );
    eprintln!(
        "  wire bytes/seat/tick: mean {:.0}, peak {}; stage bytes/frame: mean {:.0}, peak {}",
        t.wire_bytes as f64 / f64::from(seats),
        t.wire_peak,
        t.stage_bytes as f64 / f64::from(seats.max(1)),
        t.stage_peak
    );
    let per: Vec<String> =
        t.pictures.iter().map(|(k, v)| format!("{k} {:.0} KB", *v as f64 / 1024.0 / f64::from(n))).collect();
    eprintln!(
        "  seat 0 pictures/frame: {}; shot sprites drawn peak {}, of looks {}",
        per.join(", "),
        t.shot_sprites_peak,
        t.shot_looks_peak
    );
    eprintln!(
        "  allocated/tick: host {:.0} KB in {:.0} asks; seats {:.0} KB in {:.0} asks",
        t.host_asked as f64 / 1024.0 / f64::from(n),
        t.host_allocs as f64 / f64::from(n),
        t.seat_asked as f64 / 1024.0 / f64::from(n),
        t.seat_allocs as f64 / f64::from(n)
    );
}

#[test]
#[ignore = "a bench: run it in release with --nocapture"]
fn stacked_wishes_flip_bench() {
    let ticks = std::env::var("STACKED_TICKS").ok().and_then(|s| s.parse().ok()).unwrap_or(300);
    let every = std::env::var("STACKED_EVERY").ok().and_then(|s| s.parse().ok()).unwrap_or(30);
    let wishes = Wishes { book: LEARNED, skip: "" };
    let setup = Setup { weapon: Weapon::Bow, wishes, party: SEATS, all: false, seats_draw: true, host_draws: true, world3d: false, storm: 0, mix: 0, props: false };
    let mut flips = 0u32;
    let t = play(setup, ticks, |tick, run, _| {
        if tick % every == 0 {
            run.wishing = !run.wishing;
            if run.wishing {
                run.traps.push(Trap::Spikes { col: 4, row: 4, phase: 0 });
            } else if let Some(i) = run.traps.iter().rposition(|t| matches!(t, Trap::Spikes { .. })) {
                run.traps.remove(i);
            }
            flips += 1;
        }
    });
    report("flip", &t);
    eprintln!("  steady-key flips: {flips} (every {every} ticks)");
}

#[test]
#[ignore = "a bench: run it in release with --nocapture"]
fn stacked_wishes_rss_is_retention_not_live() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    fn rss_kb() -> i64 {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1).and_then(|n| n.parse::<i64>().ok()))
            })
            .unwrap_or(-1)
    }
    let ticks = std::env::var("STACKED_TICKS").ok().and_then(|s| s.parse().ok()).unwrap_or(1000);
    let wishes = Wishes { book: LEARNED, skip: "" };
    let setup = Setup { weapon: Weapon::Bow, wishes, party: SEATS, all: false, seats_draw: true, host_draws: true, world3d: false, storm: 0, mix: 0, props: false };
    let (mut first, mut last) = (0i64, 0i64);
    let t = play(setup, ticks, |tick, _, live| {
        if tick == 1 || tick % 500 == 0 {
            let rss = rss_kb();
            if tick == 1 {
                first = rss;
            }
            last = rss;
            eprintln!("  during fight: tick {tick}, live {} KB, rss {rss} KB", live / 1024);
        }
    });
    report("retention", &t);
    drop(t);
    let before = rss_kb();
    let returned = unsafe { malloc_trim(0) };
    std::thread::sleep(std::time::Duration::from_millis(250));
    let settled = rss_kb();
    let climb = (last - first).max(0);
    let held = (settled - first).max(0);
    eprintln!(
        "  fight climb: {climb} KB over {ticks} ticks; after drop rss {before} KB, malloc_trim -> {returned}, settled {settled} KB; still held: {held} KB",
    );
}

#[test]
#[ignore = "a bench: run it in release with --nocapture"]
fn stacked_wishes_bench() {
    let ticks = std::env::var("STACKED_TICKS").ok().and_then(|s| s.parse().ok()).unwrap_or(300);
    let world3d = std::env::var("STACKED_3D").is_ok_and(|v| v == "1");
    let book = if std::env::var("STACKED_BOOK").is_ok_and(|v| v == "builtin") { phrasebook::TEXT } else { LEARNED };
    let skip: &'static str = std::env::var("STACKED_SKIP").unwrap_or_default().leak();
    let wishes = Wishes { book, skip };
    let party = std::env::var("STACKED_PARTY").ok().and_then(|s| s.parse().ok()).unwrap_or(SEATS);
    let all = std::env::var("STACKED_ALL").is_ok_and(|v| v == "1");
    let only = std::env::var("STACKED_WEAPON").unwrap_or_default();
    for weapon in [Weapon::Bow, Weapon::Crossbow, Weapon::Handgonne, Weapon::Notes, Weapon::Stones]
        .into_iter()
        .filter(|w| only.is_empty() || format!("{w:?}") == only)
    {
        let run = stacked(weapon, wishes);
        let b = &run.players[&HEAVY].bonus;
        eprintln!(
            "heavy knight ({weapon:?}): shots {} volley {} bounce {} homing {} fork {} burst {} size {} pierce {} boomerang {} velocity {} burn {} knock {} vamp {} rate {}",
            b.shots, b.volley, b.bounce, b.homing, b.fork, b.burst, b.size, b.pierce, b.boomerang, b.velocity, b.burn, b.knock, b.vamp, b.rate
        );
        let mut lives = Vec::new();
        let setup = Setup { weapon, wishes, party, all, seats_draw: true, host_draws: true, world3d, storm: 0, mix: 0, props: false };
        let t = play(setup, ticks, |tick, _, live| {
            if tick % 50 == 0 {
                lives.push((tick, live / 1024));
            }
        });
        report(&format!("{weapon:?}"), &t);
        eprintln!("  live KB on this thread by tick: {lives:?}");
    }
}

#[test]
fn stacked_wishes_stay_inside_their_budget() {
    for (wishes, weapon, fills) in [
        (LEARNED_NO_BOOMERANG, Weapon::Bow, true),
        (BUILTIN, Weapon::Bow, true),
        (ALL_LEARNED, Weapon::Handgonne, false),
    ] {
        let setup = Setup { weapon, wishes, party: SEATS, all: false, seats_draw: false, host_draws: false, world3d: false, storm: 0, mix: 0, props: false };
        let (mut peak, mut monsters) = (0, 0);
        let mut cover_most = 0.0f32;
        let mut live = Vec::new();
        let t = play(setup, 150, |tick, run, bytes| {
            let view = crate::rules::cover::view(run.room());
            cover_most = cover_most.max(crate::rules::cover::covered(&run.projectiles, view));
            let mine = in_flight(&run.projectiles, HEAVY);
            assert!(mine <= KNIGHT_SHOTS, "tick {tick}: {mine} in flight");
            assert!(run.projectiles.len() <= MAX_PROJECTILES);
            peak = peak.max(mine);
            monsters = monsters.max(run.projectiles.iter().filter(|p| p.hostile).count());
            if tick == 75 || tick == 150 {
                live.push(bytes);
            }
        });
        if fills {
            assert!(peak > 0, "{weapon:?}: stacked, the knight still shoots");
            if crate::rules::cover::PARTY_COVER_ON {
                assert!(cover_most <= crate::rules::cover::PARTY_COVER + 0.5, "{weapon:?}: {cover_most:.1}% of the screen");
            }
        }
        assert!(monsters > 0, "the monsters still shoot");
        assert!(t.shot_nodes_peak <= 64 * t.shots_peak, "{} values for {} shots", t.shot_nodes_peak, t.shots_peak);
        assert!(live[1] - live[0] < 8 << 20, "{weapon:?}: the host grew {} KB in 75 ticks", (live[1] - live[0]) / 1024);
    }
    let setup = Setup {
        weapon: Weapon::Bow,
        wishes: BUILTIN,
        party: MAX_PLAYERS,
        all: true,
        seats_draw: false,
        host_draws: false,
        world3d: false,
        storm: 0,
        mix: 0,
        props: false,
    };
    let mut monsters = 0;
    play(setup, 90, |tick, run, _| {
        for id in 1..=MAX_PLAYERS {
            let mine = in_flight(&run.projectiles, id);
            assert!(mine <= KNIGHT_SHOTS, "tick {tick}: knight {id} has {mine} in flight");
        }
        assert!(run.projectiles.len() <= MAX_PROJECTILES);
        if tick > 45 {
            monsters = monsters.max(run.projectiles.iter().filter(|p| p.hostile).count());
        }
    });
    assert!(monsters > 0, "six stacked knights leave the monsters room to shoot");
    let setup = Setup { weapon: Weapon::Bow, wishes: BUILTIN, party: SEATS, all: false, seats_draw: true, host_draws: false, world3d: false, storm: 0, mix: 0, props: false };
    let t = play(setup, 45, |_, _, _| {});
    assert!(t.shot_sprites_peak > 0, "the shots are drawn");
    assert!(t.shot_looks_peak <= t.shot_sprites_peak && t.shot_sprites_peak <= t.shots_peak);
}

fn bullet_party(arms: [(Weapon, u32); 4]) -> Run {
    let mut run = stacked_party(Weapon::Bow, LEARNED_NO_BOOMERANG, 4, false);
    for (n, (id, hero)) in run.players.iter_mut().enumerate() {
        hero.weapon = arms[n].0;
        if *id != HEAVY {
            hero.bonus.shots = 2;
            hero.bonus.bounce = 2;
            hero.bonus.rate = arms[n].1;
        }
    }
    run
}

#[test]
#[ignore]
fn write_bullet_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::world3d::set_enabled(false);
    let save = |run: &Run, name: String| {
        let img = arena::frame_for(run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
        let rgba = img.rgba_scaled(3);
        image::RgbaImage::from_raw(img.w as u32 * 3, img.h as u32 * 3, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let scenes: [(&str, [(Weapon, u32); 4]); 3] = [
        ("heavy", [(Weapon::Crossbow, 0), (Weapon::Handgonne, 0), (Weapon::Crossbow, 0), (Weapon::Handgonne, 0)]),
        ("fast", [(Weapon::Bow, 100), (Weapon::Bow, 100), (Weapon::Bow, 150), (Weapon::Bow, 100)]),
        ("mixed", [(Weapon::Bow, 60), (Weapon::Crossbow, 0), (Weapon::Handgonne, 0), (Weapon::Notes, 0)]),
    ];
    for (name, arms) in scenes {
        for (mode, legacy) in [("before", true), ("after", false)] {
            for original in [false, true] {
                crate::rules::wishcraft::LEGACY.with(|l| l.set(legacy));
                arena::slot_art::use_original(original);
                let tag = format!("{name}-{mode}{}", if original { "-orig" } else { "" });
                let mut run = bullet_party(arms);
                let mut n = foes();
                let (mut peak, mut peak_at) = (0usize, 0u32);
                let mut frames = Vec::new();
                for tick in 1..=240u32 {
                    let input = inputs(&run);
                    run.step(&input);
                    keep(&mut run, &mut n);
                    let aloft = run.projectiles.iter().filter(|p| !p.hostile).count();
                    if aloft > peak {
                        (peak, peak_at) = (aloft, tick);
                    }
                    if tick == 90 {
                        save(&run, format!("{tag}-090"));
                    }
                    frames.push((tick, aloft));
                    if aloft == peak && tick > 60 {
                        save(&run, format!("{tag}-peak"));
                    }
                }
                let mean = frames.iter().skip(60).map(|f| f.1).sum::<usize>() as f32 / 180.0;
                eprintln!("{tag}: knight shots aloft, peak {peak} at tick {peak_at}, mean {mean:.1}");
            }
        }
    }
    arena::slot_art::use_original(false);
    crate::rules::wishcraft::LEGACY.with(|l| l.set(false));
}

#[path = "client_bullet_storm__tests.rs"]
mod bullet_storm;
