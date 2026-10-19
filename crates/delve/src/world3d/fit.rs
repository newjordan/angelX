//! Whether a room of the game is an item's room (`assets`), and how it
//! differs from it (docs/WORLD-3D.md, "Loose matching").
//!
//! A place of the kingdom is its item's while its tiles are the item's. A
//! room of a delve (a chamber, a side room, a nook) is its item's while it is
//! laid from the item's template: the same size and tiles but where the game
//! may lay it otherwise, its doors (open or walled up as the floor joins it)
//! and, in a delve that scatters loose blocks over its one-screen chambers,
//! up to its share of them on the template's ground. Those tiles are the
//! room's differences ([`differing`]), drawn as a [`Variant`]:
//!
//! - what the item's set stands on them is cut away (a box down to what is
//!   left of it off them, anything else whole; a doorway's leaf, lintel and
//!   torches with the doorway), so the floor or the doorway shows;
//! - what the generator stands on them for the room as it is (the wall of a
//!   doorway walled up, a boulder rolled in, the floor of a doorway opened)
//!   is drawn as extra blocks, each textured with its kind's patch of the
//!   item's plate (`assets::Swatch`), box-mapped at the plate's density.

use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

use angel_stage::scene::{
    Material, MeshRef, Node, QUAT_IDENTITY, Transform, quat_axis_angle, quat_mul, quat_rotate,
};
use angel_stage::set::{FLOOR_TOP, Kind, PLATE_MARGIN, Patches, Set, Shape};

use super::assets::{Fit, Item};
use super::blockout::{self, Role, Setting, Solid, TILE, merge, walkable};
use super::pilot::{self, Source};
use crate::rules::{Pack, Room, RoomKind, Tile};

/// Whether a room of this kind, on a floor of `pack`, may be the room item
/// `made_from` was made from (its tiles say whether it is): a place by its
/// kind, a chamber in its own delve, a side room or nook by its kind in the
/// delve its stone is.
pub fn may_be(made_from: &pilot::Item, room: &Room, pack: Pack) -> bool {
    let own_delve = match made_from.dressing.setting {
        Setting::Dungeon(delve) => delve == pack,
        _ => true,
    };
    match made_from.source {
        Source::Place(kind) => room.kind == kind,
        Source::Chamber { pack: delve, .. } | Source::Great { pack: delve, .. } => {
            delve == pack && room.kind == RoomKind::Fight
        }
        Source::Nook(kind) => room.kind == kind && own_delve,
        Source::Sanctuary => room.kind == RoomKind::Sanctuary && own_delve,
        Source::Vault => room.kind == RoomKind::Secret && own_delve,
        Source::Forge => room.kind == RoomKind::Forge && own_delve,
    }
}

/// Whether two rooms are laid alike: their size and every tile.
pub fn same_tiles(a: &Room, b: &Room) -> bool {
    (a.cols, a.rows) == (b.cols, b.rows)
        && (0..a.rows as i32).all(|r| (0..a.cols as i32).all(|c| a.tile(c, r) == b.tile(c, r)))
}

/// The tiles where `room` differs from `template`, the room item
/// `made_from` was made from, when it is that room laid as the game may lay
/// it: `None` when it is another room. A place differs nowhere; a room of a
/// delve may differ in its doors (a wall tile of its edge a doorway, or a
/// doorway a wall) and, if it is a one-screen chamber of a delve that
/// scatters loose blocks, in up to four times as many blocks on its ground
/// as the delve scatters (each lands mirrored four ways).
pub fn differing(made_from: &pilot::Item, template: &Room, room: &Room, pack: Pack) -> Option<Vec<(i32, i32)>> {
    if (room.cols, room.rows) != (template.cols, template.rows) {
        return None;
    }
    if matches!(made_from.source, Source::Place(_)) {
        return same_tiles(template, room).then(Vec::new);
    }
    let loose = match made_from.source {
        Source::Chamber { .. } => 4 * pack.scatter(),
        _ => 0,
    };
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let mut differs = Vec::new();
    let mut scattered = 0;
    for r in 0..rows {
        for c in 0..cols {
            let (was, is) = (template.tile(c, r), room.tile(c, r));
            if was == is {
                continue;
            }
            let edge = c == 0 || r == 0 || c == cols - 1 || r == rows - 1;
            let door = edge && matches!((was, is), (Tile::Wall, Tile::Door) | (Tile::Door, Tile::Wall));
            let rolled = !edge && is == Tile::Block;
            scattered += usize::from(rolled);
            if !(door || rolled) || scattered > loose {
                return None;
            }
            differs.push((c, r));
        }
    }
    Some(differs)
}

