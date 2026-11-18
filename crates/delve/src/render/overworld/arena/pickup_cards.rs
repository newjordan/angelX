//! A pickup on the floor as a card standing up with its drawing on it (the
//! operator, 2026-10-18: "make sure the coins, pickups and weapons etc are
//! obvious as game pickups ... for now just make cards with art in 3D").
//!
//! Each kind of pickup has the art team's drawing (`assets/items/plates.png`,
//! made by `scripts/delve/item_plates.py`, six to a row in [`PLATES`]
//! order). In a room drawn in 3D it stands up as a card about a knight's
//! chest high, an arm taller, bobbing over the floor, over a warm pool of
//! light that marks it as something to take (docs/BRAND.md: what a knight
//! can use is marked by light). A card is drawn in greys only, held under the
//! interactable's ceiling (the operator, 2026-10-18: "desaturate the color of
//! all the pickups make them black and white, they are way too bright").

use std::sync::OnceLock;

use super::pickup_meshes::PickupKind;

const PLATE_SHEET: &[u8] = include_bytes!("../../../../assets/items/plates.png");
pub const PLATE_CELL: u32 = 96;
pub const PLATES: [&str; 36] = [
    "bow", "sword", "axe", "staff", "spear", "hammer",
    "dagger", "crossbow", "handgonne", "mace", "shield", "key",
    "spell", "flask", "food", "heart", "gold", "bone",
    "wax", "ore", "gem", "ember", "scale", "gear",
    "rune-haste", "rune-double-damage", "rune-regeneration", "rune-arcane", "rune-invisibility", "rune-illusion",
    "rune-bounty", "rune-wisdom", "chest", "pouch", "tome-fire", "tome-frost",
];

pub const LIFT: f32 = 3.0;
pub const BOB: f32 = 1.5;
pub const BOB_TICKS: f32 = 40.0;
pub const POOL: f32 = 9.0;
pub const POOL_GLOW: f32 = 0.26;
pub const GREY: f32 = 0.62;
pub const GREY_TOP: u8 = 128;

