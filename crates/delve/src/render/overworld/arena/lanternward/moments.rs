//! The beats of an adventure: the title card, a room's name, the Warden waking,
//! its heart opening, its stagger and its fall, the retry, the party leaving a
//! room, and the Beacon's finale with its fireworks, its paper, and the
//! purse each knight takes home. Every one is a pure function of the
//! rules' own counters; one card owns the middle of the screen at a time.
use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Tone};
use super::super::knights;
use super::super::pfx::ramps;
use super::hud::{claim, plate_text};
use super::scene::{View, knight_img};
use crate::rules::lanternward::{self as lw, Act, Ws};

/// Ticks the title card holds.
const TITLE: u32 = 110;
/// Ticks a room's name card holds.
const NAME: u32 = 54;
/// Ticks a call over the Warden holds.
const CALL: u32 = 44;
/// The Warden's stagger as the rules count it down.
const STAGGER: u32 = 75;

/// The rows the title's card sits on (its top at rest); the calls of the
/// moment sit under the top bar, out of the way of the fight.
const TITLE_Y: i32 = 112;
const CALL_Y: i32 = 36;
/// Or, when the Warden is up there, low on the screen above its bar.
const LOW_Y: i32 = 146;

/// A card on the middle of the screen: what it says, how it is inked, how old it is.
struct Card {
    title: String,
    sub: String,
    age: u32,
    life: u32,
    tone: Tone,
    /// The biggest scale its words may rest at.
    cap: i32,
    /// The row its top rests on.
    y: i32,
}

/// Where a call of the moment stands: the band under the top bar or the one
/// over the Warden's bar, whichever the Warden and the knight are not in.
fn call_y(v: &View) -> i32 {
    let s = v.s;
    // Once the Beacon burns the foot of the screen is the purse's.
    if s.done.is_some() {
        return CALL_Y;
    }
    let band = |y: i32| (y - 12, y + 48);
    let in_band = |sy: i32, y: i32| (band(y).0..band(y).1).contains(&sy);
    let mut crowd = [0, 0];
    for (i, y) in [CALL_Y, LOW_Y].into_iter().enumerate() {
        if let Some(w) = &s.boss
            && in_band(v.y(w.y), y)
        {
            crowd[i] += 5;
        }
        if let Some(k) = s.seats.get(&v.id)
            && in_band(v.y(k.y), y)
        {
            crowd[i] += 3;
        }
    }
    if crowd[1] < crowd[0] { LOW_Y } else { CALL_Y }
}

fn warden_card(v: &View) -> Option<Card> {
    let s = v.s;
    let w = s.boss.as_ref()?;
    let y = call_y(v);
    let seats = s.seats.len() as u32;
    if w.hp == 0 {
        let age = u32::from(w.dead).saturating_sub(24);
        return (w.dead >= 24 && age < 70).then(|| Card { title: "THE WARDEN FALLS".into(), sub: String::new(), age, life: 70, tone: Tone::Warm, cap: 2, y });
    }
    if super::scene::woke(w) {
        let age = 70u32.saturating_sub(u32::from(w.t));
        return Some(Card { title: "THE WARDEN".into(), sub: "LIGHT ITS HEART".into(), age, life: 56, tone: Tone::Hot, cap: 2, y });
    }
    if w.st == Ws::Stagger {
        let age = STAGGER.saturating_sub(u32::from(w.t));
        return (age < CALL).then(|| Card { title: "PARRIED".into(), sub: "STRIKE NOW".into(), age, life: CALL, tone: Tone::Warm, cap: 2, y });
    }
    if s.boss_open() {
        // The tick the shutter opened: the one that made the count.
        let mut lit: Vec<u32> = w.lit.iter().copied().filter(|&t| t > 0 && t + lw::EXPOSE_TICKS > s.elapsed).collect();
        lit.sort_unstable_by(|a, b| b.cmp(a));
        let opened = lit.get(w.need(seats) as usize - 1).copied().unwrap_or(0);
        let age = s.elapsed.saturating_sub(opened);
        return (age < CALL).then(|| Card { title: "HEART OPEN".into(), sub: String::new(), age, life: CALL, tone: Tone::Warm, cap: 2, y });
    }
    None
}

