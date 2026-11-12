//! Geology: what each part of the Delve gives. The hillside above has field
//! stone and rubble only; every stone worth building a castle from is won
//! below, seam by seam, and the rock a party wins sets what its castle looks
//! like (docs/KINGDOM-WORKS.md, "Geology").
//!
//! A floor's seam is its pack's quarry stone and what comes up with it. The
//! older ores stay where they were (iron on the Mines' odd floors, moon
//! crystal on the even, coal in the Crypt, ember in Dragon Keep, crystal
//! deeper), so a land charter that asks for one is still won where it always
//! was, now as a by-product of the quarry. Deeper rock is purer and cut
//! faster: one fixed rate per depth, not a formula.
use super::ore_threats::OreKind;
use super::*;
use std::collections::BTreeMap;

/// How a seam's rock face reads on the floor: one picture each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Limestone,
    Flint,
    Basalt,
    Slate,
    Tufa,
    Clay,
    Void,
}

/// How pure the rock is. Rough rock on the top floors, prime at the bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
    Rough,
    Sound,
    Prime,
}
impl Grade {
    pub fn word(self) -> &'static str {
        match self {
            Grade::Rough => "Rough",
            Grade::Sound => "Sound",
            Grade::Prime => "Prime",
        }
    }
}

/// Stone cut a shift by a mine on the seam, floor 1 down to 6.
const RATE: [u32; 6] = [2, 2, 3, 3, 4, 4];
/// The rock's purity on each of those floors.
const GRADE: [Grade; 6] = [
    Grade::Rough,
    Grade::Rough,
    Grade::Sound,
    Grade::Sound,
    Grade::Prime,
    Grade::Prime,
];
/// The shallowest floor lead is found, in the Mines' limestone.
const GALENA_FROM: u32 = 3;

/// One pack's rock.
pub struct Row {
    pub pack: Pack,
    pub face: Face,
    /// The quarry stone's catalogue id.
    pub stone: &'static str,
    /// What the seam is called, after its grade: `Sound` + this.
    pub noun: &'static str,
    /// What a castle of it looks like.
    pub builds: &'static str,
    /// Where the Delve has it, for the field guide.
    pub whence: &'static str,
    /// What it is good for, in a few words, for the plate at the face.
    pub good: &'static str,
    /// What the stone is good for, in a line for the field guide.
    pub uses: &'static str,
    /// The sentries and guardian that hold it.
    pub guard: OreKind,
    /// By-products, besides the Mines' ore (`Seam::yields`).
    pub others: &'static [&'static str],
}

