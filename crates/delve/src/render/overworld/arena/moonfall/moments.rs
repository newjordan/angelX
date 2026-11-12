//! The beats of Moonfall: the day's card at each dawn and the dusk's, the
//! bell's rewind (a clock hand that sweeps back, time streaming in, a toll
//! that shakes the town), the hour's toll, the fireflies of the night, a
//! mask or a Ledger line found, and the lights holding. Every one is a pure
//! function of the session's own counters (the clock, the phase, a note's
//! ticks left), because the rules are a Bend module and keep no picture
//! state; one card owns the middle of the town at a time, and the card's box
//! is claimed so no word meets it.
use super::super::super::ink::{Img, bayer, hash, text_width};
use super::super::juice::{self, Tone};
use super::super::pfx::{Pen, Shape, Spray, mote, ramps};
use super::labels::Rect;
use super::{Cam, VIEW_H, VIEW_Y};
use crate::motion::{self, ease};
use crate::rules::moonfall::{
    Session, clock, control,
    state::{Knight, Mask, Note, Phase},
    town,
};
use std::cell::Cell;
use std::f32::consts::TAU;

/// Ticks the day's card holds, and the dusk's, and a mask's.
const DAY_LIFE: u32 = 64;
const DUSK_LIFE: u32 = 44;
const MASK_LIFE: u32 = 72;
/// Ticks a knight's note and a news line start with (the rules' own).
const NOTE_START: u32 = 90;
const NEWS_START: u32 = 120;

// ─── cards ───────────────────────────────────────────────────────────────────

/// A card on the town: what it says, how it is inked, how old it is.
pub(super) struct Card {
    pub title: String,
    pub sub: String,
    pub age: u32,
    pub life: u32,
    pub tone: Tone,
    pub cap: i32,
}

/// The row a card's title rests on: in the sky band over the town, where
/// no word of the town's and none of the HUD's stands, so the knights and
/// the news stay in view.
const CARD_Y: i32 = 10;

fn card_y(_: &Session, _: u32) -> i32 {
    CARD_Y
}

/// The one card on show, by rank: the lights holding, a mask earned, the
/// day's dawn, the dusk.
pub(super) fn card(s: &Session, me: &Knight, shown: u32) -> Option<Card> {
    if let Phase::Won { t } = s.phase {
        return Some(Card { title: "THE LIGHTS HOLD".into(), sub: String::new(), age: t, life: 110, tone: Tone::Warm, cap: 3 });
    }
    if !matches!(s.phase, Phase::Play) || s.completed.is_some() {
        return None;
    }
    if let Some((Note::Mask(m), left)) = me.note {
        let age = NOTE_START.saturating_sub(left);
        if age < MASK_LIFE && m != Mask::Bare {
            let title = if m == Mask::Swift { "SWIFT MASK" } else { "STOUT MASK" };
            return Some(Card { title: title.into(), sub: String::new(), age, life: MASK_LIFE, tone: Tone::Warm, cap: 2 });
        }
    }
    let day = s.nums.day.max(1);
    let k = shown / day;
    if k < 3 {
        let age = shown - k * day;
        if age < DAY_LIFE {
            return Some(Card { title: format!("DAY {}", k + 1), sub: String::new(), age, life: DAY_LIFE, tone: Tone::Warm, cap: 3 });
        }
        let dusk = k * day + day / 2;
        if shown >= dusk && shown - dusk < DUSK_LIFE {
            return Some(Card { title: "DUSK".into(), sub: String::new(), age: shown - dusk, life: DUSK_LIFE, tone: Tone::Steel, cap: 3 });
        }
    }
    None
}

/// The box a card claims: its biggest drawing (the slam, three scales over
/// its rest) and its rails, so no word and no other card meets it.
pub(super) fn card_rect(c: &Card, y: i32) -> Rect {
    let tw = text_width(&c.title).max(1);
    let rest = ((384 - 24) / tw).clamp(1, c.cap.max(1));
    let big = (rest + 3).min(176 / tw).max(rest);
    let d = big - rest;
    let w = (tw * big + 4 + 16).max(text_width(&c.sub) + 6);
    let top = (y - 6).min(y - 3 * d);
    let bottom = (y + 7 * rest + 10).max(y + 7 * rest + 4 * d + 5) + if c.sub.is_empty() { 0 } else { 14 };
    Rect::new(192 - w / 2, top, w, bottom - top)
}

