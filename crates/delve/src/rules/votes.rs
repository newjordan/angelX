//! The 0.2.2 ballot: one catalogue, realm-owned ratings, seat-local menus.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub struct Item {
    pub id: &'static str,
    pub group: &'static str,
    pub label: &'static str,
}

// Stable IDs are saved and exported. Labels may change without losing votes.
macro_rules! items {
    ($(($id:literal, $group:literal, $label:literal)),* $(,)?) => {
        pub const ITEMS: &[Item] = &[$(Item { id: $id, group: $group, label: $label }),*];
    };
}
items![
    ("controls", "Controls and HUD", "New controls"),
    ("hud", "Controls and HUD", "Clear HUD"),
    ("couch", "Controls and HUD", "Couch knight"),
    ("gamepad", "Controls and HUD", "Gamepad controls"),
    ("bow", "Arms and hits", "Bow stream"),
    ("crossbow", "Arms and hits", "Crossbow rail"),
    ("handgonne", "Arms and hits", "Handgonne fan"),
    ("hits", "Arms and hits", "Hits and sparks"),
    ("valour", "Arms and hits", "Valour skills"),
    ("tempering", "Arms and hits", "Arm tempering"),
    ("synergies", "Arms and hits", "Named synergies"),
    ("coop-pressure", "Monsters and bosses", "Co-op pressure"),
    ("bullet-patterns", "Monsters and bosses", "Monster patterns"),
    ("boss-phases", "Monsters and bosses", "Boss phases"),
    ("big-ripper", "Monsters and bosses", "Big Ripper"),
    ("grinwick", "Monsters and bosses", "Grinwick"),
    ("wish-gear", "Wishes and looks", "Wish gear"),
    ("summons", "Wishes and looks", "New summons"),
    ("shot-looks", "Wishes and looks", "Shots and orbits"),
    ("learned-wishes", "Wishes and looks", "Learned wishes"),
    ("sprite-sheets", "Wishes and looks", "Sprite sheets"),
    ("privy-popup", "The privy", "Privy pop-up"),
    ("privy-race", "The privy", "Privy race"),
    ("broadsheet-boy", "Countryside games", "Broadsheet Boy"),
    (
        "barrel-board-derby",
        "Countryside games",
        "Barrel-Board Derby"
    ),
    (
        "turnip-keepy-uppy",
        "Countryside games",
        "Turnip Keepy-Uppy"
    ),
    ("washtub-regatta", "Countryside games", "Washtub Regatta"),
    ("cheese-chuck", "Countryside games", "Cheese Chuck"),
    (
        "hobbyhorse-hurdles",
        "Countryside games",
        "Hobbyhorse Hurdles"
    ),
    (
        "squire-sled-slalom",
        "Countryside games",
        "Squire Sled Slalom"
    ),
    ("chamber-pot-put", "Countryside games", "Chamber-Pot Put"),
    (
        "wheelbarrow-cross",
        "Countryside games",
        "Wheelbarrow Cross"
    ),
    ("shield-surf", "Countryside games", "Shield Surf"),
    (
        "belfry-bat-and-ball",
        "Countryside games",
        "Belfry Bat-and-Ball"
    ),
    (
        "jester-half-pipe",
        "Countryside games",
        "Jester's Half-Pipe"
    ),
    (
        "cabbage-roll-racing",
        "Countryside games",
        "Cabbage Roll Racing"
    ),
    ("trencher-toss", "Countryside games", "Trencher Toss"),
    (
        "carnival-duck-range",
        "Countryside games",
        "Carnival Duck Range"
    ),
    ("humphrey", "Countryside games", "Humphrey the Risen"),
    ("country-spells", "Countryside games", "Countryside spells"),
    ("college", "College and wizard", "Labyrinth College"),
    ("college-secrets", "College and wizard", "College secrets"),
    ("bartosz", "College and wizard", "Bartosz the Wizard"),
    ("wizard-quests", "College and wizard", "Wizard quests"),
    ("bestiary", "College and wizard", "Bestiary lectern"),
    ("horse-ranch", "Horses and racing", "Horse ranch"),
    ("flower-fields", "Horses and racing", "Flower fields"),
    ("helios", "Horses and racing", "Helios"),
    ("chariot-racing", "Horses and racing", "Chariot racing"),
    ("horse-market", "Horses and racing", "Horse market"),
    ("horse-breeding", "Horses and racing", "Horse breeding"),
    ("kingdom-map", "World and floors", "Kingdom map"),
    ("floor-plates", "World and floors", "Floor plates"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Rating {
    Stars(u8),
    Untried(Untried),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Untried {
    #[serde(rename = "untried")]
    Yes,
}
pub type Records = BTreeMap<String, BTreeMap<String, Rating>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Toggle,
    Close,
    Up,
    Down,
    Left,
    Right,
    Rate(u8),
    Clear,
    Untried,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub serial: u64,
    pub action: Action,
}
impl Command {
    pub fn valid(self) -> bool {
        !matches!(self.action, Action::Rate(n) if !(1..=5).contains(&n))
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Seat {
    pub open: bool,
    #[serde(default)]
    pub couch: bool,
    pub selected: usize,
    pub serial: u64,
    pub hint: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub player: String,
    pub item: String,
    pub rating: Option<Rating>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(skip)]
    pub local_focus: u32,
    pub records: Records,
    pub seats: BTreeMap<u32, Seat>,
    pub seen: BTreeSet<String>,
    #[serde(skip)]
    pub ready: bool,
    #[serde(skip)]
    pub dirty: bool,
    #[serde(skip)]
    pub changes: Vec<Change>,
}
impl State {
    pub fn local_seat(&self) -> u32 {
        if self.open(self.local_focus) {
            self.local_focus
        } else if self.open(2) && self.seats.get(&2).is_some_and(|s| s.couch) {
            2
        } else {
            1
        }
    }
    pub fn open(&self, id: u32) -> bool {
        self.seats.get(&id).is_some_and(|s| s.open)
    }
    pub fn arrive(&mut self, id: u32, name: &str) {
        if self.seats.contains_key(&id) {
            return;
        }
        let hint = if !self.records.get(name).is_some_and(|r| !r.is_empty())
            && self.seen.insert(name.into())
        {
            self.dirty = true;
            HZ * 7
        } else {
            0
        };
        self.seats.insert(
            id,
            Seat {
                hint,
                ..Default::default()
            },
        );
    }
    pub fn command(&mut self, id: u32, name: &str, command: Command) {
        if !command.valid() {
            return;
        }
        self.arrive(id, name);
        let seat = self.seats.get_mut(&id).expect("arrived");
        if command.serial <= seat.serial {
            return;
        }
        seat.serial = command.serial;
        if command.action == Action::Toggle {
            seat.open = !seat.open;
            return;
        }
        if command.action == Action::Close {
            seat.open = false;
            return;
        }
        if !seat.open {
            return;
        }
        seat.selected = seat.selected.min(ITEMS.len() - 1);
        match command.action {
            Action::Up => {
                seat.selected = seat.selected.saturating_sub(1);
                return;
            }
            Action::Down => {
                seat.selected = (seat.selected + 1).min(ITEMS.len() - 1);
                return;
            }
            _ => {}
        }
        let item = ITEMS[seat.selected].id;
        let ratings = self.records.entry(name.into()).or_default();
        let old = ratings.get(item).copied();
        let stars = match old {
            Some(Rating::Stars(n)) => n,
            _ => 0,
        };
        let next = match command.action {
            Action::Rate(n) => Some(Rating::Stars(n)),
            Action::Right => Some(Rating::Stars((stars + 1).min(5))),
            Action::Left => Some(Rating::Stars(stars.saturating_sub(1).max(1))),
            Action::Untried => Some(Rating::Untried(Untried::Yes)),
            Action::Clear => None,
            _ => return,
        };
        if old == next {
            return;
        }
        if let Some(rating) = next {
            ratings.insert(item.into(), rating);
        } else {
            ratings.remove(item);
        }
        self.dirty = true;
        self.changes.push(Change {
            player: name.into(),
            item: item.into(),
            rating: next,
        });
    }
    pub fn average(&self, item: &str) -> (f32, usize) {
        average(&self.records, item)
    }
}
pub fn average(records: &Records, item: &str) -> (f32, usize) {
    let (sum, count) = records
        .values()
        .filter_map(|r| match r.get(item) {
            Some(Rating::Stars(n)) if (1..=5).contains(n) => Some(usize::from(*n)),
            _ => None,
        })
        .fold((0, 0), |(sum, count), n| (sum + n, count + 1));
    (
        if count == 0 {
            0.0
        } else {
            sum as f32 / count as f32
        },
        count,
    )
}
pub fn summary(records: &Records) -> String {
    ITEMS
        .iter()
        .map(|item| {
            let (avg, count) = average(records, item.id);
            if count == 0 {
                format!("{}: no ratings (0)", item.label)
            } else {
                format!("{}: {avg:.1}/5 ({count})", item.label)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Run {
    pub fn vote_command(&mut self, id: u32, command: Command) {
        if let Some(hero) = self.players.get(&id) {
            self.playtest.command(id, &hero.name, command);
        }
    }
    pub(super) fn ballot_inputs(&mut self, inputs: &BTreeMap<u32, Input>) -> BTreeMap<u32, Input> {
        if self.playtest.ready {
            for (&id, hero) in &self.players {
                self.playtest.arrive(id, &hero.name);
            }
        }
        for seat in self.playtest.seats.values_mut() {
            seat.hint = seat.hint.saturating_sub(1);
        }
        inputs
            .iter()
            .map(|(&id, &input)| {
                if input.skip_plate {
                    return (id, Input::default());
                }
                let was_open = self.playtest.open(id);
                if let Some(command) = input.vote {
                    self.vote_command(id, command);
                }
                (
                    id,
                    if was_open || self.playtest.open(id) || input.vote.is_some() {
                        Input::default()
                    } else {
                        input
                    },
                )
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_votes__tests.rs"]
mod tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../../tests/rules/together_votes__fixtures.rs"]
pub mod testing;
