//! The hillside works in the realm's 3D (`world::hillside`): the production
//! sim's buildings, roads and the tracks of its lines stood up as the kit's
//! blocks (`world3d::works_kit`), lit in the monolith look, and the ghost of
//! what a knight is about to lay. What moves (carts, wagons, buckets, people)
//! stands up as figures (`render::overworld::arena::hillside_art`), the flow
//! overlay lies on the ground.
//!
//! Performance: a building's blocks are made once per `(piece, state, step,
//! place)` and shared, so a works standing still costs a copy of its nodes a
//! frame, and only those in sight are copied. Each building also says where it
//! stands and of what it is made, for the look's material ramps
//! (`works_look`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::scene::{AlphaMode, Material, Node, Transform};
use angel_stage::set::{Kind, SetLamp, Shape as SetShape};

use super::blockout::Solid;
use super::hill_mesh::{self, Baked, HillPart};
use super::kit::Face;
use super::works_kit::{self, LineKind, belts};
use crate::kingdom::Piece;
use crate::render::overworld::works_kit::rollers as kroll;
use crate::render::overworld::works_kit::{self as flat, State};
use crate::rules::Run;
use crate::rules::hillside::{self as build, Ghost, Tool, Verdict};
use crate::world::hillside::{self as hill, HillView};

/// The first id of the works' nodes, clear of the chunks', the lots' and the Keep's.
pub const IDS: u64 = 0x7D00_0000_0000;

/// A building's footprint in kingdom pixels and what it is, for the look's material zones.
#[derive(Clone, Debug)]
pub struct Footprint {
    /// `[x0, z0, x1, z1]`.
    pub rect: [f32; 4],
    pub plan: String,
}

/// What the works stand as this frame: meshes (`hill_mesh`: a building, a carrier or a chunk of road
/// each one draw, not forty), and the ghost's nodes.
#[derive(Debug, Default)]
pub struct HillNodes {
    /// The buildings, roads, lines and what runs on them, baked.
    pub parts: Vec<HillPart>,
    pub lamps: Vec<SetLamp>,
    pub footprints: Vec<Footprint>,
    /// The ghost, apart (it changes every tick, the works once a second).
    pub ghost: Vec<Node>,
    pub bounds: ([f32; 3], [f32; 3]),
}

/// How much brighter what moves is drawn than the same kind standing still: a cart on its road is a
/// small dark thing in a dim world, and the look puts a tone on a ramp of eight, so a carrier's
/// tones are lifted (a wagon's iron and timber, an ox's hide) to stand off the ground it runs on.
const CARRIER_SCALE: f32 = 1.25;

/// How many carriers a works may have on its roads and lines before they are drawn in few blocks.
pub const MANY_CARRIERS: usize = 12;

/// A chunk of the kingdom in tiles (`world3d::kingdom::STAGE`: 384 by 256 pixels).
const CHUNK_TILES: (i32, i32) = (24, 16);

/// A barge's hull, pixels: its length and its beam.
const BARGE_LENGTH: f32 = 72.0;
const BARGE_BEAM: f32 = 16.0;

/// How many of the roads about the Keep the look dyes, and how far (tiles) from its ground a road is about it.
pub const ROAD_RUNS: usize = 6;
pub const KEEP_REACH: i16 = 14;

fn lifted(t: f32) -> f32 {
    (t * 1.55 + 0.04).min(0.95)
}

/// A solid as a node, tone and role as the realm draws its blocks (`lots::Share::lit`).
fn node_of(s: &Solid, alpha: Option<f32>) -> Option<Node> {
    node_with(s, alpha, |t| t)
}

fn node_with(s: &Solid, alpha: Option<f32>, tone: impl Fn(f32) -> f32) -> Option<Node> {
    let mesh = angel_stage::shapes::by_name(&s.shape.written())?;
    let shape = SetShape { line: 0, shape: s.shape.written(), mesh, kind: s.kind, at: s.at, size: s.size, turn: s.turn, id: None, top: None };
    let material = if s.kind.shines() {
        let [r, g, b] = s.kind.capture();
        Material { accent: true, ..Material::color([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0]) }
    } else {
        let t = tone(s.kind.tone());
        match alpha {
            Some(a) => Material { alpha: AlphaMode::Blend, role: s.kind.role(), ..Material::lit([t, t, t, a], 0.0) },
            None => Material { role: s.kind.role(), ..Material::lit([t, t, t, 1.0], 0.0) },
        }
    };
    let mut node = Node::new(0, mesh, material);
    node.transform = shape.transform();
    Some(node)
}

type Cache<K> = OnceLock<Mutex<HashMap<K, Arc<Vec<Node>>>>>;

fn kept<K: std::hash::Hash + Eq>(memory: &'static Cache<K>, key: K, make: impl FnOnce() -> Vec<Node>) -> Arc<Vec<Node>> {
    let memory = memory.get_or_init(Default::default);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let made = Arc::new(make());
    if let Ok(mut m) = memory.lock() {
        if m.len() > 4096 {
            m.clear();
        }
        m.insert(key, made.clone());
    }
    made
}

/// A building's baked pieces, in its own frame (its footprint's corner the origin), and the lamps it
/// lights there.
struct Built {
    baked: Vec<Arc<Baked>>,
    lamps: Vec<SetLamp>,
}

/// The kit's piece for `art` in `state` at `step`, stood on a footprint of `tiles` (across, deep) facing
/// south, baked once and shared by every building of its plan and size.
fn building(art: &str, state: State, step: u32, tiles: (i16, i16), face: Face) -> Arc<Built> {
    static MADE: OnceLock<Mutex<HashMap<(String, State, u32, (i16, i16), u8), Arc<Built>>>> = OnceLock::new();
    let step = if state.frames() > 1 { step % state.frames() } else { 0 };
    // Only a swape has a front to turn: every other building is made facing south and shared as that.
    let face = if art == "swape" { face } else { Face::South };
    let key = (art.to_string(), state, step, tiles, face as u8);
    // Made with the cache held, so two seats asking at once make one mesh and not two.
    let mut memory = MADE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = memory.get(&key) {
        return hit.clone();
    }
    let foot = [0.0, 0.0, f32::from(tiles.0) * 16.0, f32::from(tiles.1) * 16.0];
    let made = Arc::new(match works_kit::solids(art, state, step) {
        Some(piece) => {
            let placed = works_kit::stand(&piece, foot, face);
            Built { baked: hill_mesh::bake(&placed.solids, [0.0; 3], |t| t), lamps: placed.lamps }
        }
        None => Built { baked: Vec::new(), lamps: Vec::new() },
    });
    if memory.len() > 2048 {
        memory.clear();
    }
    memory.insert(key, made.clone());
    made
}

