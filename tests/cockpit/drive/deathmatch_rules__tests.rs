//! Knights Wish as a Bend module, against the native rules. The native rules
//! (`oracle`) are the oracle: two Runs from one seed take the same scripted
//! actions, one ticking the round natively and one through the shipped
//! `deathmatch-rules` leg, and every tick the whole round, the knights, the
//! sounds, the cues and the purses must agree to the byte. The unit tests that
//! used to drive the native game are here too, now driving the leg. The
//! measurements at the end are `--ignored`.
#[path = "deathmatch_oracle.rs"]
mod oracle;

use crate::agent::harness::live_policy;
use angel_delve::realm::Spoil;
use angel_delve::rules::crossroads::{Game, RESULTS_TIME, Stage};
use angel_delve::rules::deathmatch::{self as dm, *};
use angel_delve::rules::votes::{Action, Command};
use angel_delve::rules::{
    self, Book, HZ, Input, Pack, Phase, RoomKind, Run, Shot, Tile, Weapon, cards, crawl_floor,
    orbit_points, roundtable,
};
use angel_delve::wire::{self, Guest, Shots};
use oracle::{NativeRun, NativeSession};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

type Inputs = BTreeMap<u32, Input>;

/// The module a mutation check swaps in (`DEATHMATCH_LEG_DIR`).
static MUTANT: std::sync::OnceLock<angel_live::Program> = std::sync::OnceLock::new();

fn mutant_engine(id: &str, input: &serde_json::Value) -> Result<serde_json::Value, String> {
    match MUTANT.get() {
        Some(mutant) if id == "deathmatch-rules" => mutant.render(input),
        _ => Ok(live_policy::decide_shipped::<serde_json::Value>(id, input, |_| Ok(())).0),
    }
}

/// The process's rules engine, installed once: the shipped leg, or, for the
/// mutation check, the rebuilt copy of it in the directory `DEATHMATCH_LEG_DIR`
/// names (a folder holding `deathmatch-rules/`).
fn engine() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| match std::env::var_os("DEATHMATCH_LEG_DIR") {
        Some(dir) => {
            let mut registry = angel_live::Registry::validated(
                dir.into(),
                crate::agent::hotload::evaluate,
                |_, _| Ok(()),
            );
            registry.refresh(true);
            let program = registry
                .snapshot()
                .policies
                .get("deathmatch-rules")
                .cloned()
                .expect("the mutant loads");
            let _ = MUTANT.set(program);
            angel_delve::scripted::install(mutant_engine);
        }
        None => live_policy::install_scripted_rules(),
    });
}

/// A small deterministic dice, apart from the game's own.
struct Dice(u64);
impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// Wishes that make a knight's shots do the odd things the rules answer for.
fn gift(book: &mut Book, id: &str, effects: Vec<cards::Effect>) {
    let mut card = book.get("bow").unwrap().clone();
    card.id = id.into();
    card.kind = cards::Kind::Hold;
    card.arm = None;
    card.effects = effects;
    book.insert(card);
}

fn gifts(book: &mut Book) {
    use cards::Effect::*;
    gift(
        book,
        "arena-gift",
        vec![
            Damage(30),
            Rate(25),
            Shots(2),
            Volley(2),
            Fork(2),
            Homing(2),
            Pierce(1),
        ],
    );
    gift(book, "arena-passives", vec![Orbit(3), Fart(1), Chill(1)]);
    gift(
        book,
        "arena-bursts",
        vec![Burst(60), Chain(2), Vamp(15), Mend(3), Bounce(2), Knock(2)],
    );
    gift(
        book,
        "arena-fires",
        vec![
            Burn(2),
            Poison(2),
            Boomerang(1),
            Size(2),
            Velocity(30),
            Homing(1),
        ],
    );
}

/// The kits a sweep hands round: none, a champion, a mage, a gift of wishes.
const KITS: [&str; 12] = [
    "",
    "hammerer",
    "butcher",
    "alchemist",
    "scryglass",
    "arena-gift",
    "arena-passives",
    "arena-bursts",
    "arena-fires",
    "gareth",
    "galahad",
    "ironbraid",
];

fn outfit(run: &mut Run, id: u32, kit: &str) {
    match kit {
        "" => {}
        // A champion whose three spells lay a hedge, a decoy and a totem, so
        // the parity sweep covers every kind of field however the book deals.
        "arena-fields" => {
            run.take_up(id, "hammerer");
            let base = run.players[&id].spells[0]
                .clone()
                .expect("a champion has a first spell");
            let base = run.book.get(&base).unwrap().clone();
            for (slot, effect) in [
                cards::Effect::Hedge(1),
                cards::Effect::Imposture(1),
                cards::Effect::Totem(8),
            ]
            .into_iter()
            .enumerate()
            {
                let mut card = base.clone();
                card.id = format!("arena-field-{slot}");
                card.effects.retain(|e| matches!(e, cards::Effect::Cast(_)));
                card.effects.push(effect);
                run.players.get_mut(&id).unwrap().spells[slot] = Some(card.id.clone());
                run.book.insert(card);
            }
        }
        k if k.starts_with("arena-") => {
            let book = run.book.clone();
            let hero = run.players.get_mut(&id).unwrap();
            hero.deck.push(k.into());
            hero.rebonus(&book);
        }
        k => {
            run.take_up(id, k);
        }
    }
}

/// A party of `seats` in Knights Wish at the town door, the title card over.
fn party(seats: u32, kits: &[&str]) -> Run {
    engine();
    let mut run = roundtable::fixture(&dm::ADVENTURE);
    gifts(&mut run.book);
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    for (i, kit) in kits.iter().enumerate() {
        outfit(&mut run, i as u32 + 1, kit);
    }
    // The town door seated the first knight before the kit; the others join late, kit in hand.
    let (hero, book) = (run.players[&1].clone(), run.book.clone());
    run.deathmatch
        .as_mut()
        .unwrap()
        .fighters
        .insert(1, Fighter::new(&hero, 0, &book));
    run
}

/// What a lockstep run saw, to prove the script reached the branches it names.
#[derive(Debug, Default, Clone)]
struct Seen {
    ticks: u32,
    played: u32,
    kills: u32,
    respawns: u32,
    spawn_points: BTreeSet<u32>,
    wards_broken: u32,
    pickups_taken: BTreeSet<usize>,
    powered: u32,
    burning: u32,
    stunned: u32,
    chilled: u32,
    slams: u32,
    rots: u32,
    wins: u32,
    homes: u32,
    shots: u32,
    fields: BTreeSet<String>,
    sounds: BTreeSet<String>,
    cues: u32,
    laurels: u32,
    purse: u32,
    feed_max: usize,
    armour: u32,
    healed: u32,
}
impl Seen {
    fn add(&mut self, o: &Seen) {
        self.ticks += o.ticks;
        self.played += o.played;
        self.kills += o.kills;
        self.respawns += o.respawns;
        self.spawn_points.extend(&o.spawn_points);
        self.wards_broken += o.wards_broken;
        self.pickups_taken.extend(&o.pickups_taken);
        self.powered += o.powered;
        self.burning += o.burning;
        self.stunned += o.stunned;
        self.chilled += o.chilled;
        self.slams += o.slams;
        self.rots += o.rots;
        self.wins += o.wins;
        self.homes += o.homes;
        self.shots += o.shots;
        self.fields.extend(o.fields.iter().cloned());
        self.sounds.extend(o.sounds.iter().cloned());
        self.cues += o.cues;
        self.laurels += o.laurels;
        self.purse += o.purse;
        self.feed_max = self.feed_max.max(o.feed_max);
        self.armour += o.armour;
        self.healed += o.healed;
    }
}

/// The whole of what a tick may change, as one comparable text.
fn slice(run: &Run) -> serde_json::Value {
    serde_json::json!({
        "round": run.deathmatch, "players": run.players, "sounds": run.sounds, "cues": run.cues,
        "found": run.found, "order": run.deathmatch.as_ref().map(|s| s.order()),
        "valid": run.deathmatch.as_ref().map(|s| s.valid()),
    })
}

