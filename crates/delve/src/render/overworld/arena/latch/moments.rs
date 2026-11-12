//! Latch Duel's moments: the title that slams in over the 3-2-1, the GO behind it, the banner a
//! counter, a chain, a lead, match point or the whole table in step earns, the end of the round, and
//! the spark that carries a counter to its pip. Each is a pure function of an age, drawn on a layer
//! of its own (the kit's words are pictures, not the 5x7 font), and each layer claims its pixels
//! (`hud::claim`) so the words test sees every box that was drawn.
use super::super::super::ink::Img;
use super::super::juice::{self, Tone};
use super::hud::{self, Rect};
use crate::rules::latch::{COUNT, Session, timing};
use crate::rules::latch::Beat;
use crate::rules::latch_show::{Cry, Kind, PERFECT};
use crate::rules::{HZ, Run};

/// Ticks the title holds before the numbers begin, and the GO that follows the last of them.
pub(super) const TITLE: u32 = 24;
pub(super) const GO: u32 = 24;
/// Ticks each number holds.
const BEAT: u32 = (COUNT - TITLE) / 3;

/// The layer the banners are drawn on, over the top of the stage: wide enough for a twelve-letter
/// title at twice its size and the rails either side of it.
pub(super) const BANNER: Rect = (84, 38, 216, 50);
/// The layer the numbers and the GO are drawn on, in the middle of the stage.
pub(super) const DIGITS: Rect = (96, 56, 192, 112);
/// The layer the height of the arm is called on, left of the knight.
pub(super) const HINT: Rect = (60, 92, 84, 18);

/// A banner on show.
#[derive(Clone, Debug)]
pub(super) struct Banner {
    pub title: String,
    pub sub: String,
    pub tone: Tone,
    pub age: u32,
    pub life: u32,
    pub rank: u8,
}

/// The knight with a third counter, if a knight has one: the first to finish the round.
pub(super) fn winner_now(s: &Session) -> Option<u32> {
    s.duels.iter().filter(|(_, d)| d.counters >= 3).map(|(&id, _)| id).min_by_key(|&id| (s.score(id), id))
}

/// The banner seat `id`'s screen shows at the round's clock `clock`: the loudest of what is alive.
pub(super) fn banner(run: &Run, s: &Session, id: u32, me: Option<u32>, clock: u32, ended: u32) -> Option<Banner> {
    let mut all: Vec<Banner> = Vec::new();
    let mut add = |title: String, sub: String, tone: Tone, age: u32, life: u32, rank: u8| {
        if age < life {
            all.push(Banner { title, sub, tone, age, life, rank });
        }
    };
    if s.left == 0 {
        // The last breath: Clasp is broken, or the bell rings.
        if s.won {
            let who = s.order.first().copied().unwrap_or(id);
            let sub = if Some(who) == me { "YOU WIN".to_string() } else { format!("{} WINS", hud::called(run, who, me, 12)) };
            add("CLASP YIELDS".into(), sub, Tone::Warm, ended, super::finish::HOLD + super::finish::WIPE, 90);
        } else {
            add("THE BELL CLOSES".into(), "MOST COUNTERS WIN".into(), Tone::Steel, ended, super::finish::HOLD + super::finish::WIPE, 90);
        }
    }
    if s.count > 0 {
        add("LATCH DUEL".into(), "READ THEN COUNTER".into(), Tone::Warm, COUNT - s.count, TITLE, 80);
    }
    // The blow that breaks Clasp, for every screen: the winner's own, or the table's.
    if let (Some(at), Some(who)) = (s.win_at, winner_now(s)) {
        let sub = if Some(who) == me { String::new() } else { hud::called(run, who, me, 12) };
        add("FINAL COUNTER".into(), sub, Tone::Warm, clock.saturating_sub(at), 44, 70);
    }
    for m in s.show.marks.iter().filter(|m| m.who == id) {
        let age = m.age(clock);
        if let Kind::Miss { why, lost } = m.kind {
            let sub = if lost >= 2 { "CHAIN LOST" } else { "" };
            add(hud::why_word(why).into(), sub.into(), Tone::Hot, age, 28, 14);
        }
        if let Kind::Counter { chain, count, error, .. } = m.kind {
            if count >= 3 {
                continue;
            }
            let sub = timing(u32::from(error));
            if chain >= 2 {
                add(format!("CHAIN X{chain}"), sub, Tone::Warm, age, 34, 30 + chain);
            } else {
                add(if error <= PERFECT { "PERFECT" } else { "COUNTER" }.into(), sub, Tone::Warm, age, 26, 12);
            }
        }
    }
    for c in &s.show.calls {
        let age = c.age(clock);
        match c.cry {
            Cry::Sync(n) => add(format!("IN SYNC X{n}"), "ALL KNIGHTS".into(), Tone::Warm, age, 48, 50 + n.min(9)),
            Cry::Point if Some(c.who) == me && c.who == id => {
                // The second counter is a chain too, when nothing was missed: say both.
                let chain = s.duels[&id].chain;
                let sub = if chain >= 2 { format!("CHAIN X{chain}") } else { "ONE TO GO".to_string() };
                add("MATCH POINT".into(), sub, Tone::Hot, age, 46, 40)
            }
            Cry::Lead if Some(c.who) == me && c.who == id => add("YOU LEAD".into(), String::new(), Tone::Warm, age, 38, 20),
            _ => {}
        }
    }
    all.into_iter().max_by_key(|b| (b.rank, std::cmp::Reverse(b.age)))
}