/// The one card on show, by rank: the retry, an item held up, the Beacon,
/// the Warden's moments, then the title and the room's name.
fn card(v: &View) -> Option<Card> {
    let s = v.s;
    if s.wipe > 0 {
        return Some(Card { title: "TRY AGAIN".into(), sub: String::new(), age: lw::WIPE_TICKS.saturating_sub(s.wipe), life: lw::WIPE_TICKS + 8, tone: Tone::Hot, cap: 3, y: 80 });
    }
    if let Some(b) = s.banner {
        return Some(Card {
            title: b.item.name().into(),
            sub: b.item.how().into(),
            age: lw::ITEM_TICKS.saturating_sub(u32::from(b.t)),
            life: lw::ITEM_TICKS,
            tone: Tone::Warm,
            cap: 2,
            y: call_y(v),
        });
    }
    if let Some(t) = s.done {
        let age = s.elapsed.saturating_sub(t);
        return (age < 96).then(|| Card { title: "BEACON LIT".into(), sub: String::new(), age, life: 96, tone: Tone::Warm, cap: 3, y: 56 });
    }
    if s.room == 4
        && let Some(c) = warden_card(v)
    {
        return Some(c);
    }
    if s.exiting == 0 && s.room == 0 && s.elapsed < TITLE {
        return Some(Card { title: "LANTERNWARD".into(), sub: "LOCK ON. STRIKE. BLOCK.".into(), age: s.elapsed, life: TITLE, tone: Tone::Warm, cap: 3, y: TITLE_Y });
    }
    if s.exiting == 0 && s.stage_t < NAME && s.room > 0 && s.done.is_none() {
        let name = lw::world::plan(usize::from(s.room)).name;
        return Some(Card { title: name.into(), sub: String::new(), age: s.stage_t, life: NAME, tone: Tone::Steel, cap: 2, y: call_y(v) });
    }
    None
}

/// The box of the card on show, if one is (at rest: its first ticks are larger).
pub(super) fn card_box(v: &View) -> Option<(i32, i32, i32, i32)> {
    card(v).map(|c| card_rect(&c.title, &c.sub, c.age, c.cap, c.y))
}

/// The box a card's words and rails mark at rest (the slam of its first ticks
/// is larger; the tests look at settled cards).
pub(super) fn card_rect(title: &str, sub: &str, age: u32, cap: i32, y: i32) -> (i32, i32, i32, i32) {
    let tw = text_width(title).max(1);
    let rest = ((384 - 24) / tw).clamp(1, cap);
    let half = (tw * rest + 4) / 2 + 8;
    let (mut x0, mut x1) = (192 - half, 192 + half);
    let mut y1 = y + 7 * rest + 11;
    if !sub.is_empty() && age >= 4 {
        let sw = text_width(sub) + 4;
        x0 = x0.min(192 - sw / 2);
        x1 = x1.max(192 + sw / 2);
        y1 = y1.max(y + 7 * rest + 12 + 8);
    }
    (x0, y - 5, x1 - x0, y1 - (y - 5))
}

pub(super) fn draw(im: &mut Img, v: &View) {
    let s = v.s;
    let Some(k) = s.seats.get(&v.id) else { return };
    rim(im, v, k);
    if let Some(c) = card(v) {
        juice::banner(im, c.y, &c.title, &c.sub, c.age, c.life, c.tone, c.cap);
        if c.age >= 3 && c.age + 14 < c.life {
            claim("card", card_rect(&c.title, &c.sub, c.age, c.cap, c.y));
        }
        // A friend found it: their number, under the card.
        if let Some(b) = s.banner
            && u32::from(b.seat) != v.id
            && s.wipe == 0
        {
            plate_text(im, 192, if c.y < 100 { c.y + 40 } else { c.y - 16 }, &format!("SEAT {} FOUND IT", b.seat), knights::seat_cloth(u32::from(b.seat)));
        }
    }
    // The party goes through a door: the room closes in a wipe, the next opens.
    if s.exiting > 0 {
        let k = s.exiting.min(lw::EXIT_TICKS) as f32 / lw::EXIT_TICKS as f32;
        juice::iris(im, (192, 112), k, 'k');
    } else if s.stage_t < 12 && s.room > 0 && s.done.is_none() {
        juice::iris(im, (192, 112), 1.0 - s.stage_t as f32 / 12.0, 'k');
    }
    if let Some(t) = s.done {
        finale(im, v, k, t);
    }
}

