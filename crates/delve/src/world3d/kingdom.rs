//! The kingdom in 3D (docs/WORLD-3D.md, "The kingdom"; docs/REALM-MONOLITH.md,
//! "The realm, lit"): the one great room of the surface drawn as its chunks,
//! each its own set, as many of them as a knight's view takes in.
//!
//! - **Sets.** A chunk's set is what the realm makes of the kingdom's map
//!   (`realm::set_text` of the map with no wishes: what a run builds stands
//!   up as extra blocks of its own, `lots`), made once a chunk and kept.
//! - **Lit.** A chunk is drawn through the realm's look (`realm_look`): its
//!   ground's height field and its shapes toned by kind and lit by the sun
//!   and lamps, placed on its biome's ramp ([`Chunk::lit`]). Plates are
//!   retired for the kingdom: a chunk has none, and its set is all a page
//!   needs (the page fills it lit itself, `set::Sets::fill`).
//! - **A page across a wire** has no chunks of its own: the host puts the
//!   chunks near each seat's view in that seat's tree (`wire::seat_tree`,
//!   their set text only), and the page keeps those it was last sent
//!   ([`Chunks::receive`], [`Chunks::keep`]).
//! - The plates' machinery (resident plates, patches, laying a plate down
//!   for a terminal) stays for a chunk that has one, which none has now.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::scene::{Material, Node};
use angel_stage::set::{Kind, PLATE_MARGIN, Patches, Set};

use super::assets::{self, Plate, Swatch};
use super::chunks;
use crate::world::map::{CHUNK_H, CHUNK_W, CHUNKS_X, CHUNKS_Y};

/// The chunk grid: columns, rows.
pub const GRID: (i32, i32) = (CHUNKS_X, CHUNKS_Y);
/// How many chunks the kingdom is cut into.
pub const COUNT: usize = (CHUNKS_X * CHUNKS_Y) as usize;
/// A chunk's stage in the kingdom's pixels (`chunks::STAGE`).
pub const STAGE: (f32, f32) = ((CHUNK_W * 16) as f32, (CHUNK_H * 16) as f32);
/// The most plates kept decoded at once. A knight's view (24 x 14 tiles)
/// overlaps at most 2 x 2 chunks of 24 x 16; with the ring round it a
/// window draws and a page is sent, at most 4 x 4 = 16 chunks are near one
/// knight at any time (measured walking the kingdom: `near` never gives
/// more), so 16 keep everything one view may need and never thrash. Each is
/// 1536 x 1024 RGBA, 6 MB: 100 MB at most.
pub const RESIDENT: usize = 16;
/// How long a chunk without a usable plate waits before it looks again.
pub const RECHECK_SECS: u64 = 10;

/// Chunk `(cx, cz)`'s number: row by row.
pub fn index(cx: i32, cz: i32) -> Option<u32> {
    ((0..GRID.0).contains(&cx) && (0..GRID.1).contains(&cz)).then(|| (cz * GRID.0 + cx) as u32)
}

/// Chunk number `index`'s column and row.
pub fn cell(index: u32) -> (i32, i32) {
    (index as i32 % GRID.0, index as i32 / GRID.0)
}

/// The chunks a view of the kingdom covering `[x0, z0, x1, z1]` (its
/// pixels) is drawn with: every chunk it overlaps and one ring round them
/// (what drifts into view as the camera eases, what stands up into it from
/// below, the far floor a tilted window shows above it). At most 4 x 4 for
/// a knight's view; in order, row by row.
pub fn near(view: [f32; 4]) -> Vec<u32> {
    cover(view, 1)
}

/// The chunks a view of the kingdom covering `[x0, z0, x1, z1]` overlaps,
/// without `near`'s ring: at most 2 x 2 for a knight's view.
pub fn overlaps(view: [f32; 4]) -> Vec<u32> {
    cover(view, 0)
}

fn cover([x0, z0, x1, z1]: [f32; 4], ring: i32) -> Vec<u32> {
    let col = |x: f32| (x / STAGE.0).floor() as i32;
    let row = |z: f32| (z / STAGE.1).floor() as i32;
    // A view's edge exactly on a chunk's edge does not overlap the next one.
    let (c0, c1) = (col(x0) - ring, col(x1 - 0.01) + ring);
    let (r0, r1) = (row(z0) - ring, row(z1 - 0.01) + ring);
    (r0.max(0)..=r1.min(GRID.1 - 1)).flat_map(|r| (c0.max(0)..=c1.min(GRID.0 - 1)).filter_map(move |c| index(c, r))).collect()
}

/// The chunks near a knight's view of a native-sized screen whose camera
/// centres on `centre` (the kingdom's pixels), kept inside the kingdom as
/// the game keeps it (`arena::camera_exact`).
pub fn near_centre(centre: (f32, f32), view: (f32, f32)) -> Vec<u32> {
    let (kw, kh) = (GRID.0 as f32 * STAGE.0, GRID.1 as f32 * STAGE.1);
    let x = (centre.0 - view.0 / 2.0).clamp(0.0, (kw - view.0).max(0.0));
    let z = (centre.1 - view.1 / 2.0).clamp(0.0, (kh - view.1).max(0.0));
    near([x, z, x + view.0, z + view.1])
}

// ------------------------------------------------------------- a chunk ----

