//! What the circles below the hell gate do to a fight, past the health and
//! the hits every circle adds (`spectacle`, `CIRCLE_HP` and `CIRCLE_HURT`).
//!
//! Hell pushes with pressure, never by taking anything from the knights:
//!
//!  - **The horde.** Every company is bigger, more so each circle down
//!    (`HORDE`), and no circle's first room is the gentle one the floors
//!    above keep for a party off the stairs.
//!  - **Champions.** Every company is led by one of its own made an elite
//!    (`CHAMPIONS`): bigger, two and a half times the health, a shot more in
//!    each volley. In the Fifth Circle there are two.
//!  - **Embers.** From the Second Circle, while a company fights, embers
//!    fall where the knights stand (`EMBERS`): a fiery ring grows on the
//!    floor for over a second, then it lands. Hell's fire spares its own,
//!    and a hit from it is a hit like any other: the ward after a hit holds.
//!  - **Less mercy.** Draughts the slain drop often burn up before they
//!    reach the floor (`MERCY`); the dragon's heart never does.
//!
//! The circles, by the old poet's names: Limbo brings the horde, its
//! champions and the first burnt draughts; Lust the first embers; Gluttony
//! a bigger horde; Greed a thicker rain; Wrath the biggest horde, two
//! champions to a company and embers every two and a half seconds. Below
//! the Fifth, every circle is as the Fifth, and each still adds its health
//! and its hits.
//!
//! `hell_scale_sim` (`tests/rules/together_party_scale__tests.rs`) walks
//! parties of two to six into the first rooms of each circle.

use super::*;

/// More foes in every company, by circle (index 0 is the First), before a
/// chamber's share by its distance from the stairs (`stakes::RAMP_FIRST`).
const HORDE: [usize; 5] = [2, 2, 3, 3, 4];
/// Champions at the head of every company, by circle.
const CHAMPIONS: [usize; 5] = [1, 1, 1, 1, 2];
/// Ticks between embers falling while a company fights, by circle; none in
/// the First.
const EMBERS: [u32; 5] = [0, 4 * HZ, 4 * HZ, 3 * HZ, 5 * HZ / 2];
/// The most embers falling at once.
const EMBERS_AT_ONCE: usize = 6;
/// What a landing ember deals, before the circle, the floor and the mail.
pub(super) const EMBER_HIT: u32 = 10;
/// Percent of the slain's draughts and hearts that reach the floor whole,
/// by circle.
const MERCY: [u32; 5] = [65, 55, 45, 35, 30];

/// A circle's entry in one of the tables above; 0 (above the gate) has none.
fn by_circle<T: Copy>(table: &[T], circle: u32) -> Option<T> {
    (circle > 0).then(|| table[(circle as usize - 1).min(table.len() - 1)])
}

/// The foes a company of this circle brings over its own (`HORDE`).
pub(super) fn horde(circle: u32) -> usize {
    by_circle(&HORDE, circle).unwrap_or(0)
}

impl Run {
    /// Make the toughest of the company raised from `from` on its circle's
    /// champions (`CHAMPIONS`).
    pub(super) fn crown_champions(&mut self, from: usize) {
        let Some(n) = by_circle(&CHAMPIONS, self.spectacle.circle) else {
            return;
        };
        let mut company: Vec<usize> = (from..self.enemies.len())
            .filter(|&i| {
                let e = &self.enemies[i];
                !e.is_boss() && e.kind != EnemyKind::Dummy && e.kind != EnemyKind::Ward
            })
            .collect();
        company.sort_by_key(|&i| (std::cmp::Reverse(self.enemies[i].kind.hp()), i));
        let crowned = company.iter().filter(|&&i| self.enemies[i].elite).count();
        let plain: Vec<usize> = company.into_iter().filter(|&i| !self.enemies[i].elite).collect();
        for i in plain.into_iter().take(n.saturating_sub(crowned)) {
            self.enemies[i].raise_elite();
        }
    }

    /// Embers fall about the knights while a company of a circle fights
    /// (`EMBERS`): each lands where its ring was drawn.
    pub(super) fn rain_embers(&mut self) {
        let Some(every) = by_circle(&EMBERS, self.spectacle.circle).filter(|&t| t > 0) else {
            return;
        };
        if self.at_home_now() || !self.fighting() || !(self.tick + 7).is_multiple_of(u64::from(every)) {
            return;
        }
        if self.rocks.iter().filter(|r| r.ember && r.fall > 0).count() >= EMBERS_AT_ONCE {
            return;
        }
        let living: Vec<(f32, f32)> = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone && !h.absent)
            .map(|h| (h.x, h.y))
            .collect();
        if living.is_empty() {
            return;
        }
        let (hx, hy) = living[self.rng.below(living.len())];
        let jitter = |rng: &mut Rng| (rng.below(51) as f32 / 10.0) - 2.5;
        let (w, h) = (self.room().width(), self.room().height());
        let x = (hx + jitter(&mut self.rng)).clamp(3.0, w - 3.0);
        let y = (hy + jitter(&mut self.rng)).clamp(3.0, h - 3.0);
        self.rocks.push(hazards::Rock {
            x,
            y,
            fall: hazards::ROCK_FALL,
            dust: 0,
            ember: true,
        });
    }

    /// Whether a card the slain drop reaches the floor: in a circle, a
    /// draught or a heart often burns up first (`MERCY`).
    pub(super) fn spared_by_hell(&mut self, card: &str) -> bool {
        let Some(kept) = by_circle(&MERCY, self.spectacle.circle) else {
            return true;
        };
        let heals = self
            .book
            .get(card)
            .is_some_and(|c| c.effects.iter().any(|e| matches!(e, cards::Effect::Heal(_))));
        !heals || self.rng.chance(kept)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_hell__tests.rs"]
mod tests;
