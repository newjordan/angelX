//! race3d.rs — the 3D races on each knight's own screen.
//!
//! While a race hall's race is on, the room view hands the whole canvas to
//! it: the course in true 3D through `world3d`'s banded raster, under a
//! camera riding behind this seat's ball, cart or griffin. The frame comes
//! back in the realm's own inks and is laid down as ordered dots on black
//! paper, the knights stand in it as their own sprites (inside the cage
//! ball, in the tub, on the griffin's back), and the race's HUD sits over
//! it: place, clock, the field's strip, the title card, the jokes, the
//! finish board.
mod build;

use super::super::ink::{BLACK, Img, Rgb, bayer, nearest_plain, text_width};
use super::class_looks::{self, Look, Pose};
use super::knights::{self as kn, Facing};
use super::{NATIVE_H, NATIVE_W, TILE, enlarge, fire, sprites};
use super::super::kit;
use crate::render::overworld::light::Light;
use crate::rules::race3d::{
    self as r3, BALL_R, Course, Race3d, Rider, TITLE, Wreck,
};
use crate::rules::{HZ, Run};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Mesh;
use crate::render::world3d::raster::{self, Backdrop, Firelight, View3};
use std::sync::OnceLock;

/// The race owns the screen while it runs and while its board is up.
pub(super) fn takeover(run: &Run) -> bool {
    run.race3d_showing()
}

/// A seat's ink, as its knight wears it.
fn seat_ink(id: u32) -> char {
    match id {
        1 => '7',
        2 => '1',
        3 => 'A',
        _ => '4',
    }
}

std::thread_local! {
    /// The moving things' mesh, rebuilt each frame into the same buffer.
    static MOVERS: std::cell::RefCell<Mesh> = std::cell::RefCell::new(Mesh::new());
}

/// The race as the arena's two layers: the 3D course as scenery, the
/// knights and the HUD as the figures over it.
pub(super) fn layers(run: &Run, focus: Option<u32>, legible: bool) -> (Img, Img) {
    let race = run.race3d.as_ref().expect("a race is showing");
    let (w, h) = (NATIVE_W, NATIVE_H);
    let seat = focus
        .filter(|id| race.riders.contains_key(id))
        .or_else(|| race.order().first().copied());
    let view = camera(race, seat, w, h);
    let (rgba, depth) = MOVERS.with(|cell| {
        let mut movers = cell.borrow_mut();
        build::movers(&mut movers, race, seat);
        let (fog, backdrop) = match race.course {
            Course::Griffin => (70.0, Backdrop::Night),
            _ => (24.0, Backdrop::Dark([2, 3, 6])),
        };
        raster::render_meshes(
            &[build::course(race.course), &movers],
            &view,
            w as usize,
            h as usize,
            Firelight::STEADY,
            fog,
            backdrop,
        )
    });
    let mut scene = dots(rgba.as_raw(), w, h);
    knights(&mut scene, run, race, seat, &view, &depth, w, h);
    let mut over = Img::new(w, h);
    hud(&mut over, run, race, seat, &view, legible);
    (scene, over)
}

/// A course point a little past either end of the track, carried on along
/// the end's heading, so a camera behind the start line has somewhere to be.
fn world(t: &r3::Track, s: f32, x: f32, z: f32) -> V3 {
    let end = t.len();
    let (clamped, beyond) = if s < 0.0 { (0.0, s) } else if s > end { (end, s - end) } else { (s, 0.0) };
    let w = t.world(clamped, x, z);
    let yaw = t.at(clamped).yaw;
    v3(w[0] + yaw.cos() * beyond, w[1] + yaw.sin() * beyond, w[2])
}

