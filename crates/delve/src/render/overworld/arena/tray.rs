//! The skills bar along the foot of a knight's view: the weapon in hand, the guard, the
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
    cell_with_skin(icon, false)
}

/// `cell`, on the delivered HUD kit's plate when `delivered` (`hud_art`).
fn cell_with_skin(icon: &Icon, delivered: bool) -> Img {
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
    let skin = if icon.slot == Slot::Bomb && icon.count == 0 { "disabled" } else if ready { "ready" } else { "cooldown" };
    if delivered {super::hud_art::plate(&mut im, skin, (0,0,PLATE,PLATE));}
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
            let ink = if (pip as u32) < icon.pips { 'O' } else { 'g' };
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
        'O'
    } else {
        'J'
    };
    let width = super::super::ink::text_width(&label);
    im.text((PLATE - width) / 2, PLATE + 1, &label, ink);
    im
}

/// An empty socket for a spell not yet learned: a dark well in a cold rim,
/// and the key that will cast what goes in it, dim.
fn socket(key: &str) -> Img {
    let mut im = Img::new(PLATE, CELL_H);
    let rim = if crate::render::art::enabled() {
        [crate::render::art::material('S'); 3]
    } else {
        chrome::COLD_RIM
    };
    chrome::shaped(&mut im, (0, 0, PLATE, PLATE), &[2, 1], Tone::WELL.rimmed(rim), false);
    let key = shorten(key);
    let width = super::super::ink::text_width(key);
    im.text((PLATE - width) / 2, PLATE + 1, key, 'j');
    im
}

/// One place on the bar: a plate for what is held, or a spell's socket.
enum Place {
    Held(Icon),
    Socket(String),
}

impl Place {
    /// Which run of the bar it belongs to: the weapon and guard, the
    /// spells, the ultimate and bombs, the hand.
    fn group(&self) -> u8 {
        match self {
            Place::Held(icon) => match icon.slot {
                Slot::Weapon | Slot::Guard => 0,
                Slot::Spell => 1,
                Slot::Ult | Slot::Bomb => 2,
                Slot::Card => 3,
            },
            Place::Socket(_) => 1,
        }
    }

    fn image(&self, delivered: bool) -> Img {
        match self {
            Place::Held(icon) => cell_with_skin(icon, delivered),
            Place::Socket(key) => socket(key),
        }
    }
}

/// The room between two runs of the bar.
const GROUP_GAP: i32 = 6;

/// The doorway lane the bar keeps clear in the middle of the view's foot,
/// where the party comes and goes through the south door.
const LANE: i32 = 72;

/// A bar laid out: each place with its top-left, the whole's size, and
/// where its right-hand run begins (the corner's plates).
struct Bar {
    places: Vec<(Place, i32, i32)>,
    w: i32,
    h: i32,
    right: i32,
}

/// `places` in one row from `x`, a gap between runs; where the row ends.
fn row_from(places: Vec<Place>, mut x: i32, out: &mut Vec<(Place, i32, i32)>) -> i32 {
    let gap = gap();
    let mut last = None;
    for place in places {
        if last.is_some_and(|g| g != place.group()) {
            x += GROUP_GAP;
        }
        last = Some(place.group());
        out.push((place, x, 0));
        x += PLATE + gap;
    }
    x - gap
}

/// How wide `places` lie in one row.
fn row_width(places: &[Place]) -> i32 {
    let groups = places.windows(2).filter(|w| w[0].group() != w[1].group()).count() as i32;
    (places.len() as i32 * (PLATE + gap()) - gap() + groups * GROUP_GAP).max(0)
}

