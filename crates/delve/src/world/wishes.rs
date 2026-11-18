//! Where the party's wishes stand: each standing wish's lot in the wild,
//! found once per set of wishes, and the Delve's own gate lot. The
//! buildings on them are drawn by `render::overworld::wishes`.

use std::sync::{Arc, Mutex, OnceLock};

use super::map::{MAP_H, MAP_W, Place, Realm, TILE};
use crate::ink::Img;
use crate::realm::Wish;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lot {
    pub tx: i32,
    pub ty: i32,
    pub tw: i32,
    pub th: i32,
}

impl Lot {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.tx && y >= self.ty && x < self.tx + self.tw && y < self.ty + self.th
    }

    pub fn overlaps(&self, other: &Lot) -> bool {
        self.tx < other.tx + other.tw
            && other.tx < self.tx + self.tw
            && self.ty < other.ty + other.th
            && other.ty < self.ty + self.th
    }

    fn near(&self, x: i32, y: i32, margin: i32) -> bool {
        x >= self.tx - margin
            && y >= self.ty - margin
            && x < self.tx + self.tw + margin
            && y < self.ty + self.th + margin
    }
}

pub fn place_of(near: &str) -> Place {
    match near.trim().to_ascii_lowercase().as_str() {
        "gatehouse" => Place::Gatehouse,
        "rookery" => Place::Rookery,
        "scriptorium" => Place::Scriptorium,
        "smithy" => Place::Smithy,
        "chapel" => Place::Chapel,
        "round table" => Place::RoundTable,
        "observatory" => Place::Observatory,
        "lists" => Place::Lists,
        "mines" => Place::Mines,
        "dragon keep" => Place::DragonKeep,
        "dark forest" => Place::DarkForest,
        "swamp" => Place::Swamp,
        "fields" => Place::Fields,
        "village" => Place::Village,
        _ => Place::Keep,
    }
}

pub fn plaque_word(name: &str) -> String {
    const SMALL: [&str; 4] = ["the", "of", "a", "an"];
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty() && !SMALL.contains(&w.to_ascii_lowercase().as_str()))
        .map(|w| w.trim_end_matches('\'').to_uppercase())
        .collect();
    let fits = |w: &&String| w.chars().count() <= 7;
    words
        .last()
        .filter(fits)
        .or_else(|| words.iter().find(fits))
        .cloned()
        .unwrap_or_else(|| name.to_uppercase().chars().take(7).collect())
}

pub fn label(wish: &Wish) -> Img {
    let word = plaque_word(&wish.name);
    let mut im = Img::new(crate::ink::text_width(&word) + 1, 8);
    im.text(0, 0, &word, 'u');
    im
}

pub fn art(wish: &Wish) -> Img {
    Img::from_rows(&wish.art.iter().map(String::as_str).collect::<Vec<_>>())
}

pub fn footprint(wish: &Wish) -> (i32, i32) {
    let im = art(wish);
    let plaque = label(wish);
    let tw = (im.w.max(plaque.w) + TILE - 1) / TILE;
    let th = (im.h + TILE - 1) / TILE + 1;
    (tw.max(1), th.max(2))
}

fn wild(tx: i32, ty: i32) -> bool {
    super::plan::tabula()
        || (!super::map::in_set_piece(tx, ty) && super::biome::at(tx, ty) != super::biome::Biome::March)
}

fn buildable(t: u8) -> bool {
    matches!(t, b'.' | b'T' | b'^' | b'd' | b'a')
}

fn structures() -> &'static [(i32, i32, i32, i32)] {
    super::map::structures()
}

pub fn find(tw: i32, th: i32, goal: (i32, i32), taken: &[Lot]) -> Option<Lot> {
    find_in(tw, th, goal, taken, 1, false).or_else(|| find_in(tw, th, goal, taken, 0, false))
}

fn ground(lot: Lot, taken: &[Lot], margin: i32) -> Option<i64> {
    let realm = Realm::get();
    let Lot { tx, ty, tw, th } = lot;
    let mut obstacles = 0;
    for y in ty..ty + th {
        for x in tx..tx + tw {
            let t = realm.at(x, y);
            if (margin > 0 && !wild(x, y)) || !buildable(t) {
                return None;
            }
            obstacles += i64::from(matches!(t, b'T' | b'^' | b'd'));
        }
    }
    if taken.iter().any(|l| {
        (ty - margin..ty + th + margin)
            .any(|y| (tx - margin..tx + tw + margin).any(|x| l.contains(x, y)))
    }) {
        return None;
    }
    if structures().iter().any(|&(sx, sy, sw, sh)| {
        (sy..sy + sh).any(|y| (sx..sx + sw).any(|x| lot.near(x, y, margin)))
    }) {
        return None;
    }
    Some(obstacles)
}

