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

const PLATE: i32 = SIZE as i32 + 6;
const CELL_H: i32 = PLATE + 8;
const GAP: i32 = 2;

fn shorten(key: &str) -> &str {
    match key {
        "Space" => "SPC",
        "Enter" => "ENT",
        other => other,
    }
}

pub(super) fn cell(icon: &Icon) -> Img {
    cell_with_skin(icon, false)
}

fn cell_with_skin(icon: &Icon, delivered: bool) -> Img {
    let ready = icon.fill >= 1.0;
    let mut im = Img::new(PLATE, CELL_H);
    let rim = match (ready, icon.slot) {
        (true, Slot::Ult) => chrome::GOLD_RIM,
        (true, _) => chrome::BRASS_RIM,
        (false, _) => chrome::COLD_RIM,
    };
    chrome::shaped(&mut im, (0, 0, PLATE, PLATE), &[2, 1], Tone::WELL.rimmed(rim), false);
    let skin = if icon.slot == Slot::Bomb && icon.count == 0 { "disabled" } else if ready { "ready" } else { "cooldown" };
    if delivered {super::hud_art::plate(&mut im, skin, (0,0,PLATE,PLATE));}
    let art = super::fx::icon(&icon.rows());
    let grey = sprites::stone(&art);
    let shift = if icon.slot == Slot::Bomb { 2 } else { 0 };
    let (ox, oy) = ((PLATE - art.w) / 2 - shift, (PLATE - art.h) / 2);
    let lit = (icon.fill.clamp(0.0, 1.0) * art.h as f32).round() as i32;
    for y in 0..art.h {
        let src = if y >= art.h - lit { &art } else { &grey };
        for x in 0..art.w {
            if let Some(c) = src.get(x, y) {
                im.set(ox + x, oy + y, c);
            }
        }
    }
    if icon.slot == Slot::Weapon {
        for pip in 0..3 {
            let x = PLATE / 2 - 4 + pip * 3;
            let ink = if (pip as u32) < icon.pips { 'O' } else { 'g' };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                im.put(x + dx, dy, ink);
            }
        }
    }
    if let Some((share, down)) = icon.wear {
        let (x0, w) = (3, PLATE - 6);
        let lit = (share.clamp(0.0, 1.0) * w as f32).round() as i32;
        im.rect(x0, 0, w, 2, 'K');
        im.rect(x0, 0, lit, 2, if down { '7' } else { 'z' });
        if down {
            for (dx, dy) in [(0, 0), (1, 1), (2, 0), (3, 1), (4, 0)] {
                im.put(x0 + w / 2 - 2 + dx, dy, 'w');
            }
        }
    }
    if icon.high >= 4 {
        let (edge, glint) = if icon.high >= 5 { ('4', 'w') } else { ('U', 'W') };
        let mut corners = vec![(1, 1, 1, 1), (PLATE - 2, 1, -1, 1)];
        if icon.high >= 5 {
            corners.extend([(1, PLATE - 2, 1, -1), (PLATE - 2, PLATE - 2, -1, -1)]);
        }
        for (x, y, dx, dy) in corners {
            im.put(x + dx, y, edge);
            im.put(x, y + dy, edge);
            im.put(x, y, glint);
        }
    }
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

fn socket(key: &str, delivered: bool) -> Img {
    let mut im = Img::new(PLATE, CELL_H);
    let rim = chrome::COLD_RIM;
    chrome::shaped(&mut im, (0, 0, PLATE, PLATE), &[2, 1], Tone::WELL.rimmed(rim), false);
    if delivered {
        super::hud_art::plate(&mut im, "disabled", (0, 0, PLATE, PLATE));
    }
    let key = shorten(key);
    let width = super::super::ink::text_width(key);
    im.text((PLATE - width) / 2, PLATE + 1, key, 'j');
    im
}

enum Place {
    Held(Icon),
    Socket(String),
}

impl Place {
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
            Place::Socket(key) => socket(key, delivered),
        }
    }
}

const GROUP_GAP: i32 = 6;

const LANE: i32 = 72;

struct Bar {
    places: Vec<(Place, i32, i32)>,
    w: i32,
    h: i32,
    right: i32,
}

