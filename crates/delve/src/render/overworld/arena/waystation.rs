//! The waystations drawn (`crate::world::waystations`), standing beside the
//! kingdom's roads: a lean-to with its keeper and kettle, the coach waiting
//! at its post, the rally pole, and a lantern on a post. The pole's pennon
//! flies while a rally is called there, the lantern burns while a coach ran
//! or a rally stands, and a puff of dust rises at the post when a coach has
//! just come or gone: light and motion mean something is happening. The
//! keeper's talk is drawn as a panel under the view.

use super::super::fx::{self, Hue};
use super::*;
use crate::rules::waystation::REACH;
use crate::world::waystations::{self as halts, STOPS};

fn art(rows: &[&str]) -> Img {
    let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
    let mut im = Img::new(w, rows.len() as i32);
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            if c != '.' {
                im.put(x as i32, y as i32, c);
            }
        }
    }
    im
}

/// The coat a keeper wears: a dusk ink, a different one down the ring.
fn coat(stop: usize) -> char {
    ['Q', 'C', 'z', 'M', 'U', 'l', 'Q', 'C', 'z', 'M', 'U'][stop % 11]
}

/// A keeper facing the road, a cap on, a kettle-apron over the coat.
fn keeper(stop: usize) -> Img {
    let c = coat(stop);
    let rows = [
        "..bbbb..",
        ".bBBBBb.",
        ".bbbbbb.",
        ".tTTTTt.",
        ".tOTTOt.",
        "..TTTT..",
        ".cccccc.",
        "cccTTccc",
        "cc.TT.cc",
        "tc.TT.ct",
        "...TT...",
        "..b..b..",
        ".bb..bb.",
    ];
    let rows: Vec<String> = rows.iter().map(|r| r.replace('c', &c.to_string())).collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    art(&refs)
}

/// The lean-to: its roof, posts, back wall with the notice, and the counter.
fn shelter(over: &mut Img, (x, y): (i32, i32), tick: u32, kettle: bool) {
    // The roof, shingled in rows of the timber ramp, its edge lit a step.
    over.rect(x + 12, y + 8, 56, 1, 'o');
    over.rect(x + 12, y + 9, 56, 3, 'R');
    over.rect(x + 12, y + 12, 56, 4, 'r');
    over.rect(x + 12, y + 16, 56, 3, 'B');
    over.rect(x + 12, y + 19, 56, 1, 'b');
    for k in 0..11 {
        over.put(x + 14 + k * 5, y + 11, 'r');
        over.put(x + 16 + k * 5, y + 14, 'B');
        over.put(x + 13 + k * 5, y + 17, 'b');
    }
    // Under the eaves, dark; the back wall's planks.
    over.rect(x + 14, y + 20, 52, 2, 'n');
    over.rect(x + 19, y + 22, 42, 24, 'b');
    for k in 0..7 {
        over.rect(x + 23 + k * 6, y + 22, 1, 24, 'I');
    }
    // Posts on stone feet.
    for px in [16, 62] {
        over.rect(x + px, y + 20, 3, 26, 'P');
        over.rect(x + px + 2, y + 20, 1, 26, 'B');
        over.rect(x + px - 1, y + 44, 5, 3, 'G');
        over.rect(x + px - 1, y + 46, 5, 1, 'X');
    }
    // The notice on the wall: a board and a paper, its lines.
    over.rect(x + 24, y + 25, 15, 11, 'P');
    over.rect(x + 25, y + 26, 13, 9, 'T');
    for k in 0..3 {
        over.rect(x + 27, y + 28 + k * 2, 9 - k, 1, 'B');
    }
    // The counter along the front.
    over.rect(x + 20, y + 37, 40, 1, 'o');
    over.rect(x + 20, y + 38, 40, 3, 'r');
    over.rect(x + 20, y + 41, 40, 5, 'B');
    for k in 0..6 {
        over.rect(x + 25 + k * 7, y + 41, 1, 5, 'I');
    }
    // A kettle on it, steaming a little while the keeper tends it.
    over.rect(x + 50, y + 33, 6, 4, 'X');
    over.rect(x + 50, y + 33, 6, 1, 'g');
    over.put(x + 56, y + 34, 'g');
    over.put(x + 49, y + 34, 'g');
    over.put(x + 52, y + 32, 'g');
    if kettle {
        let t = tick / 12;
        over.put(x + 52 + (t % 2) as i32, y + 30 - (t % 3) as i32, 'G');
        over.put(x + 53 - (t % 2) as i32, y + 28 - (t % 3) as i32, 'j');
    }
}

