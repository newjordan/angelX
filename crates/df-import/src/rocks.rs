//! What a natural wall is made of, in the level files' ROCK classes.
//!
//! `s` soil, `r` common stone, `f` flux, `i` iron, `c` copper, `t` tin,
//! `g` gold, `v` silver, `P` platinum, `n` nickel/zinc/lead/bismuth/aluminum/
//! cobalt, `k` coal, `j` gem, `J` precious gem, `x` reagent/industrial,
//! `a` adamantine/feature, `o` obsidian/lava stone.

use crate::input::Prospect;
use std::collections::BTreeSet;

pub const CLASSES: &[(u8, &str)] = &[
    (b's', "soil"),
    (b'r', "stone"),
    (b'f', "flux"),
    (b'i', "iron"),
    (b'c', "copper"),
    (b't', "tin"),
    (b'g', "gold"),
    (b'v', "silver"),
    (b'P', "platinum"),
    (b'n', "base_metal"),
    (b'k', "coal"),
    (b'j', "gem"),
    (b'J', "precious_gem"),
    (b'x', "reagent"),
    (b'a', "adamantine"),
    (b'o', "obsidian"),
];

pub fn class_word(c: u8) -> &'static str {
    CLASSES.iter().find(|(k, _)| *k == c).map(|(_, w)| *w).unwrap_or("unknown")
}

const FLUX: &[&str] = &["LIMESTONE", "DOLOMITE", "CHALK", "MARBLE", "CALCITE"];
const IRON: &[&str] = &["HEMATITE", "MAGNETITE", "LIMONITE"];
const COPPER: &[&str] = &["NATIVE_COPPER", "MALACHITE", "TETRAHEDRITE"];
const BASE: &[&str] = &[
    "GARNIERITE",
    "SPHALERITE",
    "GALENA",
    "BISMUTHINITE",
    "NATIVE_ALUMINUM",
    "COBALTITE",
];
const COAL: &[&str] = &["LIGNITE", "COAL_BITUMINOUS"];
const REAGENT: &[&str] = &[
    "GYPSUM",
    "SALTPETER",
    "BORAX",
    "BRIMSTONE",
    "CINNABAR",
    "TALC",
    "KAOLINITE",
    "ALABASTER",
    "SELENITE",
    "SATINSPAR",
    "MICROCLINE",
    "ORTHOCLASE",
    "PERICLASE",
    "ILMENITE",
    "RUTILE",
    "PITCHBLENDE",
    "BAUXITE",
    "CRYOLITE",
    "SYLVITE",
    "ALUNITE",
    "CHROMITE",
    "PERIDOTITE",
    "OLIVINE",
    "MARCASITE",
    "PUDDINGSTONE",
];
/// Gems in case the prospect report is missing; the report's own Gems
/// section is used when there is one.
const GEMS: &[&str] = &[
    "BLOODSTONE",
    "AVENTURINE",
    "TURQUOISE",
    "MORION",
    "CITRINE",
    "CHRYSOPRASE",
    "SCHORL",
    "AMETHYST",
    "TIGEREYE",
    "SARD",
    "ONYX",
    "PRASE",
    "CARNELIAN",
    "SARDONYX",
    "JET",
    "PETRIFIED_WOOD",
    "CRYSTAL_ROCK",
    "QUARTZ_ROSE",
    "PYRITE",
    "LAPIS_LAZULI",
    "CHRYSOCOLLA",
    "MOONSTONE",
];

fn precious(t: &str) -> bool {
    t.starts_with("DIAMOND")
        || t.contains("OPAL")
        || matches!(t, "EMERALD" | "RUBY" | "SAPPHIRE" | "RUBY_STAR" | "SAPPHIRE_STAR")
        || t.starts_with("STAR ")
}

pub struct Classifier {
    gems: BTreeSet<String>,
    ores: BTreeSet<String>,
}

impl Classifier {
    pub fn new(p: &Prospect) -> Self {
        let set = |k: &str| -> BTreeSet<String> {
            p.get(k).map(|v| v.iter().map(|(n, _)| n.clone()).collect()).unwrap_or_default()
        };
        Classifier { gems: set("Gems"), ores: set("Ores") }
    }

    /// The class of an `INORGANIC:<TOKEN>` (or bare token).
    pub fn classify(&self, token: &str) -> u8 {
        let t = token.strip_prefix("INORGANIC:").unwrap_or(token);
        let is = |l: &[&str]| l.contains(&t);
        if t == "RAW_ADAMANTINE" {
            return b'a';
        }
        if t == "OBSIDIAN" {
            return b'o';
        }
        if is(IRON) {
            return b'i';
        }
        if is(COPPER) {
            return b'c';
        }
        if t == "CASSITERITE" {
            return b't';
        }
        if t == "NATIVE_GOLD" {
            return b'g';
        }
        if t == "NATIVE_SILVER" || t == "HORN_SILVER" {
            return b'v';
        }
        if t == "NATIVE_PLATINUM" {
            return b'P';
        }
        if is(BASE) {
            return b'n';
        }
        if is(COAL) {
            return b'k';
        }
        if is(FLUX) {
            return b'f';
        }
        if self.gems.contains(t) || is(GEMS) || (self.gems.is_empty() && t.contains(' ')) {
            return if precious(t) { b'J' } else { b'j' };
        }
        if precious(t) {
            return b'J';
        }
        if is(REAGENT) {
            return b'x';
        }
        if self.ores.contains(t) {
            return b'n';
        }
        b'r'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes() {
        let c = Classifier::new(&Prospect::new());
        assert_eq!(c.classify("INORGANIC:HEMATITE"), b'i');
        assert_eq!(c.classify("INORGANIC:DIAMOND_RED"), b'J');
        assert_eq!(c.classify("INORGANIC:GREEN JADE"), b'j');
        assert_eq!(c.classify("INORGANIC:WAX OPAL"), b'J');
        assert_eq!(c.classify("INORGANIC:SHALE"), b'r');
        assert_eq!(c.classify("INORGANIC:CHALK"), b'f');
        assert_eq!(c.classify("INORGANIC:KAOLINITE"), b'x');
        assert_eq!(c.classify("INORGANIC:LIGNITE"), b'k');
    }
}
