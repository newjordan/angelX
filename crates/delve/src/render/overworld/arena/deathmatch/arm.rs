//! The knight's blade in their own hands: a drawn sword with a bevelled
//! edge, a gold guard and a gauntlet on the grip that sways as they walk,
//! kicks when it fires and swings through its three strokes.
use crate::render::overworld::ink::Img;
use crate::rules::Weapon;
use crate::rules::deathmatch::Fighter;
use crate::motion::ease;

/// The sword, point up: a blade that widens from the tip with a bright edge,
/// a fuller down its face, a gold guard, a wrapped grip, a pommel.
fn sword() -> Img {
    let (w, h) = (33, 92);
    let mut im = Img::new(w, h);
    let cx = w / 2;
    for y in 0..60 {
        let half = if y < 6 { y / 2 } else { 3 + (y - 6) / 12 }.min(7);
        for dx in -half..=half {
            let ink = if dx == -half { 'H' } else if dx == half { 'J' } else if dx == 0 && y > 8 { 'G' } else if dx < 0 { 'i' } else { 'h' };
            im.put(cx + dx, y, ink);
        }
    }
    // The guard: a gold bar with turned ends.
    im.rect(cx - 15, 60, 31, 4, 'O');
    im.rect(cx - 15, 60, 31, 1, 't');
    im.rect(cx - 15, 64, 31, 1, 'B');
    im.rect(cx - 16, 58, 3, 8, 'o');
    im.rect(cx + 14, 58, 3, 8, 'o');
    // The grip: leather, wrapped.
    for y in 65..82 {
        let ink = if y % 3 == 0 { 'B' } else { 'P' };
        im.rect(cx - 2, y, 5, 1, ink);
    }
    // The pommel.
    im.rect(cx - 4, 82, 9, 5, 'O');
    im.rect(cx - 3, 82, 4, 2, 'T');
    im.rect(cx - 2, 87, 5, 1, 'B');
    im.outline_outside('k');
    im
}

/// The great maul, head up: a banded block of iron on a long wrapped haft.
fn maul() -> Img {
    let mut im = Img::new(40, 92);
    for y in 18..90 {
        im.rect(17, y, 6, 1, if y % 4 == 0 { 'B' } else { 'P' });
    }
    for y in 0..20 {
        for x in 0..38 {
            let ink = if y == 0 { 'i' } else if x == 0 { 'h' } else if x >= 36 || y == 19 { 'g' } else if x < 5 || x >= 32 { 'O' } else { 'J' };
            im.put(1 + x, 1 + y, ink);
        }
    }
    im.rect(14, 8, 12, 2, 'j');
    im.outline_outside('k');
    im
}

/// An iron glove closed round the grip: plates with a bright top edge.
fn glove(im: &mut Img, x: i32, y: i32) {
    for (dx, dy, w, h) in [(0, 0, 20, 7), (1, 7, 18, 7), (3, 14, 14, 5)] {
        for yy in 0..h {
            for xx in 0..w {
                let ink = if yy == 0 { 'i' } else if xx == 0 { 'h' } else if xx == w - 1 || yy == h - 1 { 'g' } else { 'J' };
                im.put(x + dx + xx, y + dy + yy, ink);
            }
        }
    }
    for k in 0..4 {
        im.rect(x + 2 + k * 5, y + 2, 1, 4, 'g');
    }
}

/// Whether this knight's arm is drawn as the sword (the arms with their own
/// look keep it): the bare sword, or a forged blade that is not a maul,
/// cleaver or flask rack.
pub(super) fn is_sword(f: &Fighter) -> bool {
    look(f).is_some()
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Sword,
    Maul,
}
fn look(f: &Fighter) -> Option<Look> {
    let h = &f.hero;
    match &h.forged {
        Some(w) if h.arm.as_deref() == Some("great-maul") => w.melee.is_some().then_some(Look::Maul),
        Some(w) => (w.melee.is_some() && !matches!(h.arm.as_deref(), Some("cleaver" | "flask-bandolier"))).then_some(Look::Sword),
        None => (!matches!(h.weapon, Weapon::Bow | Weapon::Crossbow | Weapon::Handgonne | Weapon::Fists)).then_some(Look::Sword),
    }
}

/// Draw `src` turned by `angle` (clockwise, radians) about its `pivot`, with
/// the pivot at `at`: the nearest pixel, so it stays pixel art.
fn blit_turned(im: &mut Img, src: &Img, pivot: (f32, f32), at: (i32, i32), angle: f32) {
    let r = src.w.max(src.h) + 2;
    let (s, c) = angle.sin_cos();
    for dy in -r..=r {
        for dx in -r..=r {
            let (fx, fy) = (dx as f32, dy as f32);
            let (sx, sy) = (c * fx + s * fy + pivot.0, -s * fx + c * fy + pivot.1);
            if let Some(col) = src.get(sx.round() as i32, sy.round() as i32) {
                im.set(at.0 + dx, at.1 + dy, col);
            }
        }
    }
}

/// The sword in hand: resting a little to the right and swaying with the
/// walk; the stroke (`swing`: its combo 1 to 3 and ticks left of nine)
/// sweeps it across, back, then down.
pub(super) fn draw(im: &mut Img, f: &Fighter, swing: Option<(u8, u8)>) {
    let maul_in_hand = look(f) == Some(Look::Maul);
    let blade = if maul_in_hand { maul() } else { sword() };
    let sway = (f.bob.sin() * 3.0) as i32;
    let kick = if f.flash > 0 { 5 } else { 0 };
    let (mut angle, mut dx, mut dy) = (0.2 + (f.bob * 0.5).cos() * 0.02, 0, 0);
    if let Some((combo, left)) = swing {
        let t = ease::out((9 - left.min(9)) as f32 / 8.0);
        match combo {
            1 => angle = 1.0 - 2.1 * t,
            2 => angle = -1.0 + 2.1 * t,
            _ => {
                angle = 0.5 - 0.9 * t;
                dy = (28.0 * ease::in_quad(t)) as i32 - 8;
            }
        }
        dx = (angle * 14.0) as i32;
    }
    let at = (222 + dx, 180 + sway + kick + dy);
    blit_turned(im, &blade, (if maul_in_hand { 20.0 } else { 16.0 }, 86.0), at, angle);
    glove(im, at.0 - 10, at.1 - 20);
}
