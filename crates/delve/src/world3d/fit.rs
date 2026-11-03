//! Footprints of a set's shapes on a room's tiles: whether two rooms are
//! laid alike, and what of a box is left off cleared ground. The kingdom's
//! lots cut and stand their builds so (`lots`), a lit room knows a place
//! laid as its item (`rooms_lit`).

use std::fmt::Write as _;

use angel_stage::set::{Kind, Shape};

use super::blockout::{self, Solid};
use crate::rules::Room;

/// Whether two rooms are laid alike: their size and every tile.
pub fn same_tiles(a: &Room, b: &Room) -> bool {
    (a.cols, a.rows) == (b.cols, b.rows)
        && (0..a.rows as i32).all(|r| (0..a.cols as i32).all(|c| a.tile(c, r) == b.tile(c, r)))
}

/// How far two footprints must overlap to be said to: a quarter pixel.
const EPS: f32 = 0.25;

pub(super) fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] - EPS && b[0] < a[2] - EPS && a[1] < b[3] - EPS && b[1] < a[3] - EPS
}

pub(super) fn clip(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let c = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
    (c[2] - c[0] > EPS && c[3] - c[1] > EPS).then_some(c)
}

/// What is left of footprint `a` off `b`: up to four rectangles.
pub(super) fn subtract(a: [f32; 4], b: [f32; 4]) -> Vec<[f32; 4]> {
    let Some(cut) = clip(a, b) else {
        return vec![a];
    };
    let mut out = Vec::new();
    let mut keep = |r: [f32; 4]| {
        if r[2] - r[0] > EPS && r[3] - r[1] > EPS {
            out.push(r);
        }
    };
    keep([a[0], a[1], cut[0], a[3]]);
    keep([cut[2], a[1], a[2], a[3]]);
    keep([cut[0], a[1], cut[2], cut[1]]);
    keep([cut[0], cut[3], cut[2], a[3]]);
    out
}

/// A set shape's footprint, [x0, z0, x1, z1] (a turned one's bounding square).
pub(super) fn footprint(s: &Shape) -> [f32; 4] {
    let (hx, hz) = (s.size[0] / 2.0, s.size[2] / 2.0);
    let (sin, cos) = s.turn.to_radians().sin_cos();
    let (hx, hz) = (cos.abs() * hx + sin.abs() * hz, sin.abs() * hx + cos.abs() * hz);
    [s.at[0] - hx, s.at[2] - hz, s.at[0] + hx, s.at[2] + hz]
}

/// Whether a set shape is a box square to the room (its footprint is it).
pub(super) fn is_box(s: &Shape) -> bool {
    s.shape == "box" && s.turn.rem_euclid(90.0).min(90.0 - s.turn.rem_euclid(90.0)) < 1e-3
}

/// A box over footprint `[x0, z0, x1, z1]`, its bottom at `y`, `h` high, as a line of a set.
pub(super) fn box_line(kind: Kind, [x0, z0, x1, z1]: [f32; 4], y: f32, h: f32, id: Option<&str>) -> String {
    let mut line = blockout::line(&Solid::block(kind, [x0, z0, x1, z1], y, h));
    if let Some(id) = id {
        let _ = write!(line, " id {id}");
    }
    line
}

/// A generated solid as a line of a set, its id left off (the item's set
/// may have it).
pub(super) fn line_of(s: &Solid) -> String {
    let mut s = s.clone();
    s.id = None;
    s.todo = None;
    blockout::line(&s)
}

