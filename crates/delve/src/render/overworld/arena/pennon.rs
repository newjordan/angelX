//! Pennon Rush: one native grain, cached battlement and per-seat sprite variants.
use super::super::{
    chrome,
    ink::{Img, ink},
};
#[cfg(test)]
use super::rafterwake::attack_stamp;
use super::rafterwake::{
    center, danger_art, flip, image, label_wells, outline, recolour, slot_image, slot_knight_pose,
};
use super::{NATIVE_H, NATIVE_W};
use crate::rules::{
    Run,
    pennon::{Beat, Foe, Q},
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
    linear_features: Vec<Linear>,
}
#[derive(serde::Deserialize)]
struct Linear {
    name: String,
    ink: char,
    pixels: Vec<[i32; 2]>,
}
struct Sprite {
    body: Img,
    details: Img,
}
struct Art {
    all: BTreeMap<String, Sprite>,
    knights: [Img; 12],
}
fn art() -> &'static Art {
    static ART: [OnceLock<Art>; 2] = [const { OnceLock::new() }; 2];
    ART[usize::from(super::slot_art::enabled())].get_or_init(|| {
        let recipe: Recipe =
            serde_json::from_str(include_str!("../../../../assets/pennon/native.json"))
                .expect("Pennon native recipe");
        let mut decoded = BTreeMap::new();
        let mut all: BTreeMap<_, _> = recipe
            .assets
            .into_iter()
            .map(|(key, a)| {
                let body = image(&a.rows, &recipe.inks);
                let mut details = Img::new(body.w, body.h);
                for f in a.linear_features {
                    if f.name.contains("mast")
                        || f.name.contains("crest")
                        || f.name.contains("banner")
                        || f.name.contains("ladder hook")
                        || f.name.contains("vane")
                        || f.name.contains("bow")
                    {
                        for [x, y] in f.pixels {
                            if a.rows[y as usize].as_bytes()[x as usize] == f.ink as u8 {
                                details.set(x, y, recipe.inks[&f.ink]);
                            }
                        }
                    }
                }
                let slot = if key.starts_with("knight-") {
                    "pennon.knight"
                } else if key.starts_with("captain-") {
                    "pennon.captain"
                } else if key.starts_with("sentry-") {
                    "pennon.sentry"
                } else if key.starts_with("fx-drop-") {
                    "pennon.drop"
                } else if key.starts_with("fx-hook-") {
                    "pennon.hook"
                } else if key.starts_with("fx-impact-") {
                    "pennon.impact"
                } else if key.starts_with("fx-dust-") {
                    "pennon.dust"
                } else {
                    match key.as_str() {
                        "bolt" => "pennon.bolt",
                        "enemy-bolt" => "pennon.enemy-bolt",
                        "repair" => "pennon.repair",
                        "merlon" => "pennon.merlon",
                        "brick" => "pennon.brick",
                        "pennant" => "pennon.pennant",
                        _ => "",
                    }
                };
                let mut source_key = key.clone();
                if key.starts_with("captain-") {
                    if let Some((base, _)) = key.rsplit_once("-stage-") {
                        source_key = base.into();
                    }
                    if let Some((base, stage)) = source_key.rsplit_once('-') {
                        if base.ends_with("-tell") && stage.parse::<u8>().is_ok() {
                            source_key = base.into();
                        }
                    }
                }
                let selected = super::slot_art::enabled()
                    && super::slot_art::bytes(slot, &source_key).is_some();
                let imported = if selected {
                    decoded
                        .entry((slot, source_key.clone()))
                        .or_insert_with(|| slot_image(slot, &source_key, &body))
                        .clone()
                } else {
                    body.clone()
                };
                if selected {
                    assert_eq!((body.w, body.h), (imported.w, imported.h));
                    details = Img::new(body.w, body.h);
                }
                let imported =
                    if selected && (key.starts_with("fx-hook-") || key.starts_with("fx-drop-")) {
                        danger_art(&imported)
                    } else {
                        imported
                    };
                (
                    key,
                    Sprite {
                        body: imported,
                        details,
                    },
                )
            })
            .collect();
        if super::slot_art::enabled() {
            for name in [
                "idle",
                "idle-shift",
                "walk-1",
                "walk-2",
                "limp-1",
                "limp-2",
                "tell",
                "attack",
                "recover",
                "flinch",
                "stagger",
                "last-stand",
            ] {
                if super::slot_art::bytes("pennon.sentry-damaged", name).is_some() {
                    let body = slot_image("pennon.sentry-damaged", name, &all["sentry-idle"].body);
                    let details = Img::new(body.w, body.h);
                    all.insert(format!("sentry-damaged-{name}"), Sprite { body, details });
                }
            }
        }
        let knights = std::array::from_fn(|p| {
            let mut im = all[&format!(
                "knight-{}",
                [
                    "idle", "run-1", "run-2", "jump", "fall", "land", "aim-up", "aim-down", "hit",
                    "victory", "duck", "shoot"
                ][p]
            )]
                .body
                .clone();
            if super::slot_art::enabled()
                && super::slot_art::bytes(
                    "pennon.knight",
                    &format!(
                        "knight-{}",
                        [
                            "idle", "run-1", "run-2", "jump", "fall", "land", "aim-up", "aim-down",
                            "hit", "victory", "duck", "shoot"
                        ][p]
                    ),
                )
                .is_some()
            {
                return im;
            }
            let from = ['e', 'f', 'g', 'h'];
            let to = ['I', 'r', 'o', 't'];
            for y in 0..im.h {
                for x in 0..im.w {
                    if let Some(c) = im.get(x, y)
                        && let Some(n) = from.iter().position(|k| recipe.inks[k] == c)
                    {
                        im.set(x, y, ink(to[n]).unwrap());
                    }
                }
            }
            im
        });
        Art { all, knights }
    })
}
struct Seat {
    imported: bool,
    colours: Vec<(char, char)>,
    poses: [[Img; 12]; 2],
    outlines: [[Img; 12]; 2],
}
fn seat_colour(run: &Run, id: u32) -> &[(char, char)] {
    let h = &run.players[&id];
    if h.colours().is_empty() {
        super::knights::seat_colours(id)
    } else {
        h.colours()
    }
}
pub(super) fn knight(run: &Run, id: u32, pose: usize, dir: i32, overlap: bool) -> Img {
    static SEATS: OnceLock<Mutex<Vec<Seat>>> = OnceLock::new();
    let colours = seat_colour(run, id);
    let mut seats = SEATS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap();
    let at = seats
        .iter()
        .position(|s| s.colours == colours && s.imported == super::slot_art::enabled())
        .unwrap_or_else(|| {
            let right = std::array::from_fn(|p| {
                let key = format!(
                    "knight-{}",
                    [
                        "idle", "run-1", "run-2", "jump", "fall", "land", "aim-up", "aim-down",
                        "hit", "victory", "duck", "shoot"
                    ][p]
                );
                let base = &art().knights[p];
                if super::slot_art::enabled()
                    && super::slot_art::bytes("pennon.knight", &key).is_some()
                {
                    slot_knight_pose(base, colours, false, p)
                } else {
                    recolour(base.clone(), colours)
                }
            });
            let left = std::array::from_fn(|p| flip(&right[p]));
            let contour = ink(colours
                .iter()
                .find(|(f, _)| *f == '7')
                .map_or('7', |(_, t)| *t))
            .unwrap();
            let outlines =
                [&right, &left].map(|a| std::array::from_fn(|p| outline(&a[p], contour)));
            seats.push(Seat {
                imported: super::slot_art::enabled(),
                colours: colours.to_vec(),
                poses: [right, left],
                outlines,
            });
            seats.len() - 1
        });
    let poses = if overlap {
        &seats[at].outlines
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
    let s = run.pennon.as_ref()?;
    if s.left == 0 {
        return None;
    }
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next()?);
    let d = &s.lanes[&id];
    if !d.foes.iter().any(|f| (d.x / Q - f.x).abs() < 48) {
        return None;
    }
    ink(seat_colour(run, id)
        .iter()
        .find(|(f, _)| *f == '7')
        .map_or('7', |(_, t)| *t))
}

