//! Land beyond the walls, from `ground(x, y)`: square chunks, each built once
//! at a level of detail that follows its distance, then only gathered.
use super::cache::{self, Key, Tris};
use super::geo::Soup;
use super::index::{self, RoadIndex};
use super::look::style;
use super::palette::{GLOW, LAVA, SKY, WATER, far_uv, special_uv};
use super::route::{Look, Route};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Tri;

pub const CHUNK: f32 = 96.;
/// Cells along a chunk side per level of detail: 8 m (lit), 8 m, 16 m, 32 m, 48 m.
const CELLS: [usize; 5] = [12, 12, 6, 3, 2];
/// Levels from this one out are baked: unlit colours with the haze built in.
const BAKED_FROM: u8 = 1;
/// The sun's direction for the baked faces.
const SUN: V3 = V3 { x: -0.55, y: -0.35, z: 0.25 };

pub fn look_index(look: Look) -> usize { match look { Look::Dune => 0, Look::Forge | Look::Ash => 1, Look::Deep => 2, Look::Castle => 3 } }

/// The detail and haze a chunk wants at `dist` metres from the eye, packed
/// for the cache key (detail in the high nibble, haze in the low). Near the
/// road the cells stay fine out to where the road is drawn.
pub fn lod_for(dist: f32, on_road: bool) -> u8 {
    let detail: u8 = if dist < 110. { 0 } else if on_road && dist < 360. { 1 } else if dist < 270. { 2 } else if dist < 480. { 3 } else { 4 };
    let haze: u8 = if dist < 110. { 0 } else if dist < 210. { 0 } else if dist < 340. { 1 } else if dist < 570. { 2 } else { 3 };
    detail << 4 | haze
}

/// The nearest road to a point, widening the search until some is found.
fn find(idx: &RoadIndex, x: f32, y: f32) -> Option<index::Near> {
    [30., 100., 320., 1300.].into_iter().find_map(|reach| idx.nearest(x, y, reach))
}

/// Whether a chunk lies on the road: close enough to need fine cells.
pub fn on_road(r: &dyn Route, cx: i32, cy: i32) -> bool {
    let (x, y) = ((cx as f32 + 0.5) * CHUNK, (cy as f32 + 0.5) * CHUNK);
    index::of(r).nearest(x, y, 110.).is_some_and(|n| n.dist < 105.)
}

/// How much of a sinking zone's drop its lake fills, and the most it may sit below the road:
/// a lake near the road is a lake the lens can see.
const LAKE_FILL: f32 = 0.55;
/// How far under the road the liquid lies at most, near the road (lava, water): the chase lens is a few metres up, and a
/// sheet that lies deeper is hidden behind the barrier for the whole of the road's width.
/// How far under the road the moat's water lies.
pub const WATER_DEEP: f32 = 1.7;
fn lake_deep(liquid: u8) -> f32 {
    use super::palette::{GL_MIST, LAVA};
    if liquid == LAVA { 2.6 } else if liquid == GL_MIST { 2.2 } else { WATER_DEEP }
}

/// The level of the lake that fills the low land of a chunk: where the land beside the road
/// sinks (a moat, a molten chasm, a lava lake), a flat sheet of liquid lies in it.
fn lake(r: &dyn Route, idx: &RoadIndex, cx: i32, cy: i32) -> Option<f32> {
    let (x, y) = ((cx as f32 + 0.5) * CHUNK, (cy as f32 + 0.5) * CHUNK);
    let n = idx.nearest(x, y, 110.)?;
    let liquid = style(r.zone(n.s).0).liquid?;
    let sink = r.sink(n.s);
    (sink > 0.).then(|| n.h - (LAKE_FILL * sink).min(lake_deep(liquid)))
}

/// Terrain height at a point, held below the road wherever the road runs, and
/// raised to the lake's sheet where it lies lower: the height, and whether it is under liquid.
fn height(r: &dyn Route, idx: &RoadIndex, lake: Option<f32>, x: f32, y: f32) -> (f32, bool) {
    let g = r.ground(x, y);
    // Beside the road the land starts under the road's level and rises to its own, over a few
    // metres: a lattice of points cannot follow a lip, and a hard step breaks into teeth.
    let (g, cap) = match idx.nearest(x, y, 30.) {
        Some(n) => {
            let t = ((n.dist - n.edge - 1.2) / 5.).clamp(0., 1.);
            let t = t * t * (3. - 2. * t);
            let low = g.min(n.h - 0.5);
            (low + (g - low) * t, n.h - 1.)
        }
        None => (g, f32::MAX),
    };
    match lake.map(|l| l.min(cap)) {
        Some(l) if g <= l => (l, true),
        _ => (g, false),
    }
}

