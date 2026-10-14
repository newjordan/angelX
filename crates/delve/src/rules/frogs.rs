//! Frogs, now and then.
//!
//! About one room in five a frog is hopping about it. A knight who comes
//! near is boarded: it leaps onto their shoulder and rides there, through
//! doors and fights, until that knight reaches the next Sanctuary. There it
//! hops down and, in a puff of smoke, is somebody else, rolled at random:
//!
//! - **a princess**, whose blessing mends the whole party fully and lends
//!   every knight ten more health for the rest of the floor;
//! - **a prince**, whose blessing makes every knight's shots a quarter
//!   harder and a fifth quicker for the rest of the floor;
//! - **a clown**, whose curse takes a quarter of every knight's health, the
//!   most they can have and what they have, until they leave the floor.
//!
//! One frog to a knight. A knight who falls drops theirs. The hexer's frogs
//! (`hexer`) are knights turned into frogs, nothing to do with these.

use super::*;

/// One room in this many holds a frog.
pub const ONE_IN: u64 = 5;
/// How near a knight must come for a frog to leap, and how long the leap is.
pub const BOARD_REACH: f32 = 2.8;
pub const LEAP_TICKS: u32 = 9;
/// How long a hop lasts, and the rest between hops.
pub const HOP_TICKS: u32 = 10;
const REST_MIN: u32 = 36;
/// The puff each way, and how long the new figure stays in the Sanctuary.
pub const PUFF: u64 = 20;
pub const LINGER: u64 = 6 * HZ as u64;
/// What each gift is worth, in percent (and the princess's health).
pub const CLOWN_CUT: u32 = 25;
pub const PRINCESS_HEALTH: u32 = 10;
pub const PRINCE_DAMAGE: u32 = 25;
pub const PRINCE_RATE: u32 = 20;
/// Roughly one in this many frogs turns out a clown (one in three).
const CLOWN_ONE_IN: u32 = 3;

/// Who the frog was.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gift {
    Princess,
    Prince,
    Clown,
}

impl Gift {
    /// The herald's line when they appear.
    pub fn herald(self) -> &'static str {
        match self {
            Gift::Princess => {
                "A princess! The party is mended, and every knight is ten stronger for this floor."
            }
            Gift::Prince => {
                "A prince! Every shot flies 25% harder and 20% faster for this floor."
            }
            Gift::Clown => "A clown! Your whole party feels 25% worse.",
        }
    }

    /// The short plate over them.
    pub fn plate(self) -> &'static str {
        match self {
            Gift::Princess => "PRINCESS: FULL MEND, +10 MAX HP",
            Gift::Prince => "PRINCE: +25% DAMAGE, +20% FIRE RATE",
            Gift::Clown => "CLOWN: -25% HEALTH THIS FLOOR",
        }
    }

    /// What the HUD says for the rest of the floor.
    pub fn perk(self) -> &'static str {
        match self {
            Gift::Princess => "the princess's blessing: +10 max health this floor",
            Gift::Prince => "the prince's blessing: +25% damage, +20% fire rate this floor",
            Gift::Clown => "the clown's curse: -25% health this floor",
        }
    }
}

/// What a knight carries from a frog's gift until the floor is left.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Boon {
    /// Percent added to their damage, and to their rate of fire.
    pub damage: u32,
    pub rate: u32,
    /// Health added to their most (the princess's), and taken from it (the
    /// clown's), both given back when the floor is left.
    pub plus: u32,
    pub cut: u32,
    /// Whose gift it is, for the HUD.
    pub from: Option<Gift>,
}

impl Boon {
    pub fn any(&self) -> bool {
        *self != Boon::default()
    }
}

/// A frog about the room.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wild {
    pub x: f32,
    pub y: f32,
    /// Where the hop began and ends, and ticks left in it.
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub hop: u32,
    /// Ticks until the next hop.
    pub rest: u32,
    /// Leaping at a knight: who, and ticks left.
    pub leap: Option<(u32, u32)>,
    /// Which way it faces: 1 or -1.
    pub face: i8,
}

/// A figure standing in a Sanctuary where a frog was.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shown {
    pub gift: Gift,
    pub x: f32,
    pub y: f32,
    pub born: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Field {
    pub wild: Vec<Wild>,
    pub shown: Vec<Shown>,
}

impl Wild {
    /// Where the frog is drawn: along its hop, lifted by it.
    pub fn lift(&self) -> f32 {
        if let Some((_, left)) = self.leap {
            let k = 1.0 - left as f32 / LEAP_TICKS as f32;
            return (k * std::f32::consts::PI).sin() * 1.1;
        }
        if self.hop == 0 {
            return 0.0;
        }
        let k = 1.0 - self.hop as f32 / HOP_TICKS as f32;
        (k * std::f32::consts::PI).sin() * 0.7
    }
}


