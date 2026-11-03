//! Where the party's wishes stand: each standing wish's lot in the wild,
//! found once per set of wishes, and the Delve's own gate lot. The
//! buildings on them are drawn by `render::overworld::wishes`.

use std::sync::{Arc, Mutex, OnceLock};

use super::map::{MAP_H, MAP_W, Place, Realm, STRUCTURES, TILE, place_tile};
use crate::ink::Img;
use crate::realm::Wish;

/// A wish's lot, in realm tiles: the building on top, its plaque on the
/// bottom row.
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

/// The place a wish names, or the Keep.
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

/// One word for the plaque: the name's last word if it fits, else its first.
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

/// The wish's name carved in the ground beneath it: quiet stone ink on black
/// paper, no board, so the landmark — not its label — carries the screen.
pub fn label(wish: &Wish) -> Img {
    let word = plaque_word(&wish.name);
    let mut im = Img::new(crate::ink::text_width(&word) + 1, 8);
    im.text(0, 0, &word, 'u');
    im
}

pub fn art(wish: &Wish) -> Img {
    Img::from_rows(&wish.art.iter().map(String::as_str).collect::<Vec<_>>())
}

/// Tiles a building of `w`x`h` pixels covers, with a plaque row beneath.
pub fn footprint(wish: &Wish) -> (i32, i32) {
    let im = art(wish);
    let plaque = label(wish);
    let tw = (im.w.max(plaque.w) + TILE - 1) / TILE;
    let th = (im.h + TILE - 1) / TILE + 1;
    (tw.max(1), th.max(2))
}

/// The open country: off every set piece, and out of the March, which
/// belongs to the houses. Wishes never move once raised: lots stay where
/// they stood.
fn wild(tx: i32, ty: i32) -> bool {
    !super::map::in_set_piece(tx, ty) && super::biome::at(tx, ty) != super::biome::Biome::March
}

/// Ground a lot may be cleared on: meadow, ash, and trees or rocks to clear.
fn buildable(t: u8) -> bool {
    matches!(t, b'.' | b'T' | b'^' | b'd' | b'a')
}

fn structures() -> &'static [(i32, i32, i32, i32)] {
    static IN_REALM: OnceLock<Vec<(i32, i32, i32, i32)>> = OnceLock::new();
    IN_REALM.get_or_init(|| {
        STRUCTURES
            .iter()
            .map(|&(ax, ay, w, h)| {
                let (x, y) = place_tile(ax, ay);
                (x, y, w, h)
            })
            .collect()
    })
}

/// The best free lot of `tw`x`th` tiles near `(gx, gy)`, avoiding `taken`:
/// in the wild with a tile of open ground round it, else (a crowded realm)
/// anywhere buildable that touches nothing, so a raised wish always stands.
pub fn find(tw: i32, th: i32, goal: (i32, i32), taken: &[Lot]) -> Option<Lot> {
    find_in(tw, th, goal, taken, 1, false).or_else(|| find_in(tw, th, goal, taken, 0, false))
}

/// Whether a lot may stand on its ground with `margin` tiles round it, and the
/// trees and rocks it would clear if so.
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
    // A tile of open ground around every lot keeps landmarks apart.
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

/// Whether a knight may set a lot here among `wishes`: the wild ground a wish
/// would stand on, with a tile of open ground all round it.
pub fn open_for(wishes: &[Wish], lot: Lot) -> bool {
    let inside = lot.tx >= 1
        && lot.ty >= 1
        && lot.tx + lot.tw < MAP_W - 1
        && lot.ty + lot.th < MAP_H - 1;
    let mut taken = reserved();
    taken.extend(lots(wishes).iter().flatten().copied());
    inside && ground(lot, &taken, 1).is_some()
}

fn find_in(
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
            // Facing a road reads as built, not dropped.
            let road = (tx - 1..tx + tw + 1)
                .flat_map(|x| (ty + th..ty + th + 2).map(move |y| (x, y)))
                .any(|(x, y)| matches!(realm.at(x, y), b'=' | b'H'));
            let (cx2, cy2) = (2 * tx + tw, 2 * ty + th);
            let (dx, dy) = (i64::from(cx2 - 2 * gx - 1), i64::from(cy2 - 2 * gy - 1));
            // Nearness to its place matters most; a road only breaks close ties.
            let score = (dx * dx + dy * dy) * if nearest { 1000 } else { 1 }
                + obstacles * 40 + if road { 0 } else { 48 };
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, lot));
            }
        }
    }
    best.map(|(_, lot)| lot)
}

/// The Delve's gate holds its own ground and the castles theirs; wishes
/// build around them.
fn reserved() -> Vec<Lot> {
    let (tx, ty, tw, th) = crate::rules::squires_town::YARD;
    let mut taken: Vec<Lot> = vec![delve_gate(), super::college::lot(), Lot { tx, ty, tw, th }];
    taken.extend(super::castles::footprints().map(|(tx, ty, tw, th)| Lot { tx, ty, tw, th }));
    taken
}

/// Each standing wish's lot, in raised order. Memoized on the wishes'
/// identity, since the search scans the whole wild.
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
    // The lots knights chose stand first, whatever order they were raised in:
    // the realm finds the others ground around them.
    let chosen = |wish: &Wish| {
        let (tw, th) = footprint(wish);
        wish.lot.map(|(tx, ty)| Lot { tx, ty, tw, th })
    };
    let mut taken = reserved();
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

/// The names of standing wishes the realm has no ground for (each would be
/// left undrawn): what a grant says instead of raising it out of sight.
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

/// Whether a realm tile has been cleared for a wish (no tree or rock drawn).
pub fn cleared(lots: &[Option<Lot>], tx: i32, ty: i32) -> bool {
    lots.iter().flatten().any(|l| l.contains(tx, ty))
}

/// The Delve's gate: a stair into the Highlands' hillside below the Mines,
/// just above the North Road. Fixed for the realm.
pub const DELVE_GATE: Lot = Lot {
    tx: 92,
    ty: 31,
    tw: 3,
    th: 3,
};

pub fn delve_gate() -> Lot {
    DELVE_GATE
}

/// Where the knight stands to enter: just before the gate's mouth.
pub fn delve_stand() -> (i32, i32) {
    let gate = delve_gate();
    (gate.tx + gate.tw / 2, gate.ty + gate.th)
}
