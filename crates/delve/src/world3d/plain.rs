//! A room with no item of its own (docs/WORLD-3D.md, "Rooms with no item"):
//! stood up in 3D as its own blockout (`blockout::room_set`, the generator
//! the items were made from, so its walls, doors and blocks are where the
//! room's tiles are), every block in its theme's patches (`theme`), so a
//! floor walked in 3D never flips between painted rooms and flat ones.
//!
//! - **Its dressing.** A room of a kind an item was made from (a nook, the
//!   Sanctuary, the vault, the forge-hall, a place of the kingdom built
//!   over) is dressed as that item, in this floor's stone when the item is
//!   a delve's; a room of the company's home that no item is, as an
//!   interior; any other as a chamber of its delve. Its theme is its
//!   dressing's (`Theme::of`).
//! - **Its blocks.** Each of its kind's patch of the theme's, as a room's
//!   extra blocks are (`fit::box_cells`: a box's faces cut into cells of the
//!   patch, every other mirrored, shaded as the plate lights a block; any
//!   other shape lit with its patch on it); a kind the theme has no patch
//!   of in its capture colour two shades down, lit (`kingdom::darker`).
//! - **An open floor's map** (`RoomKind::Depths`, up to 168 x 108 tiles) is
//!   bigger than a stage: it is stood up in pieces of a kingdom chunk's
//!   size ([`PIECE`]), each made the first time it comes near a view
//!   (`kingdom::near`, the view's chunks and a ring round them) and kept
//!   while its tiles stay as they are. A piece is the blockout of its tiles
//!   and a tile of the map round them, cut back to its own tiles, so what
//!   stands at its edge is the map's and not a room's wall.
//! - **A terminal** paints each tile with the patch of what stands tallest
//!   on it, as it paints a room's extra blocks.
//!
//! Side-on halls (the Ledge, the Wanted hall) are seen from the side, not
//! from above, and the kingdom is its chunks (`kingdom`): none is drawn so.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::scene::{Material, Node};
use angel_stage::set::{Kind, Set, Shape};

use super::blockout::{self, Dressing, Rect, Setting, TILE};
use super::kingdom::{self, Blocks};
use super::pilot::{self, Source};
use super::theme::{self, Patches, Theme};
use crate::render::overworld::Img;
use crate::rules::{Pack, Room, RoomKind};

/// An open floor's piece: a kingdom chunk's tiles.
pub const PIECE: (i32, i32) = (crate::world::map::CHUNK_W, crate::world::map::CHUNK_H);
/// How many pieces are kept made (a map has at most 7 x 7).
const PIECES_KEPT: usize = 64;
/// How a terminal shades what lies on the floor, under what stands.
pub const FLOOR_SHADE: f32 = 0.8;
/// How many of the ring round a view's pieces not yet made are made a call.
pub const RING_A_CALL: usize = 2;
/// How many rooms' answers ([`plain_for`]) are kept.
const PLAINS_KEPT: usize = 8;

/// A room with no item, as it is stood up: its theme's patches and its pieces.
pub struct Plain {
    pub theme: Theme,
    pub patches: Arc<Patches>,
    /// The room and its patches, the same number.
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

impl std::fmt::Debug for Plain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plain").field("theme", &self.theme).field("id", &self.id).field("grid", &self.grid).field("key", &self.key).finish()
    }
}

/// One piece of a plain room: its blocks, in the room's own stage pixels.
pub struct Piece {
    pub cell: (i32, i32),
    /// The room's tiles it stands for: column, row, width, height.
    pub tiles: Rect,
    /// Its blocks, placed on the room's stage.
    pub shapes: Vec<Shape>,
    /// The set they are cut from (the room's, or for a piece of an open
    /// floor its tiles' and the ring round them, on a stage of their own).
    pub text: Arc<str>,
    /// The box its blocks fill on the room's stage, low and high corners.
    pub bounds: ([f32; 3], [f32; 3]),
    pub key: u64,
    made: Mutex<Option<(u64, Arc<Blocks>)>>,
}