impl Run {
    /// A roll of `n` sides that never touches the run's own generator: the
    /// frogs come and go without moving anything else's dice.
    fn chance_of(&self, salt: u64, n: u64) -> u64 {
        mix(self.seed
            ^ self.raid_id.rotate_left(23)
            ^ self.tick.wrapping_mul(0x2545_f491_4f6c_dd1d)
            ^ salt.wrapping_mul(0x9e37_79b9_7f4a_7c15))
            % n.max(1)
    }

    /// Entering a room for the first time: about one in five has a frog.
    pub(super) fn maybe_frog(&mut self, index: usize, first_visit: bool) {
        if !first_visit || self.side_on() || self.dungeon.depth == 0 {
            return;
        }
        let room = &self.dungeon.rooms[index];
        if !matches!(
            room.kind,
            RoomKind::Fight | RoomKind::Hall | RoomKind::Treasure | RoomKind::Start | RoomKind::Stairs
        ) {
            return;
        }
        let h = mix(
            self.seed
                ^ self.raid_id.rotate_left(11)
                ^ u64::from(self.dungeon.depth) * 0x9e37
                ^ index as u64 * 0x85eb,
        );
        if h % ONE_IN != 0 {
            return;
        }
        self.drop_frog();
    }

    /// A frog on open floor, well away from the knights.
    pub(super) fn drop_frog(&mut self) {
        let room = &self.dungeon.rooms[self.at];
        let mut spot = None;
        for attempt in 0..48u64 {
            let col = 2 + self.chance_of(attempt * 2, room.cols as u64 - 4) as i32;
            let row = 2 + self.chance_of(attempt * 2 + 1, room.rows as u64 - 4) as i32;
            let (x, y) = (
                (col as f32 + 0.5) * TILE_UNITS,
                (row as f32 + 0.5) * TILE_UNITS,
            );
            let far = self
                .players
                .values()
                .all(|h| (h.x - x).hypot(h.y - y) > 9.0 - attempt as f32 / 6.0);
            if room.tile(col, row) == Tile::Floor && far {
                spot = Some((x, y));
                break;
            }
        }
        if let Some((x, y)) = spot {
            self.frogs.wild.push(Wild {
                x,
                y,
                from: (x, y),
                to: (x, y),
                hop: 0,
                rest: 20,
                leap: None,
                face: 1,
            });
        }
    }

    /// Once a tick: frogs hop and leap; a fallen carrier drops theirs; the
    /// figures in a Sanctuary take their leave.
    pub(super) fn tick_frogs(&mut self) {
        let tick = self.tick;
        self.frogs
            .shown
            .retain(|s| tick.saturating_sub(s.born) < LINGER + PUFF);
        // A fallen knight drops their frog where they fell.
        let mut dropped = Vec::new();
        for hero in self.players.values_mut() {
            if hero.rider && hero.hp == 0 {
                hero.rider = false;
                dropped.push((hero.x, hero.y));
            }
        }
        for (x, y) in dropped {
            self.frogs.wild.push(Wild {
                x,
                y,
                from: (x, y),
                to: (x, y),
                hop: 0,
                rest: 30,
                leap: None,
                face: 1,
            });
        }
        if self.frogs.wild.is_empty() {
            return;
        }
        let grid_room = self.dungeon.rooms[self.at].clone();
        let grid = Grid {
            room: &grid_room,
            barred: true,
        };
        // Who is already spoken for by a leaping frog.
        let spoken: Vec<u32> = self
            .frogs
            .wild
            .iter()
            .filter_map(|f| f.leap.map(|l| l.0))
            .collect();
        let standing: Vec<(u32, f32, f32, bool)> = self
            .players
            .iter()
            .map(|(&id, h)| {
                (
                    id,
                    h.x,
                    h.y,
                    h.hp > 0 && !h.stone && !h.rider && h.pit.stop.is_none() && h.privy == 0,
                )
            })
            .collect();
        let mut boarded = Vec::new();
        let salt = self.seed ^ tick.wrapping_mul(0x2545_f491_4f6c_dd1d);
        for (n, frog) in self.frogs.wild.iter_mut().enumerate() {
            if let Some((id, left)) = frog.leap {
                let Some(&(_, hx, hy, free)) = standing.iter().find(|s| s.0 == id) else {
                    frog.leap = None;
                    continue;
                };
                if !free {
                    frog.leap = None;
                    continue;
                }
                let steps = left.max(1) as f32;
                frog.x += (hx - frog.x) / steps;
                frog.y += (hy - frog.y) / steps;
                frog.face = if hx < frog.x { -1 } else { 1 };
                frog.leap = Some((id, left - 1));
                if left <= 1 {
                    boarded.push((n, id));
                }
                continue;
            }
            // A knight near enough, free of a frog and not already spoken for.
            if let Some(&(id, ..)) = standing
                .iter()
                .filter(|s| s.3 && !spoken.contains(&s.0))
                .filter(|s| (s.1 - frog.x).hypot(s.2 - frog.y) < BOARD_REACH)
                .min_by(|a, b| {
                    (a.1 - frog.x)
                        .hypot(a.2 - frog.y)
                        .total_cmp(&(b.1 - frog.x).hypot(b.2 - frog.y))
                })
            {
                frog.leap = Some((id, LEAP_TICKS));
                frog.hop = 0;
                continue;
            }
            if frog.hop > 0 {
                frog.hop -= 1;
                let k = 1.0 - frog.hop as f32 / HOP_TICKS as f32;
                frog.x = frog.from.0 + (frog.to.0 - frog.from.0) * k;
                frog.y = frog.from.1 + (frog.to.1 - frog.from.1) * k;
            } else if frog.rest > 0 {
                frog.rest -= 1;
            } else {
                // The next hop: a few steps any way there is floor.
                let h = mix(salt ^ (n as u64 + 1).wrapping_mul(0x9e37_79b9));
                let angle = (h % 628) as f32 / 100.0;
                let reach = 1.4 + ((h >> 12) % 18) as f32 / 10.0;
                let (tx, ty) = (frog.x + angle.cos() * reach, frog.y + angle.sin() * reach);
                if grid.clear(tx, ty, 0.4, Mover::Walker) {
                    frog.from = (frog.x, frog.y);
                    frog.to = (tx, ty);
                    frog.hop = HOP_TICKS;
                    frog.face = if tx < frog.x { -1 } else { 1 };
                }
                frog.rest = REST_MIN + ((h >> 24) % 40) as u32;
            }
        }
        for (n, id) in boarded.into_iter().rev() {
            self.frogs.wild.remove(n);
            if let Some(hero) = self.players.get_mut(&id).filter(|h| !h.rider && h.hp > 0) {
                hero.rider = true;
                self.cues.push("frog_boards".into());
                self.sounds.push("spoil_pickup");
                let name = hero.knight_name().unwrap_or_else(|| hero.name.clone());
                self.found = Some((self.tick, id, format!("A frog leaps onto {name}'s shoulder")));
            }
        }
    }

