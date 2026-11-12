//! Latch Duel's frame: the clock, the title and the pot, the toast under the title, the two names
//! under the bodies, the chain's pips and Clasp's bar, a live little card for every knight at the
//! table, and the one rule of the screen: every line of the 5x7 font is a `Word` made here, so what a
//! test reads off the screen is what was planned. Five-by-seven words live in the top band, the
//! left of the stage and the bottom bands; every banner and big number the kit draws lives in the
//! banner box or the middle of the stage, so the two never meet.
use super::super::super::ink::{Img, text_width};
use super::super::knights;
use super::fx;
use crate::rules::latch::{Beat, GOLD_PER_SEAT, Session};
use crate::rules::latch_show::{Cry, Kind, Why};
use crate::rules::{HZ, Run};
use std::sync::OnceLock;

/// A box: left, top, width, height.
pub(super) type Rect = (i32, i32, i32, i32);

pub(super) const W: i32 = 384;
pub(super) const H: i32 = 224;
/// Ticks a spark takes to carry a counter from Clasp to the pips.
pub(super) const FLY: u32 = 14;
/// Ticks a call stays a toast.
pub(super) const TOAST: u32 = 54;
/// The pips in the knight's panel: the first one's centre, the step between them, and their row.
pub(super) const PIP: (i32, i32, i32) = (42, 15, 21);
/// Clasp's bar in its panel: its left, top, a third's width and the gap between thirds.
pub(super) const BAR: (i32, i32, i32, i32) = (262, 19, 24, 3);
/// The left and right panels of the top band.
pub(super) const KNIGHT_PANEL: Rect = (4, 4, 128, 26);
pub(super) const CLASP_PANEL: Rect = (252, 4, 128, 26);
/// The row of the bottom plate the toast and the stakes stand on.
pub(super) const PLATE_Y: i32 = 178;
/// The cards along the bottom.
pub(super) const CARD: (i32, i32, i32, i32) = (58, 24, 3, 189);
/// The row the legend stands on.
pub(super) const LEGEND_Y: i32 = 215;

pub(super) fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

pub(super) fn inside(r: Rect) -> bool {
    r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= W && r.1 + r.3 <= H
}

/// A line of the 5x7 font where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Word {
    pub x: i32,
    pub y: i32,
    pub text: String,
    pub ink: char,
}

pub(super) fn centre(cx: i32, y: i32, text: impl Into<String>, ink: char) -> Word {
    let text = text.into();
    Word { x: cx - text_width(&text) / 2, y, text, ink }
}

// ─── names ───────────────────────────────────────────────────────────────────

/// How the seat `me` calls knight `id`, at most `take` letters: `YOU` for themselves and only
/// themselves, any other knight by their own name (`KNIGHT n` for one who still carries the default
/// `You`, or none).
pub(super) fn called(run: &Run, id: u32, me: Option<u32>, take: usize) -> String {
    if Some(id) == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}");
    }
    full.chars().take(take).collect()
}

/// A knight's name tag on a small card: their own name's first letters, `K2` for the default
/// `Knight 2` and for a knight still called `You`, never `YOU`.
pub(super) fn tag(run: &Run, id: u32, take: usize) -> String {
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" || full.starts_with("KNIGHT ") {
        return format!("K{id}");
    }
    full.chars().take(take).collect()
}

/// The ink a seat is marked with.
pub(super) fn ink_of(run: &Run, id: u32) -> char {
    let colours = run.players.get(&id).map_or(knights::seat_colours(id), |h| {
        if h.colours().is_empty() { knights::seat_colours(id) } else { h.colours() }
    });
    colours.iter().find(|&&(from, _)| from == '7').map_or('7', |&(_, to)| to)
}

// ─── what the table shows ────────────────────────────────────────────────────

/// The pot the round plays for.
pub(super) fn pot(s: &Session) -> u32 {
    GOLD_PER_SEAT * s.duels.len() as u32
}

