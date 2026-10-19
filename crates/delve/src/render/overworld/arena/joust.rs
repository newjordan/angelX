//! The stables and the lists, drawn: the realm's three mounts in their
//! stalls and the saddled one in the yard; at the lists, the crowd on the
//! stands and, while a bout is ridden, the riders at the gallop down the
//! tilt, the timing ring closing where they will meet, the lances breaking,
//! and a rider going over his horse's tail.
//!
//! The destrier is drawn from shapes (barrel, chest, rump, an arched neck,
//! jointed legs that move with each frame of the gallop), so one drawing
//! serves every coat, caparison and stride.

use super::super::chrome;
use super::super::ink::{Img, hash};
use super::{TILE, at, home, sprites};
use crate::realm::chivalry::Mount;
use crate::rules::Run;
use crate::rules::joust::{
    self, Aim, GROOM_AT, Hit, Joust, LED_AT, MARKET_PLATES, MOUNT_PLATE, RIVALS, STALL_PLATES, Stage,
    TEND_PLATE,
};
use crate::realm::Spoil;
use crate::rules::world::{STANDS, TILT};

/// A horse's hide, its shade, and its mane and tail.
#[derive(Clone, Copy)]
pub(super) struct Coat {
    pub(super) hide: char,
    pub(super) dark: char,
    pub(super) mane: char,
}

pub(super) fn coat(m: Mount) -> Coat {
    match m {
        // A bay with a black mane; a dark grey with a rust mane; a grey.
        Mount::Bramble => Coat {
            hide: 'R',
            dark: 'r',
            mane: 'n',
        },
        Mount::Cinder => Coat {
            hide: 'X',
            dark: 'g',
            mane: 'p',
        },
        Mount::Mist => Coat {
            hide: 'i',
            dark: 'h',
            mane: 'H',
        },
    }
}

/// The scale the destrier is drawn at, against its 52 by 46 design.
const K: f32 = 1.0;
const W: i32 = 52;
const H: i32 = 46;

/// Each leg's (upper, lower) angle from straight down in degrees, forward
/// positive, for the four frames of the gallop: near fore, far fore, near
/// hind, far hind.
const GAIT: [[(f32, f32); 4]; 4] = [
    [(35.0, 10.0), (20.0, -40.0), (-30.0, 30.0), (-10.0, 60.0)],
    [(10.0, -45.0), (40.0, 20.0), (-40.0, 10.0), (-25.0, 40.0)],
    [(-15.0, -60.0), (10.0, -20.0), (20.0, 50.0), (-35.0, 15.0)],
    [(25.0, -10.0), (-5.0, -50.0), (0.0, 70.0), (10.0, 40.0)],
];
/// Standing: four legs under the horse.
const STAND: [(f32, f32); 4] = [(4.0, 0.0), (-4.0, 0.0), (-4.0, 6.0), (4.0, 4.0)];

fn thick(im: &mut Img, (x0, y0): (f32, f32), (x1, y1): (f32, f32), ink: char, w: i32) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0) as i32 + 1;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
        for ox in 0..w {
            for oy in 0..w {
                im.put(
                    (x + ox as f32 - (w - 1) as f32 / 2.0).floor() as i32,
                    (y + oy as f32 - (w - 1) as f32 / 2.0).floor() as i32,
                    ink,
                );
            }
        }
    }
}

/// How high the gallop carries the body this frame (pixels, design scale).
fn bob(gait: Option<u32>) -> f32 {
    gait.map_or(0.0, |f| [0.0, -1.0, 0.0, 1.0][(f % 4) as usize])
}

/// A destrier side-on, facing east: at the gallop (`gait` its frame) or
/// standing, barded in a caparison of (cloth, trim) or bare with a saddle.
pub(super) fn destrier(c: Coat, gait: Option<u32>, barding: Option<(char, char)>) -> Img {
    let mut im = Img::new(W, H);
    let p = |x: f32, y: f32| (x * K, y * K);
    let legs = gait.map_or(STAND, |f| GAIT[(f % 4) as usize]);
    let by = 29.0 + bob(gait);
    let leg = |im: &mut Img, x: f32, (up, lo): (f32, f32), ink: char, near: bool| {
        let (a, b) = (up.to_radians(), (up + lo).to_radians());
        let (kx, ky) = (x + a.sin() * 7.0, by + a.cos() * 7.0);
        let (fx, fy) = (kx + b.sin() * 7.0, ky + b.cos() * 7.0);
        thick(im, p(x, by), p(kx, ky), ink, if near { 3 } else { 2 });
        thick(im, p(kx, ky), p(fx, fy), ink, 2);
        let (hx, hy) = p(fx, fy);
        im.put(hx as i32 - 1, hy as i32, 'k');
        im.put(hx as i32, hy as i32, 'k');
    };
    leg(&mut im, 33.0, legs[1], c.dark, false);
    leg(&mut im, 13.0, legs[3], c.dark, false);
    leg(&mut im, 31.0, legs[0], c.hide, true);
    leg(&mut im, 11.0, legs[2], c.hide, true);
    // The tail: streaming back at the gallop, hanging when it stands.
    for i in 0..9 {
        let t = i as f32 / 8.0;
        let (x, y) = if gait.is_some() {
            (5.0 - t * 5.0, by - 6.0 + t * 4.0)
        } else {
            (5.0 - t * 1.5, by - 6.0 + t * 9.0)
        };
        thick(&mut im, p(x, y), p(x, y + 2.0), c.mane, 2);
    }
    // The body: rump, barrel and chest.
    let mut body = Img::new(W, H);
    for (cx, cy, rx, ry) in [
        (11.0, by - 6.0, 6.5, 6.5),
        (22.0, by - 5.0, 12.0, 6.0),
        (32.0, by - 6.0, 6.5, 6.0),
    ] {
        let (x, y) = p(cx, cy);
        body.ellipse(x, y, rx * K, ry * K, c.hide);
    }
    match barding {
        // The caparison: the body, hung to just above the knees, its hem
        // scalloped and trimmed.
        Some((cloth, trim)) => {
            for x in 0..W {
                let Some(top) = (0..H).find(|&y| body.solid(x, y)) else {
                    continue;
                };
                let hem = (by + 2.0) * K - if (x / 3) % 2 == 1 { 1.0 } else { 0.0 };
                for y in top..=hem as i32 {
                    let edge = y >= hem as i32 - 1 || y == top;
                    im.put(x, y, if edge { trim } else { cloth });
                }
            }
        }
        // Bare: the hide, darker underneath, and a saddle.
        None => {
            for y in 0..H {
                for x in 0..W {
                    if body.solid(x, y) {
                        let under = (y as f32) > (by - 3.0) * K;
                        im.put(x, y, if under { c.dark } else { c.hide });
                    }
                }
            }
        }
    }
    // The neck rising forward to the head; the mane along its crest.
    for i in 0..12 {
        let t = i as f32 / 11.0;
        let (lift, reach) = if gait.is_some() {
            (8.0, 7.0)
        } else {
            (10.0, 5.0)
        };
        let (x, y) = p(34.0 + t * reach, by - 9.0 - t * lift);
        im.ellipse(x, y, (3.8 - t * 1.0) * K, (3.6 - t * 0.9) * K, c.hide);
    }
    let (hx, hy) = if gait.is_some() {
        (44.0, by - 17.5)
    } else {
        (42.0, by - 19.5)
    };
    let (x, y) = p(hx, hy);
    im.ellipse(x, y, 4.4 * K, 2.5 * K, c.hide);
    let (x, y) = p(hx + 3.5, hy + 1.9);
    im.ellipse(x, y, 2.4 * K, 1.9 * K, c.hide);
    thick(
        &mut im,
        p(hx - 3.0, hy - 3.5),
        p(hx - 3.0, hy - 5.5),
        c.hide,
        2,
    );
    for i in 0..10 {
        let t = i as f32 / 9.0;
        let (x, y) = p(32.0 + t * (hx - 36.0), by - 11.0 - t * (by - hy - 2.0));
        im.put(x as i32, y as i32, c.mane);
        im.put(x as i32 + 1, y as i32 - 1, c.mane);
    }
    let (x, y) = p(hx + 1.0, hy - 0.5);
    im.put(x as i32, y as i32, 'k');
    if barding.is_none() {
        // The saddle and its cloth.
        let (x, y) = p(17.0, by - 13.0);
        im.rect(x as i32, y as i32, (9.0 * K) as i32, 2, 'B');
        im.rect(x as i32 - 1, y as i32 + 2, (11.0 * K) as i32, 3, '7');
    }
    im.outline_outside('k');
    im
}

