//! Brasswake's copper brewery: one native pixel grain, cached art and aisle scenery.
use super::super::{
    chrome,
    ink::{Img, ink, text_width},
};
use super::{NATIVE_H, NATIVE_W};
use crate::rules::{
    HZ, Run,
    brasswake::{Beat, GOLD_PER_SEAT, Lane, WORK_PER_RECORD},
};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};
fn decode(bytes: &[u8]) -> Img {
    let png = image::load_from_memory(bytes)
        .expect("Brasswake sprite")
        .to_rgba8();
    let mut out = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] > 0 {
            out.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    out
}

fn hud() -> &'static serde_json::Value {
    static HUD: OnceLock<serde_json::Value> = OnceLock::new();
    HUD.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/art-slots/brasswake.chrome/hud.json"
        ))
        .unwrap()
    })
}
fn compact_hud() -> bool {
    static COMPACT: OnceLock<bool> = OnceLock::new();
    *COMPACT.get_or_init(|| {
        std::env::var("ANGEL_MINIGAME_HUD").is_ok_and(|v| v == "compact")
            && hud()["compact"]["compact"] == true
    })
}
fn strip_x(n: usize) -> i32 {
    hud()["strip_x"].as_i64().unwrap() as i32
        + n as i32 * hud()["seat_step"].as_i64().unwrap() as i32
}
fn strip_y() -> i32 {
    hud()["strip_y"].as_i64().unwrap() as i32
}
// These conversions run only while populating the native art caches.
fn slot(slot: &str, name: &str) -> Option<Img> {
    if !super::slot_art::enabled() {
        return None;
    }
    let data = super::slot_art::bytes(slot, name)?;
    let mut im = decode(data);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                if c.iter().copied().max().unwrap() > 224 {
                    im.set(x, y, nearest(c, 224));
                }
            }
        }
    }
    Some(im)
}
fn nearest(c: [u8; 3], ceiling: u8) -> [u8; 3] {
    super::super::ink::PALETTE
        .iter()
        .copied()
        .map(super::super::ink::rgb)
        .filter(|v| v.iter().all(|&n| n <= ceiling))
        .min_by_key(|v| {
            v.iter()
                .zip(c)
                .map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2))
                .sum::<i32>()
        })
        .unwrap()
}
fn dim(mut im: Img) -> Img {
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                im.set(
                    x,
                    y,
                    nearest(c.map(|v| (u16::from(v) * 150 / 256) as u8), 160),
                );
            }
        }
    }
    im
}
fn skin(source: Option<Img>, w: i32, h: i32) -> Option<Img> {
    let source = source?;
    let mut x0 = source.w;
    let mut y0 = source.h;
    let mut x1 = 0;
    let mut y1 = 0;
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
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let source_edge = ((x1 - x0).min(y1 - y0) / 4).max(1);
    let axis = |at: i32, size: i32, len: i32| {
        let edge = 2.min(size / 3);
        if at < edge {
            at * source_edge / edge
        } else if at >= size - edge {
            len - source_edge + (at - size + edge) * source_edge / edge
        } else {
            source_edge + (at - edge) * (len - 2 * source_edge) / (size - 2 * edge)
        }
    };
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = source.get(x0 + axis(x, w, x1 - x0), y0 + axis(y, h, y1 - y0)) {
                out.set(x, y, c);
            }
        }
    }
    // Empty fields stay flat so source wear never competes with live labels.
    out.rect(2, 2, w - 4, h - 4, 's');
    Some(out)
}
// Cell banks are baked once; the live path only stamps integer pixels.
fn cell_sprite(source: &Img, palette: &[[u8; 3]], rim: [u8; 3]) -> Img {
    super::slot_art::cell_sprite(source, palette, rim)
}
fn cell_backdrop(source: &Img) -> Img {
    let mut out = source.clone();
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = source.get(x, y) {
                out.set(x, y, c.map(|v| (u16::from(v) * 3 / 10) as u8));
            }
        }
    }
    out
}
fn clean(source: &Img, data: &serde_json::Value) -> Img {
    let mut erased = vec![false; (source.w * source.h) as usize];
    if let Some(runs) = data["erase_runs"].as_array() {
        for run in runs {
            let y = run[0].as_i64().unwrap() as i32;
            let x = run[1].as_i64().unwrap() as i32;
            let w = run[2].as_i64().unwrap() as i32;
            for x in x..x + w {
                erased[(y * source.w + x) as usize] = true;
            }
        }
    }
    let mut out = Img::new(source.w, source.h);
    for y in 0..source.h {
        for x in 0..source.w {
            if !erased[(y * source.w + x) as usize] {
                if let Some(c) = source.get(x, y) {
                    out.set(x, y, c);
                }
            }
        }
    }
    out
}
fn seat_material(source: &Img, data: &serde_json::Value, seat: char) -> Img {
    let ramp = match seat {
        'r' | '7' => [[54, 15, 24], [130, 28, 42], [195, 47, 57], [218, 105, 99]],
        'S' | 'q' | '1' => [[16, 32, 65], [27, 66, 142], [45, 109, 192], [103, 157, 209]],
        'e' | 'F' | 'l' | 'L' | 'A' => [[14, 40, 31], [21, 90, 52], [45, 143, 69], [110, 183, 106]],
        _ => [[62, 37, 13], [133, 85, 18], [193, 145, 35], [215, 181, 91]],
    };
    let mut out = source.clone();
    if let Some(runs) = data["cloth_runs"].as_array() {
        for run in runs {
            let y = run[0].as_i64().unwrap() as i32;
            let x = run[1].as_i64().unwrap() as i32;
            let w = run[2].as_i64().unwrap() as i32;
            for x in x..x + w {
                if let Some(c) = source.get(x, y) {
                    let shade = (c.into_iter().max().unwrap().saturating_sub(45) / 45).min(3);
                    out.set(x, y, ramp[shade as usize]);
                }
            }
        }
    }
    out
}
struct Art {
    damage_actions: BTreeMap<(usize, usize), [Img; 4]>,
    keeper: [[Img; 12]; 3],
    porter: [[Img; 12]; 3],
    keeper_right: [[Img; 12]; 3],
    porter_right: [[Img; 12]; 3],
    keeper_cells: [[Img; 12]; 3],
    keeper_cells_right: [[Img; 12]; 3],
    porter_cells: [[Img; 12]; 3],
    porter_cells_right: [[Img; 12]; 3],
    knight: [Img; 12],
    contact: [Img; 4],
    slam: [Img; 4],
    steam: [Img; 4],
    cask: Img,
    bread: Img,
}
fn damage(source: &Img, stage: usize, eye: [i32; 2]) -> Img {
    let mut out = source.clone();
    if stage == 0 {
        return out;
    }
    // Dark-around-light fractures stay broad enough to survive half-block sampling.
    for seam in 0..stage {
        let y = source.h * 3 / 5 + seam as i32 * 7;
        for dx in -7..=8 {
            let x = source.w / 2 + dx;
            let yy = y + if dx < 0 { dx / 3 } else { -dx / 4 };
            let width = if stage == 2 { 6 } else { 2 };
            for dy in -width..=width {
                if source.get(x, yy + dy).is_some() {
                    out.set(x, yy + dy, ink('s').unwrap());
                }
            }
            if source.get(x, yy).is_some() {
                out.set(x, yy, ink('t').unwrap());
            }
        }
    }
    if stage == 2 {
        let [cx, cy] = eye;
        for dy in -3..=3 {
            let (x, y) = (cx - 4, cy + dy);
            if source.get(x, y).is_some() {
                out.set(x, y, ink('s').unwrap());
            }
        }
    }
    out
}
fn cell_art(source: &Img, stage: usize, eye: [i32; 2], dim: bool) -> Img {
    let mut out = damage(source, stage, eye);
    let [cx, cy] = eye;
    // Curved lenses and their dark surround survive averaging into real cells.
    for dy in -4i32..=4 {
        for dx in -9i32..=9 {
            let (x, y) = (cx + dx, cy + dy);
            if dx.abs() + dy.abs() < 12 && source.get(x, y).is_some() {
                out.set(x, y, ink('s').unwrap());
            }
        }
    }
    for ex in [cx - 4, cx + 4] {
        for dy in -3i32..=3 {
            for dx in -2i32..=2 {
                let (x, y) = (ex + dx, cy + dy);
                if dx.abs() + dy.abs() < 5 && source.get(x, y).is_some() {
                    out.set(x, y, ink('S').unwrap());
                }
            }
        }
        for (dx, dy) in [
            (0, -2),
            (1, -2),
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (0, 0),
            (1, 0),
            (0, 1),
        ] {
            let (x, y) = (ex + dx, cy + dy);
            if source.get(x, y).is_some() {
                out.set(x, y, ink(if dim { 'U' } else { 'v' }).unwrap());
            }
        }
    }
    if stage == 2 {
        for dy in -4..=4 {
            for dx in -1..=0 {
                let (x, y) = (cx - 4 + dx, cy + dy);
                if source.get(x, y).is_some() {
                    out.set(x, y, ink('s').unwrap());
                }
            }
        }
    }
    out
}
fn family(name: &str) -> &'static str {
    if name.starts_with("keeper-") {
        "brasswake.keeper"
    } else if name.starts_with("porter-") {
        "brasswake.porter"
    } else if name.starts_with("knight-") {
        "brasswake.knight"
    } else if name.starts_with("effects-contact-") {
        "brasswake.contact"
    } else if name.starts_with("effects-slam-") {
        "brasswake.slam"
    } else if name.starts_with("effects-steam-") {
        "brasswake.steam"
    } else if name == "cask" {
        "brasswake.cask"
    } else {
        "brasswake.bread"
    }
}
fn sprite(name: &str, fallback: &[u8]) -> Img {
    let Some(mut im) = slot(family(name), name) else {
        return decode(fallback);
    };
    if name.starts_with("keeper-") {
        static CLEANUP: OnceLock<serde_json::Value> = OnceLock::new();
        let data = CLEANUP.get_or_init(|| {
            serde_json::from_str(r#"{"keeper-damage-1":{"erase_runs":[]},"keeper-damage-2":{"erase_runs":[]},"keeper-defeat":{"erase_runs":[[70,46,1],[71,45,1]]},"keeper-flinch":{"erase_runs":[]},"keeper-idle":{"erase_runs":[]},"keeper-recover":{"erase_runs":[[73,53,2],[74,52,5],[75,51,6],[75,60,4],[76,51,4],[76,56,2],[76,59,4],[77,50,5],[77,56,3],[77,60,4],[78,51,7],[78,59,4]]},"keeper-rush":{"erase_runs":[[69,8,5],[69,18,3],[69,22,2],[70,8,5],[70,16,8],[70,35,1],[70,37,1],[71,8,2],[71,11,2],[71,17,1],[71,20,7],[71,34,5],[71,41,1],[71,43,1],[72,8,5],[72,17,1],[72,20,8],[72,33,7],[72,41,3],[73,9,1],[73,11,4],[73,18,10],[73,33,6],[73,40,5],[73,47,3],[74,12,1],[74,14,2],[74,17,11],[74,32,3],[74,36,9],[74,48,1],[75,12,1],[75,14,2],[75,17,12],[75,33,2],[75,36,4],[75,41,6],[75,49,4],[76,10,1],[76,12,5],[76,18,13],[76,33,5],[76,39,9],[76,49,5],[77,10,6],[77,18,1],[77,21,2],[77,24,4],[77,29,3],[77,33,3],[77,39,4],[77,45,6],[77,52,4],[78,22,3],[78,26,5],[78,36,4],[78,41,3],[78,45,10],[78,56,1],[79,43,3]]},"keeper-rush-tell":{"erase_runs":[]},"keeper-slam":{"erase_runs":[[69,71,3],[70,49,1],[70,70,1],[70,72,2],[71,70,4],[72,49,2],[72,62,3],[72,70,2],[73,48,5],[73,60,6],[73,69,2],[73,72,1],[74,49,4],[74,60,6],[74,69,4],[75,47,7],[75,58,3],[75,63,3],[75,69,3],[76,48,7],[76,57,5],[76,63,4],[76,69,4],[77,49,2],[77,52,3],[77,57,1],[77,60,5],[77,66,1],[77,69,3],[78,56,3],[78,62,2],[78,66,1]]},"keeper-slam-tell":{"erase_runs":[]},"keeper-walk-1":{"erase_runs":[]},"keeper-walk-2":{"erase_runs":[]}}"#)
            .unwrap()
        });
        im = clean(&im, &data[name]);
    }
    if name.starts_with("keeper-") {
        let mut large = Img::new(im.w * 3 / 2, im.h * 3 / 2);
        for y in 0..large.h {
            for x in 0..large.w {
                if let Some(c) = im.get(x * 2 / 3, y * 2 / 3) {
                    large.set(x, y, c);
                }
            }
        }
        im = large;
    }
    if name.starts_with("keeper-") || name.starts_with("porter-") {
        im = flip(&im);
    }
    im
}
fn body(
    source: &Img,
    stage: usize,
    eye: [i32; 2],
    cells: bool,
    dimmed: bool,
    slot_id: &str,
    name: &str,
) -> Img {
    if super::slot_art::enabled()
        && (super::slot_art::bytes(slot_id, name).is_some()
            || (slot_id == "brasswake.porter"
                && name.starts_with("porter-damage-")
                && super::slot_art::bytes(slot_id, "porter-idle").is_some()))
    {
        // Delivered damage poses already carry their own eyes and fractures.
        if cells {
            cell_sprite(
                source,
                &[
                    [25, 34, 41],
                    [104, 70, 41],
                    [169, 118, 57],
                    [216, 169, 94],
                    [80, 158, 159],
                ],
                [47, 103, 111],
            )
        } else {
            source.clone()
        }
    } else if cells {
        cell_art(source, stage, eye, dimmed)
    } else {
        damage(source, stage, eye)
    }
}
const FOE_NAMES: [&str; 12] = [
    "idle",
    "walk-1",
    "walk-2",
    "slam-tell",
    "slam",
    "rush-tell",
    "rush",
    "flinch",
    "damage-1",
    "damage-2",
    "defeat",
    "recover",
];
fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    static ORIGINAL: OnceLock<Art> = OnceLock::new();
    let cache = if super::slot_art::enabled() {
        &ART
    } else {
        &ORIGINAL
    };
    cache.get_or_init(|| {
        macro_rules! s {
            ($n:literal) => {
                sprite(
                    $n,
                    include_bytes!(concat!("../../../../assets/brasswake/", $n, ".png")),
                )
            };
        }
        let keeper = [
            s!("keeper-idle"),
            s!("keeper-walk-1"),
            s!("keeper-walk-2"),
            s!("keeper-slam-tell"),
            s!("keeper-slam"),
            s!("keeper-rush-tell"),
            s!("keeper-rush"),
            s!("keeper-flinch"),
            s!("keeper-damage-1"),
            s!("keeper-damage-2"),
            s!("keeper-defeat"),
            s!("keeper-recover"),
        ];
        let porter = [
            s!("porter-idle"),
            s!("porter-walk-1"),
            s!("porter-walk-2"),
            s!("porter-slam-tell"),
            s!("porter-slam"),
            s!("porter-rush-tell"),
            s!("porter-rush"),
            s!("porter-flinch"),
            s!("porter-damage-1"),
            s!("porter-damage-2"),
            s!("porter-defeat"),
            s!("porter-recover"),
        ];
        let mut porter = porter;
        if let Some(idle) = slot("brasswake.porter", "porter-idle") {
            for (pose, stage) in [(8, 1), (9, 2)] {
                if super::slot_art::bytes(
                    "brasswake.porter",
                    &format!("porter-{}", FOE_NAMES[pose]),
                )
                .is_none()
                {
                    porter[pose] = damage(&flip(&idle), stage, [18, 18]);
                }
            }
        }
        let anchors: std::collections::BTreeMap<String, Vec<[i32; 2]>> =
            serde_json::from_str(include_str!("../../../../assets/brasswake/anchors.json"))
                .expect("Brasswake face anchors");
        let mut bank = Art {
            damage_actions: BTreeMap::new(),
            keeper_cells: std::array::from_fn(|st| {
                std::array::from_fn(|p| {
                    body(
                        &keeper[p],
                        st,
                        anchors["keeper"][p],
                        true,
                        p == 10,
                        "brasswake.keeper",
                        &format!("keeper-{}", FOE_NAMES[p]),
                    )
                })
            }),
            keeper_cells_right: std::array::from_fn(|st| {
                std::array::from_fn(|p| {
                    flip(&body(
                        &keeper[p],
                        st,
                        anchors["keeper"][p],
                        true,
                        p == 10,
                        "brasswake.keeper",
                        &format!("keeper-{}", FOE_NAMES[p]),
                    ))
                })
            }),
            porter_cells: std::array::from_fn(|st| {
                std::array::from_fn(|p| {
                    body(
                        &porter[p],
                        st,
                        anchors["porter"][p],
                        true,
                        p == 10,
                        "brasswake.porter",
                        &format!("porter-{}", FOE_NAMES[p]),
                    )
                })
            }),
            porter_cells_right: std::array::from_fn(|st| {
                std::array::from_fn(|p| {
                    flip(&body(
                        &porter[p],
                        st,
                        anchors["porter"][p],
                        true,
                        p == 10,
                        "brasswake.porter",
                        &format!("porter-{}", FOE_NAMES[p]),
                    ))
                })
            }),
            keeper_right: std::array::from_fn(|st| {
                std::array::from_fn(|p| {
                    flip(&body(
                        &keeper[p],
                        st,
                        anchors["keeper"][p],
                        false,
                        p == 10,
                        "brasswake.keeper",
                        &format!("keeper-{}", FOE_NAMES[p]),
                    ))
                })
            }),
            porter_right: std::array::from_fn(|st| {
                std::array::from_fn(|p| {
                    flip(&body(
                        &porter[p],
                        st,
                        anchors["porter"][p],
                        false,
                        p == 10,
                        "brasswake.porter",
                        &format!("porter-{}", FOE_NAMES[p]),
                    ))
                })
            }),
            keeper: std::array::from_fn(|st| {
                std::array::from_fn(|p| {
                    body(
                        &keeper[p],
                        st,
                        anchors["keeper"][p],
                        false,
                        p == 10,
                        "brasswake.keeper",
                        &format!("keeper-{}", FOE_NAMES[p]),
                    )
                })
            }),
            porter: std::array::from_fn(|st| {
                std::array::from_fn(|p| {
                    body(
                        &porter[p],
                        st,
                        anchors["porter"][p],
                        false,
                        p == 10,
                        "brasswake.porter",
                        &format!("porter-{}", FOE_NAMES[p]),
                    )
                })
            }),
            knight: [
                s!("knight-idle"),
                s!("knight-walk-1"),
                s!("knight-walk-2"),
                s!("knight-jab"),
                s!("knight-cross"),
                s!("knight-kick"),
                s!("knight-grab"),
                s!("knight-throw"),
                s!("knight-flinch"),
                s!("knight-down"),
                s!("knight-guard"),
                s!("knight-recover"),
            ],
            contact: [
                s!("effects-contact-0"),
                s!("effects-contact-1"),
                s!("effects-contact-2"),
                s!("effects-contact-3"),
            ],
            slam: [
                s!("effects-slam-0"),
                s!("effects-slam-1"),
                s!("effects-slam-2"),
                s!("effects-slam-3"),
            ],
            steam: [
                s!("effects-steam-0"),
                s!("effects-steam-1"),
                s!("effects-steam-2"),
                s!("effects-steam-3"),
            ],
            cask: s!("cask"),
            bread: s!("bread"),
        };
        if super::slot_art::enabled() {
            for (stage, pose, name) in [
                (1, 3, "stage-1-slam-tell"),
                (1, 4, "stage-1-slam"),
                (1, 11, "stage-1-recover"),
                (1, 12, "stage-1-rush-recover"),
                (1, 7, "strong-flinch"),
                (2, 7, "strong-flinch"),
                (0, 10, "defeat-2"),
                (1, 10, "defeat-2"),
                (2, 10, "defeat-2"),
                (0, 13, "defeat-1"),
                (1, 13, "defeat-1"),
                (2, 13, "defeat-1"),
                (1, 5, "stage-1-rush-tell"),
                (1, 6, "stage-1-rush"),
                (2, 9, "stage-2-idle"),
                (2, 3, "stage-2-slam-tell"),
                (2, 4, "stage-2-slam"),
            ] {
                if let Some(im) = slot("brasswake.keeper-damaged", name) {
                    let mut large = Img::new(im.w * 3 / 2, im.h * 3 / 2);
                    for y in 0..large.h {
                        for x in 0..large.w {
                            if let Some(c) = im.get(x * 2 / 3, y * 2 / 3) {
                                large.set(x, y, c);
                            }
                        }
                    }
                    let left = flip(&large);
                    let cell = cell_sprite(
                        &left,
                        &[
                            [25, 34, 41],
                            [104, 70, 41],
                            [169, 118, 57],
                            [216, 169, 94],
                            [80, 158, 159],
                        ],
                        [47, 103, 111],
                    );
                    bank.damage_actions
                        .insert((stage, pose), [left, large, cell.clone(), flip(&cell)]);
                }
            }
        }
        bank
    })
}
fn flip(im: &Img) -> Img {
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
struct SeatArt {
    colours: Vec<(char, char)>,
    imported: bool,
    poses: [[Img; 12]; 2],
    outlined: [[Img; 12]; 2],
    cells: [[Img; 12]; 2],
}
fn knight(run: &Run, id: u32, pose: usize, dir: i32, outlined: bool, cells: bool) -> Img {
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
                let mut im = art().knight[p].clone();
                let names = [
                    "idle", "walk-1", "walk-2", "jab", "cross", "kick", "grab", "throw", "flinch",
                    "down", "guard", "recover",
                ];
                let name = format!("knight-{}", names[p]);
                if super::slot_art::enabled()
                    && super::slot_art::bytes("brasswake.knight", &name).is_some()
                {
                    static MASKS: OnceLock<serde_json::Value> = OnceLock::new();
                    let masks = MASKS.get_or_init(|| {
                        serde_json::from_str(include_str!(
                            "../../../../assets/art-slots/brasswake.knight/cleanup.json"
                        ))
                        .unwrap()
                    });
                    let seat = colours
                        .iter()
                        .find(|(from, _)| *from == '7')
                        .map_or('7', |(_, to)| *to);
                    im = seat_material(&im, &masks[&name], seat);
                } else {
                    im = im.recolor(colours);
                }
                if super::slot_art::enabled() {
                    for y in 0..im.h {
                        for x in 0..im.w {
                            if let Some([r, g, b]) = im.get(x, y) {
                                if r.abs_diff(g) < 25 && g.abs_diff(b) < 28 && r > 65 {
                                    im.set(
                                        x,
                                        y,
                                        match r {
                                            0..=110 => [56, 78, 99],
                                            111..=155 => [82, 117, 151],
                                            _ => [143, 163, 168],
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
                im
            });
            let left = std::array::from_fn(|p| flip(&right[p]));
            let seat_ink = colours
                .iter()
                .find(|(from, _)| *from == '7')
                .map_or('7', |(_, to)| *to);
            let contour = ink(seat_ink).unwrap();
            let outlines = [&right, &left].map(|poses| {
                std::array::from_fn(|p| {
                    let source = &poses[p];
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
                })
            });
            let cells = [&right, &left].map(|poses| {
                std::array::from_fn(|p| {
                    let rim = match seat_ink {
                        '1' => [45, 109, 192],
                        'A' => [45, 143, 69],
                        '4' => [193, 145, 35],
                        _ => [195, 47, 57],
                    };
                    cell_sprite(
                        &poses[p],
                        &[
                            [18, 29, 40],
                            [56, 78, 99],
                            [143, 163, 168],
                            rim,
                            [165, 118, 57],
                        ],
                        rim,
                    )
                })
            });
            seats.push(SeatArt {
                cells,
                imported: super::slot_art::enabled(),
                colours: colours.to_vec(),
                poses: [right, left],
                outlined: outlines,
            });
            seats.len() - 1
        });
    let poses = if cells {
        &seats[at].cells
    } else if outlined {
        &seats[at].outlined
    } else {
        &seats[at].poses
    };
    poses[usize::from(dir < 0)][pose].clone()
}
const STAGE_TOP: i32 = 34;
const STAGE_BOTTOM: i32 = 180;
const STAGE_LIFT: i32 = 18;
fn world() -> &'static Img {
    static WORLD: OnceLock<Img> = OnceLock::new();
    static ORIGINAL: OnceLock<Img> = OnceLock::new();
    let cache = if super::slot_art::enabled() {
        &WORLD
    } else {
        &ORIGINAL
    };
    cache.get_or_init(|| {
        if let Some(room) = slot("brasswake.room", "room") {
            let room = dim(room);
            let mut im = Img::black(1200, STAGE_BOTTOM - STAGE_TOP);
            for y in 0..im.h {
                for x in 0..im.w {
                    if let Some(c) = room.get(
                        x % 240,
                        if y < 12 {
                            y * 96 / 12
                        } else {
                            96 + (y - 12) * (room.h - 96) / (STAGE_BOTTOM - STAGE_TOP - 12)
                        },
                    ) {
                        im.set(x, y, c);
                    }
                }
            }
            return im;
        }
        let mut im = Img::black(1200, STAGE_BOTTOM - STAGE_TOP);
        for section in 0..5 {
            let x = section * 240;
            for y in [0, 17, 34] {
                im.rect(x, y, 238, 1, 's');
            }
            for xx in [x + 6, x + 220] {
                im.rect(xx, 1, 5, 35, 's');
                im.rect(xx + 1, 2, 1, 33, 'S');
            }
            for xx in [x + 35, x + 126] {
                im.ellipse((xx + 28) as f32, 16.0, 26.0, 14.0, 'I');
                im.rect(xx + 3, 14, 50, 19, 'I');
                im.ellipse((xx + 28) as f32, 14.0, 25.0, 11.0, 'r');
                im.ellipse((xx + 25) as f32, 12.0, 19.0, 8.0, 'o');
                im.rect(xx + 5, 22, 45, 3, 's');
                im.rect(xx + 9, 23, 32, 1, 'S');
                im.rect(xx + 25, 1, 6, 6, 'I');
                im.rect(xx + 26, 1, 2, 5, 'o');
                im.rect(xx + 18, 30, 17, 3, 's');
                im.rect(xx + 23, 31, 5, 5, 'o');
            }
            im.rect(x, 36, 240, 110, 's');
            im.rect(x + 1, 37, 238, 107, 'k');
            for y in [60, 88, 116] {
                im.rect(x + 2, y, 236, 1, 's');
            }
            for xx in [x + 10, x + 82, x + 154, x + 226] {
                im.line(xx, 38, xx + 14, 144, 's');
            }
            im.rect(x + 2, 145, 236, 1, 'S');
        }
        im
    })
}
fn room(d: &Lane, cells: bool) -> Img {
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    static CELL_WORLD: OnceLock<Img> = OnceLock::new();
    let w = if cells {
        CELL_WORLD.get_or_init(|| cell_backdrop(world()))
    } else {
        world()
    };
    for y in 0..STAGE_BOTTOM - STAGE_TOP {
        for x in 0..254 {
            if let Some(c) = w.get(d.camera + x, y) {
                im.set(x + 12, y + STAGE_TOP, c);
            }
        }
    }
    im.line(10, STAGE_BOTTOM, 267, STAGE_BOTTOM, 'S');
    im.rect(276, STAGE_TOP, 1, 159, 'S');
    im
}
fn plates() -> &'static [Option<Img>; 3] {
    static PLATES: OnceLock<[Option<Img>; 3]> = OnceLock::new();
    PLATES.get_or_init(|| {
        std::array::from_fn(|i| {
            let (name, w, h) = [("title", 140, 18), ("panel", 99, 158), ("panel", 352, 202)][i];
            skin(slot("brasswake.chrome", name), w, h)
        })
    })
}
fn name(run: &Run, id: u32, n: usize) -> String {
    run.players
        .get(&id)
        .map_or("KNIGHT", |h| h.name.as_str())
        .to_uppercase()
        .chars()
        .take(n)
        .collect()
}
fn center(im: &mut Img, x: i32, y: i32, text: &str, c: char) {
    im.text(x - text_width(text) / 2, y, text, c);
}
fn big(im: &mut Img, x: i32, y: i32, text: &str) {
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
            im.stamp(glyph, 148 + n as i32 * 12, 18);
        }
    }
}
fn standings(run: &Run, id: u32) -> Vec<(u32, i32)> {
    let s = run.brasswake.as_ref().unwrap();
    let mut order = s
        .lanes
        .keys()
        .copied()
        .filter(|who| *who != id)
        .collect::<Vec<_>>();
    order.sort_by_key(|who| (s.score(*who), *who));
    let mut rows = order
        .into_iter()
        .enumerate()
        .map(|(n, who)| (who, 72 + n as i32 * 24))
        .collect::<Vec<_>>();
    rows.push((id, 72 + rows.len() as i32 * 24 + 4));
    rows
}
fn text_labels(run: &Run, id: u32) -> Vec<(i32, i32, String)> {
    let s = run.brasswake.as_ref().unwrap();
    let d = &s.lanes[&id];
    if s.left == 0 {
        let mut words = vec![
            (
                122,
                24,
                if s.won {
                    "BREWERY CLEARED"
                } else {
                    "SHIFT FINISHED"
                }
                .into(),
            ),
            (151, 43, "WORK".into()),
            (214, 43, "FALLS".into()),
            (273, 43, "FINISH".into()),
        ];
        for (n, &who) in s.order.iter().enumerate() {
            let y = 62 + n as i32 * 24;
            let lane = &s.lanes[&who];
            words.push((29, y, s.place(who).to_string()));
            words.push((46, y, name(run, who, 13)));
            let score = lane.score.to_string();
            words.push((188 - text_width(&score), y, score));
            words.push((228, y, lane.falls.to_string()));
            words.push((
                273,
                y,
                lane.finish.map_or("--".into(), |t| {
                    format!("{}.{:02} S", t / HZ, (t % HZ) * 100 / HZ)
                }),
            ));
        }
        let leader = s.order[0];
        let tied = s
            .order
            .iter()
            .filter(|&&who| s.score(who) == s.score(leader))
            .count() as u32;
        let pot = GOLD_PER_SEAT * s.lanes.len() as u32;
        words.push((
            52,
            166,
            if tied == 1 {
                format!("{}  {}G + CROWN", name(run, leader, 13), pot)
            } else {
                format!("{}G EACH + CROWN", pot / tied)
            },
        ));
        if pot % tied != 0 {
            words.push((52, 177, format!("SPARE GOLD  {} +1G", name(run, leader, 9))));
        }
        words.push((44, 185, "FEWER FALLS, THEN EARLIER FINISH".into()));
        words.push((
            (NATIVE_W - text_width("E AGAIN")) / 2,
            195,
            "E AGAIN".into(),
        ));
        return words;
    }
    let mut words = vec![
        (192 - text_width("BRASSWAKE") / 2, 7, "BRASSWAKE".into()),
        (285, 30, "BRAWL THEN THROW".into()),
        (14, 8, format!("{} S", s.left.div_ceil(HZ))),
        (
            285,
            8,
            format!("{}G POT", GOLD_PER_SEAT * s.lanes.len() as u32),
        ),
        (285, 53, "BREWERY RACE".into()),
        (15, 194, "ARROWS WALK".into()),
        (110, 194, "F  PUNCH".into()),
        (
            208,
            194,
            if d.holding { "E  THROW" } else { "E  GRAB" }.into(),
        ),
        (14, 208, format!("AISLE {} / 4", d.wave + 1)),
        (
            136,
            208,
            if d.holding {
                "CASK READY"
            } else {
                "HOLD E GUARD"
            }
            .into(),
        ),
        (286, 176, format!("FALLS {}", d.falls)),
        (
            286,
            188,
            format!(
                "BEST {}",
                u32::from(run.home.countryside.games.brasswake.best) * WORK_PER_RECORD
            ),
        ),
    ];
    if compact_hud() {
        let elapsed = crate::rules::brasswake::LENGTH - s.left;
        words.retain(|&(x, y, _)| {
            !(x >= 280 && y >= 47 && y < 170)
                && !(elapsed >= hud()["compact"]["hide_hints_after"].as_u64().unwrap() as u32
                    && y == 194)
        });
        for (n, &who) in s.lanes.keys().enumerate() {
            words.push((
                strip_x(n) + 10,
                strip_y() + 1,
                format!("{} {}", s.place(who), s.lanes[&who].score),
            ));
        }
    } else {
        for (who, y) in standings(run, id) {
            let r = &s.lanes[&who];
            words.push((
                285,
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
            let score = r.score.to_string();
            words.push((370 - text_width(&score), y, score));
        }
    }
    if d.down > 0 {
        words.push((105, 20, "BACK UP".into()));
    } else if d.chain == 0 {
        words.push((
            105,
            20,
            if d.wave == 3 {
                "VAT KEEPER"
            } else {
                "CLEAR THE AISLE"
            }
            .into(),
        ));
    }
    words
}
fn draw(run: &Run, focus: Option<u32>, with_text: bool) -> (Img, Img) {
    let s = run.brasswake.as_ref().unwrap();
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next().unwrap());
    let d = &s.lanes[&id];
    let a = art();
    let mut im = room(d, !with_text);
    let fx = Img::new(NATIVE_W, NATIVE_H);
    if super::slot_art::enabled() {
        for (i, x, y) in [(0, 119, 2), (1, 280, 47)] {
            if i == 1 && compact_hud() && s.left > 0 {
                continue;
            }
            if let Some(plate) = &plates()[i] {
                im.stamp(plate, x, y);
            }
        }
    }
    if s.left == 0 {
        chrome::panel(
            &mut im,
            (16, 16, 352, 202),
            chrome::Tone {
                fill: &[[16, 26, 34]],
                ..chrome::Tone::BRASS
            },
        );
        if super::slot_art::enabled() {
            if let Some(plate) = &plates()[2] {
                im.stamp(plate, 16, 16);
            }
        }
    } else {
        if compact_hud() {
            im.rect(12, 33, 254, 12, 's');
            for (n, &who) in s.lanes.keys().enumerate() {
                let col = [[195, 47, 57], [45, 109, 192], [45, 143, 69], [193, 145, 35]]
                    [(who.saturating_sub(1) % 4) as usize];
                for y in strip_y() + 1..strip_y() + 8 {
                    for x in strip_x(n) + 2..strip_x(n) + 8 {
                        im.set(x, y, col);
                    }
                }
            }
        }
        for n in 0..6 {
            im.rect(17 + n * 11, 21, 8, 7, 's');
            if n < i32::from(d.hp) {
                im.rect(18 + n * 11, 22, 6, 4, 'r');
                im.rect(19 + n * 11, 22, 4, 2, 't');
            }
        }
        for (who, y) in standings(run, id) {
            if compact_hud() {
                continue;
            }
            let r = &s.lanes[&who];
            im.rect(284, y + 19, 88, 2, 's');
            im.rect(284, y + 19, 88 * r.x / 960, 2, 'o');
        }
        let shake = if d.freeze > 0 {
            if d.freeze % 2 == 0 { 1 } else { -1 }
        } else {
            0
        };
        let px = |x: i32| x - d.camera + 12 + shake;
        let py = |y: i32| y - STAGE_LIFT;
        // Danger rectangles are the rule rectangles, projected through this seat's camera.
        for e in &d.foes {
            if matches!(e.beat, Beat::Tell | Beat::Attack) {
                let z = e.danger;
                let x = px(z.x);
                let colour = if e.rush { 'U' } else { 'o' };
                im.rect(x, py(z.y), z.w, 2, colour);
                im.rect(x, py(z.y + z.h - 2), z.w, 2, colour);
                im.rect(x, py(z.y), 2, z.h, colour);
                im.rect(x + z.w - 2, py(z.y), 2, z.h, colour);
                if e.rush {
                    for xx in (4..z.w - 4).step_by(16) {
                        im.line(
                            x + xx,
                            py(z.y + z.h / 2 - 3),
                            x + xx + 5,
                            py(z.y + z.h / 2),
                            colour,
                        );
                        im.line(
                            x + xx + 5,
                            py(z.y + z.h / 2),
                            x + xx,
                            py(z.y + z.h / 2 + 3),
                            colour,
                        );
                    }
                } else {
                    im.line(
                        x + z.w / 2 - 5,
                        py(z.y + z.h / 2),
                        x + z.w / 2 + 5,
                        py(z.y + z.h / 2),
                        't',
                    );
                    im.line(
                        x + z.w / 2,
                        py(z.y + z.h / 2 - 4),
                        x + z.w / 2,
                        py(z.y + z.h / 2 + 4),
                        't',
                    );
                }
            }
        }
        for c in &d.incoming {
            let z = c.danger;
            let x = px(z.x);
            im.rect(x, py(z.y), z.w, 2, 't');
            im.rect(x, py(z.y + z.h - 2), z.w, 2, 't');
            im.rect(x, py(z.y), 2, z.h, 't');
            im.rect(x + z.w - 2, py(z.y), 2, z.h, 't');
            if c.tell > 0 {
                let lift = c.tell as i32 * 20 / run.tune.brass_tell as i32;
                im.stamp(&a.cask, x + z.w / 2 - 12, py(z.y - 22 - lift));
            } else {
                im.stamp(
                    &a.slam[((12 - c.burst) * 4 / 12).min(3) as usize],
                    x + z.w / 2 - 24,
                    py(z.y - 8),
                );
            }
        }
        if let Some(x) = d.cask {
            im.stamp(&a.cask, px(x) - 12, py(130));
        }
        if let Some(x) = d.bread {
            im.stamp(&a.bread, px(x) - 12, py(135));
        }
        // Foot-sorted cached bodies keep their original world feet and danger geometry.
        let mut bodies = d
            .foes
            .iter()
            .enumerate()
            .map(|(n, e)| (e.y, Some(n)))
            .collect::<Vec<_>>();
        bodies.push((d.y, None));
        bodies.sort_by_key(|v| v.0);
        for (_, which) in bodies {
            if let Some(n) = which {
                let e = &d.foes[n];
                let pose = if e.hp == 0 {
                    10
                } else if e.flinch > 0 {
                    7
                } else {
                    match e.beat {
                        Beat::Tell => {
                            if e.rush {
                                5
                            } else {
                                3
                            }
                        }
                        Beat::Attack => {
                            if e.rush {
                                6
                            } else {
                                4
                            }
                        }
                        Beat::Walk => 1 + (run.tick / 7 % 2) as usize,
                        Beat::Rest => {
                            if e.stage() > 0 {
                                7 + e.stage()
                            } else {
                                0
                            }
                        }
                        Beat::Defeat => 10,
                    }
                };
                let sprite = if !with_text {
                    match (e.boss, e.dir > 0) {
                        (true, false) => &a.keeper_cells[e.stage()][pose],
                        (true, true) => &a.keeper_cells_right[e.stage()][pose],
                        (false, false) => &a.porter_cells[e.stage()][pose],
                        (false, true) => &a.porter_cells_right[e.stage()][pose],
                    }
                } else {
                    match (e.boss, e.dir > 0) {
                        (true, false) => &a.keeper[e.stage()][pose],
                        (true, true) => &a.keeper_right[e.stage()][pose],
                        (false, false) => &a.porter[e.stage()][pose],
                        (false, true) => &a.porter_right[e.stage()][pose],
                    }
                };
                let extra_pose = if e.hp == 0 && e.flinch > 4 {
                    13
                } else if e.flinch == 0 && e.beat == Beat::Rest && e.ticks < 12 {
                    if e.rush { 12 } else { 11 }
                } else {
                    pose
                };
                let sprite = if e.boss {
                    a.damage_actions
                        .get(&(e.stage(), extra_pose))
                        .or_else(|| a.damage_actions.get(&(e.stage(), pose)))
                        .map(|bank| &bank[usize::from(e.dir > 0) + 2 * usize::from(!with_text)])
                        .unwrap_or(sprite)
                } else {
                    sprite
                };
                im.stamp(sprite, px(e.x) - sprite.w / 2, py(e.y - sprite.h));
                if e.hp > 0 {
                    let max = if e.boss { 32 } else { 10 };
                    let health_y = py(e.y - sprite.h - 5).max(STAGE_TOP + 2);
                    im.rect(px(e.x) - 16, health_y, 32, 3, 's');
                    im.rect(
                        px(e.x) - 15,
                        health_y + 1,
                        30 * i32::from(e.hp) / max,
                        1,
                        'o',
                    );
                }
                if e.beat == Beat::Attack {
                    let f = ((9 - e.ticks) * 4 / 9).min(3) as usize;
                    let z = e.danger;
                    im.stamp(
                        if e.rush { &a.steam[f] } else { &a.slam[f] },
                        px(z.x + z.w / 2) - if e.rush { 16 } else { 24 },
                        py(z.y + z.h - 32),
                    );
                }
            } else {
                let sprite = knight(run, id, usize::from(d.pose), d.dir, false, !with_text);
                im.stamp(&sprite, px(d.x) - 24, py(d.y - 48));
                if d.holding {
                    im.stamp(&a.cask, px(d.x) - 4, py(d.y - 52));
                }
            }
        }
        if d.foes
            .iter()
            .any(|e| !e.boss && e.hp > 0 && (e.x - d.x).abs() < 40 && (e.y - d.y).abs() < 44)
        {
            let sprite = knight(run, id, usize::from(d.pose), d.dir, true, !with_text);
            im.stamp(&sprite, px(d.x) - 24, py(d.y - 48));
            if d.holding {
                im.stamp(&a.cask, px(d.x) - 4, py(d.y - 52));
            }
        }
        if d.effect > 0 {
            im.stamp(
                &a.contact[((12 - d.effect) * 4 / 12).min(3) as usize],
                px(d.effect_x) - 16,
                py(d.effect_y - 16),
            );
        }
        if d.chain > 0 && d.down == 0 {
            if with_text {
                big(&mut im, 105, 18, &format!("COMBO X{}", d.chain));
            } else {
                cell_pop(&mut im, d.chain);
            }
        }
        if super::slot_art::enabled() {
            for (z, colour) in d
                .foes
                .iter()
                .filter(|e| matches!(e.beat, Beat::Tell | Beat::Attack))
                .map(|e| (e.danger, if e.rush { 'U' } else { 'o' }))
                .chain(d.incoming.iter().map(|c| (c.danger, 't')))
            {
                let x = px(z.x);
                let y = py(z.y);
                im.rect(x, y, z.w, 2, colour);
                im.rect(x, y + z.h - 2, z.w, 2, colour);
                im.rect(x, y, 2, z.h, colour);
                im.rect(x + z.w - 2, y, 2, z.h, colour);
            }
        }
        // Keep the aisle and its sidebar inside distinct gutters, even after shake.
        im.rect(0, STAGE_TOP, 12, STAGE_BOTTOM - STAGE_TOP, 'k');
        im.rect(266, STAGE_TOP, 10, STAGE_BOTTOM - STAGE_TOP, 'k');
    }
    if !with_text {
        for (x, y, t) in labels(run, Some(id)) {
            for yy in y / 8 * 8..y / 8 * 8 + 8 {
                for xx in x / 4 * 4..x / 4 * 4 + t.chars().count() as i32 * 4 {
                    im.set(xx, yy, [12, 18, 24]);
                }
            }
        }
    }
    if with_text {
        for (x, y, t) in text_labels(run, id) {
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
    let s = run.brasswake.as_ref().unwrap();
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next().unwrap());
    let mut words = text_labels(run, id);
    let d = &s.lanes[&id];
    if s.left > 0 && d.chain > 0 && d.down == 0 {
        words.push((105, 22, "COMBO".into()));
    }
    words
}
pub(super) fn sign(im: &mut Img, run: &Run) {
    if run.room().kind != crate::rules::RoomKind::Tavern || run.brasswake.is_some() {
        return;
    }
    chrome::panel(im, (276, 88, 100, 30), chrome::Tone::BRASS);
    center(im, 326, 94, "BRASSWAKE", 'O');
    center(im, 326, 106, "E PLAY", 'U');
    im.stamp(&art().cask, 348, 58);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeper_hit_reaction_takes_priority_over_late_rest_recovery() {
        super::super::slot_art::use_original(false);
        let mut run = Run::brasswake_review_for_test("flinch");
        let foe = &mut run
            .brasswake
            .as_mut()
            .unwrap()
            .lanes
            .get_mut(&1)
            .unwrap()
            .foes[0];
        foe.hp = 20;
        foe.beat = Beat::Rest;
        foe.flinch = 10;
        foe.ticks = 8;
        let late = draw(&run, Some(1), true);
        run.brasswake
            .as_mut()
            .unwrap()
            .lanes
            .get_mut(&1)
            .unwrap()
            .foes[0]
            .ticks = 20;
        assert_eq!(late, draw(&run, Some(1), true));
    }

    #[test]
    fn effects_and_overlaps_leave_the_exact_danger_border_visible() {
        super::super::slot_art::use_original(false);
        for moment in ["slam", "rush", "throw"] {
            let run = Run::brasswake_review_for_test(moment);
            let d = &run.brasswake.as_ref().unwrap().lanes[&1];
            let (frame, _) = draw(&run, Some(1), false);
            for (z, colour) in d
                .foes
                .iter()
                .filter(|e| matches!(e.beat, Beat::Tell | Beat::Attack))
                .map(|e| (e.danger, if e.rush { 'U' } else { 'o' }))
                .chain(d.incoming.iter().map(|c| (c.danger, 't')))
            {
                let x = z.x - d.camera + 12;
                let y = z.y - STAGE_LIFT;
                for yy in y..y + z.h {
                    for xx in x..x + z.w {
                        if yy < y + 2 || yy >= y + z.h - 2 || xx < x + 2 || xx >= x + z.w - 2 {
                            assert_eq!(frame.get(xx, yy), ink(colour), "{moment} at {xx},{yy}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn all_four_knights_keep_their_identity_when_the_porter_overlaps() {
        super::super::slot_art::use_original(false);
        let run = Run::brasswake_review_for_test("overlap");
        for pose in [0, 3, 5, 7, 9, 10] {
            let sprites: Vec<_> = (1..=4)
                .map(|seat| knight(&run, seat, pose, 1, true, false))
                .collect();
            for sprite in &sprites {
                assert_eq!((sprite.w, sprite.h), (48, 48));
            }
            for a in 0..4 {
                for b in a + 1..4 {
                    assert_ne!(sprites[a], sprites[b], "pose {pose}: seats {a}/{b}");
                }
            }
        }
    }

    #[test]
    fn live_standings_use_podium_ties_and_keep_a_local_gutter() {
        let run = Run::brasswake_review_for_test("standings");
        let rows = standings(&run, 1);
        assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), [3, 2, 4, 1]);
        assert_eq!(rows[3].1 - rows[2].1, 28);
        let labels = text_labels(&run, 1);
        assert!(labels.iter().any(|(_, _, t)| t == "1 BRAN"));
        assert!(labels.iter().any(|(_, _, t)| t == "4 YOU"));
    }
}
