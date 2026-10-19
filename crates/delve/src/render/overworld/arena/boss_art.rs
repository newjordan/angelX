//! Beside-the-boss studies and combat art, decoded and posed once.
use super::super::ink::Img;
use super::{at, fx};
use crate::rules::{Enemy, Run, bosses::Art};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

mod pit_debt;
mod library;
include!(concat!(env!("OUT_DIR"), "/boss_assets.rs"));
pub(super) use pit_debt::{leviathan_laser, ravage};
pub(super) use library::registered as library_registered;
pub(super) use library::in_cells as library_cells;
pub(super) use library::{ground as library_ground, ground_v2 as library_ground_v2, hidden as library_hidden};
#[cfg(feature = "testing")]
pub(super) use library::floor_review as library_floor_review;
#[cfg(feature = "boss-library-review")]
pub(super) use library::review as library_review;

pub(super) fn library_figure(run: &Run, enemy: &Enemy, art: Art, modern: bool) -> Option<Arc<Img>> {
    library::figure(run, enemy, art, modern, true)
}

/// Fill existing effect and display caches without touching a simulation.
pub(super) fn warm() {
    let modern = super::art_v2::enabled();
    for art in Art::ALL {
        let _ = kit(art);
        library::warm(art, modern);
    }
    let _ = warning_inks();
}

fn file(key: &str) -> &'static [u8] {
    BOSS_FILES.iter().find(|(name, _)| *name == key).expect("registered boss sprite").1
}

#[derive(serde::Deserialize)]
struct EffectSlots {
    projectile: [String; 2], tell: [String; 4], impact: [String; 5],
    #[serde(default)]
    cell_warning_priority: bool,
    #[serde(default)]
    cell_warning_inks: Option<Vec<[u8;3]>>,
    #[serde(default)]
    point_rim: Option<[[u8;3];4]>,
    #[serde(default)]
    projectile_v2: Option<[String; 2]>,
    #[serde(default)]
    display: Option<EffectDisplay>,
    #[serde(default)]
    projectile_signal_inks: Vec<[u8; 3]>,
}

#[derive(serde::Deserialize)]
struct EffectDisplay {
    v1: [i32; 2],
    v2: [i32; 2],
    #[serde(default)]
    study_v1: Vec<[i32;2]>,
    #[serde(default)]
    study_v2: Vec<[i32;2]>,
}

/// Registered warning pigments win a reduced footprint over decorative material.
/// This retains authored strokes when a larger body occupies the same cell.
pub(super) fn library_warning_inks(run: &Run) -> Option<&'static std::collections::BTreeSet<[u8; 3]>> {
    let flight = run.projectiles.iter().filter_map(Art::of_shot)
        .any(|art| !effect_slots(art.id()).projectile_signal_inks.is_empty());
    if !flight && !run.enemies.iter().any(|e| {
        e.boss.and_then(|i| run.bosses.get(i as usize))
            .and_then(|b| Art::for_id(&b.id))
            .is_some_and(|a| library::registered(a) && effect_slots(a.id()).cell_warning_priority)
            && (e.ring_warning().is_some() || e.arms_warning().is_some())
    }) { return None; }
    Some(warning_inks())
}

fn warning_inks() -> &'static std::collections::BTreeSet<[u8; 3]> {
    static INKS: OnceLock<std::collections::BTreeSet<[u8; 3]>> = OnceLock::new();
    INKS.get_or_init(|| {
        let slots: BTreeMap<String, EffectSlots> = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/boss-effect-slots.json"))).unwrap();
        let mut colours = std::collections::BTreeSet::new();
        for (id, slots) in slots {
            colours.extend(slots.projectile_signal_inks);
            if slots.cell_warning_priority {
                if let Some(inks)=slots.cell_warning_inks {
                    colours.extend(inks);
                    continue;
                }
                for frame in slots.tell {
                    let im = png(file(&format!("{id}/{frame}")));
                    for y in 0..im.h { for x in 0..im.w {
                        if let Some(c) = im.get(x,y) { colours.insert(c); }
                    }}
                }
            }
        }
        colours
    })
}

fn effect_slots(id: &str) -> &'static EffectSlots {
    static SLOTS: OnceLock<BTreeMap<String, EffectSlots>> = OnceLock::new();
    &SLOTS.get_or_init(|| serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/boss-effect-slots.json"))).expect("boss effect manifest"))[id]
}

fn pit_debt(art: Art) -> bool {
    matches!(art, Art::RedPick | Art::Foreman | Art::Leviathan)
}

#[derive(Clone)]
struct Effects {
    projectile: [[Img; 4]; 2],
    tell: [Img; 4],
    ring_glyph: [Img; 4],
    ring_segments: Vec<Vec<Img>>,
    impact: [Img; 5],
    lane: [Img; 2],
}

struct Kit {
    body: Img,
    study: Img,
    lights: Img,
    effects: [Effects; 2],
    projectile_v2: Option<[[Img; 4]; 2]>,
    #[cfg(any(test,feature="testing"))]
    projectile_trials: [OnceLock<Vec<[[Img;4];2]>>;2],
    figures: Mutex<BTreeMap<(i32, i32, bool), Arc<[Img; 4]>>>,
    light_figures: Mutex<BTreeMap<(i32, i32, bool), Arc<Img>>>,
    material_floors: Mutex<BTreeMap<(i32, i32, bool), Arc<Img>>>,
}

fn resize(src: &Img, w: i32, h: i32) -> Img {
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = src.get(x * src.w / w, y * src.h / h) {
                out.set(x, y, c);
            }
        }
    }
    out
}

fn turn(src: &Img) -> Img {
    let mut out = Img::new(src.h, src.w);
    for y in 0..src.h {
        for x in 0..src.w {
            if let Some(c) = src.get(x, y) {
                out.set(src.h - 1 - y, x, c);
            }
        }
    }
    out
}

fn png(png: &[u8]) -> Img {
    let png = image::load_from_memory(png)
        .expect("boss study PNG")
        .to_rgba8();
    let mut study = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, c) in png.enumerate_pixels() {
        if c[3] > 127 {
            study.set(x as i32, y as i32, [c[0], c[1], c[2]]);
        }
    }
    study
}

fn reliquary(art: Art) -> bool {
    matches!(art, Art::TaxKeeper | Art::RelicLiege | Art::IronTusk)
}

