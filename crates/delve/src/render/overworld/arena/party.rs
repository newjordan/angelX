//! The party along the top of a seat's screen: a card for each knight in
//! it, little more than a framed bust and a name. The bust is a crop of the
//! knight's own figure, head and shoulders, at twice its size; the frame
//! holds their health along its foot (in the ink of their gear) and their
//! mana down its side, rimmed in brass for the seat's own knight. A fallen
//! or away knight's bust goes grey and the card says so. Under the name,
//! a standing knight's wish for the room: WISH 1 in gold while the window
//! holds one for them, WISH 0 once spent or until a room is cleared
//! (`hud::WishHud`). The band is a strip of its own, on black paper: the
//! room's view starts below it, so nothing in the room is ever under it.

use super::super::chrome::{self, Tone};
use super::*;
use crate::rules::hud::WishHud;
use crate::rules::{Hero, MAX_MANA};
use std::collections::BTreeMap;

pub const BAND_H: i32 = 28;
pub const HUD_H: i32 = dungeon_map::MINI_BAND_H + 3;
const CROP_W: i32 = 12;
const CROP_H: i32 = 10;
const BUST_K: i32 = 2;
const BUST_W: i32 = CROP_W * BUST_K;
const BUST_H: i32 = CROP_H * BUST_K;
const MANA_W: i32 = 2;
const HEALTH_H: i32 = 3;
const FRAME_W: i32 = 1 + BUST_W + MANA_W + 1;
const FRAME_H: i32 = 1 + BUST_H + HEALTH_H + 1;
const CARD_W: i32 = 92;
const CARD_GAP: i32 = 4;
const NAME_PAD: i32 = 4;
const MARGIN: i32 = 4;

fn spacing(n: i32) -> (i32, i32) {
    if n >= 6 { (2, 2) } else { (CARD_GAP, NAME_PAD) }
}

fn card_width(n: i32) -> i32 {
    let n = n.max(1);
    let (gap, _) = spacing(n);
    CARD_W.min((NATIVE_W - 2 * MARGIN - gap * (n - 1)) / n).max(FRAME_W)
}

fn card_x(i: i32, n: i32) -> i32 {
    MARGIN + i * (card_width(n) + spacing(n).0)
}

pub fn seat_size() -> (u32, u32) {
    (NATIVE_W as u32, (NATIVE_H + BAND_H + HUD_H) as u32)
}

pub fn band_rows(h: i32) -> i32 {
    (h * BAND_H / (NATIVE_H + BAND_H + HUD_H)).max(0)
}

pub fn hud_rows(h: i32) -> i32 {
    (h * HUD_H / (NATIVE_H + BAND_H + HUD_H)).max(0)
}

pub fn view_rows(h: i32) -> i32 {
    (h - band_rows(h) - hud_rows(h)).max(0)
}

pub fn hud_band(run: &Run, focus: Option<u32>) -> Img {
    use super::super::ink::{GLYPH_ADVANCE, text_width};
    if let Some(hud) = super::deathmatch::hud_band(run, focus) { return hud; }
    let mut out = Img::new(NATIVE_W, HUD_H);
    for x in 0..NATIVE_W {
        out.put(x, 0, 'K');
    }
    if run.floor_plate(focus).is_some() {
        return out;
    }
    let mut right = NATIVE_W - SIDE;
    if let Some(mini) = dungeon_map::band_minimap(run, focus) {
        let mx = NATIVE_W - MINI_W_BAND - 4;
        out.stamp(&mini, mx, (HUD_H - mini.h + 1) / 2);
        let cw = text_width("MAP");
        let cx = mx - 4 - cw;
        out.text(cx + (cw - text_width(dungeon_map::KEY)) / 2, HUD_H / 2 - 9, dungeon_map::KEY, 't');
        out.text(cx, HUD_H / 2 + 1, "MAP", 'J');
        right = cx - 4;
        debug_assert_eq!(right, bar_right(true));
    }
    let mut free = (SIDE, right);
    if let Some(id) = tray::shown(run, focus) {
        let keys = crate::rules::hud::Keys::of(id, focus.is_none());
        if let Some((bar, (x0, x1))) = tray::image_with_free(run, id, &keys, right - SIDE) {
            out.stamp(&bar, SIDE, BAR_TOP);
            free = (SIDE + x0, SIDE + x1);
        }
        if let Some(hud) = run.hud(id, &keys)
            && let Some(corpses) = hud.corpses
        {
            let span = (free.1 - free.0 - 6).max(0);
            if span >= 30 {
                let x = free.0 + 3;
                let label = format!("CORPSES {}", corpses.state);
                let label = if text_width(&label) <= span { label } else { corpses.state };
                out.text(x, 3, &label, 'Y');
                out.rect(x, 12, span, 3, 'K');
                out.rect(x, 12, (span as f32 * corpses.fill) as i32, 3, 'C');
                let label = "HOLD G: GOLEM";
                if text_width(label) <= span { out.text(x, 18, label, 'h'); }
                if let Some(wall) = hud.skeleton_wall {
                    let key = if wall.key == "Space" { "SPC" } else { &wall.key };
                    let state = wall.state.strip_suffix(" up").unwrap_or(&wall.state).to_uppercase();
                    let label = format!("{key} WALL {state}");
                    if text_width(&label) <= span { out.text(x, 29, &label, 'i'); }
                }
                return out;
            }
        }
    }
    if let Some(sign) = kingdom::band_sign(run, focus) {
        let x = free.0 + (free.1 - free.0 - sign.w) / 2;
        out.stamp(&sign, x.max(SIDE), (HUD_H - sign.h + 1) / 2);
    } else if let Some((line, lit)) = hold_line(run, focus) {
        let room = free.1 - free.0 - 4;
        let mut lines: Vec<String> = Vec::new();
        for word in line.split_whitespace() {
            match lines.last_mut() {
                Some(last) if text_width(&format!("{last} {word}")) <= room => {
                    last.push(' ');
                    last.push_str(word);
                }
                _ => lines.push(word.to_string()),
            }
        }
        lines.truncate(3);
        let step = 9;
        let top = (HUD_H - (lines.len() as i32 * step - 2)) / 2 + 1;
        for (i, text) in lines.iter().enumerate() {
            let fits = (room + 1) / GLYPH_ADVANCE;
            let text: String = text.chars().take(fits.max(0) as usize).collect();
            let x = free.0 + (free.1 - free.0 - text_width(&text)) / 2;
            out.text(x, top + i as i32 * step, &text, if lit { 'T' } else { 'h' });
        }
    }
    out
}

