//! Captured visual catalogue admission. Native painters/files remain resources
//! of their host. Rules choose which resolved visuals are admitted; the commit
//! port can change only art/looks, never a held card's executable definition.
use super::{
    Run,
    cards::{self, Card},
    looks::Looks,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CONTRACT: &str = "delve.card-paint";
pub const API: u32 = 1;
pub const MAX_CANDIDATES: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub current: Card,
    pub authored: Card,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub candidates: Vec<Candidate>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub accepted: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    /// Exact catalogue entry observed during admission. A late completion may
    /// not overwrite a newer paint or a new definition under the same ID.
    pub expected: Card,
    pub art: Vec<String>,
    pub looks: Looks,
}
pub(crate) fn valid_card(card: &Card) -> bool {
    !card.id.is_empty()
        && card.id.len() <= 128
        && card
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && card.name.len() <= 1024
        && card.by.len() <= 1024
        && card.effects.len() <= 128
        && serde_json::to_vec(card).is_ok_and(|v| v.len() <= 128 * 1024)
        && cards::check(&card.id, &card.definition_file())
            .is_ok_and(|checked| checked.card.art == card.art && checked.card.looks == card.looks)
}
/// Authorship metadata and visual progress are independent of game behavior.
/// Changes to any other field require a deliberate new definition/grant.
pub(crate) fn same_definition(current: &Card, authored: &Card) -> bool {
    let mut authored = authored.clone();
    authored.by = current.by.clone();
    authored.art = current.art.clone();
    authored.looks = current.looks.clone();
    authored == *current
}
// Legacy identity derives body parts from worn attachment metadata. Treat that
// classification as retained game state until it has its own explicit contract.
pub(crate) fn retains_identity(current: &Card, painted: &Card) -> bool {
    super::identity::piece_of(current) == super::identity::piece_of(painted)
        && super::dead_wishes::colours_body(current) == super::dead_wishes::colours_body(painted)
}
fn compatible_visuals(candidate: &Candidate) -> bool {
    let mut painted = candidate.current.clone();
    painted.art = candidate.authored.art.clone();
    painted.looks = candidate.authored.looks.clone();
    retains_identity(&candidate.current, &painted) && valid_card(&painted)
}
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        if self.candidates.len() > MAX_CANDIDATES
            || !self.candidates.iter().all(|c| {
                c.current.id == c.authored.id
                    && seen.insert(&c.current.id)
                    && valid_card(&c.current)
                    && valid_card(&c.authored)
            })
            || !serde_json::to_vec(self).is_ok_and(|v| v.len() <= 1024 * 1024)
        {
            return Err("invalid painted catalogue capture".into());
        }
        Ok(())
    }
    pub fn evaluate(&self) -> Result<Decision, String> {
        self.validate()?;
        Ok(Decision {
            accepted: self
                .candidates
                .iter()
                .filter(|c| same_definition(&c.current, &c.authored) && compatible_visuals(c))
                .map(|c| c.current.id.clone())
                .collect(),
        })
    }
    pub fn validate_result(&self, decision: &Decision) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        if decision.accepted.len() > self.candidates.len()
            || !decision.accepted.iter().all(|id| {
                seen.insert(id)
                    && self
                        .candidates
                        .iter()
                        .any(|c| c.current.id == *id && compatible_visuals(c))
            })
        {
            return Err(
                "paint policy returned an uncaptured, duplicate or identity-changing visual".into(),
            );
        }
        Ok(())
    }
    pub fn updates(&self, decision: &Decision) -> Result<Vec<Update>, String> {
        self.validate()?;
        self.validate_result(decision)?;
        Ok(self
            .candidates
            .iter()
            .filter(|c| decision.accepted.contains(&c.current.id))
            .map(|c| Update {
                expected: c.current.clone(),
                art: c.authored.art.clone(),
                looks: c.authored.looks.clone(),
            })
            .collect())
    }
}
pub fn valid_updates(updates: &[Update]) -> bool {
    let mut seen = BTreeSet::new();
    updates.len() <= MAX_CANDIDATES
        && updates.iter().all(|update| {
            let mut painted = update.expected.clone();
            painted.art = update.art.clone();
            painted.looks = update.looks.clone();
            seen.insert(&update.expected.id)
                && valid_card(&update.expected)
                && valid_card(&painted)
                && retains_identity(&update.expected, &painted)
        })
}
pub fn deliver(run: &mut Run, updates: &[Update]) -> Result<bool, String> {
    if !valid_updates(updates) {
        return Err("invalid painted catalogue delivery".into());
    }
    if updates
        .iter()
        .any(|u| run.book.get(&u.expected.id) != Some(&u.expected))
    {
        return Err("painted catalogue changed after capture".into());
    }
    let mut changed = false;
    for update in updates {
        let mut card = update.expected.clone();
        changed |= card.art != update.art || card.looks != update.looks;
        card.art = update.art.clone();
        card.looks = update.looks.clone();
        // Every ID already exists: capacity cannot fail after preparation.
        run.book.insert(card);
    }
    Ok(changed)
}

#[cfg(test)]
#[path = "../../tests/rules/card_paint__tests.rs"]
mod tests;