/// The chase camera behind `seat`'s ride, looking a way down the course.
fn camera(race: &Race3d, seat: Option<u32>, w: i32, h: i32) -> View3 {
    let t = r3::track(race.course);
    let (back, up, ahead, fov, follow) = match race.course {
        Course::Shaft => (4.6, 2.3, 6.0, 1.2, 0.55),
        Course::Cart => (4.3, 2.2, 6.5, 1.2, 0.5),
        Course::Griffin => (5.6, 1.7, 11.0, 1.25, 0.7),
    };
    let (s, x, z) = seat
        .and_then(|id| race.riders.get(&id))
        .map_or((r3::track(race.course).post(0.0) + 2.0, 0.0, 0.0), |r| build::drawn_at(t, r));
    // A ball off the edge drops away under a camera that stays at the rail.
    let (eye_z, look_z) = match race.course {
        Course::Griffin => (z + up, z + 0.4),
        _ => (z.max(0.0) + up, if z < 0.0 { z } else { 0.0 }),
    };
    let eye = world(t, s - back, x * follow, eye_z);
    let look = world(t, s + ahead, x * follow * 0.6, look_z);
    let d = look - eye;
    let flat = d.x.hypot(d.y).max(1e-3);
    let _ = (w, h);
    View3 {
        pos: eye,
        heading_rad: d.y.atan2(d.x),
        pitch: d.z.atan2(flat),
        fov_rad: fov,
    }
}

// ─── the frame on black paper ───────────────────────────────────────────────

/// The raster's colours on the realm's palette, by a 32-step cube built
/// once (the palette's nearest plain ink for each cell).
fn snap(c: [u8; 3]) -> Rgb {
    static CUBE: OnceLock<Vec<Rgb>> = OnceLock::new();
    let cube = CUBE.get_or_init(|| {
        let mut out = Vec::with_capacity(32 * 32 * 32);
        for r in 0..32u32 {
            for g in 0..32u32 {
                for b in 0..32u32 {
                    let mid = |v: u32| (v * 8 + 4) as f32;
                    out.push(nearest_plain([mid(r), mid(g), mid(b)]));
                }
            }
        }
        out
    });
    cube[(usize::from(c[0] >> 3) << 10) | (usize::from(c[1] >> 3) << 5) | usize::from(c[2] >> 3)]
}

/// The frame on black paper, as light falling off into the night: each
/// pixel is its own ink, lifted to a readable floor and stepped down by its
/// tone, so a lit surface fades smoothly toward black. Where it has faded
/// below `DUSK_LO` it is black paper; between `DUSK_LO` and `DUSK_HI` the
/// last of the light thins out in an ordered dither, a narrow fringe at the
/// edge of each pool and not a screen over every surface.
fn dots(rgba: &[u8], w: i32, h: i32) -> Img {
    dots_with(rgba, w, h, (12.0, 140.0, 1.1, 66.0))
}

/// The fringe, in the brightest channel of the faded ink.
const DUSK_LO: f32 = 22.0;
const DUSK_HI: f32 = 34.0;

pub(super) fn dots_with(rgba: &[u8], w: i32, h: i32, k: (f32, f32, f32, f32)) -> Img {
    let (black_point, white_point, gamma, ink_floor) = k;
    // The tone's ramp by whole luma, and the readable floor's lift by the
    // brightest channel: 256 entries each, built once a frame, so a pixel
    // costs table reads and a few multiplies, no powers.
    let ramp: [f32; 256] = std::array::from_fn(|l| {
        let tone = ((l as f32 - black_point) / (white_point - black_point)).clamp(0.0, 1.0).powf(gamma);
        tone.powf(0.8)
    });
    let floor: [f32; 256] = std::array::from_fn(|p| (ink_floor / (p as f32).max(1.0)).max(1.0));
    let mut px = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let c = [rgba[i], rgba[i + 1], rgba[i + 2]];
            let luma = (77 * u32::from(c[0]) + 150 * u32::from(c[1]) + 29 * u32::from(c[2])) >> 8;
            let peak = c[0].max(c[1]).max(c[2]);
            // The ink at a readable floor, then down the tone's ramp.
            let mut lift = floor[usize::from(peak)] * ramp[luma as usize];
            let bright = f32::from(peak.max(1)) * lift;
            if bright < DUSK_HI {
                if (bright - DUSK_LO) * (1.0 / (DUSK_HI - DUSK_LO)) <= bayer(x, y) {
                    px.push(BLACK);
                    continue;
                }
                // The fringe's flecks keep the fringe's own dim ink.
                lift = DUSK_HI / f32::from(peak.max(1));
            }
            let lifted = c.map(|v| (f32::from(v) * lift).min(255.0) as u8);
            px.push(snap(lifted));
        }
    }
    Img::from_rgb(w, h, &px)
}

// ─── the knights in their rides ─────────────────────────────────────────────

/// A sprite turned upside down: a knight thrown from a cart.
fn flip_v(im: &Img) -> Img {
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                out.set(x, im.h - 1 - y, c);
            }
        }
    }
    out
}

