//! The Warden: a great lantern-bearer whose lamp-heart is shuttered. A
//! thrown bolt of light, or a flare held on it, opens the shutter for a
//! while (from the second third of its health, two knights' light at once);
//! a parried beam knocks it open too. Then the blade counts. Three phases,
//! every tell at least 40 ticks, every footprint shown for the whole tell.
use super::foes::*;
use super::world::*;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ws {
    Asleep,
    Rest,
    Tell,
    Strike,
    Recover,
    /// Its beam parried: shutter wide open and a double blow waiting.
    Stagger,
    Dying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Atk {
    /// A beam swept across a fan (two, in the last phase).
    Beam,
    /// A ring along the ground: hop it or roll through it.
    Slam,
    /// Snuffers climb out of the fissures.
    Summon,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warden {
    pub x: i32,
    pub y: i32,
    pub hp: u32,
    pub full: u32,
    pub prov: u8,
    /// 0, 1 or 2: thirds of its health.
    pub phase: u8,
    pub st: Ws,
    pub t: u16,
    pub atk: Atk,
    pub n: u8,
    pub danger: Danger,
    /// The beam's middle, and how far it has swept.
    pub aim: i32,
    pub sweep: i32,
    /// The tick each seat last had light on the heart.
    pub lit: [u32; 6],
    pub hit: [u32; 6],
    pub struck: u8,
    pub dead: u16,
    /// Its health when the phase began: where a retry starts it.
    pub start: u32,
    pub face: (i32, i32),
    pub flinch: u8,
}

impl Warden {

    pub fn radius(&self) -> i32 {
        WARDEN_R * Q
    }

    /// Whether every number is one a save or the wire may carry.
    pub fn valid(&self) -> bool {
        self.hp <= self.full
            && self.full <= WARDEN_HP * MAX_PLAYERS
            && (1..=MAX_PLAYERS as u8).contains(&self.prov)
            && self.phase <= 2
            && self.t <= 600
            && (-360..=360).contains(&self.sweep)
            && dist(self.face.0, self.face.1) <= Q + 8
            && self.dead <= 600
            && self.start <= self.full
    }

    /// The lamp-heart's place: in its chest, toward where it faces.
    pub fn heart(&self) -> (i32, i32) {
        (self.x + self.face.0 * 9, self.y + self.face.1 * 9)
    }

    /// How many knights' light the shutter wants.
    pub fn need(&self, seats: u32) -> u32 {
        if self.phase >= 1 { seats.min(2) } else { 1 }
    }

    /// Whether the shutter is open at tick `now`.
    pub fn exposed(&self, now: u32, seats: u32) -> bool {
        if self.st == Ws::Stagger {
            return true;
        }
        let lit = self.lit.iter().filter(|&&t| t > 0 && t + EXPOSE_TICKS > now).count() as u32;
        lit >= self.need(seats)
    }

    /// How long the shutter has left open, 0 if shut.
    pub fn open_for(&self, now: u32, seats: u32) -> u32 {
        if !self.exposed(now, seats) {
            return 0;
        }
        if self.st == Ws::Stagger {
            return u32::from(self.t);
        }
        self.lit
            .iter()
            .filter(|&&t| t > 0)
            .map(|&t| (t + EXPOSE_TICKS).saturating_sub(now))
            .min()
            .unwrap_or(0)
    }
}
