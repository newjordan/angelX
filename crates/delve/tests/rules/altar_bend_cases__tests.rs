//! Rust oracle for the Bend port of rules/altar.rs (delve/PORTING.md).
//! Run with `--features bend-cases-games-a-small --lib altar_bend_cases -- --ignored`.
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

/// The floor's parts `keeps_an_altar` reads.
fn thin(floor: &Floor) -> Value {
    json!({
        "depth": floor.depth,
        "rooms": floor.rooms.iter().map(|r| json!({"kind": jv(&r.kind)})).collect::<Vec<_>>(),
        "open": floor.open.as_ref().map(|o| json!({"map": o.map})),
    })
}

fn party(seed: u64, extra: usize) -> Run {
    let mut run = Run::new(seed, 1, if extra > 0 { Some("Two") } else { None });
    for n in 1..extra {
        run.join(&format!("K{}", n + 2));
    }
    run
}

fn e(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

#[test]
#[ignore]
fn record_altar() {
    let mut r = Lcg(0xa17a_2026_1020_0001);
    // stone_blocks: pure.
    for _ in 0..400 {
        let (c, rr) = (r.below(40) as i32, r.below(30) as i32);
        let (tc, tr) = (c + r.below(7) as i32 - 3, rr + r.below(7) as i32 - 3);
        cases::rec_cap("altar.stone_blocks", &json!({"c": c, "r": rr, "tc": tc, "tr": tr}), &stone_blocks(c, rr, tc, tr), 400);
    }
    for open in [false, true] {
        crate::settings::set(crate::settings::Settings { open_floors: open, ..crate::settings::get() });
        for seed in 0..30u64 {
            for extra in [0usize, 1, 3] {
                let mut run = party(seed, extra);
                for depth in 0..3 {
                    if depth > 0 {
                        run.descend_for_test();
                    }
                    let floor = run.dungeon.clone();
                    for identity in [false, true] {
                        for index in 0..floor.rooms.len() {
                            if index % 3 == 0 || floor.open.as_ref().is_some_and(|o| o.map == index) {
                                cases::rec_cap(
                                    "altar.keeps_an_altar",
                                    &json!({"floor": thin(&floor), "index": index, "identity": identity}),
                                    &keeps_an_altar(&floor, index, identity),
                                    300,
                                );
                            }
                        }
                    }
                    // place_on: every room still to choose.
                    let mut bare = floor.clone();
                    bare.altar_at.clear();
                    let before = jv(&bare);
                    let mut placed = bare.clone();
                    place_on(&mut placed, false);
                    cases::rec_cap(
                        "altar.place_on",
                        &json!({"floor": before, "identity": false}),
                        &json!({"altar_at": jv(&placed.altar_at)}),
                        40,
                    );
                    // The pieces of the search, on the rooms that keep one.
                    for index in 0..floor.rooms.len() {
                        if !keeps_an_altar(&floor, index, false) {
                            continue;
                        }
                        let room = &floor.rooms[index];
                        let (arrival, extra_pts) = match &floor.open {
                            Some(o) if o.map == index => (Some(o.arrival), o.chambers.iter().map(|c| c.centre()).collect::<Vec<_>>()),
                            _ => (None, Vec::new()),
                        };
                        cases::rec_cap(
                            "altar.place",
                            &json!({"room": jv(room), "arrival": arrival.map(|a| json!([a.0, a.1])), "extra": extra_pts.iter().map(|p| json!([p.0, p.1])).collect::<Vec<_>>()}),
                            &place(room, arrival, &extra_pts).map(|p| json!([p.0, p.1])),
                            60,
                        );
                        cases::rec_cap(
                            "altar.spot",
                            &json!({"room": jv(room), "arrival": arrival.map(|a| json!([a.0, a.1]))}),
                            &spot(room, arrival).map(|p| json!([p.0, p.1])),
                            60,
                        );
                        for _ in 0..3 {
                            let (c, rr) = (4 + r.below(room.cols as u32 - 8) as i32, 4 + r.below(room.rows as u32 - 8) as i32);
                            cases::rec_cap(
                                "altar.stone_cuts_a_passage",
                                &json!({"room": jv(room), "c": c, "r": rr}),
                                &stone_cuts_a_passage(room, c, rr),
                                120,
                            );
                        }
                        if let Some(from) = arrival {
                            let on = route_tiles(room, from, &extra_pts);
                            cases::rec_cap(
                                "altar.route_tiles",
                                &json!({"room": jv(room), "from": [from.0, from.1], "extra": extra_pts.iter().map(|p| json!([p.0, p.1])).collect::<Vec<_>>()}),
                                &on,
                                40,
                            );
                            for _ in 0..4 {
                                let (c, rr) = (4 + r.below(room.cols as u32 - 8) as i32, 4 + r.below(room.rows as u32 - 8) as i32);
                                cases::rec_cap(
                                    "altar.stone_stands_on_a_route",
                                    &json!({"on": on, "cols": room.cols, "c": c, "r": rr}),
                                    &stone_stands_on_a_route(&on, room.cols as i32, c, rr),
                                    120,
                                );
                            }
                        }
                    }
                    // Run level: the altar for the room the party is in.
                    for at in 0..run.dungeon.rooms.len().min(8) {
                        let mut c = run.clone();
                        c.identity = at % 5 == 4;
                        c.at = at;
                        let slim = json!({"at": c.at, "identity": c.identity, "dungeon": jv(&c.dungeon), "players": jv(&c.players)});
                        cases::rec_cap("altar.Run.altar_due", &json!({"run": slim}), &c.altar_due(), 60);
                        cases::rec_cap("altar.Run.wish_altar", &json!({"run": slim}), &c.wish_altar().map(|p| json!([p.0, p.1])), 80);
                        let ids: Vec<u32> = c.players.keys().copied().collect();
                        for id in ids {
                            if r.below(3) == 0 {
                                if let Some((x, y)) = c.wish_altar() {
                                    let h = c.players.get_mut(&id).unwrap();
                                    (h.x, h.y) = (x + r.unit() * 4.0 - 2.0, y + r.unit() * 4.0 - 2.0);
                                }
                            }
                            let slim = json!({"at": c.at, "identity": c.identity, "dungeon": jv(&c.dungeon), "players": jv(&c.players)});
                            cases::rec_cap("altar.Run.near_altar", &json!({"run": slim, "id": id}), &c.near_altar(id), 80);
                        }
                        let mut d = c.clone();
                        let before = json!({"at": d.at, "identity": d.identity, "dungeon": jv(&d.dungeon)});
                        d.choose_altars();
                        cases::rec_cap("altar.Run.choose_altars", &json!({"run": before}), &json!({"altar_at": jv(&d.dungeon.altar_at)}), 40);
                    }
                }
            }
        }
    }
    crate::settings::set(crate::settings::Settings { open_floors: false, ..crate::settings::get() });
    // tick_wish_altar: knights walking round the lamp, pressing E, with and without wishes.
    for seed in 0..40u64 {
        for extra in [0usize, 1, 3] {
            let mut run = party(seed, extra);
            let ids: Vec<u32> = run.players.keys().copied().collect();
            for &id in &ids {
                let h = run.players.get_mut(&id).unwrap();
                h.wishes = r.below(3) as u8;
            }
            for t in 0..70 {
                let keys: BTreeMap<u32, Input> = ids
                    .iter()
                    .map(|&id| {
                        (id, Input { interact: r.below(3) != 0, move_x: r.below(3) as i8 - 1, move_y: r.below(3) as i8 - 1, ..Default::default() })
                    })
                    .collect();
                if t % 2 == 0 {
                    if let Some((x, y)) = run.wish_altar() {
                        for &id in &ids {
                            if r.below(2) == 0 {
                                let h = run.players.get_mut(&id).unwrap();
                                (h.x, h.y) = (x + r.unit() * 6.0 - 3.0, y + r.unit() * 4.0 - 2.0);
                            }
                        }
                    }
                }
                if cases::count("altar.Run.tick_wish_altar") < 140 && r.below(60) == 0 {
                    let mut c = run.clone();
                    let before = full(&c);
                    c.tick_wish_altar(&keys);
                    let after = full(&c);
                    cases::rec_cap(
                        "altar.Run.tick_wish_altar",
                        &json!({"run": before, "inputs": jv(&keys)}),
                        &pick(&after, &["players", "lamp", "found", "sounds", "dungeon"]),
                        140,
                    );
                }
                run.step(&keys);
                for h in run.players.values_mut() {
                    h.hp = h.max_hp;
                }
            }
        }
    }
    // The lamp's book-keeping and its three host hooks.
    let words = ["a wide bow", "fire", "make me fast", "bees"];
    for seed in 0..50u64 {
        let mut run = party(seed, (seed % 3) as usize);
        for step in 0..60 {
            run.tick += 1;
            let id = 1 + r.below(run.players.len() as u32);
            let w = words[r.below(4) as usize];
            let lamp0 = jv(&run.lamp);
            let tick0 = run.tick;
            match r.below(3) {
                0 => {
                    run.lamp_sent(id, w);
                    cases::rec_cap("altar.Run.lamp_sent", &json!({"run": {"lamp": lamp0, "tick": tick0}, "id": id, "words": w}), &json!({"lamp": jv(&run.lamp)}), 120);
                }
                1 => {
                    run.lamp_landed(id, w, "Quick Boots");
                    cases::rec_cap(
                        "altar.Run.lamp_landed",
                        &json!({"run": {"lamp": lamp0, "tick": tick0}, "id": id, "words": w, "name": "Quick Boots"}),
                        &json!({"lamp": jv(&run.lamp)}),
                        120,
                    );
                }
                _ => {
                    run.lamp_dropped(id, w);
                    cases::rec_cap("altar.Run.lamp_dropped", &json!({"run": {"lamp": lamp0, "tick": tick0}, "id": id, "words": w}), &json!({"lamp": jv(&run.lamp)}), 120);
                }
            }
            cases::rec_cap("altar.Lamp.quiet", &json!({"lamp": jv(&run.lamp)}), &run.lamp.quiet(), 80);
            let ft = run.tick + r.below(300) as u64;
            cases::rec_cap(
                "altar.Lamp.flaring",
                &json!({"lamp": jv(&run.lamp), "tick": ft}),
                &run.lamp.flaring(ft).map(|l| jv(l)),
                80,
            );
            let who = 1 + r.below(3);
            cases::rec_cap(
                "altar.Lamp.pending",
                &json!({"lamp": jv(&run.lamp), "id": who}),
                &run.lamp.pending(who).map(jv).collect::<Vec<_>>(),
                80,
            );
            if step % 7 == 6 {
                run.tick += 90;
            }
        }
    }
}
