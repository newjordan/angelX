//! The Delve replay corpus: golden exchanges with the rules worker, recorded
//! over scripted and bot-driven games, for `scripts/delve/replay.mjs` to drive
//! any worker binary through (docs/architecture/delve-in-bend.md, "Parity").
//!
//! ```text
//! DELVE_REPLAY_OUT=<dir> \
//!   cargo test --manifest-path crates/Cargo.toml -p angel-delve-system --profile fast \
//!   --test record_corpus -- --ignored --nocapture record_corpus
//! ```
//!
//! `DELVE_REPLAY_ONLY=name,name` records only those scenarios (the manifest is
//! then `MANIFEST.partial.json`). Each scenario is a file `<name>.jsonl.gz`:
//!
//! - line 1: `{"kind":"restore","state":<checkpoint>}`
//! - then one line a tick: `{"kind":"apply","before":<checkpoint|null>,
//!   "event":<event>,"after":<checkpoint>}`
//!
//! A checkpoint is `runtime_state::capture_for_rules(&run).into_value()` as the
//! host sends it (`after`: `capture(&run).into_value()`, as the worker replies).
//! `before` is `null` when it is byte for byte the previous line's `after` (the
//! restore state, for the first apply). The event's `phrasebook` is written
//! only when it changes (the host sends the same book every tick); a reader
//! carries the last one forward.
//!
//! The worker side of every tick is the worker's own code (`angel_delve_system::
//! apply`): restore the checkpoint, take settings and phrasebook, step, capture.
//! Between ticks the host side continues from `after` (restored), drains the
//! queues the host drains (cues, sounds, notices, feel lines) and applies the
//! scenario's host-side moves (seats joining and leaving, presence, tune), so
//! the chain is what a host would send, deltas included.
use angel_delve::rules::{
    self, Hero, Input, Phase, Run, TELEGRAPH, cards,
    layout::{RoomKind, TILE_UNITS, Tile},
    phrasebook, runtime_state, ults,
};
use angel_delve::settings::{self, Settings};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type Keys = BTreeMap<u32, Input>;

// ---------------------------------------------------------------- randomness

/// xorshift64*: the scenarios' own deterministic randomness.
#[derive(Clone)]
struct Xs(u64);
impl Xs {
    fn new(seed: u64) -> Xs {
        Xs(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn pct(&mut self, p: u64) -> bool {
        self.below(100) < p
    }
    fn axis(&mut self) -> i8 {
        self.below(3) as i8 - 1
    }
}

// ------------------------------------------------------------------- drivers

/// How a monkey presses keys: percent chances a tick.
#[derive(Clone, Copy)]
struct Style {
    fire: u64,
    interact: u64,
    dash: u64,
    swing: u64,
    cards: u64,
    spells: u64,
    extras: u64,
}
const WILD: Style = Style {
    fire: 55,
    interact: 12,
    dash: 4,
    swing: 6,
    cards: 4,
    spells: 5,
    extras: 2,
};
const CALM: Style = Style {
    fire: 10,
    interact: 8,
    dash: 1,
    swing: 2,
    cards: 1,
    spells: 1,
    extras: 0,
};
const PRESS: Style = Style {
    fire: 8,
    interact: 30,
    dash: 2,
    swing: 1,
    cards: 0,
    spells: 0,
    extras: 0,
};

/// Random but sticky keys: a walk, now and then a press.
struct Monkey {
    rng: Xs,
    walk: BTreeMap<u32, (i8, i8, u32)>,
    aim: BTreeMap<u32, (i8, i8, u32)>,
}
impl Monkey {
    fn new(seed: u64) -> Monkey {
        Monkey {
            rng: Xs::new(seed),
            walk: BTreeMap::new(),
            aim: BTreeMap::new(),
        }
    }
    fn keys(&mut self, ids: &[u32], style: Style) -> Keys {
        let mut out = Keys::new();
        for &id in ids {
            let rng = &mut self.rng;
            let walk = self.walk.entry(id).or_insert((0, 0, 0));
            if walk.2 == 0 {
                *walk = (rng.axis(), rng.axis(), 8 + rng.below(40) as u32);
            }
            walk.2 -= 1;
            let aim = self.aim.entry(id).or_insert((1, 0, 0));
            if aim.2 == 0 {
                let (mut ax, ay) = (rng.axis(), rng.axis());
                if ax == 0 && ay == 0 {
                    ax = 1;
                }
                *aim = (ax, ay, 6 + rng.below(30) as u32);
            }
            aim.2 -= 1;
            let mut input = Input {
                move_x: walk.0,
                move_y: walk.1,
                aim_x: aim.0,
                aim_y: aim.1,
                ..Input::default()
            };
            input.fire = rng.pct(style.fire);
            input.interact = rng.pct(style.interact);
            input.dash = rng.pct(style.dash);
            input.swing = rng.pct(style.swing);
            if rng.pct(style.cards) {
                input.play = 1 + rng.below(cards::HAND as u64) as u8;
            }
            if rng.pct(style.spells) {
                input.cast = 1 + rng.below(cards::SPELL_SLOTS as u64) as u8;
            }
            if rng.pct(style.extras) {
                match rng.below(9) {
                    0 => input.ult = true,
                    1 => input.bomb = true,
                    2 => input.take = true,
                    3 => input.cycle = true,
                    4 => input.ride = true,
                    5 => input.vigil = true,
                    6 => input.skill = 1 + rng.below(rules::skills::SKILLS as u64) as u8,
                    7 => input.leave_held = true,
                    _ => input.skip_plate = true,
                }
            }
            out.insert(id, input);
        }
        out
    }
}

fn sign(v: f32, dead: f32) -> i8 {
    if v > dead {
        1
    } else if v < -dead {
        -1
    } else {
        0
    }
}

fn dir8(dx: f32, dy: f32) -> (i8, i8) {
    let k = ((dy.atan2(dx) / (std::f32::consts::PI / 4.0)).round() as i32).rem_euclid(8);
    [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ][k as usize]
}

/// A patient fighter: kites to a distance, aims, dodges shots coming at it,
/// plays what its hand and spells offer. None: nothing to fight.
fn fighter(run: &Run, id: u32) -> Option<Input> {
    let hero = &run.players[&id];
    if hero.hp == 0 || hero.stone {
        return None;
    }
    let foes: Vec<_> = run
        .enemies
        .iter()
        .filter(|e| {
            e.hp > 0 && e.kind != rules::EnemyKind::Dummy && e.age >= TELEGRAPH && !e.dormant
        })
        .collect();
    let dist = |x: f32, y: f32| (x - hero.x).hypot(y - hero.y);
    let target = foes
        .iter()
        .copied()
        .min_by(|a, b| dist(a.x, a.y).total_cmp(&dist(b.x, b.y)))?;
    let melee = hero
        .forged
        .as_ref()
        .is_some_and(|w| w.melee.is_some() && w.bolt.is_none() && w.throw.is_none());
    let want = if melee { 1.8 } else { 7.5 };
    let (w, h) = (run.room().width(), run.room().height());
    let mut input = Input::default();
    let d = dist(target.x, target.y).max(0.01);
    let (ux, uy) = ((target.x - hero.x) / d, (target.y - hero.y) / d);
    let (mut vx, mut vy) = if d > want + 1.0 {
        (ux, uy)
    } else if d < want - 1.5 {
        (-ux, -uy)
    } else {
        let side = if (run.tick / 50 + u64::from(id)).is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        (-uy * side * 0.6, ux * side * 0.6)
    };
    let (ax, ay) = dir8(ux, uy);
    (input.aim_x, input.aim_y, input.fire) = (ax, ay, true);
    input.swing = d < 2.4;
    let mut threatened = 0.0f32;
    for shot in run.projectiles.iter().filter(|p| p.hostile && p.ttl > 0) {
        let (dx, dy) = (hero.x - shot.x, hero.y - shot.y);
        let dd = dx.hypot(dy);
        let speed = shot.vx.hypot(shot.vy).max(0.01);
        let closing = (shot.vx * dx + shot.vy * dy) / (speed * dd.max(0.01));
        if dd < 4.5 && closing > 0.6 {
            threatened = threatened.max(1.0 / dd.max(0.5));
            let (px, py) = (-shot.vy / speed, shot.vx / speed);
            let side = if px * dx + py * dy >= 0.0 { 1.0 } else { -1.0 };
            vx += px * side * 1.4;
            vy += py * side * 1.4;
        }
    }
    if hero.x < 5.0 {
        vx += 0.8;
    }
    if hero.x > w - 5.0 {
        vx -= 0.8;
    }
    if hero.y < 5.0 {
        vy += 0.8;
    }
    if hero.y > h - 5.0 {
        vy -= 0.8;
    }
    input.move_x = sign(vx, 0.35);
    input.move_y = sign(vy, 0.35);
    input.dash = match hero.guard {
        cards::Guard::Roll => threatened > 0.45 && hero.dash_cooldown == 0,
        cards::Guard::Shield => threatened > 0.3,
        cards::Guard::Spin => foes.iter().any(|e| dist(e.x, e.y) < 3.2),
        cards::Guard::Wall(_) => threatened > 0.3,
    };
    if hero.guard == cards::Guard::Shield && input.dash {
        input.fire = false;
    }
    let share = |h: &Hero| h.hp as f32 / h.max_hp.max(1) as f32;
    let party_low = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .any(|h| share(h) < 0.5);
    for (slot, card) in hero.hand.iter().enumerate() {
        let Some(card) = run.book.get(card) else {
            continue;
        };
        let has = |f: fn(&cards::Effect) -> bool| card.effects.iter().any(f);
        let want = (has(|e| matches!(e, cards::Effect::Heal(_))) && share(hero) < 0.5)
            || (has(|e| matches!(e, cards::Effect::Ward(_)))
                && share(hero) < 0.4
                && threatened > 0.0)
            || (has(|e| matches!(e, cards::Effect::Censer(_))) && party_low)
            || (has(|e| matches!(e, cards::Effect::Nova(_))) && foes.len() >= 3);
        if want {
            input.play = slot as u8 + 1;
            break;
        }
    }
    if hero.bombs > 0 && foes.iter().filter(|e| dist(e.x, e.y) < 5.0).count() >= 3 {
        input.bomb = true;
    }
    if let Some(slot) =
        (0..cards::SPELL_SLOTS).find(|&s| hero.spells[s].is_some() && hero.spell_cooldowns[s] == 0)
    {
        input.cast = slot as u8 + 1;
    }
    input.ult = hero.ult_charge >= ults::ULT_FULL;
    Some(input)
}

/// The tile grid of the party's room, walked.
struct Grid<'a> {
    room: &'a rules::layout::Room,
    /// Tiles a knight was pinned against: something the tiles do not show.
    blocked: &'a BTreeSet<(i32, i32)>,
}
impl Grid<'_> {
    fn open(&self, c: i32, r: i32) -> bool {
        !self.blocked.contains(&(c, r))
            && matches!(
                self.room.tile(c, r),
                Tile::Floor | Tile::Door | Tile::Ledge | Tile::Bog | Tile::Stairs
            )
    }
    /// The next tile on the shortest open path from `from` to `to`.
    fn step(&self, from: (i32, i32), to: (i32, i32)) -> Option<(i32, i32)> {
        let (cols, rows) = (self.room.cols as i32, self.room.rows as i32);
        if from == to || from.0 < 0 || from.1 < 0 || from.0 >= cols || from.1 >= rows {
            return None;
        }
        let idx = |c: i32, r: i32| (r * cols + c) as usize;
        let mut prev = vec![u32::MAX; (cols * rows) as usize];
        let mut queue = std::collections::VecDeque::from([to]);
        prev[idx(to.0, to.1)] = idx(to.0, to.1) as u32;
        while let Some((c, r)) = queue.pop_front() {
            if (c, r) == from {
                break;
            }
            for (dc, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nc, nr) = (c + dc, r + dr);
                if nc < 0 || nr < 0 || nc >= cols || nr >= rows || prev[idx(nc, nr)] != u32::MAX {
                    continue;
                }
                if !self.open(nc, nr) && (nc, nr) != from {
                    continue;
                }
                prev[idx(nc, nr)] = idx(c, r) as u32;
                queue.push_back((nc, nr));
            }
        }
        let step = prev[idx(from.0, from.1)];
        (step != u32::MAX).then(|| ((step as i32) % cols, (step as i32) / cols))
    }
}

