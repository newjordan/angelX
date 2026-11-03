//! The errands a village asks, and the floors below it that keep them.
//!
//! A villager's errand is the shape of the chapter under their village:
//! the nine floors below it lay its signs (`Sign`) as each is reached, and
//! the tracker counts them. Four kinds:
//!
//! - **Beacons**: a cold lamp on every other floor; E beside it lights it.
//! - **Gather**: a relic on every floor; walking over it picks it up.
//! - **Rescue**: one of the village's own, lost a few floors down; E
//!   beside them sends them home up the stair.
//! - **Slay**: so many of one kind of monster felled on the chapter's floors.
//!
//! An errand finished pays the party on the spot, from the purse its floor
//! keeps (`deep::purse`), and its village remembers the deed: the realm
//! keeps it (`Town::deeds`) and the village shows it when the party next
//! comes. Nothing here draws from the run's dice: a floor's signs are laid
//! where its own rooms and number say.

use super::*;
use crate::realm::Spoil;

/// The most signs laid on one floor.
pub const MOST_SIGNS: usize = 16;
/// How near (tiles) a knight stands to light a lamp, pick up a relic or
/// speak to the lost.
pub const REACH: f32 = 1.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Errand {
    Beacons,
    Gather,
    Rescue,
    Slay(EnemyKind),
}

impl Errand {
    /// The errand's sign, as the art names it: the lamp, the relic, the
    /// lost one (none for a hunt).
    pub fn sign(self) -> Option<SignKind> {
        match self {
            Errand::Beacons => Some(SignKind::Lamp),
            Errand::Gather => Some(SignKind::Relic),
            Errand::Rescue => Some(SignKind::Lost),
            Errand::Slay(_) => None,
        }
    }

    /// Whether its sign is laid on the `k`th floor of the chapter (1 to 9).
    fn laid_on(self, k: u32) -> bool {
        match self {
            Errand::Beacons => k % 2 == 1,
            Errand::Gather => true,
            // The lost wander: first on the fourth floor, then the sixth
            // and the eighth for a party that missed them.
            Errand::Rescue => matches!(k, 4 | 6 | 8),
            Errand::Slay(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignKind {
    Lamp,
    Relic,
    Lost,
}

/// An errand a village asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mission {
    pub errand: Errand,
    /// The village that asked: its floor.
    pub village: u32,
    pub culture: Culture,
    /// Who asked, by their place among the six who speak.
    pub giver: u8,
    pub have: u16,
    pub need: u16,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paid: bool,
}

impl Mission {
    pub fn done(&self) -> bool {
        self.have >= self.need
    }

    /// The tracker's line: what, and how far along.
    pub fn line(&self) -> String {
        let (lamp, relic, lost) = self.culture.sign_words();
        let what = match self.errand {
            Errand::Beacons => format!("LIGHT {lamp}S"),
            Errand::Gather => format!("GATHER {relic}S"),
            Errand::Rescue => format!("FIND THE {lost}"),
            Errand::Slay(kind) => format!("FELL {}", foe_name(kind)),
        };
        format!("{what} {}/{}", self.have.min(self.need), self.need)
    }
}

/// A hunted kind, as a tracker says it.
pub fn foe_name(kind: EnemyKind) -> String {
    let name = format!("{kind:?}").to_uppercase();
    if name.ends_with('S') { name } else { format!("{name}S") }
}

/// One of an errand's signs on this floor: where it stands, and whether
/// it is done.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sign {
    pub kind: SignKind,
    /// The errand it counts for (an index into the realm's missions).
    pub mission: u8,
    pub room: usize,
    /// Its tile in that room.
    pub col: u8,
    pub row: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub done: bool,
    /// The tick it was done, for its flare.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
    pub culture: Culture,
}

impl Sign {
    /// Its point in arena units: the middle of its tile.
    pub fn point(&self) -> (f32, f32) {
        ((f32::from(self.col) + 0.5) * TILE_UNITS, (f32::from(self.row) + 0.5) * TILE_UNITS)
    }
}

/// Where on `floor` a sign may stand, in an order of the floor's own: a
/// room with a fight or a passage in it (never the entrance, the
/// guardian's hall or a side room), as the room and a tile near the
/// middle of it to search out from; on an open floor, its map, near the
/// middle of each chamber but the arrival's and the guardian's.
fn hosts(floor: &Floor, salt: u64) -> Vec<(usize, (i32, i32))> {
    let mut places: Vec<(usize, (i32, i32))> = match &floor.open {
        Some(open) => open
            .chambers
            .iter()
            .enumerate()
            .filter(|&(i, c)| i != usize::from(open.guardian) && c.dist > 0 && !matches!(c.kind, RoomKind::Stairs | RoomKind::Lair))
            .map(|(_, c)| {
                let (x, y, w, h) = (i32::from(c.rect.0), i32::from(c.rect.1), i32::from(c.rect.2), i32::from(c.rect.3));
                (open.map, (x + w / 2, y + h / 2))
            })
            .collect(),
        None => (1..floor.rooms.len())
            .filter(|&i| matches!(floor.rooms[i].kind, RoomKind::Fight | RoomKind::Hall | RoomKind::Treasure))
            .map(|i| (i, (floor.rooms[i].cols as i32 / 2, floor.rooms[i].rows as i32 / 2)))
            .collect(),
    };
    places.sort_by_key(|&(i, (c, r))| mix(salt ^ (i as u64).wrapping_mul(0x9e37_79b9) ^ ((c as u64) << 20 | r as u64)));
    places
}

/// A clear tile in `room` for a sign near `(cx, cy)`: the first floor tile
/// out along a spiral from a point of the salt's own beside it.
fn spot(room: &Room, (cx, cy): (i32, i32), salt: u64) -> Option<(u8, u8)> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let cx = cx + (mix(salt) % 7) as i32 - 3;
    let cy = cy + (mix(salt ^ 0x51) % 5) as i32 - 2;
    let clear = |c: i32, r: i32| {
        (2..cols - 2).contains(&c)
            && (2..rows - 2).contains(&r)
            && c <= i32::from(u8::MAX)
            && r <= i32::from(u8::MAX)
            && [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dy)| room.tile(c + dx, r + dy) == Tile::Floor)
    };
    for ring in 0..8 {
        for dy in -ring..=ring {
            for dx in -ring..=ring {
                if (dx as i32).abs().max((dy as i32).abs()) != ring {
                    continue;
                }
                let (c, r) = (cx + dx, cy + dy);
                if clear(c, r) {
                    return Some((c as u8, r as u8));
                }
            }
        }
    }
    None
}

