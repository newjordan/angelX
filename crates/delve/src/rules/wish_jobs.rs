//! Spent wishes own a session-scoped reservation. Model runners remain native
//! resources; captured inputs and exact grant/refund receipts belong to the game.
use super::{Run, genie::Asked, phrasebook::Wish};
use serde::{Deserialize, Serialize};

pub const CONTRACT: &str = "delve.wish";
pub const API: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Work {
    pub ask: Asked,
    pub maker: String,
    pub binding: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub session: String,
    pub raid: u64,
    pub request: u64,
    pub work: Work,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum Outcome {
    Granted(String),
    Plain(String),
    Refunded(String),
    /// Refunded because a held identity would be overwritten; do not publish it.
    Conflict(String),
    Stale(String),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    pub input: Option<Wish>,
    pub accepted: Option<Wish>,
    pub authored: bool,
    pub reason: Option<String>,
    pub outcome: Outcome,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRequest {
    pub work: Work,
    pub wish: Wish,
}
pub type PolicyResult = Result<Wish, Vec<String>>;

pub fn valid_ask(ask: &Asked) -> bool {
    // The mirror's look goes to the worn painter (`cosmetic_wish`), never a job.
    !ask.cosmetic
        && (1..=super::MAX_PLAYERS).contains(&ask.knight)
        && !ask.words.trim().is_empty()
        && ask.words.len() <= 4096
        && (1..=5).contains(&ask.steps)
        && serde_json::to_vec(ask).is_ok_and(|b| b.len() <= 16 * 1024)
}
pub fn valid_wish(wish: &Wish) -> bool {
    !wish.id.is_empty()
        && wish.id.len() <= super::phrasebook::MAX_ID
        && wish
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !wish.name.trim().is_empty()
        && wish.name.len() <= 1024
        && (1..=super::phrasebook::MAX_TIERS).contains(&wish.tiers.len())
        && serde_json::to_vec(wish).is_ok_and(|b| b.len() <= 128 * 1024)
        && (0..wish.tiers.len()).all(|tier| wish.card(tier).is_ok())
        && {
            // Its own source read back, or, captured before learned ladders
            // climbed on to the fifth (an old save's open scroll), what that
            // source wrote: kept as it was offered.
            let read = super::phrasebook::read_checked(&wish.definition_file()).0;
            read.as_slice() == [wish.clone()]
                || (wish.derived == 0 && read.len() == 1 && read[0].written() == *wish)
        }
}
impl Work {
    /// Canonical retained binding identity, shared by owners and read-only clients.
    pub fn binding(session: &str, raid: u64, request: u64) -> String {
        format!("wish-{session}-{raid:x}-{request:x}")
    }
    pub fn valid(&self) -> bool {
        valid_ask(&self.ask)
            && self.maker.len() <= 1024
            && self.binding.starts_with("wish-")
            && self.binding.len() <= 192
    }
    pub(super) fn capture(
        run: &mut Run,
        session: &str,
        request: u64,
        ask: Asked,
        maker: String,
    ) -> Result<Self, String> {
        let at = run
            .asked
            .iter()
            .position(|a| a == &ask)
            .ok_or("the spent wish is no longer queued")?;
        let work = Self {
            ask,
            maker,
            binding: Self::binding(session, run.raid_id, request),
        };
        if !work.valid() {
            return Err("invalid captured wish".into());
        }
        // An orphan from an older checkpoint becomes a retained stale result;
        // it must not block every later player in the queue.
        if let Some(hero) = run.players.get_mut(&work.ask.knight) {
            if hero.wish_jobs.len() >= 64 {
                return Err("too many retained wishes for this knight".into());
            }
            hero.wish_jobs.push(work.binding.clone());
        }
        run.asked.remove(at);
        if work.owns(run) {
            run.lamp_sent(work.ask.knight, &work.ask.words);
        }
        Ok(work)
    }
    pub(super) fn owns(&self, run: &Run) -> bool {
        run.players
            .get(&self.ask.knight)
            .is_some_and(|hero| hero.wish_jobs.contains(&self.binding))
    }
    pub(super) fn deliver(
        &self,
        run: &mut Run,
        wish: Option<&Wish>,
        authored: bool,
        reason: &str,
    ) -> Outcome {
        if !self.owns(run) {
            return Outcome::Stale("the spent wish no longer belongs to this knight".into());
        }
        let player = self.ask.knight;
        let mut definition_conflict = false;
        let attempt = wish.map(|wish| {
            if self.ask.making.is_some()
                && let Some(why) = run.identity_refusal(player, wish)
            {
                return Err(why);
            }
            let previous_hero = run.players[&player].clone();
            let previous_maker = run.makers.get(&wish.id).cloned();

            let held = run.players.values().any(|hero| hero.deck.iter().any(|id| super::dead_wishes::wish_of(id) == Some(wish.id.as_str())));
            // The first owner names a shared canonical definition. A later
            // model result cannot rename another player's already held card.
            if authored && !held { run.made(player, &wish.id, &self.maker); }
            let mut result = (|| {
                if let Some(start) = wish.next_tier(&run.players[&player].deck) {
                    for tier in start..(start + usize::from(self.ask.steps)).min(wish.tiers.len()) {
                        let mut card = wish.card(tier)?;
                        if let Some((_, maker)) = run.makers.get(&wish.id) { card.by = maker.clone(); }
                        if run.book.get(&card.id).is_some_and(|existing| existing != &card)
                            && run.players.values().any(|hero| hero.deck.contains(&card.id)) {
                            definition_conflict = true;
                            return Err("a knight already holds a different version of this gift; give the new gift a distinct name".into());
                        }
                    }
                }
                run.aim_wish(player, wish, self.ask.target);
                // A fairy wish's grant is drawn Legendary (`nooks::gnome`).
                run.fairy_grant = self.ask.fairy.then_some(player);
                run.grant(player, wish)
            })();
            if result.is_err() {
                // Reversing aim_wish can heal a wounded summoner. Restore
                // the captured hero exactly when the grant is refused.
                run.players.insert(player, previous_hero);
                if authored {
                    if let Some(maker) = previous_maker {
                        run.makers.insert(wish.id.clone(), maker);
                    } else {
                        run.makers.remove(&wish.id);
                    }
                }
            }
            if result.is_ok() {
                for _ in 1..self.ask.steps {
                    match run.grant_more(player, wish) {
                        Some(name) => result = Ok(name),
                        None => break,
                    }
                }
                run.sealed(player, wish, self.ask.great, self.ask.fills);
                run.sealed_quietly(player);
            }
            run.fairy_grant = None;
            result
        });
        let outcome = match attempt {
            Some(Ok(name)) => {
                run.lamp_landed(player, &self.ask.words, &name);
                // Wished upon a star: it lands as the star's (`starfall`).
                if self.ask.star {
                    run.star_wish_granted(player);
                }
                Outcome::Granted(name)
            }
            failure => {
                run.lamp_dropped(player, &self.ask.words);
                let why = failure
                    .and_then(Result::err)
                    .unwrap_or_else(|| reason.to_string());
                if !definition_conflict && self.ask.making.is_some()
                    && let Ok(name) = run.grant_neutral_for(player, &self.ask.words)
                {
                    Outcome::Plain(name)
                } else {
                    run.refund_told(player, self.ask.great, &why);
                    if definition_conflict {
                        Outcome::Conflict(why)
                    } else {
                        Outcome::Refunded(why)
                    }
                }
            }
        };
        run.players
            .get_mut(&player)
            .unwrap()
            .wish_jobs
            .retain(|id| id != &self.binding);
        outcome
    }
}
impl PolicyRequest {
    pub fn validate(&self) -> Result<(), String> {
        if !self.work.valid() || !valid_wish(&self.wish) {
            return Err("invalid wish policy inputs".into());
        }
        Ok(())
    }
    pub fn validate_result(&self, result: &PolicyResult) -> Result<(), String> {
        let valid = match result {
            Ok(wish) => wish.id == self.wish.id && valid_wish(wish),
            Err(errors) => {
                !errors.is_empty() && errors.len() <= 64 && errors.iter().all(|e| e.len() <= 8192)
            }
        };
        if !valid {
            return Err("invalid wish policy result".into());
        }
        Ok(())
    }
    pub fn evaluate(&self) -> Result<PolicyResult, String> {
        self.validate()?;
        Ok(Ok(self.wish.clone()))
    }
}
impl Completion {
    pub fn valid(&self, work: &Work) -> bool {
        let pair = match (&self.input, &self.accepted) {
            (Some(input), Some(accepted)) => {
                valid_wish(input)
                    && PolicyRequest {
                        work: work.clone(),
                        wish: input.clone(),
                    }
                    .validate_result(&Ok(accepted.clone()))
                    .is_ok()
            }
            (Some(input), None) => valid_wish(input) && self.reason.is_some(),
            (None, None) => !self.authored && self.reason.is_some(),
            _ => false,
        };
        pair && self.reason.as_ref().is_none_or(|s| s.len() <= 8192)
            && match &self.outcome {
                Outcome::Granted(s)
                | Outcome::Plain(s)
                | Outcome::Refunded(s)
                | Outcome::Conflict(s)
                | Outcome::Stale(s) => s.len() <= 8192,
            }
            && serde_json::to_vec(self).is_ok_and(|b| b.len() <= 512 * 1024)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/wish_jobs__tests.rs"]
mod tests;
