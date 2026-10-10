//! Merlin, the realm's guide: an old
//! wizard who has seen every delve there ever was, stands by the Winding
//! Stair, and tells whoever walks up the one thing most worth doing next
//! that they haven't done yet. He was a toad once, for a fortnight, and has
//! opinions about witches.

use super::talents::knight_key;
use super::*;

/// Where Merlin stands in the Undercroft (tiles): west of the stair.
pub(crate) const MERLIN_AT: (f32, f32) = (8.0, 6.0);
/// The greeting bit Merlin keeps in `greeted`.
pub(super) const MERLIN_BIT: u8 = 1 << 6;
/// How close a knight comes before he speaks.
const MERLIN_REACH: f32 = 3.6;

impl Run {
    /// What Merlin thinks most worth doing next, as the tip's word: the
    /// first of his list the realm hasn't done.
    pub(crate) fn merlin_tip(&self) -> &'static str {
        let home = &self.home;
        let delved = home.deepest >= 1;
        let lesson = self
            .players
            .values()
            .any(|h| home.prowess(&knight_key(h)).waiting() > 0);
        if home.levels.is_empty() {
            "build"
        } else if !home.feats.contains("spin") {
            "wheel"
        } else if !home.boxes.is_empty() {
            "coffer"
        } else if lesson {
            "lessons"
        } else if delved && home.level(super::home::Station::Wing) == 0 {
            "wing"
        } else if delved && home.residents.len() < 4 {
            "cages"
        } else if delved && !home.feats.contains("secret_room") {
            "walls"
        } else if delved && !home.feats.contains("rune_runner") {
            "runes"
        } else if home.deepest >= 2 && !home.feats.contains("pit_tyrant") {
            "pit"
        } else if home.deepest >= 2 && !home.feats.contains("ribbit") {
            "frogs"
        } else {
            "wisdom"
        }
    }

    /// Merlin speaks as a knight walks up to him, once per approach.
    /// Returns his bit for `greeted` while someone is near.
    pub(super) fn greet_merlin(&mut self) -> u8 {
        let (mx, my) = (MERLIN_AT.0 * TILE_UNITS, MERLIN_AT.1 * TILE_UNITS);
        let close = self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - mx).hypot(h.y - my) < MERLIN_REACH);
        if !close {
            return 0;
        }
        if self.greeted & MERLIN_BIT == 0 {
            let tip = self.merlin_tip();
            self.cues.push(format!("merlin:{tip}"));
        }
        MERLIN_BIT
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_merlin__tests.rs"]
mod tests;
