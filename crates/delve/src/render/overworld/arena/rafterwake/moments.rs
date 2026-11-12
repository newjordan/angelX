//! Rafterwake's moments: the title and the 3-2-1-GO that start every tower together, the banners
//! the tower and the table call, the numbers a keeper's hit throws up, the knights' name tags and
//! the chain's chip. Every dynamic word claims its box here, in order of rank, and a word that
//! would meet a louder one is not shown, so nothing on the screen can overlap by construction.
use crate::render::overworld::ink::{Img, text_width};
use super::super::juice::{self, Tone};
use super::hud::{self, Rect, View, WIN, Word};
use crate::rules::HZ;
use crate::rules::rafterwake::{
    LENGTH, Q,
    show::{Cry, Kind},
};

/// The title card holds this many ticks of the countdown, then three beats of `BEAT`.
pub(super) const TITLE: u32 = 36;
pub(super) const BEAT: u32 = 24;
/// Ticks the GO shows after the start.
pub(super) const GO: u32 = 28;
/// Ticks a table's call and a mark's banner stay up.
const CALL: u32 = 56;
/// Ticks a number hangs over a struck keeper.
pub(super) const POP: u32 = 30;
/// The banner's top row at rest, in window rows.
pub(super) const BANNER_Y: i32 = 18;
/// The centre of the countdown's digits and of the GO, in window pixels, and the box they claim.
pub(super) const COUNT_AT: (i32, i32) = (139, 66);
const COUNT_BOX: Rect = (WIN.0 + 139 - 88, WIN.1 + 66 - 58, 176, 116);

