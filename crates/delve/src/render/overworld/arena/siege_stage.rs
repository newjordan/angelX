//! siege_stage.rs — the Siege Trail's own screen, drawn again for round two.
//!
//! A side-on field in the realm inks: an overcast dusk sky with clouds that
//! have form, rolling ground with texture that keeps its craters, a
//! trebuchet you can read (A-frame, throwing arm, counterweight box, sling,
//! and a pennant in the knight's own colour), the crew hauling the reload,
//! the keep's breakable wall, the shots in flight and their splats, and the
//! jokes — the straining crew, the flung soldier, the tipped pot, the
//! mooning defender, the felled man and his little grave, and the graves
//! left on the trail behind the party's cart. Fixed integers all the way
//! down; every number the painter uses is the number the sim uses.

use super::super::ink::{text_width, Img};
use super::knights;
use crate::rules::pastimes::{
    Ammo, Round, Siege, BLOCK_H, BLOCK_W, CASTLE_X, ENGINE_X, EPITAPHS, FIELD_W,
    GROUND_LINE, KEEP_COLS, LEGS, NOTICES, NOTICE_NONE, SOLDIERS, SPLAT, VOLLEY_HZ, WIND_MAX,
    MOON, POT, RELOAD_TICKS_BASE, MISSING_CREW_TICKS,
};
use crate::rules::Run;

pub(super) const SCALE: i32 = 1;
const SKY_H: i32 = 150;
const HILL_TOP: i32 = 120;

pub const ZOOM: i32 = 3;
pub const EASE_TICKS: i32 = 6;
pub const HOLD_RELOAD_X: i32 = ENGINE_X + 6;
pub const HOLD_IMPACT_X: i32 = CASTLE_X - 10;
pub const FIELD_H: i32 = SKY_H + 40;
pub const HOLD_Y: i32 = FIELD_H - FIELD_H / ZOOM;
pub const WIN_W: i32 = FIELD_W / ZOOM;

pub fn camera(s: &Siege) -> ((i32, i32), i32) {
    let full = ((0, 0), 1);
    if !s.fighting {
        return full;
    }
    let reload_ticks =
        RELOAD_TICKS_BASE + u32::from(SOLDIERS.saturating_sub(s.soldiers)) * MISSING_CREW_TICKS;
    let mut wy = HOLD_Y;
    let (cx, zoom) = if s.reload > 0 && s.reload <= reload_ticks {
        (HOLD_RELOAD_X, ZOOM)
    } else if let Some(sh) = &s.shot {
        wy = (sh.y / 16 - FIELD_H / ZOOM / 2).clamp(0, HOLD_Y);
        ((sh.x / 16).clamp(WIN_W / 2, FIELD_W - WIN_W / 2), ZOOM)
    } else if s.splat > 0 || s.moon > 0 || s.cheer > 0 || s.pot > 0 {
        (
            if s.splat_miss {
                s.splat_x.clamp(WIN_W / 2, FIELD_W - WIN_W / 2)
            } else {
                HOLD_IMPACT_X
            },
            ZOOM,
        )
    } else {
        return full;
    };
    let win_w = FIELD_W / zoom;
    let wx = (cx - win_w / 2).clamp(0, FIELD_W - win_w);
    let since = if s.reload > 0 && s.reload <= reload_ticks {
        reload_ticks - s.reload
    } else if s.shot.is_some() {
        0
    } else {
        (MOON + POT).saturating_sub(s.moon.max(s.pot))
    };
    let ease = EASE_TICKS.min(since as i32);
    if ease > 0 && ease < EASE_TICKS {
        let t = ease * 4 / EASE_TICKS;
        let eased_wx = FIELD_W / 2 + (wx - FIELD_W / 2) * t / 4;
        let eased_wy = wy * t / 4;
        return ((eased_wx, eased_wy), zoom);
    } else {
        return ((wx, wy), zoom);
    }
}

pub(super) fn siege_stage(cv: &mut Img, run: &Run, w: i32, h: i32) {
    let scene = scene_flat(run, w, h);
    cv.stamp(&scene, 0, 0);
}

