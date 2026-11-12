//! Brasswake's moments: the title slam and the GO, the banners the aisle and the table call, the
//! numbers a blow throws up and the sparks that carry its work to the plate. Each is a pure
//! function of ages on the run's tick, and each pixel word claims its box here (`Plan`), so the
//! picture and the words test read the same geometry.
use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Tone};
use super::hud::{self, Rect, W};
use crate::motion::{self, ease};
use crate::rules::brasswake::{
    LENGTH, Lane, Session,
    show::{Cry, Kind},
};

/// Ticks the title holds, and the GO that follows it.
pub(super) const TITLE: u32 = 44;
pub(super) const GO: u32 = 30;
/// Ticks a score takes to fly from the foe to the plate.
pub(super) const FLY: u32 = 22;
/// Ticks a number hangs over a struck foe.
pub(super) const POP: u32 = 20;
/// The banner's top row at rest.
pub(super) const BANNER_Y: i32 = 44;
/// Where the GO stands, its centre, and the box it claims for its whole life.
pub(super) const GO_AT: (i32, i32) = (192, 104);
pub(super) const GO_BOX: Rect = (96, 50, 192, 108);

/// The work still flying to the plate at `now`.
pub(super) fn in_flight(d: &Lane, now: u32) -> u32 {
    d.show
        .marks
        .iter()
        .filter_map(|m| match m.kind {
            Kind::Blow { gain, .. } if m.age(now) < FLY => Some(u32::from(gain)),
            _ => None,
        })
        .sum()
}

/// Ticks since a flying score last landed on the plate, while the bump lasts.
pub(super) fn landing(d: &Lane, now: u32) -> Option<u32> {
    d.show
        .marks
        .iter()
        .filter(|m| matches!(m.kind, Kind::Blow { .. }))
        .filter_map(|m| m.age(now).checked_sub(FLY))
        .filter(|&a| a < 8)
        .min()
}

/// The pixels the kit's `banner` marks on the screen, `age` ticks in: the rails, the words and the
/// line under them, as one box. It reads the kit's own geometry (a slam makes the first ticks
/// larger); the fade at the very end lifts the banner a few pixels, and the placements leave room.
pub(super) fn banner_rect(title: &str, sub: &str, age: u32, cap: i32) -> Rect {
    banner_rect_at(BANNER_Y, title, sub, age, cap)
}

/// `banner_rect` for a banner whose top row at rest is `y`.
pub(super) fn banner_rect_at(y: i32, title: &str, sub: &str, age: u32, cap: i32) -> Rect {
    let tw = text_width(title).max(1);
    let rest = ((W - 24) / tw).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / tw).max(rest);
    let text_w = tw * scale + 4;
    let top = y - (scale - rest) * 3;
    let half = text_w / 2 + 8;
    let (mut x0, mut x1) = (W / 2 - half, W / 2 + half);
    let y0 = top.min(y - 5);
    let mut y1 = (top + 7 * scale + 4).max(y + 7 * rest + 10);
    if !sub.is_empty() && age >= 4 {
        let sw = text_width(sub) + 4;
        x0 = x0.min(W / 2 - sw / 2);
        x1 = x1.max(W / 2 - sw / 2 + sw);
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
    pub rect: Rect,
}

/// A number over a struck foe.
#[derive(Clone, Debug)]
pub(super) struct Pop {
    pub text: String,
    /// Where it started: over the foe's head.
    pub at: (i32, i32),
    pub age: u32,
    pub scale: i32,
    pub ink: char,
    pub rect: Rect,
}

/// Everything the moments put on the screen at one tick.
#[derive(Clone, Debug, Default)]
pub(super) struct Plan {
    /// Every line of the 5x7 font on the screen.
    pub words: Vec<hud::Word>,
    pub banner: Option<Banner>,
    pub pops: Vec<Pop>,
    /// Ticks into the GO, while it shows.
    pub go: Option<u32>,
}

fn who(run: &Run, id: u32, me: u32) -> String {
    if id == me { "YOU".into() } else { hud::short(&hud::name_of(run, id, 12)) }
}
use crate::rules::Run;

