//! A castle's stones as meshes (the Works, `world::works`): every stone a
//! box or eight corners, only the faces a eye can see, one mesh a chunk.
//!
//! **Faces.** Two stones that touch hide the faces between them, but only
//! once both are laid: a face's `cover` is the `laid` at which what covers it
//! stands (sampled at five points over the face against the castle's boxes),
//! [`NEVER`] if some of it is open to the air. A mesh built for the state
//! `laid` holds the faces of its stones with `cover > laid`. A mesh built at
//! the state a run of stones was finished at stays right while later stones
//! cover it (what is covered is under them and never seen), so a finished
//! lift is merged into one mesh a chunk and never built again.
//!
//! **Pillows.** A stone at game distance is twelve pixels by six, so it is
//! shaded as a cushion: each face's vertex normals lean out toward its
//! corners, and the sun's light and the dither turn that into a bright edge
//! and a dark one, so every stone reads as a stone with no joint drawn.
//!
//! **Kept.** Meshes are made once and kept (`Cache`): a lift finished, a
//! block of [`BLOCK`] stones of the lift going up, the falsework standing at
//! a moment; only the last few stones' mesh is made as they are laid.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::scene::MeshData;

use crate::world::works::{Castle, NEVER, PX_PER_M, Shape, Stone, Work};

/// Stones in a block of the lift being laid: the unit its mesh is kept in,
/// and the unit its wet mortar dries in.
pub const BLOCK: u32 = 64;

/// The first id of a castle mesh: clear of the chunks' (`0x6000_0000` up)
/// and the items' models.
const FIRST_ID: u32 = 0x5C00_0000;

/// The kingdom's chunk size in pixels (`world3d::kingdom::STAGE`).
const CHUNK_PX: (f32, f32) = (384.0, 256.0);

/// How much a face's normals lean out toward its corners.
const PILLOW: f32 = 0.5;

/// How deep the dark at a wall's foot reaches, metres.
const FOOT: f32 = 1.6;

/// The corners of each of a stone's six faces, in the architect's ring
/// order: the two rings, then the four sides.
const FACES: [[usize; 4]; 6] = [[0, 1, 2, 3], [4, 5, 6, 7], [0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]];

/// One mesh of a castle: a chunk's share of some stones or falsework.
#[derive(Debug)]
pub struct Part {
    /// The id it is uploaded under.
    pub id: u32,
    pub mesh: Arc<MeshData>,
    /// The box it fills, kingdom pixels, low and high corners.
    pub bounds: ([f32; 3], [f32; 3]),
    /// The chunk it stands in.
    pub chunk: (i32, i32),
}

fn next_id() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(FIRST_ID);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

// -------------------------------------------------------------- covers ----

/// Each stone's faces' `cover` (see the module): the `laid` at which the
/// face is hidden, [`NEVER`] if it never is.
pub struct Covers {
    pub faces: Vec<[u32; 6]>,
}

impl Covers {
    pub fn of(castle: &Castle) -> Covers {
        // A grid of 60 cm cells over the boxes.
        const CELL: i32 = 60;
        let cell = |v: i32| v.div_euclid(CELL);
        let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        let boxes: Vec<Option<[i16; 6]>> = castle.stones.iter().map(|s| if let Shape::Box(b) = s.shape { Some(b) } else { None }).collect();
        for (i, b) in boxes.iter().enumerate() {
            let Some(b) = b else { continue };
            for z in cell(i32::from(b[2]))..=cell(i32::from(b[5])) {
                for y in cell(i32::from(b[1]))..=cell(i32::from(b[4])) {
                    for x in cell(i32::from(b[0]))..=cell(i32::from(b[3])) {
                        grid.entry((x, y, z)).or_default().push(i as u32);
                    }
                }
            }
        }
        // The earliest stone other than `own` holding point `p` (cm).
        let covered = |p: [i32; 3], own: u32| -> u32 {
            let Some(list) = grid.get(&(cell(p[0]), cell(p[1]), cell(p[2]))) else { return NEVER };
            let mut first = NEVER;
            for &j in list {
                if j == own {
                    continue;
                }
                let Some(b) = boxes[j as usize] else { continue };
                if (0..3).all(|k| i32::from(b[k]) <= p[k] && p[k] < i32::from(b[k + 3])) {
                    first = first.min(j);
                }
            }
            first
        };
        let faces = castle
            .stones
            .iter()
            .enumerate()
            .map(|(i, stone)| match stone.shape {
                Shape::Box(b) => box_covers(b, i as u32, &covered),
                Shape::Hex(c) => {
                    let low = |ring: [usize; 4]| ring.iter().all(|&k| c[k][2] <= 5);
                    let mut out = [NEVER; 6];
                    for (f, ring) in FACES.iter().enumerate() {
                        // A face lying on the ground is under it.
                        if low(*ring) && f == 0 {
                            out[f] = 0;
                        }
                    }
                    out
                }
            })
            .collect();
        Covers { faces }
    }
}