/// One chunk of the kingdom as it is drawn: its set, and its plate when it
/// has a usable one.
pub struct Chunk {
    pub index: u32,
    pub set: Set,
    /// The set file's text, as a page that makes the blocks itself is sent it.
    pub text: Arc<str>,
    /// The plate, a PNG; empty when it has no usable plate.
    pub png: Arc<[u8]>,
    /// Its kinds' patches, a PNG (`swatches.png`); empty when it has none.
    pub swatches_png: Arc<[u8]>,
    /// The same set, plate and patches, the same number (FNV-1a).
    pub key: u64,
    /// The number its plate is kept decoded and held under: its own, or
    /// the uncut chunk's for one cut for a run's lots (`Chunk::cut`), whose
    /// plate is the same.
    pub plate_key: u64,
    /// The box its blocks fill in the kingdom's pixels, low and high corners.
    pub bounds: ([f32; 3], [f32; 3]),
    swatches: OnceLock<Vec<Swatch>>,
    /// Its blocks with its plate cast on them, for the pictures they were made for.
    cast: Mutex<Option<((u32, Patches), Arc<Blocks>)>>,
    /// Its blocks without a plate, for the patches' first picture they were made for.
    plain: Mutex<Option<(u32, Arc<Blocks>)>>,
    /// Its blocks lit and toned (the realm, docs/REALM-MONOLITH.md), made once.
    lit: OnceLock<Arc<Lit>>,
}

/// A chunk's world as a look that lights it draws it
/// (docs/REALM-MONOLITH.md): what lies on the floor (its ground's height
/// field first), what stands, each its kind's tone, lit; and its ground's
/// mesh with the id it is uploaded under. Unplaced.
#[derive(Debug, Default)]
pub struct Lit {
    pub floor: Vec<Node>,
    pub standing: Vec<Node>,
    /// The meshes its nodes name: its ground's first.
    pub meshes: Vec<(u32, Arc<angel_stage::scene::MeshData>)>,
}

/// Set `set`'s world lit (`Set::lit_world_nodes`), its shapes baked: the
/// nodes of each material (its tint aside) made one mesh, each vertex
/// coloured with its node's tint, under an id of key `key`'s; a node with a
/// face of its own (a box's top toned apart) kept as it is. Ids `ids` up.
pub fn bake(set: &Set, key: u64, ids: u64) -> Lit {
    use angel_stage::scene::{MeshData, MeshRef, quat_rotate};
    let ground = angel_stage::set::ground_mesh_id(key);
    let (floor, standing, mesh) = set.lit_world_nodes(ground);
    let mut meshes: Vec<(u32, Arc<MeshData>)> = mesh.map(|m| (ground, Arc::new(m))).into_iter().collect();
    let base = 0x6000_0000 | ((key as u32 ^ (key >> 32) as u32) & 0x0FFF_FF00);
    let mut bake_list = |nodes: Vec<Node>, lies: bool, meshes: &mut Vec<(u32, Arc<MeshData>)>| -> Vec<Node> {
        let mut kept = Vec::new();
        let mut groups: Vec<(Material, MeshData)> = Vec::new();
        for node in nodes {
            let data = match node.mesh {
                Some(MeshRef::Uploaded(_)) | None => None,
                Some(m) => m.generate(),
            };
            let (Some(data), true) = (data, node.face_materials.is_empty()) else {
                kept.push(node);
                continue;
            };
            let mut material = node.material.clone();
            let tint = material.tint;
            material.tint = [1.0; 4];
            let at = match groups.iter().position(|(m, _)| *m == material) {
                Some(at) => at,
                None => {
                    groups.push((material, MeshData::default()));
                    groups.len() - 1
                }
            };
            let out = &mut groups[at].1;
            let t = node.transform;
            let first = out.positions.len() as u32;
            for (k, p) in data.positions.iter().enumerate() {
                let scaled = [p[0] * t.scale[0], p[1] * t.scale[1], p[2] * t.scale[2]];
                let r = quat_rotate(t.rotation, scaled);
                out.positions.push([r[0] + t.translation[0], r[1] + t.translation[1], r[2] + t.translation[2]]);
                let n = data.normals.get(k).copied().unwrap_or([0.0, 1.0, 0.0]);
                let n = quat_rotate(t.rotation, [n[0] / t.scale[0].max(1e-6), n[1] / t.scale[1].max(1e-6), n[2] / t.scale[2].max(1e-6)]);
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
                out.normals.push(n.map(|c| c / len));
                out.uvs.push(data.uvs.get(k).copied().unwrap_or([0.0, 0.0]));
                let c = data.colors.get(k).copied().unwrap_or([1.0; 4]);
                out.colors.push([c[0] * tint[0], c[1] * tint[1], c[2] * tint[2], c[3] * tint[3]]);
            }
            out.indices.extend(data.indices.iter().map(|i| i + first));
        }
        for (material, data) in groups {
            if data.positions.len() > angel_stage::scene::MAX_MESH_VERTICES || data.indices.len() > angel_stage::scene::MAX_MESH_INDICES {
                continue;
            }
            let id = base | (meshes.len() as u32 & 0xff) | if lies { 0x80 } else { 0 };
            meshes.push((id, Arc::new(data)));
            let mut node = Node::new(0, MeshRef::Uploaded(id), material);
            node.transform = angel_stage::scene::Transform::IDENTITY;
            kept.push(node);
        }
        kept
    };
    let (mut grounds, rest): (Vec<Node>, Vec<Node>) = floor.into_iter().partition(|n| matches!(n.mesh, Some(MeshRef::Uploaded(id)) if id == ground));
    let mut floor = bake_list(rest, true, &mut meshes);
    grounds.append(&mut floor);
    let mut floor = grounds;
    let mut standing = bake_list(standing, false, &mut meshes);
    for (k, node) in floor.iter_mut().chain(&mut standing).enumerate() {
        node.id = ids | (k as u64 + 1);
    }
    Lit { floor, standing, meshes }
}

