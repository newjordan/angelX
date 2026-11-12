//! The Wax Maze's show: what lately happened in the maze, kept so the picture
//! can make a fuss of it (`render::overworld::arena::wax_maze` draws it).
//!
//! The rules (`folly`) tell it what happened: a drop eaten, the holy water
//! taken, a pale wraith caught, a knight touched, the lead changing hands,
//! the wax and the candle running out. It answers with the calls to shout
//! (`Shout`) and keeps each knight's run of bites (the combo). Nothing here
//! changes a score or a move: it is only memory, with the juice kit's own
//! weights (`juice::Weight`) saying how hard each moment hits.
//!
//! Time is the run's own tick count, so the host, a friend's mirror and a
//! browser guest draw the same moment. Every list is capped (`valid`).

use super::juice::Weight;
use super::*;

/// Ticks each kind of moment stays on the books for its effects.
pub const POP_LIFE: u32 = 10;
pub const SPLASH_LIFE: u32 = 44;
pub const CATCH_LIFE: u32 = 48;
pub const TOUCH_LIFE: u32 = 52;
pub const SHOUT_LIFE: u32 = super::juice::CALL_LIFE;
/// Bites this near together (ticks) run on as one combo; a turn or two
/// keeps it, a stop or a crossing of eaten lane breaks it.
pub const MUNCH_GAP: u32 = 16;
/// The combo sizes that earn a banner.
pub const FEAST: [u16; 3] = [25, 50, 80];
/// Wax left when the last bites are called.
pub const LAST_BITES: u16 = 14;

const MOST_POPS: usize = 28;
const MOST_MOMENTS: usize = 8;
const MOST_SHOUTS: usize = 6;

/// A drop eaten: where, by whom, when.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pop {
    pub tile: u16,
    pub who: u8,
    pub born: u32,
}

/// The holy water taken, where it stood.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Splash {
    pub tile: u16,
    pub who: u8,
    pub born: u32,
}

/// A pale wraith caught: where it was (tile sixteenths, which are room
/// pixels), which king (`Temper::ALL`'s place), what it was worth and how
/// many had fallen to this water.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catch {
    pub at: (i32, i32),
    pub king: u8,
    pub who: u8,
    pub worth: u8,
    pub chain: u8,
    pub born: u32,
}

/// A knight touched by a calm wraith: where they stood, where the wraith
/// came from (tile sixteenths) and the wax they spilled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Touch {
    pub at: (i32, i32),
    pub from: (i32, i32),
    pub who: u8,
    pub lost: u8,
    pub born: u32,
}

/// What a banner calls. Words are the picture's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    /// A knight's combo reached the n-th of `FEAST`.
    Feast(u8),
    /// A knight took the holy water.
    Holy,
    /// The n-th wraith of one water caught (2 up).
    Catch(u8),
    /// A knight took the lead.
    Lead,
    /// Half the wax is gone.
    Half,
    /// The last bites are left.
    Last,
    /// The calm wraiths quicken as the candle burns low.
    Quicken,
}

impl Cry {
    /// How loudly it is shown: the banner with the higher rank stays on top.
    pub fn rank(self) -> u8 {
        match self {
            Cry::Feast(n) => 2 + n,
            Cry::Holy => 3,
            Cry::Catch(n) => 5 + n.min(4),
            Cry::Lead => 3,
            Cry::Half => 4,
            Cry::Last => 8,
            Cry::Quicken => 7,
        }
    }
}

/// A banner called for a knight (`who` 0: for the whole room) at a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shout {
    pub cry: Cry,
    pub who: u8,
    pub born: u32,
}

/// How the maze ended: when (run tick), who took it, the pot, the most wax
/// and whether the candle did it (the wax gone otherwise).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ended {
    pub at: u32,
    pub winner: u8,
    pub pot: u32,
    pub most: u16,
    pub burnt: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaxShow {
    pub pops: Vec<Pop>,
    pub splashes: Vec<Splash>,
    pub catches: Vec<Catch>,
    pub touches: Vec<Touch>,
    pub shouts: Vec<Shout>,
    /// Each knight's run of bites: the tick of the last and the count.
    pub combo: BTreeMap<u32, (u32, u16)>,
    /// The longest run each knight made.
    pub best: BTreeMap<u32, u16>,
    /// The knight now ahead on their own.
    pub lead: Option<u8>,
    /// Wax laid when the maze began.
    pub total: u16,
    /// Which of the room's calls have been made (`HALF`, `LAST`, `QUICKEN`).
    pub called: u8,
    pub ended: Option<Ended>,
}

const HALF: u8 = 1;
const LAST: u8 = 2;
const QUICKEN: u8 = 4;

