//! Safe claim retirement and bounded discovery feedback at the game boundary.
use super::*;
use crate::kingdom::{Claim, Command, MAX_CLAIMS, catalog};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default)]
pub struct CapacityNotice {
    entry: Option<(u64, u64, u32, usize)>,
    seams: BTreeSet<String>,
}

impl Run {
    pub fn release_frontier_claim(&mut self, key: &str) -> Result<String, String> {
        let guardian = self.kingdom.claim_bosses.get(key).map(|e| e.enemy);
        let sentries = self.kingdom.ore_sentries.get(key);
        if self.enemies.iter().chain(&self.parked).any(|e| {
            e.hp > 0
                && (guardian == Some(e.id)
                    || sentries.is_some_and(|s| s.sentries.iter().any(|s| s.id == e.id)))
        }) {
            return Err("Finish this claim's encounter before releasing it".into());
        }
        if self.dungeon.depth > 0 {
            return Err("Return to the kingdom's Claims desk to release a claim".into());
        }
        // Keep any death ledger. Rediscovering this same seam must never
        // respawn defeated sentries or mint a second guardian reward.
        self.home
            .industry
            .apply(catalog(), Command::ReleaseClaim(key.into()))
    }

    pub(super) fn reset_claim_capacity_notice(&mut self) {
        let entry = (self.seed, self.raid_id, self.dungeon.depth, self.at);
        let notice = &mut self.kingdom.claim_capacity_notice;
        if notice.entry != Some(entry) {
            notice.entry = Some(entry);
            notice.seams.clear();
        }
    }

    pub(super) fn discover_frontier_claim(&mut self, d: &industry::Deposit) -> bool {
        self.reset_claim_capacity_notice();
        let full = self.home.industry.claims.len() >= MAX_CLAIMS
            && !self.home.industry.claims.contains_key(&d.id);
        if full {
            let notice = &mut self.kingdom.claim_capacity_notice;
            if notice.seams.len() < MAX_CLAIMS && notice.seams.insert(d.id.clone()) {
                self.notices.push(format!(
                    "{}: claim ledger full ({MAX_CLAIMS}). Return to Kingdom Works, Claims; unlink and release an old claim to record this seam.",
                    d.name
                ));
            }
            return false;
        }
        self.home.industry.discover(
            catalog(),
            d.id.clone(),
            Claim {
                resource: d.resource.into(),
                name: d.name.into(),
                source: format!(
                    "{} / floor {}",
                    self.dungeon.pack.name(),
                    self.dungeon.depth
                ),
                secured: false,
                rate: 2,
            },
        )
    }
}

#[cfg(test)]
#[path = "../../tests/rules/frontier_claims__tests.rs"]
mod tests;