/// A chunk's blocks as a world draws them: what lies on the floor, and what
/// stands on it, unplaced (`angel_stage::set::place` puts them on a stage).
#[derive(Debug, Default)]
pub struct Blocks {
    pub floor: Vec<Node>,
    pub standing: Vec<Node>,
}

impl Chunk {
    /// Chunk `index` of set `text`, plate `png` (empty for none) and patches
    /// `swatches` (empty for none): refused when the set is refused or is
    /// another chunk's, or a plate is not a PNG of the set's plate size.
    pub fn new(index: u32, text: Arc<str>, png: Arc<[u8]>, swatches: Arc<[u8]>) -> Result<Chunk, String> {
        let (cx, cz) = cell(index);
        let id = chunks::chunk_id(cx, cz);
        if index as usize >= COUNT {
            return Err(format!("there is no chunk {index}"));
        }
        let set = Set::parse(&text).map_err(|refusals| {
            let lines: Vec<String> = refusals.iter().map(ToString::to_string).collect();
            format!("the {id} set is refused: {}", lines.join("; "))
        })?;
        if set.id != id {
            return Err(format!("chunk {index} is {id}, not set {}", set.id));
        }
        if !png.is_empty() && assets::png_size(&png) != Some(set.plate) {
            return Err(format!("the {id} plate is not a PNG of {:?}", set.plate));
        }
        let mut key = 0xcbf2_9ce4_8422_2325u64;
        for &byte in text.as_bytes().iter().chain(png.iter()).chain(swatches.iter()) {
            key ^= u64::from(byte);
            key = key.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let bounds = bounds_of(&set);
        Ok(Chunk {
            index,
            set,
            text,
            png,
            swatches_png: swatches,
            key,
            plate_key: key,
            bounds,
            swatches: OnceLock::new(),
            cast: Mutex::new(None),
            plain: Mutex::new(None),
            lit: OnceLock::new(),
        })
    }

    /// The same chunk drawn from set `text` instead (its own set with what
    /// stands on a run's lots cut away, `lots::Share::cut`), with the same
    /// plate and patches, its plate kept under the same number.
    pub fn cut(&self, text: Arc<str>) -> Result<Chunk, String> {
        let mut chunk = Chunk::new(self.index, text, Arc::clone(&self.png), Arc::clone(&self.swatches_png))?;
        chunk.plate_key = self.plate_key;
        Ok(chunk)
    }

    /// Its nodes' ids are this or'd with their own (1 up): a scene draws
    /// one node an id, and every chunk's set numbers its own from 1.
    pub fn ids(&self) -> u64 {
        (u64::from(self.index) + 1) << 32
    }

    /// Its world lit and toned, as the realm draws it: no plate, its
    /// ground's mesh uploaded under its own id (`set::ground_mesh_id` of its
    /// key, as a page that makes it from its set names it), and its shapes
    /// baked into a few meshes of their own, one a material (their tones in
    /// their vertices' colours), so a window draws a chunk in a handful of
    /// draws, not one a spire. Made once.
    pub fn lit(&self) -> Arc<Lit> {
        Arc::clone(self.lit.get_or_init(|| Arc::new(bake(&self.set, self.key, self.ids()))))
    }

    /// Its id, `chunk-<cx>-<cz>`.
    pub fn id(&self) -> String {
        let (cx, cz) = cell(self.index);
        chunks::chunk_id(cx, cz)
    }

    /// Whether it has a usable plate.
    pub fn painted(&self) -> bool {
        !self.png.is_empty()
    }

    /// The patches of the kinds its plate paints, decoded once.
    pub fn swatches(&self) -> &[Swatch] {
        self.swatches.get_or_init(|| assets::swatches_of(&self.swatches_png))
    }

    /// The patch to draw kind `kind` with: its own, else that of the kind
    /// whose capture colour is nearest; `None` when it has none.
    pub fn swatch(&self, kind: Kind) -> Option<&Swatch> {
        let all = self.swatches();
        all.iter().find(|s| s.kind == kind).or_else(|| {
            let want = kind.capture();
            let far = |s: &&Swatch| {
                let have = s.kind.capture();
                (0..3).map(|k| (i32::from(have[k]) - i32::from(want[k])).pow(2)).sum::<i32>()
            };
            all.iter().min_by_key(far)
        })
    }

    /// The patch a block a run stands in it (`lots`) wears for kind `kind`:
    /// its own; else the nearest kind's of those that stand (a block in a
    /// ground's patch would vanish into the ground round it), while that is
    /// near enough to pass for it ([`STANDING_NEAR`]: a slate roof in rock, a
    /// plank in a trunk, but no dark doorway in rock). `None` otherwise.
    pub fn standing_swatch(&self, kind: Kind) -> Option<&Swatch> {
        let all = self.swatches();
        if let Some(own) = all.iter().find(|s| s.kind == kind) {
            return Some(own);
        }
        let want = kind.capture();
        let far = |s: &&Swatch| {
            let have = s.kind.capture();
            (0..3).map(|k| (i32::from(have[k]) - i32::from(want[k])).pow(2)).sum::<i32>()
        };
        all.iter().filter(|s| !lies(s.kind)).min_by_key(far).filter(|s| far(s) <= STANDING_NEAR)
    }

    /// Its patches as the picture `atlas` (`assets::swatch_atlas` of them):
    /// what its blocks' surfaces its plate does not see are drawn with.
    pub fn patches(&self, atlas: u32) -> Patches {
        assets::patches_in(atlas, self.swatches())
    }

    /// Its blocks with picture `plate` (its plate) cast on them through its
    /// own projector, a surface it does not see its kind's patch of
    /// `patches`, made once for each pictures they are asked for with.
    pub fn cast(&self, plate: u32, patches: Patches) -> Arc<Blocks> {
        let mut kept = self.cast.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((pictures, blocks)) = kept.as_ref()
            && *pictures == (plate, patches)
        {
            return Arc::clone(blocks);
        }
        let (mut floor, mut standing) = self.set.world_nodes(plate, patches);
        // A scene draws one node an id: each chunk's its own.
        for node in floor.iter_mut().chain(&mut standing) {
            node.id |= self.ids();
        }
        let blocks = Arc::new(Blocks { floor, standing });
        *kept = Some(((plate, patches), Arc::clone(&blocks)));
        blocks
    }

    /// Its blocks without a plate: each of its kind's patch (`swatch(k)`
    /// names the picture kind `k`'s patch is), a box's faces cut into cells
    /// of it as a room's extra blocks are (`fit::Variant::extra_nodes`);
    /// with no patches, its kind's capture colour two shades down its bank,
    /// lit.
    pub fn plain(&self, swatch: &dyn Fn(Kind) -> u32) -> Arc<Blocks> {
        let first = swatch(Kind::ALL[0]);
        let mut kept = self.plain.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((picture, blocks)) = kept.as_ref()
            && *picture == first
        {
            return Arc::clone(blocks);
        }
        let (w, h) = self.set.camera.size();
        let (pw, ph) = self.set.plate;
        // Plate pixels to a pixel of the set's stage.
        let density = (1.0 - 2.0 * PLATE_MARGIN) * (pw as f32 / w.max(1) as f32).min(ph as f32 / h.max(1) as f32);
        let mut blocks = Blocks::default();
        for shape in &self.set.shapes {
            let mut nodes = Vec::new();
            let patch = self.swatch(shape.kind).map(|s| (swatch(s.kind), s.width as f32 / density, s.height as f32 / density));
            match patch {
                Some((picture, cw, ch)) if super::fit::is_box(shape) => super::fit::box_cells(shape, picture, (cw, ch), &mut nodes),
                Some((picture, ..)) => {
                    let material = Material { lit: true, emissive: 0.7, ..Material::picture(picture) };
                    let mut node = Node::new(0, shape.mesh, material);
                    node.transform = shape.transform();
                    nodes.push(node);
                }
                None => {
                    let [r, g, b] = darker(shape.kind.capture());
                    let tint = [f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0];
                    let mut node = Node::new(0, shape.mesh, Material::lit(tint, if shape.kind.shines() { 1.0 } else { 0.0 }));
                    node.transform = shape.transform();
                    nodes.push(node);
                }
            }
            if shape.on_floor() { blocks.floor.extend(nodes) } else { blocks.standing.extend(nodes) }
        }
        let ids = self.ids();
        for (k, node) in blocks.floor.iter_mut().chain(&mut blocks.standing).enumerate() {
            node.id = ids | (k as u64 + 1);
        }
        let blocks = Arc::new(blocks);
        *kept = Some((first, Arc::clone(&blocks)));
        blocks
    }
}

/// How near (the squared distance of their capture colours) another kind
/// is to pass for a kind a chunk has no patch of, on a block a run stands
/// (`Chunk::standing_swatch`): about 30 a channel.
pub const STANDING_NEAR: i32 = 2500;

/// Whether kind `kind` is something the ground is (a floor a block stands
/// on), not something that stands.
pub fn lies(kind: Kind) -> bool {
    matches!(kind, Kind::Ground | Kind::Grass | Kind::Path | Kind::Road | Kind::Sand | Kind::Water | Kind::Mud | Kind::Floor | Kind::Rug | Kind::Crop)
}

/// A kind's capture colour two shades down its bank (`light::step_down` of
/// the realm's nearest plain ink): how a chunk with no plate and no patches
/// draws it, dark enough to sit under the painted ones.
pub fn darker(rgb: [u8; 3]) -> [u8; 3] {
    use crate::render::overworld::{ink, light};
    let c = ink::nearest_plain([f32::from(rgb[0]), f32::from(rgb[1]), f32::from(rgb[2])]);
    light::step_down(c, 2)
}

/// The box `set`'s shapes fill, its world's low and high corners (a turned
/// shape's footprint taken as the circle round it).
fn bounds_of(set: &Set) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for s in &set.shapes {
        let reach = if s.turn.rem_euclid(90.0).abs() < 1e-3 {
            let quarter = (s.turn.rem_euclid(180.0) - 90.0).abs() < 1.0;
            if quarter { [s.size[2] / 2.0, s.size[0] / 2.0] } else { [s.size[0] / 2.0, s.size[2] / 2.0] }
        } else {
            let r = s.size[0].hypot(s.size[2]) / 2.0;
            [r, r]
        };
        let box_lo = [s.at[0] - reach[0], s.at[1], s.at[2] - reach[1]];
        let box_hi = [s.at[0] + reach[0], s.at[1] + s.size[1].max(0.0), s.at[2] + reach[1]];
        for k in 0..3 {
            lo[k] = lo[k].min(box_lo[k]);
            hi[k] = hi[k].max(box_hi[k]);
        }
    }
    if let Some(g) = &set.ground {
        let (low, high) = g.heights.iter().fold((0.0f32, 0.0f32), |(a, b), &h| (a.min(h), b.max(h)));
        let far = [g.at[0] + g.cols as f32 * g.step, g.at[1] + g.rows as f32 * g.step];
        lo = [lo[0].min(g.at[0]), lo[1].min(low), lo[2].min(g.at[1])];
        hi = [hi[0].max(far[0]), hi[1].max(high), hi[2].max(far[1])];
    }
    if lo[0] > hi[0] {
        let [x0, z0, x1, z1] = set.camera.stage();
        return ([x0, 0.0, z0], [x1, 0.0, z1]);
    }
    (lo, hi)
}