/// A works on fire over a footprint of `tiles`, baked once and shared.
fn blazing(tiles: (i16, i16), step: u32) -> Arc<Built> {
    static MADE: OnceLock<Mutex<HashMap<((i16, i16), u32), Arc<Built>>>> = OnceLock::new();
    let step = step % flat::FRAMES;
    let mut memory = MADE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = memory.get(&(tiles, step)) {
        return hit.clone();
    }
    let (w, d) = (f32::from(tiles.0) * 16.0, f32::from(tiles.1) * 16.0);
    let placed = works_kit::stand(&works_kit::blaze(w, d, step), [0.0, 0.0, w, d], Face::South);
    let made = Arc::new(Built { baked: hill_mesh::bake(&placed.solids, [0.0; 3], |t| t), lamps: placed.lamps });
    memory.insert((tiles, step), made.clone());
    made
}

/// How far below the walked ground the water stands at a point, for what floats: the river's
/// surface is lowered into its banks (`realm_ground::WATER_Y`), so a barge's lane and a barge
/// are set down to it and not left standing over the river on the bank's level.
fn water_drop(x: f32, z: f32) -> f32 {
    use super::realm_ground::{WATER_Y, terrain};
    if terrain().height_at(x, z) < -1.0 { WATER_Y + 0.2 } else { 0.0 }
}

/// One tile of road or track as the lines lay it: what it is, which sides it joins, a tower, and how
/// far along a ropeway's span it is (the cable sags between towers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Laid {
    kind: LineKind,
    links: u8,
    post: bool,
    span: u32,
    tile: (i16, i16),
    /// What the rails add to a tile of track (`rail_extra`): a switch, its throw, a bay; none for the rest.
    extra: u32,
}

/// A rail tile's switch, the way it is thrown, and its bay (side, held, the board's mark), as one number
/// for the chunk's hash: bit 0 a switch, bits 1 to 3 the throw (none, or the way plus one), bit 4 a bay,
/// bits 5 and 6 its side, bit 7 whether a wagon is working it, bits 8 to 10 its mark.
fn rail_extra(r: &hill::RailTile) -> u32 {
    let mut e = 0u32;
    if r.switch {
        e |= 1 | (r.throw.map_or(0, |d| u32::from(d) + 1) << 1);
    }
    if let Some(b) = &r.bay {
        // The mark on the board is the name's: a few bits of it, so two bays on a line read apart.
        let tag = b.name.bytes().fold(5u32, |h, c| h.wrapping_mul(31).wrapping_add(u32::from(c))) & 7;
        e |= (1 << 4) | (u32::from(b.side) << 5) | (u32::from(b.held) << 7) | (tag << 8);
    }
    e
}

fn rail_solids(extra: u32) -> Vec<Solid> {
    let throw = ((extra >> 1) & 7).checked_sub(1).map(|d| d as u8);
    let bay = (extra >> 4 & 1 == 1).then(|| (((extra >> 5) & 3) as u8, extra >> 7 & 1 == 1, ((extra >> 8) & 7) as u8));
    belts::rail_extras(extra & 1 == 1, throw, bay)
}

/// The tiles of the roads and lines in one chunk of the kingdom, baked into meshes in the world,
/// kept for as long as the tiles are what they were: a barge's lane has its ripples, one step in
/// eight; nothing else moves.
fn chunk_of_tiles(chunk: (i32, i32), tiles: &[Laid], step: u32) -> Arc<Vec<Arc<Baked>>> {
    use std::hash::{Hash, Hasher};
    static MADE: OnceLock<Mutex<HashMap<((i32, i32), u64), Arc<Vec<Arc<Baked>>>>>> = OnceLock::new();
    let rippling = tiles.iter().any(|t| t.kind == LineKind::Barge);
    let step = if rippling { step % flat::FRAMES } else { 0 };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    step.hash(&mut h);
    tiles.hash(&mut h);
    let key = (chunk, h.finish());
    let mut memory = MADE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = memory.get(&key) {
        return hit.clone();
    }
    let mut solids: Vec<Solid> = Vec::new();
    for t in tiles {
        let (ox, oz) = (f32::from(t.tile.0) * 16.0, f32::from(t.tile.1) * 16.0);
        let drop = if t.kind == LineKind::Barge { water_drop(ox + 8.0, oz + 8.0) } else { 0.0 };
        let span = if t.kind == LineKind::Rope { t.span % works_kit::rails::SPAN } else { 0 };
        for s in works_kit::line_solids_at(t.kind, t.links, t.post, span, if t.kind == LineKind::Barge { step } else { 0 }).iter() {
            let mut s = s.clone();
            s.at = [s.at[0] + ox, s.at[1] + drop, s.at[2] + oz];
            solids.push(s);
        }
        if t.extra != 0 {
            for mut s in rail_solids(t.extra) {
                s.at = [s.at[0] + ox, s.at[1], s.at[2] + oz];
                solids.push(s);
            }
        }
    }
    let made = Arc::new(hill_mesh::bake(&solids, [0.0; 3], |t| t));
    if memory.len() > 512 {
        memory.clear();
    }
    memory.insert(key, made.clone());
    made
}

/// One tile of rollerway as the chunk bakes it: what it is, the way it faces, whether its rollers are
/// turning (and at which of their four steps) and whether its hand-chain has the labourer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BeltTile {
    tile: (i16, i16),
    piece: Piece,
    dir: u8,
    phase: Option<u8>,
    crewed: bool,
}

