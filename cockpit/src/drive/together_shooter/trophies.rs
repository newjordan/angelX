//! The Trophy Hall, south of the Undercroft down the runner: what the realm
//! has slain, as bronze statuettes on plinths, and the Grail's dais at the
//! far end. Sir Kay, the seneschal, keeps it, and has opinions about every
//! piece in it.
//!
//! A guardian's statuette stands once the party has felled it (gilded
//! after ten), the dragon's in the middle of the hall; the Grail stands on
//! its dais once found. The realm keeps the count (`Home::trophies`); the
//! run only marks what fell.

use super::*;

/// One plinth: whose trophy it holds, its name on the plaque, and where it
/// stands (column, row, width, height in tiles).
pub(crate) struct Plinth {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) at: (i32, i32, i32, i32),
}

pub(crate) const PLINTHS: [Plinth; 9] = [
    Plinth {
        id: "waxen-warden",
        name: "The Waxen Warden",
        at: (4, 3, 2, 1),
    },
    Plinth {
        id: "cinderjaw",
        name: "Cinderjaw",
        at: (4, 8, 2, 1),
    },
    Plinth {
        id: "the-index",
        name: "The Index",
        at: (18, 3, 2, 1),
    },
    Plinth {
        id: "mother-of-spores",
        name: "The Mother of Spores",
        at: (18, 8, 2, 1),
    },
    Plinth {
        id: "the-bone-choir",
        name: "The Bone Choir",
        at: (4, 11, 2, 1),
    },
    Plinth {
        id: "the-foreman",
        name: "The Foreman",
        at: (18, 11, 2, 1),
    },
    Plinth {
        id: "late-fee-leviathan",
        name: "The Late-Fee Leviathan",
        at: (18, 6, 2, 1),
    },
    Plinth {
        id: "dragon",
        name: "The Dragon",
        at: (10, 5, 4, 2),
    },
    Plinth {
        id: "grail",
        name: "The Grail",
        at: (11, 10, 2, 1),
    },
];

/// Felled this many times, a statuette is gilded.
pub(crate) const GILDED: u32 = 10;
/// Where Sir Kay stands, by the door (tiles).
pub(crate) const KAY_AT: (f32, f32) = (8.6, 2.6);

/// The plinth a knight standing at `(x, y)` (arena units) is reading: the
/// one whose front they stand at.
pub(crate) fn plinth_at(x: f32, y: f32) -> Option<&'static Plinth> {
    PLINTHS.iter().find(|p| {
        let (col, row, w, h) = p.at;
        let (cx, front) = (
            (col as f32 + w as f32 / 2.0) * TILE_UNITS,
            (row + h) as f32 * TILE_UNITS,
        );
        (x - cx).abs() < w as f32 + 0.6 && y > front - 0.5 && y < front + 3.0
    })
}

/// The mark a moment leaves on the realm's trophies, if any.
fn trophy(cue: &str) -> Option<&str> {
    match cue {
        "victory" => Some("dragon"),
        "grail" => Some("grail"),
        _ => cue.strip_prefix("boss_fall:"),
    }
}

impl Run {
    /// Count what fell this tick for the Trophy Hall: the cues since
    /// `heard`.
    pub(super) fn mark_trophies(&mut self, heard: usize) {
        let marks: Vec<String> = self
            .cues
            .get(heard..)
            .unwrap_or_default()
            .iter()
            .filter_map(|cue| trophy(cue).map(|id| format!("trophy:{id}")))
            .collect();
        for mark in marks {
            *self.marks.entry(mark).or_default() += 1;
        }
    }

    /// The hall each tick: Sir Kay greets whoever walks up.
    pub(super) fn tick_trophies(&mut self) {
        const KAY_BIT: u8 = 1 << 5;
        let (kx, ky) = (KAY_AT.0 * TILE_UNITS, KAY_AT.1 * TILE_UNITS);
        let near = self
            .players
            .values()
            .any(|h| h.hp > 0 && (h.x - kx).hypot(h.y - ky) < 5.0);
        if near && self.greeted & KAY_BIT == 0 {
            let said = if self.home.trophies.is_empty() {
                "npc:kay_empty"
            } else {
                "npc:kay"
            };
            self.cues.push(said.into());
        }
        self.greeted = if near {
            self.greeted | KAY_BIT
        } else {
            self.greeted & !KAY_BIT
        };
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_trophies__tests.rs"]
mod tests;
