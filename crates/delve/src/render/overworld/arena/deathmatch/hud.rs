//! The first-person HUD: health and armour as bars, the knight's own face
//! that flinches when struck, kills as big numbers over ten pips, a ranked
//! board with a crown, the feed, and the danger of a blow taken (its
//! direction at the view's edge, a pulse when health is low).
use super::super::juice;
use super::super::knights;
use super::{EDGE, enlarge, name, panel, toned, word};
use crate::render::overworld::ink::{Img, bayer, text_width};
use crate::rules::HZ;
use crate::rules::deathmatch::{self as dm, Fighter, Session};
use crate::rules::juice::Weight;

fn bar(im: &mut Img, x: i32, y: i32, w: i32, h: i32, frac: f32, fill: char, top: char) {
    let filled = ((w as f32 * frac.clamp(0.0, 1.0)).round() as i32).clamp(0, w);
    for dy in 0..h {
        for dx in 0..w {
            let ink = if dx < filled {
                if dy == 0 { top } else { fill }
            } else if bayer(x + dx, y + dy) < 0.45 {
                'g'
            } else {
                'K'
            };
            im.put(x + dx, y + dy, ink);
        }
    }
    im.frame(x - 1, y - 1, w + 2, h + 2, 'g');
}

/// The bottom panel: health and armour, the face, the arm's readiness,
/// kills and the power's time.
pub(super) fn status(im: &mut Img, s: &Session, id: u32) {
    let f = &s.fighters[&id];
    let h = &f.hero;
    panel(im, 0, 180, 384, 44);
    for x in [139, 194, 262, 330] {
        for y in 184..220 {
            im.set(x, y, EDGE);
        }
    }
    // The blow just taken: the numbers flash and the face jolts.
    let hit = s.show.blows.iter().filter(|b| b.target == id).map(|b| b.age(s.ticks)).min();
    let jolt = hit.is_some_and(|a| a < 3);
    let low = h.hp * 100 < h.max_hp.max(1) * 30;
    word(im, 8, 185, "HEALTH", false);
    let pulse = low && (s.ticks / 6) % 2 == 0;
    let num = juice::big_text_img(&h.hp.to_string(), 2, if jolt || pulse { '7' } else { 'i' }, 'k');
    im.stamp(&num, 6, 191);
    bar(im, 46, 195, 86, 8, h.hp as f32 / h.max_hp.max(1) as f32, if low { '7' } else { 'o' }, if low { 'R' } else { 'T' });
    word(im, 8, 213, "ARMOR", false);
    bar(im, 46, 214, 86, 4, h.armor as f32 / crate::rules::MAX_ARMOR.max(1) as f32, 'J', 'h');
    // The face, flinching.
    let face = enlarge(&toned(&knights::portrait(h, id)), 1.7);
    let face = if jolt { juice::impact_frame(&face, 'H') } else { face };
    let shiver = if jolt { (s.ticks % 2) as i32 * 2 - 1 } else { 0 };
    im.stamp(&face, 166 - face.w / 2 + shiver, 218 - face.h);
    // The arm's readiness: a bar that fills as it rearms, and glints full.
    word(im, 201, 185, "ARM", false);
    let wait = h.fire_cooldown.min(30);
    bar(im, 201, 196, 56, 5, 1.0 - wait as f32 / 30.0, if wait == 0 { 'T' } else { 'J' }, if wait == 0 { 'c' } else { 'h' });
    if f.flash > 0 {
        im.rect(201, 205, 56, 2, '@');
    }
    // Kills, big, with ten pips; the number swells when a flying score lands.
    word(im, 269, 185, "KILLS", false);
    let landing = s
        .show
        .blows
        .iter()
        .filter(|b| b.owner == id && matches!(b.weight, Weight::Kill | Weight::Finale))
        .map(|b| juice::bump(b.age(s.ticks).saturating_sub(26), 8))
        .fold(0.0, f32::max);
    let scale = 3 + i32::from(landing > 0.4);
    let n = juice::big_text_img(&f.kills.to_string(), scale, if landing > 0.0 { 'T' } else { 'i' }, if landing > 0.0 { 'B' } else { 'g' });
    im.stamp(&n, 270, 196 - i32::from(scale == 4) * 2);
    word(im, 270 + n.w + 1, 204, &format!("/{}", dm::GOAL), false);
    for k in 0..dm::GOAL as i32 {
        let on = k < f.kills as i32;
        im.rect(270 + k * 5, 215, 4, 3, if on { if k == dm::GOAL as i32 - 1 { '@' } else { 'O' } } else { 'j' });
    }
    // Power: a warm bar draining while it lasts.
    word(im, 337, 185, "POWER", f.power > 0);
    if f.power > 0 {
        bar(im, 337, 196, 40, 5, f.power as f32 / dm::POWER_TIME as f32, 'O', 'T');
        word(im, 337, 205, &format!("{}", f.power.div_ceil(HZ)), true);
    } else {
        bar(im, 337, 196, 40, 5, 0.0, 'g', 'g');
    }
}

