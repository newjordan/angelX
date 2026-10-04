//! Overclass windows: moments the game opens for wishes.
//!
//! When the party clears a room, a window opens. Each knight standing may
//! make one wish in it; a known wish (the phrasebook) is granted at once,
//! in the running game, and every friend's mirror shows it next tick. The
//! window closes when the next fight begins, so wishes come between fights,
//! at the pace of the game.

use super::*;

/// An open window: the tick it opened, and the knights who have wished.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Window {
    pub(crate) opened: u64,
    pub(crate) wished: Vec<u32>,
}

impl Run {
    /// Open a window: the room is clear.
    pub(super) fn open_window(&mut self) {
        self.window = Some(Window {
            opened: self.tick,
            wished: Vec::new(),
        });
        self.cues.push("window".into());
    }

    /// Knight `id` may wish now: a window is open, they stand, and they
    /// have not wished in it.
    pub(crate) fn can_wish(&self, id: u32) -> bool {
        self.window
            .as_ref()
            .is_some_and(|w| !w.wished.contains(&id))
            && self.players.get(&id).is_some_and(|h| h.hp > 0)
    }

    /// Grant knight `id` a known wish: the next tier of its ladder, held
    /// at once (the tier below is let go). The window, if any, is spent.
    pub(crate) fn grant(&mut self, id: u32, wish: &phrasebook::Wish) -> Result<String, String> {
        let Some(hero) = self.players.get(&id) else {
            return Err("no such knight".into());
        };
        let Some(tier) = wish.next_tier(&hero.deck) else {
            return Err(format!("{} is already granted in full", wish.name));
        };
        let card = wish.card(tier)?;
        let name = card.name.clone();
        let card_id = card.id.clone();
        let more_hp = |card: &Card| {
            card.effects
                .iter()
                .map(|e| match *e {
                    cards::Effect::MaxHp(v) => v,
                    _ => 0,
                })
                .sum::<u32>()
        };
        let before = tier
            .checked_sub(1)
            .and_then(|t| self.book.get(&wish.card_id(t)).map(more_hp))
            .unwrap_or(0);
        let gained = more_hp(&card).saturating_sub(before);
        if !self.book.insert(card) {
            return Err("the book is full".into());
        }
        let prefix = format!("wish-{}-", wish.id);
        let hero = self.players.get_mut(&id).expect("checked");
        hero.deck.retain(|c| !c.starts_with(&prefix));
        hero.deck.push(card_id);
        hero.rebonus(&self.book);
        hero.max_hp += gained;
        hero.hp = (hero.hp + gained).min(hero.max_hp);
        if let Some(window) = self.window.as_mut() {
            window.wished.push(id);
        }
        self.found = Some((self.tick, id, format!("wished for {name}")));
        self.cues.push("wish_granted".into());
        self.boons.push((id, self.tick));
        self.sounds.push("card_pickup");
        Ok(name)
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_overclass__tests.rs"]
mod tests;
