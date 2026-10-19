//! The floor's cauldron, drawn (`rules::cauldron`): a black iron pot on
//! three stubby legs over a small fire, its brew a witch's green, lighting
//! the black floor round it — warm at its feet, green further out. It rises
//! out of the floor when the guardian falls and bubbles while it waits.
//! A brew plays out over it: the two cards arc in and splash, the pot roils
//! and shakes, puffs, and the new card rises out of the steam with its name
//! over it in one line. A misfire puffs and throws confetti, or leaves its
//! brewer hiccuping bubbles (the frog draws itself).

use super::super::chrome::{self, Tone};
use super::super::ink::{Img, bayer, hash, nearest_plain, text_width, vnoise};
use super::super::light::seam;
use super::{at, in_view_x};
use crate::rules::cards::Card;
use crate::rules::cauldron::{self, Ask, Brew, Cauldron, Misfire, Outcome, Tier};
use crate::rules::{HZ, Run};

/// The pot's foot below its floor point, and its sprite's box round the
/// foot: the sprite is drawn on its own canvas, then laid on the room.
const FOOT: i32 = 4;
const BOX_W: i32 = 72;
const BOX_H: i32 = 64;
/// Where the foot sits on that canvas.
const ANCHOR: (i32, i32) = (36, 58);
/// The brew's surface over the foot.
const RIM_UP: i32 = 21;

fn age(tick: u64, from: u64) -> u32 {
    tick.saturating_sub(from).min(u64::from(u32::MAX)) as u32
}

/// How hard the pot boils: a simmer at rest, rolling while a brew cooks.
fn boil(pot: &Cauldron, tick: u64) -> f32 {
    pot.brews
        .iter()
        .map(|b| {
            let a = age(tick, b.at);
            if a < cauldron::SPLASH[1] {
                0.45
            } else if a < cauldron::PUFF {
                0.45 + 0.55 * (a - cauldron::SPLASH[1]) as f32 / (cauldron::PUFF - cauldron::SPLASH[1]) as f32
            } else if a < cauldron::PUFF + HZ {
                1.0 - 0.7 * (a - cauldron::PUFF) as f32 / HZ as f32
            } else {
                0.3
            }
        })
        .fold(0.3, f32::max)
}

/// The pot's foot in room pixels, with the shake of a brew about to blow.
fn foot(pot: &Cauldron, tick: u64) -> (i32, i32) {
    let (x, y) = at(pot.x, pot.y);
    let shake = pot.brews.iter().any(|b| {
        let a = age(tick, b.at);
        (cauldron::PUFF - 18..cauldron::PUFF).contains(&a)
    });
    let dx = if shake { [0, 1, 0, -1][(tick % 4) as usize] } else { 0 };
    (x + dx, y + FOOT)
}

/// Under the knights: the light on the floor, the pot, its fire, its brew,
/// its bubbles and steam, and the splash of a card thrown in.
pub(super) fn under(cv: &mut Img, run: &Run) {
    let Some(pot) = run.cauldron_here() else {
        return;
    };
    let tick = run.tick;
    if tick < pot.rose {
        return;
    }
    let boil = boil(pot, tick);
    let (fx, fy) = foot(pot, tick);
    let rising = age(tick, pot.rose);
    let risen = (rising as f32 / cauldron::RISE_TICKS as f32).min(1.0);
    let still = super::scenery_cached(run);
    pool(cv, &still, (fx, fy), tick, boil * risen);
    let mut sprite = Img::new(BOX_W, BOX_H);
    draw_pot(&mut sprite, tick, boil);
    // Rising out of the floor: the pot climbs, cut off at the floor line.
    let eased = 1.0 - (1.0 - risen).powi(2);
    let sink = ((1.0 - eased) * (RIM_UP + 8) as f32).round() as i32;
    let (ox, oy) = (fx - ANCHOR.0, fy - ANCHOR.1 + sink);
    for y in 0..BOX_H {
        if sink > 0 && oy + y > fy {
            break;
        }
        for x in 0..BOX_W {
            if let Some(c) = sprite.get(x, y) {
                cv.set(ox + x, oy + y, c);
            }
        }
    }
    if rising < cauldron::RISE_TICKS + 8 {
        dust(cv, (fx, fy), rising);
    }
    for brew in &pot.brews {
        splashes(cv, run, brew, (fx, fy));
    }
}