// ---------------------------------------------------------- the chunks ----

/// The kingdom's chunks as a process has them: read from its asset folder
/// (a host), or as a host sent them (a page across a wire).
pub struct Chunks {
    /// The asset folder; none across a wire, where only what was sent is.
    folder: Option<PathBuf>,
    slots: Vec<Mutex<Slot>>,
    /// The decoded plates, by chunk key, the least lately used first.
    resident: Arc<Mutex<VecDeque<(u64, Arc<Plate>)>>>,
    /// The keys of the plates being decoded.
    loading: Arc<Mutex<Vec<u64>>>,
    told: AtomicBool,
    warming: AtomicBool,
}

#[derive(Default)]
struct Slot {
    chunk: Option<Arc<Chunk>>,
    /// When the folder was last looked at for it (a host's).
    #[cfg(not(target_arch = "wasm32"))]
    looked: Option<std::time::Instant>,
    #[cfg(target_arch = "wasm32")]
    looked: Option<()>,
}

impl Chunks {
    /// The chunks of asset folder `folder`: none of it is read until a chunk
    /// is asked for. A folder that is not there is a page's: only what it is
    /// sent.
    pub fn scan(folder: &Path) -> Chunks {
        Chunks {
            folder: folder.is_dir().then(|| folder.to_path_buf()),
            slots: (0..COUNT).map(|_| Mutex::new(Slot::default())).collect(),
            resident: Arc::new(Mutex::new(VecDeque::new())),
            loading: Arc::new(Mutex::new(Vec::new())),
            told: AtomicBool::new(false),
            warming: AtomicBool::new(false),
        }
    }