/// Rotate sparse authored arc/hook tiles during kit construction. The live
/// path picks one cached orientation and only stamps integer pixels.
fn ring_segment(src: &Img, direction: usize) -> Img {
    let angle = direction as f32 * std::f32::consts::TAU / 72.0;
    let (cos, sin) = (angle.cos(), angle.sin());
    let mut out = Img::new(15, 15);
    for y in 0..out.h {
        for x in 0..out.w {
            let (dx, dy) = ((x-7) as f32, (y-7) as f32);
            let sx = (dx*cos + dy*sin).round() as i32 + src.w/2;
            let sy = (-dx*sin + dy*cos).round() as i32 + src.h/2;
            if let Some(c) = src.get(sx, sy) { out.set(x, y, c); }
        }
    }
    out
}

fn bake(
    art: Art,
    body: Img,
    study: Img,
    lights: Img,
    projectile: [&[u8]; 2],
    tell: [&[u8]; 4],
    impact: [&[u8]; 5],
) -> Kit {
    let tells = tell.map(png);
    let lane = [false, true].map(|filled| {
        if art != Art::Index {
            return Img::new(0, 0);
        }
        let mut tile = tells[3].clone();
        if !filled {
            let unfilled = tells[0].get(23, 5).expect("unfilled lane ink");
            for y in 0..tile.h {
                for x in 0..tile.w {
                    if tile.get(x, y).is_some() {
                        tile.set(x, y, unfilled);
                    }
                }
            }
        }
        tile
    });
    let effects = Effects {
        ring_segments: if reliquary(art) || art == Art::Foreman {
            tells.iter().map(|im| (0..72).map(|direction| ring_segment(im, direction)).collect()).collect()
        } else { Vec::new() },
        ring_glyph: tells.clone().map(|im| resize(&im, 9, 9)),
        projectile: projectile.map(|raw| {
            let p = png(raw);
            let p1 = turn(&p);
            let p2 = turn(&p1);
            let p3 = turn(&p2);
            [p, p1, p2, p3]
        }),
        // A hostile point shot hits a knight centre at HERO_RADIUS + 0.16.
        // Bake the motif to that exact pixel envelope, never a decorative AoE.
        tell: std::array::from_fn(|stage| {
            let authored=tells[stage].clone();
            let r = ((crate::rules::HERO_RADIUS + 0.16) * super::UNIT).ceil() as i32;
            if art == Art::Index || (authored.w == r * 2 + 1 && authored.h == r * 2 + 1) {
                return authored;
            }
            let src = resize(&authored, r * 2 + 1, r * 2 + 1);
            let mut out = Img::new(src.w, src.h);
            let rim = effect_slots(art.id()).point_rim.as_ref().map(|inks|inks[stage])
                .unwrap_or_else(|| super::super::ink::ink(inks(art).1).unwrap());
            for y in -r..=r {
                for x in -r..=r {
                    let d = x * x + y * y;
                    if d <= r * r {
                        if d > (r - 1) * (r - 1) {
                            out.set(x + r, y + r, rim);
                        } else if let Some(c) = src.get(x + r, y + r) {
                            out.set(x + r, y + r, c);
                        }
                    }
                }
            }
            out
        }),
        impact: impact.map(png),
        lane,
    };
    let mut modern = effects.clone();
    if pit_debt(art) {
        // Bake the subcell exposure into the three assigned effect caches.
        // A shared gain preserves pigment ratios and bounds the peak at 242.
        let lift = |im: &mut Img| {
            for y in 0..im.h {
                for x in 0..im.w {
                    if let Some(c) = im.get(x, y) {
                        im.set(x, y, effect_exposure(c));
                    }
                }
            }
        };
        for frame in &mut modern.projectile { for im in frame { lift(im); } }
        for im in &mut modern.tell { lift(im); }
        for im in &mut modern.ring_glyph { lift(im); }
        for stage in &mut modern.ring_segments { for im in stage { lift(im); } }
        for im in &mut modern.impact { lift(im); }
    }
    Kit {
        #[cfg(any(test,feature="testing"))]
        projectile_trials: Default::default(),
        body,
        study,
        lights,
        effects: [effects, modern],
        projectile_v2: effect_slots(art.id()).projectile_v2.as_ref().map(|frames| {
            frames.each_ref().map(|name| {
                let p = png(file(&format!("{}/{name}", art.id())));
                let display = effect_slots(art.id()).display.as_ref().expect("effect display sizes");
                assert_eq!([p.w, p.h], display.v2);
                let native=png(projectile[0]);
                assert_eq!([native.w,native.h], display.v1);
                let p1=turn(&p); let p2=turn(&p1); let p3=turn(&p2);
                [p,p1,p2,p3]
            })
        }),
        figures: Mutex::new(BTreeMap::new()),
        light_figures: Mutex::new(BTreeMap::new()),
        material_floors: Mutex::new(BTreeMap::new()),
    }
}

fn kit(art: Art) -> &'static Kit {
    macro_rules! load {
        ($id:literal) => {{
            static KIT: OnceLock<Kit> = OnceLock::new();
            KIT.get_or_init(|| {
                let slots = effect_slots($id);
                bake(
                    art,
                    png(asset!($id, "v1")),
                    png(include_bytes!(concat!(
                        "../../../../../../cockpit/assets/dungeon/bosses/",
                        $id,
                        "/v2.png"
                    ))),
                    png(asset!($id, "v2-lights")),
                    std::array::from_fn(|i| file(&format!("{}/{}", $id, slots.projectile[i]))),
                    std::array::from_fn(|i| file(&format!("{}/{}", $id, slots.tell[i]))),
                    std::array::from_fn(|i| file(&format!("{}/{}", $id, slots.impact[i]))),
                )
            })
        }};
    }
    macro_rules! asset {
        ($id:literal, $role:literal) => {
            include_bytes!(concat!(
                "../../../../../../cockpit/assets/dungeon/bosses/",
                $id,
                "/",
                $role,
                ".png"
            ))
        };
    }
    match art {
        Art::Cinderjaw => load!("cinderjaw"),
        Art::Index => load!("the-index"),
        Art::Mother => load!("mother-of-spores"),
        Art::Waxen => load!("waxen-warden"),
        Art::Choir => load!("the-bone-choir"),
        Art::Marshal => load!("crypt-tithe-marshal"),
        Art::TaxKeeper => load!("crypt-tax-keeper"),
        Art::RelicLiege => load!("crypt-relic-liege"),
        Art::IronTusk => load!("iron-tusk-chief"),
        Art::RedPick => load!("red-pick-chief"),
        Art::Foreman => load!("the-foreman"),
        Art::Leviathan => load!("late-fee-leviathan"),
        _ => delivered_kit(art),
    }
}

/// Earlier study bodies remain the fallback for their twelve registered IDs.
pub(super) fn legacy(art: Art) -> bool {
    matches!(art, Art::Cinderjaw | Art::Index | Art::Mother | Art::Waxen | Art::Choir | Art::Marshal | Art::TaxKeeper | Art::RelicLiege | Art::IronTusk | Art::RedPick | Art::Foreman | Art::Leviathan)
}

