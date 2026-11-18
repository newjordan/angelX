//! The room's scenery: floor, walls and fixtures drawn in the dusk's light,
//! and kept so a room is not drawn again every tick. A dungeon room is
//! drawn in stages (`DungeonBake`); a live seat's torches flicker on a bake
//! that lands off the paint thread (`scenery_live`).

use super::*;

std::thread_local! {
    static WIRE_SCENERY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn wire_scenery(on: bool) -> impl Drop {
    struct Back(bool);
    impl Drop for Back {
        fn drop(&mut self) { WIRE_SCENERY.with(|v| v.set(self.0)); }
    }
    Back(WIRE_SCENERY.with(|v| v.replace(on)))
}

fn still_wire(run: &Run) -> bool {
    WIRE_SCENERY.with(|v| v.get()) && !run.at_home_now()
}

pub(super) fn scenery_cached(run: &Run) -> std::sync::Arc<Img> {
    if live_frame() && !still_wire(run) {
        return scenery_live(run);
    }
    scenery_cached_exact(run)
}

pub(super) fn fine_scenery_cached(run: &Run) -> Option<(std::sync::Arc<Img>, i32)> {
    type Key = (SteadyKey, u64, i32, u64, bool);
    static CACHE: OnceLock<std::sync::Mutex<Option<(Key, std::sync::Arc<Img>)>>> = OnceLock::new();
    let s = dungeon_art::fine_scale(run)?;
    let room = run.room();
    if room.cols > crate::rules::layout::MAX_COLS
        || room.rows > crate::rules::layout::MAX_ROWS
        || !dungeon_art::supported(run)
        || entrance_art::supported(run)
        || laid_plate(run).is_some()
        || world_stood(run)
    {
        return None;
    }
    let generation = hd_pack::installed().map_or(0, |p| p.generation());
    let key: Key = (steady_key(run), torch_step(run), s, generation, still_wire(run));
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(slot) = cache.lock()
        && let Some((k, img)) = slot.as_ref()
        && *k == key
    {
        return Some((std::sync::Arc::clone(img), s));
    }
    let mut frozen;
    let run = if still_wire(run) {
        frozen = run.clone();
        frozen.tick = 0;
        &frozen
    } else { run };
    let img = std::sync::Arc::new(dungeon_art::fine_scenery(run, s));
    if let Ok(mut slot) = cache.lock() {
        *slot = Some((key, std::sync::Arc::clone(&img)));
    }
    Some((img, s))
}

pub fn prepare_entry(run: &Run, focus: Option<u32>) {
    let (w, h) = (NATIVE_W, NATIVE_H);
    let _ = frame_for(run, w as i32, h as i32, focus);
    let _ = frame_for_live(run, w as i32, h as i32, focus);
}

pub fn frame_for_live(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    during_live(||frame_for(run,w,h,focus))
}

pub(in super::super) fn during_live<T>(paint:impl FnOnce()->T)->T {
    struct Arm(bool);
    impl Drop for Arm {fn drop(&mut self) {LIVE_FRAME.with(|flag|flag.set(self.0));}}
    let _arm=Arm(LIVE_FRAME.with(|flag|flag.replace(true)));
    paint()
}

std::thread_local! {
    static LIVE_FRAME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn live_frame() -> bool {
    LIVE_FRAME.with(|flag| flag.get())
}

type SteadyKey = (
    u64,
    u32,
    usize,
    u64,
    bool,
    bool,
    Option<bool>,
    usize,
    bool,
    u64,
    usize,
    (bool, crate::world3d::room::Mode),
);

fn same_room(a: &SteadyKey, b: &SteadyKey) -> bool {
    a.0 == b.0 && a.1 == b.1 && a.2 == b.2 && a.3 == b.3 && a.9 == b.9 && a.11 == b.11
}

fn steady_key(run: &Run) -> SteadyKey {
    (
        run.raid_id,
        run.dungeon.depth,
        run.at,
        run.room().fingerprint() ^ run.dungeon.pack as u64 ^ (u64::from(atari::here(run)) << 63) ^ deep_village::print(run) ^ super::reclaim::print(run),
        run.barred(),
        run.phase == Phase::Fighting,
        run.room().chest.map(|c| c.open),
        run.traps.len(),
        run.wishing,
        home_print(run),
        dungeon_art::side_state(run),
        (props_meshes_on(), crate::world3d::room::mode()),
    )
}

struct LiveScenery {
    generation: u64,
    steady: SteadyKey,
    flicker: u64,
    img: std::sync::Arc<Img>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    baking: Option<u64>,
    #[cfg(target_arch = "wasm32")]
    staged: Option<Staged>,
}

#[cfg(target_arch = "wasm32")]
struct Staged {
    flicker: u64,
    run: Box<Run>,
    bake: DungeonBake,
}

type Slot = std::sync::Arc<std::sync::Mutex<Option<LiveScenery>>>;

std::thread_local! {
    static LIVE_SCENERY: Slot = Slot::default();
}

fn live_slot() -> Slot {
    LIVE_SCENERY.with(Slot::clone)
}



static SCENERY_DRAW: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
type Job = (Slot, u64, SteadyKey, u64, Run);

#[cfg(not(target_arch = "wasm32"))]
static BAKE_TX: OnceLock<std::sync::mpsc::SyncSender<Job>> = OnceLock::new();

fn scenery_draw_lock() -> std::sync::MutexGuard<'static, ()> {
    SCENERY_DRAW
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

#[cfg(not(target_arch = "wasm32"))]
fn bake_sender() -> &'static std::sync::mpsc::SyncSender<Job> {
    BAKE_TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Job>(1);
        std::thread::Builder::new()
            .name("delve-scenery".into())
            .spawn(move || {
                while let Ok((slot, generation, steady, flicker, run)) = rx.recv() {
                    props_meshes(steady.11.0);
                    let img = lit_scenery(&run);
                    props_meshes(false);
                    let Ok(mut guard) = slot.lock() else {
                        continue;
                    };
                    let Some(live) = guard.as_mut() else {
                        continue;
                    };
                    if live.generation == generation {
                        live.img = img;
                        live.flicker = flicker;
                        live.steady = steady;
                        if live.baking == Some(generation) {
                            live.baking = None;
                        }
                    }
                }
            })
            .expect("delve scenery baker");
        tx
    })
}