/// Knight `id`'s places on the bar, laid in `max_w`, the spells' four keys
/// always shown. Two runs: the weapon, guard and spells from the left, the
/// ultimate, bombs and hand from the right, the doorway lane between them
/// clear; one centred row where the two will not part; and where that
/// will not fit either, what is held alone, wrapping upward from the
/// bottom. `None` when the knight has none (fallen, or not in the party).
fn layout(run: &Run, id: u32, keys: &Keys, max_w: i32) -> Option<Bar> {
    let hero = run.players.get(&id).filter(|h| h.hp > 0)?;
    // The Wax Maze's lanes run along the room's foot: the bar keeps out.
    if super::wax_maze::tucks_tray(run) {
        return None;
    }
    let icons = run.tray(id, keys);
    if icons.is_empty() {
        return None;
    }
    let gap = gap();
    // The spells in their slots' order, a socket where a slot is empty.
    let mut places: Vec<Place> = Vec::new();
    let mut spells: Vec<Option<Icon>> = vec![None; keys.spells.len()];
    let mut rest = Vec::new();
    for icon in icons.iter().cloned() {
        match keys.spells.iter().position(|k| icon.slot == Slot::Spell && *k == icon.key) {
            Some(i) => spells[i] = Some(icon),
            None => rest.push(icon),
        }
    }
    let mut rest = rest.into_iter().peekable();
    while let Some(icon) = rest.next_if(|i| matches!(i.slot, Slot::Weapon | Slot::Guard)) {
        places.push(Place::Held(icon));
    }
    for (i, spell) in spells.into_iter().enumerate() {
        let learned = hero.spells.get(i).is_some_and(Option::is_some);
        match spell {
            Some(icon) => places.push(Place::Held(icon)),
            None if !learned => places.push(Place::Socket(keys.spells[i].to_string())),
            None => {}
        }
    }
    places.extend(rest.map(Place::Held));
    let (left, right): (Vec<Place>, Vec<Place>) = places.into_iter().partition(|p| p.group() <= 1);
    let (lw, rw) = (row_width(&left), row_width(&right));
    let mut out = Vec::new();
    if lw + LANE + rw <= max_w {
        row_from(left, 0, &mut out);
        let x = max_w - rw;
        row_from(right, x, &mut out);
        return Some(Bar { places: out, w: max_w, h: CELL_H, right: x });
    }
    let mut places = left;
    places.extend(right);
    let row = row_width(&places);
    if row <= max_w {
        row_from(places, 0, &mut out);
        return Some(Bar { places: out, w: row, h: CELL_H, right: 0 });
    }
    // Too narrow for the whole bar: what is held, wrapping upward.
    let per_row = (((max_w + gap) / (PLATE + gap)).max(1)) as usize;
    let rows: Vec<&[Icon]> = icons.chunks(per_row).collect();
    let widest = rows.iter().map(|r| r.len() as i32 * (PLATE + gap) - gap).max().unwrap_or(0);
    for (n, row) in rows.iter().enumerate() {
        // The first icons sit on the bottom row.
        let y = (rows.len() - 1 - n) as i32 * CELL_H;
        let used = row.len() as i32 * (PLATE + gap) - gap;
        for (i, icon) in row.iter().enumerate() {
            out.push((Place::Held(icon.clone()), widest - used + i as i32 * (PLATE + gap), y));
        }
    }
    Some(Bar { places: out, w: widest, h: rows.len() as i32 * CELL_H, right: 0 })
}

/// The tray as one image no wider than `max_w` (`layout`). Empty paper
/// around the plates is transparent. `None` when the knight has none
/// (fallen, or not in the party).
pub fn image(run: &Run, id: u32, keys: &Keys, max_w: i32) -> Option<Img> {
    let bar = layout(run, id, keys, max_w)?;
    let mut out = Img::new(bar.w, bar.h);
    for (place, x, y) in bar.places {
        out.stamp(&place.image(super::hud_art::enabled(run)), x, y);
    }
    Some(out)
}

/// `image`, and the span of it left free of plates, `(x0, x1)`: the lane
/// between the bar's two runs, or after a single row up to `max_w`. A
/// seat's band says the room's hold there (`party::hud_band`).
pub(super) fn image_with_free(run: &Run, id: u32, keys: &Keys, max_w: i32) -> Option<(Img, (i32, i32))> {
    let bar = layout(run, id, keys, max_w)?;
    let mut out = Img::new(bar.w, bar.h);
    let mut left_end = 0;
    for (place, x, y) in bar.places {
        if bar.right == 0 || x < bar.right {
            left_end = left_end.max(x + PLATE);
        }
        out.stamp(&place.image(super::hud_art::enabled(run)), x, y);
    }
    let free = if bar.right > 0 { (left_end, bar.right) } else { (bar.w, max_w) };
    Some((out, free))
}

