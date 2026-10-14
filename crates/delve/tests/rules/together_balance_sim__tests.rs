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

/// What a bot does for one knight this tick.
fn bot_input(run: &Run, id: u32) -> Input {
    let hero = &run.players[&id];
    if hero.hp == 0 || hero.stone {
        return Input::default();
    }
    let foes: Vec<&Enemy> = run
        .enemies
        .iter()
        .filter(|e| e.hp > 0 && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH)
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
    // Dodge: sidestep a hostile shot that is close and coming.
    let mut threatened = 0.0f32;
    for shot in run.projectiles.iter().filter(|p| p.hostile && p.ttl > 0) {
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
    let mut run = Run::new(seed, 700 + seed, None);
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
                    knights::knight(["percival", "lynette", "gareth", "galahad"][i]).unwrap(),
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

/// The head count, room by room: the same rooms and bosses with one, two,
/// three and four knights, for how long a clear takes and what it costs.
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
        for n in 1..=4 {
            let fights: Vec<Fight> = (0..12).map(|s| normal_room(s, depth, party, n)).collect();
            row(&format!("{label} x{n} room"), &fights);
        }
    }
    println!();
    for (party, depth, label) in [
        (Party::Fresh, 2, "fresh floor 2"),
        (Party::Won, 4, "won   floor 4"),
    ] {
        for n in 1..=4 {
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
    boss.hp = boss.max_hp;
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

/// (b) Ringed by twenty-four, the crowd build cuts through them slower and
/// bleeds more than when every stroke landed whole.
#[test]
fn a_crowd_build_in_a_mob_clears_slower_than_when_every_stroke_landed_whole() {
    for depth in [4, 6] {
        let mean = |whole_strokes: bool| {
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
            let secs = fights.iter().map(|f| f.0).sum::<f32>() / 16.0;
            let lost = fights.iter().map(|f| f.2 as f32).sum::<f32>() / 16.0;
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
        assert!(
            bleeds >= bled * 1.5,
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
