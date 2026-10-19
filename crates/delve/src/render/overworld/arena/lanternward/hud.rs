//! What a knight reads on their own screen: hearts, gems, keys, flame, the
//! map in the corner, the button that would do something, one line from the
//! wick-wisp, and the few banners the adventure has (an item held up, the
//! Warden's bar, the retry, the Beacon).
use super::super::super::ink::{Img, text_width};
use super::super::knights;
use super::bank::slot_mid;
use super::scene::View;
use crate::rules::lanternward::{
    self as lw, Act, Ctx, Fk, Hold, Item, Pk, Seat, Session, Ws,
    world::Q,
};

fn label(im: &mut Img, x: i32, y: i32, s: &str, ink: char) {
    im.rect(x - 1, y - 1, text_width(s) + 2, 9, 'k');
    im.text(x, y, s, ink);
}

fn centred(im: &mut Img, cx: i32, y: i32, s: &str, ink: char) {
    label(im, cx - text_width(s) / 2, y, s, ink);
}

/// A heart with `q` quarters filled.
fn heart(im: &mut Img, x: i32, y: i32, q: u32, pulse: bool) {
    const SHAPE: [&str; 6] = [".XX.XX.", "XXXXXXX", "XXXXXXX", ".XXXXX.", "..XXX..", "...X..."];
    let full = if pulse { 'w' } else { '7' };
    for (r, row) in SHAPE.iter().enumerate() {
        for (c, b) in row.bytes().enumerate() {
            if b != b'X' {
                continue;
            }
            // Fill from the bottom up, a quarter of the rows each.
            let from_bottom = 5 - r as u32;
            let on = from_bottom * 4 < q * 6;
            im.put(x + c as i32, y + r as i32, if on { full } else { 'X' });
        }
    }
    im.rect(x, y + 6, 0, 0, 'k');
}

fn gem(im: &mut Img, x: i32, y: i32) {
    im.rect(x + 1, y, 3, 1, '6');
    im.rect(x, y + 1, 5, 2, '4');
    im.rect(x + 1, y + 3, 3, 1, '4');
    im.put(x + 2, y + 4, '@');
    im.put(x + 1, y + 1, 'w');
}

fn small_key(im: &mut Img, x: i32, y: i32, ink: char) {
    im.rect(x, y, 3, 3, ink);
    im.put(x + 1, y + 1, 'k');
    im.rect(x + 3, y + 1, 4, 1, ink);
    im.put(x + 5, y + 2, ink);
}

/// The mini map: five rooms in a row, this one lit, the done ones filled.
fn minimap(im: &mut Img, s: &Session, x: i32, y: i32) {
    for r in 0..5 {
        let bx = x + r * 9;
        let here = usize::from(s.room) == r as usize;
        let done = (r as u8) < s.room;
        im.rect(bx, y, 7, 5, 'k');
        im.rect(bx + 1, y + 1, 5, 3, if here { '6' } else if done { 'u' } else { 'X' });
        if r == 4 {
            im.put(bx + 3, y + 2, if s.done.is_some() { 'w' } else { '7' });
        }
    }
}

pub(super) fn draw(im: &mut Img, v: &View) {
    let s = v.s;
    let Some(k) = s.seats.get(&v.id) else { return };
    top_bar(im, v, k);
    wisp(im, v, k);
    bottom(im, v, k);
    banners(im, v, k);
    boss_bar(im, v);
    leave_bar(im, v);
}

