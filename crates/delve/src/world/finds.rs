//! Discoverables: thirty finds out in the open country, placed from the
//! run's seed (the same seed, the same finds) on open ground away from
//! roads, doors and fingerposts, most of them in the Wastes and the
//! Highlands: wayside shrines, ruins with a cache, campfires with a
//! wanderer, standing stones, cave mouths, a lost pack, a hermit who trades,
//! a wishing well. They stand in the clearings of the wild camps
//! (`super::dressing`), each at the end of a footpath, so the finds are
//! walked to. A knight walks up and presses E (the kingdom's trinket flow,
//! `rules::trinkets`).

use super::biome::{self, Biome};
use super::kingdom;
use super::map::{MAP_H, MAP_W, in_set_piece};
use crate::rules::Tile;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Shrine,
    Ruin,
    Campfire,
    Stones,
    Cave,
    LostPack,
    Hermit,
    Well,
}

impl Kind {
    pub fn verb(self) -> &'static str {
        match self {
            Kind::Shrine => "PRAY",
            Kind::Ruin => "SEARCH",
            Kind::Campfire => "LISTEN",
            Kind::Stones => "TOUCH",
            Kind::Cave => "LOOK IN",
            Kind::LostPack => "OPEN",
            Kind::Hermit => "TALK",
            Kind::Well => "WISH",
        }
    }

    pub fn told(self) -> &'static str {
        match self {
            Kind::Shrine => "A SHRINE STANDS IN",
            Kind::Ruin => "THERE ARE RUINS IN",
            Kind::Campfire => "A FIRE BURNS IN",
            Kind::Stones => "OLD STONES STAND IN",
            Kind::Cave => "A CAVE OPENS IN",
            Kind::LostPack => "A LOST PACK LIES IN",
            Kind::Hermit => "A HERMIT KEEPS TO",
            Kind::Well => "A WELL STANDS IN",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Find {
    pub kind: Kind,
    pub at: (i32, i32),
    pub biome: Biome,
}

pub const COUNT: usize = 30;
pub const REACH: f32 = 1.6;
pub const CACHE: u32 = 15;
pub const COIN: u32 = 10;
const APART: i32 = 14;

const SHARE: [(Biome, usize, &[Kind]); 18] = [
    (Biome::Wastes, 3, &[Kind::Ruin, Kind::Cave, Kind::Stones]),
    (Biome::Highlands, 2, &[Kind::Cave, Kind::Stones]),
    (Biome::Greenwood, 2, &[Kind::Shrine, Kind::Campfire]),
    (Biome::Fen, 1, &[Kind::Shrine]),
    (Biome::Coast, 1, &[Kind::Ruin]),
    (Biome::WindCountry, 1, &[Kind::Stones]),
    (Biome::Longbow, 1, &[Kind::Campfire]),
    (Biome::March, 1, &[Kind::Ruin]),
    (Biome::Wastes, 3, &[Kind::LostPack, Kind::Hermit, Kind::Shrine]),
    (Biome::Highlands, 2, &[Kind::Hermit, Kind::LostPack]),
    (Biome::Greenwood, 2, &[Kind::Well, Kind::LostPack]),
    (Biome::Fen, 2, &[Kind::Hermit, Kind::Well]),
    (Biome::Coast, 2, &[Kind::Well, Kind::LostPack]),
    (Biome::WindCountry, 2, &[Kind::Well, Kind::Campfire]),
    (Biome::Longbow, 1, &[Kind::Well]),
    (Biome::March, 2, &[Kind::LostPack, Kind::Hermit]),
    (Biome::TiltYards, 1, &[Kind::Campfire]),
    (Biome::Fairground, 1, &[Kind::Well]),
];

fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

pub(super) fn spots() -> &'static [(i32, i32)] {
    static SPOTS: OnceLock<Vec<(i32, i32)>> = OnceLock::new();
    SPOTS.get_or_init(|| {
        if super::plan::tabula() {
            return Vec::new();
        }
        let geo = kingdom::geography(&[]);
        let idx = |(x, y): (i32, i32)| (y * MAP_W + x) as usize;
        let mut near = vec![6u8; (MAP_W * MAP_H) as usize];
        let mut frontier: Vec<(i32, i32)> = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .filter(|&t| kingdom::is_road(&geo, t))
            .collect();
        for &t in &frontier {
            near[idx(t)] = 0;
        }
        for step in 1..6u8 {
            let mut next = Vec::new();
            for (x, y) in frontier {
                for n in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                    if n.0 >= 0 && n.1 >= 0 && n.0 < MAP_W && n.1 < MAP_H && near[idx(n)] > step {
                        near[idx(n)] = step;
                        next.push(n);
                    }
                }
            }
            frontier = next;
        }
        let castles: Vec<(i32, i32, i32, i32)> = super::castles::footprints().collect();
        let posts = super::signs::posts();
        let halts = super::waystations::placed();
        (3..MAP_H - 3)
            .flat_map(|y| (3..MAP_W - 3).map(move |x| (x, y)))
            .filter(|&t| near[idx(t)] >= 5)
            .filter(|&(x, y)| (y - 1..=y + 1).all(|y| (x - 1..=x + 1).all(|x| geo.room.tile(x, y) == Tile::Floor)))
            .filter(|&(x, y)| (y - 2..=y + 2).all(|y| (x - 2..=x + 2).all(|x| !in_set_piece(x, y))))
            .filter(|&t| {
                geo.sites.iter().all(|s| {
                    chebyshev(s.stand, t) > 6
                        && s.lot.is_none_or(|l| chebyshev((l.tx + l.tw / 2, l.ty + l.th / 2), t) > 6)
                })
            })
            .filter(|&(x, y)| {
                castles.iter().all(|&(cx, cy, cw, ch)| x < cx - 6 || y < cy - 6 || x >= cx + cw + 6 || y >= cy + ch + 6)
            })
            .filter(|&t| posts.iter().all(|p| chebyshev(p.at, t) > 4))
            .filter(|&t| halts.iter().all(|h| chebyshev((h.rect.tx + 3, h.rect.ty + 2), t) > 7))
            .collect()
    })
}

