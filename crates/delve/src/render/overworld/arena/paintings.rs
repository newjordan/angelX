//! Wish paintings on the field (docs/DELVE.md, "Paintings"), and how a
//! card's sprites move.
//!
//! Where a sprite with a painting (`draw <slot> paint=<sha>`) is drawn on
//! the field (a shot, a worn piece, an orbit, a summoned creature, an
//! aura), a frame taken apart notes it ([`note`]): the painting's hash, the
//! frame the inks show, the square they fill, whether they are flipped,
//! how they are moving, and the inks themselves. A window that has the
//! picture draws it in that square at its own resolution and takes the
//! inks out from under it (`stage_frame`); a terminal, or a window without
//! the picture yet, keeps the inks. A painting that is pixel art
//! (`draw <slot> paint=<sha> pixel`) is cut with hard pixel edges
//! ([`cut_pixels`]): blown up whole pixels at a time, never averaged.
//!
//! A sprite's motion words (`move=`) set it moving the same way at every
//! level: [`Pose::of`] is where the words have it at a tick, the same on
//! every machine. Its inks are moved here ([`Pose::apply`]), in the
//! terminal too; its painting is moved as a layer ([`Pose::place`]), turned
//! by the layer's own angle.

use std::cell::RefCell;
use std::f32::consts::TAU;

use super::super::ink::Img;
use crate::rules::HZ;
use crate::rules::looks::{Move, Sprite};

/// A painting drawn in a sprite's place.
#[derive(Clone, Debug, PartialEq)]
pub struct Painting {
    /// The picture's sha256 in the host's paint store (`looks::paint_name`).
    pub sha: String,
    /// Its normal map's, when it names one (`draw <slot> normal=`).
    pub normal: Option<String>,
    /// Pixel art (`pixel`): cut by [`cut_pixels`], never smoothed.
    pub pixel: bool,
    /// Clothing fills its part independently of the PNG subject aspect.
    pub subject: bool,
    /// The frame showing, of `frames`.
    pub frame: usize,
    pub frames: usize,
    /// Mirrored, as the inks are (a shot flying left, a knight facing left).
    pub flip: bool,
    /// The square the inks fill, unmoved, `[x, y, w, h]`: in the room's
    /// pixels, but for a shot's, which is from the shot's own point.
    pub rect: [f32; 4],
    /// Drawn under the figure it is part of (an aura on the ground), not
    /// over it.
    pub under: bool,
    /// How it is moving now.
    pub pose: Pose,
    /// The inks it takes the place of, as they were painted (moved), with
    /// their top-left in the same pixels as `rect`, or a shot's mark's.
    pub inks: Img,
    pub at: (i32, i32),
}

/// The square `sprite`'s painting fills (`Sprite::ink_square`), `[x, y, w,
/// h]` in its frame's pixels drawn `scale` times their size, flipped or not.
pub fn square(sprite: &Sprite, flip: bool, scale: f32) -> Option<[f32; 4]> {
    let (x, y, side) = sprite.ink_square()?;
    let width = sprite
        .frames
        .iter()
        .flatten()
        .map(|r| r.chars().count())
        .max()
        .unwrap_or(0) as i32;
    // A flipped frame is mirrored across its own width.
    let x = if flip { width - x - side } else { x };
    let side = side as f32 * scale;
    Some([x as f32 * scale, y as f32 * scale, side, side])
}

/// Where `sprite` turns and stretches about in its frame drawn `scale`
/// times its size: its square's middle, or the middle of `size`.
pub fn pivot(sprite: &Sprite, flip: bool, scale: f32, size: (i32, i32)) -> (f32, f32) {
    square(sprite, flip, scale).map_or(
        (size.0 as f32 / 2.0, size.1 as f32 / 2.0),
        |[x, y, w, h]| (x + w / 2.0, y + h / 2.0),
    )
}

// ─── deep enough down, a wish's painting at every grant past its first ─────

/// The floor from which a window shows a wish's painting at a grant short
/// of its top (`Sprite::hd`) in its downgraded inks' place: the first two
/// floors climb in pixels, the third on is high definition.
pub const HD_FLOOR: u32 = 3;

/// Whether a window on floor `floor` shows `hd=` paintings.
pub fn hd_floor(floor: u32) -> bool {
    floor >= HD_FLOOR
}

