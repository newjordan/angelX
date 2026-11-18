//! The Round Table adventures and Knight Racer on one shared entry shell.
//!
//! Lanternward, Moonfall, Bannerpeak, Griffinwing, Lightgallop and Knight Racer have a
//! door of their own on the kingdom map (`ADVENTURES` plus `knight_racers::ADVENTURE`).
//! A knight stands at the tile and presses E;
//! the party goes in as it does for the Forest Road (the Dark Forest's room
//! hosts the run, the kingdom door is the way back), every seat plays at once,
//! and holding Esc leaves (`exits`).
//!
//! This file is the common entry shell; the five scored adventures keep their fixed tune slots:
//! the scored list and its knob rows remain unchanged; `games()` extends
//! entry, validity and clearing with the unscored race. Each game owns its state.
use super::tune::Tune;
use super::*;
use angel_net::knob::{Knob, Side};

#[derive(Clone, Copy)]
pub struct Adventure {
    pub slug: &'static str,
    pub name: &'static str,
    pub entry: (i32, i32),
    pub slot: usize,
    pub marks: &'static [&'static str],
    pub numbers: &'static [Number; 5],
    pub showing: fn(&Run) -> bool,
    pub start: fn(&mut Run),
    pub valid: fn(&Run) -> bool,
    pub clear: fn(&mut Run),
    pub seated: fn(&Run) -> Vec<u32>,
    pub bot: fn(&Run, u32) -> Input,
}

#[derive(Clone, Copy, Debug)]
pub struct Number {
    pub name: &'static str,
    pub label: &'static str,
    pub constant: &'static str,
    pub min: u32,
    pub max: u32,
    pub start: u32,
}

impl Number {
    pub const fn stub(name: &'static str, label: &'static str, constant: &'static str) -> Number {
        Number {
            name,
            label,
            constant,
            min: 0,
            max: 1000,
            start: 100,
        }
    }
}

pub static ADVENTURES: [Adventure; 5] = [
    lanternward::ADVENTURE,
    moonfall::ADVENTURE,
    bannerpeak::ADVENTURE,
    griffinwing::ADVENTURE,
    lightgallop::ADVENTURE,
];

pub fn games() -> impl Iterator<Item = &'static Adventure> {
    ADVENTURES.iter().chain([&knight_racers::ADVENTURE, &deathmatch::ADVENTURE])
}

pub const ENTRY_REACH: f32 = 2.1;

pub const INTENT_MARKS: usize = lanternward::MARKS.len()
    + moonfall::MARKS.len()
    + bannerpeak::MARKS.len()
    + griffinwing::MARKS.len()
    + lightgallop::MARKS.len();

pub fn intent_key(key: &str) -> bool {
    games().any(|a| a.marks.contains(&key))
}

pub fn adventure(slug: &str) -> Option<&'static Adventure> {
    games().find(|a| a.slug == slug)
}

pub fn valid(run: &Run) -> bool {
    games().all(|a| (a.valid)(run))
}

pub fn clear(run: &mut Run) {
    for a in games() {
        (a.clear)(run);
    }
}

#[allow(clippy::type_complexity)]
pub fn sessions(
    run: &Run,
) -> (
    &Option<lanternward::Session>,
    &Option<moonfall::Session>,
    &Option<bannerpeak::Session>,
    &Option<griffinwing::Session>,
    &Option<lightgallop::Session>,
) {
    (
        &run.lanternward,
        &run.moonfall,
        &run.bannerpeak,
        &run.griffinwing,
        &run.lightgallop,
    )
}

pub fn unsaved(flags: &[u32; 5]) -> bool {
    flags.iter().all(|&word| word == 0)
}

pub fn unscored(scores: &[pastimes::Score; 5]) -> bool {
    scores.iter().all(|s| *s == pastimes::Score::default())
}

pub fn starts() -> [[u32; 5]; 5] {
    let mut out = [[0; 5]; 5];
    for a in &ADVENTURES {
        for (i, n) in a.numbers.iter().enumerate() {
            out[a.slot][i] = n.start;
        }
    }
    out
}

