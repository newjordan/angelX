//! The Round Table adventures: five co-op stubs on one shared shell.
//!
//! Lanternward, Moonfall, Bannerpeak, Griffinwing and Lightgallop each have a
//! door of their own on the kingdom map (`ADVENTURES`, and the tile table in
//! docs/ROUNDTABLE-ADVENTURES.md). A knight stands at the tile and presses E;
//! the party goes in as it does for the Forest Road (the Dark Forest's room
//! hosts the run, the kingdom door is the way back), every seat plays at once,
//! and holding Esc leaves (`exits`).
//!
//! This file is the whole of what the five share, and it is not a builder's:
//! the one list of them, the entry, the sums over their state (valid, clear,
//! showing), the knob rows, the crash-safe marks. Everything an adventure
//! varies lives in its own module (`rules/<slug>.rs`), which hands this file
//! one `Adventure` and never needs a line added here. That is how five
//! branches edit five files and merge without a conflict.
use super::tune::Tune;
use super::*;
use angel_net::knob::{Knob, Side};

/// One adventure's registration: what the shared shell needs of it.
#[derive(Clone, Copy)]
pub struct Adventure {
    /// The module and scene name: `lanternward`.
    pub slug: &'static str,
    /// What it is called on the kingdom map and in its own title.
    pub name: &'static str,
    /// The kingdom tile its door stands on (tiles, not units).
    pub entry: (i32, i32),
    /// Its place in `ADVENTURES` and in `Tune::roundtable`.
    pub slot: usize,
    /// Every mark it may write: the crash-safe intent journal keeps these.
    pub marks: &'static [&'static str],
    /// Its five numbers, as the knob panel shows them.
    pub numbers: &'static [Number; 5],
    /// Whether its run is under way in the party's room.
    pub showing: fn(&Run) -> bool,
    /// Open its run in the party's room (the Dark Forest's).
    pub start: fn(&mut Run),
    /// Whether its state, if any, is one a run may hold.
    pub valid: fn(&Run) -> bool,
    /// End its run: the state cleared.
    pub clear: fn(&mut Run),
    /// The knights its run seats.
    pub seated: fn(&Run) -> Vec<u32>,
    /// What a bot presses for knight `id`.
    pub bot: fn(&Run, u32) -> Input,
}

/// One of an adventure's five numbers: a fixed integer picked from play,
/// held in `Tune::roundtable` and moved from the knob panel while playing.
#[derive(Clone, Copy, Debug)]
pub struct Number {
    /// The knob's name, set by `Tune::set`.
    pub name: &'static str,
    /// What the panel calls it.
    pub label: &'static str,
    /// The fixed number in the code it stands for, and where.
    pub constant: &'static str,
    pub min: u32,
    pub max: u32,
    /// The code's number: what the knob stands at until it is moved.
    pub start: u32,
}

impl Number {
    /// A neutral placeholder, 0 to 1000 and standing at 100: the builder
    /// names it, bounds it and picks its number from play.
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

/// The five, in `Tune::roundtable` order. Each entry is the adventure's own
/// (`ADVENTURE` in its module); nothing is edited here after the prep commit.
pub static ADVENTURES: [Adventure; 5] = [
    // Lanternward
    lanternward::ADVENTURE,
    // Moonfall
    moonfall::ADVENTURE,
    // Bannerpeak
    bannerpeak::ADVENTURE,
    // Griffinwing
    griffinwing::ADVENTURE,
    // Lightgallop
    lightgallop::ADVENTURE,
];

/// How close to its tile's centre a knight stands to press E at the door
/// (units, as the kingdom's other doors).
pub const ENTRY_REACH: f32 = 2.1;

/// Every adventure's marks counted, for the intent journal's bound
/// (`countryside::INTENT_LIMIT`).
pub const INTENT_MARKS: usize = lanternward::MARKS.len()
    + moonfall::MARKS.len()
    + bannerpeak::MARKS.len()
    + griffinwing::MARKS.len()
    + lightgallop::MARKS.len();

/// Whether `key` is a mark an adventure writes: what the private checkpoint
/// keeps across a crash (`countryside::intent_key`).
pub fn intent_key(key: &str) -> bool {
    ADVENTURES.iter().any(|a| a.marks.contains(&key))
}

/// The adventure with this slug.
pub fn adventure(slug: &str) -> Option<&'static Adventure> {
    ADVENTURES.iter().find(|a| a.slug == slug)
}

