//! The riders: a knight in the saddle of a barded destrier, drawn from
//! shapes (`destrier`) with a lance that dips and rises, a pennon and a
//! plume that stream in the wind of the charge, a shield that swings high
//! or low; the knight flung from the saddle (turning as he flies); and the
//! helm that rolls away.
//!
//! A rider is built once per pose and kept (`mounted`): the poses are few
//! (a stride, five heights of the lance, three of the shield), so a frame
//! only stamps.

use super::super::super::ink::{Img, bayer, ink};
use super::{Colours, Coat, destrier};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The canvas a mounted rider is built on: the destrier's 52 by 46 at
/// `(0, HORSE_Y)`, room above it for a plume and a high lance, and to the
/// right of it for the lance's reach.
pub(super) const W: i32 = 66;
pub(super) const H: i32 = 56;
pub(super) const HORSE_Y: i32 = 10;
/// Where the horse's body stands in the canvas (its middle), and where its
/// hooves meet the ground.
pub(super) const BODY_X: i32 = 26;
pub(super) const HOOF_Y: i32 = 52;

/// How a rider sits and what he carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Pose {
    /// The gallop's frame, or none standing.
    pub gait: Option<u32>,
    /// The lance's height: 0 low, 4 high (the dip passes through the rest).
    pub aim: u8,
    /// The shield: 0 down over the chest, 2 up by the helm.
    pub guard: u8,
    /// The lance is a stump.
    pub broken: bool,
    /// The laurel on the helm.
    pub crowned: bool,
    /// No rider in the saddle.
    pub riderless: bool,
    /// The far lane: the lance reaches down across the tilt.
    pub far: bool,
    /// The wind's phase for pennon and plume, 0 to 3.
    pub wind: u8,
    /// The lance raised in salute.
    pub salute: bool,
    /// The knight's lance is couched: it is set, a glint on its tip.
    pub couched: bool,
}

/// A rider built once: facing east and west, and the dithered, dimmed
/// echoes of him that a fast charge leaves behind.
pub(super) struct Built {
    pub east: Img,
    pub west: Img,
    /// Echoes at three strengths, facing east and west.
    pub ghosts: [[Img; 3]; 2],
}

impl Built {
    fn of(im: Img) -> Built {
        let ghost = |keep: f32, flip: bool| {
            let src = if flip { im.flip_h() } else { im.clone() };
            let mut g = Img::new(src.w, src.h);
            for y in 0..src.h {
                for x in 0..src.w {
                    if let Some(c) = src.get(x, y)
                        && c != super::super::super::ink::BLACK
                        && bayer(x, y) < keep
                    {
                        g.set(x, y, super::super::super::ink::shade(c, 0.5));
                    }
                }
            }
            g
        };
        Built {
            west: im.flip_h(),
            ghosts: [
                [ghost(0.5, false), ghost(0.3, false), ghost(0.16, false)],
                [ghost(0.5, true), ghost(0.3, true), ghost(0.16, true)],
            ],
            east: im,
        }
    }

    pub fn facing(&self, east: bool) -> &Img {
        if east { &self.east } else { &self.west }
    }
}

fn cache() -> &'static Mutex<HashMap<String, Arc<Built>>> {
    static C: OnceLock<Mutex<HashMap<String, Arc<Built>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The rider on his horse, built or kept.
pub(super) fn mounted(coat: Coat, barding: (char, char), col: Colours, pose: Pose) -> Arc<Built> {
    let class = col.class.map_or(0, |(_, seat)| seat);
    let key = format!(
        "{}{}{}{}{}{}{class}{:?}",
        coat.hide, coat.dark, coat.mane, barding.0, barding.1, col.plume, pose
    );
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(b) = c.get(&key) {
        return Arc::clone(b);
    }
    let built = Arc::new(Built::of(build(coat, barding, col, pose)));
    if c.len() > 700 {
        c.clear();
    }
    c.insert(key, Arc::clone(&built));
    built
}

fn build(coat: Coat, barding: (char, char), col: Colours, pose: Pose) -> Img {
    let mut out = Img::new(W, H);
    let horse = destrier(coat, pose.gait, Some(barding));
    out.stamp(&horse, 0, HORSE_Y);
    if pose.riderless {
        // The stirrups swing empty and the reins trail.
        let by = 29 + HORSE_Y;
        out.line(24, by - 12, 27, by - 6, 'v');
        out.line(36, by - 14, 41, by - 10, 'o');
        return out;
    }
    let bob = pose.gait.map_or(0, |f| [0, -1, 0, 1][(f % 4) as usize]);
    let (sx, sy) = (21, 29 + HORSE_Y + bob - 13);
    let mut layer = Img::new(W, H);
    knight(&mut layer, (sx, sy), col, pose);
    layer.outline_outside('k');
    out.stamp(&layer, 0, 0);
    out
}

