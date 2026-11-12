//! The store's marks along the top of a seat's view: the keg and the
//! seconds, a chip for each knight (colour, name, crates; the leader
//! crowned), the pot; under them the table's bar, a store that empties as
//! the whole table breaks crates, with the marks the table calls at; and,
//! where the hall is bigger than a screen, the whole store in small, so
//! every knight can see who is hunting whom. The view's own HUD never
//! shakes.

use super::super::super::chrome;
use super::super::super::ink::{Img, text_width};
use super::super::{NATIVE_H, NATIVE_W, juice, knights};
use super::words::Ledger;
use super::bits;
use crate::rules::powder::Powder;
use crate::rules::{Run, Tile, TILE_UNITS};

/// How far down the view the top marks reach.
pub(super) const BOTTOM: i32 = 15;

const LEFT: i32 = 42;

/// One knight's place in the strip.
pub(super) struct Chip {
    pub id: u32,
    pub x: i32,
    /// Where the chip ends (the next begins).
    pub w: i32,
    /// Letters of the name that fit.
    pub name: usize,
}

/// The pot's tag.
fn pot_tag(p: &Powder) -> String {
    format!("{}G", p.ante.values().sum::<u32>())
}

/// Where the pot's coin sits.
fn pot_x(p: &Powder) -> i32 {
    NATIVE_W - 9 - text_width(&pot_tag(p)) - 8
}

/// The knights' chips, left to right in seat order, fitted to the strip.
pub(super) fn chips(p: &Powder) -> Vec<Chip> {
    let n = p.broken.len().max(1) as i32;
    let room = pot_x(p) - 6 - LEFT;
    let each = room / n;
    // Marker, gap, name, gap, up to three digits.
    let name = ((each - 7 - 3 - text_width("000") - 2) / 6).clamp(0, 8) as usize;
    p.broken.keys().enumerate().map(|(i, &id)| Chip { id, x: LEFT + each * i as i32, w: each, name }).collect()
}

/// The name a seat goes by here: `YOU` for the seat looking.
pub(super) fn called(run: &Run, id: u32, focus: Option<u32>, most: usize) -> String {
    if focus == Some(id) && most >= 3 {
        return "YOU".to_string();
    }
    run.players
        .get(&id)
        .map_or_else(|| format!("SEAT {id}"), |h| h.name.to_uppercase())
        .chars()
        .take(most)
        .collect()
}

/// Where a knight's count sits in the strip, for what flies to it.
pub(super) fn anchor(p: &Powder, id: u32) -> (i32, i32) {
    chips(p).iter().find(|c| c.id == id).map_or((NATIVE_W / 2, 8), |c| (c.x + c.w - 8, 8))
}

/// The share of the store the table has broken, 0 to 100.
pub(super) fn cleared(p: &Powder) -> u32 {
    let total = u32::from(p.show.team) + p.crates.len() as u32;
    if total == 0 { 0 } else { u32::from(p.show.team) * 100 / total }
}

