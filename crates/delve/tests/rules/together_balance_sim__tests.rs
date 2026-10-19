//! A balance harness: the real `Run::step`, bots at the controls.
//!
//! `cargo test --bin angel balance_sim -- --ignored --nocapture` prints, for
//! parties of one through four, on the early floors with fresh knights and
//! on the deep ones with the decks of a won run (`fixtures/balance_party.json`),
//! how long a room takes, what it costs in health, what healing gave back,
//! how often a knight lost the helmet (40%) or the clothes (15%), and who
//! fell. The bot kites and aims in the eight ways a keyboard can, shields or
//! rolls from shots near it, drinks a potion when hurt and throws a bomb at a
//! crowd: a patient player, not a perfect one.
//!
//! The targets, for a party of three that built well:
//!  - a normal room takes 10-25 s and costs 10-25% of the party's health;
//!  - a deep boss takes 45-90 s and costs 30-60%;
//!  - on the deep floors most fights cost somebody the helmet, most boss
//!    fights cost somebody the clothes, and a death is occasional, not usual.

use super::*;
use std::f32::consts::PI;

fn dir8(dx: f32, dy: f32) -> (i8, i8) {
    let k = ((dy.atan2(dx) / (PI / 4.0)).round() as i32).rem_euclid(8);
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

fn step_of(v: f32) -> i8 {
    if v > 0.35 {
        1
    } else if v < -0.35 {
        -1
    } else {
        0
    }
}

/// What a bot does for one knight this tick (the open floors' bots fight
/// with it too).
pub(crate) fn bot_input(run: &Run, id: u32) -> Input {
    let hero = &run.players[&id];
    if hero.hp == 0 || hero.stone {
        return Input::default();
    }
    let foes: Vec<&Enemy> = run
        .enemies
        .iter()
        .filter(|e| e.hp > 0 && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH && !e.dormant)
        .collect();
    let melee = hero
        .forged
        .as_ref()
        .is_some_and(|w| w.melee.is_some() && w.bolt.is_none() && w.throw.is_none());
    let dist = |e: &Enemy| (e.x - hero.x).hypot(e.y - hero.y);
    let target = foes
        .iter()
        .copied()
        .min_by(|a, b| dist(a).total_cmp(&dist(b)));
    let mut input = Input::default();
    let (w, h) = (run.room().width(), run.room().height());
    // Where to stand: ranged knights at a good distance, melee knights close.
    let want = if melee { 1.8 } else { 7.5 };
    let (mut vx, mut vy) = (0.0f32, 0.0f32);
    if let Some(t) = target {
        let d = dist(t).max(0.01);
        let (ux, uy) = ((t.x - hero.x) / d, (t.y - hero.y) / d);
        if d > want + 1.0 {
            (vx, vy) = (ux, uy);
        } else if d < want - 1.5 {
            (vx, vy) = (-ux, -uy);
        } else {
            let side = if (run.tick / 50 + u64::from(id)).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            (vx, vy) = (-uy * side * 0.6, ux * side * 0.6);
        }
        let (ax, ay) = dir8(ux, uy);
        input.aim_x = ax;
        input.aim_y = ay;
        input.fire = true;
        if d < 2.4 {
            input.swing = true;
        }
    }
    // Dodge: sidestep a hostile shot that is close and coming; step off a
    // lob's landing mark.
    let mut threatened = 0.0f32;
    for shot in run.projectiles.iter().filter(|p| p.hostile && p.ttl > 0) {
        if let Some(((lx, ly), _)) = patterns::lob_flight(shot) {
            let (dx, dy) = (hero.x - lx, hero.y - ly);
            let d = dx.hypot(dy);
            if d < HERO_RADIUS + patterns::LOB_SPLASH + 0.6 {
                let (ux, uy) = if d > 0.01 { (dx / d, dy / d) } else { (1.0, 0.0) };
                vx += ux * 1.6;
                vy += uy * 1.6;
                threatened = threatened.max(0.4);
            }
            continue;
        }
        let (dx, dy) = (hero.x - shot.x, hero.y - shot.y);
        let d = dx.hypot(dy);
        let speed = shot.vx.hypot(shot.vy).max(0.01);
        let closing = (shot.vx * dx + shot.vy * dy) / (speed * d.max(0.01));
        if d < 4.5 && closing > 0.6 {
            threatened = threatened.max(1.0 / d.max(0.5));
            let (px, py) = (-shot.vy / speed, shot.vx / speed);
            let side = if px * dx + py * dy >= 0.0 { 1.0 } else { -1.0 };
            vx += px * side * 1.4;
            vy += py * side * 1.4;
        }
    }
    // Step off a beam's line, as a player does when it shows its warning.
    for shot in run.projectiles.iter().filter(|p| p.hostile && p.ttl > 0) {
        let Some(beam) = shot.traits.path.beam else {
            continue;
        };
        let (ux, uy) = (beam.angle.cos(), beam.angle.sin());
        let (rx, ry) = (hero.x - shot.x, hero.y - shot.y);
        let along = rx * ux + ry * uy;
        let side = rx * -uy + ry * ux;
        if (0.0..beam.reach).contains(&along) && side.abs() < 2.5 {
            let away = if side >= 0.0 { 1.0 } else { -1.0 };
            vx += -uy * away * 1.6;
            vy += ux * away * 1.6;
            threatened = threatened.max(0.5);
        }
    }
    // Keep off the walls.
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
    input.move_x = step_of(vx);
    input.move_y = step_of(vy);
    // The guard key.
    input.dash = match hero.guard {
        cards::Guard::Roll => threatened > 0.45 && hero.dash_cooldown == 0,
        cards::Guard::Shield => threatened > 0.3,
        cards::Guard::Spin => foes.iter().any(|e| dist(e) < 3.2),
        cards::Guard::Wall(_) => threatened > 0.3,
    };
    if hero.guard == cards::Guard::Shield && input.dash {
        input.fire = false;
    }
    // Cards in hand: a potion when hurt, a ward when badly hurt, a censer
    // when the party is.
    let share = |h: &Hero| h.hp as f32 / h.max_hp.max(1) as f32;
    let party_low = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .any(|h| share(h) < 0.5);
    for (slot, id) in hero.hand.iter().enumerate() {
        let Some(card) = run.book.get(id) else {
            continue;
        };
        let has = |f: fn(&cards::Effect) -> bool| card.effects.iter().any(f);
        let heal = has(|e| matches!(e, cards::Effect::Heal(_)));
        let ward = has(|e| matches!(e, cards::Effect::Ward(_)));
        let censer = has(|e| matches!(e, cards::Effect::Censer(_)));
        let nova = has(|e| matches!(e, cards::Effect::Nova(_)));
        let want = (heal && share(hero) < 0.5)
            || (ward && share(hero) < 0.4 && threatened > 0.0)
            || (censer && party_low)
            || (nova && foes.len() >= 3);
        if want {
            input.play = slot as u8 + 1;
            break;
        }
    }
    // Bombs at a crowd, spells whenever they are ready, the ultimate when full.
    if hero.bombs > 0 && foes.iter().filter(|e| dist(e) < 5.0).count() >= 3 {
        input.bomb = true;
    }
    if let Some(slot) = (0..cards::SPELL_SLOTS)
        .find(|&s| hero.spells[s].is_some() && hero.spell_cooldowns[s] == 0 && target.is_some())
    {
        input.cast = slot as u8 + 1;
    }
    input.ult = hero.ult_charge >= ults::ULT_FULL && target.is_some();
    input
}

#[derive(Clone, Debug, Default)]
struct Fight {
    secs: f32,
    cleared: bool,
    /// Damage taken and healing received, each as a share of the party's
    /// health pool, in percent.
    dmg: f32,
    heal: f32,
    deaths: u32,
    helm: bool,
    undies: bool,
    foes: usize,
    foe_hp: u32,
    /// The health of the boss in the room, if there is one.
    boss_hp: u32,
}

/// Fight the room the party stands in until it is cleared, the party falls,
/// or `cap` seconds pass.
fn fight(run: &mut Run, cap: u32) -> Fight {
    let mut out = Fight {
        foes: run.enemies.len(),
        foe_hp: run.enemies.iter().map(|e| e.max_hp).sum(),
        boss_hp: run
            .enemies
            .iter()
            .filter(|e| e.is_boss())
            .map(|e| e.max_hp)
            .sum(),
        ..Fight::default()
    };
    let pool: u32 = run.players.values().map(|h| h.max_hp).sum();
    let (mut lost, mut healed) = (0u32, 0u32);
    let mut fell = std::collections::BTreeSet::new();
    let start = run.tick;
    for _ in 0..cap * HZ {
        let inputs: BTreeMap<u32, Input> = run
            .players
            .keys()
            .map(|&id| (id, bot_input(run, id)))
            .collect();
        let before: BTreeMap<u32, (u32, u32)> = run
            .players
            .iter()
            .map(|(&id, h)| (id, (h.hp, h.max_hp)))
            .collect();
        run.step(&inputs);
        for (&id, hero) in &run.players {
            let (was, max) = before[&id];
            if hero.max_hp == max {
                if hero.hp < was {
                    lost += was - hero.hp;
                } else {
                    healed += hero.hp - was;
                }
            }
            if hero.hp == 0 {
                fell.insert(id);
            }
            out.helm |= hero.gear >= 1 && hero.hp > 0;
            out.undies |= hero.gear >= 2 && hero.hp > 0;
        }
        if run.phase != Phase::Fighting {
            break;
        }
    }
    out.secs = (run.tick - start) as f32 / HZ as f32;
    out.cleared = run.phase != Phase::Fighting && run.phase != Phase::Wiped;
    out.dmg = lost as f32 * 100.0 / pool.max(1) as f32;
    out.heal = healed as f32 * 100.0 / pool.max(1) as f32;
    out.deaths = fell.len() as u32;
    out
}

#[derive(Clone, Copy, Debug)]
enum Party {
    /// Percival, Lynette and Gareth as the intro dresses them.
    Fresh,
    /// The three decks of a won run (`fixtures/balance_party.json`).
    Won,
    /// The won run's decks, half of each: a party part-way.
    Mid,
}

fn fixture() -> (Vec<Hero>, Vec<Card>) {
    let raw = include_str!("fixtures/balance_party.json");
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let heroes = serde_json::from_value(value["heroes"].clone()).unwrap();
    let cards = serde_json::from_value(value["cards"].clone()).unwrap();
    (heroes, cards)
}

/// A run on floor `depth` with `n` knights of `party`.
fn floor_run(seed: u64, depth: u32, party: Party, n: usize) -> Run {
    floor_run_in(seed, depth, party, n, None)
}

/// A run on floor `depth` of `pack` (the delve's own order when none), with
/// `n` knights of `party`.
fn floor_run_in(seed: u64, depth: u32, party: Party, n: usize, pack: Option<Pack>) -> Run {
    let mut run = Run::new(seed, 700 + seed, None);
    if let Some(pack) = pack {
        run.begin_in(pack);
    }
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    let (saved, cards) = fixture();
    for card in cards {
        run.book.insert(card);
    }
    let mut players = BTreeMap::new();
    for i in 0..n {
        let id = i as u32 + 1;
        let mut hero = match party {
            Party::Fresh => {
                let mut hero = Hero::new(&format!("K{id}"), 24.0, 20.0);
                run.players.insert(id, hero.clone());
                run.outfit(
                    id,
                    knights::knight(["percival", "lynette", "gareth", "galahad"][i % 4]).unwrap(),
                );
                hero = run.players.remove(&id).unwrap();
                hero
            }
            Party::Won | Party::Mid => {
                let mut hero = saved[i % saved.len()].clone();
                // Draughts are scarcer now: two cards in hand, not four.
                hero.hand.truncate(2);
                if let Party::Mid = party {
                    hero.deck.truncate(hero.deck.len() / 2);
                    hero.hand.truncate(2);
                    hero.max_hp = 100 + 40;
                }
                hero
            }
        };
        hero.invulnerable = 0;
        hero.hp = hero.max_hp;
        hero.ult_charge = ults::ULT_FULL / 2;
        players.insert(id, hero);
    }
    run.players = players;
    for hero in run.players.values_mut() {
        hero.rebonus(&run.book);
        hero.hp = hero.hp.min(hero.max_hp);
    }
    // The floor laid out for the party, as the stairs lay it
    // (`layout::party_room`); BALANCE_SOLO_ROOMS lays a lone knight's the
    // same way, to compare.
    if n > 1 {
        let party = if std::env::var_os("BALANCE_SOLO_ROOMS").is_some() { 1 } else { n };
        run.dungeon = layout::floor_for_party(run.mode, run.dungeon.depth, run.dungeon.pack, party, &mut run.rng);
        run.stock_gear_cache();
        run.populate_boss_gates();
        if pack.is_some() {
            // The relaid floor's own pen and games, as the stairs pick them.
            run.cage_someone();
            run.pick_folly();
            run.pick_gantry();
            run.pick_race3d();
        }
        run.at = 0;
        run.enter(0, None);
    }
    run
}

fn rooms_of(run: &Run, kind: RoomKind) -> Vec<usize> {
    run.dungeon
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == kind && !r.cleared)
        .map(|(i, _)| i)
        .collect()
}

