//! Retained source ownership for an open scroll. Its UI travels to clients;
//! executable definitions stay with the authoritative offer and private saves.
use super::{Run, cards::Card, genie::Offer, phrasebook::Wish};
use serde::{Deserialize, Serialize};
use std::cell::Cell;

mod policy;
pub use policy::{
    CONTRACT, Intent, Entry, Spell, Facts, Request, Decision, ResultValue, Capture, Prepared,
};

pub const API: u32 = 1;
thread_local! { static PUBLIC: Cell<bool> = const { Cell::new(false) }; }
pub struct PublicScope(bool);
impl Drop for PublicScope {
    fn drop(&mut self) {
        PUBLIC.set(self.0);
    }
}
pub fn public_scope() -> PublicScope {
    PublicScope(PUBLIC.replace(true))
}
pub fn omit(binding: &Option<Binding>) -> bool {
    PUBLIC.get() || binding.is_none()
}

fn valid_view(offer: &Offer) -> bool {
    use super::genie::{Take, Tip};
    offer.said.len() <= 4096
        && offer.line.len() <= 4096
        && offer.ask.as_ref().is_none_or(|s| s.len() <= 4096)
        && (1..=3).contains(&offer.choices.len())
        && offer.choices.iter().all(|c| {
            c.head.len() <= 1024
                && c.gives.len() <= 4096
                && c.cost.len() <= 1024
                && c.wish
                    .as_ref()
                    .is_none_or(|id| !id.is_empty() && id.len() <= 128)
                && c.tip.as_ref().is_none_or(|tip| match tip {
                    Tip::Purse(n) | Tip::Mend(n) => *n <= 1_000_000,
                })
                && if c.take == Take::Free {
                    offer.great
                        && c.wish.is_none()
                        && c.steps == 0
                        && c.tip.is_none()
                        && offer.choices.len() == 1
                } else {
                    (1..=5).contains(&c.steps)
                        && (c.take != Take::Split || (offer.great && c.steps == 1))
                        && c.wish.as_deref().is_none_or(|id| {
                            !id.starts_with("spell:")
                                || (c.take == Take::Said && c.steps <= 2 && c.tip.is_none())
                        })
                }
        })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub wish: Wish,
    pub tier: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub api: u32,
    pub raid: u64,
    pub player: u32,
    pub epoch: u64,
    pub sources: Vec<Source>,
    pub spell: Option<Card>,
    pub blood_price: u32,
}
impl Binding {
    pub fn capture(
        run: &Run,
        player: u32,
        offer: &Offer,
        book: &[Wish],
        requested: Option<&Wish>,
    ) -> Result<Self, String> {
        let hero = run
            .players
            .get(&player)
            .ok_or("the scroll's knight no longer exists")?;
        let mut bound = Self {
            api: API,
            raid: run.raid_id,
            player,
            epoch: run.seat_epoch(player),
            sources: Vec::new(),
            spell: None,
            blood_price: (hero.max_hp / super::genie::BLOOD_PRICE).max(1),
        };
        for choice in &offer.choices {
            let Some(id) = choice.wish.as_deref() else {
                continue;
            };
            if let Some(id) = id.strip_prefix("spell:") {
                let card = run
                    .book
                    .get(id)
                    .ok_or("the offered spell no longer exists")?;
                if bound.spell.as_ref().is_some_and(|old| old != card) {
                    return Err("scroll has multiple spell sources".into());
                }
                bound.spell = Some(card.clone());
            } else if !bound.sources.iter().any(|source| source.wish.id == id) {
                let wish = requested
                    .filter(|w| w.id == id)
                    .or_else(|| book.iter().find(|w| w.id == id))
                    .ok_or("the offered wish has no captured source")?;
                let tier = wish
                    .next_tier(&hero.deck)
                    .ok_or("the offered wish is already complete")?;
                bound.sources.push(Source {
                    wish: wish.clone(),
                    tier,
                });
            }
        }
        bound.validate(offer)?;
        Ok(bound)
    }
    /// Older checkpoints cannot recover definitions that were never captured.
    /// Keep their UI for dismissal, but never silently bind it to changed files.
    pub fn legacy(raid: u64, player: u32, epoch: u64) -> Self {
        Self {
            api: 0,
            raid,
            player,
            epoch,
            sources: Vec::new(),
            spell: None,
            blood_price: 0,
        }
    }
    pub fn validate(&self, offer: &Offer) -> Result<(), String> {
        if !valid_view(offer) {
            return Err("invalid retained scroll choices".into());
        }
        if self.api == 0
            && (1..=super::MAX_PLAYERS).contains(&self.player)
            && self.sources.is_empty()
            && self.spell.is_none()
            && self.blood_price == 0
        {
            return Ok(());
        }
        let mut ids = std::collections::BTreeSet::new();
        if self.api != API
            || !(1..=super::MAX_PLAYERS).contains(&self.player)
            || self.sources.len() > 3
            || self.blood_price == 0
            || !self.sources.iter().all(|source| {
                ids.insert(&source.wish.id)
                    && super::wish_jobs::valid_wish(&source.wish)
                    && source.tier < source.wish.tiers.len()
            })
            || !offer
                .choices
                .iter()
                .all(|choice| match choice.wish.as_deref() {
                    None => true,
                    Some(id) if id.starts_with("spell:") => self
                        .spell
                        .as_ref()
                        .is_some_and(|card| id.strip_prefix("spell:") == Some(card.id.as_str())),
                    Some(id) => self.sources.iter().any(|source| source.wish.id == id),
                })
            || self.sources.iter().any(|source| {
                !offer
                    .choices
                    .iter()
                    .any(|c| c.wish.as_deref() == Some(source.wish.id.as_str()))
            })
            || self.spell.as_ref().is_some_and(|card| {
                card.kind != super::cards::Kind::Spell
                    || card.id.starts_with("wish-")
                    || !offer.choices.iter().any(|c| {
                        c.wish.as_deref().and_then(|id| id.strip_prefix("spell:"))
                            == Some(card.id.as_str())
                    })
            })
            || !serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= 512 * 1024)
        {
            return Err("invalid retained scroll sources".into());
        }
        Ok(())
    }
    pub fn owns(&self, run: &Run, player: u32) -> bool {
        self.api == API
            && self.player == player
            && self.raid == run.raid_id
            && run.players.contains_key(&player)
            && run.seat_epoch(player) == self.epoch
    }
    pub fn chosen(
        &self,
        run: &Run,
        player: u32,
        offer: &Offer,
        choice: usize,
    ) -> Result<Option<Wish>, String> {
        self.validate(offer)?;
        if !self.owns(run, player) {
            return Err(
                "This scroll predates its source or seat ownership; reopen it. Your wish is kept."
                    .into(),
            );
        }
        let choice = offer.choices.get(choice).ok_or("No such choice.")?;
        if let Some(id) = choice.wish.as_deref() {
            if let Some(id) = id.strip_prefix("spell:") {
                let captured = self
                    .spell
                    .as_ref()
                    .ok_or("the scroll has no captured spell")?;
                if run
                    .book
                    .get(id)
                    .is_none_or(|current| !super::card_paint::same_definition(current, captured))
                {
                    return Err("The spell changed while its scroll was open; reopen it. Your wish is kept.".into());
                }
                let hero = &run.players[&player];
                if !hero.spells.iter().flatten().any(|held| held == id)
                    || choice.steps > 2
                    || usize::from(choice.steps) != hero.spell_tier(id) + 1
                {
                    return Err(
                        "That spell changed tier or slot; reopen it. Your wish is kept.".into(),
                    );
                }
            } else {
                let source = self
                    .sources
                    .iter()
                    .find(|s| s.wish.id == id)
                    .ok_or("the scroll has no captured wish")?;
                if source.wish.next_tier(&run.players[&player].deck) != Some(source.tier) {
                    return Err("That wish climbed while its scroll was open; reopen it. Your wish is kept.".into());
                }
                return Ok(Some(source.wish.clone()));
            }
        }
        Ok(None)
    }
}
pub fn validate_run(run: &Run) -> Result<(), String> {
    for (&player, hero) in &run.players {
        if let Some(offer) = &hero.offer {
            let binding = offer
                .binding
                .as_ref()
                .ok_or("authoritative scroll is missing retained sources")?;
            binding.validate(offer)?;
            if binding.player != player
                || binding.raid != run.raid_id
                || binding.epoch != run.seat_epoch(player)
            {
                return Err("retained scroll belongs to a different seat".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/rules/scrolls__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-home"))]
#[path = "../../tests/rules/scrolls_bend_cases__tests.rs"]
mod scrolls_bend_cases_tests;
