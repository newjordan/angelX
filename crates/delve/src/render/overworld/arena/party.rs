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

/// The band's height in native pixels.
pub const BAND_H: i32 = 28;
/// The HUD's band under the room's view, in native pixels: the skills bar,
/// the minimap and the room's hold (`hud_band`), so nothing lies over the
/// view. As tall as the captionless minimap and a margin.
pub const HUD_H: i32 = dungeon_map::MINI_BAND_H + 3;
/// The figure's crop a bust is cut from (head and shoulders), and how many
/// times it is enlarged, a pixel to a block.
const CROP_W: i32 = 12;
const CROP_H: i32 = 10;
const BUST_K: i32 = 2;
const BUST_W: i32 = CROP_W * BUST_K;
const BUST_H: i32 = CROP_H * BUST_K;
/// The frame round a bust: a rim, the mana down its right side, the
/// health along its foot.
const MANA_W: i32 = 2;
const HEALTH_H: i32 = 3;
const FRAME_W: i32 = 1 + BUST_W + MANA_W + 1;
const FRAME_H: i32 = 1 + BUST_H + HEALTH_H + 1;
/// A card (the frame and the name beside it), and the room between two.
const CARD_W: i32 = 92;
const CARD_GAP: i32 = 4;
/// The room between the frame and the name beside it.
const NAME_PAD: i32 = 4;
/// The band's margin at either end.
const MARGIN: i32 = 4;

/// The room between two cards, and between a frame and its name, for a band
/// of `n` cards: the full six (`MAX_PLAYERS`) squeeze both, so a name of
/// five letters still sits inside its own card.
fn spacing(n: i32) -> (i32, i32) {
    if n >= 6 { (2, 2) } else { (CARD_GAP, NAME_PAD) }
}

/// How wide each of `n` cards is: at most `CARD_W`, and all of them across
/// the band at native width, whole.
fn card_width(n: i32) -> i32 {
    let n = n.max(1);
    let (gap, _) = spacing(n);
    CARD_W.min((NATIVE_W - 2 * MARGIN - gap * (n - 1)) / n).max(FRAME_W)
}

/// Where card `i` of `n` begins in the band.
fn card_x(i: i32, n: i32) -> i32 {
    MARGIN + i * (card_width(n) + spacing(n).0)
}

/// A seat's screen at the art kit's scale: the party band over the room's
/// view, the HUD's band under it.
pub fn seat_size() -> (u32, u32) {
    let (w, h) = art_v2::size();
    let k = (w / NATIVE_W as u32).max(1);
    (w, h + (BAND_H + HUD_H) as u32 * k)
}

/// How many of a seat screen's `h` rows the party's band takes.
pub fn band_rows(h: i32) -> i32 {
    (h * BAND_H / (NATIVE_H + BAND_H + HUD_H)).max(0)
}

/// How many of a seat screen's `h` rows the HUD's band takes.
pub fn hud_rows(h: i32) -> i32 {
    (h * HUD_H / (NATIVE_H + BAND_H + HUD_H)).max(0)
}

/// How many of a seat screen's `h` rows are the room's view.
pub fn view_rows(h: i32) -> i32 {
    (h - band_rows(h) - hud_rows(h)).max(0)
}

