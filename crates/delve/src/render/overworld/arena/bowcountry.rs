//! Longbow country drawn: the green's butts, rope and popinjay, the
//! fletcher's shed and goose pen, the scarecrow's wheat. The scenery is
//! cached; the popinjay, the arrows, the scarecrow's clothes and the crows
//! are drawn per frame from the game's state.
use super::super::ink::{Img, hash};
use super::super::light::Light;
use super::{TILE, at, fire, home, props};
use super::super::material as mat;
use crate::rules::{
    RoomKind, Run,
    bowcountry::{self as b, BOOTH, BUTTS, CLIENT, CLOTHES, COACH, FLETCHER, LINE, PEN, POLES, QUIVERS, RACK, ROPE, SHED, STOOKS},
    pastimes::{Game, LONGBOW_ARROWS, LONGBOW_FLIGHT, LONGBOW_FULL_DRAW, Longbow},
};

type Rect = (i32, i32, i32, i32);

fn px((c, r, w, h): Rect) -> (i32, i32, i32, i32) {
    (c * TILE, r * TILE, w * TILE, h * TILE)
}

/// A straw butt on its stand.
fn butt(cv: &mut Img, x: i32, y: i32) {
    props::butt(cv, x, y);
}

/// The butts' bank of turf along the north wall, three butts on it.
fn butts(cv: &mut Img) {
    let (x, y, w, h) = px(BUTTS);
    mat::hedge(cv, (x, y - 4, w, h + 4), 261);
    for c in [7, 12, 17] {
        butt(cv, c * TILE, y + 12);
    }
}

/// The poles' feet and their height above them (pixels).
fn pole_foot() -> i32 {
    (ROPE * TILE as f32) as i32 + 9
}
const POLE: i32 = 30;

/// A tall timber pole, its foot at `(x, y)`.
fn pole(cv: &mut Img, x: i32, y: i32, height: i32) {
    cv.rect(x - 1, y - height, 3, height, 'r');
    cv.line(x - 1, y - height, x - 1, y, 'B');
    cv.rect(x - 2, y - height - 2, 5, 2, 'o');
    cv.line(x - 4, y, x + 4, y, 'B');
}

/// The judge's tally booth: a striped awning over a boarded counter and a
/// slate of tallies.
fn booth(cv: &mut Img) {
    let (x, y, w, h) = px(BOOTH);
    props::stall(cv, (x, y, w, h), ('R', 'T'), y + 14, y, 271);
    cv.rect(x + 6, y + 9, w - 12, 6, 'g');
    for i in 0..3 {
        cv.line(x + 8 + i * 3, y + 10, x + 8 + i * 3, y + 13, 'h');
    }
}

/// Quivers stood in barrels, fletchings up.
fn quivers(cv: &mut Img, (x, y): (i32, i32), n: i32) {
    for k in 0..n {
        let bx = x + 5 + k * 13;
        mat::barrel(cv, bx, y + 14);
        for s in 0..4 {
            let sx = bx - 3 + s * 2;
            cv.line(sx, y - 1, sx, y + 5, 'o');
            cv.rect(sx - 1, y - 3, 2, 3, if s % 2 == 0 { 'T' } else { 'h' });
        }
    }
}

/// The fletcher's open shed: a shingled roof on posts over a bench of
/// feathers, finished arrows bundled at the posts.
fn shed(cv: &mut Img) {
    let (x, y, w, h) = px(SHED);
    props::shed(cv, (x, y, w, h), 281);
    mat::boards(cv, (x + 8, y + h - 12, w - 16, 4), false, 282);
    cv.line(x + 6, y + 18, x + w - 7, y + 18, 'o');
    for k in 0..9 {
        let fx = x + 9 + k * 9;
        cv.line(fx, y + 19, fx, y + 23, if k % 3 == 0 { 'T' } else { 'h' });
        cv.put(fx + 1, y + 20, 'J');
    }
    for bx in [x + 6, x + w - 12] {
        for s in 0..4 {
            cv.line(bx + s, y + h - 20, bx + s + 2, y + h - 2, 'o');
            cv.put(bx + s, y + h - 21, 'T');
        }
        cv.line(bx, y + h - 10, bx + 5, y + h - 10, 'b');
    }
}

