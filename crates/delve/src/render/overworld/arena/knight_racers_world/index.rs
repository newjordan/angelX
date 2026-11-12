//! Where is the road? A grid over the centreline (the forks' too), so any world point can ask
//! its nearest stretch of road: for the land that must sit below the road,
//! the walls that must hug it, and the stand-in land of today's circuit.
use super::route::Route;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const SPACING: f32 = 3.0;
const CELL: f32 = 24.0;

#[derive(Clone, Copy, Debug)]
pub struct Sample { pub x: f32, pub y: f32, pub s: f32, pub h: f32, pub wl: f32, pub wr: f32, pub th: f32 }

/// The road nearest a world point.
#[derive(Clone, Copy, Debug)]
pub struct Near {
    pub s: f32,
    pub dist: f32,
    /// Road height there.
    pub h: f32,
    /// Half-width of the road on the point's own side.
    pub edge: f32,
}

pub struct RoadIndex {
    samples: Vec<Sample>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl RoadIndex {
    pub fn build(r: &dyn Route) -> RoadIndex {
        let lap = r.lap();
        let n = (lap / SPACING).ceil() as usize;
        let mut samples = Vec::with_capacity(n);
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for k in 0..n {
            let s = k as f32 * lap / n as f32;
            let (x, y, th) = r.pose(s);
            let (wl, wr) = r.width(s);
            cells.entry(Self::cell(x, y)).or_default().push(k as u32);
            samples.push(Sample { x, y, s, h: r.height(s), wl, wr, th });
        }
        // The forks too: the land must lie under their roads as it lies under the main one.
        for (i, f) in r.forks().iter().enumerate() {
            let n = (((f.s1 - f.s0) / SPACING).ceil() as usize).max(2);
            for k in 0..=n {
                let s = f.s0 + (f.s1 - f.s0) * k as f32 / n as f32;
                let fr = r.frame(i as u8 + 1, s);
                cells.entry(Self::cell(fr.x, fr.y)).or_default().push(samples.len() as u32);
                samples.push(Sample { x: fr.x, y: fr.y, s, h: fr.h, wl: fr.wl, wr: fr.wr, th: fr.th });
            }
        }
        RoadIndex { samples, cells }
    }

    fn cell(x: f32, y: f32) -> (i32, i32) { ((x / CELL).floor() as i32, (y / CELL).floor() as i32) }

    /// The nearest road within `reach` metres (rounded up to whole cells), if any.
    pub fn nearest(&self, x: f32, y: f32, reach: f32) -> Option<Near> {
        let (cx, cy) = Self::cell(x, y);
        let rings = (reach / CELL).ceil() as i32;
        let mut best: Option<(f32, &Sample)> = None;
        for dx in -rings..=rings {
            for dy in -rings..=rings {
                let Some(ids) = self.cells.get(&(cx + dx, cy + dy)) else { continue };
                for &i in ids {
                    let p = &self.samples[i as usize];
                    let d2 = (p.x - x) * (p.x - x) + (p.y - y) * (p.y - y);
                    if best.is_none_or(|(b, _)| d2 < b) { best = Some((d2, p)); }
                }
            }
        }
        let (d2, p) = best?;
        let (sn, cs) = p.th.sin_cos();
        let (ox, oy) = (x - p.x, y - p.y);
        let (along, lat) = (ox * cs + oy * sn, -ox * sn + oy * cs);
        Some(Near { s: p.s + along.clamp(-SPACING, SPACING), dist: d2.sqrt(), h: p.h, edge: if lat >= 0. { p.wl } else { p.wr } })
    }
}

/// The index of `r`, built once per course.
pub fn of(r: &dyn Route) -> &'static RoadIndex {
    static ALL: OnceLock<Mutex<HashMap<u32, &'static RoadIndex>>> = OnceLock::new();
    let mut all = ALL.get_or_init(Default::default).lock().unwrap();
    all.entry(r.key()).or_insert_with(|| Box::leak(Box::new(RoadIndex::build(r))))
}
