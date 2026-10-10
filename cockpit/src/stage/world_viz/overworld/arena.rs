//! The dungeon delve drawn in the overworld's own language.
//!
//! Black paper, sixteen-pixel tiles, the kit's shaded rocks for walls and
//! sparse ground marks for floors, dusk with firelight pools — and the
//! realm's own knight in the middle of it. The Crypt is slate, the Mines are
//! the overworld's timber-brown rock, the Dragon Keep charcoal over lava.
//!
//! A room is painted once at native size (16 px per tile, 8 per arena unit,
//! 384x224) and then fitted to the frame: whole-pixel enlargement when it
//! fits, otherwise a reduction where live state and the brightest mark of
//! each footprint win, so figures and shots survive on black paper.
//!
//! Scenery uses the realm's material banks and dims at dusk. Figures, shots
//! and loot are drawn after the light at full ink; their live parts are
//! signal inks.
//!
//! Contract: pure and deterministic — the same run and size paint the same
//! pixels.

mod barony;
mod bounties;
mod cards;
mod fortune;
mod hireling;
mod home;
mod joust;
mod lessons;
mod rescues;
mod runes;
mod secrets;
mod side;
pub(crate) mod sprites;
mod tavern;
mod tide;
mod trophies;
mod ults;
mod void;
mod world;
mod yard;

pub(crate) use cards::card_art;

/// The Delve's folk, for the loop's crawl to meet in its halls.
pub(crate) fn crawl_figure(who: &str) -> Option<Img> {
    let rows: &[&str] = match who {
        "merlin" => &home::MERLIN,
        "tobbin" => &home::TOBBIN,
        "blaise" => &home::BLAISE,
        "wren" => &home::WREN,
        "kay" => &trophies::KAY,
        "ector" => &lessons::ECTOR,
        "dinadan" => &tavern::DINADAN,
        "beaumains" => &hireling::BEAUMAINS,
        "maud" | "mabel" | "anselm" | "pip" => return Some(rescues::sprite_of(who)),
        _ => return None,
    };
    Some(Img::from_rows(rows))
}

/// A monster of the Delve, for the crawl.
pub(crate) fn crawl_enemy(kind: crate::drive::together_shooter::EnemyKind, tick: u32) -> Img {
    sprites::enemy(kind, tick)
}

/// A standing thing of the Delve's halls, for the crawl: a tomb, a rock, a
/// mushroom, an eye-pillar, a fire.
pub(crate) fn crawl_prop(what: &str, tick: u32) -> Img {
    match what {
        "tomb" => sprites::tomb(1),
        "rock" => sprites::rock(),
        "mushroom" => sprites::mushroom(tick / 40),
        "eye_pillar" => sprites::eye_pillar(1, tick / 3),
        _ => sprites::brazier(true, tick, 7),
    }
}
/// The crawl uses the same compact room signs as the playable arena.
pub(crate) fn crawl_sign(label: &str) -> Img {
    super::kit::district_sign(label)
}

pub(crate) use sprites::knight_in;

use std::collections::HashMap;
use std::sync::OnceLock;

use super::ink::{BLACK, Img, PALETTE, Rgb, bayer, hash, is_signal, rgb, vnoise};
use super::kit::{RockKind, Tiles, chest, district_sign, torch};
use super::light::{DUSK, Light, STEPS, light_at, step_down, warm};
use crate::drive::together_shooter::{
    BOMB_REARM, COLS, EnemyKind, HEIGHT, Pack, Phase, ROWS, Room, RoomKind, Run, TELEGRAPH,
    TILE_UNITS, Tile, Trap, VIGIL_REACH, WIDTH, ledge::FOOT,
};

/// Pixels per tile: the overworld's.
const TILE: i32 = 16;
/// Pixels per arena unit.
const UNIT: f32 = TILE as f32 / TILE_UNITS;
/// The native view: one screen of room. A great hall is drawn whole and the
/// camera shows this much of it.
pub(crate) const NATIVE_W: i32 = COLS as i32 * TILE;
pub(crate) const NATIVE_H: i32 = ROWS as i32 * TILE;

/// An arena point in native pixels.
fn at(ux: f32, uy: f32) -> (i32, i32) {
    ((ux * UNIT).floor() as i32, (uy * UNIT).floor() as i32)
}

/// Where the native room lands in a `w`x`h` frame: native pixel `n` goes to
/// `o + n * num / den`.
#[derive(Clone, Copy, Debug)]
struct Field {
    ox: i32,
    oy: i32,
    num: i32,
    den: i32,
}

impl Field {
    fn fit(w: i32, h: i32) -> Field {
        let (num, den) = if w >= NATIVE_W && h >= NATIVE_H {
            ((w / NATIVE_W).min(h / NATIVE_H), 1)
        } else if w * NATIVE_H <= h * NATIVE_W {
            (w, NATIVE_W)
        } else {
            (h, NATIVE_H)
        };
        let (tw, th) = (NATIVE_W * num / den, NATIVE_H * num / den);
        Field {
            ox: (w - tw) / 2,
            oy: (h - th) / 2,
            num,
            den,
        }
    }

    fn map(&self, nx: i32, ny: i32) -> (i32, i32) {
        (
            self.ox + nx * self.num / self.den,
            self.oy + ny * self.num / self.den,
        )
    }

    /// An arena point in frame pixels.
    fn px(&self, ux: f32, uy: f32) -> (i32, i32) {
        let (nx, ny) = at(ux, uy);
        self.map(nx, ny)
    }

    /// Fit `scenery` with `figures` over it into `out`.
    fn blit(&self, scenery: &Img, figures: &Img, out: &mut Img) {
        if self.den == 1 {
            let k = self.num;
            // The guest frame is native size: one tight pass, no bounds checks.
            if k == 1
                && self.ox == 0
                && self.oy == 0
                && out.w == scenery.w
                && out.h == scenery.h
                && figures.w == scenery.w
                && figures.h == scenery.h
            {
                Img::compose_1x(figures, scenery, out);
                return;
            }
            for y in 0..scenery.h * k {
                for x in 0..scenery.w * k {
                    if let Some(c) = figures.get(x / k, y / k).or(scenery.get(x / k, y / k)) {
                        out.set(self.ox + x, self.oy + y, c);
                    }
                }
            }
            return;
        }
        // Reduction. A figure, shot or loot mark always wins its footprint
        // (live state first, then the brightest), so black paper never
        // swallows one. Scenery keeps its brightest mark only where it
        // covers a third of the footprint: walls stay, sparse floor marks
        // fade instead of turning into noise.
        let rank = |c: Rgb| {
            let l = 299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2]);
            if is_signal(c) { l + 1_000_000 } else { l }
        };
        let (tw, th) = (
            NATIVE_W * self.num / self.den + 1,
            NATIVE_H * self.num / self.den + 1,
        );
        let cells = (tw * th) as usize;
        let (mut total, mut inked) = (vec![0u16; cells], vec![0u16; cells]);
        let mut best_scene: Vec<Option<Rgb>> = vec![None; cells];
        let mut best_figure: Vec<Option<Rgb>> = vec![None; cells];
        for y in 0..scenery.h {
            for x in 0..scenery.w {
                let (ox, oy) = self.map(x, y);
                let i = ((oy - self.oy) * tw + (ox - self.ox)) as usize;
                total[i] += 1;
                if let Some(c) = figures.get(x, y)
                    && best_figure[i].is_none_or(|e| rank(c) > rank(e))
                {
                    best_figure[i] = Some(c);
                }
                if let Some(c) = scenery.get(x, y).filter(|&c| c != BLACK) {
                    inked[i] += 1;
                    if best_scene[i].is_none_or(|e| rank(c) > rank(e)) {
                        best_scene[i] = Some(c);
                    }
                }
            }
        }
        for i in 0..cells {
            let c = best_figure[i].or(best_scene[i].filter(|_| inked[i] * 3 >= total[i]));
            if let Some(c) = c {
                out.set(self.ox + i as i32 % tw, self.oy + i as i32 / tw, c);
            }
        }
    }
}

/// The floor's place in the realm.
struct Style {
    rock: RockKind,
    /// Bare-paper ink under a light: `(cool, firelit)`.
    pool: (char, char),
}

fn style(pack: Pack) -> Style {
    match pack {
        Pack::Crypt => Style {
            rock: RockKind::Slate,
            pool: ('x', 'I'),
        },
        Pack::Cavern => Style {
            rock: RockKind::Ore,
            pool: ('b', 'I'),
        },
        Pack::Hellforge => Style {
            rock: RockKind::Char,
            pool: ('X', 'p'),
        },
        Pack::Archive => Style {
            rock: RockKind::Slate,
            pool: ('q', 'I'),
        },
        Pack::Fungal => Style {
            rock: RockKind::Ore,
            pool: ('e', 'N'),
        },
        Pack::Unknown => Style {
            rock: RockKind::Char,
            pool: ('0', 'q'),
        },
    }
}

// ─── ground marks ────────────────────────────────────────────────────────────

/// Crypt flags: a worn joint here and there, slate specks, a bone chip.
fn flags(x: i32, y: i32) -> Option<char> {
    let (lx, ly) = (x.rem_euclid(TILE), y.rem_euclid(TILE));
    let stone = hash(x.div_euclid(TILE), y.div_euclid(TILE), 81);
    let (a, b) = ((stone >> 8) as i32 % 6, (stone >> 12) as i32 % 6);
    if (ly == 0 && lx > 2 + a && lx < 7 + a && stone.is_multiple_of(5))
        || (lx == 0 && ly > 3 + b && ly < 8 + b && (stone >> 4).is_multiple_of(7))
    {
        return Some('x');
    }
    match hash(x, y, 83) % 150 {
        0 | 1 => Some('s'),
        2 => Some('S'),
        3 if hash(x / 3, y / 3, 84).is_multiple_of(5) => Some('g'),
        _ => None,
    }
}

