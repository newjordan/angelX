//! Captured kit requests and pure replacement policy. The model turn stays in
//! the host; ownership, delivery and cancellation belong to the game journal.
use super::{Run, Hero, cards, knights::Part, loot::Weapon};
use serde::{Deserialize, Serialize};

pub const CONTRACT: &str = "delve.reforge";
pub const API: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "part", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Kit {
    Offense {
        arm: Option<String>,
        weapon: Weapon,
        forged: Option<crate::realm::forge::Weapon>,
        temper: u8,
    },
    Defense {
        card: Option<String>,
        guard: cards::Guard,
    },
}
impl Kit {
    fn capture(hero: &Hero, part: Part) -> Self {
        match part {
            Part::Offense => Self::Offense {
                arm: hero.arm.clone(),
                weapon: hero.weapon,
                forged: hero.forged.clone(),
                temper: hero.temper,
            },
            Part::Defense => Self::Defense {
                card: hero.guard_card.clone(),
                guard: hero.guard,
            },
        }
    }
    fn card_id(&self) -> Option<&str> {
        match self {
            Self::Offense { arm, .. } => arm.as_deref(),
            Self::Defense { card, .. } => card.as_deref(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Work {
    pub player: u32,
    pub part: Part,
    pub words: String,
    pub player_name: String,
    pub depth: u32,
    pub room: usize,
    pub kit: Kit,
    pub original: Option<cards::Card>,
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
    pub input: Option<cards::Card>,
    pub accepted: Option<cards::Card>,
    pub result: Result<String, String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRequest {
    pub work: Work,
    pub card: cards::Card,
}
pub type PolicyResult = Result<cards::Card, Vec<String>>;
fn bounded(value: &impl Serialize, limit: usize) -> bool {
    serde_json::to_vec(value).is_ok_and(|bytes| bytes.len() <= limit)
}
impl Work {
    /// Canonical retained result identity, shared by owners and read-only clients.
    pub fn result_id(session: &str, raid: u64, request: u64) -> String {
        let hash = session.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        format!("reforge-{hash:x}-{raid:x}-{request:x}")
    }
    pub fn valid(&self) -> bool {
        (1..=super::MAX_PLAYERS).contains(&self.player)
            && self.depth > 0
            && !self.words.trim().is_empty()
            && self.words.len() <= 4096
            && self.player_name.len() <= 1024
            && self.result_id.starts_with("reforge-")
            && self.result_id.len() <= 128
            && self
                .result_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            && matches!(
                (&self.kit, self.part),
                (Kit::Offense { .. }, Part::Offense) | (Kit::Defense { .. }, Part::Defense)
            )
            && self
                .original
                .as_ref()
                .is_none_or(|card| Some(card.id.as_str()) == self.kit.card_id())
            && bounded(self, 256 * 1024)
    }
    pub(super) fn capture(
        run: &mut Run,
        session: &str,
        request: u64,
        player: u32,
        part: Part,
        words: String,
    ) -> Result<Self, String> {
        if !run.can_reforge(player) {
            return Err("only one pending reforge per knight, once per Sanctuary".into());
        }
        let hero = &run.players[&player];
        let kit = Kit::capture(hero, part);
        let work = Self {
            player,
            part,
            words,
            player_name: hero.name.clone(),
            depth: run.dungeon.depth,
            room: run.at,
            original: kit.card_id().and_then(|id| run.book.get(id)).cloned(),
            kit,
            result_id: Self::result_id(session, run.raid_id, request),
        };
        if !work.valid() {
            return Err("invalid captured reforge".into());
        }
        run.players.get_mut(&player).unwrap().reforge_job = Some(work.result_id.clone());
        Ok(work)
    }
    pub(super) fn install(&self, run: &mut Run, card: cards::Card) -> Result<String, String> {
        let hero = run.players.get(&self.player).ok_or("the knight has left")?;
        if hero.reforge_job.as_deref() != Some(&self.result_id) {
            return Err("the reforge no longer belongs to this knight".into());
        }
        if run.dungeon.depth != self.depth || run.at != self.room {
            return Err("the knight left this reforge's Sanctuary".into());
        }
        if Kit::capture(hero, self.part) != self.kit {
            return Err("the knight's captured kit changed".into());
        }
        if run.book.get(&card.id).is_some() {
            return Err("the reforged card already exists".into());
        }
        run.players.get_mut(&self.player).unwrap().reforge_job = None;
        run.reforge(self.player, self.part, card)
    }
}
impl PolicyRequest {
    pub fn validate(&self) -> Result<(), String> {
        if !self.work.valid() || !bounded(self, 512 * 1024) {
            return Err("invalid reforge policy inputs".into());
        }
        Ok(())
    }
    pub fn validate_result(&self, result: &PolicyResult) -> Result<(), String> {
        let valid = match result {
            Ok(card) => {
                card.id == self.work.result_id
                    && card.kind
                        == match self.work.part {
                            Part::Offense => cards::Kind::Arm,
                            Part::Defense => cards::Kind::Guard,
                        }
                    && cards::check(&card.id, &card.definition_file())
                        .is_ok_and(|c| c.card == *card)
            }
            Err(errors) => {
                !errors.is_empty() && errors.len() <= 64 && errors.iter().all(|e| e.len() <= 8192)
            }
        };
        if !valid || !bounded(result, 256 * 1024) {
            return Err("invalid reforge policy result".into());
        }
        Ok(())
    }
    pub fn evaluate(&self) -> Result<PolicyResult, String> {
        self.validate()?;
        let mut card = self.card.clone();
        card.id = self.work.result_id.clone();
        let wanted = match self.work.part {
            Part::Offense => cards::Kind::Arm,
            Part::Defense => cards::Kind::Guard,
        };
        let result = if card.kind != wanted {
            Err(vec![format!(
                "a {} reforge needs kind {}",
                self.work.part.word(),
                wanted.word()
            )])
        } else {
            Ok(card)
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
#[cfg(test)]
#[path = "../../tests/rules/reforge_jobs__tests.rs"]
mod tests;
