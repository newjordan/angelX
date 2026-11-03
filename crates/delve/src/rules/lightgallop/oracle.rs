//! The Lightgallop rules as they ran in Rust before they became Bend
//! (`cockpit/live/lightgallop-rules`): the road, the rider's body, the
//! session's tick, the foes, the Dimming, the tally and the bot, moved here
//! unchanged but for their imports.
//!
//! It runs only in tests. This crate's own tests tick against it through
//! `answer`, which speaks the module's wire (the same asks and answers), and
//! the cockpit's parity test includes this very file and ticks the shipped
//! module and `answer` side by side, comparing the whole session each tick.
//!
//! It is included by `#[path]` from both trees, so it may only name what a
//! `super::host` can supply: the game's clock, its seat limit, its keys and
//! knights, a knight to seat, the sound names and the spark bits' halves.
use super::host::{self, Hero, Input};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "oracle/bot.rs"]
pub mod bot;
#[path = "oracle/level.rs"]
pub mod level;
#[path = "oracle/math.rs"]
pub mod math;
#[path = "oracle/rider.rs"]
pub mod rider;
#[path = "oracle/scenes.rs"]
pub mod scenes;
#[path = "oracle/session.rs"]
pub mod session;
#[path = "oracle/terrain.rs"]
pub mod terrain;
#[path = "oracle/world.rs"]
pub mod world;

pub use bot::{Route, Skill};
pub use rider::Numbers;
pub use session::Session;

/// Knights who can ride, one plain hero for each id asked for.
pub fn heroes(ids: &Value) -> BTreeMap<u32, Hero> {
    ids.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|id| (id as u32, host::hero()))
        .collect()
}

/// The five numbers, as the ask carries them.
pub fn numbers(ask: &Value) -> Numbers {
    let n = ask.get("numbers");
    let get = |name: &str, base: i32| n.and_then(|n| n.get(name)).and_then(Value::as_i64).map_or(base, |v| v as i32);
    let d = Numbers::default();
    Numbers {
        slope: get("slope", d.slope),
        loop_min: get("loop_min", d.loop_min),
        spur_cap: get("spur_cap", d.spur_cap),
        lock_reach: get("lock_reach", d.lock_reach),
        dimming: get("dimming", d.dimming),
    }
}

fn skill(ask: &Value) -> Skill {
    let Some(s) = ask.get("skill") else { return Skill::ACE };
    Skill {
        spur: s["spur"].as_bool().unwrap_or(false),
        roll: s["roll"].as_bool().unwrap_or(false),
        lag: s["lag"].as_i64().unwrap_or(0) as i32,
        hesitate: s["hesitate"].as_u64().unwrap_or(0) as u32,
    }
}

fn sounds(names: &[&'static str]) -> Vec<usize> {
    names
        .iter()
        .map(|name| host::SOUNDS.iter().position(|s| s == name).expect("a sound the table knows"))
        .collect()
}

fn take<T: serde::de::DeserializeOwned>(ask: &Value, name: &str) -> Result<T, String> {
    serde_json::from_value(ask.get(name).cloned().unwrap_or(Value::Null)).map_err(|e| format!("{name}: {e}"))
}

/// The module's asks and answers, served by the rules as they were. The
/// session crosses with its spark bits as halves, as it does the shipped one.
pub fn answer(_id: &str, ask: &Value) -> Result<Value, String> {
    let mut ask = ask.clone();
    if let Some(session) = ask.get_mut("session") {
        host::join_taken(session);
    }
    let mut said = match ask["op"].as_str().unwrap_or("") {
        "level" => serde_json::to_value(level::level()).map_err(|e| e.to_string())?,
        "start" => json!({ "session": Session::new(&heroes(&ask["present"])) }),
        "tick" => {
            let mut session: Session = take(&ask, "session")?;
            let inputs: BTreeMap<u32, Input> = take(&ask, "inputs")?;
            let sound = session.step(&inputs, &heroes(&ask["present"]), &numbers(&ask));
            json!({ "session": session, "sounds": sounds(&sound) })
        }
        "bot" => {
            let session: Session = take(&ask, "session")?;
            let id = ask["id"].as_u64().ok_or("no knight")? as u32;
            let route = match ask["route"].as_str() {
                Some("Upper") => Route::Upper,
                Some("Lower") => Route::Lower,
                _ => Route::of_seat(id),
            };
            json!({ "input": session.drive_as(id, route, &numbers(&ask), skill(&ask)) })
        }
        other => return Err(format!("no op {other}")),
    };
    if let Some(session) = said.get_mut("session") {
        host::split_taken(session);
    }
    Ok(said)
}
