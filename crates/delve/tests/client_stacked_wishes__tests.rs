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
    EnemyKind, KNIGHT_CHILDREN, KNIGHT_SHOTS, MAX_PLAYERS, MAX_PROJECTILES, RoomKind, Weapon, cards, in_flight, phrasebook,
};
use angel_net::doc::delta::diff;
use angel_net::doc::wire::Writer;
use angel_net::proto::PROTOCOL;
use std::time::{Duration, Instant};

/// The live server's learned wishes, as its phrasebook had them.
const LEARNED: &str = include_str!("fixtures/stacked-wishes.txt");
/// Seats in the party, as the play-test had them.
const SEATS: u32 = 3;
/// The knight who holds everything.
const HEAVY: u32 = 2;

/// Bytes this thread holds, and all it has asked for (the counting
/// allocator below), so a test sees what it grew whatever runs beside it.
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    std::thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
        static ASKED: Cell<u64> = const { Cell::new(0) };
        static COUNT: Cell<u64> = const { Cell::new(0) };
    }

    pub struct Counting;

    fn note(grow: i64, asked: u64) {
        let _ = LIVE.try_with(|l| l.set(l.get() + grow));
        if asked > 0 {
            let _ = ASKED.try_with(|a| a.set(a.get() + asked));
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        }
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            note(layout.size() as i64, layout.size() as u64);
            unsafe { System.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            note(layout.size() as i64, layout.size() as u64);
            unsafe { System.alloc_zeroed(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            note(-(layout.size() as i64), 0);
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
            note(new as i64 - layout.size() as i64, new as u64);
            unsafe { System.realloc(ptr, layout, new) }
        }
    }

    /// (live bytes, bytes asked for, allocations) on this thread so far.
    pub fn now() -> (i64, u64, u64) {
        (LIVE.with(Cell::get), ASKED.with(Cell::get), COUNT.with(Cell::get))
    }
}

#[global_allocator]
static COUNTING: counting::Counting = counting::Counting;

/// The host's half for one seat: what that seat was last sent.
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

/// Which wishes the heavy knight holds: the top tier of every one in a
/// phrasebook (the live server's, `LEARNED`, or the delve's own,
/// `phrasebook::TEXT`), but those named in `skip`.
#[derive(Clone, Copy)]
struct Wishes {
    book: &'static str,
    skip: &'static str,
}

/// Everything the live server had learned.
const ALL_LEARNED: Wishes = Wishes { book: LEARNED, skip: "" };
/// The same, but the boomerang, which brings shots home and so keeps fewer
/// in flight: the heaviest of the live set.
const LEARNED_NO_BOOMERANG: Wishes = Wishes { book: LEARNED, skip: "boomerang-bullets" };
/// The delve's own ladders, every one at its top.
const BUILTIN: Wishes = Wishes { book: phrasebook::TEXT, skip: "" };

/// Knight `id` holds the top tier of every wish in `wishes`, as granted
/// (a spell in its slot), with its weapon `weapon`.
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

/// A full fight: three knights along the foot of a bare room, twelve
/// monsters across its head who never fall, the heavy knight holding
/// every wish.
fn stacked(weapon: Weapon, wishes: Wishes) -> Run {
    stacked_party(weapon, wishes, SEATS, false)
}

/// `stacked` with a party of `party`, every knight holding everything
/// when `all`.
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
    // `STACKED_OPEN=1`: on an open floor's map, as the play box ran.
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
    for n in 0..FOES {
        spawn(&mut run, n);
    }
    for id in 1..=party {
        if id == HEAVY || all {
            hold_everything(&mut run, id, weapon, wishes);
        }
    }
    run
}

/// Monsters in the room at once.
const FOES: usize = 12;
/// A monster's health: a few hits from the heavy knight, so they fall and
/// come again (drains, bursts and kills all play).
const FOE_HP: u32 = 900;

/// Where the knights stand: the room's middle, or an open map's arrival.
fn centre(run: &Run) -> (f32, f32) {
    run.dungeon.open.as_ref().filter(|_| run.open_map()).map_or((24.0, 14.0), |o| o.arrival)
}

/// Monster `n`, at its place in a ring round the knights.
fn spawn(run: &mut Run, n: usize) {
    let angle = n as f32 * std::f32::consts::TAU / FOES as f32 + (n / FOES) as f32 * 0.4;
    let (cx, cy) = centre(run);
    run.spawn_at_for_test(EnemyKind::Skeleton, cx + 15.0 * angle.cos(), cy + 9.0 * angle.sin());
    let e = run.enemies.last_mut().unwrap();
    (e.hp, e.max_hp) = (FOE_HP, FOE_HP);
}

