//! The tray in a knight's bottom corner: the weapon in hand, the guard, the
//! bombs, the cards, the spells and the ultimate as small plates of pixel
//! art, each with its key under it. A plate is a rounded well (`chrome`)
//! with a rim, brass when ready and stone when not; the picture inside is
//! coloured as far up as the thing has recharged, and stone grey above
//! that, so a cooldown reads as a colour climbing the icon.
//!
//! The pictures are the cards' own art (`drive::together_shooter::tray`
//! fits them to twelve pixels), so what a knight sees here is what the card
//! looked like on the floor. One image serves the terminal's pictures, the
//! browser's frames and, one pixel to a cell, the half-block fallback.

use super::super::chrome::{self, Tone};
use super::super::ink::Img;
use super::sprites;
use crate::rules::Run;
use crate::rules::hud::Keys;
use crate::rules::tray::{Icon, SIZE, Slot};

/// A plate: the picture and its rim.
const PLATE: i32 = SIZE as i32 + 6;
/// A plate and the key beneath it.
const CELL_H: i32 = PLATE + 8;
const GAP: i32 = 2;
fn gap() -> i32 {
    if crate::render::art::enabled() {
        crate::render::art::KEY_GUTTER
    } else {
        GAP
    }
}

/// A key's label under its plate: three letters at most, so it reads as a
/// word and not a code.
fn shorten(key: &str) -> &str {
    match key {
        "Space" => "SPC",
        "Enter" => "ENT",
        other => other,
    }
}

/// One icon's plate with its key, `PLATE` wide.
pub(super) fn cell(icon: &Icon) -> Img {
    let ready = icon.fill >= 1.0;
    let mut im = Img::new(PLATE, CELL_H);
    let rim = if crate::render::art::enabled() {
        [if ready {
            crate::render::art::TRIM
        } else {
            crate::render::art::material('S')
        }; 3]
    } else {
        match (ready, icon.slot) {
            (true, Slot::Ult) => chrome::GOLD_RIM,
            (true, _) => chrome::BRASS_RIM,
            (false, _) => chrome::COLD_RIM,
        }
    };
    chrome::shaped(&mut im, (0, 0, PLATE, PLATE), &[2, 1], Tone::WELL.rimmed(rim), false);
    let art = super::fx::icon(&icon.rows());
    let grey = sprites::stone(&art);
    // In the middle of the well; a bomb a little left of it, clear of its
    // count in the corner.
    let shift = if icon.slot == Slot::Bomb { 2 } else { 0 };
    let (ox, oy) = ((PLATE - art.w) / 2 - shift, (PLATE - art.h) / 2);
    // Coloured from the bottom up as far as the thing has recharged.
    let lit = (icon.fill.clamp(0.0, 1.0) * art.h as f32).round() as i32;
    for y in 0..art.h {
        let src = if y >= art.h - lit { &art } else { &grey };
        for x in 0..art.w {
            if let Some(c) = src.get(x, y) {
                im.set(ox + x, oy + y, c);
            }
        }
    }
    // The weapon's plate: valour's pips along its top rim, gold when lit.
    if icon.slot == Slot::Weapon {
        for pip in 0..3 {
            let x = PLATE / 2 - 4 + pip * 3;
            let ink = if (pip as u32) < icon.pips { '6' } else { 'g' };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                im.put(x + dx, dy, ink);
            }
        }
    }
    // How many bombs, in the plate's top corner; the key beneath.
    if icon.slot == Slot::Bomb {
        let count = icon.count.min(9).to_string();
        chrome::shaped(&mut im, (PLATE - 7, 1, 6, 8), &[1], Tone::CHIP, false);
        im.text(PLATE - 6, 1, &count, if icon.count > 0 { 'O' } else { 'g' });
    }
    let key = shorten(&icon.key);
    let label = if icon.slot == Slot::Weapon && icon.count > 0 {
        format!("{key}+{}", icon.count)
    } else {
        key.to_string()
    };
    let ink = if icon.slot == Slot::Bomb && icon.count == 0 {
        'g'
    } else if ready {
        't'
    } else {
        'J'
    };
    let width = super::super::ink::text_width(&label);
    im.text((PLATE - width) / 2, PLATE + 1, &label, ink);
    im
}

/// The tray as one image no wider than `max_w`: icons in a row, wrapping
/// upward from the bottom when they will not fit. Empty paper around them is
/// transparent. `None` when the knight has none (fallen, or not in the party).
pub fn image(run: &Run, id: u32, keys: &Keys, max_w: i32) -> Option<Img> {
    run.players.get(&id).filter(|h| h.hp > 0)?;
    let icons = run.tray(id, keys);
    if icons.is_empty() {
        return None;
    }
    let gap = gap();
    let per_row = (((max_w + gap) / (PLATE + gap)).max(1)) as usize;
    let rows: Vec<&[Icon]> = icons.chunks(per_row).collect();
    let widest = rows
        .iter()
        .map(|r| r.len() as i32 * (PLATE + gap) - gap)
        .max()
        .unwrap_or(0);
    let mut out = Img::new(widest, rows.len() as i32 * CELL_H);
    for (n, row) in rows.iter().enumerate() {
        // The first icons sit on the bottom row.
        let y = (rows.len() - 1 - n) as i32 * CELL_H;
        let used = row.len() as i32 * (PLATE + gap) - gap;
        for (i, icon) in row.iter().enumerate() {
            out.stamp(&cell(icon), widest - used + i as i32 * (PLATE + gap), y);
        }
    }
    Some(out)
}

