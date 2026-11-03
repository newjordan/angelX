//! The wish altar and its lamp (the user, 2026-10-20, after five friends
//! wiped in the first room waiting on their wishes: "for wishes lets make
//! a wish altar with a lamp ... we need more asynchronous runners in the
//! background ready to add skills when they are done").
//!
//! A stone altar with the genie's lamp on it stands where a floor begins:
//! the first floor's start room, a Sanctuary, and an open floor's arrival
//! chamber ([`Run::wish_altar`]). A knight who holds a wish walks up and
//! rubs the lamp with E: their wish line opens on their own screen
//! ([`Lamp::rubbed`]; T still opens it anywhere). The words go to the
//! host, the scroll offers its three ways, and the knight walks off and
//! fights while the host's grant runners write the wish, many at once.
//!
//! While any wish is in the runners' hands the lamp smokes and glows
//! ([`Lamp::wishing`]); when one lands on its knight, mid-fight or not, the
//! lamp flares, everyone hears it, and the knight sees what they were given
//! ([`Lamp::landed`]). The host says when a wish goes out and comes back
//! (`lamp_sent`, `lamp_landed`, `lamp_dropped`); the run only shows it.

use super::*;
use std::collections::BTreeSet;

/// How near a knight stands to rub the lamp, in tiles.
pub const REACH: f32 = 2.4;
/// How far a knight slides along the altar's edge in one tick, the way the
/// cauldron eases them round its rim. Not a step through the stone.
const EDGE_SLIDE: f32 = 0.35;
/// Within this of a corner, a head-on walk slides round the end. The middle
/// of a face is further in, and stays solid.
const CORNER: f32 = 0.9;
/// How long the lamp flares when a grant lands, in ticks.
pub const FLARE_TICKS: u64 = 2 * HZ as u64;
/// The most wishes the lamp shows in flight, and grants remembered.
const MOST_PENDING: usize = 24;
const MOST_LANDED: usize = 6;

/// A wish the host's grant runners are writing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub knight: u32,
    pub words: String,
    pub since: u64,
}

/// The lamp rubbed: palm on brass, a breath drawn in, a magic climbing.
pub const SOUND_RUB: &str = "lamp_rub";
/// The genie gathering out of the smoke over the lamp, `GENIE_AT` ticks
/// after the rub.
pub const SOUND_GENIE: &str = "genie_appear";
/// Ticks from a rub to the genie rising out of the smoke, as the picture
/// has him (`render::overworld::arena::magic::lamp`).
pub const GENIE_AT: u64 = 14;

/// A grant that reached its knight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Landed {
    pub knight: u32,
    pub name: String,
    pub tick: u64,
}

/// The lamp: who rubbed it last, the wishes in flight, the grants landed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lamp {
    /// The tick each knight last rubbed it: a seat opens its wish line on
    /// a newer one than it has seen.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rubbed: BTreeMap<u32, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wishing: Vec<Pending>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub landed: Vec<Landed>,
    /// Knights holding E, so a held key rubs once.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    held: BTreeSet<u32>,
}

impl Lamp {
    /// Nothing has happened at the lamp: the wire and saves leave it out.
    pub fn quiet(&self) -> bool {
        self.rubbed.is_empty() && self.wishing.is_empty() && self.landed.is_empty()
    }

    /// Knight `id`'s wishes in flight, oldest first.
    pub fn pending(&self, id: u32) -> impl Iterator<Item = &Pending> {
        self.wishing.iter().filter(move |p| p.knight == id)
    }

    /// The newest grant, while it still flares at `tick`.
    pub fn flaring(&self, tick: u64) -> Option<&Landed> {
        self.landed.last().filter(|l| tick.saturating_sub(l.tick) < FLARE_TICKS)
    }
}

