//! Motion: bones, the parts on them, poses and clips.
//!
//! A bone is a joint in model space; it turns about that joint and shifts,
//! and its children go with it. A part hangs on a bone (or is held in a
//! socket on one), so a pose moves its volume, which is then lit and drawn
//! afresh: a turned arm is never a turned bitmap. A clip keys poses in
//! time; between keys each bone's turn and shift run along the key's ease.
//!
//! Sockets are the quick-rig contract's named points (`grip`, `off_hand`,
//! `head`, `back`, `feet`, `tip`; crates/delve/src/render/overworld/arena/
//! quick_rig.rs), where held gear and the game's equipment meet the body.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::grain::Grain;
use crate::num;
use crate::recipe::{BonePose, Clip, Recipe};

/// The sockets the game's quick-rig reads, every frame.
pub const QUICK_RIG_SOCKETS: [&str; 6] = ["grip", "off_hand", "head", "back", "feet", "tip"];

/// How motion runs from one key to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    /// Even.
    #[default]
    Linear,
    /// Slow out of the key, fast into the next.
    In,
    /// Fast out of the key, settling into the next.
    Out,
    /// Slow at both ends.
    Both,
    /// Past the next key and back: an overshoot.
    Back,
    /// Past it and back, and past it again, less each time.
    Spring,
}

impl Ease {
    pub const ALL: [Ease; 6] = [Ease::Linear, Ease::In, Ease::Out, Ease::Both, Ease::Back, Ease::Spring];

    pub fn name(self) -> &'static str {
        match self {
            Ease::Linear => "linear",
            Ease::In => "in",
            Ease::Out => "out",
            Ease::Both => "both",
            Ease::Back => "back",
            Ease::Spring => "spring",
        }
    }

    /// How far along (0 at the key, 1 at the next) at `s` of the way in time.
    pub fn at(self, s: f32) -> f32 {
        let s = s.clamp(0.0, 1.0);
        match self {
            Ease::Linear => s,
            Ease::In => s * s * s,
            Ease::Out => {
                let r = 1.0 - s;
                1.0 - r * r * r
            }
            Ease::Both => {
                if s < 0.5 {
                    4.0 * s * s * s
                } else {
                    let r = 2.0 - 2.0 * s;
                    1.0 - r * r * r / 2.0
                }
            }
            Ease::Back => {
                let (c1, r) = (1.70158f32, s - 1.0);
                1.0 + (c1 + 1.0) * r * r * r + c1 * r * r
            }
            Ease::Spring => {
                let f = |s: f32| 1.0 - num::exp(-6.0 * s) * num::sin_cos(5.0 * std::f32::consts::PI * s).1;
                // Lands exactly on the next key.
                f(s) + s * (1.0 - f(1.0))
            }
        }
    }
}

/// A rigid move in the picture's plane, a turn about z then a shift, and a
/// shift in depth. Turning is clockwise on screen (y is down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub cos: f32,
    pub sin: f32,
    pub tx: f32,
    pub ty: f32,
    pub tz: f32,
}

impl Xf {
    /// No move.
    pub const ID: Xf = Xf { cos: 1.0, sin: 0.0, tx: 0.0, ty: 0.0, tz: 0.0 };

    /// A shift.
    pub fn shift(dx: f32, dy: f32, dz: f32) -> Xf {
        Xf { tx: dx, ty: dy, tz: dz, ..Xf::ID }
    }

    /// A turn of `deg` degrees about `(px, py)`.
    pub fn turn_about(deg: f32, [px, py]: [f32; 2]) -> Xf {
        let (sin, cos) = num::sin_cos_deg(deg);
        Xf { cos, sin, tx: px - (cos * px - sin * py), ty: py - (sin * px + cos * py), tz: 0.0 }
    }

    /// This move after `inner`: `self.after(inner).apply(p) == self.apply(inner.apply(p))`.
    pub fn after(self, inner: Xf) -> Xf {
        let [tx, ty] = self.apply([inner.tx, inner.ty]);
        Xf {
            cos: self.cos * inner.cos - self.sin * inner.sin,
            sin: self.sin * inner.cos + self.cos * inner.sin,
            tx,
            ty,
            tz: self.tz + inner.tz,
        }
    }

    /// Where it moves a point.
    pub fn apply(self, [x, y]: [f32; 2]) -> [f32; 2] {
        [self.cos * x - self.sin * y + self.tx, self.sin * x + self.cos * y + self.ty]
    }

    /// Where a point came from.
    pub fn unapply(self, [x, y]: [f32; 2]) -> [f32; 2] {
        let (dx, dy) = (x - self.tx, y - self.ty);
        [self.cos * dx + self.sin * dy, -self.sin * dx + self.cos * dy]
    }

    /// A direction turned with it.
    pub fn turn(self, [x, y]: [f32; 2]) -> [f32; 2] {
        [self.cos * x - self.sin * y, self.sin * x + self.cos * y]
    }

    /// Whether it moves nothing (to within a hair).
    pub fn is_still(self) -> bool {
        (self.cos - 1.0).abs() < 1e-6 && self.sin.abs() < 1e-6 && self.tx.abs() < 1e-6 && self.ty.abs() < 1e-6 && self.tz.abs() < 1e-6
    }
}

/// Every bone's change from rest, by bone index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pose(pub Vec<BonePose>);

impl Pose {
    /// Every bone at rest.
    pub fn rest(recipe: &Recipe) -> Pose {
        Pose(vec![BonePose::default(); recipe.src.rig.bones.len()])
    }