/// The colours a knight rides in: their tabard and its charge, and the
/// plume.
#[derive(Clone, Copy)]
pub(super) struct Colours {
    tabard: char,
    field: char,
    plume: char,
    /// A seated knight's class and seat: they ride in its look.
    class: Option<(super::class_looks::Look, u32)>,
}

/// Player `id`'s colours, as their knight wears them on foot.
pub(super) fn colours_of(run: &Run, id: u32) -> Colours {
    let tabard = run
        .players
        .get(&id)
        // A knight of a class rides in the seat's colour, as on foot.
        .filter(|h| super::class_looks::Look::of(h).is_none())
        .and_then(|h| {
            h.colours()
                .iter()
                .find(|(from, _)| *from == '7')
                .map(|p| p.1)
        })
        .unwrap_or(super::knights::seat_cloth(id));
    Colours {
        tabard,
        field: if tabard == '4' { 'a' } else { '4' },
        plume: tabard,
        class: run.players.get(&id).and_then(super::class_looks::Look::of).map(|l| (l, id)),
    }
}

fn rival_colours(rival: usize) -> Colours {
    let (tabard, field) = RIVALS[rival.min(RIVALS.len() - 1)].colours;
    Colours {
        tabard,
        field,
        plume: field,
        class: None,
    }
}