/// The card on show and the box it claims.
pub(super) fn card_now(s: &Session, me: &Knight) -> Option<(Card, Rect)> {
    let shown = s.shown_clock();
    let c = card(s, me, shown)?;
    let r = card_rect(&c, card_y(s, shown));
    Some((c, r))
}

pub(super) fn draw_card(im: &mut Img, s: &Session, me: &Knight) {
    if let Some((c, _)) = card_now(s, me) {
        let y = card_y(s, s.shown_clock());
        juice::banner(im, y, &c.title, &c.sub, c.age, c.life, c.tone, c.cap);
    }
}

// ─── the room's shake ────────────────────────────────────────────────────────

/// How far the town is jolted this tick, in pixels: the hand a touch leaves,
/// the toll of the bell, the moon landing, each dawn's rumble as it grows, the
/// lights holding.
pub(super) fn jolt(s: &Session, me: &Knight, tick: u32) -> (i32, i32) {
    let decay = |age: f32, life: f32| (1.0 - age / life).clamp(0.0, 1.0).powi(2);
    let mut amp = 0.0f32;
    if me.hurt + 4 >= crate::rules::moonfall::state::WARDED {
        amp = amp.max(2.0);
    }
    match s.phase {
        Phase::Rewind { t, fell, .. } => {
            let ctl = control(&s.nums).max(1) as f32;
            let t = t as f32;
            if fell {
                let land = ctl * 0.45;
                if t >= land - 3.0 && t < land + 18.0 {
                    amp = amp.max(6.0 * decay((t - land).max(0.0), 18.0));
                } else if t < land {
                    // The moon coming down rumbles louder as it nears.
                    amp = amp.max(0.8 + 2.4 * t / land);
                }
            } else {
                for b in [0.0, 9.0, 18.0] {
                    if t >= b && t < b + 8.0 {
                        amp = amp.max((if b == 0.0 { 4.0 } else { 2.0 }) * decay(t - b, 8.0));
                    }
                }
            }
        }
        Phase::Won { t } => {
            let t = t as f32;
            if t < 14.0 {
                amp = amp.max(5.0 * decay(t, 14.0));
            }
        }
        Phase::Play if s.completed.is_none() => {
            let day = s.nums.day.max(1);
            let shown = s.clock;
            // The moon steps closer at each new dawn; the Final Hours begin with a shudder.
            for k in 1..3u32 {
                let a = shown.wrapping_sub(k * day);
                if shown >= k * day && a < 20 {
                    amp = amp.max(1.6 * decay(a as f32, 20.0));
                }
            }
            let fin = clock::final_start(day);
            if shown >= fin && shown - fin < 24 {
                amp = amp.max(2.4 * decay((shown - fin) as f32, 24.0));
            }
        }
        _ => {}
    }
    if amp < 0.5 {
        return (0, 0);
    }
    let a = tick as f32 * 2.3;
    ((amp * a.sin()).round() as i32, (amp * (a * 1.37 + 1.0).cos()).round() as i32)
}

// ─── guide lines ─────────────────────────────────────────────────────────────

thread_local! {
    static GUIDES: Cell<bool> = const { Cell::new(false) };
}

/// Show the guide lines the effects are drawn along (the shots' debug layer;
/// always off in play).
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn show_guides(on: bool) {
    GUIDES.with(|g| g.set(on));
}

fn guides() -> bool {
    GUIDES.with(Cell::get)
}

/// A spiral guide: a path from `r_out` in to `r_in` about `c`, turning `turn`
/// radians on the way (negative: counter-clockwise on the screen), flattened by
/// `squash`. The rewind's streaks of time ride along it.
#[derive(Clone, Copy)]
struct Spiral {
    c: (f32, f32),
    r_out: f32,
    r_in: f32,
    a0: f32,
    turn: f32,
    squash: f32,
}

impl Spiral {
    /// The point `t` (0 at the outer end, 1 at the inner) along it.
    fn at(&self, t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, 1.0);
        let r = self.r_in + (self.r_out - self.r_in) * (1.0 - t).powf(1.6);
        let a = self.a0 + self.turn * t;
        (self.c.0 + a.cos() * r, self.c.1 + a.sin() * r * self.squash)
    }
}