/// Over the knights: the cards in flight, the puff, the new card risen with
/// its name, confetti and hiccups.
pub(super) fn over(cv: &mut Img, run: &Run, legible: bool, view: (i32, i32)) {
    let Some(pot) = run.cauldron_here() else {
        return;
    };
    let tick = run.tick;
    let (fx, fy) = foot(pot, tick);
    for brew in &pot.brews {
        let a = age(tick, brew.at);
        if a < cauldron::SPLASH[1] {
            in_flight(cv, run, brew, a, (fx, fy));
        }
        if a > cauldron::SHOWN + cauldron::HICCUP_TICKS {
            continue;
        }
        if (cauldron::PUFF..cauldron::PUFF + 16).contains(&a) {
            puff(cv, (fx, fy - RIM_UP), a - cauldron::PUFF, matches!(brew.outcome, Outcome::Misfire(_)));
        }
        match &brew.outcome {
            Outcome::Spell { card, tier, .. } if (cauldron::PUFF..cauldron::SHOWN).contains(&a) => {
                rising_card(cv, run.book.get(card), *tier, (fx, fy - RIM_UP), a - cauldron::PUFF);
            }
            Outcome::Misfire(Misfire::Confetti) if (cauldron::PUFF..cauldron::SHOWN).contains(&a) => {
                confetti(cv, (fx, fy), a - cauldron::PUFF, brew.at);
            }
            Outcome::Misfire(Misfire::Hiccups) if a >= cauldron::PUFF => {
                if let Some(hero) = run.players.get(&brew.knight) {
                    hiccups(cv, at(hero.x, hero.y), a - cauldron::PUFF);
                }
            }
            _ => {}
        }
    }
    if legible && let Some(brew) = pot.showing(tick) {
        let a = age(tick, brew.at);
        if a >= cauldron::PUFF + 6 {
            name_line(cv, brew, (fx, fy), a - cauldron::PUFF, view);
        }
    }
}

/// The pot's prompt for the seat's own knight: E to brew, the four slots
/// to pick from, or why not.
pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>, legible: bool, view: (i32, i32)) {
    let Some(pot) = run.cauldron_here() else {
        return;
    };
    let Some(id) = focus.or_else(|| run.players.keys().copied().min()) else {
        return;
    };
    let Some(ask) = run.cauldron_ask(id) else {
        return;
    };
    // While its own brew shows, the brew speaks for the pot.
    if pot.showing(run.tick).is_some_and(|b| b.knight == id) {
        return;
    }
    let (fx, fy) = foot(pot, run.tick);
    if !legible {
        cv.rect(fx - 1, fy - RIM_UP - 14, 3, 5, '6');
        return;
    }
    let top = |h: i32| (fy - RIM_UP - 12 - h).max(view.1 + 2);
    match ask {
        Ask::Brew => plate(cv, fx, top(21), &["CAULDRON", "E BREW TWO SPELLS"], &['i', 'O'], view),
        Ask::Short => plate(cv, fx, top(21), &["CAULDRON", "NEEDS TWO SPELLS"], &['i', 'h'], view),
        Ask::Brewed => plate(cv, fx, top(21), &["CAULDRON", "BREWED THIS FLOOR"], &['i', 'h'], view),
        Ask::Pick(n) => {
            let Some(hero) = run.players.get(&id) else {
                return;
            };
            let picks = pot.stir(id).map(|s| s.picks.clone()).unwrap_or_default();
            let mut lines: Vec<(String, char, bool)> = vec![("PICK TWO SPELLS".into(), 'O', false)];
            for (slot, spell) in hero.spells.iter().enumerate() {
                let Some(spell) = spell else {
                    continue;
                };
                let name = run.book.get(spell).map_or(spell.as_str(), |c| c.name.as_str());
                let picked = picks.contains(&(slot as u8));
                lines.push((format!("{} {}", slot + 1, say(name)), if picked { '6' } else { 'h' }, picked));
            }
            lines.push((if n == 2 { "E BREW" } else { "E LEAVE" }.into(), if n == 2 { '6' } else { 'J' }, false));
            let w = lines.iter().map(|l| text_width(&l.0)).max().unwrap_or(0) + 14;
            let h = lines.len() as i32 * 10 + 4;
            let px = in_view_x(fx, w, view, cv.w);
            let py = top(h);
            chrome::panel(cv, (px, py, w, h), Tone::KEY);
            for (i, (text, ink, picked)) in lines.iter().enumerate() {
                let y = py + 3 + i as i32 * 10;
                let x = if i == 0 || i == lines.len() - 1 { px + (w - text_width(text)) / 2 } else { px + 10 };
                if *picked {
                    // A drop of the brew beside what goes in.
                    cv.ellipse((px + 5) as f32 + 0.5, y as f32 + 3.5, 2.0, 2.0, 'A');
                    cv.put(px + 4, y + 2, 'Y');
                }
                cv.text(x, y, text, *ink);
            }
            // The pointer down to the pot.
            for d in 0..2 {
                cv.put(fx, py + h + d, '6');
            }
        }
    }
}

