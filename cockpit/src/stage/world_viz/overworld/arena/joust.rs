//! The stables and the lists, drawn: the realm's three mounts in their
//! stalls and the saddled one in the yard; at the lists, the crowd on the
//! stands and, while a bout is ridden, the riders at the gallop down the
//! tilt, the timing ring closing where they will meet, the lances breaking,
//! and a rider going over his horse's tail.
//!
//! The destrier is drawn from shapes (barrel, chest, rump, an arched neck,
//! jointed legs that move with each frame of the gallop), so one drawing
//! serves every coat, caparison and stride.

use super::super::ink::{Img, hash};
use super::{TILE, at, home, sprites};
use crate::drive::chivalry::Mount;
use crate::drive::together_shooter::Run;
use crate::drive::together_shooter::joust::{
    self, Aim, GROOM_AT, Hit, Joust, LED_AT, MOUNT_PLATE, RIVALS, STALL_PLATES, Stage, TEND_PLATE,
    Verdict,
};
use crate::drive::together_shooter::world::STANDS;

/// A horse's hide, its shade, and its mane and tail.
#[derive(Clone, Copy)]
pub(super) struct Coat {
    hide: char,
    dark: char,
    mane: char,
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
}

/// Player `id`'s colours, as their knight wears them on foot.
pub(super) fn colours_of(run: &Run, id: u32) -> Colours {
    let tabard = run
        .players
        .get(&id)
        .and_then(|h| {
            h.colours()
                .iter()
                .find(|(from, _)| *from == '7')
                .map(|p| p.1)
        })
        .unwrap_or(match id {
            1 => '7',
            2 => '1',
            3 => 'A',
            _ => '4',
        });
    Colours {
        tabard,
        field: if tabard == '4' { 'a' } else { '4' },
        plume: tabard,
    }
}

fn rival_colours(rival: usize) -> Colours {
    let (tabard, field) = RIVALS[rival.min(RIVALS.len() - 1)].colours;
    Colours {
        tabard,
        field,
        plume: field,
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
    for y in sy - 12..sy - 7 {
        for x in sx - 2..sx + 3 {
            im.put(x, y, if y == sy - 12 { 'H' } else { 'h' });
        }
    }
    im.put(sx + 1, sy - 10, 'k');
    im.put(sx + 2, sy - 10, 'k');
    im.line(sx - 1, sy - 13, sx - 3, sy - 15, col.plume);
    im.line(sx, sy - 13, sx - 2, sy - 15, col.plume);
    // The leg in the stirrup.
    im.line(sx, sy + 1, sx + 1, sy + 5, 'h');
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
                    'H'
                } else if charge {
                    col.field
                } else {
                    col.tabard
                },
            );
        }
    }
}

/// A knight thrown from the saddle: helm, tabard, arms flung out, legs in
/// the air; it turns over as it falls (`age` ticks after the impact) and
/// lies on its back on the sand.
fn thrown(col: Colours, age: u32) -> Img {
    let mut im = Img::new(14, 18);
    im.rect(4, 0, 6, 6, 'i');
    im.line(4, 0, 9, 0, 'H');
    im.line(6, 3, 8, 3, 'k');
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
    im.line(3, 7, 0, 4, 'h');
    im.line(10, 7, 13, 4, 'h');
    im.line(5, 13, 3, 17, 'h');
    im.line(8, 13, 10, 17, 'h');
    match age {
        0..=5 => im,
        6..=10 => sprites::quarter_turn(&im),
        11..=15 => sprites::quarter_turn(&sprites::quarter_turn(&im)),
        _ => sprites::quarter_turn(&sprites::quarter_turn(&sprites::quarter_turn(&im))),
    }
}

