//! Who is here. A knight whose player is not (a page still loading, a tab
//! hidden, a link gone quiet, a seat nobody holds) stands as stone: no foe
//! sees it, nothing hurts it, and it holds up nothing the party does. It
//! wakes the moment its player plays again, warded a moment.
//!
//! And the room holds (`Hold`) while the party arrives in it, and while
//! nobody is here at all: its foes and their shots wait and no knight is
//! hurt. The host's server says when (`set_away`, `hold`), from what the
//! engine knows of each seat; the rules alone never hold.

use super::*;

/// A knight back from away is warded this long: a second.
pub const WAKE_WARD: u32 = HZ;

/// Why the room holds its foes (`Run::held`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hold {
    /// Play on.
    #[default]
    Open,
    /// The party has just arrived: every player here is still to see the
    /// room on screen.
    Arriving,
    /// Nobody is here: every knight's player is away.
    Empty,
}

impl Run {
    /// Whether knight `id`'s player is here. Away, the knight is stone;
    /// back, it is itself again (still stone if it keeps vigil) and warded
    /// a moment. Said every tick, it holds the knight to it whatever else
    /// set its stone meanwhile.
    pub fn set_away(&mut self, id: u32, away: bool) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        if away {
            if !hero.away {
                hero.away = true;
                (hero.shielding, hero.walling, hero.dash_ticks) = (false, false, 0);
            }
            hero.stone = true;
        } else if hero.away {
            // Back: still stone, unseen and unhurt, until its player moves.
            hero.away = false;
            hero.idle = hero.hp > 0;
            hero.stone = hero.idle || hero.vigil;
            hero.invulnerable = hero.invulnerable.max(WAKE_WARD);
        }
    }

    /// A round begins: every standing knight is stone, unseen and unhurt,
    /// until its player first moves, fires, dashes or swings.
    pub fn rest_until_moved(&mut self) {
        // Only knights a player is here for: the host's empty place and a
        // seat whose player is away are not waiting to move.
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.absent && !h.away) {
            hero.idle = true;
            hero.stone = true;
        }
    }

    /// An idle knight wakes on its player's first move this round.
    pub(super) fn wake_on_first_move(hero: &mut Hero, input: &Input) {
        if hero.idle
            && !hero.away
            && (input.move_x != 0 || input.move_y != 0 || input.fire || input.dash || input.swing)
        {
            hero.idle = false;
            hero.stone = hero.vigil;
        }
    }

    /// Whether knight `id` is nobody's at all: the host's own place, kept
    /// for a host who plays no knight. Absent, it is away (stone, unseen,
    /// unhurt), counts for nothing in the party (`Run::party_len`), and is
    /// never sent to a client (`wire::publish`), so no screen draws it.
    pub fn set_absent(&mut self, id: u32, absent: bool) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        hero.absent = absent;
        if absent {
            self.set_away(id, true);
        }
    }

    /// Hold the room, or let it play (`Hold`).
    pub fn hold(&mut self, hold: Hold) {
        self.held = hold;
    }

    /// The room holds this tick: foes and their shots wait.
    pub fn holding(&self) -> bool {
        self.held != Hold::Open
    }

    /// While the room holds, no knight is hurt: every one is warded
    /// through the tick.
    pub(super) fn ward_while_held(&mut self) {
        if !self.holding() {
            return;
        }
        for hero in self.players.values_mut() {
            hero.invulnerable = hero.invulnerable.max(2);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_presence__tests.rs"]
mod tests;
