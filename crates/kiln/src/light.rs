//! The one light and the finishes: how a surface's facing becomes a tone
//! on its material's ramp.
//!
//! A surface's lit value is how squarely it faces the light (the normal
//! dotted with the direction toward the light, -1 to 1). A finish turns that
//! into a place on the ramp (0 the darkest ink, 1 the lightest), and the
//! ramp's bands do the rest: no blending, no dithering, so every tone is
//! one of the realm's inks and every band an even cluster.

use serde::Deserialize;

use crate::num;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Finish {
    Metal,
    Cloth,
    Skin,
    Matte,
    Glow,
}

impl Finish {
    pub fn name(self) -> &'static str {
        match self {
            Finish::Metal => "metal",
            Finish::Cloth => "cloth",
            Finish::Skin => "skin",
            Finish::Matte => "matte",
            Finish::Glow => "glow",
        }
    }
}

pub fn lit(n: [f32; 3], l: [f32; 3]) -> f32 {
    n[0] * l[0] + n[1] * l[1] + n[2] * l[2]
}

pub fn tone(finish: Finish, n: [f32; 3], l: [f32; 3]) -> f32 {
    let d = lit(n, l);
    match finish {
        Finish::Metal => {
            if d > 0.92 {
                return 0.999;
            }
            let mut v = if d <= 0.0 { 0.16 } else { 0.28 + 0.64 * num::pow(d, 0.95) };
            if n[1] > 0.55 && n[0] > -0.2 && d < 0.35 {
                v += 0.14;
            }
            v.min(0.92)
        }
        Finish::Cloth => {
            if d < 0.0 {
                0.12
            } else if d < 0.28 {
                0.5
            } else {
                0.88
            }
        }
        Finish::Skin => {
            let d = d.max(0.0);
            0.22 + 0.74 * d * (2.0 - d)
        }
        Finish::Matte => 0.1 + 0.85 * d.max(0.0),
        Finish::Glow => 0.4 + 0.6 * n[2].max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: [f32; 3] = [-0.434, -0.711, 0.553];

    #[test]
    fn metal_keeps_its_lightest_for_the_glint() {
        assert_eq!(tone(Finish::Metal, L, L), 0.999);
        let side = tone(Finish::Metal, [0.0, 0.0, 1.0], L);
        assert!(side > 0.3 && side < 0.92, "{side}");
        assert_eq!(tone(Finish::Metal, [1.0, 0.0, 0.0], L), 0.16);
        assert!(tone(Finish::Metal, [0.0, 0.8, 0.6], L) > tone(Finish::Metal, [-0.6, 0.8, 0.0], L));
    }

    #[test]
    fn cloth_is_three_tones() {
        let mut seen: Vec<u32> = (0..64)
            .map(|i| {
                let a = i as f32 / 64.0 * std::f32::consts::TAU;
                let (s, c) = num::sin_cos(a);
                (tone(Finish::Cloth, [c * 0.8, s * 0.8, 0.6], L) * 100.0) as u32
            })
            .collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 3);
    }
}
