//! Scripted rides to hold the module to this code: bots riding a table through
//! the whole road, random keys with knights teleported about, coming and going,
//! and knights set down just before every thing on the road. A scene hands every
//! ask to `step` and carries on from the answer it gets back, so the caller
//! decides what an ask is answered by and what is checked on the way.
use super::level;
use serde_json::{Value, json};

pub type Step<'a> = &'a mut dyn FnMut(Value) -> Value;

/// A deterministic dice.
pub struct Dice(pub u64);
impl Dice {
    pub fn next(&mut self, m: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % m
    }
}

pub fn ask(op: &str, rest: Value) -> Value {
    let mut a = rest;
    a["op"] = json!(op);
    a
}

/// Put knight `id` on the main road at column `x` with ground speed `gsp`.
pub fn put(session: &mut Value, id: u32, x: i32, gsp: i32) {
    let lvl = level::level();
    let p = &lvl.terrain.paths[lvl.main];
    let s = p.s_at_x(x);
    let at = p.sample(s);
    let r = &mut session["seats"][id.to_string()]["rider"];
    r["x"] = json!(at.x);
    r["y"] = json!(at.y);
    r["ground"] = json!({"path": lvl.main, "s": s});
    r["gsp"] = json!(gsp);
    r["dir"] = json!(if gsp < 0 { -1 } else { 1 });
    r["vx"] = json!(0);
    r["vy"] = json!(0);
    r["cam_x"] = json!(x - 152);
    r["cam_y"] = json!(at.y / 256 - 134);
    session["dim"] = json!(-4000 * 256);
}

/// A random key press.
pub fn keys(d: &mut Dice) -> Value {
    json!({
        "move_x": d.next(3) as i64 - 1, "move_y": d.next(3) as i64 - 1, "aim_x": 0, "aim_y": d.next(5).min(2) as i64 - 1,
        "fire": d.next(6) == 0, "dash": d.next(4) == 0, "bomb": false, "interact": d.next(4) == 0,
    })
}

/// Bots ride a table through the tally and, when a knight asks, again.
pub fn bots(step: Step, seats: u32, route: Option<&str>, skill: Value, limit: u32) {
    let present: Vec<u32> = (1..=seats).collect();
    let mut session = step(ask("start", json!({"present": present})))["session"].clone();
    for t in 0..limit {
        let mut inputs = serde_json::Map::new();
        for id in 1..=seats {
            let mut a = ask("bot", json!({"session": session, "id": id}));
            if let Some(r) = route {
                a["route"] = json!(r);
            }
            if !skill.is_null() {
                a["skill"] = skill.clone();
            }
            inputs.insert(id.to_string(), step(a)["input"].clone());
        }
        // A knight asks to ride again at the tally.
        if session["tally"].is_object() && t % 2 == 0 {
            inputs.insert("1".into(), json!({"move_x": 0, "move_y": 0, "aim_x": 0, "aim_y": 0, "fire": false, "dash": false, "bomb": false, "interact": true}));
        }
        let tick = step(ask("tick", json!({"session": session, "inputs": inputs, "present": present})));
        session = tick["session"].clone();
        if session["again"] == json!(true) {
            session = step(ask("start", json!({"present": present})))["session"].clone();
        }
    }
}

/// Random keys, random teleports, knights coming and going.
pub fn fuzz(step: Step, seed: u64, seats: u32, limit: u32) {
    let mut d = Dice(seed);
    let mut present: Vec<u32> = (1..=seats).collect();
    let mut session = step(ask("start", json!({"present": present})))["session"].clone();
    for t in 0..limit {
        if t % 23 == 0 {
            for id in present.clone() {
                if session["seats"].get(id.to_string()).is_some() && d.next(3) == 0 {
                    let x = 100 + d.next(19_500) as i32;
                    put(&mut session, id, x, d.next(2600) as i32 - 300);
                }
            }
        }
        if t > 0 && t % 97 == 0 && seats > 1 {
            let id = 1 + d.next(u64::from(seats)) as u32;
            if present.contains(&id) && present.len() > 1 {
                present.retain(|&p| p != id)
            } else if !present.contains(&id) {
                present.push(id);
                present.sort()
            }
        }
        let mut inputs = serde_json::Map::new();
        for &id in &present {
            inputs.insert(id.to_string(), keys(&mut d));
        }
        let mut a = ask("tick", json!({"session": session, "inputs": inputs, "present": present}));
        if t % 5 == 0 {
            a["numbers"] = json!({"slope": 20 + d.next(280), "loop_min": 128 + d.next(1400), "spur_cap": 1280 + d.next(1800), "lock_reach": 32 + d.next(208), "dimming": d.next(2048)});
        }
        session = step(a)["session"].clone();
    }
}