/// The goose pen: a fenced patch of trodden ground and feathers.
fn pen(cv: &mut Img) {
    let (x, y, w, h) = px(PEN);
    for yy in y + 4..y + h - 2 {
        for xx in x + 2..x + w - 2 {
            if let Some(ch) = mat::ground(mat::Ground::Mud, xx, yy) {
                cv.put(xx, yy, ch);
            } else if hash(xx, yy, 151).is_multiple_of(13) {
                cv.put(xx, yy, 'h');
            }
        }
    }
    mat::fence(cv, x, x + w - 1, y + 4);
    mat::fence(cv, x, x + w - 1, y + h - 1);
    for px_ in [x, x + w - 2] {
        cv.rect(px_, y + 2, 2, h - 2, 'B');
    }
}

/// A goose at `(x, y)` (its feet), facing `face` (1 east, -1 west).
fn goose(cv: &mut Img, x: i32, y: i32, face: i32, peck: bool) {
    cv.ellipse(x as f32, (y - 4) as f32, 5.0, 3.0, 'v');
    cv.rect(x - face * 5, y - 6, 2, 2, 'U');
    let (nx, ny) = (x + face * 4, if peck { y - 3 } else { y - 11 });
    cv.line(x + face * 3, y - 6, nx, ny, 'v');
    cv.rect(nx - 1, ny - 1, 3, 3, 'v');
    cv.rect(nx + face * 2 - (face < 0) as i32, ny, 2, 1, 'O');
    cv.put(nx, ny - 1, 'k');
    cv.line(x - 1, y - 1, x - 1, y, 'O');
    cv.line(x + 1, y - 1, x + 1, y, 'O');
}

/// The arrow-shaft drying rack.
fn rack(cv: &mut Img) {
    let (x, y, w, h) = px(RACK);
    mat::shadow(cv, ((x + w / 2 + 4) as f32, (y + h + 1) as f32), (w as f32 * 0.5, 2.0), &super::nooks::all);
    for px_ in [x + 3, x + w - 4] {
        mat::boards(cv, (px_, y + 2, 2, h - 2), true, 291);
    }
    for ry in [y + 6, y + 14, y + 22] {
        mat::boards(cv, (x + 1, ry, w - 2, 2), false, 292);
        for sx in (x + 5..x + w - 5).step_by(3) {
            cv.line(sx, ry - 4, sx + 1, ry + 3, 'o');
            cv.put(sx, ry - 5, 'T');
        }
    }
}

/// A stook of wheat.
fn stook(cv: &mut Img, x: i32, y: i32, v: u32) {
    props::stook(cv, x + 5, y + 10, v);
}

fn field(cv: &mut Img) {
    // Furrows between the stooks, cut short of the roads.
    for &rect in &STOOKS {
        let (x, y, w, h) = px(rect);
        for yy in (y + 1..y + h).step_by(4) {
            cv.line(x, yy, x + w - 1, yy, 'I');
        }
        // Two rows of stooks, the back row first so the front overlaps it.
        let cols = w / 13;
        for k in 0..cols * 2 {
            let sx = x + 1 + (k % cols) * 13 + (hash(k, y, 162) % 3) as i32 + (k / cols) * 6;
            let sy = y - 4 + (k / cols) * 15;
            stook(cv, sx, sy, hash(sx, sy, 163));
        }
    }
}