/// A box's six faces' covers. Face order is [`FACES`]': bottom, top, `-y`,
/// `+x`, `+y`, `-x`.
fn box_covers(b: [i16; 6], own: u32, covered: &impl Fn([i32; 3], u32) -> u32) -> [u32; 6] {
    let lo = [i32::from(b[0]), i32::from(b[1]), i32::from(b[2])];
    let hi = [i32::from(b[3]), i32::from(b[4]), i32::from(b[5])];
    // (axis, outward sign) of each face.
    const SIDES: [(usize, i32); 6] = [(2, -1), (2, 1), (1, -1), (0, 1), (1, 1), (0, -1)];
    let mut out = [NEVER; 6];
    for (f, &(axis, sign)) in SIDES.iter().enumerate() {
        // A bottom on the ground is under it.
        if f == 0 && lo[2] <= 5 {
            out[f] = 0;
            continue;
        }
        let (a, c) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let at = if sign > 0 { hi[axis] } else { lo[axis] - 1 };
        let mut worst = 0u32;
        let mut open = false;
        for (fa, fc) in [(0.2, 0.2), (0.8, 0.2), (0.2, 0.8), (0.8, 0.8), (0.5, 0.5)] {
            let mut p = [0i32; 3];
            p[axis] = at;
            p[a] = lo[a] + ((hi[a] - lo[a]) as f32 * fa) as i32;
            p[c] = lo[c] + ((hi[c] - lo[c]) as f32 * fc) as i32;
            match covered(p, own) {
                NEVER => {
                    open = true;
                    break;
                }
                j => worst = worst.max(j + 1),
            }
        }
        out[f] = if open { NEVER } else { worst };
    }
    out
}

/// A castle's covers, made once.
pub fn covers(castle: &'static Castle) -> &'static Covers {
    static MADE: Mutex<Vec<(&'static str, &'static Covers)>> = Mutex::new(Vec::new());
    let mut made = MADE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, c)) = made.iter().find(|(id, _)| *id == castle.id) {
        return c;
    }
    let c: &'static Covers = Box::leak(Box::new(Covers::of(castle)));
    made.push((castle.id.as_str(), c));
    c
}

// --------------------------------------------------------------- tones ----