/// The Mines' pebbles, strewn like the roads up to the mine mouth.
fn pebbles(x: i32, y: i32) -> Option<char> {
    if !hash(x / 6, y / 5, 86).is_multiple_of(3) {
        return None;
    }
    match hash(x, y, 85) % 64 {
        0..=3 => Some('P'),
        4 | 5 => Some('r'),
        6 => Some('o'),
        7 | 8 => Some('b'),
        _ => None,
    }
}

/// The Keep's ash, with an ember cooling in it now and then.
fn ash(x: i32, y: i32) -> Option<char> {
    match hash(x, y, 87) % 36 {
        0 => Some('X'),
        1 => Some('g'),
        2 if x % 5 == 0 => Some('K'),
        3 if hash(x / 4, y / 4, 88).is_multiple_of(9) => Some('p'),
        _ => None,
    }
}

/// The Archive's floor: worn flags, and pages that drifted off the shelves.
fn pages(x: i32, y: i32) -> Option<char> {
    let h = hash(x / 3, y / 2, 91);
    if h.is_multiple_of(97) && (x % 3 != 2) {
        return Some(if (y / 2) % 2 == 0 { 'T' } else { 't' });
    }
    flags(x, y).filter(|_| hash(x, y, 92).is_multiple_of(2))
}

/// The Fungal Deep's floor: moss in tufts, and spores that glow.
fn moss(x: i32, y: i32) -> Option<char> {
    if !hash(x / 5, y / 4, 93).is_multiple_of(3) {
        return None;
    }
    match hash(x, y, 94) % 48 {
        0..=3 => Some('l'),
        4 | 5 => Some('m'),
        6 => Some('E'),
        7 if hash(x / 7, y / 7, 95).is_multiple_of(4) => Some('Y'),
        _ => None,
    }
}

/// The Unknown's floor: hardly a floor at all — a lattice of faint light,
/// as if the world were still being worked out.
fn lattice(x: i32, y: i32) -> Option<char> {
    let on = (x.rem_euclid(TILE) == 0 && y.rem_euclid(4) == 0)
        || (y.rem_euclid(TILE) == 0 && x.rem_euclid(4) == 0);
    if on && hash(x / TILE, y / TILE, 96).is_multiple_of(3) {
        return Some('0');
    }
    (hash(x, y, 97).is_multiple_of(400)).then_some('2')
}

fn floor_mark(pack: Pack, x: i32, y: i32) -> Option<char> {
    match pack {
        Pack::Crypt => flags(x, y),
        Pack::Cavern => pebbles(x, y),
        Pack::Hellforge => ash(x, y),
        Pack::Archive => pages(x, y),
        Pack::Fungal => moss(x, y),
        Pack::Unknown => lattice(x, y),
    }
}

/// Whether native pixel `(x, y)` lies over a hazard tile.
fn hazard_at(room: &Room, x: i32, y: i32) -> bool {
    room.tile(x.div_euclid(TILE), y.div_euclid(TILE)) == Tile::Hazard
}

/// Pit, pool or lava at a native pixel, with the overworld's marks.
fn hazard_mark(pack: Pack, room: &Room, x: i32, y: i32, t: i32) -> Option<char> {
    let mut shore = i32::MAX;
    for k in 1..=3 {
        if !hazard_at(room, x - k, y)
            || !hazard_at(room, x + k, y)
            || !hazard_at(room, x, y - k)
            || !hazard_at(room, x, y + k)
        {
            shore = k;
            break;
        }
    }
    match pack {
        // A pit: a broken stone lip, then nothing.
        Pack::Crypt => match shore {
            1 => (!hash(x, y, 91).is_multiple_of(4)).then_some('S'),
            2 => (hash(x, y, 92).is_multiple_of(3)).then_some('x'),
            _ => None,
        },
        // Still water: a pale shore and wave dashes drifting east.
        Pack::Cavern => {
            match shore.saturating_sub((hash(x / 3, y / 3, 5) % 2) as i32) {
                i32::MIN..=1 => return Some('Q'),
                2 => return ((x + y) % 2 == 0).then_some('q'),
                _ => {}
            }
            let dx = x - t / 2;
            let h = hash(dx.div_euclid(9), y.div_euclid(6), 9);
            let (ox, oy) = (((h >> 4) % 5) as i32, ((h >> 8) % 5) as i32);
            let (lx, ly) = (dx.rem_euclid(9), y.rem_euclid(6));
            (h.is_multiple_of(2) && ly == oy && lx >= ox && lx < ox + 4)
                .then_some(if lx == ox + 1 { 'Q' } else { 'q' })
        }
        // Spore pools: a glowing green shimmer that drifts.
        Pack::Fungal => {
            if shore == 1 {
                return Some('E');
            }
            let glow = vnoise((x + t / 3) as f32 / 5.0, y as f32 / 4.0, 452);
            (glow > 0.45).then_some(if glow > 0.7 {
                'Y'
            } else if glow > 0.58 {
                'y'
            } else {
                'l'
            })
        }
        // The void: nothing below, now and then a far light.
        Pack::Unknown => {
            if shore == 1 {
                return (!hash(x, y, 98).is_multiple_of(3)).then_some('0');
            }
            (hash(x, y + t / 8, 99).is_multiple_of(300)).then_some('3')
        }
        // The Archive's flood, as the Mines' water.
        Pack::Archive => {
            match shore.saturating_sub((hash(x / 3, y / 3, 5) % 2) as i32) {
                i32::MIN..=1 => return Some('Q'),
                2 => return ((x + y) % 2 == 0).then_some('q'),
                _ => {}
            }
            let dx = x - t / 2;
            let h = hash(dx.div_euclid(9), y.div_euclid(6), 9);
            let (ox, oy) = (((h >> 4) % 5) as i32, ((h >> 8) % 5) as i32);
            let (lx, ly) = (dx.rem_euclid(9), y.rem_euclid(6));
            (h.is_multiple_of(2) && ly == oy && lx >= ox && lx < ox + 4)
                .then_some(if lx == ox + 1 { 'Q' } else { 'q' })
        }
        // Lava: dark crust creeping east, the melt glowing through.
        Pack::Hellforge => {
            if shore == 1 {
                return Some('n');
            }
            let crust = vnoise((x - t / 2) as f32 / 6.0, y as f32 / 3.0, 451);
            Some(if crust > 0.62 {
                'n'
            } else if crust > 0.5 {
                'b'
            } else if crust > 0.38 {
                'B'
            } else if crust > 0.24 {
                'p'
            } else if crust > 0.14 {
                'R'
            } else {
                'o'
            })
        }
    }
}

// ─── the room ────────────────────────────────────────────────────────────────

/// The door lanes of each side, as `(col, row)` of the first of two tiles.
fn doors(room: &Room) -> [(i32, i32); 4] {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    [
        (cols / 2 - 1, 0),
        (cols - 1, rows / 2 - 1),
        (cols / 2 - 1, rows - 1),
        (0, rows / 2 - 1),
    ]
}

fn corner(room: &Room, col: i32, row: i32) -> bool {
    (col == 0 || col == room.cols as i32 - 1) && (row == 0 || row == room.rows as i32 - 1)
}

/// The room's full size in native pixels.
fn room_px(room: &Room) -> (i32, i32) {
    (room.cols as i32 * TILE, room.rows as i32 * TILE)
}

/// The lit scenery changes only with the room, its doors, its chest and its
/// traps — and its torches, which flicker five times a second. Drawing it
/// (dusk light per pixel, over a whole great hall) every tick was what made
/// frames skip, so one drawing is kept and reused until one of those moves.
fn scenery_cached(run: &Run) -> std::sync::Arc<Img> {
    if live_frame() {
        return scenery_live(run);
    }
    scenery_cached_exact(run)
}

/// A live seat's frame. Same pixels as `frame_for`, except a torch flicker
/// may show the previous step until its bake lands off the paint thread.
/// A snapshot still uses `frame_for`, which stays exact.
pub(crate) fn frame_for_live(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    struct Arm;
    impl Drop for Arm {
        fn drop(&mut self) {
            LIVE_FRAME.with(|flag| flag.set(false));
        }
    }
    LIVE_FRAME.with(|flag| flag.set(true));
    let _arm = Arm;
    frame_for(run, w, h, focus)
}

std::thread_local! {
    static LIVE_FRAME: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

fn live_frame() -> bool {
    LIVE_FRAME.with(|flag| flag.get())
}

/// Everything in the scenery key except the torch step.
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
);

fn steady_key(run: &Run) -> SteadyKey {
    (
        run.raid_id,
        run.dungeon.depth,
        run.at,
        run.room().fingerprint() ^ run.dungeon.pack as u64,
        run.barred(),
        run.phase == Phase::Fighting,
        run.room().chest.map(|c| c.open),
        run.traps.len(),
        run.wishing,
        home_print(run),
    )
}

struct LiveScenery {
    generation: u64,
    steady: SteadyKey,
    flicker: u64,
    img: std::sync::Arc<Img>,
    baking: Option<u64>,
}

static LIVE_SCENERY: OnceLock<std::sync::Mutex<Option<LiveScenery>>> = OnceLock::new();
static SCENERY_DRAW: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
static BAKE_TX: OnceLock<std::sync::mpsc::Sender<(u64, SteadyKey, u64, Run)>> = OnceLock::new();