/// Stamp `sprite` standing on world point `feet`, `tall` world units high,
/// wherever the scene in front of `anchor` (its depth) leaves it showing.
#[allow(clippy::too_many_arguments)]
fn stand(scene: &mut Img, depth: &[f32], view: &View3, sprite: &Img, feet: V3, anchor: V3, tall: f32, w: i32, h: i32) -> Option<(i32, i32)> {
    let (fx, fy, fd) = raster::project_point(view, w as usize, h as usize, feet)?;
    let (_, _, ad) = raster::project_point(view, w as usize, h as usize, anchor)?;
    let px = raster::dots_tall(view, w as usize, h as usize, tall, fd);
    let k = (px / sprite.h as f32 * 8.0).floor().max(1.0) / 8.0;
    let im = enlarge(sprite, k);
    let (x0, y0) = (fx.round() as i32 - im.w / 2, fy.round() as i32 - im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            let Some(c) = im.get(x, y) else { continue };
            let (sx, sy) = (x0 + x, y0 + y);
            if sx < 0 || sy < 0 || sx >= w || sy >= h {
                continue;
            }
            if depth[(sy * w + sx) as usize] > ad - 0.02 {
                scene.set(sx, sy, c);
            }
        }
    }
    Some((fx.round() as i32, y0))
}

/// A racer as the chase camera sees them, from behind: their class's own
/// figure (running inside a ball, standing in a tub or astride a griffin)
/// in the seat's trim, or the realm's knight for a knight of no class.
fn racer(run: &Run, id: u32, course: Course, rider: &Rider) -> Img {
    let Some(hero) = run.players.get(&id) else {
        return sprites::knight(id);
    };
    match Look::of(hero) {
        Some(look) => {
            let pose = if course == Course::Shaft && rider.racing() {
                Pose::Step(u8::from((rider.spin * 1.5).sin() > 0.0))
            } else {
                Pose::Stand
            };
            class_looks::figure(look, Facing::Up, pose, true).recolor(kn::seat_colours(id))
        }
        None => kn::portrait(hero, id),
    }
}

#[allow(clippy::too_many_arguments)]
fn knights(scene: &mut Img, run: &Run, race: &Race3d, seat: Option<u32>, view: &View3, depth: &[f32], w: i32, h: i32) {
    let t = r3::track(race.course);
    // Far riders first so near ones overlap them.
    let mut field: Vec<(f32, u32, &Rider)> = race
        .riders
        .iter()
        .map(|(&id, r)| {
            let (s, x, z) = build::drawn_at(t, r);
            let d = world(t, s, x, z) - view.pos;
            (d.x * d.x + d.y * d.y + d.z * d.z, id, r)
        })
        .collect();
    field.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (near, id, rider) in field {
        // A rival right at the lens would fill the screen: leave them out
        // until they pull ahead or drop back.
        if Some(id) != seat && (near < 2.4 * 2.4 || build::hidden(race, seat, id)) {
            continue;
        }
        let (s, x, z) = build::drawn_at(t, rider);
        let mut sprite = racer(run, id, race.course, rider);
        let (feet, anchor, tall) = match race.course {
            Course::Shaft => {
                let bob = if rider.racing() { (rider.spin * 2.0).sin().abs() * 0.05 } else { 0.0 };
                (world(t, s, x, z + 0.14 + bob), world(t, s, x, z + BALL_R), 0.64)
            }
            Course::Cart => {
                let thrown = rider.crash > 0 && matches!(rider.wreck, Wreck::Buffer | Wreck::Timber);
                if thrown {
                    sprite = flip_v(&sprite);
                    let k = 1.0 - rider.crash as f32 / r3::CRASH as f32;
                    let arc = (k * std::f32::consts::PI).sin() * 1.5;
                    let p = world(t, s + k * 1.5, x, z + 0.6 + arc);
                    (p, p, 0.9)
                } else {
                    (world(t, s, x, z + 0.3), world(t, s, x, z + 0.45), 0.9)
                }
            }
            Course::Griffin => (world(t, s, x, z + 0.16), world(t, s, x, z + 0.3), 0.8),
        };
        stand(scene, depth, view, &sprite, feet, anchor, tall, w, h);
    }
}

// ─── the HUD ────────────────────────────────────────────────────────────────