pub(super) fn scenery(cv: &mut Img, run: &Run) {
    match run.room().kind {
        RoomKind::LongbowGreen => {
            butts(cv);
            booth(cv);
            let (qx, qy, _, _) = px(QUIVERS);
            quivers(cv, (qx + 2, qy + 2), 2);
            // The poles and the popinjay's rope across the butts.
            for c in [POLES.0, POLES.1] {
                pole(cv, (c * TILE as f32) as i32, pole_foot(), POLE);
            }
            let (l, r) = ((POLES.0 * TILE as f32) as i32, (POLES.1 * TILE as f32) as i32);
            for xx in l..=r {
                let sag = ((xx - l) * (r - xx)) / 900;
                cv.put(xx, pole_foot() - POLE + 2 + sag, 'P');
            }
            // The shooting line, chalked, and the distance pegs.
            let ly = (LINE * TILE as f32) as i32 + 7;
            for xx in (4 * TILE..20 * TILE).step_by(6) {
                cv.line(xx, ly, xx + 3, ly, 'J');
            }
            for c in [4, 20] {
                cv.rect(c * TILE - 1, ly - 4, 2, 6, 'o');
            }
        }
        RoomKind::Fletchery => {
            shed(cv);
            pen(cv);
            rack(cv);
            // Feathers blown about the yard.
            for k in 0..14 {
                let fx = 3 * TILE + (hash(k, 1, 171) % (18 * TILE as u32)) as i32;
                let fy = 5 * TILE + (hash(k, 2, 171) % (4 * TILE as u32)) as i32;
                cv.line(fx, fy, fx + 2, fy - 1, if k % 3 == 0 { 'h' } else { 'J' });
            }
        }
        RoomKind::Scarecrow => field(cv),
        _ => {}
    }
    // The Quiver Coach's stair: planked until the realm builds it.
    let stair = match run.room().kind {
        RoomKind::Fletchery => Some(COACH),
        RoomKind::Village => Some(b::VILLAGE_COACH),
        _ => None,
    };
    if let Some((c, r, w, h)) = stair {
        cv.rect(c * TILE, r * TILE, w * TILE, h * TILE, 'I');
        // The coach's sign: a quiver on a post.
        let (sx, sy) = (c * TILE - 4, r * TILE + 2);
        cv.line(sx, sy - 14, sx, sy + 12, 'r');
        cv.rect(sx - 3, sy - 16, 7, 9, 'B');
        for s in 0..3 {
            cv.line(sx - 2 + s * 2, sy - 20, sx - 2 + s * 2, sy - 16, 'T');
        }
        if run.home.countryside.has(b::PLOT) {
            super::open_arrow(cv, c * TILE + w * TILE / 2, r * TILE + 3);
        } else {
            super::shut_planks(cv, c * TILE, r * TILE, w * TILE);
        }
    }
}

pub(super) fn lights(lights: &mut Vec<Light>, run: &Run) {
    match run.room().kind {
        RoomKind::LongbowGreen => {
            let (x, y, w, _) = px(BOOTH);
            lights.push(fire(x + w / 2, y + 4, 52.0, 0.36));
        }
        RoomKind::Fletchery => {
            let (x, y, w, _) = px(SHED);
            lights.push(fire(x + w / 2, y + 30, 60.0, 0.36));
        }
        _ => {}
    }
}

/// The popinjay: a painted wooden parrot on a ring, hung from the rope.
fn popinjay(cv: &mut Img, x: i32, y: i32) {
    cv.line(x, y - 12, x, y - 6, 'P');
    cv.ellipse(x as f32, (y - 1) as f32, 4.0, 5.0, 'A');
    cv.ellipse((x - 1) as f32, y as f32, 2.0, 3.0, 'M');
    cv.rect(x - 2, y - 7, 5, 4, 'R');
    cv.rect(x + 3, y - 6, 2, 2, 'O');
    cv.put(x + 1, y - 6, 'k');
    cv.line(x - 2, y + 4, x - 5, y + 9, 'y');
    cv.line(x, y + 4, x - 1, y + 9, 'A');
}

/// The wind on the west pole: a pennant streaming the way it blows.
fn pennant(cv: &mut Img, x: i32, y: i32, wind: i32) {
    cv.line(x, y - 2, x, y + 9, 'o');
    if wind == 0 {
        cv.line(x + 1, y, x + 1, y + 7, 'T');
        return;
    }
    let len = 4 + wind.abs() * 3;
    let dir = if wind < 0 { -1 } else { 1 };
    for k in 1..=len {
        let half = 3 - (k * 3 / (len + 1));
        cv.line(x + dir * k, y + 3 - half, x + dir * k, y + 3 + half, if k % 4 < 2 { 'T' } else { 'R' });
    }
}