fn grey([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    let luma = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
    let v = (luma * GREY).round().min(f32::from(GREY_TOP)) as u8;
    [v, v, v, a]
}

pub fn height(kind: PickupKind) -> f32 {
    match kind {
        PickupKind::Spear | PickupKind::Staff => 20.0,
        PickupKind::Bow
        | PickupKind::Blade
        | PickupKind::Weapon
        | PickupKind::Axe
        | PickupKind::Hammer
        | PickupKind::Crossbow
        | PickupKind::Gun => 18.0,
        PickupKind::Chest => 16.0,
        _ => 14.0,
    }
}

const NAMED: [(&[&str], &str); 10] = [
    (&["dagger", "knife", "dirk"], "dagger"),
    (&["heart"], "heart"),
    (&["scroll"], "spell"),
    (&["helm", "armour", "armor", "mail", "cuirass"], "shield"),
    (&["potion", "draught", "flask", "holy-water", "elixir"], "flask"),
    (&["coin", "gold"], "gold"),
    (&["gem", "jewel", "crystal"], "gem"),
    (&["key"], "key"),
    (&["candle", "wax", "wick"], "wax"),
    (&["bone", "marrow"], "bone"),
];

pub fn plate_for(card: &crate::rules::cards::Card) -> u8 {
    let words = format!("{} {}", card.id, card.name).to_ascii_lowercase();
    let kind = super::pickup_meshes::recipe(card).kind;
    if !matches!(kind, PickupKind::Spell | PickupKind::Rune(_))
        && let Some(&(_, name)) = NAMED.iter().find(|(says, _)| says.iter().any(|w| words.contains(w)))
        && let Some(i) = PLATES.iter().position(|&p| p == name)
    {
        return i as u8;
    }
    plate_of(kind, &words)
}

pub fn plate_of(kind: PickupKind, words: &str) -> u8 {
    use crate::rules::runes::RuneKind;
    let name = match kind {
        PickupKind::Spell if ["fire", "flame", "ember", "burn", "blaze", "cinder"].iter().any(|w| words.contains(w)) => "tome-fire",
        PickupKind::Spell if ["frost", "ice", "rime", "snow", "cold", "freeze"].iter().any(|w| words.contains(w)) => "tome-frost",
        PickupKind::Spell => "spell",
        PickupKind::Ember => "ember",
        PickupKind::Rune(rune) => match rune {
            RuneKind::Haste => "rune-haste",
            RuneKind::DoubleDamage => "rune-double-damage",
            RuneKind::Regeneration => "rune-regeneration",
            RuneKind::Arcane => "rune-arcane",
            RuneKind::Invisibility => "rune-invisibility",
            RuneKind::Illusion => "rune-illusion",
            RuneKind::Bounty => "rune-bounty",
            RuneKind::Wisdom => "rune-wisdom",
        },
        PickupKind::Bow => "bow",
        PickupKind::Blade => "sword",
        PickupKind::Weapon => "mace",
        PickupKind::Axe => "axe",
        PickupKind::Staff => "staff",
        PickupKind::Spear => "spear",
        PickupKind::Hammer => "hammer",
        PickupKind::Dagger => "dagger",
        PickupKind::Crossbow => "crossbow",
        PickupKind::Gun => "handgonne",
        PickupKind::Shield => "shield",
        PickupKind::Key => "key",
        PickupKind::Gold => "gold",
        PickupKind::Gem => "gem",
        PickupKind::Ore => "ore",
        PickupKind::Gear => "gear",
        PickupKind::Chest => "chest",
        PickupKind::Pouch => "pouch",
        PickupKind::Flask => "flask",
        PickupKind::Bone => "bone",
        PickupKind::Heart => "heart",
        PickupKind::Food => "food",
        PickupKind::Wax => "wax",
        PickupKind::Scale => "scale",
    };
    PLATES.iter().position(|&p| p == name).unwrap_or(0) as u8
}

pub fn picture(index: u8) -> Option<&'static (u32, u32, Vec<u8>)> {
    static CUT: OnceLock<Vec<Option<(u32, u32, Vec<u8>)>>> = OnceLock::new();
    let cut = CUT.get_or_init(|| {
        let Ok(sheet) = image::load_from_memory_with_format(PLATE_SHEET, image::ImageFormat::Png) else {
            return Vec::new();
        };
        let sheet = sheet.to_rgba8();
        (0..PLATES.len() as u32)
            .map(|i| {
                let (cx, cy) = ((i % 6) * PLATE_CELL, (i / 6) * PLATE_CELL);
                let shown = |x: u32, y: u32| sheet.get_pixel(cx + x, cy + y)[3] >= 24;
                let (mut x0, mut y0, mut x1, mut y1) = (PLATE_CELL, PLATE_CELL, 0, 0);
                for y in 0..PLATE_CELL {
                    for x in 0..PLATE_CELL {
                        if shown(x, y) {
                            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                        }
                    }
                }
                if x1 <= x0 || y1 <= y0 {
                    return None;
                }
                let (w, h) = (x1 - x0 + 2, y1 - y0 + 2);
                let mut rgba = vec![0u8; (w * h * 4) as usize];
                for y in y0..y1 {
                    for x in x0..x1 {
                        let p = grey(sheet.get_pixel(cx + x, cy + y).0);
                        let at = (((y - y0 + 1) * w + (x - x0 + 1)) * 4) as usize;
                        rgba[at..at + 4].copy_from_slice(&p);
                    }
                }
                Some((w, h, rgba))
            })
            .collect()
    });
    cut.get(usize::from(index)).and_then(Option::as_ref)
}

pub fn place(kind: PickupKind, index: u8, (x, y): (f32, f32), centre: (f32, f32), tick: u32) -> Option<([f32; 4], f32)> {
    let &(pw, ph, _) = picture(index)?;
    let h = height(kind);
    let w = h * pw as f32 / ph.max(1) as f32;
    let lift = LIFT + BOB * (tick as f32 * std::f32::consts::TAU / BOB_TICKS).sin().abs();
    let (fx, foot) = (x - centre.0, y - centre.1);
    Some(([fx - w / 2.0, foot - lift - h, w, h], foot))
}

