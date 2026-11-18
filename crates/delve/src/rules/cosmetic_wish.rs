//! The bathroom uses the ordinary wish scroll and worn painter, but ink
//! never enters a deck, equipment slot, freedom gauge or gameplay grant.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Look {
    pub words: String,
    pub worn: looks::Sprite,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub body: bool,
}

pub fn key(name: &str) -> String {
    let hash = name.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    format!("mirror-look-{hash:016x}")
}

impl Run {
    pub fn begin_mirror_wish(&mut self, id: u32) -> Result<(), String> {
        let h = self
            .players
            .get_mut(&id)
            .filter(|h| h.hp > 0 && !h.absent && !h.away)
            .ok_or("No knight at the mirror.")?;
        h.offer = None;
        h.mirror_wish = true;
        h.place_wish = None;
        self.lamp.rubbed.insert(id, self.tick);
        Ok(())
    }

    pub(crate) fn offer_mirror_wish(
        &mut self,
        id: u32,
        words: &str,
        wish: Option<&phrasebook::Wish>,
    ) -> Result<(), String> {
        let words = words.trim();
        if words.is_empty() || words.chars().count() > 200 {
            return Err("Describe a look in 1–200 characters.".into());
        }
        let h = self
            .players
            .get_mut(&id)
            .filter(|h| h.hp > 0)
            .ok_or("No knight at the mirror.")?;
        h.offer = Some(genie::Offer {
            said: words.into(),
            binding: None,
            great: false,
            opened: self.tick,
            ask: None,
            target: None,
            star: false,
            line: "A look alone: no strength, gear or wish spent.".into(),
            choices: vec![genie::Choice {
                take: genie::Take::Said,
                wish: wish.map(|w| w.id.clone()),
                steps: 1,
                head: "Wish a look".into(),
                gives: words.into(),
                cost: "Cosmetic only".into(),
                tip: None,
            }],
        });
        Ok(())
    }

    pub(super) fn choose_mirror_wish(&mut self, id: u32, n: usize) -> Result<String, String> {
        let offer = self
            .players
            .get(&id)
            .and_then(|h| h.offer.clone())
            .ok_or("No look is open on the scroll.")?;
        let choice = offer.choices.get(n).ok_or("No such choice.")?;
        if let Some(wish) = choice.wish.as_deref().and_then(phrasebook::get) {
            if let Some(card) = (0..wish.tiers.len())
                .rev()
                .find_map(|tier| wish.card(tier).ok().filter(|c| c.looks.worn.is_some()))
            {
                self.wear_mirror_card(id, &offer.said, &card)?;
            }
        }
        let h = self.players.get_mut(&id).unwrap();
        h.offer = None;
        h.mirror_wish = false;
        let name = h.mirror_owner().to_owned();
        self.home
            .wardrobe
            .mirror_looks
            .entry(name.clone())
            .or_insert_with(|| Look {
                words: offer.said.clone(),
                worn: looks::Sprite::default(),
                body: false,
            });
        let dropped = self.home.wardrobe.mirror_looks.get_mut(&name).is_some_and(|look| {
            let changed = look.body && look.words != offer.said;
            look.words = offer.said.clone();
            if changed {
                look.body = false;
            }
            changed
        });
        if dropped {
            self.restore_mirror_look(id);
        }
        self.asked.retain(|a| !(a.cosmetic && a.knight == id));
        self.asked.push(genie::Asked {
            cosmetic: true,
            cosmetic_name: name,
            knight: id,
            words: offer.said,
            steps: 1,
            great: false,
            fills: false,
            fairy: false,
            star: false,
            making: None,
            target: dead_wishes::Target::You,
        });
        Ok("Your look is with the worn painter; no wish spent.".into())
    }

    pub fn mirror_paint_card(&self, id: u32, words: &str) -> Result<Card, String> {
        let h = self.players.get(&id).ok_or("No knight for this look.")?;
        self.mirror_paint_card_for(h.mirror_owner(), words)
    }

