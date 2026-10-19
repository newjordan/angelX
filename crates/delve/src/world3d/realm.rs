//! The realm, lit (docs/REALM-MONOLITH.md): the kingdom's chunks in the
//! monolith language, each chunk's set made from the map, the same chunks,
//! keys and streaming as before (`world3d::kingdom`), drawn through a look
//! that lights them (`realm_look`) instead of with painted plates.
//!
//! A chunk's set is:
//!
//! - **its cut of the one ground** (`realm_ground`): a height field, the
//!   walked ground the floor, the rest rising by its biome, water and lava
//!   lowered into it with sloping banks, the road a bright ribbon;
//! - **water and lava surfaces** at their levels over their tiles;
//! - **bridges** as stone decks a hair over the floor, parapets on the
//!   water's sides;
//! - **its castles** (`realm_castle`) and **each place's buildings**
//!   (`realm_place`), big stepped, slotted, finned and corniced masses on
//!   their footprints;
//! - **woods** as dark spires packed and overlapping into one ragged mass,
//!   taller toward their middles and fronts, never tall enough at their
//!   backs to bury a road behind them; **outcrops** as great slabs and
//!   stepped shelves; **dead trees** as bare spikes on the ash; **the
//!   map's rim** as woods, rock or nothing by its biome;
//! - **the fingerposts and the realm's set dressing**, as the chunks have
//!   them (`chunks`);
//! - **its lamps** where something happens: a castle's gate, a forge, the
//!   Delve's gate, a lighthouse; the few that matter most.
//!
//! Nothing stands over ground a knight walks, but a slender post.

use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

use angel_stage::set::{Kind, SetLamp};

use super::blockout::{self, TILE, merge, num};
use super::chunks::{self, CHUNK, Cover, Land, STAGE, centre, chunk_id};
use super::realm_castle;
use super::realm_ground::{self, LAVA_Y, Terrain, WATER_Y, unit};
use super::realm_mono::{Piece, slab, spire};
use super::realm_place;
use crate::world::biome::{self, Biome};

/// The kind lava's surface is written as: mud made to glow on the warm
/// ramp by the set's own `role` and `tone` (no lamp, no shadow), the chunk's
/// fires staying accents.
const LAVA: Kind = Kind::Mud;

/// The most lamps a chunk stands.
pub const LAMPS_A_CHUNK: usize = 3;

/// One named group of a chunk's pieces, as written.
struct Group {
    name: String,
    lines: Vec<String>,
}

impl Group {
    fn of(name: impl Into<String>, pieces: &[Piece]) -> Self {
        Self { name: name.into(), lines: pieces.iter().map(Piece::line).collect() }
    }
}

/// The chunk `(cx, cz)` of `land` as a set file in the monolith language.
pub fn chunk_set(land: &Land, terrain: &Terrain, cx: i32, cz: i32) -> String {
    let rect = land.chunk_rect(cx, cz);
    let mut groups = Vec::new();
    let mut lamps: Vec<(f32, SetLamp)> = Vec::new();
    groups.push(Group::of("water", &water(land, rect)));
    groups.push(Group::of("bridges", &bridges(land, rect)));
    for site in castles().iter().filter(|s| owner(s.foot) == (cx, cz)) {
        let stood = realm_castle::castle(site, terrain);
        lamps.extend(stood.lamps);
        groups.push(Group::of(format!("{}: a castle at {:?}", site.name, site.foot), &stood.pieces));
    }
    for b in land.builds.iter().filter(|b| land.owner(b) == (cx, cz) && !in_castle(b)) {
        let stood = realm_place::place(b, terrain);
        lamps.extend(stood.lamps);
        let (c, r, w, h) = b.foot;
        let name = match &b.place {
            Some(place) => format!("{} of {}: {} at {c},{r} {w}x{h}", b.name, place, b.kit.what()),
            None => format!("{}: {} at {c},{r} {w}x{h}", b.name, b.kit.what()),
        };
        groups.push(Group::of(name, &stood.pieces));
    }
    groups.push(Group::of("woods", &woods(land, terrain, rect)));
    groups.push(Group::of("outcrops", &outcrops(land, terrain, rect)));
    groups.push(Group::of("dead trees", &dead_trees(land, terrain, rect)));
    groups.push(Group::of("the rim", &rim(land, terrain, rect)));
    for g in [chunks::fingerposts(land, rect), chunks::dressed(land, rect)] {
        groups.push(Group { name: g.name.clone(), lines: g.solids.iter().map(blockout::line).collect() });
    }
    lamps.sort_by(|a, b| b.0.total_cmp(&a.0));
    lamps.truncate(LAMPS_A_CHUNK);
    write(land, terrain, cx, cz, &groups, &lamps)
}