/// The banner the screen of seat `id` shows now: the loudest of what is alive.
pub(super) fn banner(run: &Run, s: &Session, id: u32) -> Option<Banner> {
    let d = &s.lanes[&id];
    let now = run.tick as u32;
    let elapsed = LENGTH - s.left;
    let mut all: Vec<Banner> = Vec::new();
    let mut add = |title: String, sub: String, tone: Tone, age: u32, life: u32, cap: i32, rank: u8| {
        if age < life {
            let rect = banner_rect(&title, &sub, age, cap);
            all.push(Banner { title, sub, tone, age, life, cap, rank, rect });
        }
    };
    if elapsed < TITLE {
        add("BRASSWAKE".into(), "BRAWL THEN THROW".into(), Tone::Warm, elapsed, TITLE, 3, 10);
    }
    for m in &d.show.marks {
        let age = m.age(now);
        match m.kind {
            Kind::Clear { wave } if wave < 2 => add(format!("AISLE {}", wave + 2), "GATE OPEN".into(), Tone::Warm, age, 38, 2, 6),
            Kind::Clear { .. } => add("VAT KEEPER".into(), "FINAL AISLE".into(), Tone::Hot, age, 56, 3, 7),
            Kind::Quake { hit: false, .. } if (m.x - d.x).abs() < 72 && (m.y - d.y).abs() < 40 => {
                add("DODGED".into(), String::new(), Tone::Steel, age, 28, 2, 3)
            }
            Kind::Fall => add("DOWN".into(), "BACK UP".into(), Tone::Hot, age, 44, 3, 8),
            Kind::Blow { boss: true, slain: true, .. } => add("KEEPER DOWN".into(), format!("{} WORK", d.score), Tone::Warm, age, 96, 3, 9),
            Kind::Smash { hit: true, from } => {
                add("CASK HIT".into(), format!("FROM {}", who(run, from, id)), Tone::Hot, age, 40, 2, 4)
            }
            Kind::Smash { hit: false, .. } => add("DODGED".into(), String::new(), Tone::Steel, age, 30, 2, 3),
            Kind::Landed { hit: true, to } => add("DIRECT HIT".into(), format!("ON {}", who(run, to, id)), Tone::Warm, age, 44, 2, 4),
            Kind::Landed { hit: false, to } => add("MISSED".into(), format!("{} DODGED", who(run, to, id)), Tone::Steel, age, 36, 2, 3),
            Kind::Streak { n } => add(format!("COMBO X{n}"), String::new(), Tone::Warm, age, 40, 2, 5),
            _ => {}
        }
    }
    let last = LENGTH - 10 * crate::rules::HZ;
    if elapsed >= last {
        add("LAST CALL".into(), "10 SECONDS".into(), Tone::Warm, elapsed - last, 40, 2, 6);
    }
    for c in &s.show.list {
        let age = c.age(now);
        let mine = c.who == id;
        let name = who(run, c.who, id);
        match c.cry {
            Cry::Lead if mine => add("YOU LEAD".into(), String::new(), Tone::Warm, age, 56, 2, 5),
            Cry::Lead => add(format!("{name} LEADS"), String::new(), Tone::Steel, age, 56, 2, 5),
            Cry::Keeper if !mine => add(format!("{name} AT KEEPER"), String::new(), Tone::Steel, age, 56, 2, 6),
            Cry::Team => add("TEAM KICK".into(), String::new(), Tone::Warm, age, 48, 2, 4),
            Cry::Slain if !mine => add("KEEPER FELL".into(), format!("BY {name}"), Tone::Steel, age, 72, 2, 8),
            Cry::Fell if !mine => add(format!("{name} DOWN"), String::new(), Tone::Steel, age, 36, 1, 2),
            _ => {}
        }
    }
    // Nothing speaks over the GO.
    if (TITLE..TITLE + GO).contains(&elapsed) {
        return None;
    }
    all.into_iter().max_by_key(|b| (b.rank, u32::MAX - b.age))
}