/// Whether every adventure's state is one a run may hold.
pub fn valid(run: &Run) -> bool {
    ADVENTURES.iter().all(|a| (a.valid)(run))
}

/// End every adventure: a room entered or left closes them all.
pub fn clear(run: &mut Run) {
    for a in &ADVENTURES {
        (a.clear)(run);
    }
}

/// Every session, for the traces that compare two runs.
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

/// Whether no adventure has settled anything into the Home (`Home::roundtable`).
pub fn unsaved(flags: &[u32; 5]) -> bool {
    flags.iter().all(|&word| word == 0)
}

/// Whether no adventure has a record yet (`Records::roundtable`).
pub fn unscored(scores: &[pastimes::Score; 5]) -> bool {
    scores.iter().all(|s| *s == pastimes::Score::default())
}

/// The numbers as the code has them, for `Tune::default`.
pub fn starts() -> [[u32; 5]; 5] {
    let mut out = [[0; 5]; 5];
    for a in &ADVENTURES {
        for (i, n) in a.numbers.iter().enumerate() {
            out[a.slot][i] = n.start;
        }
    }
    out
}

/// Every adventure's knobs, where `tune` stands (`Tune::knobs`).
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

/// Set a knob by name, held to its bounds (`Tune::set`): whether it is one.
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

/// Whether every number stands inside its bounds (`Tune::valid`).
pub fn numbers_valid(tune: &Tune) -> bool {
    ADVENTURES.iter().all(|a| {
        a.numbers
            .iter()
            .enumerate()
            .all(|(i, n)| (n.min..=n.max).contains(&tune.roundtable[a.slot][i]))
    })
}

/// An adventure's number `i`, as `tune` holds it: what its rules read.
pub fn number(tune: &Tune, slot: usize, i: usize) -> u32 {
    tune.roundtable[slot][i]
}

/// Open `adventure` for the party: into the Dark Forest's room by the kingdom
/// door (so the way back is the real surface return), then its own start.
/// The way back is the adventure's tile, whoever joins late.
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
    /// Whether any of the five is under way in the party's room.
    pub fn roundtable_showing(&self) -> bool {
        ADVENTURES.iter().any(|a| (a.showing)(self))
    }

    /// The adventure under way, if any.
    pub fn roundtable_under_way(&self) -> Option<&'static Adventure> {
        ADVENTURES.iter().find(|a| (a.showing)(self))
    }

    /// The knights the adventure under way seats.
    pub fn roundtable_seated(&self) -> Option<Vec<u32>> {
        self.roundtable_under_way().map(|a| (a.seated)(self))
    }

    /// What the bots press this tick, every knight at the table.
    pub fn roundtable_inputs(&self) -> BTreeMap<u32, Input> {
        let Some(a) = self.roundtable_under_way() else {
            return BTreeMap::new();
        };
        self.players
            .keys()
            .map(|&id| (id, (a.bot)(self, id)))
            .collect()
    }

    /// E at an adventure's tile on the kingdom map opens it for the party.
    /// True when one opened (the rest of the kingdom's tick stands aside).
    pub(super) fn tick_roundtable_entry(&mut self, inputs: &BTreeMap<u32, Input>) -> bool {
        // A press still down from the last door is let go first
        // (`kingdom::State::door_latched`).
        if self.room().kind != RoomKind::Kingdom || self.kingdom.door_held {
            return false;
        }
        for a in &ADVENTURES {
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

    /// The adventure `slug` with the party inside it: the fixture the tests,
    /// the dev host and the smoke scenes share.
    #[cfg(any(test, feature = "testing"))]
    pub fn roundtable_for_test(slug: &str) -> Option<Run> {
        Some(match slug {
            "lanternward" => Run::lanternward_for_test(),
            "moonfall" => Run::moonfall_for_test(),
            "bannerpeak" => Run::bannerpeak_for_test(),
            "griffinwing" => Run::griffinwing_for_test(),
            "lightgallop" => Run::lightgallop_for_test(),
            _ => return None,
        })
    }
}

/// A lone knight at home, walked to `adventure`'s tile and let in: each
/// adventure's own `*_for_test`.
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