fn scenery_draw_lock() -> std::sync::MutexGuard<'static, ()> {
    SCENERY_DRAW
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn bake_sender() -> &'static std::sync::mpsc::Sender<(u64, SteadyKey, u64, Run)> {
    BAKE_TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<(u64, SteadyKey, u64, Run)>();
        std::thread::Builder::new()
            .name("delve-scenery".into())
            .spawn(move || {
                while let Ok((generation, steady, flicker, run)) = rx.recv() {
                    let img = std::sync::Arc::new(scenery(&run));
                    let Some(slot) = LIVE_SCENERY.get() else {
                        continue;
                    };
                    let Ok(mut guard) = slot.lock() else {
                        continue;
                    };
                    let Some(live) = guard.as_mut() else {
                        continue;
                    };
                    if live.generation == generation && live.steady == steady {
                        live.img = img;
                        live.flicker = flicker;
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

/// Live frames keep the last lit room when only the torch step moved, and
/// bake the new step beside the painter. A room change is still drawn now.
fn scenery_live(run: &Run) -> std::sync::Arc<Img> {
    let steady = steady_key(run);
    let flicker = run.tick / 6;
    let slot = LIVE_SCENERY.get_or_init(|| std::sync::Mutex::new(None));
    let sync_generation = {
        let mut guard = slot.lock().unwrap_or_else(|err| err.into_inner());
        match guard.as_mut() {
            Some(live) if live.steady == steady && live.flicker == flicker => {
                return std::sync::Arc::clone(&live.img);
            }
            Some(live) if live.steady == steady => {
                let stale = std::sync::Arc::clone(&live.img);
                if live.baking != Some(live.generation) {
                    live.baking = Some(live.generation);
                    let job = (live.generation, steady, flicker, run.clone());
                    let _ = bake_sender().send(job);
                }
                return stale;
            }
            Some(live) => {
                live.generation = live.generation.wrapping_add(1);
                live.baking = None;
                live.generation
            }
            None => 1,
        }
    };
    let img = std::sync::Arc::new(scenery(run));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(LiveScenery {
            generation: sync_generation,
            steady,
            flicker,
            img: std::sync::Arc::clone(&img),
            baking: None,
        });
    }
    img
}

fn scenery_cached_exact(run: &Run) -> std::sync::Arc<Img> {
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
    );
    static CACHE: OnceLock<std::sync::Mutex<Option<(Key, std::sync::Arc<Img>)>>> = OnceLock::new();
    let key: Key = (
        run.raid_id,
        run.dungeon.depth,
        run.at,
        run.room().fingerprint() ^ run.dungeon.pack as u64,
        run.barred(),
        run.phase == Phase::Fighting,
        run.tick / 6,
        run.room().chest.map(|c| c.open),
        run.traps.len(),
        run.wishing,
        home_print(run),
    );
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(slot) = cache.lock()
        && let Some((k, img)) = slot.as_ref()
        && *k == key
    {
        return std::sync::Arc::clone(img);
    }
    let img = std::sync::Arc::new(scenery(run));
    if let Ok(mut slot) = cache.lock() {
        *slot = Some((key, std::sync::Arc::clone(&img)));
    }
    img
}

/// What the Undercroft has built, for the kept drawing.
fn home_print(run: &Run) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    run.home.levels.hash(&mut h);
    for m in &run.settlement_exhibits {
        (m.room, m.col, m.row).hash(&mut h);
    }
    // Grubbins' stall opens once a goblin has come back.
    (run.home.goblins > 0).hash(&mut h);
    // The barony: the King's court, the works as built, the forges lit.
    crate::drive::together_shooter::barony::print(&run.home.barony).hash(&mut h);
    run.home.trophies.hash(&mut h);
    // The tavern's bar is kept once Maud is home.
    run.home.residents.hash(&mut h);
    h.finish()
}

/// Paint the room's scenery and light it for dusk.
fn scenery(run: &Run) -> Img {
    let _draw = scenery_draw_lock();
    if run.side_on() {
        return side::scenery(run);
    }
    if run.at_home_now() {
        let mut image = match run.room().kind {
            RoomKind::Fortune => fortune::scenery(run),
            RoomKind::Yard => yard::scenery(run),
            RoomKind::Trophies => trophies::scenery(run),
            RoomKind::Tavern => tavern::scenery(run),
            RoomKind::Hall | RoomKind::Stockpile | RoomKind::Workshop | RoomKind::Quarters => {
                home::settlement_scenery(run)
            }
            kind if kind.in_world() => world::scenery(run),
            _ => home::scenery(run),
        };
        home::research_exhibits(&mut image, run);
        return image;
    }
    let room = run.room();
    let pack = run.dungeon.pack;
    let st = style(pack);
    let tick = run.tick as u32;
    let t = (tick / 3) as i32;
    let fighting = run.phase == Phase::Fighting;
    let barred = run.barred();
    let tiles = Tiles::get();
    let (pw, ph) = room_px(room);
    let mut cv = Img::black(pw, ph);
    let mut lights = Vec::new();

    for y in 0..ph {
        for x in 0..pw {
            let ink = match room.tile(x / TILE, y / TILE) {
                Tile::Floor | Tile::Door => floor_mark(pack, x, y),
                Tile::Hazard => hazard_mark(pack, room, x, y, t),
                _ => None,
            };
            if let Some(ch) = ink {
                cv.put(x, y, ch);
            }
        }
    }
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            let (x, y) = (col * TILE, row * TILE);
            let v = hash(col, row, 101 + run.at as u32);
            match room.tile(col, row) {
                Tile::Wall if corner(room, col, row) => {
                    let im = sprites::brazier(fighting, tick / 4, v);
                    cv.stamp(&im, x + (TILE - im.w) / 2, y + TILE - im.h);
                    let s = if fighting { 0.55 } else { 0.28 };
                    let r = if fighting { 84.0 } else { 52.0 };
                    lights.push(fire(x + TILE / 2, y + 4, r, s));
                }
                Tile::Wall => cv.stamp(tiles.rock(st.rock, v), x, y),
                // A forge-hall's furnace and anvils are drawn whole below.
                Tile::Block if room.kind == RoomKind::Forge => {}
                Tile::Block if pack == Pack::Crypt => cv.stamp(&sprites::tomb(v), x, y),
                Tile::Block if pack == Pack::Archive => cv.stamp(&sprites::shelf(v), x, y),
                Tile::Block if pack == Pack::Fungal => cv.stamp(&sprites::mushroom(v), x, y),
                // The Shoggoth's body fills its threshold's north: drawn whole below.
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
    // Torches flank every doorway; bars close it while the fight lasts.
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
        // A doorway that leads side-on hangs a banner over it.
        if run
            .dungeon
            .neighbour(run.at, side)
            .is_some_and(|i| run.dungeon.rooms[i].kind == RoomKind::Ledge)
        {
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
            if first {
                cv.stamp(&sprites::stairs(!barred), col * TILE, row * TILE);
            }
        }
    }
    if let Some((px, py)) = room.privy() {
        let booth = sprites::privy();
        let (x, y) = at(px, py);
        cv.stamp(&booth, x - booth.w / 2, y - booth.h);
    }
    if room.kind == crate::drive::together_shooter::RoomKind::Sanctuary {
        // The reforge altar: Tobbin's travelling anvil on a stone plinth,
        // candles either side, its coals always lit.
        let altar = sprites::dais(tick, run.wishing);
        let (ax, ay) = room.altar();
        let (x, y) = at(ax, ay);
        cv.stamp(&altar, x - altar.w / 2, y - altar.h / 2);
        lights.push(fire(x, y - 6, if run.wishing { 84.0 } else { 64.0 }, 0.55));
    }
    if room.kind == RoomKind::Threshold {
        shoggoth(&mut cv, room, tick, &mut lights);
    }
    if room.kind == RoomKind::Forge {
        barony::forge(&mut cv, run, &mut lights, tick);
    }
    if let Some(c) = room.chest {
        let im = chest(!c.open);
        let (x, y) = at(c.x, c.y);
        cv.stamp(&im, x - im.w / 2, y - im.h / 2);
    }
    // Traps at rest are part of the room: plates in the floor, grilles in the walls.
    for trap in &run.traps {
        match *trap {
            Trap::Spikes { col, row, .. } => {
                cv.stamp(&sprites::spike_plate(), col * TILE, row * TILE)
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
        secrets::crack(&mut cv, (cx, cy), side);
    }
    if room.kind == RoomKind::Secret {
        // The vault's gold catches what light there is.
        let (x, y) = at(room.width() / 2.0, room.height() / 2.0);
        lights.push(fire(x, y, 70.0, 0.45));
    }
    dusk(&mut cv, &lights, st.pool);
    cv
}

/// The Shoggoth at its threshold: the great Unknown, not an enemy — only
/// what the party has not met yet. A mass along the north wall, heaving,
/// tendrils trailing, and everywhere eyes: curious, teal, now and then
/// blinking.
fn shoggoth(cv: &mut Img, room: &Room, tick: u32, lights: &mut Vec<Light>) {
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
            cv.set(x, y, super::ink::ink(ink).unwrap_or(BLACK));
        }
        // Tendrils, here and there, trailing onto the floor.
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
    // Its eyes: many, of every size, curious.
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

fn fire(x: i32, y: i32, r: f32, s: f32) -> Light {
    Light {
        x: x as f32,
        y: y as f32,
        r,
        s,
        fire: true,
    }
}

/// The overworld's dusk on a room: scenery steps down its own bank, fire
/// warms what it touches and leaves dithered pools on bare paper. Signal
/// inks never dim.
fn dusk(cv: &mut Img, lights: &[Light], (cool, lit): (char, char)) {
    // The dapple and the firelit inks never change: work them out once.
    static DAPPLE: OnceLock<Vec<f32>> = OnceLock::new();
    static WARM: OnceLock<HashMap<Rgb, Rgb>> = OnceLock::new();
    let dapple = DAPPLE.get_or_init(|| {
        (0..NATIVE_W * NATIVE_H)
            .map(|i| {
                let (x, y) = ((i % NATIVE_W) as f32 + 0.5, (i / NATIVE_W) as f32 + 0.5);
                (vnoise(x / 48.0, y / 48.0, 77) - 0.5) * 0.14
            })
            .collect()
    });
    let warmed = WARM.get_or_init(|| PALETTE.iter().map(|&v| (rgb(v), warm(rgb(v)))).collect());
    for y in 0..cv.h {
        for x in 0..cv.w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (local, fire) = light_at(lights, px, py);
            // The dapple repeats every screen across a great hall.
            let level = (DUSK
                + dapple[(y.rem_euclid(NATIVE_H) * NATIVE_W + x.rem_euclid(NATIVE_W)) as usize]
                + local)
                .min(1.0);
            match cv.get(x, y) {
                Some(c) if c != BLACK => {
                    let c = if fire > 0.16 + bayer(x, y) * 0.24 {
                        warmed.get(&c).copied().unwrap_or(c)
                    } else {
                        c
                    };
                    let steps = ((1.0 - level) * STEPS + bayer(x, y)).floor().max(0.0) as usize;
                    cv.set(x, y, step_down(c, steps));
                }
                _ => {
                    let p = (local - 0.22) * 0.55;
                    if p > 0.0 && bayer(x, y) < p {
                        cv.put(x, y, if fire > local * 0.5 { lit } else { cool });
                    }
                }
            }
        }
    }
}

/// Stamp `im` with its feet at native `(x, y)`.
fn stand(cv: &mut Img, im: &Img, x: i32, y: i32) {
    cv.stamp(im, x - im.w / 2, y - im.h + 1);
}

/// Feet at a world point. A live frame keeps the fractional pixel as
/// ordered dither (`stamp_sub`), so a step smaller than one pixel still
/// changes the picture. A snapshot stays on the integer cell.
fn stand_at(cv: &mut Img, im: &Img, ux: f32, uy: f32, dy: i32) {
    // Same integer anchor as `stand` (`w / 2` truncates). The fractional
    // pixel is added after that, so an odd-width foe does not slide half
    // a pixel off the snapshot, and a whole pixel still matches `stamp`.
    let px = ux * UNIT;
    let py = uy * UNIT;
    let (x, y) = (px.floor() as i32, py.floor() as i32);
    if live_frame() {
        cv.stamp_sub(
            im,
            (x - im.w / 2) as f32 + (px - px.floor()),
            (y - im.h + 1 + dy) as f32 + (py - py.floor()),
        );
    } else {
        stand(cv, im, x, y + dy);
    }
}

/// A vent grille's top-left on its wall.
fn vent_at(room: &Room, side: usize, lane: f32) -> (i32, i32) {
    let l = (lane * UNIT) as i32;
    let (pw, ph) = room_px(room);
    match side {
        1 => (pw - 8, l - 5),
        3 => (2, l - 5),
        0 => (l - 3, 2),
        _ => (l - 3, ph - 12),
    }
}

/// Live hazards over the room: rattling and striking spikes, glowing vents,
/// a doorway about to pour in a wave, rocks coming down.
fn hazards(cv: &mut Img, run: &Run) {
    use crate::drive::together_shooter::hazards::{self as hz, Spikes};
    let tick = run.tick as u32;
    for trap in &run.traps {
        match *trap {
            Trap::Spikes { col, row, phase } => match hz::spikes(run.tick, phase) {
                Spikes::Down => {}
                state => cv.stamp(
                    &sprites::spikes_out(state == Spikes::Up, tick),
                    col * TILE,
                    row * TILE,
                ),
            },
            Trap::Vent { side, lane, phase } => {
                let beat = hz::vent_beat(run.tick, phase);
                let glow = if beat >= hz::VENT_PERIOD - hz::VENT_GLOW {
                    beat + hz::VENT_GLOW + 1 - hz::VENT_PERIOD
                } else if beat < 18 {
                    hz::VENT_GLOW
                } else {
                    0
                };
                if glow > 0 {
                    let (x, y) = vent_at(run.room(), side, lane);
                    cv.stamp(&sprites::vent(glow), x, y);
                }
            }
            Trap::Rockfall => {}
        }
    }
    if let Some((side, _)) = run.waves.gate {
        // The doorway burns: something is coming through.
        let (col, row) = doors(run.room())[side];
        let (w, h) = if side % 2 == 0 {
            (2 * TILE, TILE)
        } else {
            (TILE, 2 * TILE)
        };
        for y in 0..h {
            for x in 0..w {
                let n = super::ink::hash(x + col * TILE, y + row * TILE, tick / 3);
                if n.is_multiple_of(3) {
                    cv.put(
                        col * TILE + x,
                        row * TILE + y,
                        if n.is_multiple_of(7) {
                            '6'
                        } else if n.is_multiple_of(2) {
                            '@'
                        } else {
                            '7'
                        },
                    );
                }
            }
        }
    }
    for rock in &run.rocks {
        let (x, y) = at(rock.x, rock.y);
        if rock.fall > 0 {
            let (ring, height) = sprites::falling_rock(rock.fall, hz::ROCK_FALL);
            cv.stamp(&ring, x - ring.w / 2, y - ring.h / 2);
            let stone = sprites::rock();
            cv.stamp(&stone, x - stone.w / 2, y - stone.h - height);
        } else {
            let stone = sprites::rock();
            cv.stamp(&stone, x - stone.w / 2, y - stone.h / 2);
            let puff = sprites::dust(rock.dust);
            cv.stamp(&puff, x - puff.w / 2, y - puff.h / 2);
        }
    }
}

fn figures(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let room = run.room();
    // Side-on, a body's position is its middle: figures stand on its feet.
    let feet = if run.side_on() { FOOT } else { 0.0 };
    for item in &room.items {
        let (x, y) = at(item.x, item.y);
        let im = cards::floor_card(run.book.get(&item.card), tick);
        cv.stamp(&im, x - im.w / 2, y - im.h / 2);
    }
    runes::floor(cv, run);
    secrets::draught(cv, run);
    secrets::snibbet(cv, run);
    for hero in run.players.values().filter(|h| h.hp == 0) {
        let (x, y) = at(hero.x, hero.y + feet);
        stand(cv, &sprites::fallen(), x, y);
    }
    hireling::delve(cv, run);
    for enemy in &run.enemies {
        let mut im = match enemy.boss.and_then(|i| run.bosses.get(usize::from(i))) {
            Some(boss) => Img::from_rows(&boss.art.iter().map(String::as_str).collect::<Vec<_>>()),
            // A slime's size is its stage; a giant is drawn as big as it is.
            None if enemy.kind == EnemyKind::Slime => {
                enlarge(&sprites::enemy(enemy.kind, tick), enemy.radius() / 0.75)
            }
            None if enemy.radius() > enemy.kind.radius() * 1.1 => enlarge(
                &sprites::enemy(enemy.kind, tick),
                enemy.radius() / enemy.kind.radius(),
            ),
            None => sprites::enemy(enemy.kind, tick),
        };
        if enemy.frozen > 0 {
            im = ults::iced(&im);
        }
        if enemy.kind == EnemyKind::Warboar {
            // It faces where it means to go.
            let way = if enemy.stage >= 1 {
                enemy.dir.0
            } else {
                run.players
                    .values()
                    .filter(|h| h.hp > 0)
                    .map(|h| h.x - enemy.x)
                    .fold(0.0, |a: f32, b| if a.abs() > 0.0 { a } else { b })
            };
            if way < 0.0 {
                im = im.flip_h();
            }
            foes_marks(cv, run, enemy, tick);
        }
        let (x, y) = if !run.side_on() {
            at(enemy.x, enemy.y)
        } else if enemy.kind.flies() {
            // A flier hangs on its middle; a walker stands on its feet.
            let (x, y) = at(enemy.x, enemy.y);
            (x, y + im.h / 2)
        } else {
            at(enemy.x, enemy.y + feet)
        };
        if enemy.kind == EnemyKind::Dragon {
            // The lair's master turns toward the nearest knight.
            let left = run
                .players
                .values()
                .filter(|h| h.hp > 0)
                .min_by(|a, b| {
                    let d = |h: &&crate::drive::together_shooter::Hero| {
                        (h.x - enemy.x).hypot(h.y - enemy.y)
                    };
                    d(a).total_cmp(&d(b))
                })
                .is_some_and(|h| h.x < enemy.x);
            if left {
                im = im.flip_h();
            }
            cv.stamp(&im, x - im.w / 2, y - im.h * 2 / 3);
            continue;
        }
        if enemy.age < TELEGRAPH {
            // A visible warning under a solid silhouette: spawning is not
            // invisibility, including the first paused frame of a new run.
            let radius = (im.w.max(im.h) / 2 + 2) as f32;
            for step in 0..64 {
                let angle = step as f32 / 64.0 * std::f32::consts::TAU;
                cv.put(
                    x + (angle.cos() * radius) as i32,
                    y + (angle.sin() * radius * 0.45) as i32,
                    '7',
                );
            }
        }
        if live_frame() && !run.side_on() {
            stand_at(cv, &im, enemy.x, enemy.y, 0);
        } else {
            stand(cv, &im, x, y);
        }
    }
    // The light home, in a slain dragon's lair.
    if let Some((lx, ly)) = run.light {
        let (cx, cy) = at(lx, ly);
        for y in -14..=6 {
            for x in -12..=12 {
                let (fx, fy) = (x as f32 / 12.0, y as f32 / 14.0);
                let d = fx.hypot(fy);
                if d > 1.0 {
                    continue;
                }
                let k = 1.0 - d;
                if k > 0.7 {
                    cv.put(cx + x, cy + y, 'w');
                } else if bayer(cx + x, cy + y) < k * 1.3 {
                    cv.put(cx + x, cy + y, if k > 0.4 { '6' } else { '5' });
                }
            }
        }
        for m in 0..5 {
            let rise = ((tick + m * 11) % 40) as i32;
            cv.put(cx - 8 + (hash(m as i32, 3, 41) % 17) as i32, cy - rise, '6');
        }
        let sign = district_sign("THE WAY HOME");
        cv.stamp(&sign, cx - sign.w / 2, cy - 30);
    }
    // A shut privy says so; whoever is inside is out of sight.
    if let Some((px, py)) = room.privy()
        && run.players.values().any(|h| h.privy > 0)
    {
        let sign = sprites::occupied();
        let (x, y) = at(px, py);
        cv.stamp(&sign, x - sign.w / 2, y - 28 - sign.h);
    }
    // A knight in the saddle at the lists is drawn with their horse.
    let riding = run.joust.as_ref().map(|j| j.knight);
    for (&id, hero) in run
        .players
        .iter()
        .filter(|(id, h)| h.hp > 0 && h.privy == 0 && Some(**id) != riding)
    {
        let (x, y) = at(hero.x, hero.y + feet);
        let mut im = if let Some(avatar) = &hero.avatar {
            let mut im = Img::new(16, 16);
            for y in 0..16 {
                for x in 0..16 {
                    if let Some(&[r, g, b, a]) = avatar.pixels.get((y * 2 * 32 + x * 2) as usize)
                        && a >= 128
                    {
                        im.set(x, y, [r, g, b]);
                    }
                }
            }
            im
        } else if !hero.colours().is_empty() {
            sprites::knight_in(hero.colours())
        } else {
            sprites::knight(id)
        };
        if hero.frog() {
            // Hexed: a frog, for now.
            im = sprites::frog(tick);
        }
        if hero.chilled > 0 {
            // Chilled by the Lich: rimed over, like a frozen monster.
            im = ults::iced(&im);
        }
        if hero.aim_x < -0.3 {
            im = im.flip_h();
        }
        if hero.has_rune(crate::drive::together_shooter::runes::RuneKind::Invisibility) {
            // Under Invisibility: only half there.
            im = runes::veiled(&im);
        }
        if hero.stone {
            if hero.vigil {
                // The vigil's reach: a slow ring of light around the statue.
                let turn = run.tick as f32 / 90.0;
                for step in 0..48 {
                    let angle = turn + step as f32 / 48.0 * std::f32::consts::TAU;
                    cv.rect(
                        x + (angle.cos() * VIGIL_REACH * UNIT) as i32,
                        y + (angle.sin() * VIGIL_REACH * UNIT) as i32,
                        2,
                        2,
                        if step % 2 == 0 { '2' } else { '3' },
                    );
                }
            }
            stand(cv, &sprites::stone(&im), x, y);
            continue;
        }
        if let Some(left) = hero.rolling() {
            // Tucked and tumbling: the knight turns a full circle along the
            // roll, with dust kicked up behind.
            let quarter = (10 - left.min(10)) * 4 / 10;
            let mut turned = im.clone();
            for _ in 0..quarter {
                turned = sprites::quarter_turn(&turned);
            }
            stand_at(cv, &turned, hero.x, hero.y + feet, 2);
            let (rx, ry) = hero.roll_dir();
            let puff = sprites::dust(left.min(12));
            cv.stamp(
                &puff,
                x - puff.w / 2 - (rx * 10.0) as i32,
                y - puff.h / 2 - (ry * 6.0) as i32,
            );
            continue;
        }
        if hero.tossed > 0 {
            // Thrown by a Ravage: up on an arc, tumbling, and back down.
            use crate::drive::together_shooter::tide::TOSSED;
            let flight = (TOSSED - hero.tossed.min(TOSSED)) as f32 / TOSSED as f32;
            let lift = ((flight * std::f32::consts::PI).sin() * 10.0) as i32;
            let mut turned = im.clone();
            for _ in 0..(hero.tossed / 3) % 4 {
                turned = sprites::quarter_turn(&turned);
            }
            stand_at(cv, &turned, hero.x, hero.y + feet, -lift);
            continue;
        }
        // Blink while a hit's grace lasts.
        let blink = hero.invulnerable > 0 && hero.invulnerable < 24 && (tick / 2).is_multiple_of(2);
        if !blink {
            stand_at(cv, &im, hero.x, hero.y + feet, 0);
        }
        if hero.walling
            && let Some(((x1, y1), (x2, y2))) = hero.wall_span()
        {
            // The wall: a shimmering bar of light across the aim.
            let (a, b) = (at(x1, y1), at(x2, y2));
            let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
            for i in 0..=steps {
                let (wx, wy) = (a.0 + (b.0 - a.0) * i / steps, a.1 + (b.1 - a.1) * i / steps);
                let glint = (i + tick as i32 / 2) % 7 == 0;
                for (dx, dy, ink) in [
                    (0, 0, if glint { 'w' } else { '3' }),
                    (1, 0, '2'),
                    (0, 1, '2'),
                    (0, -1, '1'),
                ] {
                    cv.put(wx + dx, wy + dy, ink);
                }
            }
        }
        if hero.shielding {
            // The kite shield held up toward the aim.
            let shield = sprites::kite_shield();
            let (sx, sy) = (
                x + (hero.aim_x * 9.0) as i32,
                y - 7 + (hero.aim_y * 7.0) as i32,
            );
            cv.stamp(&shield, sx - shield.w / 2, sy - shield.h / 2);
        }
        // The knight's true hit point: one bright pixel.
        let (hx, hy) = at(hero.x, hero.y);
        cv.put(hx, hy, 'w');
        // A swing: the blade's arc, bright at its edge and fading inward.
        let blade = if hero.sword {
            Some((2.2, 130.0))
        } else {
            hero.forged
                .as_ref()
                .and_then(|w| w.melee)
                .map(|m| (m.reach, m.arc))
        };
        if hero.swing > 0
            && let Some((reach, arc)) = blade
        {
            let angle = hero.aim_y.atan2(hero.aim_x);
            let arc = arc.to_radians();
            // The sweep moves across the arc as the swing plays out.
            let sweep = 1.0 - hero.swing as f32 / 8.0;
            for (depth, ink) in [(1.0, 'w'), (0.85, '3'), (0.7, '2')] {
                for i in 0..40 {
                    let k = i as f32 / 39.0;
                    if k > sweep + 0.35 {
                        continue;
                    }
                    let theta = angle + arc * (k - 0.5);
                    let (sx, sy) = at(
                        hero.x + theta.cos() * reach * depth,
                        hero.y + theta.sin() * reach * depth,
                    );
                    cv.put(sx, sy, ink);
                    cv.put(sx + 1, sy, ink);
                }
            }
        }
        // The aim: a short tick ahead of the knight's chest.
        let (ax, ay) = (x as f32, (y - 7) as f32);
        for r in [10.0, 11.0] {
            cv.put(
                (ax + hero.aim_x * r).round() as i32,
                (ay + hero.aim_y * r).round() as i32,
                '2',
            );
        }
        if hero.bomb_cooldown > BOMB_REARM - 12 {
            let age = (BOMB_REARM - hero.bomb_cooldown) as f32;
            let r = (3.0 + age * 3.2) * UNIT;
            let n = (r * 3.0) as i32;
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                cv.put(
                    x + (a.cos() * r).round() as i32,
                    y + (a.sin() * r).round() as i32,
                    if k % 2 == 0 { '6' } else { '5' },
                );
            }
        }
    }
    runes::auras(cv, run);
    // Shots are drawn at twice their sprite size: a hairline reads as noise
    // at speed, a two-pixel stroke reads as an arrow.
    let mut mark = Img::new(25, 25);
    for shot in &run.projectiles {
        let (x, y) = at(shot.x, shot.y);
        mark.clear_all();
        if shot.empowered {
            // Through a friend's wall: a gold halo around the shot.
            for (dx, dy) in [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
            ] {
                mark.put(12 + dx, 12 + dy, '6');
            }
        }
        if let Some((colour, shape)) = &shot.look {
            sprites::forged_shot(&mut mark, 12, 12, colour, shape);
        } else {
            sprites::shot(&mut mark, (12, 12), (shot.vx, shot.vy), shot.kind);
        }
        for my in 0..mark.h {
            for mx in 0..mark.w {
                if let Some(c) = mark.get(mx, my) {
                    let (px, py) = (x + (mx - 12) * 2, y + (my - 12) * 2);
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        cv.set(px + dx, py + dy, c);
                    }
                }
            }
        }
    }
}