/// The fall: from the saddle at the meeting, flung back along the lane,
/// up and down onto the sand, a puff of dust where he lands.
fn fall(cv: &mut Img, col: Colours, (lane_x, lane_y): (i32, i32), back: i32, age: u32) {
    let a = age.min(22) as f32;
    // A short arc back from the meeting: up, then down to the lane.
    let x = lane_x + back * (a * 1.6) as i32;
    let lift = (a * (22.0 - a) / 4.0) as i32;
    let y = lane_y - 6 - lift;
    let body = thrown(col, age);
    cv.stamp(&body, x - body.w / 2, y - body.h);
    if age >= 20 {
        // The dust settling round him.
        let d = (age - 20).min(16) as i32;
        for k in 0..10 {
            let ang = k as f32 / 10.0 * std::f32::consts::PI;
            let (dx, dy) = (
                (ang.cos() * (8 + d) as f32) as i32,
                (ang.sin() * 3.0) as i32,
            );
            if d < 14 {
                cv.put(
                    x + dx,
                    y - 1 + dy - d / 4,
                    if k % 2 == 0 { 'J' } else { 'h' },
                );
            }
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
    let (gx, gy) = at(GROOM_AT.0 * 2.0, GROOM_AT.1 * 2.0);
    let g = groom(tended, tick);
    cv.stamp(&g, gx - g.w / 2, gy - g.h + 1);
    let lit = run
        .players
        .values()
        .any(|h| h.hp > 0 && inside(TEND_PLATE, h.x, h.y));
    home::plate_marked(cv, TEND_PLATE, lit, &brush());
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
                hold_line(hero.buying, "HOLD F: SADDLE")
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
        if inside(TEND_PLATE, hero.x, hero.y) {
            let m = stable.selected;
            let last = if stable.is_tended(m) {
                ("TENDED: A KNOCK MORE, ONE BOUT".to_string(), '5')
            } else {
                hold_line(hero.buying, "HOLD F: TEND")
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
    let filled = (buying * 10 / crate::drive::together_shooter::home::BUY_HOLD).min(10) as usize;
    (format!("{label} {}", "#".repeat(filled)), 'O')
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
            cv.rect(x, y - jump, 3, 3, skin);
            cv.rect(x, y + 3 - jump, 3, 3, body);
            if cheer && jump == 2 {
                cv.put(x - 1, y - 2, skin);
                cv.put(x + 3, y - 2, skin);
            }
            x += 6 + (h % 3) as i32;
            k += 1;
        }
    }
}

/// The lists' figures: the mount plate, and a bout while one is ridden.
/// `legible`: drawn at native size or larger, so its words can be read (a
/// reduced frame leaves the score to the sidebar).
pub(super) fn lists(cv: &mut Img, run: &Run, legible: bool) {
    crowd(cv, run);
    let lit = run.joust.is_none()
        && run
            .players
            .values()
            .any(|h| h.hp > 0 && inside(MOUNT_PLATE, h.x, h.y));
    home::plate_marked(cv, MOUNT_PLATE, lit, &lance_mark());
    if let Some(j) = &run.joust {
        bout(cv, run, j, legible);
    } else {
        // The saddled mount, barded, waits by the red pavilion.
        let m = run.home.stable.selected;
        let col = colours_of(run, 1);
        let horse = destrier(coat(m), None, Some((col.tabard, col.field)));
        // Beside the mount plate, never over it.
        let (x, y) = at(9.4, joust::SOUTH_LANE + 2.4);
        cv.stamp(&horse, x - horse.w / 2, y - horse.h + 2);
    }
}

fn lance_mark() -> Img {
    Img::from_rows(&["....O", "...O.", "..O..", "OO...", "OO..."])
}

/// A bout: the riders, the ring closing where they meet, what the lances
/// did, and who went over.
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
    let him = rival_colours(usize::from(j.rival));
    let (mine, theirs) = j.hits.unwrap_or_default();
    // The rival's shield: wavering until it can be read.
    let shield_high = match j.guard_shown() {
        Some(g) => g == Aim::High,
        None if charging => (j.t / 6).is_multiple_of(2),
        None => true,
    };
    // The rival, in the far lane: drawn first, behind the tilt. A fallen
    // rider stays down through the verdict.
    let down = met || j.stage == Stage::Done;
    let age = if j.stage == Stage::Done { 99 } else { age };
    let rival_fallen = j.fallen.1 && down;
    draw_rider(
        cv,
        (rx, ry, reast),
        gait(1),
        him,
        false,
        shield_high,
        matches!(theirs, Hit::Broke) && met,
        rival_fallen,
        age,
        (RIVAL_COAT, (him.tabard, him.field)),
    );
    let knight_fallen = j.fallen.0 && down;
    draw_rider(
        cv,
        (kx, ky, keast),
        gait(0),
        me,
        j.aim == Aim::High,
        false,
        matches!(mine, Hit::Broke | Hit::Smite) && met,
        knight_fallen,
        age,
        (coat(j.mount), (me.tabard, me.field)),
    );
    // The riders who went over, where they fell.
    for (fallen, col, lane, east) in [
        (rival_fallen, him, joust::NORTH_LANE, reast),
        (knight_fallen, me, joust::SOUTH_LANE, keast),
    ] {
        if fallen {
            let (x, y) = at(joust::MEETING, lane);
            fall(cv, col, (x, y + 6), if east { -1 } else { 1 }, age);
        }
    }
    // The timing ring over the tilt where they will meet, over everything:
    // it closes as they come, and burns gold while a strike would land.
    if let Some(k) = j.closing() {
        let (cx, cy) = at(joust::MEETING, 14.6);
        let r = 6.0 + (1.0 - k) * 36.0;
        let near = meeting.saturating_sub(j.t) <= j.mount.steed().strike;
        let ink = if near { '5' } else { 'h' };
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
        let (cx, cy) = at(joust::MEETING, 14.6);
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
        if mine != Hit::Miss || theirs != Hit::Miss {
            let r = 2 + age as i32 / 2;
            if age < 10 {
                for i in 0..32 {
                    let a = i as f32 / 32.0 * std::f32::consts::TAU;
                    cv.put(
                        cx + (a.cos() * r as f32) as i32,
                        cy + (a.sin() * r as f32) as i32,
                        'w',
                    );
                }
            }
        }
        // What each lance did, by its target at the meeting, rising: the
        // knight's over the far lane, the rival's under the near one (only
        // where the words can be read; the sidebar says it otherwise).
        if !legible {
            return;
        }
        let rise = (age / 3) as i32;
        let say = |cv: &mut Img, hit: Hit, y: i32, ink: char| {
            let text = if hit == Hit::Miss {
                "MISSED".to_string()
            } else {
                format!("{} +{}", hit.word(), hit.points())
            };
            let w = super::super::ink::text_width(&text);
            cv.text((cx - w / 2).clamp(2, cv.w - w - 2), y, &text, ink);
        };
        let (_, far) = at(rx, ry);
        let (_, near) = at(kx, ky);
        say(
            cv,
            mine,
            (far - 50 - rise).max(52),
            if mine == Hit::Miss { 'h' } else { '5' },
        );
        say(
            cv,
            theirs,
            near + 10 + rise,
            if theirs == Hit::Miss { 'h' } else { '7' },
        );
    }
    if legible {
        scoreboard(cv, j);
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
    fallen: bool,
    age: u32,
    (hide, barding): (Coat, (char, char)),
) {
    let mut im = destrier(hide, gait, Some(barding));
    if !fallen {
        rider(&mut im, gait, col, high, shield_high, broken);
    }
    let im = if east { im } else { im.flip_h() };
    let (px, py) = at(x, y);
    cv.stamp(&im, px - im.w / 2, py - im.h + 6);
    let _ = age;
}

/// Over the stands while a bout is ridden: the course, the score, each
/// rider's balance, and what to press.
fn scoreboard(cv: &mut Img, j: &Joust) {
    let rival = j.rival();
    let pips = |n: u32| "#".repeat(n as usize);
    let mut lines = vec![
        (
            format!(
                "COURSE {} OF 3   YOU {} : {} {}",
                j.course + 1,
                j.score.0,
                j.score.1,
                rival.name.to_uppercase()
            ),
            '9',
        ),
        (
            format!("BALANCE {:<6}  {:>6}", pips(j.balance.0), pips(j.balance.1)),
            'h',
        ),
    ];
    match (j.stage, j.verdict) {
        (Stage::Ready, _) => lines.push(("F: SPUR   HOLD SPACE: WITHDRAW".into(), 'O')),
        (Stage::Charge, _) if j.hits.is_none() => {
            lines.push(("W/S AIM HIGH/LOW  F STRIKE  SPACE BRACE".into(), 'O'))
        }
        (Stage::Done, Some(v)) => lines.push((
            match (v, j.fallen) {
                (Verdict::Won, (_, true)) => {
                    format!("{} IS UNHORSED: YOURS", rival.name.to_uppercase())
                }
                (Verdict::Won, _) => format!("YOU BEAT {}", rival.name.to_uppercase()),
                (Verdict::Lost, (true, _)) => "UNHORSED: ON THE SAND".to_string(),
                (Verdict::Lost, _) => format!("{} HAS THE DAY", rival.name.to_uppercase()),
                (Verdict::Drawn, _) => "A DRAW".to_string(),
            },
            if v == Verdict::Won { '5' } else { 'O' },
        )),
        _ => {}
    }
    let board = home::board(&lines);
    // Under the near lane, clear of the stands and the riders.
    cv.stamp(&board, (cv.w - board.w) / 2, cv.h - board.h - 3);
}

/// To a knight on the mount plate: who they will ride against, on what.
pub(super) fn lists_boards(cv: &mut Img, run: &Run) {
    if run.joust.is_some() {
        return;
    }
    let Some(hero) = run
        .players
        .values()
        .find(|h| h.hp > 0 && inside(MOUNT_PLATE, h.x, h.y))
    else {
        return;
    };
    let stable = &run.home.stable;
    let rival = &RIVALS[joust::next_rival(stable)];
    let m = stable.selected;
    let board = home::board(&[
        (format!("THE LISTS: {}", rival.name.to_uppercase()), '9'),
        (
            format!(
                "ON {}{}",
                m.name().to_uppercase(),
                if stable.is_tended(m) { ", TENDED" } else { "" }
            ),
            'h',
        ),
        hold_line(hero.buying, "HOLD F: MOUNT"),
    ]);
    let (c, r, _, _) = MOUNT_PLATE;
    cv.stamp(
        &board,
        (c * TILE + 4).max(2),
        (r * TILE - board.h - 22).max(2),
    );
}
