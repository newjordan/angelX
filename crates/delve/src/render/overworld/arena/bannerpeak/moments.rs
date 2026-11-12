//! The run's moments, each a pure function of the session's own ticks: the title slam, the
//! banner raised (a card, a pennant climbing, fireworks), the calls (SAVED, ALL TOGETHER, the
//! stomp chain, a plate off the crown), King Cinder's crown pips, the danger rim, and the
//! finish at the Great Pole with its scores.
use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Tone};
use super::super::pfx::{Ramp, ramps};
use super::claims;
use super::{H, W, banner_ink, seat_ink};
use crate::rules::bannerpeak::{
    boss::PHASES,
    knight::Knight,
    session::{ANNOUNCE_TICKS, BANNER_NAMES, FINALE_TICKS, Phase, Session},
    show::{Cry, SHOUT_LIFE},
};
use crate::rules::{HZ, Run};

/// The title: two words slam in over the first ground, a line of three under them, and fade. The
/// card of a banner raised has the same band and takes it.
pub(super) fn title(im: &mut Img, s: &Session) {
    if s.elapsed >= claims::TITLE || s.phase != Phase::Play || s.announce.is_some() {
        return;
    }
    juice::banner(im, 34, "EIGHT BANNERS", "RUN. JUMP. RAISE.", s.elapsed, claims::TITLE, Tone::Warm, 3);
}

/// How loud a call is inked.
fn tone(cry: Cry) -> Tone {
    match cry {
        Cry::Lift => Tone::Steel,
        Cry::Plate => Tone::Hot,
        _ => Tone::Warm,
    }
}

/// The calls the table has made, placed by `claims::shouts`: the loudest on top, a quieter one
/// under it, never two of a kind, and out from under a banner's card.
pub(super) fn shouts(im: &mut Img, s: &Session, id: u32) {
    for (y, cap, title, sub, age, cry, _) in claims::shouts(s, id) {
        juice::banner(im, y, &title, &sub, age, SHOUT_LIFE, tone(cry), cap);
    }
}

/// A banner just raised: its card slams in over the top of the screen, three fireworks go up
/// behind it, and a pennant climbs a staff beside it. All of it from the announce's own clock,
/// so every seat sees the same.
pub(super) fn raised(im: &mut Img, s: &Session, tick: u32) {
    let Some(a) = s.announce else { return };
    if s.phase != Phase::Play {
        return;
    }
    let age = ANNOUNCE_TICKS.saturating_sub(a.t);
    let n = usize::from(a.banner);
    let ink = banner_ink(n);
    // The sky first: three fireworks in the colours of the realm.
    let rams: [Ramp; 3] = [ramps::GOLD, ramps::EMBER, ramps::BONE];
    for (j, x) in [96.0f32, 288.0, 192.0].into_iter().enumerate() {
        let t = age as i32 - 4 - j as i32 * 9;
        if t >= 0 {
            let top = [70.0, 84.0, 54.0][j];
            juice::firework(im, x, 150.0, top, t as u32, 17 * (a.banner as u32 + 1) + j as u32 * 5, rams[j], u64::from(tick));
        }
    }
    // The card.
    let name = BANNER_NAMES[n.min(7)];
    juice::banner(im, 30, name, &format!("BANNER {} OF 8", s.count()), age, ANNOUNCE_TICKS, Tone::Warm, 2);
    // A pennant climbing a staff either side of the card, a short rise that ends in a snap.
    for side in [-1i32, 1] {
        let x = 192 + side * 118;
        let climb = (age.min(40) as f32 / 40.0 * 34.0) as i32;
        im.rect(x - 1, 36, 2, 46, 'P');
        im.rect(x - 2, 34, 4, 3, 'o');
        let flutter = (tick / 4 % 3) as i32;
        let (fx, fy) = (x + 1, 78 - climb);
        im.rect(fx, fy, 14 - flutter, 9, ink);
        im.frame(fx, fy, 14 - flutter, 9, 'k');
        im.rect(fx + 3, fy + 2, 4, 5, 'T');
        im.rect(fx + 5, fy + 3, 3, 3, ink);
    }
}

/// The pips of King Cinder's three crowns, and the plates left on this one: shown while he is
/// awake and his crown is still up.
pub(super) fn crown_bar(im: &mut Img, s: &Session, me: Option<&Knight>) {
    let b = &s.boss;
    if !claims::crown_bar_on(s, me) {
        return;
    }
    let (bx, by, bw, bh) = claims::CROWN_BAR;
    im.rect(bx, by, bw, bh, 'k');
    im.frame(bx, by, bw, bh, 'g');
    // Three crowns: gold while still to take, ash once down.
    for i in 0..i32::from(PHASES) {
        let cx = bx + 5 + i * 14;
        let up = i >= i32::from(b.phase);
        let (fill, edge) = if up { ('t', 'o') } else { ('X', 'g') };
        for sp in 0..3 {
            let top = by + 4 + i32::from(sp % 2 != 0);
            im.rect(cx + sp * 4, top, 3, 4, fill);
            im.put(cx + sp * 4 + 1, top, if up { 'T' } else { 'g' });
        }
        im.rect(cx, by + 8, 11, 3, edge);
        im.rect(cx, by + 8, 11, 1, fill);
        if up {
            im.put(cx + 5, by + 9, '7');
        }
    }
    // The plates left on this crown, one to a seat.
    let seats = s.seats.len().clamp(1, 6) as i32;
    let left = i32::from(b.plates);
    for i in 0..seats {
        let px = bx + 49 + i * 5;
        let full = i < left;
        im.rect(px, by + 4, 4, 8, if full { 'k' } else { 'g' });
        im.rect(px + 1, by + 5, 2, 6, if full { 't' } else { 'X' });
        if full {
            im.put(px + 1, by + 5, 'T');
        }
    }
    let name = "KING CINDER";
    let (nx, ny, _, _) = claims::crown_name();
    im.text(nx, ny, name, 'o');
}

