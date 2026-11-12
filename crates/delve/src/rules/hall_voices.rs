//! The hall answers the knights, by name. The keeper greets each knight who
//! comes to his bar and calls what he pours; the patron beside a bench that
//! was just taken up raises a mug to its new knight; a stall's vendor says
//! who bought. Each line is said over the speaker for everyone in the room
//! to read, for [`CALL_TICKS`], and every one is told by something that
//! happened: a knight arrived at the bar, a pour was paid for, a calling was
//! taken up, a ware was sold.
//!
//! The hearth is the hall's too: E at it stokes it, and it leaps for
//! [`STOKE_TICKS`].

use super::alehouse::{self as a, House, PATRONS};
use super::bazaar::{self, STALL};
use super::*;

/// How long a spoken line stays up: a little over three seconds.
pub const CALL_TICKS: u64 = 100;
/// The lines up at once.
pub const MOST_CALLS: usize = 6;
/// The most letters in a spoken line.
pub const MOST_LETTERS: usize = 64;
/// The most letters of a knight's name a line carries.
const NAME_LETTERS: usize = 12;
/// How long a stoked hearth leaps: two seconds.
pub const STOKE_TICKS: u64 = 60;
/// How long a place's name stays up after the party comes into it.
pub const TITLE_TICKS: u64 = 3 * HZ as u64;
/// The bar's aisle, where the keeper greets (tiles: left, top, right,
/// bottom): the strip before his counter, the taps in it.
const BAR_AISLE: (f32, f32, f32, f32) = (13.0, 6.0, 29.0, 7.6);

/// Who says a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Voice {
    /// The keeper behind the bar.
    Keeper,
    /// The fence in the corner.
    Fence,
    /// The patron of `PATRONS[i]`, on their bench.
    Patron(u8),
    /// The vendor of bazaar stall `i`.
    Vendor(u8),
}

impl Voice {
    /// The room the speaker stands in.
    pub fn room(self) -> RoomKind {
        match self {
            Voice::Keeper | Voice::Fence => RoomKind::Alehouse,
            Voice::Patron(i) => a::patron_room(usize::from(i)),
            Voice::Vendor(_) => RoomKind::Bazaar,
        }
    }

    /// Where the speaker's feet are (tiles).
    pub fn feet(self) -> (f32, f32) {
        match self {
            Voice::Keeper => a::KEEPER_AT,
            Voice::Fence => a::FENCE_AT,
            Voice::Patron(i) => PATRONS.get(usize::from(i)).map_or((0.0, 0.0), |p| p.at),
            Voice::Vendor(i) => STALL.get(usize::from(i)).map_or((0.0, 0.0), |s| s.vendor),
        }
    }

    fn valid(self) -> bool {
        match self {
            Voice::Keeper | Voice::Fence => true,
            Voice::Patron(i) => usize::from(i) < PATRONS.len(),
            Voice::Vendor(i) => usize::from(i) < bazaar::STALLS,
        }
    }
}

/// One line said in the hall: when, by whom, the words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub at: u64,
    pub voice: Voice,
    pub text: String,
}

impl Call {
    pub fn valid(&self) -> bool {
        self.voice.valid() && self.text.chars().count() <= MOST_LETTERS
    }
}

/// A knight's name as a line carries it.
fn name_of(hero: &Hero) -> String {
    hero.name.to_uppercase().chars().take(NAME_LETTERS).collect()
}

/// Whether `(x, y)` (arena units) stands in the keeper's aisle.
fn at_the_bar(x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    (BAR_AISLE.0..BAR_AISLE.2).contains(&c) && (BAR_AISLE.1..BAR_AISLE.3).contains(&r)
}

/// The keeper's greeting for a knight: their calling if they have taken
/// one up, else a word by their seat.
fn greeting(hero: &Hero, id: u32) -> String {
    let name = name_of(hero);
    match a::patron_of(hero) {
        Some(i) => format!("EVENING, {name} THE {}. WHAT'LL IT BE?", PATRONS[i].label.to_uppercase()),
        None => match id % 3 {
            0 => format!("{name}! WHAT'LL IT BE?"),
            1 => format!("WELCOME, {name}. MIND YOUR PURSE."),
            _ => format!("{name}, FIND A BENCH. OR A POUR."),
        },
    }
}

impl Run {
    /// `voice` says `text` over its head. A speaker has one line up at a
    /// time; the oldest of too many goes.
    pub fn call(&mut self, voice: Voice, text: impl AsRef<str>) {
        let text: String = text.as_ref().to_uppercase().chars().take(MOST_LETTERS).collect();
        let calls = &mut self.alehouse.calls;
        calls.retain(|c| c.voice != voice);
        calls.push(Call { at: self.tick, voice, text });
        if calls.len() > MOST_CALLS {
            calls.remove(0);
        }
    }

    /// Whether `voice` has a line up now.
    pub fn speaking(&self, voice: Voice) -> bool {
        self.alehouse.calls.iter().any(|c| c.voice == voice && self.tick.saturating_sub(c.at) < CALL_TICKS)
    }

    /// The lines up now in the room the party stands in.
    pub fn calls_up(&self) -> impl Iterator<Item = &Call> {
        let kind = self.room().kind;
        self.alehouse
            .calls
            .iter()
            .filter(move |c| c.voice.room() == kind && self.tick.saturating_sub(c.at) < CALL_TICKS)
    }

