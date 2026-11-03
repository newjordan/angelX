//! The dreads of the Deep: the thing each people fears, waiting at the
//! stair on the floor above their village. The Capfolk's lamps went out
//! when the Glow-Drinker came and drank them; the Glimmerkin lose their
//! carts to the Seam-Swallower; the Emberfolk's coals feed the Coal-Glutton;
//! the Lampwick's lost row out into the Mist-Mother's arms; the Roostfolk's
//! bats are snared by the Snare-Queen; the Rootkin's tree rots from the
//! Blight; the Chimefolk's bells went under with the Drowned Toll; the
//! Saltkin's brine is drunk by the Brine-Wyrm.
//!
//! A dread is a `.boss` file like any guardian's, and fights by the same
//! checked rules: its signature is one of the file's charge-ups, shown on
//! the floor before it lands, drawn in its people's own way. It is never
//! one of a delve's own guardians: it is kept out of every pack's catalogue
//! (its `where` is the swamp, which the Deep never goes to, and its id is
//! its own), and a run takes it on at the end of its list when its floor
//! is reached, so no other boss's place moves (as `nooks::gnome` keeps
//! the Lawn Flamingo).
//!
//! It grows with its chapter (`boss`): the deeper the village, the more of
//! it there is, the sooner and harder its signature comes, and from the
//! second time it is met a second charge-up joins its fight. Felled, the
//! village below hears it fall (`Town::dread`): the star lands on a village
//! a step brighter already.

use super::cultures::{Culture, WHEEL};
use super::*;
use crate::rules::bosses::{self, Boss, Stage, Windup};

/// Every dread's id starts so.
pub const PREFIX: &str = "deep-";

impl Culture {
    /// The people's dread, as its file names it, and the file.
    pub fn dread(self) -> (&'static str, &'static str) {
        match self {
            Culture::Capfolk => ("deep-glow-drinker", include_str!("../../../../../cockpit/assets/dungeon/bosses/deep-glow-drinker.boss")),
            Culture::Glimmerkin => ("deep-seam-swallower", include_str!("../../../../../cockpit/assets/dungeon/bosses/deep-seam-swallower.boss")),
            Culture::Emberfolk => ("deep-coal-glutton", include_str!("../../../../../cockpit/assets/dungeon/bosses/deep-coal-glutton.boss")),
            Culture::Lampwick => ("deep-mist-mother", include_str!("../../../../../cockpit/assets/dungeon/bosses/deep-mist-mother.boss")),
            Culture::Roostfolk => ("deep-snare-queen", include_str!("../../../../../cockpit/assets/dungeon/bosses/deep-snare-queen.boss")),
            Culture::Rootkin => ("deep-blight", include_str!("../../../../../cockpit/assets/dungeon/bosses/deep-blight.boss")),
            Culture::Chimefolk => ("deep-drowned-toll", include_str!("../../../../../cockpit/assets/dungeon/bosses/deep-drowned-toll.boss")),
            Culture::Saltkin => ("deep-brine-wyrm", include_str!("../../../../../cockpit/assets/dungeon/bosses/deep-brine-wyrm.boss")),
        }
    }

    /// What the dread took from the people, as the village says it when it
    /// falls.
    pub fn dread_took(self) -> &'static str {
        match self {
            Culture::Capfolk => "THE GLOW IT DRANK GOES HOME",
            Culture::Glimmerkin => "THE SEAMS ARE SAFE TO DIG",
            Culture::Emberfolk => "THE STOLEN COALS GO BACK TO THE FORGE",
            Culture::Lampwick => "THE MIST LIFTS OFF THE LAKE",
            Culture::Roostfolk => "THE SNARES COME DOWN",
            Culture::Rootkin => "THE ROT STOPS SPREADING",
            Culture::Chimefolk => "THE BELLS CAN RING AGAIN",
            Culture::Saltkin => "THE BRINE WILL RISE AGAIN",
        }
    }
}

/// Whether `boss` is one of the Deep's dreads.
pub fn is(boss: &Boss) -> bool {
    boss.id.starts_with(PREFIX)
}

/// The people whose dread `id` is.
pub fn culture_of(id: &str) -> Option<Culture> {
    WHEEL.into_iter().find(|c| c.dread().0 == id)
}