/// A rim of danger: red along the edges as a blow lands and a slow pulse on the last wedge.
pub(super) fn danger(im: &mut Img, s: &Session, me: Option<&Knight>, tick: u32) {
    let Some(k) = me else { return };
    if s.phase != Phase::Play {
        return;
    }
    let mut k_ = 0.0f32;
    // One wedge left: a slow pulse along the edges, thin and dark.
    if k.alive() && k.pips <= 1 {
        k_ = 0.10 + 0.10 * ((tick / 10 % 2) as f32);
    }
    // Just hit: a flash that fades over the first ticks.
    if k.hurt >= 62 {
        k_ = k_.max(0.5 * (k.hurt - 62) as f32 / 8.0);
    }
    if k_ > 0.0 {
        juice::vignette(im, (0, 20, W, H - 31), k_.min(0.6), 12, '7');
    }
}

/// The curtain of the finish: the knights at the Great Pole, a night of paper and fire, and a
/// panel with each knight's sparks and score and the best the realm has seen.
pub(super) fn finale(im: &mut Img, run: &Run, s: &Session, id: u32, tick: u32) {
    if s.phase != Phase::Finale {
        return;
    }
    let t = s.phase_t;
    // Fireworks over the court, the gold, the ember and the bone in turn.
    let launches = [(80.0f32, 74.0f32, ramps::GOLD), (192.0, 50.0, ramps::EMBER), (304.0, 80.0, ramps::BONE), (140.0, 64.0, ramps::GOLD), (250.0, 58.0, ramps::EMBER)];
    for (j, (x, top, ramp)) in launches.into_iter().enumerate() {
        let start = 12 + j as u32 * 22;
        if t >= start {
            let since = t - start;
            let round = since / 56;
            juice::firework(im, x + (round as f32 * 19.0) % 30.0 - 15.0, 168.0, top, since % 56, 29 * j as u32 + round * 11, ramp, u64::from(tick));
        }
    }
    if t > 30 {
        juice::confetti(im, (0, 20, W, 190), tick, 11, 90);
    }
    let words = claims::finale_words(t);
    // The title sits in the open sky beside the pole, not over its top banners: drawn on a canvas
    // as wide as that sky and laid there.
    let mut title = Img::new(232, 44);
    juice::banner(&mut title, 6, words, "", t.min(FINALE_TICKS / 2 + 1) % (FINALE_TICKS / 2 + 1), FINALE_TICKS / 2 + 1, Tone::Warm, 2);
    im.stamp(&title, 150, 24);
    // The panel: a row per knight, the best the realm has seen, the way home.
    let score = run.home.countryside.games.roundtable[crate::rules::bannerpeak::SLOT].best;
    if t > 70 {
        let (px, py, pw, ph) = claims::panel(s.seats.len());
        let k = (((t - 70) as f32) / 12.0).min(1.0);
        im.rect(px, py, pw, (ph as f32 * k) as i32, 'k');
        im.frame(px, py, pw, (ph as f32 * k) as i32, 'o');
        if k >= 1.0 {
            let top = s.seats.iter().map(|(_, k)| k.score).max().unwrap_or(0);
            for (i, (&seat, kn)) in s.seats.iter().enumerate() {
                let y = py + 8 + i as i32 * 12;
                im.rect(px + 8, y, 7, 8, seat_ink(seat));
                im.frame(px + 8, y, 7, 8, 'k');
                im.text(px + 20, y + 1, &if seat == id { "YOU".to_string() } else { format!("KNIGHT {seat}") }, if seat == id { 'T' } else { 'h' });
                im.text(px + 84, y + 1, &format!("x{}", kn.coins), '6');
                let sc = format!("{}", kn.score);
                im.text(px + pw - 8 - text_width(&sc), y + 1, &sc, if kn.score == top { 'T' } else { 'h' });
                if kn.score == top && s.seats.len() > 1 {
                    im.rect(px + 2, y + 2, 3, 3, 't');
                }
            }
            let sec = s.elapsed / HZ;
            let line = if score > 0 { format!("TIME {}:{:02}   BEST {}", sec / 60, sec % 60, score) } else { format!("TIME {}:{:02}", sec / 60, sec % 60) };
            im.text(px + pw / 2 - text_width(&line) / 2, py + ph - 11, &line, 'o');
        }
    }
}

/// The last crown falls: black bars close in over the picture for a moment, the way a film holds
/// its breath on the winning blow.
pub(super) fn film(im: &mut Img, s: &Session) {
    let Some(m) = s.show.marks.iter().find(|m| m.kind == crate::rules::bannerpeak::show::Kind::Crown) else { return };
    let age = m.age(s.elapsed);
    if age >= 70 || s.phase != Phase::Play {
        return;
    }
    let k = if age < 8 { age as f32 / 8.0 } else if age > 56 { (70 - age) as f32 / 14.0 } else { 1.0 };
    let h = (k * 13.0) as i32;
    im.rect(0, 20, W, h, 'k');
    im.rect(0, 213 - h, W, h, 'k');
}
