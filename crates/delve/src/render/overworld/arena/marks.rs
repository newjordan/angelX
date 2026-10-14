//! What a fight writes over its figures: health bars, the flash of a hit,
//! a desperate boss's flashing and the warnings of its last stand, and the
//! prompt beside a weapon that can be swapped to.
//!
//! The bars and flashes read the run's own state (`Enemy::hurt`,
//! `Enemy::desperation`, `Hero::hp`), so a friend's mirror and a browser
//! guest's picture show exactly what the host's does.

use super::super::ink::{Img, text_width};
use super::{UNIT, at};
use crate::rules::{Enemy, EnemyKind, Hero, Run, TAKE_REACH, cards, desperate};

/// A monster's picture with its flash on: a hit goes cream, then red; a
/// boss near death pulses red hot.
pub(super) fn flashed(im: &Img, enemy: &Enemy, tick: u64) -> Img {
    if enemy.hurt >= 4 {
        return silhouette(im, ['c', 'c']);
    }
    if enemy.hurt > 0 {
        return silhouette(im, ['7', '8']);
    }
    if enemy.flashing(tick) {
        return heat(im);
    }
    im.clone()
}

/// Every pixel in one ink (`bright`, `dark`) by its lightness.
fn silhouette(im: &Img, inks: [char; 2]) -> Img {
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(x, y, if l > 70 { inks[0] } else { inks[1] });
            }
        }
    }
    out
}

/// A boss's picture heated: its shadows to blood, its body to ember, only
/// its highlights to amber, so the shape stays readable while it burns.
fn heat(im: &Img) -> Img {
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(
                    x,
                    y,
                    match l {
                        0..=60 => '8',
                        61..=190 => '7',
                        _ if (x + y) % 2 == 0 => '@',
                        _ => '7',
                    },
                );
            }
        }
    }
    out
}

/// A bar of `w` by `h` with its left edge at `(x, y)`: a dark track, the
/// fill by `share` in `ink`, and a hairline frame.
fn bar(cv: &mut Img, x: i32, y: i32, w: i32, h: i32, share: f32, fill: char) {
    cv.rect(x - 1, y - 1, w + 2, h + 2, 'g');
    cv.rect(x, y, w, h, 'k');
    let lit = (share.clamp(0.0, 1.0) * w as f32).ceil() as i32;
    cv.rect(x, y, lit, h, fill);
}

/// A monster's health, over its head: always on a boss, and on any other
/// that has taken a hit. `(x, y)` is its feet and `im` its picture.
pub(super) fn enemy_bar(cv: &mut Img, enemy: &Enemy, x: i32, y: i32, im: &Img) {
    if enemy.hp == 0 || enemy.max_hp == 0 || enemy.kind == EnemyKind::Dummy {
        return;
    }
    let boss = enemy.is_boss();
    if !boss && !enemy.elite && enemy.hp >= enemy.max_hp {
        return;
    }
    let share = enemy.hp as f32 / enemy.max_hp as f32;
    let top = y - im.h;
    if boss {
        let w = im.w.clamp(24, 40);
        let fill = if enemy.desperation() > 0.0 { '@' } else { '7' };
        bar(cv, x - w / 2, top - 6, w, 3, share, fill);
        // Where it turns desperate: a notch on the bar.
        let notch = x - w / 2 + (desperate::FROM * w as f32) as i32;
        cv.put(notch, top - 8, 'J');
    } else {
        let w = im.w.clamp(12, 20);
        // An elite's bar is gold, and always up.
        bar(
            cv,
            x - w / 2,
            top - 5,
            w,
            3,
            share,
            if enemy.elite { '4' } else { '7' },
        );
    }
}

/// A knight's health under their feet, so the player's own is always in
/// sight. Teal while well, gold once the helmet is off, red in smallclothes.
pub(super) fn hero_bar(cv: &mut Img, hero: &Hero, x: i32, y: i32) {
    if hero.hp == 0 || hero.max_hp == 0 {
        return;
    }
    let fill = match hero.gear {
        0 => '2',
        1 => '4',
        _ => '7',
    };
    bar(
        cv,
        x - 8,
        y + 3,
        16,
        2,
        hero.hp as f32 / hero.max_hp as f32,
        fill,
    );
}