/// The edge of the screen: a red rim that thumps while a knight is at two
/// hearts or less, and flares for a few ticks after a hit.
fn rim(im: &mut Img, v: &View, k: &lw::Seat) {
    let hurt = lw::HURT_INV.saturating_sub(k.ward);
    let flare = if k.ward > 0 && hurt < 9 { 1.0 - hurt as f32 / 9.0 } else { 0.0 };
    let low = if k.hp > 0 && k.hp <= lw::LOW_HP && k.act != Act::Down {
        0.28 + [0.0, 0.08, 0.14, 0.08][(v.tick / 6 % 4) as usize]
    } else {
        0.0
    };
    let k = flare.max(low);
    if k > 0.0 {
        juice::vignette(im, (0, 0, 384, 224), k, 22, '7');
    }
}

// ----- the Beacon ------------------------------------------------------------------

/// The party's score for one run: the more seconds past three minutes, the less.
pub(super) fn rating(secs: u32) -> u32 {
    1000u32.saturating_sub(secs.saturating_sub(180) * 2).clamp(1, 1000)
}

fn finale(im: &mut Img, v: &View, me: &lw::Seat, done: u32) {
    let s = v.s;
    let since = s.elapsed.saturating_sub(done);
    let tick = u64::from(v.tick);
    // Fireworks over the keep, a volley then a slower round.
    for i in 0..12u32 {
        let start = i * 8 + 4;
        if since < start {
            continue;
        }
        let x = 30.0 + ((i * 97 + 31) % 330) as f32;
        let top = 34.0 + ((i * 41) % 60) as f32;
        let ramp = [ramps::GOLD, ramps::HOLY, ramps::FIRE, ramps::FROST][i as usize % 4];
        juice::firework(im, x, 176.0, top, since - start, 17 + i, ramp, tick);
    }
    if since > 14 {
        juice::confetti(im, (0, 20, 384, 190), v.tick, 5, 70);
    }
    if since < 36 {
        return;
    }
    // The knights who lit it, in a row, crowned, each with their own purse.
    let rows: Vec<(u32, &lw::Seat)> = s.seats.iter().map(|(&id, k)| (id, k)).collect();
    let n = rows.len().max(1) as i32;
    let step = (44).min(340 / n);
    let x0 = 192 - step * (n - 1) / 2;
    let first = v.run.home.roundtable[lw::SLOT] & lw::BEGUN == 0;
    let rise = (since - 36).min(10) as i32;
    for (i, (id, k)) in rows.iter().enumerate() {
        let cx = x0 + step * i as i32;
        let fig = knight_img(v, *id, k);
        let mut big = Img::new(fig.w * 2, fig.h * 2);
        big.stamp_times(&fig, 0, 0, 2);
        let foot = 148 + 10 - rise;
        // The crown the realm gives: a small gold band over the helm.
        let top = foot - big.h;
        im.rect(cx - 6, foot + 1, 12, 2, 'K');
        im.stamp(&big, cx - big.w / 2, top);
        im.rect(cx - 4, top - 3, 8, 3, '6');
        for dx in [-4, -1, 2] {
            im.rect(cx + dx, top - 5, 2, 2, '6');
        }
        im.put(cx, top - 2, 'w');
        let pay = lw::purse(first, k.gems);
        let ink = knights::seat_cloth(*id);
        let tag = if *id == v.id { "YOU".to_string() } else { format!("{id}") };
        plate_text(im, cx, 154, &tag, ink);
        plate_text(im, cx, 164, &format!("+{pay}G"), '6');
    }
    let secs = done / crate::rules::HZ;
    let score = rating(secs);
    let best = u32::from(v.run.home.countryside.games.roundtable[lw::SLOT].best);
    let line = format!("TIME {}:{:02}", secs / 60, secs % 60);
    plate_text(im, 192 - 40, 180, &line, 'T');
    let best_line = if score >= best { "NEW BEST".to_string() } else { format!("BEST {best}") };
    plate_text(im, 192 + 40, 180, &best_line, if score >= best { '6' } else { 'h' });
    if s.finale() {
        plate_text(im, 192, 194, "E LEAVE", '6');
    }
    let _ = me;
}
