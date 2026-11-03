//! The core group's Bend case recorders (feature `bend-cases-core`): Rust
//! originals run over varied real states, each case a line
//! `{"fn", "in", "out"}` written to `$DELVE_CASES/<fn>.jsonl`
//! (`DELVE_CASES` is required) and replayed by
//! `python3 delve/tests/cases.py core`.
#![allow(dead_code)]

use super::*;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    std::env::var_os("DELVE_CASES").map(PathBuf::from).expect("set DELVE_CASES to the Bend case directory")
}

/// The cases of one Bend def, written whole on `save`.
pub struct Cases {
    name: String,
    lines: Vec<String>,
}

impl Cases {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), lines: Vec::new() }
    }
    pub fn add(&mut self, input: Value, output: Value) {
        let line = json!({"fn": self.name, "in": input, "out": output});
        self.lines.push(serde_json::to_string(&line).unwrap());
    }
    pub fn len(&self) -> usize {
        self.lines.len()
    }
    pub fn save(&self) {
        let dir = dir();
        std::fs::create_dir_all(&dir).unwrap();
        let mut text = self.lines.join("\n");
        text.push('\n');
        std::fs::write(dir.join(format!("{}.jsonl", self.name)), text).unwrap();
        eprintln!("{}: {} cases", self.name, self.lines.len());
    }
}

/// The Run as `capture_for_rules` writes it.
pub fn state(run: &Run) -> Value {
    runtime_state::capture_for_rules(run).unwrap().run
}

/// An object cut to the keys a case needs (absent keys stay absent).
pub fn keep(v: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    if let Value::Object(m) = v {
        for k in keys {
            if let Some(x) = m.get(*k) {
                out.insert((*k).to_string(), x.clone());
            }
        }
    }
    Value::Object(out)
}

pub fn to<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}

/// xorshift64: the recorders' own dice.
pub struct Dice(pub u64);
impl Dice {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    pub fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
    pub fn axis(&mut self) -> i8 {
        self.below(3) as i8 - 1
    }
}

/// Random keys: mostly walking, some fire, the odd rarer key.
pub fn random_input(d: &mut Dice) -> Input {
    let mut i = Input::default();
    if d.chance(70) {
        i.move_x = d.axis();
        i.move_y = d.axis();
    }
    if d.chance(40) {
        i.aim_x = d.axis();
        i.aim_y = d.axis();
    }
    i.fire = d.chance(35);
    i.dash = d.chance(8);
    i.bomb = d.chance(3);
    i.swing = d.chance(5);
    if d.chance(6) {
        i.play = 1 + d.below(4) as u8;
    }
    if d.chance(6) {
        i.cast = 1 + d.below(4) as u8;
    }
    i.vigil = d.chance(1);
    i.ult = d.chance(2);
    i.take = d.chance(6);
    i.interact = d.chance(10);
    if d.chance(4) {
        i.skill = 1 + d.below(5) as u8;
    }
    i.ride = d.chance(2);
    i.cycle = d.chance(3);
    i
}

pub fn random_inputs(run: &Run, d: &mut Dice) -> BTreeMap<u32, Input> {
    run.players.keys().map(|&id| (id, random_input(d))).collect()
}

/// A delve in some state: `party` knights, `floors` floors down, walked about
/// at random for `steps` ticks.
pub fn scene(seed: u64, party: u32, floors: u32, steps: u32) -> Run {
    let mut run = Run::new(seed, seed ^ 0x51ed, None);
    for id in 2..=party {
        run.join_seat(id, &format!("knight{id}"));
    }
    run.kit_out_for_test();
    for _ in 0..floors {
        run.descend_for_test();
    }
    let mut d = Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    for _ in 0..steps {
        let inputs = random_inputs(&run, &mut d);
        run.step(&inputs);
    }
    run
}