/// The strip, the store's bar and the clock's last seconds.
pub(super) fn strip(led: &mut Ledger, over: &mut Img, run: &Run, p: &Powder, focus: Option<u32>) {
    let now = run.tick as u32;
    chrome::panel(over, (4, 1, NATIVE_W - 8, 11), chrome::Tone::SLATE);
    let landed = |id: u32| -> Option<u32> {
        p.show
            .blasts
            .iter()
            .filter(|b| u32::from(b.who) == id && !b.crates.is_empty())
            .filter_map(|b| b.age(now).checked_sub(super::moments::FLY))
            .min()
    };
    over.stamp(bits::strip_keg(), 8, 2);
    let left = if p.done { 0 } else { p.left() };
    let low = left <= 10 && !p.done && !p.titled();
    let secs = if p.done { "DONE".to_string() } else { format!("{left}S") };
    let blink = low && now / 6 % 2 == 0;
    led.small(over, "clock", 19, 3, &secs, if blink { '7' } else if low { 'R' } else { 'O' });
    let top = p.broken.values().copied().max().unwrap_or(0);
    let rivals = p.broken.len() > 1;
    for chip in chips(p) {
        let n = p.broken.get(&chip.id).copied().unwrap_or(0);
        let ink = knights::seat_mark(chip.id);
        let you = focus == Some(chip.id);
        let lead = rivals && n == top && n > 0;
        over.rect(chip.x, 4, 5, 5, 'k');
        over.rect(chip.x + 1, 5, 3, 3, ink);
        if you {
            over.frame(chip.x - 1, 3, 7, 7, 'T');
        }
        if lead {
            over.stamp(bits::crown(), chip.x - 1, 0);
        }
        if chip.name > 0 {
            let name = called(run, chip.id, focus, chip.name);
            led.small(over, "name", chip.x + 8, 3, &name, if you { 'T' } else { 'i' });
        }
        // A crate flying in lands, and the count swells.
        let pulse = landed(chip.id).map_or(0.0, |age| juice::bump(age, 8));
        let num = n.to_string();
        let ink = if pulse > 0.0 { '9' } else if lead { 'T' } else { 'h' };
        led.small(over, "count", chip.x + chip.w - 3 - text_width(&num), 3 - i32::from(pulse > 0.5), &num, ink);
        // The run of crates, a fuse under the chip burning down.
        let run_len = p.show.run_of(chip.id, now);
        if run_len >= 2 {
            let last = p.show.combo.get(&chip.id).map_or(0, |c| c.0);
            let left = crate::rules::powder_show::CHAIN_GAP.saturating_sub(now.saturating_sub(last));
            let w = (chip.w - 6) * left as i32 / crate::rules::powder_show::CHAIN_GAP as i32;
            over.rect(chip.x, 10, w.max(0), 1, if left < 20 && now / 3 % 2 == 0 { '7' } else { 'O' });
        }
    }
    let tag = pot_tag(p);
    if p.ante.values().sum::<u32>() > 0 {
        over.stamp(bits::coin(), pot_x(p), 3);
        led.small(over, "pot", pot_x(p) + 7, 3, &tag, 'O');
    }
    store_bar(led, over, p, now);
}

/// The kegs a seat has to set, bottom left: a lit keg for each in hand,
/// a dark one, its fuse a short bar, for each burning. No words.
pub(super) fn hand(over: &mut Img, p: &Powder, focus: Option<u32>, now: u32) {
    let Some(me) = focus.filter(|id| p.broken.contains_key(id)) else { return };
    if p.done || p.titled() {
        return;
    }
    let burning: Vec<u32> = p.kegs.iter().filter(|k| k.owner == me).map(|k| k.fuse).collect();
    for i in 0..crate::rules::powder::KEGS_EACH {
        let x = 8 + i as i32 * 12;
        let y = NATIVE_H - 14;
        over.rect(x - 1, y - 1, 10, 10, 'k');
        over.frame(x - 1, y - 1, 10, 10, 's');
        match burning.get(i) {
            None => over.stamp(bits::strip_keg(), x, y),
            Some(&fuse) => {
                over.rect(x, y + 6, 8, 2, 'g');
                let w = (fuse as i32 * 8 / crate::rules::powder::FUSE as i32).clamp(0, 8);
                over.rect(x, y + 6, w, 2, if fuse < 20 && now / 3 % 2 == 0 { '9' } else { '7' });
                over.rect(x + 2, y + 1, 4, 3, 'g');
            }
        }
    }
}