/// The lance's tip, relative to the saddle, for a height 0 to 4.
fn tip_for(pose: Pose, reach: i32) -> (i32, i32) {
    let a = i32::from(pose.aim.min(4));
    if pose.salute {
        return (reach / 2, -26);
    }
    if pose.far {
        // Pointing down across the tilt at the near rider.
        (reach, 8 - a * 3)
    } else {
        // Pointing up across it at the far rider.
        (reach, -6 - a * 3)
    }
}

/// The knight: helm and plume, tabard, the arm that holds the lance, the
/// lance with its vamplate, coronel and pennon, and the shield before him.
fn knight(im: &mut Img, (sx, sy): (i32, i32), col: Colours, pose: Pose) {
    let wave = i32::from(pose.wind % 4);
    // Torso.
    for y in sy - 7..=sy {
        for x in sx - 2..sx + 3 {
            im.put(x, y, if (x + y) % 4 == 0 { col.field } else { col.tabard });
        }
    }
    // The helm in the hero's steel: lit along its crown, navy at its back.
    for y in sy - 13..sy - 7 {
        for x in sx - 2..sx + 4 {
            let ink = if y == sy - 13 {
                'W'
            } else if x == sx - 2 {
                'u'
            } else if y == sy - 8 {
                'U'
            } else {
                'V'
            };
            im.put(x, y, ink);
        }
    }
    // The visor's slit and breaths.
    im.put(sx + 1, sy - 10, 's');
    im.put(sx + 2, sy - 10, 's');
    im.put(sx + 3, sy - 10, 's');
    im.put(sx + 3, sy - 9, 'k');
    if !pose.crowned {
        // The plume streams back in the wind.
        let tail = [(-3, -14), (-4, -14), (-5, -13), (-6, -13)];
        for (i, (dx, dy)) in tail.iter().enumerate() {
            let lift = if (i as i32 + wave) % 2 == 0 { 0 } else { 1 };
            im.put(sx + dx, sy + dy + lift, col.plume);
            im.put(sx + dx, sy + dy + 1 + lift, col.plume);
        }
        im.put(sx - 1, sy - 14, col.plume);
        im.put(sx, sy - 14, col.plume);
        im.put(sx - 2, sy - 15, col.plume);
    }
    if let Some((look, seat)) = col.class {
        super::super::class_looks::dress_rider(im, (sx, sy), look, seat);
    }
    if pose.crowned {
        // The winner's laurel crown on the helm.
        for (x, ink) in (sx - 2..sx + 3).zip(['5', '4', '5', '4', '5']) {
            im.put(x, sy - 14, ink);
        }
        for x in [sx - 2, sx, sx + 2] {
            im.put(x, sy - 15, '6');
        }
    }
    // The leg in the stirrup.
    im.line(sx, sy + 1, sx + 1, sy + 6, 'v');
    // The lance under the arm, out past the horse's head: a line of timber
    // with the vamplate at the grip, a steel coronel and a pennon.
    let reach = if pose.broken { 10 } else { 30 };
    let grip = (sx - 5, sy - 1);
    let (tx, ty) = tip_for(pose, reach);
    let tip = (sx + tx, sy + ty);
    for k in 0..2 {
        im.line(grip.0, grip.1 + k, tip.0, tip.1 + k, 'T');
    }
    // The arm and the vamplate.
    let (ax, ay) = (sx + 4, sy - 2 + (tip.1 - sy + 6) / 8);
    im.line(sx + 1, sy - 4, ax, ay, 'v');
    im.rect(ax - 1, ay - 1, 3, 3, 'W');
    if pose.broken {
        im.put(tip.0, tip.1 - 1, 'o');
        im.put(tip.0 + 1, tip.1 + 1, 'o');
        im.put(tip.0 + 2, tip.1 + 2, 'o');
    } else {
        im.put(tip.0, tip.1, 'W');
        im.put(tip.0 + 1, tip.1, 'W');
        im.put(tip.0 + 2, tip.1, 'H');
        if pose.couched {
            // Set: a glint on the coronel.
            im.put(tip.0 + 3, tip.1, 'w');
            im.put(tip.0 + 2, tip.1 - 1, 'w');
            im.put(tip.0 + 2, tip.1 + 1, 'w');
        }
        // The pennon, a swallowtail in the plume's colour, rippling.
        let px = tip.0 - 9;
        let py = grip.1 + (tip.1 - grip.1) * (px - grip.0) / (tip.0 - grip.0).max(1) + 1;
        for k in 0..6 {
            let ripple = [0, 1, 0, -1][((k + wave) % 4) as usize];
            let len = 6 - k / 2;
            for j in 0..len {
                if k < 2 || j < len - (k % 2) {
                    im.put(px - j, py + 1 + k / 2 + ripple, if k == 1 { col.field } else { col.plume });
                }
            }
        }
    }
    // The shield before him, rimmed in steel: by the helm, at his chest or
    // between, where he guards.
    let top = match pose.guard {
        2 => sy - 13,
        1 => sy - 9,
        _ => sy - 5,
    };
    for y in top..top + 9 {
        for x in sx + 2..sx + 9 {
            let point = y >= top + 7 && (x == sx + 2 || x == sx + 8);
            if point {
                continue;
            }
            let rim = y == top || x == sx + 2 || x == sx + 8 || y == top + 8;
            let charge = x == sx + 5 || y == top + 4;
            im.put(
                x,
                y,
                if rim {
                    if y == top { 'W' } else { 'v' }
                } else if charge {
                    col.field
                } else {
                    col.tabard
                },
            );
        }
    }
}