/// The rollerways' tiles in one chunk of the kingdom, baked, for as long as they are what they were: a
/// line that is streaming has four of these a chunk (the rollers' steps), a stood one has one.
fn chunk_of_belts(chunk: (i32, i32), tiles: &[BeltTile]) -> Arc<Vec<Arc<Baked>>> {
    use std::hash::{Hash, Hasher};
    static MADE: OnceLock<Mutex<HashMap<((i32, i32), u64), Arc<Vec<Arc<Baked>>>>>> = OnceLock::new();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    tiles.hash(&mut h);
    let key = (chunk, h.finish());
    let mut memory = MADE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = memory.get(&key) {
        return hit.clone();
    }
    let mut solids: Vec<Solid> = Vec::new();
    for t in tiles {
        let (ox, oz) = (f32::from(t.tile.0) * 16.0, f32::from(t.tile.1) * 16.0);
        for s in belts::tile(t.piece, t.dir, t.phase, t.crewed).iter() {
            let mut s = s.clone();
            s.at = [s.at[0] + ox, s.at[1], s.at[2] + oz];
            solids.push(s);
        }
    }
    let made = Arc::new(hill_mesh::bake(&solids, [0.0; 3], |t| t));
    if memory.len() > 512 {
        memory.clear();
    }
    memory.insert(key, made.clone());
    made
}

/// A good on the line, baked once per `(good, which way it lies)` and lifted as the carriers are, so it
/// stands off the rollers it rides.
fn good_baked(load: flat::Load, along_x: bool) -> Arc<Vec<Arc<Baked>>> {
    static MADE: OnceLock<Mutex<HashMap<(flat::Load, bool), Arc<Vec<Arc<Baked>>>>>> = OnceLock::new();
    let mut memory = MADE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    memory.entry((load, along_x)).or_insert_with(|| Arc::new(hill_mesh::bake(&belts::good(load, along_x), [0.0; 3], lifted))).clone()
}

/// A beacon (a jam's red one, a held wagon's amber), lit or dark, baked once.
fn beacon_baked(amber: bool, on: bool) -> Arc<Vec<Arc<Baked>>> {
    static MADE: OnceLock<Mutex<HashMap<(bool, bool), Arc<Vec<Arc<Baked>>>>>> = OnceLock::new();
    let mut memory = MADE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    memory.entry((amber, on)).or_insert_with(|| Arc::new(hill_mesh::bake(&belts::beacon(amber, on), [0.0; 3], |t| t))).clone()
}

/// How many goods a view of the works draws one by one; a longer stream is drawn by its lead good on each
/// tile (a line of five hundred tiles full is a thousand and a half draws otherwise).
pub const GOODS_SOLO: usize = 240;
/// The most goods a view draws at all.
pub const GOODS_MOST: usize = 600;

/// Where a good in slot `slot` of a lane running `dir` stands on its tile, the tile's own frame, and how
/// high its bed carries it.
fn good_place(b: &hill::Belt, lane: u8, slot: usize, back: f32, raised: bool) -> Option<(f32, f32, f32)> {
    let (px, pz) = kroll::slot_at(lane, slot, back);
    // The first slot's good is still coming in from the tile behind: it is not drawn before this tile's edge.
    let along = if matches!(lane % 4, 1 | 3) { if lane % 4 == 1 { px } else { 16.0 - px } } else if lane % 4 == 2 { pz } else { 16.0 - pz };
    if along < 1.0 {
        return None;
    }
    let y = match b.piece {
        Piece::Chain => belts::CHAIN_TOP,
        Piece::Trestle if raised => belts::TRESTLE_RISE + belts::ROLLER_TOP,
        _ => belts::ROLLER_TOP,
    };
    // A house hides its middle slot.
    if matches!(b.piece, Piece::Splitter | Piece::Merge | Piece::Gate) && slot == 1 {
        return None;
    }
    Some((px, y, pz))
}

/// The tiles `[x0, z0, x1, z1]` of a set, as the straight runs of them (a maximal stretch along a row and
/// along a column), the longest first and at most `most`: where the look dyes its ramp.
fn tile_runs(set: &std::collections::BTreeSet<(i16, i16)>, most: usize) -> Vec<[i16; 4]> {
    let mut runs: Vec<[i16; 4]> = Vec::new();
    let mut lone: std::collections::BTreeSet<(i16, i16)> = set.clone();
    for horizontal in [true, false] {
        for &t in set {
            let back = if horizontal { (t.0 - 1, t.1) } else { (t.0, t.1 - 1) };
            if set.contains(&back) {
                continue;
            }
            let mut end = t;
            loop {
                let next = if horizontal { (end.0 + 1, end.1) } else { (end.0, end.1 + 1) };
                if !set.contains(&next) {
                    break;
                }
                end = next;
            }
            if end != t {
                runs.push([t.0, t.1, end.0, end.1]);
                let (mut c, step) = (t, if horizontal { (1, 0) } else { (0, 1) });
                loop {
                    lone.remove(&c);
                    if c == end {
                        break;
                    }
                    c = (c.0 + step.0, c.1 + step.1);
                }
            }
        }
    }
    runs.extend(lone.into_iter().map(|t| [t.0, t.1, t.0, t.1]));
    runs.sort_by_key(|r| std::cmp::Reverse((r[2] - r[0]).max(r[3] - r[1])));
    runs.truncate(most);
    runs
}

/// A carrier's solids, centred on the ground point `(0, 0)` heading `dir`: the kit's (built
/// heading east from the rear), centred on what they cover so a rider's place is where the carrier
/// is, not where its rear is, and a cart, wagon or sled a quarter bigger than its tile's measure so it
/// reads from the stand-back camera (the cable and the water fix the others' size).
fn carrier_solids(kind: LineKind, dir: flat::lines::Dir, load: Option<flat::Load>, step: u32, near: bool) -> Vec<Solid> {
    let solids = works_kit::vehicle_solids_lod(kind, dir, load, step, near);
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for s in solids.iter() {
        // Half of a solid's reach in each way, before any turn: enough to find the middle.
        let reach = s.size[0].max(s.size[2]) / 2.0;
        for (axis, at) in [(0, s.at[0]), (1, s.at[2])] {
            lo[axis] = lo[axis].min(at - reach);
            hi[axis] = hi[axis].max(at + reach);
        }
    }
    let (mx, mz) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
    let k = if matches!(kind, LineKind::Cart | LineKind::Tram | LineKind::Chute | LineKind::Mule) { CARRIER_SCALE } else { 1.0 };
    solids
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.at = [(s.at[0] - mx) * k, s.at[1] * k, (s.at[2] - mz) * k];
            s.size = s.size.map(|v| v * k);
            s
        })
        .collect()
}