/// Where two JSON trees differ (up to `limit` places), for a failure to name.
fn differences(
    a: &serde_json::Value,
    b: &serde_json::Value,
    path: &str,
    found: &mut Vec<String>,
    limit: usize,
) {
    use serde_json::Value::*;
    if found.len() >= limit {
        return;
    }
    match (a, b) {
        (Object(x), Object(y)) => {
            for key in x.keys().chain(y.keys()).collect::<BTreeSet<_>>() {
                let at = format!("{path}.{key}");
                match (x.get(key), y.get(key)) {
                    (Some(p), Some(q)) => differences(p, q, &at, found, limit),
                    (p, q) => found.push(format!("{at}: native {p:?} against leg {q:?}")),
                }
            }
        }
        (Array(x), Array(y)) if x.len() == y.len() => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                differences(p, q, &format!("{path}[{i}]"), found, limit);
            }
        }
        (Array(x), Array(y)) => found.push(format!(
            "{path}: native has {} entries {a}, leg {} {b}",
            x.len(),
            y.len()
        )),
        _ => {
            if a != b {
                found.push(format!("{path}: native {a} against leg {b}"));
            }
        }
    }
}

/// Two Runs from one start: one ticks the round with the native rules, one
/// with the leg. They are compared after every tick.
struct Pair {
    native: Run,
    leg: Run,
    ticks: u32,
    seen: Seen,
}
impl Pair {
    fn new(run: Run) -> Pair {
        engine();
        Pair {
            native: run.clone(),
            leg: run,
            ticks: 0,
            seen: Seen::default(),
        }
    }
    /// The same change to both, before a tick.
    fn both(&mut self, change: impl Fn(&mut Run)) {
        change(&mut self.native);
        change(&mut self.leg);
    }
    fn round(&self) -> Option<&Session> {
        self.leg.deathmatch.as_ref()
    }
    fn tick(&mut self, inputs: &Inputs) {
        let before = self.round().cloned();
        let before_sounds = self.leg.sounds.len();
        self.native.native_tick_deathmatch(inputs);
        self.leg.tick_deathmatch(inputs);
        self.ticks += 1;
        let (a, b) = (slice(&self.native), slice(&self.leg));
        if a != b {
            let mut found = Vec::new();
            differences(&a, &b, "$", &mut found, 12);
            let keys: Vec<_> = inputs
                .iter()
                .map(|(id, i)| {
                    (
                        id, i.move_x, i.move_y, i.aim_x, i.fire, i.swing, i.bomb, i.play, i.cast,
                        i.cycle, i.dash,
                    )
                })
                .collect();
            panic!(
                "the leg left the native rules at tick {} (keys {keys:?}):\n{}",
                self.ticks,
                found.join("\n")
            );
        }
        self.observe(before.as_ref(), inputs, before_sounds);
    }
    fn observe(&mut self, before: Option<&Session>, inputs: &Inputs, before_sounds: usize) {
        let seen = &mut self.seen;
        seen.ticks += 1;
        for sound in &self.leg.sounds[before_sounds..] {
            seen.sounds.insert(sound.to_string());
        }
        seen.cues = self.leg.cues.len() as u32;
        let Some(after) = self.leg.deathmatch.as_ref() else {
            if before.is_some() {
                seen.homes += 1;
            }
            return;
        };
        let Some(before) = before else { return };
        if after.ticks > dm::TITLE && after.ticks > before.ticks && after.winner.is_none() {
            seen.played += 1;
        }
        if after.winner.is_some() && before.winner.is_none() {
            seen.wins += 1;
            seen.purse = seen.purse.max(after.purse);
        }
        seen.feed_max = seen.feed_max.max(after.feed.len());
        seen.shots += after.shots.len() as u32;
        for f in after.fields.iter() {
            seen.fields.insert(format!("{:?}", f.kind));
        }
        for k in 0..5 {
            if after.pickups[k] > before.pickups[k] {
                seen.pickups_taken.insert(k);
            }
        }
        for (id, f) in &after.fighters {
            let Some(was) = before.fighters.get(id) else {
                continue;
            };
            seen.kills += u32::from(f.kills.saturating_sub(was.kills));
            if was.respawn == 1 && f.respawn == 0 {
                seen.respawns += 1;
                let at = (f.hero.x, f.hero.y);
                if let Some(i) = dm::SPAWNS.iter().position(|p| *p == at) {
                    seen.spawn_points.insert(i as u32);
                }
            }
            if was.protection > 0
                && f.protection == 0
                && was.respawn == 0
                && f.respawn == 0
                && inputs.contains_key(id)
            {
                seen.wards_broken += 1;
            }
            if f.power == dm::POWER_TIME {
                seen.powered += 1;
            }
            seen.burning += u32::from(f.burn > 0);
            seen.stunned += u32::from(f.stun > 0);
            seen.chilled += u32::from(f.hero.chilled > 0);
            seen.slams += u32::from(was.hero.champ.windup == 1);
            seen.rots += u32::from(f.hero.champ.rot > 0);
            seen.armour += u32::from(f.hero.armor > was.hero.armor);
            seen.healed += u32::from(f.hero.hp > was.hero.hp && f.respawn == 0 && was.respawn == 0);
        }
        seen.laurels = self.leg.players.values().filter(|h| h.laurel).count() as u32;
    }
}

/// A knight standing somewhere, whole and unwarded.
fn ready(run: &mut Run, id: u32, at: (f32, f32)) {
    if let Some(f) = run
        .deathmatch
        .as_mut()
        .and_then(|s| s.fighters.get_mut(&id))
    {
        (f.hero.x, f.hero.y) = at;
        f.hero.hp = f.hero.max_hp;
        f.hero.invulnerable = 0;
        f.respawn = 0;
        f.protection = 0;
    }
}

/// Past the title card with every ward down; the late seats have joined.
fn start(pair: &mut Pair) {
    pair.tick(&Inputs::new());
    pair.both(|run| {
        let s = run.deathmatch.as_mut().unwrap();
        s.ticks = dm::TITLE;
        for f in s.fighters.values_mut() {
            f.protection = 0;
        }
    });
}

#[test]
fn the_leg_holds_the_same_numbers_the_hud_and_saves_read() {
    engine();
    let rules: serde_json::Value =
        angel_delve::scripted::step("deathmatch-rules", &serde_json::json!({"op": "consts"}))
            .unwrap();
    assert_eq!(
        rules,
        serde_json::json!({
            "$": "Rules", "goal": dm::GOAL, "title": dm::TITLE, "respawn": dm::RESPAWN, "protection": dm::PROTECTION,
            "podium": dm::PODIUM, "pickup_rest": dm::PICKUP_REST, "power_time": dm::POWER_TIME,
        })
    );
}

#[test]
fn deathmatch_melee_decisions_match_native_boundaries() {
    engine();
    for (damage, expected) in [
        (0, "Light"),
        (19, "Light"),
        (20, "Medium"),
        (44, "Medium"),
        (45, "Heavy"),
        (u32::MAX, "Heavy"),
    ] {
        let got: String = angel_delve::scripted::step(
            "deathmatch-rules",
            &serde_json::json!({"op": "contact_tier", "damage": damage}),
        )
        .unwrap();
        assert_eq!(got, expected);
    }
    for winner in [false, true] {
        for connected in 0..=3 {
            let got: bool = angel_delve::scripted::step(
                "deathmatch-rules",
                &serde_json::json!({"op": "melee_tick", "winner": winner, "connected": connected}),
            )
            .unwrap();
            assert_eq!(got, !winner && connected >= 2);
        }
    }
}

// --- a pilot for the knights ---------------------------------------------------------------
fn roll(dice: &mut Dice) -> Input {
    let axis = |d: &mut Dice| d.below(3) as i8 - 1;
    Input {
        move_x: axis(dice),
        move_y: axis(dice),
        aim_x: axis(dice),
        fire: dice.chance(55),
        dash: dice.chance(8),
        bomb: dice.chance(4),
        swing: dice.chance(15),
        play: if dice.chance(10) {
            dice.below(5) as u8
        } else {
            0
        },
        cast: if dice.chance(25) {
            dice.below(5) as u8
        } else {
            0
        },
        cycle: dice.chance(5),
        ..Default::default()
    }
}

/// Keys held a while and then re-rolled, as a hand holds them; the one-press
/// keys (bomb, cards, spells) fire on the first tick of a hold only. A tick in
/// a hundred sends a key the rules must refuse.
struct Pilot {
    dice: Dice,
    held: BTreeMap<u32, (Input, u32)>,
}
impl Pilot {
    fn new(seed: u64) -> Pilot {
        Pilot {
            dice: Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1),
            held: BTreeMap::new(),
        }
    }
    fn inputs(&mut self, seats: &[u32]) -> Inputs {
        let mut out = Inputs::new();
        for &id in seats {
            let (input, left) = self.held.entry(id).or_insert((Input::default(), 0));
            if *left == 0 {
                *input = roll(&mut self.dice);
                *left = 4 + self.dice.below(24) as u32;
            } else {
                (input.bomb, input.play, input.cast, input.cycle) = (false, 0, 0, false);
            }
            *left -= 1;
            let mut sent = *input;
            if self.dice.chance(1) {
                sent.move_x = 3;
            }
            if self.dice.chance(60) || sent.valid() == false {
                out.insert(id, sent);
            }
        }
        out
    }
}

