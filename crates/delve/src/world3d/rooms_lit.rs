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
//! - **An open floor's map** stands up in pieces of a kingdom chunk's size,
//!   as `plain` cuts it, each its own set; a page makes the pieces near its
//!   view itself, from the run's tiles. Having no doors, it is lit from its
//!   passages' mouths and by braziers on its walls and what stands in it.
//! - **Across a wire** a one-screen room's set goes in the run's tree (its
//!   key and text, no plate), so a page with no asset folder draws its
//!   landmark too; a page makes the same look from the run.
//!
//! The painted path (`assets`, `fit`, `plain`) stays for a host that says
//! `ANGEL_WORLD3D_PLATES=1`.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::look::Lamp;
use angel_stage::scene::{Material, MeshData, Node};
use angel_stage::set::{Kind, Set, Shape, ground_mesh_id};

use super::blockout::{self, Dressing, Group, Rect, Role, Setting, Solid, TILE, num};
use super::plain;
use super::rooms_lit_look::{Activity, Tone};
use crate::render::overworld::Img;
use crate::rules::{Pack, Room, RoomKind, Tile};

/// The generator's version: a lit room's key holds it, so a page never
/// keeps a set made by another.
pub const VERSION: u64 = 6;
/// How many rooms' answers ([`lit_for`]) are kept.
const KEPT: usize = 8;
/// How many open floors' pieces are kept made.
const PIECES_KEPT: usize = 64;
/// How many set texts a page keeps of those its host sent.
const RECEIVED_KEPT: usize = 8;

// ---------------------------------------------------------- the switch ----

/// Whether rooms in 3D are lit (the monolith look, the default) or drawn
/// with the painted plates as before (`ANGEL_WORLD3D_PLATES=1`).
pub fn on() -> bool {
    #[cfg(test)]
    if let Some(plates) = TEST_PLATES.with(|c| c.get()) {
        return !plates;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| !std::env::var("ANGEL_WORLD3D_PLATES").is_ok_and(|v| v == "1"))
}