/// The tray laid into the bottom-right corner of a view `w` by `h`.
/// Reserve enough space for any living knight's focused tray without drawing
/// an extra image just to measure it.
pub(super) fn max_height(run: &Run, max_w: i32) -> i32 {
    let gap = gap();
    let per_row = (((max_w + gap) / (PLATE + gap)).max(1)) as usize;
    run.players
        .iter()
        .filter(|(_, h)| h.hp > 0)
        // The keys change only the labels, never how many plates there are.
        .map(|(&id, _)| run.tray(id, &Keys::FIRST).len().div_ceil(per_row) as i32 * CELL_H)
        .max()
        .unwrap_or(0)
}

/// How wide the widest living knight's tray is laid (`image`), for boards
/// that must keep clear of the corner.
pub(super) fn max_width(run: &Run, max_w: i32) -> i32 {
    let gap = gap();
    let per_row = (((max_w + gap) / (PLATE + gap)).max(1)) as usize;
    run.players
        .iter()
        .filter(|(_, h)| h.hp > 0)
        .map(|(&id, _)| run.tray(id, &Keys::FIRST).len().min(per_row) as i32 * (PLATE + gap) - gap)
        .max()
        .unwrap_or(0)
}

/// Where the tray sits in a view `w` by `h` (as `corner` lays it), for
/// boards that keep clear of it: `(x, y)` of its top-left.
pub(super) fn corner_top_left(run: &Run, w: i32, h: i32) -> (i32, i32) {
    let bottom = if crate::rules::college::is_room(run.room().kind) { 18 } else { 12 };
    (w - 14 - max_width(run, w - 8), h - bottom - max_height(run, w - 8))
}

/// The tray laid into the bottom-right corner of a view `w` by `h`.
pub(super) fn corner(over: &mut Img, run: &Run, focus: Option<u32>) {
    let id = focus
        .filter(|id| run.players.contains_key(id))
        .or_else(|| run.players.keys().next().copied());
    // A focused seat plays from its own keyboard; without one, the
    // knights share this keyboard.
    let Some(tray) = id.and_then(|id| image(run, id, Keys::of(id, focus.is_none()), over.w - 8))
    else {
        return;
    };
    // Clear of the bottom wall and the corner torch, so the keys read whole.
    let bottom = if crate::rules::college::is_room(run.room().kind) {
        18
    } else {
        12
    };
    over.stamp(&tray, over.w - tray.w - 14, over.h - tray.h - bottom);
}

/// The tray for the terminal's half-block cells, where a pixel is a cell:
/// icons shrunk to eight pixels in plates of ten, one row of as many as fit
/// in `max_w`, and the keys beneath each (to be written as text, which is
/// crisper there than a 5x7 hand at one cell a pixel): `(x, key)` with `x`
/// the plate's first column.
pub fn small(
    run: &Run,
    id: u32,
    keys: &Keys,
    max_w: i32,
) -> Option<(Img, Vec<(i32, String)>)> {
    const SMALL: usize = 8;
    const P: i32 = SMALL as i32 + 2;
    run.players.get(&id).filter(|h| h.hp > 0)?;
    let icons = run.tray(id, keys);
    let fits = (((max_w + 1) / (P + 1)).max(0) as usize).min(icons.len());
    if fits == 0 {
        return None;
    }
    let mut img = Img::new(fits as i32 * (P + 1) - 1, P);
    let mut labels = Vec::new();
    for (n, icon) in icons.iter().take(fits).enumerate() {
        let x0 = n as i32 * (P + 1);
        let ready = icon.fill >= 1.0;
        let rim = if ready { chrome::BRASS_RIM } else { chrome::COLD_RIM };
        chrome::shaped(&mut img, (x0, 0, P, P), &[1], Tone::WELL.rimmed(rim), false);
        let shrunk = crate::rules::tray::fit_to(&icon.rows(), SMALL);
        let art = super::fx::icon(&shrunk.split('/').collect::<Vec<_>>());
        let grey = sprites::stone(&art);
        let (ox, oy) = (
            x0 + 1 + (SMALL as i32 - art.w) / 2,
            1 + (SMALL as i32 - art.h) / 2,
        );
        let lit = (icon.fill.clamp(0.0, 1.0) * art.h as f32).round() as i32;
        for y in 0..art.h {
            let src = if y >= art.h - lit { &art } else { &grey };
            for x in 0..art.w {
                if let Some(c) = src.get(x, y) {
                    img.set(ox + x, oy + y, c);
                }
            }
        }
        // Cells write the key whole, and a bomb's count after a cross.
        labels.push((
            x0,
            match icon.slot {
                Slot::Bomb => format!("{} ×{}", icon.key, icon.count),
                Slot::Weapon => format!(
                    "{}{}{}{}",
                    icon.key,
                    if icon.count > 0 {
                        format!("+{}", icon.count)
                    } else {
                        String::new()
                    },
                    "◆".repeat(icon.pips.min(3) as usize),
                    "◇".repeat(3 - icon.pips.min(3) as usize)
                ),
                _ => icon.key.clone(),
            },
        ));
    }
    Some((img, labels))
}