fn hash(i: u32, salt: u32) -> f32 {
    let mut h = i.wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// A stone's own tone, 0 to 1 (before the light): its material's, a little
/// lighter for dressed work, and a wander stone to stone that makes each one
/// its own.
pub fn albedo(castle: &Castle, stone: &Stone, index: u32) -> f32 {
    let base = match castle.material(stone.mat) {
        "ashlar-pale-warm" => 0.70,
        "ashlar-pale" => 0.64,
        "dressing-pale" => 0.78,
        "oak-weathered" => 0.30,
        "oak" => 0.36,
        "lead" => 0.34,
        "slate" => 0.24,
        "iron" => 0.18,
        _ => 0.5,
    };
    let wander = (hash(index, 1) - 0.5) * 0.20;
    // A few stones noticeably darker or paler: weathering, a different quarry.
    let odd = match hash(index, 2) {
        v if v < 0.05 => -0.10,
        v if v > 0.95 => 0.08,
        _ => 0.0,
    };
    (base + wander + odd).clamp(0.10, 0.92)
}

// ---------------------------------------------------------- the quads ----

/// A mesh being made, in kingdom pixels, one per chunk.
#[derive(Default)]
struct Builder {
    meshes: HashMap<(i32, i32), MeshData>,
}

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: V3) -> Option<V3> {
    let l = dot(a, a).sqrt();
    (l > 1e-6).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

impl Builder {
    /// One face of a stone: corners `q` in kingdom pixels in ring order,
    /// `centre` the stone's middle (to turn the face outward), a tone, and
    /// how far down it darkens toward its foot (`foot` is the ground's
    /// height in pixels, `ao` whether to).
    fn face(&mut self, chunk: (i32, i32), q: [V3; 4], centre: V3, tone: f32, ao: bool) {
        let (Some(tu), Some(tv)) = (norm(sub(q[1], q[0])), norm(sub(q[3], q[0]))) else {
            // Degenerate on that side: take the diagonal.
            return self.face_diag(chunk, q, centre, tone, ao);
        };
        let mut n = match norm(cross(tu, tv)) {
            Some(n) => n,
            None => return,
        };
        let mid = [(q[0][0] + q[2][0]) / 2.0, (q[0][1] + q[2][1]) / 2.0, (q[0][2] + q[2][2]) / 2.0];
        let flip = dot(n, sub(mid, centre)) < 0.0;
        if flip {
            n = n.map(|v| -v);
        }
        self.push(chunk, q, n, tu, tv, tone, ao, flip);
    }

    fn face_diag(&mut self, chunk: (i32, i32), q: [V3; 4], centre: V3, tone: f32, ao: bool) {
        // A triangle for a face with a doubled corner: its normal from the
        // two edges that are not nothing.
        let (a, b, c) = if norm(sub(q[1], q[0])).is_none() { (q[1], q[2], q[3]) } else { (q[0], q[1], q[3]) };
        let (Some(tu), Some(tv)) = (norm(sub(b, a)), norm(sub(c, a))) else { return };
        let Some(mut n) = norm(cross(tu, tv)) else { return };
        let mid = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
        let flip = dot(n, sub(mid, centre)) < 0.0;
        if flip {
            n = n.map(|v| -v);
        }
        self.push(chunk, q, n, tu, tv, tone, ao, flip);
    }

    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, chunk: (i32, i32), q: [V3; 4], n: V3, tu: V3, tv: V3, tone: f32, ao: bool, flip: bool) {
        let mesh = self.meshes.entry(chunk).or_default();
        let base = mesh.positions.len() as u32;
        for (k, p) in q.iter().enumerate() {
            let (su, sv) = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)][k];
            // Lean the normal out toward the corner, then true it.
            let lean = [
                n[0] + PILLOW * (su * tu[0] + sv * tv[0]),
                n[1] + PILLOW * (su * tu[1] + sv * tv[1]),
                n[2] + PILLOW * (su * tu[2] + sv * tv[2]),
            ];
            let nn = norm(lean).unwrap_or(n);
            // Darker at the foot of a wall, where it meets the ground.
            let shade = if ao { 0.74 + 0.26 * smooth(p[1] / (FOOT * PX_PER_M)) } else { 1.0 };
            let c = (tone * shade).clamp(0.0, 1.0);
            mesh.positions.push(*p);
            mesh.normals.push(nn);
            mesh.colors.push([c, c, c, 1.0]);
        }
        // CCW from outside: the ring order when the normal is the ring's own.
        if flip {
            mesh.indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    /// A stone at index `index`: its faces visible at state `state`.
    fn stone(&mut self, castle: &Castle, covers: &Covers, index: u32, state: u32) {
        let stone = &castle.stones[index as usize];
        let c = stone.corners().map(|p| castle.world(p));
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &c {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let chunk = ((centre[0] / CHUNK_PX.0).floor() as i32, (centre[2] / CHUNK_PX.1).floor() as i32);
        let tone = albedo(castle, stone, index);
        for (f, ring) in FACES.iter().enumerate() {
            if covers.faces[index as usize][f] <= state {
                continue;
            }
            let q = [c[ring[0]], c[ring[1]], c[ring[2]], c[ring[3]]];
            // Sides darken to their foot; tops and bottoms do not.
            self.face(chunk, q, centre, tone * if f == 1 { 1.04 } else { 1.0 }, f >= 2);
        }
    }

    /// A box of timber in kingdom pixels (`[x0, y0, z0, x1, y1, z1]`, `y`
    /// up), its five outer faces.
    fn timber(&mut self, b: [f32; 6], tone: f32) {
        let [x0, y0, z0, x1, y1, z1] = b;
        let chunk = (((x0 + x1) / 2.0 / CHUNK_PX.0).floor() as i32, ((z0 + z1) / 2.0 / CHUNK_PX.1).floor() as i32);
        let p = |x: f32, y: f32, z: f32| [x, y, z];
        // World frame: ring order bottom (y0) then top (y1), as the stones'.
        let c = [p(x0, y0, z1), p(x1, y0, z1), p(x1, y0, z0), p(x0, y0, z0), p(x0, y1, z1), p(x1, y1, z1), p(x1, y1, z0), p(x0, y1, z0)];
        let centre = [(x0 + x1) / 2.0, (y0 + y1) / 2.0, (z0 + z1) / 2.0];
        for (f, ring) in FACES.iter().enumerate().skip(1) {
            let q = [c[ring[0]], c[ring[1]], c[ring[2]], c[ring[3]]];
            self.face(chunk, q, centre, tone * if f == 1 { 1.05 } else { 1.0 }, f >= 2);
        }
    }

    fn finish(self) -> Vec<Arc<Part>> {
        let mut out: Vec<_> = self
            .meshes
            .into_iter()
            .filter(|(_, m)| !m.indices.is_empty())
            .map(|(chunk, mesh)| {
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for p in &mesh.positions {
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
                Arc::new(Part { id: next_id(), mesh: Arc::new(mesh), bounds: (lo, hi), chunk })
            })
            .collect();
        out.sort_by_key(|p| p.chunk);
        out
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ----------------------------------------------------------- the kept ----

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    /// Stones `from..to` as they are at `state`.
    Stones { plan: String, from: u32, to: u32, state: u32 },
    /// The falsework standing once `events` of its comings and goings have passed.
    Falsework { plan: String, events: u32 },
    /// Stones `from..to` at `state` with some knocked out (a raid's gaps): which, by their numbers.
    Gapped { plan: String, from: u32, to: u32, state: u32, lost: Vec<u32> },
}

/// How many meshes' sets are kept.
const KEPT: usize = 192;

type Kept = Vec<(Key, Vec<Arc<Part>>)>;

fn kept() -> &'static Mutex<Kept> {
    static KEPT_PARTS: OnceLock<Mutex<Kept>> = OnceLock::new();
    KEPT_PARTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn keep(key: Key, make: impl FnOnce() -> Vec<Arc<Part>>) -> Vec<Arc<Part>> {
    if let Some((_, parts)) = kept().lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return parts.clone();
    }
    let parts = make();
    let mut all = kept().lock().unwrap_or_else(|e| e.into_inner());
    if all.len() >= KEPT {
        all.remove(0);
    }
    all.push((key, parts.clone()));
    parts
}

/// The meshes of stones `from..to`, each face as it is with `state` stones
/// laid (`state >= to` for a run of stones that is done), a mesh a chunk.
pub fn stones(castle: &'static Castle, from: u32, to: u32, state: u32) -> Vec<Arc<Part>> {
    let to = to.min(castle.len());
    if from >= to {
        return Vec::new();
    }
    let key = Key::Stones { plan: castle.id.clone(), from, to, state };
    keep(key, || {
        let covers = covers(castle);
        let mut b = Builder::default();
        for i in from..to {
            b.stone(castle, covers, i, state);
        }
        b.finish()
    })
}

/// [`stones`] with the stones `lost` (sorted) knocked out of the run.
pub fn stones_gapped(castle: &'static Castle, from: u32, to: u32, state: u32, lost: &[u32]) -> Vec<Arc<Part>> {
    let to = to.min(castle.len());
    let gone: Vec<u32> = lost.iter().copied().filter(|&i| i >= from && i < to).collect();
    if gone.is_empty() {
        return stones(castle, from, to, state);
    }
    let key = Key::Gapped { plan: castle.id.clone(), from, to, state, lost: gone.clone() };
    keep(key, || {
        let covers = covers(castle);
        let mut b = Builder::default();
        for i in from..to {
            if gone.binary_search(&i).is_err() {
                b.stone(castle, covers, i, state);
            }
        }
        b.finish()
    })
}

/// The falsework standing with `laid` stones laid, as meshes. It changes
/// only when a piece comes or goes, so it is made for the count of those.
pub fn falsework(castle: &'static Castle, laid: u32) -> Vec<Arc<Part>> {
    falsework_built(castle, laid, laid)
}

/// The falsework standing with `laid` stones laid and the carpenters' bays
/// raised as far as stone `built` (`rules::hands`): a piece stands from when
/// it is raised until the stone it holds up is laid.
pub fn falsework_built(castle: &'static Castle, laid: u32, built: u32) -> Vec<Arc<Part>> {
    let built = built.max(laid);
    let events = castle.falsework.iter().map(|f| u32::from(f.appears <= built) + u32::from(f.gone <= laid)).sum::<u32>();
    let key = Key::Falsework { plan: castle.id.clone(), events };
    keep(key, || {
        let mut b = Builder::default();
        for f in castle.falsework.iter().filter(|f| f.appears <= built && laid < f.gone) {
            let m = |v: i16| f32::from(v) / 100.0;
            let [x0, y0, z0, x1, y1, z1] = f.bx.map(m);
            let wood = match castle.material(f.mat) {
                "oak-weathered" => 0.34,
                _ => 0.42,
            };
            match f.kind {
                Work::Scaffold => scaffold(&mut b, castle, [x0, y0, z0, x1, y1, z1], wood),
                Work::Centring => centring(&mut b, castle, [x0, y0, z0, x1, y1, z1], wood),
                // The crane is drawn alive, a node of its own (`works_life`).
                Work::Crane | Work::Other => {}
            }
        }
        b.finish()
    })
}

/// A bay of putlog scaffolding in a box: four poles, a deck at the top and
/// at the middle, the ledgers under them, a brace.
fn scaffold(b: &mut Builder, castle: &Castle, [x0, y0, z0, x1, y1, z1]: [f32; 6], tone: f32) {
    let w = |bx: [f32; 6]| {
        // Castle metres to kingdom pixels: x, z up, y north to -z.
        let [ax, ay, az, bx_, by, bz] = bx;
        let (p, q) = (castle.world([ax, ay, az]), castle.world([bx_, by, bz]));
        [p[0].min(q[0]), p[1].min(q[1]), p[2].min(q[2]), p[0].max(q[0]), p[1].max(q[1]), p[2].max(q[2])]
    };
    let pole = 0.10;
    let h = z1 - z0;
    // Four poles.
    for (px, py) in [(x0, y0), (x1 - pole, y0), (x0, y1 - pole), (x1 - pole, y1 - pole)] {
        b.timber(w([px, py, z0, px + pole, py + pole, z1]), tone);
    }
    // Decks: the top and, if it is tall, a mid-height one.
    let mut levels = vec![z1 - 0.10];
    if h > 2.0 {
        levels.push(z0 + h / 2.0);
    }
    for z in levels {
        b.timber(w([x0, y0, z, x1, y1, z + 0.07]), tone * 1.12);
    }
    // Ledgers along the long side at the top, and a cross brace.
    b.timber(w([x0, y0, z1 - 0.9, x0 + pole * 0.7, y1, z1 - 0.9 + pole * 0.7]), tone);
    b.timber(w([x1 - pole * 0.7, y0, z1 - 0.9, x1, y1, z1 - 0.9 + pole * 0.7]), tone);
}

/// Timber centring under an arch: two posts, a cross beam, a rib across the top.
fn centring(b: &mut Builder, castle: &Castle, [x0, y0, z0, x1, y1, z1]: [f32; 6], tone: f32) {
    let w = |bx: [f32; 6]| {
        let [ax, ay, az, bx_, by, bz] = bx;
        let (p, q) = (castle.world([ax, ay, az]), castle.world([bx_, by, bz]));
        [p[0].min(q[0]), p[1].min(q[1]), p[2].min(q[2]), p[0].max(q[0]), p[1].max(q[1]), p[2].max(q[2])]
    };
    let post = 0.16;
    for (px, py) in [(x0, y0), (x1 - post, y0), (x0, y1 - post), (x1 - post, y1 - post)] {
        b.timber(w([px, py, z0, px + post, py + post, z1 - 0.3]), tone);
    }
    b.timber(w([x0, y0, z1 - 0.3, x1, y1, z1]), tone * 1.1);
}

/// Pegs for stage `stage`: a stake at each of its first course's corners,
/// made once.
pub fn pegs(castle: &'static Castle, stage: usize) -> Vec<Arc<Part>> {
    let key = Key::Falsework { plan: castle.id.clone(), events: 0x8000_0000 | stage as u32 };
    keep(key, || {
        let mut b = Builder::default();
        for [x, y] in crate::world::works_life::peg_places(castle, stage) {
            let w = |bx: [f32; 6]| {
                let (p, q) = (castle.world([bx[0], bx[1], bx[2]]), castle.world([bx[3], bx[4], bx[5]]));
                [p[0].min(q[0]), p[1].min(q[1]), p[2].min(q[2]), p[0].max(q[0]), p[1].max(q[1]), p[2].max(q[2])]
            };
            b.timber(w([x - 0.05, y - 0.05, 0.0, x + 0.05, y + 0.05, 0.55]), 0.7);
        }
        b.finish()
    })
}
