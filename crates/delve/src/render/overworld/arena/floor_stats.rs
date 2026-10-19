//! A floor's results on a seat's screen (`rules::floor_stats`): the
//! party's rows sliding in one after another with their counts ticking up,
//! the honours stamped on, the MVP crowned, and last the floor's most
//! impactful wish, opened like a seal.
//!
//! Black paper, the board's dark greys, one warm ink for what was won.
//! Every frame is the results and the steps they have shown: the host and
//! every page draw the same pixels, and the floor's plate takes over when
//! they are done (`floor_plates`).

use super::super::chrome::{self, Tone};
use super::super::ink::{BLACK, Img, hash, rgb, text_width};
use super::*;
use crate::rules::floor_stats::{End, Honour, Impact, Results, Row, SHOWN};

/// Steps a count takes to tick up, and a stamp to land.
const COUNT: u32 = 24;
const SLAM: u32 = 3;
/// Steps between one honour's stamp and the next.
const HONOUR_GAP: u32 = 5;
/// Steps a finished run's floor stands whole before its deeds come up,
/// and between one deed and the next.
const READ: u32 = 60;
const DEED_GAP: u32 = 10;
/// The most deeds the board lists; the rest are counted.
const DEEDS: usize = 6;
/// The board's columns: a heading and the right edge of its numbers.
const COLUMNS: [(&str, i32); 5] = [("KILLS", 136), ("DAMAGE", 176), ("TAKEN", 210), ("AIM", 240), ("GOLD", 268)];
/// Where a row's name and its honours begin, and the most of a name shown.
const NAME_X: i32 = 44;
const NAME_CHARS: usize = 11;
const HONOUR_X: i32 = 276;
/// The headings and the rows share the board between the headline and
/// the wish: the headings this high, the rows at most `ROW_H` each.
const BLOCK_TOP: i32 = 36;
const BLOCK_FOOT: i32 = 154;
const HEAD_H: i32 = 12;
const ROW_H: i32 = 26;
/// The wish's panel and the line under it.
const WISH: (i32, i32, i32, i32) = (40, 157, NATIVE_W - 80, 52);
const HINT_Y: i32 = 214;
/// Warm for what was won, greys for the rest.
const WARM: char = 't';
const WARM_DIM: char = 'o';
const FIGURE: char = 'i';
const SETTLED: char = 'h';
const LABEL: char = 'G';
const QUIET: char = 'g';
const ROW_FILL: char = 'k';
const MVP_FILL: char = 'K';
/// The wish's seal: dark grey in a brass rim, a black line inside it.
const SEAL: Tone = Tone {
    lip: None,
    fill: &[rgb(0x151614)],
    rim: chrome::BRASS_RIM,
    hairline: Some(BLACK),
};

/// When each part of the board comes in, in steps.
#[derive(Clone, Copy)]
struct Beats {
    n: u32,
    stagger: u32,
    honours: u32,
    mvp: u32,
    wish: u32,
    /// A finished run's deeds, after its floor has been read.
    deeds: u32,
}

impl Beats {
    fn of(results: &Results) -> Beats {
        let n = results.rows.len().max(1) as u32;
        let stagger = if n > 4 { 4 } else { 5 };
        let counted = 10 + (n - 1) * stagger + 3 + COUNT;
        let honours = counted + 3;
        let stamps = results.rows.iter().map(|r| r.honours.len() as u32).sum::<u32>();
        let mvp = honours + stamps * HONOUR_GAP + 3;
        let wish = mvp + if results.mvp.is_some() { 10 } else { 0 };
        let said = match &results.wish {
            Some(impact) => 6 + impact.wish.chars().count() as u32 + 3 + COUNT,
            None => 8,
        };
        let deeds = wish + said + READ;
        Beats { n, stagger, honours, mvp, wish, deeds }
    }

    fn row(&self, i: u32) -> u32 {
        10 + i.min(self.n) * self.stagger
    }

    /// When a row's counts start ticking up.
    fn count(&self, i: u32) -> u32 {
        self.row(i) + 3
    }
}

/// `t` of the way, eased out: quick to start, settling at the end.
fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// `value` as it stands `age` steps into a count that starts at `start`.
fn counted(value: u32, age: u32, start: u32) -> u32 {
    let t = age.saturating_sub(start) as f32 / COUNT as f32;
    (value as f32 * ease(t)).round() as u32
}