/// The village floor `floor` stands above, if it does: its dread waits at
/// this floor's stair.
pub fn guards(floor: u32) -> Option<u32> {
    (floor >= ROAD && is_village(floor + 1)).then_some(floor + 1)
}

/// The most a dread grows: by its eighth chapter it is all it will be.
const GROWS_TO: u32 = 7;

/// `culture`'s dread as it stands in chapter `chapter` (the village's,
/// from 1). Every number stays inside the boss checker's own bounds.
pub fn boss(culture: Culture, chapter: u32) -> Boss {
    let (id, raw) = culture.dread();
    let mut b = bosses::check(id, raw).expect("a dread passes the boss checker").0;
    let up = chapter.saturating_sub(1).min(GROWS_TO);
    b.hp = (b.hp + 40 * up).min(900);
    b.rage = (b.rage + 0.02 * up as f32).min(0.6);
    if let Some(sig) = b.windups.first_mut() {
        // Its signature: sooner, harder, fuller, and from the third
        // chapter on it opens the fight with it.
        sig.every = sig.every.saturating_sub(20 * up).max(150);
        sig.damage = (sig.damage + up).min(20);
        sig.shots = (sig.shots + up / 2).min(most_shots(sig.kind));
        if sig.kind != Windup::Slam {
            sig.wind = sig.wind.saturating_sub(2 * up).max(bosses::WIND_LEAST + 6);
        }
        if up >= 2 {
            sig.from = Stage::Open;
        }
    }
    // Its second charge-up: not the first time it is met; in its rage the
    // second; from the middle of the fight after that.
    match up {
        0 => b.windups.truncate(1),
        1 => {
            if let Some(second) = b.windups.get_mut(1) {
                second.from = Stage::Rage;
            }
        }
        _ => {
            if let Some(second) = b.windups.get_mut(1) {
                second.from = Stage::Middle;
                second.damage = (second.damage + up / 2).min(20);
            }
        }
    }
    b
}

/// The most shots a charge-up of `kind` may throw (`bosses::check`'s own
/// bounds).
fn most_shots(kind: Windup) -> u32 {
    match kind {
        Windup::Slam => 3,
        Windup::Leap | Windup::Inhale => 8,
        Windup::Charge | Windup::Beam => 0,
    }
}

impl Run {
    /// The dread at this floor's stair, if this floor of the Deep stands
    /// above a village: its place in the run's bosses, put there at the
    /// end of the list (or made its chapter's strength) if need be.
    pub(in crate::rules) fn ready_dread(&mut self) -> Option<usize> {
        let village = self.deep_floor().and_then(guards)?;
        let fresh = boss(self.home.deep.culture_at(village), chapter(village));
        if let Some(i) = self.bosses.iter().position(|b| b.id == fresh.id) {
            if self.bosses[i] != fresh {
                self.bosses[i] = fresh;
            }
            return Some(i);
        }
        if self.bosses.len() >= 64 {
            return None;
        }
        self.bosses.push(fresh);
        Some(self.bosses.len() - 1)
    }

    /// Where this floor's dread stands in the run's bosses, if this floor
    /// keeps one and it is there.
    pub(in crate::rules) fn dread_at(&self) -> Option<usize> {
        let village = self.deep_floor().and_then(guards)?;
        let id = self.home.deep.culture_at(village).dread().0;
        self.bosses.iter().position(|b| b.id == id)
    }

    /// The dread `id` has fallen: the village below hears it, and the
    /// realm keeps it.
    pub(in crate::rules) fn dread_felled(&mut self, id: &str, name: &str) {
        let (Some(culture), Some(village)) = (culture_of(id), self.deep_floor().and_then(guards)) else {
            return;
        };
        if self.home.deep.town(village).is_none() && self.home.deep.towns.len() < MOST_TOWNS {
            self.home.deep.towns.push(Town::new(village, culture));
        }
        if let Some(town) = self.home.deep.town_mut(village) {
            town.dread = true;
        }
        let place = culture.town(chapter(village));
        self.cues.push("deep_dread_felled".into());
        self.found = Some((self.tick, 0, format!("{name} FALLS. {}: {place} WILL HEAR IT.", culture.dread_took())));
    }
}