#[cfg(test)]
thread_local! {
    static TEST_PLATES: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Draw rooms in 3D with the painted plates on this thread (true), as
/// `ANGEL_WORLD3D_PLATES=1` does, or lit.
#[cfg(test)]
pub fn set_plates(plates: bool) {
    TEST_PLATES.with(|c| c.set(Some(plates)));
}

// ------------------------------------------------------------ the room ----

/// A room stood up lit: its tone and its pieces.
pub struct Lit {
    pub tone: Tone,
    /// The room as it is laid, its dressing and the generator's version.
    pub key: u64,
    /// Its dressing's id.
    pub id: &'static str,
    /// Its pieces across and down: one for a room, more for an open floor.
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

/// One piece of a lit room: a set of its own (a room's whole, or a chunk
/// of an open floor's map), in the room's own stage pixels.
pub struct Piece {
    pub cell: (i32, i32),
    /// The room's tiles it stands for: column, row, width, height.
    pub tiles: Rect,
    pub key: u64,
    /// Its set's text: the shapes, the ground, the lamps, and its flames as
    /// `# flame` lines the set leaves to the frame.
    pub text: Arc<str>,
    pub set: Set,
    /// Its flames (torches, braziers: `# flame` lines), drawn each frame as
    /// the room's activity says, in their own colour.
    pub flames: Vec<Shape>,
    /// What glows rather than flickers (`# coals` lines: a hearth's bed; `#
    /// lava` lines: lava), drawn each frame lit warm by its own lamps.
    pub coals: Vec<Shape>,
    pub lava: Vec<Shape>,
    /// Its water (`# water` lines), drawn each frame darker than the set's
    /// own water kind, so it runs on its ramp instead of sitting at the top
    /// of it.
    pub water: Vec<Shape>,
    /// Which of the set's lamps light lava, which burns whatever the room
    /// is doing.
    pub steady: Vec<bool>,
    /// The box its blocks fill on the room's stage, low and high corners.
    pub bounds: ([f32; 3], [f32; 3]),
    made: OnceLock<(Vec<Node>, Vec<Node>, Option<Arc<MeshData>>)>,
}

impl Piece {
    /// Its set's world, lit and toned (`Set::lit_world_nodes`), unplaced:
    /// what lies on the floor (its ground first), what stands, and its
    /// ground's mesh (uploaded as `ground_mesh_id(key)`). Made once.
    pub fn nodes(&self) -> &(Vec<Node>, Vec<Node>, Option<Arc<MeshData>>) {
        self.made.get_or_init(|| {
            let (floor, blocks, mesh) = self.set.lit_world_nodes(ground_mesh_id(self.key));
            (floor, blocks, mesh.map(Arc::new))
        })
    }

    /// Its lamps as a look lights with them, unplaced.
    pub fn lamps(&self) -> Vec<Lamp> {
        self.set.look_lamps()
    }
}

impl Lit {
    /// Its room's size in its own stage pixels.
    pub fn stage(&self) -> (f32, f32) {
        (self.room.cols as f32 * TILE, self.room.rows as f32 * TILE)
    }

    /// Whether it is an open floor's map, stood up in pieces.
    pub fn open(&self) -> bool {
        self.grid != (1, 1)
    }

    /// The room it stands for.
    pub fn room(&self) -> &Room {
        &self.room
    }

    /// Piece `cell`, made the first time it is asked for; `None` off the
    /// grid or when its set does not read.
    pub fn piece(&self, (cx, cz): (i32, i32)) -> Option<Arc<Piece>> {
        if !(0..self.grid.0).contains(&cx) || !(0..self.grid.1).contains(&cz) {
            return None;
        }
        let slot = &self.pieces[(cz * self.grid.0 + cx) as usize];
        slot.get_or_init(|| self.make_piece((cx, cz))).clone()
    }

    /// Every piece.
    pub fn every_piece(&self) -> Vec<Arc<Piece>> {
        (0..self.grid.1).flat_map(|cz| (0..self.grid.0).map(move |cx| (cx, cz))).filter_map(|c| self.piece(c)).collect()
    }

    /// The pieces near a view of `[x0, z0, x1, z1]` (the room's stage
    /// pixels): every piece of a room; of an open floor those the view
    /// overlaps, made when first asked for, and a ring round them, of which
    /// at most [`plain::RING_A_CALL`] not yet made are made a call.
    pub fn near(&self, view: [f32; 4]) -> Vec<Arc<Piece>> {
        if !self.open() {
            return self.every_piece();
        }
        let (pw, ph) = (plain::PIECE.0 as f32 * TILE, plain::PIECE.1 as f32 * TILE);
        let (c0, c1) = ((view[0] / pw).floor() as i32, ((view[2] - 0.01) / pw).floor() as i32);
        let (r0, r1) = ((view[1] / ph).floor() as i32, ((view[3] - 0.01) / ph).floor() as i32);
        let mut budget = plain::RING_A_CALL;
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
            // A one-screen room: the text its host sent, when a page was
            // sent it; else made here.
            let text = received(self.key).unwrap_or_else(|| room_text(&self.room, &self.dressing, true, None).into());
            return piece_of_text(cell, (0, 0, self.room.cols as i32, self.room.rows as i32), self.key, text);
        }
        type Kept = Mutex<VecDeque<(u64, Arc<Piece>)>>;
        static KEPT_PIECES: Kept = Mutex::new(VecDeque::new());
        let (tiles, crop) = plain::piece_rects(&self.room, true, cell);
        // Kept by its own tiles and the ring round them (a gate shutting
        // elsewhere keeps it), its dressing and the generator's version.
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
        // The ground round its edge is the map's (the tiles a corner looks
        // at), and its rock rises as far as the map's floor north of it lets
        // it (`rooms_lit_mass::CORE_REACH` tiles).
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

    /// The text of its one piece, for a page across a wire (`None` for an
    /// open floor, whose pieces a page makes itself).
    pub fn text(&self) -> Option<Arc<str>> {
        if self.open() {
            return None;
        }
        self.piece((0, 0)).map(|p| Arc::clone(&p.text))
    }

    /// Its lamps near a view of `[x0, z0, x1, z1]` (its stage pixels), unplaced.
    pub fn lamps_near(&self, view: [f32; 4]) -> Vec<Lamp> {
        self.near(view).iter().flat_map(|p| p.lamps()).collect()
    }
}

/// A piece made of set text `text`: `None` when it does not read.
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

/// The shapes text `text` holds under comment `prefix`, read as shapes of
/// the same set.
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

/// The comments a flame's, a bed of coals' and lava's lines are written
/// under: the frame draws them, not the set.
const FLAME: &str = "# flame ";
const COALS: &str = "# coals ";
const LAVA: &str = "# lava ";
const WATER: &str = "# water ";

/// The tone water is drawn with in a lit room: below the set's water kind's
/// (`Kind::Water`, 0.5, the kingdom's rivers under the sky), so a pool in a
/// dark room runs on the middle of its ramp, banded and glinting.
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

// ------------------------------------------------------------ the rooms ----

/// The number a lit room is kept under: the room as it is laid, its
/// dressing, its delve, and the generator's version. The same on a host and
/// on a page in a browser (FNV-1a over fixed-width words, not the standard
/// hasher, whose `usize` is the build's own width).
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

/// Room `room` (on a floor of `pack`) stood up lit: `None` for a room never
/// drawn so (the kingdom, the side-on halls). Kept for the last few rooms
/// asked about.
pub fn lit_for(room: &Room, pack: Pack) -> Option<Arc<Lit>> {
    type Kept = Mutex<VecDeque<(u64, Arc<Lit>)>>;
    static KEPT_ROOMS: Kept = Mutex::new(VecDeque::new());
    let (dressing, _) = plain::dressing_for(room, pack)?;
    let key = key_of(room, &dressing);
    if let Ok(kept) = KEPT_ROOMS.lock()
        && let Some((_, lit)) = kept.iter().find(|(k, _)| *k == key)
    {
        return Some(Arc::clone(lit));
    }
    let open = room.cols > crate::rules::layout::MAX_COLS || room.rows > crate::rules::layout::MAX_ROWS;
    let grid = if open {
        ((room.cols as i32 + plain::PIECE.0 - 1) / plain::PIECE.0, (room.rows as i32 + plain::PIECE.1 - 1) / plain::PIECE.1)
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

/// A delve's key light and ambient in 3D, of what its tone gives them
/// (`rooms_lit_look::Tone::key`): low, so a lit delve reads as the 2D black
/// dungeon does, its field black and its light where something burns.
pub const DEEP_KEY: f32 = 0.2;
pub const DEEP_AMBIENT: f32 = 0.3;
/// How far a lit delve's own lamps reach in 3D, of what its tone throws.
pub const DEEP_LAMP_REACH: f32 = 0.7;

/// The set texts a page was sent (`receive`), by key.
fn received_texts() -> &'static Mutex<VecDeque<(u64, Arc<str>)>> {
    static RECEIVED: Mutex<VecDeque<(u64, Arc<str>)>> = Mutex::new(VecDeque::new());
    &RECEIVED
}

/// Keep a lit room's set text its host sent (`wire::take_world_item`): a
/// page with no asset folder draws its landmark so. False when it does not
/// read.
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

/// The landmark a place's room stands (`blockout::landmark`), when the room
/// is laid as its item's own: an item built over keeps none.
fn landmark_of(room: &Room, dressing: &Dressing) -> Option<String> {
    let item = super::pilot::all().iter().find(|i| i.id() == dressing.id)?;
    if !matches!(item.source, super::pilot::Source::Place(_)) || !super::fit::same_tiles(&item.room(), room) {
        return None;
    }
    blockout::landmark(dressing.id)
}

// ------------------------------------------------------------- the text ----

/// The lit set of `room` dressed `dressing`: its blockout made monolithic,
/// its ground, its lamps and its flames. Of a whole room (`piece` none),
/// with its landmark when `landmark`; of an open floor's piece, the blockout
/// of its tiles and the map's ring round them (`crop`), cut back to its own
/// tiles (`tiles`), placed on the map.
pub fn room_text(room: &Room, dressing: &Dressing, landmark: bool, piece: Option<(Rect, Rect)>) -> String {
    let (tiles, crop) = piece.unwrap_or(((0, 0, room.cols as i32, room.rows as i32), (0, 0, room.cols as i32, room.rows as i32)));
    let cut = if piece.is_some() { plain::crop_room(room, crop) } else { room.clone() };
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
    // What stands, placed on the map; what burns apart.
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
                let Some(mut cut) = plain::cut_solid(s, keep) else { continue };
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
                shapes.push(blockout::line(&s));
            }
        }
    }
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let mut out = String::new();
    let _ = writeln!(out, "set {}", if dressing.id.is_empty() { "lit" } else { dressing.id });
    let _ = writeln!(out, "# {} lit (docs/REALM-MONOLITH.md, \"Rooms, lit\"): made by angel-delve world3d::rooms_lit", dressing.title);
    let _ = writeln!(out, "# from the room as the game lays it, tiles {},{} {}x{}.", tiles.0, tiles.1, tiles.2, tiles.3);
    let _ = writeln!(out, "camera room {} {}", num(w), num(h));
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
    // Out of doors, a lit window warms the ground before it: people are in.
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
    // The way down is lit.
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

/// How far down a pit goes, and the water, lava and spores' beds; how high
/// the ground rises under what stands on blocked tiles (a dais, below a
/// boot: `blockout::LOW`).
const PIT: f32 = 36.0;
const BED: f32 = 9.0;
const DAIS: f32 = 3.0;

/// The ground of tiles `tiles` of `room` dressed `d`: a height field a
/// quarter of a tile apart (a half for a great room), at the floor line but
/// where every tile a corner touches is a hazard (sunk to its bed, a pit
/// deep), each corner toned as what its tiles' floor is, darker at the foot
/// of what stands.
pub fn ground_line(room: &Room, d: &Dressing, (c0, r0, w, h): Rect) -> String {
    let step = if w * h <= 30 * 18 { TILE / 4.0 } else { TILE / 2.0 };
    let per = (TILE / step) as i32;
    let (cols, rows) = (w * per, h * per);
    let pit = matches!(d.setting, Setting::Dungeon(Pack::Crypt | Pack::Unknown));
    let tile_tone = |c: i32, r: i32| -> f32 {
        match room.tile(c, r) {
            Tile::Hazard if pit => 0.0,
            Tile::Hazard => 10.0,
            Tile::Bog => Kind::Mud.tone() * 100.0 * GROUND_TONE,
            // The way down: a landing brighter than the floor.
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
            // The tiles the corner touches, those in the room.
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
            // What stands on blocked tiles stands on a dais of the ground.
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
                // Flags and earth vary a little, tile by tile and across.
                let (tc, tr) = ((x / TILE).floor() as i32, (z / TILE).floor() as i32);
                let v = crate::render::overworld::ink::hash(tc, tr, 0x5107) % 5;
                let n = crate::render::overworld::ink::vnoise(x / 56.0, z / 44.0, 0x51);
                tone *= 0.96 + 0.02 * v as f32 + 0.08 * (n - 0.5);
                // Darker at the foot of what stands: a tile's width round it.
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

/// A delve's floor is not lit evenly: spill falls from each door along a lane
/// toward the room's heart, brightest at the door and dimming as it goes, its
/// edge crisp, and the floor darkens toward the walls, so the fight sits in
/// the light and the rim falls away (light = activity). A lane is this much
/// brighter than the floor beside it at its door, this many tiles wide, its
/// soft edge starting this much of the way out from its middle, and dimming
/// by this much toward the room's heart.
const LANE_GAIN: f32 = 0.70;
const LANE_WIDTH: f32 = 2.5;
const LANE_EDGE: f32 = 0.8;
const LANE_FADE: f32 = 0.7;
/// The floor loses this much toward the rim, from this much of the way out
/// from the room's middle (1 is the middle of a wall; a corner is 1.41).
const RIM_DARK: f32 = 0.45;
const RIM_FROM: f32 = 0.30;

/// The door tiles' middles of a delve room whose floor is lit by them: the
/// whole room of a delve (a piece of an open floor's map has no middle of
/// its own), none otherwise.
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

/// What floor point `(x, z)` of `room` has its tone scaled by, for the light
/// that falls from `doors` and the dark of the rim: more than 1 in a lane,
/// less toward the walls, 1 for nothing of either.
fn floor_light(room: &Room, doors: &[(f32, f32)], x: f32, z: f32) -> f32 {
    let smooth = |a: f32, b: f32, v: f32| {
        let t = ((v - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let (cx, cz) = (w / 2.0, h / 2.0);
    let out = (((x - cx) / (w / 2.0)).powi(2) + ((z - cz) / (h / 2.0)).powi(2)).sqrt();
    let mut k = 1.0 - RIM_DARK * smooth(RIM_FROM, 1.2, out);
    // The nearest lane: each door to the room's middle.
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

/// An open floor's map is lit from its passages. Each passage opens on its
/// chamber through a mouth, a gap in the wall line, and spill falls from it:
/// a lane this much brighter than the floor beside it at the mouth, this many
/// tiles long into the chamber, dimming by this much along it, its edge
/// crisp, and this many tiles back down the passage. The floor darkens toward
/// every wall by this much, from half a tile of it to this many tiles out, so
/// a chamber's middle is the light and its passages are dark.
const MOUTH_GAIN: f32 = 0.70;
const MOUTH_REACH: f32 = 8.0;
const MOUTH_FADE: f32 = 0.70;
const MOUTH_BACK: f32 = 3.0;
const WALL_DARK: f32 = 0.45;
const WALL_NEAR: f32 = 0.5;
const WALL_FAR: f32 = 5.0;
/// A mouth is a gap of this many tiles along a wall line.
const MOUTH_WIDTH: std::ops::RangeInclusive<i32> = 2..=8;
/// Braziers burn on an open floor: one on each jamb of a mouth (but the near
/// wall's), one along a long wall every this many tiles, one on this many in
/// a hundred of what stands on blocked tiles, a wall's flame this high over
/// the floor.
const SCONCE_EVERY: i32 = 12;
const PILLAR_SHARE: u32 = 20;
const BRAZIER_TOP: f32 = 24.0;

/// A way into a chamber of an open floor's map: a gap in the wall line of a
/// few tiles, a passage's mouth (or a doorway), where light falls from.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Mouth {
    /// The middle of the gap on the chamber's side of the wall line.
    at: [f32; 2],
    /// The way into the chamber, a unit step along an axis.
    into: [i32; 2],
    /// Half the gap's width, in px.
    half: f32,
    /// The wall tiles either side of the gap (its jambs).
    jambs: [(i32, i32); 2],
}

/// The mouths of `map` whose gap starts in tiles `within`: a run of
/// walkable tiles (or doorways) along a wall line, walls at both its ends, the
/// chamber wider than the gap on one side of it and a passage (or nothing
/// but a doorway's own tiles) on the other.
fn mouths(map: &Room, (c0, r0, w, h): Rect) -> Vec<Mouth> {
    let walk = |c: i32, r: i32| blockout::walkable(map.tile(c, r));
    let wall = |c: i32, r: i32| map.tile(c, r) == Tile::Wall;
    let door = |c: i32, r: i32| map.tile(c, r) == Tile::Door;
    let mut out = Vec::new();
    // Gaps along a row: the chamber is south of it (a mouth looking south) or north.
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
    // Gaps down a column: the chamber is east of it (a mouth looking east) or west.
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

/// The light of an open floor's map: spill from its mouths, dark toward its
/// walls.
struct OpenLight<'a> {
    map: &'a Room,
    mouths: Vec<Mouth>,
}

impl OpenLight<'_> {
    /// What floor point `(x, z)`'s tone is scaled by.
    fn at(&self, x: f32, z: f32) -> f32 {
        let smooth = |a: f32, b: f32, v: f32| {
            let t = ((v - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        // The nearest wall, no further than the dark reaches.
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
        // The brightest lane there is at this point.
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

/// The light of tiles `tiles` of `room` dressed `d`: a delve's open map is
/// lit from its mouths and dark toward its walls; none for a room, a place or
/// a chunk of the kingdom.
fn open_light<'a>(room: &'a Room, d: &Dressing, (c0, r0, w, h): Rect) -> Option<OpenLight<'a>> {
    if room.kind != RoomKind::Depths || !matches!(d.setting, Setting::Dungeon(_)) {
        return None;
    }
    let reach = (MOUTH_REACH + MOUTH_BACK).ceil() as i32 + 2;
    Some(OpenLight { map: room, mouths: mouths(room, (c0 - reach, r0 - reach, w + 2 * reach, h + 2 * reach)) })
}

/// The braziers of an open floor's piece `tiles` (the blockout of `groups`
/// cut from `crop`), a bowl and its flame each, in the crop's own pixels:
/// on top of the wall at each jamb of a mouth and along a long wall now and
/// then, never on a wall that looks on the camera, and on some of what
/// stands on blocked tiles.
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
    // The walls beside the mouths of the piece and a margin round it.
    let jambs: Vec<(i32, i32)> = mouths(map, (c0 - 8, r0 - 8, w + 16, h + 16)).into_iter().filter(|m| m.into != [0, -1]).flat_map(|m| m.jambs).collect();
    let mut wanted: Vec<(i32, i32)> = jambs.iter().copied().filter(|&(c, r)| inside(c, r)).collect();
    for r in r0..r0 + h {
        for c in c0..c0 + w {
            if map.tile(c, r) != Tile::Wall || jambs.iter().any(|&(jc, jr)| (jc - c).abs().max((jr - r).abs()) < 6) {
                continue;
            }
            // A wall looking south (its floor to the south), west or east, each along a slant of its own.
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
    // What stands on a blocked tile: the highest of what stands over it.
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

/// The tone (0 to 100) of the floor of tile `(c, r)`: its ground kind's
/// (`Kind::tone`), as the generator lays its ground.
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
    kind.tone() * 100.0 * GROUND_TONE
}

/// The ground under the room's masses is darker than the kind's own tone:
/// the floor is the dark field what stands and the knights read against.
pub const GROUND_TONE: f32 = 0.72;

/// The lamps of flames `flames` and lava `lava`: a lamp over each flame
/// (flames within a hand of each other one lamp), its reach by its size; a
/// lamp every few tiles along a lava pool.
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

/// How far a flame `size` across lights.
fn lamp_reach(size: f32) -> f32 {
    if size >= 12.0 {
        104.0
    } else if size >= 7.0 {
        86.0
    } else {
        66.0
    }
}

// ----------------------------------------------------------- the frame ----

/// A deep ember: the crust of lava and of a bed of coals, an accent.
pub use super::rooms_lit_look::EMBER_DEEP;

/// A colour kept as it is (an accent of the room's look).
fn accent([r, g, b]: [u8; 3]) -> Material {
    Material { accent: true, ..Material::color([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0]) }
}

/// The cracks of fire across bed `bed` (its top), a few to a tile, each
/// showing for a part of every few seconds at `t` (none at all when `t` is
/// 0: a quiet bed's few still cracks).
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
        // Each crack shows for part of its own cycle.
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

/// The flames of `pieces` as a frame draws them, unplaced: full and
/// flickering while the room burns, low embers that only the lamps warm
/// once it is quiet. Ids of their own.
pub fn flame_nodes(pieces: &[Arc<Piece>], activity: Activity, t: f32, fire: [u8; 3]) -> Vec<Node> {
    let mut out = Vec::new();
    for (p, piece) in pieces.iter().enumerate() {
        // Beds of coals and lava: a deep ember crust, cracked with fire that
        // comes and goes; coals sink to a dark crust with the room, lava
        // burns on.
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
                    // Embers: a warm-lit stone, not a light of its own.
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

/// How many of what burns in flight (an ember, a fireball, a blast) light
/// a lit room as lamps, of its look's eight.
pub const SPARKS: usize = 3;

/// A lit room on a stage, as a frame hands it to a window and a page.
pub struct Staged {
    /// What lies on the floor (each piece's ground first), what stands, and
    /// the flames, placed on the stage.
    pub floor: Vec<Node>,
    pub blocks: Vec<Node>,
    pub flames: Vec<Node>,
    /// The pieces' grounds, to upload (`ground_mesh_id` of each key).
    pub meshes: Vec<(u32, Arc<MeshData>)>,
    /// The pieces drawn, in the order their nodes are: key and set text.
    pub sets: Vec<(u64, Arc<str>)>,
    /// The look it is drawn through.
    pub look: angel_stage::look::Look,
}

impl Lit {
    /// The room on a stage: its set's pixel `p` at `p * scale + offset`,
    /// the stage `view` pixels big showing `rect` of the room's stage
    /// (`[x0, z0, x1, z1]`), only the pieces `seen` says are in sight (the
    /// box each fills, on the stage); burning as `moment` says, the knight's
    /// chest (`moment.chest`, the room's own pixels, `zoom` times as big on
    /// the stage) kept in sight; `sparks` (what burns in flight, the room's
    /// own pixels, the strongest first) lighting it before its own lamps.
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
        // As the party first comes down into the deep, the room's lamps come
        // up one after another from the party out (`room::beat`).
        if moment.rise < 1.0 {
            let from = chest.unwrap_or(centre);
            let far = |l: &Lamp| (l.at[0] - from[0]).hypot(l.at[2] - from[2]);
            let farthest = lamps.iter().map(far).fold(1.0f32, f32::max);
            for lamp in &mut lamps {
                lamp.strength *= super::room::lamp_up(moment.rise, far(lamp) / farthest);
            }
        }
        // What burns in flight lights the room first (the room's own pixels).
        let sparks: Vec<Lamp> = sparks
            .iter()
            .take(SPARKS)
            .map(|l| Lamp { at: [l.at[0] * zoom + offset[0], l.at[1] * zoom, l.at[2] * zoom + offset[2]], reach: l.reach * zoom, ..*l })
            .collect();
        let mut look = super::rooms_lit_look::look(self.tone, &lamps, moment.time, centre, reach, chest);
        // A delve keeps the black dungeon's language in 3D (docs/WORLD-3D.md,
        // "Deeper floors"): the key light a glimmer through the stone and
        // the dark between the lamps near black, so what is lit is what
        // burns or happens; coming down into the deep it rises with them.
        if matches!(self.tone, Tone::Delve(_))
            && let Some(sun) = &mut look.sun
        {
            sun.strength *= DEEP_KEY * (0.2 + 0.8 * moment.rise);
            sun.ambient *= DEEP_AMBIENT;
            // Its fires throw pools, as the 2D dungeon's torches do, not a
            // wash over the stone; what burns in flight is added after.
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

// -------------------------------------------------------- the terminal ----

impl Lit {
    /// The room as a page that draws no scenes shows it (a terminal), `size`
    /// pixels big: from above, each tile the tone of what stands tallest on
    /// it (its ground's where nothing does), lit from above, put on the
    /// room's ramp by a 4 x 4 Bayer dither, warmed round its lamps; a pit
    /// black, a flame its own colour. Kept.
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
        // What stands tallest over each tile: its top and kind; and what
        // glows on it (coals, lava).
        let mut tops: Vec<Option<(f32, Kind)>> = vec![None; (cols * rows).max(0) as usize];
        let mut glows = vec![false; (cols * rows).max(0) as usize];
        let mut wet = vec![false; (cols * rows).max(0) as usize];
        let mut lamps = Vec::new();
        for piece in &pieces {
            let (c0, r0, pw, ph) = piece.tiles;
            for r in r0..r0 + ph {
                for c in c0..c0 + pw {
                    let at = (r * cols + c) as usize;
                    if let Some(s) = plain::over_tile(&piece.set.shapes, (c, r)).or_else(|| plain::over_tile(&piece.flames, (c, r))) {
                        tops[at] = Some((s.at[1] + s.size[1], s.kind));
                    }
                    glows[at] = plain::over_tile(&piece.coals, (c, r)).or_else(|| plain::over_tile(&piece.lava, (c, r))).is_some();
                    wet[at] = plain::over_tile(&piece.water, (c, r)).is_some();
                }
            }
            lamps.extend(piece.lamps());
        }
        let ramp = self.tone.ramp();
        let luma = |c: [u8; 3]| (0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2])) / 255.0;
        let levels: Vec<f32> = ramp.iter().map(|&c| luma(c)).collect();
        let bayer = |x: i32, y: i32| angel_stage::look::bayer4()[(y & 3) as usize][(x & 3) as usize] as f32 / 16.0 + 1.0 / 32.0;
        // A level on the ramp, between its two nearest entries by the dither.
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
                    // What stands: its tone, lit from above, the taller
                    // brighter.
                    Some((top, kind)) if top > angel_stage::set::FLOOR_TOP => kind.tone() * (0.72 + (top / 120.0).min(0.35)),
                    _ if self.room.tile(c, r) == Tile::Hazard && pit => continue,
                    Some((_, kind)) => kind.tone() * 0.5,
                    None => {
                        // The floor: dark, in the shadow of what stands to
                        // its south-west (the key light is there).
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

#[cfg(test)]
#[path = "../../tests/world3d/rooms_lit__tests.rs"]
mod tests;