/// Bursts and granted wishes: an ember ring opening where a handgonne ball
/// burst; an arcane ring and rising motes round a knight whose wish landed.
fn flourishes(cv: &mut Img, run: &Run) {
    use crate::drive::together_shooter::{BLAST_TICKS, HZ};
    for &(x, y, left) in &run.blasts {
        let (cx, cy) = at(x, y);
        let k = f32::from(BLAST_TICKS - left) / f32::from(BLAST_TICKS);
        let r = 4.0 + k * 18.0;
        let n = (r * 4.0) as i32;
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let ink = match (i + left as i32) % 3 {
                0 => '6',
                1 => '@',
                _ => '7',
            };
            if !hash(i, left as i32, 31).is_multiple_of(3) {
                let ink = if k > 0.6 { '8' } else { ink };
                for dr in [0.0, 1.0] {
                    cv.put(
                        cx + (a.cos() * (r + dr)) as i32,
                        cy + (a.sin() * (r + dr) * 0.6) as i32,
                        ink,
                    );
                }
            }
        }
    }
    for &(id, at_tick) in &run.boons {
        let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0 && h.privy == 0) else {
            continue;
        };
        let age = run.tick.saturating_sub(at_tick) as f32 / HZ as f32;
        let feet = if run.side_on() {
            crate::drive::together_shooter::ledge::FOOT
        } else {
            0.0
        };
        let (cx, cy) = at(hero.x, hero.y + feet);
        let r = 6.0 + age * 6.0;
        for i in 0..36 {
            let a = i as f32 / 36.0 * std::f32::consts::TAU + age * 4.0;
            if i % 3 != 0 {
                cv.put(
                    cx + (a.cos() * r) as i32,
                    cy + (a.sin() * r * 0.45) as i32,
                    if age < 0.5 { '3' } else { '2' },
                );
            }
        }
        for m in 0..6 {
            let rise = (age * 30.0) as i32 + m * 3;
            let x = cx - 8 + (hash(m, id as i32, 41) % 17) as i32;
            cv.put(x, cy - 4 - rise, if m % 2 == 0 { '3' } else { '6' });
        }
    }
}