/// Each knight fires at a monster (a new one every half second, along the
/// nearest of the eight ways), and the heavy one casts its swarms.
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

/// Keep the fight going: fallen monsters come again, the knights stand,
/// and what the tick said is taken as the host takes it. The sounds the
/// tick made, counted.
fn keep(run: &mut Run, n: &mut usize) -> usize {
    run.enemies.retain(|e| e.hp > 0);
    while run.enemies.iter().filter(|e| !e.dormant).count() < FOES {
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
    decode: Duration,
    draw: Duration,
    draw_worst: Duration,
    stage: Duration,
    shots_peak: usize,
    shots_sum: usize,
    heavy_peak: usize,
    heavy_sum: usize,
    /// Each knight's live shots, a sample a tick (0: the monsters').
    owners: BTreeMap<u32, Vec<usize>>,
    sounds_peak: usize,
    sounds_sum: usize,
    nodes_peak: usize,
    shot_nodes_peak: usize,
    wire_bytes: usize,
    wire_peak: usize,
    stage_bytes: usize,
    stage_peak: usize,
    /// Bytes asked for by the host (step, clone, publish, encode) and by
    /// the seats (decode, draw, stage), and how many asks.
    host_asked: u64,
    host_allocs: u64,
    seat_asked: u64,
    seat_allocs: u64,
    /// Seat 0's stage pictures sent, bytes by which picture, and how many
    /// shot sprites its frame drew at most.
    pictures: BTreeMap<&'static str, usize>,
    shot_looks_peak: usize,
    shot_sprites_peak: usize,
}

fn ms(d: Duration, n: u32) -> f64 {
    d.as_secs_f64() * 1000.0 / f64::from(n.max(1))
}

/// A tree's nodes, every value in it.
fn nodes(v: &Value) -> usize {
    1 + match v {
        Value::Some(v) | Value::Variant(_, v) => nodes(v),
        Value::Seq(items) => items.iter().map(nodes).sum(),
        Value::Map(pairs) => pairs.iter().map(|(k, v)| nodes(k) + nodes(v)).sum(),
        Value::Struct(fields) => fields.iter().map(|(_, v)| nodes(v)).sum(),
        _ => 0,
    }
}

/// How the bench is set: the heavy knight's arm, whether each seat draws
/// (as a browser does) and whether the host draws the room too (a host
/// watching in its own window), and rooms in 3D.
#[derive(Clone, Copy)]
struct Setup {
    weapon: Weapon,
    /// Knights in the party, and whether every one holds everything.
    party: u32,
    all: bool,
    wishes: Wishes,
    seats_draw: bool,
    host_draws: bool,
    world3d: bool,
}

/// Run `ticks` of the stacked fight: the host steps, takes what the tick
/// said, clones and publishes; each seat is sent its patch, reads it and
/// draws its stage.
fn play(setup: Setup, ticks: u32, mut each: impl FnMut(u32, &Run, i64)) -> Tally {
    crate::world3d::set_enabled(setup.world3d);
    let mut run = stacked_party(setup.weapon, setup.wishes, setup.party, setup.all);
    let seats = setup.party;
    let mut publisher = wire::Publisher::default();
    let mut sent: Vec<Sent> = (0..seats).map(|_| Sent { names: Writer::default(), last: None }).collect();
    let params = Params { world3d: setup.world3d, ..Params::default() }.encode();
    let mut clients: Vec<DelveClient> =
        (0..seats).map(|seat| DelveClient::boot(&welcome(Some(seat), &params)).unwrap()).collect();
    let mut frame = Frame::default();
    let mut t = Tally::default();
    let mut spawned = FOES;
    let mut diag = arena::stage_frame::StageMaker::for_wire();
    for tick in 1..=ticks {
        let input = inputs(&run);
        let (_, a0, c0) = counting::now();
        let at = Instant::now();
        run.step(&input);
        let took = at.elapsed();
        t.step += took;
        t.step_worst = t.step_worst.max(took);
        let sounds = keep(&mut run, &mut spawned);
        t.sounds_peak = t.sounds_peak.max(sounds);
        t.sounds_sum += sounds;
        let at = Instant::now();
        let snapshot = run.clone();
        t.clone += at.elapsed();
        let at = Instant::now();
        let tree = publisher.publish(&snapshot).unwrap();
        t.publish += at.elapsed();
        drop(snapshot);
        t.nodes_peak = t.nodes_peak.max(nodes(&tree));
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
                        s.names.patch(&mut out, &patch);
                    }
                    false
                }
            };
            s.last = Some(view);
            if tick > 1 {
                t.wire_bytes += out.len();
                t.wire_peak = t.wire_peak.max(out.len());
            }
            bodies.push((whole, out));
        }
        t.encode += at.elapsed();
        if setup.host_draws {
            let at = Instant::now();
            let img = arena::frame_for_live(&run, 384, 224, None);
            t.watch += at.elapsed();
            drop(img);
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
        each(tick, &run, live);
    }
    crate::world3d::set_enabled(false);
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
        "  host/tick ms: step {:.3} (worst {:.3}), clone {:.3}, publish {:.3}, encode x{} {:.3}, watch draw {:.3}",
        ms(t.step, n),
        ms(t.step_worst, 1),
        ms(t.clone, n),
        ms(t.publish, n),
        t.seats,
        ms(t.encode, n),
        ms(t.watch, n),
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