const MINI_W_BAND: i32 = dungeon_map::MINI_W;
const SIDE: i32 = 6;
const BAR_TOP: i32 = (HUD_H - tray::ROW_H + 1) / 2;

fn bar_right(minimap: bool) -> i32 {
    use super::super::ink::text_width;
    if minimap {
        NATIVE_W - MINI_W_BAND - 4 - 4 - text_width("MAP") - 4
    } else {
        NATIVE_W - SIDE
    }
}

pub fn hand_plates(run: &Run, focus: Option<u32>, w: i32, h: i32) -> Vec<(usize, (i32, i32, i32, i32))> {
    if run.deathmatch_showing() { return Vec::new(); }
    if run.floor_plate(focus).is_some() {
        return Vec::new();
    }
    let Some(id) = tray::shown(run, focus) else {
        return Vec::new();
    };
    let k = (w / NATIVE_W).max(1);
    let top = h - HUD_H * k;
    let keys = crate::rules::hud::Keys::of(id, focus.is_none());
    let right = bar_right(dungeon_map::band_minimap_shown(run, focus));
    tray::hand_plates(run, id, keys, right - SIDE)
        .into_iter()
        .map(|(slot, (x, y, pw, ph))| (slot, ((SIDE + x) * k, top + (BAR_TOP + y) * k, pw * k, ph * k)))
        .collect()
}

pub fn band(run: &Run, focus: Option<u32>) -> Img {
    band_of(run, focus, None)
}

pub fn band_of(run: &Run, focus: Option<u32>, party: Option<&BTreeMap<u32, Hero>>) -> Img {
    if let Some(band) = super::deathmatch::band(run) { return band; }
    let mut out = Img::new(NATIVE_W, BAND_H);
    if run.floor_plate(focus).is_some() {
        return out;
    }
    let roster = party.unwrap_or(&run.players);
    let knights: Vec<(u32, &Hero)> = roster
        .iter()
        .filter(|(_, h)| !h.absent)
        .map(|(&id, h)| (id, h))
        .collect();
    if knights.is_empty() {
        return out;
    }
    let n = knights.len() as i32;
    let w = card_width(n);
    let own = focus.filter(|id| roster.contains_key(id));
    for (i, (id, hero)) in knights.into_iter().enumerate() {
        let x = card_x(i as i32, n);
        let unmade = hero.unmade(&run.book);
        card(&mut out, (x, 1, w, spacing(n).1), id, hero, own == Some(id), wish_of(run, id, hero), unmade, run.caves.inside(id));
    }
    for x in 0..NATIVE_W {
        out.put(x, BAND_H - 1, 'K');
    }
    out
}

fn wish_of(_run: &Run, _id: u32, hero: &Hero) -> WishHud {
    WishHud::of(hero)
}