/// `6.2S` for 186 ticks.
pub(super) fn clock_text(ticks: u32) -> String {
    format!("{}.{}S", ticks / HZ, (ticks % HZ) * 10 / HZ)
}

/// Counters the pips show for knight `id`: a counter does not light its pip until the spark that
/// carries it lands (`FLY`).
pub(super) fn shown(s: &Session, id: u32, clock: u32) -> u8 {
    let flying = s
        .show
        .marks
        .iter()
        .filter(|m| m.who == id && matches!(m.kind, Kind::Counter { .. }) && m.age(clock) < FLY)
        .count() as u8;
    s.duels[&id].counters.saturating_sub(flying)
}

/// Ticks since a spark last landed on knight `id`'s pips, while the bump lasts, and the pip it lit.
pub(super) fn landed(s: &Session, id: u32, clock: u32) -> Option<(u32, usize)> {
    s.show
        .marks
        .iter()
        .filter(|m| m.who == id)
        .filter_map(|m| match m.kind {
            Kind::Counter { count, .. } => m.age(clock).checked_sub(FLY).filter(|&a| a < 18).map(|a| (a, usize::from(count) - 1)),
            _ => None,
        })
        .min_by_key(|&(a, _)| a)
}

/// The newest mark of knight `id` that is a counter, and its age.
pub(super) fn latest_counter(s: &Session, id: u32, clock: u32) -> Option<(crate::rules::latch_show::Mark, u32)> {
    s.show.marks.iter().rev().find(|m| m.who == id && matches!(m.kind, Kind::Counter { .. })).map(|m| (*m, m.age(clock)))
}

/// The newest fold of knight `id`'s guard, and its age.
pub(super) fn latest_miss(s: &Session, id: u32, clock: u32) -> Option<(crate::rules::latch_show::Mark, u32)> {
    s.show.marks.iter().rev().find(|m| m.who == id && matches!(m.kind, Kind::Miss { .. })).map(|m| (*m, m.age(clock)))
}

/// The two plain words a fold gives.
pub(super) fn why_word(why: Why) -> &'static str {
    match why {
        Why::Early => "TOO SOON",
        Why::Wrong => "WRONG GUARD",
        Why::Slow => "TOO SLOW",
    }
}

/// The loudest call a toast shows for the screen of seat `id`: a friend's, never their own, which
/// the banners carry.
pub(super) fn toast(run: &Run, s: &Session, id: u32, me: Option<u32>, clock: u32) -> Option<(String, char)> {
    let mut best: Option<(u8, u32, String)> = None;
    for c in s.show.calls.iter().filter(|c| c.age(clock) < TOAST && Some(c.who) != me) {
        let line = match c.cry {
            Cry::Lead => format!("{} LEADS", called(run, c.who, me, 8)),
            Cry::Point => format!("{} MATCH POINT", called(run, c.who, me, 8)),
            Cry::Sync(_) => continue,
        };
        if best.as_ref().is_none_or(|b| (c.cry.rank(), c.born) > (b.0, b.1)) {
            best = Some((c.cry.rank(), c.born, line));
        }
    }
    if let Some((_, _, line)) = best {
        return Some((line, 'T'));
    }
    let calm = s.count == 0 && s.left > 0 && clock < 150 && s.duels.values().all(|d| d.counters == 0);
    let _ = id;
    calm.then(|| ("READ THEN COUNTER".to_string(), 'v'))
}

