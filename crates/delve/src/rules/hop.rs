//! Book Hop, a folly and a nod to Frogger: a hall of the Drowned Archive
//! flooded ink-dark, with lanes of floating books drifting across it, each
//! lane its own way and pace, and a dry bank at either end with a calm
//! landing in the middle. Everyone hops at once from the near bank: stand
//! on a book and it carries you; step in the ink, or ride a book into the
//! wall, and you sink back to the near bank. Each crossing to the far bank
//! scores and sends you round again. Most crossings when the minute is up
//! takes the pot and the crown; a gold kept for each one.
use super::*;

pub const HOP_TIME: u32 = 60 * HZ;
/// Each lane's pace (units a second), by lane number, cycling.
pub const PACES: [f32; 4] = [4.0, 6.5, 5.0, 8.0];
/// One book every this many tiles along a lane, and how long books are.
pub const SPACING: f32 = 6.0;
pub const BOOK: [f32; 2] = [3.0, 2.0];
/// Crossings that make a gold to keep.
pub const CROSSINGS_PER_GOLD: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hop {
    pub room: usize,
    pub t: u32,
    pub crossings: BTreeMap<u32, u16>,
    pub ante: BTreeMap<u32, u32>,
    /// Where someone sank, and how long its ripples show.
    pub splashes: Vec<(f32, f32, u32)>,
    pub done: bool,
}

impl Hop {
    pub fn valid(&self) -> bool {
        self.t <= folly::TITLE + HOP_TIME + HZ
            && self.crossings.len() <= MAX_PLAYERS as usize
            && self.crossings.keys().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.splashes.len() <= 16
    }
    pub fn left(&self) -> u32 {
        (folly::TITLE + HOP_TIME).saturating_sub(self.t).min(HOP_TIME).div_ceil(HZ)
    }
    pub fn titled(&self) -> bool {
        self.t < folly::TITLE
    }
}

/// The rows (tiles) of a hall `rows` tall: the far bank, the lanes (with
/// the calm landing between them), and the near bank.
pub fn far_bank(_rows: i32) -> i32 {
    1
}
pub fn near_bank(rows: i32) -> i32 {
    rows - 2
}
pub fn landing(rows: i32) -> i32 {
    rows / 2
}
/// The lane number of row `r`, if it is ink.
pub fn lane(rows: i32, r: i32) -> Option<usize> {
    (r > far_bank(rows) && r < near_bank(rows) && r != landing(rows)).then(|| (r - 2) as usize)
}

/// Where lane `l`'s books lie at time `t` (ticks): their left edges and
/// their length, in units, wrapping across a room `width` units wide.
pub fn books(l: usize, t: u32, width: f32) -> (Vec<f32>, f32) {
    let dir = if l % 2 == 0 { 1.0 } else { -1.0 };
    let pace = PACES[l % PACES.len()];
    let len = BOOK[l % BOOK.len()] * TILE_UNITS;
    let gap = SPACING * TILE_UNITS;
    let drift = dir * pace * t as f32 / HZ as f32 + l as f32 * 2.3 * TILE_UNITS;
    let n = (width / gap).floor().max(1.0) as usize;
    let span = n as f32 * gap;
    ((0..n).map(|k| (k as f32 * gap + drift).rem_euclid(span)).collect(), len)
}

/// Whether a body at `x` stands on one of lane `l`'s books at `t`.
pub fn afloat(l: usize, t: u32, width: f32, x: f32) -> bool {
    let (lefts, len) = books(l, t, width);
    let n = lefts.len() as f32;
    let span = n * SPACING * TILE_UNITS;
    lefts.iter().any(|&b| (x - b).rem_euclid(span) < len)
}

impl Run {
    /// Into the archive: the hall floods, the antes go in, and everyone
    /// lines up on the near bank.
    pub(super) fn start_book_hop(&mut self, index: usize) {
        let room = &mut self.dungeon.rooms[index];
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        for y in 1..rows - 1 {
            for x in 1..cols - 1 {
                room.set(x as usize, y as usize, Tile::Floor);
            }
        }
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        let (ante, scores) = folly::take_antes(&mut self.players);
        for (k, hero) in self.players.values_mut().enumerate() {
            hero.x = (cols as f32 / 2.0 - 1.5 + k as f32) * TILE_UNITS;
            hero.y = (near_bank(rows) as f32 + 0.5) * TILE_UNITS;
        }
        self.hop = Some(Hop {
            room: index,
            t: 0,
            crossings: scores.keys().map(|&id| (id, 0)).collect(),
            ante,
            splashes: Vec::new(),
            done: false,
        });
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// The hop's tick: books carry their riders, the ink takes the rest,
    /// the far bank scores, and the minute ends it.
    pub(super) fn tick_book_hop(&mut self) {
        let Some(mut h) = self.hop.take() else {
            return;
        };
        if h.room != self.at || h.done {
            self.hop = Some(h);
            return;
        }
        h.t += 1;
        for s in h.splashes.iter_mut() {
            s.2 = s.2.saturating_sub(1);
        }
        h.splashes.retain(|s| s.2 > 0);
        if h.titled() {
            self.hop = Some(h);
            return;
        }
        let (cols, rows) = (self.room().cols as i32, self.room().rows as i32);
        let width = cols as f32 * TILE_UNITS;
        let t = h.t - folly::TITLE;
        let start_y = (near_bank(rows) as f32 + 0.5) * TILE_UNITS;
        for (&id, hero) in self.players.iter_mut().filter(|(_, x)| x.hp > 0 && !x.stone) {
            let row = (hero.y / TILE_UNITS) as i32;
            if let Some(l) = lane(rows, row) {
                let dir = if l % 2 == 0 { 1.0 } else { -1.0 };
                let afloat_now = afloat(l, t, width, hero.x);
                if afloat_now {
                    // The book carries its rider.
                    hero.x += dir * PACES[l % PACES.len()] * DT;
                }
                let walled = hero.x < 1.3 * TILE_UNITS || hero.x > (cols as f32 - 1.3) * TILE_UNITS;
                if (!afloat_now || walled) && hero.invulnerable == 0 {
                    if h.splashes.len() < 16 {
                        h.splashes.push((hero.x, hero.y, HZ / 2));
                    }
                    hero.y = start_y;
                    hero.x = hero.x.clamp(2.0 * TILE_UNITS, (cols as f32 - 2.0) * TILE_UNITS);
                    hero.invulnerable = HZ / 2;
                    self.sounds.push("step_wet");
                }
            } else if row <= far_bank(rows) {
                *h.crossings.entry(id).or_default() += 1;
                hero.y = start_y;
                self.sounds.push("spoil_pickup");
            }
        }
        if t >= HOP_TIME {
            h.done = true;
            let pot: u32 = h.ante.values().sum();
            let best = h
                .crossings
                .iter()
                .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
                .map(|(&id, _)| id);
            let rivals = h.crossings.len() > 1;
            for (&id, &n) in &h.crossings {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, u32::from(n / CROSSINGS_PER_GOLD));
                    if rivals {
                        hero.laurel = Some(id) == best;
                    }
                    if Some(id) == best {
                        hero.carried.add(Spoil::Gold, pot);
                    }
                }
            }
            if let Some(room) = self.dungeon.rooms.get_mut(h.room) {
                room.cleared = true;
            }
            let name = best
                .and_then(|id| self.players.get(&id))
                .map_or_else(|| "NOBODY".to_string(), |x| x.name.to_uppercase());
            self.found = Some((self.tick, 0, format!("{name} CROSSED THE MOST!")));
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
        self.hop = Some(h);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_hop__tests.rs"]
mod tests;