/// Other bosses retain their original definition art until the library gate passes.
/// Each delivered bank is decoded lazily, once, with the same effect baker.
fn delivered_kit(art: Art) -> &'static Kit {
    static KITS: OnceLock<BTreeMap<&'static str, OnceLock<Kit>>> = OnceLock::new();
    KITS.get_or_init(|| Art::ALL.into_iter().map(|a| (a.id(), OnceLock::new())).collect())
        [art.id()].get_or_init(|| {
            let boss = crate::rules::bosses::checked(art.id()).unwrap();
            let body = Img::from_rows(&boss.art.iter().map(String::as_str).collect::<Vec<_>>());
            let slots = effect_slots(art.id());
            bake(art, body.clone(), body, Img::new(0,0),
                std::array::from_fn(|i| file(&format!("{}/{}",art.id(),slots.projectile[i]))),
                std::array::from_fn(|i| file(&format!("{}/{}",art.id(),slots.tell[i]))),
                std::array::from_fn(|i| file(&format!("{}/{}",art.id(),slots.impact[i]))))
        })
}

fn effect_exposure(c: [u8; 3]) -> [u8; 3] {
    let peak = u32::from(*c.iter().max().unwrap()).max(1);
    let gain = 320.min(242 * 256 / peak);
    c.map(|v| (u32::from(v) * gain / 256) as u8)
}

fn effects(art: Art) -> &'static Effects {
    &kit(art).effects[usize::from(crate::settings::get().art_v2)]
}

pub(super) fn mother_body() -> Img {
    kit(Art::Mother).body.clone()
}

pub(super) fn body(art: Art) -> Img {
    kit(art).body.clone()
}

fn cell_emission(c: [u8; 3]) -> [u8; 3] {
    let peak = u32::from(*c.iter().max().unwrap());
    let gain = 242 * 256 / peak.max(1);
    c.map(|v| (u32::from(v) * gain / 256) as u8)
}

/// Match the cached enlarged effect pigment to its native-kit ink. At low
/// coverage the cue owns its half-cell, retaining V1's brightness and hue.
pub(super) fn cell_effect_ink(c: [u8; 3]) -> Option<[u8; 3]> {
    static INKS: OnceLock<BTreeMap<[u8; 3], [u8; 3]>> = OnceLock::new();
    INKS.get_or_init(|| {
        let mut inks = BTreeMap::new();
        for art in [Art::RedPick, Art::Foreman, Art::Leviathan] {
            let cache = kit(art);
            let (native, modern) = (&cache.effects[0], &cache.effects[1]);
            let pairs = native.projectile.iter().flatten().zip(modern.projectile.iter().flatten())
                .chain(native.tell.iter().zip(&modern.tell))
                .chain(native.impact.iter().zip(&modern.impact));
            for (a, b) in pairs {
                for y in 0..a.h {
                    for x in 0..a.w {
                        if let (Some(a), Some(b)) = (a.get(x, y), b.get(x, y)) { inks.insert(b, a); }
                    }
                }
            }
        }
        for ink in [[63,156,140], [43,111,102], [131,186,161], [56,126,118], [38,88,89], [80,163,144]] {
            inks.insert(effect_exposure(ink), ink);
        }
        // Exact prepared emission pigments retain full value at subcell coverage.
        // Material pixels continue through the ordinary coverage average.
        for art in Art::ALL {
            for &c in &effect_slots(art.id()).projectile_signal_inks {
                for ink in [c, toned_ink(c)] { inks.insert(ink, ink); }
            }
        }
        inks
    }).get(&c).copied()
}

/// Only flight emission pigments use the terminal's finer dot grid.
pub(super) fn cell_projectile_signal(c: [u8; 3]) -> bool {
    static SIGNALS: OnceLock<std::collections::BTreeSet<[u8; 3]>> = OnceLock::new();
    SIGNALS.get_or_init(|| {
        Art::ALL.into_iter().flat_map(|a| effect_slots(a.id()).projectile_signal_inks.iter().flat_map(|&c| [c, toned_ink(c)])).collect()
    }).contains(&c)
}

pub(super) fn cell_projectile_core(c: [u8; 3]) -> bool {
    static CORES: OnceLock<std::collections::BTreeSet<[u8; 3]>> = OnceLock::new();
    CORES.get_or_init(|| {
        Art::ALL.into_iter().filter_map(|a| effect_slots(a.id()).projectile_signal_inks.first().copied()).flat_map(|c| [c, toned_ink(c)]).collect()
    }).contains(&c)
}

/// The furnace core owns its half-cell even at low coverage. Match the exact
/// cached emitter pigment, never a luminance threshold that also lifts plates.
pub(super) fn cell_signature_core(c: [u8; 3]) -> bool {
    static CORE: OnceLock<[u8; 3]> = OnceLock::new();
    // These exact local prop pigments retain their colour block at low
    // coverage: the captain's steel head/red haft and the overseer's whips.
    if matches!(c, [138,150,160] | [184,58,46] | [106,68,38]) { return true; }
    c == *CORE.get_or_init(|| {
        let lights = &kit(Art::Cinderjaw).lights;
        let mut core = [0; 3];
        let rank = |c: [u8; 3]| 299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2]);
        for y in 0..lights.h {
            for x in 0..lights.w {
                if let Some(c) = lights.get(x, y) {
                    if rank(c) > rank(core) { core = c; }
                }
            }
        }
        cell_emission(core)
    })
}

