//! Back to the kingdom once the run has fallen or is won. Any knight whose
//! player is here may ask (Enter on the verdict's board). The first to ask
//! starts a short wait everyone sees; the party goes when every player here
//! has asked, or when the wait runs out. With nobody asking, the party goes
//! anyway once the verdict has stood `VERDICT_HOLD` (longer with the run's
//! deeds to show, `Run::verdict_hold`). Nobody is yanked from the verdict
//! the moment it lands, and nobody away can hold the party there.
//!
//! The rules only keep who asked and how long is left. The host's server
//! sees the party is due (`Run::again_due`) and starts the next run, as
//! the host's own R does: the run settled into the realm, the seats and
//! their knights kept, the same link.

use super::*;
use std::collections::BTreeSet;

pub const AGAIN_WAIT: u32 = 5 * HZ;
pub const VERDICT_HOLD: u32 = 10 * HZ;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Again {
    pub asked: BTreeSet<u32>,
    pub left: u32,
}

impl Run {
    pub fn over(&self) -> bool {
        matches!(self.phase, Phase::Won | Phase::Wiped)
    }

    pub fn ask_again(&mut self, id: u32) {
        if !self.over() || self.players.get(&id).is_none_or(|h| h.away || h.absent) {
            return;
        }
        let again = self.again.get_or_insert_with(|| Again { asked: BTreeSet::new(), left: AGAIN_WAIT });
        again.asked.insert(id);
        again.left = again.left.min(AGAIN_WAIT);
    }

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
        self.pay_delve_return();
        let hold = self.verdict_hold();
        let again = self.again.get_or_insert_with(|| Again { asked: BTreeSet::new(), left: hold });
        again.left = again.left.saturating_sub(1);
        again.left == 0 || here.iter().all(|id| again.asked.contains(id))
    }

    fn pay_delve_return(&mut self) {
        if self.home.delve_paid || !crate::kingdom::pace::evening() {
            return;
        }
        let depth = self.home.deepest.max(self.dungeon.depth);
        if depth == 0 {
            return;
        }
        let boss = self.phase == Phase::Won || depth >= DEEPEST;
        let n = self.home.industry.pay_delve(depth, boss);
        if n == 0 && self.home.industry.keep_site().is_none() {
            return;
        }
        self.home.delve_paid = true;
        if n > 0 {
            self.notices.push(crate::kingdom::Kingdom::delve_plate(n));
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_again__tests.rs"]
mod tests;
