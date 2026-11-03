//! The Lightgallop rules as a Bend module, held to the Rust they came from.
//!
//! The rules live in `cockpit/live/lightgallop-rules` and `crates/delve` keeps
//! only the session's types and an adapter that asks the module once a tick.
//! What is left of the Rust rules is `crates/delve/src/rules/lightgallop/oracle*`,
//! included here verbatim, and these tests tick the shipped module and that
//! oracle side by side: the road, a fresh ride, the whole session after every
//! tick of bots riding a table through the road and the tally and the ride
//! again, of random keys with knights teleported about and coming and going,
//! and of knights set down before every thing on the road, and the keys every
//! bot presses. The measurements at the end are `--ignored`.
use crate::agent::harness::live_policy::install_scripted_rules;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::Instant;

/// What the oracle's files take from the game around them.
mod host {
    pub use angel_delve::rules::lightgallop::{SOUNDS, join_taken, split_taken};
    pub use angel_delve::rules::{HZ, Hero, Input, MAX_PLAYERS};

    /// A knight to seat: the one a fresh run starts with.
    pub fn hero() -> Hero {
        static HERO: std::sync::OnceLock<Hero> = std::sync::OnceLock::new();
        HERO.get_or_init(|| angel_delve::rules::Run::new(1, 1, None).players.values().next().expect("a knight").clone()).clone()
    }
}

#[path = "../../../crates/delve/src/rules/lightgallop/oracle.rs"]
mod oracle;

use oracle::scenes::{self, Step};

const MODULE: &str = "lightgallop-rules";

/// The shipped module's answer, as the host takes it.
fn bend(ask: &Value) -> Value {
    install_scripted_rules();
    let mut answer = angel_delve::scripted::step(MODULE, ask)
        .unwrap_or_else(|error| panic!("{MODULE} refused {} {ask}: {error}", ask["op"]));
    if ask["op"] == "level" {
        angel_delve::rules::lightgallop::level::unpack_road(&mut answer).expect("valid road geometry");
    }
    answer
}

/// The oracle's answer to the same ask.
fn old(ask: &Value) -> Value {
    oracle::answer(MODULE, ask).unwrap_or_else(|error| panic!("the oracle refused {}: {error}", ask["op"]))
}

/// A bot's keys, as the rules read them: the fields that are not the default.
fn pressed(answer: &Value) -> Value {
    let input: host::Input = serde_json::from_value(answer["input"].clone()).expect("an input");
    serde_json::to_value(input).expect("an input")
}

/// The first place two answers part, for a failure worth reading.
fn first_difference(a: &Value, b: &Value, at: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<_> = x.keys().chain(y.keys()).collect();
            keys.into_iter().find_map(|k| first_difference(x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), &format!("{at}.{k}")))
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            x.iter().zip(y).enumerate().find_map(|(i, (p, q))| first_difference(p, q, &format!("{at}[{i}]")))
        }
        _ if a == b => None,
        _ => Some(format!("{at}: module {a} oracle {b}")),
    }
}

/// What a scene got to see, so a green run is known to have covered the game.
#[derive(Default)]
struct Seen {
    asks: usize,
    ticks: usize,
    bots: usize,
    sounds: BTreeSet<u64>,
    tallies: usize,
    again: usize,
    bell: usize,
    warden_down: usize,
    down: usize,
    locks: usize,
    lunges: usize,
    loose: usize,
    shields: usize,
    crates: usize,
    springs: usize,
    pads: usize,
    posts: u64,
    most_seats: usize,
    joined_late: usize,
    ring: usize,
    rails: usize,
}

impl Seen {
    fn see(&mut self, ask: &Value, answer: &Value) {
        self.asks += 1;
        match ask["op"].as_str() {
            Some("bot") => self.bots += 1,
            Some("tick") => {
                self.ticks += 1;
                let s = &answer["session"];
                answer["sounds"].as_array().into_iter().flatten().filter_map(Value::as_u64).for_each(|c| {
                    self.sounds.insert(c);
                });
                self.tallies += usize::from(s["tally"].is_object());
                self.again += usize::from(s["again"] == json!(true));
                self.bell += usize::from(s["bell_t"].is_number());
                self.warden_down += usize::from(answer["sounds"].as_array().is_some_and(|c| c.contains(&json!(16))));
                self.posts |= s["posts"].as_u64().unwrap_or(0);
                self.springs += usize::from(s["spring_t"].as_array().is_some_and(|t| t.iter().any(|v| v.as_u64().unwrap_or(0) > 0)));
                self.pads += usize::from(s["pad_t"].as_array().is_some_and(|t| t.iter().any(|v| v.as_u64().unwrap_or(0) > 0)));
                let seats = s["seats"].as_object().map_or(0, |m| m.len());
                self.most_seats = self.most_seats.max(seats);
                for seat in s["seats"].as_object().into_iter().flat_map(|m| m.values()) {
                    let r = &seat["rider"];
                    self.down += usize::from(r["down"].as_u64().unwrap_or(0) > 0);
                    self.locks += usize::from(r["lock"].is_object());
                    self.lunges += usize::from(r["lunge"].as_u64().unwrap_or(0) > 0);
                    self.loose += usize::from(r["loose"].as_array().is_some_and(|l| !l.is_empty()));
                    self.shields += usize::from(r["shield"] == json!(true));
                    self.crates += usize::from(r["crates"].as_u64().unwrap_or(0) > 0);
                    self.joined_late += usize::from(seat["joined"].as_u64().unwrap_or(0) > 0);
                    self.ring += usize::from(r["looping"] == json!(true));
                    self.rails += usize::from(r["ground"]["path"].as_u64().is_some_and(|p| (4..8).contains(&p)));
                }
            }
            _ => {}
        }
    }
}

