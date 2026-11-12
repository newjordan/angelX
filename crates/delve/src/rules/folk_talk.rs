//! Speaking with the village: in a steward's hall, E beside one of the
//! named folk gives their word on how they fare, said from the same state
//! the rent days keep (their belly, their mood, the bed they have, what
//! they last broke) and from nowhere else. Beside the maker of a finished
//! masterwork it takes the artifact (`fey`) into the knight's pockets.
//!
//! And the party coming into a hall where the village is mourning, feasting,
//! raging or at the forge hears of it at the door: a line on the notice
//! line and a cue the knights' chatter answers (`chatter`).

use super::curios::POCKETS;
use super::fey;
use super::holdings::{self, held_floor, Holding, Holdings, RESTLESS, SLOTS};
use super::occasions::{Broke, Occasion};
use super::strolls;
use super::villagers::{Trade, COMING_OF_AGE, ELDER};
use super::*;

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (tx, ty) = ((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32);
    tx >= col && tx < col + w && ty >= row && ty < row + h
}

/// What soul `i` of `held` says of themselves, in capitals: who they are,
/// then the worst of how they fare — a plain count of their own state.
pub fn words(h: &Holdings, held: &Holding, i: usize) -> String {
    let Some(f) = held.folk.get(i) else {
        return String::new();
    };
    let lead = format!("{} THE {}", f.name.to_uppercase(), f.trade.word().to_uppercase());
    let beds = held.standing();
    let says = if held.seized.as_ref().is_some_and(|s| s.who == f.name && s.trade == f.trade) {
        let room = fey::workshop(held, f.trade).and_then(|b| holdings::room_def(&held.rooms[b].id)).map_or("WORKSHOP", |d| d.name);
        format!("THE FEY IS ON ME. THE {} IS MINE UNTIL THE WORK IS DONE.", room.to_uppercase())
    } else if let Some(m) = held.works.iter().find(|m| m.maker == f.name && m.trade == f.trade) {
        let name = m.curio().map_or("IT", |c| c.name).to_uppercase();
        format!("{name} IS MINE. E TO TAKE IT, ONE KNIGHT AT A TIME.")
    } else if let Some(Occasion::Tantrum { cause, broke, .. }) =
        held.occasion.as_ref().filter(|o| matches!(o, Occasion::Tantrum { who, .. } if *who == f.name))
    {
        let broke = match broke {
            Broke::Course(b) => {
                let room = held.rooms.get(usize::from(*b)).and_then(|r| holdings::room_def(&r.id)).map_or("ROOM", |d| d.name);
                format!("I BROKE A COURSE OF THE {}.", room.to_uppercase())
            }
            Broke::Sack => "I SPILLED A SACK OF GRAIN.".to_string(),
            Broke::Nothing => "THERE WAS NOTHING LEFT TO BREAK.".to_string(),
        };
        format!("{}. {broke}", cause.word().to_uppercase())
    } else if f.trade == Trade::Child || f.age < COMING_OF_AGE {
        format!("AT PLAY. {} YEARS OLD.", f.age)
    } else if f.hunger > 0 {
        let granary = if h.stores == 0 { "THE GRANARY IS BARE" } else { "THE GRANARY IS LOW" };
        format!("HUNGRY. {granary}.")
    } else if i >= beds {
        "NO BED FOR ME YET.".to_string()
    } else if held.raid > 0 {
        "AFRAID. RAIDERS HOLD THE PLACE.".to_string()
    } else if h.unrest >= RESTLESS {
        "AFRAID. THE REALM IS RESTLESS.".to_string()
    } else if f.mood <= -2 {
        "GLUM. I MAY WALK AWAY.".to_string()
    } else if f.age >= ELDER {
        "BY THE FIRE. THE WORK IS FOR THE YOUNG.".to_string()
    } else if f.mood >= 3 {
        format!("CONTENT HERE. A GRADE {} HAND.", f.skill)
    } else {
        format!("SETTLED. A GRADE {} HAND.", f.skill)
    };
    format!("{lead}: {says}")
}