pub fn knobs(tune: &Tune) -> Vec<Knob> {
    let d = Tune::default();
    let mut out = Vec::new();
    for a in &ADVENTURES {
        for (i, n) in a.numbers.iter().enumerate() {
            out.push(
                Knob::new(n.name, a.name, n.label, n.constant, Side::Server)
                    .range(f64::from(n.min), f64::from(n.max), 1.0)
                    .default(f64::from(d.roundtable[a.slot][i]))
                    .at(f64::from(tune.roundtable[a.slot][i])),
            );
        }
    }
    out
}

pub fn set(tune: &mut Tune, name: &str, value: u32) -> bool {
    for a in &ADVENTURES {
        for (i, n) in a.numbers.iter().enumerate() {
            if n.name == name {
                tune.roundtable[a.slot][i] = value.clamp(n.min, n.max);
                return true;
            }
        }
    }
    false
}

pub fn numbers_valid(tune: &Tune) -> bool {
    ADVENTURES.iter().all(|a| {
        a.numbers
            .iter()
            .enumerate()
            .all(|(i, n)| (n.min..=n.max).contains(&tune.roundtable[a.slot][i]))
    })
}

pub fn number(tune: &Tune, slot: usize, i: usize) -> u32 {
    tune.roundtable[slot][i]
}

pub fn enter(run: &mut Run, adventure: &Adventure) {
    let geo = map::geography(&run.kingdom.wishes);
    let Some(site) = geo.sites.iter().position(|s| s.room == RoomKind::Forest) else {
        return;
    };
    run.enter_kingdom_place(site);
    if let Some(back) = &mut run.kingdom.return_to {
        back.name = adventure.name.into();
        back.x = (adventure.entry.0 as f32 + 0.5) * TILE_UNITS;
        back.y = (adventure.entry.1 as f32 + 0.5) * TILE_UNITS;
    }
    (adventure.start)(run);
}

impl Run {
    pub fn roundtable_showing(&self) -> bool {
        games().any(|a| (a.showing)(self))
    }

    pub fn roundtable_under_way(&self) -> Option<&'static Adventure> {
        games().find(|a| (a.showing)(self))
    }

    pub fn roundtable_seated(&self) -> Option<Vec<u32>> {
        self.roundtable_under_way().map(|a| (a.seated)(self))
    }

    pub fn roundtable_inputs(&self) -> BTreeMap<u32, Input> {
        let Some(a) = self.roundtable_under_way() else {
            return BTreeMap::new();
        };
        self.players
            .keys()
            .map(|&id| (id, (a.bot)(self, id)))
            .collect()
    }

    pub(super) fn tick_roundtable_entry(&mut self, inputs: &BTreeMap<u32, Input>) -> bool {
        if self.room().kind != RoomKind::Kingdom || self.kingdom.door_held {
            return false;
        }
        if !crate::world::plan::full() {
            return false;
        }
        for a in games() {
            let (x, y) = (
                (a.entry.0 as f32 + 0.5) * TILE_UNITS,
                (a.entry.1 as f32 + 0.5) * TILE_UNITS,
            );
            let pressed = self.players.iter().any(|(id, h)| {
                h.hp > 0
                    && !h.stone
                    && inputs.get(id).is_some_and(|i| i.valid() && i.interact)
                    && !self.kingdom.door_latched.contains(id)
                    && (h.x - x).hypot(h.y - y) < ENTRY_REACH
            });
            if pressed {
                enter(self, a);
                return true;
            }
        }
        false
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn roundtable_for_test(slug: &str) -> Option<Run> {
        Some(match slug {
            "lanternward" => Run::lanternward_for_test(),
            "moonfall" => Run::moonfall_for_test(),
            "bannerpeak" => Run::bannerpeak_for_test(),
            "griffinwing" => Run::griffinwing_for_test(),
            "lightgallop" => Run::lightgallop_for_test(),
            "knight_racers" => { let mut run = fixture(&knight_racers::ADVENTURE); if let Some(s) = run.knight_racers.as_mut() { s.skip_menus(); } run }
            "deathmatch" => fixture(&deathmatch::ADVENTURE),
            _ => return None,
        })
    }
}

#[cfg(any(test, feature = "testing"))]
pub fn fixture(adventure: &Adventure) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(adventure.entry));
    enter(&mut run, adventure);
    run
}

#[cfg(test)]
#[path = "../../tests/rules/together_roundtable__tests.rs"]
mod tests;