impl Plain {
    /// Its room's size in its own stage pixels.
    pub fn stage(&self) -> (f32, f32) {
        (self.room.cols as f32 * TILE, self.room.rows as f32 * TILE)
    }

    /// Whether it is an open floor's map, stood up in pieces.
    pub fn open(&self) -> bool {
        self.grid != (1, 1)
    }

    /// Piece `cell`, made the first time it is asked for (kept across the
    /// room's changes while its own tiles stay as they are); `None` off the
    /// grid or when its set does not read.
    pub fn piece(&self, (cx, cz): (i32, i32)) -> Option<Arc<Piece>> {
        if !(0..self.grid.0).contains(&cx) || !(0..self.grid.1).contains(&cz) {
            return None;
        }
        let slot = &self.pieces[(cz * self.grid.0 + cx) as usize];
        slot.get_or_init(|| piece_of(&self.room, &self.dressing, self.open(), (cx, cz))).clone()
    }

    /// Every piece (a terminal's whole room).
    pub fn every_piece(&self) -> Vec<Arc<Piece>> {
        (0..self.grid.1).flat_map(|cz| (0..self.grid.0).map(move |cx| (cx, cz))).filter_map(|c| self.piece(c)).collect()
    }

    /// The pieces near a view of `[x0, z0, x1, z1]` (the room's stage
    /// pixels): every piece for a room; for an open floor the pieces the view
    /// overlaps and a ring round them, as the kingdom's chunks are chosen
    /// (`kingdom::near`).
    ///
    /// A piece the view overlaps is made when it is first asked for; of the
    /// ring, at most [`RING_A_CALL`] not yet made are made a call (a frame),
    /// so walking onto a map or across a piece's edge never waits on more
    /// than a few.
    pub fn near(&self, view: [f32; 4]) -> Vec<Arc<Piece>> {
        if !self.open() {
            return self.every_piece();
        }
        let (pw, ph) = (PIECE.0 as f32 * TILE, PIECE.1 as f32 * TILE);
        let (c0, c1) = ((view[0] / pw).floor() as i32, ((view[2] - 0.01) / pw).floor() as i32);
        let (r0, r1) = ((view[1] / ph).floor() as i32, ((view[3] - 0.01) / ph).floor() as i32);
        let mut budget = RING_A_CALL;
        kingdom::near(view)
            .into_iter()
            .map(kingdom::cell)
            .filter(|&(cx, cz)| cx < self.grid.0 && cz < self.grid.1)
            .filter_map(|(cx, cz)| {
                let shown = (c0..=c1).contains(&cx) && (r0..=r1).contains(&cz);
                let made = self.pieces[(cz * self.grid.0 + cx) as usize].get().is_some();
                if !shown && !made {
                    if budget == 0 {
                        return None;
                    }
                    budget -= 1;
                }
                self.piece((cx, cz))
            })
            .collect()
    }

    /// Kind `kind`'s patch, as a block is drawn with it: the picture
    /// `swatch(kind)` and how many stage pixels one copy of it covers.
    fn patch(&self, kind: Kind, swatch: &dyn Fn(Kind) -> u32) -> Option<(u32, f32, f32)> {
        self.patches.swatch(kind).map(|s| {
            let (w, h) = theme::cell(s);
            (swatch(kind), w, h)
        })
    }

