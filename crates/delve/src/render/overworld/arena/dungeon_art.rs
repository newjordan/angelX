//! Delivered dungeon scenery, shared by both native grains. All conversion,
//! grading and resampling is offline; this path only stamps registered pixels.
use super::super::ink::Rgb;
use super::*;
use std::sync::{Arc, Mutex, OnceLock};
mod lighting;

// The Swamp of Horrors has no delivered room kit: its slots are empty and
// it keeps its own painted mire (`swamp_art`).
const PACKS: [&str; 7] = ["crypt", "mines", "keep", "archive", "fungal", "unknown", "swamp"];
const FLOOR: [&str; 12] = [
    "plain", "worn", "cracked", "patched", "stained", "rich", "trim-0", "trim-1", "crack",
    "puddle", "debris", "detail",
];
const WALL: [&str; 12] = [
    "face-0",
    "face-1",
    "face-2",
    "face-3",
    "outer-nw",
    "outer-ne",
    "side-left",
    "side-right",
    "inner-nw",
    "inner-ne",
    "cap",
    "collapsed",
];
const BLOCK: [&str; 12] = [
    "pillar-0",
    "pillar-1",
    "pillar-2",
    "tomb-0",
    "tomb-1",
    "tomb-2",
    "boulder-0",
    "boulder-1",
    "boulder-2",
    "broken-0",
    "broken-1",
    "broken-2",
];
const HAZARD: [&str; 12] = [
    "fill-0", "fill-1", "fill-2", "fill-3", "edge-n", "edge-s", "edge-w", "edge-e", "outer-nw",
    "outer-se", "inner-nw", "inner-se",
];
const GATE: [&str; 12] = [
    "barred",
    "open",
    "opening",
    "side-left-barred",
    "side-left-open",
    "side-right-barred",
    "side-right-open",
    "stairs-down",
    "stairs-up",
    "boss",
    "alcove",
    "seal",
];
const DECOR: [&str; 12] = [
    "flame-0", "flame-1", "flame-2", "lamp", "barrel", "bones", "web", "prop-0", "prop-1",
    "decal-0", "decal-1", "glow",
];
const OWALL: [&str; 12] = [
    "face-0",
    "face-1",
    "face-2",
    "corner-nw",
    "corner-ne",
    "west-0",
    "west-1",
    "east-0",
    "east-1",
    "south",
    "corner-sw",
    "corner-se",
];
const ODOOR: [&str; 12] = [
    "barred",
    "open",
    "opening",
    "west-barred",
    "west-open",
    "east-barred",
    "east-open",
    "south-barred",
    "south-open",
    "stairs-down",
    "boss",
    "seal",
];
/// Side doors' drawings, by name: west and east, closed, open, locked.
const SIDEDOOR: [&str; 6] = ["west-closed", "west-open", "west-locked", "east-closed", "east-open", "east-locked"];

/// One kind of a kit's tiles (`FLOOR`, `WALL`, ...): each by its place in
/// the kind's names, and which of them a bake has drawn since `clear_drawn`
/// (indexing one that is there marks it), so the art a room stands on can
/// be listed (`tiles_drawn`).
struct KitTiles {
    kind: &'static str,
    names: &'static [&'static str],
    imgs: Vec<Option<Img>>,
    drawn: Vec<std::sync::atomic::AtomicBool>,
}