std::thread_local! {
    static HD_HERE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `paint` with the deep-floor paintings shown (`on`) or not, as they
/// were after.
pub fn with_hd<T>(on: bool, paint: impl FnOnce() -> T) -> T {
    let before = HD_HERE.with(|h| h.replace(on));
    let out = paint();
    HD_HERE.with(|h| h.set(before));
    out
}

/// The painting `sprite` shows here, and whether it is its whole gift's
/// (`paint=`, its normal map with it) or its deep-floor one (`hd=`).
pub fn shown(sprite: &Sprite) -> Option<(&String, bool)> {
    if let Some(sha) = &sprite.paint {
        return Some((sha, true));
    }
    sprite
        .hd
        .as_ref()
        .filter(|_| HD_HERE.with(std::cell::Cell::get))
        .map(|sha| (sha, false))
}

impl Painting {
    /// The painting of `sprite` showing at `tick`, flipped or not, if it
    /// has one: its square placed with its frame at `origin` (the frame's
    /// top-left where it is drawn, unmoved), drawn at `scale` times its
    /// frame's size. The inks are left for the caller to fill.
    pub fn of(
        sprite: &Sprite,
        tick: u64,
        flip: bool,
        origin: (f32, f32),
        scale: f32,
        pose: Pose,
    ) -> Option<Painting> {
        let (sha, whole) = shown(sprite)?;
        let [x, y, w, h] = square(sprite, flip, scale)?;
        Some(Painting {
            sha: sha.clone(),
            normal: sprite.normal.clone().filter(|_| whole),
            pixel: sprite.pixel && whole,
            subject: false,
            frame: sprite.frame_index(tick),
            frames: sprite.frames.len(),
            flip,
            rect: [origin.0 + x, origin.1 + y, w, h],
            under: false,
            pose,
            inks: Img::new(0, 0),
            at: (0, 0),
        })
    }

    /// Moved by `(dx, dy)`.
    pub fn shifted(mut self, (dx, dy): (i32, i32)) -> Painting {
        self.rect[0] += dx as f32;
        self.rect[1] += dy as f32;
        self.at = (self.at.0 + dx, self.at.1 + dy);
        self
    }

    /// Mirrored across a picture `width` wide, as the picture it is in is.
    pub fn mirrored(mut self, width: i32) -> Painting {
        self.rect[0] = width as f32 - self.rect[0] - self.rect[2];
        self.at.0 = width - self.at.0 - self.inks.w;
        self.inks = self.inks.flip_h();
        self.flip = !self.flip;
        self.pose.dx = -self.pose.dx;
        self.pose.angle = -self.pose.angle;
        if let Some(wave) = &mut self.pose.wave {
            if wave.across {
                wave.head_far = !wave.head_far;
            } else {
                // A swing the other way.
                wave.at += std::f32::consts::PI;
            }
        }
        self
    }

    /// The square's middle, which the pose turns and stretches about.
    pub fn middle(&self) -> (f32, f32) {
        (
            self.rect[0] + self.rect[2] / 2.0,
            self.rect[1] + self.rect[3] / 2.0,
        )
    }

    /// Where its layer goes, moved, and its turn.
    pub fn placed(&self) -> ([f32; 4], f32) {
        self.pose.place(self.rect)
    }
}

/// Stamp `im`, a frame of `sprite` showing at `tick` (flipped or not), on
/// `cv` with its top-left at `at`, moving by the sprite's words (`phase`
/// and `heading` as `Pose::of`); and note its painting, if it has one and a
/// frame is noting them, in `cv`'s pixels. `under`: the painting lies under
/// the figure it is part of.
#[allow(clippy::too_many_arguments)]
pub(super) fn stamp(
    cv: &mut Img,
    im: &Img,
    sprite: &Sprite,
    tick: u64,
    flip: bool,
    at: (i32, i32),
    moving: (u32, (f32, f32)),
    under: bool,
) {
    stamp_scaled(cv, im, sprite, tick, flip, at, moving, under, 1.0);
}

/// As `stamp`, for a frame drawn `scale` times its size (`im` already is
/// that size): a worn piece fitted to a smaller or larger body than a
/// knight's (`minions`).
#[allow(clippy::too_many_arguments)]
pub(super) fn stamp_scaled(
    cv: &mut Img,
    im: &Img,
    sprite: &Sprite,
    tick: u64,
    flip: bool,
    at: (i32, i32),
    (phase, heading): (u32, (f32, f32)),
    under: bool,
    scale: f32,
) {
    let pose = Pose::of(&sprite.moves, tick, phase, heading);
    let (moved, (ox, oy)) = pose.apply(im, pivot(sprite, flip, scale, (im.w, im.h)));
    let to = (at.0 + ox, at.1 + oy);
    cv.stamp(&moved, to.0, to.1);
    if noting_now()
        && let Some(mut painting) =
            Painting::of(sprite, tick, flip, (at.0 as f32, at.1 as f32), scale, pose)
    {
        painting.under = under;
        painting.inks = moved;
        painting.at = to;
        note(painting);
    }
}

/// A fitted hand-slot weapon supplies its aim pose explicitly. Inks and
/// the high-resolution painting must use exactly the same rotation.
pub(super) fn stamp_posed(cv: &mut Img, im: &Img, sprite: &Sprite, tick: u64, at: (i32, i32), pose: Pose, under: bool) {
    let (moved, (ox, oy)) = pose.apply(im, pivot(sprite, false, 1.0, (im.w, im.h)));
    let to = (at.0 + ox, at.1 + oy);
    cv.stamp(&moved, to.0, to.1);
    if noting_now()
        && let Some(mut painting) = Painting::of(sprite, tick, false, (at.0 as f32, at.1 as f32), 1.0, pose)
    {
        painting.under = under;
        painting.inks = moved;
        painting.at = to;
        note(painting);
    }
}

// ─── a knight's worn paintings, waiting for the knight to stand ─────────────

std::thread_local! {
    static WORN: RefCell<Option<((i32, i32), Vec<Painting>)>> = const { RefCell::new(None) };
}

/// The paintings of what a knight wears, in the pixels of their picture
/// `size` big, noted when that picture is stood (`stood`).
pub(super) fn wearing(size: (i32, i32), paints: Vec<Painting>) {
    if !paints.is_empty() && noting_now() {
        WORN.with(|w| *w.borrow_mut() = Some((size, paints)));
    }
}

/// A picture `size` big stood with its top-left at `at`: what it wears
/// is noted where it stands, if it is the picture they were worn on.
pub(super) fn stood(size: (i32, i32), at: (i32, i32)) {
    let Some((worn, paints)) = WORN.with(|w| w.borrow_mut().take()) else {
        return;
    };
    if worn == size {
        for painting in paints {
            note(painting.shifted(at));
        }
    }
}

/// What a knight wears is not standing after all (the knight's picture
/// was changed past it).
pub(super) fn unworn() {
    WORN.with(|w| w.borrow_mut().take());
}

// ─── noting them while a frame is taken apart ───────────────────────────────

std::thread_local! {
    static NOTED: RefCell<Option<Vec<Painting>>> = const { RefCell::new(None) };
}

/// Note the paintings drawn while `paint` runs: what it returned, and the
/// paintings noted and not taken (`take`) on the way.
pub(super) fn noting<T>(paint: impl FnOnce() -> T) -> (T, Vec<Painting>) {
    let before = NOTED.with(|n| n.borrow_mut().replace(Vec::new()));
    let out = paint();
    let noted = NOTED
        .with(|n| std::mem::replace(&mut *n.borrow_mut(), before))
        .unwrap_or_default();
    (out, noted)
}

/// Whether a frame is noting its paintings.
pub(super) fn noting_now() -> bool {
    NOTED.with(|n| n.borrow().is_some())
}

/// A painting drawn, when a frame is noting them.
pub(super) fn note(painting: Painting) {
    NOTED.with(|n| {
        if let Some(noted) = n.borrow_mut().as_mut() {
            noted.push(painting);
        }
    });
}

/// The paintings noted so far, taken (a figure's, once it is painted).
pub(super) fn take() -> Vec<Painting> {
    NOTED.with(|n| {
        n.borrow_mut()
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    })
}

// ─── motion ─────────────────────────────────────────────────────────────────

/// How far a shot stretches along its flight, as a part of its length.
pub const STRETCH: f32 = 0.15;
/// How far a sprite bobs, as a part of its height.
pub const BOB: f32 = 0.06;
/// How far a sprite rocks either way, in radians.
pub const WOBBLE: f32 = 0.16;
/// How far a jiggle squashes, as a part of its size.
pub const JIGGLE: f32 = 0.10;
/// How far the trailing end of a wiggle swings, as a part of its length.
pub const WIGGLE: f32 = 0.12;
/// How far a pulse swells, as a part of its size.
pub const PULSE: f32 = 0.08;
/// How far a shake throws it, as a part of its size.
pub const SHAKE: f32 = 0.04;

/// Each motion's period, in seconds (a spin's, one whole turn).
const STRETCH_SECONDS: f32 = 0.45;
const BOB_SECONDS: f32 = 1.0;
const WOBBLE_SECONDS: f32 = 0.8;
const SPIN_SECONDS: f32 = 1.2;
const JIGGLE_SECONDS: f32 = 0.6;
const WIGGLE_SECONDS: f32 = 0.7;
const PULSE_SECONDS: f32 = 1.0;

/// Where a sprite's motion words have it at a moment: moved (in parts of
/// its side), stretched across and down, turned about its middle, and a
/// wave down its length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub dx: f32,
    pub dy: f32,
    pub sx: f32,
    pub sy: f32,
    pub angle: f32,
    pub wave: Option<Wave>,
}