/// A bowed, swirling guide from `from` to `to`: a swing to one side that
/// grows and closes again, with a tight wobble along it. The moon's pull
/// draws the town's dust up along it.
#[derive(Clone, Copy)]
struct Swirl {
    from: (f32, f32),
    to: (f32, f32),
    bow: f32,
    wobble: f32,
    phase: f32,
}

impl Swirl {
    fn at(&self, t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, 1.0);
        let (dx, dy) = (self.to.0 - self.from.0, self.to.1 - self.from.1);
        let len = dx.hypot(dy).max(1.0);
        let (nx, ny) = (-dy / len, dx / len);
        let side = (t * std::f32::consts::PI).sin() * self.bow + (t * TAU * 2.0 + self.phase).sin() * self.wobble * (1.0 - t);
        (self.from.0 + dx * t + nx * side, self.from.1 + dy * t + ny * side)
    }
}

/// The debug layer: a guide's path dotted along its length, a bright mark at
/// every tenth, the ends in other inks.
fn draw_guide(im: &mut Img, at: impl Fn(f32) -> (f32, f32)) {
    for i in 0..=300 {
        let (x, y) = at(i as f32 / 300.0);
        let ink = if i < 3 { '7' } else if i > 297 { '3' } else if i % 30 < 2 { 'w' } else { '%' };
        if i % 3 != 2 {
            im.put(x as i32, y as i32, ink);
        }
    }
}

// ─── the rewind ──────────────────────────────────────────────────────────────

/// The rewind's under-picture, on the effects layer before the clock's face:
/// a clock hand sweeps counter-clockwise round the town, dimming a wedge of it
/// behind it, and time streams in from the edges toward the glass.
pub(super) fn rewind_under(fx: &mut Img, s: &Session, tick: u32) {
    let Phase::Rewind { t, fell, .. } = s.phase else { return };
    let ctl = control(&s.nums).max(1) as f32;
    let k = t as f32 / ctl;
    if fell && k < 0.45 {
        return;
    }
    let (cx, cy) = (192.0f32, 112.0f32);
    let fade = 1.0 - ((k - 0.78) / 0.22).clamp(0.0, 1.0);
    let hand = -(ease::both(k.min(1.0)) * 1.75 * TAU);
    for y in VIEW_Y..VIEW_Y + VIEW_H {
        for x in 0..fx.w {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let phi = dx.atan2(-dy);
            let diff = (phi - hand).rem_euclid(TAU);
            if diff > 1.5 {
                continue;
            }
            let alpha = 0.62 * (1.0 - diff / 1.5) * fade;
            if bayer(x, y) < alpha {
                fx.put(x, y, 'k');
            }
        }
    }
    // The hand's own bright edge, a thin dithered spoke.
    let tint = if fell { 'T' } else { '3' };
    for r in (50..230).step_by(1) {
        let (x, y) = ((cx + hand.sin() * r as f32) as i32, (cy - hand.cos() * r as f32) as i32);
        if (r + tick as i32) % 3 != 0 && (VIEW_Y..VIEW_Y + VIEW_H).contains(&y) {
            fx.put(x, y, tint);
        }
    }
    // Time streaming in: six spiral guides wind counter-clockwise into the
    // glass, and bright streaks ride them, a head and a fading tail.
    for arm in 0..6u32 {
        let guide = Spiral { c: (cx, cy), r_out: 215.0, r_in: 44.0, a0: arm as f32 / 6.0 * TAU + 0.4, turn: -1.15 * TAU * 0.75, squash: 0.78 };
        if guides() {
            draw_guide(fx, |t| guide.at(t));
        }
        for streak in 0..4u32 {
            let lag = streak as f32 * 0.14 + arm as f32 * 0.02;
            let head = ((k * 1.3 - lag) / 0.8).clamp(0.0, 1.0);
            if head <= 0.0 || head >= 1.0 {
                continue;
            }
            // A streak is a head and a tail, sampled close enough to read as one line.
            for n in 0..30usize {
                let (x, y) = guide.at((head - n as f32 * 0.0055).max(0.0));
                let band = n * 6 / 30;
                let ink = if fell { ['T', 't', 'O', 'o', 'R', 'p'][band] } else { ['w', '3', '3', '2', '2', '0'][band] };
                let (x, y) = (x as i32, y as i32);
                if (VIEW_Y..VIEW_Y + VIEW_H).contains(&y) && (0..fx.w).contains(&x) {
                    fx.put(x, y, ink);
                    if n < 18 {
                        fx.put(x + 1, y, ink);
                        fx.put(x, y + 1, ink);
                    }
                }
            }
        }
    }
}