impl Run {
    /// One tick of the village in a steward's hall: a press of E beside a
    /// soul is a word with them, or the artifact from their hands. A knight
    /// on a plate presses for the plate, not the soul beside it.
    pub(super) fn folk_talk(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.holding_raid.is_some() {
            return;
        }
        let Some(held) = held_floor(&self.home.holdings, self.dungeon.pack) else {
            return;
        };
        if held.folk.is_empty() {
            return;
        }
        let tick = self.tick;
        let plates = |held: &Holding, x: f32, y: f32| {
            inside(holdings::STEWARD_PLATE, x, y) || (0..held.rooms.len().min(SLOTS)).any(|b| inside(holdings::bay(b).1, x, y))
        };
        let mut talks: Vec<(u32, usize)> = Vec::new();
        for (&id, hero) in &self.players {
            let pressing = inputs.get(&id).copied().filter(|i| i.valid()).is_some_and(|i| i.interact);
            if !pressing || hero.buy_spent || hero.hp == 0 || hero.stone || plates(held, hero.x, hero.y) {
                continue;
            }
            if let Some(i) = strolls::near(held, tick, hero.x, hero.y) {
                talks.push((id, i));
            }
        }
        if talks.is_empty() {
            return;
        }
        let held = held.clone();
        for (id, i) in talks {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.buy_spent = true;
            }
            let f = &held.folk[i];
            let mine = held.works.iter().find(|m| m.maker == f.name && m.trade == f.trade).and_then(|m| m.curio());
            let line = match mine {
                Some(curio) => self.take_work(id, curio),
                None => words(&self.home.holdings, &held, i),
            };
            self.sounds.push("play_card");
            self.found = Some((tick, id, line));
        }
    }

    /// Knight `id` takes the masterwork `curio` into their pockets.
    fn take_work(&mut self, id: u32, curio: &'static curios::Curio) -> String {
        let name = curio.name.to_uppercase();
        if let Some(other) = self.players.iter().find(|(o, h)| **o != id && h.pockets.has(curio.id)).map(|(_, h)| h.name.to_uppercase()) {
            return format!("{other} CARRIES THE {name} THIS RUN. IT IS BACK IN THE WORKSHOP NEXT RUN.");
        }
        let book = self.book.clone();
        let Some(hero) = self.players.get_mut(&id) else {
            return String::new();
        };
        if hero.pockets.has(curio.id) {
            format!("YOU CARRY THE {name} ALREADY.")
        } else if hero.pockets.items.len() >= POCKETS {
            format!("YOUR POCKETS ARE FULL ({POCKETS}). THE {name} WAITS.")
        } else {
            hero.pockets.items.push(curio.id.to_string());
            hero.rebonus(&book);
            self.sounds.push("spoil_pickup");
            format!("{name}. + {}. - {}.", curio.boon, curio.bane)
        }
    }

    /// The party comes into a steward's hall: if the village is in the
    /// middle of something, the hall says what.
    pub(super) fn folk_arrive(&mut self) {
        let Some(held) = held_floor(&self.home.holdings, self.dungeon.pack) else {
            return;
        };
        let name = |s: &str| s.to_uppercase();
        let (cue, line) = match (&held.occasion, &held.seized) {
            (Some(Occasion::Mourning { who, trade, dying }), _) => (
                "folk_mourning",
                format!("THE VILLAGE MOURNS {} THE {}, DEAD {}.", name(who), name(trade.word()), name(dying.word())),
            ),
            (Some(Occasion::Tantrum { who, cause, .. }), _) => (
                "folk_tantrum",
                format!("{} IS {} AND HAS THROWN A TANTRUM. SPEAK WITH THEM.", name(who), name(cause.word())),
            ),
            (Some(Occasion::Feast), _) => ("folk_feast", "THE VILLAGE FEASTS: THE GRANARY WAS FULL.".to_string()),
            (None, Some(s)) => (
                "folk_fey",
                format!("{} THE {} IS SEIZED BY A FEY MOOD AND HAS TAKEN THE WORKSHOP.", name(&s.who), name(s.trade.word())),
            ),
            (None, None) => match held.works.last() {
                Some(m) => (
                    "folk_masterwork",
                    format!("{} STANDS FINISHED. {} KEEPS WATCH OVER IT.", name(m.curio().map_or("A MASTERWORK", |c| c.name)), name(&m.maker)),
                ),
                None => return,
            },
        };
        self.cues.push(cue.to_string());
        self.found = Some((self.tick, 0, line));
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_folk_life__tests.rs"]
mod tests;