/// Retain cached material presence and authored emitter hues after room light.
/// Cell emission is separate so body exposure cannot wash out the core.
pub(super) fn paint_signature_lights(
    cv: &mut super::art_v2::paint::Canvas,
    run: &Run,
    enemy: &Enemy,
    art: Art,
    cell_path: bool,
) {
    // The source body contains its own furnace/gill glints. Old light masks
    // belong to the earlier drawing and must not float over the new pose.
    if library::paint(cv, run, enemy, art, cell_path) { return; }
    if !legacy(art) { return; }
    if enemy.hurt > 0 || enemy.flashing(run.tick) {
        return;
    }
    let Some(legacy) = enemy.boss.and_then(|i| super::stature::boss_figure(run, i)) else {
        return;
    };
    let kit = kit(art);
    // The legacy figures retained their material values in the dark room.
    // Cache a minimum reflected value so the new lit studies keep that body
    // presence. Local room light can still raise a surface above this floor.
    {
        let floor = {
            let mut cache = kit.material_floors.lock().unwrap();
            cache.entry((legacy.w, legacy.h, cell_path)).or_insert_with(|| {
                let mut im = resize(&kit.study, legacy.w * 2, legacy.h * 2);
                let exposure = if cell_path {
                    if art == Art::Cinderjaw { 448 } else if matches!(art, Art::RelicLiege | Art::IronTusk) { 320 } else if pit_debt(art) { 256 } else { 416 }
                } else if matches!(art, Art::Waxen | Art::Choir | Art::Marshal) || reliquary(art) || pit_debt(art) { 256 } else { 232 };
                for y in 0..im.h {
                    for x in 0..im.w {
                        if let Some(c) = im.get(x, y) {
                            let peak = u32::from(*c.iter().max().unwrap());
                            let exposure = if cell_path && art == Art::TaxKeeper && c[2] > c[0] { 256 } else { exposure };
                            let gain = (242 * 256 / peak.max(1)).min(exposure);
                            im.set(x, y, c.map(|v| (u32::from(v) * gain / 256) as u8));
                        }
                    }
                }
                Arc::new(im)
            }).clone()
        };
        let x0 = ((enemy.x * super::UNIT - cv.origin.0) * 2.0).round() as i32 - (legacy.w / 2) * 2;
        let y0 = ((enemy.y * super::UNIT - cv.origin.1) * 2.0).round() as i32 - floor.h + 2;
        for y in 0..floor.h {
            for x in 0..floor.w {
                let (tx, ty) = (x0 + x, y0 + y);
                if tx >= 0 && ty >= 0 && tx < cv.w as i32 && ty < cv.h as i32 {
                    if let Some(c) = floor.get(x, y) {
                        let dest = &mut cv.px[ty as usize * cv.w + tx as usize];
                        for i in 0..3 { dest[i] = dest[i].max(c[i]); }
                    }
                }
            }
        }
    }
    let im = {
        let mut cache = kit.light_figures.lock().unwrap();
        cache
            .entry((legacy.w, legacy.h, cell_path))
            .or_insert_with(|| {
                let mut im = resize(&kit.lights, legacy.w * 2, legacy.h * 2);
                for y in 0..im.h {
                    for x in 0..im.w {
                        if let Some(c) = im.get(x, y) {
                            if cell_path && !pit_debt(art) { im.set(x, y, cell_emission(c)); }
                        }
                    }
                }
                Arc::new(im)
            })
            .clone()
    };
    let x0 = ((enemy.x * super::UNIT - cv.origin.0) * 2.0).round() as i32 - (legacy.w / 2) * 2;
    let y0 = ((enemy.y * super::UNIT - cv.origin.1) * 2.0).round() as i32 - im.h + 2;
    for y in 0..im.h {
        for x in 0..im.w {
            let (tx, ty) = (x0 + x, y0 + y);
            if tx >= 0 && ty >= 0 && tx < cv.w as i32 && ty < cv.h as i32 {
                if let Some(c) = im.get(x, y) {
                    cv.px[ty as usize * cv.w + tx as usize] = c;
                }
            }
        }
    }
}

/// A single baseline with short varied ticks. Tiles are decoded once; advancing
/// fill changes only which of the two cached inks is sampled along the lane.
pub(super) fn index_lane_pixel(
    i: i32,
    side: i32,
    half: i32,
    length: i32,
    stage: usize,
) -> Option<[u8; 3]> {
    let filled = i * 4 < length * (stage as i32 + 1);
    effects(Art::Index).lane[usize::from(filled)].get(i.rem_euclid(24), 5 + side * 5 / half.max(1))
}

pub(super) fn paint_study(
    cv: &mut super::art_v2::paint::Canvas,
    run: &Run,
    enemy: &Enemy,
    art: Art,
    cell_path: bool,
) {
    // Library bodies share V1's material presence; stamp once in the existing
    // post-light material pass, without the earlier redraw's exposure boost.
    if library::registered(art) || !legacy(art) { return; }
    let Some(legacy) = enemy.boss.and_then(|i| super::stature::boss_figure(run, i)) else {
        return;
    };
    let kit = kit(art);
    let frames = {
        let mut cache = kit.figures.lock().unwrap();
        cache
            .entry((legacy.w, legacy.h, cell_path))
            .or_insert_with(|| {
                let mut rest = resize(&kit.study, legacy.w * 2, legacy.h * 2);
                if cell_path {
                    let exposure = if art == Art::Cinderjaw { 448 } else if matches!(art, Art::RelicLiege | Art::IronTusk) { 320 } else if pit_debt(art) { 256 } else { 416 };
                    // Exposure belongs to the cached body, before room light and
                    // subcell coverage. Scenery, the palette and UI stay intact.
                    for y in 0..rest.h {
                        for x in 0..rest.w {
                            if let Some(c) = rest.get(x, y) {
                                // A common multiplier preserves hue ratios;
                                // independent channel clipping washed lights grey.
                                let peak = u32::from(*c.iter().max().unwrap());
                                let exposure = if art == Art::TaxKeeper && c[2] > c[0] { 256 } else { exposure };
                                let gain = (242 * 256 / peak.max(1)).min(exposure);
                                rest.set(x, y, c.map(|v| (u32::from(v) * gain / 256) as u8));
                            }
                        }
                    }
                }
                let mut frames = [
                    rest.clone(),
                    rest.clone(),
                    rest.clone(),
                    super::marks::heat(&rest),
                ];
                for y in 0..rest.h {
                    for x in 0..rest.w {
                        if let Some(c) = rest.get(x, y) {
                            frames[1].set(x, y, super::marks::hit_rgb(c, false));
                            frames[2].set(x, y, super::marks::hit_rgb(c, true));
                        }
                    }
                }
                Arc::new(frames)
            })
            .clone()
    };
    let phase = if enemy.hurt >= 4 {
        2
    } else if enemy.hurt > 0 {
        1
    } else if enemy.flashing(run.tick) {
        3
    } else {
        0
    };
    let im = &frames[phase];
    // Same feet and integer half-width as `stand`, at two samples per pixel.
    let x0 = ((enemy.x * super::UNIT - cv.origin.0) * 2.0).round() as i32 - (legacy.w / 2) * 2;
    let y0 = ((enemy.y * super::UNIT - cv.origin.1) * 2.0).round() as i32 - im.h + 2;
    for y in 0..im.h {
        for x in 0..im.w {
            let (tx, ty) = (x0 + x, y0 + y);
            if tx >= 0 && ty >= 0 && tx < cv.w as i32 && ty < cv.h as i32 {
                if let Some(c) = im.get(x, y) {
                    cv.px[ty as usize * cv.w + tx as usize] = c;
                }
            }
        }
    }
}