fn scenery_live(run: &Run) -> std::sync::Arc<Img> {
    let steady = steady_key(run);
    let flicker = torch_step(run);
    let slot = live_slot();
    let sync_generation = {
        let mut guard = slot.lock().unwrap_or_else(|err| err.into_inner());
        match guard.as_mut() {
            Some(live) if live.steady == steady && live.flicker == flicker => {
                return std::sync::Arc::clone(&live.img);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Some(live) if live.steady == steady => {
                let stale = std::sync::Arc::clone(&live.img);
                if live.baking != Some(live.generation) {
                    let job = (Slot::clone(&slot), live.generation, steady, flicker, run.clone());
                    if bake_sender().try_send(job).is_ok() {
                        live.baking = Some(live.generation);
                    }
                }
                return stale;
            }
            #[cfg(not(target_arch = "wasm32"))]
            Some(live) if same_room(&live.steady, &steady) => {
                let stale = std::sync::Arc::clone(&live.img);
                if live.baking != Some(live.generation) {
                    let next = live.generation.wrapping_add(1);
                    let job = (Slot::clone(&slot), next, steady, flicker, run.clone());
                    if bake_sender().try_send(job).is_ok() {
                        live.generation = next;
                        live.baking = Some(live.generation);
                    }
                }
                return stale;
            }
            #[cfg(target_arch = "wasm32")]
            Some(live) if live.steady == steady && !drawn_elsewhere(run) && laid_plate(run).is_none() => {
                let staged = live.staged.get_or_insert_with(|| Staged {
                    flicker,
                    run: Box::new(run.clone()),
                    bake: DungeonBake::new(run),
                });
                let done = {
                    let _draw = scenery_draw_lock();
                    staged.bake.step(&staged.run)
                };
                if done && let Some(staged) = live.staged.take() {
                    live.img = std::sync::Arc::new(staged.bake.cv);
                    remember_lights(&live.img, staged.bake.lights);
                    live.flicker = staged.flicker;
                }
                return std::sync::Arc::clone(&live.img);
            }
            Some(live) => {
                live.generation = live.generation.wrapping_add(1);
                live.baking = None;
                live.generation
            }
            None => 1,
        }
    };
    let img = lit_scenery(run);
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(LiveScenery {
            generation: sync_generation,
            steady,
            flicker,
            img: std::sync::Arc::clone(&img),
            baking: None,
            #[cfg(target_arch = "wasm32")]
            staged: None,
        });
    }
    img
}