pub const CARD: [f32; 3] = [13.0, 18.0, 1.3];
pub const TURN_TICKS: f32 = 100.0;
pub const FACE: (u32, u32) = (52, 72);
const FACE_INK: [u8; 3] = [18, 18, 22];
const CUT_INK: [u8; 3] = [8, 8, 10];
const ICON_BOX: u32 = 38;
const ICON_TOP: u32 = 12;
pub const FACE_LIFT: f32 = 1.4;
pub const FACE_TOP: u8 = 180;

pub fn rim_tone(shine: bool) -> [u8; 3] {
    if shine { [150, 134, 98] } else { [116, 117, 122] }
}

pub fn turn(x: f32, y: f32, tick: u32) -> f32 {
    let phase = ((x as i32).wrapping_mul(7).wrapping_add((y as i32).wrapping_mul(13))).rem_euclid(16) as f32 / 16.0;
    (tick as f32 / TURN_TICKS + phase).fract() * std::f32::consts::TAU
}

pub fn lift(tick: u32) -> f32 {
    LIFT + BOB * (tick as f32 * std::f32::consts::TAU / BOB_TICKS).sin().abs()
}

fn blank(shine: bool) -> Vec<u8> {
    let (w, h) = FACE;
    let im = paper(w as i32, h as i32, shine);
    im.rgba_bytes()
}

pub(super) fn paper(w: i32, h: i32, shine: bool) -> super::super::ink::Img {
    let rim = rim_tone(shine);
    let rim_width = if w == FACE.0 as i32 { 2 } else { 1 };
    let hair = rim.map(|c| (u16::from(c) * 2 / 5) as u8);
    let mut im = super::super::ink::Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let edge = x.min(w - 1 - x).min(y).min(h - 1 - y);
            let corner = x.min(w - 1 - x) + y.min(h - 1 - y);
            let c = if corner < 2 {
                CUT_INK
            } else if edge < rim_width || corner < 4 {
                rim
            } else if edge == 4 {
                hair
            } else {
                FACE_INK
            };
            im.set(x, y, c);
        }
    }
    im
}

pub fn face(index: u8, shine: bool) -> Vec<u8> {
    let (w, _) = FACE;
    let mut px = blank(shine);
    if let Some((pw, ph, src)) = picture(index) {
        let k = ICON_BOX as f32 / (*pw).max(*ph) as f32;
        let (iw, ih) = (((*pw as f32) * k).round() as u32, ((*ph as f32) * k).round() as u32);
        let (ox, oy) = ((w - iw) / 2, ICON_TOP + (ICON_BOX - ih) / 2);
        for ty in 0..ih {
            for tx in 0..iw {
                let (x0, x1) = ((tx as f32 / k) as u32, (((tx + 1) as f32 / k).ceil() as u32).min(*pw));
                let (y0, y1) = ((ty as f32 / k) as u32, (((ty + 1) as f32 / k).ceil() as u32).min(*ph));
                let (mut lit, mut cover, mut n) = (0.0f32, 0.0f32, 0.0f32);
                for sy in y0..y1.max(y0 + 1) {
                    for sx in x0..x1.max(x0 + 1) {
                        let at = ((sy * pw + sx) * 4) as usize;
                        let a = f32::from(src[at + 3]) / 255.0;
                        lit += f32::from(src[at]) * a;
                        cover += a;
                        n += 1.0;
                    }
                }
                if cover <= 0.0 {
                    continue;
                }
                let (grey, a) = ((lit / cover * FACE_LIFT).min(f32::from(FACE_TOP)), cover / n);
                let at = (((oy + ty) * w + ox + tx) * 4) as usize;
                for c in 0..3 {
                    px[at + c] = (f32::from(px[at + c]) * (1.0 - a) + grey * a).round() as u8;
                }
            }
        }
    }
    let rim = rim_tone(shine);
    let (cx, cy) = (w as i32 / 2, (ICON_TOP + ICON_BOX + 7) as i32);
    for y in cy - 3..=cy + 3 {
        for x in cx - 3..=cx + 3 {
            if (x - cx).abs() + (y - cy).abs() <= 2 {
                let at = ((y as u32 * w + x as u32) * 4) as usize;
                px[at..at + 3].copy_from_slice(&rim);
            }
        }
    }
    px
}