/// `im` turned `deg` degrees about its middle, nearest neighbour, on a
/// canvas big enough for any turn.
pub(super) fn turned(im: &Img, deg: f32) -> Img {
    let r = ((im.w * im.w + im.h * im.h) as f32).sqrt().ceil() as i32;
    let side = r + 1;
    let mut out = Img::new(side, side);
    let (s, c) = (deg.to_radians().sin(), deg.to_radians().cos());
    let (cx, cy) = ((im.w as f32 - 1.0) / 2.0, (im.h as f32 - 1.0) / 2.0);
    let o = (side as f32 - 1.0) / 2.0;
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 - o, y as f32 - o);
            // Back to the source: the inverse turn.
            let (sx, sy) = (dx * c + dy * s + cx, -dx * s + dy * c + cy);
            if let Some(p) = im.get(sx.round() as i32, sy.round() as i32) {
                out.set(x, y, p);
            }
        }
    }
    out
}

/// A knight flung from the saddle, facing east: helm (or a bare head once it
/// has come off), tabard, arms thrown out, legs kicking, the shield on his
/// arm. 16 by 24.
pub(super) fn flung(col: Colours, helm: bool, kick: bool) -> Img {
    let mut im = Img::new(16, 24);
    // Head.
    if helm {
        im.rect(5, 0, 7, 7, 'V');
        im.line(5, 0, 11, 0, 'W');
        im.line(11, 1, 11, 6, 'u');
        im.line(7, 3, 10, 3, 's');
        im.line(6, 0, 4, -1, col.plume);
    } else {
        im.rect(6, 1, 5, 6, 'O');
        im.line(6, 1, 10, 1, 'r');
        im.put(7, 3, 'k');
        im.put(9, 3, 'k');
        im.line(7, 6, 9, 6, 'p');
    }
    // Torso in the tabard, and the gorget.
    for y in 7..16 {
        for x in 4..12 {
            im.put(x, y, if (x + y) % 4 == 0 { col.field } else { col.tabard });
        }
    }
    im.line(4, 7, 11, 7, 'v');
    // Arms flung out, the shield still on one.
    im.line(3, 8, 0, 3, 'v');
    im.line(12, 8, 15, 4, 'v');
    im.rect(14, 1, 2, 3, 'W');
    // Legs.
    if kick {
        im.line(5, 16, 1, 19, 'v');
        im.line(10, 16, 15, 19, 'v');
        im.line(1, 19, 0, 22, 'u');
        im.line(15, 19, 15, 22, 'u');
    } else {
        im.line(5, 16, 3, 22, 'v');
        im.line(10, 16, 12, 22, 'v');
        im.line(3, 22, 1, 23, 'u');
        im.line(12, 22, 14, 23, 'u');
    }
    im.outline_outside('k');
    im
}

/// The helm, loose: 5 by 5, its slit turning as it rolls.
pub(super) fn helm_img(turn: u32) -> Img {
    let mut im = Img::new(5, 5);
    im.rect(0, 0, 5, 5, 'V');
    im.line(0, 0, 4, 0, 'W');
    im.line(4, 1, 4, 4, 'u');
    let slit = [(2, 1), (3, 2), (2, 3), (1, 2)][(turn % 4) as usize];
    im.put(slit.0, slit.1, 's');
    im.put(slit.0 + i32::from(turn % 2 == 0), slit.1, 's');
    im.outline_outside('k');
    im
}

/// The stands' red of a hurt rider: a sprite washed in one light ink.
pub(super) fn tint(im: &Img, c: char) -> Img {
    let mut out = im.clone();
    let Some(rgb) = ink(c) else { return out };
    for y in 0..out.h {
        for x in 0..out.w {
            if out.get(x, y).is_some_and(|p| p != super::super::super::ink::BLACK) {
                out.set(x, y, rgb);
            }
        }
    }
    out
}
