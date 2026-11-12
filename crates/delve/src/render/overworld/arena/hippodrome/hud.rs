//! The heads-up display, in plates of the realm's own: the lap and place at the left, the ladder
//! of the whole field along the top, the team's wind and iron at the right, the keys along the
//! foot, and the course in a corner. Everything that is a word comes from the plan.
use super::super::super::ink::Img;
use super::moments::{self, Plan, Rect, SCREEN};
use super::view::View;
use super::{art, world};
use crate::rules::chariot::{LAPS, Phase, show::Kind};
use crate::rules::HZ;

/// A dark plate with a lit rim and a pixel off each corner.
fn plate(im: &mut Img, (x, y, w, h): Rect) {
    for yy in y..y + h {
        for xx in x..x + w {
            let corner = (xx == x || xx == x + w - 1) && (yy == y || yy == y + h - 1);
            if !corner {
                im.put(xx, yy, 'K');
            }
        }
    }
    for xx in x + 1..x + w - 1 {
        im.put(xx, y, 'g');
        im.put(xx, y + h - 1, 'k');
    }
    for yy in y + 1..y + h - 1 {
        im.put(x, yy, 'g');
        im.put(x + w - 1, yy, 'k');
    }
}

/// A rank arrow: a little triangle, up in green or down in red.
fn arrow(over: &mut Img, x: i32, y: i32, up: bool) {
    let rows = ["...#...", "..###..", ".#####.", "#######"];
    for (j, row) in rows.iter().enumerate() {
        let dy = if up { j as i32 } else { 3 - j as i32 };
        for (i, c) in row.chars().enumerate() {
            if c == '#' {
                over.put(x + i as i32, y + dy, if up { 'Y' } else { '7' });
            }
        }
    }
}

/// A caltrop's four points.
fn spike(im: &mut Img, x: i32, y: i32, ink: char) {
    for (dx, dy) in [(0, -2), (-2, 0), (2, 0), (0, 2), (0, 0), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        im.put(x + dx, y + dy, ink);
    }
}

/// The field in a row along the top: the course's three laps run left to right, a dot for every
/// team in its ink, the viewer's ringed, the leader crowned, the flag at the end.
fn ladder(over: &mut Img, v: &View) {
    let (x, y, w, h) = (116, 2, 152, 14);
    plate(over, (x, y, w, h));
    let (x0, x1, row) = (x + 8, x + w - 14, y + 8);
    for xx in x0..=x1 {
        over.put(xx, row, if xx % 2 == 0 { 'j' } else { 'g' });
    }
    for lap in 0..=LAPS as i32 {
        let tx = x0 + (x1 - x0) * lap / LAPS as i32;
        over.rect(tx, row - 3, 1, 7, 'G');
    }
    // The flag: four squares.
    for (dx, dy) in [(0, 0), (1, 1), (1, 0), (0, 1)] {
        over.rect(x1 + 4 + dx * 3, row - 4 + dy * 3, 3, 3, if (dx + dy) % 2 == 0 { 'i' } else { 'k' });
    }
    let total = v.course().lap * LAPS as f32;
    let order = v.race.order();
    let lead = order.first().copied().unwrap_or(0);
    // The back of the field first, so the viewer's dot and the leader's lie on top.
    for &i in order.iter().rev() {
        let r = &v.race.racers[i];
        let k = (r.dist / total).clamp(0.0, 1.0);
        let tx = x0 + ((x1 - x0) as f32 * k) as i32;
        let ink = v.ink(i);
        if v.mine && i == v.me {
            over.rect(tx - 3, row - 3, 7, 7, 'k');
            over.rect(tx - 2, row - 2, 5, 5, ink);
            over.rect(tx - 1, row - 1, 3, 3, 'H');
            over.rect(tx - 1, row - 6, 3, 1, 'H');
            over.rect(tx, row - 5, 1, 1, 'H');
        } else {
            over.rect(tx - 2, row - 2, 5, 5, 'k');
            over.rect(tx - 1, row - 1, 3, 3, ink);
        }
        if i == lead {
            for (dx, dy, c) in [(-2, -6, '6'), (0, -7, '6'), (2, -6, '6'), (-1, -5, '4'), (0, -5, '4'), (1, -5, '4')] {
                over.put(tx + dx, row + dy + if v.mine && i == v.me { -2 } else { 0 }, c);
            }
        }
    }
}

