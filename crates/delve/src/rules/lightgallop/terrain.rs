//! The road as curves: every surface a rider can stand on is a *path*, a
//! polyline with an arc length, and a rider on a path is just a distance `s`
//! along it and a ground speed. That one idea carries the flat, the hill, the
//! half-pipe and the full loop: the pull of a slope is the tangent's slant,
//! the rider's lean is the tangent's angle, and a loop is a path that curls
//! back over itself.
//!
//! Everything is integer. Points are in 1/256 pixel; tangents are 12-bit
//! unit vectors that blend smoothly between segments, so a path made of short
//! straight pieces rides like a curve.
use super::math::{self, ONE, Q};

/// What a path is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The road: a rider can land on it from above.
    Road,
    /// A thin rail high above the road: a rider can land on it from above.
    Rail,
    /// A loop's curl: reached only by running into it, never by landing.
    Ring,
}

/// Where a path hands a rider on: another path, at a distance along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub path: usize,
    pub s: i32,
}

/// A place along a path that sends a rider onto another path when crossed
/// in `dir` (+1 forward, -1 back) at speed: the mouth of a loop, or a fork
/// that only a fast rider is carried through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Portal {
    pub s: i32,
    pub dir: i32,
    /// The slowest ground speed that is carried through (0: any).
    pub min_speed: i32,
    pub to: Link,
}

/// One surface.
#[derive(Clone, Debug)]
pub struct Path {
    pub kind: Kind,
    /// The vertices, in 1/256 pixel.
    pub pts: Vec<(i32, i32)>,
    /// Arc length at each vertex, in 1/256 pixel.
    pub cum: Vec<i32>,
    /// Each segment's direction, a 12-bit unit vector.
    pub dir: Vec<(i32, i32)>,
    /// Arc length at each segment's midpoint.
    mid: Vec<i32>,
    pub next: Option<Link>,
    pub prev: Option<Link>,
    pub portals: Vec<Portal>,
}

/// A point on a path with its tangent (12-bit, in the direction of travel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub x: i32,
    pub y: i32,
    pub tx: i32,
    pub ty: i32,
}

impl Path {
    pub fn len(&self) -> i32 {
        *self.cum.last().unwrap_or(&0)
    }
    fn segments(&self) -> usize {
        self.pts.len() - 1
    }
    fn seg_at(&self, s: i32) -> usize {
        self.cum
            .partition_point(|&c| c <= s)
            .saturating_sub(1)
            .min(self.segments() - 1)
    }
    /// The point at distance `s` (held to the path) and the blended tangent.
    pub fn sample(&self, s: i32) -> Sample {
        let s = s.clamp(0, self.len());
        let i = self.seg_at(s);
        let (a, d) = (self.pts[i], self.dir[i]);
        let t = i64::from(s - self.cum[i]);
        let x = a.0 + (i64::from(d.0) * t / i64::from(ONE)) as i32;
        let y = a.1 + (i64::from(d.1) * t / i64::from(ONE)) as i32;
        // Directions are knots at segment midpoints; blend between them.
        let n = self.segments();
        let (lo, hi, from, to) = if s <= self.mid[i] {
            if i == 0 {
                return Sample { x, y, tx: d.0, ty: d.1 };
            }
            (i - 1, i, self.mid[i - 1], self.mid[i])
        } else {
            if i + 1 >= n {
                return Sample { x, y, tx: d.0, ty: d.1 };
            }
            (i, i + 1, self.mid[i], self.mid[i + 1])
        };
        let span = i64::from((to - from).max(1));
        let w = i64::from(s - from);
        let (p, q) = (self.dir[lo], self.dir[hi]);
        let blend = |a: i32, b: i32| (i64::from(a) + (i64::from(b - a) * w / span)) as i32;
        Sample { x, y, tx: blend(p.0, q.0), ty: blend(p.1, q.1) }
    }
    /// The distance along the path of the point at pixel column `x_px`, for a
    /// path that runs left to right.
    pub fn s_at_x(&self, x_px: i32) -> i32 {
        let xq = i64::from(x_px) * i64::from(Q);
        let i = self
            .pts
            .partition_point(|p| i64::from(p.0) <= xq)
            .saturating_sub(1)
            .min(self.segments() - 1);
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        let dx = i64::from(b.0 - a.0).max(1);
        let seg = i64::from(self.cum[i + 1] - self.cum[i]);
        (i64::from(self.cum[i]) + (xq - i64::from(a.0)).clamp(0, dx) * seg / dx) as i32
    }
    /// The height of the surface at pixel column `x_px` (1/256 pixel), for a
    /// path that runs left to right.
    pub fn y_at_x(&self, x_px: i32) -> i32 {
        self.sample(self.s_at_x(x_px)).y
    }
    /// The leftmost and rightmost pixel columns the path covers.
    pub fn span_px(&self) -> (i32, i32) {
        let lo = self.pts.iter().map(|p| p.0).min().unwrap_or(0);
        let hi = self.pts.iter().map(|p| p.0).max().unwrap_or(0);
        (lo.div_euclid(Q), hi.div_euclid(Q))
    }
}