/// The warnings of a desperate boss: its ring, closing in dots with the
/// lane left open toward the nearest knight, and the three arms, as spokes.
pub(super) fn last_stand(cv: &mut Img, run: &Run, enemy: &Enemy, tick: u32) {
    let (x, y) = at(enemy.x, enemy.y);
    let r = (enemy.radius() * UNIT) as i32 + 4;
    if let Some(t) = enemy.ring_warning() {
        let nearest = run
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone)
            .min_by(|a, b| {
                (a.x - enemy.x)
                    .hypot(a.y - enemy.y)
                    .total_cmp(&(b.x - enemy.x).hypot(b.y - enemy.y))
            });
        let aim = nearest.map_or(0.0, |h| (h.y - enemy.y).atan2(h.x - enemy.x));
        let n = 18;
        let step = std::f32::consts::TAU / n as f32;
        // The dots draw in from wide to the ring's own radius, quicker to
        // blink as the moment comes.
        let reach = r as f32 * (1.8 - 0.8 * t);
        let on = (tick as f32 * (0.15 + t * 0.7)).fract() < 0.5 || t > 0.85;
        for i in 0..n {
            let a = aim + (i as f32 + 0.5) * step;
            let off = (a - aim).sin().atan2((a - aim).cos()).abs();
            if off < step * 1.5 {
                continue;
            }
            if on {
                let (px, py) = (
                    x + (a.cos() * reach) as i32,
                    y + (a.sin() * reach * 0.8) as i32,
                );
                let ink = if t > 0.7 { '@' } else { '7' };
                cv.rect(px, py, 2, 2, ink);
                cv.put(px - 1, py, '8');
                cv.put(px + 2, py + 1, '8');
            }
        }
        // The open lane, marked at the ring's edge: stand in it.
        for k in -1..=1 {
            let a = aim + k as f32 * step * 0.9;
            cv.rect(
                x + (a.cos() * (r as f32 + 3.0)) as i32,
                y + (a.sin() * (r as f32 + 3.0) * 0.8) as i32,
                2,
                2,
                '6',
            );
        }
    }
    if let Some(t) = enemy.arms_warning() {
        let turn = 0.0;
        for arm in 0..3 {
            let a = turn + arm as f32 * std::f32::consts::TAU / 3.0;
            let len = r + 6 + (t * 10.0) as i32;
            for k in (r / 2)..len {
                if (k + (tick as i32 / 2)) % 3 != 0 {
                    cv.put(
                        x + (a.cos() * k as f32) as i32,
                        y + (a.sin() * k as f32 * 0.8) as i32,
                        if t > 0.6 { '6' } else { '5' },
                    );
                }
            }
        }
    }
}

/// Beside a weapon a knight can swap to: its name and the key, on a plate
/// over the card. Small frames get only a gold mark, since type would blur.
pub(super) fn take_prompts(cv: &mut Img, run: &Run, legible: bool) {
    for item in &run.room().items {
        let Some(card) = run
            .book
            .get(&item.card)
            .filter(|c| c.kind == cards::Kind::Arm)
        else {
            continue;
        };
        let near = run
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - item.x).hypot(h.y - item.y) < TAKE_REACH + 2.0);
        if !near {
            continue;
        }
        let (x, y) = at(item.x, item.y);
        if !legible {
            for (dx, dy) in [(0, -14), (-1, -15), (1, -15), (0, -15)] {
                cv.put(x + dx, y + dy, '6');
            }
            continue;
        }
        let name: String = card.name.to_uppercase().chars().take(16).collect();
        let key = "E TAKE";
        let w = text_width(&name).max(text_width(key)) + 6;
        let px = (x - w / 2).clamp(0, (cv.w - w).max(0));
        let py = (y - 34).max(0);
        cv.rect(px, py, w, 21, 'k');
        cv.frame(px, py, w, 21, 'j');
        cv.text(px + (w - text_width(&name)) / 2, py + 3, &name, 'i');
        cv.text(px + (w - text_width(key)) / 2, py + 12, key, '6');
        // The swap is yours to make: it points at the card.
        cv.put(x, py + 21, '6');
        cv.put(x - 1, py + 21, '6');
        cv.put(x + 1, py + 21, '6');
        cv.put(x, py + 22, '6');
    }
}

/// "THIEF!" over each goblin that has just robbed a knight, on a plate. Small
/// frames get a gold mark alone.
pub(super) fn thief_cries(cv: &mut Img, run: &Run, legible: bool) {
    for enemy in run
        .enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::Goblin && e.cry > 0)
    {
        let (x, y) = at(enemy.x, enemy.y);
        let top = y - 22;
        if !legible {
            cv.rect(x - 1, top.max(0), 3, 6, '6');
            continue;
        }
        let word = "THIEF!";
        let w = text_width(word) + 6;
        let px = (x - w / 2).clamp(0, (cv.w - w).max(0));
        let up = top.max(3);
        cv.rect(px, up - 3, w, 11, 'k');
        cv.frame(px, up - 3, w, 11, '7');
        cv.text(
            px + 3,
            up,
            word,
            if (enemy.cry / 4) % 2 == 0 { '6' } else { '@' },
        );
    }
}