/// How finely a cell is cut: the land's steepness across it (its corners' spread and
/// how far its middle strays from them), against its size. A canyon wall that rises
/// thirty metres in ten needs a point every two, or it breaks into slabs.
fn split(lod: u8, corners: [f32; 4], middle: f32, cell: f32) -> usize {
    let (lo, hi) = corners.iter().fold((f32::MAX, f32::MIN), |(l, h), &z| (l.min(z), h.max(z)));
    let bump = (middle - corners.iter().sum::<f32>() / 4.).abs() * 2.;
    let slope = (hi - lo).max(bump) / cell;
    match lod {
        0 => if slope > 0.38 { 2 } else { 1 },
        1 => if slope > 1.4 { 2 } else { 1 },
        _ => 1,
    }
}

/// The land's samples for one chunk, on a grid a quarter of a cell apart, each taken once;
/// a ring of cells around the chunk is sampled too, so a cell on the edge knows how finely
/// its neighbour is cut.
struct Samples<'a> {
    r: &'a dyn Route,
    idx: &'a RoadIndex,
    lake: Option<f32>,
    origin: (f32, f32),
    step: f32,
    side: usize,
    memo: Vec<Option<(f32, bool)>>,
}

impl Samples<'_> {
    fn at(&mut self, kx: i32, ky: i32) -> (f32, bool) {
        let slot = (ky + 4) as usize * self.side + (kx + 4) as usize;
        if let Some(v) = self.memo[slot] { return v; }
        let v = height(self.r, self.idx, self.lake, self.origin.0 + kx as f32 * self.step, self.origin.1 + ky as f32 * self.step);
        self.memo[slot] = Some(v);
        v
    }
}

pub fn chunk(r: &dyn Route, cx: i32, cy: i32, lod: u8) -> Tris {
    cache::get(Key { course: r.key(), kind: 2, a: cx, b: cy, lod }, || build(r, cx, cy, lod >> 4, (lod & 15) as usize))
}