/// Where a falling body met the road.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub path: usize,
    pub s: i32,
    pub x: i32,
    pub y: i32,
}

/// Columns of this many pixels index the landable segments.
const BUCKET: i32 = 32;

#[derive(Clone, Debug, Default)]
pub struct Terrain {
    pub paths: Vec<Path>,
    /// For each column bucket, the (path, segment) pairs a body could land on.
    buckets: Vec<Vec<(u16, u16)>>,
}

impl Terrain {
    pub fn new(paths: Vec<Path>) -> Self {
        let mut t = Terrain { paths, buckets: Vec::new() };
        t.index();
        t
    }
    fn index(&mut self) {
        let width = self.paths.iter().map(|p| p.span_px().1).max().unwrap_or(0);
        self.buckets = vec![Vec::new(); (width / BUCKET + 2) as usize];
        for (pi, p) in self.paths.iter().enumerate() {
            if p.kind == Kind::Ring {
                continue;
            }
            for si in 0..p.segments() {
                let (a, b) = (p.pts[si], p.pts[si + 1]);
                if b.0 <= a.0 {
                    continue;
                }
                let (lo, hi) = (a.0.div_euclid(Q * BUCKET), b.0.div_euclid(Q * BUCKET));
                for k in lo.max(0)..=hi.min(self.buckets.len() as i32 - 1) {
                    self.buckets[k as usize].push((pi as u16, si as u16));
                }
            }
        }
    }
    /// The first road met moving from `a` to `b` (1/256 pixel) when `a` is on
    /// or above it and `b` is below it. A body already a hair below a road at
    /// `a` still lands on it, so rounding never lets one fall through.
    pub fn land(&self, a: (i32, i32), b: (i32, i32)) -> Option<Hit> {
        let (lo, hi) = (a.0.min(b.0) - Q, a.0.max(b.0) + Q);
        let (k0, k1) = (
            lo.div_euclid(Q * BUCKET).max(0),
            hi.div_euclid(Q * BUCKET).min(self.buckets.len() as i32 - 1),
        );
        let mut best: Option<(i64, usize, usize)> = None;
        for k in k0..=k1 {
            for &(pi, si) in &self.buckets[k as usize] {
                let (pi, si) = (pi as usize, si as usize);
                let p = &self.paths[pi];
                let (p0, p1) = (p.pts[si], p.pts[si + 1]);
                let (ex, ey) = (i64::from(p1.0 - p0.0), i64::from(p1.1 - p0.1));
                let side = |q: (i32, i32)| {
                    ex * i64::from(q.1 - p0.1) - ey * i64::from(q.0 - p0.0)
                };
                let (d0, d1) = (side(a), side(b));
                // A hair of slack, in the segment's own units: 1.5 px.
                let slack = isqrt_i64(ex * ex + ey * ey) * i64::from(Q) * 3 / 2;
                if d0 > slack || d1 <= 0 {
                    continue;
                }
                // How far along a->b the line is crossed (0 to 65536).
                let denom = d0 - d1;
                let t = if denom == 0 { 0 } else { (d0 * 65536 / denom).clamp(0, 65536) };
                let cx = i64::from(a.0) + i64::from(b.0 - a.0) * t / 65536;
                // The crossing must lie over the segment (a little leeway).
                if cx < i64::from(p0.0) - i64::from(Q) || cx > i64::from(p1.0) + i64::from(Q) {
                    continue;
                }
                if best.is_none_or(|(bt, bp, bs)| (t, pi, si) < (bt, bp, bs)) {
                    best = Some((t, pi, si));
                }
            }
        }
        let (_, pi, si) = best?;
        let p = &self.paths[pi];
        let (p0, d) = (p.pts[si], p.dir[si]);
        // Project the arrival point on the segment to find the distance.
        let along = (i64::from(b.0 - p0.0) * i64::from(d.0) + i64::from(b.1 - p0.1) * i64::from(d.1))
            / i64::from(ONE);
        let seg = i64::from(p.cum[si + 1] - p.cum[si]);
        let s = p.cum[si] + along.clamp(0, seg) as i32;
        let at = p.sample(s);
        Some(Hit { path: pi, s, x: at.x, y: at.y })
    }
}