/// Text `k` times its size, centred on `cx`.
fn big(over: &mut Img, cx: i32, y: i32, text: &str, ink: char, k: f32) -> i32 {
    let mut im = Img::new(text_width(text), 7);
    im.text(0, 0, text, ink);
    let im = enlarge(&im, k);
    over.stamp(&im, cx - im.w / 2, y);
    im.h
}

/// A dark plate behind words, with a thin rule round it.
fn plate(over: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    super::super::chrome::panel(over, (x, y, w, h), super::super::chrome::Tone::SLATE);
}

fn place_word(n: usize) -> String {
    let suffix = match n {
        1 => "ST",
        2 => "ND",
        3 => "RD",
        _ => "TH",
    };
    format!("{n}{suffix}")
}

/// The joke a crash earns: two or three words, picked by how many times
/// this knight has crashed already.
pub(super) fn crash_words(wreck: Wreck, crashes: u16) -> &'static str {
    let pick = |words: &[&'static str]| words[usize::from(crashes) % words.len()];
    match wreck {
        Wreck::Fall => pick(&["WHEEEE!", "MIND THE GAP", "NOT AGAIN", "ROPE, PLEASE"]),
        Wreck::Bonk => pick(&["BONK!", "CARTS FIRST", "ORE ON BOARD"]),
        Wreck::Buffer => pick(&["BUFFER STOP", "LINE ENDS HERE"]),
        Wreck::Gap => pick(&["NO RAIL!", "MIND THE GAP"]),
        Wreck::Timber => pick(&["TIMBER!", "JUMP NEXT TIME"]),
        Wreck::Tumble => pick(&["FEATHERS!", "BIRD DOWN", "SORRY, GRIFFIN"]),
        Wreck::None => "",
    }
}

/// The haul's little picture: a shard, a nugget, a ring.
fn haul_icon(over: &mut Img, course: Course, x: i32, y: i32) {
    match course {
        Course::Shaft => {
            for (dx, dy, c) in [(2, 0, 'z'), (1, 1, 'z'), (2, 1, '3'), (3, 1, 'z'), (0, 2, 'q'), (1, 2, 'z'), (2, 2, 'z'), (3, 2, 'z'), (4, 2, 'q'), (1, 3, 'q'), (2, 3, 'z'), (3, 3, 'q'), (2, 4, 'q')] {
                over.put(x + dx, y + dy, c);
            }
        }
        Course::Cart => {
            over.rect(x, y + 1, 5, 3, '4');
            over.put(x + 1, y + 1, '6');
            over.rect(x + 1, y, 3, 1, 'a');
        }
        Course::Griffin => {
            for (dx, dy) in [(1, 0), (2, 0), (3, 0), (0, 1), (4, 1), (0, 2), (4, 2), (0, 3), (4, 3), (1, 4), (2, 4), (3, 4)] {
                over.put(x + dx, y + dy, '4');
            }
        }
    }
}

/// A laurel wreath: two sprays of leaves curving up round a gap, tied
/// at the bottom with a gold knot.
fn laurel(over: &mut Img, x: i32, y: i32) {
    let wreath = Img::from_rows(&[
        "..y.......y..",
        ".yA.......Ay.",
        "yAM.......MAy",
        "YA.........AY",
        "MAy.......yAM",
        ".yAM.....MAy.",
        "..yAM...MAy..",
        "....MA4AM....",
        ".....454.....",
    ]);
    over.stamp(&wreath, x, y);
}

