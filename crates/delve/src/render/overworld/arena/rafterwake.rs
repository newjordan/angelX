//! The bellwright's loft, baked at one native pixel grain.
use super::super::{
    chrome,
    ink::{Img, ink, text_width},
};
use super::{NATIVE_H, NATIVE_W};
use crate::rules::{
    HZ, Run,
    rafterwake::{Beat, GOLD_PER_SEAT, LENGTH, Q, TOP, ladder, planks},
};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};
#[derive(serde::Deserialize)]
struct Recipe {
    inks: BTreeMap<char, [u8; 3]>,
    assets: BTreeMap<String, Asset>,
}
#[derive(serde::Deserialize)]
struct Asset {
    rows: Vec<String>,
    #[serde(default)]
    damage_variants: Vec<Asset>,
    #[serde(default)]
    linear_features: Vec<Linear>,
}
#[derive(serde::Deserialize)]
struct Linear {
    name: String,
    ink: char,
    pixels: Vec<[i32; 2]>,
}
struct KeeperArt {
    body: Img,
    details: Img,
}
struct Art {
    all: BTreeMap<String, Img>,
    knight: [Img; 10],
    keeper: [[KeeperArt; 12]; 3],
    tells: [[[KeeperArt; 4]; 2]; 3],
    damage_actions: BTreeMap<(usize, &'static str), KeeperArt>,
}
pub(super) fn image(rows: &[String], inks: &BTreeMap<char, [u8; 3]>) -> Img {
    let mut im = Img::new(rows[0].len() as i32, rows.len() as i32);
    for (y, row) in rows.iter().enumerate() {
        for (x, k) in row.chars().enumerate() {
            if k != '.' {
                im.set(x as i32, y as i32, inks[&k]);
            }
        }
    }
    im
}
fn linear_image(asset: &Asset, inks: &BTreeMap<char, [u8; 3]>) -> Img {
    let mut im = Img::new(64, 80);
    for feature in &asset.linear_features {
        if feature.name.contains("fracture") || feature.name.contains("dented") {
            continue;
        }
        for &[x, y] in &feature.pixels {
            if asset.rows[y as usize].as_bytes()[x as usize] == feature.ink as u8 {
                im.set(x, y, inks[&feature.ink]);
            }
        }
    }
    im
}
/// Optional source art is decoded, cleaned and registered only while baking a bank.
pub(super) fn slot_image(slot: &str, key: &str, fallback: &Img) -> Img {
    if !super::slot_art::enabled() {
        return fallback.clone();
    }
    let Some(bytes) = super::slot_art::bytes(slot, key) else {
        return fallback.clone();
    };
    let png = ::image::load_from_memory(bytes)
        .expect("reviewed slot PNG")
        .to_rgba8();
    let (width, height, ox, oy) = if slot == "pennon.captain" && png.width() == 128 {
        (64, 80, 32, 80)
    } else {
        (png.width(), png.height(), 0, 0)
    };
    let mut out = Img::new(width as i32, height as i32);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] != 0 {
            let rgb = [p[0], p[1], p[2]];
            // A restrained material highlight replaces white-hot source cores.
            let rgb = if rgb.iter().copied().max().unwrap() > 230 {
                if p[2] > p[0] {
                    ink('z').unwrap()
                } else {
                    ink('t').unwrap()
                }
            } else {
                rgb
            };
            out.set(x as i32 - ox, y as i32 - oy, rgb);
        }
    }
    if slot == "rafterwake.plank" {
        let mut supported = fallback.clone();
        supported.stamp(&out, 0, 0);
        return supported;
    }
    out
}
#[derive(serde::Deserialize)]
pub(super) struct Presentation {
    pub compact_hud: bool,
    pub hint_seconds: u32,
    keeper_scale: [i32; 2],
    pennon_scale: [i32; 2],
    pennon_sky_crop: i32,
    cell_backdrop_percent: u16,
    danger_ramp: [char; 4],
    boss_rim: char,
}
pub(super) fn presentation() -> &'static Presentation {
    static PROFILE: OnceLock<Presentation> = OnceLock::new();
    PROFILE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/art-slots/rafterwake.chrome/presentation.json"
        ))
        .expect("reviewed display profile")
    })
}
#[derive(serde::Deserialize)]
struct Materials {
    cloth: Vec<[i32; 4]>,
    crest: [i32; 4],
}
fn seat_ink(colours: &[(char, char)]) -> char {
    colours
        .iter()
        .find(|(f, _)| *f == '7')
        .map_or('7', |(_, t)| *t)
}
fn seat_ramp(colours: &[(char, char)]) -> [char; 4] {
    match seat_ink(colours) {
        '1' => ['0', 'q', '1', 'z'],
        'A' => ['D', 'l', 'A', 'C'],
        '4' => ['I', 'O', '4', 't'],
        _ => ['8', '7', '7', 'R'],
    }
}
/// Reviewed fabric regions keep the blue/ochre tabard and scarf separate from gear.
pub(super) fn slot_knight_pose(
    base: &Img,
    colours: &[(char, char)],
    ochre: bool,
    pose: usize,
) -> Img {
    static MASKS: OnceLock<[Vec<Materials>; 2]> = OnceLock::new();
    let masks = MASKS.get_or_init(|| {
        [
            serde_json::from_str(include_str!(
                "../../../../assets/art-slots/pennon.knight/seat-materials.json"
            ))
            .unwrap(),
            serde_json::from_str(include_str!(
                "../../../../assets/art-slots/rafterwake.knight/seat-materials.json"
            ))
            .unwrap(),
        ]
    });
    let m = &masks[usize::from(ochre)][pose];
    let inside = |r: &[i32; 4], x, y| x >= r[0] && y >= r[1] && x < r[2] && y < r[3];
    let ramp = seat_ramp(colours).map(|c| ink(c).unwrap());
    let mut out = base.clone();
    for y in 0..base.h {
        for x in 0..base.w {
            if let Some([r, g, b]) = base.get(x, y) {
                let blue = u16::from(b) > u16::from(r) + 18 && u16::from(b) > u16::from(g) + 8;
                let fabric = m.cloth.iter().any(|region| inside(region, x, y))
                    && (blue
                        || (g >= 90
                            && r > g
                            && u16::from(g) * 5 > u16::from(r) * 3
                            && u16::from(g) > u16::from(b) * 3 / 2));
                let crest = inside(&m.crest, x, y)
                    && if ochre {
                        r > 75 && u16::from(r) > u16::from(g) * 2 && b < 65
                    } else {
                        g > 75 && r > g && u16::from(g) > u16::from(b) * 3 / 2
                    };
                if fabric || crest {
                    let shade = usize::from(r.max(g).max(b).saturating_sub(45) / 55).min(3);
                    out.set(x, y, ramp[shade.max(1)]);
                }
            }
        }
    }
    out
}
#[cfg(test)]
pub(super) fn slot_knight(base: &Img, colours: &[(char, char)], ochre: bool) -> Img {
    slot_knight_pose(base, colours, ochre, 0)
}
/// Nearest source pixels, with a rational display scale and an unchanged foot pivot.
fn scale(source: &Img, ratio: [i32; 2]) -> Img {
    let [n, d] = ratio;
    let mut out = Img::new(source.w * n / d, source.h * n / d);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = source.get(x * d / n, y * d / n) {
                out.set(x, y, c);
            }
        }
    }
    out
}
pub(super) fn danger_colour(stage: usize) -> [u8; 3] {
    ink(presentation().danger_ramp[stage.min(3)]).unwrap()
}
pub(super) fn danger_art(source: &Img) -> Img {
    let mut out = source.clone();
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = source.get(x, y) {
                out.set(
                    x,
                    y,
                    danger_colour(usize::from(c.into_iter().max().unwrap()) / 64),
                );
            }
        }
    }
    out
}
/// Six inks per actor; the seat rim is four native pixels (one 96-column cell).
/// Cached copies avoid classifying or growing silhouettes during a live frame.
pub(super) fn cell_actor(source: &Img, colours: Option<&[(char, char)]>) -> Img {
    type Entry = (u64, Option<char>, Img);
    static CACHE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    let key = (super::slot_art::bank_hash(source), colours.map(seat_ink));
    let mut cache = CACHE.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap();
    if let Some((_, _, im)) = cache.iter().find(|(hash, seat, _)| (*hash, *seat) == key) {
        return im.clone();
    }
    let rim = ink(colours.map_or(presentation().boss_rim, seat_ink)).unwrap();
    let mut im = Img::new(source.w, source.h);
    for y in 0..source.h {
        for x in 0..source.w {
            if let Some(c) = source.get(x, y) {
                let v = (u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2])) / 10;
                let colour = if colours.is_some() && c == rim {
                    rim
                } else if v < 40 {
                    ink('K').unwrap()
                } else if c[0] > c[1] && c[0] - c[1] > 24 {
                    ink('o').unwrap()
                } else if v < 95 {
                    ink('S').unwrap()
                } else if v < 145 {
                    ink('U').unwrap()
                } else {
                    ink('v').unwrap()
                };
                im.set(x, y, colour);
            } else {
                let radius: i32 = if colours.is_some() { 4 } else { 2 };
                if (-radius..=radius).any(|dy| {
                    (-radius..=radius).any(|dx| {
                        dx.abs().max(dy.abs()) <= radius && source.get(x + dx, y + dy).is_some()
                    })
                }) {
                    im.set(x, y, rim);
                }
            }
        }
    }
    super::slot_art::record_bank(source, &im);
    cache.push((key.0, key.1, im.clone()));
    im
}
pub(super) fn cell_backdrop(source: &Img) -> Img {
    let mut out = source.clone();
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = source.get(x, y) {
                out.set(
                    x,
                    y,
                    c.map(|v| (u16::from(v) * presentation().cell_backdrop_percent / 100) as u8),
                );
            }
        }
    }
    out
}
/// The cell writer preserves its background under text, so reserve whole glyph cells.
pub(super) fn label_wells(im: &mut Img, fx: &mut Img, labels: &[(i32, i32, String)]) {
    for (x, y, text) in labels {
        let (x, y) = (x / 4 * 4, y / 8 * 8);
        im.rect(x, y, text.chars().count() as i32 * 4, 8, 'k');
        fx.rect(x, y, text.chars().count() as i32 * 4, 8, 'k');
    }
}
pub(super) fn present_stage(im: &Img, fx: &Img, pennon: bool) -> (Img, Img) {
    let mut base = Img::black(NATIVE_W, NATIVE_H);
    let mut detail = Img::new(NATIVE_W, NATIVE_H);
    let (crop, ratio, x) = if pennon {
        (
            presentation().pennon_sky_crop,
            presentation().pennon_scale,
            20,
        )
    } else {
        (0, [1, 1], 65)
    };
    let h = if pennon { 144 - crop } else { 160 };
    for (dest, source) in [(&mut base, im), (&mut detail, fx)] {
        let stage = Img::from_view(source, 12, 38 + crop, 254, h);
        dest.stamp(&scale(&stage, ratio), x, 48);
    }
    (base, detail)
}