/// Knights standing in a row on open ground, near enough to fight.
fn gather(run: &mut Run, seats: &[u32], row: f32) {
    for (i, &id) in seats.iter().enumerate() {
        if run
            .deathmatch
            .as_ref()
            .and_then(|s| s.fighters.get(&id))
            .is_some_and(|f| f.alive())
        {
            ready(run, id, (14. + 3. * i as f32, row));
            if let Some(f) = run
                .deathmatch
                .as_mut()
                .and_then(|s| s.fighters.get_mut(&id))
            {
                f.yaw = 0.;
            }
        }
    }
}

/// One sweep: a party plays `ticks` of random keys with the dice fixed by `seed`.
fn sweep(seed: u64, seats: u32, ticks: u32, nearly_won: bool) -> Seen {
    let kits: Vec<&str> = (0..seats)
        .map(|i| KITS[((seed as usize) + 5 * i as usize) % KITS.len()])
        .collect();
    let mut run = party(seats, &kits);
    run.seed = seed;
    let mut pair = Pair::new(run);
    // Cards and spells to cast: every spell the book has, round the seats.
    pair.both(|run| {
        let spells: Vec<String> = run
            .book
            .cards
            .iter()
            .filter(|c| c.kind == cards::Kind::Spell)
            .map(|c| c.id.clone())
            .collect();
        let plays: Vec<String> = run
            .book
            .cards
            .iter()
            .filter(|c| c.kind == cards::Kind::Play)
            .map(|c| c.id.clone())
            .collect();
        for (i, hero) in run.players.values_mut().enumerate() {
            if hero.spells[3].is_none() && !spells.is_empty() {
                hero.spells[3] = Some(spells[(seed as usize + i) % spells.len()].clone());
            }
            for k in 0..2 {
                if !plays.is_empty() && hero.hand.len() < cards::HAND {
                    hero.hand
                        .push(plays[(seed as usize + 3 * i + k) % plays.len()].clone());
                }
            }
            hero.carried
                .add(Spoil::Gold, 40 + 25 * (seed as u32 % 5) + 13 * i as u32);
            hero.bombs = 3;
        }
    });
    start(&mut pair);
    let seat_ids: Vec<u32> = (1..=seats).collect();
    pair.both(|run| {
        let s = run.deathmatch.as_mut().unwrap();
        for (i, f) in s.fighters.values_mut().enumerate() {
            f.kills = if nearly_won && i % 2 == 0 {
                (dm::GOAL - 2) as u16
            } else {
                0
            };
        }
        for (i, stake) in [(1u32, 30u32), (2, 7), (3, 500)] {
            if s.fighters.contains_key(&i) {
                s.ante.insert(i, stake);
            }
        }
    });
    let mut pilot = Pilot::new(seed);
    let heal = seed % 2 == 0;
    let mut gone = 0;
    for t in 0..ticks {
        if t % 70 == 0 {
            pair.both(|run| gather(run, &seat_ids, if t % 140 == 0 { 13. } else { 21. }));
        }
        if heal && t % 25 == 0 {
            pair.both(|run| {
                if let Some(s) = run.deathmatch.as_mut() {
                    for f in s.fighters.values_mut().filter(|f| f.alive()) {
                        f.hero.hp = f.hero.max_hp;
                    }
                }
            });
        }
        // Somebody drifts away, comes back, steps out of the room, a seat joins late.
        if t == ticks / 3 && seats >= 3 {
            pair.both(|run| run.set_away(seats, true));
        }
        if t == ticks / 2 && seats >= 3 {
            pair.both(|run| run.set_away(seats, false));
        }
        if t == ticks / 2 + 40 && seats >= 4 {
            pair.both(|run| {
                run.leave_activity(2);
            });
        }
        let inputs = pilot.inputs(&seat_ids);
        pair.tick(&inputs);
        if pair.leg.deathmatch.is_none() {
            gone += 1;
            if gone > 3 {
                break;
            }
        }
    }
    pair.seen
}