/// The rewind's over-picture: the bell's toll rings out, shock after shock,
/// and the moon's landing breaks the ground (the wipe's own flare comes after).
pub(super) fn rewind_over(fx: &mut Img, s: &Session, tick: u32) {
    let Phase::Rewind { t, fell, .. } = s.phase else { return };
    let ctl = control(&s.nums).max(1) as f32;
    let t = t as f32;
    // On a scratch, so no ring or ember reaches the clock bar's words.
    let mut scratch = Img::new(fx.w, fx.h);
    {
        let mut pen = Pen::new(&mut scratch, 900);
        if fell {
            let land = ctl * 0.45;
            if t >= land {
                let a = t - land;
                pen.shock((192.0, 120.0), 6.0 + a * 12.0, 3.0, 0.62, ramps::FIRE, (a / 16.0).min(1.0));
                pen.shock((192.0, 120.0), 2.0 + a * 7.0, 2.0, 0.62, ramps::GOLD, (a / 12.0).min(1.0));
                Spray::round(7, 26, (192.0, 120.0), (2.0, 6.0), (10.0, 24.0)).each(a, |m| mote(&mut pen, &m, Shape::Ember, ramps::EMBER, 1, u64::from(tick)));
            }
        } else {
            for b in [0.0, 9.0, 18.0] {
                if t >= b && t < b + 14.0 {
                    let a = t - b;
                    pen.shock((192.0, 112.0), 40.0 + a * 6.0, 2.0, 0.78, ramps::GENIE, a / 14.0);
                }
            }
        }
    }
    for y in VIEW_Y..VIEW_Y + VIEW_H {
        for x in 0..fx.w {
            if let Some(c) = scratch.get(x, y) {
                fx.set(x, y, c);
            }
        }
    }
}

// ─── the moon's pull ─────────────────────────────────────────────────────────