/// A carrier's nodes, as [`carrier_solids`] has it: made once per `(kind, dir, load, step, near)` and
/// shared. (The Keep's yard has its carts as nodes of its own: `works_yard`.)
pub(super) fn vehicle_nodes(kind: LineKind, dir: flat::lines::Dir, load: Option<flat::Load>, step: u32, near: bool) -> Arc<Vec<Node>> {
    static MADE: Cache<(LineKind, flat::lines::Dir, Option<flat::Load>, u32, bool)> = OnceLock::new();
    let step = step % flat::FRAMES;
    kept(&MADE, (kind, dir, load, step, near), || carrier_solids(kind, dir, load, step, near).iter().filter_map(|s| node_with(s, None, lifted)).collect())
}

/// A carrier baked: one mesh (and a second for what shines), made once per `(kind, dir, load, step,
/// near)` and shared by every carrier that looks so.
fn vehicle(kind: LineKind, dir: flat::lines::Dir, load: Option<flat::Load>, step: u32, near: bool) -> Arc<Vec<Arc<Baked>>> {
    type Key = (LineKind, flat::lines::Dir, Option<flat::Load>, u32, bool);
    static MADE: OnceLock<Mutex<HashMap<Key, Arc<Vec<Arc<Baked>>>>>> = OnceLock::new();
    let step = step % flat::FRAMES;
    let mut memory = MADE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = memory.get(&(kind, dir, load, step, near)) {
        return hit.clone();
    }
    let made = Arc::new(hill_mesh::bake(&carrier_solids(kind, dir, load, step, near), [0.0; 3], lifted));
    if memory.len() > 1024 {
        memory.clear();
    }
    memory.insert((kind, dir, load, step, near), made.clone());
    made
}

/// A rectangle of tiles' ground in kingdom pixels, as a flat plate of an accent
/// and four corner posts.
fn plate(rect: (i16, i16, i16, i16), colour: [f32; 3]) -> Vec<Node> {
    let (x0, z0) = (f32::from(rect.0) * 16.0, f32::from(rect.1) * 16.0);
    let (x1, z1) = ((f32::from(rect.2) + 1.0) * 16.0, (f32::from(rect.3) + 1.0) * 16.0);
    let mut out = Vec::new();
    let mut cube = |c: [f32; 3], size: [f32; 3]| {
        let mut n = Node::new(0, angel_stage::scene::MeshRef::Cube, Material { accent: true, ..Material::color([colour[0], colour[1], colour[2], 1.0]) });
        n.transform = Transform { translation: c, scale: size, ..Transform::IDENTITY };
        out.push(n);
    };
    // The frame of the footprint, flat on the ground, and a post at each corner.
    cube([(x0 + x1) / 2.0, 0.5, z0 + 0.6], [x1 - x0, 1.0, 1.2]);
    cube([(x0 + x1) / 2.0, 0.5, z1 - 0.6], [x1 - x0, 1.0, 1.2]);
    cube([x0 + 0.6, 0.5, (z0 + z1) / 2.0], [1.2, 1.0, z1 - z0]);
    cube([x1 - 0.6, 0.5, (z0 + z1) / 2.0], [1.2, 1.0, z1 - z0]);
    for (cx, cz) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        cube([cx, 5.0, cz], [1.6, 10.0, 1.6]);
    }
    out
}

/// What a verdict's accent is: a pale teal where it fits, amber where the camp is short, ember where the land refuses.
fn verdict_colour(v: Verdict) -> [f32; 3] {
    match v {
        Verdict::Fits => [0.85, 0.84, 0.72],
        Verdict::Short => [0.93, 0.68, 0.33],
        Verdict::Refused => [0.89, 0.48, 0.20],
    }
}

