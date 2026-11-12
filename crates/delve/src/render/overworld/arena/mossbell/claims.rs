//! Who has the words. Every piece of text on a Mossbell screen is a claim: a
//! box and the words in it. The bar, the cards, the panels, the prompts, the
//! calls, the pops and the countdown are laid out here in one order of rank,
//! a later claim is dropped (never moved onto another) when it would meet an
//! earlier one, and the drawing reads this same plan, so what the test
//! checks is what is drawn.

use super::super::juice::Tone;
use super::{Run, text_width};
use crate::motion::ease;
use crate::rules::mossbell::{self as rules, BELLS, GOLD_PER_SEAT, INTRO, LENGTH, Session};
use crate::rules::mossbell_show::{Cry, SHOUT_LIFE, Shout};
use crate::rules::HZ;

pub(super) const W: i32 = 384;
pub(super) const H: i32 = 224;

/// A box: left, top, width, height.
pub(crate) type Rect = (i32, i32, i32, i32);

pub(crate) fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

pub(crate) fn inside(r: Rect) -> bool {
    r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= W && r.1 + r.3 <= H
}

/// How seat `me` calls the knight `id`, at most `take` letters: `YOU` for
/// themselves and only themselves; any other knight by their own name (`KNIGHT
/// n` for one who still carries the default `You`, or none).
pub(crate) fn called(run: &Run, id: u32, me: u32, take: usize) -> String {
    if id == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}");
    }
    full.chars().take(take).collect()
}

/// A line of plain words in the 5x7 font.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Word {
    pub what: &'static str,
    /// The seat whose name this is, if it is a name.
    pub who: Option<u32>,
    pub x: i32,
    pub y: i32,
    pub text: String,
    pub ink: char,
}

impl Word {
    pub fn rect(&self) -> Rect {
        (self.x, self.y, text_width(&self.text), 7)
    }
}

/// Big words and numbers the kit draws, `what` they are and how they are drawn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Big {
    Banner { y: i32, title: String, sub: String, age: u32, life: u32, tone: Tone, cap: i32 },
    Count { label: String, age: u32, life: u32, go: bool },
    Digit { n: u32, age: u32 },
    Pop { text: String, at: (i32, i32), age: u32, ink: char },
    /// Big words that stay: the kit's text at `scale`, its top left at `(x, y)`.
    Text { text: String, x: i32, y: i32, scale: i32, ink: char },
}

/// A box on the screen with the words in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Claim {
    pub what: &'static str,
    pub who: Option<u32>,
    pub rect: Rect,
    pub text: String,
}

/// Everything said on one seat's screen, in the order it is drawn.
#[derive(Clone, Debug, Default)]
pub(crate) struct Plan {
    pub words: Vec<Word>,
    pub bigs: Vec<(Claim, Big)>,
    /// Whether a call holds the marquee (the strip's own words step aside).
    pub marquee: bool,
    /// Whether a moment holds the field (pops keep out of its way).
    pub field: bool,
    /// Words the kit draws itself (a podium's names), claimed for the test.
    pub extra: Vec<Claim>,
}

impl Plan {
    pub fn claims(&self) -> Vec<Claim> {
        let mut v: Vec<Claim> = self
            .words
            .iter()
            .map(|w| Claim { what: w.what, who: w.who, rect: w.rect(), text: w.text.clone() })
            .collect();
        v.extend(self.bigs.iter().map(|(c, _)| c.clone()));
        v.extend(self.extra.iter().cloned());
        v
    }
    fn free(&self, r: Rect) -> bool {
        inside(r) && self.claims().iter().all(|c| !meets(c.rect, r))
    }
}

// ─── the kit's own geometry, as boxes ───────────────────────────────────────