pub static ROWS: [Row; 7] = [
    Row {
        pack: Pack::Cavern,
        face: Face::Limestone,
        stone: "limestone",
        noun: "limestone face",
        builds: "pale ashlar",
        whence: "THE MINES: FLOOR 1 OR 2",
        good: "ASHLAR AND LIME",
        uses: "Pale ashlar for walls. Burns to lime for mortar.",
        guard: OreKind::Iron,
        others: &[],
    },
    Row {
        pack: Pack::Crypt,
        face: Face::Flint,
        stone: "flint",
        noun: "flint bank",
        builds: "flint banding",
        whence: "THE CRYPT: FLOOR 1 OR 2",
        good: "BANDED WALLS, COAL",
        uses: "Flint and old dressed stone for banded walls and footings.",
        guard: OreKind::Coal,
        others: &["coal", "bone"],
    },
    Row {
        pack: Pack::Hellforge,
        face: Face::Basalt,
        stone: "basalt",
        noun: "basalt column",
        builds: "a dark keep",
        whence: "DRAGON KEEP: FLOOR 3",
        good: "A DARK KEEP, EMBER",
        uses: "Dark, hard stone for a keep. Hungry on tools.",
        guard: OreKind::Ember,
        others: &["ember", "sulphur"],
    },
    Row {
        pack: Pack::Archive,
        face: Face::Slate,
        // The quarry stone is the fine freestone; the slate comes up with it as
        // `slate-block` (the production game's slater splits it for roofs).
        stone: "freestone",
        noun: "slate seam",
        builds: "slate roofs and chevron work",
        whence: "THE DROWNED ARCHIVE: FLOOR 4",
        good: "ROOFS AND CARVING",
        uses: "Slate for roofs. Fine freestone for carving chevron and voussoirs.",
        guard: OreKind::Crystal,
        others: &["slate-block", "crystal"],
    },
    Row {
        pack: Pack::Fungal,
        face: Face::Tufa,
        stone: "tufa",
        noun: "tufa bed",
        builds: "light vaulting stone",
        whence: "THE FUNGAL DEEP: FLOOR 5",
        good: "VAULTS, SALTPETRE",
        uses: "Soft, light stone for vaults. Saltpetre from the same bed.",
        guard: OreKind::Crystal,
        others: &["saltpetre", "crystal"],
    },
    Row {
        pack: Pack::Swamp,
        face: Face::Clay,
        stone: "clay",
        noun: "clay and peat bank",
        builds: "brick and thatch",
        whence: "THE SWAMP OF HORRORS",
        good: "BRICK, PEAT, THATCH",
        uses: "Clay for bricks, peat for the kiln, reeds for the roof.",
        guard: OreKind::Coal,
        others: &["peat", "bog-iron", "reeds"],
    },
    Row {
        pack: Pack::Unknown,
        face: Face::Void,
        stone: "voidstone",
        noun: "voidstone vein",
        builds: "wished-for walls",
        whence: "THE UNKNOWN: FLOOR 6",
        good: "WISHED-FOR WALLS",
        uses: "Stone that is not quite there. Late walls, wished for.",
        guard: OreKind::Crystal,
        others: &["crystal"],
    },
];

/// The row for a pack.
pub fn row(pack: Pack) -> &'static Row {
    ROWS.iter().find(|r| r.pack == pack).unwrap_or(&ROWS[1])
}

/// The floor's seam: what lies in the rock of `pack` at `depth`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seam {
    pub pack: Pack,
    pub depth: u32,
}

pub fn seam_at(pack: Pack, depth: u32) -> Seam {
    Seam { pack, depth }
}