/// Decorative attack pixels stay inside the authoritative danger rectangle.
pub(super) fn attack_stamp(
    im: &mut Img,
    fx: &mut Img,
    pic: &Img,
    x: i32,
    y: i32,
    area: (i32, i32, i32, i32),
    mask: bool,
) {
    let (ax, ay, aw, ah) = area;
    for yy in 0..pic.h {
        for xx in 0..pic.w {
            let (px, py) = (x + xx, y + yy);
            if px >= ax && px < ax + aw && py >= ay && py < ay + ah {
                if let Some(c) = pic.get(xx, yy) {
                    im.set(px, py, c);
                    if mask {
                        fx.set(px, py, [0, 0, 0]);
                    }
                }
            }
        }
    }
}

fn slot_for_key(key: &str) -> &str {
    if key.starts_with("knight-") {
        "rafterwake.knight"
    } else if key.starts_with("keeper-") {
        "rafterwake.keeper"
    } else if key.starts_with("fx-drop-") {
        "rafterwake.drop"
    } else if key.starts_with("fx-sweep-") {
        "rafterwake.sweep"
    } else if key.starts_with("fx-dust-") {
        "rafterwake.dust"
    } else {
        match key {
            "plank" => "rafterwake.plank",
            "ladder" => "rafterwake.ladder",
            "post" => "rafterwake.post",
            "brick" => "rafterwake.brick",
            _ => "",
        }
    }
}
/// Collapse a source plate to its alpha bounds, then preserve its corners.
pub(super) fn plate(source: &Img, width: i32, height: i32) -> Img {
    let (mut x0, mut y0, mut x1, mut y1) = (source.w, source.h, 0, 0);
    for y in 0..source.h {
        for x in 0..source.w {
            if source.get(x, y).is_some() {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    let mut out = Img::new(width, height);
    if x1 <= x0 || y1 <= y0 {
        return out;
    }
    let (sw, sh) = (x1 - x0, y1 - y0);
    let source_edge = (sw.min(sh) / 6).max(1);
    let edge = 3;
    let coord = |v: i32, dest: i32, src: i32| {
        if v < edge {
            v * source_edge / edge
        } else if v >= dest - edge {
            src - (dest - v) * source_edge / edge
        } else {
            source_edge + (v - edge) * (src - 2 * source_edge) / (dest - 2 * edge)
        }
    };
    for y in 0..height {
        for x in 0..width {
            if let Some(c) = source.get(x0 + coord(x, width, sw), y0 + coord(y, height, sh)) {
                out.set(x, y, c);
            }
        }
    }
    out
}
pub(super) fn slot_chrome(slot: &str, podium: bool, chain: bool) -> &'static Img {
    static SKINS: OnceLock<[Img; 8]> = OnceLock::new();
    let bank = SKINS.get_or_init(|| {
        let mut sources = BTreeMap::new();
        std::array::from_fn(|n| {
            let slot = if n / 4 == 0 {
                "rafterwake.chrome"
            } else {
                "pennon.chrome"
            };
            let mut im = Img::new(NATIVE_W, NATIVE_H);
            let panels: Vec<_> = if n % 4 >= 2 {
                vec![("podium", 16, 16, 352, 202)]
            } else if n % 2 == 1 {
                if presentation().compact_hud {
                    vec![("chain", 300, 16, 76, 16)]
                } else {
                    vec![("chain", 10, 17, 80, 21)]
                }
            } else {
                if presentation().compact_hud {
                    vec![("header", 4, 0, 376, 46)]
                } else {
                    vec![("header", 6, 0, 372, 18), ("panel", 276, 28, 108, 184)]
                }
            };
            for (key, x, y, w, h) in panels {
                let src = sources
                    .entry((slot, key))
                    .or_insert_with(|| slot_image(slot, key, &Img::new(1, 1)));
                let mut skin = plate(src, w, h);
                if src.w > 1 {
                    skin.rect(3, 3, w - 6, h - 6, if key == "podium" { 's' } else { 'k' });
                }
                im.stamp(&skin, x, y);
            }
            im
        })
    });
    &bank[usize::from(slot == "pennon.chrome") * 4 + usize::from(podium) * 2 + usize::from(chain)]
}

fn keeper_display(source: &Img) -> Img {
    if !super::slot_art::enabled() {
        return source.clone();
    }
    let mut im = source.clone();
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = source.get(x, y) {
                if u16::from(b) > u16::from(r) + 18 && b > g {
                    im.set(x, y, ink(if b > 100 { 'U' } else { 'S' }).unwrap());
                }
            }
        }
    }
    scale(&im, presentation().keeper_scale)
}
fn art() -> &'static Art {
    static ART: [OnceLock<Art>; 2] = [const { OnceLock::new() }; 2];
    ART[usize::from(super::slot_art::enabled())].get_or_init(|| {
        let recipe: Recipe =
            serde_json::from_str(include_str!("../../../../assets/rafterwake/native.json"))
                .expect("Rafterwake native recipe");
        let mut details = BTreeMap::new();
        let mut all: BTreeMap<String, Img> = recipe
            .assets
            .into_iter()
            .flat_map(|(n, s)| {
                details.insert(n.clone(), linear_image(&s, &recipe.inks));
                let mut variants = vec![(n.clone(), image(&s.rows, &recipe.inks))];
                for (stage, variant) in s.damage_variants.iter().enumerate() {
                    details.insert(
                        format!("{n}-damage-{}", stage + 1),
                        linear_image(variant, &recipe.inks),
                    );
                    variants.push((
                        format!("{n}-damage-{}", stage + 1),
                        image(&variant.rows, &recipe.inks),
                    ));
                }
                variants
            })
            .collect();
        let mut decoded = BTreeMap::new();
        for (key, body) in &mut all {
            let slot = slot_for_key(key);
            let mut source_key = key.clone();
            if key.starts_with("keeper-") {
                if let Some((base, _)) = key.rsplit_once("-damage-")
                    && base != "keeper"
                {
                    source_key = base.into();
                }
                if let Some((base, stage)) = source_key.rsplit_once('-') {
                    if base.ends_with("-tell") && stage.parse::<u8>().is_ok() {
                        source_key = base.into();
                    }
                }
            }
            let selected =
                super::slot_art::enabled() && super::slot_art::bytes(slot, &source_key).is_some();
            let imported = if selected {
                decoded
                    .entry((slot.to_owned(), source_key.clone()))
                    .or_insert_with(|| slot_image(slot, &source_key, body))
                    .clone()
            } else {
                body.clone()
            };
            if selected {
                assert_eq!((body.w, body.h), (imported.w, imported.h));
                details.insert(key.clone(), Img::new(body.w, body.h));
            }
            *body = if selected && (key.starts_with("fx-sweep-") || key.starts_with("fx-drop-")) {
                danger_art(&imported)
            } else {
                imported
            };
        }
        let knight = std::array::from_fn(|i| {
            all[&format!(
                "knight-{}",
                [
                    "idle", "run-1", "run-2", "jump", "fall", "land", "climb-1", "climb-2", "hit",
                    "victory"
                ][i]
            )]
                .clone()
        });
        let keeper = std::array::from_fn(|stage| {
            std::array::from_fn(|i| {
                let key = format!(
                    "keeper-{}",
                    [
                        "idle",
                        "walk-1",
                        "walk-2",
                        "drop-tell",
                        "drop",
                        "sweep-tell",
                        "sweep",
                        "flinch",
                        "damage-1",
                        "damage-2",
                        "defeat",
                        "recover"
                    ][i]
                );
                let key = if stage > 0 && !matches!(i, 8..=10) {
                    format!("{key}-damage-{stage}")
                } else {
                    key
                };
                KeeperArt {
                    body: keeper_display(&all[&key]),
                    details: if super::slot_art::enabled() {
                        scale(&details[&key], presentation().keeper_scale)
                    } else {
                        details[&key].clone()
                    },
                }
            })
        });
        let tells = std::array::from_fn(|damage| {
            ["drop", "sweep"].map(|attack| {
                std::array::from_fn(|stage| {
                    let key = format!("keeper-{attack}-tell-{stage}");
                    let key = if damage > 0 {
                        format!("{key}-damage-{damage}")
                    } else {
                        key
                    };
                    KeeperArt {
                        body: keeper_display(&all[&key]),
                        details: if super::slot_art::enabled() {
                            scale(&details[&key], presentation().keeper_scale)
                        } else {
                            details[&key].clone()
                        },
                    }
                })
            })
        });
        let mut damage_actions = BTreeMap::new();
        for (stage, state, name) in [
            (1, "drop-tell", "stage-1-tell-a"),
            (1, "drop", "stage-1-attack-a"),
            (1, "drop-recover", "stage-1-recover-a"),
            (1, "sweep-tell", "stage-1-tell-b"),
            (1, "sweep", "stage-1-attack-b"),
            (1, "sweep-recover", "stage-1-recover-b"),
            (1, "idle", "stage-1-recover-a"),
            (2, "idle", "stage-2-idle"),
            (2, "drop-tell", "stage-2-tell-a"),
            (1, "flinch", "strong-flinch"),
            (2, "flinch", "strong-flinch"),
            (2, "defeat-first", "defeat-1"),
            (2, "defeat", "defeat-2"),
        ] {
            if super::slot_art::enabled()
                && super::slot_art::bytes("rafterwake.keeper-damaged", name).is_some()
            {
                let source = slot_image("rafterwake.keeper-damaged", name, &all["keeper-idle"]);
                let body = keeper_display(&source);
                let details = Img::new(body.w, body.h);
                damage_actions.insert((stage, state), KeeperArt { body, details });
            }
        }
        Art {
            all,
            knight,
            keeper,
            tells,
            damage_actions,
        }
    })
}
pub(super) fn flip(im: &Img) -> Img {
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                out.set(im.w - 1 - x, y, c);
            }
        }
    }
    out
}
pub(super) fn recolour(mut im: Img, colours: &[(char, char)]) -> Img {
    let from = ['I', 'r', 'o', 't'];
    let target = colours
        .iter()
        .find(|(f, _)| *f == '7')
        .map_or('7', |(_, t)| *t);
    let ramp = match target {
        '1' => ['x', 'q', 'Q', 'z'],
        'A' => ['D', 'F', 'l', 'C'],
        '4' => ['I', 'r', 'o', 't'],
        _ => ['b', 'p', 'R', 'o'],
    };
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                if let Some(i) = from.iter().position(|k| ink(*k) == Some(c)) {
                    im.set(x, y, ink(ramp[i]).unwrap());
                }
            }
        }
    }
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                for &(from, to) in colours {
                    if ink(from) == Some(c) {
                        if let Some(colour) = ink(to) {
                            im.set(x, y, colour);
                        }
                        break;
                    }
                }
            }
        }
    }
    im
}
pub(super) fn outline(source: &Img, contour: [u8; 3]) -> Img {
    let mut im = source.clone();
    for y in 0..source.h {
        for x in 0..source.w {
            if source.get(x, y).is_none()
                && [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                    .iter()
                    .any(|&(x, y)| source.get(x, y).is_some())
            {
                im.set(x, y, contour);
            }
        }
    }
    im
}
struct SeatArt {
    imported: bool,
    colours: Vec<(char, char)>,
    poses: [[Img; 10]; 2],
    outlined: [[Img; 10]; 2],
}
fn knight(run: &Run, id: u32, pose: usize, dir: i32, outlined: bool) -> Img {
    let h = &run.players[&id];
    let colours = if h.colours().is_empty() {
        super::knights::seat_colours(id)
    } else {
        h.colours()
    };
    static SEATS: OnceLock<Mutex<Vec<SeatArt>>> = OnceLock::new();
    let mut seats = SEATS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap();
    let at = seats
        .iter()
        .position(|s| s.colours == colours && s.imported == super::slot_art::enabled())
        .unwrap_or_else(|| {
            let right = std::array::from_fn(|p| {
                let names = [
                    "idle", "run-1", "run-2", "jump", "fall", "land", "climb-1", "climb-2", "hit",
                    "victory",
                ];
                let key = format!("knight-{}", names[p]);
                let base = &art().knight[p];
                if super::slot_art::enabled()
                    && super::slot_art::bytes("rafterwake.knight", &key).is_some()
                {
                    slot_knight_pose(base, colours, true, p)
                } else {
                    recolour(base.clone(), colours)
                }
            });
            let left = std::array::from_fn(|p| flip(&right[p]));
            let seat_ink = colours
                .iter()
                .find(|(from, _)| *from == '7')
                .map_or('7', |(_, to)| *to);
            let contour = ink(seat_ink).unwrap();
            let outlines =
                [&right, &left].map(|poses| std::array::from_fn(|p| outline(&poses[p], contour)));
            seats.push(SeatArt {
                imported: super::slot_art::enabled(),
                colours: colours.to_vec(),
                poses: [right, left],
                outlined: outlines,
            });
            seats.len() - 1
        });
    let poses = if outlined {
        &seats[at].outlined
    } else {
        &seats[at].poses
    };
    poses[usize::from(dir < 0)][pose].clone()
}
pub(super) fn contour_colour(run: &Run, focus: Option<u32>) -> Option<[u8; 3]> {
    // The cached cell rim already occupies a full cell; subpixel promotion floods the body.
    if super::slot_art::enabled() {
        return None;
    }
    if !run.rafterwake_showing() {
        return None;
    }
    let s = run.rafterwake.as_ref()?;
    if s.left == 0 {
        return None;
    }
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next()?);
    let d = &s.lanes[&id];
    if (d.x / Q - d.keeper.x).abs() >= 48 || (d.y / Q - TOP).abs() >= 80 {
        return None;
    }
    let h = &run.players[&id];
    let colours = if h.colours().is_empty() {
        super::knights::seat_colours(id)
    } else {
        h.colours()
    };
    ink(colours
        .iter()
        .find(|(from, _)| *from == '7')
        .map_or('7', |(_, to)| *to))
}
pub(super) fn name(run: &Run, id: u32, n: usize) -> String {
    run.players
        .get(&id)
        .map_or("KNIGHT", |h| h.name.as_str())
        .to_uppercase()
        .chars()
        .take(n)
        .collect()
}
pub(super) fn center(im: &mut Img, x: i32, y: i32, text: &str, c: char) {
    im.text(x - text_width(text) / 2, y, text, c);
}
pub(super) fn big(im: &mut Img, x: i32, y: i32, text: &str) {
    static GLYPHS: OnceLock<Vec<(char, Img)>> = OnceLock::new();
    let gs = GLYPHS.get_or_init(|| {
        "COMBOX0123456789"
            .chars()
            .map(|c| {
                let mut s = Img::new(5, 7);
                s.text(0, 0, &c.to_string(), 't');
                let mut b = Img::new(10, 14);
                for yy in 0..14 {
                    for xx in 0..10 {
                        if let Some(col) = s.get(xx / 2, yy / 2) {
                            b.set(xx, yy, col);
                        }
                    }
                }
                (c, b)
            })
            .collect()
    });
    for (n, c) in text.chars().enumerate() {
        if let Some((_, g)) = gs.iter().find(|(k, _)| *k == c) {
            im.stamp(g, x + n as i32 * 12, y);
        }
    }
}
fn cell_pop(im: &mut Img, chain: u32) {
    static GLYPHS: OnceLock<Vec<(char, Img)>> = OnceLock::new();
    let glyphs = GLYPHS.get_or_init(|| {
        "X0123456789"
            .chars()
            .map(|c| {
                let mut source = Img::new(5, 7);
                source.text(0, 0, &c.to_string(), 't');
                let mut out = Img::new(10, 14);
                for y in 0..14 {
                    for x in 0..10 {
                        if let Some(colour) = source.get(x / 2, y / 2) {
                            out.set(x, y, colour);
                        }
                    }
                }
                (c, out)
            })
            .collect()
    });
    for (n, c) in format!("X{chain}").chars().enumerate() {
        if let Some((_, glyph)) = glyphs.iter().find(|(key, _)| *key == c) {
            im.stamp(glyph, 16 + n as i32 * 12, 18);
        }
    }
}