fn row_from(places: Vec<Place>, mut x: i32, out: &mut Vec<(Place, i32, i32)>) -> i32 {
    let gap = GAP;
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

fn row_width(places: &[Place]) -> i32 {
    let groups = places.windows(2).filter(|w| w[0].group() != w[1].group()).count() as i32;
    (places.len() as i32 * (PLATE + GAP) - GAP + groups * GROUP_GAP).max(0)
}

fn layout(run: &Run, id: u32, keys: &Keys, max_w: i32) -> Option<Bar> {
    let hero = run.players.get(&id).filter(|h| h.hp > 0)?;
    if super::wax_maze::tucks_tray(run) || super::powder_room::tucks_tray(run) || super::book_hop::tucks_tray(run) {
        return None;
    }
    let icons = run.tray(id, keys);
    if icons.is_empty() {
        return None;
    }
    let gap = GAP;
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
    let per_row = (((max_w + gap) / (PLATE + gap)).max(1)) as usize;
    let rows: Vec<&[Icon]> = icons.chunks(per_row).collect();
    let widest = rows.iter().map(|r| r.len() as i32 * (PLATE + gap) - gap).max().unwrap_or(0);
    for (n, row) in rows.iter().enumerate() {
        let y = (rows.len() - 1 - n) as i32 * CELL_H;
        let used = row.len() as i32 * (PLATE + gap) - gap;
        for (i, icon) in row.iter().enumerate() {
            out.push((Place::Held(icon.clone()), widest - used + i as i32 * (PLATE + gap), y));
        }
    }
    Some(Bar { places: out, w: widest, h: rows.len() as i32 * CELL_H, right: 0 })
}

pub fn image(run: &Run, id: u32, keys: &Keys, max_w: i32) -> Option<Img> {
    let bar = layout(run, id, keys, max_w)?;
    let mut out = Img::new(bar.w, bar.h);
    for (place, x, y) in bar.places {
        out.stamp(&place.image(super::hud_art::enabled(run)), x, y);
    }
    Some(out)
}

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

pub(super) fn hand_plates(run: &Run, id: u32, keys: &Keys, max_w: i32) -> Vec<(usize, (i32, i32, i32, i32))> {
    let Some(bar) = layout(run, id, keys, max_w) else {
        return Vec::new();
    };
    let slots = run.players.get(&id).map_or_else(Vec::new, |hero| {
        hero.hand
            .iter()
            .take(crate::rules::cards::HAND)
            .enumerate()
            .filter(|(_, card)| run.book.get(card.as_str()).is_some())
            .map(|(slot, _)| slot)
            .collect()
    });
    let plates = bar.places.iter().filter_map(|(place, x, y)| match place {
        Place::Held(icon) if icon.slot == Slot::Card => Some((*x, *y, PLATE, PLATE)),
        _ => None,
    });
    slots.into_iter().zip(plates).collect()
}

pub(super) const ROW_H: i32 = CELL_H;

const BAR_PAD: i32 = 3;
const EDGE: i32 = 14;

fn foot() -> i32 {
    BAR_PAD
}

pub(super) fn shown(run: &Run, focus: Option<u32>) -> Option<u32> {
    focus
        .filter(|id| run.players.contains_key(id))
        .or_else(|| run.players.keys().next().copied())
}

fn size_of(run: &Run, id: u32, max_w: i32) -> Option<(i32, i32)> {
    layout(run, id, &Keys::FIRST, max_w).map(|bar| (bar.w, bar.h))
}

pub(super) fn max_height(run: &Run, max_w: i32) -> i32 {
    run.players.keys().filter_map(|&id| size_of(run, id, max_w)).map(|(_, h)| h).max().unwrap_or(0)
}

pub(super) fn corner_top_left(run: &Run, w: i32, h: i32) -> (i32, i32) {
    let max_w = w - 2 * EDGE;
    let x = run
        .players
        .keys()
        .filter_map(|&id| layout(run, id, &Keys::FIRST, max_w))
        .map(|bar| (max_w - bar.w) / 2 + bar.right)
        .min()
        .unwrap_or(max_w);
    (EDGE + x, h - foot() - max_height(run, max_w))
}

pub(super) fn bar_rect(run: &Run, focus: Option<u32>, w: i32, h: i32) -> Option<(i32, i32, i32, i32)> {
    let (_, plates) = size_of(run, shown(run, focus)?, w - 2 * EDGE)?;
    let tall = plates + BAR_PAD + foot();
    Some((0, h - tall, w, tall))
}

pub(super) fn corner(over: &mut Img, run: &Run, focus: Option<u32>) {
    let id = shown(run, focus);
    let Some(tray) = id.and_then(|id| image(run, id, Keys::of(id, focus.is_none()), over.w - 2 * EDGE))
    else {
        return;
    };
    if let Some((x, y, w, _)) = bar_rect(run, focus, over.w, over.h) {
        for dx in 0..w {
            over.put(x + dx, y, 'g');
        }
    }
    over.stamp(&tray, (over.w - tray.w) / 2, over.h - foot() - tray.h);
}

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