/// Tiles a knight cannot cross once the altar stands at column `c`, row `r`.
/// The stone's box covers the tile's centre, or leaves less than a step of
/// room, so a walk aimed at that centre clips the stone.
fn stone_blocks(c: i32, r: i32, tc: i32, tr: i32) -> bool {
    let ax = (c as f32 + 0.5) * TILE_UNITS;
    let ay = (r as f32 + 1.0) * TILE_UNITS;
    let (left, right, top, foot) = (ax - 1.5 * TILE_UNITS, ax + 1.5 * TILE_UNITS, ay - 1.4 * TILE_UNITS, ay + 0.6);
    let (cx, cy) = ((tc as f32 + 0.5) * TILE_UNITS, (tr as f32 + 0.5) * TILE_UNITS);
    if cx > left && cx < right && cy > top && cy < foot {
        return true;
    }
    // One step is about 0.43. A centre closer than that still walks into the stone.
    let gap = if cx > left && cx < right {
        if cy <= top {
            top - cy
        } else if cy >= foot {
            cy - foot
        } else {
            0.0
        }
    } else if cy > top && cy < foot {
        if cx <= left {
            left - cx
        } else if cx >= right {
            cx - right
        } else {
            0.0
        }
    } else {
        return false;
    };
    gap < 0.45
}

/// The stone at (`c`, `r`) would seal a walk the party could take: some floor
/// they could reach before is no longer reachable around the stone.
fn stone_cuts_a_passage(room: &Room, c: i32, r: i32) -> bool {
    const MARGIN: i32 = 12;
    let (c0, r0) = (c - MARGIN, r - MARGIN);
    let (c1, r1) = (c + MARGIN, r + MARGIN);
    let inside = |tc: i32, tr: i32| tc >= c0 && tr >= r0 && tc <= c1 && tr <= r1;
    let walk = |tc: i32, tr: i32| {
        inside(tc, tr)
            && tc >= 0
            && tr >= 0
            && (tc as usize) < room.cols
            && (tr as usize) < room.rows
            && matches!(room.tile(tc, tr), Tile::Floor | Tile::Door | Tile::Stairs | Tile::Bog)
    };
    // A walkable tile beside the stone, so the flood starts in its component.
    let mut seed = None;
    for tr in (r - 2)..=(r + 3) {
        for tc in (c - 2)..=(c + 2) {
            if !walk(tc, tr) || stone_blocks(c, r, tc, tr) {
                continue;
            }
            let touches = DIRS.iter().any(|&(dx, dy)| {
                let (nx, ny) = (tc + dx, tr + dy);
                walk(nx, ny) && stone_blocks(c, r, nx, ny)
            });
            if touches || seed.is_none() {
                seed = Some((tc, tr));
            }
            if touches {
                break;
            }
        }
        if seed.is_some_and(|(tc, tr)| {
            DIRS.iter().any(|&(dx, dy)| {
                let (nx, ny) = (tc + dx, tr + dy);
                walk(nx, ny) && stone_blocks(c, r, nx, ny)
            })
        }) {
            break;
        }
    }
    let Some(seed) = seed else {
        return false;
    };
    let w = (c1 - c0 + 1) as usize;
    let h = (r1 - r0 + 1) as usize;
    let idx = |tc: i32, tr: i32| ((tr - r0) as usize) * w + (tc - c0) as usize;
    let flood = |block: bool| -> Vec<bool> {
        let mut seen = vec![false; w * h];
        let mut stack = vec![seed];
        seen[idx(seed.0, seed.1)] = true;
        while let Some((x, y)) = stack.pop() {
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if !walk(nx, ny) || (block && stone_blocks(c, r, nx, ny)) {
                    continue;
                }
                let j = idx(nx, ny);
                if !seen[j] {
                    seen[j] = true;
                    stack.push((nx, ny));
                }
            }
        }
        seen
    };
    let open = flood(false);
    let around = flood(true);
    open.iter().enumerate().any(|(i, &was)| {
        if !was || around[i] {
            return false;
        }
        let tc = c0 + (i % w) as i32;
        let tr = r0 + (i / w) as i32;
        !stone_blocks(c, r, tc, tr)
    })
}

