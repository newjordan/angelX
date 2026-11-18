//! The Deep Stirs. Smoke, noise and digging raise a stir; a castle that stands
//! calms the land around it. Past a threshold the Delve answers: raids on
//! quarry claims, things climbing the shaft by night. This module only
//! computes the number, from what the works are doing this moment. The game
//! reads it; nothing in the sim reads it back.
use crate::*;

pub const UNEASY: u32 = 25;
pub const STIRRING: u32 = 60;
pub const RAIDING: u32 = 100;
pub const CALM_MAX: u32 = 60;

pub fn calm_radius(laid: u32) -> i16 {
    (16 + laid / 150).min(90) as i16
}

pub fn level_of(n: u32) -> Stir {
    match n {
        n if n >= RAIDING => Stir::Raiding,
        n if n >= STIRRING => Stir::Stirring,
        n if n >= UNEASY => Stir::Uneasy,
        _ => Stir::Calm,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stir {
    Calm,
    Uneasy,
    Stirring,
    Raiding,
}
impl Stir {
    pub fn label(self) -> &'static str {
        match self {
            Self::Calm => "The Deep sleeps",
            Self::Uneasy => "The Deep is uneasy",
            Self::Stirring => "The Deep stirs",
            Self::Raiding => "The Deep answers",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pressure {
    pub smoke: u32,
    pub noise: u32,
    pub digging: u32,
    pub calm: u32,
    pub stir: u32,
    pub level: Stir,
}

impl Kingdom {
    pub fn pressure(&self, c: &Catalog) -> Pressure {
        self.pressure_with(c, 0, None)
    }

    pub fn pressure_with(&self, c: &Catalog, deep: u32, at: Option<Tile>) -> Pressure {
        let report = self.report(c);
        let (mut smoke, mut noise, mut digging) = (0u32, 0u32, 0u32);
        for (id, b) in &self.buildings {
            if report.status.get(id) != Some(&Status::Working) {
                continue;
            }
            let p = &c.plans[&b.plan];
            smoke += u32::from(p.smoke);
            noise += u32::from(p.noise);
            if p.extractor {
                digging += 3;
            }
        }
        digging += self.shaft_moved / 2 + deep;
        let calm = match at {
            Some(tile) => self.calm_at(c, tile),
            None => {
                let mut calm = 0u32;
                for (&id, s) in &self.sites {
                    calm += s.laid / 120;
                    if self.site_finished(c, id) {
                        calm += 10;
                    }
                }
                calm.min(CALM_MAX)
            }
        };
        let stir = (smoke + noise + digging).saturating_sub(calm);
        Pressure { smoke, noise, digging, calm, stir, level: level_of(stir) }
    }
}