/// Item `item` as room `room` has it, the room differing from the item's own
/// at tiles `differs` ([`differing`]): `None` when the differences cannot be
/// drawn (a set that does not read once cut).
pub fn fit(item: Arc<Item>, room: &Room, differs: &[(i32, i32)]) -> Option<Fit> {
    if differs.is_empty() {
        return Some(Fit { item, variant: None });
    }
    let variant = Variant::make(&item, room, differs)?;
    Some(Fit { item, variant: Some(Arc::new(variant)) })
}

/// How a room differs from its item's room, and how it is drawn so.
#[derive(Debug)]
pub struct Variant {
    /// Its own number: the item's, its cuts and its extra blocks.
    pub key: u64,
    /// The tiles where the room differs from the item's.
    pub differs: Vec<(i32, i32)>,
    /// The item's set with what stands on those tiles cut away: its text (as
    /// a page that makes the blocks itself is sent it) and as read.
    pub text: Arc<str>,
    pub set: Set,
    /// What the room has there that the item's set has not, in the set's
    /// pixels, each drawn with its kind's patch.
    pub extras: Vec<Shape>,
    /// Its blocks as a world draws them, made once (`Item::made`'s).
    pub(super) made: OnceLock<((u32, Patches), Vec<Node>, Vec<Node>)>,
    /// Its extra blocks as nodes, unplaced, made once.
    pub(super) extra_nodes: OnceLock<Vec<Node>>,
}

/// The first id of an extra block's nodes, clear of the set's own (1 up).
const EXTRA_IDS: u64 = 0x7e00_0000;

/// How much a face's patch is darkened by which way it faces, as the plate
/// paints a block lit from above: its top as it is, its front and its sides
/// in shade.
const FACE_TOP: f32 = 1.0;
const FACE_FRONT: f32 = 0.86;
const FACE_SIDE: f32 = 0.74;