/// Tiles that lie on a shortest walk from `from` to a door, the stairs, or
/// one of `extra` (an open floor's chamber centres). One search, so a spot
/// can be refused when the stone would stand on that walk.
fn route_tiles(room: &Room, from: (f32, f32), extra: &[(f32, f32)]) -> Vec<bool> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Stairs | Tile::Door | Tile::Bog);
    let at = |c: i32, r: i32| (r * cols + c) as usize;
    let start = ((from.0 / TILE_UNITS) as i32, (from.1 / TILE_UNITS) as i32);
    let mut on = vec![false; (cols * rows) as usize];
    if !(0..cols).contains(&start.0) || !(0..rows).contains(&start.1) || !walk(room.tile(start.0, start.1)) {
        return on;
    }
    let mut back = vec![u32::MAX; on.len()];
    back[at(start.0, start.1)] = at(start.0, start.1) as u32;
    let mut queue = std::collections::VecDeque::from([start]);
    let mut goals = Vec::new();
    while let Some((x, y)) = queue.pop_front() {
        let t = room.tile(x, y);
        if (x, y) != start && matches!(t, Tile::Door | Tile::Stairs) {
            goals.push((x, y));
        }
        for (dx, dy) in DIRS {
            let next = (x + dx, y + dy);
            if (0..cols).contains(&next.0)
                && (0..rows).contains(&next.1)
                && back[at(next.0, next.1)] == u32::MAX
                && walk(room.tile(next.0, next.1))
            {
                back[at(next.0, next.1)] = at(x, y) as u32;
                queue.push_back(next);
            }
        }
    }
    for &(x, y) in extra {
        let (c, r) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
        if (0..cols).contains(&c) && (0..rows).contains(&r) && back[at(c, r)] != u32::MAX {
            goals.push((c, r));
        }
    }
    for (c, r) in goals {
        let mut k = at(c, r);
        let home = at(start.0, start.1);
        while k != home {
            on[k] = true;
            let prev = back[k];
            if prev == u32::MAX {
                break;
            }
            k = prev as usize;
        }
    }
    on
}

fn stone_stands_on_a_route(on: &[bool], cols: i32, c: i32, r: i32) -> bool {
    for tr in (r - 1)..=(r + 1) {
        for tc in (c - 1)..=(c + 1) {
            if !stone_blocks(c, r, tc, tr) {
                continue;
            }
            if tc < 0 || tr < 0 {
                continue;
            }
            let i = (tr * cols + tc) as usize;
            if i < on.len() && on[i] {
                return true;
            }
        }
    }
    false
}

/// Where the altar stands in `room`, if it has one: a clear stretch of
/// floor three tiles wide and two deep, found by a spiral out from the
/// spot that suits the room. `arrival`: an open map's, where the party
/// comes down the stairs. Never where the stone would seal a passage, and
/// not on a walk the party takes from the arrival when one is given.
pub fn spot(room: &Room, arrival: Option<(f32, f32)>) -> Option<(f32, f32)> {
    place(room, arrival, &[])
}

fn place(room: &Room, arrival: Option<(f32, f32)>, extra: &[(f32, f32)]) -> Option<(f32, f32)> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let tile = |u: f32| (u / TILE_UNITS).floor() as i32;
    let anchor = match (room.kind, arrival) {
        // An open map: beside the arrival, clear of the gear cache above it.
        (_, Some((x, y))) => (tile(x) - 6, tile(y) - 2),
        // The start room: up and to the left of the middle, off the
        // doorways and the cache at its foot.
        (RoomKind::Start, None) => (cols / 2 - 7, rows / 2 - 3),
        // A Sanctuary: west of the reforge dais in its middle.
        (RoomKind::Sanctuary, None) => (cols / 2 - 8, rows / 2),
        _ => return None,
    };
    let routes = arrival.map(|from| route_tiles(room, from, extra));
    let clear = |c: i32, r: i32| {
        (c - 1..=c + 1).all(|c| (r..=r + 1).all(|r| room.tile(c, r) == Tile::Floor))
            // Off the doorways: no door within two tiles.
            && (c - 3..=c + 3).all(|c| (r - 2..=r + 3).all(|r| room.tile(c, r) != Tile::Door))
            // The stone stays solid. A spot whose stone would seal a walk
            // the party could take is not a place to stand it.
            && !stone_cuts_a_passage(room, c, r)
    };
    let at = |c: i32, r: i32| ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 1.0) * TILE_UNITS);
    let mut aside = None;
    for ring in 0..12i32 {
        for dr in -ring..=ring {
            for dc in -ring..=ring {
                if dr.abs().max(dc.abs()) != ring {
                    continue;
                }
                let (c, r) = (anchor.0 + dc, anchor.1 + dr);
                if c < 2 || r < 2 || c > cols - 3 || r > rows - 3 || !clear(c, r) {
                    continue;
                }
                let on_route = routes.as_ref().is_some_and(|on| stone_stands_on_a_route(on, cols, c, r));
                if !on_route {
                    return Some(at(c, r));
                }
                aside.get_or_insert((c, r));
            }
        }
    }
    aside.map(|(c, r)| at(c, r))
}

