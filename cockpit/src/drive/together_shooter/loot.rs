//! Loot and arms: when monsters drop something, the cards lying on a floor
//! (see `cards`) and the weapons a knight carries. Every roll comes from the
//! run's own seeded generator, so a raid replays exactly.

use super::EnemyKind;
use serde::{Deserialize, Serialize};

/// The run's deterministic generator (xorshift64*).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self((seed ^ 0x9e37_79b9_7f4a_7c15) | 1)
    }

    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    pub(crate) fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < percent as usize
    }

    pub(crate) fn pick<T: Copy>(&mut self, weighted: &[(T, u32)]) -> T {
        let total: u32 = weighted.iter().map(|(_, w)| w).sum();
        let mut roll = self.below(total as usize) as u32;
        for &(item, weight) in weighted {
            if roll < weight {
                return item;
            }
            roll -= weight;
        }
        weighted[0].0
    }
}

/// A knight's ranged arm. Shots are slow and heavy: one well-placed arrow
/// matters more than a stream of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Weapon {
    Bow,
    Crossbow,
    Handgonne,
}

pub(crate) struct Arms {
    pub(crate) speed: f32,
    pub(crate) damage: u32,
    pub(crate) cooldown: u32,
    /// Monsters a shot passes through before it stops.
    pub(crate) pierce: u8,
}

impl Weapon {
    pub(crate) fn arms(self) -> Arms {
        match self {
            Weapon::Bow => Arms {
                speed: 17.0,
                damage: 32,
                cooldown: 15,
                pierce: 0,
            },
            Weapon::Crossbow => Arms {
                speed: 20.0,
                damage: 75,
                cooldown: 32,
                pierce: 1,
            },
            Weapon::Handgonne => Arms {
                speed: 27.0,
                damage: 95,
                cooldown: 45,
                pierce: 0,
            },
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Weapon::Bow => "Bow",
            Weapon::Crossbow => "Crossbow",
            Weapon::Handgonne => "Handgonne",
        }
    }
}

/// Something lying on a room's floor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Item {
    /// The card's id in the run's book.
    #[serde(alias = "kind", deserialize_with = "card_id")]
    pub(crate) card: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    /// A weapon just set down stays put until the knight who dropped it
    /// steps away, so standing still never swaps arms back and forth.
    pub(crate) held_off: Option<u32>,
}

/// Percent chance a fallen monster leaves something behind.
pub(crate) fn drop_chance(kind: EnemyKind) -> u32 {
    match kind {
        EnemyKind::Bat => 20,
        EnemyKind::Skeleton | EnemyKind::Wraith | EnemyKind::Imp => 35,
        EnemyKind::Demon | EnemyKind::Dragon => 100,
        EnemyKind::Boss
        | EnemyKind::Mimic
        | EnemyKind::Goblin
        | EnemyKind::Ward
        | EnemyKind::Dummy => 0,
        EnemyKind::Sapper | EnemyKind::Hob => 30,
        EnemyKind::Necromancer | EnemyKind::Shaman => 60,
        EnemyKind::Warboar => 50,
        EnemyKind::Slime => 15,
        EnemyKind::Flesher | EnemyKind::Silkmother => 80,
        EnemyKind::Hexer => 60,
        EnemyKind::Lich | EnemyKind::Hollow => 70,
        EnemyKind::Spiderling => 4,
        // The Pit Tyrant leaves its own: the Talisman, gold, a prize.
        EnemyKind::PitTyrant => 0,
    }
}

/// Percent chance a fallen monster also leaves its delve's material.
pub(crate) fn spoil_chance(kind: EnemyKind) -> u32 {
    match kind {
        EnemyKind::Bat => 25,
        EnemyKind::Skeleton | EnemyKind::Wraith | EnemyKind::Imp => 50,
        EnemyKind::Demon => 100,
        EnemyKind::Dragon
        | EnemyKind::Boss
        | EnemyKind::Goblin
        | EnemyKind::Ward
        | EnemyKind::Dummy => 0,
        EnemyKind::Mimic | EnemyKind::Necromancer | EnemyKind::Shaman => 100,
        EnemyKind::Sapper | EnemyKind::Hob => 40,
        EnemyKind::Warboar => 80,
        EnemyKind::Slime => 20,
        EnemyKind::Flesher
        | EnemyKind::Silkmother
        | EnemyKind::Hexer
        | EnemyKind::Lich
        | EnemyKind::Hollow => 100,
        EnemyKind::Spiderling | EnemyKind::PitTyrant => 0,
    }
}

/// Checkpoints from before cards held an item kind; each one is now the
/// built-in card of the same name.
fn card_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Old {
        Id(String),
        Arms { arms: Weapon },
    }
    Ok(match Old::deserialize(d)? {
        Old::Id(id) => id,
        Old::Arms { arms } => arms.name().to_ascii_lowercase(),
    })
}