/// The Gloam Moon drawing the town's dust up to it: pale motes ride swirling
/// guides from the town to the moon's face, more of them and faster the
/// nearer it hangs (a few on the first night, a gale in the Final Hours),
/// and every guide at once while it falls. Drawn over the moon's edge, under
/// the words.
pub(super) fn pull(im: &mut Img, s: &Session, shown: u32, tick: u32) {
    let (p, mx, my, r, _) = super::sky::disc(s, shown);
    let falling = matches!(s.phase, Phase::Rewind { fell: true, t, .. } if (t as f32) < control(&s.nums) as f32 * 0.45);
    let strength = if falling {
        1.0
    } else if matches!(s.phase, Phase::Play) && s.completed.is_none() && clock::night(shown, s.nums.day) {
        let d = super::sky::dread(s, shown);
        (p.powf(1.5) * (0.4 + 0.6 * d)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if strength < 0.04 && !guides() {
        return;
    }
    let arms = 2 + (9.0 * strength).round() as u32;
    let speed = 0.006 + 0.012 * strength;
    for arm in 0..arms {
        let h = |salt: i32| (hash(arm as i32, salt, 97) & 0xffff) as f32 / 65536.0;
        let from = (24.0 + h(1) * 336.0, (VIEW_Y + 50) as f32 + h(2) * (VIEW_H - 64) as f32);
        let to = (mx + (h(3) - 0.5) * r * 0.8, my + (h(4) - 0.2) * r * 0.5);
        let guide = Swirl { from, to, bow: if arm % 2 == 0 { 34.0 } else { -34.0 } * (0.6 + h(5)), wobble: 7.0, phase: h(6) * TAU };
        if guides() {
            draw_guide(im, |t| guide.at(t));
        }
        for m in 0..3u32 {
            let t = (tick as f32 * speed * (0.7 + h(7)) + h(8) + m as f32 / 3.0).fract();
            for n in 0..14usize {
                let back = n as f32 * 0.0045;
                if t < back {
                    continue;
                }
                let (x, y) = guide.at(t - back);
                let (x, y) = (x as i32, y as i32);
                if !(0..im.w).contains(&x) || !(0..VIEW_Y + VIEW_H).contains(&y) {
                    continue;
                }
                // Thin out as it nears the face, and as the tail goes.
                if n > 0 && bayer(x, y) > 0.95 - 0.07 * n as f32 {
                    continue;
                }
                let band = n / 5;
                im.put(x, y, if falling { ['6', '4', 'o'][band] } else { ['V', 'v', 'U'][band] });
            }
        }
    }
}

// ─── the town's hours ────────────────────────────────────────────────────────

/// An hour just struck, as rings out from the bell post: the big tolls at
/// 06, 12, 18 and 24, and every hour in the Final Hours.
pub(super) fn toll(im: &mut Img, s: &Session, cam: Cam, tick: u32) {
    if !matches!(s.phase, Phase::Play) || s.completed.is_some() {
        return;
    }
    let day = s.nums.day.max(1);
    let hour = (day / 24).max(1);
    let age = s.clock % hour;
    let n = s.clock / hour;
    let big = n % 6 == 0;
    let fin = s.clock >= clock::final_start(day);
    if age >= 20 || !(big || fin) {
        return;
    }
    let (x, y) = ((town::BELL_POST.0 - cam.x) as f32, (town::BELL_POST.1 - cam.y + VIEW_Y) as f32 - 14.0);
    if !(-60.0..444.0).contains(&x) || !(-40.0..260.0).contains(&y) {
        return;
    }
    let a = age as f32;
    let mut pen = Pen::new(im, 500);
    let ramp = if fin { ramps::BLOOD } else { ramps::HOLY };
    pen.ring((x, y), 3.0 + a * 3.4, 1.4, 0.62, ramp, a / 20.0);
    if big && age < 12 {
        pen.ring((x, y), 2.0 + a * 2.0, 1.2, 0.62, ramps::GOLD, a / 12.0);
    }
    let _ = tick;
}

/// Fireflies over the fen after dusk: a few dozen warm points drifting on
/// slow loops, blinking, over the open ground of the town in view.
pub(super) fn fireflies(im: &mut Img, s: &Session, cam: Cam, shown: u32, tick: u32) {
    let day = s.nums.day.max(1);
    let m = (u64::from(shown) * 1440 / u64::from(day)) as u32 + 360;
    let hour = (m % 1440) as f32 / 60.0;
    // Out from dusk, thickest through the night, gone by the dawn.
    let on = if hour >= 18.0 { ((hour - 18.0) / 2.0).min(1.0) } else if hour < 5.5 { 1.0 } else if hour < 6.5 { (6.5 - hour).max(0.0) } else { 0.0 };
    if on <= 0.0 || s.completed.is_some() {
        return;
    }
    let n = (18.0 * on) as u32;
    for i in 0..n {
        let u = |salt: u32| (hash(i as i32, salt as i32, 93) & 0xffff) as f32 / 65536.0;
        let t = tick as f32 / 30.0;
        let wx = u(1) * (town::COLS * town::TILE) as f32 + (t * (0.3 + u(2) * 0.4) + u(3) * TAU).sin() * 14.0;
        let wy = u(4) * (town::ROWS * town::TILE) as f32 + (t * (0.25 + u(5) * 0.4) + u(6) * TAU).cos() * 10.0;
        let (x, y) = (wx as i32 - cam.x, wy as i32 - cam.y + VIEW_Y);
        if !(0..im.w).contains(&x) || !(VIEW_Y + 4..VIEW_Y + VIEW_H - 2).contains(&y) {
            continue;
        }
        let blink = ((t * (1.2 + u(7)) + u(8) * TAU).sin() * 0.5 + 0.5).powi(2);
        if blink > 0.6 {
            im.put(x, y, '6');
            im.put(x - 1, y, '4');
            im.put(x + 1, y, '4');
            im.put(x, y - 1, '4');
            im.put(x, y + 1, '4');
        } else if blink > 0.25 {
            im.put(x, y, '4');
        }
    }
}

// ─── a thing found ───────────────────────────────────────────────────────────

/// A mask's face, 13 by 10, in the realm's inks.
pub(super) fn mask_icon(m: Mask) -> Img {
    let mut im = Img::new(13, 11);
    match m {
        Mask::Swift => {
            im.ellipse(6.5, 6.0, 4.4, 4.0, 'V');
            im.ellipse(5.6, 5.2, 3.0, 2.6, 'W');
            for (x, y) in [(2, 0), (1, 1), (0, 2), (1, 3), (2, 2), (10, 0), (11, 1), (12, 2), (11, 3), (10, 2), (3, 1), (9, 1)] {
                im.put(x, y, 'W');
            }
            im.rect(3, 5, 2, 2, 'k');
            im.rect(8, 5, 2, 2, 'k');
            im.rect(5, 9, 3, 1, 'u');
        }
        Mask::Stout => {
            im.ellipse(6.5, 6.0, 5.2, 4.6, 'R');
            im.ellipse(5.6, 5.2, 3.6, 3.0, 'O');
            im.rect(1, 3, 11, 2, 'p');
            im.rect(3, 6, 2, 2, 'k');
            im.rect(8, 6, 2, 2, 'k');
            for x in [2, 6, 10] {
                im.put(x, 4, 't');
            }
            im.rect(4, 9, 5, 1, 'B');
        }
        Mask::Bare => {}
    }
    im.outline_outside('k');
    im
}

/// A closed book, 11 by 9.
fn book_icon() -> Img {
    let mut im = Img::new(11, 9);
    im.rect(0, 0, 11, 8, 'B');
    im.rect(1, 1, 9, 6, 'R');
    im.rect(5, 0, 1, 8, 'p');
    im.rect(2, 2, 2, 1, 'T');
    im.rect(7, 2, 2, 1, 'T');
    im.rect(2, 4, 2, 1, 'o');
    im.rect(7, 4, 2, 1, 'o');
    im.put(5, 3, '6');
    im.rect(0, 7, 11, 1, 'H');
    im.outline_outside('k');
    im
}

/// Where the knight stands on the screen, feet.
fn feet(k: &Knight, cam: Cam) -> (i32, i32) {
    let (px, py) = k.px();
    (px - cam.x, py - cam.y + VIEW_Y)
}

/// What the viewer's knight finds, the bursts: a mask blooming in rings and
/// stars over them, a pickup's pop. (Drawn before the HUD.)
pub(super) fn found(im: &mut Img, s: &Session, me: &Knight, cam: Cam, tick: u32) {
    if !matches!(s.phase, Phase::Play) || me.down > 0 {
        return;
    }
    let (x, y) = feet(me, cam);
    let Some((note, left)) = me.note else { return };
    let age = NOTE_START.saturating_sub(left);
    match note {
        Note::Mask(m) if m != Mask::Bare && age < MASK_LIFE => {
            let ramp = if m == Mask::Swift { ramps::FROST } else { ramps::GOLD };
            let mut pen = Pen::new(im, 700);
            if age < 24 {
                pen.shock((x as f32, y as f32 - 12.0), 4.0 + age as f32 * 2.6, 1.8, 0.7, ramp, age as f32 / 24.0);
                pen.ring((x as f32, y as f32 - 12.0), 2.0 + age as f32 * 1.6, 1.4, 0.7, ramps::HOLY, age as f32 / 20.0);
            }
            Spray::round(11, 20, (x as f32, y as f32 - 14.0), (1.4, 3.6), (14.0, 28.0)).each(age as f32, |mo| mote(&mut pen, &mo, Shape::Star, ramp, 1, u64::from(tick)));
        }
        Note::Got(_) if age < 18 => {
            let mut pen = Pen::new(im, 300);
            pen.shock((x as f32, y as f32 - 14.0), 3.0 + age as f32 * 1.4, 1.4, 0.7, ramps::GOLD, age as f32 / 18.0);
            Spray::round(13, 10, (x as f32, y as f32 - 14.0), (1.0, 2.6), (10.0, 18.0)).each(age as f32, |mo| mote(&mut pen, &mo, Shape::Spark, ramps::GOLD, 1, u64::from(tick)));
        }
        _ => {}
    }
}

/// The flights: the mask's face rises over the helm, hangs, and flies to the
/// clock bar's mask word; a book flies from the knight to the Ledger's key.
/// Each lands in a twinkle on the bar. (Drawn after the HUD, before the cards.)
pub(super) fn found_over(im: &mut Img, s: &Session, me: &Knight, cam: Cam, tick: u32) {
    if !matches!(s.phase, Phase::Play) || me.down > 0 {
        return;
    }
    let (x, y) = feet(me, cam);
    let head = (x as f32, y as f32 - 26.0);
    let bar = (VIEW_Y + VIEW_H) as f32;
    if let Some((Note::Mask(m), left)) = me.note {
        let age = NOTE_START.saturating_sub(left);
        if m != Mask::Bare && age < MASK_LIFE {
            let icon = mask_icon(m);
            let word = 300.0 - text_width(if m == Mask::Swift { "SWIFT" } else { "STOUT" }) as f32 / 2.0;
            let rise = ease::out((age as f32 / 16.0).min(1.0));
            // Over the helm, then up and over to the bar's word.
            if age < 58 {
                let (px, py) = if age < 36 {
                    (head.0, head.1 - 8.0 * rise)
                } else {
                    motion::arc((head.0, head.1 - 8.0), (word, bar - 7.0), 26.0, ease::both(((age - 36) as f32 / 22.0).min(1.0)))
                };
                im.stamp(&icon, px as i32 - icon.w / 2, (py as i32 - icon.h / 2).min(VIEW_Y + VIEW_H - icon.h - 1));
            }
            if (56..70).contains(&age) {
                let mut pen = Pen::new(im, 100);
                pen.twinkle((word, bar + 5.0), 2.6, (age - 56) as f32 / 14.0, if m == Mask::Swift { ramps::FROST } else { ramps::GOLD });
            }
        }
    }
    // The Ledger writing: a book flies from the viewer's knight to its key.
    let key = (80.0f32, bar + 27.0);
    for &(note, left) in &s.news {
        if !matches!(note, Note::Wrote(..) | Note::Learned(_) | Note::Ticked(_)) {
            continue;
        }
        let age = NEWS_START.saturating_sub(left);
        if age >= 36 {
            continue;
        }
        let book = book_icon();
        let from = (head.0, head.1 - 2.0);
        let (px, py) = motion::arc(from, (key.0, bar - 8.0), 34.0, ease::both((age as f32 / 28.0).min(1.0)));
        if age < 28 {
            im.stamp(&book, px as i32 - book.w / 2, (py as i32 - book.h / 2).min(VIEW_Y + VIEW_H - book.h - 1));
        }
        let mut pen = Pen::new(im, 200);
        if age < 14 {
            pen.twinkle((from.0 + 8.0, from.1 - 4.0), 2.4, age as f32 / 14.0, ramps::GOLD);
            pen.twinkle((from.0 - 9.0, from.1 - 1.0), 1.6, (age as f32 - 3.0) / 11.0, ramps::HOLY);
        }
        if (27..36).contains(&age) {
            pen.twinkle(key, 2.2, (age - 27) as f32 / 9.0, ramps::GOLD);
        }
        let _ = tick;
        break;
    }
}

// ─── the lights hold ─────────────────────────────────────────────────────────

/// The moon turned back: three shock rings, fireworks over the town, paper.
/// Then, once the knights are free, the party goes on at a gentler pace.
pub(super) fn finale(im: &mut Img, s: &Session, tick: u32) {
    let (age, strong) = match s.phase {
        Phase::Won { t } => (t, true),
        _ => match s.completed {
            Some(done) if matches!(s.phase, Phase::Play) => (s.elapsed.saturating_sub(done) + 120, false),
            _ => return,
        },
    };
    let ramps_cycle = [ramps::GOLD, ramps::HOLY, ramps::GENIE, ramps::ROSE];
    if strong {
        let mut pen = Pen::new(im, 700);
        for (i, at) in [(96.0f32, 150.0f32), (192.0, 140.0), (288.0, 150.0)].into_iter().enumerate() {
            let a = age as f32 - i as f32 * 5.0;
            if (0.0..26.0).contains(&a) {
                pen.shock(at, 4.0 + a * 5.0, 2.4, 0.62, ramps::GOLD, a / 26.0);
            }
        }
    }
    // Fireworks: every so often a new one rises from low in the town.
    let step = if strong { 8 } else { 40 };
    let cur = age / step;
    for n in cur.saturating_sub(8)..=cur {
        let born = n * step;
        let a = age - born;
        if a >= 66 || (strong && born < 10) {
            continue;
        }
        let h = hash(n as i32, 7, 95);
        let x = 54.0 + (h % 276) as f32;
        let top = 52.0 + ((h >> 9) % 60) as f32;
        juice::firework(im, x, (VIEW_Y + VIEW_H - 14) as f32, top, a, h, ramps_cycle[(n % 4) as usize], u64::from(tick));
    }
    if age >= 18 {
        juice::confetti(im, (0, VIEW_Y, 384, VIEW_H), age + tick % 2, 5, if strong { 54 } else { 22 });
    }
}