/// A room that keeps a wish altar: the open floor's map, a start room, or
/// a Sanctuary. The identity start and the home floor do not.
fn keeps_an_altar(floor: &Floor, index: usize, identity: bool) -> bool {
    if identity || floor.depth == 0 || index >= floor.rooms.len() {
        return false;
    }
    let on_map = floor.open.as_ref().is_some_and(|o| o.map == index);
    on_map || matches!(floor.rooms[index].kind, RoomKind::Start | RoomKind::Sanctuary)
}

/// Choose the wish altar's spot for every room on `floor` that keeps one
/// and does not yet have it. The search is [`place`]. A room already kept
/// is left where it was stood.
pub(super) fn place_on(floor: &mut Floor, identity: bool) {
    let open = floor.open.as_ref().map(|o| {
        let centres: Vec<(f32, f32)> = o.chambers.iter().map(|ch| ch.centre()).collect();
        (o.map, o.arrival, centres)
    });
    for i in 0..floor.rooms.len() {
        let key = i as u16;
        if floor.altar_at.contains_key(&key) || !keeps_an_altar(floor, i, identity) {
            continue;
        }
        let (arrival, extra) = match &open {
            Some((map, arrival, centres)) if *map == i => (Some(*arrival), centres.as_slice()),
            _ => (None, &[][..]),
        };
        floor.altar_at.insert(key, place(&floor.rooms[i], arrival, extra));
    }
}

impl Run {
    /// Stand the wish altar in every room that has one and does not yet
    /// keep a spot. A room is chosen when it is first entered, and an old
    /// save is chosen when it is loaded.
    pub(crate) fn choose_altars(&mut self) {
        place_on(&mut self.dungeon, self.identity);
    }

    /// The room the party is in keeps an altar whose spot is not yet chosen.
    fn altar_due(&self) -> bool {
        keeps_an_altar(&self.dungeon, self.at, self.identity)
            && !self.dungeon.altar_at.contains_key(&(self.at as u16))
    }

    /// Where the wish altar stands in the room the party is in: none in a
    /// room without one, or under the identity start (whose first floor
    /// asks its own questions). Chosen once, and kept on the floor.
    pub fn wish_altar(&self) -> Option<(f32, f32)> {
        if !keeps_an_altar(&self.dungeon, self.at, self.identity) {
            return None;
        }
        if let Some(kept) = self.dungeon.altar_at.get(&(self.at as u16)) {
            return *kept;
        }
        // Not yet chosen: the same search, so a caller before the first
        // enter still sees the spot. The host chooses once and keeps it.
        let open = self.dungeon.open.as_ref().filter(|o| o.map == self.at);
        let centres: Vec<(f32, f32)> = open
            .map(|o| o.chambers.iter().map(|ch| ch.centre()).collect())
            .unwrap_or_default();
        place(self.room(), open.map(|o| o.arrival), &centres)
    }

    /// Knight `id` stands within reach of the lamp.
    pub fn near_altar(&self, id: u32) -> bool {
        let (Some((x, y)), Some(hero)) = (self.wish_altar(), self.players.get(&id)) else {
            return false;
        };
        (hero.x - x).hypot(hero.y - (y - 0.5 * TILE_UNITS)) < REACH * TILE_UNITS
    }

