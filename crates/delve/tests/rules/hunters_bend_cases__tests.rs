//! Rust oracle for the Bend port of rules/hunters.rs (delve/PORTING.md).
//! Run with `--features bend-cases-games-a-small --lib hunters_bend_cases -- --ignored`.
use super::*;
use serde_json::{Value, json};

#[path = "games_a_bend_cases__tests.rs"]
mod cases;

fn jv<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}

fn full(run: &Run) -> Value {
    crate::rules::runtime_state::capture_for_rules(run).unwrap().run
}

/// The keys of the run a function changes, for its `out`.
fn pick(run: &Value, keys: &[&str]) -> Value {
    let mut out = serde_json::Map::new();
    for k in keys {
        if let Some(v) = run.get(*k) {
            out.insert((*k).to_string(), v.clone());
        }
    }
    Value::Object(out)
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 10_000.0
    }
}

fn with(seed: u64, extra: usize, kind: EnemyKind, x: f32, y: f32) -> Run {
    let mut run = Run::new(seed, 1, if extra > 0 { Some("Two") } else { None });
    for n in 1..extra {
        run.join(&format!("K{}", n + 2));
    }
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.spawn_at_for_test(kind, x, y);
    let mut at = 0.0;
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (24.0 + at, 14.0);
        hero.invulnerable = 0;
        at += 2.0;
    }
    run.phase = Phase::Fighting;
    run
}

fn hold(run: &mut Run, keys: &BTreeMap<u32, Input>) {
    for hero in run.players.values_mut() {
        hero.hp = hero.max_hp;
    }
    run.step(keys);
}

fn deeds_json(d: &foes::Deeds) -> Value {
    json!({
        "hooks": d.hooks.iter().map(|h| json!([h.0, h.1, h.2, [h.3.0, h.3.1]])).collect::<Vec<_>>(),
        "webs": d.webs.iter().map(|w| json!([w.0, w.1])).collect::<Vec<_>>(),
        "rot": d.rot.iter().map(|w| json!([w.0, w.1])).collect::<Vec<_>>(),
        "spawns": d.spawns.iter().map(|s| json!([jv(&s.0), s.1, s.2, s.3])).collect::<Vec<_>>(),
        "cues": d.cues,
    })
}

#[test]
#[ignore]
fn record_hunters() {
    let mut r = Lcg(0xfeed_beef_1234_5678);
    let mut sizes = 0usize;
    for seed in 0..40u64 {
        for extra in [0usize, 1, 3] {
            for kind in [EnemyKind::Flesher, EnemyKind::Silkmother] {
                let (ex, ey) = (30.0 + r.unit() * 12.0, 6.0 + r.unit() * 16.0);
                let mut run = with(seed, extra, kind, ex, ey);
                for t in 0..7 * HZ {
                    // A knight now and then steps off the line or rolls.
                    let keys: BTreeMap<u32, Input> = run
                        .players
                        .keys()
                        .map(|&id| {
                            (id, Input { move_x: r.below(3) as i8 - 1, move_y: r.below(3) as i8 - 1, dash: r.below(40) == 0, ..Default::default() })
                        })
                        .collect();
                    // act on every hunter, as the room walks them.
                    let room = run.room();
                    let barred = run.barred();
                    let grid = Grid { room, barred };
                    let heroes: Vec<(f32, f32)> =
                        run.players.values().filter(|h| h.targetable()).map(|h| (h.x, h.y)).collect();
                    for e in run.enemies.clone() {
                        if cases::count("hunters.act") >= 500 || r.below(100) != 0 {
                            continue;
                        }
                        let mut en = e.clone();
                        let mut deeds = foes::Deeds::default();
                        let before = jv(&en);
                        let handled = act(&mut en, &heroes, &grid, &mut deeds);
                        cases::rec_cap(
                            "hunters.act",
                            &json!({"enemy": before, "heroes": heroes.iter().map(|h| json!([h.0, h.1])).collect::<Vec<_>>(), "room": jv(room), "barred": barred}),
                            &json!({"enemy": jv(&en), "deeds": deeds_json(&deeds), "handled": handled}),
                            500,
                        );
                    }
                    // tick_hunters on a copy, when something hangs in the air or sits on the floor.
                    if cases::count("hunters.Run.tick_hunters") < 90 && (!run.hooks.is_empty() || !run.webs.is_empty() || t % 40 == 0) && r.below(25) == 0 {
                        let mut c = run.clone();
                        let before = full(&c);
                        c.tick_hunters();
                        let after = full(&c);
                        let keys = ["hooks", "webs", "players", "room_hurt", "fx", "sounds", "cues", "feats"];
                        sizes = sizes.max(before.to_string().len());
                        cases::rec_cap(
                            "hunters.Run.tick_hunters",
                            &json!({"run": before}),
                            &pick(&after, &keys),
                            90,
                        );
                    }
                    // brood_full, webbed
                    if t % 25 == 0 {
                        cases::rec_cap(
                            "hunters.Run.brood_full",
                            &json!({"run": pick(&full(&run), &["enemies"])}),
                            &run.brood_full(),
                            60,
                        );
                        let (x, y) = (run.players[&1].x + r.unit() * 4.0 - 2.0, run.players[&1].y + r.unit() * 4.0 - 2.0);
                        cases::rec_cap(
                            "hunters.Run.webbed",
                            &json!({"webs": jv(&run.webs), "x": x, "y": y}),
                            &Run::webbed(&run.webs, x, y),
                            80,
                        );
                    }
                    hold(&mut run, &keys);
                }
            }
        }
    }
    // hunt: hooks, webs and rot applied to runs with party members around.
    for seed in 0..30u64 {
        for extra in [0usize, 1, 3] {
            let mut run = with(seed, extra, EnemyKind::Flesher, 36.0, 14.0);
            for _ in 0..r.below(8) {
                run.webs.push(hunters::Web { x: r.unit() * 40.0, y: r.unit() * 20.0, left: 5 + r.below(200) });
            }
            for _ in 0..r.below(5) {
                run.hooks.push(hunters::Hook { by: 1, x: 30.0, y: 14.0, dir: (1.0, 0.0), out: 3.0, back: false, caught: None });
            }
            for hero in run.players.values_mut() {
                hero.hp = 1 + r.below(30);
            }
            let hooks: Vec<_> = (0..r.below(6)).map(|i| (i, 20.0 + r.unit() * 10.0, 14.0, (0.0, 1.0))).collect();
            let webs: Vec<_> = (0..r.below(9)).map(|_| (r.unit() * 40.0, r.unit() * 20.0)).collect();
            let rot: Vec<_> = (0..r.below(5)).map(|_| (22.0 + r.unit() * 6.0, 12.0 + r.unit() * 4.0)).collect();
            let before = full(&run);
            let slim = pick(&before, &["hooks", "webs", "players", "room_hurt"]);
            run.hunt(hooks.clone(), webs.clone(), rot.clone());
            let after = pick(&full(&run), &["hooks", "webs", "players", "room_hurt"]);
            cases::rec_cap(
                "hunters.Run.hunt",
                &json!({
                    "run": slim,
                    "hooks": hooks.iter().map(|h| json!([h.0, h.1, h.2, [h.3.0, h.3.1]])).collect::<Vec<_>>(),
                    "webs": webs.iter().map(|w| json!([w.0, w.1])).collect::<Vec<_>>(),
                    "rot": rot.iter().map(|w| json!([w.0, w.1])).collect::<Vec<_>>(),
                }),
                &after,
                200,
            );
        }
    }
    eprintln!("hunters: biggest run json {sizes} bytes");
}