pub(super) fn inks(art: Art) -> (char, char, char) {
    match art {
        Art::Cinderjaw => ('R', '@', '6'),
        Art::Index => ('U', '2', 'T'),
        Art::Mother => ('m', 'A', 'Y'),
        Art::Waxen => ('o', 'T', '6'),
        Art::Choir => ('1', '2', '3'),
        Art::Marshal => ('O', '@', '6'),
        Art::TaxKeeper => ('o', '@', '6'),
        Art::RelicLiege => ('O', '2', '3'),
        Art::IronTusk => ('S', 'Q', 'T'),
        Art::RedPick => ('o', 't', 'T'),
        Art::Foreman => ('B', 'o', 'T'),
        Art::Leviathan => ('S', 'U', 'q'),
        _ => ('S', 'U', 'T'),
    }
}

fn hue(art: Art) -> fx::Hue {
    match art {
        Art::Cinderjaw => fx::Hue::Warm,
        Art::Index => fx::Hue::Cold,
        Art::Mother => fx::Hue::Gas,
        Art::Waxen | Art::Marshal | Art::TaxKeeper | Art::IronTusk | Art::RedPick | Art::Foreman => fx::Hue::Warm,
        Art::Choir | Art::RelicLiege | Art::Leviathan => fx::Hue::Cold,
        _ => fx::Hue::Warm,
    }
}

fn centered(cv: &mut Img, im: &Img, x: i32, y: i32) {
    cv.stamp(im, x - im.w / 2, y - im.h / 2);
}

fn direction(vx: f32, vy: f32) -> usize {
    if vx.abs() >= vy.abs() {
        if vx < 0.0 { 2 } else { 0 }
    } else if vy < 0.0 {
        3
    } else {
        1
    }
}

#[cfg(any(test,feature="testing"))]
static EFFECT_STUDY: std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);

#[cfg(feature="testing")]
pub(super) fn effect_review(sample: usize) {
    EFFECT_STUDY.store(sample,std::sync::atomic::Ordering::Relaxed);
}

fn flight(art: Art, modern: bool) -> &'static [[Img;4];2] {
    let bank=kit(art);
    #[cfg(any(test,feature="testing"))]
    {
        let index=EFFECT_STUDY.load(std::sync::atomic::Ordering::Relaxed);
        if index>0 {
            let trials=bank.projectile_trials[usize::from(modern)].get_or_init(|| {
                let Some(display)=&effect_slots(art.id()).display else {return Vec::new()};
                let sizes=if modern {&display.study_v2}else{&display.study_v1};
                let base=if modern {bank.projectile_v2.as_ref().unwrap_or(&bank.effects[0].projectile)}else{&bank.effects[0].projectile};
                sizes.iter().map(|&[w,h]| std::array::from_fn(|i| {
                    let p=resize(&base[i][0],w,h);
                    let p1=turn(&p);let p2=turn(&p1);let p3=turn(&p2);
                    [p,p1,p2,p3]
                })).collect()
            });
            if let Some(frames)=trials.get(index-1) {return frames;}
        }
    }
    let frames: &'static [[Img;4];2] = match &bank.projectile_v2 {
        Some(frames) if modern => frames,
        _ => &bank.effects[usize::from(modern)].projectile,
    };
    toned(art, modern, frames)
}

/// Boss flight frames toned for play (operator, 2026-10-15: "insane, and also way too big"): drawn smaller
/// and dimmer than the delivered banks. Display only: collision, hit radii and timing are untouched.
const FLIGHT_SCALE: f32 = 0.6;
const FLIGHT_DIM: u32 = 205; // of 256

fn toned_ink(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| (u32::from(v) * FLIGHT_DIM / 256) as u8)
}

fn toned(art: Art, modern: bool, frames: &'static [[Img;4];2]) -> &'static [[Img;4];2] {
    static TONED: OnceLock<Mutex<BTreeMap<(String, bool), &'static [[Img;4];2]>>> = OnceLock::new();
    let key = (art.id().to_string(), modern);
    let map = TONED.get_or_init(Default::default);
    if let Some(done) = map.lock().unwrap().get(&key) {
        return done;
    }
    let made: [[Img;4];2] = std::array::from_fn(|a| std::array::from_fn(|d| {
        let src = &frames[a][d];
        let w = ((src.w as f32 * FLIGHT_SCALE).round() as i32).max(3);
        let h = ((src.h as f32 * FLIGHT_SCALE).round() as i32).max(3);
        let mut im = resize(src, w, h);
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = im.get(x, y) {
                    im.set(x, y, toned_ink(c));
                }
            }
        }
        im
    }));
    let made: &'static [[Img;4];2] = Box::leak(Box::new(made));
    map.lock().unwrap().insert(key, made);
    made
}

pub(super) fn native_projectile(art: Art) -> bool {
    effect_slots(art.id()).projectile_v2.is_some()
}

/// Source-density flight frames at two samples per world pixel. All decode and
/// quarter turns belong to the same OnceLock as the ordinary effect bank.
pub(super) fn projectiles_v2(cv: &mut super::art_v2::paint::Canvas, run: &Run, origin: (i32,i32)) {
    for shot in &run.projectiles {
        let Some(art)=Art::of_shot(shot) else {continue};
        if !native_projectile(art) || shot.traits.path.beam.is_some() || shot.traits.path.waiting() {continue;}
        let path=&shot.traits.path;
        if path.bursts() && path.accel<0.0 && path.top==0.0 && shot.vx.hypot(shot.vy)<0.05 {continue;}
        let im=&flight(art,true)[(run.tick/4%2) as usize][direction(shot.vx,shot.vy)];
        let x=(shot.x*super::UNIT*2.0).round() as i32-origin.0*2-im.w/2;
        let y=(shot.y*super::UNIT*2.0).round() as i32-origin.1*2-im.h/2;
        for yy in 0..im.h {for xx in 0..im.w {
            let (tx,ty)=(x+xx,y+yy);
            if tx>=0 && ty>=0 && tx<cv.w as i32 && ty<cv.h as i32 {
                if let Some(c)=im.get(xx,yy) {cv.px[ty as usize*cv.w+tx as usize]=c;}
            }
        }}
    }
}

pub(super) fn projectile(cv: &mut Img, art: Art, x: i32, y: i32, vx: f32, vy: f32, tick: u64) {
    let direction=direction(vx,vy);
    if !native_projectile(art) {fx::glow(cv, x, y, hue(art));}
    centered(
        cv,
        &flight(art,crate::settings::get().art_v2 && !native_projectile(art))[(tick / 4 % 2) as usize][direction],
        x,
        y,
    );
}

pub(super) fn tell(cv: &mut Img, art: Art, x: i32, y: i32, stage: usize) {
    centered(cv, &effects(art).tell[stage], x, y);
}

