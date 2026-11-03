//! The spells a run knows by name: the phrasebook's own, and the ones a wish
//! invented while the party was fighting.
//!
//! A knight's slots hold names (`Hero::spells: [Option<String>; SPELL_SLOTS]`),
//! so a run needs one place that answers "what does this name do" — and it has
//! to accept a name it has never seen, because that is the whole point of a
//! made-up magic. The book is bounded so a session cannot grow it without end.

use super::invented::{self, Effect, Reject, Spell};
use serde::{Deserialize, Serialize};

/// Spells one run may have invented in it. Past this the oldest is forgotten:
/// a session may keep inventing, but the book stays a book.
pub const MAX_INVENTED: usize = 64;

/// What a run knows: the made-up magics, by the name the wish gave them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Spellbook {
    /// In the order they were taught, so the first to be forgotten is known.
    invented: Vec<(String, Spell)>,
}

impl Spellbook {
    /// Taught a magic a wish made up. A book that already knows the name is
    /// taught the newer version: a wish asked twice makes the better one.
    pub fn learn(&mut self, spell: Spell) -> Result<(), Reject> {
        invented::check(&spell)?;
        let name = spell.name.clone();
        if let Some(slot) = self.invented.iter_mut().find(|(had, _)| *had == name) {
            slot.1 = spell;
            return Ok(());
        }
        self.invented.push((name, spell));
        // Forget the oldest: the session keeps inventing, the book stays bounded
        // by name, and a knight holding a forgotten name simply finds nothing.
        while self.invented.len() > MAX_INVENTED {
            self.invented.remove(0);
        }
        Ok(())
    }

    /// What a name does, if it is one of the made-up ones.
    pub fn get(&self, name: &str) -> Option<&Spell> {
        self.invented.iter().find(|(had, _)| had == name).map(|(_, spell)| spell)
    }

    /// Every made-up magic the run knows, in the order they were taught.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.invented.iter().map(|(name, _)| name.as_str())
    }

    /// How many the run has invented.
    pub fn len(&self) -> usize {
        self.invented.len()
    }

    /// Whether the run has invented nothing.
    pub fn is_empty(&self) -> bool {
        self.invented.is_empty()
    }

    /// Casts an invented magic by name. A name the book does not know casts
    /// nothing at all — the phrasebook's own spells are cast by the rules that
    /// already hold them, and this book never invents an effect.
    pub fn cast(&self, name: &str, at: (f32, f32), aim: f32) -> Vec<Effect> {
        match self.get(name) {
            Some(spell) => invented::cast(spell, at, aim),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::invented::{Element, OnHit};
    use crate::rules::Run;

    fn lash(name: &str, bolts: u32) -> Spell {
        Spell {
            name: name.to_owned(),
            bolts,
            spread: 0.5,
            damage: 4,
            element: Element::Frost,
            on_hit: OnHit::Slow,
            slow: 900,
            mana: 10,
            cooldown: 60,
        }
    }

    #[test]
    fn a_made_up_magic_is_cast_by_the_name_the_wish_gave_it() {
        let mut book = Spellbook::default();
        assert!(book.is_empty());
        book.learn(lash("frost lash", 3)).expect("inside the bounds");
        let effects = book.cast("frost lash", (0.0, 0.0), 0.0);
        assert_eq!(effects.len(), 3);
        assert_eq!(book.names().collect::<Vec<_>>(), vec!["frost lash"]);
    }

    #[test]
    fn a_name_the_book_does_not_know_casts_nothing() {
        let book = Spellbook::default();
        assert!(book.cast("frost lash", (0.0, 0.0), 0.0).is_empty());
    }

    #[test]
    fn asking_again_makes_the_better_one_and_never_a_second_entry() {
        let mut book = Spellbook::default();
        book.learn(lash("frost lash", 3)).unwrap();
        book.learn(lash("frost lash", 5)).unwrap();
        assert_eq!(book.len(), 1, "one name, one magic");
        assert_eq!(book.cast("frost lash", (0.0, 0.0), 0.0).len(), 5);
    }

    #[test]
    fn a_magic_outside_the_bounds_is_not_taught() {
        let mut book = Spellbook::default();
        let mut too_many = lash("nine hundred coins", 3);
        too_many.bolts = invented::MAX_BOLTS + 1;
        assert_eq!(book.learn(too_many), Err(Reject::TooManyBolts));
        assert!(book.is_empty());
    }

    #[test]
    fn a_run_learns_a_magic_a_wish_made_up_and_casts_it_by_name() {
        let mut run = Run::new(7, 1, None);
        assert!(run.cast_invented("frost lash", (0.0, 0.0), 0.0).is_empty(), "not yet invented");
        run.learn_spell(lash("frost lash", 3)).expect("inside the bounds");
        assert_eq!(run.cast_invented("frost lash", (0.0, 0.0), 0.0).len(), 3);
    }

    #[test]
    fn a_run_keeps_its_invented_magics_across_a_save() {
        let mut run = Run::new(7, 1, None);
        run.learn_spell(lash("frost lash", 3)).expect("inside the bounds");
        let text = serde_json::to_string(&run).expect("a run serializes");
        let back: Run = serde_json::from_str(&text).expect("and loads again");
        assert_eq!(back.cast_invented("frost lash", (0.0, 0.0), 0.0).len(), 3);
    }

    #[test]
    fn the_book_stays_bounded_and_forgets_the_oldest() {
        let mut book = Spellbook::default();
        for i in 0..MAX_INVENTED + 5 {
            book.learn(lash(&format!("magic {i}"), 1)).unwrap();
        }
        assert_eq!(book.len(), MAX_INVENTED, "bounded");
        assert!(book.get("magic 0").is_none(), "the oldest is forgotten");
        assert!(book.get(&format!("magic {}", MAX_INVENTED + 4)).is_some(), "the newest is kept");
    }
}