/// The results as a frame, `w` by `h`, `age` steps in.
pub fn frame(run: &Run, results: &Results, age: u32, w: i32, h: i32) -> Img {
    let native = board(run, results, age);
    if w == NATIVE_W && h == NATIVE_H {
        return native;
    }
    let mut out = Img::black(w.max(0), h.max(0));
    if w <= 0 || h <= 0 {
        return out;
    }
    let (tw, th) = if w * NATIVE_H <= h * NATIVE_W { (w, w * NATIVE_H / NATIVE_W) } else { (h * NATIVE_W / NATIVE_H, h) };
    if tw == 0 || th == 0 {
        return out;
    }
    let (ox, oy) = ((w - tw) / 2, (h - th) / 2);
    if tw < NATIVE_W * 2 / 3 {
        // Too small for the board's letters: its headline, its MVP and its
        // wish, in letters the size the screen has.
        compact(&mut out, results, (ox, oy, tw, th));
        return out;
    }
    for y in 0..th {
        for x in 0..tw {
            if let Some(c) = native.get(x * NATIVE_W / tw, y * NATIVE_H / th) {
                out.set(ox + x, oy + y, c);
            }
        }
    }
    out
}

/// The board at native size.
fn board(run: &Run, results: &Results, age: u32) -> Img {
    let mut cv = Img::black(NATIVE_W, NATIVE_H);
    let beats = Beats::of(results);
    let deeds_at = (!results.deeds.is_empty() && results.end != End::Cleared).then_some(beats.deeds);
    title(&mut cv, results, age, deeds_at.is_some_and(|at| age >= at));
    if let Some(d) = deeds_at.and_then(|at| age.checked_sub(at)) {
        deeds(&mut cv, &results.deeds, d);
        hint(&mut cv, run, results, age);
        return cv;
    }
    let (rh, head, top) = layout(results.rows.len());
    if age >= 8 {
        // Each heading over its own column, a short rule under it.
        for (label, right) in COLUMNS.into_iter().chain([("HONOURS", HONOUR_X + text_width("HONOURS") - 1)]) {
            let w = text_width(label);
            let x = right - w + 1;
            cv.text(x, head, label, LABEL);
            for dx in 0..w {
                cv.put(x + dx, head + 9, 'X');
            }
        }
    }
    let most = results.rows.iter().map(|r| r.line.damage).max().unwrap_or(0);
    for (i, r) in results.rows.iter().enumerate() {
        row(&mut cv, run, results, r, (i as u32, top + i as i32 * rh, rh), most, &beats, age);
    }
    wish(&mut cv, results.wish.as_ref(), &beats, age);
    hint(&mut cv, run, results, age);
    // The MVP's tag lands hard: the board jolts.
    let jolt = age.checked_sub(beats.mvp + SLAM).filter(|&d| d < 4 && results.mvp.is_some());
    if let Some(d) = jolt {
        cv = shifted(&cv, [1, -1, 1, 0][d as usize]);
    }
    // The board closes onto the floor's own plate.
    if results.end == End::Cleared {
        let left = u32::from(SHOWN).saturating_sub(age) as i32;
        if left < 7 {
            let bar = (7 - left) * NATIVE_H / 14;
            for y in (0..bar).chain(NATIVE_H - bar..NATIVE_H) {
                for x in 0..NATIVE_W {
                    cv.set(x, y, BLACK);
                }
            }
        }
    }
    cv
}

/// The rows' height, where the headings stand and where the first row
/// does, for `n` knights: headings and rows together, centred between the
/// headline and the wish.
fn layout(n: usize) -> (i32, i32, i32) {
    let n = n.max(1) as i32;
    let rh = ((BLOCK_FOOT - BLOCK_TOP - HEAD_H) / n).min(ROW_H);
    let head = BLOCK_TOP + (BLOCK_FOOT - BLOCK_TOP - HEAD_H - rh * n) / 2;
    (rh, head, head + HEAD_H)
}