/// Keep names and health above the ring's uppermost warning glyphs.
pub(super) fn ring_clearance(run: &Run, enemy: &Enemy) -> i32 {
    let art = enemy
        .boss
        .and_then(|i| run.bosses.get(i as usize))
        .and_then(|b| Art::for_id(&b.id));
    if enemy.ring_warning().is_some() && matches!(art, Some(Art::Waxen | Art::Choir | Art::Marshal | Art::TaxKeeper | Art::RelicLiege | Art::IronTusk | Art::Foreman))
    {
        47
    } else {
        0
    }
}

fn ink_line(cv: &mut Img, start: (i32,i32), end: (i32,i32), colour: [u8;3]) {
    let (mut x, mut y) = start;
    let (dx,dy) = ((end.0-x).abs(), -(end.1-y).abs());
    let (sx,sy) = ((end.0-x).signum(), (end.1-y).signum());
    let mut error = dx+dy;
    loop {
        cv.set(x,y,colour);
        if (x,y)==end { break; }
        let twice=2*error;
        if twice>=dy { error+=dy; x+=sx; }
        if twice<=dx { error+=dx; y+=sy; }
    }
}

/// Fill the narrow final band between two real rays without the holes left
/// by nested rounded line endpoints. All raster work is integer arithmetic.
fn ring_band(cv: &mut Img, run: &Run, points: [(i32,i32);4], colour: [u8;3]) {
    let room = run.room();
    let barred = run.barred();
    let x0=points.iter().map(|p| p.0).min().unwrap();
    let x1=points.iter().map(|p| p.0).max().unwrap();
    let y0=points.iter().map(|p| p.1).min().unwrap();
    let y1=points.iter().map(|p| p.1).max().unwrap();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let edges=std::array::from_fn::<_,4,_>(|i| {
                let (a,b)=(points[i],points[(i+1)%4]);
                (b.0-a.0)*(y-a.1)-(b.1-a.1)*(x-a.0)
            });
            if edges.iter().all(|v| *v>=0) || edges.iter().all(|v| *v<=0) {
                // Same blocking tiles as the shot grid, at the renderer's
                // integer tile size. Joins cannot paint across a stone corner
                // merely because their two endpoint rays were unobstructed.
                if x<0 || y<0 || x>=room.cols as i32*super::TILE || y>=room.rows as i32*super::TILE { continue; }
                let blocked = match room.tile(x/super::TILE,y/super::TILE) {
                    crate::rules::Tile::Wall | crate::rules::Tile::Block => true,
                    crate::rules::Tile::Door => barred,
                    _ => false,
                };
                if !blocked { cv.set(x,y,colour); }
            }
        }
    }
}

/// The dressed bosses' existing rage ring: exact outgoing angles and open lane.
/// These are direction glyphs, not a fabricated circular landing area.
pub(super) fn ring_warning(cv: &mut Img, run: &Run, enemy: &Enemy) -> bool {
    let Some(art) = enemy
        .boss
        .and_then(|i| run.bosses.get(i as usize))
        .and_then(|b| Art::for_id(&b.id))
    else {
        return false;
    };
    if art == Art::Foreman { return pit_debt::foreman_warning(cv, run, enemy); }
    if matches!(art, Art::Cinderjaw | Art::Index | Art::Mother | Art::RedPick | Art::Leviathan) {
        return false;
    }
    let Some(t) = enemy.ring_warning() else {
        return false;
    };
    let Some((tx, ty)) = crate::rules::press::warning_target(run, enemy) else {
        return true;
    };
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    let stage = ((t * 4.0) as usize).min(3);
    let (x, y) = at(enemy.x, enemy.y);
    let glyph = &effects(art).ring_glyph[stage];
    let kit = effects(art);
    let dim = if reliquary(art) {
        kit.tell[0].get(7,1).unwrap().map(|v| v/5)
    } else { super::super::ink::ink(inks(art).0).unwrap().map(|v| v/2) };
    let boss = &run.bosses[enemy.boss.unwrap() as usize];
    let preview = crate::rules::desperate::ring_preview(enemy, boss, aim);
    if reliquary(art) && stage == 3 {
        let rays: Vec<_> = preview.iter().filter(|(_,primary)| *primary).map(|(a,_)| *a).collect();
        let step = rays.windows(2).map(|p| p[1]-p[0]).min_by(f32::total_cmp).unwrap_or(0.0);
        for i in 0..rays.len() {
            let (a,b) = (rays[i],rays[(i+1)%rays.len()]);
            // The omitted primary directions are the real safe lane. Never
            // bridge that gap, nor join a segment hidden behind stone.
            if (b-a).rem_euclid(std::f32::consts::TAU) > step*1.01 { continue; }
            if [a,b].iter().any(|angle| crate::rules::patterns::warning_reach(run,(enemy.x,enemy.y),*angle)*super::UNIT < 43.0) { continue; }
            let point = |angle: f32, radius: f32| {
                (x+(angle.cos()*radius).round() as i32,
                 y+(angle.sin()*radius).round() as i32)
            };
            let (core, edge) = match art {
                Art::TaxKeeper => ([160,120,64], [200,160,96]),
                Art::RelicLiege => ([138,116,72], [208,184,120]),
                Art::IronTusk => ([58,74,92], [120,144,168]),
                _ => unreachable!(),
            };
            ring_band(cv,run,[point(a,35.0),point(a,41.0),point(b,41.0),point(b,35.0)],core);
            ring_band(cv,run,[point(a,39.0),point(a,41.0),point(b,41.0),point(b,39.0)],edge);
        }
    }
    for &(a, primary) in &preview {
        let reach = crate::rules::patterns::warning_reach(run, (enemy.x, enemy.y), a) * super::UNIT;
        let radius = 38.0f32.min(reach - 5.0);
        if radius <= 12.0 {
            continue;
        }
        // Only endpoint geometry uses trig. Stamping and the outgoing stroke
        // use integer pixels, with all four glyph drawings decoded once.
        let (ux, uy) = (a.cos(), a.sin());
        let end = (
            x + (ux * (radius - 5.0)).round() as i32,
            y + (uy * (radius - 5.0)).round() as i32,
        );
        let start = (
            x + (ux * (radius - 12.0)).round() as i32,
            y + (uy * (radius - 12.0)).round() as i32,
        );
        if reliquary(art) && !primary {
            // The fan shares the four shape stages: one dot, then three,
            // five and eight pixels along its real outgoing direction.
            let length = [1.0,3.0,5.0,8.0][stage];
            let from = (x+(ux*(radius-4.0-length)).round() as i32,
                y+(uy*(radius-4.0-length)).round() as i32);
            ink_line(cv,from,end,kit.tell[0].get(7,1).unwrap());
        } else {
            ink_line(cv,start,end,dim);
        }
        if primary {
            centered(
                cv,
                if reliquary(art) {
                    let direction = (a.rem_euclid(std::f32::consts::TAU)*72.0/std::f32::consts::TAU).round() as usize % 72;
                    &kit.ring_segments[stage][direction]
                } else { glyph },
                x + (ux * radius).round() as i32,
                y + (uy * radius).round() as i32,
            );
        }
    }
    true
}

