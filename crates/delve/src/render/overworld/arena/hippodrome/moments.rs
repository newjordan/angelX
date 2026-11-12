//! The words and moments on a seat's screen, as one plan: the title and the 3-2-1-GO that start
//! every team together, the call of the moment (a lead taken, a barge, a wreck, a photograph), the
//! lap and place, the keys, and the names over the other teams. Every word claims its box in
//! order of rank, and a word that would meet a louder one is not shown, so nothing on the screen
//! can overlap by construction. The plan is built from the race alone, before anything is painted.
use super::super::super::ink::{Img, bayer, text_width};
use super::super::juice::{self, Tone};
use super::super::pfx::{self, Pen, Ramp, ramps};
use super::view::View;
use crate::motion::Spring;
use crate::rules::HZ;
use crate::rules::chariot::{GATE_TICKS, LAPS, Phase, show::Kind};
use crate::rules::juice::Weight;

pub(super) type Rect = (i32, i32, i32, i32);
pub(super) const SCREEN: (i32, i32) = (384, 224);

pub(super) fn inside((x, y, w, h): Rect) -> bool {
    x >= 0 && y >= 0 && x + w <= SCREEN.0 && y + h <= SCREEN.1
}

pub(super) fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Ticks a call stands.
const CALL_LIFE: u32 = 62;
/// Where the title card's top rests, and the call of the moment's.
const TITLE_Y: i32 = 22;
const CALL_Y: i32 = 34;
/// The centre the count slams in at, and the box it claims.
pub(super) const COUNT_AT: (i32, i32) = (192, 112);
const COUNT_BOX: Rect = (192 - 66, 60, 132, 92);

/// A line of the 5x7 hand, on a small plate.
#[derive(Clone, Debug)]
pub(super) struct Word {
    pub text: String,
    pub at: (i32, i32),
    pub ink: char,
    pub kind: &'static str,
    /// The team it names, if it names one.
    pub who: Option<u8>,
}

impl Word {
    /// The plate the word sits on.
    pub fn rect(&self) -> Rect {
        (self.at.0 - 2, self.at.1 - 1, text_width(&self.text) + 4, 9)
    }
}

/// A word set in the big hand: the place.
#[derive(Clone, Debug)]
pub(super) struct Big {
    pub text: String,
    pub at: (i32, i32),
    pub scale: i32,
    pub ink: char,
}

impl Big {
    pub fn rect(&self) -> Rect {
        (self.at.0, self.at.1, text_width(&self.text) * self.scale + 4, 7 * self.scale + 4)
    }
}

