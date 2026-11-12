//! The bar across the top and the key line along the bottom: the purse (a coin that counts as
//! the flying coins land), the pie of wedges, the eight banners (the new one climbs in), the ring
//! of sparks as eight small stars, and a shield per seat with its wedges. Words are few: the keys
//! show for the first seconds of a run and then get out of the way.
use super::super::super::ink::{Img, text_width};
use super::super::juice;
use super::{W, banner_ink, pie, seat_ink};
use crate::rules::bannerpeak::{
    knight::{Knight, Size},
    session::{ANNOUNCE_TICKS, Session},
    show::Kind,
};

/// The purse as the player sees it: coins still in the air are not counted yet.
fn shown_coins(s: &Session, id: u32, k: &Knight) -> u32 {
    let flying = s
        .show
        .marks
        .iter()
        .filter(|m| m.kind == Kind::Coin && m.n == 0 && u32::from(m.who) == id && m.age(s.elapsed) < 32)
        .count() as u32;
    k.coins.saturating_sub(flying)
}

/// A small eight-pointed star, `lit` or banked.
fn star(im: &mut Img, x: i32, y: i32, lit: bool, glint: bool) {
    let (c, e) = if lit { ('6', '5') } else { ('X', 'g') };
    im.put(x, y, if glint { 'w' } else { c });
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        im.put(x + dx, y + dy, e);
    }
    if lit {
        for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
            im.put(x + dx, y + dy, '4');
        }
    }
}

pub(super) fn draw(im: &mut Img, s: &Session, id: u32, tick: u32) {
    let me = s.seats.get(&id);
    im.rect(0, 0, W, 19, 'k');
    im.rect(0, 19, W, 1, 'g');
    let Some(k) = me else { return };
    // The purse: a coin that swells when one lands.
    let landed = s
        .show
        .marks
        .iter()
        .filter(|m| m.kind == Kind::Coin && m.n == 0 && u32::from(m.who) == id)
        .map(|m| m.age(s.elapsed))
        .filter(|&a| a >= 32)
        .min()
        .map_or(0.0, |a| juice::bump(a - 32, 6));
    purse(im, 9, 9, landed);
    let n = format!("{}", shown_coins(s, id, k));
    let text = juice_text(&n, if landed > 0.0 { 'T' } else { '6' });
    im.stamp(&text, 18, 4 - i32::from(landed > 0.3));
    // The pie of wedges.
    pie(im, 56, 9, k.pips);
    if k.size != Size::Small {
        im.text(68, 5, if k.size == Size::Flame { "FLAME" } else { "BIG" }, 'O');
    }
    // The eight banners: the newest climbs into its place and blinks once.
    let bx = 112;
    let fresh = s.announce.map(|a| (usize::from(a.banner), ANNOUNCE_TICKS.saturating_sub(a.t)));
    for n in 0..8usize {
        let x = bx + n as i32 * 12;
        if s.banners & (1 << n) != 0 {
            let (rise, flash) = match fresh {
                Some((b, age)) if b == n => (((24u32.saturating_sub(age)) as i32 * 14 / 24).max(0), age < 30 && age / 3 % 2 == 0),
                _ => (0, false),
            };
            im.rect(x, 3 + rise, 9, 12, banner_ink(n));
            im.frame(x, 3 + rise, 9, 12, if flash { 'w' } else { 'k' });
            im.rect(x + 2, 4 + rise, 1, 10, 'w');
        } else {
            im.frame(x, 3, 9, 12, 'g');
        }
    }
    im.text(bx + 100, 5, &format!("{}/8", s.count()), 'H');
    // The ring of sparks: eight small stars.
    let spark_x = 244;
    let newest = s.show.marks.iter().filter(|m| m.kind == Kind::Spark).map(|m| m.age(s.elapsed)).min();
    for n in 0..8 {
        let lit = s.sparks & (1 << n) != 0;
        let glint = lit && newest.is_some_and(|a| a < 10) && tick / 2 % 2 == 0 && n == s.sparks.count_ones() as i32 - 1;
        star(im, spark_x + n * 8 + 4, 9, lit, glint);
    }
    // The seats: a shield each in the seat's colour, its wedges beside it.
    if s.seats.len() > 1 {
        for (n, (&seat, kn)) in s.seats.iter().enumerate() {
            let x = 316 + (n as i32 % 3) * 22;
            let y = 2 + (n as i32 / 3) * 9;
            let ink = seat_ink(seat);
            let down = kn.bubble.is_some();
            im.rect(x, y, 6, 7, if down { 'g' } else { ink });
            im.frame(x, y, 6, 7, 'k');
            im.put(x + 2, y + 2, if seat == id { 'T' } else { 'k' });
            im.text(x + 8, y, &if down { "O".to_string() } else { format!("{}", kn.pips) }, if down { 'z' } else { 'h' });
        }
    }
    // The keys, for the first seconds only: four short pairs, none of them an "or".
    if s.elapsed < 420 {
        let fade = if s.elapsed > 360 { (s.elapsed - 360) as f32 / 60.0 } else { 0.0 };
        let mut line = Img::new(W, 11);
        line.rect(0, 0, W, 11, 'k');
        let legend = super::claims::LEGEND;
        line.text((W - text_width(legend)) / 2, 2, legend, 'h');
        juice::dissolve(&mut line, fade);
        im.stamp(&line, 0, 213);
    }
}

fn juice_text(s: &str, ink: char) -> Img {
    let mut t = Img::new(text_width(&format!("x{s}")).max(1), 7);
    t.text(0, 0, &format!("x{s}"), ink);
    t
}

/// A round gold coin, swelling a pixel when one lands in it.
fn purse(im: &mut Img, cx: i32, cy: i32, swell: f32) {
    let r = 5 + i32::from(swell > 0.4);
    for dy in -r..=r {
        for dx in -r..=r {
            let d = dx * dx + dy * dy;
            if d > r * r {
                continue;
            }
            let ink = if d > (r - 1) * (r - 1) {
                'o'
            } else if dx + dy < -r / 2 {
                '6'
            } else if dx + dy > r / 2 {
                '4'
            } else {
                '5'
            };
            im.put(cx + dx, cy + dy, ink);
        }
    }
    // A small crescent struck on its face.
    for (dx, dy) in [(0, -2), (-1, -1), (-1, 0), (-1, 1), (0, 2)] {
        im.put(cx + dx, cy + dy, 'a');
    }
}