/// A wiggle's wave down a sprite: where it is in its swing (radians), and
/// which way its length runs. The head stays put, the tail swings most.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    pub at: f32,
    /// Its length runs across (a shot flying sideways), else down.
    pub across: bool,
    /// The head is at the far end (right, or the foot), else the near.
    pub head_far: bool,
}

impl Pose {
    pub const STILL: Pose = Pose {
        dx: 0.0,
        dy: 0.0,
        sx: 1.0,
        sy: 1.0,
        angle: 0.0,
        wave: None,
    };

    /// Where `moves` have a sprite at `tick`. `phase` keeps one sprite out
    /// of step with its fellows (a volley's shots, two knights' pieces);
    /// `heading` is which way it flies (a shot's speed), zero for none.
    pub fn of(moves: &[Move], tick: u64, phase: u32, heading: (f32, f32)) -> Pose {
        let mut pose = Pose::STILL;
        if moves.is_empty() {
            return pose;
        }
        let t = tick as f32 / HZ as f32;
        let p = (phase % 4096) as f32 / 4096.0 * TAU;
        let wave = |seconds: f32| (t / seconds * TAU + p).sin();
        let (hx, hy) = heading;
        let flying = hx != 0.0 || hy != 0.0;
        let across = flying && hx.abs() >= hy.abs();
        for m in moves {
            match m {
                Move::Stretch => {
                    // Longer along the flight, thinner across it.
                    let s = STRETCH * wave(STRETCH_SECONDS);
                    if across {
                        pose.sx *= 1.0 + s;
                        pose.sy *= 1.0 - s / 2.0;
                    } else {
                        pose.sy *= 1.0 + s;
                        pose.sx *= 1.0 - s / 2.0;
                    }
                }
                Move::Bob => pose.dy -= BOB * (wave(BOB_SECONDS) + 1.0) / 2.0,
                Move::Wobble => pose.angle += WOBBLE * wave(WOBBLE_SECONDS),
                Move::Spin => pose.angle += ((t / SPIN_SECONDS + p / TAU) % 1.0) * TAU,
                Move::Jiggle => {
                    // Wide then tall, twice a bounce.
                    let s = JIGGLE * wave(JIGGLE_SECONDS / 2.0);
                    pose.sx *= 1.0 + s;
                    pose.sy *= 1.0 - s;
                }
                Move::Wiggle => {
                    pose.wave = Some(Wave {
                        at: t / WIGGLE_SECONDS * TAU + p,
                        across,
                        // A shot's head is where it flies; a worn tail hangs
                        // from the top.
                        head_far: if across { hx > 0.0 } else { flying && hy > 0.0 },
                    });
                }
                Move::Pulse => {
                    let s = 1.0 + PULSE * (wave(PULSE_SECONDS) + 1.0) / 2.0;
                    pose.sx *= s;
                    pose.sy *= s;
                }
                Move::Shake => {
                    // A new throw every tick, the same on every machine.
                    let h = (tick as u32 ^ phase.rotate_left(13)).wrapping_mul(0x9E37_79B1);
                    pose.dx += SHAKE * (((h >> 8) % 3) as f32 - 1.0);
                    pose.dy += SHAKE * (((h >> 20) % 3) as f32 - 1.0);
                }
            }
        }
        pose
    }