fn foe_pose(f: &Foe, tick: u64) -> usize {
    if f.hp == 0 {
        10
    } else if f.flinch > 0 {
        7
    } else {
        match f.beat {
            Beat::Walk => 1 + (tick / 7 % 2) as usize,
            Beat::Tell => {
                if f.hook {
                    5
                } else {
                    3
                }
            }
            Beat::Attack => {
                if f.hook {
                    6
                } else {
                    4
                }
            }
            Beat::Rest => {
                if f.ticks < 12 {
                    11
                } else if f.stage() > 0 {
                    7 + f.stage()
                } else {
                    0
                }
            }
            Beat::Defeat => 10,
        }
    }
}

mod actors;
mod finish;
mod fx;
mod hud;
mod moments;
mod scene;

use actors::Cx;
use hud::Words;

/// One frame of the siege for the seat looked through: the picture, and the
/// small words the picture carries (drawn into it, or handed to the terminal).
fn frame(run: &Run, focus: Option<u32>) -> (Img, Words) {
    let s = run.pennon.as_ref().unwrap();
    let id = focus.filter(|id| s.lanes.contains_key(id)).unwrap_or(*s.lanes.keys().next().unwrap());
    let d = &s.lanes[&id];
    #[cfg(test)]
    hud::forget();
    let mut words = Words::default();
    let over = finish::age(s).filter(|_| s.left == 0);
    if let Some(e) = over.filter(|&e| e >= finish::FINALE) {
        return (finish::scene_for(run, s, id, e, &mut words), words);
    }
    let t = s.show.t;
    let cam = hud::camera(s, d);
    let cx = Cx { run, s, d, id, cam, t, tick: run.tick };
    let shake = moments::shake(run, &cx);
    let ids: Vec<u32> = s.lanes.keys().copied().collect();
    // The world, shaken as one: its sky, its wall, its figures and its dust.
    let mut world = Img::black(NATIVE_W, NATIVE_H);
    scene::paint(&mut world, d.seed, cam, t);
    fx::behind(&mut world, &cx);
    actors::draw(&mut world, &cx);
    fx::front(&mut world, &cx);
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    im.stamp(&world, shake.0, shake.1);
    moments::mend_edges(&mut im, shake);
    // The strips and what is thrown over the picture stay still.
    hud::top(&mut im, s, d, id, &mut words, t);
    hud::bottom(&mut im, run, s, id, &mut words);
    let last = s.show.struck(id, 3).map(|h| h.age(t));
    hud::captain(&mut im, d, last);
    let grown = s.show.hits.iter().filter(|h| h.seat == id).map(|h| h.age(t)).min();
    let lost = s.show.downs.iter().rev().find(|k| k.seat == id && !k.ward).map(|k| (k.lost, t.saturating_sub(k.born)));
    hud::combo(&mut im, d, grown, lost);
    fx::rim(&mut im, &cx);
    fx::numbers(&mut im, &cx, shake, &ids);
    if over.is_none() {
        moments::callout(&mut im, run, &cx);
        moments::celebrate(&mut im, &cx);
        moments::last_seconds(&mut im, &cx);
        moments::hints(&mut im, &mut words, &cx);
    }
    moments::opening(&mut im, &cx);
    if let Some(e) = over {
        finish::finale(&mut im, run, s, id, e);
    }
    (im, words)
}
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let (mut im, words) = frame(run, focus);
    words.draw(&mut im);
    (im, Img::new(NATIVE_W, NATIVE_H))
}
pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let (mut im, words) = frame(run, focus);
    let mut fx = Img::new(NATIVE_W, NATIVE_H);
    label_wells(&mut im, &mut fx, &words.plain());
    (im, fx)
}
pub(super) fn labels(run: &Run, focus: Option<u32>) -> Vec<(i32, i32, String)> {
    frame(run, focus).1.plain()
}