/// The board's headline, its letters set down one a step, and the time
/// the floor took under it.
fn title(cv: &mut Img, results: &Results, age: u32, deeds: bool) {
    let text = headline(results);
    let shown = age.saturating_sub(1) as usize;
    let k = 2;
    let w = text_width(&text) * k;
    let x = (NATIVE_W - w) / 2;
    let set: String = text.chars().take(shown).collect();
    big(cv, x, 7, &set, FIGURE, k);
    // The letter landing now, a step warm.
    if shown >= 1 && shown <= text.chars().count() {
        let last: String = text.chars().nth(shown - 1).into_iter().collect();
        big(cv, x + (shown as i32 - 1) * 6 * k, 7, &last, WARM, k);
    }
    if age >= 10 {
        let secs = results.ticks / u64::from(crate::rules::HZ);
        let line = if deeds { "DEEDS OF THIS DELVE".to_string() } else { format!("{} MIN {:02} SEC", secs / 60, secs % 60) };
        cv.text((NATIVE_W - text_width(&line)) / 2, 25, &line, LABEL);
    }
}

fn headline(results: &Results) -> String {
    match results.end {
        End::Cleared => format!("FLOOR {} CLEARED", results.depth),
        End::Won => "VICTORY".to_string(),
        End::Fallen => format!("FALLEN ON FLOOR {}", results.depth),
    }
}

/// A knight's honours in the order the board shows them: the shortest
/// names first, so the count of the rest fits beside the last line.
fn honours_shown(r: &Row) -> Vec<Honour> {
    let mut shown = r.honours.clone();
    shown.sort_by_key(|h| h.title().len());
    shown
}