/// An arrow, point up.
fn arrow(cv: &mut Img, x: i32, y: i32) {
    cv.line(x, y, x, y + 12, 'O');
    cv.rect(x - 1, y - 2, 3, 2, 'V');
    cv.put(x, y - 3, 'V');
    for k in 0..3 {
        cv.put(x - 1, y + 10 + k, 'T');
        cv.put(x + 1, y + 10 + k, 'T');
    }
}

/// The archer's longbow across their front, bent as far as it is drawn,
/// and the draw beside them: gold when it will carry to the rope.
fn bow(cv: &mut Img, x: i32, y: i32, draw: u32) {
    let pull = (draw.min(LONGBOW_FULL_DRAW) * 6 / LONGBOW_FULL_DRAW) as i32;
    let top = y - 28;
    for dx in -11i32..=11 {
        let bend = 4 - (dx * dx) / 30;
        cv.put(x + dx, top - bend, 'O');
        cv.put(x + dx, top - bend + 1, 'r');
    }
    cv.line(x - 11, top, x, top + pull, 'h');
    cv.line(x + 11, top, x, top + pull, 'h');
    if draw > 0 {
        arrow(cv, x, top - 9 + pull);
        let full = draw >= LONGBOW_FULL_DRAW;
        let fill = (draw.min(LONGBOW_FULL_DRAW) * 16 / LONGBOW_FULL_DRAW) as i32;
        cv.rect(x + 13, y - 22, 4, 18, 'K');
        cv.rect(x + 14, y - 5 - fill, 2, fill.max(1), if full { '5' } else { 'o' });
    }
}

/// The scarecrow on his post, wearing what has come back; his feet at
/// `(x, y)`.
fn scarecrow(cv: &mut Img, x: i32, y: i32, dressed: bool, tick: u64) {
    let sway = (hash((tick / 20) as i32, 3, 181) % 2) as i32;
    cv.rect(x - 1, y - 44, 3, 45, 'B');
    cv.line(x - 1, y - 44, x - 1, y, 'b');
    cv.rect(x - 17, y - 33 + sway, 35, 3, 'r');
    cv.line(x - 17, y - 33 + sway, x + 17, y - 33 - sway, 'o');
    // Straw at the cuffs and neck.
    for s in [-1, 1] {
        let hx = x + s * 18;
        for k in 0..4 {
            cv.line(hx, y - 33 + k, hx + s * (2 + k % 2), y - 35 + k * 2, 't');
        }
    }
    // A sack head with button eyes and a stitched grin.
    cv.ellipse(x as f32, (y - 42) as f32, 7.0, 7.0, 'o');
    cv.ellipse((x - 1) as f32, (y - 43) as f32, 5.0, 5.0, 'O');
    cv.rect(x - 4, y - 45, 2, 2, 'k');
    cv.rect(x + 3, y - 45, 2, 2, 'k');
    cv.line(x - 4, y - 39, x + 4, y - 39, 'b');
    for k in [-3, -1, 1, 3] {
        cv.put(x + k, y - 38, 'b');
    }
    for k in [-6, 6] {
        cv.line(x + k, y - 36, x + k + k / 3, y - 33, 't');
    }
    if dressed {
        // The feather coat, collar up, sleeves along the crossbar.
        cv.rect(x - 8, y - 34, 17, 20, 'U');
        cv.rect(x - 16, y - 34, 8, 5, 'U');
        cv.rect(x + 9, y - 34, 8, 5, 'U');
        for k in 0..8 {
            cv.line(x - 7 + k * 2, y - 32, x - 7 + k * 2, y - 16, if k % 2 == 0 { 'v' } else { 'u' });
        }
        cv.line(x, y - 34, x, y - 15, 'S');
        // The tin hat, dented.
        cv.ellipse(x as f32, (y - 48) as f32, 9.0, 2.0, 'j');
        cv.rect(x - 6, y - 55, 12, 7, 'J');
        cv.rect(x - 5, y - 54, 4, 2, 'h');
        cv.put(x + 2, y - 52, 'G');
        // One left boot, laced, dangling from the post.
        cv.rect(x - 4, y - 8, 6, 7, 'b');
        cv.rect(x - 8, y - 3, 5, 3, 'b');
        cv.put(x - 1, y - 7, 'o');
        cv.put(x - 1, y - 5, 'o');
    } else {
        // Striped smallclothes, and nothing else at all.
        cv.rect(x - 6, y - 25, 13, 8, 'T');
        for k in [-5, -2, 1, 4] {
            cv.line(x + k, y - 25, x + k, y - 18, 'R');
        }
        cv.line(x - 6, y - 26, x + 6, y - 26, 't');
    }
}

