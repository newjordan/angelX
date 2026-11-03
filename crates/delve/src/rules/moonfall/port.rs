//! Moonfall's rules are the Bend module `moonfall-rules` (cockpit/live/moonfall-rules).
//! The session goes in as it saves, the answer comes back as a session: this is the
//! whole Rust side, the sounds' names and the words stay here.
use super::Session;
use crate::rules::{Hero, Input};
use crate::scripted;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The sounds the module asks for, by index.
pub const CUES: [&str; 12] = [
    "circle_toll", "boss_roar", "wall_up", "play_card", "door_open", "wish_grant",
    "rock_land", "card_pickup", "mana_empty", "spoil_pickup", "knight_fall", "hero_hurt",
];

#[derive(Deserialize)]
struct Stepped {
    session: Session,
    cues: Vec<usize>,
    leaving: Vec<u32>,
    won: Option<(u32, u16)>,
}

/// What one tick did beside the session: sounds, the knights who walk out of the
/// town, and the first deliverance (seats at the table, best score).
#[derive(Default)]
pub struct Ticked {
    pub cues: Vec<&'static str>,
    pub leaving: Vec<u32>,
    pub won: Option<(u32, u16)>,
}

impl Session {
    /// What one tick asks of the module: the session as it saves, the key rows, who is awake,
    /// the sounds still owed, and how many pages each open talk has (the words stay here).
    pub fn frame(&self, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>) -> Value {
        let owed: Vec<_> = self.pending.iter().filter_map(|c| CUES.iter().position(|n| n == c)).collect();
        let keys: Vec<_> = inputs.iter().map(|(id, i)| (id, i.valid(), i.interact, i.take, i.swing, i.ult, i.play > 0, i.bomb, i.dash, i.move_x, i.move_y, i.aim_x, i.aim_y)).collect();
        let awake: Vec<_> = heroes.iter().map(|(id, h)| (id, !h.away && !h.stone)).collect();
        let pages: Vec<_> = self.seats.iter().filter_map(|(id, s)| s.knight.say.map(|say| (id, self.say_pages(&say)))).collect();
        json!({"op": "step", "session": self, "pending": owed, "inputs": keys, "heroes": awake, "pages": pages})
    }

    /// One tick through the module; a refused answer leaves the session as it was.
    pub fn advance(&mut self, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>) -> Result<Ticked, String> {
        let out: Stepped = scripted::step("moonfall-rules", &self.frame(inputs, heroes))?;
        let cues: Option<Vec<_>> = out.cues.iter().map(|&i| CUES.get(i).copied()).collect();
        let Some(cues) = cues else {
            return Err("moonfall-rules named a sound that does not exist".into());
        };
        *self = out.session;
        Ok(Ticked { cues, leaving: out.leaving, won: out.won })
    }

    /// One tick, and the sounds it made (a refused answer is a quiet tick).
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>) -> Vec<&'static str> {
        self.advance(inputs, heroes).map(|t| t.cues).unwrap_or_default()
    }

    /// What a bot presses for knight `id`.
    pub fn bot_input(&self, id: u32, _hero: &Hero) -> Input {
        let ask = json!({"op": "bot", "session": self, "id": id});
        scripted::step::<_, Value>("moonfall-rules", &ask).map_or_else(|_| Input::default(), |p| Input {
            move_x: p["move_x"].as_i64().unwrap_or(0) as i8,
            move_y: p["move_y"].as_i64().unwrap_or(0) as i8,
            interact: p["interact"] == true,
            ult: p["ult"] == true,
            swing: p["swing"] == true,
            play: p["play"].as_u64().unwrap_or(0) as u8,
            bomb: p["bomb"] == true,
            dash: p["dash"] == true,
            ..Input::default()
        })
    }
}