/// One knight's row, `i` of the party, at `y`, `rh` high.
#[allow(clippy::too_many_arguments)]
fn row(cv: &mut Img, run: &Run, results: &Results, r: &Row, (i, y, rh): (u32, i32, i32), most: u32, beats: &Beats, age: u32) {
    let start = beats.row(i);
    if age < start {
        return;
    }
    // It slides in from the left and settles.
    let slide = (start + 6).saturating_sub(age) as i32;
    let x0 = -(slide * slide) / 2;
    let tall = rh >= 24;
    let mvp = results.mvp == Some(r.id);
    let crowned = mvp && age >= beats.mvp + SLAM;
    let (bx, bw, bh) = (x0 + 6, NATIVE_W - 12, rh - 2);
    cv.rect(bx, y, bw, bh, if crowned { MVP_FILL } else { ROW_FILL });
    if crowned {
        for x in bx..bx + bw {
            cv.put(x, y, WARM_DIM);
        }
    }
    // The seat's colour down its edge, as the party band marks it.
    cv.rect(bx, y, 2, bh, super::party::seat_ink(r.id));
    // The knight as the party band shows them, head and shoulders.
    let well_h = if tall { 22 } else { bh - 2 };
    if let Some(hero) = run.players.get(&r.id) {
        let rim = if crowned { chrome::BRASS_RIM } else { chrome::COLD_RIM };
        chrome::shaped(cv, (x0 + 12, y + 1, 26, well_h), &[1], Tone::WELL.rimmed(rim), false);
        let bust = super::party::bust(&super::knights::portrait(hero, r.id));
        if tall {
            cv.stamp(&bust, x0 + 13, y + 2);
        } else {
            // A short well holds the bust at the figure's own size.
            let mut small = Img::new(bust.w / 2, bust.h / 2);
            for sy in 0..small.h {
                for sx in 0..small.w {
                    if let Some(c) = bust.get(sx * 2, sy * 2) {
                        small.set(sx, sy, c);
                    }
                }
            }
            cv.stamp(&small, x0 + 25 - small.w / 2, y + 1 + (well_h - small.h) / 2);
        }
    }
    // The name, and under it a fall or a friend raised.
    let name: String = r.name.to_uppercase().chars().take(NAME_CHARS).collect();
    let line_y = if tall { y + 4 } else { y + (bh - 7) / 2 };
    cv.text(x0 + NAME_X, line_y, &name, if crowned { WARM } else { FIGURE });
    let from = beats.count(i);
    if tall && age >= from {
        let mut said = Vec::new();
        match r.line.falls {
            0 => {}
            1 => said.push("FELL ONCE".to_string()),
            n => said.push(format!("FELL {n} TIMES")),
        }
        if r.line.raised > 0 {
            said.push(format!("RAISED {}", r.line.raised));
        }
        cv.text(x0 + NAME_X, y + 13, &said.join("  "), LABEL);
    }
    // The counts tick up together, and ring warm a moment as they land.
    let landed = age >= from + COUNT;
    let ringing = landed && age < from + COUNT + 3;
    let ink = if ringing { WARM } else if landed { SETTLED } else { FIGURE };
    let line = &r.line;
    let aim = line.aim().map(|a| format!("{}%", counted(a, age, from)));
    let cells = [
        Some(counted(line.kills, age, from).to_string()),
        Some(counted(line.damage, age, from).to_string()),
        Some(counted(line.taken, age, from).to_string()),
        aim,
        Some(counted(line.gold, age, from).to_string()),
    ];
    for ((_, right), cell) in COLUMNS.iter().zip(cells) {
        let cell = cell.unwrap_or_else(|| "-".to_string());
        cv.text(x0 + right - text_width(&cell) + 1, line_y, &cell, if age < from { QUIET } else { ink });
    }
    // The knight's share of the floor's damage along the row's foot,
    // filling as it counts.
    let (sx, sw, sy) = (x0 + NAME_X, COLUMNS[4].1 - NAME_X + 1, y + bh - 1);
    cv.rect(sx, sy, sw, 1, 'K');
    if most > 0 {
        let lit = (counted(line.damage, age, from) as i64 * sw as i64 / most as i64) as i32;
        cv.rect(sx, sy, lit, 1, if crowned { WARM_DIM } else { 'j' });
    }
    // The honours, stamped on one by one once every count has landed: a
    // medal and its name a line, and on the last line the count of those
    // with no line of their own, rising as each is struck.
    let before: u32 = results.rows.iter().take(i as usize).map(|r| r.honours.len() as u32).sum();
    let lines = if tall { 2 } else { 1 };
    let shown = honours_shown(r);
    let struck = |j: usize| beats.honours + (before + j as u32) * HONOUR_GAP;
    for (k, &honour) in shown.iter().enumerate().take(lines) {
        let Some(d) = age.checked_sub(struck(k)) else {
            continue;
        };
        let hy = if tall { y + 4 + k as i32 * 10 } else { line_y };
        stamp(cv, (x0 + HONOUR_X, hy), honour, d);
        let more = (lines..shown.len()).filter(|&j| age >= struck(j)).count();
        if k + 1 == lines && more > 0 && d >= SLAM {
            let fresh = age < struck(lines + more - 1) + 4;
            let at = x0 + HONOUR_X + 9 + text_width(honour.title()) + 6;
            cv.text(at, hy, &format!("+{more}"), if fresh { WARM } else { LABEL });
        }
    }
    // The MVP's tag across the foot of the bust.
    if mvp && age >= beats.mvp {
        let d = age - beats.mvp;
        let tag_y = y + well_h - 8;
        if d < SLAM {
            // Falling in, twice its size.
            big(cv, x0 + 8, tag_y - 10 + d as i32 * 3, "MVP", WARM, 2);
        } else {
            chrome::panel(cv, (x0 + 13, tag_y, 23, 9), Tone::KEY);
            cv.text(x0 + 16, tag_y + 1, "MVP", WARM);
            if d < SLAM + 6 {
                dust(cv, x0 + 24, tag_y + 4, d - SLAM, 16);
            }
        }
    }
}

/// An honour's medal and its name, `d` steps after it was struck: the
/// medal thrown down large, then set, a puff of dust where it lands.
fn stamp(cv: &mut Img, (x, y): (i32, i32), honour: Honour, d: u32) {
    if d < SLAM {
        let k = (SLAM + 1 - d) as i32;
        let mut big_medal = Img::new(5, 7);
        medal(&mut big_medal, 0, 0, honour);
        for my in 0..7 {
            for mx in 0..5 {
                if let Some(c) = big_medal.get(mx, my) {
                    for dy in 0..k {
                        for dx in 0..k {
                            cv.set(x + 2 - 5 * k / 2 + mx * k + dx, y + 3 - 7 * k / 2 + my * k + dy, c);
                        }
                    }
                }
            }
        }
        return;
    }
    medal(cv, x, y, honour);
    cv.text(x + 9, y, honour.title(), if d < SLAM + 4 { WARM } else { SETTLED });
    if d < SLAM + 6 {
        dust(cv, x + 2, y + 3, d - SLAM, 8);
    }
}