/// The HUD's band at native size, `NATIVE_W` by `HUD_H`, under the room's
/// view: the shown knight's skills bar from the left, the floor's minimap at
/// the right end with its key beside it, and in the bar's free span a
/// kingdom door's sign or the room's hold (the party gathering, the room
/// waiting, a fellowship line).
/// Black paper; empty under a floor's plate, which holds the screen alone.
pub fn hud_band(run: &Run, focus: Option<u32>) -> Img {
    use super::super::ink::{GLYPH_ADVANCE, text_width};
    let mut out = Img::new(NATIVE_W, HUD_H);
    // A hairline over the band, where the room's view ends.
    for x in 0..NATIVE_W {
        out.put(x, 0, 'K');
    }
    if run.floor_plate(focus).is_some() {
        return out;
    }
    const SIDE: i32 = 6;
    let mut right = NATIVE_W - SIDE;
    if let Some(mini) = dungeon_map::band_minimap(run, focus) {
        let mx = NATIVE_W - MINI_W_BAND - 4;
        out.stamp(&mini, mx, (HUD_H - mini.h + 1) / 2);
        // The key that holds the whole map up, beside it.
        let cw = text_width("MAP");
        let cx = mx - 4 - cw;
        out.text(cx + (cw - text_width(dungeon_map::KEY)) / 2, HUD_H / 2 - 9, dungeon_map::KEY, 't');
        out.text(cx, HUD_H / 2 + 1, "MAP", 'J');
        right = cx - 4;
    }
    let mut free = (SIDE, right);
    if let Some(id) = tray::shown(run, focus) {
        let keys = crate::rules::hud::Keys::of(id, focus.is_none());
        if let Some((bar, (x0, x1))) = tray::image_with_free(run, id, &keys, right - SIDE) {
            out.stamp(&bar, SIDE, (HUD_H - tray::ROW_H + 1) / 2);
            free = (SIDE + x0, SIDE + x1);
        }
    }
    // At a kingdom door, its sign (what the key opens); else the room's hold.
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

/// The minimap's width in the HUD's band.
const MINI_W_BAND: i32 = dungeon_map::MINI_W;

/// The band at native size, `NATIVE_W` by `BAND_H`: the party's cards in a
/// row from the left. Black paper (transparent) under a floor's plate,
/// which holds the screen alone.
pub fn band(run: &Run, focus: Option<u32>) -> Img {
    band_of(run, focus, None)
}

/// `band`, the cards of `party` when it is given (`MapView::party`): a
/// seat's own view holds only the knights who stand in its room, and the
/// band shows every seated knight wherever they stand.
pub fn band_of(run: &Run, focus: Option<u32>, party: Option<&BTreeMap<u32, Hero>>) -> Img {
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
    // The seat's own card; between the knights (no seat), none is.
    let own = focus.filter(|id| roster.contains_key(id));
    for (i, (id, hero)) in knights.into_iter().enumerate() {
        let x = card_x(i as i32, n);
        card(&mut out, (x, 1, w, spacing(n).1), id, hero, own == Some(id), wish_of(run, id, hero));
    }
    // A hairline under the band, where the room begins.
    for x in 0..NATIVE_W {
        out.put(x, BAND_H - 1, 'K');
    }
    out
}

/// Knight `id`'s wishes, as `Run::wish_hud` reads them, for a knight of
/// the roster who may stand in another room than this view's.
fn wish_of(_run: &Run, _id: u32, hero: &Hero) -> WishHud {
    WishHud::of(hero)
}

/// One knight's card at `(x, y)`, `w` wide, the name `pad` clear of the
/// frame: the framed bust, the name beside it, and under the name what has
/// become of them, or else their wish for the room.
fn card(
    out: &mut Img,
    (x, y, w, pad): (i32, i32, i32, i32),
    id: u32,
    hero: &Hero,
    own: bool,
    wish: WishHud,
) {
    let fallen = hero.hp == 0;
    let away = hero.away || hero.stone;
    let rim = if own { chrome::BRASS_RIM } else { chrome::COLD_RIM };
    chrome::shaped(out, (x, y, FRAME_W, FRAME_H), &[1], Tone::WELL.rimmed(rim), false);
    let figure = super::knights::portrait(hero, id);
    let bust = bust(&figure);
    let bust = if fallen || away { super::sprites::stone(&bust) } else { bust };
    out.stamp(&bust, x + 1, y + 1);
    // The mana down the frame's right side, filling from its foot.
    let mana = if fallen || away { 0.0 } else { (hero.mana / MAX_MANA).clamp(0.0, 1.0) };
    let (mx, lit) = (x + 1 + BUST_W, (mana * BUST_H as f32).round() as i32);
    out.rect(mx, y + 1, MANA_W, BUST_H, 'K');
    out.rect(mx, y + 1 + BUST_H - lit, MANA_W, lit, 'z');
    // The health along its foot, in the ink of the knight's gear.
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
    // The seat's colour, a notch in the rim over the bust.
    let seat = seat_ink(id);
    for dx in FRAME_W / 2 - 3..FRAME_W / 2 + 3 {
        out.put(x + dx, y, seat);
    }
    let tx = x + FRAME_W + pad;
    let room = (x + w - tx).max(0);
    let named = hero.shown_name().to_uppercase();
    let fits = (room + 1) / super::super::ink::GLYPH_ADVANCE;
    let name: String = named.chars().take(fits.max(0) as usize).collect();
    out.text(tx, y + 3, &name, if own { 'O' } else if fallen || away { 'J' } else { 'i' });
    // The longest word is shortened where the card is too narrow for it.
    let word = if fallen {
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
        let word: String = word.chars().take(fits.max(0) as usize).collect();
        out.text(tx, y + 14, &word, ink);
    } else if wish != WishHud::Fallen {
        // The wishes held: gold while there is one to make, and the key
        // that makes it on the seat's own card when it fits. A super wish
        // is spent first, and says so.
        let n = wish.remaining();
        let (long, mid, short) = if wish.great() {
            (if own { "SUPER WISH T".to_string() } else { "SUPER WISH".to_string() }, "SUPER".to_string(), "S!".to_string())
        } else {
            (if own && n > 0 { format!("WISH {n} T") } else { format!("WISH {n}") }, format!("WISH {n}"), format!("W{n}"))
        };
        let label = if long.chars().count() as i32 <= fits {
            long
        } else if mid.chars().count() as i32 <= fits {
            mid
        } else {
            short
        };
        out.text(tx, y + 14, &label, if n > 0 { 'O' } else { 'J' });
    }
}

/// A knight's head and shoulders from their figure: `CROP_W` by `CROP_H`
/// from its top, centred on the crown (the middle of its first inked row,
/// clear of a bow or a staff held out to the side), enlarged `BUST_K`
/// times a pixel to a block.
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

/// A seat's ink, as its knight wears it.
pub(super) fn seat_ink(id: u32) -> char {
    super::knights::seat_cloth(id)
}

/// `scene` (a frame of the room's view) as a seat's whole screen: the band
/// over it, the room's parts moved down under it.
pub fn lift(scene: RoomScene, run: &Run, focus: Option<u32>) -> RoomScene {
    lift_with(scene, run, focus, &MapView::default())
}

/// `lift`, and the whole floor's map laid over the view while the seat's
/// key holds it up (`MapView`).
pub fn lift_with(mut scene: RoomScene, run: &Run, focus: Option<u32>, map: &MapView) -> RoomScene {
    let k = (scene.view.0 / NATIVE_W).max(1);
    let (rows, hud) = (BAND_H * k, HUD_H * k);
    let view_h = scene.view.1;
    let band = scaled(&band_of(run, focus, map.party.as_ref()), k);
    let mut screen = Img::new(scene.view.0, view_h + rows + hud);
    // The bands are opaque: a great hall's rows above and below the view
    // stay hidden.
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

/// `frame` (the room's view) as a seat's whole screen, `frame.w` by
/// `frame.h` plus the bands.
pub fn lift_frame(frame: &Img, run: &Run, focus: Option<u32>) -> Img {
    lift_frame_with(frame, run, focus, &MapView::default())
}

/// `lift_frame`, and the whole floor's map over the view while the seat's
/// key holds it up (`MapView`).
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

/// `im` at `k` times its size, a pixel to a block.
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