    /// Piece `piece`'s blocks as a world draws them, unplaced, each of its
    /// kind's patch (picture `swatch(kind)`); made once for the patches and
    /// pictures they were asked with. Its nodes' ids are its own.
    pub fn blocks(&self, piece: &Piece, swatch: &dyn Fn(Kind) -> u32) -> Arc<Blocks> {
        let mut key = self.patches.key ^ u64::from(swatch(Kind::ALL[0])).rotate_left(17);
        key = key.wrapping_mul(0x0000_0100_0000_01b3);
        let mut made = piece.made.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((k, blocks)) = made.as_ref()
            && *k == key
        {
            return Arc::clone(blocks);
        }
        let mut blocks = Blocks::default();
        for shape in &piece.shapes {
            let mut nodes = Vec::new();
            match self.patch(shape.kind, swatch) {
                Some((picture, cw, ch)) if super::fit::is_box(shape) => super::fit::box_cells(shape, picture, (cw, ch), &mut nodes),
                Some((picture, ..)) => {
                    // As a room's extra blocks: lit, mostly its paint.
                    let material = Material { lit: true, emissive: 0.7, ..Material::picture(picture) };
                    let mut node = Node::new(0, shape.mesh, material);
                    node.transform = shape.transform();
                    nodes.push(node);
                }
                None => {
                    let [r, g, b] = kingdom::darker(shape.kind.capture());
                    let tint = [f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0];
                    let mut node = Node::new(0, shape.mesh, Material::lit(tint, if shape.kind.shines() { 1.0 } else { 0.0 }));
                    node.transform = shape.transform();
                    nodes.push(node);
                }
            }
            if shape.on_floor() { blocks.floor.extend(nodes) } else { blocks.standing.extend(nodes) }
        }
        // A scene draws one node an id: each piece's its own.
        let ids = (u64::from((piece.cell.1 * self.grid.0 + piece.cell.0) as u32) + 1) << 32;
        for (k, node) in blocks.floor.iter_mut().chain(&mut blocks.standing).enumerate() {
            node.id = ids | (k as u64 + 1);
        }
        let blocks = Arc::new(blocks);
        *made = Some((key, Arc::clone(&blocks)));
        blocks
    }

    /// The room as a terminal draws it, `size` pixels big: each tile the
    /// patch of what stands tallest on it (`over_tile`), tiled mirrored at
    /// the theme's density, its kind's colour two shades down where the
    /// theme has no patch of it, black where nothing is (a pit). What lies on
    /// the floor is shaded [`FLOOR_SHADE`], so what stands reads over it.
    /// Kept.
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
        let (cols, rows) = (self.room.cols as i32, self.room.rows as i32);
        let mut tops: Vec<Option<(f32, Kind)>> = vec![None; (cols * rows).max(0) as usize];
        for piece in self.every_piece() {
            let (c0, r0, pw, ph) = piece.tiles;
            for r in r0..r0 + ph {
                for c in c0..c0 + pw {
                    if let Some(shape) = over_tile(&piece.shapes, (c, r)) {
                        tops[(r * cols + c) as usize] = Some((shape.at[1] + shape.size[1], shape.kind));
                    }
                }
            }
        }
        let k = w.max(1) as f32 / self.stage().0.max(1.0);
        let tile = TILE * k;
        let mut pixels = vec![[0u8; 3]; (w.max(0) * h.max(0)) as usize];
        let mirrored = |v: f32, n: u32| {
            let n = n.max(1) as i32;
            let t = (v.floor() as i32).rem_euclid(2 * n);
            (if t < n { t } else { 2 * n - 1 - t }) as u32
        };
        for r in 0..rows {
            for c in 0..cols {
                let Some((top, kind)) = tops[(r * cols + c) as usize] else { continue };
                let swatch = self.patches.swatch(kind);
                let flat = kingdom::darker(kind.capture());
                let shade = |rgb: [u8; 3]| {
                    if top <= angel_stage::set::FLOOR_TOP { rgb.map(|v| (f32::from(v) * FLOOR_SHADE).round() as u8) } else { rgb }
                };
                let (x0, y0) = ((c as f32 * tile).round() as i32, (r as f32 * tile).round() as i32);
                let (x1, y1) = ((((c + 1) as f32) * tile).round() as i32, (((r + 1) as f32) * tile).round() as i32);
                for y in y0.max(0)..y1.min(h) {
                    for x in x0.max(0)..x1.min(w) {
                        pixels[(y * w + x) as usize] = shade(match swatch {
                            Some(s) => {
                                // The room pixel in the patch's pixels, mirrored into it.
                                let px = mirrored(x as f32 / k * theme::DENSITY, s.width);
                                let py = mirrored(y as f32 / k * theme::DENSITY, s.height);
                                let i = ((py * s.width + px) * 4) as usize;
                                [s.rgba[i], s.rgba[i + 1], s.rgba[i + 2]]
                            }
                            None => flat,
                        });
                    }
                }
            }
        }
        Img::from_rgb(w, h, &pixels)
    }
}