fn standings(run: &Run, id: u32) -> Vec<(u32, i32)> {
    let s = run.rafterwake.as_ref().unwrap();
    let mut order = s.lanes.keys().copied().collect::<Vec<_>>();
    order.sort_by_key(|who| (s.score(*who), *who));
    let own = order.iter().position(|who| *who == id).unwrap();
    if own >= 3 {
        order.truncate(3);
        order.push(id);
    }
    order
        .into_iter()
        .enumerate()
        .map(|(n, who)| (who, 56 + n as i32 * 32))
        .collect()
}
fn text_labels(run: &Run, id: u32) -> Vec<(i32, i32, String)> {
    let s = run.rafterwake.as_ref().unwrap();
    let d = &s.lanes[&id];
    if s.left == 0 {
        let mut words = vec![
            (
                (NATIVE_W
                    - text_width(if s.won {
                        "LOFT CLEARED"
                    } else {
                        "SHIFT FINISHED"
                    }))
                    / 2,
                24,
                if s.won {
                    "LOFT CLEARED"
                } else {
                    "SHIFT FINISHED"
                }
                .into(),
            ),
            (148, 43, "PROGRESS".into()),
            (212, 43, "FALLS".into()),
            (274, 43, "FINISH".into()),
        ];
        for (n, &who) in s.order.iter().enumerate() {
            let y = 64 + n as i32 * 24;
            let d = &s.lanes[&who];
            let score = d.score.to_string();
            words.extend([
                (28, y, s.place(who).to_string()),
                (
                    44,
                    y,
                    if who == id {
                        "YOU".into()
                    } else {
                        name(run, who, 12)
                    },
                ),
                (148 + text_width("PROGRESS") - text_width(&score), y, score),
                (
                    212 + text_width("FALLS") - text_width(&d.falls.to_string()),
                    y,
                    d.falls.to_string(),
                ),
                (
                    {
                        let finish = d.finish.map_or("--".into(), |t| {
                            format!("{}.{:02} S", t / HZ, (t % HZ) * 100 / HZ)
                        });
                        274 + text_width("FINISH") - text_width(&finish)
                    },
                    y,
                    d.finish.map_or("--".into(), |t| {
                        format!("{}.{:02} S", t / HZ, (t % HZ) * 100 / HZ)
                    }),
                ),
            ]);
        }
        let leader = s.order[0];
        let pot = format!("{}G POT + CROWN", GOLD_PER_SEAT * s.lanes.len() as u32);
        words.push(((NATIVE_W - text_width(&pot)) / 2, 165, pot));
        let rule = "MOST HEIGHT, FEWER FALLS, EARLIER FINISH";
        words.push(((NATIVE_W - text_width(rule)) / 2, 179, rule.into()));
        words.push((
            (NATIVE_W - text_width("E AGAIN")) / 2,
            206,
            "E AGAIN".into(),
        ));
        if s.order
            .iter()
            .filter(|&&r| s.score(r) == s.score(leader))
            .count()
            > 1
        {
            words.push((
                (NATIVE_W - text_width("TIE SPLIT")) / 2,
                151,
                "TIE SPLIT".into(),
            ));
        }
        return words;
    }
    if presentation().compact_hud {
        let mut words = vec![
            (130, 8, "RAFTERWAKE".into()),
            (16, 8, format!("{} S", s.left.div_ceil(HZ))),
            (
                282,
                8,
                format!("{}G POT", GOLD_PER_SEAT * s.lanes.len() as u32),
            ),
            (
                16,
                20,
                if d.down > 0 {
                    "CATCH YOUR BREATH"
                } else if d.height >= 370 {
                    "JUMP THE KEEPER"
                } else {
                    "CATCH NEXT DECK"
                }
                .into(),
            ),
            (12, 210, format!("DECK {} / 12", d.checkpoint)),
            (108, 210, format!("HEIGHT {}", d.height)),
            (216, 210, format!("FALLS {}", d.falls)),
            (
                300,
                210,
                format!("BEST {}", run.home.countryside.games.rafterwake.best),
            ),
        ];
        for (n, &who) in s.lanes.keys().enumerate() {
            words.push((
                18 + n as i32 * 94,
                32,
                format!(
                    "{} {} {}",
                    s.place(who),
                    if who == id {
                        "YOU".into()
                    } else {
                        name(run, who, 6)
                    },
                    s.lanes[&who].score
                ),
            ));
        }
        if LENGTH - s.left < presentation().hint_seconds * HZ {
            words.extend([
                (72, 194, "ARROWS RUN".into()),
                (166, 194, "F JUMP".into()),
                (240, 194, "UP CLIMB".into()),
            ]);
        }
        if d.chain > 0 && d.down == 0 {
            words.push((308, 20, format!("X{}", d.chain)));
        }
        return words;
    }
    let mut words = vec![
        (130, 6, "RAFTERWAKE".into()),
        (16, 6, format!("{} S", s.left.div_ceil(HZ))),
        (
            281,
            6,
            format!("{}G POT", GOLD_PER_SEAT * s.lanes.len() as u32),
        ),
        (282, 32, "CLIMB THE LOFT".into()),
        (14, 195, "ARROWS RUN".into()),
        (110, 195, "F JUMP".into()),
        (184, 195, "UP CLIMB".into()),
        (15, 210, format!("DECK {} / 12", d.checkpoint)),
        (158, 210, format!("HEIGHT {}", d.height)),
        (282, 182, format!("FALLS {}", d.falls)),
        (
            282,
            196,
            format!("BEST {}", run.home.countryside.games.rafterwake.best),
        ),
    ];
    for (who, y) in standings(run, id) {
        let v = &s.lanes[&who];
        let score = v.score.to_string();
        words.push((
            282,
            y,
            format!(
                "{} {}",
                s.place(who),
                if who == id {
                    "YOU".into()
                } else {
                    name(run, who, 7)
                }
            ),
        ));
        words.push((377 - text_width(&score), y, score));
    }
    if d.down > 0 {
        words.push((102, 23, "CATCH YOUR BREATH".into()));
    } else if d.height >= 370 {
        words.push((94, 23, "JUMP THE KEEPER".into()));
    } else {
        words.push((105, 23, "CATCH NEXT DECK".into()));
    }
    words
}
fn backdrop(terminal: bool) -> &'static Img {
    static WORLD: [OnceLock<Img>; 4] = [const { OnceLock::new() }; 4];
    WORLD[usize::from(super::slot_art::enabled()) * 2 + usize::from(terminal)].get_or_init(|| {
        let mut im = Img::black(254, 620);
        let source = slot_image("rafterwake.room", "backdrop", &Img::new(1, 1));
        if source.w > 1 {
            for y in 0..620 {
                for x in 0..254 {
                    if let Some(c) = source.get(x + (source.w - 254) / 2, y) {
                        im.set(x, y, c.map(|v| (u16::from(v) * 2 / 5) as u8));
                    }
                }
            }
        }
        for y in (0..620).step_by(36) {
            for x in [2, 238] {
                im.stamp(&art().all["post"], x, y);
            }
        }
        if terminal { cell_backdrop(&im) } else { im }
    })
}
fn danger(
    im: &mut Img,
    z: crate::rules::rafterwake::Danger,
    cam: i32,
    shake: i32,
    sweep: bool,
    progress: u32,
) {
    let x = z.x + 12 + shake;
    let y = z.y - cam + 38;
    let stage = progress.min(3);
    let colour = danger_colour(if stage < 2 { 2 } else { 3 });
    let (on, period) = match stage {
        0 => (1, 5),
        1 | 2 => (2, 5),
        _ => (3, 4),
    };
    let mut mark = |xx, yy| im.set(xx, yy, colour);
    // Four-pixel dash units survive the real 96-column half-block sampler.
    // The exact danger edges stay fixed while the interrupted marks strengthen.
    if sweep {
        for xx in 0..z.w {
            if xx / 4 % period < on {
                for dy in 0..3 {
                    mark(x + xx, y + z.h - 3 + dy);
                }
                mark(x + xx, y);
            }
        }
    } else {
        for yy in 0..z.h {
            if yy / 4 % period < on {
                for dx in 0..2 {
                    mark(x + dx, y + yy);
                    mark(x + z.w - 2 + dx, y + yy);
                }
            }
        }
        for xx in 0..z.w {
            if xx / 4 % period < on {
                for dy in 0..3 {
                    mark(x + xx, y + z.h - 3 + dy);
                }
            }
        }
    }
    if stage == 3 {
        for xx in 0..z.w {
            im.set(x + xx, y + z.h - 1, danger_colour(1));
        }
    }
}
fn draw(run: &Run, focus: Option<u32>, with_text: bool) -> (Img, Img) {
    let s = run.rafterwake.as_ref().unwrap();
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next().unwrap());
    let d = &s.lanes[&id];
    let a = art();
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    let mut fx = Img::new(NATIVE_W, NATIVE_H);
    if s.left == 0 {
        chrome::panel(
            &mut im,
            (16, 16, 352, 202),
            chrome::Tone {
                fill: &[[16, 26, 34]],
                ..chrome::Tone::BRASS
            },
        );
    } else {
        let shake = if d.freeze > 0 {
            if d.freeze % 2 == 0 { 1 } else { -1 }
        } else {
            0
        };
        let px = |x: i32| x + 12 + shake;
        let cam = if presentation().compact_hud {
            (d.camera - 8).max(0)
        } else {
            d.camera
        };
        let view_bottom = if presentation().compact_hud { 198 } else { 182 };
        let py = |y: i32| y - cam + 38;
        let bg = backdrop(!with_text);
        for y in 0..view_bottom - 38 {
            for x in 0..254 {
                if let Some(c) = bg.get(x, y + cam) {
                    im.set(x + 12, y + 38, c);
                }
            }
        }
        let elapsed = LENGTH - s.left;
        let ps = planks(d.seed, elapsed);
        for i in 1..12 {
            if let Some(x) = ladder(i) {
                let y = ps[i].y;
                if py(y) < view_bottom && py(y + 36) > 38 {
                    im.stamp(&a.all["ladder"], px(x) - 10, py(y));
                }
            }
        }
        for p in ps {
            if py(p.y) < view_bottom && py(p.y + 10) > 38 {
                for dx in (0..p.w).step_by(24) {
                    im.stamp(&a.all["plank"], px(p.x + dx), py(p.y));
                }
                if p.lift {
                    im.rect(px(p.x + 4), py(p.y + 5), p.w - 8, 2, 'S');
                }
            }
        }
        if d.brick_tell > 0 || d.brick_burst > 0 {
            danger(
                &mut im,
                d.brick,
                cam,
                shake,
                false,
                (30 - d.brick_tell) * 4 / 30,
            );
            let y = if d.brick_tell > 0 {
                d.brick.y + ((30 - d.brick_tell) as i32 * 50 / 30)
            } else {
                d.brick.y + d.brick.h - 18
            };
            im.stamp(&a.all["brick"], px(d.brick.x + 2), py(y));
        }
        let k = &d.keeper;
        // Landing/climbing dust belongs behind both figures, at their feet.
        if d.effect > 0 {
            let n = ((12 - d.effect) * 4 / 12).min(3);
            im.stamp(
                &a.all[&format!("fx-dust-{n}")],
                px(d.effect_x) - 24,
                py(d.effect_y) - 32,
            );
        }
        if py(TOP) > 38 && py(TOP - 128) < view_bottom {
            let tell_length = 33 - k.stage() as u32 * 4;
            let tell_stage = ((tell_length - k.ticks.min(tell_length)) * 4 / tell_length).min(3);
            if matches!(k.beat, Beat::Tell | Beat::Attack) {
                danger(
                    &mut im,
                    k.danger,
                    cam,
                    shake,
                    k.sweep,
                    if k.beat == Beat::Attack {
                        3
                    } else {
                        tell_stage
                    },
                );
            }
            let pose = if k.hp == 0 {
                10
            } else if k.flinch > 0 {
                7
            } else {
                match k.beat {
                    Beat::Tell => {
                        if k.sweep {
                            5
                        } else {
                            3
                        }
                    }
                    Beat::Attack => {
                        if k.sweep {
                            6
                        } else {
                            4
                        }
                    }
                    Beat::Rest => {
                        if k.ticks < 12 {
                            11
                        } else if k.stage() > 0 {
                            7 + k.stage()
                        } else {
                            0
                        }
                    }
                    Beat::Walk => 1 + (run.tick / 7 % 2) as usize,
                    Beat::Defeat => 10,
                }
            };
            let keeper = if k.beat == Beat::Tell && k.hp > 0 && k.flinch == 0 {
                &a.tells[k.stage()][usize::from(k.sweep)][tell_stage as usize]
            } else {
                &a.keeper[k.stage()][pose]
            };
            let state = if k.hp == 0 {
                if k.flinch > 4 {
                    "defeat-first"
                } else {
                    "defeat"
                }
            } else if k.flinch > 0 {
                "flinch"
            } else {
                match k.beat {
                    Beat::Tell => {
                        if k.sweep {
                            "sweep-tell"
                        } else {
                            "drop-tell"
                        }
                    }
                    Beat::Attack => {
                        if k.sweep {
                            "sweep"
                        } else {
                            "drop"
                        }
                    }
                    Beat::Rest if k.ticks < 12 => {
                        if k.sweep {
                            "sweep-recover"
                        } else {
                            "drop-recover"
                        }
                    }
                    Beat::Rest => "idle",
                    _ => "walk",
                }
            };
            let keeper = a.damage_actions.get(&(k.stage(), state)).unwrap_or(keeper);
            let body = if with_text {
                keeper.body.clone()
            } else {
                cell_actor(&keeper.body, None)
            };
            im.stamp(&body, px(k.x) - body.w / 2, py(TOP) - body.h);
            if !with_text {
                fx.stamp(&keeper.details, px(k.x) - body.w / 2, py(TOP) - body.h);
            }
            if k.hp > 0 {
                im.rect(px(k.x) - 22, py(TOP) - keeper.body.h - 5, 44, 3, 's');
                im.rect(
                    px(k.x) - 21,
                    py(TOP) - keeper.body.h - 4,
                    42 * i32::from(k.hp) / 6,
                    2,
                    'o',
                );
            }
            if k.beat == Beat::Attack {
                let n = ((10 - k.ticks) * 4 / 10).min(3);
                let effect = &a.all[&format!("fx-{}-{n}", if k.sweep { "sweep" } else { "drop" })];
                let (x, y) = (px(k.danger.x + k.danger.w / 2) - 24, py(TOP) - 32);
                if super::slot_art::enabled() {
                    attack_stamp(
                        &mut im,
                        &mut fx,
                        effect,
                        x,
                        y,
                        (px(k.danger.x), py(k.danger.y), k.danger.w, k.danger.h),
                        !with_text,
                    );
                } else {
                    im.stamp(effect, x, y);
                }
            }
        }
        let outlined = (d.x / Q - k.x).abs() < 48 && (d.y / Q - TOP).abs() < 80;
        let knight = knight(run, id, usize::from(d.pose), d.dir, outlined && with_text);
        let knight = if with_text {
            knight
        } else {
            let h = &run.players[&id];
            let colours = if h.colours().is_empty() {
                super::knights::seat_colours(id)
            } else {
                h.colours()
            };
            cell_actor(&knight, Some(colours))
        };
        im.stamp(&knight, px(d.x / Q) - 24, py(d.y / Q) - 48);
        if !with_text {
            // The seated knight owns every overlap, including its seat contour.
            for y in 0..knight.h {
                for x in 0..knight.w {
                    if knight.get(x, y).is_some() {
                        fx.set(px(d.x / Q) - 24 + x, py(d.y / Q) - 48 + y, [0, 0, 0]);
                    }
                }
            }
            fx.rect(0, 0, NATIVE_W, 38, 'k');
            fx.rect(0, view_bottom, NATIVE_W, NATIVE_H - view_bottom, 'k');
            fx.rect(0, 38, 12, 144, 'k');
            fx.rect(266, 38, NATIVE_W - 266, 144, 'k');
        }

        // The stage has a strict viewport: figures and scenery never cover header or controls.
        im.rect(0, 0, 276, 38, 'k');
        im.rect(0, view_bottom, 276, NATIVE_H - view_bottom, 'k');
        im.rect(0, 38, 12, 144, 'k');
        im.rect(266, 38, 10, 144, 'k');
        if presentation().compact_hud {
            (im, fx) = present_stage(&im, &fx, false);
        } else {
            im.line(12, 183, 265, 183, 'S');
        }
        if !presentation().compact_hud {
            im.line(276, 32, 276, 212, 'S');
        }
        if super::slot_art::enabled() {
            im.stamp(slot_chrome("rafterwake.chrome", false, false), 0, 0);
        }
        for (who, y) in standings(run, id)
            .into_iter()
            .filter(|_| !presentation().compact_hud)
        {
            im.rect(282, y + 18, 94, 3, 's');
            im.rect(282, y + 18, 94 * s.lanes[&who].score as i32 / 1000, 3, 'o');
        }
        if d.chain > 0 && d.down == 0 {
            if super::slot_art::enabled() {
                im.stamp(slot_chrome("rafterwake.chrome", false, true), 0, 0);
            }
            if !presentation().compact_hud && with_text {
                big(&mut im, 16, 20, &format!("X{}", d.chain));
            } else if !presentation().compact_hud {
                cell_pop(&mut im, d.chain);
            }
        }
    }
    if super::slot_art::enabled() && s.left == 0 {
        im.stamp(slot_chrome("rafterwake.chrome", true, false), 0, 0);
    }
    if s.left > 0 && presentation().compact_hud {
        for (n, &who) in s.lanes.keys().enumerate() {
            let h = &run.players[&who];
            let colours = if h.colours().is_empty() {
                super::knights::seat_colours(who)
            } else {
                h.colours()
            };
            for y in 32..40 {
                for x in 9 + n as i32 * 94..13 + n as i32 * 94 {
                    im.set(x, y, ink(seat_ink(colours)).unwrap());
                }
            }
        }
        if LENGTH - s.left < presentation().hint_seconds * HZ {
            im.rect(65, 190, 254, 18, 'k');
        }
    }
    let words = text_labels(run, id);
    if !with_text && s.left > 0 {
        label_wells(&mut im, &mut fx, &words);
    }
    if with_text {
        for (x, y, t) in words {
            im.text(x, y, &t, 't');
        }
    }
    (im, fx)
}
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus, true)
}
pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus, false)
}
pub(super) fn labels(run: &Run, focus: Option<u32>) -> Vec<(i32, i32, String)> {
    let s = run.rafterwake.as_ref().unwrap();
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next().unwrap());
    text_labels(run, id)
}
pub(super) fn sign(im: &mut Img, run: &Run) {
    if run.room().kind != crate::rules::RoomKind::Workshop || run.rafterwake.is_some() {
        return;
    }
    chrome::panel(im, (140, 68, 112, 34), chrome::Tone::BRASS);
    center(im, 196, 74, "RAFTERWAKE", 'O');
    center(im, 196, 88, "E PLAY", 'S');
    im.stamp(&art().all["ladder"], 178, 108);
}