/// A medal on its ribbon, five pixels wide and seven high: each honour its
/// own mark on the face.
fn medal(cv: &mut Img, x: i32, y: i32, honour: Honour) {
    const MEDAL: [&str; 7] = ["p...p", "pp.pp", ".ooo.", "oOOOo", "oOOOo", "oOOOo", ".ooo."];
    for (dy, row) in MEDAL.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if ch != '.' {
                cv.put(x + dx as i32, y + dy as i32, ch);
            }
        }
    }
    let face: &[(i32, i32)] = match honour {
        Honour::MostKills => &[(1, 3), (3, 3), (2, 4), (1, 5), (3, 5)],
        Honour::HardestHitter => &[(2, 3), (2, 4), (2, 5), (1, 4), (3, 4)],
        Honour::Sharpshooter => &[(2, 4), (1, 3), (3, 3), (1, 5), (3, 5)],
        Honour::Untouched => &[(1, 3), (2, 3), (3, 3), (2, 4), (2, 5)],
        Honour::Lifesaver => &[(2, 3), (1, 4), (2, 4), (3, 4), (2, 5)],
        Honour::DeepPockets => &[(2, 3), (1, 4), (3, 4), (2, 5)],
    };
    for &(dx, dy) in face {
        cv.put(x + dx, y + dy, 'P');
    }
}

/// Dust thrown out round `(x, y)`, `d` steps after the blow, as far as
/// `reach` pixels.
fn dust(cv: &mut Img, x: i32, y: i32, d: u32, reach: i32) {
    let spread = 2 + d as i32 * reach / 6;
    for k in 0..6 {
        let dir = (k * 60 + 15) as f32 * std::f32::consts::PI / 180.0;
        let (px, py) = (x + (dir.cos() * spread as f32) as i32, y + (dir.sin() * spread as f32 * 0.5) as i32);
        cv.put(px, py, if d < 3 { 'P' } else { 'I' });
    }
}

/// The floor's most impactful wish: a seal that opens from its middle, the
/// wish's name set in it letter by letter, then what it did.
fn wish(cv: &mut Img, impact: Option<&Impact>, beats: &Beats, age: u32) {
    let Some(d) = age.checked_sub(beats.wish) else {
        return;
    };
    let (x, y, w, h) = WISH;
    // The panel opens out from its middle.
    let open = ease(d as f32 / 6.0);
    let pw = ((w as f32 * open) as i32).max(8);
    chrome::panel(cv, (x + (w - pw) / 2, y, pw, h), SEAL);
    if d < 6 {
        return;
    }
    let head = "MOST IMPACTFUL WISH";
    cv.text(x + (w - text_width(head)) / 2, y + 7, head, LABEL);
    let Some(impact) = impact else {
        let none = "NO WISH MADE THIS FLOOR";
        cv.text(x + (w - text_width(none)) / 2, y + 24, none, 'h');
        return;
    };
    // The name, a letter a step, twice the size when it fits.
    let name = impact.wish.to_uppercase();
    let k = if text_width(&name) * 2 <= w - 16 { 2 } else { 1 };
    let nw = text_width(&name) * k;
    let (nx, ny) = (x + (w - nw) / 2, y + 18 + if k == 1 { 3 } else { 0 });
    let shown = (d - 6) as usize;
    let set: String = name.chars().take(shown).collect();
    big(cv, nx, ny, &set, WARM, k);
    let whole = shown >= name.chars().count();
    let done = 6 + name.chars().count() as u32;
    // Once whole: a rule run out to either side, and sparks round it.
    if whole {
        let reach = (((d - done) as i32) * 6).min((w - nw) / 2 - 12);
        let ry = ny + 7 * k / 2;
        for i in 0..reach.max(0) {
            cv.put(nx - 6 - i, ry, WARM_DIM);
            cv.put(nx + nw + 5 + i, ry, WARM_DIM);
        }
        if reach > 2 {
            for (ex, dir) in [(nx - 6 - reach, -1), (nx + nw + 5 + reach, 1)] {
                cv.put(ex, ry - 1, WARM_DIM);
                cv.put(ex, ry + 1, WARM_DIM);
                cv.put(ex + dir, ry, WARM_DIM);
            }
        }
        let zones = [(x + 14, nx - 10), (nx + nw + 9, x + w - 14)];
        sparks(cv, zones, (ny - 2, ny + 7 * k + 2), age);
    }
    if !whole || d < done + 3 {
        return;
    }
    // What it did, counting up.
    let from = beats.wish + done + 3;
    let who = impact.name.to_uppercase();
    let said = match impact.gain {
        Some(gain) => format!("+{}% DAMAGE FOR {who}", counted(gain, age, from)),
        None => format!("{who} DEALT {} WITH IT", counted(impact.damage, age, from)),
    };
    let ink = if age >= from + COUNT && age < from + COUNT + 3 { WARM } else { FIGURE };
    cv.text(x + (w - text_width(&said)) / 2, y + 37, &said, ink);
}