/// Scenes in all sorts: the corpus every recorder draws from.
pub fn scenes(n: u64) -> Vec<Run> {
    (0..n)
        .map(|k| {
            let seed = 1000 + k * 7;
            scene(seed, 1 + (k % 4) as u32, (k / 4 % 3) as u32, (k * 37 % 240) as u32)
        })
        .collect()
}

#[test]
#[ignore]
fn record_core_helpers() {
    let mut d = Dice(0xc0ffee);
    // Input::valid / pressing.
    let mut valid = Cases::new("rules.Input.valid");
    let mut pressing = Cases::new("rules.Input.pressing");
    for k in 0..200 {
        let mut i = random_input(&mut d);
        if k % 5 == 0 {
            i.move_x = d.below(5) as i8 - 2;
        }
        if k % 7 == 0 {
            i.play = d.below(7) as u8;
        }
        if k % 11 == 0 {
            i.cast = d.below(7) as u8;
            i.skill = d.below(8) as u8;
        }
        if k % 13 == 0 {
            i.leave_activity = true;
            i.activity_menu = d.chance(50);
            i.leave_held = d.chance(50);
            i.skip_plate = d.chance(50);
            i.again = d.chance(50);
        }
        valid.add(to(&i), json!(i.valid()));
        pressing.add(to(&i), json!(i != Input::default()));
    }
    valid.add(to(&Input::default()), json!(true));
    pressing.add(to(&Input::default()), json!(false));
    valid.save();
    pressing.save();
    // command::lay_over.
    let mut lay = Cases::new("command.lay_over");
    for _ in 0..150 {
        let (a, b) = (random_input(&mut d), random_input(&mut d));
        lay.add(json!({"under": to(&a), "over": to(&b)}), to(&command::lay_over(a, &b)));
    }
    lay.save();
    // Run helpers over real scenes.
    let mut cs: BTreeMap<&str, Cases> = BTreeMap::new();
    for name in [
        "rules.Run.active", "again.Run.over", "rules.Run.floor", "rules.Run.room", "rules.Run.at_entrance",
        "rules.Run.foes_left", "rules.Run.party_len", "rules.Run.party_size", "rules.Run.scaled", "rules.Run.away",
    ] {
        cs.insert(name, Cases::new(name));
    }
    for (k, mut run) in scenes(60).into_iter().enumerate() {
        let s = state(&run);
        if k % 5 == 3 {
            run.players.values_mut().next().unwrap().away = true;
        }
        if k % 7 == 2 {
            run.phase = if k % 2 == 0 { Phase::Won } else { Phase::Wiped };
        }
        let s = if k % 5 == 3 || k % 7 == 2 { state(&run) } else { s };
        let add = |cs: &mut BTreeMap<&str, Cases>, name: &str, keys: &[&str], out: Value| {
            cs.get_mut(name).unwrap().add(keep(&s, keys), out);
        };
        add(&mut cs, "rules.Run.active", &["phase"], json!(run.active()));
        add(&mut cs, "again.Run.over", &["phase"], json!(run.over()));
        add(&mut cs, "rules.Run.floor", &["dungeon"], json!(run.floor()));
        add(&mut cs, "rules.Run.at_entrance", &["dungeon", "at"], json!(run.at_entrance()));
        add(&mut cs, "rules.Run.foes_left", &["enemies"], json!(run.foes_left()));
        add(&mut cs, "rules.Run.party_len", &["players"], json!(run.party_len()));
        add(&mut cs, "rules.Run.party_size", &["players"], json!(run.party_size()));
        add(&mut cs, "rules.Run.scaled", &["players", "scaled_for"], json!(run.scaled()));
        let room = to(run.room());
        cs.get_mut("rules.Run.room").unwrap().add(keep(&s, &["dungeon", "at"]), room);
        for (&id, _) in &run.players {
            cs.get_mut("rules.Run.away").unwrap().add(
                json!({"run": keep(&s, &["players", "idle"]), "id": id}),
                json!(run.away(id)),
            );
        }
    }
    for c in cs.values() {
        c.save();
    }
}

