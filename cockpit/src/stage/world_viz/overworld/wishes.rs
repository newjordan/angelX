//! Wishes the party paid for, standing in the wild.
//!
//! The harness's work grows the town; the party's delves win the wild. A
//! granted wish (`together_realm`) stands on a cleared lot in a wilderness
//! screen — never on an authored place, road or water — as near as the wild
//! allows to the place it was wished beside. Lots are given out in the order
//! wishes were raised, so a new wish never moves an old one.
//!
//! Every pixel here is real state: a landmark stands only because spoils
//! from real runs paid for it.

use std::sync::{Arc, Mutex, OnceLock};

use super::ink::Img;
use super::light::Light;
use super::map::{MAP_H, MAP_W, Place, Realm, SCREEN_H, SCREEN_W, STRUCTURES, TILE, place_tile};
use super::scene::Prop;
use crate::drive::together_realm::Wish;

/// Inks that burn.
const FLAME: [char; 3] = ['5', '6', '@'];

/// A wish's lot, in realm tiles: the building on top, its plaque on the
/// bottom row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lot {
    pub(crate) tx: i32,
    pub(crate) ty: i32,
    pub(crate) tw: i32,
    pub(crate) th: i32,
}

impl Lot {
    pub(crate) fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.tx && y >= self.ty && x < self.tx + self.tw && y < self.ty + self.th
    }

    fn near(&self, x: i32, y: i32, margin: i32) -> bool {
        x >= self.tx - margin
            && y >= self.ty - margin
            && x < self.tx + self.tw + margin
            && y < self.ty + self.th + margin
    }
}

/// The place a wish names, or the Keep.
pub(crate) fn place_of(near: &str) -> Place {
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
pub(crate) fn plaque_word(name: &str) -> String {
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
fn label(wish: &Wish) -> Img {
    let word = plaque_word(&wish.name);
    let mut im = Img::new(super::ink::text_width(&word) + 1, 8);
    im.text(0, 0, &word, 'u');
    im
}

fn art(wish: &Wish) -> Img {
    Img::from_rows(&wish.art.iter().map(String::as_str).collect::<Vec<_>>())
}

/// Tiles a building of `w`x`h` pixels covers, with a plaque row beneath.
fn footprint(wish: &Wish) -> (i32, i32) {
    let im = art(wish);
    let plaque = label(wish);
    let tw = (im.w.max(plaque.w) + TILE - 1) / TILE;
    let th = (im.h + TILE - 1) / TILE + 1;
    (tw.max(1), th.max(2))
}

/// A wilderness screen of the heartland. The March to the south belongs to
/// the houses, and wishes never move: lots stay where they always stood.
fn wild(tx: i32, ty: i32) -> bool {
    let (sx, sy) = (tx.div_euclid(SCREEN_W), ty.div_euclid(SCREEN_H));
    sy < super::map::HEARTLAND_Y * 2 - 1 && (sx % 2 == 1 || sy % 2 == 1)
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

/// The best free lot of `tw`x`th` tiles near `(gx, gy)`, avoiding `taken`.
fn find(tw: i32, th: i32, (gx, gy): (i32, i32), taken: &[Lot]) -> Option<Lot> {
    let realm = Realm::get();
    let mut best: Option<(i64, Lot)> = None;
    for ty in 1..MAP_H - th - 1 {
        for tx in 1..MAP_W - tw - 1 {
            let lot = Lot { tx, ty, tw, th };
            let mut obstacles = 0;
            let mut ok = true;
            'tiles: for y in ty..ty + th {
                for x in tx..tx + tw {
                    let t = realm.at(x, y);
                    if !wild(x, y) || !buildable(t) {
                        ok = false;
                        break 'tiles;
                    }
                    obstacles += i64::from(matches!(t, b'T' | b'^' | b'd'));
                }
            }
            if !ok {
                continue;
            }
            // A tile of open ground around every lot keeps landmarks apart.
            if taken.iter().any(|l| {
                (ty - 1..ty + th + 1).any(|y| (tx - 1..tx + tw + 1).any(|x| l.contains(x, y)))
            }) {
                continue;
            }
            if structures().iter().any(|&(sx, sy, sw, sh)| {
                (sy..sy + sh).any(|y| (sx..sx + sw).any(|x| lot.near(x, y, 1)))
            }) {
                continue;
            }
            // Facing a road reads as built, not dropped.
            let road = (tx - 1..tx + tw + 1)
                .flat_map(|x| (ty + th..ty + th + 2).map(move |y| (x, y)))
                .any(|(x, y)| matches!(realm.at(x, y), b'=' | b'H'));
            let (cx2, cy2) = (2 * tx + tw, 2 * ty + th);
            let (dx, dy) = (i64::from(cx2 - 2 * gx - 1), i64::from(cy2 - 2 * gy - 1));
            // Nearness to its place matters most; a road only breaks close ties.
            let score = (dx * dx + dy * dy) + obstacles * 40 + if road { 0 } else { 48 };
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, lot));
            }
        }
    }
    best.map(|(_, lot)| lot)
}