pub(super) fn scenery_cached_exact(run: &Run) -> std::sync::Arc<Img> {
    #[cfg(test)]
    let _gate = if HOLDING_CACHE_GATE.with(|held| held.get()) {
        None
    } else {
        Some(CACHE_GATE.lock().unwrap_or_else(|error| error.into_inner()))
    };
    type Key = (
        u64,
        u32,
        usize,
        u64,
        bool,
        bool,
        u64,
        Option<bool>,
        usize,
        bool,
        u64,
        (bool, usize, bool, crate::world3d::room::Mode, bool),
    );
    static CACHE: OnceLock<std::sync::Mutex<Option<(Key, std::sync::Arc<Img>)>>> = OnceLock::new();
    let key: Key = (
        run.raid_id,
        run.dungeon.depth,
        run.at,
        run.room().fingerprint() ^ run.dungeon.pack as u64 ^ (u64::from(atari::here(run)) << 63) ^ deep_village::print(run),
        run.barred(),
        run.phase == Phase::Fighting,
        torch_step(run),
        run.room().chest.map(|c| c.open),
        run.traps.len(),
        run.wishing,
        home_print(run),
        (dungeon_art::supported(run), dungeon_art::side_state(run), props_meshes_on(), crate::world3d::room::mode(), still_wire(run)),
    );
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(slot) = cache.lock()
        && let Some((k, img)) = slot.as_ref()
        && *k == key
    {
        return std::sync::Arc::clone(img);
    }
    let img = lit_scenery(run);
    if let Ok(mut slot) = cache.lock() {
        *slot = Some((key, std::sync::Arc::clone(&img)));
    }
    img
}

#[cfg(test)]
thread_local! {
    static HOLDING_CACHE_GATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
static CACHE_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(super) fn pin_scene_cache() -> impl Drop {
    struct Pin {
        _guard: std::sync::MutexGuard<'static, ()>,
    }
    impl Drop for Pin {
        fn drop(&mut self) {
            HOLDING_CACHE_GATE.with(|held| held.set(false));
        }
    }
    let guard = CACHE_GATE.lock().unwrap_or_else(|error| error.into_inner());
    HOLDING_CACHE_GATE.with(|held| held.set(true));
    Pin { _guard: guard }
}

pub(super) fn torch_step(run: &Run) -> u64 {
    if still_wire(run)
        || matches!(run.room().kind, RoomKind::Depths | RoomKind::Alehouse | RoomKind::MusterTent)
        || crate::rules::world::bomrekedan::is_level(run.room().kind)
        || atari::here(run)
        || wax_maze::walled(run)
        || belfry_shoot::walled(run)
        || powder_room::walled(run)
        || book_hop::walled(run)
        || run.room().cols > crate::rules::layout::MAX_COLS
        || run.room().rows > crate::rules::layout::MAX_ROWS
    {
        0
    } else {
        run.tick / TORCH_TICKS
    }
}

const TORCH_TICKS: u64 = 6;

pub(super) fn torch_tick(run: &Run) -> u64 {
    torch_step(run) * TORCH_TICKS
}

pub(super) fn home_print(run: &Run) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    run.home.levels.hash(&mut h);
    let lodge = &run.home.squires_lodge;
    (lodge.recruiting,lodge.hiring,lodge.training,lodge.delves,lodge.rent_days).hash(&mut h);
    for m in &run.settlement_exhibits {
        (m.room, m.col, m.row).hash(&mut h);
    }
    (run.home.goblins > 0).hash(&mut h);
    crate::rules::barony::print(&run.home.barony).hash(&mut h);
    run.home.goods.print().hash(&mut h);
    run.home.bounties.len().saturating_sub(crate::rules::bounties::PINNED).hash(&mut h);
    run.home.trophies.hash(&mut h);
    run.home.countryside.built.hash(&mut h);
    run.home.countryside.lamps.hash(&mut h);
    (run.home.countryside.harbour == 3).hash(&mut h);
    run.home.residents.hash(&mut h);
    run.kingdom.return_to.as_ref().map(|r| &r.name).hash(&mut h);
    h.finish()
}

pub(super) fn near_wall(room: &Room, x: i32, y: i32) -> i32 {
    let (col, row) = (x.div_euclid(TILE), y.div_euclid(TILE));
    let (ox, oy) = (x.rem_euclid(TILE), y.rem_euclid(TILE));
    let wall = |c: i32, r: i32| room.tile(c, r) == Tile::Wall;
    let mut best = TILE;
    for (gap, c, r) in [(ox + 1, col - 1, row), (TILE - ox, col + 1, row), (oy + 1, col, row - 1), (TILE - oy, col, row + 1)] {
        if gap < best && wall(c, r) {
            best = gap;
        }
    }
    best
}

pub(super) fn scenery(run: &Run) -> Img {
    scenery_with_lights(run).0
}

fn scenery_with_lights(run: &Run) -> (Img, Vec<Light>) {
    let _draw = scenery_draw_lock();
    if atari::here(run) {
        return (atari::playfield(run), Vec::new());
    }
    if let Some(mut image) = laid_plate(run).map(|world| world.picture()) {
        let mut lights = Vec::new();
        room_details(run, &mut image, &mut lights);
        return (image, lights);
    }
    if let Some(image) = scenery_elsewhere(run) {
        let lights = if dungeon_art::supported(run) && !entrance_art::supported(run) {
            dungeon_art::lights(run)
        } else {
            Vec::new()
        };
        return (image, lights);
    }
    let mut bake = DungeonBake::new(run);
    while !bake.step(run) {}
    (bake.cv, bake.lights)
}