pub fn open_for(wishes: &[Wish], lot: Lot) -> bool {
    let inside = lot.tx >= 1
        && lot.ty >= 1
        && lot.tx + lot.tw < MAP_W - 1
        && lot.ty + lot.th < MAP_H - 1;
    let mut taken = reserved();
    taken.extend(halts());
    taken.extend(lots(wishes).iter().flatten().copied());
    inside && ground(lot, &taken, 1).is_some()
}

struct Sums {
    unfit: Vec<u32>,
    unwild: Vec<u32>,
    cleared: Vec<u32>,
    road: Vec<u32>,
}

const SUM_W: usize = MAP_W as usize + 1;

impl Sums {
    fn get() -> &'static Sums {
        static SUMS: OnceLock<Sums> = OnceLock::new();
        SUMS.get_or_init(|| {
            let realm = Realm::get();
            let mut sums = Sums {
                unfit: vec![0; SUM_W * (MAP_H as usize + 1)],
                unwild: vec![0; SUM_W * (MAP_H as usize + 1)],
                cleared: vec![0; SUM_W * (MAP_H as usize + 1)],
                road: vec![0; SUM_W * (MAP_H as usize + 1)],
            };
            for y in 0..MAP_H {
                for x in 0..MAP_W {
                    let t = realm.at(x, y);
                    let here = (y as usize + 1) * SUM_W + x as usize + 1;
                    let flags = [
                        u32::from(!buildable(t)),
                        u32::from(!wild(x, y)),
                        u32::from(matches!(t, b'T' | b'^' | b'd')),
                        u32::from(matches!(t, b'=' | b'H')),
                    ];
                    for (table, flag) in [&mut sums.unfit, &mut sums.unwild, &mut sums.cleared, &mut sums.road].into_iter().zip(flags) {
                        table[here] = flag + table[here - 1] + table[here - SUM_W] - table[here - SUM_W - 1];
                    }
                }
            }
            sums
        })
    }

    fn over(table: &[u32], x0: i32, y0: i32, x1: i32, y1: i32) -> u32 {
        let at = |x: i32, y: i32| table[y as usize * SUM_W + x as usize];
        at(x1, y1) + at(x0, y0) - at(x1, y0) - at(x0, y1)
    }
}

#[cfg(any(test, feature = "testing"))]
pub static SLOW_SEARCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn find_in(
    tw: i32,
    th: i32,
    (gx, gy): (i32, i32),
    taken: &[Lot],
    margin: i32,
    nearest: bool,
) -> Option<Lot> {
    #[cfg(any(test, feature = "testing"))]
    if SLOW_SEARCH.load(std::sync::atomic::Ordering::Relaxed) {
        return find_in_slow(tw, th, (gx, gy), taken, margin, nearest);
    }
    let sums = Sums::get();
    let (last_x, last_y) = (MAP_W - tw - 2, MAP_H - th - 2);
    let mut blocked = vec![false; (MAP_W * MAP_H) as usize];
    let structures = structures().iter().map(|&(x, y, w, h)| Lot { tx: x, ty: y, tw: w, th: h });
    for rock in taken.iter().copied().chain(structures) {
        if rock.tw <= 0 || rock.th <= 0 {
            continue;
        }
        for y in (rock.ty - th - margin + 1).max(1)..=(rock.ty + rock.th + margin - 1).min(last_y) {
            for x in (rock.tx - tw - margin + 1).max(1)..=(rock.tx + rock.tw + margin - 1).min(last_x) {
                blocked[(y * MAP_W + x) as usize] = true;
            }
        }
    }
    let mut best: Option<(i64, Lot)> = None;
    for ty in 1..=last_y {
        for tx in 1..=last_x {
            if blocked[(ty * MAP_W + tx) as usize] {
                continue;
            }
            let (x1, y1) = (tx + tw, ty + th);
            if Sums::over(&sums.unfit, tx, ty, x1, y1) != 0 || (margin > 0 && Sums::over(&sums.unwild, tx, ty, x1, y1) != 0) {
                continue;
            }
            let obstacles = i64::from(Sums::over(&sums.cleared, tx, ty, x1, y1));
            let road = Sums::over(&sums.road, tx - 1, y1, x1 + 1, y1 + 2) != 0;
            let (cx2, cy2) = (2 * tx + tw, 2 * ty + th);
            let (dx, dy) = (i64::from(cx2 - 2 * gx - 1), i64::from(cy2 - 2 * gy - 1));
            let score = (dx * dx + dy * dy) * if nearest { 1000 } else { 1 }
                + obstacles * 40 + if road { 0 } else { 48 };
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, Lot { tx, ty, tw, th }));
            }
        }
    }
    best.map(|(_, lot)| lot)
}

