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

/// A monster's picture with its flash on: a hit lifts its own ramp, then settles; a
/// boss near death pulses red hot.
pub(super) fn flashed(im: &Img, enemy: &Enemy, run: &Run) -> Img {
    if enemy.hurt >= 4 {
        let metal = enemy
            .boss
            .and_then(|i| run.bosses.get(usize::from(i)))
            .is_some_and(|b| b.id == crate::rules::signatures::ENGINE);
        return pale(im, true, metal);
    }
    if enemy.hurt > 0 {
        return pale(im, false, false);
    }
    // Winding up a charge-up, it glows, quicker as the blow nears.
    if let Some(boss) = enemy.boss.and_then(|i| run.bosses.get(usize::from(i)))
        && enemy.stage == crate::rules::signatures::WIND
        && let Some(up) = crate::rules::signatures::winding(enemy, boss)
    {
        let left = up.wind.saturating_sub(enemy.timer);
        let every = if left < 8 { 1 } else if left < 16 { 2 } else { 4 };
        if (run.tick / every).is_multiple_of(2) {
            return heat(im);
        }
        return im.clone();
    }
    if enemy.flashing(run.tick) {
        return heat(im);
    }
    im.clone()
}

/// Cached palette tint: each bank keeps its own pale material ramp. The
/// source's shadow planes stay darker; struck metal can catch pale cream.
fn pale(im: &Img, strong: bool, metal: bool) -> Img {
    use super::super::ink::{BANK, PALETTE, RgbMap, rgb};
    use std::sync::OnceLock;
    static TINT: OnceLock<RgbMap<[[u8; 3]; 3]>> = OnceLock::new();
    let tint = TINT.get_or_init(|| {
        PALETTE
            .iter()
            .enumerate()
            .map(|(index, &value)| {
                let c = rgb(value);
                let bank = index / BANK;
                let peak = match bank {
                    1 => [203, 213, 166],
                    2 => [228, 210, 175],
                    _ => [218, 216, 203],
                };
                let samples = [0, 1, 2].map(|phase| {
                    let wanted = if phase == 2 {
                        hit_metal_rgb(c)
                    } else {
                        hit_rgb_toward(c, peak, phase == 1)
                    };
                    let candidates = PALETTE.iter().enumerate().filter(|(i, value)| {
                        if rgb(**value).iter().any(|&channel| channel > 228) {
                            return false;
                        }
                        // Signal keeps its warm/cold family; only its palest
                        // source pixels may borrow the bounded cream highlight.
                        if phase == 2 && matches!(bank, 0 | 2 | 3) {
                            // A held strike on the engine catches across its
                            // brass and steel. The pale metal ramp must lift
                            // the dark joints too, to read at the game's 1×.
                            matches!(*i, 41..=44 | 56..=59 | 70..=71)
                        } else if bank == 4 {
                            if c[0] > c[2] {
                                *i >= 65 && *i != 72
                            } else {
                                (60..64).contains(i) || (58..=59).contains(i)
                            }
                        } else {
                            *i / BANK == bank
                                || (c.iter().copied().min().unwrap() > 170 && *i == 71)
                        }
                    });
                    candidates
                        .min_by_key(|(_, v)| {
                            let r = rgb(**v);
                            (0..3)
                                .map(|k| {
                                    (i32::from(r[k]) - i32::from(wanted[k])).pow(2) * [2, 6, 1][k]
                                })
                                .sum::<i32>()
                        })
                        .map_or(c, |(_, &v)| rgb(v))
                });
                (c, samples)
            })
            .collect()
    });
    let phase = if metal { 2 } else { usize::from(strong) };
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                out.set(
                    x,
                    y,
                    tint.get(&c).map_or_else(
                        || {
                            if metal {
                                hit_metal_rgb(c)
                            } else {
                                hit_rgb(c, strong)
                            }
                        },
                        |samples| samples[phase],
                    ),
                );
            }
        }
    }
    out
}

/// Pale reflected metal through a held hit. Warm brass and cool steel still
/// separate, with enough lift in the joints to read on an articulated boss.
pub(super) fn hit_metal_rgb(c: [u8; 3]) -> [u8; 3] {
    let light = (u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2])) / 10;
    let peak = if u16::from(c[0]) * 4 > u16::from(c[2]) * 5 {
        [228u32, 215, 192]
    } else {
        [213, 222, 224]
    };
    let amount = 148 + light / 4;
    std::array::from_fn(|k| {
        ((u32::from(c[k]) * (256 - amount) + peak[k] * amount) / 256).min(228) as u8
    })
}

/// RGB kit version of the same material tint, with no added coverage or glow.
pub(super) fn hit_rgb(c: [u8; 3], strong: bool) -> [u8; 3] {
    hit_rgb_toward(c, [224, 215, 198], strong)
}

fn hit_rgb_toward(c: [u8; 3], peak: [u8; 3], strong: bool) -> [u8; 3] {
    let light = (u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2])) / 10;
    let amount = if strong {
        80 + light / 2
    } else {
        34 + light / 6
    };
    std::array::from_fn(|k| {
        ((u32::from(c[k]) * (256 - amount) + u32::from(peak[k]) * amount) / 256).min(228) as u8
    })
}