pub(super) fn world3d(run: &Run) -> Option<crate::world3d::room::RoomWorld> {
    if !props_meshes_on() && !crate::world3d::enabled() || atari::here(run) {
        return None;
    }
    let room = run.room();
    crate::world3d::room::world(room, run.dungeon.pack, run.dungeon.depth, room_px(room))
}

pub(super) fn laid_plate(run: &Run) -> Option<crate::world3d::room::RoomWorld> {
    if props_meshes_on() { None } else { world3d(run) }
}

pub(super) fn world_stood(run: &Run) -> bool {
    props_meshes_on() && world3d(run).is_some()
}

fn lit_scenery(run: &Run) -> std::sync::Arc<Img> {
    let mut frozen;
    let run = if still_wire(run) {
        frozen = run.clone();
        frozen.tick = 0;
        &frozen
    } else { run };
    let (img, lights) = scenery_with_lights(run);
    let img = std::sync::Arc::new(img);
    remember_lights(&img, lights);
    img
}

type BakedLights = Vec<(std::sync::Weak<Img>, std::sync::Arc<Vec<Light>>)>;

static BAKED_LIGHTS: std::sync::Mutex<BakedLights> = std::sync::Mutex::new(Vec::new());

fn remember_lights(img: &std::sync::Arc<Img>, lights: Vec<Light>) {
    let mut baked = BAKED_LIGHTS.lock().unwrap_or_else(|err| err.into_inner());
    baked.retain(|(drawing, _)| drawing.strong_count() > 0);
    if baked.len() >= 8 {
        baked.remove(0);
    }
    baked.push((std::sync::Arc::downgrade(img), std::sync::Arc::new(lights)));
}

pub(super) fn baked_lights(img: &std::sync::Arc<Img>) -> std::sync::Arc<Vec<Light>> {
    let baked = BAKED_LIGHTS.lock().unwrap_or_else(|err| err.into_inner());
    baked
        .iter()
        .find(|(drawing, _)| drawing.upgrade().is_some_and(|d| std::sync::Arc::ptr_eq(&d, img)))
        .map(|(_, lights)| std::sync::Arc::clone(lights))
        .unwrap_or_default()
}

pub(super) fn drawn_elsewhere(run: &Run) -> bool {
    dungeon_art::supported(run) || run.room().kind == RoomKind::Wanted || run.room().kind == RoomKind::DeepVillage || run.side_on() || run.at_home_now()
}

pub(super) fn fixtures_drawn_here(run: &Run) -> bool {
    !entrance_art::supported(run) && (dungeon_art::supported(run) || !drawn_elsewhere(run))
}

fn scenery_elsewhere(run: &Run) -> Option<Img> {
    if entrance_art::supported(run) { return Some((*entrance_art::scene(run)).clone()); }
    if dungeon_art::supported(run) { return Some(dungeon_art::scenery(run)); }
    if !drawn_elsewhere(run) {
        return None;
    }
    if run.room().kind == RoomKind::Wanted {
        return Some(wanted::scenery(run));
    }
    if run.room().kind == RoomKind::DeepVillage {
        return Some(deep_village::scenery(run));
    }
    if run.side_on() {
        return Some(side::scenery(run));
    }
    if run.at_home_now() {
        let mut image = match run.room().kind {
            RoomKind::RealmHall => kingdom::hall(run),
            RoomKind::Fortune => fortune::scenery(run),
            RoomKind::Yard => yard::scenery(run),
            RoomKind::Trophies => trophies::scenery(run),
            RoomKind::Tavern => tavern::scenery(run),
            RoomKind::Hall | RoomKind::Stockpile | RoomKind::Muster | RoomKind::Workshop | RoomKind::Quarters => {
                home::settlement_scenery(run)
            }
            kind if crate::rules::college::is_room(kind) => college::scenery(run),
            kind if crate::rules::colosseum::is_room(kind) => colosseum::scenery(run),
            RoomKind::Alehouse => alehouse::scenery(run),
            RoomKind::MusterTent => muster_tent::scenery(run),
            kind if crate::rules::squire_rooms::is_room(kind) => squire_rooms::scenery(run),
            RoomKind::Bazaar => bazaar::scenery(run),
            kind if crate::rules::ranch::is_room(kind) => ranch::scenery(run),
            kind if crate::rules::world::bomrekedan::is_level(kind) => bomrekedan::scenery(run),
            kind if kind.in_world() => world::scenery(run),
            _ => home::scenery(run),
        };
        home::research_exhibits(&mut image, run);
        return Some(image);
    }
    None
}

struct DungeonBake {
    cv: Img,
    lights: Vec<Light>,
    stage: u8,
}

impl DungeonBake {
    const STAGES: u8 = 8;