    pub fn is_still(&self) -> bool {
        *self == Pose::STILL
    }

    /// How far the wave throws a point `u` of the way from head to tail,
    /// as a part of the length, across the length.
    fn swing(wave: &Wave, u: f32) -> f32 {
        WIGGLE * u * (wave.at - u * TAU * 0.5).sin()
    }

    /// The share of the way from head to tail of the point `(x, y)` in a
    /// box `size` big.
    fn along(wave: &Wave, (x, y): (f32, f32), (w, h): (f32, f32)) -> f32 {
        let u = if wave.across {
            x / w.max(1.0)
        } else {
            y / h.max(1.0)
        };
        if wave.head_far { 1.0 - u } else { u }.clamp(0.0, 1.0)
    }

    /// `img` moved so, its pixels' own way (nearest), about `pivot` in its
    /// pixels: the moved picture and where its top-left lands from the
    /// unmoved one's.
    pub fn apply(&self, img: &Img, pivot: (f32, f32)) -> (Img, (i32, i32)) {
        if self.is_still() || img.w <= 0 || img.h <= 0 {
            return (img.clone(), (0, 0));
        }
        let size = (img.w as f32, img.h as f32);
        let side = size.0.max(size.1);
        let (cos, sin) = (self.angle.cos(), self.angle.sin());
        let (sx, sy) = (self.sx.max(0.05), self.sy.max(0.05));
        let (dx, dy) = (self.dx * side, self.dy * side);
        let forward = |(x, y): (f32, f32)| {
            let (ux, uy) = ((x - pivot.0) * sx, (y - pivot.1) * sy);
            (
                pivot.0 + dx + ux * cos - uy * sin,
                pivot.1 + dy + ux * sin + uy * cos,
            )
        };
        // The moved box, room for the wave's swing round it.
        let reach = if self.wave.is_some() {
            WIGGLE * side + 1.0
        } else {
            0.0
        };
        let corners = [(0.0, 0.0), (size.0, 0.0), (0.0, size.1), size].map(forward);
        let x0 = (corners.iter().map(|c| c.0).fold(f32::MAX, f32::min) - reach).floor() as i32;
        let y0 = (corners.iter().map(|c| c.1).fold(f32::MAX, f32::min) - reach).floor() as i32;
        let x1 = (corners.iter().map(|c| c.0).fold(f32::MIN, f32::max) + reach).ceil() as i32;
        let y1 = (corners.iter().map(|c| c.1).fold(f32::MIN, f32::max) + reach).ceil() as i32;
        let (w, h) = (x1 - x0, y1 - y0);
        let words = img.words();
        let mut out = vec![0u32; (w.max(0) * h.max(0)) as usize];
        for oy in 0..h {
            for ox in 0..w {
                // Back from where it lands to where it came from.
                let (qx, qy) = (
                    (x0 + ox) as f32 + 0.5 - pivot.0 - dx,
                    (y0 + oy) as f32 + 0.5 - pivot.1 - dy,
                );
                let (rx, ry) = (qx * cos + qy * sin, -qx * sin + qy * cos);
                let (mut px, mut py) = (pivot.0 + rx / sx, pivot.1 + ry / sy);
                if let Some(wave) = &self.wave {
                    let off = Pose::swing(wave, Pose::along(wave, (px, py), size)) * side;
                    if wave.across {
                        py -= off;
                    } else {
                        px -= off;
                    }
                }
                let (fx, fy) = (px.floor() as i32, py.floor() as i32);
                if fx >= 0 && fy >= 0 && fx < img.w && fy < img.h {
                    out[(oy * w + ox) as usize] = words[(fy * img.w + fx) as usize];
                }
            }
        }
        (Img::from_words(w, h, out), (x0, y0))
    }