impl Run {
    /// Lay the open errands' signs on floor `floor`, as its chapter asks.
    pub(super) fn lay_signs(&mut self, floor: u32) {
        let Some(village) = errands_from(floor) else {
            return;
        };
        let k = floor - village;
        let salt = self.raid_id ^ u64::from(floor).wrapping_mul(0x2545_f491_4f6c_dd1d);
        let rooms = hosts(&self.dungeon, salt);
        let mut signs = Vec::new();
        let open: Vec<(usize, Mission)> = self.home.deep.open_on(floor).map(|(i, m)| (i, m.clone())).collect();
        for (n, (index, mission)) in open.into_iter().enumerate() {
            let Some(kind) = mission.errand.sign() else {
                continue;
            };
            if !mission.errand.laid_on(k) || rooms.is_empty() {
                continue;
            }
            let (room, near) = rooms[(n + signs.len()) % rooms.len()];
            let Some((col, row)) = spot(&self.dungeon.rooms[room], near, salt ^ (n as u64 + 1) * 0x9e37) else {
                continue;
            };
            if signs.len() < MOST_SIGNS {
                signs.push(Sign { kind, mission: index as u8, room, col, row, done: false, at: None, culture: mission.culture });
            }
        }
        if let Some(d) = &mut self.deep {
            d.signs = signs;
        }
    }

    /// One tick of the signs in the room the party stands in: a relic
    /// walked over, a lamp lit or the lost spoken to with E.
    pub(super) fn tick_signs(&mut self, deep: &mut Descent, pressed: u8) {
        let reach = REACH * TILE_UNITS;
        let mut done = Vec::new();
        for (i, sign) in deep.signs.iter().enumerate() {
            if sign.done || sign.room != self.at {
                continue;
            }
            let (sx, sy) = sign.point();
            let by = self.players.iter().find(|(id, h)| {
                h.hp > 0
                    && !h.stone
                    && (h.x - sx).hypot(h.y - sy) < reach
                    && (sign.kind == SignKind::Relic || super::super::nooks::has(pressed, **id))
            });
            if let Some((&id, _)) = by {
                done.push((i, id));
            }
        }
        for (i, id) in done {
            let sign = &mut deep.signs[i];
            sign.done = true;
            sign.at = Some(self.tick);
            let (kind, mission) = (sign.kind, usize::from(sign.mission));
            let (lamp, relic, lost) = sign.culture.sign_words();
            let said = match kind {
                SignKind::Lamp => {
                    self.sounds.push("wheel_land");
                    format!("THE {lamp} BURNS AGAIN.")
                }
                SignKind::Relic => {
                    self.sounds.push("spoil_pickup");
                    format!("A {relic}, FOR THE VILLAGE ABOVE.")
                }
                SignKind::Lost => {
                    self.sounds.push("wish_grant");
                    format!("THE {lost} IS FOUND, AND GOES HOME UP THE STAIR.")
                }
            };
            self.found = Some((self.tick, id, said));
            self.count_errand(mission, 1, deep.floor);
        }
    }

    /// A monster of `kind` felled: a hunt asked for it counts it.
    pub(in crate::rules) fn deep_kill(&mut self, kind: EnemyKind) {
        let Some(floor) = self.deep_floor() else {
            return;
        };
        let hunts: Vec<usize> = self
            .home
            .deep
            .open_on(floor)
            .filter(|(_, m)| m.errand == Errand::Slay(kind))
            .map(|(i, _)| i)
            .collect();
        for i in hunts {
            self.count_errand(i, 1, floor);
        }
    }

    /// Errand `index` gains `n`; finished, it pays the party and its
    /// village remembers.
    fn count_errand(&mut self, index: usize, n: u16, floor: u32) {
        let Some(m) = self.home.deep.missions.get_mut(index) else {
            return;
        };
        m.have = m.have.saturating_add(n).min(m.need);
        if !m.done() || m.paid {
            return;
        }
        m.paid = true;
        let (errand, village, culture) = (m.errand, m.village, m.culture);
        if let Some(town) = self.home.deep.town_mut(village)
            && town.deeds.len() < 16
        {
            town.deeds.push(errand);
        }
        let gold = purse(floor);
        for hero in self.players.values_mut().filter(|h| h.hp > 0) {
            hero.carried.add(Spoil::Gold, gold);
            hero.carried.add(Spoil::Gem, 1 + chapter(floor));
        }
        self.sounds.push("wish_grant");
        self.cues.push("deep_errand".into());
        self.shake = self.shake.max(6);
        self.found = Some((
            self.tick,
            0,
            format!("ERRAND DONE FOR {} OF {}: {gold} GOLD EACH. THEY WILL HEAR OF IT.", culture.people(), culture.town(chapter(village))),
        ));
    }
}