/// Every line of 5x7 words on the play screen of seat `id` (seat `me` is looking, if any).
pub(super) fn words(run: &Run, s: &Session, id: u32, me: Option<u32>, clock: u32) -> Vec<Word> {
    let d = &s.duels[&id];
    let mut out = Vec::new();
    // The two panels' names and the pot between them.
    out.push(Word { x: 34, y: 8, text: called(run, id, me, 10), ink: ink_of(run, id) });
    out.push(Word { x: 350 - text_width("CLASP"), y: 8, text: "CLASP".into(), ink: 't' });
    out.push(centre(192, 22, format!("{}G POT", pot(s)), 'G'));
    // The bottom plate: the title, the news and the time to beat.
    out.push(centre(46, PLATE_Y, "LATCH DUEL", 't'));
    if let Some((line, ink)) = toast(run, s, id, me, clock) {
        out.push(centre(192, PLATE_Y, line, ink));
    }
    if s.to_beat > 0 {
        out.push(centre(338, PLATE_Y, format!("BEST {}", clock_text(s.to_beat)), 'u'));
    }
    // What to do about the arm, left of the knight (its height is the kit's big word, `hint`).
    if d.beat == Beat::Tell && d.ticks <= run.tune.latch_window {
        out.push(centre(102, 113, "COUNTER", 't'));
    }
    out.push(centre(192, LEGEND_Y, "UP DOWN GUARD    F COUNTER", 'G'));
    // The cards' name tags.
    for (n, &who) in s.duels.keys().enumerate() {
        if Some(who) != me && who != id {
            let (x, y, ..) = card(s.duels.len(), n);
            out.push(Word { x: x + 27, y: y + 3, text: tag(run, who, 4), ink: ink_of(run, who) });
        }
    }
    out
}

// ─── the top band: both fighters, the clock, the chain's pips and Clasp's bar ───

fn diamond(out: &mut Img, cx: i32, cy: i32, r: i32, fill: char, edge: char) {
    for dy in -r..=r {
        let w = r - dy.abs();
        for dx in -w..=w {
            let rim = dx.abs() == w || dy.abs() == r;
            out.put(cx + dx, cy + dy, if rim { edge } else { fill });
        }
    }
}

