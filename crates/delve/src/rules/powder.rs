//! The Powder Room, a folly and a nod to Bomberman: a hall of the delve
//! set out as a powder store, pillars in a grid and crates packed between
//! them. Each knight starts in a corner. The bomb key drops a powder keg at
//! your feet (two at a time); it blows two seconds later in a cross, two
//! tiles each way, stopped by pillars, and breaks the first crate in each
//! direction. Every crate broken is yours. A blast that catches a knight
//! sends them, sooty, back to their corner and costs them three crates.
//! Most crates when the minute is up (or the crates run out) takes the pot
//! and the crown.
use super::*;
use std::collections::BTreeSet;

pub const POWDER_TIME: u32 = 60 * HZ;
/// A keg's fuse, its reach in tiles, and how long its fire shows.
pub const FUSE: u32 = 2 * HZ;
pub const REACH: i32 = 2;
pub const FIRE: u32 = HZ / 3;
/// Kegs a knight may have lit at once.
pub const KEGS_EACH: usize = 2;
/// Crates a sooting costs.
pub const SOOT: u16 = 3;
/// Crates broken that make a gold to keep.
pub const CRATES_PER_GOLD: u16 = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keg {
    pub at: (i32, i32),
    pub owner: u32,
    pub fuse: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Powder {
    pub room: usize,
    pub t: u32,
    /// Crate tiles still standing (`y * cols + x`).
    pub crates: BTreeSet<u16>,
    pub kegs: Vec<Keg>,
    /// Tiles on fire and for how long more.
    pub fire: Vec<((i32, i32), u32)>,
    pub broken: BTreeMap<u32, u16>,
    pub ante: BTreeMap<u32, u32>,
    /// Each knight's corner (tiles), and who held the bomb key last tick.
    pub corners: BTreeMap<u32, (i32, i32)>,
    pub held: Vec<u32>,
    pub done: bool,
}

impl Powder {
    pub fn valid(&self) -> bool {
        self.t <= folly::TITLE + POWDER_TIME + HZ
            && self.crates.len() <= 1024
            && self.kegs.len() <= KEGS_EACH * MAX_PLAYERS as usize
            && self.fire.len() <= 256
            && self.broken.len() <= MAX_PLAYERS as usize
            && self.held.len() <= MAX_PLAYERS as usize
            && self.broken.keys().all(|id| (1..=MAX_PLAYERS).contains(id))
    }
    pub fn left(&self) -> u32 {
        (folly::TITLE + POWDER_TIME).saturating_sub(self.t).min(POWDER_TIME).div_ceil(HZ)
    }
    pub fn titled(&self) -> bool {
        self.t < folly::TITLE
    }
}

/// The pillars: every second tile in both directions, inside the walls.
pub fn pillar(x: i32, y: i32) -> bool {
    x % 2 == 0 && y % 2 == 0
}

impl Run {
    /// Into the powder store: pillars, crates, a corner each, and the antes.
    pub(super) fn start_powder(&mut self, index: usize) {
        let room = &mut self.dungeon.rooms[index];
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let corners = [(1, 1), (cols - 2, rows - 2), (cols - 2, 1), (1, rows - 2)];
        // The doors' mouths and the corners stay clear.
        let doors: Vec<(i32, i32)> = (0..rows)
            .flat_map(|y| (0..cols).map(move |x| (x, y)))
            .filter(|&(x, y)| room.tile(x, y) == Tile::Door)
            .collect();
        let clear = |x: i32, y: i32| {
            corners.iter().any(|c| (c.0 - x).abs() + (c.1 - y).abs() <= 1)
                || doors.iter().any(|d| (d.0 - x).abs() + (d.1 - y).abs() <= 2)
        };
        let mut crates = BTreeSet::new();
        for y in 1..rows - 1 {
            for x in 1..cols - 1 {
                if pillar(x, y) {
                    room.set(x as usize, y as usize, Tile::Block);
                } else if !clear(x, y) && mix(self.seed ^ ((y * cols + x) as u64) << 3) % 10 < 7 {
                    room.set(x as usize, y as usize, Tile::Block);
                    crates.insert((y * cols + x) as u16);
                } else {
                    room.set(x as usize, y as usize, Tile::Floor);
                }
            }
        }
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        let (ante, scores) = folly::take_antes(&mut self.players);
        let mut corner_of = BTreeMap::new();
        for (k, (&id, hero)) in self.players.iter_mut().filter(|(id, _)| scores.contains_key(id)).enumerate() {
            let c = corners[k % corners.len()];
            (hero.x, hero.y) = ((c.0 as f32 + 0.5) * TILE_UNITS, (c.1 as f32 + 0.5) * TILE_UNITS);
            corner_of.insert(id, c);
        }
        self.powder = Some(Powder {
            room: index,
            t: 0,
            crates,
            kegs: Vec::new(),
            fire: Vec::new(),
            broken: scores.keys().map(|&id| (id, 0)).collect(),
            ante,
            corners: corner_of,
            held: Vec::new(),
            done: false,
        });
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// The store's tick: kegs dropped and blown, crates broken, knights
    /// sooted, and the end.
    pub(super) fn tick_powder(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut p) = self.powder.take() else {
            return;
        };
        if p.room != self.at || p.done {
            self.powder = Some(p);
            return;
        }
        p.t += 1;
        // The bomb key drops kegs here; the ordinary throw waits.
        for hero in self.players.values_mut() {
            hero.bomb_cooldown = hero.bomb_cooldown.max(2);
        }
        if p.titled() {
            self.powder = Some(p);
            return;
        }
        let cols = self.room().cols as i32;
        let tile = |x: f32, y: f32| ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
        let mut held = Vec::new();
        for (&id, hero) in self.players.iter().filter(|(_, h)| h.hp > 0 && !h.stone) {
            let down = inputs.get(&id).is_some_and(|i| i.valid() && i.bomb);
            if !down {
                continue;
            }
            held.push(id);
            if p.held.contains(&id) || !p.broken.contains_key(&id) {
                continue;
            }
            let at = tile(hero.x, hero.y);
            let mine = p.kegs.iter().filter(|k| k.owner == id).count();
            if mine < KEGS_EACH && !p.kegs.iter().any(|k| k.at == at) {
                p.kegs.push(Keg { at, owner: id, fuse: FUSE });
                self.sounds.push("bomb_toss");
            }
        }
        p.held = held;
        // Fuses; a blast sets the next keg in its cross going at once.
        let mut blasts = Vec::new();
        for k in p.kegs.iter_mut() {
            k.fuse = k.fuse.saturating_sub(1);
        }
        while let Some(i) = p.kegs.iter().position(|k| k.fuse == 0) {
            let k = p.kegs.remove(i);
            let mut cross = vec![k.at];
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                for r in 1..=REACH {
                    let (x, y) = (k.at.0 + dx * r, k.at.1 + dy * r);
                    let index = (y * cols + x) as u16;
                    if p.crates.remove(&index) {
                        self.dungeon.rooms[self.at].set(x as usize, y as usize, Tile::Floor);
                        if let Some(score) = p.broken.get_mut(&k.owner) { *score += 1; }
                        cross.push((x, y));
                        break;
                    }
                    if self.room().tile(x, y) != Tile::Floor {
                        break;
                    }
                    cross.push((x, y));
                    if let Some(next) = p.kegs.iter_mut().find(|n| n.at == (x, y)) {
                        next.fuse = 0;
                    }
                }
            }
            blasts.extend(cross);
            self.sounds.push("bomb");
            self.fx.push(Fx::Shake(3));
        }
        if !blasts.is_empty() {
            self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        }
        for &at in &blasts {
            p.fire.push((at, FIRE));
        }
        for f in p.fire.iter_mut() {
            f.1 = f.1.saturating_sub(1);
        }
        p.fire.retain(|f| f.1 > 0);
        // Sooted: back to the corner, three crates the poorer.
        let burning: Vec<(i32, i32)> = p.fire.iter().map(|f| f.0).collect();
        for (&id, hero) in self.players.iter_mut().filter(|(_, h)| h.hp > 0 && !h.stone) {
            if hero.invulnerable > 0 || !burning.contains(&tile(hero.x, hero.y)) {
                continue;
            }
            if let Some(&c) = p.corners.get(&id) {
                (hero.x, hero.y) = ((c.0 as f32 + 0.5) * TILE_UNITS, (c.1 as f32 + 0.5) * TILE_UNITS);
            }
            hero.invulnerable = HZ * 3 / 2;
            let n = p.broken.entry(id).or_default();
            *n = n.saturating_sub(SOOT);
            self.sounds.push("hero_hurt");
        }
        if p.crates.is_empty() || p.t >= folly::TITLE + POWDER_TIME {
            p.done = true;
            p.kegs.clear();
            p.fire.clear();
            let pot: u32 = p.ante.values().sum();
            let best = p
                .broken
                .iter()
                .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
                .map(|(&id, _)| id);
            let rivals = p.broken.len() > 1;
            for (&id, &n) in &p.broken {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, u32::from(n / CRATES_PER_GOLD));
                    hero.bomb_cooldown = 0;
                    if rivals {
                        hero.laurel = Some(id) == best;
                    }
                    if Some(id) == best {
                        hero.carried.add(Spoil::Gold, pot);
                    }
                }
            }
            if let Some(room) = self.dungeon.rooms.get_mut(p.room) {
                room.cleared = true;
            }
            let name = best
                .and_then(|id| self.players.get(&id))
                .map_or_else(|| "NOBODY".to_string(), |h| h.name.to_uppercase());
            self.found = Some((self.tick, 0, format!("{name} BROKE THE MOST CRATES!")));
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
        self.powder = Some(p);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_powder__tests.rs"]
mod tests;