/// Each knight's morningstars and the chains that hold them.
fn orbits(cv: &mut Img, run: &Run) {
    let ball = sprites::morningstar();
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.privy == 0) {
        for (bx, by) in run.orbits(id) {
            let (hx, hy) = at(hero.x, hero.y - 0.6);
            let (x, y) = at(bx, by);
            for k in 1..6 {
                cv.put(hx + (x - hx) * k / 6, hy + (y - hy) * k / 6, 'G');
            }
            cv.stamp(&ball, x - ball.w / 2, y - ball.h / 2);
        }
    }
}

/// A chain hit's sparks: a jagged arc from monster to monster, bright at
/// first and thinning as it fades.
fn sparks(cv: &mut Img, run: &Run) {
    for spark in &run.sparks {
        let (a, b) = (at(spark.from.0, spark.from.1), at(spark.to.0, spark.to.1));
        let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
        let seed = hash(a.0, b.1, (run.tick / 2) as u32);
        for i in 0..=steps {
            let k = i as f32 / steps as f32;
            // A zigzag across the line, worst in the middle.
            let jag = ((hash(i / 3, seed as i32, 7) % 5) as i32 - 2)
                * (1 - (2.0 * k - 1.0).abs().round() as i32 + 1);
            let x = a.0
                + (b.0 - a.0) * i / steps
                + if (b.1 - a.1).abs() > (b.0 - a.0).abs() {
                    jag
                } else {
                    0
                };
            let y = a.1
                + (b.1 - a.1) * i / steps
                + if (b.1 - a.1).abs() > (b.0 - a.0).abs() {
                    0
                } else {
                    jag
                };
            cv.put(x, y, if spark.ttl > 3 { 'w' } else { '3' });
            if spark.ttl > 3 {
                cv.put(x + 1, y, '2');
            }
        }
    }
}