/// What the font can say of a name: its letters, digits and plain marks.
fn say(name: &str) -> String {
    name.to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || " -.!?:,".contains(*c))
        .take(24)
        .collect()
}

/// A small plate of lines over the pot.
fn plate(cv: &mut Img, x: i32, py: i32, lines: &[&str], inks: &[char], view: (i32, i32)) {
    let w = lines.iter().map(|l| text_width(l)).max().unwrap_or(0) + 8;
    let h = lines.len() as i32 * 9 + 3;
    let px = in_view_x(x, w, view, cv.w);
    chrome::panel(cv, (px, py, w, h), Tone::KEY);
    for (i, (line, ink)) in lines.iter().zip(inks).enumerate() {
        cv.text(px + (w - text_width(line)) / 2, py + 3 + i as i32 * 9, line, *ink);
    }
    for d in 0..2 {
        cv.put(x, py + h + d, '6');
    }
}

/// The light the pot throws on the floor: the stone it reaches shows,
/// warm where its fire reaches, the brew's green further out, breathing
/// with the boil. Only what is still dark is lit: what stands keeps its ink.
fn pool(cv: &mut Img, still: &Img, (fx, fy): (i32, i32), tick: u64, boil: f32) {
    if boil <= 0.0 {
        return;
    }
    let breathe = 0.92 + 0.08 * (tick as f32 * 0.19).sin() + 0.04 * (tick as f32 * 0.61).sin();
    let reach = (34.0 + 12.0 * boil) * breathe;
    let (cx, cy) = (fx as f32, fy as f32 - 3.0);
    let r = reach.ceil() as i32;
    for dy in -(r / 2) - 2..=r / 2 + 2 {
        for dx in -r..=r {
            let (x, y) = (fx + dx, fy - 3 + dy);
            if cv.get(x, y).is_some() {
                continue;
            }
            let d = ((x as f32 + 0.5 - cx) / reach).hypot((y as f32 + 0.5 - cy) / (reach * 0.5));
            if d >= 1.0 {
                continue;
            }
            let under = still.get(x, y).unwrap_or([0, 0, 0]);
            // The stone's grain, so the light falls on a floor and not on
            // a screen: clumps of grey stone, of what the fire warms, of
            // what the brew greens; steps give way in soft blots (`seam`).
            let grain = vnoise(x as f32 / 3.4, y as f32 / 2.6, 91);
            let jitter = (vnoise(x as f32 / 1.2, y as f32 / 1.2, 33) - 0.5) * 1.1;
            let lit = (1.0 - d).powf(1.3) * (0.55 + 0.45 * boil) * (0.8 + 0.4 * grain);
            let v = (lit * 3.8 + jitter).max(0.0);
            let th = seam(x, y);
            let step = (v.floor() as usize + usize::from(v.fract() > th)).min(4);
            if step == 0 {
                continue;
            }
            // Warm where the fire reaches, the brew's green beyond.
            let warm = (1.0 - d / 0.5).clamp(0.0, 1.0) > th * 0.8 + 0.1;
            const WARM: [[char; 3]; 4] = [['k', 'n', 'k'], ['n', 'K', 'b'], ['b', 'X', 'I'], ['I', 'g', 'P']];
            const GREEN: [[char; 3]; 4] = [['k', 'f', 'k'], ['f', 'K', 'D'], ['D', 'X', 'F'], ['F', 'g', 'E']];
            let clump = vnoise(x as f32 / 1.7, y as f32 / 1.7, 5);
            let pick = if clump > 0.62 { 2 } else if clump > 0.4 { 1 } else { 0 };
            let ramp = if warm { WARM } else { GREEN };
            let Some(ink) = super::super::ink::ink(ramp[step - 1][pick]) else {
                continue;
            };
            let lum = |c: [u8; 3]| u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2]);
            if lum(ink) > lum(under) {
                cv.set(x, y, ink);
            }
        }
    }
}