/// Sparks winking either side of the wish's name, in `zones` (each a
/// span across) between `top` and `foot`, from the moment it is whole:
/// each at its own place, for its own moment.
fn sparks(cv: &mut Img, zones: [(i32, i32); 2], (top, foot): (i32, i32), age: u32) {
    for k in 0..8u32 {
        let life = 14;
        let phase = (age + k * 7) % life;
        let round = (age + k * 7) / life;
        let at = hash(k as i32, round as i32, 0x5eed);
        let (x0, x1) = zones[(k % 2) as usize];
        if x1 <= x0 || foot <= top {
            continue;
        }
        let px = x0 + (at % (x1 - x0) as u32) as i32;
        let py = top + ((at >> 12) % (foot - top) as u32) as i32;
        let size = [0, 1, 2, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0][phase as usize];
        if size == 0 {
            continue;
        }
        cv.put(px, py, if size == 2 { WARM } else { WARM_DIM });
        for r in 1..size {
            for (dx, dy) in [(r, 0), (-r, 0), (0, r), (0, -r)] {
                cv.put(px + dx, py + dy, WARM_DIM);
            }
        }
    }
}

/// A finished run's deeds, `d` steps after they came up: the best boxes
/// first, each struck down in turn, its name set in warm and what it was
/// for under it.
fn deeds(cv: &mut Img, ids: &[String], d: u32) {
    use crate::rules::feats::{Tier, feat};
    let rank = |t: Tier| match t {
        Tier::Legendary => 0,
        Tier::Gold => 1,
        Tier::Silver => 2,
        Tier::Bronze => 3,
    };
    let mut earned: Vec<_> = ids.iter().filter_map(|id| feat(id)).collect();
    earned.sort_by_key(|f| rank(f.tier));
    let more = earned.len().saturating_sub(DEEDS);
    earned.truncate(DEEDS);
    const DH: i32 = 24;
    // The list stands under the headline, the best box first.
    let top = BLOCK_TOP + 6;
    let (x, w) = (44, NATIVE_W - 88);
    for (k, f) in earned.iter().enumerate() {
        let Some(e) = d.checked_sub(k as u32 * DEED_GAP) else {
            break;
        };
        let y = top + k as i32 * DH;
        let slide = 6u32.saturating_sub(e) as i32;
        let x0 = x - slide * slide / 2;
        cv.rect(x0, y, w, DH - 3, ROW_FILL);
        cv.rect(x0, y, 2, DH - 3, WARM_DIM);
        // The box of its grade, struck down.
        let boxed = super::fortune::loot_box(f.tier).recolor(&[('w', 'k')]);
        let (bx, by) = (x0 + 8, y + (DH - 3 - boxed.h) / 2);
        if e < SLAM {
            let k = (SLAM + 1 - e) as i32;
            for sy in 0..boxed.h {
                for sx in 0..boxed.w {
                    if let Some(c) = boxed.get(sx, sy) {
                        for dy in 0..k {
                            for dx in 0..k {
                                cv.set(bx + 6 - 6 * k + sx * k + dx, by + 4 - 4 * k + sy * k + dy, c);
                            }
                        }
                    }
                }
            }
            continue;
        }
        cv.stamp(&boxed, bx, by);
        if e < SLAM + 6 {
            dust(cv, bx + 6, by + 4, e - SLAM, 12);
        }
        // Its name, two letters a step, its grade at the right.
        let tx = x0 + 28;
        let grade = f.tier.name().to_uppercase();
        let room = ((x0 + w - 6 - text_width(&grade) - 8 - tx + 1) / 6).max(0) as usize;
        let name: String = f.name.to_uppercase().chars().take(room).collect();
        let set = ((e - SLAM) as usize * 2).min(name.chars().count());
        let typed: String = name.chars().take(set).collect();
        cv.text(tx, y + 3, &typed, WARM);
        cv.text(x0 + w - 6 - text_width(&grade), y + 3, &grade, LABEL);
        if set == name.chars().count() {
            let room = ((x0 + w - 6 - tx + 1) / 6).max(0) as usize;
            cv.text(tx, y + 12, &clipped(f.says, room), SETTLED);
        }
    }
    if more > 0 && d >= earned.len() as u32 * DEED_GAP {
        let line = if more == 1 { "AND ONE DEED MORE".to_string() } else { format!("AND {more} DEEDS MORE") };
        cv.text((NATIVE_W - text_width(&line)) / 2, top + earned.len() as i32 * DH + 2, &line, LABEL);
    }
}