pub fn back(shine: bool) -> Vec<u8> {
    let (w, h) = FACE;
    let mut px = blank(shine);
    let rim = rim_tone(shine);
    let lattice = [34, 34, 40];
    let inner = rim.map(|c| (u16::from(c) * 3 / 5) as u8);
    let (cx, cy) = (w as i32 / 2, h as i32 / 2);
    for y in 6..h as i32 - 6 {
        for x in 6..w as i32 - 6 {
            let d = (x - cx).abs() + (y - cy).abs();
            let c = if d == 11 || d == 12 {
                rim
            } else if d <= 4 {
                inner
            } else if d < 11 {
                FACE_INK
            } else if (x + y).rem_euclid(6) == 0 || (x - y).rem_euclid(6) == 0 {
                lattice
            } else {
                continue;
            };
            let at = ((y as u32 * w + x as u32) * 4) as usize;
            px[at..at + 3].copy_from_slice(&c);
        }
    }
    px
}

pub fn nodes(id: u64, front: u32, back: u32, item: &super::pickup_meshes::ScenePickup, centre: (f32, f32)) -> [angel_stage::scene::Node; 3] {
    use angel_stage::scene::{Material, MeshRef, Node, Sampling, Transform, quat_axis_angle};
    let rim = rim_tone(item.shine).map(|c| f32::from(c) / 255.0);
    let edge = Material { lit: true, emissive: 0.4, ..Material::color([rim[0] * 0.8, rim[1] * 0.8, rim[2] * 0.8, 1.0]) };
    let side = |picture: u32| Material { lit: true, emissive: 0.75, sampling: Sampling::Linear, ..Material::picture(picture) };
    let [w, h, d] = CARD;
    let a = turn(item.x, item.y, item.tick);
    let at = [item.x - centre.0, lift(item.tick) + h / 2.0, item.y - centre.1];
    let face = |id: u64, picture: u32, round: f32| {
        let (s, c) = (a + round).sin_cos();
        let mut node = Node::new(id, MeshRef::Quad, side(picture));
        node.transform = Transform {
            translation: [at[0] + s * d / 2.0, at[1], at[2] + c * d / 2.0],
            rotation: quat_axis_angle([0.0, 1.0, 0.0], a + round),
            scale: [w, h, 1.0],
        };
        node
    };
    let mut body = Node::new(id + 2, MeshRef::Cube, edge);
    body.transform = Transform {
        translation: at,
        rotation: quat_axis_angle([0.0, 1.0, 0.0], a),
        scale: [w - 0.2, h - 0.2, d * 0.8],
    };
    [face(id, front, 0.0), face(id + 1, back, std::f32::consts::PI), body]
}