/// How tall the bar's plates and keys stand: one row.
pub(super) const ROW_H: i32 = CELL_H;

/// The margin round the skills bar's plates, inside its veil.
const BAR_PAD: i32 = 3;
/// How far the plates keep in from the view's sides: clear of the room's
/// corner torches and their light.
const EDGE: i32 = 14;

/// How far the plates' keys keep up from the view's foot: on the bottom
/// wall, or in the second art kit (`modern`) clear of the mist along it.
fn foot(modern: bool) -> i32 {
    if modern || crate::render::art::enabled() { 12 } else { BAR_PAD }
}

/// The knight a seat's tray shows: its own, or the first knight when the
/// screen sits between them.
pub(super) fn shown(run: &Run, focus: Option<u32>) -> Option<u32> {
    focus
        .filter(|id| run.players.contains_key(id))
        .or_else(|| run.players.keys().next().copied())
}

/// How tall and wide knight `id`'s bar is laid in `max_w`; `None` when it
/// has none (fallen, or not in the party). The keys change only the
/// labels, never the places.
fn size_of(run: &Run, id: u32, max_w: i32) -> Option<(i32, i32)> {
    layout(run, id, &Keys::FIRST, max_w).map(|bar| (bar.w, bar.h))
}

/// Reserve enough space for any living knight's focused tray without drawing
/// an extra image just to measure it.
pub(super) fn max_height(run: &Run, max_w: i32) -> i32 {
    run.players.keys().filter_map(|&id| size_of(run, id, max_w)).map(|(_, h)| h).max().unwrap_or(0)
}

/// Where the bar's corner plates sit in a view `w` by `h` (as `corner`
/// lays them), for boards that keep clear of them: `(x, y)` of the
/// right-hand run's top-left (the whole bar's, when it does not part).
pub(super) fn corner_top_left(run: &Run, w: i32, h: i32) -> (i32, i32) {
    let max_w = w - 2 * EDGE;
    let x = run
        .players
        .keys()
        .filter_map(|&id| layout(run, id, &Keys::FIRST, max_w))
        .map(|bar| (max_w - bar.w) / 2 + bar.right)
        .min()
        .unwrap_or(max_w);
    (EDGE + x, h - foot(false) - max_height(run, max_w))
}

/// The veil the skills bar lies on in a view `w` by `h`: the view's whole
/// foot, `(x, y, w, h)`, as tall as the shown knight's plates and a margin
/// round them. `None` when no plates are drawn.
pub(super) fn bar_rect(run: &Run, focus: Option<u32>, w: i32, h: i32) -> Option<(i32, i32, i32, i32)> {
    let (_, plates) = size_of(run, shown(run, focus)?, w - 2 * EDGE)?;
    let tall = plates + BAR_PAD + foot(false);
    Some((0, h - tall, w, tall))
}

/// The skills bar along the foot of a view: the shown knight's plates in a
/// row, centred, each with the key that fires it, over the veil that
/// `bar_rect` names (the room shows through it, darkened).
pub(super) fn corner(over: &mut Img, run: &Run, focus: Option<u32>, modern: bool) {
    let id = shown(run, focus);
    // A focused seat plays from its own keyboard; without one, the
    // knights share this keyboard.
    let Some(tray) = id.and_then(|id| image(run, id, Keys::of(id, focus.is_none()), over.w - 2 * EDGE))
    else {
        return;
    };
    // The veil's top edge, a dark bevel across the view; the second art
    // kit lays no veil.
    if let Some((x, y, w, _)) = bar_rect(run, focus, over.w, over.h).filter(|_| !modern) {
        for dx in 0..w {
            over.put(x + dx, y, 'g');
        }
    }
    over.stamp(&tray, (over.w - tray.w) / 2, over.h - foot(modern) - tray.h);
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
        let skin = if icon.slot == Slot::Bomb && icon.count == 0 { "disabled" } else if ready { "ready" } else { "cooldown" };
        if super::hud_art::enabled(run) {super::hud_art::plate(&mut img, skin, (x0,0,P,P));}
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
