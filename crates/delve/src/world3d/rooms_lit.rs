//! Rooms, lit (docs/REALM-MONOLITH.md, "Rooms, lit"): every room the game
//! draws from above stands up in 3D in the monolith look, with no plate.
//!
//! - **What stands.** The room's own blockout (`blockout::blockout`, the
//!   generator the painted items were made from, run over the room as the
//!   dice laid it, with a place's hand-placed landmark), its masses made
//!   monolithic (`rooms_lit_mass`), so its walls stand on its wall tiles,
//!   its doorways are where its doors are, and nothing stands on ground a
//!   knight walks.
//! - **Its ground** is one height field (`ground` in the set) a quarter of
//!   a tile apart, at the floor line wherever a knight walks, sunk where the
//!   floor is a hazard (a pit deep and black, water, lava and spores under
//!   their surfaces), each corner toned by what the floor is (flags, earth,
//!   a road, mire), darker at the foot of what stands.
//! - **Its lamps** are where something burns (`lamp` in the set: torches by
//!   the doors, braziers, hearths, lava, and out of doors lit windows); its
//!   flames are not the set's but drawn each frame beside it
//!   ([`Piece::flames`], `# flame` lines of its text), so they burn full
//!   while a fight is on and sink to embers once it is won. Lava and a
//!   hearth's bed are a deep ember crust cracked with fire, and water a dark
//!   pool that runs and glints, drawn by the frame too.
//! - **Toned, lit, no plate.** Every surface is its kind's tone, lit by the
//!   room's look (`rooms_lit_look`): one dark, nearly monotone ramp for its
//!   kind of place.
//! - **An open floor's map** stands up in pieces of a kingdom chunk's size
//!   ([`PIECE`]), each the blockout of its tiles and a tile of the map round
//!   them, cut back to its own tiles, its own set; a page makes the pieces near its
//!   view itself, from the run's tiles. Having no doors, it is lit from its
//!   passages' mouths and by braziers on its walls and what stands in it.
//! - **Across a wire** a one-screen room's set goes in the run's tree (its
//!   key and text, no plate), so a page with no asset folder draws its
//!   landmark too; a page makes the same look from the run.
//!
//! The painted plates and the plain rooms in a theme's patches that came
//! before were cut on 2026-10-20 (docs/WORLD-TIERS.md).

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::look::Lamp;
use angel_stage::scene::{Material, MeshData, Node};
use angel_stage::set::{Kind, Set, Shape, ground_mesh_id};

use super::blockout::{self, Dressing, Group, Rect, Role, Setting, Solid, TILE, num};
use super::pilot::{self, Source};
use super::rooms_lit_look::{Activity, Tone};
use crate::render::overworld::Img;
use crate::rules::{Pack, Room, RoomKind, Tile};

pub const VERSION: u64 = 7;
const KEPT: usize = 8;
const PIECES_KEPT: usize = 64;
const RECEIVED_KEPT: usize = 8;

pub struct Lit {
    pub tone: Tone,
    pub key: u64,
    pub id: &'static str,
    pub grid: (i32, i32),
    room: Room,
    dressing: Dressing,
    pieces: Vec<OnceLock<Option<Arc<Piece>>>>,
    picture: Mutex<Option<((i32, i32), Arc<Img>)>>,
}

impl std::fmt::Debug for Lit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lit").field("tone", &self.tone).field("id", &self.id).field("grid", &self.grid).field("key", &self.key).finish()
    }
}

pub struct Piece {
    pub cell: (i32, i32),
    pub tiles: Rect,
    pub key: u64,
    pub text: Arc<str>,
    pub set: Set,
    pub flames: Vec<Shape>,
    pub coals: Vec<Shape>,
    pub lava: Vec<Shape>,
    pub water: Vec<Shape>,
    pub steady: Vec<bool>,
    pub bounds: ([f32; 3], [f32; 3]),
    made: OnceLock<(Vec<Node>, Vec<Node>, Option<Arc<MeshData>>)>,
}

impl Piece {
    pub fn nodes(&self) -> &(Vec<Node>, Vec<Node>, Option<Arc<MeshData>>) {
        self.made.get_or_init(|| {
            let (floor, blocks, mesh) = self.set.lit_world_nodes(ground_mesh_id(self.key));
            (floor, blocks, mesh.map(Arc::new))
        })
    }

    pub fn lamps(&self) -> Vec<Lamp> {
        self.set.look_lamps()
    }
}

impl Lit {
    pub fn stage(&self) -> (f32, f32) {
        (self.room.cols as f32 * TILE, self.room.rows as f32 * TILE)
    }

    pub fn open(&self) -> bool {
        self.grid != (1, 1)
    }

    pub fn room(&self) -> &Room {
        &self.room
    }

    pub fn piece(&self, (cx, cz): (i32, i32)) -> Option<Arc<Piece>> {
        if !(0..self.grid.0).contains(&cx) || !(0..self.grid.1).contains(&cz) {
            return None;
        }
        let slot = &self.pieces[(cz * self.grid.0 + cx) as usize];
        slot.get_or_init(|| self.make_piece((cx, cz))).clone()
    }

    pub fn every_piece(&self) -> Vec<Arc<Piece>> {
        (0..self.grid.1).flat_map(|cz| (0..self.grid.0).map(move |cx| (cx, cz))).filter_map(|c| self.piece(c)).collect()
    }

    pub fn near(&self, view: [f32; 4]) -> Vec<Arc<Piece>> {
        if !self.open() {
            return self.every_piece();
        }
        let (pw, ph) = (PIECE.0 as f32 * TILE, PIECE.1 as f32 * TILE);
        let (c0, c1) = ((view[0] / pw).floor() as i32, ((view[2] - 0.01) / pw).floor() as i32);
        let (r0, r1) = ((view[1] / ph).floor() as i32, ((view[3] - 0.01) / ph).floor() as i32);
        let mut budget = RING_A_CALL;
        let mut out = Vec::new();
        for cz in (r0 - 1).max(0)..=(r1 + 1).min(self.grid.1 - 1) {
            for cx in (c0 - 1).max(0)..=(c1 + 1).min(self.grid.0 - 1) {
                let shown = (c0..=c1).contains(&cx) && (r0..=r1).contains(&cz);
                let made = self.pieces[(cz * self.grid.0 + cx) as usize].get().is_some();
                if !shown && !made {
                    if budget == 0 {
                        continue;
                    }
                    budget -= 1;
                }
                out.extend(self.piece((cx, cz)));
            }
        }
        out
    }