/// Run a scene with both the module and the oracle answering every ask; the
/// two must agree to the byte, and the scene goes on from the oracle's answer.
fn side_by_side(name: &str, scene: &dyn Fn(Step)) -> Seen {
    install_scripted_rules();
    let mut seen = Seen::default();
    scene(&mut |ask: Value| {
        let want = old(&ask);
        let got = bend(&ask);
        let (a, b) = if ask["op"] == "bot" { (pressed(&got), pressed(&want)) } else { (got.clone(), want.clone()) };
        if a != b {
            let at = first_difference(&a, &b, &ask["op"].to_string()).unwrap_or_default();
            panic!("{name}: ask {} (tick {}) parts at {at}", seen.asks, ask["session"]["elapsed"]);
        }
        seen.see(&ask, &want);
        want
    });
    seen
}

#[test]
fn the_road_is_the_road_the_oracle_lays_out() {
    let ask = json!({"op": "level"});
    if let Some(at) = first_difference(&bend(&ask), &old(&ask), "level") {
        panic!("the road parts at {at}");
    }
}

#[test]
fn a_fresh_ride_is_the_oracles_for_every_table() {
    for present in [json!([1]), json!([1, 2]), json!([2, 3]), json!([1, 2, 3, 4]), json!([6]), json!([1, 2, 3, 4, 5, 6]), json!([])] {
        let ask = json!({"op": "start", "present": present});
        if let Some(at) = first_difference(&bend(&ask), &old(&ask), &present.to_string()) {
            panic!("a fresh ride parts at {at}");
        }
    }
}

#[test]
fn bots_ride_a_table_through_the_road_the_tally_and_the_ride_again_the_same() {
    let seen = side_by_side("bots", &|step| scenes::bots_all(step));
    eprintln!("bots: {} asks, {} ticks, {} bot asks", seen.asks, seen.ticks, seen.bots);
    assert!(seen.ticks > 14_000 && seen.bots > 14_000, "a long ride: {} ticks, {} bot asks", seen.ticks, seen.bots);
    assert!(seen.tallies > 0 && seen.bell > 0, "a bot reached the bell and the tally");
    assert!(seen.most_seats >= 4 && seen.springs > 0 && seen.pads > 0 && seen.posts > 0, "the road was ridden");
    assert!(seen.locks > 0 && seen.ring > 0, "a lock and a loop");
}

#[test]
fn random_keys_teleports_and_knights_coming_and_going_tick_the_same() {
    let seen = side_by_side("fuzz", &|step| scenes::fuzz_all(step));
    eprintln!("fuzz: {} ticks, sounds {:?}", seen.ticks, seen.sounds);
    assert!(seen.ticks >= 10_000, "{} ticks", seen.ticks);
    assert!(seen.down > 0 && seen.loose > 0 && seen.lunges > 0 && seen.joined_late > 0, "falls, scattered sparks, lunges and a late seat");
    assert!(seen.most_seats == 6, "six seats sat down");
    assert!(seen.crates > 0 && seen.rails > 0, "a crate and a rail");
}

#[test]
fn knights_set_down_before_every_thing_on_the_road_tick_the_same() {
    let seen = side_by_side("features", &|step| scenes::features(step));
    eprintln!("features: {} ticks, sounds {:?}", seen.ticks, seen.sounds);
    assert!(seen.ticks >= 6_000, "{} ticks", seen.ticks);
    assert!(seen.warden_down > 0 || seen.bell > 0, "the warden's gate was reached");
}

#[test]
fn the_wardens_gate_the_bell_the_tally_and_the_ride_again_tick_the_same() {
    let seen = side_by_side("finale", &|step| scenes::finale_all(step));
    eprintln!("finale: {} ticks, sounds {:?}", seen.ticks, seen.sounds);
    assert!(seen.warden_down > 0, "the warden fell");
    assert!(seen.bell > 0 && seen.tallies > 0 && seen.again > 0, "the bell, the tally and the ride again");
}

