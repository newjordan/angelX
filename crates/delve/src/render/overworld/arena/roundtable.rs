//! The Round Table adventures' stage: the dispatch to whichever is under way,
//! and the labelled block scene a stub draws (a solid backdrop, its name, one
//! box per seat) until its builder draws the real one.
//!
//! The arena (`arena.rs`) and the cockpit's cell painter pin their own hashes
//! (`assets/arcade/cell-proof.json`), so a builder never edits them: both
//! reach an adventure only through this file, which reaches it only through
//! its own `arena/<slug>.rs`. This file is the integrator's, not a builder's.
use super::super::ink::{Img, text_width};
use crate::rules::{Run, classes};

/// The colours of a stub scene: palette letters (`ink::ink`), never the
/// signal bank.
pub(super) struct Look {
    pub name: &'static str,
    pub backdrop: char,
    pub edge: char,
    pub accent: char,
}

/// The layers (base, fx) of the adventure under way, at the native 384 by 224.
pub(super) fn layers(run: &Run, focus: Option<u32>) -> Option<(Img, Img)> {
    Some(match run.roundtable_under_way()?.slug {
        "lanternward" => super::lanternward::layers(run, focus),
        "moonfall" => super::moonfall::layers(run, focus),
        "bannerpeak" => super::bannerpeak::layers(run, focus),
        "griffinwing" => super::griffinwing::layers(run, focus),
        "lightgallop" => super::lightgallop::layers(run, focus),
        _ => return None,
    })
}

/// The same for the terminal's cell painter.
pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> Option<(Img, Img)> {
    Some(match run.roundtable_under_way()?.slug {
        "lanternward" => super::lanternward::terminal_layers(run, focus),
        "moonfall" => super::moonfall::terminal_layers(run, focus),
        "bannerpeak" => super::bannerpeak::terminal_layers(run, focus),
        "griffinwing" => super::griffinwing::terminal_layers(run, focus),
        "lightgallop" => super::lightgallop::terminal_layers(run, focus),
        _ => return None,
    })
}