/// The numbers over struck foes, each placed where it meets no word, no banner and no other number.
/// `anchor(foe)` is where a foe's head is on the screen; `fixed` the boxes of words already claimed.
pub(super) fn pops(run: &Run, s: &Session, id: u32, anchor: &dyn Fn(usize) -> Option<(i32, i32)>, fixed: &[Rect]) -> Vec<Pop> {
    let d = &s.lanes[&id];
    let now = run.tick as u32;
    let mut blows: Vec<_> = d
        .show
        .marks
        .iter()
        .filter(|m| m.wave == d.wave && m.age(now) < POP)
        .filter_map(|m| match m.kind {
            Kind::Blow { swing, slain, gain, foe, .. } => Some((m.born, swing, slain, gain, foe as usize, m.age(now))),
            _ => None,
        })
        .collect();
    blows.sort_by_key(|b| b.0);
    let mut placed: Vec<Pop> = Vec::new();
    for (_, swing, slain, gain, foe, age) in blows {
        let Some(head) = anchor(foe) else { continue };
        let (scale, ink) = if slain { (2, '6') } else { [(1, 'i'), (1, 't'), (2, 'T')][usize::from(swing.min(2))] };
        let text = format!("+{gain}");
        let (w, h) = (text_width(&text) * scale + 4, 7 * scale + 4);
        let x = (head.0).clamp(w / 2 + 2, W - w / 2 - 2);
        let mut lift = 0;
        let mut found = None;
        for _ in 0..8 {
            let hop = (ease::out((age as f32 / 7.0).min(1.0)) * 12.0) as i32 + (age as i32 * 3 / 5);
            let rect = (x - w / 2, head.1 - lift - hop - h, w, h);
            let clear = rect.1 > hud::TOP + 1
                && hud::inside(rect)
                && fixed.iter().all(|&f| !hud::meets(rect, f))
                && placed.iter().all(|p| !hud::meets(rect, p.rect));
            if clear {
                found = Some(rect);
                break;
            }
            lift += 13;
        }
        if let Some(rect) = found {
            placed.push(Pop { text, at: (x, head.1 - lift), age, scale, ink, rect });
        }
    }
    placed
}

/// The whole plan of seat `id`'s screen.
pub(super) fn plan(run: &Run, s: &Session, id: u32, anchor: &dyn Fn(usize) -> Option<(i32, i32)>) -> Plan {
    let elapsed = LENGTH - s.left;
    let banner = banner(run, s, id);
    let go = (TITLE..TITLE + GO).contains(&elapsed).then(|| elapsed - TITLE);
    let words = hud::words(run, s, id);
    let mut fixed: Vec<Rect> = words.iter().map(hud::Word::rect).collect();
    fixed.push(hud::SCORE_BOX);
    fixed.push(hud::COMBO_BOX);
    if let Some(b) = &banner {
        fixed.push((b.rect.0 - 3, b.rect.1 - 3, b.rect.2 + 6, b.rect.3 + 6));
    }
    if go.is_some() {
        fixed.push(GO_BOX);
    }
    let pops = pops(run, s, id, anchor, &fixed);
    Plan { words, banner, pops, go }
}

/// Every box of pixel text in `plan`, named.
pub(super) fn boxes(plan: &Plan) -> Vec<(&'static str, Rect)> {
    let mut out = Vec::new();
    if let Some(b) = &plan.banner {
        out.push(("banner", b.rect));
    }
    if plan.go.is_some() {
        out.push(("go", GO_BOX));
    }
    out.extend(plan.pops.iter().map(|p| ("pop", p.rect)));
    out
}

// ─── drawing ─────────────────────────────────────────────────────────────────

pub(super) fn paint(im: &mut Img, plan: &Plan) {
    if let Some(go) = plan.go {
        juice::countdown(im, GO_AT, "GO", go, GO, true);
    }
    if let Some(b) = &plan.banner {
        juice::banner(im, BANNER_Y, &b.title, &b.sub, b.age, b.life, b.tone, b.cap);
    }
    for p in &plan.pops {
        juice::pop_number(im, &p.text, p.at, p.age, POP, p.scale, p.ink);
    }
}

/// The sparks that carry a blow's work to the plate: from the foe's head up and over to the score.
pub(super) fn sparks(im: &mut Img, d: &Lane, now: u32, anchor: &dyn Fn(usize) -> Option<(i32, i32)>) {
    const PLATE: (f32, f32) = (30.0, 24.0);
    for m in &d.show.marks {
        let Kind::Blow { gain, foe, .. } = m.kind else { continue };
        let age = m.age(now);
        if m.wave != d.wave || !(8..FLY).contains(&age) {
            continue;
        }
        let Some(head) = anchor(foe as usize) else { continue };
        let t = (age - 8) as f32 / (FLY - 8) as f32;
        let big = gain > 10;
        for k in 0..4u32 {
            let tt = (t - k as f32 * 0.07).max(0.0);
            let (x, y) = motion::arc((head.0 as f32, head.1 as f32 - 14.0), PLATE, 30.0, ease::both(tt));
            let ink = ['6', '@', 'o', 'R'][k as usize];
            im.put(x as i32, y as i32, ink);
            if big && k < 2 {
                im.put(x as i32 + 1, y as i32, ink);
                im.put(x as i32, y as i32 + 1, ink);
            }
        }
    }
}