fn scene_flat(run: &Run, w: i32, h: i32) -> Img {
    let round = run.pastimes.round.as_ref().unwrap();
    let s = round.siege.as_ref().unwrap();
    let mut fb = Img::black(FIELD_W, FIELD_H);
    let ox = 0;
    let oy = 0;
    sky(&mut fb, ox, oy, run.tick as u32);
    hills(&mut fb, ox, oy, run.tick as u32);
    ground(&mut fb, ox, oy, s, run.tick as u32);
    if s.fighting {
        keep(&mut fb, ox, oy, s);
        shots(&mut fb, ox, oy, s, run.tick as u32);
        engine(&mut fb, ox, oy, s, run.tick as u32, s.owner);
        defenders(&mut fb, ox, oy, s, run.tick as u32);
    } else {
        trail_screen(&mut fb, ox, oy, s, run, round);
    }
    let ((wx, wy), wz) = camera(s);
    let mut out = zoom(&fb, (wx, wy), wz, true);
    hud(&mut out, 0, 0, s, w);
    let mut scene = Img::black(w, h);
    let (px, py) = ((w - out.w) / 2, (h - out.h) / 2);
    scene.stamp(&out, px, py);
    scene
}

fn zoom(src: &Img, (wx, wy): (i32, i32), wz: i32, opaque: bool) -> Img {
    let win_w = FIELD_W / wz;
    let win_h = FIELD_H / wz;
    let out_w = win_w * wz;
    let out_h = win_h * wz;
    let mut out = if opaque {
        Img::black(out_w, out_h)
    } else {
        Img::new(out_w, out_h)
    };
    for y in 0..win_h {
        for x in 0..win_w {
            let Some(c) = src.get(wx + x, wy + y) else { continue };
            for dy in 0..wz {
                for dx in 0..wz {
                    out.set(x * wz + dx, y * wz + dy, c);
                }
            }
        }
    }
    out
}