/// The health of the bosses a room would hold when the party walks in.
fn room_boss(run: &Run, room: usize) -> u32 {
    let mut probe = run.clone();
    probe.enter(room, Some(2));
    probe
        .enemies
        .iter()
        .filter(|e| e.is_boss())
        .map(|e| e.max_hp)
        .sum()
}

/// One normal room (no boss in it), picked by seed, fought from full health.
fn normal_room(seed: u64, depth: u32, party: Party, n: usize) -> Fight {
    let mut run = floor_run(seed, depth, party, n);
    let weights: Vec<(usize, u32)> = rooms_of(&run, RoomKind::Fight)
        .into_iter()
        .map(|r| (r, room_boss(&run, r)))
        .collect();
    let least = weights.iter().map(|w| w.1).min().expect("fight rooms");
    let rooms: Vec<usize> = weights
        .into_iter()
        .filter(|w| w.1 == least)
        .map(|w| w.0)
        .collect();
    let room = rooms[(seed as usize * 7 + 3) % rooms.len()];
    run.enter(room, Some(2));
    run.calm_for_test();
    if std::env::var_os("BALANCE_ROSTERS").is_some() {
        let mut kinds: BTreeMap<String, u32> = BTreeMap::new();
        for e in &run.enemies {
            *kinds.entry(format!("{:?}", e.kind)).or_default() += 1;
        }
        println!("  depth {depth} seed {seed}: {kinds:?}");
    }
    fight(&mut run, 150)
}

/// The floor's boss fight: the room with the toughest boss in it, a
/// guardian's or a clan chief's.
fn boss_room(seed: u64, depth: u32, party: Party, n: usize) -> Fight {
    let mut run = floor_run(seed, depth, party, n);
    let room = (0..run.dungeon.rooms.len())
        .filter(|&r| {
            !run.dungeon.rooms[r].cleared
                && !matches!(run.dungeon.rooms[r].kind, RoomKind::Home | RoomKind::Wanted)
        })
        .max_by_key(|&r| room_boss(&run, r))
        .expect("a floor has rooms");
    run.enter(room, Some(2));
    run.calm_for_test();
    fight(&mut run, 240)
}

fn row(label: &str, fights: &[Fight]) {
    let n = fights.len() as f32;
    let avg = |f: fn(&Fight) -> f32| fights.iter().map(f).sum::<f32>() / n;
    let share = |f: fn(&Fight) -> bool| fights.iter().filter(|x| f(x)).count() as f32 * 100.0 / n;
    let lo = fights.iter().map(|f| f.secs).fold(f32::MAX, f32::min);
    let hi = fights.iter().map(|f| f.secs).fold(0.0, f32::max);
    let clears: Vec<&Fight> = fights.iter().filter(|f| f.cleared).collect();
    let clear_secs = if clears.is_empty() {
        "no clears".to_string()
    } else {
        format!(
            "{:.1}s",
            clears.iter().map(|f| f.secs).sum::<f32>() / clears.len() as f32
        )
    };
    println!(
        "{label:<26} foes {:>4.1} hp {:>5.0} boss {:>5.0} | clear {:>3.0}% {:>5.1}s ({:>4.1}-{:>5.1}) clears {clear_secs} | dmg {:>4.0}% heal {:>4.0}% | helm {:>3.0}% undies {:>3.0}% | deaths/fight {:.2} fights-with-death {:>3.0}%",
        avg(|f| f.foes as f32),
        avg(|f| f.foe_hp as f32),
        avg(|f| f.boss_hp as f32),
        share(|f| f.cleared),
        avg(|f| f.secs),
        lo,
        hi,
        avg(|f| f.dmg),
        avg(|f| f.heal),
        share(|f| f.helm),
        share(|f| f.undies),
        avg(|f| f.deaths as f32),
        share(|f| f.deaths > 0),
    );
}

const SEEDS: u64 = 24;

/// `cargo test --bin angel balance_sim -- --ignored --nocapture`
#[test]
#[ignore]
fn balance_sim() {
    println!();
    for party in [Party::Fresh, Party::Mid, Party::Won] {
        let run = floor_run(1, 1, party, 3);
        for hero in run.players.values() {
            println!(
                "{party:?} {} hp {} armor {} strength {:.1} bonus {:?}",
                hero.name,
                hero.max_hp,
                hero.armor,
                hero.strength(),
                hero.bonus
            );
        }
    }
    println!();
    for (n, party, depth, label) in [
        (1, Party::Fresh, 1, "fresh x1  floor 1"),
        (1, Party::Fresh, 2, "fresh x1  floor 2"),
        (3, Party::Fresh, 1, "fresh x3  floor 1"),
        (3, Party::Fresh, 2, "fresh x3  floor 2"),
        (3, Party::Mid, 3, "mid   x3  floor 3"),
        (3, Party::Won, 4, "won   x3  floor 4"),
        (3, Party::Won, 5, "won   x3  floor 5"),
        (3, Party::Won, 6, "won   x3  floor 6"),
        (1, Party::Won, 5, "won   x1  floor 5"),
    ] {
        let fights: Vec<Fight> = (0..SEEDS)
            .map(|s| normal_room(s, depth, party, n))
            .collect();
        row(&format!("{label} room"), &fights);
    }
    println!();
    for (n, party, depth, label) in [
        (3, Party::Fresh, 1, "fresh x3  floor 1"),
        (3, Party::Fresh, 2, "fresh x3  floor 2"),
        (3, Party::Mid, 3, "mid   x3  floor 3"),
        (3, Party::Won, 4, "won   x3  floor 4"),
        (3, Party::Won, 5, "won   x3  floor 5"),
        (3, Party::Won, 6, "won   x3  floor 6"),
        (1, Party::Won, 6, "won   x1  floor 6"),
    ] {
        stakes::HEALED.with(|h| *h.borrow_mut() = [0; 4]);
        let fights: Vec<Fight> = (0..SEEDS).map(|s| boss_room(s, depth, party, n)).collect();
        row(&format!("{label} BOSS"), &fights);
        println!(
            "    mended by rest/potion/party/passive: {:?}",
            stakes::HEALED.with(|h| *h.borrow())
        );
    }
}