    /// One tick of the hall's talk: spent lines go, and the keeper greets,
    /// one knight at a time, whoever has come to his bar.
    pub(super) fn tick_voices(&mut self) {
        let tick = self.tick;
        self.alehouse.calls.retain(|c| tick.saturating_sub(c.at) < CALL_TICKS);
        if self.room().kind != RoomKind::Alehouse || self.speaking(Voice::Keeper) {
            return;
        }
        let greet = self.players.iter().find_map(|(&id, hero)| {
            let standing = hero.hp > 0 && !hero.stone && !hero.absent;
            let fresh = self.alehouse.greeted & (1 << (id & 31)) == 0;
            (standing && fresh && at_the_bar(hero.x, hero.y)).then(|| (id, greeting(hero, id)))
        });
        if let Some((id, line)) = greet {
            self.alehouse.greeted |= 1 << (id & 31);
            self.call(Voice::Keeper, line);
        }
    }

    /// The patron beside bench `i`, still seated, who raises a mug to the
    /// knight who took `i` up; the keeper if no one sits near.
    pub(super) fn toast_pick(&mut self, id: u32, i: usize) {
        let Some(hero) = self.players.get(&id) else { return };
        let (name, label) = (name_of(hero), PATRONS[i].label.to_uppercase());
        let (ax, ay) = PATRONS[i].at;
        let room = a::patron_room(i);
        let neighbour = PATRONS
            .iter()
            .enumerate()
            .filter(|&(j, p)| j != i && a::patron_room(j) == room && self.patron_taken(j, 0).is_none() && (p.at.0 - ax).hypot(p.at.1 - ay) < 3.0)
            .min_by(|(_, p), (_, q)| (p.at.0 - ax).hypot(p.at.1 - ay).total_cmp(&(q.at.0 - ax).hypot(q.at.1 - ay)))
            .map(|(j, _)| j);
        let line = format!("TO {name}, THE {label}!");
        self.sounds.push("hit_note");
        match neighbour {
            Some(j) => self.call(Voice::Patron(j as u8), line),
            None if room == RoomKind::Alehouse => self.call(Voice::Keeper, line),
            None => {}
        }
    }

    /// What the keeper calls when knight `id` has paid for pour `i`.
    pub(super) fn call_pour(&mut self, id: u32, i: usize) {
        let Some(hero) = self.players.get(&id) else { return };
        let name = name_of(hero);
        let line = match i {
            0 => format!("MILD COURAGE FOR {name}!"),
            1 => format!("LOUD OPINIONS FOR {name}!"),
            _ => format!("SECOND THOUGHTS FOR {name}!"),
        };
        self.call(Voice::Keeper, line);
    }

    /// A vendor, or the fence, says who bought what.
    pub(super) fn call_sale(&mut self, id: u32, slot: u8, ware: &str) {
        let Some(hero) = self.players.get(&id) else { return };
        // The ware is cut to fit before "SOLD. NO REFUNDS." is, never after.
        let name = name_of(hero);
        let room = MOST_LETTERS.saturating_sub(name.chars().count() + ": ".len() + " SOLD. NO REFUNDS.".len());
        let ware: String = ware.to_uppercase().chars().take(room).collect();
        let line = format!("{name}: {ware} SOLD. NO REFUNDS.");
        let stall = usize::from(slot) / bazaar::WARES;
        let voice = if stall < bazaar::STALLS { Voice::Vendor(stall as u8) } else { Voice::Fence };
        self.call(voice, line);
    }

    /// The party has come into a room of the world: its name is up for a
    /// moment, and the keeper greets afresh whoever comes to his bar.
    pub(super) fn note_arrival(&mut self) {
        self.alehouse.came = self.tick;
        if self.room().kind == RoomKind::Alehouse {
            self.alehouse.greeted = 0;
        }
    }

    /// How long ago the party came into the room it stands in, while its
    /// name is still up.
    pub fn title_age(&self) -> Option<u64> {
        let age = self.tick.saturating_sub(self.alehouse.came);
        (age < TITLE_TICKS).then_some(age)
    }

    /// The name the room shows on coming into it, and a line of what is
    /// true of it now: the tavern, the bazaar and the Delve's gate.
    pub fn place_title(&self) -> Option<(&'static str, String)> {
        match self.room().kind {
            RoomKind::Alehouse => {
                let free = (0..PATRONS.len())
                    .filter(|&i| a::patron_room(i) == RoomKind::Alehouse && self.patron_taken(i, 0).is_none())
                    .count();
                let line = if self.identity { String::new() } else { format!("{free} CALLINGS FREE AT THE TABLES") };
                Some(("THE SOZZLED GRIFFIN", line))
            }
            RoomKind::Bazaar => Some(("THE BAZAAR", format!("{} STALLS. PRICES ON THE CLOTH.", bazaar::STALLS))),
            RoomKind::Gate => Some(("THE DELVE'S GATE", "THE SOZZLED GRIFFIN, BY THE WELL".into())),
            _ => None,
        }
    }

    /// E at the hearth: it leaps.
    pub(super) fn stoke_hearth(&mut self) {
        self.alehouse.stoked = self.tick;
    }
}

impl House {
    /// How high the stoked hearth is leaping at `tick`, from 1.0 down to
    /// 0.0 as it settles; 0.0 when it was never stoked or has settled.
    pub fn stoke(&self, tick: u64) -> f32 {
        if self.stoked == 0 {
            return 0.0;
        }
        let age = tick.saturating_sub(self.stoked);
        if age >= STOKE_TICKS { 0.0 } else { 1.0 - age as f32 / STOKE_TICKS as f32 }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/hall_voices__tests.rs"]
mod tests;
