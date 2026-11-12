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
            // A guardian drawn in its own pose for its wind-up (the Lawn
            // Flamingo, a dread of the Deep) glows at its rim instead, so
            // the pose it winds up in still shows through.
            let posed = super::garden::is_flamingo(boss) || crate::rules::deep::dread::is(boss)
                || crate::rules::claim_bosses::Guardian::for_id(&boss.id).is_some();
            return if posed { rim_heat(im) } else { heat(im) };
        }
        return im.clone();
    }
    if enemy.flashing(run.tick) {
        let posed = enemy.boss.and_then(|i| run.bosses.get(usize::from(i))).is_some_and(|b| super::garden::is_flamingo(b) || crate::rules::deep::dread::is(b) || crate::rules::claim_bosses::Guardian::for_id(&b.id).is_some());
        return if posed { rim_heat(im) } else { heat(im) };
    }
    im.clone()
}

/// Cached palette tint: each bank keeps its own pale material ramp. The
/// source's shadow planes stay darker; struck metal can catch pale cream.
fn pale(im: &Img, strong: bool, metal: bool) -> Img {
    use super::super::ink::{BANK, PALETTE, PINK_BANK, RgbMap, VIOLET_BANK, bank_colours, rgb};
    use std::sync::OnceLock;
    static TINT: OnceLock<RgbMap<[[u8; 3]; 3]>> = OnceLock::new();
    let tint = TINT.get_or_init(|| {
        // A dye lifts up its own ramp toward its palest, as each realm bank
        // does toward its own pale; no realm colour's lift ever lands on one.
        let dyes = [VIOLET_BANK, PINK_BANK].into_iter().flat_map(|bank| {
            let ramp: Vec<[u8; 3]> = bank_colours(bank).iter().map(|&v| rgb(v)).collect();
            let peak = ramp[ramp.len() - 1];
            ramp.clone().into_iter().map(move |c| {
                let samples = [0, 1, 2].map(|phase| {
                    let wanted = if phase == 2 { hit_metal_rgb(c) } else { hit_rgb_toward(c, peak, phase == 1) };
                    *ramp
                        .iter()
                        .min_by_key(|r| (0..3).map(|k| (i32::from(r[k]) - i32::from(wanted[k])).pow(2) * [2, 6, 1][k]).sum::<i32>())
                        .unwrap_or(&c)
                });
                (c, samples)
            })
        });
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
            .chain(dyes)
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
/// `im` with its edge burning: its outline hot, a ring of heat outside it
/// and a band of it just inside, the rest of it as it was.
pub(super) fn rim_heat(im: &Img) -> Img {
    let mut out = im.clone();
    for y in 0..im.h {
        for x in 0..im.w {
            let solid = im.get(x, y).is_some();
            let near = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dy)| im.get(x + dx, y + dy).is_some()).count();
            if !solid && near > 0 {
                out.put(x, y, '7');
            } else if solid && near < 4 {
                out.put(x, y, if (x + y) % 2 == 0 { '@' } else { '6' });
            } else if solid && [(2, 0), (-2, 0), (0, 2), (0, -2)].iter().any(|(dx, dy)| im.get(x + dx, y + dy).is_none()) && (x + y) % 2 == 0 {
                out.put(x, y, '@');
            }
        }
    }
    out
}

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
    if boss && super::wanted::plated(run, enemy) {
        // Its health is its name plate's fill (`wanted::nameplates`).
        return;
    }
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
    shield_bar(cv, hero, x, y + 7, run.tick as u32);
}