/// The coach waiting at its post, its cover a patched canvas on bows.
fn coach(over: &mut Img, (x, y): (i32, i32)) {
    // The canvas cover, arched, on bows.
    for row in 0..11 {
        let inset = [8, 5, 3, 2, 1, 0, 0, 0, 0, 0, 0][row as usize];
        over.rect(x + 68 + inset, y + 23 + row, 22 - 2 * inset, 1, if row < 2 { 'O' } else { 't' });
    }
    over.rect(x + 68, y + 33, 22, 1, 'o');
    for k in [0, 7, 14, 21] {
        over.line(x + 68 + k, y + 33, x + 68 + k.min(21).max(1), y + 25 + (k - 10).abs() / 4, 'r');
    }
    over.put(x + 74, y + 28, 'o');
    over.put(x + 82, y + 30, 'o');
    // The box under it, its rail and a lamp hook.
    over.rect(x + 66, y + 34, 26, 8, 'r');
    over.rect(x + 66, y + 34, 26, 1, 'o');
    over.rect(x + 66, y + 41, 26, 2, 'B');
    over.rect(x + 71, y + 36, 16, 4, 'B');
    over.line(x + 92, y + 36, x + 92, y + 39, 'X');
    // Shafts, down to the ground at the near end.
    over.line(x + 66, y + 40, x + 60, y + 44, 'P');
    over.line(x + 66, y + 38, x + 60, y + 43, 'B');
    // Two wheels, an iron rim and a hub.
    for wx in [72, 86] {
        for k in 0..24 {
            let a = k as f32 * std::f32::consts::TAU / 24.0;
            over.put(x + wx + (a.cos() * 4.5) as i32, y + 44 + (a.sin() * 4.5) as i32, 'X');
        }
        over.put(x + wx, y + 44, 'o');
        over.line(x + wx - 3, y + 44, x + wx + 3, y + 44, 'g');
        over.line(x + wx, y + 41, x + wx, y + 47, 'g');
    }
}

/// The rally pole on its stone, with its pennon: flying while a rally stands
/// here, furled at half height otherwise.
fn pole(over: &mut Img, (x, y): (i32, i32), tick: u32, called: bool) {
    over.rect(x + 3, y + 44, 9, 3, 'G');
    over.rect(x + 3, y + 46, 9, 1, 'X');
    over.rect(x + 6, y - 14, 2, 59, 'P');
    over.rect(x + 7, y - 14, 1, 59, 'B');
    over.put(x + 6, y - 15, 'o');
    over.put(x + 7, y - 15, 'O');
    if called {
        // Flying: a long pennon rippling, steel blue with a lit edge.
        let wave = [0, 1, 1, 0, -1, -1][(tick / 5 % 6) as usize];
        for k in 0..16 {
            let drop = (k * 4 / 16) + if k > 4 { wave * (k / 6) } else { 0 };
            let h = 9 - k / 2;
            over.rect(x + 8 + k, y - 13 + drop, 1, h.max(2), if k % 5 == 4 { 'z' } else { 'Q' });
            over.put(x + 8 + k, y - 13 + drop, 'z');
            over.put(x + 8 + k, y - 14 + drop + h.max(2), 'q');
        }
    } else {
        // Furled and drooping at half height: a short cloth hung from the pole.
        over.rect(x + 8, y + 20, 4, 12, 'u');
        over.rect(x + 8, y + 20, 4, 1, 'U');
        over.rect(x + 8, y + 31, 4, 1, 'S');
        over.put(x + 9, y + 32, 'x');
    }
}