/// The pixels the kit's `banner` marks on the window, `age` ticks in: the rails, the words and the
/// line under them, as one box in window pixels.
pub(super) fn banner_rect(w: i32, y: i32, title: &str, sub: &str, age: u32, cap: i32) -> Rect {
    let tw = text_width(title).max(1);
    let rest = ((w - 24) / tw).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / tw).max(rest);
    let text_w = tw * scale + 4;
    let top = y - (scale - rest) * 3;
    let half = text_w / 2 + 8;
    let (mut x0, mut x1) = (w / 2 - half, w / 2 + half);
    let y0 = top.min(y - 5);
    let mut y1 = (top + 7 * scale + 4).max(y + 7 * rest + 10);
    if !sub.is_empty() && age >= 4 {
        let sw = text_width(sub) + 4;
        x0 = x0.min(w / 2 - sw / 2);
        x1 = x1.max(w / 2 - sw / 2 + sw);
        y1 = y1.max(y + 7 * rest + 12 + 11);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

/// A banner on show.
#[derive(Clone, Debug)]
pub(super) struct Banner {
    pub title: String,
    pub sub: String,
    pub tone: Tone,
    pub age: u32,
    pub life: u32,
    pub cap: i32,
    pub rank: u8,
    /// Whose moment it calls, if one knight's.
    pub who: Option<u32>,
    /// Its box on the screen.
    pub rect: Rect,
}

/// A number over a struck keeper.
#[derive(Clone, Debug)]
pub(super) struct Pop {
    pub text: String,
    /// Where it started, in window pixels.
    pub at: (i32, i32),
    pub age: u32,
    pub scale: i32,
    pub ink: char,
    /// Its box on the screen now.
    pub rect: Rect,
}

/// One beat of the countdown, or the GO.
#[derive(Clone, Debug)]
pub(super) struct Count {
    pub label: &'static str,
    pub age: u32,
    pub life: u32,
    pub go: bool,
}

/// Everything the moments put on the screen at one tick.
#[derive(Clone, Debug, Default)]
pub(super) struct Plan {
    /// Every line the pen writes on the screen, the fixed words and the admitted dynamic ones.
    pub words: Vec<Word>,
    pub banner: Option<Banner>,
    pub pops: Vec<Pop>,
    pub count: Option<Count>,
}

impl Plan {
    /// Every box the plan claims, with a name for it.
    pub fn claims(&self) -> Vec<(String, Rect, Option<u32>)> {
        let mut out: Vec<_> = self.words.iter().map(|w| (format!("{} {:?}", w.kind, w.text), w.rect(), w.who)).collect();
        if let Some(b) = &self.banner {
            out.push((format!("banner {:?}", b.title), b.rect, None));
        }
        for p in &self.pops {
            out.push((format!("pop {:?}", p.text), p.rect, None));
        }
        if self.count.is_some() {
            out.push(("count".into(), COUNT_BOX, None));
        }
        out
    }
}

fn banner_of(title: String, sub: String, tone: Tone, age: u32, life: u32, cap: i32, rank: u8, who: Option<u32>) -> Option<Banner> {
    (age < life).then(|| {
        let r = banner_rect(WIN.2, BANNER_Y, &title, &sub, age, cap);
        Banner { title, sub, tone, age, life, cap, rank, who, rect: (WIN.0 + r.0, WIN.1 + r.1, r.2, r.3) }
    })
}

/// The loudest banner alive on `v`'s screen.
fn banner(v: &View) -> Option<Banner> {
    let (s, d, now, me) = (v.s, v.d, v.now, v.me);
    let elapsed = v.elapsed();
    let mut all: Vec<Banner> = Vec::new();
    let mut add = |b: Option<Banner>| all.extend(b);
    if let Some(t) = v.intro() {
        if t < TITLE {
            add(banner_of("RAFTERWAKE".into(), "CLIMB THE LOFT".into(), Tone::Warm, t, TITLE, 3, 10, None));
        }
        return all.pop();
    }
    if elapsed < GO {
        return None;
    }
    for m in &d.show.marks {
        let age = m.age(now);
        match m.kind {
            Kind::Top { deck } if deck % 3 == 0 && deck < 12 => {
                let sub = if deck == 6 { "HALFWAY" } else { "" };
                add(banner_of(format!("DECK {deck}"), sub.into(), Tone::Warm, age, 40, 2, 3, Some(me)));
            }
            Kind::Strike { left: 2 } => add(banner_of("KEEPER LOW".into(), String::new(), Tone::Warm, age, 44, 2, 6, Some(me))),
            Kind::Strike { left: 1 } => add(banner_of("ONE MORE".into(), String::new(), Tone::Hot, age, 44, 2, 7, Some(me))),
            _ => {}
        }
    }
    let last = LENGTH - 10 * HZ;
    if elapsed >= last {
        add(banner_of("LAST 10".into(), String::new(), Tone::Hot, elapsed - last, 48, 2, 5, None));
    }
    for c in &s.show.list {
        let age = c.age(now);
        let mine = c.who == me;
        let name = v.name(c.who, 8);
        match c.cry {
            Cry::Bell(_) => add(banner_of("BELL TIME".into(), "HOP HIGH".into(), Tone::Warm, age, CALL + 16, 3, 8, None)),
            Cry::Slain if mine => add(banner_of("KEEPER DOWN".into(), String::new(), Tone::Warm, age, 72, 3, 9, Some(c.who))),
            Cry::Slain => add(banner_of(format!("{name} CLEARED"), "RACE ON".into(), Tone::Steel, age, CALL, 2, 7, Some(c.who))),
            Cry::Keeper if mine => add(banner_of("KEEPER".into(), "JUMP ITS HEAD".into(), Tone::Hot, age, CALL + 8, 3, 6, Some(c.who))),
            Cry::Keeper => add(banner_of(format!("{name} AT KEEPER"), String::new(), Tone::Steel, age, CALL, 2, 4, Some(c.who))),
            Cry::Lead if mine => add(banner_of("YOU LEAD".into(), String::new(), Tone::Warm, age, CALL, 3, 5, Some(c.who))),
            Cry::Lead => add(banner_of(format!("{name} LEADS"), String::new(), Tone::Steel, age, CALL, 2, 4, Some(c.who))),
            Cry::Fell if !mine => add(banner_of(format!("{name} FELL"), String::new(), Tone::Steel, age, 36, 1, 2, Some(c.who))),
            Cry::Fell => {}
        }
    }
    all.into_iter().max_by_key(|b| (b.rank, u32::MAX - b.age))
}

/// A point of the tower in window pixels.
pub(super) fn at_win(v: &View, x: i32, y: i32) -> (i32, i32) {
    (x + hud::PAD, y - v.cam())
}

/// The numbers over a struck keeper, placed where they meet nothing claimed.
fn pops(v: &View, taken: &[Rect]) -> Vec<Pop> {
    let mut out: Vec<Pop> = Vec::new();
    for m in &v.d.show.marks {
        let age = m.age(v.now);
        let Kind::Strike { left } = m.kind else { continue };
        if age >= POP {
            continue;
        }
        let (text, scale, ink) = if left == 0 { ("+300", 2, '6') } else { ("+70", 2, 'T') };
        let at = at_win(v, m.x, m.y - 24);
        let hop = (crate::motion::ease::out((age as f32 / 7.0).min(1.0)) * 12.0) as i32 + (age as i32 * 3 / 5);
        let (w, h) = (text_width(text) * scale + 4, 7 * scale + 4);
        let rect = (WIN.0 + at.0 - w / 2, WIN.1 + at.1 - hop - h, w, h);
        if !hud::inside(rect) || taken.iter().chain(out.iter().map(|p| &p.rect)).any(|&t| hud::meets(rect, t)) {
            continue;
        }
        out.push(Pop { text: text.into(), at, age, scale, ink, rect });
    }
    out
}

/// Where each knight stands in the line-up before the start, in tower pixels: the viewer where
/// the round begins, the rest in a row beside them, by seat.
pub(super) fn lineup(v: &View, id: u32) -> i32 {
    if id == v.me {
        return v.d.x / Q;
    }
    let slot = v.s.lanes.keys().filter(|&&k| k != v.me && k < id).count() as i32 + 1;
    v.d.x / Q + 30 * slot
}

/// How far a rival's picture is still on its way from the line-up to its real place, in tower
/// pixels, `age` ticks into the round (it runs in over the first second).
pub(super) fn arriving(v: &View, id: u32) -> i32 {
    let el = v.elapsed();
    if id == v.me || v.intro().is_none() && el >= 30 {
        return 0;
    }
    let k = if v.intro().is_some() { 1.0 } else { 1.0 - crate::motion::ease::out(el as f32 / 30.0) };
    ((lineup(v, id) - v.s.lanes[&id].x / Q) as f32 * k) as i32
}

/// Name tags for the knights in view (and the viewer's own `YOU`), most near the viewer first,
/// each only where it meets nothing claimed.
fn tags(v: &View, taken: &mut Vec<Rect>) -> Vec<Word> {
    let mut out = Vec::new();
    let cam = v.cam();
    let intro = v.intro().is_some();
    let el = v.elapsed();
    let mut ids: Vec<u32> = v.s.lanes.keys().copied().collect();
    ids.sort_by_key(|&id| ((v.s.lanes[&id].y - v.d.y).abs(), id));
    let mut row = 0;
    for id in ids {
        let lane = &v.s.lanes[&id];
        let mine = id == v.me;
        if mine && !intro && el >= 90 {
            continue;
        }
        let name = v.name(id, 8);
        let w = text_width(&name) + 4;
        let (x, y) = if intro {
            // Under the line-up, two rows so neighbours never meet.
            let (sx, _) = at_win(v, lineup(v, id), 0);
            row += 1;
            (sx - w / 2, WIN.1 + (lane.y / Q - cam) + 8 + (row % 2) * 11)
        } else {
            let (sx, sy) = at_win(v, lane.x / Q + arriving(v, id), lane.y / Q);
            (sx - w / 2, WIN.1 + sy - 60)
        };
        let x = x.clamp(2, WIN.2 - 2 - w);
        let word = Word::big(x, y, name, 1, hud::cloth(id), if mine { "you" } else { "tag" }).of(id);
        let r = word.rect();
        if !hud::inside(r) || r.1 < WIN.1 + 2 || r.1 + r.3 > WIN.1 + WIN.3 - 2 || taken.iter().any(|&t| hud::meets(r, t)) {
            continue;
        }
        taken.push(r);
        out.push(word);
    }
    out
}

/// The countdown beat showing, if any.
fn count(v: &View) -> Option<Count> {
    if let Some(t) = v.intro() {
        let beat = t.checked_sub(TITLE)? / BEAT;
        let age = (t - TITLE) % BEAT;
        return Some(Count { label: ["3", "2", "1"].get(beat as usize)?, age, life: BEAT, go: false });
    }
    let el = v.elapsed();
    (el < GO).then_some(Count { label: "GO", age: el, life: GO, go: true })
}

/// The chain's chip: `X3` over the top left of the window while the landings run.
fn chip(v: &View) -> Option<Word> {
    let d = v.d;
    (d.chain >= 2 && d.down == 0 && v.intro().is_none()).then(|| {
        let scale = if d.show.marks.iter().any(|m| matches!(m.kind, Kind::Land { .. }) && m.age(v.now) < 6) { 3 } else { 2 };
        Word::big(WIN.0 + 6, WIN.1 + 4, format!("X{}", d.chain.min(99)), scale, if d.chain >= 6 { '6' } else { 'O' }, "chain")
    })
}

/// Everything the moments put on `v`'s screen now.
pub(super) fn plan(v: &View) -> Plan {
    let mut words = hud::fixed_words(v);
    let mut taken: Vec<Rect> = Vec::new();
    let count = count(v);
    if count.is_some() {
        taken.push(COUNT_BOX);
    }
    let banner = banner(v);
    if let Some(b) = &banner {
        taken.push(b.rect);
    }
    let pops = if v.over() || v.intro().is_some() { Vec::new() } else { pops(v, &taken) };
    taken.extend(pops.iter().map(|p| p.rect));
    words.extend(tags(v, &mut taken));
    if let Some(c) = chip(v) {
        let r = c.rect();
        if !taken.iter().any(|&t| hud::meets(r, t)) {
            words.push(c);
        }
    }
    Plan { words, banner, pops, count }
}

/// Paint what the plan calls for onto the window overlay (window pixels): the banner, the numbers,
/// the countdown. The pen's words are painted by the caller, in screen pixels.
pub(super) fn paint(over: &mut Img, plan: &Plan) {
    if let Some(c) = &plan.count {
        juice::countdown(over, COUNT_AT, c.label, c.age, c.life, c.go);
    }
    for p in &plan.pops {
        juice::pop_number(over, &p.text, p.at, p.age, POP, p.scale, p.ink);
    }
    if let Some(b) = &plan.banner {
        juice::banner(over, BANNER_Y, &b.title, &b.sub, b.age, b.life, b.tone, b.cap);
    }
}