    /// A host's: every chunk made from the realm as it is asked for.
    pub fn host() -> Chunks {
        let mut chunks = Chunks::scan(Path::new(""));
        chunks.folder = Some(PathBuf::new());
        chunks
    }

    /// Whether it reads its own folder (a host) rather than what it is sent.
    pub fn local(&self) -> bool {
        self.folder.is_some()
    }

    /// Chunk `index` as it is now: its set always (a host's, the realm's,
    /// made the first time it is asked for and kept); a page's only when it
    /// was sent it.
    pub fn chunk(&self, index: u32) -> Option<Arc<Chunk>> {
        let slot = self.slots.get(index as usize)?;
        let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
        if self.folder.is_none() || slot.chunk.is_some() {
            return slot.chunk.clone();
        }
        if slot.looked.is_none() {
            #[cfg(not(target_arch = "wasm32"))]
            {
                slot.looked = Some(std::time::Instant::now());
            }
            #[cfg(target_arch = "wasm32")]
            {
                slot.looked = Some(());
            }
            slot.chunk = read_chunk(index).map(Arc::new);
        }
        slot.chunk.clone()
    }

    /// The chunks of `indices` that are there (a page's: those it was sent).
    pub fn of(&self, indices: &[u32]) -> Vec<Arc<Chunk>> {
        indices.iter().filter_map(|&i| self.chunk(i)).collect()
    }

    /// The ids of the chunks without a usable plate, of all of them (a host's).
    pub fn unpainted(&self) -> Vec<String> {
        (0..COUNT as u32).filter(|&i| !self.chunk(i).is_some_and(|c| c.painted())).map(|i| self.chunk_id(i)).collect()
    }

    fn chunk_id(&self, index: u32) -> String {
        let (cx, cz) = cell(index);
        chunks::chunk_id(cx, cz)
    }

    /// Say once, in the log, that the kingdom is drawn in 3D (a host, the
    /// first time): the realm, lit, every chunk made from the map.
    pub fn tell(&self) {
        if !self.local() || self.told.swap(true, Ordering::Relaxed) {
            return;
        }
        let made = (0..COUNT as u32).filter(|&i| self.chunk(i).is_some()).count();
        tracing::info!("world3d: the kingdom in 3D, the realm lit (docs/REALM-MONOLITH.md): {made} of {COUNT} chunks made");
    }

    /// Chunk `chunk`'s plate, decoded, when it is resident (and now the most
    /// lately used); else `None`, and it is asked for ([`Chunks::want`]).
    pub fn plate(&self, chunk: &Arc<Chunk>) -> Option<Arc<Plate>> {
        if !chunk.painted() {
            return None;
        }
        {
            let mut resident = self.resident.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(at) = resident.iter().position(|(k, _)| *k == chunk.plate_key) {
                let kept = resident.remove(at)?;
                let plate = Arc::clone(&kept.1);
                resident.push_back(kept);
                return Some(plate);
            }
        }
        self.want(chunk);
        None
    }

    /// Chunk `chunk`'s plate decoded now if it is not resident: for a proof,
    /// a test, or a terminal that cannot wait.
    pub fn plate_now(&self, chunk: &Arc<Chunk>) -> Option<Arc<Plate>> {
        if let Some(plate) = self.plate(chunk) {
            return Some(plate);
        }
        let plate = Arc::new(assets::decode(&chunk.png)?);
        keep_resident(&self.resident, chunk.plate_key, Arc::clone(&plate));
        Some(plate)
    }

