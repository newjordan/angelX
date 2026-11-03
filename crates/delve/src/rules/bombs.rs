//! Bomb strength and provision, by the floor the company has reached.
//! Drops are shared (`loot::shared`): each knight takes their own charges,
//! so a six-seat company receives six shares, not six times the damage.

use super::*;

/// Old saves start with the first belt; the tick restores their floor's
/// allowance before a card or town shop can grant a charge.
pub fn old_belt() -> u32 {
    5
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Provision {
    pub damage: u32,
    /// Simulation units; the same reach for foes and hostile shots.
    pub radius: f32,
    pub cap: u32,
    /// Extra chance per real slain foe, alongside the ordinary loot draw.
    pub drop_percent: u32,
    /// Charges in each knight's share of a dropped bomb card.
    pub drop_charges: u32,
    /// Charges added once on descending, never on entering a room.
    pub restock: u32,
}

pub const FLOORS: [Provision; 6] = [
    Provision {
        damage: 60,
        radius: 12.0,
        cap: 5,
        drop_percent: 2,
        drop_charges: 1,
        restock: 1,
    },
    Provision {
        damage: 100,
        radius: 16.0,
        cap: 6,
        drop_percent: 4,
        drop_charges: 1,
        restock: 1,
    },
    Provision {
        damage: 160,
        radius: 20.0,
        cap: 7,
        drop_percent: 6,
        drop_charges: 1,
        restock: 2,
    },
    Provision {
        damage: 240,
        radius: 24.0,
        cap: 8,
        drop_percent: 8,
        drop_charges: 2,
        restock: 2,
    },
    Provision {
        damage: 320,
        radius: 28.0,
        cap: 10,
        drop_percent: 10,
        drop_charges: 2,
        restock: 3,
    },
    Provision {
        damage: 400,
        radius: 32.0,
        cap: 12,
        drop_percent: 12,
        drop_charges: 3,
        restock: 4,
    },
];

/// Town uses the first belt; endless floors keep the last provision.
pub fn at(depth: u32) -> Provision {
    FLOORS[depth.saturating_sub(1).min(5) as usize]
}

impl Hero {
    /// Current floor allowance, bounded even for a malformed saved belt.
    pub fn bomb_limit(&self) -> u32 {
        self.bomb_capacity.clamp(old_belt(), MAX_BOMBS)
    }

    /// A shallower belt never confiscates charges already earned. Full
    /// belts refuse a pickup rather than spending the card for no gain.
    pub(super) fn give_bombs(&mut self, charges: u32) {
        self.bombs = if self.bombs < self.bomb_limit() {
            self.bombs.saturating_add(charges).min(self.bomb_limit())
        } else {
            self.bombs.min(MAX_BOMBS)
        };
    }
}

impl Run {
    pub(super) fn provision_bombs(&mut self, restock: bool) {
        let provision = at(self.dungeon.depth);
        for hero in self.players.values_mut() {
            hero.bomb_capacity = provision.cap;
            hero.bombs = hero.bombs.min(MAX_BOMBS);
            if restock && !hero.absent {
                hero.give_bombs(provision.restock);
            }
        }
    }

    /// A separate, depth-owned draw keeps ordinary card weights intact.
    pub(super) fn drop_bombs(&mut self, kind: EnemyKind, x: f32, y: f32) {
        if !matches!(kind, EnemyKind::Dummy | EnemyKind::Ward)
            && self.book.get("bomb").is_some()
            && self.rng.chance(at(self.dungeon.depth).drop_percent)
        {
            self.drop_item("bomb".into(), x, y, None);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_bombs__tests.rs"]
mod tests;