#[test]
fn a_lone_knight_holds_the_round_and_idle_parties_age_alike() {
    for seats in 1..=6 {
        let mut pair = Pair::new(party(seats, &[]));
        // Every pickup resting, a knight firing from the first tick: nothing may
        // shoot or count down until the title card has run its three seconds.
        pair.both(|run| run.deathmatch.as_mut().unwrap().pickups = [50; 5]);
        let fire = Inputs::from([(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]);
        for _ in 0..260 {
            pair.tick(&fire);
        }
        let s = pair.leg.deathmatch.as_ref().unwrap();
        if seats == 1 {
            assert_eq!(s.ticks, 0, "one knight alone never starts the clock");
            assert_eq!(s.pickups, [50; 5]);
        } else {
            assert_eq!(s.ticks, 260, "{seats} knights run the clock");
            assert_eq!(
                s.pickups, [0; 5],
                "the pickups waited out the title card and then their time"
            );
        }
    }
}

#[test]
fn random_rounds_match_the_native_rules_tick_for_tick() {
    let mut seen = Seen::default();
    for (seed, seats, nearly_won) in [
        (11u64, 2u32, false),
        (12, 3, true),
        (23, 4, false),
        (24, 4, true),
        (37, 2, true),
        (38, 6, false),
        (59, 3, false),
        (60, 4, false),
        (71, 5, true),
        (72, 2, false),
        (83, 6, true),
        (94, 4, true),
        (6, 2, false),
        (18, 3, false),
        (30, 4, true),
        (7, 3, false),
        (20, 2, true),
    ] {
        let part = sweep(seed, seats, 1500, nearly_won);
        seen.add(&part);
    }
    eprintln!("{seen:#?}");
    assert!(
        seen.played > 5000,
        "the rounds must be played: {}",
        seen.played
    );
    assert!(seen.kills >= 40, "kills: {}", seen.kills);
    assert!(seen.respawns >= 30, "respawns: {}", seen.respawns);
    assert!(
        seen.spawn_points.len() >= 4,
        "spawn points: {:?}",
        seen.spawn_points
    );
    assert!(
        seen.wards_broken >= 10,
        "wards broken: {}",
        seen.wards_broken
    );
    assert!(seen.powered >= 1, "power picked up");
    assert!(
        seen.pickups_taken.len() >= 3,
        "pickups: {:?}",
        seen.pickups_taken
    );
    assert!(
        seen.burning > 0 && seen.stunned > 0,
        "statuses {} {}",
        seen.burning,
        seen.stunned
    );
    assert!(seen.wins >= 3, "wins: {}", seen.wins);
    assert!(seen.homes >= 2, "podiums: {}", seen.homes);
    assert!(seen.fields.len() >= 2, "fields: {:?}", seen.fields);
    assert!(seen.feed_max >= 3, "feed: {}", seen.feed_max);
    assert!(
        seen.sounds.contains("hit") && seen.sounds.contains("play_card"),
        "sounds: {:?}",
        seen.sounds
    );
}

// --- the unit tests that drove the native game, now driving the leg ----------------------------
fn six() -> Run {
    engine();
    let mut r = roundtable::fixture(&ADVENTURE);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.start_deathmatch();
    r.step(&BTreeMap::new());
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = TITLE;
    for f in s.fighters.values_mut() {
        f.protection = 0;
    }
    r
}
fn ready_in(s: &mut Session, id: u32, at: (f32, f32)) {
    let f = s.fighters.get_mut(&id).unwrap();
    (f.hero.x, f.hero.y) = at;
    f.hero.hp = f.hero.max_hp;
    f.hero.invulnerable = 0;
    f.respawn = 0;
    f.protection = 0;
}
#[test]
fn deathmatch_head_melee_and_hero_kit_parity() {
    let mut pair = Pair::new(six());
    for damage in [19, 20, 44, 45] {
        pair.both(|run| ready_in(run.deathmatch.as_mut().unwrap(), 2, (15., 15.)));
        let a = pair
            .native
            .deathmatch
            .as_mut()
            .unwrap()
            .native_hit(1, 2, damage, (1., 0.));
        let b = pair
            .leg
            .deathmatch
            .as_mut()
            .unwrap()
            .hit(1, 2, damage, (1., 0.));
        assert_eq!(a, b);
        assert_eq!(slice(&pair.native), slice(&pair.leg));
    }
    pair.both(|run| {
        let s = run.deathmatch.as_mut().unwrap();
        let hero = &mut s.fighters.get_mut(&1).unwrap().hero;
        hero.hero_kit.focus = 10;
        hero.hero_kit.stacks = 3;
        assert!(hero.cooldown(100) < 100);
        let f = s.fighters.get_mut(&2).unwrap();
        f.hero.hero_kit.windrun = 12;
        f.respawn = 1;
    });
    pair.tick(&Inputs::new());
    assert_eq!(
        pair.leg.deathmatch.as_ref().unwrap().fighters[&2]
            .hero
            .hero_kit
            .windrun,
        0
    );
}

#[test]
fn kills_belong_to_the_attacker_and_ten_ends_the_round_once() {
    let mut r = six();
    let s = r.deathmatch.as_mut().unwrap();
    for n in 1..=GOAL {
        ready_in(s, 2, (15., 15.));
        s.fighters.get_mut(&2).unwrap().hero.hp = 4;
        assert!(s.hit(1, 2, 20, (1., 0.)));
        assert_eq!(s.fighters[&1].kills, n);
        assert_eq!(s.fighters[&2].deaths, n);
        assert_eq!(s.winner, if n == GOAL { Some(1) } else { None });
        assert!(!s.hit(1, 2, 20, (1., 0.)), "a corpse never pays twice");
    }
    ready_in(s, 3, (17., 15.));
    assert!(!s.hit(1, 3, 100, (1., 0.)));
    assert_eq!(s.fighters[&1].kills, GOAL);
}
#[test]
fn respawn_waits_then_chooses_the_farthest_spawn_and_wards_it() {
    let mut r = six();
    let book = r.book.clone();
    let s = r.deathmatch.as_mut().unwrap();
    for id in [1, 3, 4, 5, 6] {
        ready_in(s, id, (5., 5.));
    }
    s.fighters.get_mut(&2).unwrap().hero.hp = 4;
    s.hit(1, 2, 20, (1., 0.));
    for _ in 0..RESPAWN - 1 {
        s.step(&BTreeMap::new(), &book, 1);
    }
    assert_eq!(s.fighters[&2].respawn, 1);
    assert_eq!(s.fighters[&2].hero.hp, 0);
    s.step(&BTreeMap::new(), &book, 1);
    let f = &s.fighters[&2];
    assert_eq!((f.hero.x, f.hero.y), (43., 31.));
    assert_eq!(f.hero.hp, f.hero.max_hp);
    assert_eq!(f.protection, PROTECTION);
    assert!(!s.hit(1, 2, 50, (1., 0.)));
    s.step(
        &[(
            2,
            Input {
                fire: true,
                ..Default::default()
            },
        )]
        .into(),
        &book,
        1,
    );
    assert_eq!(s.fighters[&2].protection, 0, "firing gives up the ward");
}
#[test]
fn the_real_deck_shapes_damage_rate_volley_fork_and_homing() {
    let mut r = six();
    let mut gift = r.book.get("bow").unwrap().clone();
    gift.id = "arena-gift".into();
    gift.kind = cards::Kind::Hold;
    gift.arm = None;
    gift.effects = vec![
        cards::Effect::Damage(30),
        cards::Effect::Rate(25),
        cards::Effect::Shots(2),
        cards::Effect::Volley(2),
        cards::Effect::Fork(2),
        cards::Effect::Homing(2),
        cards::Effect::Pierce(1),
    ];
    r.book.insert(gift);
    let h = r.players.get_mut(&1).unwrap();
    h.deck.push("arena-gift".into());
    h.rebonus(&r.book);
    let mut s = Session::new(&r.players, &r.book);
    s.ticks = TITLE;
    let f = s.fighters.get_mut(&1).unwrap();
    f.yaw = 0.;
    (f.hero.x, f.hero.y) = (15., 15.);
    s.step(
        &[(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]
        .into(),
        &r.book,
        1,
    );
    let ours: Vec<_> = s.shots.iter().filter(|p| p.traits.owner == 1).collect();
    assert!(ours.len() >= 3);
    assert!(ours.iter().all(|p| p.damage > Weapon::Bow.arms().damage
        && p.traits.fork == 2
        && p.traits.homing == 2
        && p.pierce >= 1));
    assert!(s.fighters[&1].hero.fire_cooldown < Weapon::Bow.arms().cooldown);
    assert_eq!(s.fighters[&1].hero.deck, r.players[&1].deck);
}
#[test]
fn shots_hit_knights_but_never_through_the_cover() {
    let mut r = six();
    let s = r.deathmatch.as_mut().unwrap();
    ready_in(s, 1, (15., 15.));
    ready_in(s, 2, (17., 15.));
    s.fighters.get_mut(&1).unwrap().yaw = 0.;
    let hp = s.fighters[&2].hero.hp;
    for _ in 0..8 {
        s.step(
            &[(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]
            .into(),
            &r.book,
            7,
        );
    }
    assert!(s.fighters[&2].hero.hp < hp);
    s.shots.clear();
    ready_in(s, 1, (17., 17.));
    ready_in(s, 2, (23., 17.));
    let hp = s.fighters[&2].hero.hp;
    for _ in 0..45 {
        s.step(
            &[(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]
            .into(),
            &r.book,
            7,
        );
    }
    assert_eq!(s.fighters[&2].hero.hp, hp, "the pillar stops the volley");
}
#[test]
fn all_map_spawns_and_pickups_are_reachable() {
    assert!(MAP.iter().all(|r| r.len() == 24));
    let mut seen = std::collections::BTreeSet::from([(2, 2)]);
    let mut queue = std::collections::VecDeque::from([(2, 2)]);
    while let Some((x, y)) = queue.pop_front() {
        for p in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if p.0 >= 0
                && p.0 < 24
                && p.1 >= 0
                && p.1 < 18
                && MAP[p.1 as usize].as_bytes()[p.0 as usize] == b'.'
                && seen.insert(p)
            {
                queue.push_back(p);
            }
        }
    }
    for (x, y) in SPAWNS.into_iter().chain(PICKUPS.map(|(x, y, _)| (x, y))) {
        assert!(open(x, y), "{x} {y}");
        assert!(seen.contains(&((x / 2.) as i32, (y / 2.) as i32)));
    }
}
#[test]
fn old_save_and_new_wire_restore_the_round() {
    let r = six();
    let mut json = serde_json::to_value(&r).unwrap();
    json.as_object_mut().unwrap().remove("deathmatch");
    let old: Run = serde_json::from_value(json).unwrap();
    assert!(old.deathmatch.is_none());
    let bytes = serde_json::to_vec(&r).unwrap();
    let restored: Run = serde_json::from_slice(&bytes).unwrap();
    assert!(restored.valid_snapshot());
    let tree = angel_net::doc::value::to_value(&r).unwrap();
    let restored = wire::read(&tree).unwrap();
    assert_eq!(restored.deathmatch.as_ref().unwrap().fighters.len(), 6);
}
#[test]
fn fixed_inputs_replay_identically_and_six_seats_move_independently() {
    let mut r = six();
    let mut a = r.deathmatch.take().unwrap();
    let mut b = a.clone();
    let inputs = [
        (
            1,
            Input {
                move_y: -1,
                aim_x: 1,
                fire: true,
                ..Default::default()
            },
        ),
        (
            6,
            Input {
                move_x: 1,
                ..Default::default()
            },
        ),
    ]
    .into();
    let p = a.fighters[&2].hero.x;
    for _ in 0..100 {
        a.step(&inputs, &r.book, 8);
        b.step(&inputs, &r.book, 8);
    }
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.fighters[&2].hero.x, p);
    assert!(a.valid());
}
#[test]
fn the_town_door_returns_everyone_with_their_original_kit_and_one_purse() {
    engine();
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.join_seat(2, "Friend");
    for h in r.players.values_mut() {
        h.carried.add(Spoil::Gold, 100);
    }
    r.walk_kingdom(Some(ENTRY));
    r.step(&BTreeMap::new());
    assert_eq!(r.room().tile(ENTRY.0, ENTRY.1), Tile::Floor);
    let hp = r.players[&1].hp;
    let hand = r.players[&1].hand.clone();
    r.step(
        &[(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]
        .into(),
    );
    assert!(r.deathmatch_showing());
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = TITLE;
    s.fighters.get_mut(&1).unwrap().kills = 9;
    ready_in(s, 1, (15., 15.));
    ready_in(s, 2, (17., 15.));
    s.fighters.get_mut(&1).unwrap().yaw = 0.;
    s.fighters.get_mut(&2).unwrap().hero.hp = 4;
    for _ in 0..8 {
        r.step(
            &[(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]
            .into(),
        );
        if r.deathmatch.as_ref().unwrap().winner.is_some() {
            break;
        }
    }
    assert_eq!(r.deathmatch.as_ref().unwrap().winner, Some(1));
    assert_eq!(r.players[&1].carried.get(Spoil::Gold), 110);
    assert_eq!(r.players[&2].carried.get(Spoil::Gold), 90);
    for _ in 0..PODIUM + 2 {
        r.step(&BTreeMap::new());
    }
    assert!(!r.deathmatch_showing());
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert_eq!(r.players[&1].hp, hp);
    assert_eq!(r.players[&1].hand, hand);
    assert_eq!(r.players[&1].carried.get(Spoil::Gold), 110);
}
#[test]
fn protection_shields_and_rolls_all_turn_hits() {
    let mut r = six();
    let s = r.deathmatch.as_mut().unwrap();
    let f = s.fighters.get_mut(&2).unwrap();
    f.hero.guard = cards::Guard::Shield;
    f.hero.shielding = true;
    f.hero.aim_x = -1.;
    f.hero.aim_y = 0.;
    let hp = f.hero.hp;
    assert!(!s.hit(1, 2, 12, (1., 0.)));
    assert_eq!(s.fighters[&2].hero.hp, hp);
    assert!(s.fighters[&2].hero.shield_wear.showing());
    let f = s.fighters.get_mut(&2).unwrap();
    f.hero.shielding = false;
    f.hero.guard = cards::Guard::Roll;
    f.walk(Input {
        dash: true,
        ..Default::default()
    });
    assert!(!s.hit(1, 2, 12, (1., 0.)));
}
#[test]
fn leaving_or_away_cannot_be_farmed_and_a_late_join_gets_their_kit() {
    let mut r = six();
    r.leave_activity(6);
    r.set_away(5, true);
    r.step(&BTreeMap::new());
    let s = r.deathmatch.as_mut().unwrap();
    assert!(!s.fighters[&6].connected);
    assert!(!s.fighters[&5].connected);
    assert!(!s.hit(1, 6, 100, (1., 0.)));
    assert!(!s.hit(1, 5, 100, (1., 0.)));
}

#[test]
fn champions_keep_the_maul_and_their_own_spell_slots() {
    let mut r = six();
    assert!(r.take_up(1, "hammerer"));
    let mut s = Session::new(&r.players, &r.book);
    s.ticks = TITLE;
    ready_in(&mut s, 1, (15., 15.));
    ready_in(&mut s, 2, (17., 15.));
    let f = s.fighters.get_mut(&1).unwrap();
    f.yaw = 0.;
    assert!(f.hero.forged.as_ref().unwrap().melee.is_some());
    assert_eq!(f.hero.spells[0].as_deref(), Some("storm-hammer"));
    s.step(
        &[(
            1,
            Input {
                cast: 3,
                ..Default::default()
            },
        )]
        .into(),
        &r.book,
        1,
    );
    assert!(s.fighters[&1].hero.champ.fury > 0);
    let hp = s.fighters[&2].hero.hp;
    s.step(
        &[(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]
        .into(),
        &r.book,
        1,
    );
    assert!(s.fighters[&2].hero.hp < hp);
    assert!(
        s.shots.is_empty(),
        "the maul must not silently become a bow"
    );
    s.step(
        &[(
            1,
            Input {
                cast: 1,
                ..Default::default()
            },
        )]
        .into(),
        &r.book,
        1,
    );
    assert!(
        s.shots
            .iter()
            .any(|p| p.traits.owner == 1 && p.kind == Shot::Sacred)
    );
}
#[test]
fn pickups_are_shared_and_reappear_after_their_wait() {
    let mut r = six();
    let s = r.deathmatch.as_mut().unwrap();
    let (x, y, _) = PICKUPS[0];
    ready_in(s, 1, (x, y));
    s.fighters.get_mut(&1).unwrap().hero.hp = 20;
    s.step(&BTreeMap::new(), &r.book, 1);
    assert_eq!(s.fighters[&1].hero.hp, 55);
    assert_eq!(s.pickups[0], PICKUP_REST);
    ready_in(s, 2, (x, y));
    s.fighters.get_mut(&2).unwrap().hero.hp = 20;
    s.step(&BTreeMap::new(), &r.book, 1);
    assert_eq!(s.fighters[&2].hero.hp, 20);
    let (x, y, _) = PICKUPS[4];
    ready_in(s, 1, (x, y));
    s.step(&BTreeMap::new(), &r.book, 1);
    assert_eq!(s.fighters[&1].power, POWER_TIME);
    s.pickups[0] = 1;
    s.step(&BTreeMap::new(), &r.book, 1);
    assert_eq!(s.fighters[&2].hero.hp, 55);
}
#[test]
fn a_restored_round_keeps_hit_timing_and_patch_readers_keep_every_seat() {
    use std::borrow::Cow;
    let mut r = six();
    let mut s = r.deathmatch.take().unwrap();
    let mut loaded: Session = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    for round in [&mut s, &mut loaded] {
        ready_in(round, 2, (15., 15.));
        round.hit(1, 2, 8, (1., 0.));
    }
    assert_eq!(
        s.fighters[&2].hero.invulnerable,
        loaded.fighters[&2].hero.invulnerable
    );
    assert_eq!(s.fighters[&2].hero.hp, loaded.fighters[&2].hero.hp);
    r.deathmatch = Some(s);
    let tree = wire::publish(&r).unwrap();
    let mut guest = Guest::default();
    guest.read(&tree, None, &Shots::Rebuild).unwrap();
    r.step(
        &[(
            6,
            Input {
                move_x: 1,
                ..Default::default()
            },
        )]
        .into(),
    );
    let tree = wire::publish(&r).unwrap();
    let back = guest
        .read(
            &tree,
            Some(&[Cow::Borrowed("tick"), Cow::Borrowed("deathmatch")]),
            &Shots::Same,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&back.deathmatch).unwrap(),
        serde_json::to_vec(&r.deathmatch).unwrap()
    );
}
#[test]
fn the_round_suspends_the_room_and_its_original_heroes() {
    let mut r = six();
    let hp: Vec<_> = r
        .players
        .values()
        .map(|h| (h.hp, h.x, h.y, h.bombs, h.hand.clone()))
        .collect();
    let foe = r.enemies.clone();
    for _ in 0..60 {
        r.step(
            &[(
                1,
                Input {
                    fire: true,
                    bomb: true,
                    ..Default::default()
                },
            )]
            .into(),
        );
    }
    assert_eq!(
        hp,
        r.players
            .values()
            .map(|h| (h.hp, h.x, h.y, h.bombs, h.hand.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        serde_json::to_vec(&foe).unwrap(),
        serde_json::to_vec(&r.enemies).unwrap()
    );
}

#[test]
fn held_orbits_clouds_and_chill_reach_knight_targets() {
    let mut r = six();
    let mut card = r.book.get("bow").unwrap().clone();
    card.id = "arena-passives".into();
    card.kind = cards::Kind::Hold;
    card.arm = None;
    card.effects = vec![
        cards::Effect::Orbit(3),
        cards::Effect::Fart(1),
        cards::Effect::Chill(1),
    ];
    r.book.insert(card);
    r.players
        .get_mut(&1)
        .unwrap()
        .deck
        .push("arena-passives".into());
    let mut s = Session::new(&r.players, &r.book);
    s.ticks = TITLE;
    ready_in(&mut s, 1, (15., 15.));
    s.fighters.get_mut(&1).unwrap().yaw = 0.;
    let orb = orbit_points(&s.fighters[&1].hero, 1, u64::from(TITLE))[0];
    ready_in(&mut s, 2, orb);
    let hp = s.fighters[&2].hero.hp;
    s.orbits(1);
    assert!(s.fighters[&2].hero.hp < hp);
    ready_in(&mut s, 2, (17., 15.));
    for _ in 0..4 {
        s.step(
            &[(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]
            .into(),
            &r.book,
            4,
        );
    }
    assert!(s.fields.iter().any(|f| f.owner == 1 && f.damage > 0));
    assert!(s.fighters[&2].hero.chilled > 0);
}

// --- targeted rounds, native against leg ---------------------------------------------------------
#[test]
fn the_tenth_kill_ends_the_round_and_the_podium_pays_the_pot() {
    let mut pair = Pair::new(party(3, &[]));
    pair.both(|run| {
        for (id, gold) in [(1u32, 100u32), (2, 100), (3, 5)] {
            let hero = run.players.get_mut(&id).unwrap();
            hero.carried = Default::default();
            hero.carried.add(Spoil::Gold, gold);
        }
    });
    start(&mut pair);
    pair.both(|run| {
        let s = run.deathmatch.as_mut().unwrap();
        s.ante = [(1, 10), (2, 10), (3, 10)].into();
        s.fighters.get_mut(&1).unwrap().kills = (dm::GOAL - 1) as u16;
        s.fighters.get_mut(&2).unwrap().kills = 4;
        ready(run, 1, (15., 13.));
        ready(run, 2, (17., 13.));
        ready(run, 3, (40., 21.));
        let s = run.deathmatch.as_mut().unwrap();
        s.fighters.get_mut(&1).unwrap().yaw = 0.;
        s.fighters.get_mut(&2).unwrap().hero.hp = 3;
    });
    let fire = Inputs::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..12 {
        pair.tick(&fire);
        if pair.round().is_some_and(|s| s.winner.is_some()) {
            break;
        }
    }
    let s = pair.round().unwrap();
    assert_eq!(s.winner, Some(1), "the tenth kill wins");
    assert_eq!(s.fighters[&1].kills, dm::GOAL as u16);
    assert_eq!(s.ended, 0);
    let gold = |pair: &Pair, id: u32| pair.leg.players[&id].carried.get(Spoil::Gold);
    assert_eq!(
        (gold(&pair, 1), gold(&pair, 2), gold(&pair, 3)),
        (100 - 10 + 25, 90, 0),
        "the pot is 10 + 10 + the 5 the third could pay"
    );
    assert_eq!(s.purse, 25);
    assert!(
        pair.leg.players[&1].laurel && !pair.leg.players[&2].laurel && !pair.leg.players[&3].laurel
    );
    for _ in 0..dm::PODIUM + 2 {
        pair.tick(&Inputs::new());
    }
    assert!(
        pair.leg.deathmatch.is_none(),
        "the podium ran its six seconds and everyone went home"
    );
}

/// Every champion's feats and the wishes that chill, burn, burst and cloud, at
/// arm's length with the target kept alive, so every consequence of a landed
/// shot runs and is compared.
#[test]
fn champions_and_wishes_match_the_native_rules_when_they_land() {
    let mut total = Seen::default();
    for (kit, keys) in [
        ("hammerer", vec![1u8, 2, 3, 0]),
        ("butcher", vec![1, 2, 3, 0]),
        ("alchemist", vec![1, 2, 3, 0]),
        ("scryglass", vec![1, 2, 3, 4]),
        ("arena-passives", vec![0, 4, 0]),
        ("arena-bursts", vec![0, 4]),
        ("arena-fires", vec![0, 3]),
        ("arena-gift", vec![0, 4, 2]),
        ("arena-fields", vec![1, 2, 3, 0]),
        ("galahad", vec![0, 4]),
    ] {
        let mut pair = Pair::new(party(2, &[kit]));
        pair.both(|run| run.seed = 5);
        start(&mut pair);
        pair.both(|run| {
            ready(run, 1, (15., 13.));
            ready(run, 2, (17.5, 13.));
            let s = run.deathmatch.as_mut().unwrap();
            s.fighters.get_mut(&1).unwrap().yaw = 0.;
        });
        for t in 0..150u32 {
            let key = keys[(t as usize / 8) % keys.len()];
            let mut inputs = Inputs::new();
            inputs.insert(
                1,
                Input {
                    fire: t % 3 != 0,
                    swing: t % 11 == 0,
                    cast: if t % 8 == 0 { key } else { 0 },
                    aim_x: 0,
                    ..Default::default()
                },
            );
            pair.both(|run| {
                let s = run.deathmatch.as_mut().unwrap();
                let f = s.fighters.get_mut(&2).unwrap();
                if t % 2 == 0 && f.alive() {
                    f.hero.hp = f.hero.max_hp;
                    f.hero.x = 17.5;
                    f.hero.y = 13.;
                }
                // The shooter stays half hurt, so a leech has something to mend.
                let hero = &mut s.fighters.get_mut(&1).unwrap().hero;
                hero.hp = hero.max_hp / 2;
            });
            pair.tick(&inputs);
        }
        total.add(&pair.seen);
    }
    eprintln!("{total:#?}");
    assert!(
        total.chilled > 0 && total.burning > 0,
        "{} {}",
        total.chilled,
        total.burning
    );
    assert!(
        total.slams > 0 && total.rots > 0,
        "slams {} rots {}",
        total.slams,
        total.rots
    );
    assert!(total.fields.len() >= 2, "{:?}", total.fields);
    assert!(total.healed > 0 || total.shots > 100);
}

/// The slam that lands the tenth kill ends the round in the middle of the
/// knight's own turn: the trigger they are holding fires nothing.
#[test]
fn a_slam_that_wins_the_round_ends_the_knights_turn_before_they_fire() {
    let mut pair = Pair::new(party(2, &["hammerer"]));
    start(&mut pair);
    pair.both(|run| {
        ready(run, 1, (15., 13.));
        ready(run, 2, (16.5, 13.));
        let s = run.deathmatch.as_mut().unwrap();
        s.fighters.get_mut(&1).unwrap().kills = (dm::GOAL - 1) as u16;
        s.fighters.get_mut(&1).unwrap().hero.champ.windup = 1;
        s.fighters.get_mut(&1).unwrap().yaw = 0.;
        s.fighters.get_mut(&2).unwrap().hero.hp = 1;
    });
    pair.tick(&Inputs::from([(
        1,
        Input {
            fire: true,
            swing: true,
            ..Default::default()
        },
    )]));
    let s = pair.round().unwrap();
    assert_eq!(s.winner, Some(1), "the slam took the tenth");
    assert!(
        s.shots.is_empty(),
        "and the shot the knight held never left"
    );
    assert_eq!(s.fighters[&1].flash, 0);
}

/// A ward of any ticks left turns every blow; a blow on the spent ward lands.
#[test]
fn a_warded_knight_turns_every_blow_until_the_ward_is_spent() {
    let mut pair = Pair::new(party(2, &[]));
    start(&mut pair);
    for protection in [0u32, 1, 2, 59, 60] {
        for hits in 0..2 {
            pair.both(|run| {
                ready(run, 1, (15., 13.));
                ready(run, 2, (17., 13.));
                run.deathmatch
                    .as_mut()
                    .unwrap()
                    .fighters
                    .get_mut(&2)
                    .unwrap()
                    .protection = protection;
            });
            let (mut a, mut b) = (
                pair.native.deathmatch.take().unwrap(),
                pair.leg.deathmatch.take().unwrap(),
            );
            let way = (1., 0.);
            let (x, y) = (
                a.native_hit(1, 2, 7 + hits, way),
                b.hit(1, 2, 7 + hits, way),
            );
            assert_eq!(x, y, "protection {protection}");
            assert_eq!(x, protection == 0, "protection {protection}");
            assert_eq!(
                serde_json::to_value(&a).unwrap(),
                serde_json::to_value(&b).unwrap(),
                "protection {protection}"
            );
            (pair.native.deathmatch, pair.leg.deathmatch) = (Some(a), Some(b));
        }
    }
}

#[test]
fn the_farthest_spawn_wins_and_a_tie_keeps_the_lowest_index() {
    engine();
    let ask = |clear: Vec<f32>| -> u64 {
        let answer: serde_json::Value = angel_delve::scripted::step(
            "deathmatch-rules",
            &serde_json::json!({"op": "spawn", "clear": clear}),
        )
        .unwrap();
        answer["index"].as_u64().unwrap()
    };
    assert_eq!(ask(vec![100.; 8]), 0, "nobody about: the first spawn");
    assert_eq!(ask(vec![0.; 8]), 0);
    assert_eq!(
        ask(vec![5., 9., 9., 2., 9., 1., 9., 9.]),
        1,
        "a tie for farthest keeps the lowest"
    );
    assert_eq!(ask(vec![5., 9., 8.9, 2., 9.5, 1., 9.5, 9.]), 4);
    assert_eq!(ask(vec![1., 2., 3., 4., 5., 6., 7., 100.]), 7);
}

#[test]
fn the_standings_break_ties_by_deaths_then_seat() {
    let mut r = party(4, &[]);
    engine();
    r.step(&Inputs::new());
    let s = r.deathmatch.as_mut().unwrap();
    for (id, kills, deaths) in [(1u32, 3u16, 2u16), (2, 3, 1), (3, 5, 9), (4, 3, 1)] {
        let f = s.fighters.get_mut(&id).unwrap();
        (f.kills, f.deaths) = (kills, deaths);
    }
    assert_eq!(s.order(), vec![3, 2, 4, 1]);
}

// --- the interlude: Knights Wish as a crossroads card -----------------------------------------------
fn stairs(depth: u32, seats: u32) -> Run {
    engine();
    let mut run = Run::new(47, 9, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.dungeon = crawl_floor(depth, Pack::Crypt, 47);
    run.at = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap_or(0);
    run.boss_gates = None;
    run.enemies.clear();
    run.phase = Phase::Exploring;
    run.floor_plates.clear();
    for room in &mut run.dungeon.rooms {
        room.cleared = true;
    }
    run
}

fn vote(run: &mut Run, id: u32, choice: u8) {
    let serial = run.playtest.seats.get(&id).map_or(1, |s| s.serial + 1);
    run.step(&BTreeMap::from([(
        id,
        Input {
            vote: Some(Command {
                serial,
                action: Action::Rate(choice),
            }),
            ..Default::default()
        },
    )]));
}

fn deathmatch_card() -> Game {
    serde_json::from_str("\"deathmatch\"").expect("the pool must deserialize the deathmatch card")
}

fn deathmatch_round(seats: u32) -> Run {
    engine();
    let mut run = stairs(3, seats);
    run.players.get_mut(&2).unwrap().hp = 0;
    for h in run.players.values_mut() {
        h.carried.add(Spoil::Gold, 200);
    }
    run.descend();
    run.crossroads.as_mut().unwrap().choices = [deathmatch_card(), Game::Powder, Game::Gander];
    for id in 1..=seats {
        vote(&mut run, id, 1);
    }
    run
}

#[test]
fn deathmatch_seats_six_including_the_fallen_and_ends_on_the_crossroads_clock() {
    let mut run = deathmatch_round(6);
    assert_eq!(run.crossroads.as_ref().unwrap().stage, Stage::Playing);
    assert!(run.valid_snapshot(), "launch");
    assert!(!run.predictable(1));
    assert_eq!(run.deathmatch.as_ref().unwrap().fighters.len(), 6);
    run.crossroads.as_mut().unwrap().left = 1;
    run.step(&BTreeMap::new());
    assert_eq!(run.crossroads.as_ref().unwrap().stage, Stage::Results);
    assert_eq!(run.crossroads.as_ref().unwrap().order.len(), 6);
    assert!(run.valid_snapshot(), "results");
    for _ in 0..RESULTS_TIME {
        run.step(&BTreeMap::new());
    }
    assert!(run.crossroads.is_none());
    assert_eq!(run.dungeon.depth, 4);
    assert!(
        run.players[&2].hp > 0,
        "the usual descent revives the fallen"
    );
    assert_eq!(
        run.bank
            .iter()
            .filter(|h| h.why == "crossroads prize")
            .count(),
        6
    );
}

#[test]
fn deathmatch_launches_the_whole_roster_and_ticks_borrowed_bodies() {
    let mut run = deathmatch_round(6);
    let s = run
        .deathmatch
        .as_mut()
        .expect("launch from dungeon, not the town door");
    assert_eq!(
        s.fighters.keys().copied().collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
    assert!(s.fighters[&2].hero.hp > 0);
    assert!(s.ante.is_empty());
    s.ticks = dm::TITLE;
    let old = s.fighters[&2].yaw;
    run.step(&BTreeMap::from([(
        2,
        Input {
            aim_x: 1,
            ..Default::default()
        },
    )]));
    assert_ne!(run.deathmatch.as_ref().unwrap().fighters[&2].yaw, old);
    assert_eq!(run.players[&2].hp, 0);
    assert!(run.valid_snapshot());
    assert!(run.pastimes.heat.is_none());
    assert!(run.race3d.is_none());
}

#[test]
fn deathmatch_ten_kills_ranks_by_kills_deaths_and_seat_and_only_pays_crossroads() {
    let mut run = deathmatch_round(6);
    let carried: Vec<_> = run.players.values().map(|h| h.carried.clone()).collect();
    let wishes = run.players[&3].wishes;
    let s = run.deathmatch.as_mut().unwrap();
    for (id, kills, deaths) in [(1, 7, 2), (2, 7, 1), (3, 10, 4), (4, 7, 1)] {
        let f = s.fighters.get_mut(&id).unwrap();
        f.kills = kills;
        f.deaths = deaths;
    }
    s.winner = Some(3);
    // Even a restored town stake must not pay in the crossroads.
    s.ante = (1..=6).map(|id| (id, 20)).collect();
    s.ended = dm::PODIUM;
    run.step(&BTreeMap::new());
    let c = run.crossroads.as_ref().unwrap();
    assert_eq!(c.stage, Stage::Results);
    assert_eq!(c.order, vec![3, 2, 4, 1, 5, 6]);
    let prizes: Vec<_> = run
        .bank
        .iter()
        .filter(|h| h.why == "crossroads prize")
        .map(|h| (h.hero, h.spoils.get(Spoil::Gold)))
        .collect();
    assert_eq!(
        prizes,
        vec![(3, 100), (2, 60), (4, 30), (1, 30), (5, 30), (6, 30)]
    );
    assert_eq!(
        run.players
            .values()
            .map(|h| h.carried.clone())
            .collect::<Vec<_>>(),
        carried
    );
    assert_eq!(run.players[&3].wishes, wishes + 1);
    assert!(run.players[&3].laurel);
    assert_eq!(run.dungeon.depth, 3, "no town return");
    let bytes = serde_json::to_vec(&run).unwrap();
    let mut run: Run = serde_json::from_slice(&bytes).unwrap();
    for _ in 0..RESULTS_TIME {
        run.step(&BTreeMap::new());
    }
    assert!(run.crossroads.is_none());
    assert!(run.deathmatch.is_none());
    assert_eq!(run.dungeon.depth, 4);
    assert!(run.players[&2].hp > 0);
    assert_eq!(
        run.bank
            .iter()
            .filter(|h| h.why == "crossroads prize")
            .count(),
        6
    );
    assert!(run.valid_snapshot());
}

#[test]
fn deathmatch_cap_uses_the_crossroads_clock_even_with_everyone_away() {
    let run = deathmatch_round(2);
    assert_eq!(run.crossroads.as_ref().unwrap().left, 3 * 60 * HZ);
    let mut run: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    for h in run.players.values_mut() {
        h.away = true;
    }
    run.crossroads.as_mut().unwrap().left = 1;
    run.step(&BTreeMap::new());
    assert_eq!(run.crossroads.as_ref().unwrap().stage, Stage::Results);
    assert_eq!(run.crossroads.as_ref().unwrap().order, vec![1, 2]);
    assert!(run.valid_snapshot());
}

#[test]
fn deathmatch_real_tenth_frag_ends_early_without_taking_any_town_stakes() {
    let mut run = deathmatch_round(2);
    let carried: Vec<_> = run.players.values().map(|h| h.carried.clone()).collect();
    let s = run.deathmatch.as_mut().unwrap();
    s.ticks = dm::TITLE;
    s.ante = BTreeMap::from([(1, 20), (2, 20)]);
    let f = s.fighters.get_mut(&1).unwrap();
    (f.hero.x, f.hero.y, f.yaw) = (13., 23., 0.);
    f.kills = dm::GOAL - 1;
    f.protection = 0;
    f.hero.fire_cooldown = 0;
    let f = s.fighters.get_mut(&2).unwrap();
    (f.hero.x, f.hero.y) = (17., 23.);
    f.hero.hp = 1;
    f.protection = 0;
    for _ in 0..2 * HZ {
        run.step(&BTreeMap::from([(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]));
        if run.crossroads.as_ref().unwrap().stage == Stage::Results {
            break;
        }
    }
    assert_eq!(run.crossroads.as_ref().unwrap().stage, Stage::Results);
    assert_eq!(run.crossroads.as_ref().unwrap().order, vec![1, 2]);
    assert_eq!(
        run.players
            .values()
            .map(|h| h.carried.clone())
            .collect::<Vec<_>>(),
        carried
    );
    assert!(run.deathmatch.is_none());
    assert_eq!(run.dungeon.depth, 3);
    assert_eq!(
        run.bank
            .iter()
            .filter(|h| h.why == "crossroads prize")
            .count(),
        2
    );
}

/// The same rounds as the crossroads plays them (no town stake, no late joiners),
/// native against leg.
#[test]
fn crossroads_rounds_match_the_native_rules_tick_for_tick() {
    let mut seen = Seen::default();
    for (seed, seats) in [(3u64, 3u32), (4, 6), (5, 2)] {
        let mut pair = Pair::new(deathmatch_round(seats));
        pair.both(|run| {
            run.seed = seed;
            let s = run.deathmatch.as_mut().unwrap();
            s.ticks = dm::TITLE;
            for (i, f) in s.fighters.values_mut().enumerate() {
                f.protection = 0;
                f.kills = if i == 0 { (dm::GOAL - 2) as u16 } else { 0 };
            }
            s.ante = (1..=seats).map(|id| (id, 20)).collect();
        });
        let ids: Vec<u32> = (1..=seats).collect();
        let mut pilot = Pilot::new(seed);
        for t in 0..900u32 {
            if t % 60 == 0 {
                pair.both(|run| gather(run, &ids, if t % 120 == 0 { 13. } else { 21. }));
                pair.both(|run| {
                    for f in run
                        .deathmatch
                        .as_mut()
                        .unwrap()
                        .fighters
                        .values_mut()
                        .filter(|f| f.alive())
                    {
                        f.hero.hp = f.hero.max_hp;
                    }
                });
            }
            let inputs = pilot.inputs(&ids);
            pair.tick(&inputs);
        }
        seen.add(&pair.seen);
        assert!(
            pair.native.players.values().all(|h| !h.laurel),
            "the crossroads pays no town stake, native or leg"
        );
    }
    assert!(
        seen.kills >= 5 && seen.wins >= 1,
        "kills {} wins {}",
        seen.kills,
        seen.wins
    );
}

// --- measurements (ignored) ------------------------------------------------------------------------
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
static CALLS: AtomicU64 = AtomicU64::new(0);
static ENGINE_NS: AtomicU64 = AtomicU64::new(0);
static IN_BYTES: AtomicU64 = AtomicU64::new(0);
static OUT_BYTES: AtomicU64 = AtomicU64::new(0);
static SIZING: AtomicBool = AtomicBool::new(false);
static OPS: std::sync::Mutex<BTreeMap<String, (u64, u64)>> = std::sync::Mutex::new(BTreeMap::new());

fn counting_engine(id: &str, input: &serde_json::Value) -> Result<serde_json::Value, String> {
    let at = Instant::now();
    let out = live_policy::decide_shipped::<serde_json::Value>(id, input, |_| Ok(())).0;
    let spent = at.elapsed().as_nanos() as u64;
    ENGINE_NS.fetch_add(spent, Relaxed);
    CALLS.fetch_add(1, Relaxed);
    if SIZING.load(Relaxed) {
        IN_BYTES.fetch_add(input.to_string().len() as u64, Relaxed);
        OUT_BYTES.fetch_add(out.to_string().len() as u64, Relaxed);
        let mut ops = OPS.lock().unwrap();
        let entry = ops
            .entry(input["op"].as_str().unwrap_or("?").to_string())
            .or_default();
        entry.0 += 1;
        entry.1 += spent;
    }
    Ok(out)
}

/// This thread's CPU time, which a busy machine inflates far less than the wall clock.
fn cpu_ns() -> u128 {
    let mut at = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut at) };
    at.tv_sec as u128 * 1_000_000_000 + at.tv_nsec as u128
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

/// Four knights in a running fight: the native tick against the leg's, per tick.
#[test]
#[ignore = "measures the leg: --ignored --nocapture per_tick_cost"]
fn per_tick_cost_at_four_knights() {
    angel_delve::scripted::install(counting_engine);
    let seats = 4u32;
    let ids: Vec<u32> = (1..=seats).collect();
    let mut native = party(seats, &["", "arena-gift", "arena-bursts", "alchemist"]);
    native.seed = 77;
    let mut leg = native.clone();
    let setup = |run: &mut Run| {
        run.native_tick_deathmatch(&Inputs::new());
        let s = run.deathmatch.as_mut().unwrap();
        s.ticks = dm::TITLE;
        for f in s.fighters.values_mut() {
            f.protection = 0;
        }
    };
    setup(&mut native);
    setup(&mut leg);
    let total = 1500u32;
    let mut pilot = Pilot::new(77);
    let inputs: Vec<Inputs> = (0..total).map(|_| pilot.inputs(&ids)).collect();
    let cost = |sizing: bool| {
        SIZING.store(sizing, Relaxed);
        CALLS.store(0, Relaxed);
        ENGINE_NS.store(0, Relaxed);
        IN_BYTES.store(0, Relaxed);
        OUT_BYTES.store(0, Relaxed);
        let mut times: [Vec<u128>; 4] = Default::default();
        let (mut a, mut b) = (native.clone(), leg.clone());
        for (t, input) in inputs.iter().enumerate() {
            if t % 70 == 0 {
                for run in [&mut a, &mut b] {
                    gather(run, &ids, if t % 140 == 0 { 13. } else { 21. });
                    for f in run.deathmatch.as_mut().unwrap().fighters.values_mut() {
                        (f.hero.hp, f.kills) = (f.hero.max_hp, 0);
                    }
                }
            }
            let (wall, cpu) = (Instant::now(), cpu_ns());
            a.native_tick_deathmatch(input);
            times[0].push(wall.elapsed().as_nanos());
            times[1].push(cpu_ns() - cpu);
            let (wall, cpu) = (Instant::now(), cpu_ns());
            b.tick_deathmatch(input);
            times[2].push(wall.elapsed().as_nanos());
            times[3].push(cpu_ns() - cpu);
        }
        times
    };
    // The floor of one call on this machine now: the smallest question the leg answers.
    let floor = |n: usize| {
        let ask = serde_json::json!({"op": "begin", "winner": false, "ended": 0, "connected": 4, "ticks": 90, "pickups": [0, 0, 0, 0, 0]});
        let (mut wall, mut cpu) = (Vec::new(), Vec::new());
        for _ in 0..n {
            let (at, c) = (Instant::now(), cpu_ns());
            let _ = counting_engine("deathmatch-rules", &ask).unwrap();
            wall.push(at.elapsed().as_nanos());
            cpu.push(cpu_ns() - c);
        }
        wall.sort_unstable();
        cpu.sort_unstable();
        (wall, cpu)
    };
    let (before, before_cpu) = floor(3000);
    let mut times = cost(false);
    let (after, after_cpu) = floor(3000);
    let (calls, engine_ns) = (CALLS.load(Relaxed), ENGINE_NS.load(Relaxed));
    for (name, v) in [
        ("wall", &before),
        ("cpu", &before_cpu),
        ("wall", &after),
        ("cpu", &after_cpu),
    ] {
        eprintln!(
            "one call, smallest question, {name}: median {:.1} us  p95 {:.1} us",
            percentile(v, 0.5) as f64 / 1e3,
            percentile(v, 0.95) as f64 / 1e3
        );
    }
    for t in &mut times {
        t.sort_unstable();
    }
    let budget = 1_000_000_000u128 / u128::from(rules::HZ);
    let line = |name: &str, v: &[u128]| {
        eprintln!(
            "{name:>11}: median {:>7.1} us  p95 {:>7.1} us  p99 {:>7.1} us  max {:>8.1} us  = {:.3}% of the {} us frame at the median, {:.3}% at p95",
            percentile(v, 0.5) as f64 / 1e3,
            percentile(v, 0.95) as f64 / 1e3,
            percentile(v, 0.99) as f64 / 1e3,
            *v.last().unwrap() as f64 / 1e3,
            percentile(v, 0.5) as f64 * 100.0 / budget as f64,
            budget / 1000,
            percentile(v, 0.95) as f64 * 100.0 / budget as f64,
        );
    };
    line("native wall", &times[0]);
    line("native cpu", &times[1]);
    line("leg wall", &times[2]);
    line("leg cpu", &times[3]);
    eprintln!(
        "leg: {:.2} engine calls a tick, {:.1} us a tick inside the engine",
        calls as f64 / f64::from(total),
        engine_ns as f64 / 1e3 / f64::from(total)
    );
    let _ = cost(true);
    let sized = CALLS.load(Relaxed).max(1);
    eprintln!(
        "payload: {:.0} bytes in and {:.0} bytes out per call; {:.0} bytes in and {:.0} bytes out per tick",
        IN_BYTES.load(Relaxed) as f64 / sized as f64,
        OUT_BYTES.load(Relaxed) as f64 / sized as f64,
        IN_BYTES.load(Relaxed) as f64 / f64::from(total),
        OUT_BYTES.load(Relaxed) as f64 / f64::from(total),
    );
    for (op, (n, ns)) in OPS.lock().unwrap().iter() {
        eprintln!(
            "  {op:>10}: {n:>6} calls, {:>6.1} us each, {:.2} a tick",
            *ns as f64 / 1e3 / *n as f64,
            *n as f64 / f64::from(total)
        );
    }
}