pub fn finds(seed: u64) -> Arc<Vec<Find>> {
    type Cache = Mutex<Option<(u64, Arc<Vec<Find>>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((s, found)) = &*cache
        && *s == seed
    {
        return found.clone();
    }
    let out = Arc::new(if super::plan::tabula() { Vec::new() } else { place(seed) });
    *cache = Some((seed, out.clone()));
    out
}

fn ranked(home: Biome, salt: u32, tiles: impl Iterator<Item = (i32, i32)>) -> Vec<(i32, i32)> {
    let mut here: Vec<(u32, (i32, i32))> = tiles
        .filter(|&(x, y)| biome::at(x, y) == home)
        .map(|(x, y)| (crate::ink::hash(x, y, salt), (x, y)))
        .collect();
    here.sort();
    here.into_iter().map(|(_, at)| at).collect()
}

fn place(seed: u64) -> Vec<Find> {
    let salt = (seed ^ (seed >> 32)) as u32;
    let wild = || {
        super::dressing::get()
            .camps
            .iter()
            .filter(|c| c.class == super::dressing::Class::Wild)
            .map(|c| c.at)
    };
    let mut out: Vec<Find> = Vec::new();
    for (home, share, kinds) in SHARE {
        let candidates = ranked(home, salt, wild()).into_iter().chain(ranked(home, salt, spots().iter().copied()));
        let mut taken = 0;
        for at in candidates {
            if taken == share {
                break;
            }
            if out.iter().any(|f| chebyshev(f.at, at) < APART) {
                continue;
            }
            out.push(Find { kind: kinds[taken % kinds.len()], at, biome: home });
            taken += 1;
        }
    }
    out.truncate(COUNT);
    out
}

#[cfg(test)]
#[path = "../../tests/world/finds__tests.rs"]
mod tests;