/// The pixels the kit's `banner` marks on the whole screen, `age` ticks in:
/// the rails and the words and the line under them, as one box. A fade lifts
/// the banner while it dissolves, and the box follows it.
pub(crate) fn banner_rect(y: i32, title: &str, sub: &str, age: u32, life: u32, cap: i32) -> Rect {
    let tw = text_width(title).max(1);
    let rest = ((W - 24) / tw).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / tw).max(rest);
    let text_w = tw * scale + 4;
    let top = y - (scale - rest) * 3;
    let half = text_w / 2 + 8;
    let (mut x0, mut x1) = (W / 2 - half, W / 2 + half);
    let tail = life.saturating_sub(14);
    let lift = if age > tail { (ease::in_quad((age - tail) as f32 / 14.0) * 10.0) as i32 } else { 0 };
    let y0 = top.min(y - 5) - lift;
    let mut y1 = (top + 7 * scale + 4).max(y + 7 * rest + 10);
    if !sub.is_empty() && age >= 4 {
        let sw = text_width(sub) + 4;
        x0 = x0.min(W / 2 - sw / 2);
        x1 = x1.max(W / 2 - sw / 2 + sw);
        y1 = y1.max(y + 7 * rest + 12 + 11);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

/// The box a countdown beat takes at its largest, centred on `at`.
fn count_rect(label: &str, at: (i32, i32)) -> Rect {
    let width = text_width(label).max(5);
    let rest = (150 / width).clamp(4, 9);
    let scale = (rest as f32 * 1.7).round() as i32;
    let (w, h) = (text_width(label) * scale + 4, 7 * scale + 4);
    (at.0 - w / 2, at.1 - h / 2, w, h)
}

/// The box a score pop takes at its highest, `age` ticks old, standing on `at`.
pub(crate) fn pop_rect(text: &str, at: (i32, i32), age: u32) -> Rect {
    let hop = (ease::out((age as f32 / 7.0).min(1.0)) * 12.0) as i32 + (age as i32 * 3 / 5);
    let (w, h) = (text_width(text) + 4, 11);
    (at.0 - w / 2, at.1 - hop - h, w, h)
}

// ─── the screen ─────────────────────────────────────────────────────────────

/// The title and the countdown run on the session's intro ticks.
pub(crate) const TITLE: u32 = 42;
pub(crate) const BEAT: u32 = 12;
/// Ticks GO shows once the tables are free.
pub(crate) const GO: u32 = 28;
/// Ticks the table is shown as it stood when the clock ran out, before the podium.
pub(crate) const LIVE: u32 = 36;
/// Where a field moment, the countdown and the last digits stand.
pub(crate) const FIELD_Y: i32 = 88;
pub(crate) const MIDDLE: (i32, i32) = (192, 118);

fn word(what: &'static str, x: i32, y: i32, text: impl Into<String>, ink: char) -> Word {
    Word { what, who: None, x, y, text: text.into(), ink }
}

fn centred(what: &'static str, cx: i32, y: i32, text: impl Into<String>, ink: char) -> Word {
    let text = text.into();
    word(what, cx - text_width(&text) / 2, y, text, ink)
}

/// The knights by place, the leader first; ties to the lower seat.
pub(crate) fn order(s: &Session) -> Vec<u32> {
    let mut ids: Vec<u32> = s.tables.keys().copied().collect();
    ids.sort_by_key(|&id| (s.score(id), id));
    ids
}

/// The height of a card in the race column for `n` knights.
pub(crate) fn card_height(n: usize) -> i32 {
    if n <= 4 { 26 } else { 17 }
}

/// A call as the seat `me` sees it: its words, whether it is for the field,
/// and its tone. `None` when it is not for this seat to see.
fn voice(run: &Run, s: &Session, sh: &Shout, me: u32) -> Option<(String, String, Tone, bool, u32)> {
    let who = u32::from(sh.who);
    let mine = who == me;
    let name = || called(run, who, me, 8);
    let lit = |n: u32| s.tables.get(&me).map_or(false, |t| t.ball.lit.count_ones() >= n);
    match sh.cry {
        Cry::Jackpot if mine => Some(("JACKPOT".into(), "+500".into(), Tone::Warm, true, 78)),
        Cry::Jackpot => Some((format!("{} JACKPOT", name()), String::new(), Tone::Steel, false, SHOUT_LIFE)),
        Cry::KeeperDown if mine => {
            let sub = if lit(3) { "ALL LIT" } else { "LIGHT ALL 3" };
            Some(("KEEPER DOWN".into(), sub.into(), Tone::Warm, true, 54))
        }
        Cry::KeeperDown => Some((format!("{} KEEPER DOWN", name()), String::new(), Tone::Steel, false, SHOUT_LIFE)),
        Cry::Cleared if mine => {
            let sub = s.jackpot.first().map_or_else(String::new, |&h| format!("{} FIRST", called(run, h, me, 8)));
            Some(("TABLE CLEARED".into(), sub, Tone::Steel, true, 54))
        }
        Cry::Cleared => None,
        Cry::Bell => Some(("BELL TIME".into(), "BUMPERS X2".into(), Tone::Warm, true, 54)),
        Cry::Last => Some(("LAST TEN".into(), String::new(), Tone::Hot, false, SHOUT_LIFE)),
        Cry::Lead if mine => Some(("YOU LEAD".into(), String::new(), Tone::Warm, false, SHOUT_LIFE)),
        Cry::Lead => Some((format!("{} LEADS", name()), String::new(), Tone::Steel, false, SHOUT_LIFE)),
        Cry::Lit(n) if mine => Some((if n >= 3 { "ALL LIT" } else { "TWO LIT" }.into(), String::new(), Tone::Warm, false, 40)),
        Cry::KeeperLow if mine => Some(("KEEPER LOW".into(), String::new(), Tone::Warm, false, 40)),
        Cry::Lit(_) | Cry::KeeperLow => None,
    }
}

/// Whether a call's words name the knight it is for (and so must say `YOU` or their name).
fn named(sh: &Shout, me: u32) -> bool {
    let mine = u32::from(sh.who) == me;
    sh.who != 0 && (sh.cry == Cry::Lead || (!mine && matches!(sh.cry, Cry::Jackpot | Cry::KeeperDown)))
}

/// The whole of a seat's screen while the round is on (or waiting to begin).
pub(crate) fn plan(run: &Run, me: u32) -> Plan {
    let s = run.mossbell.as_ref().expect("a Mossbell session");
    let now = run.tick as u32;
    let mut p = Plan::default();
    let n = s.tables.len();
    let age = LENGTH - s.left;
    let playing = s.intro == 0;

    // ── what holds the field, and what holds the marquee ──
    let mut field: Option<(Claim, Big, u8)> = None;
    let mut marquee: Option<((u8, std::cmp::Reverse<u32>), Claim, Big)> = None;
    if !playing {
        let t = INTRO - s.intro;
        let (big, rect, text) = if t < TITLE {
            let (title, sub) = ("MOSSBELL", "KEEP IT ROLLING");
            (
                Big::Banner { y: FIELD_Y - 4, title: title.into(), sub: sub.into(), age: t, life: TITLE, tone: Tone::Warm, cap: 3 },
                banner_rect(FIELD_Y - 4, title, sub, t, TITLE, 3),
                title.to_string(),
            )
        } else {
            let label = ["3", "2", "1"][(((t - TITLE) / BEAT) as usize).min(2)];
            (
                Big::Count { label: label.into(), age: (t - TITLE) % BEAT, life: BEAT, go: false },
                count_rect(label, MIDDLE),
                label.to_string(),
            )
        };
        field = Some((Claim { what: "intro", who: None, rect, text }, big, 9));
    } else if age < GO {
        let big = Big::Count { label: "GO".into(), age, life: GO, go: true };
        field = Some((Claim { what: "go", who: None, rect: count_rect("GO", MIDDLE), text: "GO".into() }, big, 9));
    } else if let Some(e) = s.show.since_end(now).filter(|&e| s.left == 0 && e < LIVE) {
        let big = Big::Banner { y: FIELD_Y, title: "TIME".into(), sub: String::new(), age: e, life: LIVE, tone: Tone::Warm, cap: 3 };
        field = Some((Claim { what: "time", who: None, rect: banner_rect(FIELD_Y, "TIME", "", e, LIVE, 3), text: "TIME".into() }, big, 9));
    } else {
        for sh in &s.show.shouts {
            let Some((title, sub, tone, big, life)) = voice(run, s, sh, me) else { continue };
            let a = sh.age(now);
            if a >= life {
                continue;
            }
            if big {
                let rank = sh.cry.rank();
                if field.as_ref().map_or(true, |f| f.2 < rank) {
                    let (y, cap) = (FIELD_Y, if sh.cry == Cry::Jackpot { 4 } else { 3 });
                    let rect = banner_rect(y, &title, &sub, a, life, cap);
                    field = Some((
                        Claim { what: "moment", who: named(sh, me).then_some(u32::from(sh.who)), rect, text: title.clone() },
                        Big::Banner { y, title, sub, age: a, life, tone, cap },
                        rank,
                    ));
                }
            } else {
                // The strip is 268 wide: a long call goes at the smaller size.
                let tw = text_width(&title);
                let cap = if tw * 2 + 22 <= 262 { 2 } else { 1 };
                let rect = banner_rect(16, &title, "", a, life, cap);
                let claim = Claim { what: "call", who: named(sh, me).then_some(u32::from(sh.who)), rect, text: title.clone() };
                let key = (sh.cry.rank(), std::cmp::Reverse(a));
                if marquee.as_ref().map_or(true, |m| key > m.0) {
                    marquee = Some((key, claim, Big::Banner { y: 16, title, sub: String::new(), age: a, life, tone, cap }));
                }
            }
        }
        // The last five seconds, one big number each.
        let rem = s.left;
        if field.is_none() && rem > 0 && rem <= 5 * HZ {
            let (sec, a) = (rem.div_ceil(HZ), HZ - 1 - (rem - 1) % HZ);
            let scale = 4 + (3 - a as i32).max(0);
            let (w, h) = (text_width("5") * scale + 4, 7 * scale + 4);
            let rect = (MIDDLE.0 - w / 2, 72 - (scale - 4) * 3, w, h);
            field = Some((Claim { what: "digit", who: None, rect, text: sec.to_string() }, Big::Digit { n: sec, age: a }, 1));
        }
    }

    p.field = field.is_some();
    p.marquee = marquee.is_some();
    if !playing && INTRO - s.intro >= 8 && INTRO - s.intro < TITLE {
        p.words.push(centred("hint", 150, 158, "FLIP", 'i'));
        p.words.push(centred("hint", 234, 158, "NUDGE", 'i'));
    }

    // ── the bar ──
    if marquee.is_none() {
        p.words.push(centred("title", 192, 7, "MOSSBELL", 'i'));
        let ringing = rules::bell(age) && playing;
        p.words.push(centred(
            "line",
            192,
            25,
            if ringing { "BELL TIME X2" } else { "KEEP IT ROLLING" },
            if ringing { 't' } else { 'i' },
        ));
    }
    let secs = s.left.div_ceil(HZ);
    p.words.push(word("clock", 12, 8, format!("{secs} S"), if secs <= 10 { '7' } else { 'i' }));
    let pot = format!("{}G POT", GOLD_PER_SEAT * n as u32);
    p.words.push(word("pot", 329, 8, pot, 'i'));

    // ── the race column ──
    let rows = order(s);
    let rh = card_height(n);
    for (k, &id) in rows.iter().enumerate() {
        let y0 = 52 + k as i32 * rh;
        let text = called(run, id, me, 8);
        p.words.push(Word { what: "card name", who: Some(id), x: 7, y: y0 + 1, text, ink: if id == me { 'T' } else { 'i' } });
        let score = s.points(id).min(99999).to_string();
        let ydy = if n <= 4 { 11 } else { 9 };
        p.words.push(word("card score", 54 - text_width(&score), y0 + ydy, score, if id == me { 't' } else { 'h' }));
    }

    // ── the panel on the right ──
    let b = &s.tables[&me].ball;
    p.words.push(word("keeper", 332, 51, "KEEPER", 'i'));
    p.words.push(word("lamps", 332, 88, "LAMPS", 'i'));
    p.words.push(word("jackpot", 332, 118, "JACKPOT", 'i'));
    if s.jackpot.is_empty() {
        p.words.push(word("jackpot state", 332, 129, "OPEN", 'h'));
    } else {
        p.words.push(word("jackpot state", 332, 129, "WON", 't'));
        let mut y = 140;
        for &id in s.jackpot.iter().take(2) {
            p.words.push(Word { what: "jackpot holder", who: Some(id), x: 332, y, text: called(run, id, me, 8), ink: if id == me { 'T' } else { 'h' } });
            y += 9;
        }
    }
    p.words.push(word("drains", 332, 160, format!("{} DRAINS", b.drains), 'h'));
    if rules::bell(age) && playing {
        p.words.push(word("bell", 332, 182, "X2", 't'));
    }
    p.words.push(centred("prompt", 158, 208, "FLIP AND NUDGE", 'i'));

    // ── the moments ──
    if let Some((claim, big, _)) = field {
        p.bigs.push((claim, big));
    }
    if let Some((_, claim, big)) = marquee {
        p.bigs.push((claim, big));
    }

    // ── score pops: the viewer's own, where nothing else is ──
    if !p.field && playing {
        let mut pops: Vec<_> = s.show.pops.iter().filter(|q| u32::from(q.who) == me).collect();
        pops.sort_by_key(|q| std::cmp::Reverse(q.born));
        for q in pops {
            let a = q.age(now);
            let at = pop_origin(q.at);
            let text = format!("+{}", q.gain);
            let rect = pop_rect(&text, at, a);
            if p.free(rect) {
                let ink = if q.at == 3 { 'o' } else if q.gain >= 50 { '@' } else { 'T' };
                p.bigs.push((Claim { what: "pop", who: None, rect, text: text.clone() }, Big::Pop { text, at, age: a, ink }));
            }
        }
    }
    p
}

/// Where a pop stands when it is thrown: over its bumper, or under the keeper.
fn pop_origin(at: u8) -> (i32, i32) {
    match at {
        0 => (super::OX + super::field_x(44), super::OY + 57 - 16),
        1 => (super::OX + super::field_x(116), super::OY + 57 - 16),
        2 => (super::OX + super::field_x(80), super::OY + 83 - 16),
        _ => (super::OX + super::field_x(80) + 40, super::OY + 52),
    }
}

/// The ticks into the round each bell rings, for the bar's ticking.
pub(crate) fn bell_clock(age: u32) -> (bool, u32, u32) {
    for &(from, to) in &BELLS {
        if (from..to).contains(&age) {
            return (true, to - age, to - from);
        }
    }
    let next = BELLS.iter().map(|&(from, _)| from).find(|&f| f > age);
    match next {
        Some(f) => (false, f - age, f.saturating_sub(BELLS[0].0).max(1)),
        None => (false, 0, 1),
    }
}

// ─── the podium ─────────────────────────────────────────────────────────────

/// Where the podium stands: the kit's box, and the floor under its steps.
pub(crate) const PODIUM: Rect = (0, 48, W, 150);
pub(crate) const FLOOR: i32 = PODIUM.1 + PODIUM.3 - 6;

/// The knights in the order of the finish, the winner first.
pub(crate) fn places(s: &Session) -> Vec<u32> {
    if s.order.len() == s.tables.len() { s.order.clone() } else { order(s) }
}

/// The words of the finish, `a` ticks after the table's iris opened.
pub(crate) fn finish(run: &Run, me: u32, a: u32) -> Plan {
    let s = run.mossbell.as_ref().expect("a Mossbell session");
    let mut p = Plan::default();
    let ranks = places(s);
    let n = ranks.len();
    let lead = ranks[0];
    let tied = ranks.iter().filter(|&&id| s.score(id) == s.score(lead)).count();
    let title = if n == 1 {
        if s.jackpot.is_empty() { "TIME UP".to_string() } else { "JACKPOT".to_string() }
    } else if tied > 1 {
        "DEAD HEAT".to_string()
    } else if lead == me {
        "YOU WIN".to_string()
    } else {
        format!("{} WINS", called(run, lead, me, 9))
    };
    let top = s.points(lead);
    // The winner's name rides in the title; the jackpot's holder under it.
    let mut sub = format!("{top} PTS");
    if let Some(&holder) = s.jackpot.first().filter(|_| n > 1) {
        sub = format!("{sub}  JACKPOT {}", called(run, holder, me, 8));
    }
    if a >= 6 {
        let age = a - 6;
        let big = Big::Banner { y: 16, title: title.clone(), sub: sub.clone(), age, life: age + 100, tone: Tone::Warm, cap: 3 };
        let claim = Claim {
            what: "title",
            who: if n > 1 && tied == 1 { Some(lead) } else { None },
            rect: banner_rect(16, &title, &sub, age, age + 100, 3),
            text: title.clone(),
        };
        p.bigs.push((claim, big));
    }
    // The podium's names and lines, as the kit sets them under the steps.
    let cx = PODIUM.0 + PODIUM.2 / 2;
    let spots = [cx, cx - 88, cx + 88];
    for (i, &id) in ranks.iter().take(3).enumerate() {
        if a < [34, 26, 18][i] {
            continue;
        }
        let name = called(run, id, me, 9);
        let line = format!("{} PTS", s.points(id).min(99999));
        let (nw, lw) = (text_width(&name), text_width(&line));
        p.extra.push(Claim { what: "podium name", who: Some(id), rect: (spots[i] - nw / 2, FLOOR + 4, nw, 7), text: name });
        p.extra.push(Claim { what: "podium line", who: None, rect: (spots[i] - lw / 2, FLOOR + 13, lw, 7), text: line });
    }
    // The rest of the table, small and plain, along the foot.
    for (k, &id) in ranks.iter().enumerate().skip(3) {
        if a < 24 + k as u32 * 3 {
            continue;
        }
        let col = k as i32 - 3;
        let text = format!("{} {} {}", k + 1, called(run, id, me, 8), s.points(id).min(99999));
        p.words.push(Word { what: "rival", who: Some(id), x: 8 + col * 128, y: 214, text, ink: if id == me { 'T' } else { 'h' } });
    }
    // The purse, counting up, down the left; the best, down the right.
    let gold = s.payout(me);
    if a >= 24 && n > 1 {
        let shown = (gold as f32 * ease::out(((a - 24) as f32 / 40.0).min(1.0))) as u32;
        let text = format!("+{shown}G");
        let w = text_width(&text) * 2 + 4;
        p.bigs.push((
            Claim { what: "purse", who: None, rect: (6, 84, w, 18), text: text.clone() },
            Big::Text { text, x: 6, y: 84, scale: 2, ink: 'T' },
        ));
        p.words.push(word("purse label", 8, 106, if gold > 0 { "YOUR PURSE" } else { "NO PURSE" }, 'h'));
    }
    if a >= 30 {
        // The hall's own record (it holds no more than a thousand), and the jackpots taken.
        let hall = run.home.countryside.games.mossbell.clone();
        let best = s.tables.values().map(|t| t.ball.score.min(1000)).max().unwrap_or(0).max(u32::from(hall.best)).to_string();
        p.words.push(word("best label", 328, 84, "HALL BEST", 'h'));
        let w = text_width(&best) * 2 + 4;
        p.bigs.push((
            Claim { what: "best", who: None, rect: (328, 94, w, 18), text: best.clone() },
            Big::Text { text: best, x: 328, y: 94, scale: 2, ink: 'o' },
        ));
        if s.show.record {
            p.words.push(word("record", 328, 116, "NEW BEST", 't'));
        }
        p.words.push(word("jackpots label", 328, 134, "JACKPOTS", 'h'));
        p.words.push(word("jackpots", 328, 143, hall.wins.min(999).to_string(), 'o'));
    }
    if a >= 45 && (a / 12) % 2 == 0 {
        p.words.push(word("again", 330, 8, "E AGAIN", 't'));
    }
    p
}
