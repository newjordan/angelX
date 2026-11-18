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
//!
//! The chunks' painted plates, and the machinery that kept them decoded and
//! laid them down for a terminal, were cut on 2026-10-20 (none had a plate).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::scene::Node;
use angel_stage::set::Set;

use super::chunks;
use crate::world::map::{CHUNK_H, CHUNK_W, CHUNKS_X, CHUNKS_Y};

pub const GRID: (i32, i32) = (CHUNKS_X, CHUNKS_Y);
pub const COUNT: usize = (CHUNKS_X * CHUNKS_Y) as usize;
pub const STAGE: (f32, f32) = ((CHUNK_W * 16) as f32, (CHUNK_H * 16) as f32);
pub const RESIDENT: usize = 16;

pub fn index(cx: i32, cz: i32) -> Option<u32> {
    ((0..GRID.0).contains(&cx) && (0..GRID.1).contains(&cz)).then(|| (cz * GRID.0 + cx) as u32)
}

pub fn cell(index: u32) -> (i32, i32) {
    (index as i32 % GRID.0, index as i32 / GRID.0)
}

pub fn near(view: [f32; 4]) -> Vec<u32> {
    cover(view, 1)
}

pub fn overlaps(view: [f32; 4]) -> Vec<u32> {
    cover(view, 0)
}

fn cover([x0, z0, x1, z1]: [f32; 4], ring: i32) -> Vec<u32> {
    let col = |x: f32| (x / STAGE.0).floor() as i32;
    let row = |z: f32| (z / STAGE.1).floor() as i32;
    let (c0, c1) = (col(x0) - ring, col(x1 - 0.01) + ring);
    let (r0, r1) = (row(z0) - ring, row(z1 - 0.01) + ring);
    (r0.max(0)..=r1.min(GRID.1 - 1)).flat_map(|r| (c0.max(0)..=c1.min(GRID.0 - 1)).filter_map(move |c| index(c, r))).collect()
}

pub fn near_centre(centre: (f32, f32), view: (f32, f32)) -> Vec<u32> {
    let (kw, kh) = (GRID.0 as f32 * STAGE.0, GRID.1 as f32 * STAGE.1);
    let x = (centre.0 - view.0 / 2.0).clamp(0.0, (kw - view.0).max(0.0));
    let z = (centre.1 - view.1 / 2.0).clamp(0.0, (kh - view.1).max(0.0));
    near([x, z, x + view.0, z + view.1])
}

pub struct Chunk {
    pub index: u32,
    pub set: Set,
    pub text: Arc<str>,
    pub key: u64,
    pub bounds: ([f32; 3], [f32; 3]),
    lit: OnceLock<Arc<Lit>>,
}

#[derive(Debug, Default)]
pub struct Lit {
    pub floor: Vec<Node>,
    pub standing: Vec<Node>,
    pub meshes: Vec<(u32, Arc<angel_stage::scene::MeshData>)>,
}

pub fn bake(set: &Set, key: u64, ids: u64) -> Lit {
    let lit = angel_stage::set::bake(set, key, ids);
    Lit { floor: lit.floor, standing: lit.standing, meshes: lit.meshes }
}

impl Chunk {
    pub fn new(index: u32, text: Arc<str>) -> Result<Chunk, String> {
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
        let mut key = 0xcbf2_9ce4_8422_2325u64;
        for &byte in text.as_bytes() {
            key ^= u64::from(byte);
            key = key.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let bounds = bounds_of(&set);
        Ok(Chunk { index, set, text, key, bounds, lit: OnceLock::new() })
    }

    pub fn cut(&self, text: Arc<str>) -> Result<Chunk, String> {
        Chunk::new(self.index, text)
    }

    pub fn ids(&self) -> u64 {
        (u64::from(self.index) + 1) << 32
    }

    pub fn lit(&self) -> Arc<Lit> {
        Arc::clone(self.lit.get_or_init(|| Arc::new(bake(&self.set, self.key, self.ids()))))
    }

    pub fn id(&self) -> String {
        let (cx, cz) = cell(self.index);
        chunks::chunk_id(cx, cz)
    }

}

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

pub struct Chunks {
    folder: Option<PathBuf>,
    slots: Vec<Mutex<Slot>>,
    told: AtomicBool,
    warming: AtomicBool,
}

#[derive(Default)]
struct Slot {
    chunk: Option<Arc<Chunk>>,
    #[cfg(not(target_arch = "wasm32"))]
    looked: Option<std::time::Instant>,
    #[cfg(target_arch = "wasm32")]
    looked: Option<()>,
}

impl Chunks {
    pub fn scan(folder: &Path) -> Chunks {
        Chunks {
            folder: folder.is_dir().then(|| folder.to_path_buf()),
            slots: (0..COUNT).map(|_| Mutex::new(Slot::default())).collect(),
            told: AtomicBool::new(false),
            warming: AtomicBool::new(false),
        }
    }

    pub fn host() -> Chunks {
        let mut chunks = Chunks::scan(Path::new(""));
        chunks.folder = Some(PathBuf::new());
        chunks
    }

    pub fn local(&self) -> bool {
        self.folder.is_some()
    }

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