/// Hero `id` as the state tree holds it.
pub fn hero_json(run: &Run, id: u32) -> Value {
    state(run)["players"][id.to_string()].clone()
}

/// Run `f` on a hero (a clone of the run's) and answer the hero as it stands after.
pub fn with_hero(run: &Run, id: u32, f: impl FnOnce(&mut Hero)) -> Value {
    let mut r = run.clone();
    f(r.players.get_mut(&id).unwrap());
    hero_json(&r, id)
}

fn pick_hero(run: &Run, d: &mut Dice) -> u32 {
    let ids: Vec<u32> = run.players.keys().copied().collect();
    ids[d.below(ids.len() as u64) as usize]
}

#[test]
#[ignore]
fn record_core_state() {
    let mut d = Dice(0xfeed);
    let scenes = scenes(48);
    // Hero::hurt / blow over knights in all sorts of kit.
    let mut hurt = Cases::new("hero.Hero.hurt");
    let mut blow = Cases::new("hero.Hero.blow");
    let mut hurt_most = Cases::new("hero.Hero.hurt_most");
    for (k, run) in scenes.iter().enumerate() {
        for rep in 0..4 {
            let id = pick_hero(run, &mut d);
            let mutate = |h: &mut Hero, d: &mut Dice| {
                h.armor = d.below(3) as u32;
                h.glass = d.chance(25);
                h.incoming = [60, 100, 120, 140, 200][d.below(5) as usize];
                h.owned.body = d.chance(70);
                h.owned.helmet = d.chance(70);
                h.invulnerable = if d.chance(20) { d.below(30) as u32 } else { 0 };
                h.hit_ward = 45;
                if d.chance(15) {
                    h.stone = true;
                }
                if d.chance(10) {
                    h.hp = 0;
                }
                if d.chance(10) {
                    h.flee_ticks = 5;
                }
                if d.chance(30) {
                    h.hp = 1 + d.below(h.max_hp as u64) as u32;
                }
            };
            let mut seed_run = run.clone();
            mutate(seed_run.players.get_mut(&id).unwrap(), &mut d);
            let before = hero_json(&seed_run, id);
            let damage = 1 + d.below(120) as u32;
            let out = with_hero(&seed_run, id, |h| h.hurt(damage));
            hurt.add(json!({"h": before, "damage": damage}), out);
            let most = 1 + d.below(80) as u32;
            let out = with_hero(&seed_run, id, |h| h.hurt_most(damage, most));
            hurt_most.add(json!({"h": before, "damage": damage, "most": most}), out);
            let tag = if rep % 2 == 0 { 0 } else { 1 + d.below(3) as u32 };
            let mut blown = false;
            let out = with_hero(&seed_run, id, |h| blown = h.blow(damage, tag));
            blow.add(json!({"h": before, "damage": damage, "tag": tag}), json!({"h": out, "struck": blown}));
            let _ = k;
        }
    }
    hurt.save();
    hurt_most.save();
    blow.save();
    // Hitstop.
    let mut inputs_c = Cases::new("hitstop.State.inputs");
    let mut finish_c = Cases::new("hitstop.Run.finish_hitstop");
    for (k, run) in scenes.iter().enumerate() {
        for rep in 0..6 {
            let mut r = run.clone();
            let ids: Vec<u32> = r.players.keys().copied().collect();
            let mut buffered = serde_json::Map::new();
            if rep % 3 == 1 {
                for &id in ids.iter().take(2) {
                    buffered.insert(id.to_string(), to(&random_input(&mut d)));
                }
                buffered.insert("5".into(), to(&random_input(&mut d)));
            }
            r.hitstop = serde_json::from_value(json!({
                "left": if rep % 2 == 0 { d.below(4) } else { 0 },
                "cooldown": d.below(16),
                "held": d.chance(40),
                "buffered": buffered,
                "pending": d.below(5),
            }))
            .unwrap();
            if rep == 5 {
                r.tune = serde_json::from_value(json!({"hitstop": 0})).unwrap();
            }
            let inputs: BTreeMap<u32, Input> = r.players.keys().map(|&id| (id, random_input(&mut d))).collect();
            let s = state(&r);
            let (mut st, tune, players) = (r.hitstop.clone(), r.tune, r.players.clone());
            let resume = st.inputs(&tune, &players, &inputs);
            let resume_j = match resume {
                hitstop::Resume::Wait => json!("wait"),
                hitstop::Resume::Same => json!("same"),
                hitstop::Resume::Fresh(m) => json!({"fresh": to(&m)}),
            };
            let out_state = if st.quiet() { json!({}) } else { json!({"hitstop": to(&st)}) };
            inputs_c.add(
                json!({"run": keep(&s, &["hitstop", "tune"]), "ids": ids, "inputs": to(&inputs)}),
                json!({"state": out_state, "resume": resume_j}),
            );
            // finish.
            let mut r2 = r.clone();
            r2.shake = d.below(6) as u32;
            let s2 = state(&r2);
            r2.finish_hitstop();
            finish_c.add(keep(&s2, &["hitstop", "tune", "shake"]), keep(&state(&r2), &["hitstop", "shake"]));
            let _ = k;
        }
    }
    inputs_c.save();
    finish_c.save();
}