/// The top of the screen, in the first 32 pixels (the terminal's text rows
/// cover exactly these): hearts, then gems, keys and the lantern's flame; the
/// table's other knights in the middle; the map in the corner.
fn top_bar(im: &mut Img, v: &View, k: &Seat) {
    let s = v.s;
    // Hearts.
    let hearts = k.max_hp.div_ceil(lw::HEART);
    im.rect(2, 2, (hearts.min(10) as i32) * 9 + 3, 12, 'k');
    for h in 0..hearts.min(10) {
        let col = h as i32;
        let q = k.hp.saturating_sub(h * lw::HEART).min(lw::HEART);
        let low = k.hp <= lw::LOW_HP && k.hp > 0 && (v.tick / 8).is_multiple_of(2) && q > 0;
        heart(im, 4 + col * 9, 4, q, low);
    }
    // Gems, bombs, keys, then the flame (and the ember's taper under it).
    let y = 16;
    im.rect(2, y - 1, 118, 9, 'k');
    gem(im, 4, y);
    im.text(12, y, &format!("{}", k.gems), 'T');
    let mut x = 12 + text_width(&format!("{}", k.gems)) + 6;
    if k.bombs > 0 {
        im.ellipse((x + 2) as f32, (y + 3) as f32, 3.0, 3.0, 'g');
        im.put(x + 3, y, '6');
        im.text(x + 8, y, &format!("{}", k.bombs), 'T');
        x += 8 + text_width(&format!("{}", k.bombs)) + 5;
    }
    for _ in 0..s.inv.small_keys {
        small_key(im, x, y + 1, '6');
        x += 8;
    }
    if let Hold::Carried(c, ..) = s.inv.boss
        && u32::from(c) == v.id
    {
        small_key(im, x, y + 1, '7');
        x += 8;
    }
    if s.inv.lantern {
        let r = lw::Rules::of(&v.run.tune);
        let fx = x.max(62) + 2;
        im.rect(fx, y, 4, 7, 'g');
        im.rect(fx + 1, y + 1, 2, 4, if k.flaring() { 'w' } else { '6' });
        let w = (k.flame * 30 / r.flame.max(1)).min(30) as i32;
        im.rect(fx + 6, y + 2, 32, 3, 'X');
        im.rect(fx + 6, y + 2, w, 3, if k.flame < r.flame / 5 { '7' } else { '4' });
    }
    if k.taper > 0 {
        let w = (k.taper * 24 / lw::TAPER).min(24) as i32;
        im.rect(2, y + 9, 30, 6, 'k');
        im.rect(4, y + 11, w, 2, 'O');
    }
    // The table's other knights, in a row along the top.
    let mut rx = 132;
    for (&id, o) in &s.seats {
        if id == v.id {
            continue;
        }
        let ink = knights::seat_cloth(id);
        im.rect(rx - 1, 2, 30, 9, 'k');
        im.text(rx, 3, &format!("{id}"), ink);
        let w = (o.hp * 18 / o.max_hp.max(1)).min(18) as i32;
        im.rect(rx + 8, 5, 18, 3, 'X');
        im.rect(rx + 8, 5, w, 3, if o.act == Act::Down { 'g' } else { ink });
        if o.act == Act::Down {
            im.text(rx + 8, 3, "!", '7');
        }
        rx += 32;
    }
    // The map in the corner.
    if s.inv.map {
        im.rect(331, 2, 51, 9, 'k');
        minimap(im, s, 333, 4);
    }
}

/// The wick-wisp: a small light that floats by the knight and tells them
/// the one thing to try.
fn wisp(im: &mut Img, v: &View, k: &Seat) {
    let (x, y) = (v.x(k.x) + 16, v.y(k.y) - 26);
    let bob = [0, -1, -2, -1][(v.tick / 4 % 4) as usize];
    slot_mid(im, "wisp", "idle", v.tick, x.clamp(10, 372), y.clamp(30, 200) + bob);
}

fn bottom(im: &mut Img, v: &View, k: &Seat) {
    let s = v.s;
    // What E would do.
    let text = lw::hint::context(k);
    let ink = if k.ctx == Ctx::None { 'u' } else { '6' };
    if k.act == Act::Free || k.act == Act::Item {
        label(im, 380 - text_width(&text), 215, &text, ink);
    }
    // A sign or a kingsman being read, else a locked switch's hint, else
    // the wisp's line.
    if k.read.is_some() {
        if let Some(words) = lw::hint::line(s, k, &lw::Rules::of(&v.run.tune)) {
            centred(im, 192, 205, &words, 'H');
        }
    } else if let Some(h) = lw::hint::lock_hint(s, k) {
        centred(im, 192, 205, h, 'T');
    } else if let Some(h) = lw::hint::hint(s, k, &lw::Rules::of(&v.run.tune)) {
        label(im, 4, 215, h, '6');
    }
}