pub(super) fn impact(cv: &mut Img, art: Art, x: i32, y: i32, ttl: u8) {
    let age = crate::rules::feel::IMPACT_TICKS.saturating_sub(ttl);
    let phase = usize::from(age) * 5 / usize::from(crate::rules::feel::IMPACT_TICKS);
    centered(cv, &effects(art).impact[phase.min(4)], x, y);
}

/// Destination marks only: held shots and the stopping point of a braking
/// mine. Ordinary travelling shots have no landing area; no source ring
/// pretends they do. Mines warn for their full fuse (51 ticks / 1.7 seconds).
pub(super) fn landings(cv: &mut Img, run: &Run) {
    for shot in &run.projectiles {
        let Some(art) = Art::of_shot(shot) else {
            continue;
        };
        let p = &shot.traits.path;
        if p.waiting() && p.mark {
            if art == Art::Index {
                continue;
            }
            let (x, y) = at(shot.x, shot.y);
            tell(
                cv,
                art,
                x,
                y,
                usize::from(23u8.saturating_sub(p.wait)) * 3 / 23,
            );
        } else if let Some((x, y)) = mine_landing(shot) {
            // A mine stopped by stone never reaches its projected destination.
            let distance = (x - shot.x).hypot(y - shot.y);
            let reach = crate::rules::patterns::warning_reach(
                run,
                (shot.x, shot.y),
                shot.vy.atan2(shot.vx),
            );
            if distance <= reach {
                let (x, y) = at(x, y);
                tell(
                    cv,
                    art,
                    x,
                    y,
                    (usize::from(p.age) * 4 / usize::from(p.fuse)).min(3),
                );
            }
        }
    }
}