/// A knight small: their guard, trimmed to the figure and brought down to `h` rows.
pub(super) fn knight_mini(run: &Run, id: u32, h: i32) -> Option<Img> {
    let src = super::knight_img(run, id, 0, false)?;
    let (mut x0, mut y0, mut x1, mut y1) = (src.w, src.h, 0, 0);
    for y in 0..src.h {
        for x in 0..src.w {
            if src.get(x, y).is_some() {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    if x1 <= x0 {
        return None;
    }
    let (fw, fh) = (x1 - x0, y1 - y0);
    let w = (fw * h / fh).max(1);
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = src.get(x0 + x * fw / w, y0 + y * fh / h) {
                out.set(x, y, c);
            }
        }
    }
    Some(out)
}

/// The top band of seat `id`'s screen: their panel (knight, name, the chain's pips lit by the spark
/// that carries each counter), Clasp's panel (its face at the plates it has left, its bar of thirds
/// chipped as they go) and between them the clock, big, red in the last five seconds.
pub(super) fn paint_top(out: &mut Img, run: &Run, s: &Session, id: u32, clock: u32, tick: u64) {
    let d = &s.duels[&id];
    let ink = ink_of(run, id);
    let lit = shown(s, id, clock);
    let bump = landed(s, id, clock);
    let counter = latest_counter(s, id, clock);
    let struck = counter.is_some_and(|(_, age)| age < 3);
    let (kx, ky, kw, kh) = KNIGHT_PANEL;
    let (cx0, cy0, cw, ch) = CLASP_PANEL;
    out.frame(kx, ky, kw, kh, ink);
    out.frame(kx + 1, ky + 1, kw - 2, kh - 2, 'S');
    out.frame(cx0, cy0, cw, ch, 't');
    out.frame(cx0 + 1, cy0 + 1, cw - 2, ch - 2, 'S');
    if let Some(k) = knight_mini(run, id, 22) {
        out.stamp(&k, kx + 3 + (22 - k.w) / 2, ky + 2);
    }
    let face = mini(usize::from(lit.min(2)), super::pose(d));
    if struck {
        out.stamp(&fx::flash_frame(face), cx0 + kw - 25, cy0 + 2);
    } else {
        out.stamp(face, cx0 + kw - 25, cy0 + 2);
    }
    // The chain's three pips: lit as the spark lands, swelling as it does, gold along a chain.
    for p in 0..3usize {
        let cx = PIP.0 + p as i32 * PIP.1;
        let is_lit = p < usize::from(lit);
        let swell = bump.filter(|&(_, which)| which == p).map_or(0, |(age, _)| (juice_bump(age) * 2.0) as i32);
        let edge = if is_lit && d.chain >= 2 { 't' } else if is_lit { 'T' } else { 'S' };
        diamond(out, cx, PIP.2, 4 + swell, if is_lit { ink } else { 's' }, edge);
        if is_lit {
            out.put(cx - 1, PIP.2 - 1, 'H');
        } else if usize::from(lit) == p && d.beat == Beat::Tell && d.ticks <= run.tune.latch_window && (tick / 3) % 2 == 0 {
            // The pip about to be won breathes in the closing beat.
            diamond(out, cx, PIP.2, 4, 's', 'o');
        }
        if let Some((age, _)) = bump.filter(|&(age, w)| w == p && age < 8) {
            fx::cheer(out, (cx as f32, PIP.2 as f32), age, 7 + p as u32, tick, false);
        }
    }
    // Clasp's bar: thirds of plate, one lost with every counter and chipped white as it goes.
    let (x0, y, w, h) = BAR;
    let lost_age = landed(s, id, clock).map(|(a, _)| a);
    for i in 0..3i32 {
        let x = x0 + i * (w + 3);
        let alive = i < 3 - i32::from(lit);
        out.rect(x - 1, y - 1, w + 2, h + 2, 'S');
        if alive {
            out.rect(x, y, w, h, 'r');
            out.rect(x, y, w, 1, 'R');
        } else if i == 3 - i32::from(lit) && lost_age.is_some_and(|a| a < 4) {
            out.rect(x, y, w, h, 'H');
        } else if let Some(age) = lost_age.filter(|&a| a < 14 && i == 3 - i32::from(lit)) {
            fx::chips(out, ((x + w / 2) as f32, y as f32), age, 31 + i as u32, tick, 4);
        }
    }
    // The clock, big between them.
    let secs = s.left.div_ceil(HZ);
    let hurry = s.left > 0 && s.count == 0 && secs <= 5;
    let fill = if hurry && (tick / 4) % 2 == 0 { 'R' } else if hurry { 'T' } else { 'v' };
    let img = super::super::juice::big_text_img(&secs.to_string(), 2, fill, 'k');
    let at = (192 - img.w / 2, 3);
    claim("clock", &img, at);
    out.stamp(&img, at.0, at.1);
}

fn juice_bump(age: u32) -> f32 {
    super::super::juice::bump(age, 14)
}

// ─── the table's cards ───────────────────────────────────────────────────────

/// Card `i` of `n`: its box.
pub(super) fn card(n: usize, i: usize) -> Rect {
    let (w, h, gap, y) = CARD;
    let total = n as i32 * w + (n as i32 - 1) * gap;
    ((W - total) / 2 + i as i32 * (w + gap), y, w, h)
}

/// A rival's Clasp, small: every pose at every plate it can have lost, baked once.
fn mini(stage: usize, pose: usize) -> &'static Img {
    static BANK: OnceLock<[[Img; 9]; 3]> = OnceLock::new();
    let bank = BANK.get_or_init(|| {
        std::array::from_fn(|stage| {
            std::array::from_fn(|pose| {
                let src = &super::art().states[stage][pose];
                let mut out = Img::new(22, 22);
                for y in 0..22 {
                    for x in 0..22 {
                        if let Some(c) = src.get(x * src.w / 22 + 1, y * src.h / 22 + 1) {
                            out.set(x, y, c);
                        }
                    }
                }
                out
            })
        })
    });
    &bank[stage.min(2)][pose.min(8)]
}