/// The works as meshes for a view `[x0, z0, x1, z1]` of the kingdom's pixels (a margin of a few tiles
/// is kept round it), and the ghost of `focus`'s builder; `None` off the hillside.
pub fn of(run: &Run, view: [f32; 4], focus: Option<u32>) -> Option<HillNodes> {
    let hv: Arc<HillView> = hill::of(run)?;
    let step = hill::frame(run);
    let margin = 64.0;
    let seen = |lo: [f32; 2], hi: [f32; 2]| hi[0] > view[0] - margin && lo[0] < view[2] + margin && hi[1] > view[1] - margin && lo[1] < view[3] + margin;
    let near = |rect: [i16; 4]| seen([f32::from(rect[0]) * 16.0, f32::from(rect[1]) * 16.0], [(f32::from(rect[2]) + 1.0) * 16.0, (f32::from(rect[3]) + 1.0) * 16.0]);
    let mut out = HillNodes { bounds: ([f32::MAX; 3], [f32::MIN; 3]), ..HillNodes::default() };
    let mut next = 0u64;
    let mut grow = |out: &mut HillNodes, rect: [i16; 4], height: f32| {
        let (lo, hi) = (&mut out.bounds.0, &mut out.bounds.1);
        lo[0] = lo[0].min(f32::from(rect[0]) * 16.0);
        lo[2] = lo[2].min(f32::from(rect[1]) * 16.0);
        hi[0] = hi[0].max((f32::from(rect[2]) + 1.0) * 16.0);
        hi[2] = hi[2].max((f32::from(rect[3]) + 1.0) * 16.0);
        lo[1] = 0.0;
        hi[1] = hi[1].max(height);
    };
    let foot = |rect: [i16; 4]| [f32::from(rect[0]) * 16.0, f32::from(rect[1]) * 16.0, (f32::from(rect[2]) + 1.0) * 16.0, (f32::from(rect[3]) + 1.0) * 16.0];
    for b in &hv.buildings {
        if b.art.is_empty() || !near(b.rect) {
            continue;
        }
        let tiles = (b.rect[2] - b.rect[0] + 1, b.rect[3] - b.rect[1] + 1);
        let face = [Face::North, Face::East, Face::South, Face::West][usize::from(b.facing % 4)];
        let piece = building(b.art, b.state, step, tiles, face);
        let at = [f32::from(b.rect[0]) * 16.0, 0.0, f32::from(b.rect[1]) * 16.0];
        out.parts.extend(piece.baked.iter().map(|baked| HillPart { baked: baked.clone(), at }));
        out.lamps.extend(piece.lamps.iter().map(|l| SetLamp { at: [l.at[0] + at[0], l.at[1], l.at[2] + at[2]], ..*l }));
        out.footprints.push(Footprint { rect: foot(b.rect), plan: b.plan.clone() });
        grow(&mut out, b.rect, 48.0);
        // A works the Deep has set alight burns over its roof.
        if b.status == crate::kingdom::Status::Fire {
            let fire = blazing(tiles, step);
            out.parts.extend(fire.baked.iter().map(|baked| HillPart { baked: baked.clone(), at }));
            out.lamps.extend(fire.lamps.iter().map(|l| SetLamp { at: [l.at[0] + at[0], l.at[1], l.at[2] + at[2]], ..*l }));
        }
    }
    let keep = {
        let (x0, y0, x1, y1) = crate::kingdom::keep_rect();
        [x0, y0, x1, y1]
    };
    for [x0, z0, x1, z1] in road_runs(&hv.roads, keep) {
        if near([x0, z0, x1, z1]) {
            out.footprints.push(Footprint { rect: foot([x0, z0, x1, z1]), plan: "road-run".to_string() });
        }
    }
    // The roads and the lines' tiles by the chunk of the kingdom they lie in, whole (not only what
    // this view takes in, so a chunk's mesh is the same one as the view moves).
    let mut laid: std::collections::BTreeMap<(i32, i32), Vec<Laid>> = std::collections::BTreeMap::new();
    let chunk_of = |t: (i16, i16)| (i32::from(t.0).div_euclid(CHUNK_TILES.0), i32::from(t.1).div_euclid(CHUNK_TILES.1));
    for w in &hv.roads {
        laid.entry(chunk_of(w.tile)).or_default().push(Laid { kind: LineKind::Cart, links: w.links, post: false, span: 0, tile: w.tile, extra: 0 });
    }
    // The rail network's tiles lie as the tramway's did, sleepers on ballast, and its wagons run as the
    // tramway's did (the sim's tramways are rail now).
    for r in &hv.rails {
        laid.entry(chunk_of(r.tile)).or_default().push(Laid { kind: LineKind::Tram, links: r.links, post: false, span: 0, tile: r.tile, extra: rail_extra(r) });
    }
    for line in &hv.lines {
        // A laid tramway, chute, ropeway or cart line lies in the ramp of what it is made of, run by run, so it
        // stands off the grass and what runs on it reads on it (`works_look`).
        let run_plan = match line.kind {
            flat::lines::Kind::Tram => Some("tram-run"),
            flat::lines::Kind::Chute => Some("chute-run"),
            flat::lines::Kind::Cart => Some("road-run"),
            flat::lines::Kind::Rope => Some("rope-run"),
            flat::lines::Kind::Mule => Some("mule-run"),
            flat::lines::Kind::Barge => None,
        };
        if let Some(plan) = run_plan {
            for run in runs(&line.path) {
                if near(run) {
                    out.footprints.push(Footprint { rect: foot(run), plan: plan.to_string() });
                }
            }
        }
        let kind = line_kind(line.kind);
        let n = line.path.len();
        for (i, t) in line.path.iter().enumerate() {
            if i == 0 || i + 1 == n {
                continue;
            }
            let mut links = 0;
            for j in [i - 1, i + 1] {
                let o = line.path[j];
                links |= match (o.0 - t.0, o.1 - t.1) {
                    (0, -1) => 1,
                    (1, 0) => 2,
                    (0, 1) => 4,
                    (-1, 0) => 8,
                    _ => 0,
                };
            }
            // A ropeway has a tower every fourth tile; the cable sags between them.
            let (post, span) = (kind == LineKind::Rope && i % 4 == 1, ((i + 3) % 4) as u32);
            laid.entry(chunk_of(*t)).or_default().push(Laid { kind, links, post, span, tile: *t, extra: 0 });
        }
    }
    for (chunk, mut tiles) in laid {
        let (x0, z0) = (chunk.0 * CHUNK_TILES.0 * 16, chunk.1 * CHUNK_TILES.1 * 16);
        if !seen([x0 as f32, z0 as f32], [(x0 + CHUNK_TILES.0 * 16) as f32, (z0 + CHUNK_TILES.1 * 16) as f32]) {
            continue;
        }
        tiles.sort_by_key(|t| (t.tile, t.kind as u8));
        out.parts.extend(chunk_of_tiles(chunk, &tiles, step).iter().map(|baked| HillPart { baked: baked.clone(), at: [0.0; 3] }));
    }
    // The rail network's runs, for the look's timber ramp (its bays lie in them), and a bay's lantern lit
    // while a wagon works it.
    if !hv.rails.is_empty() {
        let set: std::collections::BTreeSet<(i16, i16)> = hv.rails.iter().map(|r| r.tile).filter(|t| near([t.0, t.1, t.0, t.1])).collect();
        for run in tile_runs(&set, 24) {
            out.footprints.push(Footprint { rect: foot(run), plan: "tram-run".to_string() });
        }
        for r in hv.rails.iter().filter(|r| r.bay.as_ref().is_some_and(|b| b.held) && near([r.tile.0, r.tile.1, r.tile.0, r.tile.1])) {
            out.lamps.push(SetLamp { at: [f32::from(r.tile.0) * 16.0 + 8.0, 14.0, f32::from(r.tile.1) * 16.0 + 8.0], reach: 40.0, strength: 0.6 });
        }
    }
    // The rollerways: their tiles baked a chunk at a time, the goods riding them, and a beacon over the head
    // of a jam.
    if !hv.belts.is_empty() {
        belt_parts(&mut out, &hv, run, view, margin, &near, &foot);
    }
    // What runs on them: carts on the roads, a wagon, bucket, sled or barge on each line. Past a
    // dozen of them in a works the carriers are drawn in a few blocks each (`works_kit::vehicle_solids_lod`).
    let few = hv.carts.len() + hv.wagons.len() + hv.lines.iter().map(|l| l.riders.len()).sum::<usize>() <= MANY_CARRIERS;
    let frac = hill::frac(run);
    let mover = |out: &mut HillNodes, kind: LineKind, r: &hill::Rider| {
        let (px, pz) = r.place(frac);
        let (px, pz) = (px * 16.0, pz * 16.0);
        if px < view[0] - margin || px > view[2] + margin || pz < view[1] - margin || pz > view[3] + margin {
            return;
        }
        let baked = vehicle(kind, r.dir, r.load, if r.moving { step } else { 0 }, few);
        let drop = if kind == LineKind::Barge { water_drop(px, pz) } else { 0.0 };
        out.parts.extend(baked.iter().map(|b| HillPart { baked: b.clone(), at: [px, drop, pz] }));
        if kind == LineKind::Barge {
            // The barge lies in the timber ramp: a box about its hull, a short way past it.
            let (half_long, half_wide) = (BARGE_LENGTH / 2.0, BARGE_BEAM / 2.0);
            let (hx, hz) = if matches!(r.dir, flat::lines::Dir::East | flat::lines::Dir::West) { (half_long, half_wide) } else { (half_wide, half_long) };
            out.footprints.push(Footprint { rect: [px - hx, pz - hz, px + hx, pz + hz], plan: "barge-hull".to_string() });
        }
    };
    // The carts bound for the Keep's yard that are near it are drawn in the bailey, on the lane to the
    // crane's foot (`works_yard`), not here on the road.
    let relayed: std::collections::BTreeSet<usize> = if super::works_yard::shows_real_carts(run) { hv.relayed().collect() } else { Default::default() };
    for (i, r) in hv.carts.iter().enumerate() {
        if !relayed.contains(&i) {
            mover(&mut out, LineKind::Cart, r);
        }
    }
    for w in &hv.wagons {
        mover(&mut out, LineKind::Tram, &w.rider);
        // A wagon held up for the track carries an amber tag, a jammed one the blinking red.
        let jammed = w.state == crate::kingdom::WagonState::Jammed;
        if jammed || w.state == crate::kingdom::WagonState::Queued {
            let (px, pz) = w.rider.place(frac);
            let (px, pz) = (px * 16.0, pz * 16.0);
            if px > view[0] - margin && px < view[2] + margin && pz > view[1] - margin && pz < view[3] + margin {
                let on = (step / 4) % 2 == 0;
                out.parts.extend(beacon_baked(!jammed, on).iter().map(|b| HillPart { baked: b.clone(), at: [px, 9.0, pz] }));
            }
        }
    }
    for line in &hv.lines {
        let kind = line_kind(line.kind);
        for r in &line.riders {
            mover(&mut out, kind, r);
        }
    }
    // The ghost: a plate of its verdict's accent on the ground, and, for a building, the piece itself half there.
    if let Some(g) = focus.and_then(|id| run.hill_ghost(id)) {
        ghost_nodes(&g, step, &mut out.ghost, &mut next);
    }
    if out.bounds.0[0] == f32::MAX {
        out.bounds = ([0.0; 3], [0.0; 3]);
    }
    Some(out)
}

