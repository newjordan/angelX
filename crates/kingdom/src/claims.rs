//! Claim retirement frees the bounded mine ledger without erasing a won
//! expedition's charter evidence or turning it into another resource payout.
use crate::*;
use std::collections::BTreeSet;

impl Kingdom {
    pub fn has_secured_resource(&self, resource: &str) -> bool {
        self.secured_resources.contains(resource)
            || self
                .claims
                .values()
                .any(|q| q.secured && q.resource == resource)
    }

    /// The union admits old saves without requiring an eager migration.
    pub fn secured_evidence(&self) -> BTreeSet<String> {
        self.secured_resources
            .iter()
            .cloned()
            .chain(
                self.claims
                    .values()
                    .filter(|q| q.secured)
                    .map(|q| q.resource.clone()),
            )
            .collect()
    }

    pub(crate) fn unlink_claim(&mut self, c: &Catalog, id: u32) -> Result<String, String> {
        let b = self.buildings.get_mut(&id).ok_or("No mine selected")?;
        if !c.plans[&b.plan].extractor {
            return Err("Only mine outposts link resource claims".into());
        }
        if b.claim.take().is_none() {
            return Err("This mine has no linked claim".into());
        }
        b.progress = 0;
        b.work_credit = 0;
        Ok("Mine unlinked. Buffered ore and travelling cargo are kept".into())
    }

    pub(crate) fn release_claim(&mut self, id: &str) -> Result<String, String> {
        let claim = self.claims.get(id).ok_or("No claim selected")?;
        if self
            .buildings
            .values()
            .any(|b| b.claim.as_deref() == Some(id))
        {
            return Err("Unlink this claim's mine before releasing it".into());
        }
        if claim.secured {
            self.secured_resources.insert(claim.resource.clone());
        }
        self.claims.remove(id);
        Ok("Claim released. A ledger slot is free; earned land charters are kept".into())
    }
}

#[cfg(test)]
#[path = "claims_tests.rs"]
mod tests;