#[cfg(any(test, feature = "testing"))]
fn find_in_slow(
    tw: i32,
    th: i32,
    (gx, gy): (i32, i32),
    taken: &[Lot],
    margin: i32,
    nearest: bool,
) -> Option<Lot> {
    let realm = Realm::get();
    let mut best: Option<(i64, Lot)> = None;
    for ty in 1..MAP_H - th - 1 {
        for tx in 1..MAP_W - tw - 1 {
            let lot = Lot { tx, ty, tw, th };
            let Some(obstacles) = ground(lot, taken, margin) else {
                continue;
            };
            let road = (tx - 1..tx + tw + 1)
                .flat_map(|x| (ty + th..ty + th + 2).map(move |y| (x, y)))
                .any(|(x, y)| matches!(realm.at(x, y), b'=' | b'H'));
            let (cx2, cy2) = (2 * tx + tw, 2 * ty + th);
            let (dx, dy) = (i64::from(cx2 - 2 * gx - 1), i64::from(cy2 - 2 * gy - 1));
            let score = (dx * dx + dy * dy) * if nearest { 1000 } else { 1 }
                + obstacles * 40 + if road { 0 } else { 48 };
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, lot));
            }
        }
    }
    best.map(|(_, lot)| lot)
}

fn halts() -> Vec<Lot> {
    super::waystations::placed().iter().map(|p| p.rect).collect()
}

fn reserved() -> Vec<Lot> {
    let (tx, ty, tw, th) = crate::rules::squires_town::YARD;
    let mut taken: Vec<Lot> = vec![delve_gate(), super::college::lot()];
    if super::plan::full() {
        taken.push(Lot { tx, ty, tw, th });
    }
    taken.extend(super::castles::footprints().map(|(tx, ty, tw, th)| Lot { tx, ty, tw, th }));
    if super::plan::tabula() {
        taken.extend(super::houses::footprints().map(|(tx, ty, tw, th)| Lot { tx, ty, tw, th: th + 3 }));
    }
    taken
}

pub fn lots(wishes: &[Wish]) -> Arc<Vec<Option<Lot>>> {
    type Cache = Mutex<
        Option<(
            Vec<(String, u32, usize, usize, String)>,
            Arc<Vec<Option<Lot>>>,
        )>,
    >;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key: Vec<_> = wishes
        .iter()
        .map(|w| {
            (
                w.id.clone(),
                w.raised,
                w.art.len(),
                w.art.iter().map(|r| r.chars().count()).max().unwrap_or(0),
                format!("{}|{}|{:?}", w.near, w.name, w.lot),
            )
        })
        .collect();
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(slot) = cache.lock()
        && let Some((k, lots)) = slot.as_ref()
        && *k == key
    {
        return Arc::clone(lots);
    }
    let mut taken = reserved();
    let fixed = taken.clone();
    let tabula = super::plan::tabula();
    let chosen = |wish: &Wish| {
        let (tw, th) = footprint(wish);
        wish.lot
            .map(|(tx, ty)| Lot { tx, ty, tw, th })
            .filter(|l| !tabula || !fixed.iter().any(|f| f.overlaps(l)))
    };
    taken.extend(halts());
    taken.extend(wishes.iter().filter_map(chosen));
    let mut out = Vec::with_capacity(wishes.len());
    for wish in wishes {
        if let Some(lot) = chosen(wish) {
            out.push(Some(lot));
            continue;
        }
        let (tw, th) = footprint(wish);
        let goal = place_of(&wish.near).stand_world();
        let lot = find(tw, th, goal, &taken);
        if let Some(lot) = lot {
            taken.push(lot);
        }
        out.push(lot);
    }
    let lots = Arc::new(out);
    if let Ok(mut slot) = cache.lock() {
        *slot = Some((key, Arc::clone(&lots)));
    }
    lots
}

pub fn unplaced(wishes: &[Wish]) -> Vec<String> {
    let lots = lots(wishes);
    names_without_ground(wishes, &lots)
}

pub fn names_without_ground(wishes: &[Wish], lots: &[Option<Lot>]) -> Vec<String> {
    wishes
        .iter()
        .zip(lots.iter())
        .filter(|(_, lot)| lot.is_none())
        .map(|(w, _)| w.name.clone())
        .collect()
}

pub fn cleared(lots: &[Option<Lot>], tx: i32, ty: i32) -> bool {
    lots.iter().flatten().any(|l| l.contains(tx, ty))
}

pub const DELVE_GATE: Lot = Lot {
    tx: 92,
    ty: 31,
    tw: 3,
    th: 3,
};

pub fn delve_gate() -> Lot {
    if super::plan::tabula() {
        let (tx, ty, tw, th) = super::plan::tabula::DELVE_GATE;
        return Lot { tx, ty, tw, th };
    }
    DELVE_GATE
}

pub fn delve_stand() -> (i32, i32) {
    let gate = delve_gate();
    (gate.tx + gate.tw / 2, gate.ty + gate.th)
}

#[cfg(test)]
#[path = "../../tests/world/wishes__tests.rs"]
mod tests;
