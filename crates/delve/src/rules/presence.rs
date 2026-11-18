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

pub const WAKE_WARD: u32 = HZ;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hold {
    #[default]
    Open,
    Arriving,
    Empty,
}

impl Run {
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
            hero.away = false;
            hero.idle = hero.hp > 0;
            hero.stone = hero.idle || hero.vigil;
            hero.invulnerable = hero.invulnerable.max(WAKE_WARD);
        }
    }

    pub fn rest_until_moved(&mut self) {
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.absent && !h.away) {
            hero.idle = true;
            hero.stone = true;
        }
    }

    pub(super) fn wake_on_first_move(hero: &mut Hero, input: &Input) {
        if hero.idle
            && !hero.away
            && (input.move_x != 0
                || input.move_y != 0
                || input.fire
                || input.dash
                || input.swing
                || input.interact)
        {
            hero.idle = false;
            hero.stone = hero.vigil;
        }
    }

    pub fn set_absent(&mut self, id: u32, absent: bool) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        hero.absent = absent;
        if absent {
            self.set_away(id, true);
        }
    }

    pub fn hold(&mut self, hold: Hold) {
        self.held = hold;
    }

    pub fn holding(&self) -> bool {
        self.held != Hold::Open
    }

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
#[cfg(all(test, feature = "bend-cases-home"))]
#[path = "../../tests/rules/presence_bend_cases__tests.rs"]
mod presence_bend_cases_tests;