    /// The pose a clip holds at frame `t` (fractions allowed).
    pub fn of_clip(recipe: &Recipe, clip: &Clip, t: f32) -> Pose {
        let keyed = |k: usize| {
            let mut pose = Pose::rest(recipe);
            for (b, p) in &clip.keys[k].pose {
                if let Some(i) = recipe.bone(b) {
                    pose.0[i] = *p;
                }
            }
            pose
        };
        let n = clip.keys.len();
        let first = clip.keys[0].t;
        let last = clip.keys[n - 1].t;
        let frames = clip.frames as f32;
        // The key before `t`, the one after it, and how far between in time.
        let (a, b, s) = if t <= first && !clip.looped || n == 1 {
            return keyed(0);
        } else if t >= last {
            if !clip.looped {
                return keyed(n - 1);
            }
            let span = frames - last + first;
            (n - 1, 0, if span > 0.0 { (t - last) / span } else { 0.0 })
        } else if t < first {
            let span = frames - last + first;
            (n - 1, 0, if span > 0.0 { (t + frames - last) / span } else { 0.0 })
        } else {
            let j = clip.keys.iter().rposition(|k| k.t <= t).unwrap_or(0);
            let span = clip.keys[j + 1].t - clip.keys[j].t;
            (j, j + 1, (t - clip.keys[j].t) / span)
        };
        let e = clip.keys[a].ease.at(s);
        let (pa, pb) = (keyed(a), keyed(b));
        let mix = |x: f32, y: f32| x + (y - x) * e;
        Pose(
            pa.0.iter()
                .zip(&pb.0)
                .map(|(x, y)| BonePose { turn: mix(x.turn, y.turn), dx: mix(x.dx, y.dx), dy: mix(x.dy, y.dy), dz: mix(x.dz, y.dz) })
                .collect(),
        )
    }

    /// This pose with every shift on a whole pixel of `grain`: a figure
    /// drawn at 16 px a tile bobs by a pixel or not at all, never by a
    /// fraction the vote would turn into shimmer.
    pub fn snapped(&self, grain: Grain) -> Pose {
        let g = grain.px();
        let snap = |v: f32| (v * g).round() / g;
        Pose(self.0.iter().map(|p| BonePose { dx: snap(p.dx), dy: snap(p.dy), ..*p }).collect())
    }

    /// Each bone's move from rest to this pose, parents first.
    pub fn world(&self, recipe: &Recipe) -> Vec<Xf> {
        let mut out: Vec<Xf> = Vec::with_capacity(self.0.len());
        for (i, bone) in recipe.src.rig.bones.iter().enumerate() {
            let p = self.0.get(i).copied().unwrap_or_default();
            let local = Xf::shift(p.dx, p.dy, p.dz).after(Xf::turn_about(p.turn, bone.at));
            let parent = recipe.parents[i].map_or(Xf::ID, |j| out[j]);
            out.push(parent.after(local));
        }
        out
    }
}

/// The move that takes a part from where it is written to where it is in
/// this pose. A part held in a socket is written about the socket, pointing
/// up.
pub fn part_xf(recipe: &Recipe, part: usize, world: &[Xf]) -> Xf {
    let r = &recipe.parts[part];
    if let Some(name) = &r.socket {
        return socket_xf(recipe, name, world).unwrap_or(Xf::ID);
    }
    r.bone.map_or(Xf::ID, |b| world[b])
}

/// The move from a socket's own space (its point at the origin, up the way
/// what it holds points) to the picture, in this pose.
pub fn socket_xf(recipe: &Recipe, name: &str, world: &[Xf]) -> Option<Xf> {
    let s = recipe.src.rig.sockets.get(name)?;
    let bone = s.bone.as_deref().and_then(|b| recipe.bone(b)).map_or(Xf::ID, |b| world[b]);
    let (sin, cos) = num::sin_cos_deg(s.turn);
    Some(bone.after(Xf { cos, sin, tx: s.at[0], ty: s.at[1], tz: 0.0 }))
}

/// Every socket's point in model space, in this pose.
pub fn sockets(recipe: &Recipe, world: &[Xf]) -> BTreeMap<String, [f32; 2]> {
    recipe
        .src
        .rig
        .sockets
        .keys()
        .filter_map(|name| socket_xf(recipe, name, world).map(|x| (name.clone(), [x.tx, x.ty])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_start_and_land_on_their_keys() {
        for e in Ease::ALL {
            assert!(e.at(0.0).abs() < 1e-5, "{}", e.name());
            assert!((e.at(1.0) - 1.0).abs() < 1e-5, "{}", e.name());
        }
        assert!(Ease::Back.at(0.8) > 1.0);
        assert!(Ease::In.at(0.5) < 0.5 && Ease::Out.at(0.5) > 0.5);
    }

    #[test]
    fn a_turn_about_a_joint_keeps_the_joint() {
        let x = Xf::turn_about(90.0, [2.0, 3.0]);
        let j = x.apply([2.0, 3.0]);
        assert!((j[0] - 2.0).abs() < 1e-5 && (j[1] - 3.0).abs() < 1e-5);
        // Clockwise on screen: right of the joint goes below it.
        let p = x.apply([3.0, 3.0]);
        assert!((p[0] - 2.0).abs() < 1e-5 && (p[1] - 4.0).abs() < 1e-5);
        let back = x.unapply(p);
        assert!((back[0] - 3.0).abs() < 1e-5 && (back[1] - 3.0).abs() < 1e-5);
        let two = Xf::shift(1.0, 0.0, 0.5).after(x);
        assert_eq!(two.apply([3.0, 3.0]), Xf::shift(1.0, 0.0, 0.5).apply(x.apply([3.0, 3.0])));
    }
}