/// Whether build `b` is a castle or stands inside one's wall (the Keep's old
/// keep, its turrets, the Rookery and the Chapel are in its face): the
/// castle stands for it (`realm_castle`).
pub fn in_castle(b: &chunks::Build) -> bool {
    let (c, r, w, h) = b.foot;
    realm_castle::is_castle(b.kit) || crate::world::castles::at(c + w / 2, r + h / 2).is_some()
}

/// The chunk a footprint belongs to: the one under its middle.
fn owner((c, r, w, h): blockout::Rect) -> (i32, i32) {
    ((c + w / 2).div_euclid(CHUNK.0), (r + h / 2).div_euclid(CHUNK.1))
}

/// The kingdom's castles (the map with no wishes), made once.
pub fn castles() -> &'static [realm_castle::Site] {
    static SITES: OnceLock<Vec<realm_castle::Site>> = OnceLock::new();
    SITES.get_or_init(|| realm_castle::sites(super::kingdom::land()))
}

/// The set the realm makes of chunk `index` (the map with no wishes),
/// made once.
pub fn set_text(index: u32) -> Arc<str> {
    static TEXTS: OnceLock<Vec<OnceLock<Arc<str>>>> = OnceLock::new();
    let count = super::kingdom::COUNT;
    let texts = TEXTS.get_or_init(|| (0..count).map(|_| OnceLock::new()).collect());
    let (cx, cz) = super::kingdom::cell(index);
    Arc::clone(texts[index as usize % count].get_or_init(|| chunk_set(super::kingdom::land(), realm_ground::terrain(), cx, cz).into()))
}

// ---------------------------------------------------------------- water ----

/// Water and lava's surfaces over their tiles, merged into runs.
fn water(land: &Land, (c, r, w, h): blockout::Rect) -> Vec<Piece> {
    let cells: Vec<Option<bool>> = (r..r + h)
        .flat_map(|y| (c..c + w).map(move |x| (x, y)))
        .map(|(x, y)| match land.cover(x, y) {
            Cover::Water | Cover::Bridge => Some(false),
            Cover::Lava => Some(true),
            _ => None,
        })
        .collect();
    merge(w as usize, h as usize, &cells)
        .into_iter()
        .map(|((mc, mr, mw, mh), lava)| {
            let [x0, z0, x1, z1] = blockout::px((c + mc, r + mr, mw, mh));
            let (kind, y) = if lava { (LAVA, LAVA_Y) } else { (Kind::Water, WATER_Y) };
            Piece::new("plane", kind, [(x0 + x1) / 2.0, y, (z0 + z1) / 2.0], [x1 - x0, 0.0, z1 - z0])
        })
        .collect()
}