/// The scenes between them cover the game: every sound it makes is heard, and
/// every kind of thing on the road is met. Only the oracle ticks here.
#[test]
fn the_scenes_between_them_make_every_sound() {
    let mut heard = BTreeSet::new();
    for scene in [scenes::fuzz_all as fn(Step), scenes::features, scenes::finale_all, scenes::bots_all] {
        let mut seen = Seen::default();
        scene(&mut |ask: Value| {
            let answer = old(&ask);
            seen.see(&ask, &answer);
            answer
        });
        heard.extend(seen.sounds);
    }
    let missing: Vec<_> = (0..host::SOUNDS.len() as u64).filter(|c| !heard.contains(c)).map(|c| host::SOUNDS[c as usize]).collect();
    assert!(missing.is_empty(), "never heard: {missing:?}");
}

/// The adapter the game takes: the session's own types in, the next session
/// out, checked, against the oracle on the same keys.
#[test]
fn the_games_adapter_ticks_a_table_as_the_oracle_does() {
    use angel_delve::rules::lightgallop::Session;
    use angel_delve::rules::{Hero, Input};
    use std::collections::BTreeMap;
    install_scripted_rules();
    let heroes: BTreeMap<u32, Hero> = (1..=3).map(|id| (id, host::hero())).collect();
    let numbers = angel_delve::rules::lightgallop::Numbers::default();
    let mut mine = Session::new(&heroes).expect("a fresh ride");
    let present = json!([1, 2, 3]);
    let mut theirs = old(&json!({"op": "start", "present": present}))["session"].clone();
    for tick in 0..900u32 {
        let mut inputs: BTreeMap<u32, Input> = BTreeMap::new();
        for id in 1..=3u32 {
            let keys = mine.bot_input(id, &heroes[&id]);
            let said = old(&json!({"op": "bot", "session": theirs, "id": id}));
            assert_eq!(serde_json::to_value(keys).unwrap(), pressed(&said), "knight {id} tick {tick}");
            inputs.insert(id, keys);
        }
        let sounds = mine.step(&inputs, &heroes, &numbers);
        let answer = old(&json!({"op": "tick", "session": theirs, "inputs": inputs, "present": present}));
        theirs = answer["session"].clone();
        let mut mine_json = serde_json::to_value(&mine).unwrap();
        host::split_taken(&mut mine_json);
        if let Some(at) = first_difference(&mine_json, &theirs, "session") {
            panic!("tick {tick}: the adapter's session parts at {at}");
        }
        let names: Vec<&str> = answer["sounds"].as_array().unwrap().iter().map(|c| host::SOUNDS[c.as_u64().unwrap() as usize]).collect();
        assert_eq!(sounds, names, "tick {tick}");
    }
}

// ---- measurements (cargo test ... -- --ignored --nocapture) ----------------------------------

/// Per-tick cost at four riders: the oracle's native tick, and the module's
/// call as the host makes it (the session laid out, run in the warm isolate,
/// read back), over a ride of bots.
#[test]
#[ignore]
fn what_a_tick_costs_at_four_riders() {
    install_scripted_rules();
    let present = json!([1, 2, 3, 4]);
    let mut session = old(&json!({"op": "start", "present": present}))["session"].clone();
    let (mut native, mut bent) = (Vec::new(), Vec::new());
    let (mut in_bytes, mut out_bytes) = (0usize, 0usize);
    for tick in 0..1500u32 {
        let mut inputs = serde_json::Map::new();
        for id in 1..=4 {
            inputs.insert(id.to_string(), old(&json!({"op": "bot", "session": session, "id": id}))["input"].clone());
        }
        let ask = json!({"op": "tick", "session": session, "inputs": inputs, "present": present});
        let t = Instant::now();
        let want = old(&ask);
        native.push(t.elapsed().as_nanos() as u64);
        let t = Instant::now();
        let got = bend(&ask);
        bent.push(t.elapsed().as_nanos() as u64);
        if tick > 0 {
            in_bytes += ask.to_string().len();
            out_bytes += got.to_string().len();
        }
        assert_eq!(got, want);
        session = want["session"].clone();
    }
    let stats = |mut v: Vec<u64>| {
        v.sort_unstable();
        (v[v.len() / 2] / 1000, v[v.len() * 95 / 100] / 1000, v[v.len() - 1] / 1000)
    };
    let (n, b) = (stats(native), stats(bent));
    eprintln!("native oracle  p50 {} us  p95 {} us  max {} us", n.0, n.1, n.2);
    eprintln!("bend module    p50 {} us  p95 {} us  max {} us", b.0, b.1, b.2);
    eprintln!("a tick asks {} bytes and is answered {} bytes", in_bytes / 1499, out_bytes / 1499);
    eprintln!("the module's p50 is {:.2}% of the {} ms a frame has", b.0 as f64 / 33_333.0 * 100.0, 1000 / host::HZ);
}
