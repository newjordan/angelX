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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Kind {
    Road,
    Rail,
    Ring,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Link {
    pub path: usize,
    pub s: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Portal {
    pub s: i32,
    pub dir: i32,
    pub min_speed: i32,
    pub to: Link,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Path {
    pub kind: Kind,
    pub pts: Vec<(i32, i32)>,
    pub cum: Vec<i32>,
    pub dir: Vec<(i32, i32)>,
    mid: Vec<i32>,
    pub next: Option<Link>,
    pub prev: Option<Link>,
    pub portals: Vec<Portal>,
}

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
    pub fn sample(&self, s: i32) -> Sample {
        let s = s.clamp(0, self.len());
        let i = self.seg_at(s);
        let (a, d) = (self.pts[i], self.dir[i]);
        let t = i64::from(s - self.cum[i]);
        let x = a.0 + (i64::from(d.0) * t / i64::from(ONE)) as i32;
        let y = a.1 + (i64::from(d.1) * t / i64::from(ONE)) as i32;
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
    pub fn y_at_x(&self, x_px: i32) -> i32 {
        self.sample(self.s_at_x(x_px)).y
    }
    pub fn span_px(&self) -> (i32, i32) {
        let lo = self.pts.iter().map(|p| p.0).min().unwrap_or(0);
        let hi = self.pts.iter().map(|p| p.0).max().unwrap_or(0);
        (lo.div_euclid(Q), hi.div_euclid(Q))
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Terrain {
    pub paths: Vec<Path>,
}