/// The bench: `cargo test --release -q --manifest-path crates/delve/Cargo.toml
/// stacked_wishes_bench -- --ignored --nocapture` (`STACKED_TICKS`, and
/// `STACKED_3D=1` for rooms in 3D, `STACKED_OPEN=1` on an open floor,
/// `STACKED_BOOK=builtin` for the delve's own ladders and `STACKED_SKIP` a
/// list of wishes to leave out).
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
    // `STACKED_WEAPON=Bow`: that arm only.
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
        let setup = Setup { weapon, wishes, party, all, seats_draw: true, host_draws: true, world3d };
        let t = play(setup, ticks, |tick, _, live| {
            if tick % 50 == 0 {
                lives.push((tick, live / 1024));
            }
        });
        report(&format!("{weapon:?}"), &t);
        eprintln!("  live KB on this thread by tick: {lives:?}");
    }
}

/// However many wishes one knight stacks, their shots stay inside the
/// budget: theirs in flight under `KNIGHT_SHOTS`, the room's under
/// `MAX_PROJECTILES` with the monsters still shooting, a seat's stage
/// drawing each shot look once, and nothing growing tick on tick. Counts
/// and bytes, never the clock. (Before the budget a stacked bow kept 150
/// in flight, the delve's own ladders 161.)
#[test]
fn stacked_wishes_stay_inside_their_budget() {
    for (wishes, weapon, fills) in [
        (LEARNED_NO_BOOMERANG, Weapon::Bow, true),
        (BUILTIN, Weapon::Bow, true),
        (ALL_LEARNED, Weapon::Handgonne, false),
    ] {
        let setup = Setup { weapon, wishes, party: SEATS, all: false, seats_draw: false, host_draws: false, world3d: false };
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
            // Stacked, the knight still shoots, and the party's shots keep
            // to their share of the screen (`cover`): never the room full.
            assert!(peak > 0, "{weapon:?}: stacked, the knight still shoots");
            assert!(cover_most <= crate::rules::cover::PARTY_COVER + 0.5, "{weapon:?}: {cover_most:.1}% of the screen");
        }
        assert!(monsters > 0, "the monsters still shoot");
        // The tree carries no more shots than the room holds (a shot is
        // under 64 values of it).
        assert!(t.shot_nodes_peak <= 64 * t.shots_peak, "{} values for {} shots", t.shot_nodes_peak, t.shots_peak);
        assert!(live[1] - live[0] < 8 << 20, "{weapon:?}: the host grew {} KB in 75 ticks", (live[1] - live[0]) / 1024);
    }
    // A full party of six, every knight holding everything: each inside
    // its budget, the room inside its own, the monsters still shooting.
    let setup = Setup {
        weapon: Weapon::Bow,
        wishes: BUILTIN,
        party: MAX_PLAYERS,
        all: true,
        seats_draw: false,
        host_draws: false,
        world3d: false,
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
    // A seat drawing the heaviest: each look of the shots' sheet once,
    // never more than the shots in flight.
    let setup = Setup { weapon: Weapon::Bow, wishes: BUILTIN, party: SEATS, all: false, seats_draw: true, host_draws: false, world3d: false };
    let t = play(setup, 45, |_, _, _| {});
    assert!(t.shot_sprites_peak > 0, "the shots are drawn");
    assert!(t.shot_looks_peak <= t.shot_sprites_peak && t.shot_sprites_peak <= t.shots_peak);
}