pub(super) fn siege_parts(run: &Run, w: i32, h: i32) -> (Img, Vec<super::room_scene::Figure>) {
    let scene = scene_flat(run, w, h);
    let mut figures = Vec::new();
    let Some(round) = run.pastimes.round.as_ref() else {
        return (scene, figures);
    };
    let Some(s) = round.siege.as_ref() else {
        return (scene, figures);
    };
    if !(s.fighting && s.shot.is_some()) {
        return (scene, figures);
    }
    let mut fb = Img::new(FIELD_W, FIELD_H);
    flying_shot(&mut fb, 0, 0, s, run.tick as u32);
    let ((wx, wy), wz) = camera(s);
    let out = zoom(&fb, (wx, wy), wz, false);
    let (px, py) = ((w - out.w) / 2, (h - out.h) / 2);
    let (mut x0, mut y0, mut x1, mut y1) = (out.w, out.h, 0i32, 0i32);
    for y in 0..out.h {
        for x in 0..out.w {
            if out.get(x, y).is_some() {
                if x < x0 { x0 = x; }
                if x >= x1 { x1 = x + 1; }
                if y < y0 { y0 = y; }
                if y >= y1 { y1 = y + 1; }
            }
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return (scene, figures);
    }
    let (bw, bh) = (x1 - x0, y1 - y0);
    let mut sprite = Img::new(bw, bh);
    for dy in 0..bh {
        for dx in 0..bw {
            if let Some(c) = out.get(x0 + dx, y0 + dy) {
                sprite.set(dx, dy, c);
            }
        }
    }
    for dy in 0..bh {
        for dx in 0..bw {
            if let Some(c) = sprite.get(dx, dy) {
                if scene.get(px + x0 + dx, py + y0 + dy) != Some(c) {
                    return (scene, figures);
                }
            }
        }
    }
    figures.push(super::room_scene::Figure {
        img: sprite,
        x: px + x0,
        y: py + y0,
        frac: (0.0, 0.0),
        depth: (py + y0) as f32,
        height: 12.0,
        additive: true,
        bodies: Vec::new(),
        foe: false,
        paints: Vec::new(),
    });
    (scene, figures)
}

fn sx(ox: i32, x: i32) -> i32 {
    ox + x * SCALE
}
fn sy(oy: i32, y: i32) -> i32 {
    oy + y * SCALE
}

fn sky(cv: &mut Img, ox: i32, oy: i32, tick: u32) {
    let drift = (tick / 9) as i32;
    for x in 0..FIELD_W {
        for y in 0..SKY_H {
            let ink = if y < SKY_H / 3 {
                's'
            } else if y < 2 * SKY_H / 3 {
                'x'
            } else if y < SKY_H - 8 {
                'S'
            } else {
                'U'
            };
            let wob = ((x / 7 + y / 11) % 5 == 0) as i32;
            let ink = if wob == 1 && y < SKY_H - 8 && (y / 7) % 2 == 0 {
                if y < SKY_H / 2 { 'x' } else { 'u' }
            } else {
                ink
            };
            cv.put(sx(ox, x), sy(oy, y), ink);
        }
    }
    cloud(cv, ox, oy, (70 + drift, 22), 13);
    cloud(cv, ox, oy, (240 + drift, 40), 10);
    cloud(cv, ox, oy, (170 + drift / 2, 12), 8);
    cloud(cv, ox, oy, (330 + drift * 2, 30), 9);
    for dy in -6..=6 {
        for dx in -8..=8 {
            if dx * dx + dy * dy * 2 <= 36 {
                cv.put(sx(ox, 44 + dx), sy(oy, 118 + dy), 'u');
            }
        }
    }
}

fn cloud(cv: &mut Img, ox: i32, oy: i32, (x, y): (i32, i32), r: i32) {
    let x = x % (FIELD_W + 40);
    for &(ox2, oy2, rr) in &[(0, 0, r), (r - 1, -r / 3, r - 2), (2 * r - 2, 0, r - 3)] {
        for dy in -rr..=rr {
            for dx in -rr..=rr {
                if dx * dx + dy * dy * 4 <= rr * rr * 4 {
                    let ink = if dy < -rr / 3 { 'v' } else { 'U' };
                    cv.put(sx(ox, x + ox2 + dx), sy(oy, y + oy2 + dy), ink);
                }
            }
        }
    }
}

fn hills(cv: &mut Img, ox: i32, oy: i32, tick: u32) {
    let drift = (tick / 24) as i32;
    for x in 0..FIELD_W {
        let m = (x + drift) % 64;
        let ridge = 18 + m.min(64 - m) / 2;
        let col = sx(ox, x);
        for y in 0..ridge {
            let ink = if y > ridge - 3 {
                'x'
            } else if (x / 3 + y / 2) % 7 == 0 {
                's'
            } else {
                'S'
            };
            cv.put(col, sy(oy, HILL_TOP - y), ink);
        }
    }
}

fn ground(cv: &mut Img, ox: i32, oy: i32, s: &Siege, tick: u32) {
    for x in 0..FIELD_W {
        let g = s.ground_at(x);
        let col = sx(ox, x);
        for y in 0..g {
            let grass = y >= g - 3;
            let ink = if grass {
                match (x * 7 + y * 3) % 11 {
                    0 => 'A',
                    1..=3 => 'M',
                    _ => 'L',
                }
            } else {
                match (x + y * 2) % 9 {
                    0 => 'b',
                    1..=2 => 'I',
                    _ => 'B',
                }
            };
            cv.put(col, sy(oy, GROUND_LINE - y), ink);
        }
        for y in 0..26 {
            let ink = if y < 3 {
                'I'
            } else if (x / 5 + y / 3) % 6 == 0 {
                'I'
            } else {
                'b'
            };
            cv.put(col, sy(oy, GROUND_LINE + y), ink);
        }
        if (x * 31 + 17) % 13 == 0 && g > 0 {
            let sway = ((tick / 8 + x as u32) % 2) as i32;
            cv.put(col, sy(oy, GROUND_LINE - g - 1 + sway), 'A');
        }
    }
}

fn keep(cv: &mut Img, ox: i32, oy: i32, s: &Siege) {
    let base = GROUND_LINE - s.ground_at(CASTLE_X + BLOCK_W);
    for row in 0..s.keep.rows {
        for col in 0..KEEP_COLS {
            let i = col * usize::from(s.keep.rows) + usize::from(row);
            let hp = s.keep.blocks[i];
            let x = CASTLE_X + col as i32 * BLOCK_W;
            let y = base - (row as i32 + 1) * BLOCK_H;
            block(cv, ox, oy, x, y, hp);
        }
    }
    if s.keep.banner {
        let bx = CASTLE_X + BLOCK_W;
        let by = base - s.keep.rows as i32 * BLOCK_H - 12;
        for dy in 0..10 {
            for dx in 0..8 {
                let ink = if dx == 0 { 'i' } else { 'l' };
                cv.put(sx(ox, bx + dx), sy(oy, by + dy), ink);
            }
        }
        for dy in 0..3 {
            cv.put(sx(ox, bx + 8), sy(oy, by + 2 + dy), '4');
        }
        cv.put(sx(ox, bx + 3), sy(oy, by + 4), '4');
        cv.put(sx(ox, bx + 4), sy(oy, by + 4), '4');
        cv.put(sx(ox, bx + 3), sy(oy, by + 5), '4');
    }
}

fn block(cv: &mut Img, ox: i32, oy: i32, x: i32, y: i32, hp: u8) {
    let body = match hp {
        0 => return,
        1 => 'x',
        2 => 'S',
        _ => 'u',
    };
    for dy in 0..BLOCK_H {
        for dx in 0..BLOCK_W - 1 {
            let edge = dx == 0 || dy == 0;
            let ink = if dy == BLOCK_H - 1 { 's' } else if edge { 'U' } else { body };
            cv.put(sx(ox, x + dx), sy(oy, y + dy), ink);
        }
    }
    if hp == 1 {
        cv.line(sx(ox, x + 2), sy(oy, y + 2), sx(ox, x + 7), sy(oy, y + 7), 's');
    }
}

fn engine(cv: &mut Img, ox: i32, oy: i32, s: &Siege, tick: u32, owner: u32) {
    let gx = ENGINE_X;
    let gy = GROUND_LINE - s.ground_at(gx);
    let head = (gx + 1, gy - 34);
    thick_line(cv, ox, oy, (gx - 10, gy), head, 'B', 'r');
    thick_line(cv, ox, oy, (gx + 10, gy), head, 'b', 'I');
    thick_line(cv, ox, oy, (gx - 6, gy - 14), (gx + 6, gy - 14), 'b', 'n');
    cv.put(sx(ox, head.0), sy(oy, head.1), 'V');
    cv.put(sx(ox, head.0 - 1), sy(oy, head.1), 'U');
    cv.put(sx(ox, head.0 + 1), sy(oy, head.1), 'u');

    let t = if s.reload > 0 {
        (s.reload as i32 * 9 / 22).clamp(0, 9)
    } else {
        i32::from(s.aim) - 2
    };
    let tip = (head.0 - 22 + t * 3, head.1 - 12 + t);
    thick_line(cv, ox, oy, head, tip, 'o', 'O');
    cv.put(sx(ox, tip.0), sy(oy, tip.1), 'i');

    let cw_short = (head.0 + 8 - t, head.1 + 8 + t);
    for dy in 0..7 {
        for dx in 0..7 {
            let edge = dx == 0 || dy == 0 || dx == 6 || dy == 6;
            let ink = if dy < 2 { 'U' } else if edge { 'x' } else { 'X' };
            cv.put(sx(ox, cw_short.0 + dx), sy(oy, cw_short.1 + dy), ink);
        }
    }
    cv.line(sx(ox, head.0 + 2), sy(oy, head.1 + 2), sx(ox, cw_short.0 + 2), sy(oy, cw_short.1), 'u');
    cv.line(sx(ox, head.0 + 4), sy(oy, head.1 + 1), sx(ox, cw_short.0 + 5), sy(oy, cw_short.1), 'u');

    let pocket = (tip.0 - 1, tip.1 + 4);
    cv.line(sx(ox, tip.0), sy(oy, tip.1), sx(ox, pocket.0), sy(oy, pocket.1), 'H');
    let load = Ammo::from_index(s.volleys);
    ammo(cv, ox, oy, (pocket.0 - 1, pocket.1), load);
    cv.line(sx(ox, tip.0), sy(oy, tip.1), sx(ox, pocket.0 + 4), sy(oy, pocket.1), 'i');

    let flag = Img::from_rows(&["77", "77", "77"]).recolor(knights::seat_colours(owner));
    let pole_top = head.1 - 8;
    cv.line(sx(ox, head.0), sy(oy, head.1 - 1), sx(ox, head.0), sy(oy, pole_top), 'n');
    cv.stamp(&flag, sx(ox, head.0 + 1), sy(oy, pole_top));

    let pose = s.crew_pose(tick);
    let crew = s.soldiers.min(SOLDIERS);
    let hauling = s.reload > 0;
    if hauling && crew > 0 {
        let anchor = (cw_short.0 + 3, cw_short.1 + 7);
        cv.line(sx(ox, anchor.0), sy(oy, anchor.1), sx(ox, gx - 13), sy(oy, gy - 4), 'H');
    }
    for n in 0..crew {
        let cx = gx - 16 - (n as i32 / 2) * 6;
        let cy = gy - (n as i32 % 2) * 3;
        crewman(cv, ox, oy, (cx, cy), pose, hauling && s.soldiers <= 3);
    }
    if s.limp > 0 {
        let back = VOLLEY_HZ - s.limp;
        let lx = gx + 10 + back as i32 / 6;
        crewman(cv, ox, oy, (lx, gy), 3, false);
    }
    for i in 0..i32::from(s.power) {
        let px = gx - 10;
        let py = gy - 2 - i * 2;
        cv.put(sx(ox, px), sy(oy, py), '4');
    }
    for i in 0..i32::from(s.aim) {
        let px = gx + 10 + i;
        let py = gy + 4;
        cv.put(sx(ox, px), sy(oy, py), '5');
    }
}

fn thick_line(cv: &mut Img, ox: i32, oy: i32, a: (i32, i32), b: (i32, i32), body: char, edge: char) {
    cv.line(sx(ox, a.0), sy(oy, a.1), sx(ox, b.0), sy(oy, b.1), body);
    cv.line(sx(ox, a.0), sy(oy, a.1 - 1), sx(ox, b.0), sy(oy, b.1 - 1), edge);
}

fn crewman(cv: &mut Img, ox: i32, oy: i32, (x, y): (i32, i32), pose: u8, strain: bool) {
    let lean = match pose {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 1,
    } + i32::from(strain && pose == 2);
    let shake = strain && pose == 2 && (x + y) % 2 == 0;
    let bx = x + if shake { 1 } else { 0 };
    for dy in 0..5 {
        cv.put(sx(ox, bx + lean), sy(oy, y - 5 + dy), 'P');
    }
    cv.put(sx(ox, bx + lean), sy(oy, y - 7), '9');
    cv.line(sx(ox, bx), sy(oy, y), sx(ox, bx - 1), sy(oy, y + 1), 'n');
    cv.line(sx(ox, bx + 1), sy(oy, y), sx(ox, bx + 2), sy(oy, y + 1), 'n');
    cv.line(sx(ox, bx + lean + 1), sy(oy, y - 4), sx(ox, bx + lean + 7), sy(oy, y - 3 - lean), 'H');
}

fn ammo(cv: &mut Img, ox: i32, oy: i32, (x, y): (i32, i32), a: Ammo) {
    match a {
        Ammo::Boulder => {
            for dy in 0..4 {
                for dx in 0..5 {
                    let edge = dx == 0 || dy == 0 || dx == 4 || dy == 3;
                    cv.put(sx(ox, x + dx), sy(oy, y + dy), if edge { 'U' } else { 'u' });
                }
            }
        }
        Ammo::Cow => {
            cv.rect(sx(ox, x), sy(oy, y + 1), 5 * SCALE, 3 * SCALE, '9');
            cv.put(sx(ox, x + 5), sy(oy, y), '9');
            for k in 0..4 {
                cv.put(sx(ox, x + 1 + k), sy(oy, y), 'k');
            }
            cv.put(sx(ox, x), sy(oy, y + 4), 'n');
            cv.put(sx(ox, x + 4), sy(oy, y + 4), 'n');
        }
        Ammo::Beehive => {
            for dy in 0..4 {
                let wid = 5 - (dy % 2);
                for dx in 0..wid {
                    cv.put(sx(ox, x + dx), sy(oy, y + dy), if dy % 2 == 0 { 't' } else { 'T' });
                }
            }
            cv.put(sx(ox, x + 5), sy(oy, y - 1), '6');
        }
        Ammo::Pig => {
            cv.rect(sx(ox, x), sy(oy, y + 1), 5 * SCALE, 3 * SCALE, 'p');
            cv.put(sx(ox, x + 5), sy(oy, y + 1), 'p');
            cv.put(sx(ox, x), sy(oy, y + 4), 'n');
            cv.put(sx(ox, x + 3), sy(oy, y + 4), 'n');
        }
        Ammo::Eels => {
            cv.rect(sx(ox, x), sy(oy, y), 4 * SCALE, 4 * SCALE, 'B');
            cv.line(sx(ox, x), sy(oy, y + 2), sx(ox, x + 4), sy(oy, y + 2), 'o');
            cv.put(sx(ox, x + 4), sy(oy, y - 1), 'z');
        }
    }
}

fn flying_shot(cv: &mut Img, ox: i32, oy: i32, s: &Siege, tick: u32) {
    let Some(sh) = &s.shot else { return };
    let (px, py) = (sh.x / 16, sh.y / 16);
    ammo(cv, ox, oy, (px, py), sh.ammo);
    if sh.ammo == Ammo::Beehive {
        for k in 1..=4 {
            let wob = ((tick + k * 3) % 5) as i32 - 2;
            let wob2 = ((tick / 2 + k * 2) % 5) as i32 - 2;
            cv.put(sx(ox, px - k as i32 * 2), sy(oy, py + 1 + wob + wob2 / 2), '6');
        }
        cv.put(sx(ox, px + 1), sy(oy, py - 2), '6');
    }
    if sh.flung {
        let f = (tick / 3) % 4;
        cv.put(sx(ox, px + 2), sy(oy, py - 4), '9');
        cv.put(sx(ox, px + 2), sy(oy, py - 3), 'P');
        let (l1, l2) = match f {
            0 => ((1, -1), (3, 1)),
            1 => ((2, 0), (2, 0)),
            2 => ((3, 1), (1, -1)),
            _ => ((2, 0), (2, 0)),
        };
        cv.put(sx(ox, px + l1.0), sy(oy, py - 2 + l1.1), 'n');
        cv.put(sx(ox, px + l2.0), sy(oy, py - 2 + l2.1), 'n');
    }
}

fn shots(cv: &mut Img, ox: i32, oy: i32, s: &Siege, tick: u32) {
    flying_shot(cv, ox, oy, s, tick);
    if let Some(sh) = &s.castle_shot {
        let (px, py) = (sh.x / 16, sh.y / 16);
        for dy in 0..3 {
            for dx in 0..3 {
                cv.put(sx(ox, px + dx), sy(oy, py + dy), 'X');
            }
        }
    }
    if s.splat > 0 {
        let x = s.splat_x;
        let gy = GROUND_LINE - s.ground_at(x);
        let f = ((SPLAT - s.splat) / (SPLAT / 3)).min(2);
        let load = Ammo::from_index(s.volleys.saturating_sub(1));
        splat(cv, ox, oy, x, gy, load, f, tick);
    }
    if s.felled > 0 && s.soldiers > 0 {
        let x = s.pot_land;
        let gy = GROUND_LINE - s.ground_at(x);
        cv.put(sx(ox, x - 2), sy(oy, gy - 1), '9');
        cv.line(sx(ox, x - 1), sy(oy, gy - 1), sx(ox, x + 3), sy(oy, gy - 1), 'P');
        cv.put(sx(ox, x + 4), sy(oy, gy - 1), 'n');
    }
}

fn splat(cv: &mut Img, ox: i32, oy: i32, x: i32, gy: i32, a: Ammo, f: u32, tick: u32) {
    match a {
        Ammo::Boulder => {
            let r = 3 + f as i32 * 2;
            for dy in -r..=0 {
                for dx in -r..=r {
                    if dx * dx + dy * dy * 3 <= r * r {
                        let ink = if dy < -r / 2 { 'v' } else { 'V' };
                        cv.put(sx(ox, x + dx), sy(oy, gy + dy - 1), ink);
                    }
                }
            }
        }
        Ammo::Cow => {
            cv.put(sx(ox, x), sy(oy, gy - 2), 'n');
            cv.put(sx(ox, x + 2), sy(oy, gy - 2), 'n');
            cv.rect(sx(ox, x), sy(oy, gy - 1), 3, 2, '9');
            if f >= 1 {
                cv.put(sx(ox, x + 2), sy(oy, gy - 2), 'n');
                cv.put(sx(ox, x + 5), sy(oy, gy - 4), 'i');
                cv.put(sx(ox, x + 6), sy(oy, gy - 4), 'H');
                cv.put(sx(ox, x + 6), sy(oy, gy - 3), 'H');
                cv.put(sx(ox, x + 5), sy(oy, gy - 2), 'i');
                cv.text(sx(ox, x + 5), sy(oy, gy - 5), "MOO", 'i');
            }
        }
        Ammo::Beehive => {
            for k in 0..6 {
                let ang = (k * 11 + tick * 3) % 24;
                let dx = (ang as i32 - 12) * (1 + f as i32) / 4;
                let dy = ((tick + k * 5) % 7) as i32 - 3 - f as i32;
                cv.put(sx(ox, x + dx), sy(oy, gy - 4 + dy), '6');
            }
            cv.rect(sx(ox, x - 1), sy(oy, gy - 2), 3, 2, 't');
        }
        Ammo::Pig => {
            cv.rect(sx(ox, x), sy(oy, gy - 1), 3, 2, 'p');
            cv.put(sx(ox, x - 1), sy(oy, gy - 2), 'p');
            if f >= 1 {
                cv.put(sx(ox, x - 3), sy(oy, gy - 4), 'i');
                cv.put(sx(ox, x - 2), sy(oy, gy - 4), 'H');
                cv.put(sx(ox, x - 2), sy(oy, gy - 3), 'H');
                cv.put(sx(ox, x - 3), sy(oy, gy - 2), 'i');
                cv.text(sx(ox, x - 9), sy(oy, gy - 5), "OINK", 'i');
            }
        }
        Ammo::Eels => {
            cv.rect(sx(ox, x - 1), sy(oy, gy - 1), 4, 1, 'u');
            for k in 0..4 {
                let up = ((tick / 2 + k * 3) % 4) as i32;
                let dx = k as i32 * 2 - 3;
                let dy = -1 - (up % 3);
                cv.put(sx(ox, x + dx), sy(oy, gy + dy), 'z');
            }
            cv.put(sx(ox, x), sy(oy, gy - 4), 'u');
        }
    }
}

fn defenders(cv: &mut Img, ox: i32, oy: i32, s: &Siege, tick: u32) {
    let base = GROUND_LINE - s.ground_at(CASTLE_X + BLOCK_W);
    let wall_top = base - s.keep.rows as i32 * BLOCK_H;
    if s.keep.defenders > 0 && s.keep.banner {
        for n in 0..s.keep.defenders.min(3) {
            let dx = CASTLE_X + 2 + n as i32 * 14;
            let dy = wall_top - 4;
            cv.put(sx(ox, dx), sy(oy, dy), '9');
            cv.rect(sx(ox, dx - 1), sy(oy, dy + 1), 1 * SCALE + 1, 3 * SCALE, 'v');
            if s.pot > 0 {
                cv.line(sx(ox, dx), sy(oy, dy), sx(ox, dx - 3), sy(oy, dy - 3), '9');
                cv.rect(sx(ox, dx - 5), sy(oy, dy - 6), 3 * SCALE, 2 * SCALE, 'V');
            }
        }
    }
    if s.moon > 0 && s.keep.defenders > 0 {
        let dx = CASTLE_X + 3;
        let dy = wall_top - 6;
        let wag = (tick / 6) % 2;
        cv.put(sx(ox, dx), sy(oy, dy), '9');
        cv.put(sx(ox, dx), sy(oy, dy + 1), 'P');
        let mx = dx + 2 + wag as i32;
        cv.put(sx(ox, mx), sy(oy, dy), 'c');
        cv.put(sx(ox, mx), sy(oy, dy + 1), 'c');
    }
}

fn trail_screen(cv: &mut Img, ox: i32, oy: i32, s: &Siege, run: &Run, _round: &Round) {
    let path_y = HILL_TOP - 4;
    for x in 8..FIELD_W - 8 {
        let wave = ((x / 16) % 2) as i32;
        let py = path_y + wave;
        for d in 0..2 {
            cv.put(sx(ox, x), sy(oy, py + d), if d == 0 { 'o' } else { 'O' });
        }
        if x % 4 == 0 {
            cv.put(sx(ox, x), sy(oy, py + 2), 'r');
        }
    }
    let cart_x = 30 + (run.tick as i32 / 8) % (FIELD_W - 80);
    cart(cv, ox, oy, (cart_x, path_y - 2), run.tick as u32);
    let step = (run.tick as u32 / 5) % 4;
    let crew = s.soldiers.min(SOLDIERS);
    let mut n = 0;
    for i in 0..crew + 1 {
        let x = cart_x + 14 + i as i32 * 6;
        let bob = if (step % 2) as i32 == i32::from(i % 2) { 0 } else { 1 };
        if i == 0 {
            cv.put(sx(ox, x), sy(oy, path_y - 6 + bob), '4');
            cv.put(sx(ox, x), sy(oy, path_y - 5 + bob), 'i');
            cv.put(sx(ox, x), sy(oy, path_y - 4 + bob), 'I');
        } else {
            n += 1;
            let tunic = if n % 3 == 0 { 'R' } else { 'P' };
            cv.put(sx(ox, x), sy(oy, path_y - 4 + bob), '9');
            cv.put(sx(ox, x), sy(oy, path_y - 3 + bob), tunic);
            cv.put(sx(ox, x), sy(oy, path_y - 2 + bob), tunic);
        }
        cv.put(sx(ox, x), sy(oy, path_y - 1), 'n');
    }
    let leg = LEGS[(usize::from(s.leg) - 1).min(LEGS.len() - 1)];
    let bx = FIELD_W / 2 - 40;
    let by = path_y - 40;
    cv.line(sx(ox, bx + 6), sy(oy, by + 10), sx(ox, bx + 6), sy(oy, path_y), 'B');
    cv.line(sx(ox, bx + 66), sy(oy, by + 10), sx(oy, bx + 66), sy(oy, path_y), 'B');
    for dy in 0..10 {
        for dx in 0..72 {
            let edge = dx == 0 || dy == 0 || dx == 71 || dy == 9;
            let ink = if edge { 'B' } else if dy == 1 { 'r' } else { 'o' };
            cv.put(sx(ox, bx + dx), sy(oy, by + dy), ink);
        }
    }
    let line1 = if s.mourn > 0 || s.notice > 2 {
        if s.notice == NOTICE_NONE { String::new() } else { NOTICES[usize::from(s.notice).min(NOTICES.len() - 1)].to_string() }
    } else {
        leg.0.to_string()
    };
    let line2 = if s.mourn > 0 {
        String::new()
    } else {
        format!("{}  /  {}", leg.1, leg.2)
    };
    cv.text(sx(ox, bx + 4 + (68 - text_width(&line1)) / 2), sy(oy, by + 3), &line1, 'n');
    cv.text(
        sx(ox, bx + (72 - text_width(&line2).min(68)) / 2),
        sy(oy, by + 6),
        &line2,
        'k',
    );
    let mut gx = 8;
    let mut gy = path_y + 8;
    for (i, &g) in s.graves.iter().enumerate() {
        if g != 255 && i + 1 < usize::from(s.leg) {
            let (name, line) = EPITAPHS[usize::from(g.min(9))];
            for dy in 0..7 {
                for dx in 0..5 {
                    let edge = dx == 0 || dy == 0 || dx == 4 || dy == 6;
                    let ink = if dy == 2 || dy == 3 { if dx == 2 { 'v' } else if edge { 'S' } else { 's' } } else if edge { 'S' } else { 's' };
                    cv.put(sx(ox, gx + dx), sy(oy, gy - 7 + dy), ink);
                }
            }
            cv.put(sx(ox, gx + 2), sy(oy, gy - 5), 'v');
            cv.line(sx(ox, gx + 2), sy(oy, gy - 4), sx(ox, gx + 2), sy(oy, gy - 3), 'v');
            let said = format!("{name}: {line}");
            cv.text(sx(ox, gx + 8), sy(oy, gy - 6), &said, 'v');
            gx += 8 + text_width(&said) + 10;
            if gx > FIELD_W - 60 {
                gx = 8;
                gy += 12;
            }
        }
    }
    if s.grave < 255 && s.mourn > 0 {
        let (name, line) = EPITAPHS[usize::from(s.grave.min(9))];
        let gx2 = bx + 80;
        cv.put(sx(ox, gx2), sy(oy, path_y - 3), 'u');
        cv.put(sx(ox, gx2), sy(oy, path_y - 2), 'S');
        cv.put(sx(ox, gx2), sy(oy, path_y - 1), 'S');
        let said = format!("Here lies {name}. {line}");
        cv.text(
            sx(ox, (FIELD_W - text_width(&said)) / 2),
            sy(oy, path_y + 6),
            &said,
            '9',
        );
    }
}

fn cart(cv: &mut Img, ox: i32, oy: i32, (x, y): (i32, i32), tick: u32) {
    for dy in 0..3 {
        for dx in 0..12 {
            cv.put(sx(ox, x + dx), sy(oy, y - 2 + dy), if dy == 0 { 'O' } else { 'r' });
        }
    }
    for dx in 1..11 {
        cv.put(sx(ox, x + dx), sy(oy, y - 3), if dx % 3 == 0 { 't' } else { 'o' });
    }
    cv.rect(sx(ox, x + 3), sy(oy, y - 5), 4, 2, 'T');
    cv.line(sx(ox, x + 11), sy(oy, y - 1), sx(ox, x + 16), sy(oy, y - 1), 'r');
    cv.put(sx(ox, x + 16), sy(oy, y - 2), 'R');
    let spin = (tick / 5) % 4;
    for &(wx, wy) in &[(x + 2, y + 1), (x + 8, y + 1)] {
        for dy in -2..=2 {
            for dx in -2..=2 {
                let d = dx * dx + dy * dy;
                if d == 4 || d == 5 {
                    cv.put(sx(ox, wx + dx), sy(oy, wy + dy), 'r');
                }
            }
        }
        cv.put(sx(ox, wx), sy(oy, wy), 'p');
        let (s1, s2) = match spin {
            0 => ((0, -2), (0, 2)),
            1 => ((2, 0), (-2, 0)),
            2 => ((1, -1), (-1, 1)),
            _ => ((-1, -1), (1, 1)),
        };
        cv.put(sx(ox, wx + s1.0), sy(oy, wy + s1.1), 'o');
        cv.put(sx(ox, wx + s2.0), sy(oy, wy + s2.1), 'o');
    }
}

fn hud(cv: &mut Img, ox: i32, oy: i32, s: &Siege, w: i32) {
    let mut line = format!("MEN {}   STONES {}   FOOD {}", s.soldiers, s.stones, s.food);
    if s.fighting {
        let wind = s.wind * 100 / WIND_MAX;
        line = format!("{}   WIND {}%   DMG {}", line, wind, s.damage);
    }
    cv.text(sx(ox, 2), sy(oy, GROUND_LINE + 6), &line, 'i');
    let mut pips = String::from("CASTLES ");
    for i in 1..=3 {
        pips.push(if i < s.leg || (i == s.leg && s.taken >= i) { '#' } else { '-' });
    }
    cv.text(sx(ox, 2), sy(oy, GROUND_LINE + 16), &pips, 'i');
    if s.fighting {
        let bars = FIELD_W - 8;
        let lit = bars as u32 * s.volley_left / VOLLEY_HZ;
        cv.line(sx(ox, 4), sy(oy, 4), sx(ox, 4 + bars), sy(oy, 4), 's');
        cv.line(sx(ox, 4), sy(oy, 4), sx(ox, 4 + lit as i32), sy(oy, 4), '4');
    }
    let _ = w;
}