/// Knights put just before each thing on the road, riding at it, and at each
/// loop's mouth and the fork's slow and fast.
pub fn features(step: Step) {
    let lvl = level::level();
    let main = &lvl.terrain.paths[lvl.main];
    let mut spots: Vec<(i32, Option<i32>)> = Vec::new();
    spots.extend(lvl.foes.iter().map(|f| (f.x, None)));
    spots.extend(lvl.springs.iter().map(|f| (f.x, None)));
    spots.extend(lvl.pads.iter().map(|f| (f.x, None)));
    spots.extend(lvl.posts.iter().map(|f| (f.x, None)));
    spots.extend(lvl.crates.iter().map(|f| (f.x, None)));
    spots.push((lvl.bell.0, None));
    for q in main.portals.iter().filter(|q| q.dir > 0) {
        let at = main.sample(q.s).x / 256;
        // The loop's mouth from a standing start falls off the ceiling; from a run-up it holds.
        let (near, slow, fast) = if q.min_speed > 0 { (40, 700, 1700) } else { (80, 0, 1500) };
        spots.push((at + near, Some(slow)));
        spots.push((at + 300, Some(fast)));
    }
    spots.sort();
    let mut d = Dice(0x51ed_270b_a5d3_7c1f);
    for (n, (x, speed)) in spots.into_iter().enumerate() {
        let seats = 1 + (n as u32 % 4);
        let present: Vec<u32> = (1..=seats).collect();
        let mut session = step(ask("start", json!({"present": present})))["session"].clone();
        for id in 1..=seats {
            put(&mut session, id, x - 80 - 40 * id as i32, speed.unwrap_or(600 + d.next(1800) as i32));
        }
        session["provisioned"] = json!(6);
        let go = if speed.is_some() { 1 } else { d.next(4) };
        for t in 0..70u64 {
            let mut inputs = serde_json::Map::new();
            for id in 1..=seats {
                let k = match go {
                    0 => json!({"move_x": 1, "move_y": if t > 6 { 1 } else { 0 }, "aim_x": 0, "aim_y": 0, "fire": false, "dash": false, "bomb": false}),
                    1 => json!({"move_x": 1, "move_y": 0, "aim_x": 0, "aim_y": 0, "fire": false, "dash": speed.is_none() && t % 9 == 0, "bomb": false, "interact": speed.is_none() && t % 11 == u64::from(id) % 11}),
                    2 => json!({"move_x": 1, "move_y": 0, "aim_x": 0, "aim_y": 0, "fire": true, "dash": false, "bomb": false, "interact": t % 6 == 0}),
                    _ => keys(&mut d),
                };
                inputs.insert(id.to_string(), k);
            }
            session = step(ask("tick", json!({"session": session, "inputs": inputs, "present": present})))["session"].clone();
        }
    }
}

/// The warden's gate, the bell, the tally and the ride again: knights set down a
/// little before the warden with the Dimming far behind, bots riding them on.
pub fn finale(step: Step, seats: u32, limit: u32) {
    let lvl = level::level();
    let warden = lvl.foes.iter().find(|f| f.kind == super::world::FoeKind::Warden).map_or(19_000, |f| f.x);
    let present: Vec<u32> = (1..=seats).collect();
    let mut session = step(ask("start", json!({"present": present})))["session"].clone();
    for id in 1..=seats {
        put(&mut session, id, warden - 420 - 50 * id as i32, 1500);
    }
    session["provisioned"] = json!(seats);
    let mut rounds = 0;
    for t in 0..limit {
        let mut inputs = serde_json::Map::new();
        for id in 1..=seats {
            let a = ask("bot", json!({"session": session, "id": id}));
            inputs.insert(id.to_string(), step(a)["input"].clone());
        }
        if session["tally"].is_object() {
            for id in 1..=seats {
                let held = t % 2 == 0 && id == 1;
                inputs.insert(id.to_string(), json!({"move_x": 0, "move_y": 0, "aim_x": 0, "aim_y": 0, "fire": false, "dash": false, "bomb": false, "interact": held}));
            }
        }
        session = step(ask("tick", json!({"session": session, "inputs": inputs, "present": present})))["session"].clone();
        if session["again"] == json!(true) {
            rounds += 1;
            session = step(ask("start", json!({"present": present})))["session"].clone();
            if rounds < 2 {
                for id in 1..=seats {
                    put(&mut session, id, warden - 420 - 50 * id as i32, 1500);
                }
                session["provisioned"] = json!(seats);
            }
        }
    }
}

/// The bots' rides, as the parity test and the dump run them.
pub fn bots_all(step: Step) {
    let mut step = step;
    bots(&mut step, 1, None, Value::Null, 6200);
    bots(&mut step, 2, Some("Upper"), Value::Null, 3000);
    bots(&mut step, 3, Some("Lower"), json!({"spur": false, "roll": false, "lag": 30, "hesitate": 240}), 3000);
    bots(&mut step, 4, None, json!({"spur": false, "roll": false, "lag": 40, "hesitate": 60}), 3000);
}

pub fn fuzz_all(step: Step) {
    let mut step = step;
    for (seed, seats) in [(11u64, 1u32), (23, 2), (37, 3), (41, 4), (59, 4), (61, 6), (73, 2)] {
        fuzz(&mut step, seed, seats, 1500);
    }
}

pub fn finale_all(step: Step) {
    let mut step = step;
    for seats in [1, 2, 4, 6] {
        finale(&mut step, seats, 1400);
    }
}