/// The head count, room by room: the same rooms and bosses with one knight
/// to a full table of six, for how long a clear takes and what it costs.
/// `cargo test --bin angel party_size_sim -- --ignored --nocapture`
#[test]
#[ignore]
fn party_size_sim() {
    println!();
    for (party, depth, label) in [
        (Party::Fresh, 1, "fresh floor 1"),
        (Party::Fresh, 2, "fresh floor 2"),
        (Party::Mid, 3, "mid   floor 3"),
        (Party::Won, 4, "won   floor 4"),
    ] {
        for n in 1..=MAX_PLAYERS as usize {
            let fights: Vec<Fight> = (0..12).map(|s| normal_room(s, depth, party, n)).collect();
            row(&format!("{label} x{n} room"), &fights);
        }
    }
    println!();
    for (party, depth, label) in [
        (Party::Fresh, 2, "fresh floor 2"),
        (Party::Won, 4, "won   floor 4"),
    ] {
        for n in 1..=MAX_PLAYERS as usize {
            let fights: Vec<Fight> = (0..8).map(|s| boss_room(s, depth, party, n)).collect();
            row(&format!("{label} x{n} BOSS"), &fights);
        }
    }
}

// The area rule and the lifesteal limit (`aoe`, `stakes`), and the build
// that made them: on 2026-10-10 a party of morningstars and leeches stood in
// the thick of every room and nothing mattered.

/// Dame Thrudi built for a crowd: her twin axes and axe spin, two
/// morningstars, and every leech and mend the held cards allow.
const CROWD_DECK: &[&str] = &[
    "wish-morningstar-3",
    "wish-shoulder-demon-3",
    "leech-fang",
    "wish-leech-1",
    "wish-mend-3",
    "ember-quiver",
    "ember-quiver",
    "wish-heavy-1",
    "quick-draw",
    "quick-draw",
    "mail",
    "mail",
];
/// A bow knight's leech build: mends on every hit of a piercing pair of
/// arrows, a leech on every kill, two morningstars.
const GUNNER_DECK: &[&str] = &[
    "wish-mend-3",
    "leech-fang",
    "wish-twin-2",
    "bodkin-heads",
    "bodkin-heads",
    "wish-morningstar-3",
    "ember-quiver",
    "ember-quiver",
    "quick-draw",
    "quick-draw",
    "mail",
    "mail",
];
/// A bow and damage: one monster at a time.
const PLAIN_DECK: &[&str] = &[
    "ember-quiver",
    "ember-quiver",
    "wish-heavy-1",
    "quick-draw",
    "quick-draw",
    "mail",
    "mail",
];

/// A run on floor `depth` with `n` of the won run's knights, each re-armed
/// as `who` and holding `deck`.
fn built(seed: u64, depth: u32, n: usize, who: &str, deck: &[&str]) -> Run {
    let mut run = floor_run(seed, depth, Party::Won, n);
    let who = knights::knight(who).unwrap();
    let arm = run.book.get(who.arm).cloned().unwrap();
    let guard = run.book.get(who.guard).cloned().unwrap();
    for hero in run.players.values_mut() {
        hero.equip(&arm);
        hero.guard = guard.guard.unwrap_or_default();
        hero.guard_card = Some(guard.id.clone());
        hero.knight = Some(who.id.to_string());
        hero.deck = deck.iter().map(|c| c.to_string()).collect();
        hero.rebonus(&run.book);
        hero.hp = hero.max_hp;
        hero.winds = 0;
        hero.talisman = false;
    }
    run
}