    /// A painting's layer moved so: its square `rect` stretched and turned
    /// about its middle and moved, and the turn. A wave down its length is
    /// the one thing a layer cannot bend: it swings the whole picture
    /// about its head instead, as far as the wave throws its tail.
    pub fn place(&self, rect: [f32; 4]) -> ([f32; 4], f32) {
        let [x, y, w, h] = rect;
        let side = w.max(h);
        let (sx, sy) = (self.sx.max(0.05), self.sy.max(0.05));
        let (mut cx, mut cy) = (x + w / 2.0 + self.dx * side, y + h / 2.0 + self.dy * side);
        let mut angle = self.angle;
        if let Some(wave) = &self.wave {
            // The tail's throw as a turn about the head.
            let turn = Pose::swing(wave, 1.0).atan() * if wave.head_far { -1.0 } else { 1.0 };
            let turn = if wave.across { turn } else { -turn };
            let head = match (wave.across, wave.head_far) {
                (true, false) => (x, y + h / 2.0),
                (true, true) => (x + w, y + h / 2.0),
                (false, false) => (x + w / 2.0, y),
                (false, true) => (x + w / 2.0, y + h),
            };
            let (hx, hy) = (head.0 + self.dx * side, head.1 + self.dy * side);
            let (ux, uy) = (cx - hx, cy - hy);
            let (c, s) = (turn.cos(), turn.sin());
            (cx, cy) = (hx + ux * c - uy * s, hy + ux * s + uy * c);
            angle += turn;
        }
        let (nw, nh) = (w * sx, h * sy);
        ([cx - nw / 2.0, cy - nh / 2.0, nw, nh], angle)
    }
}

/// A phase from a few numbers, for `Pose::of`.
pub fn phase_of(parts: &[u64]) -> u32 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &part in parts {
        h ^= part;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (h ^ (h >> 29)) as u32
}

// ─── the pictures ───────────────────────────────────────────────────────────

/// Window pictures for paintings start here, eight a painting (each of up
/// to four frames, flipped or not): clear of the stage's own and of an HD
/// pack's (`LAYER_PICTURES + 0x0010_0000` up).
pub const PAINT_PICTURES: u32 = angel_stage::LAYER_PICTURES + 0x0060_0000;
/// The most paintings a drawer keeps pictures for.
pub const MAX_PAINTINGS: u32 = 0x8000;