impl KitTiles {
    fn new(kind: &'static str, names: &'static [&'static str], imgs: Vec<Option<Img>>) -> Self {
        let drawn = imgs.iter().map(|_| std::sync::atomic::AtomicBool::new(false)).collect();
        Self { kind, names, imgs, drawn }
    }

    /// Whether tile `i` is in the kit, without counting it drawn.
    fn has(&self, i: usize) -> bool {
        self.imgs.get(i).is_some_and(Option::is_some)
    }

    fn mark(&self, i: usize) {
        if self.has(i) {
            self.drawn[i].store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// The same tiles, each made by `f`.
    fn map(&self, f: impl Fn(&Img) -> Img) -> Self {
        Self::new(self.kind, self.names, self.imgs.iter().map(|im| im.as_ref().map(&f)).collect())
    }
}

impl std::ops::Index<usize> for KitTiles {
    type Output = Option<Img>;
    fn index(&self, i: usize) -> &Option<Img> {
        self.mark(i);
        &self.imgs[i]
    }
}

impl std::ops::Deref for KitTiles {
    type Target = [Option<Img>];
    fn deref(&self) -> &[Option<Img>] {
        &self.imgs
    }
}

struct Bank {
    floor: KitTiles,
    slabs: Vec<[Option<Img>;4]>,
    wall: KitTiles,
    block: KitTiles,
    hazard: KitTiles,
    gate: KitTiles,
    decor: KitTiles,
    owall: KitTiles,
    odoor: KitTiles,
    sidedoor: KitTiles,
}

impl Bank {
    fn kinds(&self) -> [&KitTiles; 9] {
        [&self.floor, &self.wall, &self.block, &self.hazard, &self.gate, &self.decor, &self.owall, &self.odoor, &self.sidedoor]
    }

    /// A kit's tiles, its floor's material quadrants made from its floor.
    fn of(floor: KitTiles, [wall, block, hazard, gate, decor, owall, odoor, sidedoor]: [KitTiles; 8], scale: i32) -> Self {
        // Cache the four material quadrants once. Room admission then uses
        // packed row stamps rather than sampling every floor pixel again.
        let slabs=floor.imgs.iter().map(|im|std::array::from_fn(|q|im.as_ref().map(|source| {
            let t=16*scale;let mut out=Img::new(t,t);
            for y in 0..t {for x in 0..t {
                if let Some(c)=source.get((q as i32%2*t+x)/2,(q as i32/2*t+y)/2) {out.set(x,y,c);}
            }}out
        }))).collect();
        Bank { floor, slabs, wall, block, hazard, gate, decor, owall, odoor, sidedoor }
    }
}

fn bank(pack: Pack) -> &'static Bank {
    static BANKS: [OnceLock<Bank>; 7] = [const { OnceLock::new() }; 7];
    BANKS[pack as usize].get_or_init(|| {
        let load = |kind: &'static str, names: &'static [&'static str]| {
            KitTiles::new(kind, names, names
                .iter()
                .map(|name| {
                    let slot = format!("dgn.{kind}.{}", PACKS[pack as usize]);
                    let name = name.to_string();
                    slot_art::bytes(&slot, &name).map(|_| {
                        let mut im=slot_art::load(&slot,&name,&[]);
                        if kind=="decor" || kind=="block" {
                            for y in 0..im.h {for x in 0..im.w {
                                if im.get(x,y).is_some_and(|c| matches!(c,[80,198,179]|[143,232,220]|[228,255,248])) {
                                    im.set(x,y,[47,74,82]);
                                }
                            }}
                        }im
                    })
                })
                .collect())
        };
        let sidedoor = KitTiles::new("sidedoor", &SIDEDOOR, SIDEDOOR.iter().map(|name| {
            let slot = format!("dgn.sidedoor-{}", PACKS[pack as usize]);
            let name = name.to_string();
            slot_art::bytes(&slot, &name).map(|_|slot_art::load(&slot, &name, &[]))
        }).collect());
        Bank::of(load("floor", &FLOOR), [
            load("wall", &WALL),
            load("block", &BLOCK),
            load("hazard", &HAZARD),
            load("gate", &GATE),
            load("decor", &DECOR),
            load("owall", &OWALL),
            load("odoor", &ODOOR),
            sidedoor,
        ], 1)
    })
}

/// A kit's tiles at `scale` (3 and up) from the installed HD pack
/// (`hd_pack`): each 1x tile's drawing where the pack has one at exactly
/// that scale, keyed by the tile's look (`look_key`, the same key a sprite
/// has, so a block standing among the crowd and the same block in the
/// room's art are one drawing), else the tile itself with each pixel
/// `scale` square. Made once a pack, kit and scale.
fn fine_bank(pack: Pack, scale: i32) -> Arc<Bank> {
    type Fine = Mutex<Vec<((Pack, i32, u64), Arc<Bank>)>>;
    static FINE: OnceLock<Fine> = OnceLock::new();
    let art = super::hd_pack::installed();
    let key = (pack, scale, art.as_ref().map_or(0, |a| a.generation()));
    let fine = FINE.get_or_init(Default::default);
    if let Some((_, bank)) = fine.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return bank.clone();
    }
    let base = bank(pack);
    let finer = |im: &Img| -> Img {
        let drawn = art.as_ref().and_then(|a| a.drawing_at(look_key(im), (im.w as u32, im.h as u32), scale as u32));
        match drawn {
            Some(d) => {
                let mut out = Img::new(d.width as i32, d.height as i32);
                for (i, px) in d.rgba.chunks_exact(4).enumerate() {
                    if px[3] >= 128 {
                        out.set(i as i32 % d.width as i32, i as i32 / d.width as i32, [px[0], px[1], px[2]]);
                    }
                }
                out
            }
            None => {
                let mut out = Img::new(im.w * scale, im.h * scale);
                out.stamp_times(im, 0, 0, scale);
                out
            }
        }
    };
    let bank = Arc::new(Bank::of(base.floor.map(&finer), [
        base.wall.map(&finer),
        base.block.map(&finer),
        base.hazard.map(&finer),
        base.gate.map(&finer),
        base.decor.map(&finer),
        base.owall.map(&finer),
        base.odoor.map(&finer),
        base.sidedoor.map(&finer),
    ], scale));
    let mut fine = fine.lock().unwrap_or_else(|e| e.into_inner());
    // A few kits and packs at a time: a host walks one floor's kit.
    if fine.len() >= 8 {
        fine.remove(0);
    }
    fine.push((key, bank.clone()));
    bank
}

/// The scale the installed HD pack composes this room's kit at, when it has
/// a drawing of any of the kit's tiles: a pack of knights alone leaves the
/// room as it was.
pub(super) fn fine_scale(run: &Run) -> Option<i32> {
    type Covered = Mutex<Vec<((Pack, u64), bool)>>;
    static COVERED: OnceLock<Covered> = OnceLock::new();
    let scale = super::hd_pack::room_scale()?;
    let art = super::hd_pack::installed()?;
    let key = (run.dungeon.pack, art.generation());
    let covered = COVERED.get_or_init(Default::default);
    let known = covered.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key).map(|&(_, c)| c);
    let any = known.unwrap_or_else(|| {
        let base = bank(run.dungeon.pack);
        let any = base
            .kinds()
            .iter()
            .flat_map(|tiles| tiles.imgs.iter().flatten())
            .any(|im| art.drawing_at(look_key(im), (im.w as u32, im.h as u32), scale as u32).is_some());
        let mut covered = covered.lock().unwrap_or_else(|e| e.into_inner());
        if covered.len() >= 16 {
            covered.remove(0);
        }
        covered.push((key, any));
        any
    });
    any.then_some(scale)
}

/// Every tile of `run`'s kit at the game's grain, by kind and name.
#[cfg(any(test, feature = "testing"))]
pub(super) fn kit_tiles(run: &Run) -> Vec<(&'static str, &'static str, Img)> {
    let b = bank(run.dungeon.pack);
    b.kinds()
        .iter()
        .flat_map(|tiles| tiles.imgs.iter().zip(tiles.names).filter_map(|(im, &name)| Some((tiles.kind, name, im.clone()?))))
        .collect()
}

/// The tiles of `run`'s kit its room is laid with at the game's grain (its
/// fires' every flame too), by kind and name: the art the room stands on,
/// for an art request (`hd_pack`'s `write_hd_request`).
#[cfg(any(test, feature = "testing"))]
pub(super) fn tiles_drawn(run: &Run) -> Vec<(&'static str, &'static str, Img)> {
    let b = bank(run.dungeon.pack);
    for tiles in b.kinds() {
        for drawn in &tiles.drawn {
            drawn.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let scene = bake(run, 1);
    if !scene.fires.is_empty() {
        (0..3).for_each(|i| b.decor.mark(i));
    }
    b.kinds()
        .iter()
        .flat_map(|tiles| {
            tiles.imgs.iter().zip(tiles.names).zip(&tiles.drawn).filter_map(|((im, &name), drawn)| {
                drawn.load(std::sync::atomic::Ordering::Relaxed).then(|| Some((tiles.kind, name, im.clone()?))).flatten()
            })
        })
        .collect()
}
/// Decode and prepare each delivered bank before the first displayed room.
pub(super) fn warm() {
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown] {
        let _ = bank(pack);
    }
}

pub(super) fn supported(run: &Run) -> bool {
    !run.race3d_showing()
        && !super::atari::here(run)
        && slot_art::enabled()
        && !run.at_home_now()
        && !run.side_on()
        && run.circle() == 0
        && run.mode != crate::rules::fortune::Mode::LightsOut
        && run.room().kind != RoomKind::Wanted
        && run.room().kind != RoomKind::DeepVillage
        // The Wax Maze lays its own stone and lanes (`wax_maze::walls`).
        && !super::wax_maze::walled(run)
        && !super::belfry_shoot::walled(run)
        && !super::powder_room::walled(run)
        && !super::book_hop::walled(run)
        && bank(run.dungeon.pack).floor.imgs[..6]
            .iter()
            .all(Option::is_some)
}
#[derive(Clone, Copy)]
pub(super) struct Object {
    pub x: i32,
    pub feet: i32,
    pub index: usize,
}
pub(super) struct Scene {
    pub base: Img,
    pub objects: Vec<Object>,
    pub fires: Vec<(i32, i32)>,
    shimmer: Vec<(i32, i32, [Rgb; 4])>,
}
#[derive(PartialEq, Eq, Clone)]
struct Key {
    layout: u64,
    room: usize,
    seed: u64,
    pack: Pack,
    barred: bool,
    side_state: usize,
    /// At 3 and up, the scale and the HD pack the tiles are drawn from.
    fine: Option<(i32, u64)>,
}
pub(super) fn scene(run: &Run) -> Arc<Scene> {
    scene_at(run, 1)
}
/// The room's still art at the game's grain (1), or at `s` (3 and up) from
/// the installed HD pack's tiles (`fine_bank`); one kept for each.
fn scene_at(run: &Run, s: i32) -> Arc<Scene> {
    #[cfg(test)]
    let _gate = if HOLDING_CACHE_GATE.with(|h| h.get()) {
        None
    } else {
        Some(CACHE_GATE.lock().unwrap_or_else(|p| p.into_inner()))
    };
    type Cache = Mutex<Option<(Key, Arc<Scene>)>>;
    static CACHE: [OnceLock<Cache>; 2] = [const { OnceLock::new() }; 2];
    let cache = CACHE[usize::from(s >= 3)].get_or_init(Default::default);
    let key = Key {
        layout: run.room().fingerprint(),
        room: run.at,
        seed: run.raid_id,
        pack: run.dungeon.pack,
        barred: run.barred(),
        side_state: side_state(run),
        fine: (s >= 3).then(|| (s, super::hd_pack::installed().map_or(0, |a| a.generation()))),
    };
    if let Some((k, scene)) = cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *k == key {
            return scene.clone();
        }
    }
    let scene = Arc::new(bake(run, s));
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, scene.clone()));
    scene
}
// Existing combat bars and an empty-room wave lock have the same collision.
// Their delivered drawings distinguish the live foe closure from the wave lock.
pub(super) fn side_state(run: &Run) -> usize {
    if !run.barred() {1} else if run.foes_left() {0} else {2}
}