/// A crow, perched or flapping: rook-black, with a grey beak and a
/// sheen on the wing so it reads against the night.
fn crow(cv: &mut Img, x: i32, y: i32, frame: u64) {
    cv.ellipse(x as f32, y as f32, 4.0, 3.0, 'X');
    cv.ellipse(x as f32, (y - 1) as f32, 3.0, 1.0, 'g');
    cv.rect(x + 3, y - 4, 4, 4, 'X');
    cv.rect(x + 7, y - 3, 2, 1, 'J');
    cv.put(x + 5, y - 3, 'h');
    cv.line(x - 4, y + 1, x - 7, y + 2, 'g');
    if frame % 2 == 0 {
        cv.line(x - 2, y - 2, x - 6, y - 7, 'j');
        cv.line(x, y - 2, x - 1, y - 8, 'j');
    }
    cv.put(x - 1, y + 3, 'G');
    cv.put(x + 1, y + 3, 'G');
}

/// One of the scarecrow's clothes, lying where the crows dropped it.
fn piece(cv: &mut Img, i: usize, x: i32, y: i32) {
    match i {
        0 => {
            cv.ellipse(x as f32, (y + 1) as f32, 8.0, 2.0, 'j');
            cv.rect(x - 5, y - 6, 11, 7, 'J');
            cv.rect(x - 4, y - 5, 4, 2, 'h');
            cv.put(x + 2, y - 3, 'G');
        }
        1 => {
            // A coat of goose feathers, sleeves out.
            cv.rect(x - 6, y - 10, 13, 13, 'U');
            cv.rect(x - 11, y - 10, 5, 8, 'U');
            cv.rect(x + 7, y - 10, 5, 8, 'U');
            for k in 0..6 {
                cv.line(x - 5 + k * 2, y - 8, x - 5 + k * 2, y + 2, if k % 2 == 0 { 'v' } else { 'u' });
            }
            cv.line(x, y - 10, x, y + 2, 'S');
            cv.rect(x - 2, y - 12, 5, 2, 'v');
        }
        _ => {
            // One left boot, rather chewed.
            cv.rect(x - 2, y - 10, 6, 10, 'b');
            cv.rect(x - 7, y - 3, 6, 4, 'b');
            cv.line(x - 2, y - 10, x + 3, y - 10, 'I');
            for k in [-8, -6, -4] {
                cv.put(x, y + k, 'o');
            }
            cv.line(x - 7, y, x + 3, y, 'n');
        }
    }
}

