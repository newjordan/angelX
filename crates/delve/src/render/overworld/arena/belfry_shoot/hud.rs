//! The shoot's marks along the top of a seat's view: the bell and the
//! seconds, each knight's chip (name and harm, the leader crowned), the pot,
//! and under them the table's bar: ten pips that fill as the whole table
//! fells bats, and the bell that tolls when they are full. The view's own
//! HUD never shakes.

use super::super::super::chrome;
use super::super::super::ink::{Img, text_width};
use super::super::{NATIVE_W, juice, knights};
use super::bits;
use crate::rules::belfry_show::TOLL_EVERY;
use crate::rules::folly::{BAT_TIME, BatShoot};
use crate::rules::{HZ, Run};

/// How far down the view the top marks reach.
pub(super) const BOTTOM: i32 = 21;

/// Ticks a toll lights the pips and swings the strip's bell.
pub(super) const TOLLED: u32 = 24;

/// One knight's place in the strip.
pub(super) struct Chip {
    pub id: u32,
    pub x: i32,
    /// Letters of the name that fit.
    pub name: usize,
}

const LEFT: i32 = 40;
const RIGHT: i32 = 44;

/// The knights' chips, left to right in seat order, fitted to the strip.
pub(super) fn chips(shoot: &BatShoot) -> Vec<Chip> {
    let n = shoot.scores.len().max(1) as i32;
    let room = NATIVE_W - 8 - LEFT - RIGHT;
    let each = room / n;
    let name = ((each - 7 - 4 - 20 - 14) / 5).clamp(1, 5) as usize;
    shoot.scores.keys().enumerate().map(|(i, &id)| Chip { id, x: 4 + LEFT + each * i as i32, name }).collect()
}

/// The name a seat goes by here: `YOU` for the seat looking.
pub(super) fn called(run: &Run, id: u32, focus: Option<u32>, most: usize) -> String {
    if focus == Some(id) {
        return "YOU".to_string();
    }
    run.players
        .get(&id)
        .map_or_else(|| format!("SEAT {id}"), |h| h.name.to_uppercase())
        .chars()
        .take(most)
        .collect()
}

/// Where a knight's number sits in the strip, for what flies to it.
pub(super) fn anchor(shoot: &BatShoot, id: u32) -> (i32, i32) {
    chips(shoot)
        .iter()
        .find(|c| c.id == id)
        .map_or((NATIVE_W / 2, 8), |c| (c.x + 7 + c.name as i32 * 5 + 12, 8))
}

/// How long since the table's bell last tolled, if lately.
pub(super) fn tolled(shoot: &BatShoot, now: u32) -> Option<u32> {
    shoot.show.tolls.last().map(|&t| now.saturating_sub(t)).filter(|&a| a < TOLLED)
}

/// The strip and the table's bar.
pub(super) fn strip(over: &mut Img, run: &Run, shoot: &BatShoot, focus: Option<u32>) {
    let now = run.tick as u32;
    chrome::panel(over, (4, 1, NATIVE_W - 8, 12), chrome::Tone::SLATE);
    let ring = tolled(shoot, now);
    // The bell, swinging on a toll, and the seconds.
    let swung = ring.is_some_and(|a| a / 3 % 2 == 0);
    over.stamp(bits::strip(swung), 8, 2);
    let left = if shoot.done { 0 } else { shoot.left() };
    let low = left <= 10 && !shoot.done && !shoot.titled();
    let secs = if shoot.done { "DONE".to_string() } else { format!("{left}S") };
    let blink = low && now / 6 % 2 == 0;
    over.text(20, 4, &secs, if blink { '7' } else if low { 'R' } else { 'O' });
    let best = shoot.scores.values().max().copied().unwrap_or(0);
    let rivals = shoot.scores.len() > 1;
    for chip in chips(shoot) {
        let harm = shoot.scores.get(&chip.id).copied().unwrap_or(0);
        let ink = knights::seat_mark(chip.id);
        let you = focus == Some(chip.id);
        over.rect(chip.x, 5, 5, 5, 'k');
        over.rect(chip.x + 1, 6, 3, 3, ink);
        let name = called(run, chip.id, focus, chip.name);
        let nx = over.text(chip.x + 7, 4, &name, if you { 'T' } else { 'i' });
        let lead = rivals && harm == best && harm > 0;
        // A number flying in lands, and the knight's score swells.
        let landed = shoot
            .show
            .hits
            .iter()
            .filter(|h| h.kill && u32::from(h.who) == chip.id)
            .filter_map(|h| h.age(now).checked_sub(super::moments::FLY))
            .min();
        let pulse = landed.map_or(0.0, |age| juice::bump(age, 8));
        if lead {
            over.stamp(bits::crown(), nx + 2, 2);
        }
        let num = harm.to_string();
        let ink = if pulse > 0.0 { '9' } else if lead { 'T' } else { 'h' };
        over.text(nx + 5 + (4 - num.len() as i32).max(0) * 5, 4 - i32::from(pulse > 0.5), &num, ink);
    }
    // The pot, with a coin.
    let pot: u32 = shoot.ante.values().sum();
    let tag = format!("{pot}G");
    let px = NATIVE_W - 9 - text_width(&tag) - 8;
    over.stamp(bits::coin(), px, 4);
    over.text(px + 7, 4, &tag, 'O');
    pips(over, shoot, now, ring.filter(|_| shoot.show.team > 0));
}

/// The table's bar under the strip: ten pips toward the next toll, and the
/// table's count.
fn pips(over: &mut Img, shoot: &BatShoot, now: u32, ring: Option<u32>) {
    let filled = if ring.is_some() { u32::from(TOLL_EVERY) } else { u32::from(shoot.show.team % TOLL_EVERY) };
    let (w, gap) = (6, 2);
    let total = u32::from(TOLL_EVERY) as i32 * (w + gap) - gap;
    let x0 = (NATIVE_W - total) / 2;
    let count = shoot.show.team.to_string();
    over.rect(x0 - 3, 14, total + 6, 6, 'k');
    over.frame(x0 - 3, 14, total + 6, 6, 's');
    for i in 0..u32::from(TOLL_EVERY) {
        let x = x0 + i as i32 * (w + gap);
        let lit = i < filled;
        let ink = if let Some(a) = ring {
            if a / 2 % 2 == 0 { '9' } else { '6' }
        } else if lit {
            // The newest pip is bright a moment.
            let newest = i + 1 == filled && shoot.show.hits.iter().any(|h| h.kill && h.age(now) < 6);
            if newest { '9' } else { 'O' }
        } else {
            's'
        };
        over.rect(x, 16, w, 2, ink);
    }
    // The count, in the table's gold, to the right of the bar.
    over.text(x0 + total + 8, 14, &count, if ring.is_some() { '9' } else { 'o' });
    let _ = (BAT_TIME, HZ);
}
