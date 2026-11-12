//! The hillside works' solids merged into meshes. A building is forty blocks
//! or so, a cart as many, a road tile half a dozen; as nodes each is a draw
//! (about seven microseconds of GPU time on a desktop card), so a works of
//! seventy buildings, a hundred road tiles and a score of carts is two
//! thousand draws a frame. Baked into a mesh each, a building, a carrier or a
//! chunk of road is one.
//!
//! - A **building** is baked in its own frame (its footprint's corner the
//!   origin) once per `(plan, state, step, size)` and put where it stands by
//!   its node's translation, so every sawpit shares one mesh.
//! - A **carrier** is baked centred, heading as the rider heads, once per
//!   `(kind, heading, load, step, detail)`.
//! - The **roads and lines** are baked in the world, one mesh a chunk of the
//!   kingdom, made again only when what lies in that chunk changes (a tile
//!   laid or taken up, a ropeway's tower, a barge lane's ripple one step on),
//!   told by a hash of its tiles.
//!
//! A mesh carries its blocks' tones as its vertex colours (what a node's
//! material tone was), so the look sees what it saw; the blocks that shine
//! (a fire, a lit window) and the water are meshes of their own, drawn with
//! the accent and the water materials ([`PartRole`]).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::scene::{MeshData, MeshRef, Role, quat_rotate};
use angel_stage::set::Shape as SetShape;

use super::blockout::Solid;

/// The first id of a hillside mesh, clear of the chunks' (`0x6000_0000` up), the Keep's
/// (`works_mesh`, `0x5C00_0000`) and the items' models.
const FIRST_ID: u32 = 0x5D00_0000;

/// How a mesh is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PartRole {
    /// Lit, its tones in its vertex colours.
    Plain,
    /// Lit, as water is (it runs and glints).
    Water,
    /// Keeps its own colour, undithered, as a fire or a lit window does.
    Accent,
}

/// One baked mesh, in its own frame, and what it is.
#[derive(Debug)]
pub struct Baked {
    pub id: u32,
    pub mesh: Arc<MeshData>,
    pub role: PartRole,
    /// The box it fills in its own frame, low and high corners.
    pub bounds: ([f32; 3], [f32; 3]),
}

/// A baked mesh as it stands this frame: moved by `at`.
#[derive(Clone, Debug)]
pub struct HillPart {
    pub baked: Arc<Baked>,
    pub at: [f32; 3],
}

impl HillPart {
    /// The box it fills in the kingdom's pixels.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let (lo, hi) = self.baked.bounds;
        (std::array::from_fn(|k| lo[k] + self.at[k]), std::array::from_fn(|k| hi[k] + self.at[k]))
    }
}

fn next_id() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(FIRST_ID);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// A shape's own mesh, made once.
fn unit(name: &str) -> Option<Arc<(MeshRef, MeshData)>> {
    static MADE: OnceLock<Mutex<HashMap<String, Option<Arc<(MeshRef, MeshData)>>>>> = OnceLock::new();
    let memory = MADE.get_or_init(Default::default);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(name).cloned()) {
        return hit;
    }
    let made = angel_stage::shapes::by_name(name).and_then(|r| r.generate().map(|d| Arc::new((r, d))));
    if let Ok(mut m) = memory.lock() {
        m.insert(name.to_string(), made.clone());
    }
    made
}

/// An empty mesh being added to.
#[derive(Default)]
struct Heap {
    mesh: MeshData,
    lo: [f32; 3],
    hi: [f32; 3],
}

impl Heap {
    fn new() -> Self {
        Self { lo: [f32::MAX; 3], hi: [f32::MIN; 3], ..Self::default() }
    }

    /// `unit` placed as `shape` places it, every vertex `colour`.
    fn add(&mut self, unit: &MeshData, shape: &SetShape, colour: [f32; 4], shift: [f32; 3]) {
        let t = shape.transform();
        let base = self.mesh.positions.len() as u32;
        let n = unit.positions.len();
        for (i, p) in unit.positions.iter().enumerate() {
            let scaled = [p[0] * t.scale[0], p[1] * t.scale[1], p[2] * t.scale[2]];
            let r = quat_rotate(t.rotation, scaled);
            let at = [r[0] + t.translation[0] + shift[0], r[1] + t.translation[1] + shift[1], r[2] + t.translation[2] + shift[2]];
            for k in 0..3 {
                self.lo[k] = self.lo[k].min(at[k]);
                self.hi[k] = self.hi[k].max(at[k]);
            }
            self.mesh.positions.push(at);
            // A normal turns by the inverse of the scale, so a squashed block keeps its faces' directions.
            let m = unit.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
            let s = quat_rotate(t.rotation, [m[0] / t.scale[0].max(1e-4), m[1] / t.scale[1].max(1e-4), m[2] / t.scale[2].max(1e-4)]);
            let len = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt().max(1e-6);
            self.mesh.normals.push([s[0] / len, s[1] / len, s[2] / len]);
            self.mesh.uvs.push(unit.uvs.get(i).copied().unwrap_or([0.0, 0.0]));
            self.mesh.colors.push(colour);
        }
        self.mesh.indices.extend(unit.indices.iter().map(|&k| k + base));
        debug_assert_eq!(self.mesh.positions.len() as u32, base + n as u32);
    }

    fn done(self, role: PartRole) -> Option<Baked> {
        if self.mesh.positions.is_empty() || self.mesh.indices.is_empty() {
            return None;
        }
        Some(Baked { id: next_id(), mesh: Arc::new(self.mesh), role, bounds: (self.lo, self.hi) })
    }
}

/// `solids`, each at its place plus `shift`, baked: their tones (through `tone`) in the vertex colours,
/// the shining blocks in one mesh, water in another, the rest in a third; those of them there are.
pub fn bake(solids: &[Solid], shift: [f32; 3], tone: impl Fn(f32) -> f32) -> Vec<Arc<Baked>> {
    let (mut plain, mut water, mut accent) = (Heap::new(), Heap::new(), Heap::new());
    for s in solids {
        let name = s.shape.written();
        let Some(unit) = unit(&name) else { continue };
        let shape = SetShape { line: 0, shape: name, mesh: unit.0, kind: s.kind, at: s.at, size: s.size, turn: s.turn, id: None, top: None };
        if s.kind.shines() {
            let [r, g, b] = s.kind.capture();
            accent.add(&unit.1, &shape, [f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0], shift);
        } else {
            let t = tone(s.kind.tone());
            let heap = if s.kind.role() == Role::Water { &mut water } else { &mut plain };
            heap.add(&unit.1, &shape, [t, t, t, 1.0], shift);
        }
    }
    [(plain, PartRole::Plain), (water, PartRole::Water), (accent, PartRole::Accent)].into_iter().filter_map(|(h, r)| h.done(r)).map(Arc::new).collect()
}
