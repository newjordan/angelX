//! The realm's inks, the only colours kiln draws in, and ramps of them.
//!
//! `assets/realm.hex` is the game's palette as the painter reads it: one ink
//! a line, its colour, its key (the character the game's ink rows use) and
//! its family. An [`Ink`] is its place in that list plus one; ink 0 is clear.

use std::sync::OnceLock;

/// One of the realm's inks, or clear (0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Ink(pub u8);

impl Ink {
    /// No ink: the pixel is clear.
    pub const CLEAR: Ink = Ink(0);

    pub fn is_clear(self) -> bool {
        self.0 == 0
    }
}

/// A palette: inks by key, colour and family.
#[derive(Clone, Debug)]
pub struct Palette {
    keys: Vec<char>,
    rgb: Vec<[u8; 3]>,
    family: Vec<String>,
}

impl Palette {
    /// The realm's own, compiled in.
    pub fn realm() -> &'static Palette {
        static REALM: OnceLock<Palette> = OnceLock::new();
        REALM.get_or_init(|| {
            Palette::parse(include_str!("../assets/realm.hex")).expect("the realm's palette reads")
        })
    }

    /// A palette from `.hex` text: `rrggbb key family` a line; blank lines
    /// and lines starting with `#` are skipped.
    pub fn parse(text: &str) -> Result<Palette, String> {
        let (mut keys, mut rgb, mut family) = (Vec::new(), Vec::new(), Vec::new());
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut words = line.split_whitespace();
            let hex = words.next().unwrap_or_default();
            let key = words.next().unwrap_or_default();
            let mut chars = key.chars();
            let (Some(k), None) = (chars.next(), chars.next()) else {
                return Err(format!("line {}: a key is one character, not {key:?}", n + 1));
            };
            if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("line {}: {hex:?} is not rrggbb", n + 1));
            }
            if k == '.' || keys.contains(&k) {
                return Err(format!("line {}: key {k:?} is taken", n + 1));
            }
            let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
            keys.push(k);
            rgb.push([c(0), c(2), c(4)]);
            family.push(words.collect::<Vec<_>>().join(" "));
        }
        if keys.len() > 254 {
            return Err(format!("{} inks: at most 254", keys.len()));
        }
        Ok(Palette { keys, rgb, family })
    }

    /// How many inks it has, clear not counted.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The ink a key names; `.` is clear.
    pub fn ink(&self, key: char) -> Option<Ink> {
        if key == '.' {
            return Some(Ink::CLEAR);
        }
        self.keys.iter().position(|&k| k == key).map(|i| Ink(i as u8 + 1))
    }

    /// An ink's key; clear is `.`.
    pub fn key(&self, ink: Ink) -> char {
        if ink.is_clear() { '.' } else { self.keys.get(ink.0 as usize - 1).copied().unwrap_or('?') }
    }

    /// An ink's colour; clear has none.
    pub fn rgb(&self, ink: Ink) -> Option<[u8; 3]> {
        if ink.is_clear() { None } else { self.rgb.get(ink.0 as usize - 1).copied() }
    }

    /// An ink's family (structure, foliage, stone, violet, ...).
    pub fn family(&self, ink: Ink) -> &str {
        if ink.is_clear() { "" } else { self.family.get(ink.0 as usize - 1).map_or("", |s| s) }
    }

    /// Every ink, in order.
    pub fn inks(&self) -> impl Iterator<Item = Ink> + '_ {
        (1..=self.keys.len()).map(|i| Ink(i as u8))
    }

    /// An ink's lightness, 0..=1 (Rec. 601 luma).
    pub fn luma(&self, ink: Ink) -> f32 {
        self.rgb(ink).map_or(0.0, |[r, g, b]| (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0)
    }
}

/// Inks from dark to light: a material's tones.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ramp(pub Vec<Ink>);

impl Ramp {
    /// A ramp from its keys, dark to light (`"sxSuUvVW"`).
    pub fn parse(keys: &str, palette: &Palette) -> Result<Ramp, String> {
        let inks = keys
            .chars()
            .map(|k| palette.ink(k).filter(|i| !i.is_clear()).ok_or_else(|| format!("no ink {k:?}")))
            .collect::<Result<Vec<_>, _>>()?;
        if inks.is_empty() {
            return Err("a ramp needs an ink".into());
        }
        Ok(Ramp(inks))
    }

    /// The tone at `v` (0 darkest, 1 lightest), held to the ramp.
    pub fn at(&self, v: f32) -> Ink {
        let n = self.0.len();
        let i = (v.clamp(0.0, 1.0) * n as f32).floor() as usize;
        self.0[i.min(n - 1)]
    }

    /// `ink` moved `steps` along the ramp (negative darker), held to its ends;
    /// an ink not on the ramp is returned as it is.
    pub fn step(&self, ink: Ink, steps: i32) -> Ink {
        match self.0.iter().position(|&i| i == ink) {
            Some(p) => self.0[(p as i32 + steps).clamp(0, self.0.len() as i32 - 1) as usize],
            None => ink,
        }
    }

    pub fn darkest(&self) -> Ink {
        self.0[0]
    }

    pub fn lightest(&self) -> Ink {
        *self.0.last().expect("a ramp has an ink")
    }

    /// Its keys, dark to light.
    pub fn keys(&self, palette: &Palette) -> String {
        self.0.iter().map(|&i| palette.key(i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_realm_has_its_inks_and_its_new_dyes() {
        let p = Palette::realm();
        assert_eq!(p.len(), 74);
        assert_eq!(p.rgb(p.ink('k').unwrap()), Some([0x0b, 0x0a, 0x08]));
        assert_eq!(p.family(p.ink('[').unwrap()), "violet");
        assert_eq!(p.family(p.ink('%').unwrap()), "pink");
        assert_eq!(p.ink('.'), Some(Ink::CLEAR));
        assert_eq!(p.key(p.ink('W').unwrap()), 'W');
    }

    #[test]
    fn a_ramp_runs_dark_to_light_and_holds_its_ends() {
        let p = Palette::realm();
        let steel = Ramp::parse("sxSuUvVW", p).unwrap();
        assert_eq!(steel.at(0.0), p.ink('s').unwrap());
        assert_eq!(steel.at(1.0), p.ink('W').unwrap());
        assert_eq!(steel.step(p.ink('u').unwrap(), -2), p.ink('x').unwrap());
        assert_eq!(steel.step(p.ink('s').unwrap(), -5), p.ink('s').unwrap());
        assert!(Ramp::parse("s#", p).is_err());
        assert_eq!(steel.keys(p), "sxSuUvVW");
    }
}
