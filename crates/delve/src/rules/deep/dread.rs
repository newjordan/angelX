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

pub const PREFIX: &str = "deep-";

impl Culture {
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

pub fn is(boss: &Boss) -> bool {
    boss.id.starts_with(PREFIX)
}

pub fn culture_of(id: &str) -> Option<Culture> {
    WHEEL.into_iter().find(|c| c.dread().0 == id)
}

pub fn guards(floor: u32) -> Option<u32> {
    (floor >= ROAD && is_village(floor + 1)).then_some(floor + 1)
}

const GROWS_TO: u32 = 7;

pub fn boss(culture: Culture, chapter: u32) -> Boss {
    let (id, raw) = culture.dread();
    let mut b = bosses::check(id, raw).expect("a dread passes the boss checker").0;
    let up = chapter.saturating_sub(1).min(GROWS_TO);
    b.hp = (b.hp + 40 * up).min(900);
    b.rage = (b.rage + 0.02 * up as f32).min(0.6);
    if let Some(sig) = b.windups.first_mut() {
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

pub fn awaken(b: &mut Boss, stir: u8) {
    let s = u32::from(stir.min(road::MOST_STIR));
    b.hp = (b.hp + STIR_HP * s).min(900);
    b.rage = (b.rage + STIR_RAGE * s as f32).min(0.6);
    if let Some(sig) = b.windups.first_mut() {
        sig.every = sig.every.saturating_sub(STIR_SOONER * s).max(150);
    }
}

pub const STIR_HP: u32 = 40;
pub const STIR_RAGE: f32 = 0.03;
pub const STIR_SOONER: u32 = 15;

fn most_shots(kind: Windup) -> u32 {
    match kind {
        Windup::Slam => 3,
        Windup::Leap | Windup::Inhale => 8,
        Windup::Charge | Windup::Beam => 0,
    }
}

impl Run {
    pub(in crate::rules) fn ready_dread(&mut self) -> Option<usize> {
        let village = self.deep_floor().and_then(guards)?;
        let mut fresh = boss(self.home.deep.culture_at(village), chapter(village));
        awaken(&mut fresh, self.deep.as_ref().map_or(0, |d| d.stir));
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

    pub(in crate::rules) fn dread_at(&self) -> Option<usize> {
        let village = self.deep_floor().and_then(guards)?;
        let id = self.home.deep.culture_at(village).dread().0;
        self.bosses.iter().position(|b| b.id == id)
    }

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