/// The number the 3-2-1 shows `age` ticks into the count, and how far into it.
pub(super) fn digit(s: &Session) -> Option<(&'static str, u32)> {
    if s.count == 0 {
        return None;
    }
    let t = COUNT - s.count;
    let t = t.checked_sub(TITLE)?;
    Some((["3", "2", "1"][((t / BEAT) as usize).min(2)], t % BEAT))
}

/// Draw a kit moment on a layer of its own, claim its pixels and lay it over `out`.
fn on_layer(out: &mut Img, name: &'static str, (x, y, w, h): Rect, draw: impl FnOnce(&mut Img)) {
    let mut layer = Img::new(w, h);
    draw(&mut layer);
    hud::claim(name, &layer, (x, y));
    out.stamp(&layer, x, y);
}

/// Everything the moments put over the play screen of seat `id`.
pub(super) fn paint(out: &mut Img, run: &Run, s: &Session, id: u32, me: Option<u32>, clock: u32, ended: u32) {
    if let Some((label, age)) = digit(s) {
        on_layer(out, "digit", DIGITS, |im| juice::countdown(im, (96, 56), label, age, BEAT, false));
        return;
    }
    if s.count == 0 && s.left > 0 && clock < GO {
        on_layer(out, "go", DIGITS, |im| juice::countdown(im, (96, 56), "GO", clock, GO, true));
        return;
    }
    if let Some(b) = banner(run, s, id, me, clock, ended) {
        on_layer(out, "banner", BANNER, |im| juice::banner(im, 12, &b.title, &b.sub, b.age, b.life, b.tone, 2));
    }
    // The height of the arm that is coming, in the kit's big letters, gold when the beat is closing.
    let d = &s.duels[&id];
    if d.beat == Beat::Tell && s.left > 0 {
        let open = d.ticks <= run.tune.latch_window;
        on_layer(out, "hint", HINT, |im| {
            let word = juice::big_text_img(if d.height < 0 { "HIGH" } else { "LOW" }, 2, if open && (run.tick / 2) % 2 == 0 { 'T' } else { 't' }, 'B');
            im.stamp(&word, 42 - word.w / 2, 2);
        });
    }
}

/// The last five seconds of the clock, as how red the stage's rim is: 0 to 1, 0 before then.
pub(super) fn hurry(s: &Session, tick: u64) -> f32 {
    if s.count > 0 || !(1..=5 * HZ).contains(&s.left) {
        return 0.0;
    }
    let pulse = 0.5 + 0.5 * ((tick as f32) * 0.3).sin();
    (1.0 - s.left as f32 / (5 * HZ) as f32) * 0.6 * (0.4 + 0.6 * pulse)
}