/// The pot itself on its canvas, its foot at `ANCHOR`: three stubby legs,
/// a small fire of two crossed logs under the belly, black iron with a dull
/// sheen up its left and the fire's red along its underside, a riveted
/// band, a thick lip, and the brew, rolling, with bubbles and steam.
fn draw_pot(im: &mut Img, tick: u64, boil: f32) {
    let (ax, ay) = ANCHOR;
    let (cx, base) = (ax as f32 + 0.5, ay);
    let t = tick as u32;
    // Two logs crossed under it, their ends glowing.
    im.line(ax - 9, base, ax + 8, base - 2, 'b');
    im.line(ax - 9, base - 1, ax + 8, base - 3, 'B');
    im.line(ax - 8, base - 3, ax + 9, base, 'b');
    im.line(ax - 8, base - 2, ax + 9, base + 1, 'n');
    for (ex, ey) in [(ax + 8, base - 3), (ax - 8, base - 3), (ax + 9, base)] {
        im.put(ex, ey, if (t / 5 + ex as u32) % 3 == 0 { 'o' } else { 'R' });
    }
    // The belly.
    let (bcy, brx, bry) = ((base - 12) as f32, 14.5, 10.0);
    for y in (bcy - bry) as i32 - 1..=(bcy + bry) as i32 + 1 {
        for x in (cx - brx) as i32 - 1..=(cx + brx) as i32 + 1 {
            let dx = (x as f32 + 0.5 - cx) / brx;
            let dy = (y as f32 + 0.5 - bcy) / bry;
            let d = dx * dx + dy * dy;
            if d > 1.0 {
                continue;
            }
            let ink = if d > 0.86 {
                // The outline, lit red where the fire is under it.
                if dy > 0.6 && dx.abs() < 0.45 { 'B' } else if dy > 0.5 && dx.abs() < 0.6 { 'b' } else { 'k' }
            } else if dy > 0.72 && dx.abs() < 0.5 {
                'b'
            } else if dy > 0.55 && dx.abs() < 0.6 {
                'n'
            } else if dx < -0.42 && dx > -0.78 && dy > -0.62 && dy < 0.25 {
                // The sheen: a dull band up the left of the belly.
                if dx < -0.6 && dy > -0.45 && dy < 0.05 { 'j' } else { 'g' }
            } else if dx < -0.15 && dy < 0.35 {
                'X'
            } else if dx > 0.55 {
                'k'
            } else {
                'K'
            };
            im.put(x, y, ink);
        }
    }
    // The riveted band round the belly's shoulder.
    for x in (cx - 13.5) as i32..=(cx + 13.5) as i32 {
        let dx = (x as f32 + 0.5 - cx) / 14.5;
        let y = base - 16 + (dx * dx * 2.4).round() as i32;
        let ink = if dx < -0.35 { 'G' } else if dx < 0.4 { 'g' } else { 'X' };
        im.put(x, y, ink);
        im.put(x, y + 1, 'k');
        if (x - ax).rem_euclid(5) == 0 {
            im.put(x, y, if dx < 0.3 { 'J' } else { 'j' });
        }
    }
    // Three legs, the middle one behind.
    for (lx, front) in [(ax - 10, true), (ax - 1, false), (ax + 8, true)] {
        let h = if front { 5 } else { 3 };
        im.rect(lx, base - h, 3, h, 'K');
        im.put(lx, base - h, 'g');
        im.put(lx, base - 1, 'X');
        im.put(lx + 2, base - 1, 'k');
    }
    // The fire licking up round the belly's foot: tongues that flicker.
    for i in 0..9 {
        let x = ax - 7 + i * 2 - (i % 2);
        let mid = 1.0 - ((i - 4) as f32 / 4.5).abs();
        let flick = (hash(x, (t / 3) as i32, 7) % 3) as f32;
        let h = (1.0 + mid * (2.5 + 2.5 * boil) + flick * 0.7) as i32;
        for k in 0..h {
            let f = k as f32 / h as f32;
            let ink = if f < 0.3 { 'R' } else if f < 0.6 { 'o' } else if f < 0.85 { 'O' } else { 't' };
            im.put(x, base - 1 - k, ink);
        }
        if i % 3 == 1 {
            // Sparks loosed upward now and then.
            let s = (t + (i as u32) * 7) % 19;
            if s < 6 {
                im.put(x + (s as i32 % 2), base - 3 - h - s as i32, if s < 3 { 'O' } else { 'r' });
            }
        }
    }
    // The lip: a thick iron rim, lit along its top, darker inside.
    let ry = (base - RIM_UP) as f32;
    im.ellipse(cx, ry + 0.5, 14.0, 4.6, 'k');
    im.ellipse(cx, ry, 13.4, 4.1, 'g');
    im.ellipse(cx, ry + 0.4, 11.8, 3.2, 'X');
    for x in (cx - 13.0) as i32..=(cx + 13.0) as i32 {
        let dx = (x as f32 + 0.5 - cx) / 13.4;
        let top = ry - 4.1 * (1.0 - dx * dx).max(0.0).sqrt();
        let bottom = ry + 4.1 * (1.0 - dx * dx).max(0.0).sqrt();
        im.put(x, top.round() as i32, if dx < -0.15 { 'J' } else if dx < 0.5 { 'j' } else { 'g' });
        im.put(x, bottom.round() as i32, if dx < -0.2 { 'G' } else { 'X' });
    }
    // The brew, a deep green rolling lighter, risen a little with the boil.
    let (lrx, lry) = (11.0, 2.7);
    let ly = ry + 0.6 - boil * 0.6;
    for y in (ly - lry) as i32 - 1..=(ly + lry) as i32 + 1 {
        for x in (cx - lrx) as i32 - 1..=(cx + lrx) as i32 + 1 {
            let dx = (x as f32 + 0.5 - cx) / lrx;
            let dy = (y as f32 + 0.5 - ly) / lry;
            if dx * dx + dy * dy > 1.0 {
                continue;
            }
            let swirl = ((x as f32 * 0.7 + tick as f32 * (0.12 + 0.12 * boil)).sin()
                + (y as f32 * 1.9 + x as f32 * 0.3 - tick as f32 * 0.09).cos())
                * 0.5;
            let ink = if dy < -0.55 {
                // The far edge, in the lip's shadow.
                'F'
            } else if swirl > 0.75 - boil * 0.3 {
                'A'
            } else if swirl > 0.15 {
                'm'
            } else if swirl > -0.45 {
                'l'
            } else {
                'E'
            };
            im.put(x, y, ink);
        }
    }
    // Bubbles: each its own beat, swelling and popping.
    let count = 4 + (boil * 8.0) as i32;
    for b in 0..count {
        let period = 12 + hash(b, 3, 11) % 12;
        let phase = (t + hash(b, 5, 13) % period) % period;
        let round = ((t + hash(b, 5, 13) % period) / period) as i32;
        let bx = ax - 8 + (hash(b, round, 17) % 17) as i32;
        let by = ly as i32 - 1 + (hash(b, round, 19) % 3) as i32;
        let grow = phase * 4 / period;
        match grow {
            0 => im.put(bx, by, 'A'),
            1 => {
                im.put(bx, by, 'M');
                im.put(bx + 1, by, 'A');
            }
            2 => {
                im.put(bx, by - 1, 'Y');
                im.put(bx + 1, by - 1, 'y');
                im.put(bx - 1, by, 'A');
                im.put(bx + 2, by, 'A');
                im.put(bx, by, 'l');
                im.put(bx + 1, by, 'l');
            }
            _ => {
                // Pop: flecks thrown up.
                im.put(bx - 1, by - 2, 'y');
                im.put(bx + 2, by - 3, 'A');
                im.put(bx, by - 4 - (boil * 2.0) as i32, 'M');
            }
        }
    }
    // Steam: soft puffs climbing off the brew, drifting and thinning.
    let wisps = 3 + (boil * 4.0) as i32;
    for w in 0..wisps {
        let period = 44 + hash(w, 1, 23) % 24;
        let phase = (t + hash(w, 2, 29) % period) % period;
        let round = ((t + hash(w, 2, 29) % period) / period) as i32;
        let sx = ax - 7 + (hash(w, round, 31) % 15) as i32;
        let rise = phase as f32 * (0.3 + 0.15 * boil);
        let thin = phase as f32 / period as f32;
        if thin > 0.8 {
            continue;
        }
        let (x, y) = (sx as f32 + (rise * 0.16 + w as f32).sin() * 2.5, ly - 3.0 - rise);
        let r = 1.0 + thin * 2.2;
        for py in (y - r) as i32..=(y + r) as i32 {
            for px in (x - r) as i32..=(x + r) as i32 {
                let d = (px as f32 + 0.5 - x).hypot(py as f32 + 0.5 - y) / r;
                if d > 1.0 {
                    continue;
                }
                if im.get(px, py).is_none() {
                    im.put(px, py, if d < 0.45 && thin < 0.5 { 'G' } else if thin < 0.7 { 'g' } else { 'X' });
                }
            }
        }
    }
}