fn isqrt_i64(n: i64) -> i64 {
    math::isqrt(n.max(0) as u64) as i64
}

/// A path being laid down: pixel coordinates in, 1/256 pixel kept.
pub struct Builder {
    kind: Kind,
    pts: Vec<(i32, i32)>,
}

/// The longest a straight run is left before it is cut, in pixels: short
/// pieces keep the blended tangent honest.
const CUT: i32 = 24;

impl Builder {
    pub fn new(kind: Kind, x: i32, y: i32) -> Self {
        Builder { kind, pts: vec![(x * Q, y * Q)] }
    }
    /// Where the path stands now, in pixels.
    pub fn here(&self) -> (i32, i32) {
        let p = self.pts[self.pts.len() - 1];
        (p.0 / Q, p.1 / Q)
    }
    fn push(&mut self, x: i32, y: i32) {
        if self.pts.last() != Some(&(x, y)) {
            self.pts.push((x, y));
        }
    }
    /// A straight run to (x, y), in pixels.
    pub fn line(&mut self, x: i32, y: i32) -> &mut Self {
        let (x0, y0) = self.pts[self.pts.len() - 1];
        let (x1, y1) = (x * Q, y * Q);
        let len = math::isqrt(((i64::from(x1 - x0)).pow(2) + (i64::from(y1 - y0)).pow(2)) as u64) as i32;
        let n = (len / (CUT * Q) + 1).max(1);
        for i in 1..=n {
            self.push(
                x0 + ((i64::from(x1 - x0) * i64::from(i)) / i64::from(n)) as i32,
                y0 + ((i64::from(y1 - y0) * i64::from(i)) / i64::from(n)) as i32,
            );
        }
        self
    }
    /// An S-curve to (x, y), flat at both ends: a hill's flank.
    pub fn ease(&mut self, x: i32, y: i32) -> &mut Self {
        let (x0, y0) = self.pts[self.pts.len() - 1];
        let (x1, y1) = (x * Q, y * Q);
        let n = ((x1 - x0).abs() / (8 * Q)).max(2);
        for i in 1..=n {
            let k = math::smooth((i * ONE) / n);
            self.push(
                x0 + ((i64::from(x1 - x0) * i64::from(i)) / i64::from(n)) as i32,
                y0 + ((i64::from(y1 - y0) * i64::from(k)) / i64::from(ONE)) as i32,
            );
        }
        self
    }
    /// An arc of the circle of radius `r` about (cx, cy), from turn `a0` to
    /// turn `a1`, drifting `drift` pixels to the right over its length (a
    /// loop's lean, so its mouth and its exit do not overlap).
    pub fn arc(&mut self, cx: i32, cy: i32, r: i32, a0: i32, a1: i32, drift: i32) -> &mut Self {
        let n = ((a1 - a0).abs() / 2).max(1);
        for i in 1..=n {
            let a = a0 + (a1 - a0) * i / n;
            let x = cx * Q
                + ((i64::from(r) * i64::from(Q) * i64::from(math::cos(a))) / i64::from(ONE)) as i32
                + (i64::from(drift) * i64::from(Q) * i64::from(i) / i64::from(n)) as i32;
            let y = cy * Q
                + ((i64::from(r) * i64::from(Q) * i64::from(math::sin(a))) / i64::from(ONE)) as i32;
            self.push(x, y);
        }
        self
    }
    pub fn build(self) -> Path {
        let n = self.pts.len();
        assert!(n >= 2, "a path needs two points");
        let mut cum = vec![0; n];
        let mut dir = Vec::with_capacity(n - 1);
        let mut mid = Vec::with_capacity(n - 1);
        for i in 0..n - 1 {
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let (dx, dy) = (i64::from(b.0 - a.0), i64::from(b.1 - a.1));
            let len = isqrt_i64(dx * dx + dy * dy) as i32;
            cum[i + 1] = cum[i] + len;
            dir.push(math::unit(dx, dy));
            mid.push(cum[i] + len / 2);
        }
        Path {
            kind: self.kind,
            pts: self.pts,
            cum,
            dir,
            mid,
            next: None,
            prev: None,
            portals: Vec::new(),
        }
    }
}