/// Keys that walk a knight toward tile `goal` of its room.
fn walk_to(run: &Run, hero: &Hero, goal: (i32, i32), blocked: &BTreeSet<(i32, i32)>) -> (i8, i8) {
    let here = ((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
    let to = |c: i32, r: i32| ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
    let (cx, cy) = match (Grid {
        room: run.room(),
        blocked,
    })
    .step(here, goal)
    {
        Some(next) => to(next.0, next.1),
        None => to(goal.0, goal.1),
    };
    let (dx, dy) = (cx - hero.x, cy - hero.y);
    (sign(dx, 0.35), sign(dy, 0.35))
}

/// A party that crawls a dungeon floor: fights what is in the room, then walks
/// to the doorway of the next room it has not seen, and finally to the stairs.
struct Pilot {
    rng: Xs,
    wander: Option<(i32, i32)>,
    until: u64,
    /// Where each knight was, and since when; a knight pinned for a while
    /// shuffles off in a random direction.
    pinned: BTreeMap<u32, ((f32, f32), u64, (i8, i8, u64))>,
    /// Tiles of the room it stands in that stopped a knight.
    blocked: (usize, BTreeSet<(i32, i32)>),
}
impl Pilot {
    fn new(seed: u64) -> Pilot {
        Pilot {
            rng: Xs::new(seed),
            wander: None,
            until: 0,
            pinned: BTreeMap::new(),
            blocked: (usize::MAX, BTreeSet::new()),
        }
    }
    /// The tile the party should walk to now, and the way to push once there.
    fn goal(&mut self, run: &Run) -> Option<((i32, i32), (i8, i8))> {
        let room = run.room();
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        if run.open_map() {
            // An open floor is one map: wander to far floor tiles.
            if self.wander.is_none() || run.tick >= self.until {
                for _ in 0..40 {
                    let (c, r) = (
                        1 + self.rng.below(cols as u64 - 2) as i32,
                        1 + self.rng.below(rows as u64 - 2) as i32,
                    );
                    if room.tile(c, r) == Tile::Floor {
                        self.wander = Some((c, r));
                        break;
                    }
                }
                self.until = run.tick + 150;
            }
            return self.wander.map(|g| (g, (0, 0)));
        }
        let floor = &run.dungeon;
        // Rooms by walking distance from here; the first unseen one, else the stairs.
        let mut first: BTreeMap<usize, usize> = BTreeMap::new();
        let mut queue = std::collections::VecDeque::from([run.at]);
        let mut seen = BTreeSet::from([run.at]);
        let mut aim = None;
        while let Some(at) = queue.pop_front() {
            if at != run.at && !floor.rooms[at].visited && floor.rooms[at].kind != RoomKind::Secret
            {
                aim = Some(at);
                break;
            }
            for dir in 0..4 {
                let Some(next) = floor.neighbour(at, dir) else {
                    continue;
                };
                if seen.insert(next) {
                    first.insert(next, if at == run.at { dir } else { first[&at] });
                    queue.push_back(next);
                }
            }
        }
        if aim.is_none() && room.kind == RoomKind::Stairs {
            let at = (0..rows)
                .flat_map(|r| (0..cols).map(move |c| (c, r)))
                .find(|&(c, r)| room.tile(c, r) == Tile::Stairs);
            return at.map(|g| (g, (0, 0)));
        }
        let aim = aim.or_else(|| {
            (0..floor.rooms.len()).find(|&i| {
                floor.rooms[i].kind == RoomKind::Stairs && i != run.at && seen.contains(&i)
            })
        })?;
        let dir = first.get(&aim).copied()?;
        let (door, push) = match dir {
            0 => ((cols / 2, 0), (0, -1)),
            1 => ((cols - 1, rows / 2), (1, 0)),
            2 => ((cols / 2, rows - 1), (0, 1)),
            _ => ((0, rows / 2), (-1, 0)),
        };
        Some((door, push))
    }
    fn keys(&mut self, run: &Run) -> Keys {
        if self.blocked.0 != run.at {
            self.blocked = (run.at, BTreeSet::new());
        }
        let goal = self.goal(run);
        let mut out = Keys::new();
        for (&id, hero) in &run.players {
            let fought = fighter(run, id);
            let engaged = fought.is_some();
            let mut input = fought.unwrap_or_default();
            if !engaged && hero.hp > 0 && !hero.stone {
                let nearest = run
                    .enemies
                    .iter()
                    .filter(|e| e.hp > 0 && e.kind != rules::EnemyKind::Dummy)
                    .min_by(|a, b| {
                        (a.x - hero.x)
                            .hypot(a.y - hero.y)
                            .total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
                    });
                if let (Phase::Fighting, Some(e)) = (run.phase, nearest) {
                    // Nothing to shoot yet: walk at the nearest body.
                    (input.move_x, input.move_y) =
                        (sign(e.x - hero.x, 1.0), sign(e.y - hero.y, 1.0));
                    (input.aim_x, input.aim_y) = dir8(e.x - hero.x, e.y - hero.y);
                    input.fire = true;
                } else if let Some((tile, push)) = goal {
                    let here = ((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
                    (input.move_x, input.move_y) = if here == tile {
                        push
                    } else {
                        walk_to(run, hero, tile, &self.blocked.1)
                    };
                }
            }
            if run.tick % 40 == 0 {
                input.interact = true;
            }
            let rng = &mut self.rng;
            let (at, since, shuffle) =
                self.pinned
                    .entry(id)
                    .or_insert(((hero.x, hero.y), run.tick, (0, 0, 0)));
            if (hero.x - at.0).hypot(hero.y - at.1) > 0.2 {
                (*at, *since) = ((hero.x, hero.y), run.tick);
            } else if run.tick.saturating_sub(*since) > 45 && !engaged {
                let here = ((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
                let (mx, my) = (i32::from(input.move_x), i32::from(input.move_y));
                if (mx, my) != (0, 0) {
                    self.blocked.1.insert((here.0 + mx, here.1 + my));
                    if mx != 0 && my != 0 {
                        self.blocked.1.insert((here.0 + mx, here.1));
                        self.blocked.1.insert((here.0, here.1 + my));
                    }
                }
                *shuffle = (rng.axis(), rng.axis(), run.tick + 20);
                *since = run.tick;
            }
            if run.tick < shuffle.2 {
                (input.move_x, input.move_y) = (shuffle.0, shuffle.1);
            }
            out.insert(id, input);
        }
        out
    }
}

// ----------------------------------------------------------------- scenarios

struct Tick {
    inputs: Keys,
    step: bool,
}
impl Tick {
    fn go(inputs: Keys) -> Tick {
        Tick { inputs, step: true }
    }
}

/// A host's side of a tick: moves on the run it keeps, then the keys.
type Drive = Box<dyn FnMut(&mut Run, u64) -> Tick>;

struct Rig {
    run: Run,
    drive: Drive,
}

struct Scenario {
    name: &'static str,
    covers: &'static str,
    ticks: usize,
    settings: Settings,
    /// A phrasebook (text, errors) other than the stock one.
    book: Option<(String, Vec<String>)>,
    make: Box<dyn Fn() -> Rig>,
}

fn sc(
    name: &'static str,
    covers: &'static str,
    ticks: usize,
    make: impl Fn() -> Rig + 'static,
) -> Scenario {
    Scenario {
        name,
        covers,
        ticks,
        settings: Settings::default(),
        book: None,
        make: Box::new(make),
    }
}

fn open(mut s: Scenario) -> Scenario {
    s.settings.open_floors = true;
    s
}

fn knights(run: &mut Run, n: u32) {
    for id in 2..=n {
        run.join_seat(id, &format!("Knight {id}"));
    }
    for (id, class) in (1..=n).zip(rules::classes::CLASSES.iter().cycle()) {
        run.become_class(id, class);
    }
}

fn ids(run: &Run) -> Vec<u32> {
    run.players.keys().copied().collect()
}

/// A run on floor `depth` with `n` knights, playing the pilot.
fn crawl(seed: u64, depth: u32, n: u32) -> Rig {
    let mut run = Run::new(seed, 700 + seed, None);
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    knights(&mut run, n);
    pilot_rig(run, seed)
}

fn pilot_rig(run: Run, seed: u64) -> Rig {
    let mut pilot = Pilot::new(seed ^ 0xc0ffee);
    Rig {
        run,
        drive: Box::new(move |run, _| Tick::go(pilot.keys(run))),
    }
}

/// Floor `depth`, its guardian's room entered with `n` knights, the pilot fighting.
fn guardian(seed: u64, depth: u32, n: u32) -> Rig {
    let mut run = Run::new(seed, 800 + seed, None);
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    knights(&mut run, n);
    run.clear_prerequisites_for_test();
    if let Some(room) = run.boss_gates.as_ref().map(|g| g.guardian) {
        run.enter(room, Some(2));
    }
    pilot_rig(run, seed)
}

/// Every boss a floor's pack can raise, one after another in the first hall.
fn boss_parade(seed: u64, depth: u32, n: u32, per: u64) -> Rig {
    let mut run = Run::new(seed, 900 + seed, None);
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    knights(&mut run, n);
    let pack = run.dungeon.pack;
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && r.cols >= 24)
        .unwrap_or(1);
    run.enter_for_test(room);
    run.open_room_for_test(room);
    let mut pilot = Pilot::new(seed);
    let drive: Drive = Box::new(move |run, t| {
        if t % per == 0 {
            run.enemies.clear();
            run.projectiles.clear();
            let bosses = run.pack_bosses_for_test(pack);
            let (w, h) = (run.room().width(), run.room().height());
            if !bosses.is_empty() {
                run.spawn_boss_at_for_test(
                    bosses[(t / per) as usize % bosses.len()],
                    w / 2.0,
                    h / 3.0,
                );
            }
            run.phase = Phase::Fighting;
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
        }
        Tick::go(pilot.keys(run))
    });
    Rig { run, drive }
}

fn surface(seats: u32, levels: u8) -> Run {
    let mut home = rules::home::Home::default();
    if levels > 0 {
        for station in [
            rules::home::Station::Forge,
            rules::home::Station::Hearth,
            rules::home::Station::Rack,
            rules::home::Station::Chapel,
            rules::home::Station::Map,
            rules::home::Station::Wheel,
            rules::home::Station::Wing,
        ] {
            home.levels.insert(station, levels);
        }
    }
    let mut treasury = angel_delve::realm::Spoils::default();
    treasury.add(angel_delve::realm::Spoil::Gold, 9000);
    let mut run = Run::at_home(41, 1, None, home, treasury);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.walk_kingdom(None);
    run.floor_plates.clear();
    run
}

/// The party into the place of room kind `kind` on the kingdom map or the
/// Undercroft, as a walker through its door would be.
fn visit(run: &mut Run, kind: RoomKind) {
    let geo = rules::map::geography(&run.kingdom.wishes);
    let root = if rules::college::is_room(kind) {
        RoomKind::CollegeGrounds
    } else {
        kind
    };
    if let Some(site) = geo.sites.iter().position(|s| s.room == root) {
        run.enter_kingdom_place(site);
    }
    if run.room().kind != kind {
        if let Some(at) = rules::world::room_of(&run.dungeon, kind) {
            run.arrive(at, (12.0, 10.5));
        }
    }
}

/// Visits sites `first..first + count` of the kingdom map in turn, `per` ticks each.
fn tour(first: usize, count: usize, per: u64, seats: u32, seed: u64) -> Rig {
    let run = surface(seats, 2);
    let mut monkey = Monkey::new(seed);
    let drive: Drive = Box::new(move |run, t| {
        if t % per == 0 {
            let site = first + (t / per) as usize % count;
            run.enter_kingdom_place(site);
        }
        let ids = ids(run);
        let style = if t % 3 == 0 { PRESS } else { CALM };
        Tick::go(monkey.keys(&ids, style))
    });
    Rig { run, drive }
}

/// A fixture run started with `seats` knights, played by `keys`.
fn played(mut run: Run, seats: u32, keys: impl FnMut(&Run, u64) -> Keys + 'static) -> Rig {
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.floor_plates.clear();
    let mut keys = keys;
    Rig {
        run,
        drive: Box::new(move |run, t| Tick::go(keys(run, t))),
    }
}

/// What the Round Table's bots press; after the game, a monkey.
fn table(slug: &'static str, seats: u32, seed: u64) -> Rig {
    let run = Run::roundtable_for_test(slug).expect("a Round Table game");
    let mut monkey = Monkey::new(seed);
    let mut rig = played(run, seats, move |run, _| {
        if run.roundtable_showing() {
            run.roundtable_inputs()
        } else {
            let ids = ids(run);
            monkey.keys(&ids, CALM)
        }
    });
    // A seat that sits down late, as a joiner does.
    let mut late = false;
    let inner = std::mem::replace(&mut rig.drive, Box::new(|_, _| Tick::go(Keys::new())));
    let mut inner = inner;
    rig.drive = Box::new(move |run, t| {
        if t == 40 && !late && seats > 1 {
            late = true;
        }
        inner(run, t)
    });
    rig
}

/// A shelf game: the lone press that starts it, then its seat bots.
fn shelf(make: fn() -> Run, seats: u32, seed: u64) -> Rig {
    let mut monkey = Monkey::new(seed);
    let mut run = make();
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.floor_plates.clear();
    let drive: Drive = Box::new(move |run, t| {
        if t == 0 {
            return Tick::go(Keys::from([(
                1,
                Input {
                    interact: true,
                    ..Input::default()
                },
            )]));
        }
        Tick::go(shelf_bots(run, &mut monkey))
    });
    Rig { run, drive }
}

fn shelf_bots(run: &Run, monkey: &mut Monkey) -> Keys {
    use rules::{brasswake, pennon, rafterwake};
    let mut out = Keys::new();
    for &id in run.players.keys() {
        let input = if let Some(s) = &run.brasswake {
            s.lanes
                .get(&id)
                .map(|d| d.bot_input(brasswake::LENGTH - s.left))
        } else if let Some(s) = &run.rafterwake {
            s.lanes
                .get(&id)
                .map(|d| d.bot_input(rafterwake::LENGTH - s.left))
        } else if let Some(s) = &run.pennon {
            s.lanes
                .get(&id)
                .map(|d| d.bot_input(pennon::LENGTH - s.left))
        } else if let Some(s) = &run.mossbell {
            s.tables.get(&id).map(|d| d.bot_input())
        } else if let Some(s) = &run.cinder {
            s.boards.get(&id).map(|d| d.bot_input())
        } else if let Some(s) = &run.latch {
            s.duels.get(&id).map(|d| d.bot_input(1 + id % 3))
        } else {
            None
        };
        out.insert(id, input.unwrap_or_default());
    }
    if run.minigame_showing() {
        out
    } else {
        monkey.keys(&ids(run), CALM)
    }
}

/// The pastimes of the kingdom's greens, `per` ticks each, a monkey at the keys.
fn pastimes(games: Vec<rules::pastimes::Game>, per: u64, seats: u32, seed: u64) -> Rig {
    let run = surface(seats, 1);
    let mut monkey = Monkey::new(seed);
    let drive: Drive = Box::new(move |run, t| {
        if t % per == 0 {
            let game = games[(t / per) as usize % games.len()];
            visit(run, game.room());
            let (x, y) = game.start();
            for hero in run.players.values_mut() {
                (hero.x, hero.y) = (x * TILE_UNITS, y * TILE_UNITS);
            }
        }
        let style = if t % 90 < 45 { WILD } else { PRESS };
        Tick::go(monkey.keys(&ids(run), style))
    });
    Rig { run, drive }
}

/// A race hall of `course` with `seats` riders; the drift bot drives the Touge.
fn race(course: rules::race3d::Course, seats: u32, seed: u64) -> Rig {
    let run = angel_delve::render::overworld::arena::race3d::testing::racing(course, seats);
    let mut monkey = Monkey::new(seed);
    let mut rig = played(run, 1, move |run, _| {
        let mut keys = monkey.keys(&ids(run), PRESS);
        if let Some(race) = run.race3d.as_ref() {
            for (&id, input) in keys.iter_mut() {
                if race.course == rules::race3d::Course::Touge {
                    *input = rules::race3d::drift_bot(race, id);
                } else {
                    // Gas, steer by turns, now and then a jump.
                    input.fire = true;
                    input.dash = (run.tick + u64::from(id) * 7) % 53 == 0;
                }
            }
        }
        keys
    });
    rig.run.floor_plates.clear();
    rig
}

/// The first seed from 41 whose floor `depth` has a hall of 20 or more columns.
fn hall_floor(depth: u32, seats: u32) -> (Run, usize) {
    for seed in 41..200 {
        let mut run = Run::new(seed, 1, None);
        for id in 2..=seats {
            run.join_seat(id, &format!("Knight {id}"));
        }
        while run.dungeon.depth < depth {
            run.descend_for_test();
        }
        if let Some(at) = run
            .dungeon
            .rooms
            .iter()
            .position(|s| s.kind == RoomKind::Hall && s.cols >= 20)
        {
            return (run, at);
        }
    }
    panic!("no hall");
}

/// A hall's folly (the depth picks which), a monkey playing it.
fn folly(depth: u32, seats: u32, seed: u64) -> Rig {
    let (mut run, at) = hall_floor(depth, seats);
    run.clear_prerequisites_for_test();
    run.folly_room = Some(at);
    run.race3d_room = None;
    run.gantry_room = None;
    run.enter(at, None);
    run.floor_plates.clear();
    let mut monkey = Monkey::new(seed);
    Rig {
        run,
        drive: Box::new(move |run, _| Tick::go(monkey.keys(&ids(run), WILD))),
    }
}

/// The six nooks, 200 ticks each.
fn nooks_rig(seats: u32) -> Rig {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    let mut monkey = Monkey::new(404);
    let drive: Drive = Box::new(move |run, t| {
        if t % 200 == 0 {
            let kind = rules::nooks::KINDS[(t / 200) as usize % rules::nooks::KINDS.len()];
            let at = run.dungeon.rooms.len();
            run.dungeon
                .rooms
                .push(rules::nooks::room(kind, (99 + t as i32 / 200, 99), 2));
            run.enter(at, None);
            run.nook = Some(rules::nooks::Nook {
                room: at,
                ..Default::default()
            });
            run.floor_plates.clear();
        }
        Tick::go(monkey.keys(&ids(run), PRESS))
    });
    Rig { run, drive }
}

/// Floor 3's stairs with the floor cleared: the party stands on them, goes down
/// into the crossroads, votes, and plays the card it picked.
fn crossroads(seats: u32, seed: u64) -> Rig {
    use rules::votes::{Action, Command};
    let mut run = Run::new(47, 9, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    while run.dungeon.depth < 3 {
        run.descend_for_test();
    }
    // The third floor ends in the dragon's lair and has no stairs of its own; a
    // stair is laid in the first room (the test fixtures do the same) and the
    // party stands on it.
    run.boss_gates = None;
    run.enemies.clear();
    run.phase = Phase::Exploring;
    for room in &mut run.dungeon.rooms {
        room.cleared = true;
    }
    let at = run.at;
    let room = &run.dungeon.rooms[at];
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let spot = (2..rows - 2)
        .flat_map(|r| (2..cols - 2).map(move |c| (c, r)))
        .find(|&(c, r)| {
            room.tile(c, r) == Tile::Floor
                && room.tile(c + 1, r) == Tile::Floor
                && room.tile(c, r + 1) == Tile::Floor
        })
        .expect("a floor tile");
    run.dungeon.rooms[at].set_for_test(spot.0 as usize, spot.1 as usize, Tile::Stairs);
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
        (hero.x, hero.y) = (
            (spot.0 as f32 + 0.5) * TILE_UNITS,
            (spot.1 as f32 + 0.5) * TILE_UNITS,
        );
    }
    run.floor_plates.clear();
    let mut monkey = Monkey::new(seed);
    let mut rng = Xs::new(seed);
    let drive: Drive = Box::new(move |run, t| {
        // Standing still on the stairs takes the party down; then the vote, then the game.
        let mut keys = if run.crossroads.is_some() {
            monkey.keys(&ids(run), WILD)
        } else {
            ids(run)
                .into_iter()
                .map(|id| (id, Input::default()))
                .collect()
        };
        if run.crossroads_voting() && t % 20 == 3 {
            for (&id, input) in keys.iter_mut() {
                let serial = run.playtest.seats.get(&id).map_or(1, |s| s.serial + 1);
                input.vote = Some(Command {
                    serial,
                    action: Action::Rate(1 + rng.below(3) as u8),
                });
            }
        }
        Tick::go(keys)
    });
    Rig { run, drive }
}

/// Dame Fortune's lever pulled again and again, the wheel left to spin.
fn fortune_wheel(seed: u64) -> Rig {
    let mut home = rules::home::Home {
        deepest: 3,
        ..rules::home::Home::default()
    };
    home.levels.insert(rules::home::Station::Wheel, 2);
    let mut treasury = angel_delve::realm::Spoils::default();
    treasury.add(angel_delve::realm::Spoil::Gold, 9000);
    let mut run = Run::at_home(seed, 1, None, home, treasury);
    run.join_seat(2, "Seat 2");
    run.enter_for_test(1);
    let (c, r, w, h) = rules::fortune::LEVER;
    let drive: Drive = Box::new(move |run, t| {
        if t % 400 == 0 {
            if let Some(hero) = run.players.get_mut(&1) {
                (hero.x, hero.y) = (
                    (c as f32 + w as f32 / 2.0) * TILE_UNITS,
                    (r as f32 + h as f32 / 2.0) * TILE_UNITS,
                );
            }
        }
        let pull = t % 400 < u64::from(rules::home::BUY_HOLD) + 2;
        Tick::go(Keys::from([
            (
                1,
                Input {
                    interact: pull,
                    ..Input::default()
                },
            ),
            (
                2,
                Input {
                    move_x: if t % 60 < 30 { 1 } else { -1 },
                    ..Input::default()
                },
            ),
        ]))
    });
    Rig { run, drive }
}

/// Wishes asked of the phrasebook, granted to a party that fights on.
fn wishing(seed: u64, seats: u32) -> Rig {
    let mut run = Run::new(seed, 500 + seed, None);
    knights(&mut run, seats);
    while run.dungeon.depth < 2 {
        run.descend_for_test();
    }
    for hero in run.players.values_mut() {
        hero.wishes = 6;
        hero.great_wishes = 1;
    }
    let said = [
        "fire faster",
        "a bigger spiked shield",
        "golden spiky armor that explodes",
        "faster feet",
        "arrows that burn",
        "heal me",
    ];
    let mut pilot = Pilot::new(seed);
    let drive: Drive = Box::new(move |run, t| {
        if t % 60 == 30 {
            let id = 1 + (t / 60) as u32 % seats;
            let deck = run
                .players
                .get(&id)
                .map(|h| h.deck.clone())
                .unwrap_or_default();
            if let Some(wish) = phrasebook::find_for(said[(t / 60) as usize % said.len()], &deck) {
                let _ = run.wish(id, &wish);
            }
        }
        Tick::go(pilot.keys(run))
    });
    Rig { run, drive }
}

/// A cleared floor's cauldron: spells learned, thrown in, brewed.
fn cauldron(seed: u64) -> Rig {
    use rules::cauldron;
    let mut run = Run::new(seed, 2, Some("Matt"));
    run.join_seat(2, "Wren");
    while run.dungeon.depth < 2 {
        run.descend_for_test();
    }
    run.boss_gates = None;
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(room);
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run.clear_for_test();
    for id in [1, 2] {
        if let Some(hero) = run.players.get_mut(&id) {
            hero.spells[0] = Some("cabbage-storm".into());
            hero.spells[1] = Some("hedge-maze".into());
        }
    }
    let mut monkey = Monkey::new(seed);
    let drive: Drive = Box::new(move |run, t| {
        if let Some(pot) = run.cauldron_here().cloned() {
            // Knight 1 a step south of the pot; presses spaced apart.
            if t % 40 == 1 {
                if let Some(hero) = run.players.get_mut(&1) {
                    (hero.x, hero.y) = (pot.x, pot.y + 2.0);
                }
            }
        }
        let mut keys = Keys::new();
        let press = match t % 80 {
            5 => Input {
                interact: true,
                take: true,
                ..Input::default()
            },
            15 => Input {
                cast: 1,
                ..Input::default()
            },
            25 => Input {
                cast: 2,
                ..Input::default()
            },
            35 => Input {
                interact: true,
                take: true,
                ..Input::default()
            },
            _ => Input::default(),
        };
        keys.insert(1, press);
        let other = monkey.keys(&[2], CALM);
        keys.extend(other);
        let _ = cauldron::PUFF;
        Tick::go(keys)
    });
    Rig { run, drive }
}

/// The Undercroft's rooms and the surface's halls, visited in turn, pressed at.
fn home_rooms(seed: u64) -> Rig {
    let mut home = rules::home::Home::default();
    for station in [
        rules::home::Station::Forge,
        rules::home::Station::Hearth,
        rules::home::Station::Rack,
        rules::home::Station::Chapel,
        rules::home::Station::Map,
        rules::home::Station::Wheel,
        rules::home::Station::Wing,
    ] {
        home.levels.insert(station, 1);
    }
    let mut treasury = angel_delve::realm::Spoils::default();
    treasury.add(angel_delve::realm::Spoil::Gold, 9000);
    let mut run = Run::at_home(seed, 3, None, home, treasury);
    run.join_seat(2, "Seat 2");
    let kinds = [
        RoomKind::Home,
        RoomKind::Fortune,
        RoomKind::Yard,
        RoomKind::Trophies,
        RoomKind::Tavern,
        RoomKind::Stockpile,
        RoomKind::Workshop,
        RoomKind::Quarters,
        RoomKind::Hall,
        RoomKind::Muster,
        RoomKind::Eyrie,
        RoomKind::Gate,
    ];
    let mut monkey = Monkey::new(seed);
    let drive: Drive = Box::new(move |run, t| {
        if t % 110 == 0 {
            let kind = kinds[(t / 110) as usize % kinds.len()];
            if let Some(at) = rules::world::room_of(&run.dungeon, kind) {
                run.arrive(at, (12.0, 10.5));
            }
        }
        Tick::go(monkey.keys(&ids(run), if t % 2 == 0 { PRESS } else { CALM }))
    });
    Rig { run, drive }
}

/// A dungeon floor played by a monkey: every key a knight has, pressed at random.
fn chaos(seed: u64, depth: u32, n: u32) -> Rig {
    let mut run = Run::new(seed, 600 + seed, None);
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    knights(&mut run, n);
    let mut monkey = Monkey::new(seed);
    Rig {
        run,
        drive: Box::new(move |run, _| Tick::go(monkey.keys(&ids(run), WILD))),
    }
}

/// Host-side moves a live host makes between ticks: seats come and go, presence
/// and the room's hold change, the tune moves, the world is paused a while.
fn host_moves(seed: u64) -> Rig {
    let mut rig = crawl(seed, 2, 3);
    let mut inner = std::mem::replace(&mut rig.drive, Box::new(|_, _| Tick::go(Keys::new())));
    let mut rng = Xs::new(seed);
    rig.drive = Box::new(move |run, t| {
        match t {
            100 => run.join_seat(4, "Late Knight"),
            160 => run.set_absent(2, true),
            220 => run.set_away(3, true),
            260 => run.set_away(3, false),
            300 => run.set_absent(2, false),
            340 => run.hold(rules::presence::Hold::Arriving),
            345 => run.hold(rules::presence::Hold::Open),
            400 => run.leave(4),
            450 => run.hold(rules::presence::Hold::Empty),
            470 => run.hold(rules::presence::Hold::Open),
            520 => run.join_seat(5, "Second Late"),
            700 => run.leave(1),
            720 => run.join_seat(1, "Back Again"),
            _ => {}
        }
        if t % 90 == 45 {
            run.latch_input_age
                .insert(1 + rng.below(3) as u32, rng.below(40));
        }
        if t == 600 {
            run.tune.hitstop = 3;
            run.tune.hitstop_boss_kill = 6;
        }
        let mut tick = inner(run, t);
        if (240..270).contains(&t) {
            tick.step = false;
        }
        tick
    });
    rig
}

fn scenarios() -> Vec<Scenario> {
    let mut all = Vec::new();
    // The dungeon: parties of one, two and four, floors one to six.
    all.push(sc(
        "solo-f1-crawl",
        "one knight crawling floor 1: doors, fights, loot, the stairs",
        1800,
        || crawl(11, 1, 1),
    ));
    for (depth, seed) in (1..=6u32).zip(21u64..) {
        let (name, covers): (&'static str, &'static str) = [
            ("duo-f1-crawl", "two knights crawl floor 1 (Crypt)"),
            ("duo-f2-crawl", "two knights crawl floor 2 (Cavern)"),
            ("duo-f3-crawl", "two knights crawl floor 3 (Hellforge)"),
            ("duo-f4-crawl", "two knights crawl floor 4 (Archive)"),
            ("duo-f5-crawl", "two knights crawl floor 5 (Fungal Deep)"),
            ("duo-f6-crawl", "two knights crawl floor 6 (the Unknown)"),
        ][depth as usize - 1];
        all.push(sc(name, covers, 1100, move || crawl(seed, depth, 2)));
        let (name, covers): (&'static str, &'static str) = [
            ("squad-f1-crawl", "four knights crawl floor 1"),
            ("squad-f2-crawl", "four knights crawl floor 2"),
            ("squad-f3-crawl", "four knights crawl floor 3"),
            ("squad-f4-crawl", "four knights crawl floor 4"),
            ("squad-f5-crawl", "four knights crawl floor 5"),
            ("squad-f6-crawl", "four knights crawl floor 6"),
        ][depth as usize - 1];
        all.push(sc(name, covers, 1100, move || crawl(seed + 40, depth, 4)));
    }
    all.push(open(sc(
        "open-solo-f2",
        "a lone knight on an open floor (one map, side rooms)",
        1100,
        || crawl(61, 2, 1),
    )));
    all.push(open(sc(
        "open-squad-f3",
        "four knights on an open floor",
        1100,
        || crawl(62, 3, 4),
    )));
    for (depth, seed) in (1..=6u32).zip(71u64..) {
        let (name, covers): (&'static str, &'static str) = [
            ("boss-guardian-f1", "floor 1 guardian's room, 4 knights"),
            ("boss-guardian-f2", "floor 2 guardian's room, 4 knights"),
            ("boss-guardian-f3", "floor 3 guardian's room, 4 knights"),
            ("boss-guardian-f4", "floor 4 guardian's room, 4 knights"),
            ("boss-guardian-f5", "floor 5 guardian's room, 4 knights"),
            ("boss-guardian-f6", "floor 6 guardian's room, 4 knights"),
        ][depth as usize - 1];
        all.push(sc(name, covers, 700, move || guardian(seed, depth, 4)));
        let (name, covers): (&'static str, &'static str) = [
            ("boss-parade-f1", "every Crypt boss in turn, 3 knights"),
            ("boss-parade-f2", "every Cavern boss in turn, 3 knights"),
            ("boss-parade-f3", "every Hellforge boss in turn, 3 knights"),
            ("boss-parade-f4", "every Archive boss in turn, 3 knights"),
            (
                "boss-parade-f5",
                "every Fungal Deep boss in turn, 3 knights",
            ),
            ("boss-parade-f6", "every Unknown boss in turn, 3 knights"),
        ][depth as usize - 1];
        all.push(sc(name, covers, 1500, move || {
            boss_parade(seed + 10, depth, 3, 250)
        }));
    }
    all.push(sc(
        "lair-dragon",
        "the last floor's lair: the dragon, 4 knights",
        900,
        || {
            let mut run = Run::new(90, 990, None);
            knights(&mut run, 4);
            run.lair_for_test();
            pilot_rig(run, 90)
        },
    ));
    all.push(sc(
        "wipe-duo-f2",
        "two knights hurt to one hit point fall in a fight: the wipe, the ask to go again",
        900,
        || {
            let mut rig = crawl(91, 2, 2);
            let mut inner =
                std::mem::replace(&mut rig.drive, Box::new(|_, _| Tick::go(Keys::new())));
            rig.drive = Box::new(move |run, t| {
                if t == 120 {
                    for hero in run.players.values_mut() {
                        (hero.hp, hero.invulnerable) = (1, 0);
                    }
                }
                let mut tick = inner(run, t);
                if run.phase == Phase::Wiped {
                    for input in tick.inputs.values_mut() {
                        input.again = t % 7 == 0;
                        input.skip_plate = t % 11 == 0;
                    }
                }
                tick
            });
            rig
        },
    ));
    // Home, the kingdom and its places.
    all.push(sc(
        "kingdom-walk",
        "two knights wander the kingdom map pressing E",
        1000,
        || {
            let run = surface(2, 2);
            let mut monkey = Monkey::new(5);
            Rig {
                run,
                drive: Box::new(move |run, _| Tick::go(monkey.keys(&ids(run), PRESS))),
            }
        },
    ));
    all.push(sc(
        "home-undercroft",
        "the Undercroft built to level 2: forge, hearth, rack, chapel, map, wheel, tavern wing",
        1200,
        || {
            let mut home = rules::home::Home::default();
            for station in [
                rules::home::Station::Forge,
                rules::home::Station::Hearth,
                rules::home::Station::Rack,
                rules::home::Station::Chapel,
                rules::home::Station::Map,
                rules::home::Station::Wheel,
                rules::home::Station::Wing,
            ] {
                home.levels.insert(station, 2);
            }
            let mut treasury = angel_delve::realm::Spoils::default();
            treasury.add(angel_delve::realm::Spoil::Gold, 9000);
            let mut run = Run::at_home(7, 1, None, home, treasury);
            for id in 2..=3 {
                run.join_seat(id, &format!("Seat {id}"));
            }
            let mut monkey = Monkey::new(6);
            Rig {
                run,
                drive: Box::new(move |run, _| Tick::go(monkey.keys(&ids(run), PRESS))),
            }
        },
    ));
    for (n, (first, count)) in [
        (0usize, 13usize),
        (13, 13),
        (26, 13),
        (39, 13),
        (52, 13),
        (65, 13),
    ]
    .into_iter()
    .enumerate()
    {
        let name: &'static str = [
            "kingdom-tour-1",
            "kingdom-tour-2",
            "kingdom-tour-3",
            "kingdom-tour-4",
            "kingdom-tour-5",
            "kingdom-tour-6",
        ][n];
        all.push(sc(
            name,
            "visits 13 places of the kingdom map in turn, 100 ticks each",
            13 * 100,
            move || tour(first, count, 100, 2, 100 + n as u64),
        ));
    }
    // The Round Table's adventures and the races.
    for (n, slug) in [
        "lanternward",
        "moonfall",
        "bannerpeak",
        "griffinwing",
        "lightgallop",
        "knight_racers",
        "deathmatch",
    ]
    .into_iter()
    .enumerate()
    {
        let (name, covers): (&'static str, &'static str) = [
            ("game-lanternward", "Lanternward, 3 seats on bots"),
            ("game-moonfall", "Moonfall, 3 seats on bots"),
            ("game-bannerpeak", "Bannerpeak, 3 seats on bots"),
            ("game-griffinwing", "Griffinwing, 3 seats on bots"),
            ("game-lightgallop", "Lightgallop, 3 seats on bots"),
            ("game-knight-racers", "Knight Racers, 3 seats on bots"),
            ("game-deathmatch", "Deathmatch, 3 seats on bots"),
        ][n];
        all.push(sc(name, covers, 1100, move || {
            table(slug, 3, 200 + n as u64)
        }));
    }
    for (name, covers, make) in [
        (
            "shelf-brasswake",
            "Brasswake, 3 seats",
            Run::brasswake_for_test as fn() -> Run,
        ),
        (
            "shelf-rafterwake",
            "Rafterwake, 3 seats",
            Run::rafterwake_for_test,
        ),
        ("shelf-pennon", "Pennon Rush, 3 seats", Run::pennon_for_test),
        (
            "shelf-mossbell",
            "Mossbell, 3 seats",
            Run::mossbell_for_test,
        ),
        (
            "shelf-cinder",
            "Cinder Stack, 3 seats",
            Run::cinder_for_test,
        ),
        ("shelf-latch", "Latch Duel, 3 seats", Run::latch_for_test),
    ] {
        all.push(sc(name, covers, 900, move || shelf(make, 3, 300)));
    }
    // Pastimes, rides and the small games.
    {
        use rules::pastimes::Game;
        let games = Game::ALL.to_vec();
        for (n, name) in ["pastimes-a", "pastimes-b", "pastimes-c"]
            .into_iter()
            .enumerate()
        {
            let part: Vec<Game> = games.iter().copied().skip(n * 7).take(7).collect();
            all.push(sc(
                name,
                "a third of the greens' pastimes, 7 games in turn, 2 seats",
                7 * 150,
                move || pastimes(part.clone(), 150, 2, 600 + n as u64),
            ));
        }
    }
    all.push(sc(
        "ride-hippodrome",
        "a chariot race at the Hippodrome",
        800,
        || {
            let mut run = surface(2, 1);
            visit(&mut run, RoomKind::Hippodrome);
            run.board_chariot(1);
            if let Some(c) = run.chariot.as_mut() {
                c.phase = rules::chariot::Phase::Racing;
            }
            let mut monkey = Monkey::new(7);
            Rig {
                run,
                drive: Box::new(move |run, _| Tick::go(monkey.keys(&ids(run), WILD))),
            }
        },
    ));
    all.push(sc(
        "ride-lists",
        "a bout at the lists: mount, charge, clash",
        800,
        || {
            let mut run = surface(2, 1);
            visit(&mut run, RoomKind::Lists);
            run.mount_up(1);
            if let Some(j) = run.joust.as_mut() {
                j.stage = rules::joust::Stage::Charge;
            }
            let mut monkey = Monkey::new(8);
            Rig {
                run,
                drive: Box::new(move |run, _| Tick::go(monkey.keys(&ids(run), WILD))),
            }
        },
    ));
    for (name, course) in [
        ("race3d-shaft", rules::race3d::Course::Shaft),
        ("race3d-cart", rules::race3d::Course::Cart),
        ("race3d-griffin", rules::race3d::Course::Griffin),
        ("race3d-touge", rules::race3d::Course::Touge),
    ] {
        all.push(sc(name, "a race hall course, 4 riders", 800, move || {
            race(course, 4, 800)
        }));
    }
    for (name, covers, depth) in [
        ("folly-wax-maze", "the Wax Maze (floor 1 hall)", 1u32),
        ("folly-belfry-shoot", "the Belfry Shoot (floor 2 hall)", 2),
        ("folly-powder-room", "the Powder Room (floor 3 hall)", 3),
        ("folly-book-hop", "Book Hop (floor 6 hall)", 6),
    ] {
        all.push(sc(name, covers, 700, move || {
            folly(depth, 3, 900 + u64::from(depth))
        }));
    }
    all.push(sc(
        "nooks",
        "the six nooks in turn, 2 seats (scripted brains)",
        6 * 200,
        || nooks_rig(2),
    ));
    all.push(sc(
        "forest-road",
        "the Forest Road run, 3 seats on bots",
        900,
        || {
            let mut run = Run::forest_road_for_test();
            for id in 2..=3 {
                run.join_seat(id, &format!("Knight {id}"));
            }
            run.start_forest_road();
            run.floor_plates.clear();
            let mut monkey = Monkey::new(11);
            Rig {
                run,
                drive: Box::new(move |run, _| {
                    if let Some(stage) = &run.forest_road {
                        Tick::go(
                            run.players
                                .iter()
                                .map(|(&id, h)| (id, stage.bot_input(id, h)))
                                .collect(),
                        )
                    } else {
                        Tick::go(monkey.keys(&ids(run), CALM))
                    }
                }),
            }
        },
    ));
    // Crossroads, wishes, the cauldron, host moves, a full table.
    all.push(sc(
        "crossroads",
        "floor 3's stairs: the descent into the crossroads, the vote, the card played",
        1500,
        || crossroads(3, 21),
    ));
    all.push(sc(
        "fortune-wheel",
        "Dame Fortune's lever pulled, the wheel spun, its mode set",
        1200,
        || fortune_wheel(23),
    ));
    all.push(sc(
        "wishes-duo",
        "wishes asked of the stock phrasebook and granted mid-fight, 2 knights",
        1000,
        || wishing(31, 2),
    ));
    all.push(sc(
        "cauldron",
        "the cauldron on a cleared floor: spells thrown in and brewed",
        700,
        || cauldron(5),
    ));
    all.push(sc(
        "host-moves",
        "seats joining and leaving, presence, hold, tune and paused ticks",
        800,
        || host_moves(41),
    ));
    all.push(sc(
        "home-rooms",
        "the Undercroft's rooms and the surface halls visited in turn, E pressed",
        12 * 110,
        || home_rooms(17),
    ));
    all.push(sc(
        "chaos-f2",
        "floor 2 with 3 knights on random keys: every key the game has",
        900,
        || chaos(61, 2, 3),
    ));
    all.push(sc(
        "chaos-f5",
        "floor 5 with 3 knights on random keys",
        900,
        || chaos(62, 5, 3),
    ));
    all.push(sc(
        "six-seats",
        "a full table of six knights on floor 3",
        700,
        || crawl(51, 3, 6),
    ));
    all
}

// ------------------------------------------------------------------ recorder

fn drain(run: &mut Run) {
    // What the host takes out of the run after a tick (see realm-host's step).
    run.cues.clear();
    run.sounds.clear();
    run.seat_sounds.clear();
    run.notices.clear();
    run.seat_notices.clear();
    run.feel.lines.clear();
}

struct Tape {
    child: std::process::Child,
    stdin: Option<std::io::BufWriter<std::process::ChildStdin>>,
}
impl Tape {
    fn create(path: &Path) -> Tape {
        let file = std::fs::File::create(path).expect("corpus file");
        let mut child = Command::new("gzip")
            .args(["-n", "-6", "-c"])
            .stdin(Stdio::piped())
            .stdout(file)
            .spawn()
            .expect("gzip");
        let stdin = std::io::BufWriter::with_capacity(1 << 20, child.stdin.take().unwrap());
        Tape {
            child,
            stdin: Some(stdin),
        }
    }
    fn line(&mut self, parts: &[&str]) {
        let w = self.stdin.as_mut().unwrap();
        for part in parts {
            w.write_all(part.as_bytes()).unwrap();
        }
        w.write_all(b"\n").unwrap();
    }
    fn finish(mut self) {
        drop(self.stdin.take());
        assert!(self.child.wait().unwrap().success(), "gzip failed");
    }
}

/// The registry's places that are on in every room (`rules::activity::WORLD`, `PLACES`).
const ALWAYS_ON: [&str; 8] = [
    "the kingdom",
    "the countryside",
    "the pastimes",
    "the merchant",
    "the holdings",
    "the ranch",
    "the barony",
    "Labyrinth College",
];

#[derive(Default)]
struct Entry {
    name: String,
    covers: String,
    file: String,
    ticks: usize,
    stepped: usize,
    bytes_gz: u64,
    bytes_raw: u64,
    before_lines: usize,
    seats: usize,
    rooms: BTreeSet<String>,
    activities: BTreeSet<String>,
    games: BTreeSet<String>,
    phases: BTreeSet<String>,
    depths: BTreeSet<u32>,
    changed: BTreeSet<String>,
    keys: BTreeSet<String>,
    settings: Option<Settings>,
    custom_book: bool,
}

fn record(sc: &Scenario, dir: &Path, stock: &(String, Vec<String>)) -> Entry {
    settings::set(sc.settings);
    let book = sc.book.clone().unwrap_or_else(|| stock.clone());
    phrasebook::install(book.0, book.1).expect("phrasebook");
    let file = format!("{}.jsonl.gz", sc.name);
    let mut tape = Tape::create(&dir.join(&file));
    let mut rig = (sc.make)();
    let mut entry = Entry {
        name: sc.name.into(),
        covers: sc.covers.into(),
        file,
        settings: Some(sc.settings),
        custom_book: sc.book.is_some(),
        ..Entry::default()
    };
    let mut raw = 0u64;
    let first = runtime_state::capture_for_rules(&rig.run)
        .expect("capture")
        .into_value();
    entry.keys = first["run"]
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let mut previous = serde_json::to_string(&first).unwrap();
    // The worker restores it as the host's first word.
    runtime_state::restore_value(&first).expect("the opening state restores");
    tape.line(&[r#"{"kind":"restore","state":"#, &previous, "}"]);
    raw += previous.len() as u64 + 32;
    let mut context = None;
    let mut step = |run: &mut Run, inputs: &Keys| run.step(inputs);
    let mut shown_book: Option<(String, Vec<String>)> = None;
    let mut last_top: Option<serde_json::Map<String, Value>> = None;
    for t in 0..sc.ticks as u64 {
        let tick = (rig.drive)(&mut rig.run, t);
        let before = runtime_state::capture_for_rules(&rig.run)
            .expect("capture")
            .into_value();
        let before_text = serde_json::to_string(&before).unwrap();
        let book = phrasebook::snapshot();
        let mut event = json!({"inputs": tick.inputs, "settings": settings::get(), "phrasebook": book, "step": tick.step});
        // The worker's own apply wants the whole event; the file leaves a repeated book out.
        let full = event.clone();
        if shown_book.as_ref() == Some(&book) {
            event.as_object_mut().unwrap().remove("phrasebook");
        } else {
            shown_book = Some(book);
        }
        let reply = angel_delve_system::apply(
            &json!({"state": before, "event": full}),
            &mut context,
            &mut step,
        )
        .unwrap_or_else(|e| panic!("{}: tick {t}: the worker refused: {e}", sc.name));
        let after = reply["state"].clone();
        let after_text = serde_json::to_string(&after).unwrap();
        let event_text = serde_json::to_string(&event).unwrap();
        let same = before_text == previous;
        entry.before_lines += usize::from(!same);
        tape.line(&[
            r#"{"kind":"apply","before":"#,
            if same { "null" } else { &before_text },
            r#","event":"#,
            &event_text,
            r#","after":"#,
            &after_text,
            "}",
        ]);
        raw += (if same { 4 } else { before_text.len() } + event_text.len() + after_text.len() + 60)
            as u64;
        entry.stepped += usize::from(tick.step);
        // Coverage: which top-level parts of the Run moved this tick.
        if let Some(top) = after["run"].as_object() {
            if let Some(last) = &last_top {
                for (k, v) in top {
                    if last.get(k) != Some(v) {
                        entry.changed.insert(k.clone());
                    }
                }
            }
            last_top = Some(top.clone());
        }
        previous = after_text;
        // The host's run continues from the worker's word.
        rig.run = runtime_state::restore_value(&after)
            .unwrap_or_else(|e| panic!("{}: tick {t}: reply does not restore: {e}", sc.name));
        entry.rooms.insert(format!("{:?}", rig.run.room().kind));
        entry.depths.insert(rig.run.dungeon.depth);
        entry.phases.insert(format!("{:?}", rig.run.phase));
        entry.activities.extend(
            rig.run
                .activities_on()
                .into_iter()
                .filter(|a| !ALWAYS_ON.contains(a))
                .map(String::from),
        );
        if let Some(game) = rig.run.roundtable_under_way() {
            entry.games.insert(game.slug.into());
        }
        entry.seats = entry.seats.max(rig.run.players.len());
        drain(&mut rig.run);
    }
    tape.finish();
    entry.ticks = sc.ticks;
    entry.bytes_raw = raw;
    entry.bytes_gz = std::fs::metadata(dir.join(&entry.file)).unwrap().len();
    entry
}

#[test]
#[ignore]
fn record_corpus() {
    let dir: PathBuf = std::env::var_os("DELVE_REPLAY_OUT")
        .map(PathBuf::from)
        .expect("set DELVE_REPLAY_OUT to the replay corpus directory");
    std::fs::create_dir_all(&dir).unwrap();
    let only: Option<Vec<String>> = std::env::var("DELVE_REPLAY_ONLY")
        .ok()
        .map(|s| s.split(',').map(String::from).collect());
    angel_delve_system::install_rules_engine();
    let stock = phrasebook::snapshot();
    let mut entries = Vec::new();
    for sc in scenarios() {
        if only
            .as_ref()
            .is_some_and(|o| !o.iter().any(|n| n == sc.name))
        {
            continue;
        }
        let started = std::time::Instant::now();
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| record(&sc, &dir, &stock)));
        match outcome {
            Ok(entry) => {
                eprintln!(
                    "{:28} {:5} ticks {:7.1} MB gz ({:7.1} raw) {:5} explicit befores {:6.1}s",
                    entry.name,
                    entry.ticks,
                    entry.bytes_gz as f64 / 1e6,
                    entry.bytes_raw as f64 / 1e6,
                    entry.before_lines,
                    started.elapsed().as_secs_f32()
                );
                entries.push(entry);
            }
            Err(_) => {
                eprintln!("{:28} FAILED", sc.name);
                let _ = std::fs::remove_file(dir.join(format!("{}.jsonl.gz", sc.name)));
            }
        }
    }
    let list: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "name": e.name, "file": e.file, "covers": e.covers,
                "ticks": e.ticks, "stepped": e.stepped,
                "bytes": e.bytes_gz, "raw_bytes": e.bytes_raw, "explicit_before": e.before_lines,
                "seats": e.seats, "settings": e.settings, "custom_phrasebook": e.custom_book,
                "depths": e.depths, "rooms": e.rooms, "phases": e.phases,
                "activities": e.activities, "roundtable": e.games,
                "state_keys_moved": e.changed,
            })
        })
        .collect();
    let moved: BTreeSet<&String> = entries.iter().flat_map(|e| e.changed.iter()).collect();
    let keys: BTreeSet<&String> = entries.iter().flat_map(|e| e.keys.iter()).collect();
    let never: Vec<&&String> = keys.iter().filter(|k| !moved.contains(**k)).collect();
    let manifest = json!({
        "format": "delve-replay/1",
        "schema": runtime_state::SCHEMA,
        "scenarios": list,
        "totals": {
            "scenarios": entries.len(),
            "ticks": entries.iter().map(|e| e.ticks).sum::<usize>(),
            "bytes": entries.iter().map(|e| e.bytes_gz).sum::<u64>(),
            "state_keys": keys.len(),
            "state_keys_moved": moved.len(),
            "state_keys_never_moved": never,
        },
    });
    let name = if only.is_none() {
        "MANIFEST.json"
    } else {
        "MANIFEST.partial.json"
    };
    std::fs::write(
        dir.join(name),
        serde_json::to_string_pretty(&manifest).unwrap() + "\n",
    )
    .unwrap();
}