fn build(r: &dyn Route, cx: i32, cy: i32, lod: u8, hz: usize) -> Vec<Tri> {
    let idx = index::of(r);
    let n = CELLS[lod as usize];
    let cell = CHUNK / n as f32;
    let (x0, y0) = (cx as f32 * CHUNK, cy as f32 * CHUNK);
    let baked = lod >= BAKED_FROM;
    let lake = lake(r, idx, cx, cy);
    let side = 4 * (n + 2) + 1;
    let mut smp = Samples { r, idx, lake, origin: (x0, y0), step: cell * 0.25, side, memo: vec![None; side * side] };
    let ni = n as i32;
    let mut factor = vec![1usize; (n + 2) * (n + 2)];
    for j in -1..=ni {
        for i in -1..=ni {
            let c = [smp.at(4 * i, 4 * j).0, smp.at(4 * i + 4, 4 * j).0, smp.at(4 * i + 4, 4 * j + 4).0, smp.at(4 * i, 4 * j + 4).0];
            factor[(j + 1) as usize * (n + 2) + (i + 1) as usize] = split(lod, c, smp.at(4 * i + 2, 4 * j + 2).0, cell);
        }
    }
    let fac = |i: i32, j: i32| factor[(j + 1) as usize * (n + 2) + (i + 1) as usize];
    let chunk_look = find(idx, x0 + CHUNK * 0.5, y0 + CHUNK * 0.5).map(|n| r.zone(n.s).0).unwrap_or(Look::Dune);
    let mut g = Soup::new();
    // `flood`: the road's height beside the land, for the baked light.
    let paint = |g: &mut Soup, a: V3, b: V3, c: V3, look: Look, road_h: Option<f32>, wet: bool| {
        let nrm = (b - a).cross(c - a).normalize();
        if wet && style(look).liquid.is_some() {
            let liquid = style(look).liquid.unwrap();
            if baked {
                if liquid == super::palette::LAVA { g.tri_uv(a, b, c, [special_uv(3); 3], SKY); }
                else if liquid == super::palette::GL_MIST { g.tri_uv(a, b, c, [special_uv(52); 3], SKY); }
                else { g.tri_uv(a, b, c, [special_uv(10); 3], SKY); }
            } else if liquid == super::palette::GL_MIST {
                // Light, not a surface: flat, unshaded and fog-proof, in cells of cyan, deep blue, violet and white.
                let m = (a + b + c) * (1. / 3.);
                let h = super::noise::hash((m.x / 9.).floor() as i32, (m.y / 9.).floor() as i32, 811);
                let kind = if h < 0.5 { 52 } else if h < 0.86 { 53 } else if h < 0.96 { 54 } else { 55 };
                g.tri_uv(a, b, c, [special_uv(kind); 3], SKY);
            } else {
                g.tri(a, b, c, liquid);
            }
        } else if baked {
            let up = if nrm.z < 0. { -nrm } else { nrm };
            let lit = 0.08 + 0.92 * up.dot(SUN.normalize()).max(0.);
            // Higher land catches more light than the floors, so ranges read against the lowland.
            let rel = road_h.map_or(0., |h| ((a.z + b.z + c.z) / 3. - h) / 90.).clamp(-0.5, 1.);
            let bright = ((lit * 6.5 + rel * 2.2).clamp(0., 7.99)) as usize;
            g.tri_uv(a, b, c, [far_uv(look_index(look), bright, hz); 3], SKY);
        } else {
            let st = style(look);
            if nrm.z.abs() < 0.74 {
                let uv = |q: V3| match (st.wall_glow, road_h) {
                    // A face lit from its foot: height above the road, the glow in the first three
                    // units of `v` and the strata above, repeating without the glow.
                    (true, Some(h)) => { let t = (q.z - h + 2.5).max(0.) * 0.35; [(q.x + q.y) * 0.18, if t < 4. { t } else { 4. + (t - 4.) % 3.9 }] }
                    _ => [(q.x + q.y) * 0.18, q.z * 0.35],
                };
                g.tri_uv(a, b, c, [uv(a), uv(b), uv(c)], st.rock);
            } else {
                g.tri(a, b, c, st.ground);
            }
        }
    };
    // Skirts down every chunk edge, so neighbours of another detail never show a crack.
    let drop = 4. + cell * 0.7;
    for j in 0..ni {
        for i in 0..ni {
            let f = fac(i, j);
            let (mx, my) = (x0 + (i as f32 + 0.5) * cell, y0 + (j as f32 + 0.5) * cell);
            let near = idx.nearest(mx, my, 100.);
            let look = if lod < BAKED_FROM { near.map(|n| r.zone(n.s).0).unwrap_or(chunk_look) } else { chunk_look };
            let road_h = near.map(|n| n.h);
            // How finely each side is cut where it meets a neighbour: the coarser of the two.
            let edge = [fac(i - 1, j).min(f), fac(i + 1, j).min(f), fac(i, j - 1).min(f), fac(i, j + 1).min(f)];
            let q = 4 / f as i32;
            // The (f + 1) squared points of the cell: real samples inside and where the neighbour is as fine,
            // laid on the neighbour's straighter edge where it is not.
            let mut pts: Vec<(V3, bool)> = Vec::with_capacity((f + 1) * (f + 1));
            for b in 0..=f {
                for a in 0..=f {
                    let (kx, ky) = (4 * i + a as i32 * q, 4 * j + b as i32 * q);
                    // On a side shared with a coarser neighbour: laid between that side's own samples.
                    let side_of = |on: bool, e: usize, k: usize| if on && k % (f / e) != 0 { Some((k / (f / e) * (f / e), f / e)) } else { None };
                    let corner = (a == 0 || a == f) && (b == 0 || b == f);
                    let laid = if corner { None }
                        else if let Some(sp) = side_of(a == 0, edge[0], b) { Some((true, 0, sp)) }
                        else if let Some(sp) = side_of(a == f, edge[1], b) { Some((true, f, sp)) }
                        else if let Some(sp) = side_of(b == 0, edge[2], a) { Some((false, 0, sp)) }
                        else { side_of(b == f, edge[3], a).map(|sp| (false, f, sp)) };
                    let (z, wet) = match laid {
                        Some((vertical, fixed, (k0, span))) => {
                            let k = if vertical { b } else { a };
                            let pick = |smp: &mut Samples, kk: usize| if vertical { smp.at(4 * i + fixed as i32 * q, 4 * j + kk as i32 * q) } else { smp.at(4 * i + kk as i32 * q, 4 * j + fixed as i32 * q) };
                            let ((z0, w0), (z1, w1)) = (pick(&mut smp, k0), pick(&mut smp, k0 + span));
                            (z0 + (z1 - z0) * (k - k0) as f32 / span as f32, w0 && w1)
                        }
                        None => smp.at(kx, ky),
                    };
                    pts.push((v3(x0 + kx as f32 * cell * 0.25, y0 + ky as f32 * cell * 0.25, z), wet));
                }
            }
            let pt = |a: usize, b: usize| pts[b * (f + 1) + a];
            for b in 0..f {
                for a in 0..f {
                    let (p00, p10, p11, p01) = (pt(a, b), pt(a + 1, b), pt(a + 1, b + 1), pt(a, b + 1));
                    let parity = (i as usize * f + a + j as usize * f + b) % 2;
                    let (t0, t1) = if parity == 0 { ([p00, p10, p11], [p00, p11, p01]) } else { ([p00, p10, p01], [p10, p11, p01]) };
                    for t in [t0, t1] { paint(&mut g, t[0].0, t[1].0, t[2].0, look, road_h, t.iter().all(|v| v.1)); }
                }
            }
            // The chunk's own edge, along this cell's side of it.
            let skirt = |g: &mut Soup, a: V3, b: V3| {
                let (da, db) = (a - v3(0., 0., drop), b - v3(0., 0., drop));
                paint(g, a, b, db, chunk_look, None, false);
                paint(g, a, db, da, chunk_look, None, false);
            };
            for k in 0..f {
                // The farthest land is far enough that a crack between two of its details is haze.
                if lod >= 4 { break; }
                if i == 0 { skirt(&mut g, pt(0, k).0, pt(0, k + 1).0); }
                if i == ni - 1 { skirt(&mut g, pt(f, k).0, pt(f, k + 1).0); }
                if j == 0 { skirt(&mut g, pt(k, 0).0, pt(k + 1, 0).0); }
                if j == ni - 1 { skirt(&mut g, pt(k, f).0, pt(k + 1, f).0); }
            }
        }
    }
    // The liquid goes last, so a frame can find it from the end and move only that.
    let mut t = g.t;
    t.sort_by_key(|q| is_liquid(q));
    t
}