    /// Decode chunk `chunk`'s plate off the frame (on a thread of its own),
    /// unless it is resident or on its way; kept resident once it is.
    pub fn want(&self, chunk: &Arc<Chunk>) {
        if !chunk.painted() {
            return;
        }
        if self.resident.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|(k, _)| *k == chunk.plate_key) {
            return;
        }
        {
            let mut loading = self.loading.lock().unwrap_or_else(|e| e.into_inner());
            if loading.contains(&chunk.plate_key) {
                return;
            }
            loading.push(chunk.plate_key);
        }
        let (png, key) = (Arc::clone(&chunk.png), chunk.plate_key);
        let (resident, loading) = (Arc::clone(&self.resident), Arc::clone(&self.loading));
        let decode = move || {
            if let Some(plate) = assets::decode(&png) {
                keep_resident(&resident, key, Arc::new(plate));
            }
            loading.lock().unwrap_or_else(|e| e.into_inner()).retain(|&k| k != key);
        };
        #[cfg(not(target_arch = "wasm32"))]
        if std::thread::Builder::new().name("world3d-plate".into()).spawn(decode).is_err() {
            self.loading.lock().unwrap_or_else(|e| e.into_inner()).retain(|&k| k != key);
        }
        #[cfg(target_arch = "wasm32")]
        decode();
    }

    /// How many plates are decoded now.
    pub fn resident(&self) -> usize {
        self.resident.lock().map_or(0, |r| r.len())
    }

    /// A chunk sent across a wire, kept unless it is the one kept already.
    /// False when it does not read.
    pub fn receive(&self, index: u32, key: u64, text: &str, png: &[u8], swatches: &[u8]) -> bool {
        let Some(slot) = self.slots.get(index as usize) else {
            return false;
        };
        let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
        if slot.chunk.as_ref().is_some_and(|c| c.key == key) {
            return true;
        }
        match Chunk::new(index, text.into(), png.into(), swatches.into()) {
            Ok(chunk) => {
                slot.chunk = Some(Arc::new(chunk));
                true
            }
            Err(why) => {
                tracing::warn!("world3d: a chunk sent does not read: {why}");
                false
            }
        }
    }

    /// Keep only the chunks `indices` of those sent (a page: what its tree
    /// holds now); a host keeps its own.
    pub fn keep(&self, indices: &[u32]) {
        if self.local() {
            return;
        }
        for (i, slot) in self.slots.iter().enumerate() {
            if !indices.contains(&(i as u32)) {
                slot.lock().unwrap_or_else(|e| e.into_inner()).chunk = None;
            }
        }
        let keys: Vec<u64> = self.slots.iter().filter_map(|s| s.lock().ok()?.chunk.as_ref().map(|c| c.plate_key)).collect();
        self.resident.lock().unwrap_or_else(|e| e.into_inner()).retain(|(k, _)| keys.contains(k));
    }
}

fn keep_resident(resident: &Mutex<VecDeque<(u64, Arc<Plate>)>>, key: u64, plate: Arc<Plate>) {
    let mut resident = resident.lock().unwrap_or_else(|e| e.into_inner());
    resident.retain(|(k, _)| *k != key);
    while resident.len() >= RESIDENT {
        resident.pop_front();
    }
    resident.push_back((key, plate));
}

/// The set chunk `index` is drawn from: the realm's (`realm::set_text`, the
/// map with no wishes), made once.
pub fn set_text(index: u32) -> Arc<str> {
    super::realm::set_text(index)
}

/// The kingdom as the chunks' generator reads it (the map with no wishes), made once.
pub fn land() -> &'static chunks::Land<'static> {
    static LAND: OnceLock<chunks::Land<'static>> = OnceLock::new();
    static GEO: OnceLock<Arc<crate::world::kingdom::Geography>> = OnceLock::new();
    LAND.get_or_init(|| chunks::Land::of(GEO.get_or_init(|| crate::world::kingdom::geography(&[])).as_ref()))
}

/// Chunk `index` as the realm makes it (docs/REALM-MONOLITH.md): its set,
/// drawn lit; plates are retired for the kingdom, so it has none.
fn read_chunk(index: u32) -> Option<Chunk> {
    let none: Arc<[u8]> = Arc::from(&[][..]);
    Chunk::new(index, set_text(index), Arc::clone(&none), none)
        .map_err(|why| tracing::warn!("world3d: the realm's chunk {index} does not read: {why}"))
        .ok()
}

/// Make every chunk's set and look the folder over now, off the frame (a
/// host turning the world in 3D on): the first view of the kingdom then
/// waits on nothing but its plates. Says which chunks have no plate.
pub fn warm() {
    let registry = chunks();
    if !registry.local() || registry.warming.swap(true, Ordering::Relaxed) {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = std::thread::Builder::new().name("world3d-chunks".into()).spawn(move || registry.tell());
}

/// The process's chunks, its folder (`assets::folder`) looked at as they are asked for.
pub fn chunks() -> &'static Chunks {
    #[cfg(test)]
    if let Some(chunks) = assets::test_chunks() {
        return chunks;
    }
    static CHUNKS: OnceLock<Chunks> = OnceLock::new();
    // A host makes the realm's chunks itself, asset folder or none; a page
    // (wasm) has only what it is sent.
    #[cfg(not(target_arch = "wasm32"))]
    return CHUNKS.get_or_init(Chunks::host);
    #[cfg(target_arch = "wasm32")]
    CHUNKS.get_or_init(|| Chunks::scan(&assets::folder()))
}

// ---------------------------------------------------------- terminals ----

/// How many chunks' plates laid down a terminal keeps (each 384 x 256 of
/// the kingdom's pixels, 0.4 MB): two views' worth of near chunks.
pub const LAID_KEPT: usize = 32;