/// A lone knight of `deck` in the middle of an emptied fight room, ringed
/// by `count` of the floor's plain monsters.
fn mobbed(seed: u64, depth: u32, who: &str, deck: &[&str], count: usize) -> Run {
    let mut run = built(seed, depth, 1, who, deck);
    let room = rooms_of(&run, RoomKind::Fight)[0];
    run.enter(room, Some(2));
    run.calm_for_test();
    run.enemies.clear();
    let (cx, cy) = (run.room().width() / 2.0, run.room().height() / 2.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (cx, cy);
    let kinds: Vec<EnemyKind> = run
        .dungeon
        .pack
        .roster_at(depth)
        .iter()
        .map(|&(kind, _)| kind)
        .filter(|kind| !kind.elite())
        .collect();
    for i in 0..count {
        let a = i as f32 * std::f32::consts::TAU / count as f32;
        let r = 3.0 + (i % 2) as f32 * 1.5;
        run.spawn_at_for_test(kinds[i % kinds.len()], cx + a.cos() * r, cy + a.sin() * r);
    }
    run.phase = Phase::Fighting;
    run
}

/// Into the thick of it: walk at the nearest monster, swing and spin, never
/// step back, never roll, never drink, never cast a spell it happened on.
fn tank_input(run: &Run, id: u32) -> Input {
    let mut input = bot_input(run, id);
    let hero = &run.players[&id];
    let near = run
        .enemies
        .iter()
        .filter(|e| e.hp > 0 && e.kind != EnemyKind::Dummy)
        .min_by(|a, b| {
            (a.x - hero.x)
                .hypot(a.y - hero.y)
                .total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
        });
    (input.move_x, input.move_y) = (0, 0);
    if let Some(e) = near {
        let (dx, dy) = (e.x - hero.x, e.y - hero.y);
        if dx.hypot(dy) > 1.2 {
            (input.move_x, input.move_y) = dir8(dx, dy);
        }
    }
    input.dash &= hero.guard == cards::Guard::Spin;
    input.play = 0;
    input.cast = 0;
    input.bomb = false;
    input
}

/// A mob fought to the end: the seconds until all but two of it had fallen
/// and until the last did (or the knight did, or `cap` seconds passed), the
/// health the knight lost, and whether it fell.
fn mob_fight(run: &mut Run, cap: u32) -> (f32, f32, u32, bool) {
    let start = run.tick;
    let (mut most, mut lost) = (None, 0);
    for _ in 0..cap * HZ {
        let input = tank_input(run, 1);
        let was = run.players[&1].hp;
        run.step(&BTreeMap::from([(1, input)]));
        lost += was.saturating_sub(run.players[&1].hp);
        let left = run.enemies.iter().filter(|e| e.hp > 0).count();
        if left <= 2 && most.is_none() {
            most = Some(run.tick);
        }
        if left == 0 || run.players[&1].hp == 0 {
            break;
        }
    }
    let secs = |t: u64| (t - start) as f32 / HZ as f32;
    (
        secs(most.unwrap_or(run.tick)),
        secs(run.tick),
        lost,
        run.players[&1].hp == 0,
    )
}

/// Swing at whatever is nearest, and nothing else: no step, no guard, no
/// draught, no bomb.
fn still_input(run: &Run, id: u32) -> Input {
    let mut input = bot_input(run, id);
    (input.move_x, input.move_y) = (0, 0);
    input.dash = false;
    input.play = 0;
    input.bomb = false;
    input
}

/// A lone knight of `deck` standing beside the floor's toughest boss, its
/// health made endless and waves coming in to feed the leech.
fn by_the_boss(seed: u64, depth: u32, who: &str, deck: &[&str]) -> Run {
    let mut run = built(seed, depth, 1, who, deck);
    // The floor's one boss holds its stairs: open the way to it.
    run.clear_prerequisites_for_test();
    let room = (0..run.dungeon.rooms.len())
        .filter(|&r| {
            !run.dungeon.rooms[r].cleared
                && !matches!(run.dungeon.rooms[r].kind, RoomKind::Home | RoomKind::Wanted)
        })
        .max_by_key(|&r| room_boss(&run, r))
        .expect("a floor has rooms");
    run.enter(room, Some(2));
    run.calm_for_test();
    run.waves.left = 3;
    run.waves.total = 3;
    let boss = run
        .enemies
        .iter_mut()
        .find(|e| e.is_boss())
        .expect("a boss");
    boss.max_hp *= 100;
    // Endless, and in the middle of its fight (`bosses::Stage`): a guardian
    // at full health holds to its opening's few shots, which no fight
    // stays in for long.
    boss.hp = boss.max_hp * 6 / 10;
    boss.seen_hp = boss.hp;
    let (bx, by) = (boss.x, boss.y);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (bx, by + 3.0);
    run
}

/// Knight 1 stands and swings until it falls or `cap` seconds pass: the
/// seconds it stood (None if it never fell), and its lifesteal and other
/// passive mends in that time.
fn stand(run: &mut Run, cap: u32) -> (Option<f32>, u32) {
    stakes::HEALED.with(|h| *h.borrow_mut() = [0; 4]);
    let start = run.tick;
    for _ in 0..cap * HZ {
        let input = still_input(run, 1);
        run.step(&BTreeMap::from([(1, input)]));
        if run.players[&1].hp == 0 {
            let secs = (run.tick - start) as f32 / HZ as f32;
            return (Some(secs), stakes::HEALED.with(|h| h.borrow()[3]));
        }
    }
    (None, stakes::HEALED.with(|h| h.borrow()[3]))
}

/// Run `f` with every stroke landing whole, as before the area rule.
fn whole<T>(f: impl FnOnce() -> T) -> T {
    aoe::WHOLE.with(|w| w.set(true));
    let out = f();
    aoe::WHOLE.with(|w| w.set(false));
    out
}

#[test]
fn one_stroke_lands_whole_on_the_three_nearest_and_a_quarter_past_them() {
    let mut run = Run::new(3, 1, None);
    run.calm_for_test();
    run.enemies.clear();
    for d in [4.0, 1.0, 6.0, 2.0, 5.0, 3.0] {
        run.spawn_at_for_test(EnemyKind::Skeleton, 10.0 + d, 10.0);
    }
    let before: Vec<u32> = run.enemies.iter().map(|e| e.hp).collect();
    let struck = aoe::strike(&mut run.enemies, (10.0, 10.0), 40, |_| true);
    assert_eq!(struck.len(), 6);
    let mut took: Vec<(f32, u32)> = run
        .enemies
        .iter()
        .zip(&before)
        .map(|(e, was)| (e.x, was - e.hp))
        .collect();
    took.sort_by(|a, b| a.0.total_cmp(&b.0));
    let took: Vec<u32> = took.into_iter().map(|t| t.1).collect();
    assert_eq!(took, [40, 40, 40, 10, 10, 10], "nearest first");
    // And one alone takes all of it, as it always did.
    let lone = aoe::strike(&mut run.enemies[..1], (10.0, 10.0), 40, |_| true);
    assert_eq!(lone[0].1, 40);
}

#[test]
fn the_axe_spin_cuts_the_three_nearest_whole_and_the_rest_a_quarter() {
    let mut run = Run::new(3, 1, None);
    run.calm_for_test();
    run.enemies.clear();
    run.outfit(1, knights::knight("ironbraid").unwrap());
    let (hx, hy) = (run.players[&1].x, run.players[&1].y);
    for k in 0..6 {
        let a = k as f32 * std::f32::consts::TAU / 6.0;
        let r = 1.2 + k as f32 * 0.3;
        run.spawn_at_for_test(EnemyKind::Skeleton, hx + a.cos() * r, hy + a.sin() * r);
    }
    for e in &mut run.enemies {
        (e.max_hp, e.hp, e.frozen) = (10_000, 10_000, 10_000);
    }
    run.phase = Phase::Fighting;
    let spin = Input {
        dash: true,
        ..Input::default()
    };
    run.step(&BTreeMap::from([(1, spin)]));
    let took: Vec<u32> = run.enemies.iter().map(|e| e.max_hp - e.hp).collect();
    let cut = run.players[&1].scaled(26);
    assert_eq!(
        took,
        [
            cut,
            cut,
            cut,
            cut.div_ceil(4),
            cut.div_ceil(4),
            cut.div_ceil(4)
        ]
    );
}

#[test]
fn morningstars_strike_on_a_beat_at_about_half_their_old_weight() {
    let mut run = Run::new(3, 1, None);
    run.calm_for_test();
    run.enemies.clear();
    let hero = run.players.get_mut(&1).unwrap();
    hero.bonus.orbit = 2;
    hero.invulnerable = 100_000;
    let (hx, hy) = (hero.x, hero.y);
    run.spawn_at_for_test(EnemyKind::Skeleton, hx + 1.8, hy);
    let e = &mut run.enemies[0];
    (e.max_hp, e.hp, e.frozen) = (1_000_000, 1_000_000, 1_000_000);
    run.phase = Phase::Fighting;
    for _ in 0..10 * HZ {
        let was = run.enemies[0].hp;
        run.step(&BTreeMap::new());
        if run.enemies[0].hp < was {
            assert!(
                run.tick.is_multiple_of(u64::from(ORBIT_EVERY)),
                "only on the beat"
            );
        }
        (run.players.get_mut(&1).unwrap().x, run.enemies[0].x) = (hx, hx + 1.8);
    }
    // Before the beat two morningstars took 64 a second off a monster
    // beside the knight, every tick they touched it.
    let a_second = (1_000_000 - run.enemies[0].hp) / 10;
    assert!((20..=45).contains(&a_second), "{a_second} a second");
}

#[test]
fn a_tick_of_kills_mends_once_and_lifesteal_stops_at_two_a_second() {
    // The first floor: every mend lands whole, and only this limit holds it.
    let mut run = Run::new(3, 1, None);
    run.calm_for_test();
    run.enemies.clear();
    let hero = run.players.get_mut(&1).unwrap();
    hero.bonus.vamp = 3;
    hero.hp = 20;
    hero.invulnerable = 100_000;
    let healed_over = |run: &mut Run, ticks: u32, kills: usize| {
        let mut healed = 0;
        for _ in 0..ticks {
            for k in 0..kills {
                run.spawn_at_for_test(EnemyKind::Skeleton, 30.0 + k as f32, 10.0);
            }
            run.phase = Phase::Fighting;
            for e in &mut run.enemies {
                e.hp = 0;
            }
            let was = run.players[&1].hp;
            run.step(&BTreeMap::new());
            healed += run.players[&1].hp - was;
        }
        healed
    };
    // Five fall to one blow: one leech, held to the second's two.
    let once = healed_over(&mut run, 1, 5);
    assert!((1..=stakes::LEECH_PER_SECOND).contains(&once), "{once}");
    // A kill every tick for three seconds: two a second at most.
    let three = healed_over(&mut run, 3 * HZ, 1);
    assert!(
        three <= 4 * stakes::LEECH_PER_SECOND,
        "{three} in three seconds"
    );
    assert!(
        three >= 2 * stakes::LEECH_PER_SECOND,
        "but it does mend: {three}"
    );
}

/// (a) The build that stood still and lived: beside an endless boss, a
/// knight that only swings falls, on an early floor (healing whole) and a
/// deep one, and its lifesteal never passes two a second.
#[test]
fn a_morningstar_and_leech_build_that_stands_beside_a_boss_falls() {
    for depth in [2, 6] {
        for (who, deck) in [("ironbraid", CROWD_DECK), ("percival", GUNNER_DECK)] {
            for seed in 0..4 {
                let mut run = by_the_boss(seed, depth, who, deck);
                let (fell, leeched) = stand(&mut run, 120);
                let Some(secs) = fell else {
                    panic!("{who} on floor {depth} (seed {seed}) stood 120 s and lived");
                };
                assert!(
                    leeched as f32 <= stakes::LEECH_PER_SECOND as f32 * (secs + 1.0),
                    "{who} floor {depth} seed {seed}: {leeched} mended in {secs} s"
                );
            }
        }
    }
}

/// Seeds the mob fight below averages over: enough that one floor's rooms
/// or one run's dice do not decide the mean.
const MOB_SEEDS: u64 = 64;

/// (b) Ringed by twenty-four, the crowd build cuts through them slower and
/// bleeds more than when every stroke landed whole.
#[test]
fn a_crowd_build_in_a_mob_clears_slower_than_when_every_stroke_landed_whole() {
    for depth in [4, 6] {
        let mean = |whole_strokes: bool| {
            let fights: Vec<(f32, f32, u32, bool)> = (0..MOB_SEEDS)
                .map(|seed| {
                    let mut run = mobbed(seed, depth, "ironbraid", CROWD_DECK, 24);
                    if whole_strokes {
                        whole(|| mob_fight(&mut run, 60))
                    } else {
                        mob_fight(&mut run, 60)
                    }
                })
                .collect();
            let secs = fights.iter().map(|f| f.0).sum::<f32>() / MOB_SEEDS as f32;
            let lost = fights.iter().map(|f| f.2 as f32).sum::<f32>() / MOB_SEEDS as f32;
            (secs, lost)
        };
        let ((then, bled), (now, bleeds)) = (mean(true), mean(false));
        println!(
            "floor {depth}: most of a mob of 24 down in {then:.1} s whole, {now:.1} s now; lost {bled:.0} then, {bleeds:.0} now"
        );
        assert!(
            now >= then * 1.25,
            "floor {depth}: {now:.1} s against {then:.1} s"
        );
        // A quarter more blood at the least, as for the time: over these
        // seeds floor 4 bleeds about 1.4 times as much, and new dice alone
        // (another raid's) take a sixteen-seed mean from 1.8 to 1.4, so half
        // again was the luck of one sample, not the rule.
        assert!(
            bleeds >= bled * 1.25,
            "floor {depth}: lost {bleeds:.0} against {bled:.0}"
        );
    }
}

/// (c) One monster alone is untouched by the area rule, even with three
/// knights firing. Boss rooms now have extra co-op escorts, so isolate the
/// single target rather than assuming their whole fight is single-target.
#[test]
fn single_target_play_is_untouched_by_the_area_rule() {
    for depth in [2, 5] {
        for seed in 0..3 {
            let play = |whole_strokes: bool| {
                let mut run = built(seed, depth, 3, "percival", PLAIN_DECK);
                let room = rooms_of(&run, RoomKind::Fight)[0];
                run.enter(room, Some(2));
                run.calm_for_test();
                run.enemies.clear();
                run.spawn_at_for_test(EnemyKind::Skeleton, 24.0, 14.0);
                let f = if whole_strokes {
                    whole(|| fight(&mut run, 120))
                } else {
                    fight(&mut run, 120)
                };
                (
                    f.secs,
                    f.dmg,
                    f.cleared,
                    run.enemies.iter().map(|e| e.hp).sum::<u32>(),
                )
            };
            assert_eq!(play(false), play(true), "floor {depth} seed {seed}");
        }
    }
}

/// Before and after, for tuning: `cargo test --bin angel aoe_sim -- --ignored --nocapture`.
#[test]
#[ignore]
fn aoe_sim() {
    for depth in [2, 4, 6] {
        for whole_strokes in [true, false] {
            let fights: Vec<(f32, f32, u32, bool)> = (0..16)
                .map(|seed| {
                    let mut run = mobbed(seed, depth, "ironbraid", CROWD_DECK, 24);
                    if whole_strokes {
                        whole(|| mob_fight(&mut run, 60))
                    } else {
                        mob_fight(&mut run, 60)
                    }
                })
                .collect();
            let n = fights.len() as f32;
            println!(
                "mob of 24, floor {depth}, {}: most fallen {:.1} s, all {:.1} s, lost {:.0}, fell {:.0}%",
                if whole_strokes { "whole" } else { "rule " },
                fights.iter().map(|f| f.0).sum::<f32>() / n,
                fights.iter().map(|f| f.1).sum::<f32>() / n,
                fights.iter().map(|f| f.2 as f32).sum::<f32>() / n,
                fights.iter().filter(|f| f.3).count() as f32 * 100.0 / n,
            );
        }
        for (who, deck) in [("ironbraid", CROWD_DECK), ("percival", GUNNER_DECK)] {
            let line: Vec<String> = (0..8)
                .map(|seed| {
                    let mut run = by_the_boss(seed, depth, who, deck);
                    let (fell, leeched) = stand(&mut run, 120);
                    format!(
                        "{}/{leeched}",
                        fell.map_or("lived".into(), |s| format!("{s:.0}s"))
                    )
                })
                .collect();
            println!("standing {who} floor {depth}: {}", line.join(" "));
        }
    }
}

/// One of the swamp's fights, picked by seed: a plain fight room, or the
/// floor's stairs hall (on its last floor, the Mire King's).
fn swamp_fight(seed: u64, depth: u32, party: Party, n: usize, hall: bool) -> Fight {
    let mut run = floor_run_in(seed, depth, party, n, Some(Pack::Swamp));
    let room = if hall {
        rooms_of(&run, RoomKind::Stairs)[0]
    } else {
        let rooms = rooms_of(&run, RoomKind::Fight);
        rooms[(seed as usize * 7 + 3) % rooms.len()]
    };
    run.enter(room, Some(2));
    run.calm_for_test();
    if std::env::var_os("BALANCE_ROSTERS").is_some() {
        let mut kinds: BTreeMap<String, u32> = BTreeMap::new();
        for e in &run.enemies {
            *kinds.entry(format!("{:?}", e.kind)).or_default() += 1;
        }
        println!("  swamp {depth} seed {seed} x{n}: {kinds:?}");
    }
    fight(&mut run, if hall { 240 } else { 150 })
}

/// The Swamp of Horrors, knight by knight: its three floors' fight rooms
/// and stairs halls (the third's is the Mire King's) with one to four
/// knights, fresh on the first two floors and part-way on the third, as the
/// main delve's sims meet them.
/// `cargo test -p angel-delve swamp_party_sim -- --ignored --nocapture`
#[test]
#[ignore]
fn swamp_party_sim() {
    println!();
    for (party, depth, label) in [
        (Party::Fresh, 1, "swamp fresh floor 1"),
        (Party::Fresh, 2, "swamp fresh floor 2"),
        (Party::Mid, 3, "swamp mid   floor 3"),
    ] {
        for n in 1..=4 {
            let fights: Vec<Fight> = (0..12).map(|s| swamp_fight(s, depth, party, n, false)).collect();
            row(&format!("{label} x{n} room"), &fights);
        }
        for n in 1..=4 {
            let fights: Vec<Fight> = (0..8).map(|s| swamp_fight(s, depth, party, n, true)).collect();
            let what = if depth == crate::rules::swamp::FLOORS { "KING" } else { "HALL" };
            row(&format!("{label} x{n} {what}"), &fights);
        }
    }
}

// The shots and the cover (2026-10-14): what the monsters throw, measured,
// and a party's fights before and after a change to either.

/// What one thrower's shots were over a stretch: each kind seen, the
/// speeds seen in flight (units a second), the widest strike (units past a
/// knight's own body), and how many were loosed.
#[derive(Default)]
struct Census {
    kinds: std::collections::BTreeSet<String>,
    forms: std::collections::BTreeSet<String>,
    speeds: Vec<f32>,
    girth: f32,
    loosed: u64,
}

impl Census {
    fn take(&mut self, run: &Run) {
        for p in run.projectiles.iter().filter(|p| p.hostile && p.in_flight()) {
            self.kinds.insert(format!("{:?}", p.kind));
            self.forms.insert(patterns::family(p).name().to_string());
            self.speeds.push(p.vx.hypot(p.vy));
            self.girth = self.girth.max(patterns::girth(p));
        }
    }

    fn line(&mut self, who: &str) -> String {
        self.speeds.sort_by(f32::total_cmp);
        let at = |q: f32| {
            self.speeds
                .get(((self.speeds.len() as f32 - 1.0) * q).round() as usize)
                .copied()
                .unwrap_or(0.0)
        };
        format!(
            "| {who} | {} | {} | {:.1} / {:.1} / {:.1} | {:.2} | {} |",
            self.kinds.iter().cloned().collect::<Vec<_>>().join(" "),
            self.forms.iter().cloned().collect::<Vec<_>>().join(" "),
            at(0.05),
            at(0.5),
            at(0.95),
            self.girth,
            self.loosed,
        )
    }
}

/// Every monster kind of every pack (plain and elite) and every guardian,
/// alone in a room for twenty seconds against a knight who cannot fall:
/// the kinds, families and speeds it throws.
/// `cargo test shot_census -- --ignored --nocapture`
#[test]
#[ignore]
fn shot_census() {
    println!("| thrower | shot kinds | families | speed p5 / p50 / p95 | strike past body | loosed in 20 s |");
    println!("|---|---|---|---|---|---|");
    let packs = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown];
    for pack in packs {
        let mut kinds: Vec<EnemyKind> = Vec::new();
        for depth in [1, 3, 6] {
            for &(kind, _) in pack.roster_at(depth) {
                if !kinds.contains(&kind) {
                    kinds.push(kind);
                }
            }
        }
        for kind in kinds {
            for elite in [false, true] {
                let mut run = Run::new(4, 1, None);
                let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
                run.enter_for_test(room);
                run.dungeon.pack = pack;
                run.dungeon.depth = 3;
                run.calm_for_test();
                run.enemies.clear();
                run.projectiles.clear();
                run.spawn_at_for_test(kind, 24.0, 8.0);
                if let Some(e) = run.enemies.last_mut() {
                    e.elite = elite;
                }
                let hero = run.players.get_mut(&1).unwrap();
                (hero.x, hero.y) = (24.0, 20.0);
                hero.invulnerable = 1_000_000;
                let mut census = Census::default();
                let before = run.hostile_shots;
                for _ in 0..20 * HZ {
                    for e in &mut run.enemies {
                        e.hp = e.max_hp.max(1);
                    }
                    run.step(&BTreeMap::new());
                    census.take(&run);
                }
                census.loosed = run.hostile_shots - before;
                if census.loosed > 0 {
                    let who = format!("{pack:?} {kind:?}{}", if elite { " elite" } else { "" });
                    println!("{}", census.line(&who));
                }
            }
        }
    }
    for pack in packs {
        let mut probe = Run::new(7, 1, None);
        let count = probe.pack_bosses_for_test(pack).len();
        for k in 0..count {
            let mut run = Run::new(7, 1, None);
            let stairs = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
            run.enter_for_test(stairs);
            let index = run.pack_bosses_for_test(pack)[k];
            run.enemies.clear();
            run.projectiles.clear();
            run.calm_for_test();
            let (w, h) = (run.room().width(), run.room().height());
            run.spawn_boss_at_for_test(index, w / 2.0, 7.0);
            let name = run.bosses[index].id.clone();
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = (w / 2.0 - 8.0, (h - 5.0).min(16.0));
            hero.invulnerable = 1_000_000;
            let mut census = Census::default();
            let before = run.hostile_shots;
            for t in 0..30 * HZ {
                let share = [1.0, 0.6, 0.12][(t / (10 * HZ)) as usize];
                for boss in run.enemies.iter_mut().filter(|e| e.kind == EnemyKind::Boss) {
                    boss.hp = ((boss.max_hp as f32 * share) as u32).max(1);
                }
                run.step(&BTreeMap::new());
                census.take(&run);
            }
            census.loosed = run.hostile_shots - before;
            println!("{}", census.line(&format!("{pack:?} boss {name}")));
        }
    }
}