/// The picture of frame `frame` of the painting a drawer keeps in place
/// `slot`, flipped or not.
pub fn picture(slot: u32, frame: usize, flip: bool) -> u32 {
    PAINT_PICTURES + (slot % MAX_PAINTINGS) * 8 + (frame.min(3) as u32) * 2 + u32::from(flip)
}

/// The smallest and largest a painting's frame is cut to, in pixels.
pub const MIN_SIDE: u32 = 16;
pub const MAX_SIDE: u32 = 1024;

/// How big to cut a painting shown `shown` window pixels across: twice
/// that (a picture has no smaller copies of itself to draw from), within
/// the bounds.
pub fn side_for(shown: f32) -> u32 {
    ((shown * 2.0).ceil() as u32).clamp(MIN_SIDE, MAX_SIDE)
}

/// A painting's frames cut as a window draws them, `side` pixels square
/// (no more than the painting has), straight RGBA: frame `f` is its square
/// of the strip (`looks::crop`), averaged down. `rgba` is the strip,
/// `width` by `height`, straight. None when nothing in it is drawn or the
/// frames do not divide it.
pub fn cut(
    rgba: &[u8],
    (width, height): (u32, u32),
    frames: usize,
    side: u32,
) -> Option<(u32, Vec<Vec<u8>>)> {
    if rgba.len() != width as usize * height as usize * 4 {
        return None;
    }
    let alpha = |x: u32, y: u32| rgba[(y as usize * width as usize + x as usize) * 4 + 3];
    let crop = crate::rules::looks::crop(width, height, frames, |x, y| alpha(x, y) >= 128)?;
    let side = side.clamp(1, crop.span);
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames as i64 {
        let mut px = vec![0u8; side as usize * side as usize * 4];
        for ty in 0..side {
            for tx in 0..side {
                // The strip's box this pixel covers, in the frame's cell.
                let x0 = crop.x + i64::from(tx * crop.span / side);
                let x1 = (crop.x + i64::from(((tx + 1) * crop.span).div_ceil(side))).max(x0 + 1);
                let y0 = crop.y + i64::from(ty * crop.span / side);
                let y1 = (crop.y + i64::from(((ty + 1) * crop.span).div_ceil(side))).max(y0 + 1);
                let (mut n, mut sum) = (0u64, [0u64; 4]);
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        n += 1;
                        if sx < 0 || sy < 0 || sx >= i64::from(crop.cell) || sy >= i64::from(height)
                        {
                            continue;
                        }
                        let i = (sy as usize * width as usize
                            + (f * i64::from(crop.cell) + sx) as usize)
                            * 4;
                        let a = u64::from(rgba[i + 3]);
                        for k in 0..3 {
                            sum[k] += u64::from(rgba[i + k]) * a;
                        }
                        sum[3] += a;
                    }
                }
                let o = (ty as usize * side as usize + tx as usize) * 4;
                if sum[3] > 0 {
                    for k in 0..3 {
                        px[o + k] = (sum[k] / sum[3]) as u8;
                    }
                    px[o + 3] = (sum[3] / n.max(1)) as u8;
                }
            }
        }
        out.push(px);
    }
    Some((side, out))
}

/// A pixel-art painting's frames cut as a window draws them: each frame's
/// square (`looks::crop`, as `cut` crops) blown up a whole number of times,
/// every pixel a hard block, to at least `side` (at most [`MAX_SIDE`], and
/// never below the painting's own pixels). Drawn at the size it shows, the
/// window then only shrinks hard blocks, so the pixels stay crisp at any
/// size, where averaging (`cut`) would blur them. The side and the frames,
/// or None as for `cut`.
pub fn cut_pixels(
    rgba: &[u8],
    (width, height): (u32, u32),
    frames: usize,
    side: u32,
) -> Option<(u32, Vec<Vec<u8>>)> {
    if rgba.len() != width as usize * height as usize * 4 {
        return None;
    }
    let alpha = |x: u32, y: u32| rgba[(y as usize * width as usize + x as usize) * 4 + 3];
    let crop = crate::rules::looks::crop(width, height, frames, |x, y| alpha(x, y) >= 128)?;
    let span = crop.span.max(1);
    let times = side.div_ceil(span).clamp(1, (MAX_SIDE / span).max(1));
    let out_side = span * times;
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames as i64 {
        let mut px = vec![0u8; out_side as usize * out_side as usize * 4];
        for sy in 0..span {
            for sx in 0..span {
                let (x, y) = (crop.x + i64::from(sx), crop.y + i64::from(sy));
                if x < 0 || y < 0 || x >= i64::from(crop.cell) || y >= i64::from(height) {
                    continue;
                }
                let i = (y as usize * width as usize + (f * i64::from(crop.cell) + x) as usize) * 4;
                let pixel = &rgba[i..i + 4];
                if pixel[3] == 0 {
                    continue;
                }
                for by in 0..times {
                    let row = (sy * times + by) as usize * out_side as usize;
                    for bx in 0..times {
                        let o = (row + (sx * times + bx) as usize) * 4;
                        px[o..o + 4].copy_from_slice(pixel);
                    }
                }
            }
        }
        out.push(px);
    }
    Some((out_side, out))
}