fn mine_landing(shot: &crate::rules::Projectile) -> Option<(f32, f32)> {
    let p = &shot.traits.path;
    if !p.bursts()
        || p.accel >= 0.0
        || p.top != 0.0
        || p.fuse == 0
        || p.turn != 0.0
        || p.wave != 0.0
        || p.seek != 0.0
    {
        return None;
    }
    let speed = shot.vx.hypot(shot.vy);
    if speed == 0.0 {
        return Some((shot.x, shot.y));
    }
    // Sum the simulation's discrete braking steps, which steer before moving.
    let brake = -p.accel * (1.0 / crate::rules::HZ as f32);
    let n = (speed / brake)
        .ceil()
        .min(f32::from(p.fuse.saturating_sub(p.age)));
    let travel =
        (n * speed - brake * n * (n + 1.0) * 0.5).max(0.0) * (1.0 / crate::rules::HZ as f32);
    Some((
        shot.x + shot.vx / speed * travel,
        shot.y + shot.vy / speed * travel,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn roster_has_distinct_cached_effects_and_studies() {
        let arts = Art::ALL;
        let hashes: Vec<_> = arts
            .iter()
            .map(|&art| {
                let k = effects(art);
                assert!(std::ptr::eq(k, effects(art)));
                assert!(kit(art).study.w <= 64 && kit(art).study.h <= 64);
                assert_eq!([k.projectile[0][0].w, k.projectile[0][0].h], effect_slots(art.id()).display.as_ref().map(|d|d.v1).unwrap_or(if reliquary(art) || pit_debt(art) { [24, 24] } else { [9, 9] }));
                assert_ne!(
                    k.projectile[0][0].content_hash(),
                    k.projectile[1][0].content_hash()
                );
                assert_eq!(
                    (k.tell[3].w, k.tell[3].h),
                    if art == Art::Index {
                        (24, 11)
                    } else {
                        (11, 11)
                    }
                );
                assert_eq!((k.impact[4].w, k.impact[4].h), (28, 28));
                k.projectile[0][0].content_hash()
            })
            .collect();
        for (i, hash) in hashes.iter().enumerate() {
            assert!(!hashes[..i].contains(hash));
        }
    }

    #[test]
    fn closing_warning_band_clips_each_pixel_against_stone_and_barred_doors() {
        for tile in [crate::rules::Tile::Wall, crate::rules::Tile::Block, crate::rules::Tile::Door] {
            let mut run = super::super::art_v2::testing::boss("crypt-tax-keeper");
            assert!(run.barred());
            run.dungeon.rooms[run.at].set(12,7,tile);
            let mut cv = Img::black(384,224);
            ring_band(&mut cv,&run,[(188,108),(212,108),(212,140),(188,140)],[160,120,60]);
            assert_eq!(cv.get(196,120),Some([0,0,0]));
            assert_eq!(cv.get(188,120),Some([160,120,60]));
        }
    }

    #[test]
    fn appearance_preserves_live_attacks_and_split_children() {
        for art in Art::ALL.into_iter().filter(|a| !matches!(a, Art::Grinwick | Art::BigRipper)) {
            let id = art.id();
            let mut painted = super::super::art_v2::testing::boss(id);
            let mut plain = painted.clone();
            let index = plain.enemies[0].boss.unwrap() as usize;
            plain.bosses[index].id = "unpainted-review".into();
            let mut seen = 0;
            for _ in 0..500 {
                for run in [&mut painted, &mut plain] {
                    for hero in run.players.values_mut() {
                        hero.hp = hero.max_hp;
                    }
                    run.step(&Default::default());
                }
                let mut shots = painted.projectiles.clone();
                for shot in &mut shots {
                    if shot.hostile {
                        assert_eq!(Art::of_shot(shot), Art::for_id(id));
                        seen += 1;
                        shot.look = None;
                    }
                }
                assert_eq!(
                    serde_json::to_value(shots).unwrap(),
                    serde_json::to_value(&plain.projectiles).unwrap(),
                    "{id} trajectories and damage"
                );
                let mut ravages = painted.ravages.clone();
                for ravage in &mut ravages { ravage.boss_art = None; }
                assert_eq!(ravages, plain.ravages, "{id} Ravage physics");
                assert_eq!(
                    serde_json::to_value(&painted.players).unwrap(),
                    serde_json::to_value(&plain.players).unwrap(),
                    "{id} player state"
                );
                assert_eq!(
                    serde_json::to_value(&painted.enemies).unwrap(),
                    serde_json::to_value(&plain.enemies).unwrap(),
                    "{id} monster state"
                );
            }
            assert!(seen > 0, "{id} really fired");
        }
    }

    #[test]
    #[ignore = "a guardian has had no last stand since its fight came in stages (play-test branch of 2026-10-14): its rage is its charge-ups, so no crypt ring is released"]
    fn crypt_ring_stages_follow_live_release_angles_and_party_fans() {
        for id in ["waxen-warden", "the-bone-choir", "crypt-tithe-marshal", "crypt-tax-keeper", "crypt-relic-liege", "iron-tusk-chief"] {
            for fury in 0u8..=3 {
                for prefix in [0u32, 3, 13] {
                    let mut run = super::super::art_v2::testing::boss(id);
                    run.works.clouds.clear();
                    let index = run.enemies[0].boss.unwrap() as usize;
                    run.bosses[index].moves = crate::rules::bosses::Move::Anchor;
                    run.bosses[index].attacks = if prefix == 0 {
                        vec![]
                    } else {
                        vec![crate::rules::bosses::Attack {
                            pattern: crate::rules::bosses::Pattern::Aimed,
                            shots: prefix,
                            arc: 0.0,
                            speed: 6.0,
                            every: 1,
                            damage: 9,
                            bolt: crate::rules::bosses::Bolt::Wisp,
                            from: Default::default(),
                        }]
                    };
                    run.enemies[0].age = 99;
                    run.bosses[index].phases = None;
                    for (seat, hero) in &mut run.players {
                        hero.x = if *seat == 1 { 24.0 } else { 42.0 };
                        hero.y = 24.0;
                    }
                    run.enemies[0].x = 24.0;
                    run.enemies[0].y = 15.0;
                    let mut enemy = serde_json::to_value(&run.enemies[0]).unwrap();
                    enemy["fury"] = serde_json::json!(fury);
                    run.enemies[0] = serde_json::from_value(enemy).unwrap();
                    run.enemies[0].hp = run.enemies[0].max_hp * 3 / 10;
                    run.enemies[0].seen_hp = run.enemies[0].hp;
                    run.enemies[0].wits.foe = 1;
                    let mut hashes = Vec::new();
                    for dread in [1, 10, 19, 28] {
                        run.enemies[0].dread = dread;
                        let mut cv = Img::black(384, 224);
                        assert!(ring_warning(&mut cv, &run, &run.enemies[0]));
                        hashes.push(cv.content_hash());
                    }
                    for (i, hash) in hashes.iter().enumerate() {
                        assert!(
                            !hashes[..i].contains(hash),
                            "{id} fury {fury} stage {i} is visible"
                        );
                    }
                    run.enemies[0].dread = crate::rules::desperate::RING_WARN - 1;
                    let preview = crate::rules::desperate::ring_preview(
                        &run.enemies[0],
                        &run.bosses[index],
                        std::f32::consts::FRAC_PI_2,
                    );
                    assert_eq!(preview.iter().filter(|(_, primary)| *primary).count(), 15);
                    assert_eq!(
                        preview.len(),
                        15 + 2
                            * crate::rules::stakes::FAN_SHOTS.saturating_sub(prefix as usize)
                            * usize::from(fury)
                    );
                    let mut cv = Img::black(384, 224);
                    ring_warning(&mut cv, &run, &run.enemies[0]);
                    if fury == 0 {
                        assert_eq!(cv.get(192, 158), Some([0, 0, 0]), "unfanned safe lane");
                    }
                    let mut released = run.clone();
                    released.step(&Default::default());
                    let ring: Vec<_> = released
                        .projectiles
                        .iter()
                        .filter(|p| p.hostile && p.kind == crate::rules::Shot::Ember && p.damage == 14)
                        .collect();
                    assert!(!ring.is_empty());
                    for shot in ring {
                        let a = shot.vy.atan2(shot.vx);
                        assert!(
                            preview
                                .iter()
                                .any(|(p, _)| (a - p).sin().abs() < 0.0001 && (a - p).cos() > 0.0),
                            "{id} real release angle is previewed"
                        );
                        let reach = crate::rules::patterns::warning_reach(&run, (24.0, 15.0), a)
                            * super::super::UNIT;
                        let radius = 38.0f32.min(reach - 5.0);
                        if radius <= 12.0 {
                            continue;
                        }
                        let x = 192 + (a.cos() * (radius - 7.0)).round() as i32;
                        let y = 120 + (a.sin() * (radius - 7.0)).round() as i32;
                        assert!(
                            (-2..=2).any(|dy| (-2..=2)
                                .any(|dx| cv.get(x + dx, y + dy).is_some_and(|c| c != [0, 0, 0]))),
                            "{id} live ray has its integer warning stroke"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn braking_warning_stays_at_the_simulated_landing() {
        let mut run = super::super::art_v2::testing::boss("cinderjaw");
        run.enemies[0].frozen = 1000;
        run.works.clouds.clear();
        for hero in run.players.values_mut() {
            hero.x = 5.0;
            hero.y = 5.0;
        }
        let mut mine: crate::rules::Projectile = serde_json::from_value(serde_json::json!({
            "x":24.0,"y":10.0,"vx":0.0,"vy":8.0,"hostile":true,
            "kind":"ember","damage":1,"pierce":0,"last_hit":null,"ttl":200
        }))
        .unwrap();
        mine.look = Art::Cinderjaw.look();
        mine.traits.path.accel = -8.0;
        mine.traits.path.fuse = 51;
        mine.traits.path.then = crate::rules::patterns::Then::Burst(6, 4.0);
        let expected = mine_landing(&mine).unwrap();
        run.projectiles = vec![mine];
        for _ in 0..31 {
            run.step(&Default::default());
            let mine = run
                .projectiles
                .iter()
                .find(|p| p.traits.path.bursts())
                .unwrap();
            let predicted = mine_landing(mine).unwrap();
            assert!(
                (expected.0 - predicted.0).abs() < 0.001
                    && (expected.1 - predicted.1).abs() < 0.001
            );
        }
        let mine = run
            .projectiles
            .iter()
            .find(|p| p.traits.path.bursts())
            .unwrap();
        assert!(mine.vx.hypot(mine.vy) < 0.001);
        assert!((expected.0 - mine.x).abs() < 0.001 && (expected.1 - mine.y).abs() < 0.001);
    }

    #[test]
    fn old_impacts_read_and_new_identity_survives_the_wire() {
        use crate::rules::{Shot, feel::Impact};
        let old = serde_json::json!({"x":1.0,"y":2.0,"kind":Shot::Ember,"ttl":6});
        let mut impact: Impact = serde_json::from_value(old).unwrap();
        assert_eq!(impact.boss_art, None);
        impact.boss_art = Some(Art::Cinderjaw);
        let bytes = serde_json::to_vec(&impact).unwrap();
        assert_eq!(impact, serde_json::from_slice(&bytes).unwrap());
    }
}