    pub fn of(&self, indices: &[u32]) -> Vec<Arc<Chunk>> {
        indices.iter().filter_map(|&i| self.chunk(i)).collect()
    }

    pub fn tell(&self) {
        if !self.local() || self.told.swap(true, Ordering::Relaxed) {
            return;
        }
        let made = (0..COUNT as u32).filter(|&i| self.chunk(i).is_some()).count();
        tracing::info!("world3d: the kingdom in 3D, the realm lit (docs/REALM-MONOLITH.md): {made} of {COUNT} chunks made");
    }

    pub fn receive(&self, index: u32, key: u64, text: &str) -> bool {
        let Some(slot) = self.slots.get(index as usize) else {
            return false;
        };
        let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
        if slot.chunk.as_ref().is_some_and(|c| c.key == key) {
            return true;
        }
        match Chunk::new(index, text.into()) {
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

    pub fn keep(&self, indices: &[u32]) {
        if self.local() {
            return;
        }
        for (i, slot) in self.slots.iter().enumerate() {
            if !indices.contains(&(i as u32)) {
                slot.lock().unwrap_or_else(|e| e.into_inner()).chunk = None;
            }
        }
    }
}

pub fn set_text(index: u32) -> Arc<str> {
    super::realm::set_text(index)
}

pub fn land() -> &'static chunks::Land<'static> {
    static LAND: OnceLock<chunks::Land<'static>> = OnceLock::new();
    static GEO: OnceLock<Arc<crate::world::kingdom::Geography>> = OnceLock::new();
    LAND.get_or_init(|| chunks::Land::of(GEO.get_or_init(|| crate::world::kingdom::geography(&[])).as_ref()))
}

fn read_chunk(index: u32) -> Option<Chunk> {
    Chunk::new(index, set_text(index))
        .map_err(|why| tracing::warn!("world3d: the realm's chunk {index} does not read: {why}"))
        .ok()
}

pub fn warm() {
    let registry = chunks();
    if !registry.local() || registry.warming.swap(true, Ordering::Relaxed) {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = std::thread::Builder::new().name("world3d-chunks".into()).spawn(move || registry.tell());
}

pub fn chunks() -> &'static Chunks {
    #[cfg(test)]
    if let Some(chunks) = TEST_CHUNKS.with(|c| c.get()) {
        return chunks;
    }
    static CHUNKS: OnceLock<Chunks> = OnceLock::new();
    #[cfg(not(target_arch = "wasm32"))]
    return CHUNKS.get_or_init(Chunks::host);
    #[cfg(target_arch = "wasm32")]
    CHUNKS.get_or_init(|| Chunks::scan(Path::new("")))
}

#[cfg(test)]
std::thread_local! {
    static TEST_CHUNKS: std::cell::Cell<Option<&'static Chunks>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub fn with_folder<T>(folder: &Path, f: impl FnOnce() -> T) -> T {
    struct Back(Option<&'static Chunks>);
    impl Drop for Back {
        fn drop(&mut self) {
            TEST_CHUNKS.with(|c| c.set(self.0));
        }
    }
    let chunks: &'static Chunks = Box::leak(Box::new(Chunks::scan(folder)));
    let _back = Back(TEST_CHUNKS.with(|c| c.replace(Some(chunks))));
    f()
}

#[cfg(any(test, feature = "testing"))]
pub fn proof_run(feet: &[[f32; 2]]) -> crate::rules::Run {
    proof_run_built(feet, Vec::new())
}

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

#[cfg(any(test, feature = "testing"))]
pub fn spot_behind(cx: i32, cz: i32, kinds: &[angel_stage::set::Kind]) -> Option<[f32; 2]> {
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
            let feet = [3.0, 8.0, 13.0, 19.0, 24.0].into_iter().map(|d| [s.at[0], s.at[2] - half - d]).find(|f| walk(f[0], f[1]))?;
            let inside = feet[0] > x0 + 16.0 && feet[0] < x1 - 16.0 && feet[1] > z0 + 16.0 && feet[1] < z1;
            inside.then_some((s.at[1] + s.size[1], feet))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, feet)| feet)
}

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

#[cfg(any(test, feature = "testing"))]
pub fn road_over_a_border() -> Option<([f32; 2], (i32, i32), (i32, i32))> {
    let geo = crate::world::kingdom::geography(&[]);
    let cols = geo.room.cols as i32;
    let road = |c: i32, r: i32| (0..cols).contains(&c) && r >= 0 && geo.roads.get((r * cols + c) as usize).copied().unwrap_or(false);
    for cz in 0..GRID.1 {
        for cx in 0..GRID.0 - 1 {
            let x = (cx + 1) * CHUNK_W;
            for r in cz * CHUNK_H + 3..(cz + 1) * CHUNK_H - 3 {
                if road(x - 2, r) && road(x - 1, r) && road(x, r) && road(x + 1, r) {
                    return Some(([(x - 1) as f32 * 16.0, r as f32 * 16.0 + 12.0], (cx, cz), (cx + 1, cz)));
                }
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "../../tests/world3d/kingdom__tests.rs"]
mod tests;