/// The table's bar: a fuse along the strip's foot that burns the store
/// away, a gap at each mark the table calls at (a quarter, half, most);
/// brightest where a crate has just broken.
fn store_bar(_led: &mut Ledger, over: &mut Img, p: &Powder, now: u32) {
    let (x0, w, y0) = (8, NATIVE_W - 16, 12);
    let pct = cleared(p);
    over.rect(x0, y0, w, 3, 'k');
    let fill = w * pct as i32 / 100;
    let fresh = p.show.blasts.iter().any(|b| !b.crates.is_empty() && b.age(now) < 8);
    for dx in 0..fill {
        let ink = if fresh && (dx + now as i32) % 9 < 2 { '9' } else if pct >= 80 { '@' } else { 'o' };
        over.rect(x0 + dx, y0, 1, 2, ink);
        over.put(x0 + dx, y0, if ink == 'o' { 't' } else { '9' });
    }
    over.rect(x0 + fill, y0, 1, 2, if fill > 0 { '9' } else { 's' });
    for mark in [25, 50, 80] {
        over.put(x0 + w * mark / 100, y0 + 2, 'h');
    }
}

/// The whole store in small, two pixels a tile, bottom right: stone
/// floor, piers, crates, lit kegs, flames, the knights in their colours;
/// the view's own frame. Only for a hall bigger than a screen.
pub(super) fn minimap(over: &mut Img, run: &Run, p: &Powder, focus: Option<u32>, (ox, oy): (i32, i32)) {
    let room = run.room();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    if cols * super::super::TILE <= NATIVE_W && rows * super::super::TILE <= NATIVE_H {
        return;
    }
    let (w, h) = (cols * 2, rows * 2);
    let (x0, y0) = (NATIVE_W - w - 8, NATIVE_H - h - 8);
    over.rect(x0 - 3, y0 - 3, w + 6, h + 6, 'k');
    over.frame(x0 - 3, y0 - 3, w + 6, h + 6, 's');
    let now = run.tick as u32;
    for r in 0..rows {
        for c in 0..cols {
            let ink = match room.tile(c, r) {
                Tile::Floor | Tile::Door | Tile::Stairs => 'x',
                Tile::Block if crate::rules::powder::pillar(c, r) => 'u',
                Tile::Block => 'o',
                _ => continue,
            };
            over.rect(x0 + c * 2, y0 + r * 2, 2, 2, ink);
        }
    }
    for b in p.show.blasts.iter().filter(|b| b.age(now) < 14) {
        for (tx, ty) in b.tiles() {
            over.rect(x0 + tx * 2, y0 + ty * 2, 2, 2, if b.age(now) < 4 { '9' } else { '@' });
        }
    }
    for k in &p.kegs {
        over.rect(x0 + k.at.0 * 2 - 1, y0 + k.at.1 * 2 - 1, 4, 4, if now / 4 % 2 == 0 { '9' } else { '7' });
    }
    // The view's own frame, so a knight sees where they are.
    let (vx, vy) = (x0 + ox.clamp(0, cols * super::super::TILE) / 8, y0 + oy.clamp(0, rows * super::super::TILE) / 8);
    let (vw, vh) = ((NATIVE_W / 8).min(x0 + w - vx), (NATIVE_H / 8).min(y0 + h - vy));
    for dx in (0..vw).step_by(2) {
        over.put(vx + dx, vy, 'h');
        over.put(vx + dx, vy + vh - 1, 'h');
    }
    for dy in (0..vh).step_by(2) {
        over.put(vx, vy + dy, 'h');
        over.put(vx + vw - 1, vy + dy, 'h');
    }
    for (&id, hero) in run.players.iter().filter(|(id, h)| h.hp > 0 && !h.stone && !h.absent && p.broken.contains_key(id)) {
        let px = x0 + (hero.x / TILE_UNITS * 2.0) as i32 - 1;
        let py = y0 + (hero.y / TILE_UNITS * 2.0) as i32 - 1;
        if focus == Some(id) && now / 5 % 2 == 0 {
            over.rect(px - 2, py - 2, 7, 7, 'k');
        }
        over.rect(px - 1, py - 1, 4, 4, 'k');
        over.rect(px, py, 3, 3, knights::seat_mark(id));
    }
}