/// Roads over water: a stone deck a hair over the floor, its sides down
/// into the water, parapets along the water's sides.
fn bridges(land: &Land, (c, r, w, h): blockout::Rect) -> Vec<Piece> {
    let bridge = |x: i32, y: i32| land.cover(x, y) == Cover::Bridge;
    let cells: Vec<Option<bool>> = (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).map(|(x, y)| bridge(x, y).then_some(true)).collect();
    let mut out = Vec::new();
    for ((mc, mr, mw, mh), _) in merge(w as usize, h as usize, &cells) {
        let [x0, z0, x1, z1] = blockout::px((c + mc, r + mr, mw, mh));
        out.push(Piece::new("box", Kind::Stone, [(x0 + x1) / 2.0, -9.0, (z0 + z1) / 2.0], [x1 - x0, 9.2, z1 - z0]));
    }
    for y in r..r + h {
        for x in c..c + w {
            if !bridge(x, y) {
                continue;
            }
            let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
            let wet = |dx: i32, dy: i32| matches!(land.cover(x + dx, y + dy), Cover::Water);
            let rail = |a: [f32; 4]| Piece::new("box", Kind::Stone, [(a[0] + a[2]) / 2.0, 0.0, (a[1] + a[3]) / 2.0], [a[2] - a[0], 6.0, a[3] - a[1]]);
            if wet(0, -1) {
                out.push(rail([x0, z0, x1, z0 + 2.5]));
            }
            if wet(0, 1) {
                out.push(rail([x0, z1 - 2.5, x1, z1]));
            }
            if wet(-1, 0) {
                out.push(rail([x0, z0, x0 + 2.5, z1]));
            }
            if wet(1, 0) {
                out.push(rail([x1 - 2.5, z0, x1, z1]));
            }
        }
    }
    out
}

// ---------------------------------------------------------------- woods ----

/// How tall what stands on tile `(x, y)` may be before it buries the ground
/// a knight walks behind it: a little over the tiles no one walks north of it.
fn cap(terrain: &Terrain, x: i32, y: i32) -> f32 {
    34.0 + 16.0 * f32::from(terrain.north_clear(x, y))
}

/// Woods: dark spires packed and overlapping, a mass with a ragged crown,
/// taller toward its middle; one or two a tile, each kept inside what its
/// tile may reach over.
fn woods(land: &Land, terrain: &Terrain, rect: blockout::Rect) -> Vec<Piece> {
    let mut out = Vec::new();
    for (x, y) in tiles(rect) {
        if land.cover(x, y) != Cover::Woods || land.walkable(x, y) {
            continue;
        }
        spires_on(land, terrain, x, y, &mut out);
    }
    out
}

fn spires_on(land: &Land, terrain: &Terrain, x: i32, y: i32, out: &mut Vec<Piece>) {
    let [rw, rn, re, rs] = chunks::reach(land, x, y);
    let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
    let (lo_x, hi_x, lo_z, hi_z) = (x0 - rw, x1 + re, z0 - rn, z1 + rs);
    let count = if unit(x, y, 401) < 0.45 { 2 } else { 1 };
    for k in 0..count {
        let salt = 410 + k * 10;
        let (jx, jz) = (unit(x, y, salt) * 2.0 - 1.0, unit(x, y, salt + 1) * 2.0 - 1.0);
        let want = if k == 0 { 7.5 + 2.5 * unit(x, y, salt + 2) } else { 5.5 + 2.0 * unit(x, y, salt + 2) };
        let (mx, mz) = ((x0 + x1) / 2.0 + jx * 4.5, (z0 + z1) / 2.0 + jz * 4.5);
        let r = want.min(mx - lo_x).min(hi_x - mx).min(mz - lo_z).min(hi_z - mz).max(3.0);
        let inland = terrain.inland(mx, mz);
        let want_h = 60.0 + 72.0 * angel_stage::look::smoothstep(0.0, 80.0, inland) + 28.0 * unit(x, y, salt + 3);
        let h = want_h.min(cap(terrain, x, y)).max(26.0) * if k == 0 { 1.0 } else { 0.8 };
        let ground = terrain.height_at(mx, mz);
        out.push(spire(mx, ground - 3.0, mz, r, h + 3.0, unit(x, y, salt + 4) * 60.0));
    }
}

