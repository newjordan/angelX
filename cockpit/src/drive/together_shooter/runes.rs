//! Power runes. Now and then, a few seconds into a fight, a
//! rune wells up out of the floor somewhere in the room, and whoever reaches
//! it first has it: Haste, Double Damage, Regeneration, Arcane,
//! Invisibility, Illusion, Bounty or Wisdom. The Herald calls each by name
//! the moment it is taken, as a ringside announcer would, and Fortune's
//! audience loves nothing more than a race for one.
//!
//! (Not the Forge's runes: those are the lines a weapon card is written in.)

use super::ults::{Phantom, ULT_FULL};
use super::*;
use crate::drive::together_realm::Spoil;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuneKind {
    /// Run faster.
    Haste,
    /// Hit twice as hard.
    DoubleDamage,
    /// Mend quickly, until full or hit.
    Regeneration,
    /// The ultimate charges, spells and the roll come back quicker.
    Arcane,
    /// Monsters cannot see the knight.
    Invisibility,
    /// Two images of the knight, at once.
    Illusion,
    /// Gold for every knight standing, at once.
    Bounty,
    /// Experience for the whole party, at once.
    Wisdom,
}

impl RuneKind {
    pub(crate) const ALL: [RuneKind; 8] = [
        RuneKind::Haste,
        RuneKind::DoubleDamage,
        RuneKind::Regeneration,
        RuneKind::Arcane,
        RuneKind::Invisibility,
        RuneKind::Illusion,
        RuneKind::Bounty,
        RuneKind::Wisdom,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            RuneKind::Haste => "Haste",
            RuneKind::DoubleDamage => "Double Damage",
            RuneKind::Regeneration => "Regeneration",
            RuneKind::Arcane => "Arcane",
            RuneKind::Invisibility => "Invisibility",
            RuneKind::Illusion => "Illusion",
            RuneKind::Bounty => "Bounty",
            RuneKind::Wisdom => "Wisdom",
        }
    }

    /// The word in a cue: `rune:double_damage`.
    pub(crate) fn word(self) -> &'static str {
        match self {
            RuneKind::Haste => "haste",
            RuneKind::DoubleDamage => "double_damage",
            RuneKind::Regeneration => "regeneration",
            RuneKind::Arcane => "arcane",
            RuneKind::Invisibility => "invisibility",
            RuneKind::Illusion => "illusion",
            RuneKind::Bounty => "bounty",
            RuneKind::Wisdom => "wisdom",
        }
    }

    /// How long it stays with a knight; nothing for a rune spent at once.
    pub(crate) fn lasts(self) -> u32 {
        match self {
            RuneKind::Haste => 12 * HZ,
            RuneKind::DoubleDamage | RuneKind::Regeneration | RuneKind::Arcane => 15 * HZ,
            RuneKind::Invisibility => 10 * HZ,
            RuneKind::Illusion | RuneKind::Bounty | RuneKind::Wisdom => 0,
        }
    }
}

/// A rune in the room: what, where, and the tick it wells up.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Rune {
    pub(crate) kind: RuneKind,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) at: u64,
}

/// A rune a knight carries, and the ticks it has left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Held {
    pub(crate) kind: RuneKind,
    pub(crate) left: u32,
}

/// How far into a fight a rune wells up, and the chance a fight has one.
pub(crate) const RUNE_AFTER: u32 = 4 * HZ;
const RUNE_CHANCE: u64 = 35;
/// Rune Rush: a fresh rune this often while a fight lasts.
const RUNE_AGAIN: u32 = 10 * HZ;
/// A knight this close takes it.
pub(crate) const RUNE_REACH: f32 = 1.8;
/// Haste's pace.
pub(crate) const HASTE: f32 = 1.5;
/// Regeneration mends a point every this many ticks (ten a second).
const MEND_EVERY: u64 = 3;
/// Bounty's gold, at the top and for each floor down; Wisdom's experience.
const BOUNTY_GOLD: u32 = 15;
const BOUNTY_PER_FLOOR: u32 = 10;
const WISDOM_XP: u32 = 60;
const WISDOM_PER_FLOOR: u32 = 20;
/// Runes in one delve for Rune Runner.
const RUNNER: u32 = 4;

impl Hero {
    /// This knight carries a rune of `kind`.
    pub(crate) fn has_rune(&self, kind: RuneKind) -> bool {
        self.rune.is_some_and(|r| r.kind == kind && r.left > 0)
    }
}