/// The rollerways as parts: their tiles baked a chunk at a time (four steps of the rollers' turn each),
/// footprints for the look's ramp, the goods on the beds each its small shared mesh moved to its slot,
/// a beacon at the head of each jam.
fn belt_parts(out: &mut HillNodes, hv: &HillView, run: &Run, view: [f32; 4], margin: f32, near: &impl Fn([i16; 4]) -> bool, foot: &impl Fn([i16; 4]) -> [f32; 4]) {
    use std::collections::BTreeMap;
    let (step, frac) = (hill::frame(run), hill::frac(run));
    let step4 = (step % 4) as u8;
    let in_sight = |x: f32, z: f32| x > view[0] - margin && x < view[2] + margin && z > view[1] - margin && z < view[3] + margin;
    let chunk_of = |t: (i16, i16)| (i32::from(t.0).div_euclid(CHUNK_TILES.0), i32::from(t.1).div_euclid(CHUNK_TILES.1));
    let mut by_chunk: BTreeMap<(i32, i32), Vec<BeltTile>> = BTreeMap::new();
    for b in &hv.belts {
        let phase = (b.moved > 0).then_some(step4);
        by_chunk.entry(chunk_of(b.tile)).or_default().push(BeltTile { tile: b.tile, piece: b.piece, dir: b.dir, phase, crewed: b.crewed });
    }
    for (chunk, mut tiles) in by_chunk {
        let (x0, z0) = (chunk.0 * CHUNK_TILES.0 * 16, chunk.1 * CHUNK_TILES.1 * 16);
        let (x1, z1) = ((x0 + CHUNK_TILES.0 * 16) as f32, (z0 + CHUNK_TILES.1 * 16) as f32);
        if x1 <= view[0] - margin || (x0 as f32) >= view[2] + margin || z1 <= view[1] - margin || (z0 as f32) >= view[3] + margin {
            continue;
        }
        tiles.sort_by_key(|t| t.tile);
        out.parts.extend(chunk_of_belts(chunk, &tiles).iter().map(|baked| HillPart { baked: baked.clone(), at: [0.0; 3] }));
    }
    let set: std::collections::BTreeSet<(i16, i16)> = hv.belts.iter().map(|b| b.tile).filter(|t| near([t.0, t.1, t.0, t.1])).collect();
    for run in tile_runs(&set, 24) {
        out.footprints.push(Footprint { rect: foot(run), plan: "roller-run".to_string() });
    }
    // The goods: every slot of every lane in sight; a stream too long to draw whole is shown by the lead good
    // on each tile, and past that the nearest to the middle of the view.
    struct Good {
        at: [f32; 3],
        load: flat::Load,
        along_x: bool,
        lead: bool,
    }
    let mut goods: Vec<Good> = Vec::new();
    for b in hv.belts.iter().filter(|b| near([b.tile.0, b.tile.1, b.tile.0, b.tile.1])) {
        let (ox, oz) = (f32::from(b.tile.0) * 16.0, f32::from(b.tile.1) * 16.0);
        let back = (f32::from(b.moved) * (1.0 - frac)).min(3.0);
        let lanes: [(&[Option<flat::Load>; 3], u8, bool); 2] = [(&b.slots, b.dir, true), (&b.across, (b.dir + 1) % 4, false)];
        for (lane, (slots, dir, raised)) in lanes.into_iter().enumerate() {
            if lane == 1 && b.piece != Piece::Trestle {
                continue;
            }
            let lead = slots.iter().rposition(Option::is_some);
            for (i, g) in slots.iter().enumerate() {
                let Some(load) = *g else { continue };
                // The raised lane of a trestle is the sim's first (`slots`); the one under it, `across`.
                if let Some((px, y, pz)) = good_place(b, dir, i, back, raised) {
                    goods.push(Good { at: [ox + px, y, oz + pz], load, along_x: matches!(dir % 4, 1 | 3), lead: lead == Some(i) });
                }
            }
        }
    }
    if goods.len() > GOODS_SOLO {
        goods.retain(|g| g.lead);
    }
    if goods.len() > GOODS_MOST {
        let mid = [(view[0] + view[2]) / 2.0, (view[1] + view[3]) / 2.0];
        goods.sort_by(|a, b| ((a.at[0] - mid[0]).hypot(a.at[2] - mid[1])).total_cmp(&(b.at[0] - mid[0]).hypot(b.at[2] - mid[1])));
        goods.truncate(GOODS_MOST);
    }
    for g in goods {
        out.parts.extend(good_baked(g.load, g.along_x).iter().map(|baked| HillPart { baked: baked.clone(), at: g.at }));
    }
    // The beacons over the heads of jams.
    let on = (step / 4) % 2 == 0;
    for b in hv.belts.iter().filter(|b| b.stuck >= 8 && hv.jams.iter().any(|(t, _)| *t == b.tile)) {
        let (x, z) = (f32::from(b.tile.0) * 16.0 + 8.0, f32::from(b.tile.1) * 16.0 + 8.0);
        if in_sight(x, z) {
            out.parts.extend(beacon_baked(false, on).iter().map(|baked| HillPart { baked: baked.clone(), at: [x, 8.0, z] }));
        }
    }
}

