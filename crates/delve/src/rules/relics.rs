//! The relics of the deep: rare finds below Dragon Keep that the realm's
//! great works at its holdings each want one of. A relic is found, never
//! bought — hauled up in a knight's purse like any spoil and spent once,
//! into a bay.
//!
//! Which floors have them: the flooded Archive gives up hearthstones,
//! forgehearts and ledgerglass; the Fungal deep bellmetal and warhorns;
//! the Unknown floor, waystones. One chest in six on those floors, one
//! guardian in three. The draw is the delve's own dice (`Rng`), a plain
//! number, so the table is tested without a run.

use super::*;
use crate::realm::Spoil;

/// One find in every six draws (the chest roll).
pub const CHEST_EVERY: u32 = 6;
/// The chest's draw space: four finds' worth of table turns.
pub const CHEST_DRAW: u32 = 24;
/// One find in every three draws (a guardian's fall).
pub const GUARDIAN_EVERY: u32 = 6;
pub const GUARDIAN_DRAW: u32 = 12;

/// What the floors of `pack` give up, in the order the draws turn through
/// them. The shallow floors (and the Swamp) give up nothing.
pub fn table(pack: Pack) -> &'static [Spoil] {
    match pack {
        Pack::Archive => &[Spoil::Hearthstone, Spoil::Forgeheart, Spoil::Ledgerglass],
        Pack::Fungal => &[Spoil::Bellmetal, Spoil::Warhorn, Spoil::Ledgerglass],
        Pack::Unknown => &[Spoil::Waystone, Spoil::Bellmetal, Spoil::Forgeheart],
        _ => &[],
    }
}

/// Whether draw `draw` from `pack`'s floors is a relic, and which. A pure
/// function of the number: `every` is the table's stride, `draw` the dice.
pub fn find(pack: Pack, draw: u32, every: u32) -> Option<Spoil> {
    let table = table(pack);
    if table.is_empty() || every == 0 || draw % every != 0 {
        return None;
    }
    Some(table[(draw / every) as usize % table.len()])
}

impl Run {
    /// The deep gives a relic up: into the first living knight's purse, to
    /// be banked with the rest.
    pub(super) fn relic_find(&mut self, draw: u32, every: u32) {
        let Some(relic) = find(self.dungeon.pack, draw, every) else {
            return;
        };
        let Some((&id, _)) = self.players.iter().find(|(_, h)| h.hp > 0 && !h.stone) else {
            return;
        };
        if let Some(hero) = self.players.get_mut(&id) {
            hero.carried.add(relic, 1);
        }
        self.found = Some((self.tick, id, format!("A {}, up from the deep!", relic.word())));
        self.cues.push("relic".into());
        self.sounds.push("spoil_pickup");
    }
}
