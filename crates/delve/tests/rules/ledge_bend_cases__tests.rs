//! Rust oracle for the Bend port of rules/ledge.rs (delve/PORTING.md): records
//! `{"fn", "in", "out"}` cases under `$DELVE_CASES`. Run with
//! `--features bend-cases-games-a-small --lib ledge_bend_cases -- --ignored`.
use super::*;
use serde_json::{Value, json};

#[path = "games_a_bend_cases__tests.rs"]
mod cases;

fn jv<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
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
}

fn halls(run: &Run) -> Vec<usize> {
    (0..run.dungeon.rooms.len())
        .filter(|&i| matches!(run.dungeon.rooms[i].kind, RoomKind::Ledge | RoomKind::Wanted))
        .collect()
}

/// The parts of a run `side_on` and `arrive_side_on` and `perch` read.
fn slim(run: &Run) -> Value {
    json!({
        "at": run.at,
        "dungeon": { "rooms": jv(&run.dungeon.rooms) },
        "players": jv(&run.players),
        "rng": jv(&run.rng),
    })
}

fn party(seed: u64, extra: usize) -> Run {
    let mut run = Run::new(seed, 1, if extra > 0 { Some("Two") } else { None });
    for n in 1..extra {
        run.join(&format!("K{}", n + 2));
    }
    run
}

#[test]
#[ignore]
fn record_ledge() {
    let mut r = Lcg(0x9e37_79b9_7f4a_7c15);
    let mut halls_seen = 0;
    for seed in 0..60u64 {
        for extra in [0usize, 1, 3] {
            let mut run = party(seed, extra);
            // Floor 1 .. 3: every side-on hall of each.
            for depth in 0..3 {
                if depth > 0 {
                    run.descend_for_test();
                }
                for at in halls(&run) {
                    halls_seen += 1;
                    run.enter(at, Some(3));
                    run.calm_for_test();
                    run.enemies.clear();
                    let ids: Vec<u32> = run.players.keys().copied().collect();
                    cases::rec_cap(
                        "ledge.Run.side_on",
                        &json!({"run": slim(&run)}),
                        &run.side_on(),
                        200,
                    );
                    for _ in 0..90 {
                        // Each knight's step, with a random key, on a copy.
                        for &id in &ids {
                            if cases::count("ledge.walk") >= 700 || r.below(10) != 0 {
                                continue;
                            }
                            let room = run.room();
                            let barred = run.barred();
                            let grid = Grid { room, barred };
                            let mut hero = run.players[&id].clone();
                            let input = Input {
                                move_x: r.below(3) as i8 - 1,
                                move_y: if r.below(5) == 0 { -1 } else if r.below(9) == 0 { 1 } else { 0 },
                                ..Default::default()
                            };
                            let dx = [0.0f32, 0.25, -0.25, 0.6, -0.6, 1.1, -1.1][r.below(7) as usize];
                            let rolling = r.below(7) == 0;
                            let before = jv(&hero);
                            let fell = walk(&mut hero, &input, &grid, dx, rolling);
                            cases::rec_cap(
                                "ledge.walk",
                                &json!({"hero": before, "input": jv(&input), "room": jv(room), "barred": barred, "dx": dx, "rolling": rolling}),
                                &json!({"hero": jv(&hero), "fell": fell}),
                                700,
                            );
                        }
                        for (i, enemy) in run.enemies.clone().iter().enumerate() {
                            if i > 2 {
                                break;
                            }
                            let room = run.room();
                            let barred = run.barred();
                            let grid = Grid { room, barred };
                            let mut e = enemy.clone();
                            let before = jv(&e);
                            fall(&mut e, &grid);
                            cases::rec_cap(
                                "ledge.fall",
                                &json!({"enemy": before, "room": jv(room), "barred": barred}),
                                &jv(&e),
                                200,
                            );
                        }
                        let mut keys = BTreeMap::new();
                        for &id in &ids {
                            keys.insert(
                                id,
                                Input {
                                    move_x: r.below(3) as i8 - 1,
                                    move_y: if r.below(6) == 0 { -1 } else { 0 },
                                    ..Default::default()
                                },
                            );
                        }
                        run.step(&keys);
                        for hero in run.players.values_mut() {
                            hero.hp = hero.max_hp;
                        }
                    }
                    // stand_on on this room's columns.
                    let room = run.room().clone();
                    for _ in 0..4 {
                        let x = (r.below(room.cols as u32 * 100) as f32) / 100.0 * TILE_UNITS;
                        let from = (r.below(room.rows as u32 * 100) as f32) / 100.0 * TILE_UNITS;
                        cases::rec_cap(
                            "ledge.stand_on",
                            &json!({"room": jv(&room), "x": x, "from": from}),
                            &stand_on(&room, x, from),
                            300,
                        );
                    }
                    // arrive_side_on and perch from the hall's door.
                    let mut arrive = run.clone();
                    let before = slim(&arrive);
                    arrive.arrive_side_on();
                    cases::rec_cap(
                        "ledge.Run.arrive_side_on",
                        &json!({"run": before}),
                        &json!({"players": jv(&arrive.players)}),
                        120,
                    );
                    for flies in [false, true] {
                        for _ in 0..3 {
                            let mut p = run.clone();
                            let before = slim(&p);
                            let pos = p.perch(flies);
                            cases::rec_cap(
                                "ledge.Run.perch",
                                &json!({"run": before, "flies": flies}),
                                &json!({"rng": jv(&p.rng), "pos": [pos.0, pos.1]}),
                                200,
                            );
                            run.rng = p.rng.clone();
                        }
                    }
                }
            }
        }
    }
    // The kinds that are not halls answer false.
    for seed in 0..10u64 {
        let run = Run::new(seed, 1, None);
        cases::rec_cap("ledge.Run.side_on", &json!({"run": slim(&run)}), &run.side_on(), 220);
    }
    eprintln!("ledge: {halls_seen} halls");
}