/// Dust thrown up as the pot shoulders out of the floor.
fn dust(cv: &mut Img, (fx, fy): (i32, i32), age: u32) {
    let t = age as f32 / (cauldron::RISE_TICKS + 8) as f32;
    for i in 0..12 {
        let a = i as f32 * std::f32::consts::TAU / 12.0 + 0.3;
        let r = 12.0 + t * 12.0 + (i % 3) as f32;
        let (x, y) = (fx as f32 + a.cos() * r, fy as f32 + a.sin() * r * 0.42 - t * 5.0 - 1.0);
        let size = 2.2 * (1.0 - t) + 0.6;
        for py in (y - size) as i32..=(y + size) as i32 {
            for px in (x - size) as i32..=(x + size) as i32 {
                let d = (px as f32 + 0.5 - x).hypot(py as f32 + 0.5 - y) / size;
                if d <= 1.0 && bayer(px, py) < (1.0 - t) * (1.2 - d) && cv.get(px, py).is_none() {
                    cv.put(px, py, if d < 0.5 { 'j' } else { 'g' });
                }
            }
        }
    }
}

/// A small card face for the pot's ingredients and the new spell: a dark
/// card, a pewter rim (pale brass for a potent brew, old gold for a
/// perfect one), its drawing in greys.
fn card_face(card: Option<&Card>, rim: char, (w, h): (i32, i32)) -> Img {
    let mut im = Img::new(w, h);
    im.rect(1, 1, w - 2, h - 2, 'K');
    im.frame(0, 0, w, h, rim);
    for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        im.clear(x, y);
    }
    if w < 8 {
        im.put(w / 2, h / 2, 'g');
        return im;
    }
    let drawing = card.and_then(|c| super::pickup_cards::picture(super::pickup_cards::plate_for(c)));
    if let Some((pw, ph, rgba)) = drawing {
        let box_ = (w - 4).min(h - if h < 16 { 5 } else { 8 }) as f32;
        let k = box_ / (*pw).max(*ph) as f32;
        let (iw, ih) = ((*pw as f32 * k).round() as i32, (*ph as f32 * k).round() as i32);
        let (ox, oy) = ((w - iw) / 2, 2 + i32::from(h >= 16) + (box_ as i32 - ih) / 2);
        for y in 0..ih {
            for x in 0..iw {
                // The mean of what this pixel covers.
                let (x0, x1) = ((x as f32 / k) as u32, (((x + 1) as f32 / k) as u32).min(*pw));
                let (y0, y1) = ((y as f32 / k) as u32, (((y + 1) as f32 / k) as u32).min(*ph));
                let (mut lit, mut cover, mut n) = (0.0f32, 0.0f32, 0.0f32);
                for sy in y0..y1.max(y0 + 1) {
                    for sx in x0..x1.max(x0 + 1) {
                        let at = ((sy * pw + sx) * 4) as usize;
                        let a = f32::from(rgba[at + 3]) / 255.0;
                        lit += f32::from(rgba[at]) * a;
                        cover += a;
                        n += 1.0;
                    }
                }
                if cover / n < 0.4 {
                    continue;
                }
                let grey = (lit / cover * super::pickup_cards::FACE_LIFT).min(f32::from(super::pickup_cards::FACE_TOP));
                im.set(ox + x, oy + y, nearest_plain([grey, grey, grey]));
            }
        }
    }
    if h >= 16 {
        im.put(w / 2, h - 3, rim);
        im.put(w / 2 - 1, h - 3, rim);
    }
    im
}