    fn new(run: &Run) -> DungeonBake {
        let (pw, ph) = room_px(run.room());
        DungeonBake { cv: Img::black(pw, ph), lights: Vec::new(), stage: 0 }
    }

    fn step(&mut self, run: &Run) -> bool {
        let (h, rows) = (self.cv.h, run.room().rows as i32);
        match self.stage {
            0 => floor_marks(run, &mut self.cv, 0..h / 2),
            1 => floor_marks(run, &mut self.cv, h / 2..h),
            2 => wall_tiles(run, &mut self.cv, &mut self.lights, 0..rows / 2),
            3 => wall_tiles(run, &mut self.cv, &mut self.lights, rows / 2..rows),
            4 => room_fixtures(run, &mut self.cv, &mut self.lights),
            5 => super::super::material::relief(&mut self.cv),
            6 => light_rows(&mut self.cv, &self.lights, style(run.dungeon.pack).pool, 0.0, &[], 0..h / 2),
            _ => light_rows(&mut self.cv, &self.lights, style(run.dungeon.pack).pool, 0.0, &[], h / 2..h),
        }
        self.stage += 1;
        self.stage >= Self::STAGES
    }
}

pub(super) fn floor_marks(run: &Run, cv: &mut Img, rows: std::ops::Range<i32>) {
    let room = run.room();
    let pack = run.dungeon.pack;
    let t = (run.tick as u32 / 3) as i32;
    let pw = cv.w;
    let still = still_floor(run, pw, cv.h);
    let pools = matches!(pack, Pack::Cavern | Pack::Archive | Pack::Swamp);
    for y in rows.clone() {
        for x in 0..pw {
            let tile = room.tile(x / TILE, y / TILE);
            if tile == Tile::Hazard {
                if !pools && let Some(ch) = hazard_mark(pack, room, x, y, t) {
                    cv.put(x, y, ch);
                }
            } else if tile == Tile::Bog {
            } else if let Some(c) = still.get(x, y) {
                cv.set(x, y, c);
            }
        }
    }
    let (r0, r1) = (rows.start.div_euclid(TILE) - 1, (rows.end - 1).div_euclid(TILE) + 1);
    let any_water = (r0..=r1).any(|r| (0..room.cols as i32).any(|c| room.tile(c, r) == Tile::Hazard));
    if pools && any_water {
        let wet = |x: i32, y: i32| room.tile(x.div_euclid(TILE), y.div_euclid(TILE)) == Tile::Hazard;
        super::super::material::water_field(cv, (0, rows.start, pw, rows.end - rows.start), &wet, 140 + pack as u32, (run.tick / 6) as u32);
    }
    let any_bog = (r0..=r1).any(|r| (0..room.cols as i32).any(|c| room.tile(c, r) == Tile::Bog));
    if any_bog {
        let bog = |x: i32, y: i32| room.tile(x.div_euclid(TILE), y.div_euclid(TILE)) == Tile::Bog;
        swamp_art::bog_field(cv, (0, rows.start, pw, rows.end - rows.start), &bog, (run.tick / 6) as u32);
    }
}

fn still_floor(run: &Run, w: i32, h: i32) -> std::sync::Arc<Img> {
    type Cache = std::sync::Mutex<std::collections::HashMap<(u32, u64, i32, i32), std::sync::Arc<Img>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let room = run.room();
    let pack = run.dungeon.pack;
    let key = (pack as u32, room.fingerprint(), w, h);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let mut layer = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let ink = match room.tile(x / TILE, y / TILE) {
                Tile::Floor => super::super::material::skirting(pack as u32, x, y, near_wall(room, x, y)).or_else(|| floor_mark(pack, x, y)),
                Tile::Door => floor_mark(pack, x, y),
                _ => None,
            };
            if let Some(ch) = ink {
                layer.put(x, y, ch);
            }
        }
    }
    let layer = std::sync::Arc::new(layer);
    if let Ok(mut c) = cache.lock() {
        if c.len() > 96 {
            c.clear();
        }
        let most = (crate::rules::layout::MAX_COLS * crate::rules::layout::MAX_ROWS) as i32 * TILE * TILE;
        if w * h > most {
            c.retain(|k, _| k.2 * k.3 <= most);
        }
        c.insert(key, std::sync::Arc::clone(&layer));
    }
    layer
}