fn hud(over: &mut Img, run: &Run, race: &Race3d, seat: Option<u32>, view: &View3, legible: bool) {
    let (w, h) = (over.w, over.h);
    let t = r3::track(race.course);
    let me = seat.and_then(|id| race.riders.get(&id));
    // Feathers about a tumbling or ruffled griffin, stars round a bonked ball.
    for (&id, rider) in &race.riders {
        let (s, x, z) = build::drawn_at(t, rider);
        let Some((px, py, d)) = raster::project_point(view, w as usize, h as usize, world(t, s, x, z + 0.6)) else {
            continue;
        };
        let (px, py) = (px as i32, py as i32);
        let spread = (raster::dots_tall(view, w as usize, h as usize, 1.0, d) as i32).clamp(4, 40);
        let flutter = match race.course {
            Course::Griffin => rider.crash > 0 || rider.stun > 0,
            Course::Shaft => rider.wreck == Wreck::Bonk && rider.crash > 0,
            Course::Cart => false,
        };
        if flutter {
            for k in 0..10u32 {
                let a = k as f32 * 0.63 + race.t as f32 * 0.21;
                let r = spread as f32 * (0.5 + 0.08 * (k % 4) as f32);
                let (fx, fy) = (px + (a.cos() * r) as i32, py + (a.sin() * r * 0.6) as i32 + (race.t as i32 + k as i32 * 3) % 7);
                let ink = if race.course == Course::Griffin { ['i', 'H', 'h'][k as usize % 3] } else { '5' };
                over.put(fx, fy, ink);
                if race.course == Course::Griffin {
                    over.put(fx + 1, fy, ink);
                    over.put(fx + 2, fy + 1, ink);
                }
                if race.course == Course::Shaft {
                    over.put(fx - 1, fy, ink);
                    over.put(fx + 1, fy, ink);
                    over.put(fx, fy - 1, ink);
                    over.put(fx, fy + 1, ink);
                }
            }
        }
        // The rope that hauls a fallen knight back.
        if matches!(rider.wreck, Wreck::Fall | Wreck::Gap) && rider.crash > 0 && rider.crash <= r3::CRASH / 3 {
            over.line(px, 0, px, py - spread / 2, 'P');
        }
        // A tag over each other knight: their place, in their own ink.
        if Some(id) != seat && legible && d > 1.0 && (0..w).contains(&px) && py > 10 && !build::hidden(race, seat, id) {
            let tag = race.place(id).to_string();
            over.text(px - 2, py - spread - 9, &tag, seat_ink(id));
        }
    }
    if !legible {
        return;
    }
    if race.done {
        board(over, run, race, seat);
        return;
    }
    // The field down the course: a strip, a pip for each knight, a flag.
    let (sx, sy, sw) = (w / 2 - 80, 8, 160);
    over.rect(sx, sy + 2, sw, 1, 'g');
    over.rect(sx + sw, sy - 2, 1, 6, 'h');
    over.rect(sx + sw + 1, sy - 2, 3, 2, '9');
    for (&id, rider) in &race.riders {
        let k = (rider.s / t.finish).clamp(0.0, 1.0);
        let x = sx + (k * sw as f32) as i32;
        let size = if Some(id) == seat { 5 } else { 3 };
        over.rect(x - size / 2, sy + 2 - size / 2, size, size, seat_ink(id));
        if Some(id) == seat {
            over.frame(x - 3, sy - 1, 7, 7, 'H');
        }
    }
    if let Some(me) = me {
        let place = race.place(seat.unwrap());
        big(over, 26, 4, &place_word(place), if place == 1 { '5' } else { 'h' }, 2.0);
        haul_icon(over, race.course, 8, 24);
        over.text(16, 23, &format!("{}", me.haul), 'h');
    }
    let secs = format!("{}", race.left());
    big(over, w - 22, 4, &secs, if race.left() <= 10 { '7' } else { 'h' }, 2.0);
    if race.titled() {
        title(over, race);
        return;
    }
    // The two keys, small and dim along the bottom.
    let [(k1, w1), (k2, w2)] = race.course.keys();
    let hint = format!("{k1} {w1}   {k2} {w2}");
    let hw = text_width(&hint);
    super::super::chrome::panel(over, ((w - hw) / 2 - 4, h - 12, hw + 8, 11), super::super::chrome::Tone::SLATE);
    over.text((w - hw) / 2, h - 10, &hint, 'J');
    if race.t < TITLE + HZ && me.is_none_or(|r| r.crash == 0) {
        big(over, w / 2, h / 2 - 20, "GO!", '5', 4.0);
    }
    if let Some(me) = me {
        if me.crash > 0 && me.wreck != Wreck::None {
            let words = crash_words(me.wreck, me.crashes);
            big(over, w / 2, 44, words, '9', 2.0);
        } else if let Some(_) = me.finish {
            let place = race.place(seat.unwrap());
            big(over, w / 2, 44, &format!("{} HOME", place_word(place)), '5', 2.0);
        } else if me.burst > 0 && race.course == Course::Griffin {
            over.text(w / 2 - 14, 70, "BURST", '4');
        }
    }
}