/// A banner on show.
#[derive(Clone, Debug)]
pub(super) struct Banner {
    pub title: String,
    pub sub: String,
    pub tone: Tone,
    pub age: u32,
    pub life: u32,
    pub y: i32,
    pub cap: i32,
    pub rank: u8,
    /// The team it calls, if one's.
    pub who: Option<u8>,
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
    pub words: Vec<Word>,
    pub bigs: Vec<Big>,
    pub banner: Option<Banner>,
    pub count: Option<Count>,
    /// The boxes the picture owns without words: plates, the minimap.
    pub plates: Vec<(&'static str, Rect)>,
    /// The words the kit sets on the podium's steps: what kind, what, where, whose team.
    pub podium: Vec<(&'static str, String, Rect, Option<u8>)>,
}

impl Plan {
    /// Every box the plan claims, with a name for it.
    pub fn claims(&self) -> Vec<(String, Rect, Option<u8>)> {
        let mut out: Vec<_> = self.words.iter().map(|w| (format!("{} {:?}", w.kind, w.text), w.rect(), w.who)).collect();
        out.extend(self.bigs.iter().map(|b| (format!("big {:?}", b.text), b.rect(), None)));
        if let Some(b) = &self.banner {
            out.push((format!("banner {:?}", b.title), b.rect, b.who));
        }
        if self.count.is_some() {
            out.push(("count".into(), COUNT_BOX, None));
        }
        out.extend(self.plates.iter().map(|&(n, r)| (n.to_string(), r, None)));
        out.extend(self.podium.iter().map(|(k, t, r, who)| (format!("{k} {t:?}"), *r, *who)));
        out
    }

    fn free(&self, r: Rect) -> bool {
        inside(r) && self.claims().iter().all(|(_, c, _)| !meets(r, *c))
    }
}

/// The pixels the kit's `banner` marks on the canvas, `age` ticks in: the rails, the words and the
/// line under them, as one box.
pub(super) fn banner_rect(y: i32, title: &str, sub: &str, age: u32, cap: i32) -> Rect {
    let w = SCREEN.0;
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
    (x0.max(0), y0, x1.min(w) - x0.max(0), y1 - y0)
}

pub(super) fn banner_of(title: String, sub: String, tone: Tone, age: u32, life: u32, y: i32, cap: i32, rank: u8, who: Option<u8>) -> Option<Banner> {
    (age < life).then(|| {
        // The box stays what it is at its widest and tallest, so a word never meets another
        // mid-slam: the slam grows it, the claim is the grown one.
        let rect = banner_rect(y, &title, &sub, 0, cap);
        let settled = banner_rect(y, &title, &sub, 99, cap);
        let (x0, y0) = (rect.0.min(settled.0), rect.1.min(settled.1));
        let (x1, y1) = ((rect.0 + rect.2).max(settled.0 + settled.2), (rect.1 + rect.3).max(settled.1 + settled.3));
        // A sub line is claimed from the start, though it shows after the fourth tick.
        let withsub = banner_rect(y, &title, &sub, 99, cap);
        let (x0, x1) = (x0.min(withsub.0), x1.max(withsub.0 + withsub.2));
        Banner { title, sub, tone, age, life, y, cap, rank, who, rect: (x0, y0, x1 - x0, y1 - y0) }
    })
}

fn place_word(n: u32) -> String {
    let suffix = match n {
        1 => "ST",
        2 => "ND",
        3 => "RD",
        _ => "TH",
    };
    format!("{n}{suffix}")
}

/// The call standing for the viewer: the loudest of what happened lately.
fn call(v: &View) -> Option<Banner> {
    let race = v.race;
    let now = v.now;
    let me = v.my_team();
    let mut best: Option<Banner> = None;
    let mut offer = |b: Option<Banner>| {
        if let Some(b) = b {
            if best.as_ref().is_none_or(|o| b.rank > o.rank || (b.rank == o.rank && b.age < o.age)) {
                best = Some(b);
            }
        }
    };
    for f in &race.show.fx {
        let age = f.age(now);
        let mine = v.is_me(f.team);
        let theirs = v.is_me(f.other);
        let name = |t: u8| v.called(t, 9);
        let make = |rank: u8, life: u32, title: &str, sub: &str, tone: Tone, who: Option<u8>| {
            banner_of(title.into(), sub.into(), tone, age, life, CALL_Y, if rank >= 6 { 3 } else { 2 }, rank, who)
        };
        match f.kind {
            Kind::Photo if age < 76 => {
                let sub = if f.n == 0 { "DEAD HEAT".to_string() } else { format!("{}.{:02} SECONDS", f.n / 100, f.n % 100) };
                offer(make(9, 76, "PHOTO FINISH", &sub, Tone::Warm, None));
            }
            Kind::Finish if mine && age < CALL_LIFE => {
                let place = u32::from(f.n);
                let sub = if place == 1 && race.racers.len() > 1 && v.race.seats().len() > 1 { "TAKE THE LAUREL" } else { "" };
                offer(make(8, CALL_LIFE, &format!("{} HOME", place_word(place)), sub, if place == 1 { Tone::Warm } else { Tone::Steel }, Some(me)));
            }
            Kind::Finish if !mine && f.n == 1 && age < 44 => {
                offer(make(5, 44, &format!("{} HOME", name(f.team)), "FIRST", Tone::Steel, Some(f.team)));
            }
            Kind::Wreck if mine && age < 46 => {
                let sub = if f.other == 255 { "IRON".to_string() } else { format!("BY {}", name(f.other)) };
                offer(make(7, 46, "WRECKED", &sub, Tone::Hot, Some(f.other).filter(|&o| o != 255)));
            }
            Kind::Wreck if !mine && f.weight >= Weight::Crushing && age < 36 => {
                offer(make(2, 36, &format!("{} DOWN", name(f.team)), "", Tone::Steel, Some(f.team)));
            }
            Kind::Barge if mine && age < 34 => {
                offer(make(if f.weight >= Weight::Crushing { 6 } else { 4 }, 34, "BARGE", &name(f.other), Tone::Warm, Some(f.other)));
            }
            Kind::Barge if theirs && age < 34 => {
                offer(make(if f.weight >= Weight::Crushing { 6 } else { 5 }, 34, "SHOVED", &format!("BY {}", name(f.team)), Tone::Hot, Some(f.team)));
            }
            Kind::Lead if mine && age < CALL_LIFE => {
                offer(make(6, CALL_LIFE, "LEAD", &format!("PASSED {}", name(f.other)), Tone::Warm, Some(f.other)));
            }
            Kind::Lead if !mine && age < 44 => {
                offer(make(if theirs { 5 } else { 3 }, 44, &format!("{} LEADS", name(f.team)), "", if theirs { Tone::Hot } else { Tone::Steel }, Some(f.team)));
            }
            Kind::Pass if mine && age < 46 => {
                offer(make(4, 46, &place_word(u32::from(f.n)), &format!("PASSED {}", name(f.other)), Tone::Warm, Some(f.other)));
            }
            Kind::Pass if theirs && age < 40 => {
                offer(make(3, 40, "PASSED", &format!("BY {}", name(f.team)), Tone::Hot, Some(f.team)));
            }
            Kind::Final if mine && age < CALL_LIFE => offer(make(6, CALL_LIFE, "FINAL LAP", "", Tone::Warm, Some(me))),
            Kind::Lap if mine && f.n >= 1 && age < 40 && u32::from(f.n) + 1 < LAPS => {
                offer(make(3, 40, &format!("LAP {}", u32::from(f.n) + 1), "", Tone::Steel, Some(me)));
            }
            Kind::Fastest if mine && age < 56 => {
                offer(make(5, 56, "FASTEST LAP", &format!("{}.{} SECONDS", f.n / 10, f.n % 10), Tone::Warm, Some(me)));
            }
            _ => {}
        }
    }
    best
}

/// Where the count is: the beat (3, 2, 1) while the gate holds, the GO after it drops.
fn count(v: &View) -> Option<Count> {
    let race = v.race;
    match race.phase {
        Phase::Gate => {
            let beat = (race.t / HZ).min(2) as usize;
            Some(Count { label: ["3", "2", "1"][beat], age: race.t % HZ, life: HZ, go: false })
        }
        Phase::Racing if race.t < HZ => Some(Count { label: "GO", age: race.t, life: HZ, go: true }),
        _ => None,
    }
}

/// Ticks of the race the keys stay up.
const KEYS_FOR: u32 = 5 * HZ;

/// The keys, as the words beside them: one plain word each.
pub(super) const KEYS: [(&[&str], &str); 3] = [(&["A", "D"], "STEER"), (&["SPACE"], "WHIP"), (&["X"], "IRON")];

/// The prompts' boxes along the foot: for each, the keycaps and the word, left to right.
pub(super) struct Prompt {
    pub caps: Vec<(Rect, &'static str)>,
    pub word: Word,
}

pub(super) fn prompts() -> Vec<Prompt> {
    let mut x = 8;
    let y = 207;
    KEYS.iter()
        .map(|&(keys, word)| {
            let mut caps = Vec::new();
            for &k in keys {
                let w = text_width(k) + 6;
                caps.push(((x, y - 1, w, 11), k));
                x += w + 2;
            }
            let w = Word { text: word.into(), at: (x + 3, y + 1), ink: 'h', kind: "key", who: None };
            x += text_width(word) + 14;
            Prompt { caps, word: w }
        })
        .collect()
}

fn keys_up(v: &View) -> bool {
    match v.race.phase {
        Phase::Gate => true,
        Phase::Racing => v.race.t < KEYS_FOR,
        Phase::Done => false,
    }
}

/// The plan of a seat's screen at this tick.
pub(super) fn plan_of(v: &View) -> Plan {
    if let Some(age) = super::finish::ended(v).filter(|&a| a >= super::finish::OPENS) {
        return super::finish::plan(v, age - super::finish::OPENS);
    }
    let mut p = Plan::default();
    let race = v.race;
    // The plates the picture owns: the ladder along the top, the team's marks, the minimap.
    p.plates.push(("ladder", (116, 2, 152, 14)));
    p.plates.push(("team", (SCREEN.0 - 70, 2, 66, 22)));
    p.plates.push(("minimap", (SCREEN.0 - 96, SCREEN.1 - 58, 92, 54)));
    if race.phase != Phase::Gate {
        let place = place_word(v.place(v.me) as u32);
        let lap = (race.racers[v.me].lap(v.course()) + 1).min(LAPS);
        let w = Word { text: format!("LAP {lap}"), at: (9, 6), ink: 't', kind: "lap", who: None };
        p.words.push(w);
        p.bigs.push(Big { text: place, at: (4, 21), scale: 2, ink: if v.place(v.me) == 1 { 't' } else { 'i' } });
    }
    if keys_up(v) {
        for pr in prompts() {
            for (r, _) in &pr.caps {
                p.plates.push(("keycap", *r));
            }
            p.words.push(pr.word);
        }
    }
    p.count = count(v);
    // The title card of the course while the gate holds, else the call of the moment.
    if race.phase == Phase::Gate && race.t < 66 {
        let c = v.course();
        let title = c.name.to_uppercase();
        p.banner = banner_of(title, "THREE LAPS".into(), Tone::Warm, race.t, 66, TITLE_Y, 2, 10, None);
    } else if let Some(age) = super::finish::ended(v) {
        p.banner = super::finish::frozen_banner(v, age);
    } else if let Some(b) = call(v) {
        if b.age < b.life {
            p.banner = Some(b);
        }
    }
    // A banner that would meet what is already claimed is not shown.
    if let Some(b) = p.banner.take() {
        let held: Vec<Rect> = p.claims().iter().map(|c| c.1).collect();
        if inside(b.rect) && held.iter().all(|&c| !meets(b.rect, c)) {
            p.banner = Some(b);
        }
    }
    // The names over the other teams, nearest the viewer first, each on the first free row.
    let cam = v.camera();
    let me = v.map_px(v.me);
    let mut others: Vec<(usize, f32)> = (0..race.racers.len())
        .filter(|&i| i != v.me || !v.mine)
        .map(|i| {
            let (x, y) = v.map_px(i);
            (i, (x - me.0).hypot(y - me.1))
        })
        .collect();
    others.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (i, _) in others.into_iter().take(5) {
        let (x, y) = v.map_px(i);
        let (sx, sy) = (x as i32 - cam.0, y as i32 - cam.1);
        let team = race.racers[i].team;
        let text = v.called(team, 9);
        if text.is_empty() {
            continue;
        }
        let tw = text_width(&text);
        for lift in [42, 51, 60, 33] {
            let at = (sx - tw / 2, sy - lift);
            let word = Word { text: text.clone(), at, ink: v.ink(i), kind: "tag", who: Some(team) };
            if p.free(word.rect()) {
                p.words.push(word);
                break;
            }
        }
    }
    p
}

// ─── drawing ─────────────────────────────────────────────────────────────────

/// A word on its plate: dark, with a pixel of rim.
pub(super) fn draw_word(im: &mut Img, w: &Word) {
    let (x, y, pw, ph) = w.rect();
    for yy in y..y + ph {
        for xx in x..x + pw {
            let corner = (xx == x || xx == x + pw - 1) && (yy == y || yy == y + ph - 1);
            if !corner {
                im.put(xx, yy, if yy == y + ph - 1 { 'g' } else { 'K' });
            }
        }
    }
    im.text(w.at.0, w.at.1, &w.text, w.ink);
}

/// A thin dither of black paper behind words that sit over a bright scene, softest at its edges.
fn scrim(im: &mut Img, (x0, y, w, h): Rect) {
    for yy in y.max(0)..(y + h).min(im.h) {
        for xx in x0.max(0)..(x0 + w).min(im.w) {
            let across = ((xx - x0).min(x0 + w - 1 - xx) as f32 / 20.0).min(1.0);
            let down = (((yy - y).min(y + h - 1 - yy)) as f32 / 6.0).min(1.0);
            if bayer(xx, yy) < 0.62 * across.min(down) {
                im.put(xx, yy, 'k');
            }
        }
    }
}

/// A figure slamming in at `at`, springing to size, ringing, and going.
fn slam(im: &mut Img, at: (i32, i32), label: &str, age: u32, life: u32, go: bool) {
    if age >= life {
        return;
    }
    let width = text_width(label).max(5);
    let rest = (60 / width).clamp(3, if go { 5 } else { 4 });
    let pop = Spring::SNAPPY.at(age as f32, 1.6, 1.0, 0.0).clamp(0.9, 1.6);
    let out = ((age as f32 - life as f32 * 0.7) / (life as f32 * 0.3)).max(0.0);
    let scale = (((rest as f32 * pop) * (1.0 - out * 0.4)).round() as i32).max(1);
    let (fill, edge) = if go { ('5', 'p') } else { ('T', 'B') };
    let mut t = juice::big_text_img(label, scale, if age < 2 { 'H' } else { fill }, edge);
    juice::dissolve(&mut t, out);
    {
        let mut pen = Pen::new(im, 700);
        let a = age as f32;
        let c = (at.0 as f32, at.1 as f32 + 4.0);
        pen.shock(c, (6.0 + a * 10.0).min(84.0), 2.2, 0.5, if go { ramps::GOLD } else { ramps::BONE }, (a / 14.0).min(1.0));
        if go {
            pen.shock(c, (2.0 + a * 16.0).min(110.0), 1.6, 0.5, ramps::EMBER, (a / 12.0).min(1.0));
            pfx::rays(&mut pen, (at.0 as f32, at.1 as f32), 14, 64.0, 0.7, 0.2, ramps::GOLD, (a / 14.0).min(1.0));
        }
    }
    im.stamp(&t, at.0 - t.w / 2, at.1 - t.h / 2);
}

/// The gantry of three lamps: red as the beats go by, green on the start.
fn lamps(im: &mut Img, t: u32, green: bool, since: u32) {
    let (cx, cy) = (192, 74);
    im.rect(cx - 46, cy - 12, 92, 24, 'K');
    im.frame(cx - 46, cy - 12, 92, 24, 'g');
    im.rect(cx - 2, cy + 12, 4, 6, 'g');
    for i in 0..3u32 {
        let lit = green || t >= i * HZ;
        let x = cx - 30 + i as i32 * 30;
        let born = if green { since } else { t.saturating_sub(i * HZ) } as f32;
        let swell = if lit && born < 5.0 { 1.0 + (1.0 - born / 5.0) * 0.5 } else { 1.0 };
        let r = 8.0 * swell;
        let ramp: Ramp = if green { ramps::NATURE } else { ramps::BLOOD };
        for dy in -10..=10 {
            for dx in -10..=10 {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d > r {
                    continue;
                }
                let ink = if !lit {
                    if d > r - 1.5 { 'j' } else { 'g' }
                } else if d > r - 1.5 {
                    ramp.step(4)
                } else if d > r * 0.55 {
                    ramp.step(2)
                } else if (dx + 2) * (dx + 2) + (dy + 3) * (dy + 3) < 6 {
                    ramp.step(0)
                } else {
                    ramp.step(1)
                };
                im.put(x + dx, cy + dy, ink);
            }
        }
    }
}

/// The start over the whole canvas: the lamps count red, the digits slam in, the green and GO
/// ring out.
pub(super) fn draw_start(over: &mut Img, plan: &Plan, v: &View) {
    let race = v.race;
    let Some(c) = &plan.count else { return };
    if race.phase == Phase::Gate {
        lamps(over, race.t, false, 0);
    } else {
        lamps(over, race.t, true, race.t);
    }
    slam(over, COUNT_AT, c.label, c.age, c.life, c.go);
}

/// The banner of the plan.
pub(super) fn draw_banner(over: &mut Img, b: &Banner) {
    if b.rank >= 6 || b.y == TITLE_Y {
        scrim(over, b.rect);
    }
    juice::banner(over, b.y, &b.title, &b.sub, b.age, b.life, b.tone, b.cap);
}

/// How long the gate holds: the title and the three beats.
pub(super) const GATE: u32 = GATE_TICKS;