/// Rock: great slabs on each two by two of it, stepped into shelves toward
/// its middle; a lesser slab on a tile of rock alone.
fn outcrops(land: &Land, terrain: &Terrain, (c, r, w, h): blockout::Rect) -> Vec<Piece> {
    let mut out = Vec::new();
    let rock = |x: i32, y: i32| land.cover(x, y) == Cover::Rock && !land.walkable(x, y);
    let mut done = std::collections::HashSet::new();
    // The two by twos, aligned to even tiles (a chunk's sides are even, so
    // none straddles two chunks).
    for y in (r..r + h).step_by(2) {
        for x in (c..c + w).step_by(2) {
            let four = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
            if !four.iter().all(|&(a, b)| rock(a, b)) {
                continue;
            }
            done.extend(four);
            let [x0, z0, x1, z1] = blockout::px((x, y, 2, 2));
            let (mx, mz) = ((x0 + x1) / 2.0 + (unit(x, y, 501) - 0.5) * 3.0, (z0 + z1) / 2.0 + (unit(x, y, 502) - 0.5) * 3.0);
            let (sw, sd) = (25.0 + 4.0 * unit(x, y, 503), 21.0 + 6.0 * unit(x, y, 504));
            let turn = (unit(x, y, 505) - 0.5) * 12.0;
            let base = terrain.lowest([mx - sw / 2.0, mz - sd / 2.0, mx + sw / 2.0, mz + sd / 2.0]) - 2.0;
            let inland = terrain.inland(mx, mz);
            let want = 14.0 + 34.0 * angel_stage::look::smoothstep(0.0, 64.0, inland) + 10.0 * unit(x, y, 506);
            let tall = want.min(cap(terrain, x, y)).max(10.0);
            let top_ground = terrain.height_at(mx, mz);
            let first = (top_ground - base) + tall;
            out.push(slab(Kind::Rock, mx, base, mz, [sw, first, sd], turn));
            if tall > 20.0 {
                // A shelf on it, set back.
                let more = 8.0 + 10.0 * unit(x, y, 507);
                out.push(slab(Kind::Rock, mx + (unit(x, y, 508) - 0.5) * 4.0, base + first, mz - sd * 0.16, [sw * 0.66, more, sd * 0.6], turn + (unit(x, y, 509) - 0.5) * 10.0));
            }
        }
    }
    for (x, y) in tiles((c, r, w, h)) {
        if !rock(x, y) || done.contains(&(x, y)) {
            continue;
        }
        let [rw, rn, re, rs] = chunks::reach(land, x, y);
        let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
        let (mx, mz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
        let sw = (11.0 + 3.0 * unit(x, y, 511)).min(TILE + 2.0 * rw.min(re));
        let sd = (9.0 + 3.0 * unit(x, y, 512)).min(TILE + 2.0 * rn.min(rs));
        let base = terrain.lowest([mx - sw / 2.0, mz - sd / 2.0, mx + sw / 2.0, mz + sd / 2.0]) - 2.0;
        let tall = (9.0 + 12.0 * unit(x, y, 513)).min(cap(terrain, x, y));
        out.push(slab(Kind::Rock, mx, base, mz, [sw, terrain.height_at(mx, mz) - base + tall, sd], (unit(x, y, 514) - 0.5) * 8.0));
    }
    out
}

/// Dead trees on the ash: bare dark spikes, a crooked limb on each.
fn dead_trees(land: &Land, terrain: &Terrain, rect: blockout::Rect) -> Vec<Piece> {
    let mut out = Vec::new();
    for (x, y) in tiles(rect) {
        if land.cover(x, y) != Cover::DeadTree || land.walkable(x, y) || unit(x, y, 601) > 0.6 {
            continue;
        }
        let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
        let (mx, mz) = ((x0 + x1) / 2.0 + (unit(x, y, 602) - 0.5) * 6.0, (z0 + z1) / 2.0 + (unit(x, y, 603) - 0.5) * 6.0);
        let h = (28.0 + 26.0 * unit(x, y, 604)).min(cap(terrain, x, y));
        let g = terrain.height_at(mx, mz) - 2.0;
        out.push(Piece { turn: unit(x, y, 605) * 90.0, ..Piece::new("cone-4", Kind::Trunk, [mx, g, mz], [4.5, h + 2.0, 4.5]) });
        let a = unit(x, y, 606) * 360.0;
        let (s, co) = a.to_radians().sin_cos();
        out.push(Piece { turn: a, ..Piece::new("box", Kind::Trunk, [mx + co * 4.0, g + h * 0.55, mz - s * 4.0], [9.0, 1.4, 1.4]) });
    }
    out
}

/// The map's rim: woods in the green country, slabs in the hills and the
/// Wastes, nothing on the Coast (its sea).
fn rim(land: &Land, terrain: &Terrain, rect: blockout::Rect) -> Vec<Piece> {
    let mut out = Vec::new();
    for (x, y) in tiles(rect) {
        if land.cover(x, y) != Cover::Hedge || land.walkable(x, y) {
            continue;
        }
        match biome::at(x, y) {
            Biome::Coast => {}
            Biome::Highlands | Biome::Wastes => {
                let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
                let (mx, mz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
                let base = terrain.lowest([x0, z0, x1, z1]) - 2.0;
                let tall = (24.0 + 30.0 * unit(x, y, 701)).min(cap(terrain, x, y).max(20.0));
                out.push(slab(Kind::Rock, mx, base, mz, [15.0, terrain.height_at(mx, mz) - base + tall, 14.0], (unit(x, y, 702) - 0.5) * 10.0));
            }
            _ => spires_on(land, terrain, x, y, &mut out),
        }
    }
    out
}

fn tiles((c, r, w, h): blockout::Rect) -> impl Iterator<Item = (i32, i32)> {
    (r..r + h).flat_map(move |y| (c..c + w).map(move |x| (x, y)))
}

// ------------------------------------------------------------- the text ----

fn write(land: &Land, terrain: &Terrain, cx: i32, cz: i32, groups: &[Group], lamps: &[(f32, SetLamp)]) -> String {
    let (c, r, w, h) = land.chunk_rect(cx, cz);
    let [x, z] = centre(cx, cz);
    let mut out = String::new();
    let _ = writeln!(out, "set {}", chunk_id(cx, cz));
    let _ = writeln!(
        out,
        "# The realm, lit: chunk {cx},{cz} in {}, tiles {c}..{} x {r}..{} ({w} x {h} of {} kingdom pixels).",
        biome::chunk(cx, cz).name(),
        c + w,
        r + h,
        TILE
    );
    let _ = writeln!(out, "# Made from the game's own map by angel-delve world3d::realm (docs/REALM-MONOLITH.md).");
    let _ = writeln!(out, "camera chunk {} {} centre {} {}", STAGE.0, STAGE.1, num(x), num(z));
    let _ = writeln!(out, "{}", terrain.statement(cx, cz));
    if groups.iter().any(|g| g.name == "water" && g.lines.iter().any(|l| l.starts_with("plane mud"))) {
        let _ = writeln!(out, "# lava glows on the warm ramp\nrole mud ember\ntone mud 30");
    }
    for g in groups {
        if g.lines.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n# {}", g.name);
        for l in &g.lines {
            let _ = writeln!(out, "{l}");
        }
    }
    if !lamps.is_empty() {
        let _ = writeln!(out, "\n# lamps: where something happens");
        for (_, l) in lamps {
            let _ = writeln!(out, "lamp at {} {} {} reach {} strength {}", num(l.at[0]), num(l.at[1]), num(l.at[2]), num(l.reach), num(l.strength));
        }
    }
    out
}

#[cfg(test)]
#[path = "../../tests/world3d/realm__tests.rs"]
mod tests;