/// A painting's frames cut as a window draws them: pixel art by
/// [`cut_pixels`], anything else by [`cut`].
pub fn cut_as(
    pixel: bool,
    rgba: &[u8],
    size: (u32, u32),
    frames: usize,
    side: u32,
) -> Option<(u32, Vec<Vec<u8>>)> {
    if pixel {
        cut_pixels(rgba, size, frames, side)
    } else {
        cut(rgba, size, frames, side)
    }
}

/// Worn clothing's common alpha subject, normalized to a square texture.
/// Placement supplies the destination rectangle, so the PNG's transparent
/// square margins and aspect can never shrink or displace torso armour.
/// All frames share one crop, preserving animation without breathing scale.
pub fn cut_subject(
    rgba: &[u8],
    (width, height): (u32, u32),
    frames: usize,
    side: u32,
) -> Option<(u32, Vec<Vec<u8>>)> {
    if rgba.len() != width as usize * height as usize * 4
        || frames == 0
        || width % frames as u32 != 0
    {
        return None;
    }
    let cell = width / frames as u32;
    let (mut left, mut top, mut right, mut bottom) = (cell, height, 0, 0);
    let mut any = false;
    for f in 0..frames as u32 {
        for y in 0..height {
            for x in 0..cell {
                if rgba[((y * width + f * cell + x) * 4 + 3) as usize] >= 128 {
                    any = true;
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
                }
            }
        }
    }
    if !any {
        return None;
    }
    let (w, h) = (right - left + 1, bottom - top + 1);
    let side = side.clamp(1, MAX_SIDE);
    let cut = (0..frames as u32)
        .map(|f| {
            let mut out = Vec::with_capacity(side as usize * side as usize * 4);
            for y in 0..side {
                for x in 0..side {
                    let i = (((top + y * h / side) * width + f * cell + left + x * w / side) * 4)
                        as usize;
                    out.extend_from_slice(&rgba[i..i + 4]);
                }
            }
            out
        })
        .collect();
    Some((side, cut))
}

/// A painting's frames' normal maps (docs/WORLD-TIERS.md, "Normal maps"),
/// each `side` square as `cut` cut its frames: from its own normal map
/// (`normal`, straight RGBA the painting's `size`, green up), each normal
/// under the painting's alpha so its frames crop and average as the
/// painting's do, made unit and opaque. None when it does not cut so.
pub fn cut_normals(
    painting: &[u8],
    normal: &[u8],
    size: (u32, u32),
    frames: usize,
    side: u32,
) -> Option<Vec<Vec<u8>>> {
    if normal.len() != painting.len() {
        return None;
    }
    let mut under = normal.to_vec();
    for (n, p) in under.chunks_exact_mut(4).zip(painting.chunks_exact(4)) {
        n[3] = p[3];
    }
    let (cut_side, mut cut) = cut(&under, size, frames, side)?;
    (cut_side == side && cut.len() == frames).then_some(())?;
    cut.iter_mut().for_each(|f| angel_stage::normals::tidied(f));
    Some(cut)
}

/// Normal maps made from a painting's frames as `cut` cut them, `side`
/// square, drawn over a square `art` pixels of the room across (the art's
/// grain, so it rounds as the figure it stands for).
pub fn made_normals(frames: &[Vec<u8>], side: u32, art: f32) -> Vec<Vec<u8>> {
    let grain = side as f32 / art.max(1.0);
    frames
        .iter()
        .map(|f| angel_stage::normals::of_picture(f, (side, side), grain))
        .collect()
}

/// How many of a page's pixels a room pixel is drawn at, about, for the
/// size a page's paintings are cut to: a page fits the view to its canvas,
/// which it does not say.
pub const PAGE_SCALE: f32 = 6.0;

/// A page's paintings, across a wire (a browser): those it is to fetch
/// (`wanted`), each one's frames once it has decoded it (`put`), and the
/// pictures it was sent, each once, with the stage frame that first names
/// it.
#[derive(Default)]
pub struct PagePaintings {
    known: std::collections::BTreeMap<(String, bool), PagePainting>,
    wanted: std::collections::VecDeque<(String, bool)>,
    sent: std::collections::BTreeSet<u32>,
    next: u32,
}