impl Variant {
    fn make(item: &Item, room: &Room, differs: &[(i32, i32)]) -> Option<Variant> {
        let dressing = &item.made_from.dressing;
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        // The tiles that differ, as few rectangles as a greedy pass makes,
        // in the set's pixels: [x0, z0, x1, z1].
        let mut cells = vec![None; room.cols * room.rows];
        for &(c, r) in differs {
            cells[(r * cols + c) as usize] = Some(());
        }
        let rects: Vec<[f32; 4]> = merge(room.cols, room.rows, &cells).into_iter().map(|(rect, ())| blockout::px(rect)).collect();
        // Round a doorway that differs: a tile each way along the wall.
        let doors: Vec<(i32, i32)> =
            differs.iter().copied().filter(|&(c, r)| c == 0 || r == 0 || c == cols - 1 || r == rows - 1).collect();
        let near_door = |foot: [f32; 4]| doors.iter().any(|&t| overlaps(foot, grow(tile_rect(t), TILE)));
        let on_differs = |foot: [f32; 4]| rects.iter().any(|&r| overlaps(foot, r));

        // What the item's set stands on them, cut away. Its generated lines
        // come first, in the generator's order; the landmark's after them.
        let ours = blockout::blockout(&item.room, dressing);
        let roles: Vec<Role> = ours.iter().flat_map(|g| g.solids.iter().map(move |_| g.role)).collect();
        let walls: Vec<[f32; 5]> = item
            .set
            .shapes
            .iter()
            .enumerate()
            .filter(|(i, s)| roles.get(*i) == Some(&Role::Wall) && s.shape == "box")
            .map(|(_, s)| {
                let [x0, z0, x1, z1] = footprint(s);
                [x0, z0, x1, z1, s.at[1] + s.size[1]]
            })
            .collect();
        let mut lines: Vec<String> = item.text.lines().map(str::to_string).collect();
        for (i, shape) in item.set.shapes.iter().enumerate() {
            if shape.on_floor() {
                continue;
            }
            let foot = footprint(shape);
            let door = roles.get(i) == Some(&Role::Door) && near_door(foot);
            if !door && !on_differs(foot) {
                continue;
            }
            let line = lines.get_mut(shape.line.saturating_sub(1))?;
            let mut out = format!("# cut where the room differs: {}", line.trim());
            if !door && is_box(shape) {
                let mut left = vec![foot];
                for &r in &rects {
                    left = left.into_iter().flat_map(|piece| subtract(piece, r)).collect();
                }
                for (k, piece) in left.iter().enumerate() {
                    let id = if k == 0 { shape.id.as_deref() } else { None };
                    let _ = write!(out, "\n{}", box_line(shape.kind, *piece, shape.at[1], shape.size[1], id));
                }
            }
            *line = out;
        }
        let mut text = lines.join("\n");
        text.push('\n');
        let set = Set::parse(&text)
            .map_err(|why| tracing::warn!("world3d: {} cut for a room does not read: {:?}", item.id, why.first()))
            .ok()?;

        // What the generator stands there for the room as it is.
        let theirs = blockout::blockout(room, dressing);
        let mut extra = String::new();
        let opened = |t: (i32, i32)| item.room.tile(t.0, t.1) == Tile::Wall && walkable(room.tile(t.0, t.1));
        for group in &theirs {
            for s in &group.solids {
                let foot = s.footprint();
                let flat = s.top() <= FLOOR_TOP;
                match group.role {
                    // The floor of a doorway opened where the item has wall.
                    Role::Ground | Role::Hazard => {
                        for &t in differs.iter().filter(|&&t| opened(t)) {
                            if let Some(piece) = clip(foot, tile_rect(t)) {
                                let _ = writeln!(extra, "{}", box_line(s.kind, piece, s.bottom(), s.size[1], None));
                            }
                        }
                    }
                    // A doorway the room has where the item has none: its leaf,
                    // lintel and torches whole.
                    Role::Door => {
                        let open = doors.iter().any(|&t| walkable(room.tile(t.0, t.1)) && overlaps(foot, grow(tile_rect(t), TILE)));
                        if open {
                            let _ = writeln!(extra, "{}", line_of(s));
                        }
                    }
                    Role::Wall | Role::Prop if !flat => {
                        if s.shape == blockout::Shape::Box && s.turn.rem_euclid(90.0) == 0.0 {
                            for &r in &rects {
                                let Some(piece) = clip(foot, r) else { continue };
                                // A wall walled up to the height the item's wall
                                // stands at beside it.
                                let mut height = s.size[1];
                                if group.role == Role::Wall {
                                    let beside = walls.iter().filter(|w| touches(piece, [w[0], w[1], w[2], w[3]])).map(|w| w[4]).fold(f32::NAN, f32::max);
                                    if beside.is_finite() {
                                        height = height.min(beside - s.bottom()).max(1.0);
                                    }
                                }
                                let _ = writeln!(extra, "{}", box_line(s.kind, piece, s.bottom(), height, None));
                            }
                        } else {
                            let centre = [(foot[0] + foot[2]) / 2.0, (foot[1] + foot[3]) / 2.0];
                            if rects.iter().any(|r| (r[0]..r[2]).contains(&centre[0]) && (r[1]..r[3]).contains(&centre[1])) {
                                let _ = writeln!(extra, "{}", line_of(s));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let (w, h) = item.set.camera.size();
        let extras = Set::parse(&format!("set extra\ncamera room {w} {h}\n{extra}"))
            .map_err(|why| tracing::warn!("world3d: {}'s extra blocks do not read: {:?}", item.id, why.first()))
            .ok()?
            .shapes;
        let mut key = item.key ^ 0x9e37_79b9_7f4a_7c15;
        for &byte in text.as_bytes().iter().chain(extra.as_bytes()) {
            key ^= u64::from(byte);
            key = key.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Some(Variant {
            key,
            differs: differs.to_vec(),
            text: text.into(),
            set,
            extras,
            made: OnceLock::new(),
            extra_nodes: OnceLock::new(),
        })
    }

    /// Its extra blocks as nodes in the set's world (unplaced): a box's
    /// faces the camera can see (its top, its front and its sides), each cut
    /// into cells of its kind's patch at the plate's density, every other
    /// cell mirrored so the patch tiles; any other shape whole, its patch on
    /// it by the mesh's own uvs and lit. A kind the item has no patch for
    /// (and no other) is its capture colour, lit. `swatch` names the picture
    /// each kind's patch is.
    pub fn extra_nodes(&self, item: &Item, swatch: &dyn Fn(Kind) -> u32) -> &[Node] {
        self.extra_nodes.get_or_init(|| {
            let (w, h) = item.set.camera.size();
            let (pw, ph) = item.set.plate;
            // Plate pixels to a pixel of the set's stage.
            let density = (1.0 - 2.0 * PLATE_MARGIN) * (pw as f32 / w.max(1) as f32).min(ph as f32 / h.max(1) as f32);
            let mut nodes = Vec::new();
            for shape in &self.extras {
                let patch = item.swatch(shape.kind).map(|s| (swatch(s.kind), s.width as f32 / density, s.height as f32 / density));
                let id = EXTRA_IDS + nodes.len() as u64;
                match patch {
                    Some((picture, cw, ch)) if is_box(shape) => box_cells(shape, picture, (cw, ch), &mut nodes),
                    Some((picture, ..)) => {
                        let material = Material { lit: true, emissive: 0.7, ..Material::picture(picture) };
                        let mut node = Node::new(id, shape.mesh, material);
                        node.transform = shape.transform();
                        nodes.push(node);
                    }
                    None => {
                        let [r, g, b] = shape.kind.capture();
                        let tint = [f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0];
                        let mut node = Node::new(id, shape.mesh, Material::lit(tint, if shape.kind.shines() { 1.0 } else { 0.0 }));
                        node.transform = shape.transform();
                        nodes.push(node);
                    }
                }
            }
            for (k, node) in nodes.iter_mut().enumerate() {
                node.id = EXTRA_IDS + k as u64;
            }
            nodes
        })
    }

    /// The extra block standing tallest over tile `(c, r)`, if any stands on
    /// it: what a terminal paints the tile with.
    pub fn over_tile(&self, (c, r): (i32, i32)) -> Option<&Shape> {
        let tile = tile_rect((c, r));
        self.extras.iter().filter(|s| overlaps(footprint(s), tile)).max_by(|a, b| (a.at[1] + a.size[1]).total_cmp(&(b.at[1] + b.size[1])))
    }
}

/// A box's seen faces as cells of its patch, `cell` set pixels a cell.
pub(super) fn box_cells(shape: &Shape, picture: u32, cell: (f32, f32), out: &mut Vec<Node>) {
    let [sx, sy, sz] = shape.size;
    let turn = quat_axis_angle([0.0, 1.0, 0.0], shape.turn.to_radians());
    let up = quat_axis_angle([1.0, 0.0, 0.0], -std::f32::consts::FRAC_PI_2);
    let right = quat_axis_angle([0.0, 1.0, 0.0], std::f32::consts::FRAC_PI_2);
    let left = quat_axis_angle([0.0, 1.0, 0.0], -std::f32::consts::FRAC_PI_2);
    // Each face: its middle (about the foot, before the turn), how it lies,
    // its extent across and up its picture, and its shade.
    let faces = [
        ([0.0, sy, 0.0], up, (sx, sz), FACE_TOP),
        ([0.0, sy / 2.0, sz / 2.0], QUAT_IDENTITY, (sx, sy), FACE_FRONT),
        ([sx / 2.0, sy / 2.0, 0.0], right, (sz, sy), FACE_SIDE),
        ([-sx / 2.0, sy / 2.0, 0.0], left, (sz, sy), FACE_SIDE),
    ];
    for (middle, lie, (across, along), shade) in faces {
        if across <= 0.0 || along <= 0.0 {
            continue;
        }
        let (nu, nv) = ((across / cell.0).round().max(1.0) as u32, (along / cell.1).round().max(1.0) as u32);
        let (cu, cv) = (across / nu as f32, along / nv as f32);
        let rotation = quat_mul(turn, lie);
        for iu in 0..nu {
            for iv in 0..nv {
                // The cell's middle in the face's own plane, x across, y up it.
                let local = [(iu as f32 + 0.5) * cu - across / 2.0, (iv as f32 + 0.5) * cv - along / 2.0, 0.0];
                let on_face = quat_rotate(lie, local);
                let offset = quat_rotate(turn, [middle[0] + on_face[0], middle[1] + on_face[1], middle[2] + on_face[2]]);
                let flip = |odd: bool| if odd { -1.0 } else { 1.0 };
                let material = Material {
                    tint: [shade, shade, shade, 1.0],
                    double_sided: true,
                    ..Material::picture(picture)
                };
                let mut node = Node::new(0, MeshRef::Quad, material);
                node.transform = Transform {
                    translation: [shape.at[0] + offset[0], shape.at[1] + offset[1], shape.at[2] + offset[2]],
                    rotation,
                    scale: [cu * flip(iu % 2 == 1), cv * flip(iv % 2 == 1), 1.0],
                };
                out.push(node);
            }
        }
    }
}

/// Tile `(c, r)` in the set's pixels: [x0, z0, x1, z1].
fn tile_rect((c, r): (i32, i32)) -> [f32; 4] {
    blockout::px((c, r, 1, 1))
}

fn grow([x0, z0, x1, z1]: [f32; 4], by: f32) -> [f32; 4] {
    [x0 - by, z0 - by, x1 + by, z1 + by]
}

/// How far two footprints must overlap to be said to: a quarter pixel.
const EPS: f32 = 0.25;

pub(super) fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] - EPS && b[0] < a[2] - EPS && a[1] < b[3] - EPS && b[1] < a[3] - EPS
}

/// Whether two footprints share an edge (or overlap).
fn touches(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] <= b[2] + EPS && b[0] <= a[2] + EPS && a[1] <= b[3] + EPS && b[1] <= a[3] + EPS
}

pub(super) fn clip(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let c = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
    (c[2] - c[0] > EPS && c[3] - c[1] > EPS).then_some(c)
}

/// What is left of footprint `a` off `b`: up to four rectangles.
pub(super) fn subtract(a: [f32; 4], b: [f32; 4]) -> Vec<[f32; 4]> {
    let Some(cut) = clip(a, b) else {
        return vec![a];
    };
    let mut out = Vec::new();
    let mut keep = |r: [f32; 4]| {
        if r[2] - r[0] > EPS && r[3] - r[1] > EPS {
            out.push(r);
        }
    };
    keep([a[0], a[1], cut[0], a[3]]);
    keep([cut[2], a[1], a[2], a[3]]);
    keep([cut[0], a[1], cut[2], cut[1]]);
    keep([cut[0], cut[3], cut[2], a[3]]);
    out
}

/// A set shape's footprint, [x0, z0, x1, z1] (a turned one's bounding square).
pub(super) fn footprint(s: &Shape) -> [f32; 4] {
    let (hx, hz) = (s.size[0] / 2.0, s.size[2] / 2.0);
    let (sin, cos) = s.turn.to_radians().sin_cos();
    let (hx, hz) = (cos.abs() * hx + sin.abs() * hz, sin.abs() * hx + cos.abs() * hz);
    [s.at[0] - hx, s.at[2] - hz, s.at[0] + hx, s.at[2] + hz]
}

/// Whether a set shape is a box square to the room (its footprint is it).
pub(super) fn is_box(s: &Shape) -> bool {
    s.shape == "box" && s.turn.rem_euclid(90.0).min(90.0 - s.turn.rem_euclid(90.0)) < 1e-3
}

/// A box over footprint `[x0, z0, x1, z1]`, its bottom at `y`, `h` high, as a line of a set.
pub(super) fn box_line(kind: Kind, [x0, z0, x1, z1]: [f32; 4], y: f32, h: f32, id: Option<&str>) -> String {
    let mut line = blockout::line(&Solid::block(kind, [x0, z0, x1, z1], y, h));
    if let Some(id) = id {
        let _ = write!(line, " id {id}");
    }
    line
}

/// A generated solid as a line of a set, its id left off (the item's set
/// may have it).
pub(super) fn line_of(s: &Solid) -> String {
    let mut s = s.clone();
    s.id = None;
    s.todo = None;
    blockout::line(&s)
}

#[cfg(test)]
#[path = "../../tests/world3d/fit__tests.rs"]
mod tests;
