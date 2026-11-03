//! Bounded shot feedback carried by the snapshot, never guessed by a viewer.
use super::{Projectile, Shot, feel::Feel, patterns};
use serde::{Deserialize, Serialize};

pub const LIFE: u8 = 20;
const MAX: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mark {
    Release,
    Wall,
    Flesh,
    Pop,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub kind: Shot,
    pub hostile: bool,
    pub owner: u8,
    pub radius: f32,
    pub mark: Mark,
    pub ttl: u8,
    /// The spell that loosed it, by its knight's slot and one (0: none),
    /// for the spell's own impact (`ShotTraits::spell`).
    #[serde(default, skip_serializing_if = "unspelled")]
    pub spell: u8,
}
fn unspelled(n: &u8) -> bool {
    *n == 0
}
impl Event {
    pub fn age(&self) -> u8 {
        (LIFE - 1).saturating_sub(self.ttl)
    }
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.dx, self.dy, self.radius]
            .iter()
            .all(|v| v.is_finite())
            && self.radius >= 0.0
            && self.ttl <= LIFE
    }
}
impl Feel {
    pub(super) fn ranged(&mut self, shot: &Projectile, mark: Mark) {
        if self.ranged.len() >= MAX {
            self.ranged.remove(0);
        }
        let speed = shot.vx.hypot(shot.vy);
        let (dx, dy) = if speed > 0.001 {
            (shot.vx / speed, shot.vy / speed)
        } else {
            (1.0, 0.0)
        };
        self.ranged.push(Event {
            x: shot.x,
            y: shot.y,
            dx,
            dy,
            kind: shot.kind,
            hostile: shot.hostile,
            owner: shot.traits.owner,
            radius: patterns::girth(shot),
            mark,
            ttl: LIFE,
            spell: shot.traits.spell,
        });
    }
    pub(super) fn ranged_valid(&self) -> bool {
        self.ranged.len() <= MAX && self.ranged.iter().all(Event::valid)
    }
}
#[cfg(test)]
#[path = "../../tests/rules/ranged__tests.rs"]
mod tests;
