//! The ramps an effect burns down, by name: the cards' palettes
//! (`crates/delve/src/rules/vfx.rs`, drawn in the game by `pfx::ramps`)
//! with the same names and the same inks, and a few more for the blades
//! and the soot. Kiln's ramps run dark to light, so each is the game's
//! read backwards: its last ink is the hot core.

use crate::palette::{Palette, Ramp};

/// Every named ramp, its inks dark to light.
pub const NAMED: &[(&str, &str)] = &[
    ("fire", "BpR@69"),
    ("ember", "bpRo@6"),
    ("frost", "qQzVWw"),
    ("holy", "oOtT69"),
    ("gold", "rO4569"),
    ("arcane", "()[]^w"),
    ("genie", "0q123w"),
    ("storm", "q1z3Ww"),
    ("poison", "FlAyCY"),
    ("nature", "ElLMyY"),
    ("blood", "nBp77c"),
    ("rose", "!?;%+9"),
    ("poop", "bBrPot"),
    ("brew", "d()[]^"),
    ("void", "Kd()[^"),
    ("water", "sSqQzw"),
    ("bone", "jGJhiH"),
    ("smoke", "gjGJhi"),
    ("earth", "bIPoOT"),
    // A blade's steel, from its shadow to the white of its edge.
    ("steel", "xSuUvVW"),
    // A hit's sparks: a forge's colours up to a white heat.
    ("spark", "pRo@569"),
    // Soot and the dark of a crack.
    ("char", "kKgjG"),
];

/// A ramp by its name.
pub fn named(name: &str) -> Option<Ramp> {
    NAMED.iter().find(|(n, _)| *n == name).and_then(|(_, keys)| Ramp::parse(keys, Palette::realm()).ok())
}

/// Every name, space between.
pub fn names() -> String {
    NAMED.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" ")
}

/// The named ramp nearest a run of colours from dark to light: each ramp
/// and the colours sampled at the same five points along them, the sum of
/// the squared distances least.
pub fn nearest(colours: &[[u8; 3]]) -> &'static str {
    if colours.is_empty() {
        return "arcane";
    }
    let p = Palette::realm();
    let sample = |c: &[[u8; 3]], t: f32| c[((t * (c.len() - 1) as f32).round() as usize).min(c.len() - 1)];
    let mut best = ("arcane", f32::MAX);
    for (name, keys) in NAMED {
        let inks: Vec<[u8; 3]> = keys.chars().filter_map(|k| p.ink(k).and_then(|i| p.rgb(i))).collect();
        let mut d = 0.0;
        for s in 0..5 {
            let t = s as f32 / 4.0;
            let (a, b) = (sample(colours, t), sample(&inks, t));
            // Weighted by how the eye reads each channel.
            let w = [0.3, 0.59, 0.11];
            d += (0..3).map(|i| w[i] * (f32::from(a[i]) - f32::from(b[i])).powi(2)).sum::<f32>();
        }
        if d < best.1 {
            best = (name, d);
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_ramp_is_the_realms_inks() {
        for (name, _) in NAMED {
            let r = named(name).unwrap_or_else(|| panic!("{name} reads"));
            assert!(r.0.len() >= 4, "{name}");
        }
        assert_eq!(named("fire").unwrap().keys(Palette::realm()), "BpR@69");
        assert!(named("plaid").is_none());
    }

    #[test]
    fn a_run_of_colours_finds_its_ramp() {
        // A fire's: dark red to cream.
        let fire = [[0x43, 0x17, 0x2b], [0xa1, 0x32, 0x35], [0xe8, 0x6a, 0x2d], [0xff, 0xc4, 0x5c], [0xff, 0xf1, 0xb0]];
        assert!(["fire", "ember", "spark"].contains(&nearest(&fire)), "{}", nearest(&fire));
        let ice = [[0x18, 0x2e, 0x61], [0x28, 0x6c, 0x9a], [0x52, 0xbf, 0xd0], [0xa7, 0xea, 0xf4], [0xf0, 0xff, 0xff]];
        assert!(["frost", "water", "genie", "storm"].contains(&nearest(&ice)), "{}", nearest(&ice));
    }
}