/// A kite shield's meter under a knight's health (`shield::Wear`), for
/// everyone to see while the shield is up, worn or knocked aside: steel
/// for what it can still turn; knocked aside, a crack across the bar
/// (bright at first, as the shield drops) and red for its way back.
pub(super) fn shield_bar(cv: &mut Img, hero: &Hero, x: i32, y: i32, tick: u32) {
    let wear = hero.shield_wear;
    if hero.guard != crate::rules::cards::Guard::Shield || !(hero.shielding || wear.showing()) {
        return;
    }
    let (x0, w) = (x - 8, 16);
    cv.rect(x0 - 1, y - 1, w + 2, 3, 'k');
    cv.rect(x0, y, w, 1, 'g');
    if wear.down > 0 {
        let lit = (wear.back() * w as f32).round() as i32;
        cv.rect(x0, y, lit, 1, '7');
        let fresh = wear.down + crate::rules::shield::SHIELD_DOWN / 3 > crate::rules::shield::SHIELD_DOWN;
        let ink = if fresh || (tick / 4).is_multiple_of(2) { 'w' } else { 'J' };
        for (dx, dy) in [(-4, 0), (-3, -1), (-2, 0), (-1, 1), (0, 0), (1, -1), (2, 0), (3, 1), (4, 0)] {
            cv.put(x + dx, y + dy, ink);
        }
    } else {
        let lit = (wear.left(hero.max_hp) * w as f32).ceil() as i32;
        cv.rect(x0, y, lit, 1, 'z');
    }
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
/// A seat sees its own: what its knight could take, near its knight; its
/// own set-down shield too, which only the take key takes back.
pub(super) fn take_prompts(cv: &mut Img, run: &Run, legible: bool, view: (i32, i32), focus: Option<u32>) {
    for item in run.room().items.iter().filter(|item| run.lies_for(item, focus)) {
        let Some(card) = run
            .book
            .get(&item.card)
            .filter(|c| c.kind == cards::Kind::Arm || item.owner.is_some_and(|o| focus == Some(o)))
        else {
            continue;
        };
        // A champion keeps his own arm: no swap is offered him.
        if card.kind == cards::Kind::Arm
            && focus.and_then(|f| run.players.get(&f)).is_some_and(|h| crate::rules::champions::of(h).is_some())
        {
            continue;
        }
        let near = run
            .players
            .iter()
            .filter(|(id, _)| focus.is_none_or(|f| f == **id))
            .any(|(_, h)| h.hp > 0 && !h.stone && (h.x - item.x).hypot(h.y - item.y) < TAKE_REACH + 2.0);
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

/// Who took what off the floor, as seat `focus` sees it (the operator,
/// 2026-10-19: "we don't see if a friendly picked up a card or not"): a
/// friend's take rings where it lay in their colours and their name rises
/// as a mote flies to them; what the friends took their own of and still
/// lies for this knight wears a pip in each taker's colours; and this
/// knight's own take says what came of it over their head a moment
/// ("Potion: Z to drink", "hand full").
pub(super) fn takes(cv: &mut Img, run: &Run, legible: bool, view: (i32, i32), focus: Option<u32>) {
    use crate::rules::feel::TAKE_TICKS;
    for item in run.room().items.iter().filter(|item| !item.took.is_empty() && run.lies_for(item, focus)) {
        let (x, y) = at(item.x, item.y);
        let n = item.took.len().min(4) as i32;
        let left = x - (n * 3 - 1) / 2;
        for (i, &by) in item.took.iter().take(4).enumerate() {
            let px = left + i as i32 * 3;
            cv.rect(px - 1, y + 6, 4, 4, 'K');
            cv.rect(px, y + 7, 2, 2, super::party::seat_ink(by));
        }
    }
    for take in run.feel.takes.iter().filter(|t| focus != Some(t.by)) {
        let Some(hero) = run.players.get(&take.by) else {
            continue;
        };
        let ink = super::party::seat_ink(take.by);
        let age = i32::from(TAKE_TICKS - take.ttl.min(TAKE_TICKS));
        let (x, y) = at(take.x, take.y);
        // A ring opening where it lay, in their colours.
        if age < 10 {
            let r = 3.0 + age as f32 * 1.2;
            for i in 0..16 {
                let a = i as f32 / 16.0 * std::f32::consts::TAU;
                cv.put(x + (a.cos() * r).round() as i32, y + (a.sin() * r * 0.45).round() as i32, ink);
            }
        }
        // A mote flying from there to them.
        let k = (age as f32 / 12.0).min(1.0);
        let (kx, ky) = at(hero.x, hero.y);
        if k < 1.0 {
            let lift = (k * std::f32::consts::PI).sin() * 8.0;
            let (mx, my) = (x as f32 + (kx - x) as f32 * k, y as f32 + (ky - y) as f32 * k - lift);
            cv.rect(mx.round() as i32 - 1, my.round() as i32 - 1, 3, 3, ink);
        }
        // Their name popping up where it lay, rising, and lifting away
        // through the dither as the take is forgotten (`notices::board_from`).
        if legible {
            let name: String = hero.name.to_uppercase().chars().take(12).collect();
            let w = text_width(&name);
            let px = super::in_view_x(x, w + 4, view, cv.w) + 2;
            let py = (y - 18 - age / 4).max(0);
            let mut plate = Img::new(w + 4, 9);
            plate.rect(0, 0, w + 4, 9, 'K');
            plate.text(2, 1, &name, ink);
            super::notices::board_from(cv, &plate, (px - 2, py - 1), age as f32, Some(f32::from(TAKE_TICKS)), 5.0);
        } else if age < 20 {
            cv.rect(x - 1, (y - 10 - age / 3).max(0), 3, 3, ink);
        }
    }
    // This knight's own take, said over their head.
    let Some((when, who, line)) = run.found.as_ref() else {
        return;
    };
    let shown = run.tick.saturating_sub(*when) < u64::from(crate::rules::HZ * 5 / 2);
    if !legible || !shown || Some(*who) != focus || !crate::rules::take_line(line) {
        return;
    }
    let Some(hero) = run.players.get(who) else {
        return;
    };
    let (x, y) = at(hero.x, hero.y);
    let text: String = line.to_uppercase().chars().take(24).collect();
    let w = text_width(&text) + 6;
    let px = super::in_view_x(x, w, view, cv.w);
    let py = (y - 40).max(0);
    // Up into place past its line and back, held to be read, and lifted
    // away through the dither at the end (`notices::board_from`).
    let mut plate = Img::new(w + 1, 12);
    super::super::chrome::panel(&mut plate, (0, 0, w, 11), super::super::chrome::Tone::KEY);
    plate.text(3, 2, &text, 'i');
    let age = run.tick.saturating_sub(*when) as f32;
    super::notices::board_from(cv, &plate, (px, py), age, Some((crate::rules::HZ * 5 / 2) as f32), 7.0);
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

/// A warboar's tells: the line it marks before a charge, and the stars
/// over its head while it stands dazed.
pub(super) fn foes_marks(cv: &mut Img, run: &Run, enemy: &crate::rules::Enemy, tick: u32) {
    let (x, y) = at(enemy.x, enemy.y);
    match enemy.stage {
        1 => {
            let (dx, dy) = enemy.dir;
            let (px, py) = (-dy, dx);
            for k in 4..40 {
                if (k + tick as i32 / 2) % 5 < 2 {
                    continue;
                }
                let s = k as f32 * 2.2;
                for w in [-1.0f32, 0.0, 1.0] {
                    cv.put(
                        x + (dx * s + px * w) as i32,
                        y + (dy * s + py * w) as i32,
                        if w == 0.0 { '7' } else { '8' },
                    );
                }
            }
        }
        3 => {
            for k in 0..3 {
                let a = tick as f32 * 0.3 + k as f32 * 2.1;
                cv.put(
                    x + (a.cos() * 6.0) as i32,
                    y - 12 + (a.sin() * 2.0) as i32,
                    '6',
                );
            }
        }
        _ => {}
    }
    let _ = run;
}