/// Chunk `chunk`'s plate laid back down on the ground it was painted over,
/// for a terminal (and a page that draws no scenes): each pixel of the
/// chunk's stage the plate averaged over the area its patch of floor is cast
/// from, as a room's is (`room::RoomWorld::picture`), so what stands is
/// drawn up from its foot. `None` while its plate is not decoded (it is
/// asked for, off the frame) or when it has none. Kept, [`LAID_KEPT`] of them.
pub fn laid(chunk: &Arc<Chunk>) -> Option<Arc<crate::render::overworld::Img>> {
    type Kept = VecDeque<(u64, Arc<crate::render::overworld::Img>)>;
    static KEPT: Mutex<Kept> = Mutex::new(VecDeque::new());
    if !chunk.painted() {
        return None;
    }
    if let Ok(mut kept) = KEPT.lock()
        && let Some(at) = kept.iter().position(|(k, _)| *k == chunk.key)
    {
        let hit = kept.remove(at)?;
        let img = Arc::clone(&hit.1);
        kept.push_back(hit);
        return Some(img);
    }
    let plate = chunks().plate(chunk)?;
    let img = Arc::new(lay_down(chunk, &plate));
    if let Ok(mut kept) = KEPT.lock() {
        while kept.len() >= LAID_KEPT {
            kept.pop_front();
        }
        kept.push_back((chunk.key, Arc::clone(&img)));
    }
    Some(img)
}

fn lay_down(chunk: &Chunk, plate: &Plate) -> crate::render::overworld::Img {
    let [x0, z0, x1, z1] = chunk.set.camera.stage();
    let (w, h) = ((x1 - x0).round() as i32, (z1 - z0).round() as i32);
    let (pw, ph) = (plate.width as i32, plate.height as i32);
    let m = chunk.set.projector().view_projection;
    // Where the floor under the stage's pixel corner (x, z) is cast from, in plate pixels.
    let cast = |x: f32, z: f32| {
        let p = [x0 + x, 0.0, z0 + z];
        let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
        let q = row(3).max(1e-6);
        [(row(0) / q + 1.0) * 0.5 * pw as f32, (1.0 - row(1) / q) * 0.5 * ph as f32]
    };
    let corners: Vec<Vec<[f32; 2]>> = (0..=h).map(|z| (0..=w).map(|x| cast(x as f32, z as f32)).collect()).collect();
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for z in 0..h as usize {
        for x in 0..w as usize {
            let quad = [corners[z][x], corners[z][x + 1], corners[z + 1][x], corners[z + 1][x + 1]];
            let lo = |k: usize| quad.iter().map(|c| c[k]).fold(f32::INFINITY, f32::min);
            let hi = |k: usize| quad.iter().map(|c| c[k]).fold(f32::NEG_INFINITY, f32::max);
            let (px0, px1) = ((lo(0).floor() as i32).max(0), (hi(0).ceil() as i32).min(pw));
            let (py0, py1) = ((lo(1).floor() as i32).max(0), (hi(1).ceil() as i32).min(ph));
            let mut sum = [0u32; 3];
            let mut n = 0u32;
            for py in py0..py1 {
                for px in px0..px1 {
                    let i = ((py * pw + px) * 4) as usize;
                    // A clear texel is the black paper under the plate.
                    let a = u32::from(plate.rgba[i + 3]);
                    for (k, total) in sum.iter_mut().enumerate() {
                        *total += u32::from(plate.rgba[i + k]) * a / 255;
                    }
                    n += 1;
                }
            }
            pixels.push(if n == 0 { [0, 0, 0] } else { sum.map(|v| ((v + n / 2) / n) as u8) });
        }
    }
    crate::render::overworld::Img::from_rgb(w, h, &pixels)
}

/// The plates of the chunks a view of `view` pixels at `origin` (the
/// kingdom's pixels) overlaps, laid down over `img` (the view's picture of
/// the kingdom) where each chunk has one; the rest left as they are. A
/// chunk whose plate is not decoded yet comes in a frame or two later.
/// Where a chunk's plate is laid, what the run stands on its lots (`lots`)
/// is painted over it, each tile with the patch of what stands tallest
/// there, and the wishes' names on the ground before them; a chunk with no
/// plate keeps the baked kingdom's, which draws them already.
pub fn lay_plates(img: &mut crate::render::overworld::Img, origin: (i32, i32), lots: Option<&super::lots::Lots>) {
    let (ox, oy) = origin;
    let view = [ox as f32, oy as f32, (ox + img.w) as f32, (oy + img.h) as f32];
    let col = |x: f32| (x / STAGE.0).floor() as i32;
    let row = |z: f32| (z / STAGE.1).floor() as i32;
    let registry = chunks();
    warm();
    let mut laid_down = Vec::new();
    for cz in row(view[1]).max(0)..=row(view[3] - 0.01).min(GRID.1 - 1) {
        for cx in col(view[0]).max(0)..=col(view[2] - 0.01).min(GRID.0 - 1) {
            let Some(chunk) = index(cx, cz).and_then(|i| registry.chunk(i)) else { continue };
            let Some(laid) = laid(&chunk) else { continue };
            let (sx, sz) = ((cx as f32 * STAGE.0) as i32, (cz as f32 * STAGE.1) as i32);
            // The overlap, in the view's pixels.
            let (x0, x1) = ((sx - ox).max(0), (sx + laid.w - ox).min(img.w));
            let (y0, y1) = ((sz - oy).max(0), (sz + laid.h - oy).min(img.h));
            for y in y0..y1 {
                for x in x0..x1 {
                    if let Some(c) = laid.get(x + ox - sx, y + oy - sz) {
                        img.set(x, y, c);
                    }
                }
            }
            laid_down.push(chunk);
        }
    }
    let Some(lots) = lots else { return };
    for chunk in &laid_down {
        if let Some(share) = lots.share(chunk.index) {
            share.paint(img, origin, chunk);
        }
    }
    lots.lay_plaques(img, origin, &|i| laid_down.iter().any(|c| c.index == i));
}

// ------------------------------------------------------------ proofs ----

/// A run out in the kingdom (docs/WORLD-3D.md, "The kingdom"), a knight a
/// spot of `feet` (the kingdom's pixels), each camera on its knight, for a
/// proof shot or a walk.
#[cfg(any(test, feature = "testing"))]
pub fn proof_run(feet: &[[f32; 2]]) -> crate::rules::Run {
    proof_run_built(feet, Vec::new())
}