/// Each standing wish's lot, in raised order. Memoized on the wishes'
/// identity, since the search scans the whole wild.
pub(crate) fn lots(wishes: &[Wish]) -> Arc<Vec<Option<Lot>>> {
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
                format!("{}|{}", w.near, w.name),
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
    // The Delve's gate holds its own ground; wishes build around it.
    let mut taken: Vec<Lot> = vec![delve_gate()];
    let mut out = Vec::with_capacity(wishes.len());
    for wish in wishes {
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

/// Whether a realm tile has been cleared for a wish (no tree or rock drawn).
pub(crate) fn cleared(lots: &[Option<Lot>], tx: i32, ty: i32) -> bool {
    lots.iter().flatten().any(|l| l.contains(tx, ty))
}

/// Stage the standing wishes in realm coordinates.
pub(crate) fn stage(wishes: &[Wish], props: &mut Vec<Prop>, lights: &mut Vec<Light>, tick: u32) {
    let lots = lots(wishes);
    for (wish, lot) in wishes.iter().zip(lots.iter()) {
        let Some(lot) = lot else {
            continue;
        };
        let img = art(wish);
        let base = (lot.ty + lot.th - 1) * TILE;
        let x = lot.tx * TILE + (lot.tw * TILE - img.w) / 2;
        // Firelight only where the art itself burns: flame inks, with a
        // hot core. Banners and gems are signal too, but they do not burn.
        let fires: Vec<(i32, i32)> = wish
            .art
            .iter()
            .enumerate()
            .flat_map(|(y, row)| {
                row.chars()
                    .enumerate()
                    .filter(|(_, c)| FLAME.contains(c))
                    .map(move |(x, _)| (x as i32, y as i32))
            })
            .collect();
        let core = wish.art.iter().any(|row| row.contains(['6', '@']));
        if core && !fires.is_empty() {
            let (sx, sy) = fires
                .iter()
                .fold((0, 0), |(ax, ay), &(px, py)| (ax + px, ay + py));
            let n = fires.len() as i32;
            let breath = 0.92 + 0.08 * (tick as f32 * 0.5 + wish.raised as f32 * 2.3).sin();
            lights.push(Light {
                x: (x + sx / n) as f32,
                y: (base - img.h + sy / n) as f32,
                r: 22.0 + (n as f32).sqrt() * 2.0,
                s: 0.45 * breath,
                fire: true,
            });
        }
        props.push(Prop { x, base, img });
        let plaque = label(wish);
        props.push(Prop {
            x: lot.tx * TILE + (lot.tw * TILE - plaque.w) / 2,
            base: base + 3 + plaque.h,
            img: plaque,
        });
    }
}

/// The Delve's gate: a stair into the hill near the Mines, on a lot of its
/// own in the wild. Fixed for the realm, found once.
pub(crate) fn delve_gate() -> Lot {
    static GATE: OnceLock<Lot> = OnceLock::new();
    *GATE.get_or_init(|| {
        let goal = Place::Mines.stand_world();
        find(3, 3, (goal.0 + 3, goal.1 + 4), &[]).unwrap_or(Lot {
            tx: goal.0 + 3,
            ty: goal.1 + 4,
            tw: 3,
            th: 3,
        })
    })
}

/// Where the knight stands to enter: just before the gate's mouth.
pub(crate) fn delve_stand() -> (i32, i32) {
    let gate = delve_gate();
    (gate.tx + gate.tw / 2, gate.ty + gate.th)
}

/// The gate drawn: a slate rise with a carved stone arch, steps going down
/// into the dark, and a torch on each side that burns while the Delve is
/// called or on.
pub(crate) fn delve_sprite(lit: bool, tick: u32) -> Img {
    let mut im = super::kit::blob(
        40,
        30,
        &[
            (11.0, 19.0, 11.0, 11.0),
            (29.0, 19.0, 11.0, 11.0),
            (20.0, 12.0, 15.0, 11.0),
        ],
        &['x', 'S', 'u', 'U', 'v'],
        0.5,
        29,
        2.0,
    );
    im.outline_inside('s');
    // The arch: dressed stone around a dark mouth, steps going down.
    for y in 11..30 {
        for x in 12..28 {
            let (dx, dy) = ((x as f32 - 19.5) / 8.0, (y as f32 - 19.0) / 8.0);
            let inside_arch = y >= 19 || dx * dx + dy * dy <= 1.0;
            let mouth = (x as f32 - 19.5).abs() <= 5.5
                && (y >= 19 || {
                    let (mx, my) = ((x as f32 - 19.5) / 5.5, (y as f32 - 19.0) / 5.5);
                    mx * mx + my * my <= 1.0
                });
            if mouth {
                // Treads lit a little nearer the top, then only dark.
                let tread = y >= 21 && (y - 21) % 3 == 0;
                im.put(x, y, if tread && y < 28 { 'g' } else { 'k' });
            } else if inside_arch {
                let block = (x + y / 3 * 2) % 4 == 0 || y % 3 == 0;
                im.put(x, y, if block { 'J' } else { 'h' });
            }
        }
    }
    let flame = |k: u32| {
        if !lit {
            'a'
        } else if (tick / 4 + k).is_multiple_of(3) {
            '6'
        } else {
            '@'
        }
    };
    for (x, k) in [(9, 0u32), (30, 1)] {
        im.rect(x, 20, 1, 9, 'n');
        im.put(x, 19, 'b');
        im.put(x, 18, flame(k));
        im.put(x, 17, flame(k + 1));
        if lit {
            im.put(x - 1, 18, '@');
            im.put(x + 1, 18, '@');
        }
    }
    im
}

/// A wish's standard: an iron-shod pole with a teal pennant whose rune
/// catches the light now and then.
fn standard(tick: u32, k: u32) -> Img {
    let mut im = Img::new(5, 14);
    for y in 2..14 {
        im.put(0, y, if y == 13 { 'g' } else { 'B' });
    }
    im.put(0, 1, 'J');
    let glint = (tick / 6 + k * 5).is_multiple_of(9);
    for (x, y, ink) in [
        (1, 2, '1'),
        (2, 2, '1'),
        (3, 2, '1'),
        (4, 2, '0'),
        (1, 3, '1'),
        (2, 3, if glint { '3' } else { '2' }),
        (3, 3, '1'),
        (1, 4, '1'),
        (2, 4, '1'),
        (1, 5, '0'),
    ] {
        im.put(x, y, ink);
    }
    im
}

/// Stage the Delve's gate in realm coordinates, with its torchlight when lit
/// and a standard beside it for each wish the party holds (up to six, on
/// the open ground before the stream).
pub(crate) fn stage_gate(
    lit: bool,
    boons: u8,
    props: &mut Vec<Prop>,
    lights: &mut Vec<Light>,
    tick: u32,
) {
    let gate = delve_gate();
    let img = delve_sprite(lit, tick);
    let base = (gate.ty + gate.th - 1) * TILE + TILE - 1;
    let x = gate.tx * TILE + (gate.tw * TILE - img.w) / 2;
    for k in 0..u32::from(boons.min(6)) {
        // A row on the open ground east of the gate, the newest farthest.
        let sx = x + img.w + 3 + k as i32 * 5;
        let flag = standard(tick, k);
        props.push(Prop {
            x: sx,
            base: base - 1,
            img: flag,
        });
    }
    if lit {
        for tx in [9, 30] {
            lights.push(Light {
                x: (x + tx) as f32,
                y: (base - img.h + 18) as f32,
                r: 30.0,
                s: 0.5,
                fire: true,
            });
        }
    }
    props.push(Prop { x, base, img });
}