fn wall_tiles(run: &Run, cv: &mut Img, lights: &mut Vec<Light>, rows: std::ops::Range<i32>) {
    let room = run.room();
    let pack = run.dungeon.pack;
    let st = style(pack);
    let tick = run.tick as u32;
    let fighting = run.phase == Phase::Fighting;
    let tiles = Tiles::get();
    let maze = wax_maze::walled(run);
    let belfry = belfry_shoot::walled(run);
    let store = powder_room::walled(run);
    for row in rows {
        for col in 0..room.cols as i32 {
            let (x, y) = (col * TILE, row * TILE);
            let v = hash(col, row, 101 + run.at as u32);
            match room.tile(col, row) {
                Tile::Wall if open::deep_rock(room, col, row) => {}
                Tile::Wall if corner(room, col, row) && room.kind != RoomKind::Depths => {
                    let im = sprites::brazier(fighting, tick / 4, v);
                    cv.stamp(&im, x + (TILE - im.w) / 2, y + TILE - im.h);
                    let s = if fighting { 0.55 } else { 0.28 };
                    let r = if fighting { 84.0 } else { 52.0 };
                    lights.push(fire(x + TILE / 2, y + 4, r, s));
                }
                Tile::Wall | Tile::Block | Tile::Floor if maze => wax_maze::walls::lay(cv, room, col, row),
                Tile::Wall | Tile::Block | Tile::Floor | Tile::Door | Tile::Stairs if belfry => belfry_shoot::walls::lay(cv, room, col, row),
                Tile::Wall | Tile::Block | Tile::Floor | Tile::Door | Tile::Stairs if store => powder_room::walls::lay(cv, room, col, row),
                Tile::Wall => {
                    let floor = |c: i32, r: i32| matches!(room.tile(c, r), Tile::Floor | Tile::Door | Tile::Stairs | Tile::Hazard);
                    let open = [floor(col, row - 1), floor(col + 1, row), floor(col, row + 1), floor(col - 1, row)];
                    let (ramp, rough): (&[char], bool) = match pack {
                        Pack::Crypt => (super::super::material::ramp::SLATE, false),
                        Pack::Archive => (super::super::material::ramp::STONE, false),
                        Pack::Cavern => (&['n', 'b', 'I', 'B', 'P', 'r', 'o'], true),
                        Pack::Fungal => (&['n', 'D', 'F', 'E', 'e', 'l', 'm'], true),
                        Pack::Hellforge | Pack::Unknown => (&['k', 'K', 'X', 'g', 'j', 'G', 'J'], pack == Pack::Unknown),
                        Pack::Swamp => (swamp_art::WALL_RAMP, true),
                    };
                    super::super::material::wall_tile(cv, (x, y), TILE, open, ramp, rough, 300 + pack as u32);
                }
                Tile::Block if room.kind == RoomKind::Forge => {}
                Tile::Block if room.kind.nook() => {}
                Tile::Block if room.kind == RoomKind::DwarfHall => {}
                Tile::Block if pack == Pack::Crypt => cv.stamp(&sprites::tomb(v), x, y),
                Tile::Block if pack == Pack::Archive => cv.stamp(&sprites::shelf(v), x, y),
                Tile::Block if pack == Pack::Fungal => cv.stamp(&sprites::mushroom(v), x, y),
                Tile::Block if pack == Pack::Swamp => swamp_art::reeds(cv, x, y, v),
                Tile::Block if room.kind == RoomKind::Threshold && row <= 3 => {}
                Tile::Block if pack == Pack::Unknown => {
                    cv.stamp(&sprites::eye_pillar(v, tick / 6), x, y)
                }
                Tile::Block => cv.stamp(tiles.rock(st.rock, v), x, y),
                Tile::Hazard if pack == Pack::Hellforge && (col + row) % 2 == 0 => {
                    lights.push(fire(x + TILE / 2, y + TILE / 2, 26.0, 0.14));
                }
                _ => {}
            }
        }
    }
}