/// The title card: the race's name, its words, its keys, and the count.
fn title(over: &mut Img, race: &Race3d) {
    let (w, h) = (over.w, over.h);
    let (pw, ph) = (236, 104);
    let (px, py) = ((w - pw) / 2, (h - ph) / 2 - 8);
    plate(over, px, py, pw, ph);
    big(over, w / 2, py + 8, race.course.name(), '5', 2.0);
    if race.practice {
        over.text((w - text_width("PRACTICE")) / 2, py + ph - 10, "PRACTICE", 'J');
    }
    big(over, w / 2, py + 30, race.course.words(), 'h', 1.0);
    let [(k1, w1), (k2, w2)] = race.course.keys();
    let keys = format!("{k1} {w1}    {k2} {w2}");
    over.text((w - text_width(&keys)) / 2, py + 42, &keys, 'J');
    let count = (TITLE - race.t).div_ceil(HZ).max(1);
    big(over, w / 2, py + 56, &count.to_string(), '9', 5.0);
}

/// The finish board, the same for everyone: who came home where, in what
/// time, what they picked up, and the gold they took.
fn board(over: &mut Img, run: &Run, race: &Race3d, seat: Option<u32>) {
    let w = over.w;
    let order = race.order();
    let (pw, ph) = (280, 52 + order.len() as i32 * 20);
    let (px, py) = ((w - pw) / 2, 30);
    plate(over, px, py, pw, ph);
    big(over, w / 2, py + 7, if race.practice { "PRACTICE FLIGHT" } else { race.course.board() }, '5', 1.0);
    big(over, w / 2, py + 18, race.course.name(), 'J', 1.0);
    for (k, &id) in order.iter().enumerate() {
        let r = &race.riders[&id];
        let y = py + 36 + k as i32 * 20;
        if Some(id) == seat {
            super::super::chrome::highlight(over, (px + 4, y - 3, pw - 8, 18));
        }
        let place = place_word(k + 1);
        over.text(px + 10, y + 3, &place, if k == 0 { '5' } else { 'h' });
        let face = run.players.get(&id).map_or_else(|| sprites::knight(id), |h| kn::portrait(h, id));
        over.stamp(&face, px + 44 - face.w / 2, y + 14 - face.h);
        let name = run
            .players
            .get(&id)
            .map_or_else(|| format!("KNIGHT {id}"), |h| h.name.to_uppercase());
        let name: String = name.chars().take(12).collect();
        over.text(px + 56, y + 3, &name, seat_ink(id));
        let time = r.finish.map_or_else(|| "OUT".to_string(), |f| format!("{}S", (f - TITLE).div_ceil(HZ)));
        over.text(px + 140, y + 3, &time, 'h');
        haul_icon(over, race.course, px + 172, y + 4);
        over.text(px + 180, y + 3, &format!("{}", r.haul), 'h');
        let gold = race.won.get(&id).copied().unwrap_or(0);
        if !race.practice {
            over.text(px + 204, y + 3, &format!("+{gold}G"), '4');
        }
        if k == 0 && race.riders.len() > 1 && !race.practice {
            laurel(over, px + pw - 28, y - 1);
        }
    }
}

// ─── the Griffin Eyrie ──────────────────────────────────────────────────────