/// The fletcher: a leather apron, a felt hat with a goose quill in it,
/// an arrow in hand being sighted down.
fn fletcher(cv: &mut Img, x: i32, y: i32, tick: u64) {
    let look = ((tick / 40) % 3) as i32 - 1;
    cv.rect(x - 4, y - 2, 3, 3, 'b');
    cv.rect(x + 1, y - 2, 3, 3, 'b');
    cv.rect(x - 5, y - 15, 10, 13, 'P');
    cv.rect(x - 4, y - 13, 8, 11, 'r');
    cv.line(x - 4, y - 13, x + 3, y - 13, 'B');
    cv.rect(x - 6, y - 14, 2, 8, 'P');
    cv.rect(x + 4, y - 14, 2, 8, 'P');
    cv.rect(x - 3, y - 22, 7, 7, 't');
    cv.put(x - 1 + look, y - 19, 'k');
    cv.put(x + 2 + look, y - 19, 'k');
    cv.rect(x - 5, y - 24, 11, 3, 'I');
    cv.rect(x - 3, y - 27, 7, 3, 'I');
    cv.line(x + 4, y - 26, x + 9, y - 33, 'v');
    cv.line(x + 5, y - 26, x + 9, y - 31, 'U');
    // The arrow, held up to the eye.
    cv.line(x + 6, y - 10, x + 13, y - 22, 'O');
    cv.put(x + 13, y - 23, 'V');
}

/// The Quiver Coach, standing by its stair: a little covered cart with a
/// quiver painted on the side.
fn coach(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y - 14, 22, 11, 'B');
    cv.rect(x + 1, y - 13, 20, 9, 'r');
    for k in 0..11 {
        cv.line(x - 1 + k / 4, y - 15 - k / 3, x + 22 - k / 4, y - 15 - k / 3, if k % 3 == 0 { 'o' } else { 'T' });
    }
    cv.rect(x + 8, y - 12, 5, 7, 'b');
    for k in 0..3 {
        cv.line(x + 9 + k, y - 15, x + 9 + k, y - 12, 'h');
    }
    for wx in [x + 4, x + 17] {
        cv.ellipse(wx as f32, (y - 2) as f32, 3.0, 3.0, 'b');
        cv.put(wx, y - 2, 'o');
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    if !b::PLACES.contains(&kind) {
        return;
    }
    let (stage, clothes) = b::progress(&run.home.countryside, &run.marks);
    // The clothes still out, and a crow keeping each company.
    for (i, &(room, p, _)) in CLOTHES.iter().enumerate() {
        if room != kind || stage == 2 || clothes & (1 << i) != 0 {
            continue;
        }
        let (x, y) = at(p.0 * 2.0, p.1 * 2.0);
        // The tin hat sits on the east pole's top; the rest lie about.
        let lift = if kind == RoomKind::LongbowGreen { y - (pole_foot() - POLE) + 2 } else { 0 };
        piece(cv, i, x, y - lift);
        crow(cv, x + 13, y - lift - 4, run.tick / 8);
        if stage == 1 {
            super::near_prompt(cv, run, (x, y), "TAKE");
        }
    }
    match kind {
        RoomKind::LongbowGreen => green(cv, run),
        RoomKind::Fletchery => {
            let (x, y) = at(FLETCHER.0 * 2.0, FLETCHER.1 * 2.0);
            fletcher(cv, x, y - 4, run.tick);
            super::near_prompt(cv, run, (x, y - 10), "TALK");
            let (px_, py, pw, _) = px(PEN);
            for k in 0..4 {
                let gx = px_ + 12 + k * 22 + (hash(k, (run.tick / 30) as i32, 191) % 5) as i32;
                let face = if hash(k, (run.tick / 45) as i32, 192).is_multiple_of(2) { 1 } else { -1 };
                let peck = (run.tick / 6 + k as u64).is_multiple_of(5);
                goose(cv, gx.min(px_ + pw - 8), py + 26, face, peck);
            }
            if run.home.countryside.has(b::PLOT) {
                let (c, r, _, _) = COACH;
                coach(cv, (c - 2) * TILE + 6, (r + 1) * TILE + 2);
            }
        }
        RoomKind::Scarecrow => {
            let (x, y) = at(CLIENT.0 * 2.0, CLIENT.1 * 2.0);
            let base = y - 10;
            scarecrow(cv, x, base, stage == 2, run.tick);
            if stage < 2 {
                // His picket sign.
                let w = super::super::ink::text_width("ON STRIKE") + 6;
                cv.line(x + 24, base, x + 24, base - 28, 'r');
                super::super::chrome::inset_panel(cv, (x + 14, base - 40, w, 12), super::super::chrome::Tone::PAINTED);
                cv.text(x + 17, base - 38, "ON STRIKE", 'b');
            }
            // The prompt stands off to his left, clear of his straw.
            super::near_prompt(cv, run, (x - 30, y + 4), match stage { 0 => "ASK", 1 if clothes == 7 => "DRESS", _ => "TALK" });
            // Crows on the stooks, bolder while he's off duty.
            let n = if stage == 2 { 1 } else { 4 };
            for k in 0..n {
                let (c, r, w, _) = STOOKS[k];
                let cx = c * TILE + 6 + (hash(k as i32, 5, 193) % (w as u32 * TILE as u32 - 12)) as i32;
                let hop = ((run.tick / 7 + k as u64 * 3) % 9 == 0) as i32 * 2;
                crow(cv, cx, r * TILE + 2 - hop, run.tick / 9 + k as u64);
            }
        }
        _ => {}
    }
}

