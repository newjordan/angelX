//! The recruits' ultimates (the user, 2026-10-30: "the first time a
//! recruit is picked, the person who picked the recruit is congratulated
//! and gets to wish an ultimate for that hero").
//!
//! The knights of the company each carry their own ultimate (`ults`); the
//! recruits who joined them in the tavern come with none. The first knight
//! to take a recruit up is congratulated and handed a wish, and until it is
//! spent their scroll asks one question, the recruit's ultimate, answered
//! as the identity start's is (`identity::Ask::Ultimate`): a card naming
//! the game's ultimate their words are nearest. The answer belongs to the
//! recruit, not to the knight who wished it: it is kept with the home
//! (`Home::recruit_ults`), whoever takes the recruit up after casts it, and
//! the card leaves the wisher's deck.
use super::*;
use std::collections::BTreeMap;

/// A recruit's ultimate as it was wished: the power, the name the wish gave
/// it, and who wished it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecruitUlt {
    pub ult: ults::Ult,
    pub name: String,
    pub by: String,
}

pub type RecruitUlts = BTreeMap<String, RecruitUlt>;

/// Whether the knight of `kit` is a recruit: one with no ultimate of their
/// own.
pub fn recruit(kit: &str) -> bool {
    knights::knight(kit).is_some() && ults::Ult::own(kit).is_none()
}

/// The question the scroll asks a knight wishing a recruit's ultimate.
pub fn question(kit: &str) -> String {
    let name = knights::knight(kit).map_or(kit, |k| k.name);
    format!("What is {name}'s ultimate?")
}

/// What a knight wishing the recruit `kit`'s ultimate is wishing for, as
/// a `identity::Making` with only the ultimate open.
pub fn making(kit: &str) -> identity::Making {
    identity::Making {
        parts: vec!["body".into()],
        weapon: true,
        recruit: Some(kit.to_string()),
        ..identity::Making::default()
    }
}

impl Run {
    /// The ultimate the knight of `kit` carries: their own, the one wished
    /// for them as a recruit, or the Pilgrim's Arrow while none is.
    pub fn own_ult(&self, kit: Option<&str>) -> ults::Ult {
        let kit = kit.unwrap_or("percival");
        ults::Ult::own(kit)
            .or_else(|| self.home.recruit_ults.get(kit).map(|r| r.ult))
            .unwrap_or(ults::Ult::PilgrimsArrow)
    }

    /// Knight `id` has just taken up `kit` (sat as `label`) in the tavern. A recruit whose
    /// ultimate is still unwished makes them its wisher: they are
    /// congratulated, the party hears it, and the first time this delve
    /// they are given a wish to spend on it.
    pub(super) fn first_recruit(&mut self, id: u32, kit: &str, label: &str) {
        if !recruit(kit) || self.home.recruit_ults.contains_key(kit) {
            return;
        }
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        hero.recruit_ult = Some(kit.to_string());
        if !hero.recruits_asked.iter().any(|k| k == kit) {
            hero.recruits_asked.push(kit.to_string());
            hero.wishes = hero.wishes.saturating_add(1);
        }
        let name = knights::knight(kit).map_or(kit, |k| k.name);
        let wisher = hero.name.to_uppercase();
        self.say(
            id,
            format!(
                "YOU ARE THE {}, THE FIRST TO RECRUIT {}! WISH THEIR ULTIMATE: PRESS T.",
                label.to_uppercase(),
                name.to_uppercase()
            ),
            true,
        );
        self.found = Some((self.tick, id, format!("{wisher}: FIRST TO RECRUIT {}", name.to_uppercase())));
        if self.notices.len() < 8 {
            self.notices.push(format!("{wisher} is the first to recruit {name}, and wishes their ultimate."));
        }
        self.cues.push(format!("recruit_first:{kit}"));
        self.sounds.push("wish_grant");
    }

    /// A wish just granted to knight `id`: if it gave the ultimate of the
    /// recruit they are wishing for, the ultimate is the recruit's from now
    /// on, the card leaves their deck, and the party hears whose it is.
    pub(super) fn keep_recruit_ult(&mut self, id: u32, card: &Card) {
        let Some(ult) = card.ult else { return };
        let Some(hero) = self.players.get_mut(&id) else { return };
        let Some(kit) = hero.recruit_ult.take() else { return };
        hero.deck.retain(|c| c != &card.id);
        let wisher = hero.name.clone();
        if hero.knight.as_deref() == Some(kit.as_str()) {
            hero.ult = Some(ult);
        }
        let book = self.book.clone();
        if let Some(hero) = self.players.get_mut(&id) {
            hero.rebonus(&book);
        }
        let name = knights::knight(&kit).map_or(kit.as_str(), |k| k.name).to_string();
        self.home.recruit_ults.insert(kit.clone(), RecruitUlt { ult, name: card.name.clone(), by: wisher.clone() });
        if self.notices.len() < 8 {
            self.notices.push(format!("{name}'s ultimate is {}: {}, wished by {wisher}.", card.name, ult.name()));
        }
        self.cues.push(format!("recruit_ult:{kit}:{}", ult.word()));
    }
}

#[cfg(test)]
#[path = "../../tests/rules/recruit_ults__tests.rs"]
mod tests;