#[cfg(test)]
mod slot_tests {
    use super::*;
    #[test]
    fn reviewed_knights_preserve_the_canvas_and_four_seat_colours() {
        super::super::slot_art::use_original(false);
        let original = &art().knight[0];
        assert_eq!((original.w, original.h), (48, 48));
        let seats: Vec<_> = (1..=4)
            .map(|id| slot_knight(original, super::super::knights::seat_colours(id), true))
            .collect();
        for a in 0..4 {
            for b in a + 1..4 {
                assert_ne!(seats[a], seats[b]);
            }
        }
        // A failed or unreviewed slot must leave the whole original stamp intact.
        assert_eq!(slot_image("absent.slot", "idle", original), *original);
    }
    #[test]
    fn display_banks_preserve_feet_and_separate_cloth_from_steel() {
        super::super::slot_art::use_original(false);
        let a = art();
        assert_eq!((a.keeper[0][0].body.w, a.keeper[0][0].body.h), (102, 128));
        for (pose, source) in a.knight.iter().enumerate() {
            for id in 1..=4 {
                let colours = super::super::knights::seat_colours(id);
                let dressed = slot_knight_pose(source, colours, true, pose);
                for y in 0..source.h {
                    for x in 0..source.w {
                        assert_eq!(source.get(x, y).is_some(), dressed.get(x, y).is_some());
                        if let Some([r, g, b]) = source.get(x, y) {
                            if r.abs_diff(g) < 18 && g.abs_diff(b) < 18 {
                                assert_eq!(
                                    source.get(x, y),
                                    dressed.get(x, y),
                                    "metal changed at pose {pose} ({x},{y})"
                                );
                            }
                        }
                    }
                }
                let cells = cell_actor(&dressed, Some(colours));
                let inks: std::collections::BTreeSet<_> =
                    cells.pixels().filter(|c| *c != [0, 0, 0]).collect();
                assert!(inks.len() <= 6, "cell actor has {} inks", inks.len());
                assert!(inks.contains(&ink(seat_ink(colours)).unwrap()));
            }
        }
    }
    #[test]
    fn plank_override_keeps_every_original_support_pixel() {
        super::super::slot_art::use_original(true);
        let original = art().all["plank"].clone();
        super::super::slot_art::use_original(false);
        let imported = &art().all["plank"];
        assert_eq!((original.w, original.h), (imported.w, imported.h));
        for y in 0..original.h {
            for x in 0..original.w {
                if original.get(x, y).is_some() {
                    assert!(imported.get(x, y).is_some(), "support hole at {x},{y}");
                }
            }
        }
    }
}