impl WaxShow {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self) -> bool {
        let seat = |id: u32| (1..=MAX_PLAYERS).contains(&id);
        self.pops.len() <= MOST_POPS
            && self.splashes.len() <= MOST_MOMENTS
            && self.catches.len() <= MOST_MOMENTS
            && self.touches.len() <= MOST_MOMENTS
            && self.shouts.len() <= MOST_SHOUTS
            && self.combo.len() <= MAX_PLAYERS as usize
            && self.best.len() <= MAX_PLAYERS as usize
            && self.combo.keys().chain(self.best.keys()).all(|&id| seat(id))
            && self.pops.iter().all(|p| seat(u32::from(p.who)))
            && self.splashes.iter().all(|p| seat(u32::from(p.who)))
            && self.catches.iter().all(|c| seat(u32::from(c.who)) && c.king < 4 && c.chain <= 4)
            && self.touches.iter().all(|t| seat(u32::from(t.who)))
            && self.lead.is_none_or(|id| seat(u32::from(id)))
            && self.ended.is_none_or(|e| e.winner == 0 || seat(u32::from(e.winner)))
    }

    /// Let go what is old at `now`.
    pub fn tick(&mut self, now: u32) {
        self.pops.retain(|p| now.saturating_sub(p.born) < POP_LIFE);
        self.splashes.retain(|p| now.saturating_sub(p.born) < SPLASH_LIFE);
        self.catches.retain(|p| now.saturating_sub(p.born) < CATCH_LIFE);
        self.touches.retain(|p| now.saturating_sub(p.born) < TOUCH_LIFE);
        self.shouts.retain(|p| now.saturating_sub(p.born) < SHOUT_LIFE);
    }

    /// A drop eaten: it pops, and runs the knight's combo on. Returns the
    /// combo's size now.
    pub fn bite(&mut self, now: u32, who: u32, tile: u16) -> u16 {
        if self.pops.len() >= MOST_POPS {
            self.pops.remove(0);
        }
        self.pops.push(Pop { tile, who: who as u8, born: now });
        let (last, n) = self.combo.get(&who).copied().unwrap_or((0, 0));
        let n = if n > 0 && now.saturating_sub(last) <= MUNCH_GAP { n.saturating_add(1) } else { 1 };
        self.combo.insert(who, (now, n));
        let best = self.best.entry(who).or_default();
        *best = (*best).max(n);
        if let Some(rung) = FEAST.iter().position(|&f| f == n) {
            self.shout(now, Cry::Feast(rung as u8), who);
        }
        n
    }

    /// A knight's combo now: their bites in a row, while it still runs.
    pub fn run_of(&self, who: u32, now: u32) -> u16 {
        match self.combo.get(&who) {
            Some(&(last, n)) if now.saturating_sub(last) <= MUNCH_GAP => n,
            _ => 0,
        }
    }

    /// A knight's combo broken: a wraith touched them.
    pub fn broken(&mut self, who: u32) {
        self.combo.remove(&who);
    }

    /// The holy water taken from `tile`.
    pub fn splash(&mut self, now: u32, who: u32, tile: u16) {
        if self.splashes.len() >= MOST_MOMENTS {
            self.splashes.remove(0);
        }
        self.splashes.push(Splash { tile, who: who as u8, born: now });
        self.shout(now, Cry::Holy, who);
    }

    /// A pale wraith caught, the `chain`-th on this water.
    pub fn caught(&mut self, now: u32, who: u32, at: (i32, i32), king: usize, worth: u16, chain: u8) {
        if self.catches.len() >= MOST_MOMENTS {
            self.catches.remove(0);
        }
        self.catches.push(Catch { at, king: king as u8, who: who as u8, worth: worth.min(255) as u8, chain, born: now });
        if chain >= 2 {
            self.shout(now, Cry::Catch(chain), who);
        }
    }

    /// A knight touched.
    pub fn touched(&mut self, now: u32, who: u32, at: (i32, i32), from: (i32, i32), lost: u16) {
        if self.touches.len() >= MOST_MOMENTS {
            self.touches.remove(0);
        }
        self.touches.push(Touch { at, from, who: who as u8, lost: lost.min(255) as u8, born: now });
        self.broken(who);
    }

    /// Call a banner (the loudest stays on top when they come together).
    pub fn shout(&mut self, now: u32, cry: Cry, who: u32) {
        if self.shouts.len() >= MOST_SHOUTS {
            self.shouts.remove(0);
        }
        self.shouts.push(Shout { cry, who: who as u8, born: now });
    }

    /// After a score moved: the lead's change of hands, half the wax gone,
    /// the last bites. `left` is the wax still on the floor.
    pub fn standings(&mut self, now: u32, eaten: &BTreeMap<u32, u16>, left: u16) {
        let top = eaten.values().copied().max().unwrap_or(0);
        let leaders: Vec<u32> = eaten.iter().filter(|&(_, &n)| n == top && top > 0).map(|(&id, _)| id).collect();
        // A lead worth calling: out of a tie, a few drops in, taken from
        // another's hands.
        let lead = (leaders.len() == 1 && eaten.len() > 1 && top >= 12).then(|| leaders[0] as u8);
        if let Some(one) = lead
            && self.lead != Some(one)
        {
            if self.lead.is_some() {
                self.shout(now, Cry::Lead, u32::from(one));
            }
            self.lead = Some(one);
        }
        if self.total > 0 && left * 2 <= self.total && self.called & HALF == 0 {
            self.called |= HALF;
            self.shout(now, Cry::Half, 0);
        }
        if self.total > 0 && left <= LAST_BITES && self.called & LAST == 0 && self.called & HALF != 0 {
            self.called |= LAST;
            self.shout(now, Cry::Last, 0);
        }
    }

    /// The calm wraiths quicken: called once.
    pub fn quicken(&mut self, now: u32) {
        if self.called & QUICKEN == 0 {
            self.called |= QUICKEN;
            self.shout(now, Cry::Quicken, 0);
        }
    }

    /// The maze is done.
    pub fn finish(&mut self, now: u32, winner: u32, pot: u32, most: u16, burnt: bool) {
        self.ended = Some(Ended { at: now, winner: winner as u8, pot, most, burnt });
    }

    /// Ticks since the maze ended at `now`, while it is the show.
    pub fn since_end(&self, now: u32) -> Option<u32> {
        self.ended.map(|e| now.saturating_sub(e.at))
    }
}

/// The weight of a pale wraith caught, the `chain`-th on one water: the
/// first holds the room as a kill does, the fourth as the winning blow.
pub fn catch_weight(chain: u8) -> Weight {
    if chain >= 4 { Weight::Finale } else { Weight::Kill }
}

#[cfg(test)]
#[path = "../../tests/rules/wax_show__tests.rs"]
mod tests;