/// The knight in the saddle: helm and plume, tabard, shield (high by the
/// helm or low at the waist), and the lance (couched toward the far lane,
/// aimed high or low; a stump once it has broken).
fn rider(
    im: &mut Img,
    gait: Option<u32>,
    col: Colours,
    high: bool,
    shield_high: bool,
    broken: bool,
    crowned: bool,
) {
    let by = 29.0 + bob(gait);
    let (sx, sy) = ((21.0 * K) as i32, ((by - 13.0) * K) as i32);
    // Torso, then helm.
    for y in sy - 7..=sy {
        for x in sx - 2..sx + 3 {
            im.put(
                x,
                y,
                if (x + y) % 4 == 0 {
                    col.field
                } else {
                    col.tabard
                },
            );
        }
    }
    // The helm in the hero's steel: lit along its crown, navy at its back.
    for y in sy - 12..sy - 7 {
        for x in sx - 2..sx + 3 {
            let ink = if y == sy - 12 {
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
    im.put(sx + 1, sy - 10, 's');
    im.put(sx + 2, sy - 10, 's');
    if !crowned {
        im.line(sx - 1, sy - 13, sx - 3, sy - 15, col.plume);
        im.line(sx, sy - 13, sx - 2, sy - 15, col.plume);
    }
    if let Some((look, seat)) = col.class {
        super::class_looks::dress_rider(im, (sx, sy), look, seat);
    }
    if crowned {
        // The winner's laurel crown on the helm, in place of the plume.
        for (x, ink) in (sx - 2..sx + 3).zip(['5', '4', '5', '4', '5']) {
            im.put(x, sy - 13, ink);
        }
        for x in [sx - 2, sx, sx + 2] {
            im.put(x, sy - 14, '6');
        }
    }
    // The leg in the stirrup.
    im.line(sx, sy + 1, sx + 1, sy + 5, 'v');
    // The lance under the arm, out past the horse's head.
    let reach = if broken { 9 } else { 26 };
    let tip = (sx + reach, sy - if high { 9 } else { 5 });
    for k in 0..2 {
        im.line(sx - 5, sy - 3 + k, tip.0, tip.1 + k, 'T');
    }
    if broken {
        im.put(tip.0, tip.1 - 1, 'o');
        im.put(tip.0 + 1, tip.1 + 1, 'o');
    } else {
        im.put(tip.0, tip.1, 'W');
        im.put(tip.0 + 1, tip.1, 'W');
    }
    // The shield before him, rimmed in steel: high by the helm, or low at
    // the waist. Where it is is where he guards.
    let top = if shield_high { sy - 13 } else { sy - 5 };
    for y in top..top + 8 {
        for x in sx + 2..sx + 8 {
            let point = y >= top + 6 && (x == sx + 2 || x == sx + 7);
            if point {
                continue;
            }
            let rim = y == top || x == sx + 2 || x == sx + 7 || (y == top + 7);
            let charge = x == sx + 4 || x == sx + 5 || y == top + 3;
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

/// A knight thrown from the saddle: helm (or, once it has come off, a
/// bare head), tabard, arms flung out, legs in the air; it turns over as it
/// falls (`age` ticks after the impact) and lies on its back on the sand.
fn thrown(col: Colours, age: u32, helm: bool, kick: bool) -> Img {
    let mut im = Img::new(14, 18);
    if helm {
        im.rect(4, 0, 6, 6, 'V');
        im.line(4, 0, 9, 0, 'W');
        im.line(9, 1, 9, 5, 'u');
        im.line(6, 3, 8, 3, 's');
    } else {
        im.rect(5, 1, 4, 5, 'O');
        im.line(5, 1, 8, 1, 'r');
        im.put(6, 3, 'k');
        im.put(8, 3, 'k');
        im.line(6, 5, 7, 5, 'p');
    }
    for y in 6..13 {
        for x in 4..10 {
            im.put(
                x,
                y,
                if (x + y) % 4 == 0 {
                    col.field
                } else {
                    col.tabard
                },
            );
        }
    }
    // Arms flung out, legs kicking.
    im.line(3, 7, 0, 4, 'v');
    im.line(10, 7, 13, 4, 'v');
    if kick {
        im.line(5, 13, 1, 15, 'v');
        im.line(8, 13, 12, 15, 'v');
    } else {
        im.line(5, 13, 3, 17, 'v');
        im.line(8, 13, 10, 17, 'v');
    }
    match age {
        0..=5 => im,
        6..=10 => sprites::quarter_turn(&im),
        11..=15 => sprites::quarter_turn(&sprites::quarter_turn(&im)),
        _ => sprites::quarter_turn(&sprites::quarter_turn(&sprites::quarter_turn(&im))),
    }
}

/// When the helm comes off in a fall, in ticks after the impact.
const HELM_OFF: u32 = 8;

/// The fall: from the saddle at the meeting, flung back along the lane, up
/// and over and down onto the sand. The helm comes off at the top of the
/// arc and bounces away on its own, the dust settles, and little birds go
/// round and round over the knight on his back.
fn fall(cv: &mut Img, col: Colours, (lane_x, lane_y): (i32, i32), back: i32, age: u32, tick: u32) {
    let a = age.min(22) as f32;
    // A short arc back from the meeting: up, then down to the lane.
    let x = lane_x + back * (a * 1.6) as i32;
    let lift = (a * (22.0 - a) / 4.0) as i32;
    let y = lane_y - 6 - lift;
    // On his back, the legs still kicking at the air.
    let kick = age >= 22 && (tick / 4).is_multiple_of(2);
    let body = thrown(col, age, age < HELM_OFF, kick);
    cv.stamp(&body, x - body.w / 2, y - body.h);
    if age >= HELM_OFF {
        // The helm: on along the arc, two bounces, then it rolls to a stop
        // a little way off, the visor turning over as it goes.
        let t = (age - HELM_OFF).min(60) as i32;
        let x0 = lane_x + back * (HELM_OFF as f32 * 1.6) as i32;
        let hx = x0 + back * (t * 2).min(30) + back * ((t - 15).clamp(0, 10) / 2);
        let ground = lane_y - 9;
        let hy = match t {
            0..=9 => ground - 18 - (t * (9 - t)) / 2 + t * 2,
            10..=15 => ground - ((t - 10) * (15 - t)) / 2,
            _ => ground,
        };
        let turn = if t < 25 { (t / 2) % 4 } else { 1 };
        cv.rect(hx - 2, hy - 2, 5, 5, 'V');
        cv.line(hx - 2, hy - 2, hx + 2, hy - 2, 'W');
        cv.line(hx + 2, hy - 1, hx + 2, hy + 2, 'u');
        let slit = [(0, -1), (1, 0), (0, 1), (-1, 0)][turn as usize];
        cv.put(hx + slit.0, hy + slit.1, 's');
        cv.put(hx + slit.0 * 2, hy + slit.1, 's');
    }
    if (20..44).contains(&age) {
        // The thump: dust thrown out round him, settling.
        let d = (age - 20) as i32;
        for k in 0..16 {
            let ang = k as f32 / 16.0 * std::f32::consts::TAU;
            let r = 6.0 + d as f32 * 0.9;
            let (dx, dy) = ((ang.cos() * r) as i32, (ang.sin() * r * 0.4) as i32);
            if (k + d) % 3 != 0 {
                cv.put(x + dx, lane_y - 7 + dy - d / 6, if k % 2 == 0 { 'J' } else { 'h' });
            }
        }
    }
    if age >= 24 {
        // Three little birds, round and round over his head, flapping.
        let (bx, by) = (x, y - body.h - 6);
        for i in 0..3u32 {
            let ang = tick as f32 * 0.2 + i as f32 * std::f32::consts::TAU / 3.0;
            let (px, py) = (bx + (ang.cos() * 11.0) as i32, by + (ang.sin() * 4.0) as i32);
            let wing = if (tick / 3 + i).is_multiple_of(2) { -1 } else { 1 };
            for (dx, dy) in [(-2, wing), (-1, 0), (1, 0), (2, wing)] {
                cv.put(px + dx, py + dy, 'T');
            }
            cv.put(px, py, 'O');
        }
    }
}

// ── The stables ───────────────────────────────────────────────────────────

/// A horse's head and neck over its stall's half-door, ears flicking.
fn stall_head(c: Coat, tick: u32, seed: u32) -> Img {
    let mut im = Img::new(16, 16);
    // Neck rising out of the dark, the head turned out toward the yard.
    for y in 6..16 {
        for x in 4..10 {
            im.put(x, y, if x == 4 { c.dark } else { c.hide });
        }
    }
    im.ellipse(9.0, 5.0, 5.0, 3.0, c.hide);
    im.ellipse(12.5, 7.0, 2.6, 2.2, c.hide);
    for y in 4..14 {
        im.put(5, y, c.mane);
    }
    let flick = (tick / 20 + seed).is_multiple_of(5);
    im.line(6, 2, 6, if flick { 0 } else { 1 }, c.hide);
    im.line(8, 2, 8, 0, c.hide);
    im.put(10, 4, 'k');
    im.put(14, 7, 'k');
    // The white blaze down Bramble's and Cinder's faces.
    if c.hide != 'i' {
        im.line(11, 3, 13, 7, 'H');
    }
    im.outline_outside('k');
    // The half-door across the bottom.
    im.rect(0, 12, 16, 4, 'B');
    im.line(0, 12, 15, 12, 'o');
    im.line(0, 14, 15, 14, 'r');
    im
}

/// The groom: a stable hand in a leather jerkin, a brush in hand.
fn groom(brushing: bool, tick: u32) -> Img {
    let up = brushing && (tick / 6).is_multiple_of(2);
    Img::from_rows(&[
        "....rRRr.....",
        "...rRRRRr....",
        "...OOOOOO....",
        "...OKOOKO....",
        "....OOOO.....",
        "...pBBBBp....",
        "..pBBBBBBp...",
        "..OBBBBBBO...",
        if up {
            "...BBBBBBOtt."
        } else {
            "...BBBBBB....."
        },
        if up { "...PPPPPP...." } else { "...PPPPPPOtt." },
        "...PPPPPP....",
        "....nn.nn....",
        "...nnn.nnn...",
    ])
}

/// The stables' figures: the mounts over their half-doors, the saddled
/// one out in the yard, the groom, and the plates.
pub(super) fn stables(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let stable = &run.home.stable;
    for (i, m) in Mount::ALL.into_iter().enumerate() {
        let (dx, dw) = super::world::stall_doors()[i];
        if m != stable.selected {
            let head = stall_head(coat(m), tick, i as u32);
            cv.stamp(&head, dx + (dw - head.w) / 2, 2 * TILE + 8);
        } else {
            // An empty stall: straw on its floor.
            for k in 0..6 {
                cv.put(dx + 4 + k * 3, 4 * TILE - 3, 'o');
            }
        }
        let lit = run
            .players
            .values()
            .any(|h| h.hp > 0 && inside(STALL_PLATES[i], h.x, h.y));
        home::plate_marked(cv, STALL_PLATES[i], lit, &horseshoe());
    }
    // The saddled mount in the yard, by the trough; tended, it gleams and
    // wears a ribbon.
    let tended = stable.is_tended(stable.selected);
    let mut horse = destrier(coat(stable.selected), None, None);
    if tended {
        let c = coat(stable.selected);
        let gleam = match c.hide {
            'R' => 'o',
            'X' => 'G',
            _ => 'W',
        };
        for y in 0..horse.h {
            for x in 0..horse.w {
                if horse.get(x, y) == super::super::ink::ink(c.hide)
                    && (x + y + (tick / 8) as i32).rem_euclid(9) == 0
                {
                    horse.put(x, y, gleam);
                }
            }
        }
        horse.put(33, 9, '7');
        horse.put(34, 8, '7');
    }
    let (lx, ly) = at(LED_AT.0 * 2.0, LED_AT.1 * 2.0);
    cv.stamp(&horse, lx - horse.w / 2, ly - horse.h + 4);
    if tended {
        // A brushed coat catches the light: a glint travels along its back.
        for k in 0..3u32 {
            let phase = (tick / 5 + k * 7) % 21;
            if phase < 3 {
                let x = lx - horse.w / 2 + 10 + (k as i32 * 9) + phase as i32;
                let y = ly - horse.h + 18 + (k as i32 % 2) * 3;
                cv.put(x, y, '6');
                if phase == 1 {
                    cv.put(x - 1, y, '5');
                    cv.put(x + 1, y, '5');
                    cv.put(x, y - 1, '5');
                    cv.put(x, y + 1, '5');
                }
            }
        }
    }
    market(cv, run);
    let (gx, gy) = at(GROOM_AT.0 * 2.0, GROOM_AT.1 * 2.0);
    let g = groom(tended, tick);
    cv.stamp(&g, gx - g.w / 2, gy - g.h + 1);
    let lit = run
        .players
        .values()
        .any(|h| h.hp > 0 && inside(TEND_PLATE, h.x, h.y));
    home::plate_marked(cv, TEND_PLATE, lit, &brush());
}

/// A market horse's coat, from its name: the realm's own three and a black.
fn coat_for(name: &str) -> Coat {
    match hash(name.len() as i32, name.bytes().map(i32::from).sum(), 7) % 4 {
        0 => coat(Mount::Bramble),
        1 => coat(Mount::Cinder),
        2 => coat(Mount::Mist),
        _ => RIVAL_COAT,
    }
}

/// The horse market: today's four in their pens before the rails, each
/// with its plate, a price tag, or SOLD.
fn market(cv: &mut Img, run: &Run) {
    let stable = &run.home.stable;
    let gold = run.treasury.get(Spoil::Gold);
    for (i, horse) in stable.todays_market().iter().enumerate().take(MARKET_PLATES.len()) {
        let plate = MARKET_PLATES[i];
        let (c, r, w, _) = plate;
        let (mx, foot) = (c * TILE + w * TILE / 2, r * TILE + 2);
        let state = joust::market_open(stable, gold, &run.marks, i);
        if state != Err("SOLD") {
            let im = destrier(coat_for(&horse.name), None, None);
            cv.stamp(&im, mx - im.w / 2, foot - im.h + 2);
        }
        let lit = run.players.values().any(|h| h.hp > 0 && inside(plate, h.x, h.y));
        home::plate_marked(cv, plate, lit, &horseshoe());
        let tag = match state {
            Ok(price) => format!("{price}G"),
            Err(why) => why.split(' ').next().unwrap_or(why).to_string(),
        };
        let tw = super::super::ink::text_width(&tag);
        let tone = if state.is_ok() { chrome::Tone::KEY } else { chrome::Tone::SLATE };
        chrome::label(cv, (mx - tw / 2, foot - 50), &tag, if state.is_ok() { 'O' } else { 'h' }, tone);
    }
}

fn horseshoe() -> Img {
    Img::from_rows(&[".OOO.", "O...O", "O...O", "O...O", "O...O"])
}

fn brush() -> Img {
    Img::from_rows(&["OOOOO", "OOOOO", "O.O.O", "O.O.O"])
}

fn inside((c, r, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (cc, rr) = (x / 2.0, y / 2.0);
    cc >= c as f32 && cc < (c + w) as f32 && rr >= r as f32 && rr < (r + h) as f32
}

/// The stables' boards: a stall's mount, or the trough, to a knight on its
/// plate.
pub(super) fn stables_boards(cv: &mut Img, run: &Run) {
    let stable = &run.home.stable;
    for hero in run.players.values().filter(|h| h.hp > 0) {
        if let Some(i) = STALL_PLATES.iter().position(|&p| inside(p, hero.x, hero.y)) {
            let m = Mount::ALL[i];
            let last = if stable.selected == m {
                ("SADDLED FOR THE LISTS".to_string(), '5')
            } else {
                hold_line(hero.buying, "HOLD E: SADDLE")
            };
            let board = home::board(&[
                (m.name().to_uppercase(), '9'),
                (m.says().to_uppercase(), 'h'),
                last,
            ]);
            // Over the stable's roof, clear of the yard and the mount in it.
            let (c, _, w, _) = STALL_PLATES[i];
            let x = (c * TILE + w * TILE / 2 - board.w / 2).clamp(2, cv.w - board.w - 2);
            cv.stamp(&board, x, 4);
            return;
        }
        if let Some(i) = MARKET_PLATES.iter().position(|&p| inside(p, hero.x, hero.y)) {
            let Some(horse) = stable.todays_market().into_iter().nth(i) else {
                continue;
            };
            let last = match joust::market_open(stable, run.treasury.get(Spoil::Gold), &run.marks, i) {
                Ok(price) => hold_line(hero.buying, &format!("HOLD E: BUY {price}G")),
                Err(why) => (why.to_string(), 'h'),
            };
            let board = home::board(&[
                (horse.name.to_uppercase(), '9'),
                (if horse.mare { "A MARE" } else { "A STALLION" }.to_string(), 'h'),
                last,
            ]);
            let (c, _, w, _) = MARKET_PLATES[i];
            let x = (c * TILE + w * TILE / 2 - board.w / 2).clamp(2, cv.w - board.w - 2);
            cv.stamp(&board, x, 4);
            return;
        }
        if inside(TEND_PLATE, hero.x, hero.y) {
            let m = stable.selected;
            let last = if stable.is_tended(m) {
                ("TENDED: A KNOCK MORE, ONE BOUT".to_string(), '5')
            } else {
                hold_line(hero.buying, "HOLD E: TEND")
            };
            let board = home::board(&[
                (format!("TEND {}", m.name().to_uppercase()), '9'),
                ("BRUSH, WATER, CHECK THE TACK".to_string(), 'h'),
                last,
            ]);
            let (c, r, _, h) = TEND_PLATE;
            cv.stamp(&board, (c * TILE).max(2), (r + h) * TILE + 4);
            return;
        }
    }
}

fn hold_line(buying: u32, label: &str) -> (String, char) {
    use crate::rules::home::{BUY_HOLD, hold_bar};
    (format!("{label} {}", hold_bar(buying, BUY_HOLD)), 'O')
}

// ── The lists ─────────────────────────────────────────────────────────────

/// The crowd on the stands: two rows of the realm's folk, cheering when a
/// lance lands.
pub(super) fn crowd(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let cheer = run.joust.as_ref().is_some_and(|j| {
        (j.stage == Stage::Charge
            && j.hits
                .is_some_and(|(a, b)| a != Hit::Miss || b != Hit::Miss))
            || j.verdict.is_some()
    });
    let (c, r, w, _) = STANDS;
    let skins = ['O', 'o', 't', 'P'];
    let cloth = ['B', 'u', 'p', 'E', 'X', 'r', 'S'];
    for row in 0..2 {
        let y = r * TILE + 15 + row * 8;
        let mut x = c * TILE + 3 + row * 3;
        let mut k = 0;
        while x < (c + w) * TILE - 4 {
            // Leave the caravan entrance's six-tile gap empty, including a
            // spectator's cheering hands. The benches have the same gap.
            if run.room().doors[0] && x + 4 >= 9 * TILE && x - 1 < 15 * TILE {
                x += 7;
                k += 1;
                continue;
            }
            let h = hash(k, row, 211);
            let jump = if cheer && (tick / 4 + h).is_multiple_of(2) {
                2
            } else if (tick / 30 + h).is_multiple_of(11) {
                1
            } else {
                0
            };
            let skin = skins[(h % 4) as usize];
            let body = cloth[((h >> 3) % 7) as usize];
            spectator(cv, x, y - jump, h, skin, body, cheer && jump == 2);
            x += 6 + (h % 3) as i32;
            k += 1;
        }
    }
}

/// One of the crowd, five pixels wide and six tall with its feet on row
/// `y + 6`: a round head under its own hair, hood or cap, rounded shoulders
/// in its cloth lit on the left, and both arms up while it cheers. `h`
/// picks the look, so the stands hold folk and not a row of stamps.
pub(super) fn spectator(cv: &mut Img, x: i32, y: i32, h: u32, skin: char, body: char, cheering: bool) {
    let hair = ['n', 'b', 'I', 'g', 'J', 'R'][((h >> 6) % 6) as usize];
    let hat = (h >> 9) % 5;
    let lit = match body {
        'B' => 'P',
        'u' => 'U',
        'p' => 'r',
        'E' => 'l',
        'X' => 'g',
        'r' => 'R',
        _ => 'u',
    };
    // A shorter one now and then: a squire, a child.
    let y = if (h >> 12) % 4 == 0 { y + 1 } else { y };
    // The head: rounded, its crown in hair or hat.
    let crown = match hat {
        0 => body,
        1 => '7',
        2 => 'S',
        _ => hair,
    };
    cv.put(x + 1, y, crown);
    cv.put(x + 2, y, crown);
    cv.put(x + 3, y, crown);
    if hat == 0 {
        // A hood: the cloth comes down either side of the face.
        cv.put(x + 1, y + 1, body);
        cv.put(x + 3, y + 1, body);
        cv.put(x + 2, y + 1, skin);
    } else {
        cv.put(x + 1, y + 1, skin);
        cv.put(x + 2, y + 1, skin);
        cv.put(x + 3, y + 1, skin);
    }
    cv.put(x + 2, y + 2, skin);
    // Shoulders, rounded, lit down the left.
    for dx in 1..4 {
        cv.put(x + dx, y + 3, if dx == 1 { lit } else { body });
    }
    for dx in 0..5 {
        cv.put(x + dx, y + 4, if dx == 0 { lit } else { body });
    }
    if cheering {
        cv.put(x, y + 1, skin);
        cv.put(x + 4, y + 1, skin);
        cv.put(x, y + 2, body);
        cv.put(x + 4, y + 2, body);
    }
}

/// The tilt: a painted barrier down the middle of the lists, on posts, its
/// boards striped in the realm's red and timber.
fn tilt(cv: &mut Img) {
    let (c, r, w, _) = TILT;
    let (x0, x1) = (c * TILE, (c + w) * TILE);
    let y = r * TILE + 4;
    for x in x0..x1 {
        let stripe = (x - x0) / 12 % 2 == 0;
        cv.put(x, y, 'O');
        for dy in 1..5 {
            cv.put(x, y + dy, if stripe { 'p' } else { 'P' });
        }
        cv.put(x, y + 5, 'b');
    }
    for x in (x0..=x1).step_by(24) {
        let x = x.min(x1 - 2);
        cv.rect(x, y - 2, 2, 10, 'I');
        cv.put(x, y - 3, 'o');
        cv.put(x + 1, y - 3, 'o');
    }
}

/// The lists' figures: the tilt, the mount plate, the gallery, and a bout
/// while one is ridden. `legible`: drawn at native size or larger, so its
/// words can be read (a reduced frame leaves the score to the sidebar).
pub(super) fn lists(cv: &mut Img, run: &Run, legible: bool) {
    crowd(cv, run);
    tilt(cv);
    let lit = run.joust.is_none()
        && run
            .players
            .values()
            .any(|h| h.hp > 0 && inside(MOUNT_PLATE, h.x, h.y));
    home::plate_marked(cv, MOUNT_PLATE, lit, &lance_mark());
    if let Some(j) = &run.joust {
        bout(cv, run, j, legible);
        return;
    }
    // The saddled mount, barded, waits beside the mount plate; with a
    // friend at the lists, the next mount waits at the far end in theirs.
    let m = run.home.stable.selected;
    let col = colours_of(run, 1);
    let horse = destrier(coat(m), None, Some((col.tabard, col.field)));
    let (x, y) = at(9.4, joust::SOUTH_LANE + 2.4);
    cv.stamp(&horse, x - horse.w / 2, y - horse.h + 2);
    let challenger = run
        .players
        .iter()
        .find(|(_, h)| h.hp > 0 && inside(MOUNT_PLATE, h.x, h.y))
        .and_then(|(&id, _)| run.challenger(id));
    if let Some(friend) = challenger {
        let col = colours_of(run, friend);
        let next = Mount::ALL[(m.index() + 1) % Mount::ALL.len()];
        let horse = destrier(coat(next), None, Some((col.tabard, col.field))).flip_h();
        let (x, y) = at(joust::EAST_END - 4.0, joust::NORTH_LANE + 0.6);
        cv.stamp(&horse, x - horse.w / 2, y - horse.h + 2);
    }
}

fn lance_mark() -> Img {
    Img::from_rows(&["....O", "...O.", "..O..", "OO...", "OO..."])
}

/// The rider in the far saddle: a friend in their colours on their horse,
/// or the champion in his on a black.
fn far_colours(run: &Run, j: &Joust) -> (Colours, Coat) {
    match &j.foe {
        Some(f) => (colours_of(run, f.id), coat(f.mount)),
        None => (rival_colours(usize::from(j.rival)), RIVAL_COAT),
    }
}

/// A rider's name, as the boards show it: 0 the knight, 1 the other.
fn rider_name(run: &Run, j: &Joust, side: u8) -> String {
    match j.rider(side) {
        Some(id) => run
            .players
            .get(&id)
            .map_or_else(|| format!("KNIGHT {id}"), |h| h.name.to_uppercase()),
        None => j.rival().name.to_uppercase(),
    }
}

/// A horse's gift at the lists, in one word.
fn gift(m: Mount) -> &'static str {
    match m {
        Mount::Bramble => "STEADY",
        Mount::Cinder => "FAST",
        Mount::Mist => "TRUE",
    }
}

/// The 5x7 hand at twice its size, edged in black so it reads over the
/// sand and the riders.
fn big(cv: &mut Img, cx: i32, y: i32, text: &str, ink: char) -> i32 {
    let w = super::super::ink::text_width(text);
    let mut small = Img::new(w + 2, 9);
    for (dx, dy) in [(0, 1), (2, 1), (1, 0), (1, 2), (0, 0), (2, 2), (0, 2), (2, 0)] {
        small.text(dx, dy, text, 'k');
    }
    small.text(1, 1, text, chrome::dusk_mark(ink));
    let im = super::enlarge(&small, 2.0);
    let x = (cx - im.w / 2).clamp(1, (cv.w - im.w - 1).max(1));
    cv.stamp(&im, x, y);
    im.w
}

/// Small words edged in black, centred on `cx`.
fn small(cv: &mut Img, cx: i32, y: i32, text: &str, ink: char) {
    let w = super::super::ink::text_width(text);
    let x = (cx - w / 2).clamp(1, (cv.w - w - 1).max(1));
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        cv.text(x + dx, y + dy, text, 'k');
    }
    cv.text(x, y, text, chrome::dusk_mark(ink));
}

/// A bout: the riders, the ring closing where they meet, what the lances
/// did, who went over, the pass counter and the gallery's wagers.
fn bout(cv: &mut Img, run: &Run, j: &Joust, legible: bool) {
    let ((kx, ky, keast), (rx, ry, reast)) = j.riders();
    let charging = j.stage == Stage::Charge;
    let meeting = j.meeting();
    let met = charging && j.hits.is_some();
    let age = j.since_impact().unwrap_or(0);
    // The gallop's frame; held still through the clash.
    let stride = if j.clashing() { meeting } else { j.t };
    let gait = |seed: u32| charging.then_some((stride / 3 + seed) % 4);
    let me = colours_of(run, j.knight);
    let (him, his_coat) = far_colours(run, j);
    let (mine, theirs) = j.hits.unwrap_or_default();
    // A champion's shield and lance waver until they can be read.
    let waver = (j.t / 6).is_multiple_of(2);
    let shield_high = match j.guard_shown() {
        Some(g) => g == Aim::High,
        None if charging => waver,
        None => true,
    };
    let lance_high = match j.lance_shown() {
        Some(a) => a == Aim::High,
        None if charging => !waver,
        None => false,
    };
    // The far rider: drawn first, behind the tilt. A fallen rider stays
    // down through the verdict.
    let down = met || j.stage == Stage::Done;
    let age = if j.stage == Stage::Done { 99 } else { age };
    let far_fallen = j.fallen.1 && down;
    draw_rider(
        cv,
        (rx, ry, reast),
        gait(1),
        him,
        lance_high,
        shield_high,
        matches!(theirs, Hit::Broke | Hit::Smite) && met,
        (far_fallen, j.winner() == Some(1)),
        (his_coat, (him.tabard, him.field)),
    );
    tilt_front(cv);
    let near_fallen = j.fallen.0 && down;
    draw_rider(
        cv,
        (kx, ky, keast),
        gait(0),
        me,
        j.aim == Aim::High,
        j.shield == Aim::High,
        matches!(mine, Hit::Broke | Hit::Smite) && met,
        (near_fallen, j.winner() == Some(0)),
        (coat(j.mount), (me.tabard, me.field)),
    );
    // The riders who went over, where they fell.
    for (fallen, col, lane, east) in [
        (far_fallen, him, joust::NORTH_LANE, reast),
        (near_fallen, me, joust::SOUTH_LANE, keast),
    ] {
        if fallen {
            let (x, y) = at(j.meeting_x(), lane);
            fall(cv, col, (x, y + 6), if east { -1 } else { 1 }, age, run.tick as u32);
        }
    }
    let (cx, cy) = at(j.meeting_x(), 14.6);
    // The timing ring over the tilt where they will meet, over everything:
    // it closes as they come, and burns gold while a strike would land.
    if let Some(k) = j.closing() {
        let r = 6.0 + (1.0 - k) * 36.0;
        let ink = if j.striking() { '5' } else { 'h' };
        for i in 0..96 {
            let a = i as f32 / 96.0 * std::f32::consts::TAU;
            cv.put(
                cx + (a.cos() * r) as i32,
                cy + (a.sin() * r * 0.6) as i32,
                ink,
            );
        }
        cv.put(cx, cy, '6');
    }
    // What the lances did, where they met.
    if met && age < 30 {
        if matches!(mine, Hit::Broke | Hit::Smite) || matches!(theirs, Hit::Broke | Hit::Smite) {
            // Splinters flying, turning as they go, falling to the sand.
            for s in 0..14 {
                let a = (s as f32 / 14.0) * std::f32::consts::TAU + 0.3;
                let d = 3.0 + age as f32 * (1.0 + (s % 3) as f32 * 0.5);
                let (x, y) = (
                    cx + (a.cos() * d) as i32,
                    cy + (a.sin() * d * 0.7) as i32 + (age * age / 30) as i32,
                );
                let (dx, dy) = [(1, 0), (1, 1), (0, 1), (-1, 1)][((age / 2 + s) % 4) as usize];
                cv.put(x, y, 'T');
                cv.put(x + dx, y + dy, if s % 3 == 0 { 'o' } else { 'T' });
                cv.put(x + 2 * dx, y + 2 * dy, 'o');
            }
        }
        if (mine != Hit::Miss || theirs != Hit::Miss) && age < 12 {
            // The crack of the hit: a star at the meeting, rays thrown out
            // from it, and a ring running after them.
            let r = 4 + age as i32 * 2;
            for i in 0..48 {
                let a = i as f32 / 48.0 * std::f32::consts::TAU;
                cv.put(
                    cx + (a.cos() * r as f32) as i32,
                    cy + (a.sin() * r as f32 * 0.7) as i32,
                    if age < 6 { 'W' } else { 'h' },
                );
            }
            if age < 6 {
                for i in 0..10 {
                    let a = i as f32 / 10.0 * std::f32::consts::TAU + 0.15;
                    let (r0, r1) = (3.0 + age as f32 * 2.0, 12.0 + age as f32 * 3.0);
                    cv.line(
                        cx + (a.cos() * r0) as i32,
                        cy + (a.sin() * r0 * 0.7) as i32,
                        cx + (a.cos() * r1) as i32,
                        cy + (a.sin() * r1 * 0.7) as i32,
                        if i % 2 == 0 { 'T' } else { 'O' },
                    );
                }
                cv.rect(cx - 2, cy - 2, 5, 5, 'T');
                cv.line(cx - 5, cy, cx + 5, cy, 'W');
                cv.line(cx, cy - 4, cx, cy + 4, 'W');
            }
        }
    }
    if !legible {
        return;
    }
    // Under the lists, big: what each lance did, the rider's name before
    // it in their colours; or, when one went over, just that.
    let (_, near) = at(kx, ky);
    let words_y = near + 10;
    if (far_fallen || near_fallen) && age < 90 {
        let side = if far_fallen { 1 } else { 0 };
        let col = if far_fallen { him } else { me };
        small(cv, cx, words_y, &rider_name(run, j, side), col.tabard);
        big(cv, cx, words_y + 9, "UNHORSED!", 'O');
    } else if met && age < 45 {
        let theirs_aim = j.foe.as_ref().map_or(j.lance, |f| f.aim);
        for (row, (side, hit, aim, col)) in [(0u8, mine, j.aim, me), (1, theirs, theirs_aim, him)]
            .into_iter()
            .enumerate()
        {
            let y = words_y + row as i32 * 20;
            let name = rider_name(run, j, side);
            let word = hit.flash(aim);
            let nw = super::super::ink::text_width(&name);
            let ww = super::super::ink::text_width(word) * 2 + 4;
            let x0 = cx - (nw + 6 + ww) / 2;
            small(cv, x0 + nw / 2, y + 6, &name, col.tabard);
            big(cv, x0 + nw + 6 + ww / 2, y, word, if hit == Hit::Miss { 'h' } else { 'O' });
        }
    }
    // The horses' names at the ends, while they are still being picked.
    if j.stage == Stage::Ready && j.course == 0 {
        let far_mount = j.foe.as_ref().map(|f| f.mount);
        for (x, y, m) in [(kx, ky, Some(j.mount)), (rx, ry, far_mount)] {
            let Some(m) = m else {
                continue;
            };
            let (px, py) = at(x, y);
            let ly = if y > joust::MEETING * 0.6 { py + 10 } else { py - 52 };
            small(cv, px, ly, &format!("{} {}", m.name().to_uppercase(), gift(m)), 'T');
        }
    }
    gallery(cv, run, j);
    pass_counter(cv, run, j);
}

/// The near rail of the tilt again, over the far rider's legs: he rides
/// behind it.
fn tilt_front(cv: &mut Img) {
    let (c, r, w, _) = TILT;
    let y = r * TILE + 4;
    for x in c * TILE..(c + w) * TILE {
        if (x - c * TILE) / 12 % 2 == 0 {
            cv.put(x, y + 2, 'p');
        }
    }
}

/// The gallery on the bench: each knight's pennant in the colours of the
/// rider they back, and what they have on it.
fn gallery(cv: &mut Img, run: &Run, j: &Joust) {
    for (&id, w) in &j.gallery {
        let Some(hero) = run.players.get(&id) else {
            continue;
        };
        let (x, y) = at(hero.x, hero.y);
        let top = y - 30;
        let Some(side) = w.side else {
            small(cv, x, top - 2, "?", 'h');
            continue;
        };
        let backed = if side == 0 {
            colours_of(run, j.knight)
        } else {
            far_colours(run, j).0
        };
        // The pole and a pennant streaming from it, fluttering.
        let flap = ((run.tick as u32 / 5 + id).is_multiple_of(2)) as i32;
        // Held up in the knight's hand beside the helm.
        cv.line(x + 7, top, x + 7, top + 22, 'o');
        for k in 0..7 {
            let len = 7 - k;
            let yy = top + k + flap * (k / 4);
            cv.line(x + 8, yy, x + 8 + len + 2, yy, if k == 3 { backed.field } else { backed.tabard });
        }
        let (text, ink) = match w.won {
            Some(true) => (format!("+{}G", w.stake), '5'),
            Some(false) => (format!("-{}G", w.stake), 'h'),
            None => (format!("{}G", w.stake), 'T'),
        };
        small(cv, x + 10, top - 9, &text, ink);
    }
}

/// Over the stands: the pass, big, and under it each rider with the
/// passes they have won, and the balance they have left.
fn pass_counter(cv: &mut Img, run: &Run, j: &Joust) {
    let cx = cv.w / 2;
    let title = if j.stage == Stage::Done {
        "BOUT OVER".to_string()
    } else {
        format!("PASS {} OF 3", j.course + 1)
    };
    let left = rider_name(run, j, 0);
    let right = rider_name(run, j, 1);
    let lw = super::super::ink::text_width(&left);
    let rw = super::super::ink::text_width(&right);
    let half = lw.max(rw) + 18;
    let pw = (half * 2).max(super::super::ink::text_width(&title) * 2 + 10);
    let (px, py, ph) = (cx - pw / 2, 1, 41);
    chrome::board(cv, (px, py, pw, ph), 'B');
    big(cv, cx, py + 2, &title, 'i');
    let y = py + 21;
    let me = colours_of(run, j.knight);
    let him = far_colours(run, j).0;
    cv.text(cx - 16 - lw, y, &left, me.tabard);
    cv.text(cx + 17, y, &right, him.tabard);
    // Passes won: a shield apiece, filled in the winner's colour.
    for k in 0..2u8 {
        let fill = |won: u8, col: Colours| if k < won { col.tabard } else { 'g' };
        shield_pip(cv, cx - 12 + i32::from(k) * 5, y, fill(j.passes.0, me));
        shield_pip(cv, cx + 3 + i32::from(k) * 5, y, fill(j.passes.1, him));
    }
    // Balance left: a pip a knock, under each name.
    for (n, x0, dir) in [(j.balance.0, cx - 18, -1), (j.balance.1, cx + 15, 1)] {
        for k in 0..n.min(10) as i32 {
            cv.rect(x0 + dir * k * 4, y + 10, 3, 3, 'h');
        }
    }
}

fn shield_pip(cv: &mut Img, x: i32, y: i32, ink: char) {
    for (dy, (a, b)) in [(0, (0, 3)), (1, (0, 3)), (2, (0, 3)), (3, (0, 3)), (4, (1, 2)), (5, (1, 2))] {
        for dx in a..=b {
            cv.put(x + dx, y + dy, ink);
        }
    }
}

/// The rival's horse: a black.
const RIVAL_COAT: Coat = Coat {
    hide: 'K',
    dark: 'k',
    mane: 'g',
};

#[allow(clippy::too_many_arguments)]
fn draw_rider(
    cv: &mut Img,
    (x, y, east): (f32, f32, bool),
    gait: Option<u32>,
    col: Colours,
    high: bool,
    shield_high: bool,
    broken: bool,
    (fallen, crowned): (bool, bool),
    (hide, barding): (Coat, (char, char)),
) {
    let mut im = destrier(hide, gait, Some(barding));
    if !fallen {
        rider(&mut im, gait, col, high, shield_high, broken, crowned);
    }
    let im = if east { im } else { im.flip_h() };
    let (px, py) = at(x, y);
    cv.stamp(&im, px - im.w / 2, py - im.h + 6);
}

/// A key on the board: its letter in a little cap.
fn keycap(cv: &mut Img, x: i32, y: i32, key: &str) -> i32 {
    let w = super::super::ink::text_width(key) + 4;
    chrome::keycap(cv, (x, y, w, 11));
    cv.text(x + 2, y + 2, key, 'i');
    w
}

/// A row of key prompts, each a cap or two and a word or two, on a dark
/// strip at the foot of the lists.
fn prompts(cv: &mut Img, items: &[(&[&str], &str)], status: Option<(&str, char)>) {
    let gap = 12;
    let widths: Vec<i32> = items
        .iter()
        .map(|(caps, word)| {
            caps.iter()
                .map(|k| super::super::ink::text_width(k) + 6)
                .sum::<i32>()
                + super::super::ink::text_width(word)
                + 2
        })
        .collect();
    let row = widths.iter().sum::<i32>() + gap * (items.len() as i32 - 1).max(0);
    let sw = status.map_or(0, |(s, _)| super::super::ink::text_width(s));
    let w = row.max(sw) + 12;
    let h = if status.is_some() && !items.is_empty() { 30 } else { 17 };
    let x0 = ((cv.w - w) / 2 - 40).max(2);
    let y0 = cv.h - h - 4;
    chrome::board(cv, (x0, y0, w, h), 'B');
    let mut y = y0 + 3;
    if let Some((s, ink)) = status {
        cv.text(x0 + (w - sw) / 2, y + 2, s, ink);
        y += 13;
    }
    let mut x = x0 + (w - row) / 2;
    for ((caps, word), width) in items.iter().zip(&widths) {
        let mut cx = x;
        for k in caps.iter() {
            cx += keycap(cv, cx, y, k) + 2;
        }
        cv.text(cx, y + 2, word, 'O');
        x += width + gap;
    }
}

/// The boards at the lists: on the mount plate, who you would ride
/// against and for what; in a bout, each knight's own keys for the moment
/// (`focus` is whose screen this is), and the result once it is over.
pub(super) fn lists_boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(j) = &run.joust else {
        plate_board(cv, run);
        return;
    };
    if j.stage == Stage::Done {
        result_board(cv, run, j);
        return;
    }
    let me = focus.unwrap_or(1);
    let side = if me == j.knight {
        Some(0)
    } else if j.foe.as_ref().is_some_and(|f| f.id == me) {
        Some(1)
    } else {
        None
    };
    let lone = j.foe.is_none();
    match (side, j.stage) {
        (Some(s), Stage::Ready) => {
            let spurred = if s == 0 {
                j.spurred
            } else {
                j.foe.as_ref().is_some_and(|f| f.spurred)
            };
            if spurred {
                prompts(cv, &[], Some(("READY FOR THE FLAG", 'T')));
            } else if j.course == 0 {
                prompts(cv, &[(&["A", "D"], "HORSE"), (&["F"], "SPUR")], None);
            } else if lone {
                prompts(cv, &[(&["F"], "SPUR"), (&["SPC"], "HOLD: LEAVE")], None);
            } else {
                prompts(cv, &[(&["F"], "SPUR")], None);
            }
        }
        (Some(_), Stage::Charge) if j.hits.is_none() => {
            if j.unset() {
                prompts(cv, &[(&["W", "S"], "AIM"), (&["E"], "SHIELD")], None);
            } else {
                prompts(cv, &[(&["F"], "STRIKE")], None);
            }
            if j.striking() {
                let (cx, cy) = at(j.meeting_x(), 14.6);
                big(cv, cx, cy - 58, "STRIKE!", 'O');
            }
        }
        (None, _) if j.gallery.contains_key(&me) => {
            let w = &j.gallery[&me];
            let backing = w
                .side
                .map(|s| format!("{}G ON {}", w.stake, rider_name(run, j, s)));
            let open = j.stage == Stage::Ready && j.course == 0;
            if open {
                let status = backing.as_deref().map(|b| (b, 'T'));
                prompts(cv, &[(&["A", "D"], "BACK"), (&["F"], "RAISE")], status);
            } else if let Some(b) = backing {
                prompts(cv, &[], Some((&b, 'T')));
            }
        }
        _ => {}
    }
}

/// To a knight on the mount plate: who they would ride against, on what,
/// and for what purse.
fn plate_board(cv: &mut Img, run: &Run) {
    let Some((&id, hero)) = run
        .players
        .iter()
        .find(|(_, h)| h.hp > 0 && inside(MOUNT_PLATE, h.x, h.y))
    else {
        return;
    };
    let stable = &run.home.stable;
    let m = stable.selected;
    let against = match run.challenger(id) {
        Some(friend) => run
            .players
            .get(&friend)
            .map_or_else(|| "A FRIEND".into(), |h| h.name.to_uppercase()),
        None => RIVALS[joust::next_rival(stable)].name.to_uppercase(),
    };
    let riders = if run.challenger(id).is_some() { 2 } else { 1 };
    let purse = if run.lists_free_gold() >= joust::ENTRY * riders {
        format!("PURSE {}G", joust::PURSE)
    } else {
        "FOR HONOUR".to_string()
    };
    let board = home::board(&[
        (format!("THE LISTS: {against}"), '9'),
        (
            format!(
                "ON {}{}   {purse}",
                m.name().to_uppercase(),
                if stable.is_tended(m) { ", TENDED" } else { "" }
            ),
            'h',
        ),
        hold_line(hero.buying, "HOLD E: MOUNT"),
    ]);
    let (c, r, _, _) = MOUNT_PLATE;
    cv.stamp(
        &board,
        (c * TILE + 4).max(2),
        (r * TILE - board.h - 22).max(2),
    );
}

/// The bout is over: who has the day, what the purse came to, and each
/// wager from the gallery.
fn result_board(cv: &mut Img, run: &Run, j: &Joust) {
    let title = match j.winner() {
        Some(s) => format!("{} WINS", rider_name(run, j, s)),
        None => "A DRAW".to_string(),
    };
    let mut lines = Vec::new();
    match (j.winner(), j.fallen) {
        (Some(0), (_, true)) | (Some(1), (true, _)) => {
            let loser = rider_name(run, j, j.winner().map_or(0, |s| 1 - s));
            lines.push((format!("{loser} IS ON THE SAND"), 'h'));
        }
        (Some(_), _) => lines.push((format!("{} PASSES TO {}", j.passes.0.max(j.passes.1), j.passes.0.min(j.passes.1)), 'h')),
        _ => lines.push(("EVERY STAKE HANDED BACK".to_string(), 'h')),
    }
    if let Some(side) = j.winner() {
        let laurel = if j.rider(side).is_some() {
            " AND THE LAUREL"
        } else {
            ""
        };
        if j.purse > 0 {
            lines.push((format!("+{}G{laurel}", joust::PURSE - joust::ENTRY), '5'));
        } else if !laurel.is_empty() {
            lines.push(("THE LAUREL, FOR HONOUR".to_string(), '5'));
        }
    }
    for (&id, w) in &j.gallery {
        let Some(won) = w.won else {
            continue;
        };
        let name = run
            .players
            .get(&id)
            .map_or_else(|| format!("KNIGHT {id}"), |h| h.name.to_uppercase());
        lines.push(if won {
            (format!("{name} +{}G", w.stake), '5')
        } else {
            (format!("{name} -{}G", w.stake), 'h')
        });
    }
    let board = home::board(&lines);
    let tw = super::super::ink::text_width(&title) * 2 + 8;
    let w = board.w.max(tw + 8);
    let h = board.h + 22;
    let x0 = ((cv.w - w) / 2 - 40).max(2);
    let y0 = cv.h - h - 4;
    chrome::board(cv, (x0, y0, w, h), 'B');
    big(cv, x0 + w / 2, y0 + 3, &title, if j.winner().is_some() { 'O' } else { 'i' });
    cv.stamp(&board, x0 + (w - board.w) / 2, y0 + 20);
    // The laurel over the winner's name.
    if let Some(side) = j.winner()
        && j.rider(side).is_some()
    {
        laurel(cv, x0 + w / 2, y0 - 7);
    }
}

/// A laurel wreath: two sprays of leaves meeting under a gold knot, drawn
/// at twice the hand's size.
fn laurel(cv: &mut Img, cx: i32, y: i32) {
    let mut im = Img::new(27, 8);
    let c = 13;
    for k in 0..6 {
        let dy = 5 - k / 2 - 2;
        for dir in [-1, 1] {
            let x = c + dir * (2 + k * 2);
            im.put(x, dy + 2, 'M');
            im.put(x, dy + 1, 'y');
            im.put(x + dir, dy + 3, 'l');
        }
    }
    im.put(c, 6, '5');
    im.put(c - 1, 7, '5');
    im.put(c + 1, 7, '5');
    let im = super::enlarge(&im, 2.0);
    cv.stamp(&im, cx - im.w / 2, y - im.h / 2);
}