/// The team's wind and iron at the right: ten blocks of breath, and the spikes left in the bag.
fn team_plate(over: &mut Img, v: &View) {
    let (x, y, w, h) = (SCREEN.0 - 70, 2, 66, 22);
    plate(over, (x, y, w, h));
    let r = &v.race.racers[v.me];
    let blown = r.wind < 0.25;
    let n = 10;
    let filled = ((r.wind * n as f32).round() as i32).clamp(0, n);
    let flick = blown && (v.race.clock / 4) % 2 == 0;
    // A little whip beside the bar.
    over.line(x + 5, y + 8, x + 8, y + 3, 'o');
    over.line(x + 8, y + 3, x + 6, y + 2, 'O');
    over.rect(x + 3, y + 8, 3, 3, 'B');
    for i in 0..n {
        let ink = if i < filled { if blown { if flick { '7' } else { 'R' } } else if i < 3 { 'o' } else { 'O' } } else { 'j' };
        over.rect(x + 12 + i * 5, y + 4, 4, 6, ink);
    }
    for k in 0..3 {
        spike(over, x + 14 + k * 12, y + 16, if (k as u8) < r.caltrops { 'h' } else { 'g' });
    }
}

/// The course in its corner, the teams dotted on it.
fn minimap(over: &mut Img, v: &View) {
    let world = world::world(v.race.track);
    let (bw, bh) = (world.mini.w, world.mini.h);
    let (x0, y0) = (SCREEN.0 - bw - 4, SCREEN.1 - bh - 4);
    over.stamp(&world.mini, x0, y0);
    let (ox, oy, s) = world.mini_xf;
    let course = v.course();
    let dot = |over: &mut Img, i: usize, size: i32, ink: char| {
        let r = &v.race.racers[i];
        let (tx, ty) = crate::rules::chariot::pos_on(course, r.dist + r.coast, r.lane);
        let (px, py) = (x0 + (ox + tx * s) as i32, y0 + (oy + ty * s) as i32);
        over.rect(px - size / 2, py - size / 2, size, size, ink);
    };
    for i in 0..v.race.racers.len() {
        if !(v.mine && i == v.me) {
            dot(over, i, 2, v.ink(i));
        }
    }
    // The viewer blinks a little, so they are found.
    let blink = (v.race.clock / 6) % 2 == 0;
    dot(over, v.me, 5, 'k');
    dot(over, v.me, 3, '9');
    if blink {
        dot(over, v.me, 1, v.ink(v.me));
    }
}

/// The keys along the foot, one plain word each.
fn keys(over: &mut Img) {
    for pr in moments::prompts() {
        for &((x, y, w, h), key) in &pr.caps {
            over.rect(x, y, w, h - 2, 'X');
            over.rect(x, y, w, 1, 'j');
            over.rect(x, y + h - 2, w, 2, 'k');
            over.text(x + 3, y + 2, key, 'i');
        }
    }
}

/// The red rim of a wreck of your own, and the pale of the first home.
fn rims(over: &mut Img, v: &View) {
    let (w, h) = (over.w, over.h);
    let now = v.now;
    for f in v.race.show.fx.iter().filter(|f| v.is_me(f.team)) {
        let age = f.age(now);
        if f.kind == Kind::Wreck && age < 20 {
            super::super::juice::vignette(over, (0, 0, w, h), 1.0 - age as f32 / 20.0, if f.weight >= crate::rules::juice::Weight::Kill { 16 } else { 11 }, '8');
        }
    }
}

/// The whole display over the race: the plates and what the plan holds.
pub(super) fn draw(over: &mut Img, v: &View, plan: &Plan) {
    let race = v.race;
    rims(over, v);
    if race.phase != Phase::Gate {
        ladder(over, v);
        team_plate(over, v);
        // The lap's pips under the word, the place big, and the arrow of a place won or lost.
        let lap = (race.racers[v.me].lap(v.course()) + 1).min(LAPS);
        for k in 0..LAPS {
            let (px, py) = (9 + k as i32 * 8, 16);
            let lit = k < lap;
            over.rect(px, py, 6, 3, if lit { 'O' } else { 'j' });
            if lit {
                over.rect(px + 1, py, 4, 1, 't');
            }
        }
        let now = v.now;
        let up = race.show.fx.iter().filter(|f| matches!(f.kind, Kind::Pass | Kind::Lead) && v.is_me(f.team)).map(|f| f.age(now)).min();
        let down = race.show.fx.iter().filter(|f| matches!(f.kind, Kind::Pass | Kind::Lead) && v.is_me(f.other)).map(|f| f.age(now)).min();
        let flick = |age: u32| age < 40 && (age >= 12 || age % 4 < 2);
        if let Some(a) = up.filter(|&a| flick(a)) {
            arrow(over, 46, 26 - (a as i32 / 6).min(2), true);
        } else if let Some(a) = down.filter(|&a| flick(a)) {
            arrow(over, 46, 26 + (a as i32 / 6).min(2), false);
        }
        for b in &plan.bigs {
            let img = super::super::juice::big_text_img(&b.text, b.scale, b.ink, 'B');
            over.stamp(&img, b.at.0, b.at.1);
        }
        minimap(over, v);
    }
    if plan.words.iter().any(|w| w.kind == "key") {
        keys(over);
    }
    for w in &plan.words {
        moments::draw_word(over, w);
    }
    let _ = (art::STRIDE, HZ);
}