fn rim_of(tier: Tier) -> char {
    match tier {
        Tier::Common => 'J',
        Tier::Potent => '$',
        Tier::Perfect => 'O',
    }
}

/// The two cards arcing from the brewer into the pot.
fn in_flight(cv: &mut Img, run: &Run, brew: &Brew, age: u32, (fx, fy): (i32, i32)) {
    let Some(hero) = run.players.get(&brew.knight) else {
        return;
    };
    let (hx, hy) = at(hero.x, hero.y);
    let from = (hx as f32, hy as f32 - 10.0);
    let to = (fx as f32, (fy - RIM_UP) as f32);
    for i in 0..2 {
        let start = if i == 0 { 0 } else { cauldron::SPLASH[0] };
        let end = cauldron::SPLASH[i];
        if !(start..end).contains(&age) {
            continue;
        }
        let t = (age - start) as f32 / (end - start) as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let y = from.1 + (to.1 - from.1) * t - 16.0 * (t * std::f32::consts::PI).sin();
        let face = card_face(run.book.get(&brew.cards[i]), 'J', (9, 12));
        cv.stamp(&face, x as i32 - 4, y as i32 - 6);
    }
}

/// Green flecks thrown up where a card hits the brew.
fn splashes(cv: &mut Img, run: &Run, brew: &Brew, (fx, fy): (i32, i32)) {
    let a = age(run.tick, brew.at);
    for (i, &hit) in cauldron::SPLASH.iter().enumerate() {
        if !(hit..hit + 9).contains(&a) {
            continue;
        }
        let t = (a - hit) as f32;
        for k in 0..7 {
            let ang = -std::f32::consts::PI * (0.12 + 0.76 * k as f32 / 6.0) + i as f32 * 0.2;
            let v = 1.1 + (k % 3) as f32 * 0.35;
            let x = fx as f32 + ang.cos() * v * t * 1.2;
            let y = (fy - RIM_UP) as f32 + ang.sin() * v * t * 1.6 + 0.16 * t * t;
            cv.put(x as i32, y as i32, if k % 2 == 0 { 'y' } else { 'A' });
        }
    }
}