    fn make_piece(&self, cell: (i32, i32)) -> Option<Arc<Piece>> {
        if !self.open() {
            let text = received(self.key).unwrap_or_else(|| room_text(&self.room, &self.dressing, true, None).into());
            return piece_of_text(cell, (0, 0, self.room.cols as i32, self.room.rows as i32), self.key, text);
        }
        type Kept = Mutex<VecDeque<(u64, Arc<Piece>)>>;
        static KEPT_PIECES: Kept = Mutex::new(VecDeque::new());
        let (tiles, crop) = piece_rects(&self.room, true, cell);
        let mut key = 0x9E37_79B9_7F4A_7C15 ^ VERSION;
        for &byte in self.dressing.id.as_bytes() {
            key = (key ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        if let Setting::Dungeon(pack) = self.dressing.setting {
            key = (key ^ (pack as u64 + 1)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        let mut mix = |v: u64| {
            key ^= v;
            key = key.wrapping_mul(0x0000_0100_0000_01b3);
        };
        for v in [crop.0, crop.1, crop.2, crop.3, tiles.0, tiles.1, tiles.2, tiles.3] {
            mix(v as u64);
        }
        for r in (crop.1 - super::rooms_lit_mass::CORE_REACH)..(crop.1 + crop.3 + 1) {
            for c in (crop.0 - 1)..(crop.0 + crop.2 + 1) {
                mix(self.room.tile(c, r) as u64);
            }
        }
        if let Ok(kept) = KEPT_PIECES.lock()
            && let Some((_, piece)) = kept.iter().find(|(k, _)| *k == key)
        {
            return Some(Arc::clone(piece));
        }
        let text: Arc<str> = room_text(&self.room, &self.dressing, false, Some((tiles, crop))).into();
        let piece = piece_of_text(cell, tiles, key, text)?;
        if let Ok(mut kept) = KEPT_PIECES.lock() {
            if kept.len() >= PIECES_KEPT {
                kept.pop_front();
            }
            kept.push_back((key, Arc::clone(&piece)));
        }
        Some(piece)
    }

    pub fn text(&self) -> Option<Arc<str>> {
        if self.open() {
            return None;
        }
        self.piece((0, 0)).map(|p| Arc::clone(&p.text))
    }

}

fn piece_of_text(cell: (i32, i32), tiles: Rect, key: u64, text: Arc<str>) -> Option<Arc<Piece>> {
    let set = Set::parse(&text)
        .map_err(|why| tracing::warn!("world3d: a lit room's set does not read: {:?}", why.first()))
        .ok()?;
    let (flames, coals, lava) = (lines_of(&text, &set, FLAME), lines_of(&text, &set, COALS), lines_of(&text, &set, LAVA));
    let water = lines_of(&text, &set, WATER);
    let steady = set
        .lamps
        .iter()
        .map(|l| {
            lava.iter().any(|s| {
                let ([x0, z0], [x1, z1]) = ([s.at[0] - s.size[0] / 2.0, s.at[2] - s.size[2] / 2.0], [s.at[0] + s.size[0] / 2.0, s.at[2] + s.size[2] / 2.0]);
                l.at[0] >= x0 - 0.5 && l.at[0] <= x1 + 0.5 && l.at[2] >= z0 - 0.5 && l.at[2] <= z1 + 0.5
            })
        })
        .collect();
    let bounds = bounds_of(&set, tiles);
    Some(Arc::new(Piece { cell, tiles, key, text, set, flames, coals, lava, water, steady, bounds, made: OnceLock::new() }))
}

fn lines_of(text: &str, set: &Set, prefix: &str) -> Vec<Shape> {
    let mut body = String::new();
    let (w, h) = set.camera.size();
    let _ = writeln!(body, "set drawn\ncamera room {w} {h}");
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(prefix) {
            let _ = writeln!(body, "{rest}");
        }
    }
    Set::parse(&body).map(|s| s.shapes).unwrap_or_default()
}

const FLAME: &str = "# flame ";
const COALS: &str = "# coals ";
const LAVA: &str = "# lava ";
const WATER: &str = "# water ";

pub const WATER_TONE: f32 = 0.3;

fn bounds_of(set: &Set, (c, r, w, h): Rect) -> ([f32; 3], [f32; 3]) {
    let [x0, z0, x1, z1] = blockout::px((c, r, w, h));
    let mut lo = [x0, -40.0, z0];
    let mut hi = [x1, 0.0, z1];
    for s in &set.shapes {
        let reach = s.size[0].hypot(s.size[2]) / 2.0;
        lo = [lo[0].min(s.at[0] - reach), lo[1].min(s.at[1]), lo[2].min(s.at[2] - reach)];
        hi = [hi[0].max(s.at[0] + reach), hi[1].max(s.at[1] + s.size[1]), hi[2].max(s.at[2] + reach)];
    }
    (lo, hi)
}

fn key_of(room: &Room, dressing: &Dressing) -> u64 {
    let mut key = 0xcbf2_9ce4_8422_2325u64 ^ VERSION.rotate_left(41);
    let mut mix = |v: u64| {
        key ^= v;
        key = key.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for v in [room.cell.0 as u64, room.cell.1 as u64, room.cols as u64, room.rows as u64] {
        mix(v);
    }
    for &byte in format!("{:?}", room.kind).as_bytes() {
        mix(u64::from(byte));
    }
    for door in room.doors {
        mix(u64::from(door));
    }
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            mix(room.tile(c, r) as u64);
        }
    }
    for &byte in dressing.id.as_bytes() {
        mix(u64::from(byte));
    }
    mix(match dressing.setting {
        Setting::Dungeon(pack) => pack as u64 + 1,
        _ => 0,
    });
    key
}

pub fn lit_for(room: &Room, pack: Pack) -> Option<Arc<Lit>> {
    type Kept = Mutex<VecDeque<(u64, Arc<Lit>)>>;
    static KEPT_ROOMS: Kept = Mutex::new(VecDeque::new());
    let dressing = dressing_for(room, pack)?;
    let key = key_of(room, &dressing);
    if let Ok(kept) = KEPT_ROOMS.lock()
        && let Some((_, lit)) = kept.iter().find(|(k, _)| *k == key)
    {
        return Some(Arc::clone(lit));
    }
    let open = room.cols > crate::rules::layout::MAX_COLS || room.rows > crate::rules::layout::MAX_ROWS;
    let grid = if open {
        ((room.cols as i32 + PIECE.0 - 1) / PIECE.0, (room.rows as i32 + PIECE.1 - 1) / PIECE.1)
    } else {
        (1, 1)
    };
    let lit = Arc::new(Lit {
        tone: Tone::of(room.kind, dressing.id, dressing.setting),
        key,
        id: dressing.id,
        grid,
        room: room.clone(),
        dressing,
        pieces: (0..grid.0 * grid.1).map(|_| OnceLock::new()).collect(),
        picture: Mutex::new(None),
    });
    if let Ok(mut kept) = KEPT_ROOMS.lock() {
        if kept.len() >= KEPT {
            kept.pop_front();
        }
        kept.push_back((key, Arc::clone(&lit)));
    }
    Some(lit)
}

pub const DEEP_LAMP_REACH: f32 = 0.95;

fn received_texts() -> &'static Mutex<VecDeque<(u64, Arc<str>)>> {
    static RECEIVED: Mutex<VecDeque<(u64, Arc<str>)>> = Mutex::new(VecDeque::new());
    &RECEIVED
}

pub fn receive(key: u64, text: &str) -> bool {
    if Set::parse(text).is_err() {
        return false;
    }
    if let Ok(mut kept) = received_texts().lock() {
        if !kept.iter().any(|(k, _)| *k == key) {
            if kept.len() >= RECEIVED_KEPT {
                kept.pop_front();
            }
            kept.push_back((key, text.into()));
        }
    }
    true
}

fn received(key: u64) -> Option<Arc<str>> {
    received_texts().lock().ok()?.iter().find(|(k, _)| *k == key).map(|(_, t)| Arc::clone(t))
}

fn capped(s: &Solid, dungeon: bool) -> String {
    let line = blockout::line(s);
    if !dungeon || s.shape != blockout::Shape::Box || s.size[1] < CAP_HEIGHT {
        return line;
    }
    if !matches!(s.kind, Kind::Stone | Kind::Brick | Kind::Plaster | Kind::Slate | Kind::Rock | Kind::Wood | Kind::Metal | Kind::Bone) {
        return line;
    }
    let cap = (s.kind.tone() * CAP_LIFT * 100.0).round().clamp(0.0, 100.0) as u8;
    match line.split_once("  # ") {
        Some((body, note)) => format!("{body} top {cap}  # {note}"),
        None => format!("{line} top {cap}"),
    }
}

const DEEP_TONES: [(&str, u8); 3] = [("foliage", 24), ("trunk", 20), ("hedge", 22)];

const CAP_HEIGHT: f32 = 5.0;
const CAP_LIFT: f32 = 1.9;

fn landmark_of(room: &Room, dressing: &Dressing) -> Option<String> {
    let item = super::pilot::all().iter().find(|i| i.id() == dressing.id)?;
    if !matches!(item.source, super::pilot::Source::Place(_)) || !super::fit::same_tiles(&item.room(), room) {
        return None;
    }
    blockout::landmark(dressing.id)
}

pub fn room_text(room: &Room, dressing: &Dressing, landmark: bool, piece: Option<(Rect, Rect)>) -> String {
    let (tiles, crop) = piece.unwrap_or(((0, 0, room.cols as i32, room.rows as i32), (0, 0, room.cols as i32, room.rows as i32)));
    let cut = if piece.is_some() { crop_room(room, crop) } else { room.clone() };
    let mut groups = blockout::blockout(&cut, dressing);
    if landmark
        && let Some(text) = landmark_of(room, dressing)
    {
        match blockout::landmark_solids(&text) {
            Ok(solids) => groups.push(Group::new(Role::Prop, "landmark", solids)),
            Err(why) => tracing::warn!("world3d: {}'s landmark is refused: {:?}", dressing.id, why.first()),
        }
    }
    super::rooms_lit_mass::monolith_in(&mut groups, &cut, dressing, room, (crop.0, crop.1));
    if piece.is_some() {
        let braziers = open_braziers(room, dressing, tiles, crop, &groups);
        if !braziers.is_empty() {
            groups.push(Group::new(Role::Prop, "braziers on an open floor's walls", braziers));
        }
    }
    let dungeon = matches!(dressing.setting, Setting::Dungeon(_));
    let origin = [crop.0 as f32 * TILE, crop.1 as f32 * TILE];
    let keep = blockout::px((tiles.0 - crop.0, tiles.1 - crop.1, tiles.2, tiles.3));
    let (mut shapes, mut flames, mut coals, mut lava, mut water) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for g in &groups {
        if g.role == Role::Ground {
            continue;
        }
        let comment = format!("# {}", g.name);
        let mut first = true;
        for s in &g.solids {
            let s = if piece.is_some() {
                let Some(mut cut) = cut_solid(s, keep) else { continue };
                cut.at[0] += origin[0];
                cut.at[2] += origin[1];
                cut
            } else {
                s.clone()
            };
            if s.kind == Kind::Water {
                water.push(s);
            } else if s.kind == Kind::Fire && g.role == Role::Hazard {
                lava.push(s);
            } else if s.kind == Kind::Fire && s.shape == blockout::Shape::Box && s.size[0].max(s.size[2]) >= 10.0 && s.size[1] <= 4.0 {
                coals.push(s);
            } else if s.kind == Kind::Fire {
                flames.push(s);
            } else {
                if first {
                    shapes.push(comment.clone());
                    first = false;
                }
                shapes.push(capped(&s, dungeon));
            }
        }
    }
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let mut out = String::new();
    let _ = writeln!(out, "set {}", if dressing.id.is_empty() { "lit" } else { dressing.id });
    let _ = writeln!(out, "# {} lit (docs/REALM-MONOLITH.md, \"Rooms, lit\"): made by angel-delve world3d::rooms_lit", dressing.title);
    let _ = writeln!(out, "# from the room as the game lays it, tiles {},{} {}x{}.", tiles.0, tiles.1, tiles.2, tiles.3);
    let _ = writeln!(out, "camera room {} {}", num(w), num(h));
    if dungeon {
        for (kind, tone) in DEEP_TONES {
            let _ = writeln!(out, "tone {kind} {tone}");
        }
    }
    for line in shapes {
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "\n# what burns, drawn by the frame as the room burns");
    for (prefix, list) in [(FLAME, &flames), (COALS, &coals), (LAVA, &lava), (WATER, &water)] {
        for f in list {
            let _ = writeln!(out, "{prefix}{}", blockout::line(f));
        }
    }
    let _ = writeln!(out, "\n# the ground");
    let _ = writeln!(out, "{}", ground_line(room, dressing, tiles));
    let _ = writeln!(out, "\n# where something burns");
    let fires: Vec<Solid> = flames.iter().chain(&coals).cloned().collect();
    let mut lamps = lamps_of(&fires, &lava);
    if matches!(dressing.setting, Setting::Outdoor { .. } | Setting::Courtyard { .. }) {
        for s in groups.iter().flat_map(|g| &g.solids).filter(|s| s.kind == Kind::Window) {
            let at = [s.at[0] + origin[0], s.at[1], s.at[2] + origin[1]];
            if piece.is_some() && !(keep[0]..keep[2]).contains(&s.at[0]) {
                continue;
            }
            if lamps.iter().any(|l| (l.at[0] - at[0]).hypot(l.at[2] - at[2]) < 18.0) {
                continue;
            }
            lamps.push(Lamp { at, reach: 34.0, strength: 0.5 });
        }
    }
    let stairs: Vec<Option<()>> = (tiles.1..tiles.1 + tiles.3)
        .flat_map(|r| (tiles.0..tiles.0 + tiles.2).map(move |c| (c, r)))
        .map(|(c, r)| (room.tile(c, r) == Tile::Stairs).then_some(()))
        .collect();
    for ((c, r, sw, sh), ()) in blockout::merge(tiles.2 as usize, tiles.3 as usize, &stairs) {
        let (x, z) = ((tiles.0 + c) as f32 * TILE + sw as f32 * TILE / 2.0, (tiles.1 + r) as f32 * TILE + sh as f32 * TILE / 2.0);
        lamps.push(Lamp { at: [x, 12.0, z], reach: 56.0, strength: 0.7 });
    }
    for lamp in lamps {
        let _ = writeln!(out, "lamp at {} {} {} reach {} strength {}", num(lamp.at[0]), num(lamp.at[1]), num(lamp.at[2]), num(lamp.reach), num(lamp.strength));
    }
    out
}

const PIT: f32 = 36.0;
const BED: f32 = 9.0;
const DAIS: f32 = 3.0;

pub fn ground_line(room: &Room, d: &Dressing, (c0, r0, w, h): Rect) -> String {
    let step = if w * h <= 30 * 18 { TILE / 4.0 } else { TILE / 2.0 };
    let per = (TILE / step) as i32;
    let (cols, rows) = (w * per, h * per);
    let pit = matches!(d.setting, Setting::Dungeon(Pack::Crypt | Pack::Unknown));
    let tile_tone = |c: i32, r: i32| -> f32 {
        match room.tile(c, r) {
            Tile::Hazard if pit => 0.0,
            Tile::Hazard => 10.0,
            Tile::Bog => Kind::Mud.tone() * 100.0 * ground_scale(d),
            Tile::Stairs => Kind::Path.tone() * 100.0,
            Tile::Wall | Tile::Block => ground_tone(d, c, r) * 0.7,
            _ => ground_tone(d, c, r),
        }
    };
    let blocked = |c: i32, r: i32| matches!(room.tile(c, r), Tile::Wall | Tile::Block);
    let doors = floor_doors(room, d, (c0, r0, w, h));
    let open_light = open_light(room, d, (c0, r0, w, h));
    let (mut heights, mut tones) = (Vec::new(), Vec::new());
    for j in 0..=rows {
        for i in 0..=cols {
            let (x, z) = ((c0 * per + i) as f32 * step, (r0 * per + j) as f32 * step);
            let (ca, cb) = (((x - 0.01) / TILE).floor() as i32, ((x + 0.01) / TILE).floor() as i32);
            let (ra, rb) = (((z - 0.01) / TILE).floor() as i32, ((z + 0.01) / TILE).floor() as i32);
            let mut touch = Vec::with_capacity(4);
            for r in [ra, rb] {
                for c in [ca, cb] {
                    if (0..room.cols as i32).contains(&c) && (0..room.rows as i32).contains(&r) && !touch.contains(&(c, r)) {
                        touch.push((c, r));
                    }
                }
            }
            let sunk = !touch.is_empty() && touch.iter().all(|&(c, r)| room.tile(c, r) == Tile::Hazard);
            let raised = !touch.is_empty() && touch.iter().all(|&(c, r)| room.tile(c, r) == Tile::Block);
            let depth = if raised {
                DAIS
            } else if !sunk {
                0.0
            } else if pit {
                -PIT
            } else {
                -BED
            };
            heights.push(depth);
            let mut tone = touch.iter().map(|&(c, r)| tile_tone(c, r)).sum::<f32>() / touch.len().max(1) as f32;
            if !sunk {
                let (tc, tr) = ((x / TILE).floor() as i32, (z / TILE).floor() as i32);
                let v = crate::render::overworld::ink::hash(tc, tr, 0x5107) % 5;
                let n = crate::render::overworld::ink::vnoise(x / 56.0, z / 44.0, 0x51);
                tone *= 0.96 + 0.02 * v as f32 + 0.08 * (n - 0.5);
                let near = (-1..=1).flat_map(|dr| (-1..=1).map(move |dc| (dc, dr))).any(|(dc, dr)| {
                    let (c, r) = (((x + dc as f32 * step * 1.5) / TILE).floor() as i32, ((z + dr as f32 * step * 1.5) / TILE).floor() as i32);
                    (0..room.cols as i32).contains(&c) && (0..room.rows as i32).contains(&r) && blocked(c, r)
                });
                if near {
                    tone *= 0.72;
                }
                if let Some(doors) = &doors {
                    tone *= floor_light(room, doors, x, z);
                }
                if let Some(light) = &open_light {
                    tone *= light.at(x, z);
                }
            }
            tones.push(tone.round().clamp(0.0, 100.0) as u8);
        }
    }
    let mut out = format!("ground {cols} {rows} step {} at {} {} heights", num(step), num(c0 as f32 * TILE), num(r0 as f32 * TILE));
    for v in &heights {
        let _ = write!(out, " {}", num(*v));
    }
    out.push_str(" tones");
    for t in &tones {
        let _ = write!(out, " {t}");
    }
    out
}

const LANE_GAIN: f32 = 0.70;
const LANE_WIDTH: f32 = 2.5;
const LANE_EDGE: f32 = 0.8;
const LANE_FADE: f32 = 0.7;
const RIM_DARK: f32 = 0.45;
const RIM_FROM: f32 = 0.30;

fn floor_doors(room: &Room, d: &Dressing, (c0, r0, w, h): Rect) -> Option<Vec<(f32, f32)>> {
    let whole = (c0, r0, w, h) == (0, 0, room.cols as i32, room.rows as i32);
    if !whole || !matches!(d.setting, Setting::Dungeon(_)) {
        return None;
    }
    Some(
        (0..room.rows as i32)
            .flat_map(|r| (0..room.cols as i32).map(move |c| (c, r)))
            .filter(|&(c, r)| room.tile(c, r) == Tile::Door)
            .map(|(c, r)| ((c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE))
            .collect(),
    )
}

fn floor_light(room: &Room, doors: &[(f32, f32)], x: f32, z: f32) -> f32 {
    let smooth = |a: f32, b: f32, v: f32| {
        let t = ((v - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let (cx, cz) = (w / 2.0, h / 2.0);
    let out = (((x - cx) / (w / 2.0)).powi(2) + ((z - cz) / (h / 2.0)).powi(2)).sqrt();
    let mut k = 1.0 - RIM_DARK * smooth(RIM_FROM, 1.2, out);
    let mut lane: Option<(f32, f32)> = None;
    for &(dx, dz) in doors {
        let (ex, ez) = (cx - dx, cz - dz);
        let along = (((x - dx) * ex + (z - dz) * ez) / (ex * ex + ez * ez).max(1e-3)).clamp(0.0, 1.0);
        let off = ((x - (dx + ex * along)).powi(2) + (z - (dz + ez * along)).powi(2)).sqrt() / TILE;
        if lane.is_none_or(|(best, _)| off < best) {
            lane = Some((off, along));
        }
    }
    if let Some((off, along)) = lane {
        k *= 1.0 + LANE_GAIN * (1.0 - smooth(LANE_WIDTH * LANE_EDGE, LANE_WIDTH, off)) * (1.0 - LANE_FADE * along);
    }
    k
}

const MOUTH_GAIN: f32 = 0.70;
const MOUTH_REACH: f32 = 8.0;
const MOUTH_FADE: f32 = 0.70;
const MOUTH_BACK: f32 = 3.0;
const WALL_DARK: f32 = 0.45;
const WALL_NEAR: f32 = 0.5;
const WALL_FAR: f32 = 5.0;
const MOUTH_WIDTH: std::ops::RangeInclusive<i32> = 2..=8;
const SCONCE_EVERY: i32 = 12;
const PILLAR_SHARE: u32 = 20;
const BRAZIER_TOP: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Mouth {
    at: [f32; 2],
    into: [i32; 2],
    half: f32,
    jambs: [(i32, i32); 2],
}

fn mouths(map: &Room, (c0, r0, w, h): Rect) -> Vec<Mouth> {
    let walk = |c: i32, r: i32| blockout::walkable(map.tile(c, r));
    let wall = |c: i32, r: i32| map.tile(c, r) == Tile::Wall;
    let door = |c: i32, r: i32| map.tile(c, r) == Tile::Door;
    let mut out = Vec::new();
    for r in r0..r0 + h {
        let mut c = c0;
        while c < c0 + w {
            if !walk(c, r) || !wall(c - 1, r) {
                c += 1;
                continue;
            }
            let mut e = c;
            while walk(e, r) {
                e += 1;
            }
            if MOUTH_WIDTH.contains(&(e - c)) && wall(e, r) {
                for dz in [1, -1] {
                    let chamber = (c..e).all(|k| walk(k, r + dz)) && walk(c - 1, r + dz) && walk(e, r + dz);
                    let beyond = (c..e).all(|k| walk(k, r - dz) || door(k, r));
                    if chamber && beyond {
                        let z = if dz > 0 { r + 1 } else { r } as f32 * TILE;
                        out.push(Mouth { at: [(c + e) as f32 * TILE / 2.0, z], into: [0, dz], half: (e - c) as f32 * TILE / 2.0, jambs: [(c - 1, r), (e, r)] });
                    }
                }
            }
            c = e;
        }
    }
    for c in c0..c0 + w {
        let mut r = r0;
        while r < r0 + h {
            if !walk(c, r) || !wall(c, r - 1) {
                r += 1;
                continue;
            }
            let mut e = r;
            while walk(c, e) {
                e += 1;
            }
            if MOUTH_WIDTH.contains(&(e - r)) && wall(c, e) {
                for dc in [1, -1] {
                    let chamber = (r..e).all(|k| walk(c + dc, k)) && walk(c + dc, r - 1) && walk(c + dc, e);
                    let beyond = (r..e).all(|k| walk(c - dc, k) || door(c, k));
                    if chamber && beyond {
                        let x = if dc > 0 { c + 1 } else { c } as f32 * TILE;
                        out.push(Mouth { at: [x, (r + e) as f32 * TILE / 2.0], into: [dc, 0], half: (e - r) as f32 * TILE / 2.0, jambs: [(c, r - 1), (c, e)] });
                    }
                }
            }
            r = e;
        }
    }
    out
}

struct OpenLight<'a> {
    map: &'a Room,
    mouths: Vec<Mouth>,
}

impl OpenLight<'_> {
    fn at(&self, x: f32, z: f32) -> f32 {
        let smooth = |a: f32, b: f32, v: f32| {
            let t = ((v - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let (c, r) = ((x / TILE).floor() as i32, (z / TILE).floor() as i32);
        let span = WALL_FAR.ceil() as i32 + 1;
        let mut near = WALL_FAR * TILE;
        for rr in r - span..=r + span {
            for cc in c - span..=c + span {
                if self.map.tile(cc, rr) == Tile::Wall {
                    let dx = ((x - (cc as f32 + 0.5) * TILE).abs() - TILE / 2.0).max(0.0);
                    let dz = ((z - (rr as f32 + 0.5) * TILE).abs() - TILE / 2.0).max(0.0);
                    near = near.min(dx.hypot(dz));
                }
            }
        }
        let dark = WALL_DARK * (1.0 - smooth(WALL_NEAR * TILE, WALL_FAR * TILE, near));
        let mut lane = 0.0f32;
        for m in &self.mouths {
            let (px, pz) = (x - m.at[0], z - m.at[1]);
            let along = px * m.into[0] as f32 + pz * m.into[1] as f32;
            if along > MOUTH_REACH * TILE || along < -MOUTH_BACK * TILE {
                continue;
            }
            let across = (px * m.into[1] as f32 - pz * m.into[0] as f32).abs();
            let edge = m.half + TILE / 2.0;
            let down = if along >= 0.0 { 1.0 - MOUTH_FADE * along / (MOUTH_REACH * TILE) } else { 1.0 + along / (MOUTH_BACK * TILE) };
            lane = lane.max((1.0 - smooth(edge * 0.8, edge, across)) * down);
        }
        (1.0 - dark) * (1.0 + MOUTH_GAIN * lane)
    }
}

fn open_light<'a>(room: &'a Room, d: &Dressing, (c0, r0, w, h): Rect) -> Option<OpenLight<'a>> {
    if room.kind != RoomKind::Depths || !matches!(d.setting, Setting::Dungeon(_)) {
        return None;
    }
    let reach = (MOUTH_REACH + MOUTH_BACK).ceil() as i32 + 2;
    Some(OpenLight { map: room, mouths: mouths(room, (c0 - reach, r0 - reach, w + 2 * reach, h + 2 * reach)) })
}

fn open_braziers(map: &Room, d: &Dressing, tiles: Rect, crop: Rect, groups: &[Group]) -> Vec<Solid> {
    if map.kind != RoomKind::Depths || !matches!(d.setting, Setting::Dungeon(_)) {
        return Vec::new();
    }
    let (c0, r0, w, h) = tiles;
    let walk = |c: i32, r: i32| blockout::walkable(map.tile(c, r));
    let inside = |c: i32, r: i32| (c0..c0 + w).contains(&c) && (r0..r0 + h).contains(&r);
    let brazier = |x: f32, z: f32, y: f32| {
        [
            Solid::block(Kind::Metal, [x - 4.0, z - 4.0, x + 4.0, z + 4.0], y, 3.0),
            Solid::new(blockout::Shape::Cone, Kind::Fire, [x, y + 3.0, z], [6.0, 7.8, 6.0]),
        ]
    };
    let jambs: Vec<(i32, i32)> = mouths(map, (c0 - 8, r0 - 8, w + 16, h + 16)).into_iter().filter(|m| m.into != [0, -1]).flat_map(|m| m.jambs).collect();
    let mut wanted: Vec<(i32, i32)> = jambs.iter().copied().filter(|&(c, r)| inside(c, r)).collect();
    for r in r0..r0 + h {
        for c in c0..c0 + w {
            if map.tile(c, r) != Tile::Wall || jambs.iter().any(|&(jc, jr)| (jc - c).abs().max((jr - r).abs()) < 6) {
                continue;
            }
            let slant = if walk(c, r + 1) {
                c + 5 * r
            } else if walk(c + 1, r) {
                r + 5 * c + 3
            } else if walk(c - 1, r) {
                r + 5 * c + 7
            } else {
                continue;
            };
            if slant.rem_euclid(SCONCE_EVERY) == 0 {
                wanted.push((c, r));
            }
        }
    }
    let mut out = Vec::new();
    for (c, r) in wanted {
        out.extend(brazier((c - crop.0) as f32 * TILE + TILE / 2.0, (r - crop.1) as f32 * TILE + TILE / 2.0, BRAZIER_TOP));
    }
    let mut stands: Vec<(i32, i32, f32, [f32; 2])> = Vec::new();
    for s in groups.iter().filter(|g| g.role == Role::Prop).flat_map(|g| &g.solids) {
        let [x0, z0, x1, z1] = s.footprint();
        let round = matches!(s.shape, blockout::Shape::Box | blockout::Shape::Sphere);
        if !round || s.kind == Kind::Fire || x1 - x0 < 10.0 || z1 - z0 < 10.0 || s.top() < 6.0 {
            continue;
        }
        let (c, r) = (crop.0 + ((x0 + x1) / 2.0 / TILE).floor() as i32, crop.1 + ((z0 + z1) / 2.0 / TILE).floor() as i32);
        if map.tile(c, r) != Tile::Block || !inside(c, r) {
            continue;
        }
        match stands.iter_mut().find(|p| (p.0, p.1) == (c, r)) {
            Some(p) => p.2 = p.2.max(s.top()),
            None => stands.push((c, r, s.top(), [(x0 + x1) / 2.0, (z0 + z1) / 2.0])),
        }
    }
    for (c, r, y, [x, z]) in stands {
        if crate::render::overworld::ink::hash(c, r, 0xB2A2) % 100 < PILLAR_SHARE {
            out.extend(brazier(x, z, y));
        }
    }
    out
}

fn ground_tone(d: &Dressing, c: i32, r: i32) -> f32 {
    let kind = match d.setting {
        Setting::Indoor | Setting::Hall => Kind::Floor,
        Setting::Dungeon(Pack::Cavern | Pack::Fungal | Pack::Swamp) => Kind::Ground,
        Setting::Dungeon(_) => Kind::Floor,
        Setting::Outdoor { .. } | Setting::Courtyard { .. } => {
            let inside = |rects: &[Rect]| rects.iter().any(|&(x, y, w, h)| c >= x && c < x + w && r >= y && r < y + h);
            if inside(d.paved) {
                Kind::Road
            } else if inside(d.roads) {
                Kind::Path
            } else {
                match d.setting {
                    Setting::Outdoor { field, .. } | Setting::Courtyard { field } => match field {
                        blockout::Field::Grass | blockout::Field::Marsh | blockout::Field::Scree => Kind::Grass,
                        blockout::Field::Sand => Kind::Sand,
                        blockout::Field::Earth => Kind::Ground,
                        blockout::Field::Snow => Kind::Plaster,
                    },
                    _ => Kind::Ground,
                }
            }
        }
    };
    kind.tone() * 100.0 * ground_scale(d)
}

fn ground_scale(d: &Dressing) -> f32 {
    match d.setting {
        Setting::Dungeon(Pack::Hellforge) => GROUND_TONE,
        Setting::Dungeon(_) => DEEP_GROUND_TONE,
        _ => GROUND_TONE,
    }
}

pub const GROUND_TONE: f32 = 0.72;
pub const DEEP_GROUND_TONE: f32 = 0.84;

pub fn lamps_of(flames: &[Solid], lava: &[Solid]) -> Vec<Lamp> {
    let mut lamps: Vec<(Lamp, f32)> = Vec::new();
    for f in flames {
        let at = [f.at[0], f.at[1] + f.size[1] * 0.8, f.at[2]];
        let big = f.size[0].max(f.size[2]);
        if let Some((lamp, n)) = lamps.iter_mut().find(|(l, _)| (l.at[0] - at[0]).hypot(l.at[2] - at[2]) < 10.0) {
            *n += 1.0;
            lamp.reach = lamp.reach.max(lamp_reach(big));
            continue;
        }
        lamps.push((Lamp { at, reach: lamp_reach(big), strength: 0.95 }, 1.0));
    }
    let mut out: Vec<Lamp> = lamps.into_iter().map(|(l, _)| l).collect();
    for pool in lava {
        let [x0, z0, x1, z1] = pool_rect(pool);
        let (w, d) = (x1 - x0, z1 - z0);
        let n = ((w.max(d) / (4.0 * TILE)).ceil() as i32).max(1);
        for k in 0..n {
            let t = (k as f32 + 0.5) / n as f32;
            let (x, z) = if w >= d { (x0 + w * t, (z0 + z1) / 2.0) } else { ((x0 + x1) / 2.0, z0 + d * t) };
            out.push(Lamp { at: [x, 6.0, z], reach: 46.0 + w.min(d) * 0.6, strength: 0.85 });
        }
    }
    out
}

fn pool_rect(s: &Solid) -> [f32; 4] {
    s.footprint()
}

fn lamp_reach(size: f32) -> f32 {
    if size >= 12.0 {
        104.0
    } else if size >= 7.0 {
        86.0
    } else {
        66.0
    }
}

pub use super::rooms_lit_look::EMBER_DEEP;

fn accent([r, g, b]: [u8; 3]) -> Material {
    Material { accent: true, ..Material::color([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0]) }
}

fn cracks(bed: &Shape, seed: u32, t: f32) -> Vec<Shape> {
    let Some(mesh) = angel_stage::shapes::by_name("box") else {
        return Vec::new();
    };
    let (sx, sz) = (bed.size[0], bed.size[2]);
    let top = bed.at[1] + bed.size[1] + 0.25;
    let count = ((sx * sz) / (TILE * TILE) * 3.0).ceil().max(1.0) as u32;
    let mut out = Vec::new();
    for k in 0..count {
        let v = crate::render::overworld::ink::hash(k as i32, seed as i32, 0x1A7A);
        let (u, w) = ((v & 0xFF) as f32 / 255.0, ((v >> 8) & 0xFF) as f32 / 255.0);
        let long = 4.0 + ((v >> 16) & 7) as f32;
        let across = (v >> 20) & 1 == 0;
        let (lx, lz) = if across { (long.min(sx - 2.0), 1.5) } else { (1.5, long.min(sz - 2.0)) };
        if lx <= 0.5 || lz <= 0.5 {
            continue;
        }
        let phase = ((v >> 24) & 0xFF) as f32 / 255.0;
        if t > 0.0 && ((t * 0.35 + phase).fract() > 0.7) {
            continue;
        }
        if t == 0.0 && k % 3 != 0 {
            continue;
        }
        let x = bed.at[0] - sx / 2.0 + lx / 2.0 + 1.0 + u * (sx - lx - 2.0).max(0.0);
        let z = bed.at[2] - sz / 2.0 + lz / 2.0 + 1.0 + w * (sz - lz - 2.0).max(0.0);
        out.push(Shape { line: 0, shape: "box".into(), mesh, kind: Kind::Fire, at: [x, top - 0.25, z], size: [lx, 0.25, lz], turn: 0.0, id: None, top: None });
    }
    out
}

pub fn flame_nodes(pieces: &[Arc<Piece>], activity: Activity, t: f32, fire: [u8; 3]) -> Vec<Node> {
    let mut out = Vec::new();
    for (p, piece) in pieces.iter().enumerate() {
        let beds = piece.coals.iter().map(|s| (s, activity == Activity::Cleared)).chain(piece.lava.iter().map(|s| (s, false)));
        let mut n = 0u64;
        let mut push = |out: &mut Vec<Node>, mesh, transform, material| {
            let mut node = Node::new(0xF1B_0000_0000 | ((p as u64) << 24) | n, mesh, material);
            node.transform = transform;
            out.push(node);
            n += 1;
        };
        for pool in &piece.water {
            let material = Material { role: angel_stage::scene::Role::Water, ..Material::lit([WATER_TONE, WATER_TONE, WATER_TONE, 1.0], 0.0) };
            push(&mut out, pool.mesh, pool.transform(), material);
        }
        for (k, (bed, quiet)) in beds.enumerate() {
            let crust = if quiet { Material::lit([0.3, 0.3, 0.3, 1.0], 0.0) } else { accent(EMBER_DEEP) };
            push(&mut out, bed.mesh, bed.transform(), crust);
            for crack in cracks(bed, k as u32 + 7 * p as u32, if quiet { 0.0 } else { t }) {
                let colour = if quiet { EMBER_DEEP } else { super::rooms_lit_look::FLAME };
                push(&mut out, crack.mesh, crack.transform(), accent(colour));
            }
        }
        for (k, flame) in piece.flames.iter().enumerate() {
            let mut shape = flame.clone();
            let lick = super::rooms_lit_look::flicker(t * 1.6, k + 3 * p);
            let material = match activity {
                Activity::Cleared => {
                    shape.size[1] *= 0.35;
                    Material::lit([0.55, 0.55, 0.55, 1.0], 0.0)
                }
                _ => {
                    shape.size[1] *= lick;
                    accent(fire)
                }
            };
            let mut node = Node::new(0xF1A_0000_0000 | ((p as u64) << 16) | k as u64, shape.mesh, material);
            node.transform = shape.transform();
            out.push(node);
        }
    }
    out
}

pub const SPARKS: usize = 3;

pub struct Staged {
    pub floor: Vec<Node>,
    pub blocks: Vec<Node>,
    pub flames: Vec<Node>,
    pub meshes: Vec<(u32, Arc<MeshData>)>,
    pub sets: Vec<(u64, Arc<str>)>,
    pub look: angel_stage::look::Look,
}

impl Lit {
    pub fn staged(
        &self,
        (scale, offset): (f32, [f32; 3]),
        zoom: f32,
        view: (f32, f32),
        rect: [f32; 4],
        moment: &super::room::Moment,
        sparks: &[Lamp],
        seen: &dyn Fn([f32; 3], [f32; 3]) -> bool,
    ) -> Staged {
        let at = |p: [f32; 3]| [p[0] * scale + offset[0], p[1] * scale + offset[1], p[2] * scale + offset[2]];
        let pieces: Vec<Arc<Piece>> = self.near(rect).into_iter().filter(|p| seen(at(p.bounds.0), at(p.bounds.1))).collect();
        let many = pieces.len() > 1;
        let tag = |k: usize, mut n: Node| {
            if many {
                n.id |= (k as u64 + 1) << 40;
            }
            n
        };
        let (mut floor, mut blocks, mut meshes, mut sets, mut lamps) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (k, piece) in pieces.iter().enumerate() {
            let (f, b, mesh) = piece.nodes();
            floor.extend(angel_stage::set::place(f, scale, offset).into_iter().map(|n| tag(k, n)));
            blocks.extend(angel_stage::set::place(b, scale, offset).into_iter().map(|n| tag(k, n)));
            if let Some(mesh) = mesh {
                meshes.push((ground_mesh_id(piece.key), Arc::clone(mesh)));
            }
            sets.push((piece.key, Arc::clone(&piece.text)));
            for (lamp, steady) in piece.lamps().into_iter().zip(piece.steady.iter().copied().chain(std::iter::repeat(false))) {
                let burns = if steady { Activity::Fought } else { moment.activity };
                lamps.push(Lamp { at: at(lamp.at), reach: lamp.reach * scale * burns.reach(), strength: lamp.strength * burns.strength() });
            }
        }
        let flames = angel_stage::set::place(&flame_nodes(&pieces, moment.activity, moment.time, self.tone.flame()), scale, offset);
        let centre = [view.0 / 2.0, 0.0, view.1 / 2.0];
        let reach = view.0.hypot(view.1);
        let chest = moment.chest.map(|[x, y, z]| [x * zoom + offset[0], y * zoom, z * zoom + offset[2]]);
        if moment.rise < 1.0 {
            let from = chest.unwrap_or(centre);
            let far = |l: &Lamp| (l.at[0] - from[0]).hypot(l.at[2] - from[2]);
            let farthest = lamps.iter().map(far).fold(1.0f32, f32::max);
            for lamp in &mut lamps {
                lamp.strength *= super::room::lamp_up(moment.rise, far(lamp) / farthest);
            }
        }
        let sparks: Vec<Lamp> = sparks
            .iter()
            .take(SPARKS)
            .map(|l| Lamp { at: [l.at[0] * zoom + offset[0], l.at[1] * zoom, l.at[2] * zoom + offset[2]], reach: l.reach * zoom, ..*l })
            .collect();
        let mut look = super::rooms_lit_look::look(self.tone, &lamps, moment.time, centre, reach, chest);
        if matches!(self.tone, Tone::Delve(_))
            && let Some(sun) = &mut look.sun
        {
            sun.strength *= 0.2 + 0.8 * moment.rise;
            for lamp in &mut look.lamps {
                lamp.reach *= DEEP_LAMP_REACH;
            }
        }
        if !sparks.is_empty() {
            look.lamps.truncate(angel_stage::look::LAMP_MAX - sparks.len());
            look.lamps.splice(0..0, sparks);
        }
        Staged { floor, blocks, flames, meshes, sets, look }
    }
}

impl Lit {
    pub fn picture(&self, size: (i32, i32)) -> Img {
        if let Ok(kept) = self.picture.lock()
            && let Some((s, img)) = kept.as_ref()
            && *s == size
        {
            return (**img).clone();
        }
        let img = Arc::new(self.paint(size));
        if let Ok(mut kept) = self.picture.lock() {
            *kept = Some((size, Arc::clone(&img)));
        }
        Arc::unwrap_or_clone(img)
    }

    fn paint(&self, (w, h): (i32, i32)) -> Img {
        use super::rooms_lit_look::FLAME;
        let (warm_ramp, flame) = (self.tone.warm(), self.tone.flame());
        let (cols, rows) = (self.room.cols as i32, self.room.rows as i32);
        let pieces = self.every_piece();
        let mut tops: Vec<Option<(f32, Kind)>> = vec![None; (cols * rows).max(0) as usize];
        let mut glows = vec![false; (cols * rows).max(0) as usize];
        let mut wet = vec![false; (cols * rows).max(0) as usize];
        let mut lamps = Vec::new();
        for piece in &pieces {
            let (c0, r0, pw, ph) = piece.tiles;
            for r in r0..r0 + ph {
                for c in c0..c0 + pw {
                    let at = (r * cols + c) as usize;
                    if let Some(s) = over_tile(&piece.set.shapes, (c, r)).or_else(|| over_tile(&piece.flames, (c, r))) {
                        tops[at] = Some((s.at[1] + s.size[1], s.kind));
                    }
                    glows[at] = over_tile(&piece.coals, (c, r)).or_else(|| over_tile(&piece.lava, (c, r))).is_some();
                    wet[at] = over_tile(&piece.water, (c, r)).is_some();
                }
            }
            lamps.extend(piece.lamps());
        }
        let ramp = self.tone.ramp();
        let luma = |c: [u8; 3]| (0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2])) / 255.0;
        let levels: Vec<f32> = ramp.iter().map(|&c| luma(c)).collect();
        let bayer = |x: i32, y: i32| angel_stage::look::bayer4()[(y & 3) as usize][(x & 3) as usize] as f32 / 16.0 + 1.0 / 32.0;
        let pick = |level: f32, x: i32, y: i32| -> usize {
            let i = levels.iter().rposition(|&l| l <= level).unwrap_or(0);
            if i + 1 >= levels.len() {
                return i;
            }
            let f = (level - levels[i]) / (levels[i + 1] - levels[i]).max(1e-6);
            if f > bayer(x, y) { i + 1 } else { i }
        };
        let k = w.max(1) as f32 / self.stage().0.max(1.0);
        let pit = matches!(self.dressing.setting, Setting::Dungeon(Pack::Crypt | Pack::Unknown));
        let top_at = |c: i32, r: i32| -> f32 {
            if !(0..cols).contains(&c) || !(0..rows).contains(&r) {
                return 0.0;
            }
            tops[(r * cols + c) as usize].map_or(0.0, |(t, _)| t)
        };
        let mut pixels = vec![[0u8; 3]; (w.max(0) * h.max(0)) as usize];
        for y in 0..h {
            for x in 0..w {
                let (sx, sz) = ((x as f32 + 0.5) / k, (y as f32 + 0.5) / k);
                let (c, r) = ((sx / TILE).floor() as i32, (sz / TILE).floor() as i32);
                if !(0..cols).contains(&c) || !(0..rows).contains(&r) {
                    continue;
                }
                let at = (r * cols + c) as usize;
                let warm: f32 = lamps
                    .iter()
                    .map(|l| {
                        let d = (l.at[0] - sx).hypot(l.at[2] - sz);
                        let fall = (1.0 - d / l.reach.max(1e-3)).max(0.0);
                        l.strength * fall * fall
                    })
                    .sum::<f32>()
                    .min(1.0);
                let px = &mut pixels[(y * w + x) as usize];
                if glows[at] {
                    let crack = crate::render::overworld::ink::hash(x / 3, y / 2, 0x1A7A) % 11 == 0;
                    *px = if crack { FLAME } else { EMBER_DEEP };
                    continue;
                }
                if wet[at] && tops[at].is_none_or(|(top, _)| top <= angel_stage::set::FLOOR_TOP) {
                    let water = self.tone.water();
                    *px = water[3 + usize::from(bayer(x, y) < 0.3)];
                    continue;
                }
                let level = match tops[at] {
                    Some((_, kind)) if kind.shines() => {
                        *px = flame;
                        continue;
                    }
                    Some((top, kind)) if top > angel_stage::set::FLOOR_TOP => kind.tone() * (0.72 + (top / 120.0).min(0.35)),
                    _ if self.room.tile(c, r) == Tile::Hazard && pit => continue,
                    Some((_, kind)) => kind.tone() * 0.5,
                    None => {
                        let shade = (1..=3).any(|d| {
                            let (dx, dz) = (sx - 5.0 * d as f32, sz + 5.0 * d as f32);
                            top_at((dx / TILE).floor() as i32, (dz / TILE).floor() as i32) > 5.0 * d as f32
                        });
                        ground_tone(&self.dressing, c, r) / 100.0 * if shade { 0.36 } else { 0.62 }
                    }
                };
                let i = pick((level * (1.0 + 0.4 * warm)).min(1.0), x, y);
                *px = if bayer(y, x) < warm * 0.3 { warm_ramp[i] } else { ramp[i] };
            }
        }
        Img::from_rgb(w, h, &pixels)
    }
}

pub const PIECE: (i32, i32) = (crate::world::map::CHUNK_W, crate::world::map::CHUNK_H);
pub const RING_A_CALL: usize = 2;

pub fn over_tile(shapes: &[Shape], (c, r): (i32, i32)) -> Option<&Shape> {
    let (x, z) = ((c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE);
    shapes
        .iter()
        .filter(|s| {
            let quarter = (s.turn.rem_euclid(180.0) - 90.0).abs() < 45.0;
            let (sx, sz) = if quarter { (s.size[2], s.size[0]) } else { (s.size[0], s.size[2]) };
            if sx * sz < TILE * TILE / 4.0 {
                return false;
            }
            let (dx, dz) = ((x - s.at[0]) / (sx / 2.0).max(1e-3), (z - s.at[2]) / (sz / 2.0).max(1e-3));
            let round = ["sphere", "cone", "blob", "rock", "cylinder"].iter().any(|n| s.shape.starts_with(n));
            if round { dx * dx + dz * dz <= 1.0 } else { dx.abs() <= 1.0 && dz.abs() <= 1.0 }
        })
        .max_by(|a, b| (a.at[1] + a.size[1]).total_cmp(&(b.at[1] + b.size[1])))
}

fn at_home(kind: RoomKind) -> bool {
    matches!(kind, RoomKind::Stockpile | RoomKind::Muster | RoomKind::Workshop | RoomKind::Quarters | RoomKind::RealmHall)
}

fn kin(item: &pilot::Item, room: &Room) -> bool {
    match item.source {
        Source::Place(kind) | Source::Nook(kind) => kind == room.kind,
        Source::Sanctuary => room.kind == RoomKind::Sanctuary,
        Source::Vault => room.kind == RoomKind::Secret,
        Source::Forge => room.kind == RoomKind::Forge,
        Source::Chamber { .. } | Source::Great { .. } => false,
    }
}

fn fits_in(d: &Dressing, room: &Room) -> bool {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let inside = |&(c, r, w, h): &Rect| c >= 0 && r >= 0 && c + w <= cols && r + h <= rows;
    d.props.iter().all(|(rect, _)| inside(rect)) && d.roads.iter().all(inside) && d.paved.iter().all(inside)
}

fn generic_id(setting: Setting) -> &'static str {
    match setting {
        Setting::Dungeon(Pack::Crypt) => "plain-crypt",
        Setting::Dungeon(Pack::Cavern) => "plain-cavern",
        Setting::Dungeon(Pack::Hellforge) => "plain-hellforge",
        Setting::Dungeon(Pack::Archive) => "plain-archive",
        Setting::Dungeon(Pack::Fungal) => "plain-fungal",
        Setting::Dungeon(Pack::Unknown) => "plain-unknown",
        Setting::Dungeon(Pack::Swamp) => "plain-swamp",
        Setting::Indoor => "plain-interior",
        Setting::Hall => "plain-hall",
        Setting::Outdoor { .. } | Setting::Courtyard { .. } => "plain-outdoor",
    }
}

pub fn dressing_for(room: &Room, pack: Pack) -> Option<Dressing> {
    if matches!(room.kind, RoomKind::Kingdom | RoomKind::Ledge | RoomKind::Wanted) {
        return None;
    }
    if let Some(item) = pilot::all().iter().find(|i| kin(i, room) && fits_in(&i.dressing, room)) {
        return Some(match item.dressing.setting {
            Setting::Dungeon(_) => Dressing { setting: Setting::Dungeon(pack), ..item.dressing },
            _ => item.dressing,
        });
    }
    let setting = if at_home(room.kind) { Setting::Indoor } else { Setting::Dungeon(pack) };
    Some(Dressing { id: generic_id(setting), title: "A room with no item of its own, its own blockout", setting, ..Dressing::BASE })
}

pub fn piece_rects(room: &Room, open: bool, (cx, cz): (i32, i32)) -> (Rect, Rect) {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    if !open {
        return ((0, 0, cols, rows), (0, 0, cols, rows));
    }
    let (c0, r0) = (cx * PIECE.0, cz * PIECE.1);
    let (w, h) = (PIECE.0.min(cols - c0), PIECE.1.min(rows - r0));
    let (x0, y0) = ((c0 - 1).max(0), (r0 - 1).max(0));
    ((c0, r0, w, h), (x0, y0, (c0 + w + 1).min(cols) - x0, (r0 + h + 1).min(rows) - y0))
}

pub fn crop_room(room: &Room, (c0, r0, w, h): Rect) -> Room {
    let mut tiles = Vec::with_capacity((w * h).max(0) as usize);
    for r in r0..r0 + h {
        for c in c0..c0 + w {
            tiles.push(room.tile(c, r));
        }
    }
    let mut cut = Room::kingdom(tiles, w as usize, h as usize);
    cut.kind = room.kind;
    cut
}

pub fn cut_solid(s: &blockout::Solid, keep: [f32; 4]) -> Option<blockout::Solid> {
    const EPS: f32 = 0.25;
    if s.shape == blockout::Shape::Box {
        let foot = s.footprint();
        let cut = [foot[0].max(keep[0]), foot[1].max(keep[1]), foot[2].min(keep[2]), foot[3].min(keep[3])];
        if cut[2] - cut[0] <= EPS || cut[3] - cut[1] <= EPS {
            return None;
        }
        if cut == foot {
            return Some(s.clone());
        }
        let mut out = blockout::Solid::block(s.kind, cut, s.at[1], s.size[1]);
        out.todo = s.todo;
        return Some(out);
    }
    let inside = s.at[0] >= keep[0] && s.at[0] < keep[2] && s.at[2] >= keep[1] && s.at[2] < keep[3];
    inside.then(|| s.clone())
}

#[cfg(test)]
#[path = "../../tests/world3d/rooms_lit__tests.rs"]
mod tests;