/// The green in play: the popinjay on its rope, the wind, the archer's
/// bow and the arrow in the air; at rest, the popinjay parked by its pole.
fn green(cv: &mut Img, run: &Run) {
    let rope_y = (ROPE * TILE as f32) as i32;
    let round = run.pastimes.round.as_ref().filter(|r| r.game == Game::Longbow);
    let wind = round.map_or(0, |r| Longbow::wind(r.progress));
    pennant(cv, (POLES.0 * TILE as f32) as i32, pole_foot() - POLE - 12, wind);
    let Some(r) = round else {
        popinjay(cv, (POLES.0 * TILE as f32) as i32 + 10, rope_y - 2);
        return;
    };
    let Some(bw) = r.longbow.as_ref() else {
        return;
    };
    popinjay(cv, Longbow::target(r) * TILE / 256, rope_y - 2);
    // Where an arrow loosed now would come down, wind and all.
    if bw.flight.is_none() && bw.wait == 0 {
        let land = (bw.aim + wind * LONGBOW_FLIGHT as i32) * TILE / 256;
        for k in 0..3 {
            cv.line(land - k, rope_y + 6 + k, land + k, rope_y + 6 + k, '5');
        }
    }
    if let Some(a) = &bw.flight {
        arrow(cv, a.x * TILE / 256, a.y() * TILE / 256);
    }
    if let Some(hero) = run.players.get(&bw.owner) {
        let (x, y) = at(hero.x, hero.y);
        bow(cv, x, y, bw.draw);
    }
    // Arrows left, as fletchings on the quiver stand.
    let left = LONGBOW_ARROWS.saturating_sub(bw.arrows);
    let (qx, qy, _, _) = px(QUIVERS);
    for k in 0..left as i32 {
        cv.rect(qx + 2 + k * 4, qy - 10, 2, 4, '5');
    }
}

/// Whether the field's errand board is up.
pub(super) fn board_visible(run: &Run) -> bool {
    run.pastimes.round.is_none()
        && run.room().kind == RoomKind::Scarecrow
        && super::knight_near(run, CLIENT, 3.5)
}

pub(super) fn boards(cv: &mut Img, run: &Run) {
    if !board_visible(run) {
        return;
    }
    let (stage, clothes) = b::progress(&run.home.countryside, &run.marks);
    let mut lines: Vec<(String, char)> = vec![("THE SCARECROW".into(), '9')];
    match stage {
        0 => lines.push(("E: ASK HIM".into(), 'O')),
        1 => {
            let missing: Vec<&str> = CLOTHES
                .iter()
                .enumerate()
                .filter(|(i, _)| clothes & (1 << i) == 0)
                .map(|(_, c)| c.2)
                .collect();
            if missing.is_empty() {
                lines.push(("E: DRESS HIM".into(), 'O'));
            } else {
                lines.push(("FIND HIS CLOTHES".into(), 'O'));
                lines.push((format!("{}/3 FOUND", 3 - missing.len()), 'h'));
            }
        }
        _ => lines.push(("BACK AT WORK".into(), 'h')),
    }
    cv.stamp(&home::board(&lines), 4, 13);
}
