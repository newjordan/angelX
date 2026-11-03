//! Rust oracle for the Bend port of rules/aerie.rs (delve/PORTING.md).
//! Run with `--features bend-cases-games-a-small --lib aerie_bend_cases -- --ignored`.
use super::*;
use crate::rules::{home::Home, world};
use serde_json::{Value, json};

#[path = "games_a_bend_cases__tests.rs"]
mod cases;

fn jv<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}

fn full(run: &Run) -> Value {
    crate::rules::runtime_state::capture_for_rules(run).unwrap().run
}

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

fn in_the(kind: RoomKind, seed: u64, extra: usize) -> Run {
    let mut run = Run::at_home(seed, 1, None, Home::default(), Spoils::default());
    for n in 0..extra {
        run.join_seat(2 + n as u32, &format!("Two{n}"));
    }
    run.marks.insert(QUEST_MARKS[0].into(), 1);
    run.arrive(world::room_of(&run.dungeon, kind).unwrap(), (12.0, 7.0));
    run
}

#[test]
#[ignore]
fn record_aerie() {
    let mut r = Lcg(0xae71_e000_0000_0001);
    let kinds = [RoomKind::Belfry, RoomKind::Aqueduct, RoomKind::Aviary, RoomKind::Rookery];
    // pigeon_at / flying / progress: pure.
    for seed in 0..12u64 {
        for kind in kinds {
            let run = in_the(kind, seed, 1);
            let room = run.room();
            for _ in 0..6 {
                let i = r.below(3) as usize;
                let tick = if r.below(4) == 0 { (r.next() as u64) << r.below(12) } else { r.below(5000) as u64 };
                cases::rec_cap("aerie.pigeon_at", &json!({"room": jv(room), "i": i, "tick": tick}), &json!([pigeon_at(room, i, tick).0, pigeon_at(room, i, tick).1]), 300);
            }
            for _ in 0..4 {
                let (stage, papers) = (r.below(3) as u8, r.below(8) as u8);
                let p = if r.below(3) == 0 { None } else { Some(Pigeons { dropped: if r.below(2) == 0 { Some((r.below(3) as u8, 10.0, 8.0)) } else { None }, ..Default::default() }) };
                cases::rec_cap(
                    "aerie.flying",
                    &json!({"kind": jv(&kind), "stage": stage, "papers": papers, "pigeons": p.as_ref().map(jv)}),
                    &flying(kind, stage, papers, p.as_ref()),
                    200,
                );
            }
            let mut shaped = room.clone();
            shape(&mut shaped);
            cases::rec_cap("aerie.shape", &json!({"room": jv(room)}), &jv(&shaped), 40);
        }
    }
    for _ in 0..80 {
        let mut run = in_the(RoomKind::Belfry, r.below(9) as u64, 0);
        run.home.countryside.post = r.below(3) as u8;
        run.home.countryside.minutes = r.below(8) as u8;
        let mut marks = BTreeMap::new();
        for m in QUEST_MARKS {
            if r.below(2) == 0 {
                marks.insert(m.to_string(), 1u32);
            }
        }
        let c = &run.home.countryside;
        let (stage, papers) = progress(c, &marks);
        cases::rec_cap(
            "aerie.progress",
            &json!({"country": {"post": c.post, "minutes": c.minutes}, "marks": jv(&marks)}),
            &json!([stage, papers]),
            120,
        );
    }
    for _ in 0..80 {
        let mut p = Pigeons::default();
        for id in 1..=r.below(8) {
            p.bonks.insert(id, r.below(5) as u8);
            p.poop.insert(id + r.below(2), r.below(10) as u8);
        }
        if r.below(2) == 0 {
            p.dropped = Some((r.below(4) as u8, r.unit() * 300.0 - 20.0, r.unit() * 220.0 - 10.0));
        }
        cases::rec_cap("aerie.Pigeons.valid", &json!({"pigeons": jv(&p)}), &p.valid(), 80);
    }
    // tick_aerie: the whole quest, in each place, with shots and swings and E.
    for seed in 0..40u64 {
        for kind in kinds {
            for extra in [0usize, 1, 3] {
                let mut run = in_the(kind, seed, extra);
                let ids: Vec<u32> = run.players.keys().copied().collect();
                if r.below(3) == 0 {
                    run.marks.insert(QUEST_MARKS[1 + r.below(3) as usize].into(), 1);
                }
                for t in 0..3 * LAP {
                    // Knights wander near the pigeon's path, the clerk, or a dropped paper.
                    let (px, py) = pigeon_at(run.room(), r.below(3) as usize, run.tick + 1);
                    for &id in &ids {
                        if r.below(5) == 0 {
                            let h = run.players.get_mut(&id).unwrap();
                            (h.x, h.y) = match r.below(3) {
                                0 => (px + r.unit() * 3.0 - 1.5, py + r.unit() * 3.0 - 1.5),
                                1 => (CLERK.0 * TILE_UNITS + r.unit() * 2.0 - 1.0, CLERK.1 * TILE_UNITS + r.unit() * 2.0 - 1.0),
                                _ => (h.x + r.unit() * 6.0 - 3.0, h.y + r.unit() * 4.0 - 2.0),
                            };
                            h.swing = if r.below(4) == 0 { 3 } else { 0 };
                        }
                    }
                    if r.below(7) == 0 {
                        let by = ids[r.below(ids.len() as u32) as usize];
                        run.projectiles.push(crate::rules::Projectile {
                            x: px,
                            y: py,
                            vx: 0.0,
                            vy: 0.0,
                            hostile: false,
                            kind: crate::rules::Shot::Arrow,
                            look: None,
                            damage: 1,
                            pierce: 0,
                            last_hit: None,
                            ttl: 5,
                            empowered: false,
                            traits: crate::rules::ShotTraits { owner: by as u8, ..Default::default() },
                        });
                    }
                    let keys: BTreeMap<u32, Input> =
                        ids.iter().map(|&id| (id, Input { interact: r.below(2) == 0, ..Default::default() })).collect();
                    run.tick += 1;
                    let mut c = run.clone();
                    let record = cases::count("aerie.Run.tick_aerie") < 170 && r.below(250) == 0;
                    let before = if record { Some(full(&c)) } else { None };
                    c.tick_aerie(&keys);
                    if let Some(before) = before {
                        let after = full(&c);
                        cases::rec_cap(
                            "aerie.Run.tick_aerie",
                            &json!({"run": before, "inputs": jv(&keys)}),
                            &pick(&after, &["players", "pigeons", "marks", "found", "sounds", "projectiles"]),
                            170,
                        );
                    }
                    // Go on from the result so the quest advances.
                    run = c;
                    if t % 90 == 89 && r.below(2) == 0 {
                        run.home.countryside.post = 1;
                    }
                }
            }
        }
    }
}