/// [`proof_run`] with the party's `wishes` built on their lots
/// (`world3d::lots`).
#[cfg(any(test, feature = "testing"))]
pub fn proof_run_built(feet: &[[f32; 2]], wishes: Vec<crate::realm::Wish>) -> crate::rules::Run {
    use crate::render::overworld::arena::UNIT;
    let mut run = crate::rules::Run::at_home(0x3D, 0x3D, Some("Friend"), Default::default(), Default::default());
    run.kingdom.wishes = Arc::new(wishes);
    for (k, name) in ["Goose Inspector", "Wren", "Corvid"].iter().enumerate().take(feet.len().saturating_sub(1)) {
        run.join_seat(k as u32 + 2, name);
    }
    run.walk_kingdom(None);
    let ids: Vec<u32> = run.players.keys().copied().collect();
    for (id, &[x, y]) in ids.into_iter().zip(feet) {
        let hero = run.players.get_mut(&id).expect("a seat");
        (hero.x, hero.y) = (x / UNIT, y / UNIT);
        hero.camera = Some((hero.x, hero.y - 1.0));
    }
    run
}

/// Where a knight stands just behind (north of) the tallest thing of kinds
/// `kinds` in chunk `(cx, cz)` with walkable ground behind it, so it hides
/// the knight's legs: its feet, the kingdom's pixels.
#[cfg(any(test, feature = "testing"))]
pub fn spot_behind(cx: i32, cz: i32, kinds: &[Kind]) -> Option<[f32; 2]> {
    let chunk = chunks().chunk(index(cx, cz)?)?;
    let geo = crate::world::kingdom::geography(&[]);
    let walk = |x: f32, z: f32| {
        let (c, r) = ((x / 16.0).floor() as i32, (z / 16.0).floor() as i32);
        (0..geo.room.cols as i32).contains(&c) && (0..geo.room.rows as i32).contains(&r) && super::blockout::walkable(geo.room.tile(c, r))
    };
    let [x0, z0, x1, z1] = chunk.set.camera.stage();
    chunk
        .set
        .shapes
        .iter()
        .filter(|s| kinds.contains(&s.kind) && !s.on_floor() && s.at[1] + s.size[1] >= 20.0)
        .filter_map(|s| {
            let quarter = (s.turn.rem_euclid(180.0) - 90.0).abs() < 45.0;
            let half = if quarter { s.size[0] / 2.0 } else { s.size[2] / 2.0 };
            // The nearest open ground north of it (a building's lot is
            // wider than the building), within a tile and a half.
            let feet = [3.0, 8.0, 13.0, 19.0, 24.0].into_iter().map(|d| [s.at[0], s.at[2] - half - d]).find(|f| walk(f[0], f[1]))?;
            let inside = feet[0] > x0 + 16.0 && feet[0] < x1 - 16.0 && feet[1] > z0 + 16.0 && feet[1] < z1;
            inside.then_some((s.at[1] + s.size[1], feet))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, feet)| feet)
}

/// Where a knight stands on open ground just behind (north of) a grove in
/// chunk `(cx, cz)`, three wood tiles deep at least to its south, so the
/// crowns hide its legs: its feet, the kingdom's pixels.
#[cfg(any(test, feature = "testing"))]
pub fn spot_behind_trees(cx: i32, cz: i32) -> Option<[f32; 2]> {
    let land = land();
    let (c0, r0, w, h) = land.chunk_rect(cx, cz);
    let woods = |c: i32, r: i32| c >= 0 && r >= 0 && c < land.cols && r < land.rows && land.cover(c, r) == chunks::Cover::Woods;
    (r0 + 1..r0 + h - 3)
        .flat_map(|r| (c0 + 1..c0 + w - 1).map(move |c| (c, r)))
        .find(|&(c, r)| land.walkable(c, r) && land.walkable(c, r - 1) && (1..=3).all(|k| woods(c, r + k)))
        .map(|(c, r)| [c as f32 * 16.0 + 8.0, r as f32 * 16.0 + 14.0])
}

/// A road crossing the border between two chunks that both have plates
/// (any two, when none do): a knight's feet on it, a tile short of the
/// border, and the chunks either side.
#[cfg(any(test, feature = "testing"))]
pub fn road_over_a_border() -> Option<([f32; 2], (i32, i32), (i32, i32))> {
    let geo = crate::world::kingdom::geography(&[]);
    let cols = geo.room.cols as i32;
    let road = |c: i32, r: i32| (0..cols).contains(&c) && r >= 0 && geo.roads.get((r * cols + c) as usize).copied().unwrap_or(false);
    let painted = |cx: i32, cz: i32| index(cx, cz).and_then(|i| chunks().chunk(i)).is_some_and(|c| c.painted());
    let mut any = None;
    for cz in 0..GRID.1 {
        for cx in 0..GRID.0 - 1 {
            let x = (cx + 1) * CHUNK_W;
            for r in cz * CHUNK_H + 3..(cz + 1) * CHUNK_H - 3 {
                if road(x - 2, r) && road(x - 1, r) && road(x, r) && road(x + 1, r) {
                    let found = ([(x - 1) as f32 * 16.0, r as f32 * 16.0 + 12.0], (cx, cz), (cx + 1, cz));
                    if painted(cx, cz) && painted(cx + 1, cz) {
                        return Some(found);
                    }
                    any = any.or(Some(found));
                }
            }
        }
    }
    any
}

#[cfg(test)]
#[path = "../../tests/world3d/kingdom__tests.rs"]
mod tests;