/// The lantern on its post: iron and dark glass while nothing is happening;
/// a warm light and its pool while it burns.
fn lantern(over: &mut Img, (x, y): (i32, i32), burning: bool, tick: u32) {
    over.rect(x + 103, y + 16, 2, 30, 'P');
    over.rect(x + 104, y + 16, 1, 30, 'B');
    over.rect(x + 100, y + 44, 8, 3, 'G');
    over.rect(x + 100, y + 46, 8, 1, 'X');
    // The lantern box and its cap.
    over.rect(x + 100, y + 7, 8, 9, 'X');
    over.rect(x + 99, y + 6, 10, 2, 'g');
    over.put(x + 103, y + 5, 'g');
    over.put(x + 104, y + 5, 'g');
    if burning {
        let flick = if hash(tick as i32 / 3, 5, 41) % 4 == 0 { '5' } else { '6' };
        over.rect(x + 101, y + 9, 6, 5, '4');
        over.rect(x + 102, y + 10, 4, 3, flick);
        fx::glow(over, x + 104, y + 11, Hue::Warm);
        fx::glow(over, x + 104, y + 20, Hue::Warm);
    } else {
        over.rect(x + 101, y + 9, 6, 5, 's');
        over.put(x + 102, y + 10, 'S');
    }
}

/// The ground under a halt: trodden earth, thin at its edges.
fn ground(over: &mut Img, (x, y): (i32, i32)) {
    for py in 33..56 {
        for px in 0..112 {
            let (dx, dy) = ((px - 56) as f32 / 56.0, (py - 46) as f32 / 10.0);
            let d = dx * dx + dy * dy;
            if d > 1.0 {
                continue;
            }
            let h = hash(px + (x & 63), py + (y & 63), 17) % 8;
            let thin = (d * 8.0) as u32;
            if h + thin < 5 {
                over.put(x + px, y + py, if h % 2 == 0 { 'Z' } else { 'b' });
            }
        }
    }
}

/// The halts in view, painted onto `over` (whose top-left is world pixel
/// `origin`), under everything that moves: a knight walks in front of the
/// counter.
pub(super) fn draw(over: &mut Img, run: &Run, origin: (i32, i32)) {
    let tick = run.tick as u32;
    for p in halts::placed() {
        let (x, y) = (p.rect.tx * TILE - origin.0, p.rect.ty * TILE - origin.1);
        if x < -120 || y < -80 || x > NATIVE_W + 120 || y > NATIVE_H + 80 {
            continue;
        }
        let stops = &run.kingdom.stops;
        let burning = stops.burning(p.stop, run.tick);
        let called = stops.rally.is_some_and(|r| usize::from(r.stop) == p.stop);
        ground(over, (x, y));
        shelter(over, (x, y), tick, true);
        coach(over, (x, y));
        pole(over, (x, y), tick, called);
        lantern(over, (x, y), burning, tick);
        // The keeper, in front of his counter.
        let man = keeper(p.stop);
        let (kx, ky) = (p.keeper.0 * TILE + TILE / 2 - origin.0, (p.keeper.1 + 1) * TILE - origin.1);
        over.stamp(&man, kx - man.w / 2, ky - man.h);
        // Dust where a coach has just come or gone.
        if let Some((from, to, at)) = stops.rode {
            let since = run.tick.saturating_sub(at);
            if (usize::from(from) == p.stop || usize::from(to) == p.stop) && since < crate::rules::trinkets::MOMENT {
                let r = 4.0 + since as f32 * 0.6;
                let (cx, cy) = (x + 80, y + 47);
                for k in 0..14 {
                    let a = k as f32 * std::f32::consts::TAU / 14.0;
                    over.put(cx + (a.cos() * r) as i32, cy + (a.sin() * r * 0.4) as i32, 'P');
                }
            }
        }
    }
}