    /// E at the lamp: a knight who holds a wish rubs it, and their wish
    /// line opens; one who holds none is told so.
    pub(super) fn tick_wish_altar(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.altar_due() {
            self.choose_altars();
        }
        let Some((ax, ay)) = self.wish_altar() else {
            self.lamp.held.clear();
            return;
        };
        // The genie comes up out of the smoke a breath after a rub.
        if self.lamp.rubbed.values().any(|&at| self.tick.checked_sub(at) == Some(GENIE_AT)) {
            self.sounds.push(SOUND_GENIE);
        }
        // The altar is stone. A knight standing in it is set on the nearest
        // edge. One walking into a corner slides along that edge and goes
        // round; one walking straight into the middle of a face stays on the
        // near side. The stone is never a door.
        let (left, right, top, foot) = (ax - 1.5 * TILE_UNITS, ax + 1.5 * TILE_UNITS, ay - 1.4 * TILE_UNITS, ay + 0.6);
        for (&id, hero) in self.players.iter_mut() {
            if !(hero.x > left && hero.x < right && hero.y > top && hero.y < foot) {
                continue;
            }
            let input = inputs.get(&id).copied().unwrap_or_default();
            let (mx, my) = (f32::from(input.move_x), f32::from(input.move_y));
            let nearest = [
                (hero.x - left, 0),
                (right - hero.x, 1),
                (hero.y - top, 2),
                (foot - hero.y, 3),
            ]
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(3, |(_, side)| side);
            let (along, deeper, span_near, span_far) = match nearest {
                0 => (my, mx > 0.0, hero.y - top, foot - hero.y),
                1 => (my, mx < 0.0, hero.y - top, foot - hero.y),
                2 => (mx, my > 0.0, hero.x - left, right - hero.x),
                _ => (mx, my < 0.0, hero.x - left, right - hero.x),
            };
            match nearest {
                0 => hero.x = left,
                1 => hero.x = right,
                2 => hero.y = top,
                _ => hero.y = foot,
            }
            // Along the face they are walking, or toward the nearer end of a
            // corner they walked into. Not through the stone, and not off the
            // middle of a face they met head-on.
            let slide = if along != 0.0 {
                along.signum() * EDGE_SLIDE
            } else if deeper && span_near.min(span_far) < CORNER {
                if span_near < span_far { -EDGE_SLIDE } else { EDGE_SLIDE }
            } else {
                0.0
            };
            match nearest {
                0 | 1 => hero.y += slide,
                _ => hero.x += slide,
            }
        }
        let mut rubs = Vec::new();
        for (&id, hero) in &self.players {
            let down = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            if !down {
                self.lamp.held.remove(&id);
                continue;
            }
            if hero.hp == 0 || hero.absent || !self.lamp.held.insert(id) {
                continue;
            }
            if self.near_altar(id) {
                rubs.push(id);
            }
        }
        for id in rubs {
            if self.players.get(&id).is_some_and(|h| h.offer.is_some()) {
                // The scroll is already open on their view: pick from it.
                continue;
            }
            if self.can_wish(id) {
                self.lamp.rubbed.insert(id, self.tick);
                self.sounds.push(SOUND_RUB);
            } else {
                self.found = Some((self.tick, id, "NO WISH HELD: EACH FLOOR BRINGS TWO".into()));
            }
        }
    }

    /// The host handed knight `id`'s words to a grant runner: the lamp
    /// smokes until it lands or is let go.
    pub fn lamp_sent(&mut self, id: u32, words: &str) {
        // A restart teaches the same spent wish again. One smoke, not two.
        if self.lamp.wishing.iter().any(|p| p.knight == id && p.words == words) {
            return;
        }
        if self.lamp.wishing.len() >= MOST_PENDING {
            self.lamp.wishing.remove(0);
        }
        self.lamp.wishing.push(Pending { knight: id, words: words.to_string(), since: self.tick });
    }

    /// Knight `id`'s wish in those words came back granted as `name`: the
    /// lamp flares and the knight is told, wherever they are.
    pub fn lamp_landed(&mut self, id: u32, words: &str, name: &str) {
        self.lamp_dropped(id, words);
        if self.lamp.landed.len() >= MOST_LANDED {
            self.lamp.landed.remove(0);
        }
        self.lamp.landed.push(Landed { knight: id, name: name.to_string(), tick: self.tick });
    }

    /// Knight `id`'s wish in those words is no longer in flight.
    pub fn lamp_dropped(&mut self, id: u32, words: &str) {
        if let Some(at) = self.lamp.wishing.iter().position(|p| p.knight == id && p.words == words) {
            self.lamp.wishing.remove(at);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/altar__tests.rs"]
mod tests;

#[cfg(all(test, feature = "bend-cases-games-a-small"))]
#[path = "../../tests/rules/altar_bend_cases__tests.rs"]
mod altar_bend_cases_tests;