/// A loop of radius `r` standing on the road at pixel column `x`, whose
/// ground is at height `y`: the path of its curl, entered at the bottom and
/// left again `drift` pixels further on.
pub fn ring(x: i32, y: i32, r: i32, drift: i32) -> Path {
    let (cx, cy) = (x, y - r);
    let mut b = Builder::new(Kind::Ring, x, y);
    // From the bottom (turn 64), up the near wall, across the top, down the
    // far wall and home: a full circle turning the way the rider runs.
    b.arc(cx, cy, r, 64, 64 - 256, drift);
    b.build()
}

/// Hang a loop on a road: `ring` (from `ring()`) is entered from `road` at
/// the column its bottom starts on and leaves back onto the road at the
/// column it ends on. Returns the ring's index.
pub fn attach_ring(paths: &mut Vec<Path>, road: usize, mut ring: Path) -> usize {
    let at = paths.len();
    let (enter, leave) = (ring.pts[0].0 / Q, ring.pts[ring.pts.len() - 1].0 / Q);
    let (s_in, s_out) = (paths[road].s_at_x(enter), paths[road].s_at_x(leave));
    ring.prev = Some(Link { path: road, s: s_in });
    ring.next = Some(Link { path: road, s: s_out });
    paths[road].portals.push(Portal { s: s_in, dir: 1, min_speed: 0, to: Link { path: at, s: 0 } });
    paths[road].portals.push(Portal { s: s_out, dir: -1, min_speed: 0, to: Link { path: at, s: ring.len() } });
    paths.push(ring);
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(len: i32) -> Path {
        let mut b = Builder::new(Kind::Road, 0, 100);
        b.line(len, 100);
        b.build()
    }

    #[test]
    fn a_flat_path_runs_level_and_measures_its_length() {
        let p = flat(200);
        assert_eq!(p.len(), 200 * Q);
        let at = p.sample(50 * Q);
        assert_eq!((at.x, at.y, at.tx, at.ty), (50 * Q, 100 * Q, ONE, 0));
        assert_eq!(p.s_at_x(120), 120 * Q);
        assert_eq!(p.y_at_x(120), 100 * Q);
    }

    #[test]
    fn a_slope_leans_its_tangent_and_a_hill_is_flat_at_its_crest() {
        let mut b = Builder::new(Kind::Road, 0, 100);
        b.ease(200, 40).ease(400, 100);
        let p = b.build();
        let mid = p.sample(p.s_at_x(100));
        assert!(mid.ty < -1500, "climbing in the flank: {}", mid.ty);
        let crest = p.sample(p.s_at_x(200));
        assert!(crest.ty.abs() < 120, "level at the top: {}", crest.ty);
        assert_eq!(p.y_at_x(200), 40 * Q);
        let down = p.sample(p.s_at_x(300));
        assert!(down.ty > 1500, "falling on the far side: {}", down.ty);
    }

    #[test]
    fn a_ring_closes_on_itself_with_its_drift_and_turns_the_whole_way() {
        let r = ring(100, 300, 40, 16);
        let (first, last) = (r.pts[0], r.pts[r.pts.len() - 1]);
        assert_eq!(first, (100 * Q, 300 * Q));
        assert!((last.0 - 116 * Q).abs() <= 2 && (last.1 - 300 * Q).abs() <= 2, "{last:?}");
        // The circumference is about 2*pi*40 plus the drift's share.
        assert!((r.len() / Q - 251).abs() < 6, "{}", r.len() / Q);
        let at = |s: i32| r.sample(s);
        // Bottom, near wall, top, far wall.
        let q = r.len() / 4;
        assert!(at(0).tx > 3900 && at(0).ty.abs() < 200);
        assert!(at(q).tx.abs() < 600 && at(q).ty < -3800, "rising: {:?}", at(q));
        assert!(at(2 * q).tx < -3800 && at(2 * q).ty.abs() < 400, "upside down: {:?}", at(2 * q));
        assert!(at(3 * q).tx.abs() < 600 && at(3 * q).ty > 3800, "falling: {:?}", at(3 * q));
        let top = at(2 * q);
        assert!((top.y - 220 * Q).abs() < 2 * Q, "the top is a diameter up: {}", top.y / Q);
    }

    #[test]
    fn a_body_lands_from_above_and_rises_through_from_below() {
        let t = Terrain::new(vec![flat(400)]);
        let hit = t.land((100 * Q, 90 * Q), (104 * Q, 108 * Q)).unwrap();
        assert_eq!((hit.path, hit.y), (0, 100 * Q));
        assert!((hit.x - 104 * Q).abs() < Q, "it lands where it was going: {}", hit.x);
        assert!(t.land((100 * Q, 120 * Q), (104 * Q, 90 * Q)).is_none(), "a one-way road");
        assert!(t.land((100 * Q, 60 * Q), (104 * Q, 70 * Q)).is_none(), "still above");
    }

    #[test]
    fn a_body_a_hair_under_the_road_still_lands_and_a_ring_is_not_landable() {
        let t = Terrain::new(vec![flat(400), ring(200, 100, 40, 16)]);
        assert!(t.land((100 * Q, 100 * Q + 128), (100 * Q, 104 * Q)).is_some());
        // Through the ring's far wall from above is just air.
        assert!(t.land((240 * Q, 40 * Q), (240 * Q, 90 * Q)).is_none());
    }

    #[test]
    fn the_first_of_two_roads_is_the_one_landed_on() {
        let mut rail = Builder::new(Kind::Rail, 0, 60);
        rail.line(400, 60);
        let t = Terrain::new(vec![flat(400), rail.build()]);
        let hit = t.land((100 * Q, 40 * Q), (100 * Q, 120 * Q)).unwrap();
        assert_eq!(hit.path, 1, "the rail is met first on the way down");
        let hit = t.land((100 * Q, 80 * Q), (100 * Q, 120 * Q)).unwrap();
        assert_eq!(hit.path, 0, "below the rail, the road");
    }
}