/// The board, top left: rank, name and kills, a thin line toward ten under
/// each, a crown on the leader, your own row warm; match point blinks.
pub(super) fn board(im: &mut Img, s: &Session, id: u32) {
    panel(im, 6, 6, 100, 16 + s.fighters.len() as i32 * 10);
    word(im, 12, 10, "FIRST TO TEN", true);
    let top = s.order().first().map(|&l| s.fighters[&l].kills).unwrap_or(0);
    for (k, other) in s.order().into_iter().enumerate() {
        let kills = s.fighters[&other].kills;
        let y = 21 + k as i32 * 10;
        let blink = kills + 1 == dm::GOAL && (s.ticks / 5) % 2 == 0;
        word(im, 20, y, &format!("{:8} {:2}", name(s, other, 8), kills), other == id || blink);
        word(im, 12, y, &format!("{}", other), other == id);
        if k == 0 && top > 0 {
            // The leader's crown, three prongs.
            for (dx, dy) in [(0, 2), (2, 0), (4, 2), (1, 3), (2, 3), (3, 3)] {
                im.put(96 + dx, y + dy, if blink { 'c' } else { 't' });
            }
        }
        let w = (92 * kills as i32 / dm::GOAL as i32).min(92);
        for dx in 0..w {
            im.put(12 + dx, y + 8, if other == id { 'o' } else { 'J' });
        }
    }
}

/// The feed, top right: who took whom, yours in warm, the newest sliding in.
pub(super) fn feed(im: &mut Img, s: &Session, id: u32) {
    for (k, frag) in s.feed.iter().rev().take(3).enumerate() {
        let line = format!("{} > {}", name(s, frag.killer, 9), name(s, frag.victim, 9));
        let age = s.ticks.saturating_sub(frag.tick);
        let slide = (6u32.saturating_sub(age) * 7) as i32;
        let mine = frag.killer == id || frag.victim == id;
        word(im, 378 - text_width(&line) + slide, 8 + k as i32 * 10, &line, mine);
    }
}

/// The danger of blows taken, over the view: a dark-red chevron at the edge
/// toward each attacker (`yaw` is where the knight faces), and a pulsing rim
/// when health is low.
pub(super) fn danger(im: &mut Img, s: &Session, f: &Fighter, id: u32, yaw: f32) {
    for b in s.show.blows.iter().filter(|b| b.target == id && b.age(s.ticks) < 22) {
        let age = b.age(s.ticks);
        let to = (b.from.1 - f.hero.y).atan2(b.from.0 - f.hero.x) - yaw;
        let fade = age as f32 / 22.0;
        let reach = 5 + (b.weight as i32) * 2;
        for step in -7..=7 {
            let a = to + step as f32 * 0.05;
            let (px, py) = (192.0 + a.sin() * 150.0, 82.0 - a.cos() * 62.0);
            for r in 0..reach {
                let (qx, qy) = (px - a.sin() * r as f32 * 1.6, py + a.cos() * r as f32 * 0.7);
                if bayer(qx as i32, qy as i32) < (1.0 - fade) * (1.0 - r as f32 / reach as f32 * 0.7) {
                    im.put(qx as i32, qy as i32, if r < 2 { '7' } else { 'p' });
                }
            }
        }
        if age < 5 {
            juice::vignette(im, (0, 0, 384, 180), 0.9 - age as f32 * 0.15, 14 + b.weight as i32 * 3, 'p');
        }
    }
    let h = &f.hero;
    if f.alive() && h.hp * 100 < h.max_hp.max(1) * 30 {
        let beat = ((s.ticks % 24) as f32 / 24.0 * std::f32::consts::TAU).sin().max(0.0);
        juice::vignette(im, (0, 0, 384, 180), 0.25 + beat * 0.45, 12, 'p');
    }
}