pub(super) fn room_fixtures(run: &Run, cv: &mut Img, lights: &mut Vec<Light>) {
    let room = run.room();
    let tick = run.tick as u32;
    let barred = run.barred() || run.open_guarded();
    open::fixtures(run, cv, lights);
    for (side, &(col, row)) in doors(room).iter().enumerate() {
        if !room.doors[side] {
            continue;
        }
        let across = side % 2 == 0;
        let (x, y) = (col * TILE, row * TILE);
        let flank = if across {
            [(x - TILE / 2, y), (x + 2 * TILE + TILE / 2, y)]
        } else {
            [(x, y - TILE / 2), (x, y + 2 * TILE + TILE / 2)]
        };
        for (k, (fx, fy)) in flank.into_iter().enumerate() {
            let im = torch(tick / 4, (side * 2 + k) as u32);
            let (tx, ty) = if across {
                (fx - 2, fy + 3)
            } else {
                (fx + TILE / 2 - 2, fy - 4)
            };
            cv.stamp(&im, tx, ty);
            lights.push(fire(tx + 2, ty + 1, 40.0, 0.3));
        }
        if barred {
            cv.stamp(&sprites::portcullis(across), x, y);
        }
        if run.dungeon.neighbour(run.at, side).is_some_and(|i| {
            matches!(
                run.dungeon.rooms[i].kind,
                RoomKind::Ledge | RoomKind::Wanted
            )
        }) {
            let banner = side::banner();
            let (bx, by) = match side {
                0 => (x + TILE - banner.w / 2, 1),
                2 => (x + TILE - banner.w / 2, y + TILE - banner.h - 1),
                1 => (x + TILE - banner.w - 1, y + TILE - banner.h / 2),
                _ => (x + 1, y + TILE - banner.h / 2),
            };
            cv.stamp(&banner, bx, by);
        }
    }
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            let first = room.tile(col, row) == Tile::Stairs
                && room.tile(col - 1, row) != Tile::Stairs
                && room.tile(col, row - 1) != Tile::Stairs;
            if first
                && crate::rules::caves::floor_mouth(run.room().kind, col, row).is_none()
                && crate::rules::caves::yard_mouth(run.room().kind, col, row).is_none()
            {
                cv.stamp(&sprites::stairs(!barred), col * TILE, row * TILE);
            }
        }
    }
    if run.dungeon.depth == 1 && run.room().kind == crate::rules::caves::FLOOR_MOUTH.room {
        let (x, y, _, _) = crate::rules::caves::FLOOR_MOUTH.at;
        super::super::cave_seat::stamp_mouth(cv, x * TILE + (TILE - 32) / 2, y * TILE - 9, 1);
    }
    super::super::cave_seat::mouth_line(cv, run);
    room_details(run, cv, lights);
}

pub(super) fn room_details(run: &Run, cv: &mut Img, lights: &mut Vec<Light>) {
    let room = run.room();
    let tick = run.tick as u32;
    if room.kind == crate::rules::RoomKind::Sanctuary {
        let altar = sprites::dais(tick, run.wishing);
        let (ax, ay) = room.altar();
        let (x, y) = at(ax, ay);
        cv.stamp(&altar, x - altar.w / 2, y - altar.h / 2);
        lights.push(fire(x, y - 6, if run.wishing { 84.0 } else { 64.0 }, 0.55));
    }
    if let Some((ax, ay)) = run.wish_altar() {
        let (x, y) = at(ax, ay);
        lights.push(fire(x, y - 16, 58.0, 0.5));
    }
    if room.kind == RoomKind::Threshold {
        shoggoth(cv, room, tick, lights);
    }
    if room.kind == RoomKind::Forge {
        barony::forge(cv, run, lights, tick);
    }
    if room.kind.nook() {
        nooks::scenery(cv, run, lights);
    }
    if room.kind == RoomKind::DwarfHall {
        super::reclaim::scenery(cv, run, lights);
    }
    if let Some(c) = room.chest {
        let im = chest(!c.open);
        let (x, y) = at(c.x, c.y);
        if !props_meshes_on() {
            cv.stamp(&im, x - im.w / 2, y - im.h / 2);
        }
    }
    for trap in &run.traps {
        match *trap {
            Trap::Spikes { col, row, .. } => {
                let plate = match super::demonic::style(run.dungeon.pack) {
                    Some(style) => super::demonic::spike_plate(style),
                    None => sprites::spike_plate(),
                };
                cv.stamp(&plate, col * TILE, row * TILE)
            }
            Trap::Vent { side, lane, .. } => {
                let (x, y) = vent_at(run.room(), side, lane);
                cv.stamp(&sprites::vent(0), x, y);
            }
            Trap::Rockfall => {}
        }
    }
    if let Some((host, cx, cy)) = run.dungeon.crack()
        && host == run.at
    {
        let side = run.dungeon.secret.map_or(0, |s| s.side);
        secrets::crack(cv, (cx, cy), side);
    }
    if room.kind == RoomKind::Secret {
        let (x, y) = at(room.width() / 2.0, room.height() / 2.0);
        lights.push(fire(x, y, 70.0, 0.45));
    }
}