/// Every knight's card along the bottom: their Clasp in the pose it is in, their pips, their name,
/// a crown for the one in front. A counter flashes the card and throws sparks; a fold reddens it.
pub(super) fn paint_cards(out: &mut Img, run: &Run, s: &Session, id: u32, clock: u32, tick: u64) {
    let n = s.duels.len();
    for (i, (&who, d)) in s.duels.iter().enumerate() {
        let (x, y, w, h) = card(n, i);
        let own = who == id;
        let ink = ink_of(run, who);
        let counter = latest_counter(s, who, clock).filter(|&(_, age)| age < 12);
        let miss = latest_miss(s, who, clock).filter(|&(_, age)| age < 10);
        out.rect(x, y, w, h, 'k');
        let frame = if counter.is_some_and(|(_, age)| age < 4) {
            'H'
        } else if own {
            ink
        } else {
            's'
        };
        out.frame(x, y, w, h, frame);
        if own || counter.is_some() {
            out.frame(x + 1, y + 1, w - 2, h - 2, if own { 'S' } else { 'S' });
        }
        // Their Clasp, in the pose and at the plates it shows, white for the frames of a blow.
        let portrait = mini(usize::from(d.counters.min(2)), super::pose(d));
        if counter.is_some_and(|(_, age)| age < 2) {
            out.stamp(&fx::flash_frame(portrait), x + 2, y + 1);
        } else {
            out.stamp(portrait, x + 2, y + 1);
        }
        if let Some((_, age)) = miss {
            fx::chips(out, ((x + 12) as f32, (y + 8) as f32), age, 5 + who, tick, 3);
        }
        // Their three pips.
        for p in 0..3i32 {
            let px = x + 27 + p * 10;
            out.rect(px, y + 14, 8, 6, 'S');
            if p < i32::from(d.counters) {
                let flash = counter.is_some_and(|(m, age)| matches!(m.kind, Kind::Counter { count, .. } if i32::from(count) == p + 1) && age < 3);
                out.rect(px + 1, y + 15, 6, 4, if flash { 'H' } else { ink });
            }
        }
        if let Some((_, age)) = counter {
            fx::cheer(out, ((x + 13) as f32, (y + 11) as f32), age, 3 + who * 7, tick, d.counters >= 2);
        }
        // The crown on the knight in front.
        if s.show.leader == Some(who) {
            let (cx, cy) = (x + w - 7, y + 3);
            out.rect(cx - 2, cy + 2, 5, 2, 't');
            for k in [-2, 0, 2] {
                out.put(cx + k, cy + 1, 't');
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    static CLAIMS: std::cell::RefCell<Vec<(&'static str, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Note the pixels of `layer`, about to be stamped at `at`, as a box the kit's words claim.
pub(super) fn claim(name: &'static str, layer: &Img, at: (i32, i32)) {
    #[cfg(test)]
    {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 0..layer.h {
            for x in 0..layer.w {
                if layer.get(x, y).is_some() {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                }
            }
        }
        if x1 > x0 {
            CLAIMS.with(|c| c.borrow_mut().push((name, (at.0 + x0, at.1 + y0, x1 - x0, y1 - y0))));
        }
    }
    #[cfg(not(test))]
    let _ = (name, layer, at);
}

/// Note a box of the screen as the kit's words claim it (a podium's name, for one).
pub(super) fn claim_box(name: &'static str, rect: Rect) {
    #[cfg(test)]
    CLAIMS.with(|c| c.borrow_mut().push((name, rect)));
    #[cfg(not(test))]
    let _ = (name, rect);
}

/// Run `f` and collect the boxes the kit's words claimed while it drew.
#[cfg(test)]
pub(super) fn claimed<R>(f: impl FnOnce() -> R) -> (R, Vec<(&'static str, Rect)>) {
    CLAIMS.with(|c| c.borrow_mut().clear());
    let out = f();
    (out, CLAIMS.with(|c| std::mem::take(&mut *c.borrow_mut())))
}