fn struck_card(out: &mut Img, (x, y, hy, span, lit): (i32, i32, i32, i32, i32), hero: &Hero) {
    use crate::motion::flash;
    use crate::rules::spectacle::STRUCK;
    if hero.struck == 0 || hero.hp == 0 {
        return;
    }
    let age = STRUCK.saturating_sub(hero.struck);
    if flash(age, 2, 2) {
        out.rect(x + 1, hy, lit, HEALTH_H, 'w');
    }
    if age < 8 && lit > 0 && lit < span {
        out.rect(x + lit, hy, 1, HEALTH_H, if age < 4 { 'c' } else { 'V' });
    }
    let rim = match age {
        0..=2 => Some('7'),
        3..=5 => Some('8'),
        _ => None,
    };
    if let Some(ink) = rim {
        for dx in 1..FRAME_W - 1 {
            out.put(x + dx, y, ink);
            out.put(x + dx, y + FRAME_H - 1, ink);
        }
        for dy in 1..FRAME_H - 1 {
            out.put(x, y + dy, ink);
            out.put(x + FRAME_W - 1, y + dy, ink);
        }
    }
}

fn card(
    out: &mut Img,
    (x, y, w, pad): (i32, i32, i32, i32),
    id: u32,
    hero: &Hero,
    own: bool,
    wish: WishHud,
    unmade: bool,
    in_cave: bool,
) {
    let fallen = hero.hp == 0;
    let away = hero.away || hero.stone;
    let rim = if own { chrome::BRASS_RIM } else { chrome::COLD_RIM };
    chrome::shaped(out, (x, y, FRAME_W, FRAME_H), &[1], Tone::WELL.rimmed(rim), false);
    let figure = super::knights::portrait(hero, id);
    let bust = bust(&figure);
    let bust = if fallen || away { super::sprites::stone(&bust) } else { bust };
    out.stamp(&bust, x + 1, y + 1);
    let mana = if fallen || away { 0.0 } else { (hero.mana / MAX_MANA).clamp(0.0, 1.0) };
    let (mx, lit) = (x + 1 + BUST_W, (mana * BUST_H as f32).round() as i32);
    out.rect(mx, y + 1, MANA_W, BUST_H, 'K');
    out.rect(mx, y + 1 + BUST_H - lit, MANA_W, lit, 'z');
    let health = if fallen || hero.max_hp == 0 { 0.0 } else { hero.hp as f32 / hero.max_hp as f32 };
    let fill = match hero.gear {
        0 => '2',
        1 => '4',
        _ => '7',
    };
    let (hy, span) = (y + 1 + BUST_H, BUST_W + MANA_W);
    out.rect(x + 1, hy, span, HEALTH_H, 'K');
    let lit = (health.clamp(0.0, 1.0) * span as f32).ceil() as i32;
    out.rect(x + 1, hy, lit, HEALTH_H, if away { 'J' } else { fill });
    struck_card(out, (x, y, hy, span, lit), hero);
    let seat = seat_ink(id);
    for dx in FRAME_W / 2 - 3..FRAME_W / 2 + 3 {
        out.put(x + dx, y, seat);
    }
    let tx = x + FRAME_W + pad;
    let room = (x + w - tx).max(0);
    let named = hero.shown_name().to_uppercase();
    let fits = (room + 1) / super::super::ink::GLYPH_ADVANCE;
    let take = fits.max(0) as usize;
    let name: String = if named.chars().count() <= take {
        named
    } else {
        named.chars().take(take).collect()
    };
    out.text(tx, y + 3, &name, if own { 'O' } else if fallen || away { 'J' } else { 'i' });
    let word = if in_cave {
        Some((if super::super::ink::text_width("IN THE CAVE") <= room { "IN THE CAVE" } else { "IN CAVE" }, 'h'))
    } else if fallen {
        Some((if super::super::ink::text_width("FALLEN") <= room { "FALLEN" } else { "DOWN" }, '7'))
    } else if hero.away {
        Some(("AWAY", 'h'))
    } else if hero.stone {
        Some(("STONE", 'h'))
    } else if hero.vigil {
        Some(("VIGIL", 'h'))
    } else {
        None
    };
    if let Some((word, ink)) = word {
        let word: std::borrow::Cow<str> = if word.chars().count() <= take {
            std::borrow::Cow::Borrowed(word)
        } else {
            std::borrow::Cow::Owned(word.chars().take(take).collect())
        };
        out.text(tx, y + 14, &word, ink);
    } else if wish != WishHud::Fallen {
        let n = wish.remaining();
        let (long, mid, short) = if unmade && hero.wishes > 0 {
            (if own { format!("MAKE ME {n} T") } else { format!("MAKE ME {n}") }, format!("MAKE {n}"), format!("M{n}"))
        } else if wish.great() {
            (if own { "SUPER WISH T".to_string() } else { "SUPER WISH".to_string() }, "SUPER".to_string(), "S!".to_string())
        } else {
            (if own && n > 0 { format!("WISH {n} T") } else { format!("WISH {n}") }, format!("WISH {n}"), format!("W{n}"))
        };
        let label = if long.chars().count() as i32 <= fits {
            long
        } else if mid.chars().count() as i32 <= fits {
            mid
        } else if !unmade && !wish.great() && wish_pips(out, (tx, y + 15), n, own, room) {
            return;
        } else {
            short
        };
        out.text(tx, y + 14, &label, if n > 0 { 'O' } else { 'J' });
    }
}