/// A bot that stands where it is and shoots: aims and fires as `bot_input`
/// does, and never takes a step.
fn standing_input(run: &Run, id: u32) -> Input {
    let mut input = bot_input(run, id);
    input.move_x = 0;
    input.move_y = 0;
    input.dash = false;
    input
}

/// `fight` with standing knights.
fn stand_fight(run: &mut Run, cap: u32) -> Fight {
    let pool: u32 = run.players.values().map(|h| h.max_hp).sum();
    let (mut lost, mut fell) = (0u32, std::collections::BTreeSet::new());
    let start = run.tick;
    for _ in 0..cap * HZ {
        let inputs: BTreeMap<u32, Input> =
            run.players.keys().map(|&id| (id, standing_input(run, id))).collect();
        let before: BTreeMap<u32, (u32, u32)> =
            run.players.iter().map(|(&id, h)| (id, (h.hp, h.max_hp))).collect();
        run.step(&inputs);
        for (&id, hero) in &run.players {
            let (was, max) = before[&id];
            if hero.max_hp == max && hero.hp < was {
                lost += was - hero.hp;
            }
            if hero.hp == 0 {
                fell.insert(id);
            }
        }
        if run.phase != Phase::Fighting {
            break;
        }
    }
    Fight {
        secs: (run.tick - start) as f32 / HZ as f32,
        cleared: run.phase != Phase::Fighting && run.phase != Phase::Wiped,
        dmg: lost as f32 * 100.0 / pool.max(1) as f32,
        deaths: fell.len() as u32,
        ..Fight::default()
    }
}

