//! Brasswake's copper brewery: one native pixel grain, cached art and aisle scenery.
use super::super::{
    chrome,
    ink::{Img, ink, text_width},
};
use super::{NATIVE_H, NATIVE_W};
use super::{juice, pfx};
use crate::rules::{
    Run,
    brasswake::{Beat, Foe, Lane, show::Kind},
    juice::Weight,
};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

mod finish;
mod fx;
mod hud;
mod life;
mod moments;
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
    #[allow(dead_code)]
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
const STAGE_TOP: i32 = hud::TOP;
const STAGE_BOTTOM: i32 = hud::BOTTOM;
/// A lane's own y is the screen's y: the aisle lies on the painted floor as it was painted.
const STAGE_LIFT: i32 = 0;
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
            // The painted brewery, row for row: vats and arches on the wall, the floor below.
            let room = dim(room);
            let mut im = Img::black(1200, STAGE_BOTTOM - STAGE_TOP);
            for y in 0..im.h {
                for x in 0..im.w {
                    if let Some(c) = room.get(x % 240, y * room.h / im.h) {
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
/// The aisle's left edge on the screen: the knight stands a third of the way across and the
/// brewery's far end never scrolls past the painted room.
fn view_x(d: &Lane) -> i32 {
    (d.x - 140).clamp(0, 1200 - NATIVE_W)
}
/// The aisle's backdrop at this seat's camera, the width of the screen.
fn room(d: &Lane, cells: bool) -> Img {
    // A window onto the stage's rows: nothing a blow throws up can leak into the bars.
    let mut im = Img::window(NATIVE_W, NATIVE_H, (0, STAGE_TOP, NATIVE_W, STAGE_BOTTOM - STAGE_TOP));
    static CELL_WORLD: OnceLock<Img> = OnceLock::new();
    let w = if cells {
        CELL_WORLD.get_or_init(|| cell_backdrop(world()))
    } else {
        world()
    };
    im.stamp(w, -view_x(d), STAGE_TOP);
    im
}
fn center(im: &mut Img, x: i32, y: i32, text: &str, c: char) {
    im.text(x - text_width(text) / 2, y, text, c);
}
/// The pose a foe stands in now, and its sprite.
fn foe_sprite(a: &'static Art, e: &Foe, tick: u64, with_text: bool) -> &'static Img {
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
            Beat::Walk => 1 + (tick / 7 % 2) as usize,
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
    if e.boss {
        a.damage_actions
            .get(&(e.stage(), extra_pose))
            .or_else(|| a.damage_actions.get(&(e.stage(), pose)))
            .map(|bank| &bank[usize::from(e.dir > 0) + 2 * usize::from(!with_text)])
            .unwrap_or(sprite)
    } else {
        sprite
    }
}
/// Where a foe's head is on the screen: the number a blow throws up starts there.
fn head(d: &Lane, a: &Art, n: usize) -> Option<(i32, i32)> {
    let e = d.foes.get(n)?;
    let h = if e.boss { a.keeper[0][0].h } else { a.porter[0][0].h };
    Some((e.x - view_x(d), e.y - STAGE_LIFT - h - 4))
}
/// The rule's danger rectangles, drawn exactly where they are and never covered.
fn danger_borders(im: &mut Img, d: &Lane, px: &dyn Fn(i32) -> i32) {
    for e in d.foes.iter().filter(|e| matches!(e.beat, Beat::Tell | Beat::Attack)) {
        let z = e.danger;
        let (x, y, c) = (px(z.x), z.y - STAGE_LIFT, if e.rush { 'U' } else { 'o' });
        for (rx, ry, rw, rh) in [(x, y, z.w, 2), (x, y + z.h - 2, z.w, 2), (x, y, 2, z.h), (x + z.w - 2, y, 2, z.h)] {
            im.rect(rx, ry, rw, rh, c);
        }
    }
    for k in &d.incoming {
        let z = k.danger;
        let (x, y) = (px(z.x), z.y - STAGE_LIFT);
        for (rx, ry, rw, rh) in [(x, y, z.w, 2), (x, y + z.h - 2, z.w, 2), (x, y, 2, z.h), (x + z.w - 2, y, 2, z.h)] {
            im.rect(rx, ry, rw, rh, 't');
        }
    }
}
/// The aisle at this tick: the room, its life, the floor's marks, the bodies in order of their
/// feet, every effect over them. Not yet shaken, and without the frame.
fn aisle(run: &Run, d: &Lane, id: u32, with_text: bool) -> Img {
    let a = art();
    let now = run.tick as u32;
    let c = fx::Ctx { d, now, cam: view_x(d) };
    let px = |x: i32| x - view_x(d);
    let py = |y: i32| y - STAGE_LIFT;
    let mut im = room(d, !with_text);
    life::paint(&mut im, c, run.tick);
    fx::floor(&mut im, c);
    fx::telegraph(&mut im, c, run.tune.brass_tell);
    fx::incoming_floor(&mut im, c, run.tune.brass_tell);
    danger_borders(&mut im, d, &px);
    // The pickups bob and twinkle.
    let bob = ((run.tick / 6) % 2) as i32;
    if let Some(x) = d.cask {
        fx::shadow(&mut im, px(x), d.y.clamp(126, 174) - 2, 9, 3, 0.55);
        im.stamp(&a.cask, px(x) - 12, py(130) - bob);
        let near = (x - d.x).abs() <= 28 && (d.y - 150).abs() <= 20 && !d.holding;
        if near || (run.tick / 12) % 3 == 0 {
            let mut pen = pfx::Pen::new(&mut im, 40);
            pen.twinkle((px(x) as f32 + 5.0, 124.0 - bob as f32), if near { 3.0 } else { 2.0 }, ((run.tick % 24) as f32) / 24.0, pfx::ramps::GOLD);
        }
    }
    if let Some(x) = d.bread {
        fx::shadow(&mut im, px(x), 150, 8, 3, 0.5);
        im.stamp(&a.bread, px(x) - 12, py(135) - bob);
        let mut pen = pfx::Pen::new(&mut im, 80);
        pen.wisp((px(x) as f32, 128.0), run.tick as f32, (0.0, 1.0e6), 18.0, 12.0, 2.0, fx::STEAM, 3);
        pen.twinkle((px(x) as f32 - 6.0, 126.0), 2.0, ((run.tick % 20) as f32) / 20.0, pfx::ramps::GOLD);
    }
    // Incoming casks fall out of the dark above the aisle onto their marks; one that broke leaves
    // its splash sprite for a moment.
    for k in &d.incoming {
        let z = k.danger;
        let x = px(z.x) + z.w / 2;
        if k.tell > 0 {
            let up = fx::fall_height(k.tell, run.tune.brass_tell);
            let spin = (k.tell / 3) % 2 == 0;
            let img = if spin { a.cask.clone() } else { a.cask.flip_h() };
            im.stamp(&img, x - 12, py(z.y - 22) - up);
        } else {
            im.stamp(&a.slam[((12 - k.burst) * 4 / 12).min(3) as usize], x - 24, py(z.y - 8));
        }
    }
    // Bodies, feet first.
    let mut bodies = d.foes.iter().enumerate().map(|(n, e)| (e.y, Some(n))).collect::<Vec<_>>();
    bodies.push((d.y, None));
    bodies.sort_by_key(|v| v.0);
    let strike = |n: usize| {
        d.show
            .marks
            .iter()
            .filter(|m| m.wave == d.wave && matches!(m.kind, Kind::Blow { foe, .. } if usize::from(foe) == n))
            .min_by_key(|m| m.age(now))
    };
    for (_, which) in bodies {
        let Some(n) = which else {
            fx::shadow(&mut im, px(d.x), d.y, 15, 4, 0.6);
            let struck = d.show.marks.iter().filter(|m| matches!(m.kind, Kind::Hurt { guarded: false, .. })).min_by_key(|m| m.age(now));
            let hit = struck.map(|m| m.age(now));
            let blink = d.hurt > 0 && d.down == 0 && hit.is_none_or(|a| a >= 3) && (now / 3) % 2 == 1;
            if !blink {
                let mut sprite = knight(run, id, usize::from(d.pose), d.dir, false, !with_text);
                let (mut slide, mut grow) = (0, 1.0);
                if let Some(m) = struck.filter(|m| m.age(now) < 14) {
                    // Knocked back: it shows where it was and slides home, squashed and stretched.
                    let age = m.age(now);
                    slide = i32::from(m.dir) * (run.tune.brass_shove as f32 * crate::motion::math::exp(-0.35 * age as f32)) as i32;
                    if age < 3 {
                        sprite = juice::impact_frame(&sprite, 'c');
                    } else {
                        let w = m.kind.weight();
                        let sq = juice::squash_stretch(&sprite, age, w.squash() * 0.7);
                        (sprite, grow) = sq;
                    }
                }
                let top = py(d.y) - (48.0 * grow).round() as i32;
                im.stamp(&sprite, px(d.x) - sprite.w / 2 + slide, top);
                if d.holding {
                    im.stamp(&a.cask, px(d.x) - 4 + slide, py(d.y - 52) - bob);
                }
                if d.combo == 2 && d.chain > 0 && d.down == 0 && !d.holding {
                    // The kick is next: a ring winds at the boot.
                    let mut pen = pfx::Pen::new(&mut im, 90);
                    let k = ((run.tick % 12) as f32) / 12.0;
                    pen.ring((px(d.x) as f32 + 6.0 * f32::from(d.dir as i8), d.y as f32 + 1.0), 8.0 + 8.0 * (1.0 - k), 1.0, 0.5, pfx::ramps::EMBER, k * 0.6);
                }
            }
            continue;
        };
        let e = &d.foes[n];
        let sprite = foe_sprite(a, e, run.tick, with_text);
        let struck = strike(n);
        let mut body = sprite.clone();
        let mut grow = 1.0;
        let mut slide = 0;
        if let Some(m) = struck {
            let age = m.age(now);
            let w = m.kind.weight();
            if age < w.flash() {
                body = juice::impact_frame(&body, 'H');
            } else if age < 14 {
                (body, grow) = juice::squash_stretch(sprite, age, w.squash());
            }
            let way = i32::from(m.dir);
            slide = -way * (run.tune.brass_shove as f32 * crate::motion::math::exp(-0.35 * age as f32)) as i32;
        }
        let top = py(e.y) - (sprite.h as f32 * grow).round() as i32;
        let x = px(e.x) + slide - body.w / 2;
        fx::shadow(&mut im, px(e.x) + slide, e.y, if e.boss { 26 } else { 15 }, if e.boss { 6 } else { 4 }, 0.6);
        // A fallen porter dissolves away; the keeper stays where it lay.
        let mut fade = if e.hp == 0 && !e.boss && e.ticks >= 30 { ((e.ticks - 30) as f32 / 30.0).min(1.0) } else { 0.0 };
        if e.hp == 0 && !e.boss && e.ticks >= 60 {
            fade = 1.0;
        }
        if fade < 1.0 {
            if fade > 0.0 {
                juice::dissolve(&mut body, fade);
            }
            im.stamp(&body, x, top);
        }
        if e.hp > 0 {
            let max = if e.boss { 32 } else { 10 };
            let health_y = (py(e.y - sprite.h - 5)).max(STAGE_TOP + 2);
            im.rect(px(e.x) - 16, health_y, 32, 3, 's');
            im.rect(px(e.x) - 15, health_y + 1, 30 * i32::from(e.hp) / max, 1, 'o');
        }
        if e.beat == Beat::Tell && e.ticks < 10 && (e.ticks / 2) % 2 == 0 {
            // The warning over the head: one more heartbeat and it comes down.
            let (hx, hy) = (px(e.x), py(e.y - sprite.h) - 10);
            im.rect(hx - 1, hy, 3, 6, '7');
            im.rect(hx - 1, hy + 8, 3, 2, '7');
            im.rect(hx, hy + 1, 1, 4, 'c');
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
    }
    if d.foes.iter().any(|e| !e.boss && e.hp > 0 && (e.x - d.x).abs() < 40 && (e.y - d.y).abs() < 44) {
        let sprite = knight(run, id, usize::from(d.pose), d.dir, true, !with_text);
        im.stamp(&sprite, px(d.x) - 24, py(d.y - 48));
        if d.holding {
            im.stamp(&a.cask, px(d.x) - 4, py(d.y - 52) - bob);
        }
    }
    // Every effect over them.
    fx::swoosh(&mut im, c, run.tick);
    for m in &d.show.marks {
        match m.kind {
            Kind::Blow { .. } if m.wave == d.wave => fx::blow(&mut im, c, m, run.tick),
            Kind::Hurt { .. } => fx::hurt(&mut im, c, m, run.tick),
            Kind::Quake { .. } => fx::quake(&mut im, c, m, run.tick),
            Kind::Smash { .. } => fx::smash(&mut im, c, m, run.tick),
            Kind::Bread => fx::bread(&mut im, c, m, run.tick),
            Kind::Grab => fx::heft(&mut im, c, m, run.tick),
            Kind::Throw => {
                fx::heft(&mut im, c, m, run.tick);
                fx::thrown(&mut im, c, &a.cask, m);
            }
            Kind::Step => fx::step(&mut im, c, m, run.tick),
            Kind::Fall | Kind::Rise => fx::tumble(&mut im, c, m, run.tick),
            _ => {}
        }
    }
    // The rule's rectangles come last: nothing a blow throws up hides the exact danger.
    danger_borders(&mut im, d, &px);
    // The gate's way on, and a keeper's win.
    if d.foes.iter().all(|e| e.hp == 0) && d.wave < 3 && d.finish.is_none() {
        let wiggle = ((run.tick / 4) % 2) as i32 * 2;
        for k in 0..3 {
            let x = 340 + k * 8 + wiggle;
            for t in 0..5 {
                im.put(x + t, 100 + t, 't');
                im.put(x + t, 108 - t, 't');
            }
        }
    }
    if hud::keeper_down(d) {
        let since = d.show.latest(|m| matches!(m.kind, Kind::Blow { boss: true, slain: true, .. }).then(|| m.age(now)));
        juice::confetti(&mut im, (0, STAGE_TOP, NATIVE_W, STAGE_BOTTOM - STAGE_TOP), run.tick as u32, 0x77, 22);
        if let Some(age) = since {
            for (n, x) in [96.0, 200.0, 296.0].into_iter().enumerate() {
                if age >= n as u32 * 9 {
                    juice::firework(&mut im, x, 120.0, 62.0 + n as f32 * 6.0, age - n as u32 * 9, 0x31 + n as u32, pfx::ramps::GOLD, run.tick);
                }
            }
        }
    }
    im
}
fn draw(run: &Run, focus: Option<u32>, with_text: bool) -> (Img, Img) {
    let s = run.brasswake.as_ref().unwrap();
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next().unwrap());
    let d = &s.lanes[&id];
    let now = run.tick as u32;
    let fx_layer = Img::new(NATIVE_W, NATIVE_H);
    let scene = aisle(run, d, id, with_text);
    if s.left == 0 {
        let sprite = |who: u32| knight(run, who, 0, 1, false, !with_text);
        let mut im = finish::paint(&scene, run, s, id, &sprite, with_text);
        if !with_text {
            blank_labels(&mut im, run, id);
        }
        return (im, fx_layer);
    }
    let blows = d
        .show
        .marks
        .iter()
        .map(|m| (m.kind.weight(), m.age(now)))
        .filter(|(w, _)| *w >= Weight::Solid);
    let (dx, dy) = juice::shake(blows, run.tick);
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    im.stamp(&scene, dx, dy);
    if dy != 0 {
        // A shaken stage slides over its bars' edges: put the paper back.
        im.rect(0, STAGE_TOP - 9, NATIVE_W, 9, 'k');
        im.rect(0, STAGE_BOTTOM + 1, NATIVE_W, 9, 'k');
    }
    fx::rim(&mut im, fx::Ctx { d, now, cam: view_x(d) }, run.tick);
    hud::paint(&mut im, run, s, id);
    let a = art();
    let anchor = |n: usize| head(d, a, n);
    moments::sparks(&mut im, d, now, &anchor);
    let plan = moments::plan(run, s, id, &anchor);
    moments::paint(&mut im, &plan);
    if with_text {
        for w in &plan.words {
            im.text(w.x, w.y, &w.text, w.ink);
        }
    } else {
        blank_labels(&mut im, run, id);
    }
    (im, fx_layer)
}
/// The terminal paints its own words: keep their cells clear of the pictures.
fn blank_labels(im: &mut Img, run: &Run, id: u32) {
    for (x, y, t) in labels(run, Some(id)) {
        for yy in y / 8 * 8..y / 8 * 8 + 8 {
            for xx in x / 4 * 4..x / 4 * 4 + t.chars().count() as i32 * 4 {
                im.set(xx, yy, [12, 18, 24]);
            }
        }
    }
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
    let words = if s.left == 0 { finish::words(run, s, id) } else { hud::words(run, s, id) };
    words.into_iter().map(|w| (w.x, w.y, w.text)).collect()
}
/// Every box of words on seat `id`'s screen at this tick, named: what the words test walks.
#[cfg(test)]
pub(super) fn boxes(run: &Run, id: u32) -> Vec<(&'static str, hud::Rect)> {
    let s = run.brasswake.as_ref().unwrap();
    let d = &s.lanes[&id];
    if s.left == 0 {
        return finish::boxes(run, s, id);
    }
    let a = art();
    let anchor = |n: usize| head(d, a, n);
    let plan = moments::plan(run, s, id, &anchor);
    let mut out: Vec<(&'static str, hud::Rect)> = plan.words.iter().map(|w| ("word", w.rect())).collect();
    out.push(("score", hud::SCORE_BOX));
    if hud::combo_shown(d) {
        out.push(("combo", hud::COMBO_BOX));
    }
    out.extend(moments::boxes(&plan));
    out
}
pub(super) fn sign(im: &mut Img, run: &Run) {
    // Under the rumours, when a knight reads them, its E PLAY would stand
    // alone below them as if the board were the game.
    if run.room().kind != crate::rules::RoomKind::Tavern || run.brasswake.is_some() || super::tavern::reading_rumours(run) {
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
                let x = z.x - view_x(d);
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
    fn live_standings_rank_by_work_then_falls_and_keep_the_local_seat_named_you() {
        let run = Run::brasswake_review_for_test("standings");
        let s = run.brasswake.as_ref().unwrap();
        // All four tie on work; seat 3 finished, seat 1 fell twice.
        assert_eq!(hud::ranked(s), [3, 2, 4, 1]);
        let words = hud::words(&run, s, 1);
        assert!(words.iter().any(|w| w.text == "YOU"));
        assert!(words.iter().any(|w| w.text == "BRAN"));
        assert_eq!(hud::short("Knight 2"), "K2");
        assert_eq!(hud::short("Gawain"), "GAWA");
    }

    fn table(seats: u32) -> Run {
        super::super::slot_art::use_original(false);
        let mut r = Run::brasswake_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        if seats == 1 {
            r.leave(2);
        }
        r.step(&std::collections::BTreeMap::from([(
            1,
            crate::rules::Input { interact: true, ..Default::default() },
        )]));
        assert!(r.brasswake.is_some());
        r
    }

    /// No two boxes of words meet and none leaves the 384 by 224 screen; every line the 5x7 pen
    /// wrote on the screen is one of the claimed boxes.
    fn assert_words_clear(r: &Run, what: &str) {
        let s = r.brasswake.as_ref().unwrap();
        for &id in s.lanes.keys() {
            let boxes = boxes(r, id);
            for (i, (a, ra)) in boxes.iter().enumerate() {
                assert!(hud::inside(*ra), "{what} seat {id}: {a} {ra:?} leaves the screen");
                for (b, rb) in &boxes[i + 1..] {
                    assert!(!hud::meets(*ra, *rb), "{what} seat {id}: {a} {ra:?} meets {b} {rb:?}");
                }
            }
        }
    }

    fn assert_pen_inside_claims(r: &Run, what: &str) {
        use super::super::super::ink::said;
        let s = r.brasswake.as_ref().unwrap();
        for &id in s.lanes.keys() {
            let claimed: Vec<_> = boxes(r, id).into_iter().filter(|(k, _)| matches!(*k, "word" | "podium name" | "podium line")).map(|b| b.1).collect();
            let (_, said) = said::read(|| draw(r, Some(id), true));
            for line in said.lines.iter().filter(|l| l.on == (384, 224)) {
                let rect = (line.at.0, line.at.1, text_width(&line.text), 7);
                assert!(hud::inside(rect), "{what} seat {id}: {:?} leaves the screen", line.text);
                assert!(claimed.contains(&rect), "{what} seat {id}: {:?} at {rect:?} is not a claimed box", line.text);
            }
        }
    }

    /// Where a frame's time goes, over a bot's round: microseconds a phase, mean.
    #[test]
    #[ignore = "measures the picture's cost"]
    fn brasswake_phase_times() {
        use std::time::Instant;
        let mut r = table(4);
        let mut t = [0u128; 6];
        let mut n = 0u128;
        for tick in 0..1500u32 {
            let s = r.brasswake.as_ref().unwrap();
            let keys = s.lanes.iter().map(|(&id, l)| (id, l.bot_input(crate::rules::brasswake::LENGTH - s.left))).collect();
            r.step(&keys);
            if tick % 3 != 0 || tick < 20 {
                continue;
            }
            let s = r.brasswake.as_ref().unwrap();
            let d = &s.lanes[&1];
            let c = fx::Ctx { d, now: r.tick as u32, cam: view_x(d) };
            let at = Instant::now();
            let _ = room(d, false);
            t[0] += at.elapsed().as_micros();
            let at = Instant::now();
            let mut sink = Img::window(384, 224, (0, 34, 384, 146));
            life::paint(&mut sink, c, r.tick);
            t[1] += at.elapsed().as_micros();
            let at = Instant::now();
            let _ = aisle(&r, d, 1, true);
            t[2] += at.elapsed().as_micros();
            let at = Instant::now();
            let mut im = Img::black(384, 224);
            hud::paint(&mut im, &r, s, 1);
            t[3] += at.elapsed().as_micros();
            let at = Instant::now();
            let _ = draw(&r, Some(1), true);
            t[4] += at.elapsed().as_micros();
            let at = Instant::now();
            let a = art();
            let anchor = |n: usize| head(d, a, n);
            let _ = moments::plan(&r, s, 1, &anchor);
            t[5] += at.elapsed().as_micros();
            n += 1;
        }
        println!("per frame (us): room {} life {} aisle {} hud {} draw {} plan {}", t[0] / n, t[1] / n, t[2] / n, t[3] / n, t[4] / n, t[5] / n);
    }

    #[test]
    fn the_terminal_path_draws_every_moment_with_its_words_left_to_the_cells() {
        super::super::slot_art::use_original(false);
        for moment in ["kick", "slam", "rush", "throw", "flinch", "defeat", "down", "guard", "standings", "podium", "wave-2"] {
            let run = Run::brasswake_review_for_test(moment);
            let (rgb, _) = layers(&run, Some(1));
            let (cells, _) = terminal_layers(&run, Some(1));
            assert_eq!((rgb.w, rgb.h), (384, 224), "{moment}");
            assert_eq!((cells.w, cells.h), (384, 224), "{moment}");
            let words = labels(&run, Some(1));
            assert!(!words.is_empty(), "{moment}");
            // Where the terminal prints a word the picture keeps its cells clear.
            for (x, y, t) in words {
                assert_eq!(cells.get(x / 4 * 4, y / 8 * 8), Some([12, 18, 24]), "{moment}: {t}");
            }
        }
    }

    #[test]
    fn the_words_never_overlap_and_every_prompt_sits_inside_the_screen() {
        for seats in 1..=6u32 {
            let mut r = table(seats);
            let mut t = 0u32;
            loop {
                let Some(s) = r.brasswake.as_ref() else { break };
                if s.left == 0 && t > 0 && t % 5 == 0 && s.done + 200 < r.tick as u32 {
                    break;
                }
                let elapsed = crate::rules::brasswake::LENGTH - s.left;
                let keys = s
                    .lanes
                    .iter()
                    // Seats start a little apart, so the table sees a lead change and a team kick.
                    .map(|(&id, l)| (id, if s.left > 0 && elapsed < (id - 1) * 18 { Default::default() } else { l.bot_input(elapsed) }))
                    .collect();
                r.step(&keys);
                t += 1;
                assert_words_clear(&r, &format!("{seats} seats, tick {t}"));
                if t % 7 == 0 {
                    assert_pen_inside_claims(&r, &format!("{seats} seats, tick {t}"));
                }
                if t > 2600 {
                    break;
                }
            }
            assert!(r.brasswake.as_ref().unwrap().left == 0, "{seats} seats: the round ended");
        }
    }

    #[test]
    fn a_storm_of_numbers_calls_and_long_names_still_leaves_every_word_clear() {
        use crate::rules::brasswake::show::{Cry, Kind};
        let mut r = table(6);
        for (id, name) in [(1, "Wolfgang Amadeus"), (2, "Bartholomew"), (3, "Ælfric"), (4, "X"), (5, "Guinevere Of Orkney"), (6, "Knight 6")] {
            r.players.get_mut(&id).unwrap().name = name.into();
        }
        let now = r.tick as u32;
        let s = r.brasswake.as_mut().unwrap();
        s.left -= 600;
        for (&id, d) in &mut s.lanes {
            d.score = 99_000 + id * 1_111;
            d.chain = 987;
            d.wave = 0;
            for (n, e) in d.foes.iter_mut().enumerate() {
                e.x = d.x + 30 + n as i32 * 12;
            }
            for k in 0..14 {
                d.show.mark(Kind::Blow { swing: (k % 3) as u8, boss: false, slain: k % 4 == 0, gain: if k % 4 == 0 { 110 } else { 10 }, foe: (k % 2) as u8 }, 0, (d.foes[0].x, d.foes[0].y), 1, now.saturating_sub(k * 2));
            }
            d.show.mark(Kind::Streak { n: 985 }, 0, (d.x, d.y), 1, now);
            d.show.mark(Kind::Smash { hit: true, from: 6 }, 0, (d.x, d.y), 1, now);
        }
        for (n, cry) in [Cry::Lead, Cry::Keeper, Cry::Team, Cry::Slain, Cry::Fell].into_iter().enumerate() {
            s.show.call(cry, 1 + n as u32 % 6, now);
        }
        assert_words_clear(&r, "storm");
        assert_pen_inside_claims(&r, "storm");
        // The end screen, six seats, the longest names.
        let s = r.brasswake.as_mut().unwrap();
        s.left = 0;
        s.won = true;
        s.done = now;
        s.order = (1..=6).collect();
        for tick in [0u32, 30, 90, 400] {
            r.tick = u64::from(now + tick);
            assert_words_clear(&r, "end");
            assert_pen_inside_claims(&r, "end");
        }
    }
}