/// Where a rune wells up: one of a few open spots in the room, clear of
/// pillars, water and pits on every side.
fn rune_spot(room: &Room, roll: u64) -> Option<(f32, f32)> {
    const SPOTS: [(f32, f32); 7] = [
        (0.5, 0.5),
        (0.25, 0.3),
        (0.75, 0.3),
        (0.25, 0.7),
        (0.75, 0.7),
        (0.5, 0.28),
        (0.5, 0.72),
    ];
    let clear = |x: f32, y: f32| {
        let (c, r) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
        [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .all(|&(dc, dr)| room.tile(c + dc, r + dr) == Tile::Floor)
    };
    let start = (roll >> 8) as usize % SPOTS.len();
    (0..SPOTS.len())
        .map(|k| SPOTS[(start + k) % SPOTS.len()])
        .map(|(fx, fy)| (room.width() * fx, room.height() * fy))
        .find(|&(x, y)| clear(x, y))
}

impl Run {
    /// A fight begins: perhaps a rune, a few seconds in (always, in Rune
    /// Rush).
    pub(super) fn place_rune(&mut self) {
        self.rune = None;
        self.well_up(0);
    }

    /// A rune for this fight, rolled from the room and `salt`.
    fn well_up(&mut self, salt: u64) {
        if self.phase != Phase::Fighting || self.dungeon.depth == 0 {
            return;
        }
        let roll = mix(self.seed
            ^ self.raid_id.rotate_left(7)
            ^ u64::from(self.dungeon.depth).rotate_left(41)
            ^ (self.at as u64).rotate_left(13)
            ^ salt.rotate_left(29));
        let chance = if self.mode == fortune::Mode::RuneRush {
            100
        } else {
            RUNE_CHANCE
        };
        if roll % 100 >= chance {
            return;
        }
        let kind = RuneKind::ALL[(roll / 100 % RuneKind::ALL.len() as u64) as usize];
        let Some((x, y)) = rune_spot(self.room(), roll) else {
            return;
        };
        self.rune = Some(Rune {
            kind,
            x,
            y,
            at: self.tick + u64::from(RUNE_AFTER),
        });
    }

    /// Each tick: a rune wells up and is taken; the knights' runes run down.
    pub(super) fn tick_runes(&mut self) {
        // Rune Rush: while the fight lasts, a fresh rune wells up a while
        // after the last is taken.
        if self.mode == fortune::Mode::RuneRush
            && self.rune.is_none()
            && self.phase == Phase::Fighting
            && self.tick.is_multiple_of(u64::from(RUNE_AGAIN))
        {
            self.well_up(self.tick);
        }
        if let Some(rune) = self.rune {
            if rune.at == self.tick {
                self.cues.push("rune_up".into());
                self.sounds.push("play_card");
            }
            if self.tick >= rune.at {
                let taker = self
                    .players
                    .iter()
                    .find(|(_, h)| {
                        h.hp > 0
                            && !h.stone
                            && h.privy == 0
                            && (h.x - rune.x).hypot(h.y - rune.y) < RUNE_REACH
                    })
                    .map(|(&id, _)| id);
                if let Some(id) = taker {
                    self.rune = None;
                    self.take_rune(id, rune.kind);
                }
            }
        }
        let tick = self.tick;
        for hero in self.players.values_mut() {
            let Some(held) = hero.rune.as_mut() else {
                continue;
            };
            held.left = held.left.saturating_sub(1);
            match held.kind {
                RuneKind::Regeneration if hero.hp > 0 => {
                    if tick.is_multiple_of(MEND_EVERY) {
                        hero.hp = (hero.hp + 1).min(hero.max_hp);
                    }
                    // Whole again: the rune has done its work.
                    if hero.hp >= hero.max_hp {
                        held.left = 0;
                    }
                }
                RuneKind::Arcane if hero.hp > 0 => {
                    hero.ult_charge = (hero.ult_charge + 2).min(ULT_FULL);
                    for wait in &mut hero.spell_cooldowns {
                        *wait = wait.saturating_sub(1);
                    }
                    hero.dash_cooldown = hero.dash_cooldown.saturating_sub(1);
                }
                _ => {}
            }
            if held.left == 0 || hero.hp == 0 {
                hero.rune = None;
            }
        }
    }

    /// Knight `id` reached the rune first.
    fn take_rune(&mut self, id: u32, kind: RuneKind) {
        let depth = self.dungeon.depth;
        match kind {
            RuneKind::Illusion => {
                if let Some(hero) = self.players.get(&id) {
                    let (hx, hy, px, py) = (hero.x, hero.y, -hero.aim_y, hero.aim_x);
                    self.phantoms.retain(|p| p.owner != id);
                    for side in [-1.0f32, 1.0] {
                        self.phantoms.push(Phantom {
                            owner: id,
                            x: hx + px * side * 2.6,
                            y: hy + py * side * 2.6,
                            dx: px * side * 2.6,
                            dy: py * side * 2.6,
                            left: ults::PHANTOM_TICKS,
                        });
                    }
                }
            }
            RuneKind::Bounty => {
                let gold = BOUNTY_GOLD + BOUNTY_PER_FLOOR * depth;
                for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                    hero.carried.add(Spoil::Gold, gold);
                }
                self.sounds.push("spoil_pickup");
            }
            RuneKind::Wisdom => {
                *self.marks.entry("xp".into()).or_default() += WISDOM_XP + WISDOM_PER_FLOOR * depth;
            }
            _ => {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.rune = Some(Held {
                        kind,
                        left: kind.lasts(),
                    });
                }
            }
        }
        self.cues.push(format!("rune:{}", kind.word()));
        self.found = Some((self.tick, id, format!("{}!", kind.name())));
        self.sounds.push("card_pickup");
        self.thrill(15);
        self.runes_taken += 1;
        if self.runes_taken >= RUNNER {
            self.notice("rune_runner");
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_runes__tests.rs"]
mod tests;