/// The pot boils over: a cloud swelling up off the brew, lobed, lit green
/// from beneath (pale for a misfire), and eaten away as it thins.
fn puff(cv: &mut Img, (cx, cy): (i32, i32), age: u32, misfire: bool) {
    let t = age as f32 / 16.0;
    let r = 6.0 + 12.0 * t.sqrt();
    let lobes = [(0.0, 0.0, 1.0), (-0.62, 0.22, 0.78), (0.6, 0.18, 0.8), (-0.3, -0.5, 0.72), (0.34, -0.55, 0.7), (0.0, -0.85, 0.55)];
    let (core, mid, edge, fleck) = if misfire { ('c', 'h', 'J', '$') } else { ('y', 'A', 'l', 'Y') };
    for (lx, ly, k) in lobes {
        let (ox, oy) = (cx as f32 + lx * r, cy as f32 - 2.0 - 7.0 * t + ly * r * 0.75);
        let lr = r * 0.55 * k;
        for y in (oy - lr) as i32 - 1..=(oy + lr) as i32 + 1 {
            for x in (ox - lr) as i32 - 1..=(ox + lr) as i32 + 1 {
                let d = (x as f32 + 0.5 - ox).hypot((y as f32 + 0.5 - oy) * 1.1) / lr;
                if d > 1.0 || (t > 0.4 && bayer(x, y) < (t - 0.4) * 1.9 + d * 0.25) {
                    continue;
                }
                // Lit from beneath: the underside brightest.
                let under = (y as f32 + 0.5 - oy) / lr;
                let ink = if d > 0.82 {
                    edge
                } else if under > 0.25 || d < 0.35 {
                    core
                } else {
                    mid
                };
                cv.put(x, y, ink);
            }
        }
    }
    // Flecks of the brew thrown out with it.
    for k in 0..8 {
        let a = -std::f32::consts::PI * (0.1 + 0.8 * k as f32 / 7.0);
        let (x, y) = (cx as f32 + a.cos() * r * 1.15, cy as f32 - 4.0 + a.sin() * r * 0.9 + t * t * 10.0);
        cv.put(x as i32, y as i32, fleck);
    }
}

/// The new card rising out of the steam, bobbing, then dissolving.
fn rising_card(cv: &mut Img, card: Option<&Card>, tier: Tier, (cx, rim): (i32, i32), age: u32) {
    // The cloud first, then the card up through it.
    const LATE: u32 = 6;
    let rise = cauldron::RISEN - cauldron::PUFF;
    let t = (age.saturating_sub(LATE) as f32 / (rise - LATE) as f32).min(1.0);
    let eased = 1.0 - (1.0 - t).powi(3);
    let bob = if age > rise { ((age - rise) as f32 * 0.2).sin() * 1.2 } else { 0.0 };
    let (w, h) = (19, 26);
    // From in the brew to well over it.
    let top = rim + 2 - (eased * (h + 24) as f32) as i32 + bob.round() as i32;
    let face = card_face(card, rim_of(tier), (w, h));
    let left = cauldron::SHOWN - cauldron::PUFF - age;
    let fade = (left as f32 / 14.0).min(1.0);
    // The brew's light caught round its edge.
    let glow = if tier == Tier::Common { 'E' } else { 'l' };
    for y in 1..h - 1 {
        if top + y > rim - 1 {
            break;
        }
        for x in [-1, w] {
            if bayer(cx - w / 2 + x, top + y) < 0.7 * fade {
                cv.put(cx - w / 2 + x, top + y, glow);
            }
        }
    }
    for y in 0..h {
        // Still coming out of the brew: what is below the rim is hidden.
        if top + y > rim - 1 {
            break;
        }
        for x in 0..w {
            if let Some(c) = face.get(x, y)
                && bayer(cx - w / 2 + x, top + y) < fade
            {
                cv.set(cx - w / 2 + x, top + y, c);
            }
        }
    }
}