/// The Eyrie's yard: crags either side, the perch between them with its
/// great nest of branches, a brazier by the master's post, and the gorge's
/// edge falling away at the top.
pub(super) fn eyrie_scenery(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    for &(c, r, w, h) in &r3::EYRIE_ROCKS[..2] {
        let (px, py, pw, ph) = (c * TILE, r * TILE, w * TILE, h * TILE);
        let rock = kit::blob(
            pw,
            ph + 6,
            &[
                (pw as f32 * 0.3, ph as f32 * 0.6, pw as f32 * 0.32, ph as f32 * 0.5),
                (pw as f32 * 0.65, ph as f32 * 0.5, pw as f32 * 0.36, ph as f32 * 0.55),
            ],
            &['k', 'x', 'g', 'j', 'G'],
            0.4,
            31 + c as u32,
            1.5,
        );
        cv.stamp(&rock, px, py);
    }
    // The perch: a stone stump crowned with a nest.
    let (c, r, w, h) = r3::EYRIE_ROCKS[2];
    let (px, py, pw, ph) = (c * TILE, r * TILE, w * TILE, h * TILE);
    let stump = kit::blob(pw, ph, &[(pw as f32 * 0.5, ph as f32 * 0.6, pw as f32 * 0.42, ph as f32 * 0.5)], &['k', 'x', 'g', 'j'], 0.4, 37, 1.5);
    cv.stamp(&stump, px, py);
    cv.ellipse((px + pw / 2) as f32, (py + 7) as f32, pw as f32 * 0.42, 5.0, 'b');
    for k in 0..14 {
        let x = px + 6 + k * 4;
        cv.line(x, py + 4 + k % 2 * 2, x + 5, py + 8 - k % 2 * 2, if k % 3 == 0 { 'P' } else { 'I' });
    }
    // The take-off ring before the perch, scuffed into the turf in gold.
    let (rx, ry) = ((r3::PERCH.0 * TILE as f32) as i32, (r3::PERCH.1 * TILE as f32) as i32 + 22);
    for k in 0..48 {
        let a = k as f32 / 48.0 * std::f32::consts::TAU;
        let (x, y) = (rx + (a.cos() * 26.0) as i32, ry + (a.sin() * 11.0) as i32);
        cv.put(x, y, if k % 6 == 0 { '4' } else { 'a' });
    }
    // Feathers about the yard, and the master's brazier.
    for k in 0..12 {
        let (fx, fy) = (40 + (kit_hash(k, 1) % 300) as i32, 90 + (kit_hash(k, 2) % 110) as i32);
        cv.line(fx, fy, fx + 2, fy - 1, if k % 2 == 0 { 'h' } else { 'o' });
    }
    let (c, r, _, _) = r3::EYRIE_ROCKS[3];
    let brazier = kit::brazier(tick, 5);
    cv.stamp(&brazier, c * TILE + 40, r * TILE - 4);
    lights.push(fire(c * TILE + 44, r * TILE, 52.0, 0.4));
}

fn kit_hash(k: i32, s: u32) -> u32 {
    super::super::ink::hash(k, k * 3, s)
}

/// The Eyrie's figures: a griffin on the nest (two of them, preening),
/// the griffin master at the post, and the word by the perch.
pub(super) fn eyrie_figures(over: &mut Img, run: &Run) {
    if run.race3d_showing() {
        return;
    }
    let (px, py) = ((r3::PERCH.0 * TILE as f32) as i32, (r3::PERCH.1 * TILE as f32) as i32);
    let beat = (run.tick / 10 % 4) as i32;
    for (k, dx) in [(0, -26), (1, 18)] {
        let (gx, gy) = (px + dx, py - 34 + (k * 3));
        // Tawny body, white head, gold beak, wings stirring.
        over.rect(gx, gy, 12, 7, 'o');
        over.rect(gx + 1, gy + 1, 10, 2, 'O');
        over.rect(gx + 2, gy + 7, 2, 4, 'r');
        over.rect(gx + 8, gy + 7, 2, 4, 'r');
        let head = if k == 0 { gx + 10 } else { gx - 4 };
        over.rect(head, gy - 5, 6, 6, 'i');
        over.rect(head + 1, gy - 4, 4, 2, 'H');
        over.put(if k == 0 { head + 6 } else { head - 1 }, gy - 3, '5');
        over.put(if k == 0 { head + 4 } else { head + 1 }, gy - 4, 'k');
        let lift = [0, 2, 4, 2][beat as usize];
        for w in 0..9 {
            let wx = if k == 0 { gx + 2 - w } else { gx + 10 + w };
            over.line(wx, gy - lift + w / 3, wx, gy + 3, if w % 3 == 0 { 'R' } else { 'r' });
        }
        over.line(gx + if k == 0 { -2 } else { 13 }, gy + 2, gx + if k == 0 { -6 } else { 17 }, gy + 6, 'o');
    }
    // The griffin master, a leather-aproned figure with a hood.
    let (mx, my) = (px + 34, py + 2);
    over.rect(mx, my - 14, 7, 6, 'B');
    over.rect(mx + 1, my - 12, 5, 3, 't');
    over.rect(mx - 1, my - 8, 9, 8, 'b');
    over.rect(mx + 1, my - 7, 5, 6, 'P');
    over.put(mx + 2, my - 11, 'k');
    over.put(mx + 4, my - 11, 'k');
    super::near_prompt(over, run, (px, py + 8), "FLY");
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__race3d__tests.rs"]
mod tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../../../../tests/render/arena__race3d__fixtures.rs"]
pub mod testing;
