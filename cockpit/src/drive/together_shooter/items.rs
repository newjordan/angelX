//! Items as play cards a knight keeps in hand: the Fae
//! Dagger (you are where you were aiming), the Pendragon Sceptre (for a while
//! no hex, chill, web or hook takes), and the Censer (a green pulse that
//! mends every knight close by). Their card files are in the delve's book.

use super::*;

/// How far a Censer's pulse reaches.
pub(crate) const CENSER_REACH: f32 = 8.0;

/// The Fae Dagger: the knight along its aim, in short steps so that no
/// wall is passed through; a breath of grace on arrival.
pub(super) fn blink(hero: &mut Hero, grid: &Grid) {
    if hero.blink == 0 {
        return;
    }
    let (ax, ay) = unit(hero.aim_x, hero.aim_y);
    let mut left = hero.blink as f32;
    while left > 0.0 {
        let step = left.min(0.5);
        let was = (hero.x, hero.y);
        (hero.x, hero.y) = grid.slide(
            (hero.x, hero.y),
            (ax * step, ay * step),
            HERO_RADIUS,
            Mover::Hero,
        );
        if (hero.x - was.0).hypot(hero.y - was.1) < step * 0.5 {
            break;
        }
        left -= step;
    }
    hero.blink = 0;
    hero.invulnerable = hero.invulnerable.max(6);
}

impl Run {
    /// Each tick: Censer pulses go out; Pendragon Sceptres run down.
    pub(super) fn tick_items(&mut self) {
        let pulses: Vec<(u32, f32, f32, u32)> = self
            .players
            .iter_mut()
            .filter(|(_, h)| h.censer > 0)
            .map(|(&id, h)| {
                let mend = std::mem::take(&mut h.censer);
                (id, h.x, h.y, mend)
            })
            .collect();
        for (id, x, y, mend) in pulses {
            for hero in self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && (h.x - x).hypot(h.y - y) < CENSER_REACH)
            {
                hero.hp = (hero.hp + mend).min(hero.max_hp);
            }
            if self.boons.len() < 8 {
                self.boons.push((id, self.tick));
            }
            self.cues.push("censer".into());
        }
        for hero in self.players.values_mut() {
            hero.immune = hero.immune.saturating_sub(1);
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_items__tests.rs"]
mod tests;
