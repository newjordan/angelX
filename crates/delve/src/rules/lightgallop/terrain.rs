//! The road as curves, as the picture and the save check read them: every
//! surface a rider can stand on is a *path*, a polyline with an arc length,
//! and a rider on a path is a distance `s` along it. The road is built, and
//! ridden, in `cockpit/live/lightgallop-rules`; this is the shape it hands
//! over (`level.rs`).
//!
//! Everything is integer. Points are in 1/256 pixel; tangents are 12-bit
//! unit vectors that blend smoothly between segments, so a path made of short
//! straight pieces draws like a curve.
use super::math::{ONE, Q};
use serde::Deserialize;

/// What a path is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Kind {
    /// The road: a rider can land on it from above.
    Road,
    /// A thin rail high above the road: a rider can land on it from above.
    Rail,
    /// A loop's curl: reached only by running into it, never by landing.
    Ring,
}

/// Where a path hands a rider on: another path, at a distance along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Link {
    pub path: usize,
    pub s: i32,
}

/// A place along a path that sends a rider onto another path when crossed
/// in `dir` (+1 forward, -1 back) at speed: the mouth of a loop, or a fork
/// that only a fast rider is carried through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Portal {
    pub s: i32,
    pub dir: i32,
    /// The slowest ground speed that is carried through (0: any).
    pub min_speed: i32,
    pub to: Link,
}

/// One surface.
#[derive(Clone, Debug, Deserialize)]
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

/// Every surface of the road.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Terrain {
    pub paths: Vec<Path>,
}