/// A boss's picture heated: its shadows to blood, its body to ember, only
/// its highlights to amber, so the shape stays readable while it burns.
pub(super) fn heat(im: &Img) -> Img {
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
/// fill by `share` in `ink`, in a grey rim lit along its top and its corner
/// pixels cut (`chrome::Tone::HEALTH`), so it reads round on any floor.
#[allow(clippy::too_many_arguments)]
pub(super) fn bar(cv: &mut Img, x: i32, y: i32, w: i32, h: i32, share: f32, fill: char, delivered: bool) {
    super::super::chrome::shaped(cv, (x - 1, y - 1, w + 2, h + 2), &[1], super::super::chrome::Tone::HEALTH, false);
    if delivered {super::hud_art::plate(cv,"health",(x-1,y-1,w+2,h+2));}
    let lit = (share.clamp(0.0, 1.0) * w as f32).ceil() as i32;
    cv.rect(x, y, lit, h, fill);
}

/// A monster's health, over its head: always on a boss, and on any other
/// that has taken a hit. `(x, y)` is its feet and `im` its picture.
pub(super) fn enemy_top(run: &Run, enemy: &Enemy, feet: i32, height: i32, view: (i32,i32)) -> i32 {
    let top = feet - height.max(super::boss_art::ring_clearance(run,enemy));
    let library = enemy.boss.and_then(|i| run.bosses.get(i as usize))
        .and_then(|b| crate::rules::bosses::Art::for_id(&b.id))
        .is_some_and(super::boss_art::library_registered);
    // Reserve the existing name panel and bar stack when tall art leaves view.
    if library { top.max(view.1 + 22) } else { top }
}

pub(super) fn enemy_bar(cv: &mut Img, run: &Run, enemy: &Enemy, x: i32, y: i32, im: &Img, view: (i32,i32)) {
    if enemy.hp == 0 || enemy.max_hp == 0 || enemy.kind == EnemyKind::Dummy {
        return;
    }
    let boss = enemy.is_boss();
    if !boss && !enemy.elite && enemy.hp >= enemy.max_hp {
        return;
    }
    let share = enemy.hp as f32 / enemy.max_hp as f32;
    let top = enemy_top(run,enemy,y,im.h,view);
    if boss {
        let w = im.w.clamp(24, 40);
        let fill = if enemy.desperation() > 0.0 { '@' } else { '7' };
        bar(cv, x - w / 2, top - 6, w, 3, share, fill, super::hud_art::enabled(run));
        if super::hud_art::enabled(run) {
            super::hud_art::plate(cv,"boss",(x-w/2-3,top-9,w+6,9));
            cv.rect(x-w/2,top-6,(share.clamp(0.0,1.0)*w as f32).ceil() as i32,3,fill);
        }
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
            super::hud_art::enabled(run),
        );
    }
}

/// The Boss Key over its carrier's gold bar, `(x, y)` its feet and `im`
/// its picture: a key on a small slate plate, a glint running along it,
/// so a party can pick the carrier out of its camp.
pub(super) fn key_mark(cv: &mut Img, x: i32, y: i32, im: &Img, tick: u32) {
    const ROWS: [&str; 4] = [".444.....", "4...44444", "4...4.4.4", ".444....."];
    let key = Img::from_rows(&ROWS);
    let (x0, y0) = (x - key.w / 2, y - im.h - 13);
    super::super::chrome::panel(cv, (x0 - 2, y0 - 2, key.w + 4, key.h + 4), super::super::chrome::Tone::SLATE);
    cv.stamp(&key, x0, y0);
    // The glint walks the key once a second, and rests between.
    let along = (tick % 40) as usize;
    for (row, line) in ROWS.iter().enumerate() {
        if line.as_bytes().get(along).is_some_and(|&b| b != b'.') {
            cv.put(x0 + along as i32, y0 + row as i32, '$');
        }
    }
}

/// A knight's health under their feet, so the player's own is always in
/// sight. Teal while well, gold once the helmet is off, red in smallclothes.
pub(super) fn hero_bar(cv: &mut Img, run: &Run, hero: &Hero, x: i32, y: i32) {
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
        super::hud_art::enabled(run),
    );
}

/// The warnings of a desperate boss: its ring, closing in dots with the
/// lane left open toward the nearest knight, and the three arms, as spokes.
pub(super) fn last_stand(cv: &mut Img, run: &Run, enemy: &Enemy, tick: u32) {
    let (x, y) = at(enemy.x, enemy.y);
    let r = (enemy.radius() * UNIT) as i32 + 4;
    if let Some(t) = enemy.ring_warning().filter(|_| !super::boss_art::ring_warning(cv, run, enemy)) {
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
pub(super) fn take_prompts(cv: &mut Img, run: &Run, legible: bool, view: (i32, i32)) {
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
        let px = super::in_view_x(x, w, view, cv.w);
        let py = (y - 34).max(0);
        super::super::chrome::panel(cv, (px, py, w, 21), super::super::chrome::Tone::KEY);
        cv.text(px + (w - text_width(&name)) / 2, py + 3, &name, 'i');
        cv.text(px + (w - text_width(key)) / 2, py + 12, key, 'O');
        // The swap is yours to make: it points at the card.
        cv.put(x, py + 21, '6');
        cv.put(x - 1, py + 21, '6');
        cv.put(x + 1, py + 21, '6');
        cv.put(x, py + 22, '6');
    }
}

/// "THIEF!" over each goblin that has just robbed a knight, on a plate. Small
/// frames get a gold mark alone.
pub(super) fn thief_cries(cv: &mut Img, run: &Run, legible: bool, view: (i32, i32)) {
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
        let px = super::in_view_x(x, w, view, cv.w);
        let up = top.max(3);
        super::super::chrome::panel(
            cv,
            (px, up - 3, w, 11),
            super::super::chrome::Tone::SLATE.rimmed(super::super::chrome::rim_of('7')),
        );
        cv.text(
            px + 3,
            up,
            word,
            if (enemy.cry / 4) % 2 == 0 { '6' } else { '@' },
        );
    }
}