fn wish_pips(out: &mut Img, (x, y): (i32, i32), n: u8, own: bool, room: i32) -> bool {
    const HELD: [&str; 5] = ["..t..", ".OOO.", "OOOOO", ".OOO.", "..O.."];
    const NONE: [&str; 5] = ["..J..", ".J.J.", "J...J", ".J.J.", "..J.."];
    let (rows, count) = if n == 0 { (NONE, 1) } else { (HELD, i32::from(n)) };
    let key = own && n > 0;
    let w = count * 6 - 1 + if key { 6 + super::super::ink::text_width("T") } else { 0 };
    if w > room {
        return false;
    }
    let pip = Img::from_rows(&rows);
    for k in 0..count {
        out.stamp(&pip, x + k * 6, y);
    }
    if key {
        out.text(x + count * 6 + 5, y - 1, "T", 'O');
    }
    true
}

pub(super) fn bust(figure: &Img) -> Img {
    let top = (0..figure.h).find(|&y| (0..figure.w).any(|x| figure.get(x, y).is_some())).unwrap_or(0);
    let inked: Vec<i32> = (0..figure.w).filter(|&x| figure.get(x, top).is_some()).collect();
    let crown = match (inked.first(), inked.last()) {
        (Some(a), Some(b)) => (a + b + 1) / 2,
        _ => figure.w / 2,
    };
    let x0 = crown - CROP_W / 2;
    let mut out = Img::new(BUST_W, BUST_H);
    for y in 0..BUST_H {
        for x in 0..BUST_W {
            if let Some(c) = figure.get(x0 + x / BUST_K, top + y / BUST_K) {
                out.set(x, y, c);
            }
        }
    }
    out
}

pub(super) fn seat_ink(id: u32) -> char {
    super::knights::seat_cloth(id)
}

pub fn lift(scene: RoomScene, run: &Run, focus: Option<u32>) -> RoomScene {
    lift_with(scene, run, focus, &MapView::default())
}

pub fn lift_with(mut scene: RoomScene, run: &Run, focus: Option<u32>, map: &MapView) -> RoomScene {
    let k = (scene.view.0 / NATIVE_W).max(1);
    let (rows, hud) = (BAND_H * k, HUD_H * k);
    let view_h = scene.view.1;
    let band = scaled(&band_of(run, focus, map.party.as_ref()), k);
    let mut screen = Img::new(scene.view.0, view_h + rows + hud);
    screen.stamp(&Img::black(scene.view.0, rows), 0, 0);
    screen.stamp(&band, 0, 0);
    screen.stamp(&scene.screen, 0, rows);
    if let Some(overlay) = dungeon_map::overlay(run, focus, map) {
        screen.stamp(&scaled(&overlay, k), 0, rows);
    }
    screen.stamp(&Img::black(scene.view.0, hud), 0, rows + view_h);
    screen.stamp(&scaled(&hud_band(run, focus), k), 0, rows + view_h);
    scene.screen = screen;
    scene.view.1 += rows + hud;
    scene.camera.1 -= rows as f32;
    if let Some([vx, vy, vw, vh]) = scene.veil {
        scene.veil = Some([vx, vy + rows, vw, vh]);
    }
    scene
}

pub fn lift_frame(frame: &Img, run: &Run, focus: Option<u32>) -> Img {
    lift_frame_with(frame, run, focus, &MapView::default())
}

pub fn lift_frame_with(frame: &Img, run: &Run, focus: Option<u32>, map: &MapView) -> Img {
    let k = (frame.w / NATIVE_W).max(1);
    let (rows, hud) = (BAND_H * k, HUD_H * k);
    let mut out = Img::black(frame.w, frame.h + rows + hud);
    out.stamp(&scaled(&band_of(run, focus, map.party.as_ref()), k), 0, 0);
    out.stamp(frame, 0, rows);
    if let Some(overlay) = dungeon_map::overlay(run, focus, map) {
        out.stamp(&scaled(&overlay, k), 0, rows);
    }
    out.stamp(&scaled(&hud_band(run, focus), k), 0, rows + frame.h);
    out
}

fn scaled(im: &Img, k: i32) -> Img {
    if k <= 1 {
        return im.clone();
    }
    let mut out = Img::new(im.w * k, im.h * k);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = im.get(x / k, y / k) {
                out.set(x, y, c);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__party__tests.rs"]
mod tests;