pub(super) fn sign(im: &mut Img, run: &Run) {
    if run.room().kind != crate::rules::RoomKind::Muster || run.pennon.is_some() {
        return;
    }
    chrome::panel(im, (124, 30, 144, 48), chrome::Tone::BRASS);
    center(im, 196, 36, "SIEGE MUSTER", 'O');
    center(im, 196, 50, "PENNON RUSH", 'o');
    center(im, 196, 64, "E PLAY", 'S');
    im.stamp(&art().all["pennant"].body, 180, 110);
}

#[cfg(test)]
mod slot_tests {
    use super::*;
    #[test]
    fn source_banks_keep_registered_canvases_and_seat_identity() {
        super::super::slot_art::use_original(false);
        let a = art();
        for (name, w, h) in [
            ("knight-idle", 48, 48),
            ("captain-idle", 64, 80),
            ("sentry-idle", 48, 48),
            ("brick", 32, 16),
            ("bolt", 16, 8),
        ] {
            let im = &a.all[name].body;
            assert_eq!((im.w, im.h), (w, h));
            assert!(im.pixels().any(|c| c != [0, 0, 0]));
        }
        let seats: Vec<_> = (1..=4)
            .map(|id| {
                slot_knight_pose(
                    &a.knights[0],
                    super::super::knights::seat_colours(id),
                    false,
                    0,
                )
            })
            .collect();
        for i in 0..4 {
            for j in i + 1..4 {
                assert_ne!(seats[i], seats[j]);
            }
        }
    }
    #[test]
    fn captain_rule_poses_have_no_mixed_style_holes() {
        super::super::slot_art::use_original(false);
        for state in [
            "idle",
            "walk-1",
            "walk-2",
            "drop-tell",
            "drop",
            "hook-tell",
            "hook",
            "flinch",
            "damage-1",
            "damage-2",
            "defeat",
            "recover",
        ] {
            assert!(
                super::super::slot_art::bytes("pennon.captain", &format!("captain-{state}"))
                    .is_some(),
                "Captain {state} fell back"
            );
        }
        let a = art();
        for state in ["damage-1", "damage-2"] {
            let im = &a.all[&format!("sentry-{state}")].body;
            assert!(
                (0..20).any(|y| (0..im.w).any(|x| im.get(x, y).is_some())),
                "standing damage became a grounded slump"
            );
        }
        for (pose, source) in a.knights.iter().enumerate() {
            for id in 1..=4 {
                let dressed =
                    slot_knight_pose(source, super::super::knights::seat_colours(id), false, pose);
                for y in 0..source.h {
                    for x in 0..source.w {
                        assert_eq!(source.get(x, y).is_some(), dressed.get(x, y).is_some());
                        if let Some([r, g, b]) = source.get(x, y) {
                            // Dark blue cloth shares neutral shadow inks; protect neutral metal highlights.
                            if r.max(g).max(b) >= 100 && r.abs_diff(g) < 18 && g.abs_diff(b) < 18 {
                                assert_eq!(source.get(x, y), dressed.get(x, y));
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn decorative_attack_cannot_expand_the_danger_area() {
        let mut im = Img::black(64, 48);
        let mut fx = Img::new(64, 48);
        let mut effect = Img::new(48, 32);
        effect.rect(0, 0, 48, 32, 'o');
        attack_stamp(&mut im, &mut fx, &effect, 8, 8, (16, 20, 24, 12), true);
        for y in 0..48 {
            for x in 0..64 {
                let inside = (16..40).contains(&x) && (20..32).contains(&y);
                assert_eq!(
                    im.get(x, y),
                    Some(if inside { ink('o').unwrap() } else { [0, 0, 0] })
                );
                assert_eq!(fx.get(x, y).is_some(), inside);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__pennon_show__tests.rs"]
mod show_tests;
