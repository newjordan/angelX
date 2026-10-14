//! Back to the kingdom once the run has fallen or is won. Any knight whose
//! player is here may ask (Enter on the verdict's board). The first to ask
//! starts a short wait everyone sees; the party goes when every player here
//! has asked, or when the wait runs out. Nobody is yanked from the verdict
//! the moment it lands, and nobody away can hold the party there.
//!
//! The rules only keep who asked and how long is left. The host's server
//! sees the party is due (`Run::again_due`) and starts the next run, as
//! the host's own R does: the run settled into the realm, the seats and
//! their knights kept, the same link.

use super::*;
use std::collections::BTreeSet;

/// How long the party waits once the first knight asks: five seconds.
pub const AGAIN_WAIT: u32 = 5 * HZ;

/// Who has asked to go back, and the ticks left before the party goes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Again {
    pub asked: BTreeSet<u32>,
    pub left: u32,
}

impl Run {
    /// The run has fallen or is won: its verdict stands on the board.
    pub fn over(&self) -> bool {
        matches!(self.phase, Phase::Won | Phase::Wiped)
    }

    /// Knight `id` asks to go back to the kingdom: only once the run is
    /// over, and only a knight whose player is here.
    pub fn ask_again(&mut self, id: u32) {
        if !self.over() || self.players.get(&id).is_none_or(|h| h.away || h.absent) {
            return;
        }
        let again = self.again.get_or_insert_with(|| Again { asked: BTreeSet::new(), left: AGAIN_WAIT });
        again.asked.insert(id);
    }

    /// One tick of the wait: whether the party goes now, every player here
    /// having asked or the wait having run out. A run playing again (a host
    /// who started it another way) forgets the asking.
    pub fn again_due(&mut self) -> bool {
        if !self.over() {
            self.again = None;
            return false;
        }
        let here: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| !h.away && !h.absent)
            .map(|(&id, _)| id)
            .collect();
        let Some(again) = self.again.as_mut() else {
            return false;
        };
        again.left = again.left.saturating_sub(1);
        again.left == 0 || here.iter().all(|id| again.asked.contains(id))
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_again__tests.rs"]
mod tests;