/// Parties of two and four, standing and moving, through a normal room of
/// every floor and every floor's boss: seconds a fight and the share of
/// the party's health lost, the numbers a change to shots or cover must
/// keep within a quarter of.
/// `cargo test shots_party_check -- --ignored --nocapture`
#[test]
#[ignore]
fn shots_party_check() {
    let seeds: u64 = std::env::var("PARTY_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(8);
    let party = |depth: u32| match depth {
        1 | 2 => Party::Fresh,
        3 => Party::Mid,
        _ => Party::Won,
    };
    for n in [2usize, 4] {
        for moving in [false, true] {
            let (mut secs, mut dmg, mut count, mut clears, mut deaths) = (0.0f32, 0.0f32, 0u32, 0u32, 0u32);
            let (mut bsecs, mut bdmg, mut bcount, mut bclears) = (0.0f32, 0.0f32, 0u32, 0u32);
            for depth in 1..=6 {
                for seed in 0..seeds {
                    let mut run = floor_run(seed, depth, party(depth), n);
                    let rooms = rooms_of(&run, RoomKind::Fight);
                    let room = rooms[(seed as usize * 7 + 3) % rooms.len()];
                    run.enter(room, Some(2));
                    run.calm_for_test();
                    let f = if moving { fight(&mut run, 150) } else { stand_fight(&mut run, 150) };
                    secs += f.secs;
                    dmg += f.dmg;
                    count += 1;
                    clears += u32::from(f.cleared);
                    deaths += f.deaths;
                }
                for seed in 0..seeds.div_ceil(2) {
                    let mut run = floor_run(seed * 13 + u64::from(depth), depth, party(depth), n);
                    let room = (0..run.dungeon.rooms.len())
                        .filter(|&r| {
                            !run.dungeon.rooms[r].cleared
                                && !matches!(run.dungeon.rooms[r].kind, RoomKind::Home | RoomKind::Wanted)
                        })
                        .max_by_key(|&r| room_boss(&run, r))
                        .unwrap();
                    run.enter(room, Some(2));
                    run.calm_for_test();
                    let f = if moving { fight(&mut run, 240) } else { stand_fight(&mut run, 240) };
                    bsecs += f.secs;
                    bdmg += f.dmg;
                    bcount += 1;
                    bclears += u32::from(f.cleared);
                }
            }
            let c = count as f32;
            let b = bcount as f32;
            println!(
                "x{n} {:<8} rooms: {:.1} s, lost {:.1}%, cleared {clears}/{count}, fell {deaths} | bosses: {:.1} s, lost {:.1}%, cleared {bclears}/{bcount}",
                if moving { "moving" } else { "standing" },
                secs / c,
                dmg / c,
                bsecs / b,
                bdmg / b,
            );
        }
    }
}

// ===========================================================================
// PARTY CHECK (test-only, UNCOMMITTED): a 2-4 knight naked-start party sim for
// the pre-deploy check. Prints `PCROW,...` lines; see /work/artifacts/world3d/
// party-check/. Env: PC_DEPTHS PC_NS PC_SEEDS PC_SECS PC_OPEN PC_HOST_PLAYS.
// ===========================================================================

fn pc_list(name: &str, default: &[u32]) -> Vec<u32> {
    std::env::var(name)
        .ok()
        .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect::<Vec<u32>>())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_vec())
}

const PC_NAMES: [&str; 5] = ["Wren", "Bors", "Ysolde", "Kay", "Tor"];

/// A naked-start party of `knights` playing on floor `depth` of the delve's
/// own (closed) floors, the host's seat empty (`absent_host`: as the live box
/// is) or playing.
fn pc_naked(seed: u64, depth: u32, knights: usize, absent_host: bool) -> Run {
    let mut run = Run::at_home(
        seed,
        700 + seed,
        None,
        crate::rules::home::Home::default(),
        crate::realm::Spoils::default(),
    );
    run.naked_start = true;
    for id in run.players.keys().copied().collect::<Vec<_>>() {
        run.restore_gear(id);
    }
    let last = if absent_host { knights as u32 + 1 } else { knights as u32 };
    for id in 2..=last {
        run.join_seat(id, PC_NAMES[(id as usize - 2) % PC_NAMES.len()]);
    }
    if absent_host {
        run.set_absent(1, true);
    }
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    assert_eq!(run.party_len(), knights);
    run
}

/// The same party on an open floor (the live box lays every floor open).
fn pc_naked_open(seed: u64, depth: u32, knights: usize, absent_host: bool) -> Run {
    let armed = std::env::var_os("PC_ARMED").is_some();
    let mut run = Run::new(seed, 700 + seed, None);
    run.naked_start = !armed;
    run.restore_gear(1);
    let last = if absent_host { knights as u32 + 1 } else { knights as u32 };
    for id in 2..=last {
        run.join_seat(id, PC_NAMES[(id as usize - 2) % PC_NAMES.len()]);
    }
    if armed {
        // The intro's three knights, dressed as the balance harness dresses them.
        let ids: Vec<u32> = run.players.keys().copied().collect();
        for (i, id) in ids.into_iter().enumerate() {
            run.outfit(id, knights::knight(["percival", "lynette", "gareth", "galahad"][i % 4]).unwrap());
            let hero = run.players.get_mut(&id).unwrap();
            hero.invulnerable = 0;
            hero.hp = hero.max_hp;
        }
    }
    if absent_host {
        run.set_absent(1, true);
    }
    run.open_floor_for_test(depth);
    assert_eq!(run.party_len(), knights);
    run
}

#[derive(Clone, Debug, Default)]
struct PcStat {
    /// The most one knight lost in a single tick, percent of its health.
    maxhit: u32,
    /// The most one knight lost in any second, percent of its health.
    max1s: u32,
    /// The party's whole loss, percent of its pool.
    lost: u32,
    fell: u32,
    wiped: Option<u32>,
    cleared: Option<u32>,
    /// A pavise was in the room at some tick (one spawned late counts).
    sawpav: bool,
}

/// Play `secs` of the room the party stands in. `walk`: paced steps with the
/// shots of `bot_input`; not `walk`: standing still, shooting. `bot`: the
/// whole bot (kites, dodges).
fn pc_play(run: &mut Run, walk: bool, bot: bool, secs: u32, stop_on_clear: bool) -> PcStat {
    use std::collections::VecDeque;
    let ids: Vec<u32> = run.players.iter().filter(|(_, h)| !h.absent).map(|(&id, _)| id).collect();
    let pool: u32 = ids.iter().map(|i| run.players[i].max_hp).sum();
    let mut st = PcStat::default();
    let mut win: BTreeMap<u32, VecDeque<u32>> = ids.iter().map(|&i| (i, VecDeque::new())).collect();
    let mut dead = std::collections::BTreeSet::new();
    let mut lost = 0u32;
    let mut down_for = 0u32;
    for t in 0..secs * HZ {
        let inputs: BTreeMap<u32, Input> = ids
            .iter()
            .map(|&id| {
                let mut i = bot_input(run, id);
                if !bot {
                    i.dash = false;
                    i.move_x = if walk { ((t / 70 + id) % 3) as i8 - 1 } else { 0 };
                    i.move_y = if walk { if (t / 45) % 2 == 0 { -1 } else { 1 } } else { 0 };
                }
                (id, i)
            })
            .collect();
        let before: BTreeMap<u32, (u32, u32)> = ids.iter().map(|i| (*i, (run.players[i].hp, run.players[i].max_hp))).collect();
        run.step(&inputs);
        st.sawpav |= run.enemies.iter().any(|e| e.kind == EnemyKind::Pavise && e.hp > 0);
        for &id in &ids {
            let Some(h) = run.players.get(&id) else { continue };
            let (was, max) = before[&id];
            let d = if h.max_hp == max { was.saturating_sub(h.hp) } else { 0 };
            lost += d;
            st.maxhit = st.maxhit.max(d * 100 / max.max(1));
            let w = win.get_mut(&id).unwrap();
            w.push_back(d);
            if w.len() > HZ as usize {
                w.pop_front();
            }
            st.max1s = st.max1s.max(w.iter().sum::<u32>() * 100 / max.max(1));
            if h.hp == 0 {
                dead.insert(id);
            }
        }
        // The invariant the 10-14 hang broke: every knight a player plays
        // down is a wipe, whoever sits in the host's empty seat.
        let all_down = ids.iter().all(|i| run.players.get(i).is_none_or(|h| h.hp == 0));
        down_for = if all_down { down_for + 1 } else { 0 };
        if down_for > HZ && run.phase != Phase::Wiped && run.phase != Phase::Won {
            panic!("HANG: every played knight is down at tick {t} and the phase is {:?}", run.phase);
        }
        if run.phase == Phase::Wiped {
            st.wiped = Some(t);
            break;
        }
        if stop_on_clear && run.phase != Phase::Fighting {
            st.cleared = Some(t);
            break;
        }
    }
    st.lost = lost * 100 / pool.max(1);
    st.fell = dead.len() as u32;
    st
}

fn pc_row(set: &str, depth: u32, n: usize, mode: &str, seed: u64, room: &str, pav: usize, foes: usize, s: &PcStat) {
    println!(
        "PCROW,{set},{depth},{n},{mode},{seed},{room},{pav},{foes},{},{},{},{},{},{},{}",
        s.maxhit,
        s.max1s,
        s.lost,
        s.fell,
        s.wiped.map_or(-1, i64::from),
        s.cleared.map_or(-1, i64::from),
        u8::from(s.sawpav),
    );
}