/// The works as one node a block, the way they were drawn before they were baked into meshes
/// (`hill_mesh`): what the proof's before-and-after is measured against (`glass/examples/rails3d_proof`),
/// and nothing else uses.
#[doc(hidden)]
pub fn legacy_nodes(run: &Run, view: [f32; 4]) -> Vec<Node> {
    let Some(hv) = hill::of(run) else { return Vec::new() };
    let (step, margin, frac) = (hill::frame(run), 64.0, hill::frac(run));
    let seen = |lo: [f32; 2], hi: [f32; 2]| hi[0] > view[0] - margin && lo[0] < view[2] + margin && hi[1] > view[1] - margin && lo[1] < view[3] + margin;
    let tile_seen = |t: (i16, i16)| seen([f32::from(t.0) * 16.0, f32::from(t.1) * 16.0], [(f32::from(t.0) + 1.0) * 16.0, (f32::from(t.1) + 1.0) * 16.0]);
    let mut out: Vec<Node> = Vec::new();
    let mut tile_nodes = |out: &mut Vec<Node>, kind: LineKind, links: u8, post: bool, span: u32, t: (i16, i16)| {
        let (ox, oz) = (f32::from(t.0) * 16.0, f32::from(t.1) * 16.0);
        let drop = if kind == LineKind::Barge { water_drop(ox + 8.0, oz + 8.0) } else { 0.0 };
        for s in works_kit::line_solids_at(kind, links, post, span, if kind == LineKind::Barge { step } else { 0 }).iter() {
            let mut s = s.clone();
            s.at = [s.at[0] + ox, s.at[1] + drop, s.at[2] + oz];
            out.extend(node_of(&s, None));
        }
    };
    for b in &hv.buildings {
        let foot = [f32::from(b.rect[0]) * 16.0, f32::from(b.rect[1]) * 16.0, (f32::from(b.rect[2]) + 1.0) * 16.0, (f32::from(b.rect[3]) + 1.0) * 16.0];
        if b.art.is_empty() || !seen([foot[0], foot[1]], [foot[2], foot[3]]) {
            continue;
        }
        if let Some(piece) = works_kit::solids(b.art, b.state, if b.state.frames() > 1 { step % b.state.frames() } else { 0 }) {
            out.extend(works_kit::stand(&piece, foot, Face::South).solids.iter().filter_map(|s| node_of(s, None)));
        }
    }
    for w in &hv.roads {
        if tile_seen(w.tile) {
            tile_nodes(&mut out, LineKind::Cart, w.links, false, 0, w.tile);
        }
    }
    for line in &hv.lines {
        let kind = line_kind(line.kind);
        let n = line.path.len();
        for (i, t) in line.path.iter().enumerate() {
            if i == 0 || i + 1 == n || !tile_seen(*t) {
                continue;
            }
            let mut links = 0;
            for j in [i - 1, i + 1] {
                let o = line.path[j];
                links |= match (o.0 - t.0, o.1 - t.1) {
                    (0, -1) => 1,
                    (1, 0) => 2,
                    (0, 1) => 4,
                    (-1, 0) => 8,
                    _ => 0,
                };
            }
            tile_nodes(&mut out, kind, links, kind == LineKind::Rope && i % 4 == 1, ((i + 3) % 4) as u32, *t);
        }
    }
    let few = hv.carts.len() + hv.lines.iter().map(|l| l.riders.len()).sum::<usize>() <= MANY_CARRIERS;
    let mut movers = |out: &mut Vec<Node>, kind: LineKind, r: &hill::Rider| {
        let (px, pz) = r.place(frac);
        let (px, pz) = (px * 16.0, pz * 16.0);
        if px < view[0] - margin || px > view[2] + margin || pz < view[1] - margin || pz > view[3] + margin {
            return;
        }
        let drop = if kind == LineKind::Barge { water_drop(px, pz) } else { 0.0 };
        for n in vehicle_nodes(kind, r.dir, r.load, if r.moving { step } else { 0 }, few).iter() {
            let mut n = n.clone();
            n.transform.translation = [n.transform.translation[0] + px, n.transform.translation[1] + drop, n.transform.translation[2] + pz];
            out.push(n);
        }
    };
    for r in &hv.carts {
        movers(&mut out, LineKind::Cart, r);
    }
    for line in &hv.lines {
        for r in &line.riders {
            movers(&mut out, line_kind(line.kind), r);
        }
    }
    for (k, n) in out.iter_mut().enumerate() {
        n.id = IDS + k as u64;
    }
    out
}

