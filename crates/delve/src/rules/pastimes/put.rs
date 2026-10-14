//! California-Games-style throwing, but the kingdom's athletes use clean pots.
//! Hold/release power, an adjustable launch angle, gravity and signed wind.
use super::*;

pub const TARGETS: [i32; 3] = [12, 16, 20];
pub const MAX_CHARGE: u32 = HZ;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pot {
    pub x: i32,
    pub z: i32,
    pub dx: i32,
    pub dz: i32,
    pub left: u32,
}
impl Pot {
    fn valid(&self) -> bool {
        (256..=22 * 256).contains(&self.x)
            && (1..=3000).contains(&self.z)
            && (1..=260).contains(&self.dx)
            && self.dz.unsigned_abs() <= 240
            && self.left > 0
            && self.left <= 3 * HZ
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Put {
    pub owner: u32,
    pub angle: u8,
    pub charge: u32,
    pub shots: u8,
    pub wait: u32,
    pub flight: Option<Pot>,
}
impl Put {
    pub fn new(owner: u32) -> Self {
        Self {
            owner,
            angle: 45,
            charge: 0,
            shots: 0,
            wait: 0,
            flight: None,
        }
    }
    pub fn wind(progress: u8) -> i32 {
        [-1, 1, -2][usize::from(progress.min(2))]
    }
    pub fn valid(&self, round: &Round) -> bool {
        self.owner > 0
            && (20..=70).contains(&self.angle)
            && self.charge <= MAX_CHARGE
            && self.shots <= 8
            && self.wait <= HZ / 2
            && self.flight.as_ref().is_none_or(Pot::valid)
            && (self.flight.is_none() || (self.charge == 0 && self.wait == 0))
            && (self.wait == 0 || self.charge == 0)
            && u16::from(round.progress)
                + u16::from(round.mistakes)
                + u16::from(self.flight.is_some())
                == u16::from(self.shots)
            && (self.shots < 8 || self.flight.is_some())
    }
}
impl Run {
    pub(super) fn tick_put(
        &mut self,
        round: &mut Round,
        inputs: &BTreeMap<u32, Input>,
        eligible: &[(u32, f32, f32)],
    ) -> bool {
        let p = round.put.as_mut().unwrap();
        let Some(&(_, x, y)) = eligible.iter().find(|&&(id, _, _)| id == p.owner) else {
            self.found = Some((
                self.tick,
                0,
                "The thrower left. Pots parked.".into(),
            ));
            return false;
        };
        let input = inputs
            .get(&p.owner)
            .filter(|i| i.valid())
            .copied()
            .unwrap_or_default();
        if let Some(mut pot) = p.flight.take() {
            pot.x += pot.dx;
            pot.z += pot.dz;
            pot.dx = (pot.dx + Put::wind(round.progress)).clamp(1, 260);
            pot.dz -= 12;
            pot.left = pot.left.saturating_sub(1);
            // A structurally valid hand-edited checkpoint can start at a bound
            // no real launch reaches. Keep acceptance closed under one tick:
            // terminate a rising/out-of-velocity-bound flight as a miss rather
            // than emitting an invalid live state. Ordinary arcs are unchanged.
            let outside =
                !(256..=22 * 256).contains(&pot.x) || pot.left == 0 || (pot.z > 0 && !pot.valid());
            if pot.z <= 0 || outside {
                // Only a landing scores, not flying through a target or walking.
                let target = TARGETS[usize::from(round.progress)] * 256;
                if !outside && pot.x.abs_diff(target) <= 256 {
                    round.progress += 1;
                    self.sounds.push("spoil_pickup");
                    self.found = Some((
                        self.tick,
                        p.owner,
                        "A perfect pot!".into(),
                    ));
                } else {
                    round.mistakes += 1;
                    self.sounds.push("clang");
                    self.found = Some((
                        self.tick,
                        p.owner,
                        "Miss! An archaeological pot. -25".into(),
                    ));
                }
                p.wait = HZ / 2;
            } else {
                p.flight = Some(pot);
            }
        } else if p.wait > 0 {
            p.wait -= 1;
        } else if (x - 4.0 * TILE_UNITS).hypot(y - 7.0 * TILE_UNITS) >= 2.2 {
            p.charge = 0;
        } else {
            let tilt = if input.aim_y != 0 {
                input.aim_y
            } else {
                input.move_y
            };
            p.angle = (i32::from(p.angle) - i32::from(tilt)).clamp(20, 70) as u8;
            if input.fire {
                p.charge = (p.charge + 1).min(MAX_CHARGE);
            } else if p.charge > 0 && p.shots < 8 {
                let angle = f32::from(p.angle).to_radians();
                let speed = 80.0 + p.charge as f32 * 5.0;
                p.flight = Some(Pot {
                    x: 4 * 256,
                    z: 1,
                    dx: (speed * angle.cos()).round() as i32,
                    dz: (speed * angle.sin()).round() as i32,
                    left: 3 * HZ,
                });
                p.charge = 0;
                p.shots += 1;
                self.sounds.push("play_card");
            }
        }
        if p.shots == 8 && p.flight.is_none() && round.progress < Game::Pot.need() {
            self.found = Some((
                self.tick,
                p.owner,
                "Eight pots lost! The sea keeps them."
                    .into(),
            ));
            return false;
        }
        true
    }
}
