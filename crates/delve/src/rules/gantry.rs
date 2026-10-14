//! The Keg Gantry, the third folly: a nod to Donkey Kong in the Keep's
//! broken tower. On a floor with a side-on hall, that hall becomes a scaffold
//! of three girders, each with a gap at one end, the gaps alternating. A
//! drunk ogre at the top rolls kegs off the top girder; they roll to the
//! gap, drop to the girder below and roll back the other way, down to the
//! floor and out.
//!
//! Everyone climbs at once (the girders are planks: jump up through them).
//! A keg that catches a knight knocks them back to the bottom, unhurt; a keg
//! cleared in the air scores. The first knight to stand on the top girder
//! takes the pot and the crown; if the candle burns down first, the highest
//! climber does. Each knight antes a tenth of their gold, as at any folly.
use super::*;

/// How long the climb lasts, after the title card.
pub const CLIMB_TIME: u32 = 60 * HZ;
/// A keg every this many ticks, and its rolling pace (units a second).
pub const KEG_EVERY: u32 = HZ * 3 / 2;
pub const KEG_PACE: f32 = 9.0;
/// How many kegs roll at once, at most.
pub const KEGS: usize = 8;
/// Kegs jumped that make a gold to keep.
pub const JUMPS_PER_GOLD: u16 = 2;

/// The scaffold: girders (`=`) on rows 2, 5 and 8, the floor below. The
/// room's own doors are kept wherever they stand.
const SCAFFOLD: [&str; ROWS] = [
    "################################################",
    "#..............................................#",
    "#=========================================.....#",
    "#..............................................#",
    "#..............................................#",
    "#.....=========================================#",
    "#..............................................#",
    "#..............................................#",
    "#=========================================.....#",
    "#..............................................#",
    "#..............................................#",
    "################################################",
    "################################################",
    "################################################",
];
/// The top girder's row: standing on it is the summit.
pub const TOP_ROW: i32 = 2;

/// A keg, in 1/16 units so the folly stays exact and comparable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keg {
    pub x: i32,
    pub y: i32,
    /// Rolling east (1) or west (-1); falling while `drop` > 0.
    pub dir: i8,
    pub drop: i32,
    /// Knights who have jumped it (each scores it once).
    pub cleared: Vec<u32>,
}

