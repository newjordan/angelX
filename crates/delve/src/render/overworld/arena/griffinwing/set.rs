//! The set pieces on the rail as painted billboards, when their art is whole:
//! the bonus gates and hearth rings (`griffinwing.gate`, `.hearth-ring`) and
//! the last beacon (`griffinwing.beacon`). Each is a picture standing at the
//! sim's own point, its size from the sim's own radius, drawn over the 3D scene
//! wherever the scene's depth leaves it showing (a cliff or a griffin nearer
//! than the piece hides it; the clear middle of a ring shows what is beyond).
//! Art never sets a hitbox and never moves a piece: the rules own where a ring
//! is, how wide it judges and which seat has flown it; the pictures only
//! dress that. Without the art the stage's own hoops and blocks are built in
//! the 3D scene as before (`painted` says which).
use super::bank::{self, Pic};
use super::cam::Cam;
use super::{H, W};
use crate::render::overworld::ink::Img;
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::raster;
use crate::rules::griffinwing::trig::Det;
use crate::rules::griffinwing::{Session, Thing, bit, rail, rail::units};

/// A gate's clear hole is 17.0 dots in radius in its painting and a hearth
/// ring's band 16.8 (medians over the native idle frames, measured along rays
/// that miss the clasps and the lanterns): the sprite is scaled until that
/// opening is the rules' radius less a tenth of a unit (the most a painted
/// piece may look smaller than its radius), so what a knight must fit through
/// is what the sim judges (a gate's 2.0 units, a ring's 2.2). The band and the
/// lanterns stand outside it, their outer edge on about the distance the rules
/// still count a pass (the radius and a craft's own, one unit).
const GATE_HOLE: f32 = 17.0;
const RING_HOLE: f32 = 16.8;
const WITHIN: f32 = 0.1;
/// A beacon is drawn from its tower's foot, in the stage's old box's height.
const BEACON_FOOT: f32 = -16.0;
const BEACON_TALL: f32 = 31.0;
/// The painted tower's own height in its picture (it stands 156 dots tall in
/// the 160 of its canvas).
const BEACON_PICTURE: f32 = 156.0;

fn slot(thing: Thing) -> Option<&'static str> {
    match thing {
        Thing::Gate => Some("gate"),
        Thing::Ring => Some("hearth-ring"),
        _ => None,
    }
}

/// Whether `thing` is drawn as a painted billboard (its slot is painted
/// whole); if so the 3D scene leaves its blocks out.
pub(super) fn painted(thing: Thing) -> bool {
    slot(thing).is_some_and(|id| bank::pic(id, "idle", 0).is_some())
}

/// Whether the last beacon is drawn as a painted billboard.
pub(super) fn beacon_painted() -> bool {
    bank::pic("beacon", "unlit", 0).is_some()
}

/// A rail point as the eye has it: the dot it lands on and how far.
fn project(cam: &Cam, s: f32, x: f32, z: f32) -> Option<(f32, f32, f32)> {
    let w = rail::rail().world(s, x, z);
    raster::project_point(&cam.view(), W as usize, H as usize, cam.point(v3(w[0], w[1], w[2])))
}

/// How many dots a world unit is across at `depth`.
fn dots_per_unit(cam: &Cam, depth: f32) -> f32 {
    raster::dots_tall(&cam.view(), W as usize, H as usize, 1.0, depth)
}

/// What is to be stood in the picture: a frame, where its anchor lands, how
/// many dots a picture dot is across and up, and how far the piece is.
pub(super) struct Piece {
    pub pic: &'static Pic,
    pub at: (f32, f32),
    pub scale: (f32, f32),
    pub depth: f32,
}

/// Draw `piece` over `im` where `depth` leaves it showing. A far piece is
/// drawn from a halved picture, so its dots are about one to one with the
/// screen's.
pub(super) fn blit(im: &mut Img, depth: &[f32], piece: &Piece) {
    let (mut level, mut k) = (0usize, piece.scale.0.min(piece.scale.1));
    while k < 0.7 && level < 3 {
        k *= 2.0;
        level += 1;
    }
    let img = piece.pic.level(level);
    let step = (1 << level) as f32;
    let (sx, sy) = (piece.scale.0 * step, piece.scale.1 * step);
    let (ax, ay) = ((piece.pic.ax >> level) as f32, (piece.pic.ay >> level) as f32);
    let (px, py) = piece.at;
    let x0 = ((px - ax * sx).floor() as i32).max(0);
    let x1 = ((px + (img.w as f32 - ax) * sx).ceil() as i32).min(W);
    let y0 = ((py - ay * sy).floor() as i32).max(0);
    let y1 = ((py + (img.h as f32 - ay) * sy).ceil() as i32).min(H);
    for y in y0..y1 {
        let v = ay + (y as f32 + 0.5 - py) / sy;
        if v < 0.0 {
            continue;
        }
        for x in x0..x1 {
            let u = ax + (x as f32 + 0.5 - px) / sx;
            if u < 0.0 {
                continue;
            }
            if let Some(c) = img.get(u as i32, v as i32)
                && depth[(y * W + x) as usize] > piece.depth - 0.25
            {
                im.set(x, y, c);
            }
        }
    }
}