/// Naked-start parties of 2-4, standing and walking, in every room (or, with
/// PC_OPEN set, every chamber) of floors 1-6: the most one knight lost to a
/// single hit and to any second, and any wipe.
/// `cargo test -p angel-delve --lib party_check -- --ignored --nocapture`
#[test]
#[ignore]
fn party_check() {
    let depths = pc_list("PC_DEPTHS", &[1, 2, 3, 4, 5, 6]);
    let ns = pc_list("PC_NS", &[2, 3, 4]);
    let seeds = pc_list("PC_SEEDS", &[0, 1]);
    let secs = pc_list("PC_SECS", &[20])[0];
    let absent = std::env::var_os("PC_HOST_PLAYS").is_none();
    let open = std::env::var_os("PC_OPEN").is_some();
    let armed = std::env::var_os("PC_ARMED").is_some();
    let set = if open && armed { "open-armed" } else if open { "open" } else if armed { "armed" } else { "closed" };
    for &depth in &depths {
        for &n in &ns {
            for &seed in &seeds {
                if open {
                    let base = pc_naked_open(u64::from(seed), depth, n as usize, absent);
                    let o = base.dungeon.open.clone().unwrap();
                    for (ci, c) in o.chambers.iter().enumerate() {
                        if c.roster.is_empty() {
                            continue;
                        }
                        let pav = c.roster.iter().filter(|k| **k == EnemyKind::Pavise).count();
                        let modes: &[(&str, bool, bool)] =
                            if armed { &[("stand", false, false), ("walk", true, false), ("bot", false, true)] } else { &[("stand", false, false), ("walk", true, false)] };
                        for &(mode, walk, bot) in modes {
                            let mut run = base.clone();
                            let ids: Vec<u32> = run.players.iter().filter(|(_, h)| !h.absent).map(|(&i, _)| i).collect();
                            let spots = run.open_spots(o.map, c.centre(), ids.len());
                            for (id, (x, y)) in ids.iter().zip(spots) {
                                let h = run.players.get_mut(id).unwrap();
                                (h.x, h.y) = (x, y);
                            }
                            let foes = c.roster.len();
                            let st = pc_play(&mut run, walk, bot, secs, false);
                            pc_row(set, depth, n as usize, mode, u64::from(seed), &format!("c{ci}:{:?}", c.kind), pav, foes, &st);
                        }
                    }
                } else {
                    let base = if armed {
                        floor_run(u64::from(seed), depth, Party::Fresh, n as usize)
                    } else {
                        pc_naked(u64::from(seed), depth, n as usize, absent)
                    };
                    for room in 0..base.dungeon.rooms.len() {
                        let mut probe = base.clone();
                        probe.enter(room, Some(2));
                        if probe.phase != Phase::Fighting || probe.enemies.is_empty() {
                            continue;
                        }
                        let kind = format!("r{room}:{:?}", probe.dungeon.rooms[room].kind);
                        let pav = probe.enemies.iter().filter(|e| e.kind == EnemyKind::Pavise).count();
                        let foes = probe.enemies.len();
                        let modes: &[(&str, bool, bool)] =
                            if armed { &[("stand", false, false), ("walk", true, false), ("bot", false, true)] } else { &[("stand", false, false), ("walk", true, false)] };
                        for &(mode, walk, bot) in modes {
                            let mut run = probe.clone();
                            let st = pc_play(&mut run, walk, bot, secs, true);
                            pc_row(set, depth, n as usize, mode, u64::from(seed), &kind, pav, foes, &st);
                        }
                    }
                }
            }
        }
    }
}

/// Armed (intro-dressed) knights against pavises: 2-4 knights, floors 2-6,
/// two and four pavises put before them in a fight room, standing and with
/// the whole bot. What the pavise rule (a shield knocked aside by what it
/// turns) changes: clear time and what the party lost.
/// `cargo test -p angel-delve --lib party_check_pavise -- --ignored --nocapture`
#[test]
#[ignore]
fn party_check_pavise() {
    let depths = pc_list("PC_DEPTHS", &[2, 3, 4, 5, 6]);
    let ns = pc_list("PC_NS", &[2, 3, 4]);
    let seeds = pc_list("PC_SEEDS", &[0, 1, 2]);
    for &depth in &depths {
        for &n in &ns {
            for &seed in &seeds {
                for k in [2usize, 4] {
                    for (mode, bot) in [("stand", false), ("bot", true)] {
                        let mut run = floor_run(u64::from(seed), depth, Party::Fresh, n as usize);
                        let rooms = rooms_of(&run, RoomKind::Fight);
                        let room = rooms[(seed as usize * 7 + 3) % rooms.len()];
                        run.enter(room, Some(2));
                        run.calm_for_test();
                        run.enemies.clear();
                        let (w, h) = (run.room().width(), run.room().height());
                        let spots = run.open_spots(run.at, (w * 0.62, h * 0.5), k);
                        for (x, y) in spots {
                            run.spawn_at_for_test(EnemyKind::Pavise, x, y);
                            let e = run.enemies.last_mut().unwrap();
                            e.age = TELEGRAPH + 5;
                            e.hp = e.max_hp;
                            e.seen_hp = e.max_hp;
                        }
                        let foes = run.enemies.len();
                        let st = pc_play(&mut run, false, bot, 45, true);
                        pc_row("pavise", depth, n as usize, mode, u64::from(seed), &format!("k{k}"), k, foes, &st);
                    }
                }
            }
        }
    }
}

/// Wipe and stuck states with the host's absent seat present.
/// `cargo test -p angel-delve --lib party_check_wipes -- --ignored --nocapture`
#[test]
#[ignore]
fn party_check_wipes() {
    let wiped_within = |run: &mut Run, ticks: u32| -> Option<u32> {
        for t in 0..ticks {
            run.step(&BTreeMap::new());
            if run.phase == Phase::Wiped {
                return Some(t);
            }
        }
        None
    };
    for depth in [1u32, 3, 6] {
        for n in [2usize, 3, 4] {
            // 1. Every played knight down at once, the empty seat alive (stone,
            //    away, absent): a wipe within a few ticks.
            let mut base = pc_naked(3, depth, n, true);
            let fight = (0..base.dungeon.rooms.len())
                .find(|&r| {
                    let mut p = base.clone();
                    p.enter(r, Some(2));
                    p.phase == Phase::Fighting && !p.enemies.is_empty()
                })
                .expect("a fighting room");
            base.enter(fight, Some(2));
            assert!(base.players[&1].absent && base.players[&1].hp > 0);
            let mut run = base.clone();
            for (&id, h) in run.players.iter_mut() {
                if id != 1 {
                    h.hp = 0;
                }
            }
            let at = wiped_within(&mut run, 5);
            println!("PCWIPE,all-down-host-alive,{depth},{n},{at:?}");
            assert!(at.is_some(), "all down, host seat alive, depth {depth} x{n}: not wiped");
            // ... and the host dead too.
            let mut run = base.clone();
            for h in run.players.values_mut() {
                h.hp = 0;
            }
            let at = wiped_within(&mut run, 5);
            println!("PCWIPE,all-down-host-dead,{depth},{n},{at:?}");
            assert!(at.is_some());
            // 2. All but one down: not a wipe; then the last falls: a wipe.
            let mut run = base.clone();
            let ids: Vec<u32> = run.players.keys().copied().filter(|&i| i != 1).collect();
            for &id in &ids[1..] {
                run.players.get_mut(&id).unwrap().hp = 0;
            }
            run.players.get_mut(&ids[0]).unwrap().hp = 100_000;
            run.players.get_mut(&ids[0]).unwrap().max_hp = 100_000;
            let at = wiped_within(&mut run, 90);
            println!("PCWIPE,one-stands,{depth},{n},{at:?}");
            assert!(at.is_none(), "one knight stands: no wipe (depth {depth} x{n})");
            run.players.get_mut(&ids[0]).unwrap().hp = 0;
            let at = wiped_within(&mut run, 5);
            assert!(at.is_some(), "the last falls: wipe");
            // 3. The verdict stands, and the party goes back: the empty seat
            //    holds nobody (again_due with nobody asking and with the
            //    friends asking).
            let mut asked = run.clone();
            for &id in &ids {
                asked.ask_again(id);
            }
            let mut quick = None;
            for t in 0..40 * HZ {
                asked.step(&BTreeMap::new());
                if asked.again_due() {
                    quick = Some(t);
                    break;
                }
            }
            let mut slow = None;
            for t in 0..40 * HZ {
                run.step(&BTreeMap::new());
                if run.again_due() {
                    slow = Some(t);
                    break;
                }
            }
            println!("PCWIPE,again-due,{depth},{n},asked={quick:?},nobody-asked={slow:?}");
            assert!(quick.is_some() && slow.is_some(), "the verdict never lets the party go");
            // 4. A friend away (stone) while the others fall: by the rule it
            //    keeps the party up; the game still runs.
            let mut run = base.clone();
            let ids: Vec<u32> = run.players.keys().copied().filter(|&i| i != 1).collect();
            run.set_away(ids[0], true);
            for &id in &ids[1..] {
                run.players.get_mut(&id).unwrap().hp = 0;
            }
            let at = wiped_within(&mut run, 120);
            println!("PCWIPE,friend-away-others-down,{depth},{n},{at:?}");
        }
    }
    // 5. An open floor, the empty seat present.
    for n in [2usize, 3] {
        let mut run = pc_naked_open(5, 2, n, true);
        for (&id, h) in run.players.iter_mut() {
            if id != 1 {
                h.hp = 0;
            }
        }
        let at = wiped_within(&mut run, 5);
        println!("PCWIPE,open-all-down,2,{n},{at:?}");
        assert!(at.is_some(), "open floor, all down: not wiped");
    }
}

/// The wishes each knight of the 2026-10-18 play-test made, in the order
/// they made them (`fixtures/last_game_phrasebook.txt` is that workspace's
/// phrasebook, as the wish-writer wrote it).
const LAST_GAME_WISHES: [&[&str]; 2] = [
    &[
        "homing-bullets", "sperm-bullets", "draining-bullets", "faster-firing", "tuxedo",
        "bigger-bullets", "health-drain", "trump-bullets", "more-damage", "damage-over-time",
        "pass-through", "life-drain", "penta-volley", "fly-faster", "pink-pony", "adoring-fans",
        "blocking-bullets", "heavy-hits",
    ],
    &[
        "fartjuice", "larger-feet", "silence-of-turds", "exploding-frogs", "fleet-foot",
        "formal-wear", "powerbottom", "poop", "penta-shot", "rollerblades-of-fire",
        "heavy-poop-heads", "poop-worms", "poop-armor", "corn-minions", "corn-minion-turret",
    ],
];

