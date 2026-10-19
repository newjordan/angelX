//! The fellowship at a way out. A doorway, or the stairs down, moves the
//! party only when every knight who counts stands in that same one: a knight
//! who is alive, here, awake and not turned to stone. Fallen, away and
//! absent knights (`Run::away`, `Hero::absent`) never hold the party back,
//! and a knight alone goes at once.
//!
//! Knights who reach a way first wait in it, and may walk back out whenever
//! they like: nothing locks or pushes them, and leaving stops counting them.
//! While some wait and the rest have yet to come, the waiting see
//! "WAITING FOR FELLOWSHIP" and their friends are shown where they are
//! awaited (`Run::fellowship_line`). A retreat to the kingdom stays a seat's
//! own decision (`exits`), and the boss door, the guardian's lock and the
//! Winding Stair's hold work as before.

use super::*;

/// The stairs, as a way out beside the four doorways (north, east, south,
/// west: `0..=3`).
pub const STAIRS: u8 = 4;

/// What each way is called, by number.
const WAYS: [&str; 5] = ["THE NORTH DOOR", "THE EAST DOOR", "THE SOUTH DOOR", "THE WEST DOOR", "THE STAIRS"];

/// The knights waiting in a way out for the rest of the party.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Each waiting knight's seat and the way it stands in.
    pub waiting: Vec<(u32, u8)>,
}

impl State {
    pub(super) fn clear(&mut self) {
        self.waiting.clear();
    }

    /// Knights `ways` (a seat and a way each) wait.
    pub(super) fn wait(&mut self, ways: impl Iterator<Item = (u32, u8)>) {
        self.waiting.extend(ways);
    }

    /// Whether nobody waits (a save then leaves the state out).
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// Whether knight `id` stands waiting.
    pub fn is_waiting(&self, id: u32) -> bool {
        self.waiting.iter().any(|&(seat, _)| seat == id)
    }
}

impl Run {
    /// Whether any knight waits in a way out for the rest of the party.
    pub fn waiting_for_fellowship(&self) -> bool {
        !self.fellowship.waiting.is_empty()
    }

    /// What the screen of seat `focus` says of the wait, and whether that
    /// seat is among those waiting (its words flash): "WAITING FOR
    /// FELLOWSHIP" to a knight who stands in a way, and to the rest where
    /// their friends wait.
    pub fn fellowship_line(&self, focus: Option<u32>) -> Option<(String, bool)> {
        let &(_, way) = self.fellowship.waiting.first()?;
        if focus.is_some_and(|id| self.fellowship.is_waiting(id)) {
            return Some(("WAITING FOR FELLOWSHIP".into(), true));
        }
        let place = WAYS[usize::from(way).min(WAYS.len() - 1)];
        Some((format!("FRIENDS WAIT AT {place}"), false))
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_fellowship__tests.rs"]
mod tests;