/// What stands tallest over tile `(c, r)` of `shapes` (the room's stage
/// pixels), what a terminal paints the tile with: of the shapes over the
/// tile's middle (a round one over its round footprint) that cover a
/// quarter of a tile at least (not a torch on a wall), the one whose top is
/// highest.
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

// ------------------------------------------------------------ the rooms ----

/// Whether rooms with no item stand up plain while rooms are drawn in 3D:
/// only when the host says `ANGEL_WORLD3D_PLAIN=1` (otherwise they are drawn
/// flat, as ever, and only items' rooms stand up: unpainted, a theme's
/// patches read darker and muddier than the flat art). A page across a wire
/// stands them up only when its host sends their theme (`wire::WORLD_THEME`).
pub fn on() -> bool {
    #[cfg(test)]
    if TEST_ON.with(|c| c.get()) {
        return true;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("ANGEL_WORLD3D_PLAIN").is_ok_and(|v| v == "1"))
}

#[cfg(test)]
thread_local! {
    static TEST_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Stand rooms with no item up on this thread as `ANGEL_WORLD3D_PLAIN=1` does.
#[cfg(test)]
pub fn set_on(on: bool) {
    TEST_ON.with(|c| c.set(on));
}

/// The generic dressings' ids, a theme each (`Theme::ALL`'s order).
const IDS: [&str; 12] = [
    "plain-crypt",
    "plain-cavern",
    "plain-hellforge",
    "plain-archive",
    "plain-fungal",
    "plain-unknown",
    "plain-swamp",
    "plain-interior",
    "plain-hall",
    "plain-tower",
    "plain-college",
    "plain-outdoor",
];

/// The rooms of the company's home wing that no item is: interiors. (A
/// `Hall` is a stone passage between chambers, at home as below.)
fn at_home(kind: RoomKind) -> bool {
    matches!(kind, RoomKind::Stockpile | RoomKind::Muster | RoomKind::Workshop | RoomKind::Quarters | RoomKind::RealmHall)
}

/// Whether item `item` was made from a room of `room`'s kind (whatever its
/// delve): a place, a nook, the Sanctuary, the vault, the forge-hall.
fn kin(item: &pilot::Item, room: &Room) -> bool {
    match item.source {
        Source::Place(kind) | Source::Nook(kind) => kind == room.kind,
        Source::Sanctuary => room.kind == RoomKind::Sanctuary,
        Source::Vault => room.kind == RoomKind::Secret,
        Source::Forge => room.kind == RoomKind::Forge,
        Source::Chamber { .. } | Source::Great { .. } => false,
    }
}

/// Whether every rectangle dressing `d` names lies in `room`.
fn fits_in(d: &Dressing, room: &Room) -> bool {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let inside = |&(c, r, w, h): &Rect| c >= 0 && r >= 0 && c + w <= cols && r + h <= rows;
    d.props.iter().all(|(rect, _)| inside(rect)) && d.roads.iter().all(inside) && d.paved.iter().all(inside)
}

/// What room `room` (on a floor of `pack`) with no item is dressed as, and
/// its theme: the dressing of the item made from a room of its kind, in
/// this floor's stone when that is a delve's; else an interior for a room
/// of the company's home, a chamber of its delve for any other. `None` for a
/// room not drawn so: the kingdom, and the side-on halls.
pub fn dressing_for(room: &Room, pack: Pack) -> Option<(Dressing, Theme)> {
    if matches!(room.kind, RoomKind::Kingdom | RoomKind::Ledge | RoomKind::Wanted) {
        return None;
    }
    if let Some(item) = pilot::all().iter().find(|i| kin(i, room) && fits_in(&i.dressing, room)) {
        return Some(match item.dressing.setting {
            Setting::Dungeon(_) => {
                let setting = Setting::Dungeon(pack);
                (Dressing { setting, ..item.dressing }, Theme::of_setting(setting))
            }
            _ => (item.dressing, Theme::of(item)),
        });
    }
    let setting = if at_home(room.kind) { Setting::Indoor } else { Setting::Dungeon(pack) };
    let theme = Theme::of_setting(setting);
    let dressing = Dressing {
        id: IDS[theme.slot() as usize],
        title: "A room with no painted item, its own blockout in its theme's patches",
        setting,
        ..Dressing::BASE
    };
    Some((dressing, theme))
}

/// The theme room `room` (on a floor of `pack`) is drawn in when it has no
/// item; `None` for a room not drawn so.
pub fn theme_for(room: &Room, pack: Pack) -> Option<Theme> {
    dressing_for(room, pack).map(|(_, theme)| theme)
}

/// Room `room` with no item (on a floor of `pack`), stood up plain, when
/// its theme has patches (the asset folder's, or those a page was sent):
/// `None` for a room not drawn so, or a theme with none (it is drawn flat,
/// as ever). Kept for the last few rooms asked about.
pub fn plain_for(room: &Room, pack: Pack) -> Option<Arc<Plain>> {
    type Kept = Mutex<VecDeque<(u64, Option<Arc<Plain>>)>>;
    let (dressing, theme) = dressing_for(room, pack)?;
    let patches = super::assets::themes().get(theme)?;
    let mut key = room.fingerprint() ^ patches.key.rotate_left(29);
    for &byte in dressing.id.as_bytes().iter().chain(theme.name().as_bytes()) {
        key ^= u64::from(byte);
        key = key.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Its key holds its patches': a test's own folder's are its own.
    static KEPT: Kept = Mutex::new(VecDeque::new());
    if let Ok(kept) = KEPT.lock()
        && let Some((_, plain)) = kept.iter().find(|(k, _)| *k == key)
    {
        return plain.clone();
    }
    let open = room.cols > crate::rules::layout::MAX_COLS || room.rows > crate::rules::layout::MAX_ROWS;
    let grid = if open {
        ((room.cols as i32 + PIECE.0 - 1) / PIECE.0, (room.rows as i32 + PIECE.1 - 1) / PIECE.1)
    } else {
        (1, 1)
    };
    let plain = Some(Arc::new(Plain {
        theme,
        patches,
        key,
        id: dressing.id,
        grid,
        room: room.clone(),
        dressing,
        pieces: (0..grid.0 * grid.1).map(|_| OnceLock::new()).collect(),
        picture: Mutex::new(None),
    }));
    if let Ok(mut kept) = KEPT.lock() {
        if kept.len() >= PLAINS_KEPT {
            kept.pop_front();
        }
        kept.push_back((key, plain.clone()));
    }
    plain
}

/// Piece `cell` of room `room` dressed `dressing`: the whole room's
/// blockout, or (`open`) the blockout of the piece's tiles and a tile of
/// the map round them, cut back to its own tiles. Kept by what it is made
/// of, so a map that changes elsewhere keeps the pieces it had.
fn piece_of(room: &Room, dressing: &Dressing, open: bool, (cx, cz): (i32, i32)) -> Option<Arc<Piece>> {
    type Kept = Mutex<VecDeque<(u64, Arc<Piece>)>>;
    static KEPT: Kept = Mutex::new(VecDeque::new());
    let (tiles, crop) = piece_rects(room, open, (cx, cz));
    let mut key = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |v: u64| {
        key ^= v;
        key = key.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for v in [crop.0, crop.1, crop.2, crop.3, tiles.0, tiles.1] {
        mix(v as u64);
    }
    for &byte in dressing.id.as_bytes() {
        mix(u64::from(byte));
    }
    mix(match dressing.setting {
        Setting::Dungeon(pack) => pack as u64 + 1,
        _ => 0,
    });
    for r in crop.1..crop.1 + crop.3 {
        for c in crop.0..crop.0 + crop.2 {
            mix(room.tile(c, r) as u64);
        }
    }
    if let Ok(kept) = KEPT.lock()
        && let Some((_, piece)) = kept.iter().find(|(k, _)| *k == key)
    {
        return Some(Arc::clone(piece));
    }
    let cut = if open { crop_room(room, crop) } else { room.clone() };
    let text: Arc<str> = blockout::room_set(&cut, dressing).into();
    let set = Set::parse(&text)
        .map_err(|why| tracing::warn!("world3d: a plain room's set does not read: {:?}", why.first()))
        .ok()?;
    let shapes = if open {
        let (c0, r0, w, h) = tiles;
        let keep = blockout::px((c0 - crop.0, r0 - crop.1, w, h));
        let origin = [crop.0 as f32 * TILE, crop.1 as f32 * TILE];
        set.shapes.iter().filter_map(|s| cut_to(s, keep)).map(|mut s| {
            s.at[0] += origin[0];
            s.at[2] += origin[1];
            s
        }).collect()
    } else {
        set.shapes
    };
    let piece = Arc::new(Piece { cell: (cx, cz), tiles, bounds: bounds_of(&shapes, tiles), shapes, text, key, made: Mutex::new(None) });
    if let Ok(mut kept) = KEPT.lock() {
        if kept.len() >= PIECES_KEPT {
            kept.pop_front();
        }
        kept.push_back((key, Arc::clone(&piece)));
    }
    Some(piece)
}

/// Piece `cell`'s tiles of room `room` (all of them but on an open floor),
/// and the tiles its blockout is cut from: the same, or (`open`) those and a
/// tile of the map round them where the map goes on.
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

/// Tiles `rect` of `room` as a room of their own.
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

/// Shape `s` cut back to footprint `keep` (`[x0, z0, x1, z1]`): a box square
/// to the room down to what of it lies inside, any other shape whole when its
/// foot's middle does; `None` when nothing of it does.
fn cut_to(s: &Shape, keep: [f32; 4]) -> Option<Shape> {
    const EPS: f32 = 0.25;
    if super::fit::is_box(s) {
        let quarter = (s.turn.rem_euclid(180.0) - 90.0).abs() < 45.0;
        let (hx, hz) = if quarter { (s.size[2] / 2.0, s.size[0] / 2.0) } else { (s.size[0] / 2.0, s.size[2] / 2.0) };
        let foot = [s.at[0] - hx, s.at[2] - hz, s.at[0] + hx, s.at[2] + hz];
        let cut = [foot[0].max(keep[0]), foot[1].max(keep[1]), foot[2].min(keep[2]), foot[3].min(keep[3])];
        if cut[2] - cut[0] <= EPS || cut[3] - cut[1] <= EPS {
            return None;
        }
        if cut == foot {
            return Some(s.clone());
        }
        let mut out = s.clone();
        out.turn = 0.0;
        out.at = [(cut[0] + cut[2]) / 2.0, s.at[1], (cut[1] + cut[3]) / 2.0];
        out.size = [cut[2] - cut[0], s.size[1], cut[3] - cut[1]];
        out.id = None;
        return Some(out);
    }
    let inside = s.at[0] >= keep[0] && s.at[0] < keep[2] && s.at[2] >= keep[1] && s.at[2] < keep[3];
    inside.then(|| s.clone())
}

/// A blockout's solid `s` cut back to footprint `keep` (`[x0, z0, x1,
/// z1]`), as [`cut_to`] cuts a set's shape: a box square to the room down to
/// what of it lies inside, any other solid whole when its foot's middle
/// does; `None` when nothing of it does (a lit room's piece,
/// `rooms_lit`).
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

/// The box `shapes` fill (a turned one's footprint taken as the circle round
/// it); the floor of tiles `tiles` when there are none.
fn bounds_of(shapes: &[Shape], (c, r, w, h): Rect) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for s in shapes {
        let reach = s.size[0].hypot(s.size[2]) / 2.0;
        let box_lo = [s.at[0] - reach, s.at[1], s.at[2] - reach];
        let box_hi = [s.at[0] + reach, s.at[1] + s.size[1].max(0.0), s.at[2] + reach];
        for k in 0..3 {
            lo[k] = lo[k].min(box_lo[k]);
            hi[k] = hi[k].max(box_hi[k]);
        }
    }
    if lo[0] > hi[0] {
        let [x0, z0, x1, z1] = blockout::px((c, r, w, h));
        return ([x0, 0.0, z0], [x1, 0.0, z1]);
    }
    (lo, hi)
}

#[cfg(test)]
#[path = "../../tests/world3d/plain__tests.rs"]
mod tests;