/// The last game's phrasebook, read as the host reads a workspace's.
fn last_game_book() -> Vec<phrasebook::Wish> {
    let text = include_str!("fixtures/last_game_phrasebook.txt");
    phrasebook::read(&format!("{}\n{text}", phrasebook::LEARNED))
}

/// Wish steps a knight has made on reaching floor `depth`: the start's
/// three, two a floor below, and a super wish (two steps) a floor's boss.
fn steps_by(depth: u32) -> usize {
    let below = depth.saturating_sub(1) as usize;
    usize::from(overclass::START_WISHES) + below * usize::from(overclass::FLOOR_WISHES) + below * 2
}

/// `n` intro-dressed knights on floor `depth`, each having wished the last
/// game's wishes (knight one's list, then two's, by turns) for every step
/// the floors so far gave: new wishes first, then each climbs again.
fn wished_run(seed: u64, depth: u32, n: usize, wished: bool) -> Run {
    let mut run = floor_run(seed, depth, Party::Fresh, n);
    if !wished {
        return run;
    }
    let book = last_game_book();
    let ids: Vec<u32> = run.players.keys().copied().collect();
    for (i, id) in ids.into_iter().enumerate() {
        let list = LAST_GAME_WISHES[i % 2];
        let mut steps = steps_by(depth);
        'climb: for _round in 0..3 {
            for wish in list.iter().filter_map(|w| book.iter().find(|b| b.id == *w)) {
                if steps == 0 {
                    break 'climb;
                }
                // WISH_ONLY=hold|spell: that kind of wish only, to see what each gives.
                if std::env::var("WISH_ONLY").is_ok_and(|k| k != wish.kind) {
                    continue;
                }
                if run.grant(id, wish).is_ok() {
                    steps -= 1;
                }
            }
        }
        let hero = run.players.get_mut(&id).unwrap();
        hero.hp = hero.max_hp;
    }
    // Held to the floor's budget, as the first tick would.
    run.keep_wish_floor();
    run
}

/// One room of `run`'s floor fought from full health: the floor's lightest
/// plain room, or (`boss`) its toughest boss.
fn wish_fight(mut run: Run, seed: u64, boss: bool) -> Fight {
    let room = if boss {
        (0..run.dungeon.rooms.len())
            .filter(|&r| {
                !run.dungeon.rooms[r].cleared
                    && !matches!(run.dungeon.rooms[r].kind, RoomKind::Home | RoomKind::Wanted)
            })
            .max_by_key(|&r| room_boss(&run, r))
            .expect("a floor has rooms")
    } else {
        let weights: Vec<(usize, u32)> = rooms_of(&run, RoomKind::Fight)
            .into_iter()
            .map(|r| (r, room_boss(&run, r)))
            .collect();
        let least = weights.iter().map(|w| w.1).min().expect("fight rooms");
        let rooms: Vec<usize> = weights.into_iter().filter(|w| w.1 == least).map(|w| w.0).collect();
        rooms[(seed as usize * 7 + 3) % rooms.len()]
    };
    run.enter(room, Some(2));
    run.calm_for_test();
    fight(&mut run, if boss { 240 } else { 150 })
}

/// The party's power, a knight's held bonus summed over the party.
fn wish_power(run: &Run) -> String {
    let h = run.players.values().next().unwrap();
    let b = &h.bonus;
    format!(
        "k1 dmg+{} rate+{} shots+{} pierce{} fork{} homing{} vamp{} size{} burn{} poison{} fart{} knock{} vel+{} armor{} hp{}",
        b.damage, b.rate, b.shots, b.pierce, b.fork, b.homing, b.vamp, b.size, b.burn, b.poison, b.fart, b.knock, b.velocity, b.armor, h.max_hp
    )
}

/// The last game's party, floors 1-6, 2-4 knights: fresh knights against
/// the same knights holding the wishes the last game's players made, in a
/// plain room and the floor's boss room. Clear share and time, damage per
/// second (the room's whole health over its clear time), deaths a fight.
/// `cargo test -p angel-delve --lib wish_party_check -- --ignored --nocapture`
#[test]
#[ignore]
fn wish_party_check() {
    let depths = pc_list("PC_DEPTHS", &[1, 2, 3, 4, 5, 6]);
    let ns = pc_list("PC_NS", &[2, 3, 4]);
    let seeds = pc_list("PC_SEEDS", &[0, 1, 2, 3]);
    println!();
    for &depth in &depths {
        println!("floor {depth}: {}", wish_power(&wished_run(0, depth, 2, true)));
        for &n in &ns {
            for boss in [false, true] {
                for wished in [false, true] {
                    let fights: Vec<Fight> = seeds
                        .iter()
                        .map(|&s| wish_fight(wished_run(u64::from(s), depth, n as usize, wished), u64::from(s), boss))
                        .collect();
                    let k = fights.len() as f32;
                    let clears: Vec<&Fight> = fights.iter().filter(|f| f.cleared).collect();
                    let secs = clears.iter().map(|f| f.secs).sum::<f32>() / clears.len().max(1) as f32;
                    let dps = clears.iter().map(|f| f.foe_hp as f32 / f.secs.max(0.1)).sum::<f32>() / clears.len().max(1) as f32;
                    println!(
                        "WISHROW,{depth},{n},{},{},hp {:.0},clear {:.0}%,secs {:.1},dps {:.0},dmg {:.0}%,deaths {:.2}",
                        if boss { "boss" } else { "room" },
                        if wished { "wished" } else { "fresh" },
                        fights.iter().map(|f| f.foe_hp as f32).sum::<f32>() / k,
                        clears.len() as f32 * 100.0 / k,
                        secs,
                        dps,
                        fights.iter().map(|f| f.dmg).sum::<f32>() / k,
                        fights.iter().map(|f| f.deaths as f32).sum::<f32>() / k,
                    );
                }
            }
        }
    }
}

/// The heaviest wished party there is: four knights, each holding every
/// wish of the last game (both knights' lists, climbed as far as they go)
/// and the loot that adds shots (Twin String, Fork, Awakened Fork, Bodkin
/// Heads, Quick Draw), on floor `depth`'s boss room.
fn heaviest_run(seed: u64, depth: u32) -> Run {
    let mut run = floor_run(seed, depth, Party::Fresh, 4);
    let book = last_game_book();
    let ids: Vec<u32> = run.players.keys().copied().collect();
    for id in ids {
        for _round in 0..3 {
            for list in LAST_GAME_WISHES {
                for wish in list.iter().filter_map(|w| book.iter().find(|b| b.id == *w)) {
                    let _ = run.grant(id, wish);
                }
            }
        }
        let hero = run.players.get_mut(&id).unwrap();
        for loot in ["twin-string", "fork", "awakened-fork", "bodkin-heads", "quick-draw"] {
            hero.deck.push(loot.into());
        }
        hero.rebonus(&run.book);
        hero.wish_floor = 6;
        hero.rebonus(&run.book);
        hero.hp = hero.max_hp;
    }
    run
}

/// What the heaviest wished party's shots cover of the screen, the room's
/// monsters made to last (twenty times their health) and every knight
/// firing at the nearest the whole time: the most and the mean, percent,
/// and the most shots aloft, with the cover rule off and on.
/// `cargo test -p angel-delve --release --lib shot_cover_check -- --ignored --nocapture`
#[test]
#[ignore]
fn shot_cover_check() {
    println!();
    for off in [true, false] {
        cover::OFF.with(|o| o.set(off));
        for depth in [3u32, 5] {
            for seed in 0..4u64 {
                let mut run = heaviest_run(seed, depth);
                let room = rooms_of(&run, RoomKind::Fight)
                    .into_iter()
                    .max_by_key(|&r| run.dungeon.rooms[r].cols * run.dungeon.rooms[r].rows)
                    .unwrap();
                run.enter(room, Some(2));
                run.calm_for_test();
                for e in run.enemies.iter_mut() {
                    e.max_hp *= 20;
                    e.hp = e.max_hp;
                }
                for hero in run.players.values_mut() {
                    hero.invulnerable = u32::MAX / 2;
                }
                let view = cover::view(run.room());
                let (mut most, mut sum, mut ticks, mut aloft) = (0.0f32, 0.0f32, 0u32, 0usize);
                for _ in 0..30 * HZ {
                    let inputs: BTreeMap<u32, Input> = run
                        .players
                        .keys()
                        .map(|&id| {
                            let mut i = bot_input(&run, id);
                            i.fire = true;
                            (id, i)
                        })
                        .collect();
                    run.step(&inputs);
                    let c = cover::covered(&run.projectiles, view);
                    most = most.max(c);
                    sum += c;
                    ticks += 1;
                    aloft = aloft.max(run.projectiles.iter().filter(|p| cover::counted(p)).count());
                    if run.phase != Phase::Fighting {
                        break;
                    }
                }
                println!(
                    "COVER,{},floor {depth},seed {seed},most {most:.0}%,mean {:.0}%,aloft {aloft},secs {:.1}",
                    if off { "before" } else { "after" },
                    sum / ticks.max(1) as f32,
                    ticks as f32 / HZ as f32,
                );
            }
        }
    }
    cover::OFF.with(|o| o.set(false));
    println!("{}", wish_power(&heaviest_run(0, 5)));
}