/// Confetti thrown out of the pot, fluttering down round it to lie on the
/// floor, then gone.
fn confetti(cv: &mut Img, (fx, fy): (i32, i32), age: u32, salt: u64) {
    const INKS: [char; 6] = ['7', '4', '2', 'C', 'c', 'Q'];
    let t = age as f32;
    let left = cauldron::SHOWN - cauldron::PUFF - age;
    for i in 0..56 {
        let h = hash(i, salt as i32, 37);
        // Up and out of the pot, then a slow flutter down.
        let spread = ((h % 1000) as f32 / 500.0 - 1.0) * 30.0;
        let up = 14.0 + (h / 1000 % 14) as f32;
        let ground = (h / 14000 % 16) as f32 - 4.0;
        let fly = (t / 14.0).min(1.0);
        let x = fx as f32 + spread * (1.0 - (1.0 - fly).powi(2)) + (t * 0.25 + i as f32).sin() * 1.8 * (1.0 - fly * 0.5);
        let peak = (fy - RIM_UP) as f32 - up;
        let fall = (t - 14.0).max(0.0) * 0.7;
        let y = if t < 14.0 { (fy - RIM_UP) as f32 - up * (1.0 - (1.0 - fly).powi(2)) } else { (peak + fall).min(fy as f32 + ground) };
        if left < 12 && hash(i, (age / 2) as i32, 41) % 12 >= left {
            continue;
        }
        let ink = INKS[(h % 6) as usize];
        let (x, y) = (x as i32, y as i32);
        cv.put(x, y, ink);
        // A flake turning over as it falls: now its face, now its edge.
        if (age / 4 + i as u32) % 3 != 0 {
            cv.put(x + 1, y, ink);
        }
    }
}

/// Hiccups: a bubble burps up off the knight now and then, rises, and pops.
fn hiccups(cv: &mut Img, (kx, ky): (i32, i32), age: u32) {
    if age >= cauldron::HICCUP_TICKS {
        return;
    }
    let beat = 24;
    let phase = age % beat;
    let (x, y) = (kx + 2 + (age / beat % 2) as i32 * 3, ky - 22 - phase as i32 * 2 / 3);
    if phase + 4 < beat {
        let r = 1.5 + phase as f32 / beat as f32 * 2.5;
        cv.ellipse(x as f32 + 0.5, y as f32 + 0.5, r, r, 'E');
        for k in 0..16 {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            cv.put(x + (a.cos() * r).round() as i32, y + (a.sin() * r).round() as i32, 'A');
        }
        cv.put(x - 1, y - 1, 'Y');
    } else {
        for (dx, dy) in [(-3, 0), (3, 0), (0, -3), (0, 3), (-2, -2), (2, 2), (2, -2), (-2, 2)] {
            cv.put(x + dx, y + dy, 'y');
        }
    }
}

/// The brew's one line, over its card: the new spell's name, or the misfire.
fn name_line(cv: &mut Img, brew: &Brew, (fx, fy): (i32, i32), age: u32, view: (i32, i32)) {
    let text = say(&brew.outcome.says());
    let ink = match &brew.outcome {
        Outcome::Spell { tier: Tier::Common, .. } => 'i',
        Outcome::Spell { tier: Tier::Potent, .. } => '$',
        Outcome::Spell { tier: Tier::Perfect, .. } => '6',
        Outcome::Misfire(_) => 'h',
    };
    let left = cauldron::SHOWN - cauldron::PUFF - age;
    if left < 6 {
        return;
    }
    let spell = matches!(brew.outcome, Outcome::Spell { .. });
    let w = text_width(&text) + 8;
    let px = in_view_x(fx, w, view, cv.w);
    // Over the risen card, or over the pot when no card came.
    let py = (fy - RIM_UP - if spell { 66 } else { 30 }).max(view.1 + 2);
    chrome::panel(cv, (px, py, w, 12), Tone::SLATE);
    cv.text(px + 4, py + 3, &text, ink);
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__cauldron__shots.rs"]
mod shots;