impl Seam {
    fn row(self) -> &'static Row {
        row(self.pack)
    }
    fn index(self) -> usize {
        self.depth.clamp(1, DEEPEST) as usize - 1
    }
    pub fn stone(self) -> &'static str {
        self.row().stone
    }
    pub fn face(self) -> Face {
        self.row().face
    }
    pub fn grade(self) -> Grade {
        GRADE[self.index()]
    }
    /// `Sound limestone face`: what the claim is called.
    pub fn name(self) -> String {
        format!("{} {}", self.grade().word(), self.row().noun)
    }
    pub fn good(self) -> &'static str {
        self.row().good
    }
    pub fn builds(self) -> &'static str {
        self.row().builds
    }
    pub fn uses(self) -> &'static str {
        self.row().uses
    }
    /// The sentries that hold this floor's seam. The Mines' even floors are
    /// the moon crystal's, as they were: prism stalkers and the wyrm.
    pub fn guard(self) -> OreKind {
        if self.row().pack == Pack::Cavern && self.depth.is_multiple_of(2) {
            OreKind::Crystal
        } else {
            self.row().guard
        }
    }
    /// Stone a mine on this seam cuts each shift.
    pub fn rate(self) -> u32 {
        RATE[self.index()]
    }
    /// The by-products, in the order they are told, with their rates.
    pub fn others(self) -> Vec<(&'static str, u32)> {
        let prime = self.grade() == Grade::Prime;
        let mut found: Vec<&'static str> = Vec::new();
        if self.row().pack == Pack::Cavern {
            // The Mines keep their old alternation, so iron and moon crystal
            // charters are still won where they were.
            found.push(if self.depth.is_multiple_of(2) {
                "crystal"
            } else {
                "iron-ore"
            });
            if self.depth >= GALENA_FROM {
                found.push("galena");
            }
        }
        found.extend(self.row().others);
        found
            .into_iter()
            .enumerate()
            // The first by-product is the ore the floor always gave, at the
            // rate it always had (2), and a third faster again in prime rock.
            .map(|(i, r)| (r, if i == 0 { if prime { 3 } else { 2 } } else { 1 }))
            .collect()
    }
    /// Everything the seam gives, resource to rate a shift.
    pub fn yields(self) -> BTreeMap<String, u32> {
        let mut out = BTreeMap::from([(self.stone().to_string(), self.rate())]);
        for (r, n) in self.others() {
            out.insert(r.to_string(), n);
        }
        out
    }
    pub fn gives(self, resource: &str) -> bool {
        self.stone() == resource || self.others().iter().any(|(r, _)| *r == resource)
    }
    /// `Pale ashlar for walls...`, then what else comes up, for the words on
    /// screen. Names come from the catalogue.
    pub fn also(self) -> String {
        let c = crate::kingdom::catalog();
        let names: Vec<String> = self
            .others()
            .iter()
            .map(|(r, _)| {
                c.resources
                    .get(*r)
                    .map_or_else(|| (*r).to_string(), |n| n.name.to_lowercase())
            })
            .collect();
        if names.is_empty() {
            String::new()
        } else {
            format!("Also {}.", names.join(", "))
        }
    }
    /// Where the seam lies: `The Mines / floor 3`.
    pub fn source(self) -> String {
        format!("{} / floor {}", self.pack.name(), self.depth)
    }
}

/// The sentries and guardian that may hold a seam, from the resource of its
/// claim: a quarry stone (the Mines' limestone has two sets, by floor) or,
/// on a claim from before geology, an ore.
pub fn guards_for(resource: &str) -> Vec<OreKind> {
    if let Some(kind) = OreKind::legacy(resource) {
        return vec![kind];
    }
    match ROWS.iter().find(|r| r.stone == resource) {
        Some(r) if r.pack == Pack::Cavern => vec![OreKind::Iron, OreKind::Crystal],
        Some(r) => vec![r.guard],
        None => Vec::new(),
    }
}

/// The first of them, for a resource asked of alone (a land charter's).
pub fn guard_for(resource: &str) -> Option<OreKind> {
    guards_for(resource).first().copied()
}

/// Whether a claim's stone is a quarry stone, not a legacy ore.
pub fn is_stone(resource: &str) -> bool {
    ROWS.iter().any(|r| r.stone == resource)
}

/// What the Keep's hillside outcrop gives: field stone and rubble, the poor
/// early source before any seam is won. It is no quarry stone (no face, no
/// guards), but it stands in the stockyard with them.
pub const RUBBLE: &str = "rubble";

/// A quarry stone as the words on screen name it: the catalogue calls it
/// "Flint block" (it is the good a cart carries), and a sentence about a block
/// of it, or the next floor's stone, wants "Flint".
pub fn rock_name(stone: &str) -> String {
    let name = crate::kingdom::catalog().resources.get(stone).map_or(stone, |r| r.name.as_str());
    name.strip_suffix(" block").unwrap_or(name).to_string()
}

/// Whether `resource` is rough stone the stockyard keeps: a quarry stone or
/// the hillside's rubble.
pub fn is_rough(resource: &str) -> bool {
    is_stone(resource) || resource == RUBBLE
}

/// The face a claim's picture is drawn with: a quarry stone's own, or for a
/// legacy ore claim the pack's stone it was found in.
pub fn face_for(resource: &str) -> Option<Face> {
    ROWS.iter().find(|r| r.stone == resource).map(|r| r.face)
}

#[cfg(test)]
#[path = "../../tests/rules/geology__tests.rs"]
mod tests;