#[cfg(test)]
thread_local! {
    static HOLDING_CACHE_GATE: std::cell::Cell<bool> = std::cell::Cell::new(false);
}
#[cfg(test)]
static CACHE_GATE: Mutex<()> = Mutex::new(());

/// Hold the scene cache so a parallel test cannot evict it mid-check.
#[cfg(test)]
fn pin_scene_cache() -> impl Drop {
    struct Pin {
        _guard: std::sync::MutexGuard<'static, ()>,
    }
    impl Drop for Pin {
        fn drop(&mut self) {
            HOLDING_CACHE_GATE.with(|h| h.set(false));
        }
    }
    let guard = CACHE_GATE.lock().unwrap_or_else(|p| p.into_inner());
    HOLDING_CACHE_GATE.with(|h| h.set(true));
    Pin { _guard: guard }
}
fn stamp(cv: &mut Img, bank: &KitTiles, index: usize, x: i32, y: i32) {
    if let Some(im) = &bank[index] {
        cv.stamp(im, x, y);
    }
}
fn wall(cv: &mut Img, b: &Bank, room: &Room, c: i32, r: i32, s: i32, v: usize) {
    let t = 16 * s;
    let (x, y) = (c * t, r * t);
    if b.owall.has(0) {
        let i = if r == 0 {
            if c == 0 {
                3
            } else if c == room.cols as i32 - 1 {
                4
            } else {
                if c % 5 == 3 { 2 } else { v % 2 }
            }
        } else if r == room.rows as i32 - 1 {
            if c == 0 {
                10
            } else if c == room.cols as i32 - 1 {
                11
            } else {
                9
            }
        } else if c == 0 {
            5 + v % 2
        } else if c == room.cols as i32 - 1 {
            7 + v % 2
        } else {
            v % 3
        };
        if let Some(im) = &b.owall[i] {
            cv.stamp(im, x, if r == 0 || (c > 0 && c < room.cols as i32 - 1 && r < room.rows as i32 - 1) { y } else { y + t - im.h });
            return;
        }
    }
    // The old angled corners/side runs are deliberately not transformed.
    if r == 0 || (c > 0 && c < room.cols as i32 - 1 && r < room.rows as i32 - 1) {
        if let Some(im) = &b.wall[v % 4] {
            cv.stamp(im, x, y + t - im.h);
        }
    } else if r == room.rows as i32 - 1 && b.wall.has(6) {
        if let Some(im) = &b.wall[6 + v % 2] {
            cv.stamp(im, x, y + t - im.h);
        }
    } else if let Some(im) = &b.wall[10] {
        // End-pillars give a truthful upright boundary until flat caps arrive.
        cv.stamp(im, x, y + t - im.h);
    }
}
fn hazard(cv: &mut Img, b: &Bank, room: &Room, c: i32, r: i32, s: i32, v: usize) {
    let t = 16 * s;
    let (x, y) = (c * t, r * t);
    if !b.hazard.has(0) {
        return;
    }
    stamp(cv, &b.hazard, v % 4, x, y);
    let dry = |dc, dr| room.tile(c + dc, r + dr) != Tile::Hazard;
    // Rim samples are clipped to the danger tile: they never claim a dry tile
    // is blocked. Quarter tiles compose every combination without rotating art.
    for (edge, (dx, dy)) in [(0, -1), (0, 1), (-1, 0), (1, 0)].into_iter().enumerate() {
        if !dry(dx, dy) {
            continue;
        }
        if let Some(im) = &b.hazard[4 + edge] {
            for yy in 0..t {
                for xx in 0..t {
                    let rim = match edge {
                        0 => yy < t / 4,
                        1 => yy >= t * 3 / 4,
                        2 => xx < t / 4,
                        _ => xx >= t * 3 / 4,
                    };
                    if rim {
                        if let Some(c) = im.get(xx, yy) {
                            cv.set(x + xx, y + yy, c);
                        }
                    }
                }
            }
        }
    }
    for (west, north) in [(true, true), (false, true), (true, false), (false, false)] {
        let dx = if west { -1 } else { 1 };
        let dy = if north { -1 } else { 1 };
        let outer = dry(dx, 0) && dry(0, dy);
        let inner = !dry(dx, 0) && !dry(0, dy) && dry(dx, dy);
        if !outer && !inner {
            continue;
        }
        // The delivered top-left outer and top-right inner quadrants cover
        // their counterparts by reflection, as requested by the art judge.
        let index = if outer { 8 } else { 10 };
        if let Some(im) = &b.hazard[index] {
            let q = t / 3;
            for yy in 0..q {
                for xx in 0..q {
                    let sx = if outer { xx } else { t - 1 - xx };
                    if let Some(c) = im.get(sx, yy) {
                        cv.set(
                            x + if west { xx } else { t - 1 - xx },
                            y + if north { yy } else { t - 1 - yy },
                            c,
                        );
                    }
                }
            }
        }
    }
}
fn objects_for_room(run:&Run,s:i32)->Vec<Object> {
    let room=run.room();let t=16*s;
    let seed=(run.raid_id as u32).wrapping_add(run.at as u32*31);
    let mut objects=vec![];
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            if room.tile(c, r) == Tile::Block
                && room.kind != RoomKind::Forge
                && room.kind != RoomKind::DwarfHall
                && !room.kind.nook()
                && !(room.kind == RoomKind::Threshold && r <= 3)
            {
                let v = hash(c, r, seed);
                let (x, y) = (c * t, r * t);
                // Templates own collision; type selection never adds a prop to
                // an unblocked floor cell. Three families, all one tile wide.
                let family = match run.dungeon.pack {
                    Pack::Crypt | Pack::Archive => 3,
                    Pack::Cavern => 6,
                    _ => 0,
                };
                let index = family + v as usize % 3;
                let object = Object {
                    x,
                    feet: y + t,
                    index,
                };
                objects.push(object);
            }
        }
    }
    objects
}
fn bake(run: &Run, s: i32) -> Scene {
    let room = run.room();
    let fine = (s >= 3).then(|| fine_bank(run.dungeon.pack, s));
    let b = fine.as_deref().unwrap_or_else(|| bank(run.dungeon.pack));
    let t = 16 * s;
    let mut cv = Img::black(room.cols as i32 * t, room.rows as i32 * t);
    let objects;
    // An open floor's edge is rock, left dark: its fires are its own
    // (`open::fixtures`, drawn over this), not the kit's sconces round it.
    let mut fires = if run.open_here().is_some() { Vec::new() } else { lighting::fixtures(room, t) };
    let seed = (run.raid_id as u32).wrapping_add(run.at as u32 * 31);
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            let v = hash(c / 2, r / 2, seed);
            let (x, y) = (c * t, r * t);
            match room.tile(c, r) {
                Tile::Floor | Tile::Bog | Tile::Door | Tile::Stairs | Tile::Ledge | Tile::Block => {
                    let variant = if run.dungeon.pack == Pack::Fungal {
                        // Quiet broad earth in the fighting lanes; root-rich
                        // deliveries stay rare and against the perimeter.
                        if v % 29 == 0 { 1 } else { 0 }
                    } else if run.dungeon.pack == Pack::Hellforge {
                        if v % 23 == 0 {
                            2
                        } else {
                            [0, 1, 3][v as usize % 3]
                        }
                    } else if v % 16 == 0 {
                        5
                    } else if v % 11 == 0 {
                        3
                    } else {
                        [0, 1, 2, 4][v as usize % 4]
                    };
                    let variant = if run.dungeon.pack != Pack::Fungal && room.tile(c, r - 1) == Tile::Wall {
                        6 + (v as usize % 2)
                    } else {
                        variant
                    };
                    // A source slab spans two collision cells. Registration and
                    // collision stay on the original grid; only material scale changes.
                    if let Some(im) = &b.slabs[variant][(r%2*2+c%2) as usize] {
                        b.floor.mark(variant);
                        cv.stamp(im,x,y);
                    }

                }
                Tile::Hazard => hazard(&mut cv, b, room, c, r, s, v as usize),
                Tile::Wall => {},
            }
        }
    }
    let (mut shimmer, reveal) = lighting::floor(&mut cv, run, s, &fires);
    // What stands on the floor is drawn alone, put in the room's light,
    // then laid over it.
    let floor = std::mem::replace(&mut cv, Img::new(room.cols as i32 * t, room.rows as i32 * t));
    // Faces overlap the first floor row; drawing them after the floor is
    // essential, otherwise the following tile erases their lower half.
    for r in 0..room.rows as i32 { for c in 0..room.cols as i32 {
        // Rock with no floor round it is the dark the chambers stand in.
        if room.tile(c, r) == Tile::Wall && !super::open::deep_rock(room, c, r) {
            wall(&mut cv, b, room, c, r, s, hash(c,r,seed) as usize);
        }
    }}
    objects=objects_for_room(run,s);
    for &object in &objects {stamp_block(&mut cv,b,object);}
    reveal.grade(&mut cv);
    let standing = std::mem::replace(&mut cv, floor);
    cv.stamp(&standing, 0, 0);
    for (side, &(c, r)) in super::doors(room).iter().enumerate() {
        if !room.doors[side] {
            continue;
        }
        let index = match side {
            0 => usize::from(!run.barred()),
            1 => 5 + usize::from(!run.barred()),
            2 => 7 + usize::from(!run.barred()),
            _ => 3 + usize::from(!run.barred()),
        };
        let (x, y) = (c * t, r * t);
        let side_art = match side {1 => b.sidedoor[3 + side_state(run)].as_ref(), 3 => b.sidedoor[side_state(run)].as_ref(), _ => None};
        if let Some(im) = side_art {
            cv.stamp(im, x, y);
        } else if let Some(im) = &b.odoor[index] {
            cv.stamp(im, x, y);
        } else if side % 2 == 0 {
            stamp(&mut cv, &b.gate, usize::from(!run.barred()), x, y - 8 * s);
        } else if !run.barred() {
            cv.rect(x+2*s,y,t-4*s,2*t,'s');
        } else if run.barred() {
            let mut im = super::sprites::portcullis(false);
            let tint=lighting::colour(run.dungeon.pack);
            for yy in 0..im.h {for xx in 0..im.w {
                if let Some(c)=im.get(xx,yy) {
                    let value=(54*c[0] as u32+183*c[1] as u32+19*c[2] as u32)/256;
                    im.set(xx,yy,tint.map(|v|(v as u32*value/360).min(90) as u8));
                }
            }}
            im.rect(im.w/2-1,im.h/2-1,3,3,'7');
            im.put(im.w/2,im.h/2,'6');
            if s > 1 {
                cv.stamp_times(&im, x, y, s);
            } else {
                cv.stamp(&im, x, y);
            }
        }
    }
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            if crate::rules::caves::floor_mouth(run.room().kind, c, r).is_some()
                || crate::rules::caves::yard_mouth(run.room().kind, c, r).is_some()
            {
                continue;
            }
            if room.tile(c, r) == Tile::Stairs
                && room.tile(c - 1, r) != Tile::Stairs
                && room.tile(c, r - 1) != Tile::Stairs
            {
                if b.odoor.has(9) {
                    stamp(&mut cv, &b.odoor, 9, c * t, r * t);
                } else if b.gate.has(8) {
                    stamp(&mut cv, &b.gate, 8, c * t, r * t);
                } else {
                    let im = super::sprites::stairs(!run.barred());
                    if s > 1 {
                        cv.stamp_times(&im, c * t, r * t, s);
                    } else {
                        cv.stamp(&im, c * t, r * t);
                    }
                }
            }
        }
    }
    // Cluster paired delivered props at wall bases, clear of door mouths.
    // Walkable lanes never gain a solid-looking decoration.
    for (i, &(x,y)) in fires.iter().enumerate() {
        let c = x / t; let r = y / t;
        let neighbour = if c == 0 { (c, r+1) } else if c == room.cols as i32-1 { (c,r-1) } else { (c+1,r) };
        let spacing = if run.dungeon.pack == Pack::Fungal { 6 } else { 2 };
        if i % spacing == 0 && room.tile(neighbour.0,neighbour.1) == Tile::Wall {
            let index = [4,6,7,8][hash(c,r,seed+19) as usize % 4];
            if let Some(im) = &b.decor[index] {
                cv.stamp(im,neighbour.0*t,neighbour.1*t+t-im.h);
            }
        }
    }

    shimmer.retain(|&(x,y,c)| cv.get(x,y)==Some(c[0]));
    Scene {
        base: cv,
        objects,
        fires: std::mem::take(&mut fires),
        shimmer,
    }
}
fn stamp_block(cv: &mut Img, b: &Bank, o: Object) {
    if let Some(im) = &b.block[o.index] {
        cv.stamp(im, o.x, o.feet - im.h);
    }
}
/// A block standing among the crowd at the game's grain (`stamp_block`),
/// noted as a sprite stood up there (`note_body`): a window with an HD
/// pack's drawing of it draws that in its place, the same drawing the
/// room's finer still art has it in (`fine_bank`).
pub(super) fn stand_object(cv: &mut Img, run: &Run, o: Object) {
    if let Some(im) = &bank(run.dungeon.pack).block[o.index] {
        super::note_body(im, o.x, o.feet - im.h);
        cv.stamp(im, o.x, o.feet - im.h);
    }
}
/// The fires and the light shimmering round them at `tick`.
pub(super) fn flames(cv: &mut Img, run: &Run, tick: u64, s: i32, scene: &Scene) {
    let fine = (s >= 3).then(|| fine_bank(run.dungeon.pack, s));
    let b = fine.as_deref().unwrap_or_else(|| bank(run.dungeon.pack));
    for &(x,y,colours) in &scene.shimmer { cv.set(x,y,colours[(tick / 4 % 4) as usize]); }
    for (i, &(x, y)) in scene.fires.iter().enumerate() {
        if let Some(im) = &b.decor[((tick / 6 + i as u64) % 3) as usize] {
            cv.stamp(im, x - im.w / 2, y + 8 * s - im.h);
        }
    }
}
/// The room's still art, kept a torch step (`scenery_cached`): its fires,
/// shimmer and motes drawn at the step's first tick (`torch_tick`), so the
/// kept drawing is the same whichever tick of the step drew it.
pub(super) fn scenery(run: &Run) -> Img {
    let scene = scene(run);
    let mut im = scene.base.clone();
    let tick = super::scenery::torch_tick(run);
    flames(&mut im, run, tick, 1, &scene);
    let tint=lighting::colour(run.dungeon.pack);
    for i in 0..14 {
        let v=hash(i,run.at as i32,71);
        let x=((v as u64+tick/3)%im.w as u64) as i32;
        let y=(((v>>10) as u64+tick/5)%im.h as u64) as i32;
        if let Some(c)=im.get(x,y) {im.set(x,y,std::array::from_fn(|k|((c[k] as u16*89+tint[k] as u16*11)/100) as u8));}
    }
    // Special furniture, traps, chests and sealed walls keep their existing
    // exact geometry and state. Dungeon doors/stairs above own only their art.
    super::room_details(run, &mut im, &mut vec![]);
    stamp_delve_mouth(run, &mut im, 1);
    super::super::cave_seat::mouth_line(&mut im, run);
    im
}
/// The room's still art at `s` (3 and up) from the installed HD pack's
/// tiles (`fine_bank`): `scenery` drawn finer, with its fires, its motes
/// and its fixtures as at 1x (a chest, a trap, a sealed wall are the
/// game's own: each of their pixels `s` square), for a window to draw in
/// the 1x art's place.
pub(super) fn fine_scenery(run: &Run, s: i32) -> Img {
    let scene = scene_at(run, s);
    let mut im = scene.base.clone();
    let tick = super::scenery::torch_tick(run);
    flames(&mut im, run, tick, s, &scene);
    let (w, h) = (im.w / s, im.h / s);
    let tint=lighting::colour(run.dungeon.pack);
    for i in 0..14 {
        let v=hash(i,run.at as i32,71);
        let x=((v as u64+tick/3)%w.max(1) as u64) as i32;
        let y=(((v>>10) as u64+tick/5)%h.max(1) as u64) as i32;
        for (px, py) in (0..s * s).map(|k| (x * s + k % s, y * s + k / s)) {
            if let Some(c)=im.get(px,py) {im.set(px,py,std::array::from_fn(|k|((c[k] as u16*89+tint[k] as u16*11)/100) as u8));}
        }
    }
    let mut details = Img::new(w, h);
    super::room_details(run, &mut details, &mut vec![]);
    im.stamp_times(&details, 0, 0, s);
    stamp_delve_mouth(run, &mut im, s);
    im
}
pub(super) fn contact(cv:&mut Img,run:&Run,x:i32,y:i32,width:i32) {
    // `scene` hashes the whole floor. Callers with a crowd already in hand
    // use `contact_on` so an open floor is not hashed once per figure.
    contact_on(cv,&scene(run),x,y,width);
}
pub(super) fn contact_on(cv:&mut Img,ground:&Scene,x:i32,y:i32,width:i32) {
    let rx=(width/4).clamp(3,14);
    let ground=&ground.base;
    for dy in -2..=2 {for dx in -rx..=rx {
        if dx*dx*4+dy*dy*rx*rx<=rx*rx*4 {
            let c=cv.get(x+dx,y+dy).or_else(||ground.get(x+dx,y+dy)).unwrap_or([0;3]);
            cv.set(x+dx,y+dy,c.map(|v|(v as u16*2/5) as u8));
        }
    }}
}
pub(super) fn foe_rim(im:&Img,pack:Pack)->Img {
    // The source silhouette is retained. A one-pixel upper/left edge
    // distinguishes moving threats from the desaturated static props.
    let mut out=im.clone();
    let tint=match pack {
        Pack::Crypt=>[99,157,194],Pack::Unknown=>[143,163,168],
        Pack::Hellforge=>[110,138,142],Pack::Cavern=>[143,163,168],
        Pack::Fungal=>[179,194,196],Pack::Archive=>[176,178,168],
        Pack::Swamp=>[150,176,140],
    };
    for y in 0..im.h {for x in 0..im.w {
        if let Some(c)=im.get(x,y) {
            if im.get(x-1,y).is_none() || im.get(x,y-1).is_none() {
                // Shared-palette rim replaces only the outer source pixel.
                if u16::from(c[0])+u16::from(c[1])+u16::from(c[2])<250 {out.set(x,y,std::array::from_fn(|k|((c[k] as u16*2+tint[k] as u16)/3) as u8));}
            }
        }
    }}out
}
pub(super) fn knight_shot(cv:&mut Img,p:&crate::rules::Projectile,x:i32,y:i32)->bool {
    if p.hostile || !matches!(p.kind,Shot::Arrow|Shot::Bolt) || p.sprite_card().is_some()
        || p.look.is_some() || p.empowered || p.traits.path.beam.is_some() || p.traits.path.waiting() {return false;}
    let speed=p.vx.abs().max(p.vy.abs()).max(0.01);
    let (dx,dy)=(p.vx/speed,p.vy/speed);
    // A stroke 8 pixels long at its old size. A one-pixel line is no
    // thinner when a shot is smaller, so its length follows the square
    // root of the shot's size (`wishcraft`), and a stroke is never under
    // 5 pixels: its direction still reads.
    let k=p.scale().sqrt();
    let at=|n:f32|(x+(dx*n*k).round() as i32,y+(dy*n*k).round() as i32);
    let (tx,ty)=at(-5.0);let (hx,hy)=at(2.0);
    // Short direction-bearing stroke; the one-pixel contour follows that
    // stroke instead of making a black disc behind the whole projectile.
    // A small shot is its stroke alone: the contour would be most of it.
    if k>=0.85 {
        for (ox,oy) in [(-1,0),(1,0),(0,-1),(0,1)] {cv.line(tx+ox,ty+oy,hx+ox,hy+oy,'s');}
    }
    cv.line(tx,ty,hx,hy,'2');
    let (mx,my)=at(-2.0);cv.line(mx,my,hx,hy,'V');
    cv.put(hx,hy,'W');true
}
#[cfg(test)]
mod tests {
    use super::*;
    fn four(pack: Pack) -> Run {
        let mut r = super::super::testing::fight();
        for id in 2..=4 {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.dungeon.pack = pack;
        r.floor_plates.clear();
        r
    }

    fn inked(im: &Img) -> usize {
        let mut n = 0;
        for y in 0..im.h {
            for x in 0..im.w {
                if im.get(x, y).is_some() {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn floor_two_start_room_has_no_arch_and_no_label() {
        let mut run = Run::new(7, 1, Some("Ash"));
        run.descend();
        assert_eq!(run.dungeon.depth, 1);
        let start = run
            .dungeon
            .rooms
            .iter()
            .position(|room| room.kind == crate::rules::caves::FLOOR_MOUTH.room)
            .expect("floor 1 has a start room");
        run.at = start;
        let (col, row, _, _) = crate::rules::caves::FLOOR_MOUTH.at;
        let hero = run.players.values_mut().next().unwrap();
        hero.x = (col as f32) * crate::rules::TILE_UNITS;
        hero.y = (row as f32) * crate::rules::TILE_UNITS;
        hero.stone = false;
        hero.away = false;
        hero.absent = false;
        let mut floor1 = Img::new(320, 240);
        stamp_delve_mouth(&run, &mut floor1, 1);
        let mut lights = Vec::new();
        let mut room1 = Img::new(320, 240);
        super::super::scenery::room_fixtures(&run, &mut room1, &mut lights);
        let mut label1 = Img::new(320, 240);
        super::super::super::cave_seat::mouth_line(&mut label1, &run);
        assert!(inked(&floor1) > 0, "floor 1 drew no arch");
        assert!(inked(&label1) > 0, "floor 1 drew no label");
        let mouth_ink = inked(&room1);
        run.dungeon.depth = 2;
        let mut floor2 = Img::new(320, 240);
        stamp_delve_mouth(&run, &mut floor2, 1);
        let mut room2 = Img::new(320, 240);
        super::super::scenery::room_fixtures(&run, &mut room2, &mut lights);
        let mut label2 = Img::new(320, 240);
        super::super::super::cave_seat::mouth_line(&mut label2, &run);
        assert_eq!(inked(&floor2), 0, "floor 2 drew the arch");
        assert_eq!(inked(&label2), 0, "floor 2 drew the label");
        assert!(inked(&room2) < mouth_ink, "floor 2 scenery still drew the mouth");
    }

    #[test]
    fn side_door_cache_distinguishes_foes_from_an_empty_wave_lock() {
        for pack in [Pack::Crypt,Pack::Cavern,Pack::Hellforge,Pack::Fungal,Pack::Unknown] {
            let mut run=Run::new(17,1,None);
            run.dungeon=crate::rules::crawl_floor(2,pack,17);
            let at=run.dungeon.rooms.iter().position(|r|r.kind==RoomKind::Fight).unwrap();
            run.enter_for_test(at);run.calm_for_test();run.enemies.clear();run.waves=Default::default();
            run.phase=Phase::Fighting;
            for side in [1,3] {run.dungeon.rooms[at].open_door(side);}
            run.spawn_at_for_test(EnemyKind::Imp,24.0,6.0);
            assert!(run.barred());
            let closed=scene(&run);
            run.enemies.clear();run.waves.left=1;
            assert!(run.barred());
            let locked=scene(&run);
            for side in [1,3] {
                let (x,y)=super::super::doors(run.room())[side];
                let rect=(x*16,y*16,16,32);
                let crop=|im:&Img|Img::from_view(im,rect.0,rect.1,rect.2,rect.3).content_hash();
                assert_ne!(crop(&closed.base),crop(&locked.base),"{pack:?} side {side}");
            }
        }
    }

    #[test]
    fn the_kit_covers_each_pack_and_only_collision_tiles_get_obstacles() {
        for pack in [
            Pack::Crypt,
            Pack::Cavern,
            Pack::Hellforge,
            Pack::Archive,
            Pack::Fungal,
            Pack::Unknown,
        ] {
            let r = four(pack);
            assert!(
                super::super::drawn_elsewhere(&r),
                "live wasm must retain the slot scenery path"
            );
            let before = serde_json::to_vec(&r).unwrap();
            {
                let s = 1;
                let a = scene(&r);
                let b = bake(&r, s);
                assert_eq!(a.base, b.base, "cached scenery is exact {pack:?}");
                assert_eq!(
                    (a.base.w, a.base.h),
                    (r.room().cols as i32 * 16 * s, r.room().rows as i32 * 16 * s)
                );
                // Native source banks still obey the shared ink gate. RGB
                // illumination is a post-process.
                for im in bank(pack).floor.iter().flatten() {
                    assert!(im.pixels().all(|c| super::super::super::ink::palette_index(c).is_some()));
                }
                assert!(a.shimmer.iter().all(|&(x,y,_)| r.room().tile(x/(16*s),y/(16*s))!=Tile::Wall));
                let blocks = (0..r.room().rows as i32)
                    .flat_map(|row| (0..r.room().cols as i32).map(move |col| (col, row)))
                    .filter(|&(c, row)| r.room().tile(c, row) == Tile::Block)
                    .count();
                assert_eq!(a.objects.len(), blocks);
                for o in &a.objects {
                    assert_eq!(
                        r.room().tile(o.x / (16 * s), o.feet / (16 * s) - 1),
                        Tile::Block
                    );
                    assert!(bank(pack).block[o.index].is_some());
                }
                for f in &bank(pack).floor.imgs[..6] {
                    let f = f.as_ref().unwrap();
                    let brightness = f
                        .pixels()
                        .map(|c| {
                            (u32::from(c[0]) * 54 + u32::from(c[1]) * 183 + u32::from(c[2]) * 19)
                                / 256
                        })
                        .sum::<u32>()
                        / (f.w * f.h) as u32;
                    assert!(brightness < 65, "quiet floor {pack:?}: {brightness}");
                    assert!(
                        (0..16 * s).all(|y| (0..16 * s).all(|x| f.get(x, y).is_some())),
                        "no transparent seams {pack:?}"
                    );
                }
            }
            assert_eq!(before, serde_json::to_vec(&r).unwrap());
        }
    }
    fn luma_of(c: Rgb) -> u32 {
        (u32::from(c[0]) * 54 + u32::from(c[1]) * 183 + u32::from(c[2]) * 19) / 256
    }

    #[test]
    fn a_rooms_floor_away_from_every_light_is_near_black_and_beside_a_sconce_it_is_lit() {
        for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown] {
            let run = four(pack);
            let room = run.room();
            let t = 16;
            let scene = scene(&run);
            assert!(!scene.fires.is_empty(), "{pack:?}: the room has sconces");
            // Every light the floor knows: sconces, open doors, hazards.
            let mut lights: Vec<(i32, i32)> = lighting::fixtures(room, t);
            for (side, &(c, r)) in super::super::doors(room).iter().enumerate() {
                if room.doors[side] {
                    lights.push((c * t + t / 2, r * t + t / 2));
                }
            }
            for r in 0..room.rows as i32 {
                for c in 0..room.cols as i32 {
                    if room.tile(c, r) == Tile::Hazard {
                        lights.push((c * t + t / 2, r * t + t / 2));
                    }
                }
            }
            let near = |x: i32, y: i32, tiles: i32| lights.iter().any(|&(lx, ly)| (lx - x).pow(2) + (ly - y).pow(2) < (tiles * t).pow(2));
            let (mut far, mut lit) = (Vec::new(), Vec::new());
            for r in 0..room.rows as i32 {
                for c in 0..room.cols as i32 {
                    if room.tile(c, r) != Tile::Floor {
                        continue;
                    }
                    // A tile's middle, away from the walls' faces.
                    let (x, y) = (c * t + t / 2, r * t + t / 2);
                    let Some(px) = scene.base.get(x, y) else { continue };
                    if !near(x, y, 4) {
                        far.push(luma_of(px));
                    } else if scene.fires.iter().any(|&(fx, fy)| (fx - x).pow(2) + (fy - y).pow(2) <= (2 * t).pow(2)) {
                        lit.push(luma_of(px));
                    }
                }
            }
            assert!(far.len() > 10, "{pack:?}: a room has floor away from its lights ({})", far.len());
            let mean = far.iter().sum::<u32>() / far.len() as u32;
            assert!(mean < 20, "{pack:?}: the unlit floor is near black, mean luma {mean}");
            assert!(!lit.is_empty(), "{pack:?}: floor beside a sconce");
            let beside = *lit.iter().max().unwrap();
            assert!(beside >= mean * 2 + 12, "{pack:?}: beside a sconce {beside} against {mean} in the dark");
        }
    }

    #[test]
    fn an_open_floors_deep_rock_is_left_black_and_its_edge_has_no_kit_sconces() {
        let mut run = Run::new(7, 7, None);
        run.join_seat(2, "Wren");
        run.join_seat(3, "Bors");
        run.open_floor_for_test(3);
        assert!(run.open_here().is_some());
        assert!(supported(&run), "the open map takes the slot scenery");
        let room = run.room();
        let scene = scene(&run);
        let t = 16;
        // The rock a wall's art cannot spill onto: rock all round, two deep.
        let core = |c: i32, r: i32| (-1..=1).all(|dr| (-1..=1).all(|dc| super::super::open::deep_rock(room, c + dc, r + dr)));
        let (mut rock, mut walls) = (0, 0);
        for r in 0..room.rows as i32 {
            for c in 0..room.cols as i32 {
                walls += usize::from(room.tile(c, r) == Tile::Wall && !super::super::open::deep_rock(room, c, r));
                if room.tile(c, r) == Tile::Wall && core(c, r) {
                    rock += 1;
                    for y in 0..t {
                        for x in 0..t {
                            let px = scene.base.get(c * t + x, r * t + y).unwrap_or([0; 3]);
                            assert!(px == [0; 3], "deep rock at {c},{r} is black, was {px:?} at {x},{y}");
                        }
                    }
                }
            }
        }
        assert!(walls > 20, "the walls round the floor are still drawn ({walls})");
        assert!(rock > 100, "an open map is mostly rock ({rock})");
        // The kit's sconces round the edge are not used; the floor's own fires light it.
        assert!(scene.fires.is_empty());
        assert!(!lights(&run).is_empty());
        assert_eq!(lights(&run).len(), lighting::open_fires(&run).len());
    }

    #[test]
    fn light_fields_are_tick_only_and_do_not_rebake_for_flicker() {
        let r=four(Pack::Crypt);
        let mut later=r.clone();later.tick+=4;
        {
            let s = 1;
            // Hold the one-entry scene cache so a parallel room cannot take it
            // between the two lookups. Tick is not part of the key.
            let _pin = pin_scene_cache();
            let a=scene(&r);let b=scene(&later);
            assert!(Arc::ptr_eq(&a,&b), "drawing a later tick takes the baked room, not a new one");
            assert!(!a.fires.is_empty());
            assert!(a.shimmer.iter().any(|&(_,_,c)| c[0]!=c[1]));
            let mut im=a.base.clone();flames(&mut im,&r,r.tick,s,&a);
            let mut other=a.base.clone();flames(&mut other,&later,later.tick,s,&b);
            assert_ne!(im,other);
        }
    }
    #[test]
    fn friendly_arrow_keeps_a_thin_directional_core_without_a_disc() {
        let p:crate::rules::Projectile=serde_json::from_value(serde_json::json!({
            "x":7.0,"y":7.0,"vx":1.0,"vy":0.0,"hostile":false,
            "kind":"arrow","damage":1,"pierce":0,"ttl":100
        })).unwrap();
        let mut im=Img::new(32,32);
        assert!(knight_shot(&mut im,&p,16,16));
        assert!((0..32).all(|y| (0..32).all(|x| (15..=17).contains(&y) || im.get(x,y).is_none())));
        assert!(im.get(18,16).is_some());
        let mut hostile=p;hostile.hostile=true;
        assert!(!knight_shot(&mut im,&hostile,16,16));
    }
    #[test]
    fn the_fungal_decor_keeps_none_of_its_cyan_inks() {
        for im in bank(Pack::Fungal).decor.iter().flatten() {
            assert!(im.pixels().all(|c|!matches!(c,[80,198,179]|[143,232,220]|[228,255,248])));
        }
    }
    #[test]
    fn every_hazard_neighbourhood_stays_inside_its_collision_tile() {
        for pack in [
            Pack::Crypt,
            Pack::Cavern,
            Pack::Hellforge,
            Pack::Archive,
            Pack::Fungal,
            Pack::Unknown,
        ] {
            let r = four(pack);
            {
                let s = 1;
                for mask in 0..256 {
                    let mut room = r.room().clone();
                    room.set(1, 1, Tile::Hazard);
                    for (i, (dx, dy)) in [
                        (-1, -1),
                        (0, -1),
                        (1, -1),
                        (-1, 0),
                        (1, 0),
                        (-1, 1),
                        (0, 1),
                        (1, 1),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        room.set(
                            (1 + dx) as usize,
                            (1 + dy) as usize,
                            if mask & (1 << i) != 0 {
                                Tile::Hazard
                            } else {
                                Tile::Floor
                            },
                        );
                    }
                    let t = 16 * s;
                    let mut cv = Img::new(t * 3, t * 3);
                    hazard(&mut cv, bank(pack), &room, 1, 1, s, 0);
                    for y in 0..cv.h {
                        for x in 0..cv.w {
                            if x < t || x >= 2 * t || y < t || y >= 2 * t {
                                assert_eq!(
                                    cv.get(x, y),
                                    None,
                                    "hazard must not paint dry collision tiles"
                                );
                            }
                        }
                    }
                    assert!((t..2 * t).any(|y| (t..2 * t).any(|x| cv.get(x, y).is_some())));
                }
            }
        }
    }
    #[test]
    fn door_state_changes_art_without_changing_geometry() {
        let mut r = four(Pack::Crypt);
        let at = r.at;
        // A prop to stand behind, whatever the floor laid in this room.
        if !(0..r.room().rows as i32).any(|y| (0..r.room().cols as i32).any(|x| r.room().tile(x, y) == Tile::Block)) {
            r.dungeon.rooms[at].set_for_test(5, 5, Tile::Block);
        }
        for side in 0..4 {
            r.dungeon.rooms[at].open_door(side);
        }
        let mut open = r.clone();
        open.enemies.clear();
        open.phase = Phase::Exploring;
        assert_eq!(r.room().fingerprint(), open.room().fingerprint());
        assert_ne!(scene(&r).base, scene(&open).base);
        {
            let barred = scene(&r);
            let clear = scene(&open);
            let t = 16;
            for (side, &(c, row)) in super::super::doors(r.room()).iter().enumerate() {
                let (x0, y0) = (c * t, row * t);
                let (w, h) = if side % 2 == 0 {
                    (2 * t, t)
                } else {
                    (t, 2 * t)
                };
                assert!(
                    (y0..y0 + h).any(
                        |y| (x0..x0 + w).any(|x| barred.base.get(x, y) != clear.base.get(x, y))
                    ),
                    "door {side} changes"
                );
            }
        }
    }
    #[test]
    fn a_torch_steps_drawing_is_the_same_whichever_of_its_ticks_drew_it() {
        // Two ticks of one torch step, a shimmer phase and the motes apart.
        let mut first = four(Pack::Crypt);
        first.tick = 600;
        let mut last = first.clone();
        last.tick = 605;
        assert_eq!(super::super::scenery::torch_step(&first), super::super::scenery::torch_step(&last));
        assert_eq!(scenery(&first), scenery(&last));
        // The drawing kept for the step, baked by its last tick (another
        // seat, another test's thread), draws the first tick's frame as its
        // own bake does: hold the one kept drawing so nothing else bakes.
        let mut elsewhere = first.clone();
        elsewhere.raid_id ^= 1;
        let frame = |run: &Run| super::super::frame_v1(run, 384, 224, Some(1));
        let _pin = super::super::scenery::pin_scene_cache();
        frame(&elsewhere);
        let own = frame(&first);
        frame(&elsewhere);
        frame(&last);
        assert!(own == frame(&first), "the step's kept drawing is the first tick's own");
    }
    #[test]
    fn four_seat_rendering_is_read_only_and_lights_out_retains_its_old_path() {
        let mut r = four(Pack::Crypt);
        let before = serde_json::to_vec(&r).unwrap();
        for id in 1..=4 {
            let a = super::super::frame_v1(&r, 384, 224, Some(id));
            assert_eq!(a, super::super::frame_v1(&r, 384, 224, Some(id)));
        }
        assert_eq!(before, serde_json::to_vec(&r).unwrap());
        r.mode = crate::rules::fortune::Mode::LightsOut;
        assert!(!supported(&r));
    }
}

/// A frame of snapped paper, the colour `palette_frame` gives every
/// uninked word. Filling once matches snapping the letterbox pixel by pixel.
pub(super) fn paper_frame(w: i32, h: i32) -> Img {
    let v = snapper()([0, 0, 0]);
    let word = 0xff00_0000 | (v[0] as u32) << 16 | (v[1] as u32) << 8 | v[2] as u32;
    let n = (w.max(0) as usize).saturating_mul(h.max(0) as usize);
    Img::from_words(w.max(0), h.max(0), vec![word; n])
}

/// v1 transport retains the realm's indexed-ink contract after RGB lighting.
/// Exact RGB neighbours are memoised; native v2 keeps the continuous grade.
pub(super) fn palette_frame(im: Img) -> Img {
    let mut snap = snapper();
    let pixels=im.words().iter().map(|&word| {
        let v=snap([(word>>16) as u8,(word>>8) as u8,word as u8]);
        0xff00_0000 | (v[0] as u32)<<16 | (v[1] as u32)<<8 | v[2] as u32
    }).collect();
    Img::from_words(im.w,im.h,pixels)
}

/// The colour `palette_frame` snaps each pixel to, its memo held while the
/// snapper lives: a frame in parts snaps what it composes the same way
/// (`room_scene`).
pub(super) fn snapper() -> impl FnMut(Rgb) -> Rgb {
    use super::super::ink::{PALETTE,RgbMap,every_colour,rgb};
    static COLOURS:OnceLock<Mutex<RgbMap<Rgb>>>=OnceLock::new();
    // Every ink stays itself, a dye too; only a colour off them all is
    // snapped, and only ever to the realm's own.
    let mut colours=COLOURS.get_or_init(|| Mutex::new(every_colour().map(|c|(rgb(c),rgb(c))).collect()))
        .lock().unwrap_or_else(|e|e.into_inner());
    move |c: Rgb| *colours.entry(c).or_insert_with(|| PALETTE.iter().filter(|&&p|p!=0xe4fff8).map(|&p|rgb(p))
        .min_by_key(|p|(0..3).map(|k|(p[k] as i32-c[k] as i32).pow(2)).sum::<i32>()).unwrap())
}

/// `im`'s inked pixels snapped as `palette_frame` snaps them; its paper
/// stays clear.
pub(super) fn palette_layer(im: &mut Img) {
    let mut snap = snapper();
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                im.set(x, y, snap(c));
            }
        }
    }
}

/// The delivered kit's fixtures as the room's own fires, for a window that
/// lights the room itself (`room_scene`): their pools are already in the
/// still art, as a brazier's are in the first kit's.
pub(super) fn lights(run: &Run) -> Vec<super::super::light::Light> {
    if run.open_here().is_some() {
        return lighting::open_fires(run);
    }
    lighting::fixtures(run.room(), 16)
        .into_iter()
        .map(|(x, y)| super::super::light::Light { x: x as f32, y: y as f32, r: 52.0, s: 0.28, fire: true })
        .collect()
}

fn stamp_delve_mouth(run: &Run, im: &mut Img, s: i32) {
    if run.dungeon.depth != 1 {
        return;
    }
    let mouth = crate::rules::caves::FLOOR_MOUTH;
    if run.room().kind != mouth.room {
        return;
    }
    let (x, y, _, _) = mouth.at;
    let t = super::TILE;
    super::super::cave_seat::stamp_mouth(im, (x * t + (t - 32) / 2) * s, (y * t - 9) * s, s);
}
