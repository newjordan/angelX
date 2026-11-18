//! Ravage: a guardian's `ravage` attack throws the
//! floor up round it. Ripples show every ring at once, one lane left clear;
//! then the rings burst outward one after another, and a knight on a ring as
//! it bursts is struck and thrown into the air, helpless for half a second.
//! The rings touch, so there is no standing between them: step into the
//! lane, roll as your ring comes, or be farther off than the last one.
//! A Pendragon Sceptre does not help.

use super::*;
use std::f32::consts::{PI, TAU};

pub const WARN: u32 = 36;
pub const RING_GAP: f32 = 2.2;
pub const BAND: f32 = RING_GAP / 2.0 + 0.05;
pub const UP: u32 = 15;
const THROW: f32 = 1.2;
pub const TOSSED: u32 = HZ / 2;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ravage {
    pub x: f32,
    pub y: f32,
    pub rings: u8,
    pub lane: f32,
    pub width: f32,
    pub step: u32,
    pub damage: u32,
    pub age: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss_art: Option<bosses::Art>,
    #[serde(default, skip_serializing_if = "zero")]
    pub blow: u32,
}

pub fn step_for(speed: f32) -> u32 {
    (RING_GAP / speed.max(1.0) * HZ as f32).round().max(2.0) as u32
}

impl Ravage {
    pub fn cast(
        (x, y): (f32, f32),
        rings: u32,
        arc: f32,
        speed: f32,
        damage: u32,
        beat: u64,
    ) -> Ravage {
        Ravage {
            x,
            y,
            rings: rings.clamp(1, 8) as u8,
            lane: (mix(beat) % 360) as f32 * PI / 180.0,
            width: arc.to_radians(),
            step: step_for(speed),
            damage,
            age: 0,
            boss_art: None,
            blow: 0,
        }
    }

    pub fn ring(&self, k: u8) -> (f32, u32) {
        (RING_GAP * f32::from(k + 1), WARN + u32::from(k) * self.step)
    }

    pub fn in_lane(&self, x: f32, y: f32) -> bool {
        let heading = (y - self.y).atan2(x - self.x);
        let off = (heading - self.lane + PI).rem_euclid(TAU) - PI;
        off.abs() <= self.width / 2.0
    }

    fn done(&self) -> bool {
        self.age > self.ring(self.rings.saturating_sub(1)).1 + UP
    }
}

impl Run {
    pub(super) fn tick_ravage(&mut self, cast: Vec<Ravage>) {
        for ravage in cast {
            if self.ravages.len() < 3 {
                self.ravages.push(ravage);
            }
        }
        for hero in self.players.values_mut() {
            hero.tossed = hero.tossed.saturating_sub(1);
        }
        let room = &self.dungeon.rooms[self.at];
        let grid = Grid { room, barred: true };
        let (mut burst, mut struck) = (false, false);
        for ravage in &mut self.ravages {
            ravage.age += 1;
            for k in 0..ravage.rings {
                let (radius, at) = ravage.ring(k);
                if ravage.age != at {
                    continue;
                }
                burst = true;
                for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                    let d = (hero.x - ravage.x).hypot(hero.y - ravage.y);
                    if (d - radius).abs() > BAND
                        || ravage.in_lane(hero.x, hero.y)
                        || hero.invulnerable > 0
                    {
                        continue;
                    }
                    if ravage.blow != 0 {
                        if !hero.blow(ravage.damage, ravage.blow) {
                            continue;
                        }
                    } else {
                        hero.hurt(ravage.damage);
                    }
                    hero.tossed = TOSSED;
                    let (ux, uy) = unit(hero.x - ravage.x, hero.y - ravage.y);
                    (hero.x, hero.y) = grid.slide(
                        (hero.x, hero.y),
                        (ux * THROW, uy * THROW),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                    struck = true;
                }
                if let Some(hire) = self.hireling.as_mut().filter(|h| !h.down()) {
                    let d = (hire.x - ravage.x).hypot(hire.y - ravage.y);
                    if (d - radius).abs() <= BAND
                        && !ravage.in_lane(hire.x, hire.y)
                        && hire.invulnerable == 0
                    {
                        hire.hurt(ravage.damage);
                        let (ux, uy) = unit(hire.x - ravage.x, hire.y - ravage.y);
                        (hire.x, hire.y) = grid.slide(
                            (hire.x, hire.y),
                            (ux * THROW, uy * THROW),
                            HERO_RADIUS,
                            Mover::Hero,
                        );
                    }
                }
            }
        }
        if burst {
            self.sounds.push("rock_land");
            self.fx.push(Fx::Shake(5));
        }
        if struck {
            self.room_hurt = true;
            self.cues.push("ravaged".into());
        }
        self.ravages.retain(|r| !r.done());
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_tide__tests.rs"]
mod tests;