/// Whether a triangle is a sheet of lava or water, near (a painted material) or far (a flat light).
pub fn is_liquid(t: &Tri) -> bool {
    t.mat == LAVA || t.mat == WATER || (t.mat == SKY && (t.uv[0] == special_uv(3) || t.uv[0] == special_uv(10)))
}

/// Sets the lakes in the tail of `tris` moving: the near ones flow across their painted surface
/// and pulse where a wave of heat passes; the far ones step through four lights as it does.
pub fn flow(tris: &mut [Tri], ticks: u32) {
    let t = ticks as f32;
    let mut i = tris.len();
    while i > 0 && is_liquid(&tris[i - 1]) { i -= 1; }
    for q in &mut tris[i..] { animate(q, t); }
}

/// The same for painted lava and water anywhere in `tris`: the channels beside the road, the
/// falls of a cliff.
pub fn flow_all(tris: &mut [Tri], ticks: u32) {
    let t = ticks as f32;
    for q in tris { if q.mat == LAVA || q.mat == WATER { animate(q, t); } }
}

fn animate(q: &mut Tri, t: f32) {
    let c = (q.v[0] + q.v[1] + q.v[2]) * (1. / 3.);
    let lava = q.mat == LAVA || (q.mat == SKY && q.uv[0] == special_uv(3));
    // A sheet that stands up is a fall: its surface runs down it.
    let fall = q.normal.z.abs() < 0.5;
    if lava {
        // Waves of heat roll over the lake, two crossing, and the crust drifts.
        let w = 0.5 + 0.25 * ((c.x * 0.045 + c.y * 0.03 - t * 0.09).sin() + (c.x * -0.02 + c.y * 0.05 + t * 0.06).sin());
        if q.mat == LAVA {
            let (du, dv) = if fall { (0., t * 0.03) } else { (t * 0.0105, t * 0.0042) };
            for uv in &mut q.uv { uv[0] += du; uv[1] += dv; }
            if w > 0.74 { q.mat = GLOW; }
        } else {
            let kind = if w < 0.3 { 17 } else if w < 0.52 { 15 } else if w < 0.74 { 3 } else { 16 };
            q.uv = [special_uv(kind); 3];
        }
    } else if q.mat == WATER {
        let (du, dv) = if fall { (0., t * 0.04) } else { (t * 0.0021, t * -0.0013) };
        for uv in &mut q.uv { uv[0] += du; uv[1] += dv; }
    }
}