pub fn turning_sprite(icon: Option<&crate::render::overworld::ink::Img>, shine: bool, (x, y): (f32, f32), tick: u32) -> crate::render::overworld::ink::Img {
    use crate::render::overworld::ink::{Img, ink, nearest_plain};
    const W: i32 = 12;
    const H: i32 = 16;
    let rim = if shine { '$' } else { 'J' };
    let a = turn(x, y, tick);
    let open = a.cos();
    let mut card = Img::new(W, H);
    card.rect(1, 1, W - 2, H - 2, 'K');
    card.frame(0, 0, W, H, rim);
    for (cx, cy) in [(0, 0), (W - 1, 0), (0, H - 1), (W - 1, H - 1)] {
        card.clear(cx, cy);
    }
    match icon {
        Some(icon) if open >= 0.0 => {
            let fit = 8.0 / icon.w.max(icon.h).max(8) as f32;
            let (iw, ih) = ((icon.w as f32 * fit).round() as i32, (icon.h as f32 * fit).round() as i32);
            let (ox, oy) = ((W - iw) / 2, 2 + (8 - ih) / 2);
            for iy in 0..ih {
                for ix in 0..iw {
                    if let Some([r, g, b]) = icon.get((ix as f32 / fit) as i32, (iy as f32 / fit) as i32) {
                        let l = (0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)) * GREY;
                        card.set(ox + ix, oy + iy, nearest_plain([l, l, l]));
                    }
                }
            }
            card.put(W / 2, H - 3, rim);
            card.put(W / 2 - 1, H - 3, rim);
        }
        _ => {
            let (cx, cy) = (W / 2, H / 2);
            for py in 2..H - 2 {
                for px in 2..W - 2 {
                    let d = (px * 2 + 1 - W).abs() / 2 + (py - cy).abs();
                    if d == 4 {
                        card.put(px, py, rim);
                    } else if d <= 1 && px >= cx - 1 {
                        card.put(px, py, 'g');
                    }
                }
            }
        }
    }
    let bob = if (tick as f32 * std::f32::consts::TAU / BOB_TICKS).sin().abs() > 0.5 { 0 } else { 1 };
    let mut out = Img::new(W, H + 2);
    let wide = ((W as f32 * open.abs()).round() as i32).max(1);
    let left = (W - wide) / 2;
    if wide <= 2 {
        let edge = ink(rim).unwrap_or([120, 120, 120]);
        for py in 1..H - 1 {
            for px in 0..wide {
                out.set(left + px, py + bob, edge);
            }
        }
        return out;
    }
    for px in 0..wide {
        let sx = ((px as f32 + 0.5) * W as f32 / wide as f32) as i32;
        for py in 0..H {
            if let Some(c) = card.get(sx.min(W - 1), py) {
                out.set(left + px, py + bob, c);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drawing_reads_and_every_kind_has_one() {
        for i in 0..PLATES.len() as u8 {
            let &(w, h, ref rgba) = picture(i).unwrap_or_else(|| panic!("{} reads", PLATES[usize::from(i)]));
            assert!(w > 20 && h > 20, "{}: {w}x{h}", PLATES[usize::from(i)]);
            assert!(rgba.chunks(4).filter(|p| p[3] > 128).count() > 300, "{} shows something", PLATES[usize::from(i)]);
            assert!(
                rgba.chunks(4).all(|p| p[0] == p[1] && p[1] == p[2] && p[0] <= GREY_TOP),
                "{} is in greys under the interactable's ceiling",
                PLATES[usize::from(i)]
            );
        }
        assert_eq!(PLATES[usize::from(plate_of(PickupKind::Gold, ""))], "gold");
        assert_eq!(PLATES[usize::from(plate_of(PickupKind::Spell, "fireball"))], "tome-fire");
        assert_eq!(PLATES[usize::from(plate_of(PickupKind::Rune(crate::rules::runes::RuneKind::Haste), ""))], "rune-haste");
        let book = crate::rules::cards::Book::builtin();
        for (id, plate) in [("fae-dagger", "dagger"), ("heart", "heart"), ("thunder-scroll", "spell"), ("iron-helm", "shield"), ("gold", "gold"), ("bow", "bow")] {
            let card = book.get(id).unwrap_or_else(|| panic!("{id} in the book"));
            assert_eq!(PLATES[usize::from(plate_for(card))], plate, "{id}");
        }
    }

    #[test]
    fn a_card_stands_on_its_floor_point_about_a_knights_chest_high() {
        let gold = plate_of(PickupKind::Gold, "");
        let ([x, y, w, h], foot) = place(PickupKind::Gold, gold, (100.0, 60.0), (90.0, 50.0), 0).unwrap();
        assert_eq!(foot, 10.0);
        assert!((x + w / 2.0 - 10.0).abs() < 1e-4, "centred on its point");
        assert!(y + h <= foot - LIFT + 1e-4 && y + h >= foot - LIFT - BOB - 1e-4, "floating over it");
        assert!(h >= 14.0 && w > 8.0);
        let sword = plate_of(PickupKind::Blade, "");
        assert!(place(PickupKind::Blade, sword, (0.0, 0.0), (0.0, 0.0), 0).unwrap().0[3] > h, "an arm stands taller");
    }

    const AT: (f32, f32) = (0.0, 0.0);

    fn gold_icon() -> crate::render::overworld::ink::Img {
        let book = crate::rules::cards::Book::builtin();
        crate::render::overworld::arena::cards::floor_plate(book.get("gold"), 0)
    }

    fn width_of(im: &crate::render::overworld::ink::Img) -> i32 {
        (0..im.w).filter(|&x| (0..im.h).any(|y| im.get(x, y).is_some())).count() as i32
    }

    #[test]
    fn a_turning_card_shows_its_face_then_its_back_and_narrows_to_an_edge_between() {
        let icon = gold_icon();
        assert_eq!(turn(AT.0, AT.1, 0), 0.0);
        let face = turning_sprite(Some(&icon), false, AT, 0);
        let quarter = turning_sprite(Some(&icon), false, AT, (TURN_TICKS / 4.0) as u32);
        let half = turning_sprite(Some(&icon), false, AT, (TURN_TICKS / 2.0) as u32);
        assert_eq!(width_of(&face), 12);
        assert_eq!(width_of(&half), 12);
        assert!(width_of(&quarter) <= 2, "side on it is a line, {} wide", width_of(&quarter));
        assert!(face.h >= 16 && face.h == half.h && face.h == quarter.h);
        let greys = |im: &crate::render::overworld::ink::Img| {
            (2..10).flat_map(|y| (2..10).map(move |x| (x, y)))
                .filter_map(|(x, y)| im.get(x, y))
                .filter(|c| {
                    let (hi, lo) = (*c.iter().max().unwrap(), *c.iter().min().unwrap());
                    hi > 40 && hi - lo <= 24
                })
                .count()
        };
        assert!(greys(&face) > 8, "the face shows its drawing ({})", greys(&face));
        assert!(greys(&face) > greys(&half) + 4, "the back is not the face ({} against {})", greys(&face), greys(&half));
        assert_ne!(face, half);
        let again = turning_sprite(Some(&icon), false, AT, TURN_TICKS as u32);
        assert_eq!(width_of(&again), 12);
    }

    #[test]
    fn a_cards_face_and_back_are_face_sized_opaque_and_the_faces_drawing_is_in_greys() {
        let (w, h) = FACE;
        for shine in [false, true] {
            let back = back(shine);
            assert_eq!(back.len(), (w * h * 4) as usize);
            assert!(back.chunks(4).all(|p| p[3] == 255), "the back is opaque");
            for index in [plate_of(PickupKind::Gold, ""), plate_of(PickupKind::Blade, ""), plate_of(PickupKind::Spell, "fireball")] {
                let face = face(index, shine);
                assert_eq!(face.len(), (w * h * 4) as usize);
                assert!(face.chunks(4).all(|p| p[3] == 255), "the face is opaque");
                assert_ne!(face, back);
                let mut bright = 0;
                for y in ICON_TOP..ICON_TOP + ICON_BOX {
                    for x in (w - ICON_BOX) / 2..(w + ICON_BOX) / 2 {
                        let p = &face[((y * w + x) * 4) as usize..][..3];
                        let (hi, lo) = (*p.iter().max().unwrap(), *p.iter().min().unwrap());
                        assert!(hi - lo <= 4, "{index}: grey at {x},{y}: {p:?}");
                        assert!(hi <= FACE_TOP, "{index}: under the face's ceiling at {x},{y}: {p:?}");
                        bright += usize::from(hi > 60);
                    }
                }
                assert!(bright > 40, "plate {index} is drawn on the face ({bright})");
            }
        }
        assert_ne!(back(true), back(false));
    }
}