/// The rail's way forward at `s`, in the eye's frame: the line a ring's plane
/// is square to.
fn forward(cam: &Cam, s: f32) -> V3 {
    let (sin, cos) = rail::rail().at(s).yaw.dsin_cos();
    cam.dir(v3(cos, sin, 0.0))
}

/// The set pieces of the moment, over `im` (the scene and its sky), as `seat`
/// sees them.
pub(super) fn paint(im: &mut Img, depth: &[f32], cam: &Cam, s: &Session, seat: u32) {
    let anchor = units(s.anchor());
    let mut pieces: Vec<Piece> = Vec::new();
    for p in &s.props {
        let Some(id) = slot(p.thing) else { continue };
        let taken = p.taken & bit(seat) != 0;
        if taken || (units(p.s) - anchor).abs() >= 110.0 {
            continue;
        }
        let Some(pic) = bank::at(id, "idle", s.elapsed) else {
            continue;
        };
        let su = units(p.s);
        let Some((px, py, d)) = project(cam, su, units(p.x), units(p.z)) else {
            continue;
        };
        if d < 1.0 {
            continue;
        }
        let hole = if p.thing == Thing::Gate { GATE_HOLE } else { RING_HOLE };
        let k = (units(p.w) - WITHIN) * dots_per_unit(cam, d) / hole;
        // A ring is round in a plane square to the rail: turned from the eye,
        // it narrows by the cosine of the turn.
        let to = cam.point(rail_point(su, units(p.x), units(p.z))).normalize();
        let narrow = to.dot(forward(cam, su)).abs().clamp(0.35, 1.0);
        pieces.push(Piece {
            pic,
            at: (px, py),
            scale: (k * narrow, k),
            depth: d,
        });
    }
    pieces.extend(beacon_piece(cam, s));
    // Far to near, so a near piece covers a far one.
    pieces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    for piece in &pieces {
        blit(im, depth, piece);
    }
}

/// The last beacon as a piece, when it is on the rail near enough to draw and
/// painted.
fn beacon_piece(cam: &Cam, s: &Session) -> Option<Piece> {
    let su = beacon_at(s)?;
    if !beacon_painted() || (su - units(s.anchor())).abs() >= 160.0 {
        return None;
    }
    let (px, py, d) = project(cam, su, 0.0, BEACON_FOOT)?;
    if d < 1.0 {
        return None;
    }
    let k = BEACON_TALL * dots_per_unit(cam, d) / BEACON_PICTURE;
    Some(Piece {
        pic: beacon_frame(s)?,
        at: (px, py),
        scale: (k, k),
        depth: d,
    })
}

fn rail_point(s: f32, x: f32, z: f32) -> V3 {
    let w = rail::rail().world(s, x, z);
    v3(w[0], w[1], w[2])
}

/// Where along the rail the last beacon stands, when it is on the rail: lit
/// for the way home, dark before the Wyrm falls.
pub(super) fn beacon_at(s: &Session) -> Option<f32> {
    use crate::rules::griffinwing::Mode;
    if matches!(s.mode, Mode::Victory | Mode::Tally) {
        Some(units(s.home) + 100.0)
    } else if s.stage == 2 && s.boss.is_none() {
        Some(units(rail::ORIGIN[2]) + 560.0)
    } else {
        None
    }
}

/// The beacon's frame: dark, or one of the four flames at the sheet's own
/// durations.
fn beacon_frame(s: &Session) -> Option<&'static Pic> {
    use crate::rules::griffinwing::Mode;
    if matches!(s.mode, Mode::Victory | Mode::Tally) {
        bank::at("beacon", "lit", s.elapsed)
    } else {
        bank::pic("beacon", "unlit", 0)
    }
}
