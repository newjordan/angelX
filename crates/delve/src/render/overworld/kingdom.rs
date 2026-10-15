//! The kingdom drawn: its landscape with every site's building, the region
//! names lettered on the ground, and the whole realm baked once as the
//! atlas the Delve's kingdom room is cut from. The geography itself is the
//! world's (`crate::world::kingdom`), read here as this module's own.

pub use crate::world::kingdom::*;

use super::{Img, Scene, TILE, View};
use crate::realm::Wish;
use crate::rules::RoomKind;
use std::sync::{Arc, Mutex, OnceLock};

/// Letter each region's name onto `cv` (whose top-left is world pixel
/// `origin`) in a quiet grey with a rule each side, and return the
/// rectangles they take so plaques keep clear of them.
pub fn letter_regions(cv: &mut Img, origin: (i32, i32)) -> Vec<(i32, i32, i32, i32)> {
    let mut taken = Vec::new();
    for (name, (tx, ty)) in regions() {
        let text = name.to_uppercase();
        let w = super::ink::text_width(&text);
        let (x, y) = (tx * TILE + TILE / 2 - origin.0 - w / 2, ty * TILE + 4 - origin.1);
        if x + w + 12 < 0 || y + 9 < 0 || x - 12 > cv.w || y > cv.h {
            continue;
        }
        cv.line(x - 11, y + 3, x - 4, y + 3, 'G');
        cv.line(x + w + 3, y + 3, x + w + 10, y + 3, 'G');
        cv.text(x, y, &text, 'h');
        taken.push((x - 12, y - 2, w + 24, 11));
    }
    taken
}

pub fn landscape(scene: &Scene, view: View) -> Img {
    let geo = geography(&scene.wishes);
    let mut cv = super::render_landscape(scene, view, &geo.cleared_mask, &geo.roads);
    paint_sites(&mut cv, &geo, view);
    cv
}

fn paint_sites(cv: &mut Img, geo: &Geography, view: View) {
    // The forge's yard: an anvil by the Smithy's door, smoke over it.
    if let Some(smithy) = geo.sites.iter().find(|s| s.room == RoomKind::Smithy) {
        let (x, y) = (smithy.stand.0 * TILE - 20 - view.x, smithy.stand.1 * TILE + 2 - view.y);
        cv.stamp(&super::landmarks::anvil(), x, y);
        cv.stamp(&super::kit::smoke(4), x - 2, y - 22);
    }
    for (i, site) in geo.sites.iter().enumerate() {
        if let Some(l) = site.lot {
            if !view.touches(l.tx * TILE, l.ty * TILE, 4 * TILE, 4 * TILE) {
                continue;
            }
            let building = building(site, i);
            cv.stamp(
                &building,
                l.tx * TILE + (4 * TILE - building.w) / 2 - view.x,
                (l.ty + l.th) * TILE - building.h - view.y,
            );
        }
    }
}

/// What stands on countryside site `i`'s lot, sized to fit it.
/// A building never changes once drawn: each site's is drawn once and kept.
fn building(site: &Site, i: usize) -> std::sync::Arc<Img> {
    type Cache = std::sync::Mutex<std::collections::HashMap<(u32, usize), std::sync::Arc<Img>>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let key = (site.room as u32, i);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let im = std::sync::Arc::new(draw_building(site, i));
    if let Ok(mut c) = cache.lock() {
        c.insert(key, std::sync::Arc::clone(&im));
    }
    im
}

fn draw_building(site: &Site, i: usize) -> Img {
    use super::kit::{self, House, Roof, Wall};
    let building = if let Some(landmark) = super::landmarks::landmark(site.room) {
        landmark
    } else {
        match site.room {
            RoomKind::Ranch => super::arena::ranch::landmark(),
            RoomKind::LandOffice => super::arena::holdings::land_office_building(),
            _ => kit::house(&House {
                w: 48,
                h: 40,
                roof_h: 17,
                wall: if i % 2 == 0 { Wall::Stone } else { Wall::Timber },
                roof: if i % 3 == 0 { Roof::Slate } else { Roof::Tile },
                door_glow: false,
                windows: 2,
                lit: false,
                chimney: true,
            }),
        }
    };
    let k = ((building.w + 63) / 64).max((building.h + 47) / 48).max(1);
    if k > 1 {
        let mut small = Img::new(building.w / k, building.h / k);
        for y in 0..small.h {
            for x in 0..small.w {
                if let Some(c) = building.get(x * k, y * k) {
                    small.set(x, y, c);
                }
            }
        }
        small
    } else {
        building
    }
}

/// The world pixel row of the top of what stands on site `i`'s lot, so
/// its name plaque sits just over it; `None` for the realm's own places.
pub fn roof(geo: &Geography, i: usize) -> Option<i32> {
    geo.roofs
        .get_or_init(|| {
            geo.sites
                .iter()
                .enumerate()
                .map(|(i, site)| site.lot.map(|l| (l.ty + l.th) * TILE - building(site, i).h))
                .collect()
        })
        .get(i)
        .copied()
        .flatten()
}

struct Atlas {
    image: Arc<Img>,
    ranks: Arc<Vec<u32>>,
}

fn atlas_data(wishes: &[Wish]) -> Arc<Atlas> {
    type Cache = Mutex<Option<(Vec<Wish>, Arc<Atlas>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some((old, atlas)) = &*cache
        && old == wishes
    {
        return atlas.clone();
    }
    let mut scene = Scene::resting();
    scene.tier = 8;
    scene.ambient = 0.84;
    scene.delve_lit = true;
    scene.knight.x = -1000.0;
    scene.knight.y = -1000.0;
    scene.wishes = Arc::new(wishes.to_vec());
    let img = landscape(&scene, View::realm());
    let ranks = (0..img.h)
        .flat_map(|y| {
            let img = &img;
            (0..img.w).map(move |x| {
                let c = img.get(x, y).unwrap_or(super::ink::BLACK);
                299 * u32::from(c[0])
                    + 587 * u32::from(c[1])
                    + 114 * u32::from(c[2])
                    + if super::ink::is_signal(c) {
                        1_000_000
                    } else {
                        0
                    }
            })
        })
        .collect();
    let atlas = Arc::new(Atlas {
        image: Arc::new(img),
        ranks: Arc::new(ranks),
    });
    *cache = Some((wishes.to_vec(), atlas.clone()));
    atlas
}

pub fn atlas(wishes: &[Wish]) -> Arc<Img> {
    atlas_data(wishes).image.clone()
}

pub fn atlas_ranks(wishes: &[Wish]) -> Arc<Vec<u32>> {
    atlas_data(wishes).ranks.clone()
}
