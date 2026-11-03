//! Identified asynchronous cauldron work. The host owns model jobs; the game
//! owns captured inputs, spell bindings and once-only delivery receipts.
use super::{Run, cauldron, cards};
use serde::{Deserialize, Serialize};

pub const CONTRACT: &str = "delve.brew";
pub const API: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Work {
    pub ask: cauldron::Brewing,
    pub inputs: [cards::Card; 2],
    pub placeholder: cards::Card,
    pub player_name: String,
    pub result_id: String,
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
#[serde(deny_unknown_fields)]
pub struct Completion {
    /// Exact model answer, retained before any painting can change its file.
    pub input: Option<cards::Card>,
    pub accepted: Option<cards::Card>,
    pub result: Result<String, String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRequest {
    pub work: Work,
    pub card: cards::Card,
    pub taken: Vec<cauldron::Taken>,
}
pub type PolicyResult = Result<cards::Card, Vec<String>>;

fn bounded(value: &impl Serialize, limit: usize) -> bool {
    serde_json::to_vec(value).is_ok_and(|bytes| bytes.len() <= limit)
}
fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
pub fn valid_outbox(asks: &[cauldron::Brewing], raid: u64) -> bool {
    asks.len() <= 4096
        && asks.iter().all(|ask| {
            ask.raid == raid
                && (1..=super::MAX_PLAYERS).contains(&ask.knight)
                && ask.depth > 0
                && ask.cards.iter().all(|value| id(value))
                && id(&ask.placeholder)
        })
        && bounded(&asks, 1024 * 1024)
}
impl Work {
    fn binding_suffix(session: &str, raid: u64, request: u64) -> String {
        let hash = session.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        format!("{hash:x}-{raid:x}-{request:x}")
    }
    /// Canonical retained result identity, shared by owners and read-only clients.
    pub fn result_id(session: &str, raid: u64, request: u64) -> String {
        format!("brewed-{}", Self::binding_suffix(session, raid, request))
    }
    pub(super) fn capturable(run: &Run, ask: &cauldron::Brewing) -> bool {
        ask.raid == run.raid_id
            && ask.cards.iter().all(|id| run.book.get(id).is_some())
            && run.book.get(&ask.placeholder).is_some()
            && run.players.get(&ask.knight).is_some_and(|hero| {
                hero.spells
                    .iter()
                    .any(|slot| slot.as_ref() == Some(&ask.placeholder))
            })
    }
    pub fn valid(&self) -> bool {
        valid_outbox(std::slice::from_ref(&self.ask), self.ask.raid)
            && self
                .inputs
                .iter()
                .zip(&self.ask.cards)
                .all(|(card, id)| &card.id == id)
            && self.placeholder.id == self.ask.placeholder
            && id(&self.result_id)
            && self.player_name.len() <= 1024
            && bounded(self, 256 * 1024)
    }
    pub(super) fn capture(run: &mut Run, session: &str, request: u64) -> Result<Self, String> {
        let mut ask = run.brewing.first().cloned().ok_or("no brew is waiting")?;
        if !valid_outbox(std::slice::from_ref(&ask), run.raid_id) {
            return Err("invalid queued brew".into());
        }
        let inputs = ask.cards.clone().map(|id| run.book.get(&id).cloned());
        let [Some(a), Some(b)] = inputs else {
            return Err("brew inputs are missing".into());
        };
        let mut placeholder = run
            .book
            .get(&ask.placeholder)
            .cloned()
            .ok_or("brew stand-in is missing")?;
        let hero = run.players.get(&ask.knight).ok_or("the brewer has left")?;
        let slot = hero
            .spells
            .iter()
            .position(|s| s.as_ref() == Some(&ask.placeholder))
            .ok_or("the brewer no longer holds the stand-in")?;
        let player_name = hero.name.clone();
        // Rebind this particular stand-in, so a later floor/seat cannot inherit
        // a stale answer merely by holding the same recipe card.
        let suffix = Self::binding_suffix(session, run.raid_id, request);
        let result_id = format!("brewed-{suffix}");
        placeholder.id = format!("brew-pending-{suffix}");
        if run.book.get(&placeholder.id).is_some() || run.book.get(&result_id).is_some() {
            return Err("brew binding identity already exists".into());
        }
        if !run.book.insert(placeholder.clone()) {
            return Err("the spell book is full".into());
        }
        let hero = run.players.get_mut(&ask.knight).unwrap();
        hero.spells[slot] = Some(placeholder.id.clone());
        for bar in &mut hero.bar {
            if matches!(bar, Some(cards::BarSlot::Spell(id)) if id == &ask.placeholder) {
                *bar = Some(cards::BarSlot::Spell(placeholder.id.clone()));
            }
        }
        ask.placeholder = placeholder.id.clone();
        let work = Self {
            ask,
            inputs: [a, b],
            placeholder,
            player_name,
            result_id,
        };
        if !work.valid() {
            return Err("invalid captured brew".into());
        }
        run.brewing.remove(0);
        Ok(work)
    }
}
impl PolicyRequest {
    pub fn validate(&self) -> Result<(), String> {
        if !self.work.valid() || self.taken.len() > 4096 || !bounded(self, 1024 * 1024) {
            return Err("invalid brew policy inputs".into());
        }
        Ok(())
    }
    pub fn validate_result(&self, result: &PolicyResult) -> Result<(), String> {
        let valid = match result {
            Ok(card) => {
                card.id == self.work.result_id
                    && card.kind == cards::Kind::Spell
                    && cards::check(&card.id, &card.file())
                        .is_ok_and(|checked| checked.card == *card)
            }
            Err(errors) => {
                !errors.is_empty()
                    && errors.len() <= 64
                    && errors.iter().all(|error| error.len() <= 8192)
            }
        };
        if !valid || !bounded(result, 256 * 1024) {
            return Err("invalid brew policy result".into());
        }
        Ok(())
    }
    pub fn evaluate(&self) -> Result<PolicyResult, String> {
        self.validate()?;
        let mut card = self.card.clone();
        card.id = self.work.result_id.clone();
        let errors = cauldron::faults(
            &card,
            [&self.work.inputs[0], &self.work.inputs[1]],
            &self.taken,
        );
        let result = if errors.is_empty() {
            Ok(card)
        } else {
            Err(errors)
        };
        self.validate_result(&result)?;
        Ok(result)
    }
}
impl Completion {
    pub fn valid(&self, work: &Work) -> bool {
        bounded(self, 512 * 1024)
            && match (&self.input, &self.accepted) {
                (Some(input), Some(accepted)) => PolicyRequest {
                    work: work.clone(),
                    card: input.clone(),
                    taken: Vec::new(),
                }
                .validate_result(&Ok(accepted.clone()))
                .is_ok(),
                (None, None) => self.result.is_err(),
                _ => false,
            }
            && match &self.result {
                Ok(s) | Err(s) => s.len() <= 8192,
            }
    }
}

/// Install a checked answer. No authored power/name policy runs here; the
/// active implementation already computed it. World guards precede all effects.
pub(super) fn install(
    run: &mut Run,
    ask: &cauldron::Brewing,
    card: cards::Card,
) -> Result<String, String> {
    if ask.raid != run.raid_id {
        return Err("the brew belongs to another raid".into());
    }
    if run.book.get(&card.id).is_some() {
        return Err("this brew has already risen".into());
    }
    let hero = run
        .players
        .get(&ask.knight)
        .ok_or("the knight has left the delve")?;
    let slot = hero
        .spells
        .iter()
        .position(|s| s.as_ref() == Some(&ask.placeholder))
        .ok_or("the knight no longer holds this brew's stand-in")?;
    let (id, name) = (card.id.clone(), card.name.clone());
    if !run.book.insert(card) {
        return Err("the spell book is full".into());
    }
    let hero = run.players.get_mut(&ask.knight).unwrap();
    hero.spells[slot] = Some(id.clone());
    // A late model reply must not grant a free recharge or alter another slot.
    for bar in &mut hero.bar {
        if matches!(bar, Some(cards::BarSlot::Spell(s)) if s == &ask.placeholder) {
            *bar = Some(cards::BarSlot::Spell(id.clone()));
        }
    }
    if let Some(pot) = run
        .cauldron
        .as_mut()
        .filter(|p| p.raid == ask.raid && p.depth == ask.depth)
        && let Some(brew) = pot.brews.iter_mut().find(|b| b.knight == ask.knight)
        && let cauldron::Outcome::Spell {
            card, name: shown, ..
        } = &mut brew.outcome
    {
        (*card, *shown) = (id, name.clone());
    }
    run.sounds.push("wish_grant");
    run.found = Some((run.tick, ask.knight, format!("brewed {name}")));
    Ok(name)
}

#[cfg(test)]
#[path = "../../tests/rules/brew_jobs__tests.rs"]
mod tests;