/// The native scene fitted to a `w` by `h` frame, centred, nearest sample.
pub(super) fn frame(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Option<Img> {
    let (im, _) = layers(run, focus)?;
    let mut out = Img::black(w.max(0), h.max(0));
    if w <= 0 || h <= 0 {
        return Some(out);
    }
    let scale = (i64::from(w) * 224).min(i64::from(h) * 384);
    let (dw, dh) = ((scale / 224) as i32, (scale / 384) as i32);
    let (ox, oy) = ((w - dw) / 2, (h - dh) / 2);
    for y in 0..dh {
        for x in 0..dw {
            if let Some(c) = im.get(x * 384 / dw, y * 224 / dh) {
                out.set(ox + x, oy + y, c);
            }
        }
    }
    Some(out)
}

/// Every adventure's door on the kingdom map, in the kingdom's own layer.
pub(super) fn kingdom(im: &mut Img, run: &Run, origin: (i32, i32)) {
    super::lanternward::kingdom(im, run, origin);
    super::moonfall::kingdom(im, run, origin);
    super::bannerpeak::kingdom(im, run, origin);
    super::griffinwing::kingdom(im, run, origin);
    super::lightgallop::kingdom(im, run, origin);
}

/// Words on a dark plate, so they read on any ground.
pub(super) fn label(im: &mut Img, x: i32, y: i32, s: &str, ink: char) {
    im.rect(x - 1, y - 1, text_width(s) + 2, 9, 'k');
    im.text(x, y, s, ink);
}

/// `s` centred on column `cx`.
fn centred(im: &mut Img, cx: i32, y: i32, s: &str, ink: char) {
    label(im, cx - text_width(s) / 2, y, s, ink);
}

/// `s` at `k` times the font's size on a dark plate, left edge at `x`: the
/// terminal's cells are four pixels wide, so only large words read there.
fn large(im: &mut Img, x: i32, y: i32, s: &str, ink: char, k: i32) {
    let mut words = Img::new(text_width(s), 7);
    words.text(0, 0, s, ink);
    im.rect(x - 2, y - 2, words.w * k + 4, 7 * k + 4, 'k');
    im.stamp_times(&words, x, y, k);
}

/// Letters and digits the font has, in capitals, at most `n` of them.
fn plain(s: &str, n: usize) -> String {
    s.to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .take(n)
        .collect()
}

/// The stub stage: a solid backdrop, the name large, "stub adventure" under
/// it, one labelled box for every seated knight on the knight's own screen
/// (the viewer's box lit), and what to hold to leave.
pub(super) fn stub_layers(run: &Run, focus: Option<u32>, look: &Look) -> (Img, Img) {
    let mut im = Img::black(384, 224);
    let fx = Img::new(384, 224);
    im.rect(6, 6, 372, 212, look.backdrop);
    im.frame(6, 6, 372, 212, look.edge);
    im.frame(8, 8, 368, 208, look.edge);
    // The name, three times the font's size.
    let name = look.name.to_uppercase();
    let mut title = Img::new(text_width(&name), 7);
    title.text(0, 0, &name, look.accent);
    im.stamp_times(&title, (384 - title.w * 3) / 2, 28, 3);
    let sub = "STUB ADVENTURE";
    large(&mut im, 192 - text_width(sub), 54, sub, 'H', 2);
    // A mark that slides along the rule under the title: the sim is ticking.
    let slide = (run.tick % 180) as i32 * 2;
    im.rect(12, 74, 360, 1, look.edge);
    im.rect(12 + slide, 73, 6, 3, look.accent);
    // One box per seat at the table, three to a row.
    let seats: Vec<u32> = run
        .players
        .keys()
        .copied()
        .filter(|id| !run.exits.outside.contains_key(id))
        .collect();
    for (n, id) in seats.iter().enumerate() {
        let (col, row) = ((n % 3) as i32, (n / 3) as i32);
        let (x, y) = (30 + col * 112, 88 + row * 52);
        let lit = focus == Some(*id);
        im.rect(x, y, 100, 46, 'k');
        im.frame(x, y, 100, 46, if lit { look.accent } else { look.edge });
        if lit {
            im.frame(x + 1, y + 1, 98, 44, look.accent);
        }
        large(&mut im, x + 6, y + 4, &format!("SEAT {id}"), 'H', 2);
        let hero = &run.players[id];
        let class = classes::of(hero).map_or("KNIGHT", |c| c.name);
        im.text(x + 6, y + 23, &plain(class, 14), look.accent);
        im.text(x + 6, y + 34, &plain(&hero.name, 14), 'v');
    }
    let hint = "HOLD ESC TO LEAVE";
    large(&mut im, 192 - text_width(hint), 196, hint, 'H', 2);
    (im, fx)
}

/// The adventure's door on the kingdom map: a post and a board in its colour,
/// its name over it and E under it. Drawn only while its tile is in view.
pub(super) fn stub_kingdom(im: &mut Img, origin: (i32, i32), entry: (i32, i32), look: &Look) {
    let (x, y) = (entry.0 * 16 + 8 - origin.0, entry.1 * 16 + 8 - origin.1);
    let name = look.name.to_uppercase();
    if x < -(text_width(&name) / 2) - 8
        || x > 392 + text_width(&name) / 2
        || !(-24..248).contains(&y)
    {
        return;
    }
    im.rect(x - 1, y - 4, 3, 12, 'P');
    im.rect(x - 7, y - 10, 15, 8, look.accent);
    im.frame(x - 7, y - 10, 15, 8, 'k');
    centred(im, x, y - 22, &name, 'H');
    centred(im, x, y + 10, "E", look.accent);
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__roundtable__tests.rs"]
mod tests;