fn line_kind(kind: flat::lines::Kind) -> LineKind {
    match kind {
        flat::lines::Kind::Cart => LineKind::Cart,
        flat::lines::Kind::Tram => LineKind::Tram,
        flat::lines::Kind::Rope => LineKind::Rope,
        flat::lines::Kind::Chute => LineKind::Chute,
        flat::lines::Kind::Barge => LineKind::Barge,
        flat::lines::Kind::Mule => LineKind::Mule,
    }
}

/// The roads about the Keep as straight runs of three tiles or more, `[x0, z0, x1, z1]` in tiles
/// (inclusive), the longest first and at most [`ROAD_RUNS`]: the supply lines that reach the castle,
/// which the look dyes as worn earth so they read from the stand-back camera. Only the tiles within
/// [`KEEP_REACH`] tiles of the Keep's ground count; the rest of a road net is the plain ribbon.
fn road_runs(roads: &[hill::Way4], keep: [i16; 4]) -> Vec<[i16; 4]> {
    use std::collections::BTreeSet;
    let near = |t: (i16, i16)| t.0 >= keep[0] - KEEP_REACH && t.0 <= keep[2] + KEEP_REACH && t.1 >= keep[1] - KEEP_REACH && t.1 <= keep[3] + KEEP_REACH;
    let set: BTreeSet<(i16, i16)> = roads.iter().map(|w| w.tile).filter(|t| near(*t)).collect();
    let mut runs: Vec<[i16; 4]> = Vec::new();
    // Along rows, then along columns: a maximal stretch of the set with no gap.
    for horizontal in [true, false] {
        let mut seen: BTreeSet<(i16, i16)> = BTreeSet::new();
        for &t in &set {
            if seen.contains(&t) {
                continue;
            }
            let back = if horizontal { (t.0 - 1, t.1) } else { (t.0, t.1 - 1) };
            if set.contains(&back) {
                continue;
            }
            let mut end = t;
            seen.insert(t);
            loop {
                let next = if horizontal { (end.0 + 1, end.1) } else { (end.0, end.1 + 1) };
                if !set.contains(&next) {
                    break;
                }
                seen.insert(next);
                end = next;
            }
            if (end.0 - t.0).max(end.1 - t.1) >= 2 {
                runs.push([t.0, t.1, end.0, end.1]);
            }
        }
    }
    runs.sort_by_key(|r| std::cmp::Reverse((r[2] - r[0]).max(r[3] - r[1])));
    runs.truncate(ROAD_RUNS);
    runs
}

/// A path as its straight runs: the tiles `[x0, z0, x1, z1]` (inclusive) of each stretch that keeps
/// one heading, a corner tile in both its stretches.
fn runs(path: &[(i16, i16)]) -> Vec<[i16; 4]> {
    let mut out = Vec::new();
    if path.len() < 2 {
        return out;
    }
    let step = |a: (i16, i16), b: (i16, i16)| (b.0 - a.0, b.1 - a.1);
    let (mut from, mut heading) = (0usize, step(path[0], path[1]));
    for i in 2..=path.len() {
        let turned = i == path.len() || step(path[i - 1], path[i]) != heading;
        if turned {
            let (a, b) = (path[from], path[i - 1]);
            out.push([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]);
            if i < path.len() {
                from = i - 1;
                heading = step(path[i - 1], path[i]);
            }
        }
    }
    out
}

fn ghost_nodes(g: &Ghost, step: u32, out: &mut Vec<Node>, next: &mut u64) {
    let colour = verdict_colour(g.verdict);
    let mut plated = plate(g.rect, colour);
    for n in &mut plated {
        n.id = IDS + (1 << 20) + *next;
        *next += 1;
    }
    out.extend(plated);
    if let Tool::Plan(id) = &g.tool {
        let art = hill::art_of(id);
        let rect = [g.rect.0, g.rect.1, g.rect.2, g.rect.3];
        if !art.is_empty()
            && let Some(piece) = works_kit::solids(art, State::Idle, step)
        {
            let foot = [f32::from(rect[0]) * 16.0, f32::from(rect[1]) * 16.0, (f32::from(rect[2]) + 1.0) * 16.0, (f32::from(rect[3]) + 1.0) * 16.0];
            for s in &works_kit::stand(&piece, foot, Face::South).solids {
                if let Some(mut n) = node_of(s, Some(0.5)) {
                    n.id = IDS + (1 << 20) + *next;
                    *next += 1;
                    out.push(n);
                }
            }
        }
    }
    // Cursor post and a line's walked ground.
    for t in &g.path {
        let mut n = Node::new(0, angel_stage::scene::MeshRef::Cube, Material { accent: true, ..Material::color([0.93, 0.68, 0.33, 1.0]) });
        n.transform = Transform { translation: [(f32::from(t.0) + 0.5) * 16.0, 1.0, (f32::from(t.1) + 0.5) * 16.0], scale: [6.0, 2.0, 6.0], ..Transform::IDENTITY };
        n.id = IDS + (1 << 20) + *next;
        *next += 1;
        out.push(n);
    }
    let _ = build::tools;
    let _ = Kind::Fire;
}

#[cfg(test)]
#[path = "../../tests/world3d/hillside__tests.rs"]
mod tests;