#[test]
#[ignore]
fn record_core_step_helpers() {
    let mut d = Dice(0xbeef);
    let scenes = scenes(60);
    let mut idle_c = Cases::new("rules.Run.count_idle");
    let mut rescale_c = Cases::new("rules.Run.rescale_party");
    let mut turbo_c = Cases::new("fortune.Run.turbo");
    let mut harden_c = Cases::new("fortune.Run.harden");
    let mut collapse_c = Cases::new("fortune.Run.tick_collapse");
    let mut marks_c = Cases::new("trophies.Run.mark_trophies");
    let mut show_c = Cases::new("fx.Run.show_fx");
    let mut present_c = Cases::new("fx.Run.present");
    let mut age_c = Cases::new("gore.Run.age_gore");
    let mut hits_c = Cases::new("desperate.Run.mark_hits");
    for (k, run) in scenes.into_iter().enumerate() {
        let mut run = run;
        // count_idle: random inputs, some invalid or default.
        let inputs: BTreeMap<u32, Input> = run
            .players
            .keys()
            .filter(|_| d.chance(85))
            .map(|&id| (id, if d.chance(30) { Input::default() } else { random_input(&mut d) }))
            .collect();
        run.idle.insert(1, d.below(700) as u32);
        let s = state(&run);
        let mut r2 = run.clone();
        r2.count_idle(&inputs);
        idle_c.add(json!({"run": keep(&s, &["players", "idle"]), "inputs": to(&inputs)}), keep(&state(&r2), &["idle"]));
        // rescale_party: the party changes under the monsters.
        let mut r3 = run.clone();
        let before = r3.scaled();
        if k % 3 == 0 {
            if let Some(h) = r3.players.values_mut().next() {
                h.away = true;
            }
        }
        if k % 4 == 1 && r3.players.len() > 1 {
            let last = *r3.players.keys().last().unwrap();
            r3.players.get_mut(&last).unwrap().absent = true;
        }
        let s3 = state(&r3);
        r3.rescale_party(before);
        rescale_c.add(
            json!({"run": keep(&s3, &["players", "enemies", "scaled_for"]), "before": before}),
            keep(&state(&r3), &["enemies", "scaled_for"]),
        );
        // modes.
        let mut r4 = run.clone();
        let modes = ["turbo", "glass_jaw", "collapse", "ironman", "all_random", "long_way_down"];
        r4.mode = serde_json::from_value(json!(modes[k % modes.len()])).unwrap();
        r4.tick = (k as u64 * 7 + d.below(40)) | if k % 2 == 0 { 3 } else { 0 };
        let s4 = state(&r4);
        turbo_c.add(keep(&s4, &["mode", "tick", "dungeon"]), json!(r4.turbo()));
        r4.harden();
        harden_c.add(keep(&s4, &["mode", "players"]), keep(&state(&r4), &["players"]));
        // the collapse.
        let mut r5 = run.clone();
        r5.mode = serde_json::from_value(json!(if k % 5 == 4 { "turbo" } else { "collapse" })).unwrap();
        let at = r5.tick + d.below(60);
        r5.collapse_at = if k % 7 == 6 { None } else { Some(at) };
        r5.tick = at.saturating_sub([0, 1, 30 * 30, 2 * 30, 5][k % 5] as u64) + d.below(2);
        r5.rocks.truncate(d.below(12) as usize);
        let s5 = state(&r5);
        r5.tick_collapse();
        collapse_c.add(
            keep(&s5, &["mode", "collapse_at", "tick", "phase", "players", "rocks", "rng", "dungeon", "at", "fx", "cues", "kingdom", "exits"]),
            keep(&state(&r5), &["rocks", "rng", "fx", "cues"]),
        );
        // trophies.
        let mut r6 = run.clone();
        let names = ["victory", "grail", "boss_fall:cinderjaw", "boss_fall:the-index", "knight_down", "low_hp", "elite"];
        let heard = r6.cues.len();
        for n in 0..(1 + d.below(5)) {
            r6.cues.push(names[((k as u64 + n) % names.len() as u64) as usize].into());
        }
        r6.marks.insert("trophy:dragon".into(), d.below(3) as u32);
        let s6 = state(&r6);
        r6.mark_trophies(heard);
        marks_c.add(json!({"run": keep(&s6, &["cues", "marks"]), "heard": heard}), keep(&state(&r6), &["marks"]));
        // fx.
        let mut r7 = run.clone();
        r7.shake = d.below(8) as u32;
        r7.sparks = (0..d.below(33)).map(|i| Spark { from: (1.0, 2.0), to: (3.0 + i as f32, 4.0), ttl: d.below(4) as u8 }).collect();
        for n in 0..d.below(6) {
            r7.fx.push(if n % 2 == 0 { Fx::Shake(d.below(12) as u32) } else { Fx::Spark(Spark { from: (n as f32, 0.5), to: (2.0, 3.0), ttl: 5 }) });
        }
        let s7 = state(&r7);
        let mut r8 = r7.clone();
        r7.show_fx();
        show_c.add(keep(&s7, &["shake", "fx", "sparks"]), keep(&state(&r7), &["shake", "fx", "sparks"]));
        // present also ages gore and echoes.
        r8.tick = 500 + d.below(300);
        r8.feel.echoes = (0..d.below(5)).map(|i| (400 + d.below(300) + i, "hit")).collect();
        let s8 = state(&r8);
        r8.present();
        present_c.add(
            keep(&s8, &["shake", "fx", "sparks", "gore", "feel", "sounds", "tick"]),
            keep(&state(&r8), &["shake", "fx", "sparks", "gore", "feel", "sounds"]),
        );
        let _ = &mut age_c;
        // mark_hits.
        let mut r9 = run.clone();
        for e in r9.enemies.iter_mut() {
            if d.chance(50) {
                e.hp = e.hp.saturating_sub(d.below(20) as u32);
            }
        }
        let s9 = state(&r9);
        r9.mark_hits();
        hits_c.add(keep(&s9, &["enemies"]), keep(&state(&r9), &["enemies"]));
    }
    for c in [&idle_c, &rescale_c, &turbo_c, &harden_c, &collapse_c, &marks_c, &show_c, &present_c, &hits_c] {
        c.save();
    }
}