/// A finished run's verdict, on a board in the middle of the view.
fn verdict(cv: &mut Img, run: &Run) {
    let text = match run.phase {
        Phase::Won => "VICTORY",
        Phase::Wiped => "FALLEN",
        _ => return sanctuary_sign(cv, run),
    };
    let sign = district_sign(text);
    cv.stamp(&sign, (NATIVE_W - sign.w) / 2, (NATIVE_H - sign.h) / 2);
}

/// For a few seconds after a Sanctuary opens: its name, and what it is for.
fn sanctuary_sign(cv: &mut Img, run: &Run) {
    let hz = u64::from(crate::drive::together_shooter::HZ);
    // A room won opens a wish window: a short sign says so.
    let in_sanctuary = run.room().kind == crate::drive::together_shooter::RoomKind::Sanctuary;
    let (at, title, hint, last) = match (run.hallowed, run.window.as_ref()) {
        // Only in the Sanctuary itself: a knight who walks straight out
        // leaves its sign behind.
        (Some(at), _) if in_sanctuary && run.tick.saturating_sub(at) < 4 * hz => {
            (at, "SANCTUARY", "T: ONE WISH", 4)
        }
        (_, Some(w)) if run.room().kind != crate::drive::together_shooter::RoomKind::Sanctuary => {
            (w.opened, "A WISH", "T TO MAKE IT", 2)
        }
        _ => return,
    };
    if run.tick.saturating_sub(at) >= last * hz {
        return;
    }
    let sign = district_sign(title);
    let y = NATIVE_H / 2 - sign.h;
    cv.stamp(&sign, (NATIVE_W - sign.w) / 2, y);
    let w = super::ink::text_width(hint);
    let mut plate = Img::new(w + 6, 11);
    plate.rect(0, 0, w + 6, 11, 'K');
    plate.text(3, 2, hint, '6');
    cv.stamp(&plate, (NATIVE_W - plate.w) / 2, y + sign.h + 2);
}

/// Where the camera's view starts in a room's native pixels: centred on
/// `focus` (or between the living knights), kept inside the room. An
/// ordinary room is exactly one view, so its camera never moves.
pub(crate) fn camera(run: &Run, focus: Option<u32>) -> (i32, i32) {
    let (pw, ph) = room_px(run.room());
    let alive = |h: &&crate::drive::together_shooter::Hero| h.hp > 0;
    let watched: Vec<(f32, f32)> = match focus.and_then(|id| run.players.get(&id)).filter(alive) {
        Some(hero) => vec![(hero.x, hero.y)],
        None => run
            .players
            .values()
            .filter(alive)
            .map(|h| (h.x, h.y))
            .collect(),
    };
    let (sx, sy) = watched
        .iter()
        .fold((0.0, 0.0), |(ax, ay), &(x, y)| (ax + x, ay + y));
    let n = watched.len().max(1) as f32;
    let (cx, cy) = if watched.is_empty() {
        (pw / 2, ph / 2)
    } else {
        at(sx / n, sy / n)
    };
    (
        (cx - NATIVE_W / 2).clamp(0, (pw - NATIVE_W).max(0)),
        (cy - NATIVE_H / 2).clamp(0, (ph - NATIVE_H).max(0)),
    )
}

/// Where this seat's knight is stamped in its own frame. The page slides
/// that sprite on a delayed route so the key is on screen before the next
/// picture can return. `follows` means the camera tracks the knight, so the
/// page scrolls the view instead of walking the sprite through a fixed room.
pub(crate) fn focus_stamp_box(run: &Run, id: u32) -> Option<(i32, i32, i32, i32, bool)> {
    let hero = run.players.get(&id).filter(|h| h.hp > 0 && h.privy == 0)?;
    if run.side_on() {
        return None;
    }
    let (ox, oy) = camera(run, Some(id));
    let (pw, ph) = room_px(run.room());
    let follows = pw > NATIVE_W || ph > NATIVE_H;
    let px = hero.x * UNIT;
    let py = hero.y * UNIT;
    // The kit knight is 16×16, feet on the same anchor `stand_at` uses.
    let (w, h) = (16, 16);
    let x = px.floor() as i32 - w / 2 - ox;
    let y = py.floor() as i32 - h + 1 - oy;
    Some((x, y, w, h, follows))
}

/// One view of `full` from `(ox, oy)`.
fn crop(full: &Img, (ox, oy): (i32, i32)) -> Img {
    Img::from_view(full, ox, oy, NATIVE_W, NATIVE_H)
}

/// What the view cannot show: a mark on its edge toward each monster (red)
/// and each knight (their colours) out of sight in a great hall.
fn beyond(over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    let (cx, cy) = ((ox + NATIVE_W / 2) as f32, (oy + NATIVE_H / 2) as f32);
    let mut mark = |x: i32, y: i32, ink: char| {
        if (ox..ox + NATIVE_W).contains(&x) && (oy..oy + NATIVE_H).contains(&y) {
            return;
        }
        let (dx, dy) = (x as f32 - cx, y as f32 - cy);
        let k = ((NATIVE_W / 2 - 4) as f32 / dx.abs().max(0.01))
            .min((NATIVE_H / 2 - 4) as f32 / dy.abs().max(0.01));
        let (ex, ey) = (
            (NATIVE_W / 2) as f32 + dx * k,
            (NATIVE_H / 2) as f32 + dy * k,
        );
        for (px, py) in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)] {
            over.put(ex as i32 + px, ey as i32 + py, ink);
        }
    };
    for enemy in &run.enemies {
        let (x, y) = at(enemy.x, enemy.y);
        mark(x, y, '7');
    }
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0) {
        let (x, y) = at(hero.x, hero.y);
        mark(x, y, ['5', '1', 'A', '4'][(id as usize).clamp(1, 4) - 1]);
    }
}