fn banners(im: &mut Img, v: &View, k: &Seat) {
    let s = v.s;
    if let Some(b) = s.banner {
        let name = b.item.name();
        let how = b.item.how();
        let w = text_width(name).max(text_width(how)) + 12;
        let x = 192 - w / 2;
        im.rect(x, 60, w, 30, 'k');
        im.frame(x, 60, w, 30, '6');
        centred(im, 192, 66, name, '6');
        centred(im, 192, 78, how, 'T');
        if let Some(o) = s.seats.get(&u32::from(b.seat))
            && u32::from(b.seat) != v.id
        {
            let _ = o;
            centred(im, 192, 94, &format!("SEAT {} FOUND IT", b.seat), knights::seat_cloth(u32::from(b.seat)));
        }
    }
    // The room's name, the first moments in it.
    if s.stage_t < 70 && s.exiting == 0 && s.done.is_none() {
        let name = lw::world::plan(usize::from(s.room)).name;
        let fade = s.stage_t < 50;
        if fade {
            centred(im, 192, 40, name, 'T');
        }
    }
    if s.elapsed < 90 && s.room == 0 {
        centred(im, 192, 110, "LOCK ON. STRIKE. BLOCK.", 'H');
    }
    if s.wipe > 0 {
        centred(im, 192, 100, "TRY AGAIN", '7');
    }
    if s.exiting > 0 {
        let cover = (s.exiting.min(lw::EXIT_TICKS) as i32 * 224 / lw::EXIT_TICKS as i32).min(224);
        let _ = cover;
        centred(im, 192, 100, "ONWARD", 'T');
    }
    if let Some(t) = s.done {
        let since = s.elapsed.saturating_sub(t);
        if since > 20 {
            centred(im, 192, 70, "THE BEACON BURNS", '6');
            let first = v.run.home.roundtable[lw::SLOT] & lw::BEGUN == 0;
            let pay = lw::purse(first, k.gems);
            centred(im, 192, 84, &format!("PURSE {pay}G   TIME {}S", t / crate::rules::HZ), 'T');
            if s.finale() {
                centred(im, 192, 98, "E LEAVE", '6');
            }
        }
    }
    let _ = (Item::Map, Fk::Snuffer, Pk::Pot, Q);
}

fn boss_bar(im: &mut Img, v: &View) {
    let Some(w) = &v.s.boss else { return };
    if w.st == Ws::Asleep || w.hp == 0 {
        return;
    }
    let (x, y, width) = (126, 197, 200);
    im.rect(x - 2, y - 11, width + 4, 18, 'k');
    im.text(x, y - 9, "THE WARDEN", 'H');
    let open = v.s.boss_open();
    if open {
        im.text(x + width - text_width("HEART OPEN"), y - 9, "HEART OPEN", '6');
    } else {
        let need = w.need(v.s.seats.len() as u32);
        let line = if need >= 2 { "NEEDS TWO LIGHTS" } else { "HEART SHUT" };
        im.text(x + width - text_width(line), y - 9, line, 'u');
    }
    im.rect(x, y, width, 4, 'X');
    im.rect(x, y, (u64::from(w.hp) * width as u64 / u64::from(w.full.max(1))) as i32, 4, if open { '6' } else { 'R' });
    // The thirds.
    for t in [1, 2] {
        im.rect(x + width * t / 3, y - 1, 1, 6, 'k');
    }
}

fn leave_bar(im: &mut Img, v: &View) {
    let p = v.run.leave_progress(v.id);
    if p > 0 {
        let w = (p * 60 / crate::rules::exits::LEAVE_TICKS) as i32;
        im.rect(160, 170, 64, 16, 'k');
        im.text(164, 173, "LEAVING", 'H');
        im.rect(162, 182, 60, 2, 'X');
        im.rect(162, 182, w, 2, 'w');
    }
}
