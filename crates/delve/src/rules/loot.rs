//! Loot and arms: when monsters drop something, the cards lying on a floor
//! (see `cards`) and the weapons a knight carries. Every roll comes from the
//! run's own seeded generator, so a raid replays exactly.

use super::EnemyKind;
use serde::{Deserialize, Serialize};

/// The run's deterministic generator (xorshift64*).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
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

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    pub fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < percent as usize
    }

    pub fn pick<T: Copy>(&mut self, weighted: &[(T, u32)]) -> T {
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

/// A knight's ranged arm. Each is a shape, not a number: the bow a stream,
/// the crossbow a rail, the handgonne a fan. Shots fly well ahead of the
/// knight; the monsters' stay slow and readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weapon {
    Fists,
    Bow,
    Crossbow,
    Handgonne,
}

pub struct Arms {
    pub speed: f32,
    pub damage: u32,
    pub cooldown: u32,
    /// Monsters a shot passes through before it stops.
    pub pierce: u8,
    /// Shots in one pull, spread evenly over `arc` degrees.
    pub shots: u32,
    pub arc: f32,
    /// The most a shot strays from the aim, in radians, so a stream looks
    /// alive.
    pub jitter: f32,
    /// Ticks a shot flies before it falls.
    pub ttl: u32,
}

impl Weapon {
    /// The arm untempered. Tempering (`feel::TEMPER_MAX` times, by picking
    /// up the arm already held) adds to its shape, step by step:
    ///
    /// | Arm | 1 | 2 | 3 |
    /// |---|---|---|---|
    /// | Bow | a second stream | a third | the outer streams home |
    /// | Crossbow | a thicker rail | it splits on its first kill | it chains |
    /// | Handgonne | seven balls | they bounce once | they burst |
    pub fn arms(self) -> Arms {
        match self {
            Weapon::Fists => Arms { speed: 0.0, damage: 6, cooldown: 9, pierce: 0, shots: 0, arc: 80.0, jitter: 0.0, ttl: 0 },
            Weapon::Bow => Arms {
                speed: 40.0,
                damage: 24,
                cooldown: 10,
                pierce: 0,
                shots: 1,
                arc: 0.0,
                jitter: 0.05,
                ttl: 2 * super::HZ,
            },
            Weapon::Crossbow => Arms {
                speed: 70.0,
                damage: 55,
                cooldown: 23,
                pierce: u8::MAX,
                shots: 1,
                arc: 0.0,
                jitter: 0.0,
                ttl: super::HZ,
            },
            Weapon::Handgonne => Arms {
                speed: 34.0,
                damage: 14,
                cooldown: 25,
                pierce: 0,
                shots: 5,
                arc: 30.0,
                jitter: 0.0,
                ttl: 20,
            },
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Weapon::Fists => "Fists",
            Weapon::Bow => "Bow",
            Weapon::Crossbow => "Crossbow",
            Weapon::Handgonne => "Handgonne",
        }
    }
}

/// Something lying on a room's floor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    /// The card's id in the run's book.
    #[serde(alias = "kind", deserialize_with = "card_id")]
    pub card: String,
    pub x: f32,
    pub y: f32,
    /// A weapon just set down stays put until the knight who dropped it
    /// steps away, so standing still never swaps arms back and forth.
    pub held_off: Option<u32>,
}

/// Percent chance a fallen monster leaves something behind.
pub fn drop_chance(kind: EnemyKind) -> u32 {
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
        EnemyKind::Biter | EnemyKind::Shard => 6,
        EnemyKind::Mole | EnemyKind::Leech | EnemyKind::Mirrorling | EnemyKind::Geode => 30,
        EnemyKind::Pavise | EnemyKind::Hound => 45,
        EnemyKind::Marksman | EnemyKind::Spitter | EnemyKind::Ringer => 55,
        // The Pit Tyrant leaves its own: the Talisman, gold, a prize.
        EnemyKind::PitTyrant => 0,
        // Humphrey always leaves something; consider it compost rent.
        EnemyKind::Risen => 100,
        EnemyKind::Broodmarrow => 90,
        EnemyKind::Oathbound => 55,
        EnemyKind::Whirlgrave => 65,
    }
}

/// Percent chance a fallen monster also leaves its delve's material.
pub fn spoil_chance(kind: EnemyKind) -> u32 {
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
        EnemyKind::Biter | EnemyKind::Shard => 8,
        EnemyKind::Mole | EnemyKind::Leech | EnemyKind::Mirrorling | EnemyKind::Geode => 35,
        EnemyKind::Pavise | EnemyKind::Hound => 50,
        EnemyKind::Marksman | EnemyKind::Spitter | EnemyKind::Ringer => 60,
        // No delve material in him: only cabbages and grudges.
        EnemyKind::Risen => 0,
        EnemyKind::Broodmarrow => 90,
        EnemyKind::Oathbound => 55,
        EnemyKind::Whirlgrave => 65,
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