impl Keg {
    pub fn pos(&self) -> (f32, f32) {
        (self.x as f32 / 16.0, self.y as f32 / 16.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gantry {
    pub room: usize,
    pub t: u32,
    pub kegs: Vec<Keg>,
    /// Kegs each knight has jumped, the highest girder each has reached
    /// (0 the floor, 3 the summit), and each one's ante.
    pub jumped: BTreeMap<u32, u16>,
    pub height: BTreeMap<u32, u8>,
    pub ante: BTreeMap<u32, u32>,
    pub done: bool,
}

impl Gantry {
    pub fn valid(&self) -> bool {
        self.t <= folly::TITLE + CLIMB_TIME + HZ
            && self.kegs.len() <= KEGS
            && self.jumped.len() <= MAX_PLAYERS as usize
            && self.height.values().all(|&h| h <= 3)
            && self.kegs.iter().all(|k| k.cleared.len() <= MAX_PLAYERS as usize)
    }
    pub fn left(&self) -> u32 {
        (folly::TITLE + CLIMB_TIME).saturating_sub(self.t).min(CLIMB_TIME).div_ceil(HZ)
    }
    pub fn titled(&self) -> bool {
        self.t < folly::TITLE
    }
}

/// Which girder (0 floor .. 3 summit) a body standing at `y` (units) is on.
fn girder(y: f32) -> u8 {
    let row = (y / TILE_UNITS) as i32;
    match row {
        r if r <= TOP_ROW => 3,
        r if r <= 5 => 2,
        r if r <= 8 => 1,
        _ => 0,
    }
}

impl Run {
    /// On a floor with a side-on hall, that hall is the floor's gantry.
    pub(super) fn pick_gantry(&mut self) {
        self.gantry = None;
        self.gantry_room = None;
        if self.dungeon.depth == 0 {
            return;
        }
        self.gantry_room = self
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Ledge && !r.cleared && r.cols == SCAFFOLD[0].len());
    }

    /// Into the gantry: the scaffold goes up (doors kept), the antes go in,
    /// and the ogre takes his place.
    pub(super) fn start_gantry(&mut self, index: usize) {
        let room = &mut self.dungeon.rooms[index];
        for (row, line) in SCAFFOLD.iter().enumerate() {
            for (col, b) in line.bytes().enumerate() {
                if room.tile(col as i32, row as i32) == Tile::Door {
                    continue;
                }
                room.set(
                    col,
                    row,
                    match b {
                        b'#' => Tile::Block,
                        b'=' => Tile::Ledge,
                        _ => Tile::Floor,
                    },
                );
            }
        }
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        let (ante, scores) = folly::take_antes(&mut self.players);
        self.gantry = Some(Gantry {
            room: index,
            t: 0,
            kegs: Vec::new(),
            jumped: scores.keys().map(|&id| (id, 0)).collect(),
            height: scores.keys().map(|&id| (id, 0)).collect(),
            ante,
            done: false,
        });
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// The climb's tick: kegs roll and fall, knights are caught or clear
    /// them, and the summit ends it.
    pub(super) fn tick_gantry(&mut self) {
        let Some(mut g) = self.gantry.take() else {
            return;
        };
        if g.room != self.at || g.done {
            self.gantry = Some(g);
            return;
        }
        g.t += 1;
        if g.titled() {
            self.gantry = Some(g);
            return;
        }
        let cols = self.room().cols as i32;
        // The ogre rolls one off the top girder, eastward.
        if (g.t - folly::TITLE).is_multiple_of(KEG_EVERY) && g.kegs.len() < KEGS {
            g.kegs.push(Keg {
                x: (3.0 * TILE_UNITS * 16.0) as i32,
                y: ((TOP_ROW as f32 - 0.5) * TILE_UNITS * 16.0) as i32,
                dir: 1,
                drop: 0,
                cleared: Vec::new(),
            });
            self.sounds.push("rock_land");
        }
        let step = (KEG_PACE * DT * 16.0) as i32;
        let room = &self.dungeon.rooms[self.at];
        g.kegs.retain_mut(|k| {
            let (x, y) = k.pos();
            let (col, row) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
            let under = room.tile(col, row + 1);
            let footed = matches!(under, Tile::Ledge | Tile::Block);
            if k.drop > 0 || !footed {
                // Falling: down until something under it, then roll back.
                k.y += step * 2;
                k.drop += 1;
                let (_, ny) = k.pos();
                let nrow = (ny / TILE_UNITS) as i32;
                if matches!(room.tile(col, nrow + 1), Tile::Ledge | Tile::Block) && k.drop > 4 {
                    k.y = ((nrow as f32 + 0.5) * TILE_UNITS * 16.0) as i32;
                    k.drop = 0;
                    k.dir = -k.dir;
                }
            } else {
                k.x += step * i32::from(k.dir);
            }
            let (x, _) = k.pos();
            x > 0.5 && x < (cols as f32 - 0.5) * TILE_UNITS
        });
        // Knights: caught, or clear in the air; and how high each has got.
        let start = (3.3, (10.0 + 0.5) * TILE_UNITS - ledge::FOOT);
        let mut summit = None;
        for (&id, hero) in self.players.iter_mut().filter(|(_, h)| h.hp > 0 && !h.stone) {
            for k in g.kegs.iter_mut() {
                let (kx, ky) = k.pos();
                let dx = (hero.x - kx).abs();
                if dx > 0.9 {
                    continue;
                }
                let above = ky - hero.y;
                if (0.0..0.9).contains(&above) || (hero.y - ky).abs() < 0.6 {
                    if hero.invulnerable == 0 && !k.cleared.contains(&id) {
                        // Bowled over: back to the bottom, unhurt.
                        (hero.x, hero.y) = start;
                        hero.vy = 0.0;
                        hero.invulnerable = HZ;
                        self.sounds.push("hero_hurt");
                    }
                } else if (0.9..2.4).contains(&above) && !hero.grounded && !k.cleared.contains(&id) {
                    k.cleared.push(id);
                    *g.jumped.entry(id).or_default() += 1;
                    self.sounds.push("spoil_pickup");
                }
            }
            let at = girder(hero.y + ledge::FOOT - 0.1);
            let best = g.height.entry(id).or_default();
            *best = (*best).max(at);
            if at == 3 && hero.grounded && summit.is_none() {
                summit = Some(id);
            }
        }
        if summit.is_some() || g.t >= folly::TITLE + CLIMB_TIME {
            g.done = true;
            g.kegs.clear();
            let best = summit.or_else(|| {
                g.height
                    .iter()
                    .max_by_key(|(id, h)| (**h, g.jumped.get(id).copied().unwrap_or(0), std::cmp::Reverse(**id)))
                    .map(|(&id, _)| id)
            });
            let pot: u32 = g.ante.values().sum();
            let rivals = g.height.len() > 1;
            for (&id, &n) in &g.jumped {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, u32::from(n / JUMPS_PER_GOLD));
                    if rivals {
                        hero.laurel = Some(id) == best;
                    }
                    if Some(id) == best {
                        hero.carried.add(Spoil::Gold, pot);
                    }
                }
            }
            if let Some(room) = self.dungeon.rooms.get_mut(g.room) {
                room.cleared = true;
            }
            let name = best
                .and_then(|id| self.players.get(&id))
                .map_or_else(|| "NOBODY".to_string(), |h| h.name.to_uppercase());
            self.found = Some((self.tick, 0, format!("{name} TOPPED THE GANTRY!")));
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
        self.gantry = Some(g);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_gantry__tests.rs"]
mod tests;