/// One view of the room, 16 pixels to a tile: lit scenery, and the figures
/// over it on a layer of their own.
/// `legible`: the frame is drawn at native size or larger, so the 5x7 hand
/// of in-world boards can be read (a reduced frame leaves them to the HUD).
fn layers(run: &Run, focus: Option<u32>, legible: bool) -> (Img, Img) {
    debug_assert_eq!(
        (NATIVE_W, NATIVE_H),
        at(WIDTH, HEIGHT),
        "a tile is two arena units"
    );
    let (pw, ph) = room_px(run.room());
    let mut over = Img::new(pw, ph);
    hazards(&mut over, run);
    let fortune_hall = run.room().kind == RoomKind::Fortune;
    let in_yard = run.room().kind == RoomKind::Yard;
    let in_hall = run.room().kind == RoomKind::Trophies;
    let in_tavern = run.room().kind == RoomKind::Tavern;
    if run.at_home_now() {
        if fortune_hall {
            fortune::figures(&mut over, run);
        } else if in_yard {
            yard::figures(&mut over, run);
            lessons::figures(&mut over, run);
        } else if in_hall {
            trophies::figures(&mut over, run);
        } else if in_tavern {
            tavern::figures(&mut over, run);
        } else if run.room().kind == RoomKind::Home {
            home::figures(&mut over, run);
            rescues::residents(&mut over, run);
            hireling::at_the_stair(&mut over, run);
        }
        match run.room().kind {
            RoomKind::Stables => joust::stables(&mut over, run),
            RoomKind::Lists => joust::lists(&mut over, run, legible),
            _ => {}
        }
        world::entrance_ring(&mut over, run);
    }
    if matches!(
        run.room().kind,
        RoomKind::MineHead | RoomKind::KingsHall | RoomKind::Gate | RoomKind::Forge
    ) {
        barony::plates(&mut over, run);
        barony::folk(&mut over, run);
    }
    ults::under(&mut over, run);
    foes_hazards(&mut over, run);
    tide::ravages(&mut over, run);
    void::holes(&mut over, run);
    rescues::cage(&mut over, run);
    figures(&mut over, run);
    tallow(&mut over, run);
    hooks(&mut over, run);
    fan_boxes(&mut over, run);
    sparks(&mut over, run);
    orbits(&mut over, run);
    flourishes(&mut over, run);
    ults::over(&mut over, run);
    if run.at_home_now() && legible {
        if fortune_hall {
            fortune::boards(&mut over, run);
        } else if in_yard {
            yard::boards(&mut over, run);
            lessons::boards(&mut over, run);
        } else if in_hall {
            trophies::boards(&mut over, run);
            tavern::wing_ledger(&mut over, run);
        } else if in_tavern {
            tavern::boards(&mut over, run);
        } else if run.room().kind == RoomKind::Home {
            home::boards(&mut over, run);
        }
        if run.room().kind.in_world() || run.room().kind == RoomKind::Home {
            world::boards(&mut over, run);
        }
        match run.room().kind {
            RoomKind::Stables => joust::stables_boards(&mut over, run),
            RoomKind::Lists => joust::lists_boards(&mut over, run),
            _ => {}
        }
    }
    if legible
        && matches!(
            run.room().kind,
            RoomKind::MineHead | RoomKind::KingsHall | RoomKind::Forge
        )
    {
        barony::boards(&mut over, run);
    }
    let banner = legible.then(|| run.banner_now()).flatten();
    let origin = camera(run, focus);
    // A guardian's fall, a bomb: the view jolts, settling as it fades.
    let origin = if run.shake > 0 {
        let k = run.shake.min(12) as i32 / 2;
        let h = super::ink::hash(run.tick as i32, run.shake as i32, 91);
        (
            origin.0 + (h % (2 * k as u32 + 1)) as i32 - k,
            origin.1 + ((h >> 8) % (2 * k as u32 + 1)) as i32 - k,
        )
    } else {
        origin
    };
    let mut over = crop(&over, origin);
    let mut scenery = crop(&scenery_cached(run), origin);
    if run.mode == crate::drive::together_shooter::fortune::Mode::LightsOut && !run.at_home_now() {
        lights_out(&mut scenery, &mut over, run, origin);
    }
    beyond(&mut over, run, origin);
    verdict(&mut over, run);
    if let Some(feat) = banner {
        achievement_banner(&mut over, run, feat);
    } else if legible {
        dare_board(&mut over, run);
    }
    (scenery, over)
}

/// A warboar's tells: the line it marks before a charge, and the stars
/// over its head while it stands dazed.
fn foes_marks(cv: &mut Img, run: &Run, enemy: &crate::drive::together_shooter::Enemy, tick: u32) {
    let (x, y) = at(enemy.x, enemy.y);
    match enemy.stage {
        1 => {
            let (dx, dy) = enemy.dir;
            let (px, py) = (-dy, dx);
            for k in 4..40 {
                if (k + tick as i32 / 2) % 5 < 2 {
                    continue;
                }
                let s = k as f32 * 2.2;
                for w in [-1.0f32, 0.0, 1.0] {
                    cv.put(
                        x + (dx * s + px * w) as i32,
                        y + (dy * s + py * w) as i32,
                        if w == 0.0 { '7' } else { '8' },
                    );
                }
            }
        }
        3 => {
            for k in 0..3 {
                let a = tick as f32 * 0.3 + k as f32 * 2.1;
                cv.put(
                    x + (a.cos() * 6.0) as i32,
                    y - 12 + (a.sin() * 2.0) as i32,
                    '6',
                );
            }
        }
        _ => {}
    }
    let _ = run;
}

/// Kegs on the floor and bombs in the air, with the ring where each will go.
fn foes_hazards(cv: &mut Img, run: &Run) {
    use crate::drive::together_shooter::foes::{KEG_ARMED, LOB_FALL};
    use crate::drive::together_shooter::hunters::{HOOK_RANGE, WEB_REACH};
    let tick = run.tick as u32;
    // Webs: spokes and rings of silk, thinning as they fade.
    for web in &run.webs {
        let (x, y) = at(web.x, web.y);
        let r = WEB_REACH * 8.0;
        let thin = if web.left < 30 { 2 } else { 1 };
        for spoke in 0..8 {
            let a = spoke as f32 * std::f32::consts::FRAC_PI_4 + 0.2;
            for k in (2..r as i32).step_by(2 * thin) {
                let k = k as f32;
                cv.put(
                    x + (a.cos() * k) as i32,
                    y + (a.sin() * k * 0.6) as i32,
                    'J',
                );
            }
        }
        for ring in [0.35f32, 0.65, 0.95] {
            let rr = r * ring;
            let n = (rr * 2.0) as i32;
            for i in (0..n).step_by(2 * thin) {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                cv.put(
                    x + (a.cos() * rr) as i32,
                    y + (a.sin() * rr * 0.6) as i32,
                    'h',
                );
            }
        }
    }
    // The Pit Tyrant's slam gathering: a red ring, filling in as it comes.
    for slam in &run.slams {
        use crate::drive::together_shooter::pit::{SLAM_REACH, SLAM_WINDUP};
        let (x, y) = at(slam.x, slam.y);
        let k = 1.0 - slam.left as f32 / SLAM_WINDUP as f32;
        let r = SLAM_REACH * 8.0;
        let n = 72;
        let step = if k > 0.66 {
            1
        } else if k > 0.33 {
            2
        } else {
            3
        };
        for i in (0..n).step_by(step) {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            for rr in [r, r - 1.0] {
                cv.put(
                    x + (a.cos() * rr) as i32,
                    y + (a.sin() * rr * 0.6) as i32,
                    '7',
                );
            }
        }
    }
    // The Flesher's stare: his hook's line, red on the floor.
    for flesher in run
        .enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::Flesher && e.stage == 1 && e.hp > 0)
    {
        let (x, y) = at(flesher.x, flesher.y);
        let reach = HOOK_RANGE * 8.0;
        // Two rows of marching dashes, either side of the hook's path.
        let (nx, ny) = (-flesher.dir.1, flesher.dir.0);
        for k in 10..reach as i32 {
            if (k as u32 + tick).rem_euclid(5) > 2 {
                continue;
            }
            let k = k as f32;
            for side in [-1.5f32, 1.5] {
                cv.put(
                    x + (flesher.dir.0 * k + nx * side) as i32,
                    y + (flesher.dir.1 * k + ny * side) as i32,
                    '7',
                );
            }
        }
    }
    for keg in &run.kegs {
        let (x, y) = at(keg.x, keg.y);
        let im = sprites::keg(keg.fuse, keg.fuse < KEG_ARMED, tick);
        cv.stamp(&im, x - im.w / 2, y - im.h / 2);
        if keg.fuse < KEG_ARMED {
            let r = 22.0;
            let n = 60;
            for i in (0..n).step_by(3) {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                cv.put(
                    x + (a.cos() * r) as i32,
                    y + (a.sin() * r * 0.6) as i32,
                    '8',
                );
            }
        }
    }
    for lob in &run.lobs {
        let (tx, ty) = at(lob.x, lob.y);
        let k = 1.0 - lob.fall as f32 / LOB_FALL as f32;
        let r = 19.0;
        let n = 56;
        for i in (0..n).step_by(2) {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            cv.put(
                tx + (a.cos() * r) as i32,
                ty + (a.sin() * r * 0.6) as i32,
                '7',
            );
        }
        let (fx, fy) = at(lob.from.0, lob.from.1);
        let arc = (k * std::f32::consts::PI).sin() * 26.0;
        let (bx, by) = (
            fx + ((tx - fx) as f32 * k) as i32,
            fy + ((ty - fy) as f32 * k) as i32 - arc as i32,
        );
        let im = sprites::bomb(tick);
        cv.stamp(&im, bx - im.w / 2, by - im.h / 2);
    }
}