    pub fn mirror_paint_card_for(&self, name: &str, words: &str) -> Result<Card, String> {
        let mut card = self
            .book
            .get("iron-helm")
            .cloned()
            .ok_or("No worn card template.")?;
        card.id = key(name);
        card.name = "Mirror look".into();
        card.text = words.split_whitespace().collect::<Vec<_>>().join(" ");
        card.kind = cards::Kind::Hold;
        card.effects.clear();
        card.arm = None;
        card.guard = None;
        card.ult = None;
        card.cooldown = 0;
        card.drop = 0;
        card.chest = 0;
        card.looks = looks::Looks::default();
        card.looks.worn = self
            .home
            .wardrobe
            .mirror_looks
            .get(name)
            .filter(|l| !l.worn.frames.is_empty())
            .map(|l| l.worn.clone());
        Ok(card)
    }

    pub fn wear_mirror_card(&mut self, id: u32, words: &str, card: &Card) -> Result<(), String> {
        let worn = card
            .looks
            .worn
            .clone()
            .ok_or("That wish has no worn picture yet.")?;
        let name = self
            .players
            .get(&id)
            .ok_or("No knight for this look.")?
            .mirror_owner()
            .to_owned();
        let body = self.home.wardrobe.mirror_looks.get(&name).is_some_and(|l| l.body && l.words == words);
        let mirror = key(&name);
        for h in self.players.values_mut() {
            h.set_aside.retain(|k| k != &mirror);
        }
        self.home.wardrobe.mirror_looks.insert(
            name,
            Look {
                words: words.into(),
                worn,
                body,
            },
        );
        self.restore_mirror_look(id);
        Ok(())
    }

    pub fn update_mirror_paint(&mut self, card: &Card) -> bool {
        let Some(worn) = card.looks.worn.clone() else {
            return false;
        };
        let names: Vec<_> = self
            .home
            .wardrobe
            .mirror_looks
            .keys()
            .map(String::as_str)
            .chain(self.players.values().map(|h| h.mirror_owner()))
            .filter(|name| key(name) == card.id)
            .map(str::to_owned)
            .collect();
        if names.is_empty() {
            return false;
        }
        for name in names {
            let body = self.home.wardrobe.mirror_looks.get(&name).is_some_and(|l| l.body && l.words == card.text);
            self.home.wardrobe.mirror_looks.insert(
                name,
                Look {
                    words: card.text.clone(),
                    worn: worn.clone(),
                    body,
                },
            );
        }
        let ids: Vec<_> = self.players.keys().copied().collect();
        for id in ids {
            self.restore_mirror_look(id);
        }
        true
    }

    pub(super) fn restore_mirror_look(&mut self, id: u32) {
        let Some(h) = self.players.get(&id) else {
            return;
        };
        let owner = h.mirror_owner().to_owned();
        let inherited = self.home.wardrobe.mirror_looks.get(&h.name).cloned();
        if !self.home.wardrobe.mirror_looks.contains_key(&owner) {
            if let Some(look) = inherited {
                self.home.wardrobe.mirror_looks.insert(owner.clone(), look);
            }
        }
        let Some(look) = self.home.wardrobe.mirror_looks.get(&owner) else {
            return;
        };
        let body = look.body;
        let Ok(mut card) = self.mirror_paint_card(id, &look.words) else {
            return;
        };
        if body {
            card.looks.worn = None;
        }
        self.book.insert(card);
    }

    pub fn set_mirror_body(&mut self, card: &str, words: &str, on: bool) -> bool {
        let mut changed = false;
        for (name, look) in self.home.wardrobe.mirror_looks.iter_mut() {
            if key(name) == card && look.body != on && (!on || look.words == words) {
                look.body = on;
                changed = true;
            }
        }
        if changed {
            let ids: Vec<_> = self.players.keys().copied().collect();
            for id in ids {
                self.restore_mirror_look(id);
            }
        }
        changed
    }

    pub fn mirror_bodies(&self) -> Vec<(String, u32)> {
        self.players
            .iter()
            .filter(|(_, h)| {
                self.home
                    .wardrobe
                    .mirror_looks
                    .get(h.mirror_owner())
                    .is_some_and(|look| look.body)
            })
            .map(|(&id, h)| (key(h.mirror_owner()), id))
            .collect()
    }

    pub fn has_mirror_body(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| {
            self.home.wardrobe.mirror_looks.get(h.mirror_owner()).is_some_and(|look| look.body)
        })
    }
}

#[cfg(test)]
#[path = "../../tests/rules/cosmetic_wish__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/cosmetic_body__tests.rs"]
mod body_tests;