pub(super) fn shoggoth(cv: &mut Img, room: &Room, tick: u32, lights: &mut Vec<Light>) {
    let pw = room.cols as i32 * TILE;
    let heave = (tick as f32 * 0.05).sin();
    for x in 0..pw {
        let edge = 4.0 * TILE as f32
            + (x as f32 * 0.045).sin() * 6.0
            + (x as f32 * 0.11 + heave).sin() * 3.0;
        for y in 0..edge as i32 {
            let n = vnoise(x as f32 / 9.0, y as f32 / 7.0 + heave * 0.2, 501);
            let ink = if n > 0.66 {
                'g'
            } else if n > 0.45 {
                'X'
            } else if n > 0.3 {
                'K'
            } else {
                'k'
            };
            cv.set(x, y, super::super::ink::ink(ink).unwrap_or(BLACK));
        }
        if hash(x / 7, 1, 502).is_multiple_of(5) && x % 7 == 3 {
            let len = 8 + (hash(x, 2, 503) % 14) as i32;
            for k in 0..len {
                let sway = ((k as f32 * 0.4 + x as f32 + heave * 3.0).sin() * 1.5) as i32;
                cv.put(
                    x + sway,
                    edge as i32 + k,
                    if k % 3 == 0 { 'g' } else { 'X' },
                );
            }
        }
    }
    for i in 0..30 {
        let ex = (hash(i, 7, 504) % pw.max(1) as u32) as i32;
        let ey = 10 + (hash(i, 8, 505) % 40) as i32;
        let r = 1 + (hash(i, 9, 506) % 4) as i32;
        let closed = hash(i, (tick / 6) as i32, 507).is_multiple_of(11);
        if closed {
            cv.line(ex - r, ey, ex + r, ey, 'g');
            continue;
        }
        for dy in -r..=r {
            for dx in -r - 1..=r + 1 {
                if dx * dx * 2 / 3 + dy * dy > r * r {
                    continue;
                }
                let ink = if dx * dx + dy * dy <= (r / 2).max(0) * (r / 2).max(0) {
                    'K'
                } else if dx * dx + dy * dy <= r * r * 2 / 3 {
                    if r > 2 { '3' } else { '2' }
                } else {
                    'h'
                };
                cv.put(ex + dx, ey + dy, ink);
            }
        }
        if r >= 3 {
            lights.push(Light {
                x: ex as f32,
                y: ey as f32,
                r: 18.0,
                s: 0.12,
                fire: false,
            });
        }
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn wire_torch_steps_keep_the_plate_but_terminal_steps_still_advance() {
        let _pin = pin_scene_cache();
        let mut run = Run::new(3, 1, Some("Wire"));
        while run.dungeon.depth < 3 { run.descend_for_test(); }
        let room = run.dungeon.rooms.iter()
            .position(|r| r.kind == RoomKind::Fight).unwrap();
        run.enter_for_test(room);
        let kept;
        {
            let _wire = wire_scenery(true);
            let first = scenery_cached_exact(&run);
            run.tick += 12;
            assert_eq!(torch_step(&run), 0, "wire lights animate without a new bake");
            kept = scenery_cached_exact(&run);
            assert!(std::sync::Arc::ptr_eq(&first, &kept), "the wire keeps the actual plate, not just equal pixels");
            {
                let _terminal = wire_scenery(false);
                assert_eq!(torch_step(&run), run.tick / 6);
            }
            assert_eq!(torch_step(&run), 0, "a nested drawer restores the wire mode");
        }
        assert_eq!(torch_step(&run), run.tick / 6, "the caller's mode is restored");
        let terminal = scenery_cached_exact(&run);
        assert!(!std::sync::Arc::ptr_eq(&kept, &terminal), "wire and terminal bakes have distinct keys");
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod live_flip_tests {
    use super::*;

    #[test]
    fn a_fight_flip_relights_beside_the_painter_and_a_room_change_draws_now() {
        *live_slot().lock().unwrap_or_else(|err| err.into_inner()) = None;
        let mut run = Run::new(3, 1, Some("Flip"));
        while run.dungeon.depth < 3 {
            run.descend_for_test();
        }
        let fight = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight)
            .expect("a fight room on the floor");
        run.enter_for_test(fight);

        let first = scenery_live(&run);

        run.traps.push(crate::rules::Trap::Spikes { col: 4, row: 4, phase: 0 });
        let shown = scenery_live(&run);
        assert!(
            std::sync::Arc::ptr_eq(&first, &shown),
            "a fight flip must show the last art, not relight on the paint thread"
        );

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let landed = loop {
            std::thread::sleep(std::time::Duration::from_millis(5));
            let _ = scenery_live(&run);
            let slot = live_slot();
            let guard = slot.lock().unwrap_or_else(|err| err.into_inner());
            let live = guard.as_ref().expect("the live slot was emptied mid-test");
            if !std::sync::Arc::ptr_eq(&live.img, &first) {
                assert_eq!(
                    live.steady,
                    steady_key(&run),
                    "the baker must adopt the flipped key, not the stale one"
                );
                assert_ne!(live.baking, Some(live.generation), "the job was left marked in flight");
                break std::sync::Arc::clone(&live.img);
            }
            assert!(std::time::Instant::now() < deadline, "the baker never landed the relit room");
        };

        let again = scenery_live(&run);
        assert!(std::sync::Arc::ptr_eq(&landed, &again), "a landed flip must serve the kept art");

        let elsewhere = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.fingerprint() != run.room().fingerprint())
            .expect("another room on the floor");
        run.enter_for_test(elsewhere);
        let moved = scenery_live(&run);
        assert!(
            !std::sync::Arc::ptr_eq(&landed, &moved),
            "a room change must draw the new room now, not show the old art"
        );
    }
}