/// Each halt's name over its roof, and the key by the keeper a knight stands
/// at; returns the rectangles they take.
pub(super) fn marks(over: &mut Img, run: &Run, origin: (i32, i32), focus: Option<u32>) -> Vec<(i32, i32, i32, i32)> {
    let hero = run.players.get(&focus.unwrap_or(1));
    let mut taken = Vec::new();
    for p in halts::placed() {
        let (x, y) = (p.rect.tx * TILE - origin.0, p.rect.ty * TILE - origin.1);
        if x < -120 || y < -80 || x > NATIVE_W + 120 || y > NATIVE_H + 80 {
            continue;
        }
        let name = STOPS[p.stop].name.to_uppercase();
        let w = super::super::super::ink::text_width(&name);
        taken.push(chrome::label(over, (x + 58 - w / 2, y - 2), &name, 'h', chrome::Tone::SLATE));
        let near = hero.is_some_and(|h| {
            let (kx, ky) = ((p.keeper.0 as f32 + 0.5) * TILE_UNITS, (p.keeper.1 as f32 + 0.5) * TILE_UNITS);
            (h.x - kx).hypot(h.y - ky) < REACH * TILE_UNITS
        });
        if near && !run.kingdom.stops.talks.contains_key(&focus.unwrap_or(1)) {
            let label = "E: TALK";
            let tw = super::super::super::ink::text_width(label);
            let kx = p.keeper.0 * TILE + TILE / 2 - origin.0;
            taken.push(chrome::label(over, (kx - tw / 2, y - 16), label, 'O', chrome::Tone::KEY));
        }
        taken.push((x, y + 4, 112, 52));
    }
    taken
}

/// The keeper's talk with the knight, as a panel under the view.
pub(super) fn talk(ui: &mut Img, run: &Run, focus: Option<u32>) {
    let id = focus.unwrap_or(1);
    let Some(view) = run.stop_talk_view(id) else {
        return;
    };
    let stop = run.kingdom.stops.talks.get(&id).map_or(0, |t| usize::from(t.stop));
    let wrap = |s: &str, n: usize| {
        let mut out = Vec::new();
        let mut row = String::new();
        for word in s.split_whitespace() {
            if !row.is_empty() && row.len() + 1 + word.len() > n {
                out.push(std::mem::take(&mut row));
            }
            if !row.is_empty() {
                row.push(' ');
            }
            row.push_str(word);
        }
        if !row.is_empty() {
            out.push(row);
        }
        out
    };
    let lines = wrap(&view.words, 50);
    let h = (lines.len() as i32 * 9 + view.answers.len() as i32 * 10 + 30).max(62);
    let y = ui.h - h - 6;
    chrome::panel(ui, (6, y, ui.w - 12, h), chrome::Tone::FRAMED);
    chrome::ring(ui, (6, y, ui.w - 12, h), 3);
    let face = super::super::resized(&keeper(stop), 3.0);
    chrome::inset_panel(ui, (12, y + 8, face.w + 6, face.h + 6), chrome::Tone::WELL);
    ui.stamp(&face, 15, y + 11);
    let tx = 24 + face.w;
    ui.text(tx, y + 7, &view.keeper.to_uppercase(), '9');
    ui.text(tx + super::super::super::ink::text_width(view.keeper) + 12, y + 7, &view.halt.to_uppercase(), 'J');
    for (i, line) in lines.iter().enumerate() {
        ui.text(tx, y + 19 + i as i32 * 9, line, 'i');
    }
    let ay = y + 22 + lines.len() as i32 * 9;
    for (i, answer) in view.answers.iter().enumerate() {
        let spent = answer.starts_with("3: NO ONE") || answer.starts_with("3: THE RALLY");
        ui.text(tx, ay + i as i32 * 10, answer, if spent { 'G' } else { 'O' });
    }
}