    /// A Sanctuary reached: every frog carried hops down and is someone.
    pub(super) fn frogs_arrive(&mut self) {
        let riders: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.rider && h.hp > 0)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        for (id, x, y) in riders {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.rider = false;
            }
            let gift = if self.chance_of(u64::from(id) * 7 + 1, u64::from(CLOWN_ONE_IN)) == 0 {
                Gift::Clown
            } else if self.chance_of(u64::from(id) * 7 + 2, 2) == 0 {
                Gift::Princess
            } else {
                Gift::Prince
            };
            self.give(gift);
            self.frogs.shown.push(Shown {
                gift,
                x: x + 2.4,
                y: y - 0.4,
                born: self.tick,
            });
            self.found = Some((self.tick, id, gift.herald().into()));
            self.cues.push(
                match gift {
                    Gift::Princess => "frog_princess",
                    Gift::Prince => "frog_prince",
                    Gift::Clown => "frog_clown",
                }
                .into(),
            );
            self.sounds.push("wheel_land");
        }
    }

    /// A gift to the whole party, until the floor is left.
    pub fn give(&mut self, gift: Gift) {
        for hero in self.players.values_mut() {
            match gift {
                Gift::Princess => {
                    hero.boon.plus += PRINCESS_HEALTH;
                    hero.max_hp += PRINCESS_HEALTH;
                    if hero.hp > 0 {
                        hero.hp = hero.max_hp;
                    }
                }
                Gift::Prince => {
                    hero.boon.damage = PRINCE_DAMAGE;
                    hero.boon.rate = PRINCE_RATE;
                }
                Gift::Clown => {
                    let cut = hero.max_hp * CLOWN_CUT / 100;
                    hero.max_hp = hero.max_hp.saturating_sub(cut).max(1);
                    hero.boon.cut += cut;
                    if hero.hp > 0 {
                        let hurt = hero.hp * CLOWN_CUT / 100;
                        hero.hp = hero.hp.saturating_sub(hurt).max(1).min(hero.max_hp);
                    }
                }
            }
            hero.boon.from = Some(gift);
        }
    }

    /// The floor is left: every gift is given back.
    pub(super) fn lift_gifts(&mut self) {
        for hero in self.players.values_mut() {
            if !hero.boon.any() {
                continue;
            }
            hero.max_hp = (hero.max_hp + hero.boon.cut)
                .saturating_sub(hero.boon.plus)
                .max(1);
            hero.boon = Boon::default();
            // Health grown under the clown's curse never lifts above the cap.
            hero.fit_hp();
            hero.hp = if hero.hp == 0 { 0 } else { hero.hp.clamp(1, hero.max_hp) };
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_frogs__tests.rs"]
mod tests;