/// A painting as a page has it: to be fetched, asked for, cut, or not to
/// be had.
enum PagePainting {
    Wanted {
        slot: u32,
        side: u32,
        frames: usize,
        pixel: bool,
    },
    Asked {
        slot: u32,
        side: u32,
        frames: usize,
        pixel: bool,
    },
    Ready {
        slot: u32,
        side: u32,
        cut: Vec<Vec<u8>>,
    },
    Failed,
}

impl PagePaintings {
    /// The picture of `p`'s frame, flipped or not, once the page has the
    /// painting; its pixels go in `pictures` the first time. A painting not
    /// yet had is wanted (`wanted`), cut to twice the size it shows at
    /// `scale` page pixels to a room pixel.
    pub fn picture(
        &mut self,
        p: &Painting,
        scale: f32,
        pictures: &mut Vec<angel_stage::Picture>,
    ) -> Option<u32> {
        let side = side_for(p.rect[2].max(p.rect[3]) * scale);
        let key = (p.sha.clone(), p.subject);
        match self.known.get_mut(&key) {
            None => {
                if self.next >= MAX_PAINTINGS {
                    return None;
                }
                self.known.insert(
                    key.clone(),
                    PagePainting::Wanted {
                        slot: self.next,
                        side,
                        frames: p.frames,
                        pixel: p.pixel,
                    },
                );
                self.next += 1;
                self.wanted.push_back(key);
                None
            }
            Some(
                PagePainting::Wanted { side: had, .. } | PagePainting::Asked { side: had, .. },
            ) => {
                *had = (*had).max(side);
                None
            }
            Some(PagePainting::Ready { slot, side, cut }) => {
                let frame = cut.get(p.frame)?;
                let id = picture(*slot, p.frame, p.flip);
                if self.sent.insert(id) {
                    let rgba = if p.flip {
                        flipped(frame, *side)
                    } else {
                        frame.clone()
                    };
                    pictures.push(angel_stage::Picture {
                        id,
                        width: *side,
                        height: *side,
                        rgba,
                    });
                }
                Some(id)
            }
            Some(PagePainting::Failed) => None,
        }
    }

    /// The next painting the page is to fetch (`/paint/<sha256>.png`), by
    /// its hash; it hands back its pixels (`put`), or says it could not
    /// (`failed`).
    pub fn wanted(&mut self) -> Option<String> {
        let sha = self.wanted.pop_front()?;
        if let Some(&PagePainting::Wanted {
            slot,
            side,
            frames,
            pixel,
        }) = self.known.get(&sha)
        {
            self.known.insert(
                sha.clone(),
                PagePainting::Asked {
                    slot,
                    side,
                    frames,
                    pixel,
                },
            );
        }
        Some(sha.0)
    }

    /// The page decoded painting `sha`, `width` by `height` straight RGBA:
    /// cut into its frames. False when it was not asked for or does not
    /// cut (its frames do not divide it, nothing is drawn on it).
    pub fn put(&mut self, sha: &str, (width, height): (u32, u32), rgba: &[u8]) -> bool {
        let mut accepted = false;
        for subject in [false, true] {
            let key = (sha.to_string(), subject);
            let Some(&PagePainting::Asked {
                slot,
                side,
                frames,
                pixel,
            }) = self.known.get(&key)
            else {
                continue;
            };
            let cut = if subject {
                cut_subject(rgba, (width, height), frames, side)
            } else {
                cut_as(pixel, rgba, (width, height), frames, side)
            };
            let state = match cut {
                Some((side, cut)) => {
                    accepted = true;
                    PagePainting::Ready { slot, side, cut }
                }
                None => PagePainting::Failed,
            };
            self.known.insert(key, state);
        }
        accepted
    }

    /// The page could not fetch or decode painting `sha`: its inks stand.
    pub fn failed(&mut self, sha: &str) {
        for subject in [false, true] {
            if let Some(state) = self.known.get_mut(&(sha.to_string(), subject)) {
                *state = PagePainting::Failed;
            }
        }
    }

    /// The page lost its pictures: each goes again when next named.
    pub fn forget(&mut self) {
        self.sent.clear();
    }
}

/// A frame cut by `cut`, mirrored.
pub fn flipped(rgba: &[u8], side: u32) -> Vec<u8> {
    let side = side as usize;
    let mut out = vec![0u8; rgba.len()];
    for y in 0..side {
        for x in 0..side {
            let (from, to) = ((y * side + x) * 4, (y * side + side - 1 - x) * 4);
            out[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    out
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__paintings_tests.rs"]
mod tests;