/// `text` in at most `room` letters, cut at a word with three dots when
/// it is longer.
fn clipped(text: &str, room: usize) -> String {
    let text = text.to_uppercase();
    if text.chars().count() <= room {
        return text;
    }
    let mut out = String::new();
    for word in text.split_whitespace() {
        let next = if out.is_empty() { word.to_string() } else { format!("{out} {word}") };
        if next.chars().count() + 3 > room {
            break;
        }
        out = next;
    }
    let mut out = out.trim_end_matches(['.', ',', ';', ':']).to_string();
    out.push_str("...");
    out
}

/// The line at the board's foot: the key that goes on, or the way back
/// from a finished run.
fn hint(cv: &mut Img, run: &Run, results: &Results, age: u32) {
    if age < 16 {
        return;
    }
    let line = match results.end {
        End::Cleared => format!("ANY KEY: ON TO FLOOR {}", results.depth + 1),
        _ => super::again_hint(run),
    };
    cv.text((NATIVE_W - text_width(&line)) / 2, HINT_Y, &line, LABEL);
}

/// The board where the screen is too small for it: its headline, its MVP
/// and its wish, a line each.
fn compact(out: &mut Img, results: &Results, (x, y, w, h): (i32, i32, i32, i32)) {
    let mut lines: Vec<(String, char)> = vec![(
        match results.end {
            End::Cleared => format!("FLOOR {} CLEARED", results.depth),
            End::Won => "VICTORY".to_string(),
            End::Fallen => format!("FALLEN ON FLOOR {}", results.depth),
        },
        FIGURE,
    )];
    if let Some(row) = results.mvp.and_then(|id| results.row(id)) {
        lines.push((format!("MVP {}", row.name.to_uppercase()), WARM));
    }
    if let Some(impact) = &results.wish {
        lines.push((impact.wish.to_uppercase(), WARM));
    }
    let total = lines.len() as i32 * 10;
    for (i, (line, ink)) in lines.into_iter().enumerate() {
        let fits = ((w + 1) / 6).max(0) as usize;
        let line: String = line.chars().take(fits).collect();
        out.text(x + (w - text_width(&line)) / 2, y + (h - total) / 2 + i as i32 * 10, &line, ink);
    }
}

/// `text` at `k` times the font's size, a pixel to a block.
fn big(cv: &mut Img, x: i32, y: i32, text: &str, ink: char, k: i32) {
    if text.is_empty() {
        return;
    }
    let mut small = Img::new(text_width(text), 7);
    small.text(0, 0, text, ink);
    for sy in 0..small.h {
        for sx in 0..small.w {
            if let Some(c) = small.get(sx, sy) {
                for dy in 0..k {
                    for dx in 0..k {
                        cv.set(x + sx * k + dx, y + sy * k + dy, c);
                    }
                }
            }
        }
    }
}

/// `im` moved `dy` pixels down (up when negative), black where it left.
fn shifted(im: &Img, dy: i32) -> Img {
    let mut out = Img::black(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y - dy) {
                out.set(x, y, c);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__floor_stats__tests.rs"]
mod tests;