/// Lady Tallow at her knight's heel, about her business.
fn tallow(cv: &mut Img, run: &Run) {
    let Some(cat) = &run.cat else {
        return;
    };
    let tick = run.tick as u32;
    let hissing = run.tick.saturating_sub(cat.hissed) < 20 && cat.hissed > 0;
    let im = sprites::tallow(cat.moving, hissing, cat.carrying.is_some(), tick);
    let im = if cat.left { im.flip_h() } else { im };
    let (x, y) = at(cat.x, cat.y);
    cv.stamp(&im, x - im.w / 2, y - im.h + 2);
}

/// The Flesher's hooks in the air: the chain back to his hand, and the
/// hook at its end (with whoever it caught).
fn hooks(cv: &mut Img, run: &Run) {
    for hook in &run.hooks {
        let Some(flesher) = run.enemies.iter().find(|e| e.id == hook.by) else {
            continue;
        };
        let (bx, by) = at(flesher.x, flesher.y);
        let (hx, hy) = at(hook.x, hook.y);
        let (dx, dy) = ((hx - bx) as f32, (hy - by) as f32);
        let length = dx.hypot(dy).max(1.0);
        for k in (6..length as i32).step_by(2) {
            let t = k as f32 / length;
            let ink = if (k / 2) % 2 == 0 { 'h' } else { 'J' };
            cv.put(bx + (dx * t) as i32, by + (dy * t) as i32, ink);
        }
        let im = sprites::hook();
        cv.stamp(&im, hx - im.w / 2, hy - im.h / 2);
    }
}

/// Fan boxes floating down: a gold ring that tightens where each will
/// land, and the box swaying down on its parachute.
fn fan_boxes(cv: &mut Img, run: &Run) {
    use crate::drive::together_shooter::audience::BOX_FALL;
    let tick = run.tick as u32;
    for parcel in &run.fan_boxes {
        let (x, y) = at(parcel.x, parcel.y);
        // 1 at the top of the fall, 0 on the ground.
        let k = parcel.fall as f32 / BOX_FALL as f32;
        let r = 5.0 + 10.0 * k;
        let n = 28;
        for i in (0..n).step_by(2) {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            cv.put(
                x + (a.cos() * r) as i32,
                y + (a.sin() * r * 0.55) as i32,
                '5',
            );
        }
        let sway = (parcel.fall as f32 * 0.18).sin() * 3.0 * k;
        let im = sprites::fan_box(tick);
        let (bx, by) = (x + sway as i32, y - (k * 72.0) as i32);
        cv.stamp(&im, bx - im.w / 2, by - im.h + 2);
    }
}

/// A new achievement, the way the Herald announces them: a board across
/// the top of the view, the name, the Herald's line, and the box.
fn achievement_banner(cv: &mut Img, run: &Run, feat: &crate::drive::together_shooter::feats::Feat) {
    use super::ink::text_width;
    // The Herald's line, wrapped to the board.
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in feat.says.to_uppercase().split_whitespace() {
        if !line.is_empty() && text_width(&format!("{line} {word}")) > 300 {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    let rows: Vec<(String, char)> = std::iter::once(("NEW ACHIEVEMENT!".to_string(), '6'))
        .chain(std::iter::once((feat.name.to_uppercase(), '9')))
        .chain(lines.into_iter().map(|l| (l, 'h')))
        .chain(std::iter::once((
            format!("REWARD: {}", feat.tier.name().to_uppercase()),
            '5',
        )))
        .collect();
    let w = rows.iter().map(|(t, _)| text_width(t)).max().unwrap_or(0) + 12;
    let h = rows.len() as i32 * 9 + 7;
    let mut board = Img::new(w, h);
    board.rect(0, 0, w, h, 'K');
    board.frame(0, 0, w, h, '4');
    board.frame(1, 1, w - 2, h - 2, 'a');
    for (i, (text, ink)) in rows.iter().enumerate() {
        board.text((w - text_width(text)) / 2, 4 + i as i32 * 9, text, *ink);
    }
    // It drops in from the top and settles.
    let (at, _) = run.banner.as_ref().expect("a banner is up");
    let age = run.tick.saturating_sub(*at) as i32;
    let y = (age * 3 - h).min(4);
    cv.stamp(&board, (NATIVE_W - w) / 2, y);
}

/// Fortune's dare as the party arrives on a floor: her red board drops in
/// from the top, the dare on it, and settles.
fn dare_board(cv: &mut Img, run: &Run) {
    use super::ink::text_width;
    use crate::drive::together_shooter::dares::CALLED;
    let Some(dare) = run
        .dare
        .as_ref()
        .filter(|d| run.tick.saturating_sub(d.since) < CALLED)
    else {
        return;
    };
    let rows = [
        ("FORTUNE DARES YOU".to_string(), '6'),
        (dare.kind.name().to_uppercase(), '9'),
        (dare.kind.says().to_uppercase(), 'h'),
        ("HER PURSE: GOLD AND A GEM EACH".to_string(), '5'),
    ];
    let w = rows.iter().map(|(t, _)| text_width(t)).max().unwrap_or(0) + 12;
    let h = rows.len() as i32 * 9 + 7;
    let mut board = Img::new(w, h);
    board.rect(0, 0, w, h, 'K');
    board.frame(0, 0, w, h, '7');
    board.frame(1, 1, w - 2, h - 2, '9');
    for (i, (text, ink)) in rows.iter().enumerate() {
        board.text((w - text_width(text)) / 2, 4 + i as i32 * 9, text, *ink);
    }
    let age = run.tick.saturating_sub(dare.since) as i32;
    let y = (age * 3 - h).min(4);
    cv.stamp(&board, (NATIVE_W - w) / 2, y);
}

/// Lights Out: beyond each knight's lantern and the room's fires the
/// delve is dark. Scenery sinks to black paper; figures sink to faint
/// silhouettes. Signal inks — eyes, embers, flames, shots — still glow.
fn lights_out(scenery: &mut Img, over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    const LANTERN: f32 = 58.0;
    let mut lamps: Vec<(f32, f32, f32)> = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .map(|h| {
            let (x, y) = at(h.x, h.y);
            ((x - ox) as f32, (y - oy - 6) as f32, LANTERN)
        })
        .collect();
    let room = run.room();
    for (side, &(col, row)) in doors(room).iter().enumerate() {
        if room.doors[side] {
            lamps.push((
                (col * TILE + TILE - ox) as f32,
                (row * TILE + TILE - oy) as f32,
                26.0,
            ));
        }
    }
    let (pw, ph) = room_px(room);
    for (cx, cy) in [(8, 8), (pw - 8, 8), (8, ph - 8), (pw - 8, ph - 8)] {
        lamps.push(((cx - ox) as f32, (cy - oy) as f32, 30.0));
    }
    for y in 0..scenery.h {
        for x in 0..scenery.w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let lit = lamps
                .iter()
                .map(|&(lx, ly, r)| 1.0 - (px - lx).hypot(py - ly) / r)
                .fold(0.0f32, f32::max);
            if lit > 0.35 {
                continue;
            }
            // A dithered edge where the lantern's light gives out.
            let dark = ((0.35 - lit) * 16.0 + bayer(x, y) * 2.0) as usize;
            if let Some(c) = scenery.get(x, y)
                && c != BLACK
                && !is_signal(c)
            {
                scenery.set(x, y, step_down(c, dark.min(7)));
            }
            if let Some(c) = over.get(x, y)
                && c != BLACK
                && !is_signal(c)
            {
                over.set(x, y, step_down(c, dark.min(4)));
            }
        }
    }
}

/// A sprite drawn `k` times its size, nearest-neighbour: a giant.
fn enlarge(im: &Img, k: f32) -> Img {
    let (w, h) = (
        (im.w as f32 * k).round() as i32,
        (im.h as f32 * k).round() as i32,
    );
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get((x as f32 / k) as i32, (y as f32 / k) as i32) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// One view composed at native size.
#[cfg_attr(not(test), allow(dead_code))]
fn native(run: &Run) -> Img {
    let (mut cv, over) = layers(run, None, true);
    cv.stamp(&over, 0, 0);
    cv
}

/// The room the party stands in, as a `w`x`h` pixel frame.
pub(crate) fn frame(run: &Run, w: i32, h: i32) -> Img {
    frame_for(run, w, h, None)
}

/// The view a given knight's camera shows (`None`: between the knights).
pub(crate) fn frame_for(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    let mut out = Img::black(w.max(0), h.max(0));
    if w < 12 || h < 8 {
        return out;
    }
    let legible = w >= NATIVE_W && h >= NATIVE_H;
    let (scenery, over) = layers(run, focus, legible);
    Field::fit(w, h).blit(&scenery, &over, &mut out);
    out
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/overworld__arena_tests.rs"]
mod tests;

/// Snapshot only on worker admission; the expensive compose is lazy.
pub(crate) struct LazyFrame {
    run: Run,
    w: u32,
    h: u32,
    focus: Option<u32>,
    rgba: std::sync::OnceLock<Vec<u8>>,
}
impl LazyFrame {
    pub(crate) fn new(run: Run, w: u32, h: u32, focus: Option<u32>) -> Self {
        Self {
            run,
            w,
            h,
            focus,
            rgba: std::sync::OnceLock::new(),
        }
    }
}
impl AsRef<[u8]> for LazyFrame {
    fn as_ref(&self) -> &[u8] {
        self.rgba.get_or_init(|| {
            frame_for(&self.run, self.w as i32, self.h as i32, self.focus).rgba_bytes()
        })
    }
}
